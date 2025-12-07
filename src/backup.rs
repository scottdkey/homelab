use crate::config::EnvConfig;
use crate::exec::SshConnection;
use anyhow::{Context, Result};
use std::time::SystemTime;

// New host-level backup functions
pub fn backup_host(hostname: &str, config: &EnvConfig) -> Result<()> {
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

    // Get backup path from config (required)
    let backup_base = host_config.backup_path.as_ref().with_context(|| {
        format!(
            "Backup path not configured for {}\n\nAdd to .env:\n  HOST_{}_BACKUP_PATH=\"/path/to/backups/{}\"",
            hostname,
            hostname.to_uppercase(),
            hostname
        )
    })?;

    println!(
        "Backing up all Docker volumes on {} ({})...",
        hostname, target_host
    );
    println!();

    let default_user = crate::config::get_default_username();
    let host_with_user = format!("{}@{}", default_user, target_host);
    let ssh_conn = SshConnection::new(&host_with_user)?;

    perform_backup(&ssh_conn, hostname, backup_base)?;

    println!();
    println!("✓ Backup complete for {}", hostname);

    Ok(())
}

pub fn list_backups(hostname: &str, config: &EnvConfig) -> Result<()> {
    let host_config = config
        .hosts
        .get(hostname)
        .with_context(|| format!("Host '{}' not found in .env", hostname))?;

    let target_host = if let Some(ip) = &host_config.ip {
        ip.clone()
    } else if let Some(tailscale) = &host_config.tailscale {
        tailscale.clone()
    } else {
        anyhow::bail!("No IP or Tailscale hostname configured for {}", hostname);
    };

    // Get backup path from config (required)
    let backup_base = host_config.backup_path.as_ref().with_context(|| {
        format!(
            "Backup path not configured for {}\n\nAdd to .env:\n  HOST_{}_BACKUP_PATH=\"/path/to/backups/{}\"",
            hostname,
            hostname.to_uppercase(),
            hostname
        )
    })?;

    println!("Listing backups for {} ({})...", hostname, target_host);
    println!();

    let default_user = crate::config::get_default_username();
    let host_with_user = format!("{}@{}", default_user, target_host);
    let ssh_conn = SshConnection::new(&host_with_user)?;

    list_backup_directories(&ssh_conn, backup_base)?;

    Ok(())
}

pub fn restore_host(hostname: &str, backup_name: Option<&str>, config: &EnvConfig) -> Result<()> {
    let host_config = config
        .hosts
        .get(hostname)
        .with_context(|| format!("Host '{}' not found in .env", hostname))?;

    let target_host = if let Some(ip) = &host_config.ip {
        ip.clone()
    } else if let Some(tailscale) = &host_config.tailscale {
        tailscale.clone()
    } else {
        anyhow::bail!("No IP or Tailscale hostname configured for {}", hostname);
    };

    // Get backup path from config (required)
    let backup_base = host_config.backup_path.as_ref().with_context(|| {
        format!(
            "Backup path not configured for {}\n\nAdd to .env:\n  HOST_{}_BACKUP_PATH=\"/path/to/backups/{}\"",
            hostname,
            hostname.to_uppercase(),
            hostname
        )
    })?;

    if let Some(backup) = backup_name {
        println!("Restoring {} from backup '{}'...", hostname, backup);
        println!();

        let default_user = crate::config::get_default_username();
        let host_with_user = format!("{}@{}", default_user, target_host);
        let ssh_conn = SshConnection::new(&host_with_user)?;

        perform_restore(&ssh_conn, hostname, backup_base, backup)?;

        println!();
        println!("✓ Restore complete for {}", hostname);
    } else {
        // List backups and prompt
        println!("No backup name specified. Available backups:");
        println!();
        list_backups(hostname, config)?;
        println!();
        println!(
            "Use: hal backup {} restore --backup <backup-name>",
            hostname
        );
    }

    Ok(())
}

