use crate::config;
use crate::config::env_file;
use crate::config_manager;
use crate::db;
use anyhow::{Context, Result};
use chrono;
use std::io::{self, Write};
use std::path::PathBuf;

use crate::ConfigCommands;
use crate::CreateConfigCommands;
use crate::DbCommands;

/// Handle config commands
pub fn handle_config(
    arg: Option<&str>,
    verbose: bool,
    db: bool,
    command: Option<&ConfigCommands>,
) -> Result<()> {
    // Known global commands that should not be treated as hostnames
    let global_commands = [
        "show",
        "init",
        "set-env",
        "stable",
        "experimental",
        "create",
        "env",
        "db",
        "backup",
        "commit",
        "backup-to-env",
        "delete",
        "diff",
    ];

    // If arg is provided and it's not a known command, treat it as a hostname
    if let Some(arg_str) = arg {
        if !global_commands.contains(&arg_str.to_lowercase().as_str()) {
            // This is a hostname
            let hostname = arg_str;
            match command {
                None | Some(ConfigCommands::Show) => {
                    show_host_config(hostname)?;
                }
                Some(ConfigCommands::Commit) => {
                    commit_host_config_to_db(hostname)?;
                }
                Some(ConfigCommands::BackupToEnv) => {
                    backup_host_config_to_env(hostname)?;
                }
                Some(ConfigCommands::Delete { from_env }) => {
                    delete_host_config(hostname, *from_env)?;
                }
                Some(ConfigCommands::Ip { value }) => {
                    set_host_field(hostname, "ip", &value)?;
                }
                Some(ConfigCommands::Hostname { value }) => {
                    set_host_field(hostname, "hostname", &value)?;
                }
                Some(ConfigCommands::Tailscale { value }) => {
                    set_host_field(hostname, "tailscale", &value)?;
                }
                Some(ConfigCommands::BackupPath { value }) => {
                    set_host_field(hostname, "backup_path", &value)?;
                }
                Some(ConfigCommands::Backup { hostname: _ }) => {
                    // This shouldn't happen when hostname is provided, but handle it
                    set_backup_location(Some(hostname))?;
                }
                Some(ConfigCommands::Diff) => {
                    anyhow::bail!(
                        "Diff command is global only. Use 'halvor config diff' to see all differences"
                    );
                }
                _ => {
                    anyhow::bail!("Command not valid for hostname-specific operations");
                }
            }
            return Ok(());
        }
    }

    // Handle global config commands
    // If arg is a known command, use it; otherwise use the subcommand
    let cmd = if let Some(arg_str) = arg {
        // Map string to command
        match arg_str.to_lowercase().as_str() {
            "show" => ConfigCommands::Show,
            "init" => ConfigCommands::Init,
            "env" => ConfigCommands::Env,
            "stable" => ConfigCommands::SetStable,
            "experimental" => ConfigCommands::SetExperimental,
            "commit" => ConfigCommands::Commit,
            "backup" => ConfigCommands::BackupToEnv,
            "diff" => ConfigCommands::Diff,
            _ => {
                // Use the subcommand if provided, otherwise default to Show
                command.cloned().unwrap_or(ConfigCommands::Show)
            }
        }
    } else {
        command.cloned().unwrap_or(ConfigCommands::Show)
    };

    match cmd {
        ConfigCommands::Show => {
            if db {
                show_db_config(verbose)?;
            } else {
                show_current_config(verbose)?;
            }
        }
        ConfigCommands::Commit => {
            commit_all_to_db()?;
        }
        ConfigCommands::BackupToEnv => {
            backup_all_to_env()?;
        }
        ConfigCommands::Init => {
            config_manager::init_config_interactive()?;
        }
        ConfigCommands::SetEnv { path } => {
            set_env_path(path.as_str())?;
        }
        ConfigCommands::SetStable => {
            config_manager::set_release_channel(config_manager::ReleaseChannel::Stable)?;
        }
        ConfigCommands::SetExperimental => {
            config_manager::set_release_channel(config_manager::ReleaseChannel::Experimental)?;
        }
        ConfigCommands::Create { command } => {
            handle_create_config(command)?;
        }
        ConfigCommands::Env => {
            create_example_env_file()?;
        }
        ConfigCommands::Db { command } => match command {
            DbCommands::Generate => {
                db::core::generator::generate_structs()?;
            }
            DbCommands::Backup { path } => {
                backup_database(path.as_deref())?;
            }
        },
        ConfigCommands::Backup { hostname } => {
            set_backup_location(hostname.as_deref())?;
        }
        ConfigCommands::Delete { .. } => {
            anyhow::bail!(
                "Delete requires a hostname. Usage: halvor config <hostname> delete [--from-env]"
            );
        }
        ConfigCommands::Diff => {
            show_config_diff()?;
        }
        ConfigCommands::Ip { .. }
        | ConfigCommands::Hostname { .. }
        | ConfigCommands::Tailscale { .. }
        | ConfigCommands::BackupPath { .. } => {
            anyhow::bail!(
                "Field commands require a hostname. Usage: halvor config <hostname> <field> <value>"
            );
        }
    }
    Ok(())
}

