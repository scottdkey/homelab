use crate::config::{self, EnvConfig};
use crate::exec::SshConnection;
use anyhow::{Context, Result};

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

    // Get SSH connection
    let default_user = config::get_default_username();
    let host_with_user = format!("{}@{}", default_user, target_host);
    let ssh_conn = SshConnection::new(&host_with_user)?;

    // Execute setup using Rust-native operations
    setup_smb_mounts_remote(&ssh_conn, config)?;

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

    // Get SSH connection
    let default_user = config::get_default_username();
    let host_with_user = format!("{}@{}", default_user, target_host);
    let ssh_conn = SshConnection::new(&host_with_user)?;

    // Execute uninstall using Rust-native operations
    uninstall_smb_mounts_remote(&ssh_conn, config)?;

    println!();
    println!("✓ SMB mounts removed from {}", hostname);

    Ok(())
}

fn setup_smb_mounts_remote(ssh_conn: &SshConnection, config: &EnvConfig) -> Result<()> {
    println!("=== SMB Configuration ===");
    println!("Configuration loaded from .env file");
    println!(
        "Number of SMB servers configured: {}",
        config.smb_servers.len()
    );

    // Add configuration summary
    for (server_name, server_config) in &config.smb_servers {
        println!(
            "  - {}: {} ({} share(s))",
            server_name,
            server_config.host,
            server_config.shares.len()
        );
        for share in &server_config.shares {
            println!("    └─ {} -> /mnt/smb/{}/{}", share, server_name, share);
        }
    }
    println!();

    // Install SMB client
    install_smb_client(ssh_conn)?;

    // Clean up old mounts
    cleanup_old_mounts(ssh_conn)?;

    // Create mount directory
    println!("=== Creating SMB mount directory ===");
    ssh_conn.execute_interactive("sudo", &["mkdir", "-p", "/mnt/smb"])?;
    println!("✓ Mount directory created");
    println!();

    // Mount each share
    for (server_name, server_config) in &config.smb_servers {
        for share_name in &server_config.shares {
            let mount_point = format!("/mnt/smb/{}/{}", server_name, share_name);
            let share_path = format!("//{}/{}", server_config.host, share_name);

            setup_smb_share(
                ssh_conn,
                server_name,
                share_name,
                &share_path,
                &mount_point,
                server_config,
            )?;
        }
    }

    println!();
    println!("=== SMB setup complete ===");

    Ok(())
}

fn install_smb_client(ssh_conn: &SshConnection) -> Result<()> {
    println!("=== Installing SMB client ===");

    // Check if mount.cifs exists
    if ssh_conn.check_command_exists("mount.cifs")? {
        println!("✓ SMB client already installed");
        return Ok(());
    }

    // Detect package manager and install
    if ssh_conn.check_command_exists("apt-get")? {
        ssh_conn.execute_interactive("sudo", &["apt-get", "update"])?;
        ssh_conn.execute_interactive("sudo", &["apt-get", "install", "-y", "cifs-utils"])?;
    } else if ssh_conn.check_command_exists("yum")? {
        ssh_conn.execute_interactive("sudo", &["yum", "install", "-y", "cifs-utils"])?;
    } else if ssh_conn.check_command_exists("dnf")? {
        ssh_conn.execute_interactive("sudo", &["dnf", "install", "-y", "cifs-utils"])?;
    } else {
        anyhow::bail!("Unsupported package manager for SMB client installation");
    }

    println!("✓ SMB client installed");
    Ok(())
}

fn cleanup_old_mounts(ssh_conn: &SshConnection) -> Result<()> {
    println!("=== Cleaning up old mounts ===");

    // List directories in /mnt/smb
    let list_output = ssh_conn.execute_simple("ls", &["-1", "/mnt/smb"])?;
    if !list_output.status.success() {
        // Directory doesn't exist yet, nothing to clean up
        return Ok(());
    }

    let dirs_str = String::from_utf8_lossy(&list_output.stdout);
    for server_dir in dirs_str.lines() {
        let server_dir = server_dir.trim();
        if server_dir.is_empty() {
            continue;
        }

        let full_path = format!("/mnt/smb/{}", server_dir);

        // Check if it's a mount point
        let mountpoint_check = ssh_conn.execute_simple("mountpoint", &["-q", &full_path]);
        if let Ok(output) = mountpoint_check {
            if output.status.success() {
                println!("Found old mount at {}, unmounting...", full_path);
                ssh_conn
                    .execute_simple("sudo", &["umount", &full_path])
                    .ok();
                remove_fstab_entry(ssh_conn, &full_path)?;
                println!("✓ Cleaned up old mount at {}", full_path);
            }
        }
    }

    println!();
    Ok(())
}

