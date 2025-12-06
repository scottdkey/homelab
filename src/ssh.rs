use crate::config::{self, EnvConfig};
use anyhow::{Context, Result};
use std::io::{self, Write};
use std::process::{Command, Stdio};
use which;

pub fn remove_ssh_host_key(host: &str) -> Result<()> {
    println!("Removing host key for {} from known_hosts...", host);

    let status = Command::new("ssh-keygen").args(["-R", host]).status()?;

    if status.success() {
        println!("✓ Removed host key for {}", host);
        Ok(())
    } else {
        anyhow::bail!("Failed to remove host key for {}", host);
    }
}

pub fn prompt_remove_host_key(host: &str) -> Result<bool> {
    print!("Remove host key for {} from known_hosts? [y/N]: ", host);
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let response = input.trim().to_lowercase();
    Ok(response == "y" || response == "yes")
}

fn connect_ssh_key_based(host: &str, user: Option<&str>, ssh_args: &[String]) -> Result<()> {
    // First, test if key-based auth works (silently)
    let host_str = if let Some(u) = user {
        format!("{}@{}", u, host)
    } else {
        host.to_string()
    };

    // Quick test to see if key-based auth is available
    let test_output = Command::new("ssh")
        .args([
            "-o",
            "ConnectTimeout=1",
            "-o",
            "BatchMode=yes",
            "-o",
            "PreferredAuthentications=publickey",
            "-o",
            "PasswordAuthentication=no",
            "-o",
            "StrictHostKeyChecking=no",
            &host_str,
            "echo",
            "test",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output();

    // If test fails, key-based auth isn't available
    if let Ok(result) = test_output {
        if !result.status.success() {
            anyhow::bail!("Key-based authentication not available");
        }
    } else {
        anyhow::bail!("Key-based authentication not available");
    }

    // Key-based auth works, now actually connect
    let mut cmd = Command::new("ssh");

    // Use key-based authentication only (no password prompts)
    cmd.args([
        "-o",
        "PreferredAuthentications=publickey",
        "-o",
        "PasswordAuthentication=no",
        "-o",
        "StrictHostKeyChecking=no",
    ]);

    cmd.arg(&host_str);

    if !ssh_args.is_empty() {
        cmd.args(ssh_args);
    }

    // Allow interactive output for the actual connection
    cmd.stdin(Stdio::inherit());
    cmd.stdout(Stdio::inherit());
    cmd.stderr(Stdio::inherit());

    // Execute SSH - this will open the interactive session
    let status = cmd.status()?;

    // If SSH exits successfully, we're done
    if status.success() {
        std::process::exit(0);
    } else {
        // SSH connection failed, return error so we can try password-based auth
        anyhow::bail!(
            "SSH connection failed with exit code: {}",
            status.code().unwrap_or(1)
        );
    }
}

pub fn connect_ssh(host: &str, user: Option<&str>, ssh_args: &[String]) -> Result<()> {
    let mut cmd = Command::new("ssh");

    // Add options to allow password authentication (fallback)
    cmd.args([
        "-o",
        "PreferredAuthentications=keyboard-interactive,password,publickey",
        "-o",
        "StrictHostKeyChecking=no",
    ]);

    // Build host string with optional user
    let host_str = if let Some(u) = user {
        format!("{}@{}", u, host)
    } else {
        host.to_string()
    };

    cmd.arg(&host_str);

    if !ssh_args.is_empty() {
        cmd.args(ssh_args);
    }

    // Allow interactive authentication (password prompts, etc.)
    cmd.stdin(Stdio::inherit());
    cmd.stdout(Stdio::inherit());
    cmd.stderr(Stdio::inherit());

    // Execute SSH - this will block and allow password prompts
    let status = cmd.status()?;

    // If SSH exits successfully, we're done
    if status.success() {
        std::process::exit(0);
    } else {
        // SSH connection failed, return error so we can try next host
        anyhow::bail!(
            "SSH connection failed with exit code: {}",
            status.code().unwrap_or(1)
        );
    }
}

pub fn copy_ssh_key(
    host: &str,
    server_user: Option<&str>,
    target_user: Option<&str>,
) -> Result<()> {
    // Determine server username (to SSH into the server)
    let default_server_user = config::get_default_username();
    let server_username = if let Some(u) = server_user {
        u.to_string()
    } else {
        print!(
            "Server username to SSH into {} (press Enter for '{}'): ",
            host, default_server_user
        );
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let input_username = input.trim();
        if input_username.is_empty() {
            default_server_user
        } else {
            input_username.to_string()
        }
    };

    // Determine target username (where to install the key)
    let target_username = if let Some(u) = target_user {
        u.to_string()
    } else {
        // Default to the same as server username, but allow override
        let default_target = server_username.clone();
        print!(
            "Target username to install key for (press Enter for '{}'): ",
            default_target
        );
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let input_username = input.trim();
        if input_username.is_empty() {
            default_target
        } else {
            input_username.to_string()
        }
    };

    println!(
        "Copying SSH public key to {}@{} (installing for user: {})...",
        server_username, host, target_username
    );

    // Find the public key file using config module
    let home = config::get_home_dir()?;
    let home_str = home.to_string_lossy().to_string();

    let pubkey_paths = [
        format!("{}/.ssh/id_rsa.pub", home_str),
        format!("{}/.ssh/id_ed25519.pub", home_str),
        format!("{}/.ssh/id_ecdsa.pub", home_str),
    ];

    let pubkey_content = pubkey_paths
        .iter()
        .find_map(|path| std::fs::read_to_string(path).ok())
        .ok_or_else(|| {
            anyhow::anyhow!("No SSH public key found. Please generate one with: ssh-keygen")
        })?;

    let pubkey_line = pubkey_content.trim();

    // Build host string with server username
    let host_str = format!("{}@{}", server_username, host);

    // If target user is different from server user, we need to use sudo
    if target_username != server_username {
        // First, check if the user exists
        let check_user_cmd = format!(r#"id -u {} >/dev/null 2>&1"#, target_username);

        println!(
            "Checking if user '{}' exists on remote system...",
            target_username
        );
        let mut check_cmd = Command::new("ssh");
        check_cmd.arg("-o").arg("StrictHostKeyChecking=no");
        check_cmd
            .arg("-o")
            .arg("PreferredAuthentications=keyboard-interactive,password");
        check_cmd.arg("-t");
        check_cmd.arg(&host_str);
        check_cmd.arg(&check_user_cmd);

        check_cmd.stdout(Stdio::null());
        check_cmd.stderr(Stdio::null());

        let user_exists = check_cmd.status()?.success();

        if !user_exists {
            // User doesn't exist, create it and set a password
            println!(
                "User '{}' does not exist. Creating user...",
                target_username
            );

            // Prompt for password
            print!("Set password for user '{}' (required): ", target_username);
            io::stdout().flush()?;

            let mut password_input = String::new();
            io::stdin().read_line(&mut password_input)?;
            let password = password_input.trim();

            if password.is_empty() {
                anyhow::bail!("Password is required to create user '{}'", target_username);
            }

            // Create user and set password using chpasswd (more secure than passing password in command)
            // We'll use a here-document approach via SSH
            let create_user_cmd = format!(
                r#"sudo useradd -m -s /bin/bash {} && echo '{}:{}' | sudo chpasswd"#,
                target_username, target_username, password
            );

            let mut create_cmd = Command::new("ssh");
            create_cmd.arg("-o").arg("StrictHostKeyChecking=no");
            create_cmd
                .arg("-o")
                .arg("PreferredAuthentications=keyboard-interactive,password");
            create_cmd.arg("-t");
            create_cmd.arg(&host_str);
            create_cmd.arg(&create_user_cmd);

            create_cmd.stdin(Stdio::inherit());
            create_cmd.stdout(Stdio::inherit());
            create_cmd.stderr(Stdio::inherit());

            let create_status = create_cmd.status()?;
            if !create_status.success() {
                anyhow::bail!("Failed to create user {} on {}", target_username, host);
            }

            println!("✓ User '{}' created with password", target_username);
        } else {
            // User exists, check if password is set
            println!("✓ User '{}' already exists", target_username);

            // Check if password is set by looking at /etc/shadow
            // Empty password field means ! or * or empty
            let check_password_cmd = format!(
                r#"sudo grep '^{}:' /etc/shadow | cut -d: -f2 | grep -qE '^[!*]?$' && echo 'NO_PASSWORD' || echo 'HAS_PASSWORD'"#,
                target_username
            );

            let mut password_check_cmd = Command::new("ssh");
            password_check_cmd.arg("-o").arg("StrictHostKeyChecking=no");
            password_check_cmd
                .arg("-o")
                .arg("PreferredAuthentications=keyboard-interactive,password");
            password_check_cmd.arg("-t");
            password_check_cmd.arg(&host_str);
            password_check_cmd.arg(&check_password_cmd);

            password_check_cmd.stdout(Stdio::piped());
            password_check_cmd.stderr(Stdio::null());

            let password_check_output = password_check_cmd.output()?;
            let password_status_str = String::from_utf8_lossy(&password_check_output.stdout);
            let password_status = password_status_str.trim();

            // Check if the command succeeded and what the output was
            if password_check_output.status.success() {
                if password_status == "NO_PASSWORD" {
                    println!("User '{}' exists but has no password set.", target_username);
                    print!(
                        "Set password for user '{}' (press Enter to skip): ",
                        target_username
                    );
                    io::stdout().flush()?;

                    let mut password_input = String::new();
                    io::stdin().read_line(&mut password_input)?;
                    let password = password_input.trim();

                    if !password.is_empty() {
                        // Set the password
                        let set_password_cmd =
                            format!(r#"echo '{}:{}' | sudo chpasswd"#, target_username, password);

                        let mut set_pwd_cmd = Command::new("ssh");
                        set_pwd_cmd.arg("-o").arg("StrictHostKeyChecking=no");
                        set_pwd_cmd
                            .arg("-o")
                            .arg("PreferredAuthentications=keyboard-interactive,password");
                        set_pwd_cmd.arg("-t");
                        set_pwd_cmd.arg(&host_str);
                        set_pwd_cmd.arg(&set_password_cmd);

                        set_pwd_cmd.stdin(Stdio::inherit());
                        set_pwd_cmd.stdout(Stdio::inherit());
                        set_pwd_cmd.stderr(Stdio::inherit());

                        let set_pwd_status = set_pwd_cmd.status()?;
                        if set_pwd_status.success() {
                            println!("✓ Password set for user '{}'", target_username);
                        } else {
                            eprintln!(
                                "Warning: Failed to set password for user '{}'",
                                target_username
                            );
                        }
                    } else {
                        println!("Skipping password setup - user can login with SSH keys only");
                    }
                } else {
                    println!("✓ User '{}' has a password set", target_username);
                }
            } else {
                // If we can't check, assume password is set (safer assumption)
                println!(
                    "Note: Could not verify password status for user '{}'",
                    target_username
                );
            }
        }

        // Now install the SSH key
        // Use getent to get the actual home directory path
        let append_cmd = format!(
            r#"HOME_DIR=$(getent passwd {} | cut -d: -f6) && sudo mkdir -p "$HOME_DIR/.ssh" && sudo chmod 700 "$HOME_DIR/.ssh" && echo '{}' | sudo tee -a "$HOME_DIR/.ssh/authorized_keys" > /dev/null && sudo chown {}:{} "$HOME_DIR/.ssh/authorized_keys" && sudo chmod 600 "$HOME_DIR/.ssh/authorized_keys" && sudo chown {}:{} "$HOME_DIR/.ssh""#,
            target_username,
            pubkey_line,
            target_username,
            target_username,
            target_username,
            target_username
        );

        println!("Installing SSH key for user '{}'...", target_username);
        let mut ssh_cmd = Command::new("ssh");
        ssh_cmd.arg("-o").arg("StrictHostKeyChecking=no");
        ssh_cmd
            .arg("-o")
            .arg("PreferredAuthentications=keyboard-interactive,password");
        ssh_cmd.arg("-t"); // Force pseudo-terminal for sudo prompts
        ssh_cmd.arg(&host_str);
        ssh_cmd.arg(&append_cmd);

        ssh_cmd.stdin(Stdio::inherit());
        ssh_cmd.stdout(Stdio::inherit());
        ssh_cmd.stderr(Stdio::inherit());

        let status = ssh_cmd.status()?;

        if status.success() {
            println!(
                "✓ SSH key copied successfully to {}@{} (installed for user: {})",
                server_username, host, target_username
            );
            Ok(())
        } else {
            anyhow::bail!(
                "Failed to install SSH key for user {} on {}",
                target_username,
                host
            );
        }
    } else {
        // Same user, use standard ssh-copy-id
        if which::which("ssh-copy-id").is_err() {
            anyhow::bail!(
                "ssh-copy-id not found. Please install openssh-client or openssh-clients"
            );
        }

        let mut cmd = Command::new("ssh-copy-id");
        cmd.arg("-o").arg("StrictHostKeyChecking=no");
        cmd.arg("-o")
            .arg("PreferredAuthentications=keyboard-interactive,password");
        cmd.arg("-f"); // Force mode - don't check if key is already installed
        cmd.arg(&host_str);

        cmd.stdin(Stdio::inherit());
        cmd.stdout(Stdio::inherit());
        cmd.stderr(Stdio::inherit());

        let status = cmd.status()?;

        if status.success() {
            println!("✓ SSH key copied successfully to {}", host_str);
            Ok(())
        } else {
            anyhow::bail!("Failed to copy SSH key to {}", host_str);
        }
    }
}

pub fn ssh_to_host(
    hostname: &str,
    user: Option<String>,
    fix_keys: bool,
    copy_keys: bool,
    ssh_args: &[String],
    config: &EnvConfig,
) -> Result<()> {
    use crate::config::list_available_hosts;

    // If hostname is empty, list available hosts
    if hostname.is_empty() {
        list_available_hosts(config);
        anyhow::bail!("Please specify a hostname");
    }

    let host_config = config.hosts.get(hostname).with_context(|| {
        format!(
            "Host '{}' not found in .env\n\nAdd configuration to .env:\n  HOST_{}_IP=\"<ip-address>\"\n  HOST_{}_TAILSCALE=\"<tailscale-hostname>\"",
            hostname,
            hostname.to_uppercase(),
            hostname.to_uppercase()
        )
    })?;

    // Collect all possible host addresses
    let mut all_hosts = Vec::new();
    if let Some(ip) = &host_config.ip {
        all_hosts.push(ip.clone());
    }
    if let Some(tailscale) = &host_config.tailscale {
        all_hosts.push(tailscale.clone());
        all_hosts.push(format!("{}.{}", tailscale, config.tailnet_base));
    }

    // If fix_keys is enabled, remove host keys for all possible addresses
    if fix_keys {
        println!("Fix keys mode enabled. Removing host keys for all configured addresses...");
        for host in &all_hosts {
            if prompt_remove_host_key(host)? {
                remove_ssh_host_key(host)?;
            }
        }
    }

    let mut tried_hosts = Vec::new();

    // If --keys flag is set, copy SSH key first (will prompt for username if needed)
    if copy_keys {
        let target_host = if let Some(ip) = &host_config.ip {
            ip.as_str()
        } else if let Some(tailscale) = &host_config.tailscale {
            tailscale.as_str()
        } else {
            anyhow::bail!("No IP or Tailscale hostname configured for {}", hostname);
        };

        // For key copying, determine username (prompt if not provided)
        let username_for_keys: Option<String> = if let Some(u) = &user {
            Some(u.clone())
        } else {
            // Prompt for username
            let default_user = config::get_default_username();
            print!("Username (press Enter for '{}'): ", default_user);
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;

            let input_username = input.trim();
            if input_username.is_empty() {
                Some(default_user)
            } else {
                Some(input_username.to_string())
            }
        };

        // For key copying, we need server username (to SSH in) and target username (where to install key)
        // Prompt for server username, then target username
        // Server username defaults to what we'll use to connect, target username is where key goes
        let default_target_user = config::get_default_username();

        print!(
            "Target username to install key for (press Enter for '{}'): ",
            default_target_user
        );
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let target_username = if input.trim().is_empty() {
            default_target_user
        } else {
            input.trim().to_string()
        };

        copy_ssh_key(
            target_host,
            username_for_keys.as_deref(), // Server username (to SSH into)
            Some(&target_username),       // Target username (where to install key)
        )?;
        // After copying keys, we can proceed with connection
    }

    // Determine username for connection - use provided, or try default first
    let username: Option<String> = if let Some(u) = user {
        Some(u)
    } else {
        // Don't prompt yet - try with default username first via key-based auth
        None // Will use default username from environment when connecting
    };

    let username_ref = username.as_deref();

    // Build list of hosts to try in order
    let mut hosts_to_try: Vec<(String, String)> = Vec::new();

    if let Some(ip) = &host_config.ip {
        hosts_to_try.push((ip.clone(), format!("IP: {}", ip)));
    }

    if let Some(tailscale) = &host_config.tailscale {
        hosts_to_try.push((tailscale.clone(), format!("Tailscale: {}", tailscale)));
        hosts_to_try.push((
            format!("{}.{}", tailscale, config.tailnet_base),
            format!("Tailscale FQDN: {}.{}", tailscale, config.tailnet_base),
        ));
    }

    // Try each host in sequence
    let total_hosts = hosts_to_try.len();
    for (idx, (host, description)) in hosts_to_try.iter().enumerate() {
        tried_hosts.push(host.clone());

        // Try key-based authentication first (silently, no prompts)
        // Use default username first (SSH typically needs a username)
        let default_username = config::get_default_username();

        // Try with default username first
        match connect_ssh_key_based(host, Some(&default_username), ssh_args) {
            Ok(_) => return Ok(()),
            Err(_) => {} // Key-based auth failed, continue
        }

        // If username was explicitly provided via flag, try that too
        if let Some(ref u) = username {
            if u != &default_username {
                match connect_ssh_key_based(host, Some(u), ssh_args) {
                    Ok(_) => return Ok(()),
                    Err(_) => {} // Key-based auth failed, continue
                }
            }
        }

        // All key-based auth attempts failed, need password-based auth
        println!("Attempting to connect to {} ({})...", description, host);

        // Use default username for password auth (no prompt needed)
        let final_username = if username_ref.is_none() {
            // Use default username without prompting
            Some(default_username)
        } else {
            username.clone()
        };
        // Try to connect with password authentication as fallback
        // This will allow interactive password prompts
        match connect_ssh(host, final_username.as_deref(), ssh_args) {
            Ok(_) => {
                // Connection succeeded, we're done
                return Ok(());
            }
            Err(e) => {
                // Connection failed, try next host
                eprintln!("Connection to {} failed: {}", host, e);
                if idx < total_hosts - 1 {
                    println!("Trying next host...");
                }
            }
        }
    }

    // All attempts failed
    eprintln!("✗ Failed to connect to any host");
    eprintln!("  Tried:");
    for host in &tried_hosts {
        eprintln!("    - {}", host);
    }
    std::process::exit(1);
}