/// Show current configuration
fn show_current_config(verbose: bool) -> Result<()> {
    let hal_config = config_manager::load_config()?;
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("HAL Configuration");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();

    // Show HAL settings
    if let Some(ref env_path) = hal_config.env_file_path {
        println!("Environment file: {}", env_path.display());
        if env_path.exists() {
            println!("  Status: ✓ Found");
        } else {
            println!("  Status: ✗ Not found");
        }
    } else {
        println!("Environment file: Not configured");
        println!("  Run 'halvor config init' to set it up");
    }

    println!();
    println!(
        "Config file location: {}",
        config_manager::get_config_file_path()?.display()
    );
    let channel_name = match hal_config.release_channel {
        config_manager::ReleaseChannel::Stable => "stable",
        config_manager::ReleaseChannel::Experimental => "experimental",
    };
    println!("Release channel: {}", channel_name);
    println!();
    if let Ok(db_path) = db::get_db_path() {
        println!("Database location: {}", db_path.display());
    }
    println!();

    // Show .env configuration
    let homelab_dir = config::find_homelab_dir();
    if let Ok(dir) = &homelab_dir {
        if let Ok(env_config) = config::load_env_config(dir) {
            println!(
                "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
            );
            println!("Environment Configuration (.env)");
            println!(
                "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
            );
            println!();

            // Show hosts
            if !env_config.hosts.is_empty() {
                println!("Hosts:");
                let mut hostnames: Vec<_> = env_config.hosts.keys().collect();
                hostnames.sort();
                for hostname in hostnames {
                    let config = &env_config.hosts[hostname];
                    println!("  {}", hostname);
                    if let Some(ref ip) = config.ip {
                        println!("    IP: {}", ip);
                    }
                    if let Some(ref hostname_val) = config.hostname {
                        println!("    Hostname: {}", hostname_val);
                    }
                    if let Some(ref tailscale) = config.tailscale {
                        if config
                            .hostname
                            .as_ref()
                            .map(|h| h.to_lowercase() != tailscale.to_lowercase())
                            .unwrap_or(true)
                        {
                            println!("    Tailscale: {}", tailscale);
                        }
                    }
                    if let Some(ref backup_path) = config.backup_path {
                        println!("    Backup Path: {}", backup_path);
                    }
                }
                println!();
            }

            // Show SMB servers
            if !env_config.smb_servers.is_empty() {
                println!("SMB Servers:");
                let mut server_names: Vec<_> = env_config.smb_servers.keys().collect();
                server_names.sort();
                for server_name in server_names {
                    let smb = &env_config.smb_servers[server_name];
                    println!("  {}", server_name);
                    println!("    Host: {}", smb.host);
                    if !smb.shares.is_empty() {
                        println!("    Shares: {}", smb.shares.join(", "));
                    }
                    if let Some(ref username) = smb.username {
                        println!("    Username: {}", username);
                    }
                    if let Some(ref password) = smb.password {
                        if verbose {
                            println!("    Password: {}", password);
                        } else {
                            println!("    Password: {}", "*".repeat(password.len().min(8)));
                        }
                    }
                    if let Some(ref options) = smb.options {
                        println!("    Options: {}", options);
                    }
                }
                println!();
            }

            if env_config.hosts.is_empty() && env_config.smb_servers.is_empty() {
                println!("No configuration found in .env file");
                println!();
            }
        }
    }

    Ok(())
}

