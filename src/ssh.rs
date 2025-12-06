use crate::config::{self, EnvConfig};
use anyhow::{Context, Result};
use std::io::{self, Write};
use std::process::{Command, Stdio};

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

pub fn connect_ssh(host: &str, user: Option<&str>, ssh_args: &[String]) -> Result<()> {
    let mut cmd = Command::new("ssh");

    // Add options to allow password authentication
    cmd.args([
        "-o",
        "PreferredAuthentications=keyboard-interactive,password",
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

pub fn ssh_to_host(
    hostname: &str,
    user: Option<String>,
    fix_keys: bool,
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

    // Determine username - use provided, prompt, or use default
    let username: Option<String> = if let Some(u) = user {
        Some(u)
    } else {
        // Prompt for username
        let default_user = config::get_default_username();

        print!("Username (press Enter for '{}'): ", default_user);
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let input_username = input.trim();
        if input_username.is_empty() {
            // Use default from environment
            Some(default_user)
        } else {
            // Use provided username
            Some(input_username.to_string())
        }
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
        println!("Attempting to connect to {} ({})...", description, host);

        // Try to connect - this will allow interactive password prompts
        // If connection succeeds, this function won't return (it will exec ssh)
        // If it fails, we'll catch the error and try the next host
        match connect_ssh(host, username_ref, ssh_args) {
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
