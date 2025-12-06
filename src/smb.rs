use crate::config::{self, EnvConfig};
use anyhow::{Context, Result};
use std::process::{Command, Stdio};

pub fn setup_smb_mounts(hostname: &str, config: &EnvConfig) -> Result<()> {
    let host_config = config.hosts.get(hostname).with_context(|| {
        format!(
            "Host '{}' not found in .env\n\nAdd configuration to .env:\n  HOST_{}_IP=\"<ip-address>\"\n  HOST_{}_TAILSCALE=\"<tailscale-hostname>\"",
            hostname,
            hostname.to_uppercase(),
            hostname.to_uppercase()
        )
    })?;

    let target_host = if let Some(ip) = &host_config.ip {
        ip.clone()
    } else if let Some(tailscale) = &host_config.tailscale {
        tailscale.clone()
    } else {
        anyhow::bail!("No IP or Tailscale hostname configured for {}", hostname);
    };

    println!("Setting up SMB mounts on {} ({})...", hostname, target_host);
    println!();

    // Build the SMB setup script
    let script = build_smb_setup_script(config)?;

    // Execute the script via SSH
    execute_smb_script(&target_host, &script)?;

    println!();
    println!("✓ SMB mount setup complete for {}", hostname);

    Ok(())
}

pub fn uninstall_smb_mounts(hostname: &str, config: &EnvConfig) -> Result<()> {
    let host_config = config.hosts.get(hostname).with_context(|| {
        format!(
            "Host '{}' not found in .env\n\nAdd configuration to .env:\n  HOST_{}_IP=\"<ip-address>\"\n  HOST_{}_TAILSCALE=\"<tailscale-hostname>\"",
            hostname,
            hostname.to_uppercase(),
            hostname.to_uppercase()
        )
    })?;

    let target_host = if let Some(ip) = &host_config.ip {
        ip.clone()
    } else if let Some(tailscale) = &host_config.tailscale {
        tailscale.clone()
    } else {
        anyhow::bail!("No IP or Tailscale hostname configured for {}", hostname);
    };

    println!(
        "Uninstalling SMB mounts on {} ({})...",
        hostname, target_host
    );
    println!();

    // Build the SMB uninstall script
    let script = build_smb_uninstall_script(config)?;

    // Execute the script via SSH
    execute_smb_script(&target_host, &script)?;

    println!();
    println!("✓ SMB mounts removed from {}", hostname);

    Ok(())
}