/// Set environment file path
fn set_env_path(path: &str) -> Result<()> {
    let env_path = std::path::PathBuf::from(path);
    let env_path = if env_path.is_relative() {
        std::env::current_dir()?.join(&env_path)
    } else {
        env_path
    };
    let env_path = env_path
        .canonicalize()
        .with_context(|| format!("Failed to resolve path: {}", path))?;

    if !env_path.exists() {
        anyhow::bail!("File does not exist: {}", env_path.display());
    }

    config_manager::set_env_file_path(&env_path)?;
    Ok(())
}

/// Show host configuration and source
fn show_host_config(hostname: &str) -> Result<()> {
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Host Configuration: {}", hostname);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();

    // Check env file
    let homelab_dir = config::find_homelab_dir();
    let env_config = if let Ok(dir) = &homelab_dir {
        config::load_env_config(dir)
            .ok()
            .and_then(|cfg| cfg.hosts.get(hostname).map(|c| ("env", c.clone())))
    } else {
        None
    };

    // Check database
    let db_config = db::get_host_config(hostname)
        .ok()
        .flatten()
        .map(|c| ("db", c));

    if env_config.is_none() && db_config.is_none() {
        anyhow::bail!(
            "Host '{}' not found in environment file or database",
            hostname
        );
    }

    if let Some((_source, config)) = env_config {
        println!("Source: Environment file (.env)");
        println!(
            "  IP Address: {}",
            config.ip.as_deref().unwrap_or("Not set")
        );
        println!(
            "  Hostname: {}",
            config.hostname.as_deref().unwrap_or("Not set")
        );
        if let Some(ref tailscale) = config.tailscale {
            if config
                .hostname
                .as_ref()
                .map(|h| h.to_lowercase() != tailscale.to_lowercase())
                .unwrap_or(true)
            {
                println!("  Tailscale: {} (different from hostname)", tailscale);
            }
        }
        println!(
            "  Backup Path: {}",
            config.backup_path.as_deref().unwrap_or("Not set")
        );
        println!();
    }

    if let Some((_source, config)) = db_config {
        println!("Source: Database (SQLite)");
        println!(
            "  IP Address: {}",
            config.ip.as_deref().unwrap_or("Not set")
        );
        println!(
            "  Hostname: {}",
            config.hostname.as_deref().unwrap_or("Not set")
        );
        if let Some(ref tailscale) = config.tailscale {
            if config
                .hostname
                .as_ref()
                .map(|h| h.to_lowercase() != tailscale.to_lowercase())
                .unwrap_or(true)
            {
                println!("  Tailscale: {} (different from hostname)", tailscale);
            }
        }
        println!(
            "  Backup Path: {}",
            config.backup_path.as_deref().unwrap_or("Not set")
        );
        println!();
    }

    Ok(())
}