fn perform_backup(ssh_conn: &SshConnection, hostname: &str, backup_base: &str) -> Result<()> {
    let datetime = chrono::DateTime::<chrono::Utc>::from(SystemTime::now());
    let timestamp_str = datetime.format("%Y%m%d_%H%M%S").to_string();
    let backup_dir = format!("{}/{}", backup_base, timestamp_str);

    println!("=== Host Backup Configuration ===");
    println!("Host: {}", hostname);
    println!("Backup Directory: {}", backup_dir);
    println!();

    // Check if backup base directory exists (parent of backup_base)
    if let Some(parent) = std::path::Path::new(backup_base).parent() {
        let parent_str = parent.to_string_lossy();
        let check_mount = ssh_conn.execute_simple("test", &["-d", &parent_str])?;
        if !check_mount.status.success() {
            anyhow::bail!(
                "Error: Backup base directory {} does not exist or is not mounted\nMake sure SMB mount is set up: hal smb {} setup",
                parent_str,
                hostname
            );
        }
    }

    // Create backup base directory if it doesn't exist
    let dir_exists = ssh_conn.execute_simple("test", &["-d", backup_base])?;
    if !dir_exists.status.success() {
        println!("Creating backup base directory: {}", backup_base);
        ssh_conn.mkdir_p(backup_base)?;
        println!("✓ Created backup base directory");
    }

    // Create backup directory
    ssh_conn.mkdir_p(&backup_dir)?;
    println!("✓ Created backup directory: {}", backup_dir);

    println!();
    println!("=== Stopping all containers ===");

    // Get running containers
    let running_output = ssh_conn.execute_simple("docker", &["ps", "-q"])?;
    let running_containers = String::from_utf8_lossy(&running_output.stdout);
    let running_containers: Vec<&str> = running_containers
        .lines()
        .filter(|l| !l.trim().is_empty())
        .collect();

    if !running_containers.is_empty() {
        println!(
            "Stopping {} running container(s)...",
            running_containers.len()
        );
        let stop_cmd = format!("docker stop {}", running_containers.join(" "));
        let stop_output = ssh_conn.execute_shell(&stop_cmd)?;
        if !stop_output.status.success() {
            // Try with sudo
            let sudo_stop = ssh_conn.execute_shell(&format!(
                "sudo docker stop {}",
                running_containers.join(" ")
            ))?;
            if !sudo_stop.status.success() {
                eprintln!("⚠ Warning: Some containers may not have stopped");
            }
        }
        println!("✓ All containers stopped");
    } else {
        println!("✓ No running containers to stop");
    }

    println!();
    println!("=== Backing up Docker volumes ===");

    // Get all Docker volumes
    let volumes_output =
        ssh_conn.execute_simple("docker", &["volume", "ls", "--format", "{{.Name}}"])?;
    let volumes_str = String::from_utf8_lossy(&volumes_output.stdout);
    let volumes: Vec<&str> = volumes_str
        .lines()
        .filter(|l| !l.trim().is_empty())
        .collect();

    if volumes.is_empty() {
        println!("No Docker volumes found");
    } else {
        println!("Found {} volume(s) to backup:", volumes.len());
        for vol in &volumes {
            println!("  - {}", vol);
        }
        println!();

        // Backup each volume
        for vol in &volumes {
            println!("  Backing up volume: {}", vol);
            let backup_cmd = format!(
                "docker run --rm -v {}:/data:ro -v {}:/backup alpine tar czf /backup/{}.tar.gz -C /data .",
                vol, backup_dir, vol
            );
            let backup_output = ssh_conn.execute_shell(&backup_cmd)?;
            if backup_output.status.success() {
                println!("    ✓ Volume {} backed up", vol);
            } else {
                // Try with sudo
                let sudo_backup_cmd = format!(
                    "sudo docker run --rm -v {}:/data:ro -v {}:/backup alpine tar czf /backup/{}.tar.gz -C /data .",
                    vol, backup_dir, vol
                );
                let sudo_output = ssh_conn.execute_shell(&sudo_backup_cmd)?;
                if sudo_output.status.success() {
                    println!("    ✓ Volume {} backed up", vol);
                } else {
                    println!("    ✗ Failed to backup volume: {}", vol);
                }
            }
        }
    }

    println!();
    println!("=== Backing up bind mounts from containers ===");

    // Get all containers
    let containers_output =
        ssh_conn.execute_simple("docker", &["ps", "-a", "--format", "{{.Names}}"])?;
    let containers_str = String::from_utf8_lossy(&containers_output.stdout);
    let containers: Vec<&str> = containers_str
        .lines()
        .filter(|l| !l.trim().is_empty())
        .collect();

    if containers.is_empty() {
        println!("No containers found");
    } else {
        for container in &containers {
            // Get bind mounts for this container
            let inspect_cmd = format!(
                r#"docker inspect {} --format '{{{{range .Mounts}}}}{{{{if eq .Type "bind"}}}}{{{{.Source}}}}{{{{end}}}}{{{{end}}}}'"#,
                container
            );
            let mounts_output = ssh_conn.execute_shell(&inspect_cmd)?;
            let mounts_str = String::from_utf8_lossy(&mounts_output.stdout);
            let mounts: Vec<&str> = mounts_str
                .lines()
                .filter(|l| !l.trim().is_empty())
                .collect();

            for mount_path in &mounts {
                // Check if mount path is a directory
                let check_dir = ssh_conn.execute_simple("test", &["-d", mount_path])?;
                if check_dir.status.success() {
                    let mount_name = mount_path
                        .split('/')
                        .last()
                        .unwrap_or("unknown")
                        .replace('/', "_");
                    let backup_name = format!("{}_{}", container, mount_name);
                    println!("  Backing up bind mount from {}: {}", container, mount_path);

                    let backup_cmd = format!(
                        "docker run --rm -v {}:/data:ro -v {}:/backup alpine tar czf /backup/{}.tar.gz -C /data .",
                        mount_path, backup_dir, backup_name
                    );
                    let backup_output = ssh_conn.execute_shell(&backup_cmd)?;
                    if backup_output.status.success() {
                        println!(
                            "    ✓ Bind mount {} backed up as {}.tar.gz",
                            mount_path, backup_name
                        );
                    } else {
                        // Try with sudo
                        let sudo_backup_cmd = format!(
                            "sudo docker run --rm -v {}:/data:ro -v {}:/backup alpine tar czf /backup/{}.tar.gz -C /data .",
                            mount_path, backup_dir, backup_name
                        );
                        let sudo_output = ssh_conn.execute_shell(&sudo_backup_cmd)?;
                        if sudo_output.status.success() {
                            println!(
                                "    ✓ Bind mount {} backed up as {}.tar.gz",
                                mount_path, backup_name
                            );
                        } else {
                            println!("    ✗ Failed to backup bind mount: {}", mount_path);
                        }
                    }
                }
            }
        }
    }

    // Create metadata file
    let metadata = format!(
        "Host: {}\nTimestamp: {}\nDate: {}\nVolume Count: {}\nVolumes:\n{}",
        hostname,
        timestamp_str,
        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
        volumes.len(),
        volumes
            .iter()
            .map(|v| format!("  - {}", v))
            .collect::<Vec<_>>()
            .join("\n")
    );
    ssh_conn.write_file(
        &format!("{}/backup-info.txt", backup_dir),
        metadata.as_bytes(),
    )?;
    println!("✓ Created backup metadata");

    println!();
    println!("=== Starting containers ===");

    if !running_containers.is_empty() {
        println!("Starting containers...");
        let start_cmd = format!("docker start {}", running_containers.join(" "));
        let start_output = ssh_conn.execute_shell(&start_cmd)?;
        if !start_output.status.success() {
            let sudo_start = ssh_conn.execute_shell(&format!(
                "sudo docker start {}",
                running_containers.join(" ")
            ))?;
            if !sudo_start.status.success() {
                eprintln!("⚠ Warning: Some containers may not have started");
            }
        }
        println!("✓ Containers started");
    } else {
        println!("✓ No containers to start");
    }

    println!();
    println!("=== Backup Summary ===");
    println!("Backup location: {}", backup_dir);
    println!("Backup name: {}", timestamp_str);
    println!("Volumes backed up: {}", volumes.len());

    let list_output = ssh_conn.execute_shell(&format!("ls -lh {}", backup_dir))?;
    if list_output.status.success() {
        let list_str = String::from_utf8_lossy(&list_output.stdout);
        for line in list_str.lines().skip(1) {
            println!("{}", line);
        }
    }

    Ok(())
}