fn setup_smb_share(
    ssh_conn: &SshConnection,
    server_name: &str,
    share_name: &str,
    share_path: &str,
    mount_point: &str,
    server_config: &crate::config::SmbServerConfig,
) -> Result<()> {
    println!();
    println!("=== Setting up {} - {} ===", server_name, share_name);
    println!("Configuration:");
    println!("  Server: {}", server_name);
    println!("  Host: {}", server_config.host);
    println!("  Share: {}", share_name);
    println!("  Mount Point: {}", mount_point);
    println!(
        "  Username: {}",
        server_config
            .username
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("(not set)")
    );
    println!(
        "  Options: {}",
        server_config
            .options
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("(none)")
    );

    // Validate credentials
    let username = server_config.username.as_ref().ok_or_else(|| {
        anyhow::anyhow!(
            "No username configured for {} - {}",
            server_name,
            share_name
        )
    })?;

    let password = server_config.password.as_ref().ok_or_else(|| {
        anyhow::anyhow!(
            "No password configured for {} - {}",
            server_name,
            share_name
        )
    })?;

    // Create mount point
    ssh_conn.execute_interactive("sudo", &["mkdir", "-p", mount_point])?;

    // Check if already mounted
    let mountpoint_check = ssh_conn.execute_simple("mountpoint", &["-q", mount_point]);
    if let Ok(output) = mountpoint_check {
        if output.status.success() {
            println!(
                "✓ {} - {} is already mounted at {}",
                server_name, share_name, mount_point
            );
            return Ok(());
        }
    }

    // Get user ID and group ID
    let uid_output = ssh_conn.execute_simple("id", &["-u"])?;
    let uid = String::from_utf8_lossy(&uid_output.stdout)
        .trim()
        .to_string();

    let gid_output = ssh_conn.execute_simple("id", &["-g"])?;
    let gid = String::from_utf8_lossy(&gid_output.stdout)
        .trim()
        .to_string();

    // Build mount options
    let mut mount_opts = format!(
        "username={},password={},uid={},gid={}",
        username, password, uid, gid
    );
    if let Some(ref opts) = server_config.options {
        mount_opts.push_str(&format!(",{}", opts));
    }

    println!("Mounting: {} -> {}", share_path, mount_point);

    // Mount the share
    let mount_result = ssh_conn.execute_simple(
        "sudo",
        &[
            "mount",
            "-t",
            "cifs",
            share_path,
            mount_point,
            "-o",
            &mount_opts,
        ],
    );

    if mount_result.is_ok() && mount_result.as_ref().unwrap().status.success() {
        println!(
            "✓ {} - {} mounted at {}",
            server_name, share_name, mount_point
        );

        // Add to /etc/fstab
        let fstab_entry = format!(
            "{} {} cifs {},_netdev 0 0",
            share_path, mount_point, mount_opts
        );
        add_fstab_entry(ssh_conn, mount_point, &fstab_entry)?;
    } else {
        anyhow::bail!(
            "Failed to mount {} - {} at {}",
            server_name,
            share_name,
            mount_point
        );
    }

    Ok(())
}

fn add_fstab_entry(ssh_conn: &SshConnection, mount_point: &str, entry: &str) -> Result<()> {
    // Check if entry already exists
    let fstab_content = ssh_conn.read_file("/etc/fstab")?;
    if fstab_content.lines().any(|line| line.contains(mount_point)) {
        println!("✓ Entry already exists in /etc/fstab");
        return Ok(());
    }

    // Append entry to /etc/fstab
    let new_content = format!("{}\n{}", fstab_content.trim_end(), entry);
    ssh_conn.write_file("/tmp/fstab.new", new_content.as_bytes())?;
    ssh_conn.execute_interactive("sudo", &["mv", "/tmp/fstab.new", "/etc/fstab"])?;
    println!("✓ Added to /etc/fstab for automatic mounting");
    println!("  Entry: {}", entry);
    Ok(())
}

fn remove_fstab_entry(ssh_conn: &SshConnection, mount_point: &str) -> Result<()> {
    let fstab_content = ssh_conn.read_file("/etc/fstab")?;
    let filtered_lines: Vec<&str> = fstab_content
        .lines()
        .filter(|line| !line.contains(mount_point))
        .collect();

    if filtered_lines.len() == fstab_content.lines().count() {
        // No entry found, nothing to remove
        return Ok(());
    }

    let new_content = filtered_lines.join("\n");
    if !new_content.is_empty() {
        ssh_conn.write_file("/tmp/fstab.new", new_content.as_bytes())?;
        ssh_conn.execute_interactive("sudo", &["mv", "/tmp/fstab.new", "/etc/fstab"])?;
    }
    Ok(())
}

fn uninstall_smb_mounts_remote(ssh_conn: &SshConnection, config: &EnvConfig) -> Result<()> {
    println!("=== Unmounting SMB shares ===");

    // Unmount each share
    for (server_name, server_config) in &config.smb_servers {
        for share_name in &server_config.shares {
            let mount_point = format!("/mnt/smb/{}/{}", server_name, share_name);

            // Check if mounted
            let mountpoint_check = ssh_conn.execute_simple("mountpoint", &["-q", &mount_point]);
            if let Ok(output) = mountpoint_check {
                if output.status.success() {
                    println!("Unmounting {} - {}...", server_name, share_name);
                    let umount_result = ssh_conn.execute_simple("sudo", &["umount", &mount_point]);
                    if umount_result.is_ok() && umount_result.as_ref().unwrap().status.success() {
                        println!("✓ {} - {} unmounted", server_name, share_name);
                    } else {
                        println!("✗ Failed to unmount {} - {}", server_name, share_name);
                    }
                } else {
                    println!("✓ {} - {} is not mounted", server_name, share_name);
                }
            }

            // Remove from /etc/fstab
            remove_fstab_entry(ssh_conn, &mount_point)?;
            println!("✓ Removed {} from /etc/fstab", mount_point);

            // Remove mount point directory
            let dir_check = ssh_conn.execute_simple("test", &["-d", &mount_point]);
            if let Ok(output) = dir_check {
                if output.status.success() {
                    let rmdir_result = ssh_conn.execute_simple("sudo", &["rmdir", &mount_point]);
                    if rmdir_result.is_ok() && rmdir_result.as_ref().unwrap().status.success() {
                        println!("✓ Removed mount point {}", mount_point);
                    } else {
                        println!("Mount point {} not empty, leaving it", mount_point);
                    }
                }
            }
        }
    }

    println!();
    println!("=== SMB uninstall complete ===");

    Ok(())
}

// Removed build_smb_uninstall_script - replaced with uninstall_smb_mounts_remote
// Removed execute_smb_script - replaced with direct SshConnection usage