/// Show differences between .env and database configurations
fn show_config_diff() -> Result<()> {
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Configuration Differences (.env vs Database)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();

    // Load .env config
    let homelab_dir = config::find_homelab_dir();
    let env_config = if let Ok(dir) = &homelab_dir {
        config::load_env_config(dir).ok()
    } else {
        None
    };

    // Load DB configs
    let db_hosts = db::list_hosts().ok().unwrap_or_default();
    let mut db_configs = std::collections::HashMap::new();
    for hostname in &db_hosts {
        if let Ok(Some(config)) = db::get_host_config(hostname) {
            db_configs.insert(hostname.clone(), config);
        }
    }

    // Get all unique hostnames
    let mut all_hostnames = std::collections::HashSet::new();
    if let Some(ref env_cfg) = env_config {
        for hostname in env_cfg.hosts.keys() {
            all_hostnames.insert(hostname.clone());
        }
    }
    for hostname in db_hosts {
        all_hostnames.insert(hostname);
    }

    if all_hostnames.is_empty() {
        println!("No hosts found in .env file or database");
        return Ok(());
    }

    let mut hostnames: Vec<_> = all_hostnames.iter().collect();
    hostnames.sort();

    let mut has_differences = false;

    for hostname in hostnames {
        let env_host = env_config.as_ref().and_then(|c| c.hosts.get(hostname));
        let db_host = db_configs.get(hostname);

        let mut host_has_diff = false;

        // Check IP
        let env_ip = env_host.and_then(|h| h.ip.as_ref());
        let db_ip = db_host.and_then(|h| h.ip.as_ref());
        if env_ip != db_ip {
            host_has_diff = true;
            println!("Host: {}", hostname);
            println!("  IP Address:");
            if let Some(ip) = env_ip {
                println!("    .env:     {}", ip);
            } else {
                println!("    .env:     (not set)");
            }
            if let Some(ip) = db_ip {
                println!("    database: {}", ip);
            } else {
                println!("    database: (not set)");
            }
        }

        // Check Hostname
        let env_hostname = env_host.and_then(|h| h.hostname.as_ref());
        let db_hostname = db_host.and_then(|h| h.hostname.as_ref());
        if env_hostname != db_hostname {
            if !host_has_diff {
                println!("Host: {}", hostname);
            }
            host_has_diff = true;
            println!("  Hostname:");
            if let Some(hn) = env_hostname {
                println!("    .env:     {}", hn);
            } else {
                println!("    .env:     (not set)");
            }
            if let Some(hn) = db_hostname {
                println!("    database: {}", hn);
            } else {
                println!("    database: (not set)");
            }
        }

        // Check Tailscale
        let env_tailscale = env_host.and_then(|h| h.tailscale.as_ref());
        let db_tailscale = db_host.and_then(|h| h.tailscale.as_ref());
        if env_tailscale != db_tailscale {
            if !host_has_diff {
                println!("Host: {}", hostname);
            }
            host_has_diff = true;
            println!("  Tailscale:");
            if let Some(ts) = env_tailscale {
                println!("    .env:     {}", ts);
            } else {
                println!("    .env:     (not set)");
            }
            if let Some(ts) = db_tailscale {
                println!("    database: {}", ts);
            } else {
                println!("    database: (not set)");
            }
        }

        // Check Backup Path
        let env_backup = env_host.and_then(|h| h.backup_path.as_ref());
        let db_backup = db_host.and_then(|h| h.backup_path.as_ref());
        if env_backup != db_backup {
            if !host_has_diff {
                println!("Host: {}", hostname);
            }
            host_has_diff = true;
            println!("  Backup Path:");
            if let Some(bp) = env_backup {
                println!("    .env:     {}", bp);
            } else {
                println!("    .env:     (not set)");
            }
            if let Some(bp) = db_backup {
                println!("    database: {}", bp);
            } else {
                println!("    database: (not set)");
            }
        }

        if host_has_diff {
            println!();
            has_differences = true;
        }
    }

    if !has_differences {
        println!("✓ No differences found - .env and database configurations match");
    }

    Ok(())
}