fn list_backup_directories(ssh_conn: &SshConnection, backup_base: &str) -> Result<()> {
    println!("=== Available Backups ===");

    // Check if backup directory exists
    let dir_check = ssh_conn.execute_simple("test", &["-d", backup_base])?;
    if !dir_check.status.success() {
        anyhow::bail!(
            "Error: Backup directory {} does not exist or is not mounted",
            backup_base
        );
    }

    // List backup directories
    let find_output = ssh_conn.execute_shell(&format!(
        "find {} -mindepth 1 -maxdepth 1 -type d",
        backup_base
    ))?;

    if !find_output.status.success() {
        println!("No backups found");
        return Ok(());
    }

    let find_str = String::from_utf8_lossy(&find_output.stdout);
    let backup_dirs: Vec<&str> = find_str.lines().filter(|l| !l.trim().is_empty()).collect();

    if backup_dirs.is_empty() {
        println!("No backups found");
        return Ok(());
    }

    println!("Found {} backup(s):", backup_dirs.len());
    println!();

    for backup_dir in &backup_dirs {
        let backup_name = backup_dir.split('/').last().unwrap_or("unknown");

        // Get backup date
        let stat_cmd = format!(
            r#"stat -c %y "{}" 2>/dev/null || stat -f %Sm "{}" 2>/dev/null || echo "unknown""#,
            backup_dir, backup_dir
        );
        let date_output = ssh_conn.execute_shell(&stat_cmd)?;
        let backup_date = String::from_utf8_lossy(&date_output.stdout)
            .trim()
            .to_string();

        println!("  - {}", backup_name);
        println!("    Date: {}", backup_date);

        // Read backup info if it exists
        let info_path = format!("{}/backup-info.txt", backup_dir);
        let info_exists = ssh_conn.file_exists(&info_path)?;
        if info_exists {
            let info_output = ssh_conn.execute_shell(&format!("cat {}", info_path))?;
            if info_output.status.success() {
                let info_str = String::from_utf8_lossy(&info_output.stdout);
                println!("    Info:");
                for line in info_str.lines() {
                    println!("      {}", line);
                }
            }
        }
        println!();
    }

    Ok(())
}

