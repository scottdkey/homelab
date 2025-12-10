use crate::config_manager;
use crate::db;
use anyhow::Result;
use std::env;
use std::io::{self, Write};
use std::path::Path;

/// Handle uninstall command
pub fn handle_uninstall(skip_confirmation: bool) -> Result<()> {
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Uninstall halvor");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();

    // Collect all binaries and backup files to remove
    let home = env::var("HOME").or_else(|_| env::var("USERPROFILE"))?;
    let locations = vec![
        format!("{}/.cargo/bin/hal", home),
        format!("{}/.cargo/bin/halvor", home),
        format!("{}/.local/bin/hal", home),
        format!("{}/.local/bin/halvor", home),
        "/usr/local/bin/hal".to_string(),
        "/usr/local/bin/halvor".to_string(),
        "/usr/bin/hal".to_string(),
        "/usr/bin/halvor".to_string(),
    ];

    let mut binaries_to_remove = Vec::new();
    let mut backups_to_remove = Vec::new();

    // Check for binaries
    for path_str in &locations {
        let path = Path::new(path_str);
        if path.exists() {
            binaries_to_remove.push(path_str.clone());
        }
    }

    // Check for backup files
    let backup_patterns = vec![
        format!("{}/.cargo/bin/hal*.bak", home),
        format!("{}/.cargo/bin/halvor*.bak", home),
        format!("{}/.local/bin/hal*.bak", home),
        format!("{}/.local/bin/halvor*.bak", home),
        "/usr/local/bin/hal*.bak".to_string(),
        "/usr/local/bin/halvor*.bak".to_string(),
        "/usr/bin/hal*.bak".to_string(),
        "/usr/bin/halvor*.bak".to_string(),
    ];

    // Use glob to find backup files
    for pattern in &backup_patterns {
        if let Ok(entries) = glob::glob(pattern) {
            for entry in entries.flatten() {
                if entry.is_file() {
                    backups_to_remove.push(entry.to_string_lossy().to_string());
                }
            }
        }
    }

    if binaries_to_remove.is_empty() && backups_to_remove.is_empty() {
        println!("✓ No hal or halvor binaries or backup files found to remove.");
        println!("Checked locations:");
        for loc in &locations {
            println!("  - {}", loc);
        }
        return Ok(());
    }

    // Show what will be removed
    println!("The following files will be removed:");
    println!();
    if !binaries_to_remove.is_empty() {
        println!("Binaries:");
        for bin in &binaries_to_remove {
            println!("  - {}", bin);
        }
        println!();
    }
    if !backups_to_remove.is_empty() {
        println!("Backup files:");
        for backup in &backups_to_remove {
            println!("  - {}", backup);
        }
        println!();
    }

    // Ask for confirmation
    if !skip_confirmation {
        print!("Are you sure you want to continue? [y/N]: ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") && !input.trim().eq_ignore_ascii_case("yes") {
            println!("Uninstall cancelled.");
            return Ok(());
        }
    }

    println!();
    println!("Removing files...");

    // Remove binaries
    for bin_path in &binaries_to_remove {
        let path = Path::new(bin_path);
        if path.exists() {
            if bin_path.starts_with("/usr") {
                // System path, need sudo
                println!("  Removing {} (requires sudo)...", bin_path);
                let output = std::process::Command::new("sudo")
                    .arg("rm")
                    .arg("-f")
                    .arg(bin_path)
                    .output()?;
                if !output.status.success() {
                    eprintln!("  ⚠ Warning: Failed to remove {}", bin_path);
                } else {
                    println!("  ✓ Removed {}", bin_path);
                }
            } else {
                // User path, no sudo needed
                println!("  Removing {}...", bin_path);
                if let Err(e) = std::fs::remove_file(bin_path) {
                    eprintln!("  ⚠ Warning: Failed to remove {}: {}", bin_path, e);
                } else {
                    println!("  ✓ Removed {}", bin_path);
                }
            }
        }
    }

    // Remove backup files
    for backup_path in &backups_to_remove {
        let path = Path::new(backup_path);
        if path.exists() {
            if backup_path.starts_with("/usr") {
                // System path, need sudo
                println!("  Removing {} (requires sudo)...", backup_path);
                let output = std::process::Command::new("sudo")
                    .arg("rm")
                    .arg("-f")
                    .arg(backup_path)
                    .output()?;
                if !output.status.success() {
                    eprintln!("  ⚠ Warning: Failed to remove {}", backup_path);
                } else {
                    println!("  ✓ Removed {}", backup_path);
                }
            } else {
                // User path, no sudo needed
                println!("  Removing {}...", backup_path);
                if let Err(e) = std::fs::remove_file(backup_path) {
                    eprintln!("  ⚠ Warning: Failed to remove {}: {}", backup_path, e);
                } else {
                    println!("  ✓ Removed {}", backup_path);
                }
            }
        }
    }

    println!();

    // Ask about removing database and config data
    let mut remove_data = false;
    if !skip_confirmation {
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("⚠️  WARNING: Database and Configuration Removal");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!();
        println!("This will permanently delete:");
        println!("  - Database file (halvor.db) - contains all stored data");
        println!("  - Configuration file (config.toml) - contains your settings");
        println!("  - Encryption key (.halvor_key) - used to decrypt stored data");
        println!();
        println!("⚠️  This action is IRREVERSIBLE!");
        println!("   All your current configuration and stored data will be lost.");
        println!();
        print!("Do you want to remove database and configuration data? [y/N]: ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if input.trim().eq_ignore_ascii_case("y") || input.trim().eq_ignore_ascii_case("yes") {
            remove_data = true;
        }
    }

    if remove_data {
        println!();
        println!("Removing database and configuration data...");

        // Remove database file
        if let Ok(db_path) = db::get_db_path() {
            if db_path.exists() {
                println!("  Removing database: {}", db_path.display());
                if let Err(e) = std::fs::remove_file(&db_path) {
                    eprintln!("  ⚠ Warning: Failed to remove database: {}", e);
                } else {
                    println!("  ✓ Removed database");
                }
            }
        }

        // Remove config file
        if let Ok(config_path) = config_manager::get_config_file_path() {
            if config_path.exists() {
                println!("  Removing config file: {}", config_path.display());
                if let Err(e) = std::fs::remove_file(&config_path) {
                    eprintln!("  ⚠ Warning: Failed to remove config file: {}", e);
                } else {
                    println!("  ✓ Removed config file");
                }
            }
        }

        // Remove encryption key
        if let Ok(config_dir) = config_manager::get_config_dir() {
            let key_path = config_dir.join(".halvor_key");
            if key_path.exists() {
                println!("  Removing encryption key: {}", key_path.display());
                if let Err(e) = std::fs::remove_file(&key_path) {
                    eprintln!("  ⚠ Warning: Failed to remove encryption key: {}", e);
                } else {
                    println!("  ✓ Removed encryption key");
                }
            }

            // Try to remove the config directory if it's empty
            if let Ok(mut entries) = std::fs::read_dir(&config_dir) {
                if entries.next().is_none() {
                    if let Err(e) = std::fs::remove_dir(&config_dir) {
                        eprintln!("  ⚠ Warning: Failed to remove config directory: {}", e);
                    } else {
                        println!("  ✓ Removed config directory");
                    }
                }
            }
        }
    } else if !skip_confirmation {
        println!();
        println!("Database and configuration data preserved.");
        println!(
            "  - Database: {}",
            db::get_db_path()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| "unknown".to_string())
        );
        println!(
            "  - Config: {}",
            config_manager::get_config_file_path()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| "unknown".to_string())
        );
    }

    println!();
    println!("✓ Uninstall complete!");
    Ok(())
}