/// Show database configuration
fn show_db_config(verbose: bool) -> Result<()> {
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Database Configuration");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();

    if let Ok(db_path) = db::get_db_path() {
        println!("Database location: {}", db_path.display());
        println!();
    }

    // Load all hosts from database
    let db_hosts = db::list_hosts()?;

    if db_hosts.is_empty() {
        println!("No hosts found in database");
        return Ok(());
    }

    println!("Hosts:");
    let mut hostnames: Vec<_> = db_hosts.iter().collect();
    hostnames.sort();

    for hostname in hostnames {
        if let Ok(Some(config)) = db::get_host_config(hostname) {
            println!("  {}", hostname);
            if let Some(ref ip) = config.ip {
                println!("    IP Address: {}", ip);
            }
            if let Some(ref hostname_val) = config.hostname {
                println!("    Hostname: {}", hostname_val);
            }
            if let Some(ref tailscale) = config.tailscale {
                if config
                    .hostname
                    .as_ref()
                    .map(|h| h.to_lowercase() != tailscale.to_lowercase())
                    .unwrap_or(true)
                {
                    println!("    Tailscale: {} (different from hostname)", tailscale);
                }
            }
            if let Some(ref backup_path) = config.backup_path {
                println!("    Backup Path: {}", backup_path);
            }

            // Show provisioning info if available
            if let Ok(Some((
                _last_provisioned,
                docker_version,
                tailscale_installed,
                portainer_installed,
                _metadata,
            ))) = db::get_host_info(hostname)
            {
                if docker_version.is_some() || tailscale_installed || portainer_installed {
                    println!("    Provisioning:");
                    if let Some(ref dv) = docker_version {
                        println!("      Docker: {}", dv);
                    }
                    if tailscale_installed {
                        println!("      Tailscale: ✓ Installed");
                    }
                    if portainer_installed {
                        println!("      Portainer: ✓ Installed");
                    }
                }
            }
            println!();
        }
    }

    // Show SMB servers from database
    let db_smb_servers = db::list_smb_servers()?;
    if !db_smb_servers.is_empty() {
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("SMB Servers");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!();

        for server_name in &db_smb_servers {
            if let Ok(Some(smb)) = db::get_smb_server(server_name) {
                println!("  {}", server_name);
                println!("    Host: {}", smb.host);
                if !smb.shares.is_empty() {
                    println!("    Shares: {}", smb.shares.join(", "));
                }
                if let Some(ref username) = smb.username {
                    println!("    Username: {}", username);
                }
                if let Some(ref password) = smb.password {
                    if verbose {
                        println!("    Password: {}", password);
                    } else {
                        println!("    Password: {}", "*".repeat(password.len().min(8)));
                    }
                }
                if let Some(ref options) = smb.options {
                    println!("    Options: {}", options);
                }
                println!();
            }
        }
    }

    Ok(())
}

/// Commit all host configurations from env file to database
fn commit_all_to_db() -> Result<()> {
    let homelab_dir = config::find_homelab_dir()?;
    let env_config = config::load_env_config(&homelab_dir)?;

    let mut committed_hosts = 0;
    let mut committed_smb = 0;

    // Commit hosts
    if !env_config.hosts.is_empty() {
        for (hostname, host_config) in &env_config.hosts {
            db::store_host_config(hostname, host_config)?;
            committed_hosts += 1;
        }
    }

    // Commit SMB servers
    if !env_config.smb_servers.is_empty() {
        for (server_name, smb_config) in &env_config.smb_servers {
            db::store_smb_server(server_name, smb_config)?;
            committed_smb += 1;
        }
    }

    if committed_hosts == 0 && committed_smb == 0 {
        println!("No hosts or SMB servers found in .env file to commit");
        return Ok(());
    }

    if committed_hosts > 0 {
        println!(
            "✓ Committed {} host configuration(s) to database",
            committed_hosts
        );
    }
    if committed_smb > 0 {
        println!(
            "✓ Committed {} SMB server configuration(s) to database",
            committed_smb
        );
    }

    Ok(())
}

/// Backup all host configurations from database to env file
fn backup_all_to_env() -> Result<()> {
    let all_hosts = db::list_hosts()?;

    if all_hosts.is_empty() {
        println!("No hosts found in database to backup");
        return Ok(());
    }

    let env_path = config_manager::get_env_file_path().ok_or_else(|| {
        anyhow::anyhow!("Environment file path not configured. Run 'halvor config init' first")
    })?;
    let mut backed_up = 0;

    for hostname in &all_hosts {
        if let Some(config) = db::get_host_config(hostname)? {
            env_file::write_host_to_env_file(&env_path, hostname, &config)?;
            backed_up += 1;
        }
    }

    println!(
        "✓ Backed up {} host configuration(s) to .env file",
        backed_up
    );
    Ok(())
}

/// Commit host configuration from env file to database
fn commit_host_config_to_db(hostname: &str) -> Result<()> {
    let homelab_dir = config::find_homelab_dir()?;
    let env_config = config::load_env_config(&homelab_dir)?;

    let host_config = env_config
        .hosts
        .get(hostname)
        .ok_or_else(|| anyhow::anyhow!("Host '{}' not found in environment file", hostname))?;

    db::store_host_config(hostname, host_config)?;
    println!(
        "✓ Committed host configuration for '{}' to database",
        hostname
    );
    Ok(())
}

