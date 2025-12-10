use crate::config;
use crate::db;
use anyhow::Result;

/// List available servers/hosts
pub fn handle_list(verbose: bool) -> Result<()> {
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Available Servers");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();

    // Try to load from env file
    let homelab_dir = config::find_homelab_dir();
    let (env_hosts, tailnet_base) = if let Ok(dir) = &homelab_dir {
        match config::load_env_config(dir) {
            Ok(cfg) => (Some(cfg.hosts), cfg._tailnet_base),
            Err(_) => (None, "ts.net".to_string()),
        }
    } else {
        (None, "ts.net".to_string())
    };

    // Try to load from database
    let db_hosts = db::list_hosts().ok();

    // Combine hosts from both sources
    let mut all_hosts = std::collections::HashMap::new();

    if let Some(hosts) = env_hosts {
        for (name, config) in hosts {
            all_hosts.insert(name, ("env", config));
        }
    }

    if let Some(hosts) = db_hosts {
        for name in hosts {
            // Get host config from DB if available
            if let Ok(Some(config)) = db::get_host_config(&name) {
                // If host exists in both, mark as "both"
                if let Some((source, _)) = all_hosts.get_mut(&name) {
                    *source = "both";
                } else {
                    all_hosts.insert(name, ("db", config));
                }
            } else if !all_hosts.contains_key(&name) {
                // Host exists in DB but no config - create empty config
                let empty_config = config::HostConfig {
                    ip: None,
                    hostname: None,
                    tailscale: None,
                    backup_path: None,
                };
                all_hosts.insert(name, ("db", empty_config));
            }
        }
    }

    if all_hosts.is_empty() {
        println!("No servers found.");
        println!();
        println!("To add servers:");
        println!("  halvor config create ssh <hostname>");
        return Ok(());
    }

    // Sort hostnames for consistent output
    let mut hostnames: Vec<_> = all_hosts.keys().collect();
    hostnames.sort();

    if verbose {
        for hostname in &hostnames {
            let (source, config) = all_hosts.get(*hostname).unwrap();
            println!("Hostname: {}", hostname);
            println!(
                "  Source: {}",
                match *source {
                    "env" => "Environment file (.env)",
                    "db" => "Database (SQLite)",
                    "both" => "Environment file & Database",
                    _ => "Unknown",
                }
            );
            if let Some(ref ip) = config.ip {
                println!("  IP Address: {}", ip);
            }
            if let Some(ref hostname) = config.hostname {
                println!("  Hostname: {}.{}", hostname, tailnet_base);
            }
            if let Some(ref tailscale) = config.tailscale {
                if config
                    .hostname
                    .as_ref()
                    .map(|h| h != tailscale)
                    .unwrap_or(true)
                {
                    println!(
                        "  Tailscale: {}.{} (different from hostname)",
                        tailscale, tailnet_base
                    );
                }
            }
            if let Some(ref backup_path) = config.backup_path {
                println!("  Backup Path: {}", backup_path);
            }
            // Get provisioning info from DB if available
            if let Ok(Some(info)) = db::get_host_info(hostname) {
                if let Some(ref docker_version) = info.1 {
                    println!("  Docker Version: {}", docker_version);
                }
                println!(
                    "  Tailscale Installed: {}",
                    if info.2 { "Yes" } else { "No" }
                );
                println!(
                    "  Portainer Installed: {}",
                    if info.3 { "Yes" } else { "No" }
                );
                if let Some(ref metadata) = info.4 {
                    println!("  Metadata: {}", metadata);
                }
            }
            println!();
        }
    } else {
        println!("Servers:");
        for hostname in &hostnames {
            let (source, config) = all_hosts.get(*hostname).unwrap();
            let mut info = vec![];
            if let Some(ref ip) = config.ip {
                info.push(format!("IP: {}", ip));
            }
            if let Some(ref hostname) = config.hostname {
                info.push(format!("Host: {}", hostname));
            }
            if let Some(ref tailscale) = config.tailscale {
                if config
                    .hostname
                    .as_ref()
                    .map(|h| h != tailscale)
                    .unwrap_or(true)
                {
                    info.push(format!("TS: {}", tailscale));
                }
            }
            let source_marker = match *source {
                "env" => "[env]",
                "db" => "[db]",
                "both" => "[env+db]",
                _ => "",
            };
            if info.is_empty() {
                println!("  {} {}", hostname, source_marker);
            } else {
                println!("  {} {} ({})", hostname, source_marker, info.join(", "));
            }
        }
        println!();
        println!("Use 'halvor list --verbose' for detailed information.");
    }

    Ok(())
}