fn build_smb_setup_script(config: &EnvConfig) -> Result<String> {
    let mut script = String::from("#!/bin/bash\nset -e\n\n");

    script.push_str(&format!(
        r#"
echo "=== SMB Configuration ==="
echo "Configuration loaded from .env file"
echo "Number of SMB servers configured: {}"
"#,
        config.smb_servers.len()
    ));

    // Add configuration summary
    for (server_name, server_config) in &config.smb_servers {
        script.push_str(&format!(
            r#"echo "  - {}: {} ({} share(s))"
"#,
            server_name,
            server_config.host,
            server_config.shares.len()
        ));
        for share in &server_config.shares {
            script.push_str(&format!(
                r#"echo "    └─ {} -> /mnt/smb/{}/{}"
"#,
                share, server_name, share
            ));
        }
    }
    script.push_str("echo \"\"\n");

    script.push_str(
        r#"
echo "=== Installing SMB client ==="
if ! command -v mount.cifs &> /dev/null; then
    if command -v apt-get &> /dev/null; then
        sudo apt-get update
        sudo apt-get install -y cifs-utils
    elif command -v yum &> /dev/null; then
        sudo yum install -y cifs-utils
    elif command -v dnf &> /dev/null; then
        sudo dnf install -y cifs-utils
    else
        echo "Error: Unsupported package manager for SMB client installation"
        exit 1
    fi
    echo "✓ SMB client installed"
else
    echo "✓ SMB client already installed"
fi

echo ""
echo "=== Cleaning up old mounts ==="
# Check for old top-level mounts (from before multiple shares support)
for server_dir in /mnt/smb/*/; do
    if [ -d "$server_dir" ]; then
        SERVER_NAME=$(basename "$server_dir")
        # Check if the server directory itself is a mount point (old style)
        if mountpoint -q "$server_dir" 2>/dev/null; then
            echo "Found old mount at $server_dir, unmounting..."
            sudo umount "$server_dir" 2>/dev/null || true
            # Remove from /etc/fstab
            sudo sed -i "\|$server_dir|d" /etc/fstab 2>/dev/null || true
            echo "✓ Cleaned up old mount at $server_dir"
        fi
    fi
done

echo ""
echo "=== Creating SMB mount directory ==="
sudo mkdir -p /mnt/smb
echo "✓ Mount directory created"

"#,
    );

    // Add mount commands for each SMB server and each share
    for (server_name, server_config) in &config.smb_servers {
        // Mount each share for this server
        for share_name in &server_config.shares {
            let mount_point = format!("/mnt/smb/{}/{}", server_name, share_name);
            let share_path = format!("//{}/{}", server_config.host, share_name);

            let username_str = server_config
                .username
                .as_ref()
                .map(|s| s.as_str())
                .unwrap_or("");
            let password_str = server_config
                .password
                .as_ref()
                .map(|s| s.as_str())
                .unwrap_or("");
            let options_line = if let Some(ref opts) = server_config.options {
                format!("MOUNT_OPTS=\"$MOUNT_OPTS,{}\"", opts)
            } else {
                String::new()
            };

            script.push_str(&format!(
                r#"
echo ""
echo "=== Setting up {} - {} ==="
echo "Configuration:"
echo "  Server: {}"
echo "  Host: {}"
echo "  Share: {}"
echo "  Mount Point: {}"
echo "  Username: {}"
echo "  Options: {}"
MOUNT_POINT="{}"
SHARE_PATH="{}"

# Create mount point
sudo mkdir -p "$MOUNT_POINT"

# Check if already mounted
if mountpoint -q "$MOUNT_POINT" 2>/dev/null; then
    echo "✓ {} - {} is already mounted at $MOUNT_POINT"
else
    # Get credentials
    USERNAME="{}"
    PASSWORD="{}"
    
    if [ -z "$USERNAME" ]; then
        echo "Warning: No username configured for {} - {}, skipping"
        continue
    fi
    
    if [ -z "$PASSWORD" ]; then
        echo "Warning: No password configured for {} - {}, skipping"
        continue
    fi
    
    # Build mount options
    MOUNT_OPTS="username=$USERNAME,password=$PASSWORD,uid=$(id -u),gid=$(id -g)"
    {}
    
    echo "Mounting: $SHARE_PATH -> $MOUNT_POINT"
    # Mount the share
    if sudo mount -t cifs "$SHARE_PATH" "$MOUNT_POINT" -o "$MOUNT_OPTS"; then
        echo "✓ {} - {} mounted at $MOUNT_POINT"
        
        # Add to /etc/fstab for persistence
        FSTAB_ENTRY="$SHARE_PATH $MOUNT_POINT cifs $MOUNT_OPTS,_netdev 0 0"
        if ! grep -q "$MOUNT_POINT" /etc/fstab; then
            echo "$FSTAB_ENTRY" | sudo tee -a /etc/fstab > /dev/null
            echo "✓ Added to /etc/fstab for automatic mounting"
            echo "  Entry: $FSTAB_ENTRY"
        else
            echo "✓ Entry already exists in /etc/fstab"
        fi
    else
        echo "✗ Failed to mount {} - {} at $MOUNT_POINT"
    fi
fi
"#,
                server_name,
                share_name,
                server_name,
                server_config.host,
                share_name,
                mount_point,
                if username_str.is_empty() {
                    "(not set)"
                } else {
                    username_str
                },
                if let Some(ref opts) = server_config.options {
                    opts
                } else {
                    "(none)"
                },
                mount_point,
                share_path,
                server_name,
                share_name,
                username_str,
                password_str,
                server_name,
                share_name,
                server_name,
                share_name,
                options_line,
                server_name,
                share_name,
                server_name,
                share_name
            ));
        }
    }

    script.push_str("\necho \"\n=== SMB setup complete ===\"\n");

    Ok(script)
}

fn build_smb_uninstall_script(config: &EnvConfig) -> Result<String> {
    let mut script = String::from("#!/bin/bash\nset -e\n\n");

    script.push_str(
        r#"
echo "=== Unmounting SMB shares ==="
"#,
    );

    // Add unmount commands for each SMB server and each share
    for (server_name, server_config) in &config.smb_servers {
        for share_name in &server_config.shares {
            let mount_point = format!("/mnt/smb/{}/{}", server_name, share_name);

            script.push_str(&format!(
            r#"
MOUNT_POINT="{}"
if mountpoint -q "$MOUNT_POINT" 2>/dev/null; then
    echo "Unmounting {} - {}..."
    if sudo umount "$MOUNT_POINT"; then
        echo "✓ {} - {} unmounted"
    else
        echo "✗ Failed to unmount {} - {}"
    fi
else
    echo "✓ {} - {} is not mounted"
fi

# Remove from /etc/fstab
if grep -q "$MOUNT_POINT" /etc/fstab; then
    sudo sed -i "\|$MOUNT_POINT|d" /etc/fstab
    echo "✓ Removed {} from /etc/fstab"
fi

# Remove mount point directory
if [ -d "$MOUNT_POINT" ]; then
    sudo rmdir "$MOUNT_POINT" 2>/dev/null && echo "✓ Removed mount point $MOUNT_POINT" || echo "Mount point $MOUNT_POINT not empty, leaving it"
fi
"#,
                mount_point, server_name, share_name, server_name, share_name, server_name, share_name, server_name, share_name, mount_point
            ));
        }
    }

    script.push_str("\necho \"\n=== SMB uninstall complete ===\"\n");

    Ok(script)
}

fn execute_smb_script(host: &str, script: &str) -> Result<()> {
    use std::io::Write;

    // Try key-based authentication first with default username
    let default_user = config::get_default_username();
    let host_with_user = format!("{}@{}", default_user, host);

    // Test if key-based auth works
    let test_cmd = format!(
        r#"ssh -o ConnectTimeout=1 -o BatchMode=yes -o PreferredAuthentications=publickey -o PasswordAuthentication=no -o StrictHostKeyChecking=no {} 'echo test' >/dev/null 2>&1"#,
        host_with_user
    );

    let test_status = Command::new("sh")
        .arg("-c")
        .arg(&test_cmd)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    let use_key_auth = test_status.is_ok() && test_status.unwrap().success();

    // Determine username - use default if key auth works, otherwise prompt
    let username = if use_key_auth {
        default_user
    } else {
        print!(
            "Username for {} (press Enter for '{}'): ",
            host, default_user
        );
        std::io::stdout().flush()?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        if input.trim().is_empty() {
            default_user
        } else {
            input.trim().to_string()
        }
    };

    let host_with_user = format!("{}@{}", username, host);

    // Write script to temp file on remote system
    let temp_script_path = format!("/tmp/hal-smb-{}.sh", std::process::id());

    // Write script to remote file
    let mut write_cmd = Command::new("ssh");
    if use_key_auth {
        write_cmd.args([
            "-o",
            "StrictHostKeyChecking=no",
            "-o",
            "PreferredAuthentications=publickey",
            "-o",
            "PasswordAuthentication=no",
            &host_with_user,
            "bash",
            "-c",
            &format!(
                "cat > {} && chmod +x {}",
                temp_script_path, temp_script_path
            ),
        ]);
    } else {
        write_cmd.args([
            "-o",
            "StrictHostKeyChecking=no",
            "-o",
            "PreferredAuthentications=publickey,keyboard-interactive,password",
            &host_with_user,
            "bash",
            "-c",
            &format!(
                "cat > {} && chmod +x {}",
                temp_script_path, temp_script_path
            ),
        ]);
    }

    write_cmd.stdin(Stdio::piped());
    write_cmd.stdout(Stdio::null());
    write_cmd.stderr(Stdio::inherit());

    let mut write_child = write_cmd.spawn()?;
    if let Some(mut stdin) = write_child.stdin.take() {
        stdin.write_all(script.as_bytes())?;
        stdin.flush()?;
        drop(stdin);
    }

    let write_status = write_child.wait()?;
    if !write_status.success() {
        anyhow::bail!("Failed to write SMB script to remote system");
    }

    // Now execute the script
    let mut exec_cmd = Command::new("ssh");
    if use_key_auth {
        exec_cmd.args([
            "-o",
            "PreferredAuthentications=publickey",
            "-o",
            "PasswordAuthentication=no",
            "-o",
            "StrictHostKeyChecking=no",
            "-tt",
            &host_with_user,
            "bash",
            &temp_script_path,
        ]);
    } else {
        exec_cmd.args([
            "-o",
            "PreferredAuthentications=keyboard-interactive,password,publickey",
            "-o",
            "StrictHostKeyChecking=no",
            "-tt",
            &host_with_user,
            "bash",
            &temp_script_path,
        ]);
    }

    exec_cmd.stdin(Stdio::inherit());
    exec_cmd.stdout(Stdio::inherit());
    exec_cmd.stderr(Stdio::inherit());

    let status = exec_cmd.status()?;

    // Clean up the temporary script
    let _ = Command::new("ssh")
        .args([
            "-o",
            "StrictHostKeyChecking=no",
            "-o",
            "BatchMode=yes",
            &host_with_user,
            "rm",
            "-f",
            &temp_script_path,
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    if !status.success() {
        anyhow::bail!(
            "SMB script failed with exit code: {}",
            status.code().unwrap_or(1)
        );
    }

    Ok(())
}