/// Backup host configuration from database to env file
fn backup_host_config_to_env(hostname: &str) -> Result<()> {
    let config = db::get_host_config(hostname)?
        .ok_or_else(|| anyhow::anyhow!("Host '{}' not found in database", hostname))?;

    let env_path = config_manager::get_env_file_path().ok_or_else(|| {
        anyhow::anyhow!("Environment file path not configured. Run 'halvor config init' first")
    })?;
    env_file::write_host_to_env_file(&env_path, hostname, &config)?;

    println!(
        "✓ Backed up host configuration for '{}' to .env file",
        hostname
    );
    Ok(())
}

/// Delete host configuration
fn delete_host_config(hostname: &str, from_env: bool) -> Result<()> {
    if from_env {
        print!("⚠ Warning: This will delete the host from the .env file. Continue? [y/N]: ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") && !input.trim().eq_ignore_ascii_case("yes") {
            println!("Deletion cancelled.");
            return Ok(());
        }
        let env_path = config_manager::get_env_file_path().ok_or_else(|| {
            anyhow::anyhow!("Environment file path not configured. Run 'halvor config init' first")
        })?;
        env_file::remove_host_from_env_file(&env_path, hostname)?;
        println!("✓ Deleted host '{}' from .env file", hostname);
    }

    db::delete_host_config(hostname)?;
    println!("✓ Deleted host '{}' from database", hostname);
    Ok(())
}

/// Handle create config commands
fn handle_create_config(command: CreateConfigCommands) -> Result<()> {
    match command {
        CreateConfigCommands::App => {
            create_app_config()?;
        }
        CreateConfigCommands::Smb { server_name } => {
            create_smb_config(server_name.as_deref())?;
        }
        CreateConfigCommands::Ssh { hostname } => {
            create_ssh_config(hostname.as_deref())?;
        }
    }
    Ok(())
}

/// Create app configuration interactively
fn create_app_config() -> Result<()> {
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Create App Configuration");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();
    println!("⚠ App configuration creation not yet implemented");
    Ok(())
}

/// Create SMB configuration interactively
fn create_smb_config(server_name: Option<&str>) -> Result<()> {
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Create SMB Server Configuration");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();
    println!("⚠ SMB configuration creation not yet implemented");
    Ok(())
}

/// Create SSH host configuration interactively
fn create_ssh_config(hostname: Option<&str>) -> Result<()> {
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Create SSH Host Configuration");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();
    println!("⚠ SSH configuration creation not yet implemented");
    Ok(())
}

/// Create example .env file
fn create_example_env_file() -> Result<()> {
    let env_path = config_manager::get_env_file_path().ok_or_else(|| {
        anyhow::anyhow!("Environment file path not configured. Run 'halvor config init' first")
    })?;
    if env_path.exists() {
        print!(
            "⚠ .env file already exists at {}. Overwrite? [y/N]: ",
            env_path.display()
        );
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") && !input.trim().eq_ignore_ascii_case("yes") {
            println!("Cancelled.");
            return Ok(());
        }
    }

    let example_content = r#"# Tailnet base domain
TAILNET_BASE=ts.net

# Host configurations
# Format: HOST_<HOSTNAME>_IP, HOST_<HOSTNAME>_TAILSCALE, HOST_<HOSTNAME>_BACKUP_PATH
# Example:
# HOST_bellerophon_IP=10.10.10.14
# HOST_bellerophon_TAILSCALE=bellerophon
# HOST_bellerophon_BACKUP_PATH=/backup/bellerophon

# SMB Server configurations
# Format: SMB_<SERVERNAME>_HOST, SMB_<SERVERNAME>_SHARES, SMB_<SERVERNAME>_USERNAME, SMB_<SERVERNAME>_PASSWORD
# Example:
# SMB_nas_HOST=nas.local
# SMB_nas_SHARES=media,documents,backups
# SMB_nas_USERNAME=user
# SMB_nas_PASSWORD=password
"#;

    std::fs::write(&env_path, example_content).with_context(|| {
        format!(
            "Failed to write example .env file to {}",
            env_path.display()
        )
    })?;

    println!("✓ Created example .env file at {}", env_path.display());
    Ok(())
}