fn perform_restore(
    ssh_conn: &SshConnection,
    hostname: &str,
    backup_base: &str,
    backup_name: &str,
) -> Result<()> {
    let backup_dir = format!("{}/{}", backup_base, backup_name);

    println!("=== Restore Configuration ===");
    println!("Host: {}", hostname);
    println!("Backup: {}", backup_name);
    println!("Backup Directory: {}", backup_dir);
    println!();

    // Check if backup base directory exists (parent of backup_base)
    if let Some(parent) = std::path::Path::new(backup_base).parent() {
        let parent_str = parent.to_string_lossy();
        let check_mount = ssh_conn.execute_simple("test", &["-d", &parent_str])?;
        if !check_mount.status.success() {
            anyhow::bail!(
                "Error: Backup base directory {} does not exist or is not mounted\nMake sure SMB mount is set up: hal smb <hostname> setup",
                parent_str
            );
        }
    }

    // Create backup base directory if it doesn't exist
    let dir_exists = ssh_conn.execute_simple("test", &["-d", backup_base])?;
    if !dir_exists.status.success() {
        ssh_conn.mkdir_p(backup_base)?;
        println!("Created backup base directory: {}", backup_base);
    }

    // Check if backup exists
    let backup_exists = ssh_conn.execute_simple("test", &["-d", &backup_dir])?;
    if !backup_exists.status.success() {
        println!("Error: Backup directory does not exist: {}", backup_dir);
        println!("Available backups:");
        let list_output = ssh_conn.execute_shell(&format!("ls -1 {}", backup_base))?;
        if list_output.status.success() {
            let list_str = String::from_utf8_lossy(&list_output.stdout);
            for line in list_str.lines() {
                println!("  {}", line);
            }
        } else {
            println!("  (none)");
        }
        anyhow::bail!("Backup directory does not exist: {}", backup_dir);
    }

    println!();
    println!("=== Stopping all containers ===");

    // Get running containers
    let running_output = ssh_conn.execute_simple("docker", &["ps", "-q"])?;
    let running_containers = String::from_utf8_lossy(&running_output.stdout);
    let running_containers: Vec<&str> = running_containers
        .lines()
        .filter(|l| !l.trim().is_empty())
        .collect();

    if !running_containers.is_empty() {
        println!(
            "Stopping {} running container(s)...",
            running_containers.len()
        );
        let stop_cmd = format!("docker stop {}", running_containers.join(" "));
        let stop_output = ssh_conn.execute_shell(&stop_cmd)?;
        if !stop_output.status.success() {
            let sudo_stop = ssh_conn.execute_shell(&format!(
                "sudo docker stop {}",
                running_containers.join(" ")
            ))?;
            if !sudo_stop.status.success() {
                eprintln!("⚠ Warning: Some containers may not have stopped");
            }
        }
        println!("✓ All containers stopped");
    } else {
        println!("✓ No running containers to stop");
    }

    println!();
    println!("=== Restoring Docker volumes ===");

    // List backup files
    let list_files = ssh_conn.execute_shell(&format!(
        "ls -1 {}/*.tar.gz 2>/dev/null || true",
        backup_dir
    ))?;
    let files_str = String::from_utf8_lossy(&list_files.stdout);
    let backup_files: Vec<&str> = files_str
        .lines()
        .filter(|l| !l.trim().is_empty() && l.ends_with(".tar.gz"))
        .collect();

    for backup_file in &backup_files {
        // Extract volume name from filename (remove path and .tar.gz extension)
        let vol_name = backup_file
            .split('/')
            .last()
            .unwrap_or("")
            .trim_end_matches(".tar.gz");

        if vol_name.is_empty() {
            continue;
        }

        println!("Restoring volume: {}", vol_name);

        // Check if volume exists, create if not
        let inspect_output = ssh_conn.execute_simple("docker", &["volume", "inspect", vol_name])?;
        if !inspect_output.status.success() {
            // Try with sudo
            let sudo_inspect =
                ssh_conn.execute_shell(&format!("sudo docker volume inspect {}", vol_name))?;
            if !sudo_inspect.status.success() {
                // Create volume
                let create_output =
                    ssh_conn.execute_simple("docker", &["volume", "create", vol_name])?;
                if !create_output.status.success() {
                    let sudo_create = ssh_conn
                        .execute_shell(&format!("sudo docker volume create {}", vol_name))?;
                    if !sudo_create.status.success() {
                        println!("  ✗ Failed to create volume: {}", vol_name);
                        continue;
                    }
                }
                println!("  Created volume: {}", vol_name);
            }
        }

        // Restore volume
        let restore_cmd = format!(
            "docker run --rm -v {}:/data -v {}:/backup alpine sh -c 'cd /data && rm -rf * && tar xzf /backup/{}.tar.gz'",
            vol_name, backup_dir, vol_name
        );
        let restore_output = ssh_conn.execute_shell(&restore_cmd)?;
        if restore_output.status.success() {
            println!("  ✓ Restored volume: {}", vol_name);
        } else {
            // Try with sudo
            let sudo_restore_cmd = format!(
                "sudo docker run --rm -v {}:/data -v {}:/backup alpine sh -c 'cd /data && rm -rf * && tar xzf /backup/{}.tar.gz'",
                vol_name, backup_dir, vol_name
            );
            let sudo_restore = ssh_conn.execute_shell(&sudo_restore_cmd)?;
            if sudo_restore.status.success() {
                println!("  ✓ Restored volume: {}", vol_name);
            } else {
                println!("  ✗ Failed to restore volume: {}", vol_name);
            }
        }
    }

    println!();
    println!("=== Starting containers ===");

    if !running_containers.is_empty() {
        println!("Starting containers...");
        let start_cmd = format!("docker start {}", running_containers.join(" "));
        let start_output = ssh_conn.execute_shell(&start_cmd)?;
        if !start_output.status.success() {
            let sudo_start = ssh_conn.execute_shell(&format!(
                "sudo docker start {}",
                running_containers.join(" ")
            ))?;
            if !sudo_start.status.success() {
                eprintln!("⚠ Warning: Some containers may not have started");
            }
        }
        println!("✓ Containers started");
    } else {
        println!("✓ No containers to start");
    }

    println!();
    println!("=== Restore Summary ===");
    println!("Restored from: {}", backup_dir);
    println!("Host: {}", hostname);

    Ok(())
}