/// Backup SQLite database
fn backup_database(path: Option<&str>) -> Result<()> {
    let db_path = db::get_db_path()?;
    if !db_path.exists() {
        anyhow::bail!("Database not found at {}", db_path.display());
    }

    let backup_path = if let Some(p) = path {
        PathBuf::from(p)
    } else {
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        std::env::current_dir()?.join(format!("halvor_backup_{}.db", timestamp))
    };

    std::fs::copy(&db_path, &backup_path).with_context(|| {
        format!(
            "Failed to copy database from {} to {}",
            db_path.display(),
            backup_path.display()
        )
    })?;

    println!("✓ Database backed up to {}", backup_path.display());
    Ok(())
}

/// Set backup location
fn set_backup_location(hostname: Option<&str>) -> Result<()> {
    let target_hostname = hostname.unwrap_or("localhost");

    print!("Enter backup path for {}: ", target_hostname);
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let backup_path = input.trim().to_string();

    if backup_path.is_empty() {
        anyhow::bail!("Backup path cannot be empty");
    }

    set_host_field(target_hostname, "backup_path", &backup_path)?;
    Ok(())
}

/// Set a specific field for a host (preserves other fields, auto-writes to .env)
fn set_host_field(hostname: &str, field: &str, value: &str) -> Result<()> {
    // Get existing config from both sources
    let homelab_dir = config::find_homelab_dir();
    let env_config = if let Ok(dir) = &homelab_dir {
        config::load_env_config(dir).ok()
    } else {
        None
    };
    let db_config = db::get_host_config(hostname).ok().flatten();

    // Merge configs (env takes precedence, then db, then defaults)
    let mut merged = config::HostConfig {
        ip: env_config
            .as_ref()
            .and_then(|c| c.hosts.get(hostname))
            .and_then(|c| c.ip.clone())
            .or_else(|| db_config.as_ref().and_then(|c| c.ip.clone())),
        hostname: env_config
            .as_ref()
            .and_then(|c| c.hosts.get(hostname))
            .and_then(|c| c.hostname.clone())
            .or_else(|| db_config.as_ref().and_then(|c| c.hostname.clone())),
        tailscale: env_config
            .as_ref()
            .and_then(|c| c.hosts.get(hostname))
            .and_then(|c| c.tailscale.clone())
            .or_else(|| db_config.as_ref().and_then(|c| c.tailscale.clone())),
        backup_path: env_config
            .as_ref()
            .and_then(|c| c.hosts.get(hostname))
            .and_then(|c| c.backup_path.clone())
            .or_else(|| db_config.as_ref().and_then(|c| c.backup_path.clone())),
    };

    // Update the specific field
    match field {
        "ip" => {
            merged.ip = Some(value.to_string());
        }
        "hostname" => {
            // Warn if hostname differs from the key
            if value.to_lowercase() != hostname.to_lowercase() {
                println!(
                    "⚠ Warning: Hostname '{}' differs from key '{}'",
                    value, hostname
                );
                println!("  This may cause confusion. Consider using the hostname as the key.");
            }
            merged.hostname = Some(value.to_string());
        }
        "tailscale" => {
            merged.tailscale = Some(value.to_string());
        }
        "backup_path" => {
            merged.backup_path = Some(value.to_string());
        }
        _ => {
            anyhow::bail!("Unknown field: {}", field);
        }
    }

    // Write to .env file
    let env_path = config_manager::get_env_file_path().ok_or_else(|| {
        anyhow::anyhow!("Environment file path not configured. Run 'halvor config init' first")
    })?;
    env_file::write_host_to_env_file(&env_path, hostname, &merged)?;

    // Also update database
    db::store_host_config(hostname, &merged)?;

    println!(
        "✓ Set {} for '{}' to '{}' (written to .env and database)",
        field, hostname, value
    );
    Ok(())
}
