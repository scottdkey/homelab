use crate::MigrateCommands;
use crate::db;
use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

/// Handle migrate commands
pub fn handle_migrate(command: MigrateCommands) -> Result<()> {
    let conn = db::get_connection()?;

    match command {
        MigrateCommands::Up => {
            println!(
                "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
            );
            println!("Migrating database up (one migration)");
            println!(
                "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
            );
            db::migrations::migrate_up(&conn)?;
        }
        MigrateCommands::Down => {
            println!(
                "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
            );
            println!("Rolling back database (one migration)");
            println!(
                "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
            );
            db::migrations::migrate_down(&conn)?;
        }
        MigrateCommands::Status => {
            println!(
                "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
            );
            println!("Migration Status");
            println!(
                "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
            );
            println!();

            let status = db::migrations::get_migration_status(&conn)?;
            let current_version = db::migrations::get_current_migration_version(&conn)?;

            println!("Current version: {}", current_version);
            println!();
            println!(
                "{:<8} {:<40} {:<12} {:<12}",
                "Version", "Name", "Status", "Rollback"
            );
            println!("{}", "-".repeat(80));

            for (version, name, is_applied, can_rollback) in status {
                let status_str = if is_applied {
                    "✓ Applied"
                } else {
                    "  Pending"
                };
                let rollback_str = if can_rollback { "Yes" } else { "No" };
                println!(
                    "{:<8} {:<40} {:<12} {:<12}",
                    version, name, status_str, rollback_str
                );
            }
        }
        MigrateCommands::Generate { description }
        | MigrateCommands::GenerateShort { description } => {
            generate_migration(description, &conn)?;
        }
    }

    Ok(())
}

/// Generate a new migration file
fn generate_migration(description: Vec<String>, _conn: &rusqlite::Connection) -> Result<()> {
    // Migration generation is now manual - schemas are generated from the database
    // Users should write migrations manually, then run `halvor db generate` to update structs
    if description.is_empty() {
        anyhow::bail!(
            "Migration description is required. Example: halvor migrate generate add users table"
        );
    }

    let desc = description.join("_").to_lowercase().replace(" ", "_");
    create_migration_file(&desc, &[], &[])
}

/// Helper to create migration file
fn create_migration_file(desc: &str, up_sql: &[String], down_sql: &[String]) -> Result<()> {
    // Find the highest migration number
    let migrations_dir = PathBuf::from("src/db/migrations");
    let mut max_version = 0u32;

    if migrations_dir.exists() {
        for entry in fs::read_dir(&migrations_dir)? {
            let entry = entry?;
            let path = entry.path();
            if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                if file_name.ends_with(".rs") && file_name != "mod.rs" {
                    let parts: Vec<&str> = file_name.trim_end_matches(".rs").split('_').collect();
                    if let Some(version_str) = parts.first() {
                        if let Ok(version) = version_str.parse::<u32>() {
                            max_version = max_version.max(version);
                        }
                    }
                }
            }
        }
    }

    let next_version = max_version + 1;
    let version_str = format!("{:03}", next_version);
    let file_name = format!("{}_{}.rs", version_str, desc);
    let file_path = migrations_dir.join(&file_name);

    // Create migration file content
    let up_content = if up_sql.is_empty() {
        r#"    // TODO: Implement migration
    // Example:
    // conn.execute(
    //     "CREATE TABLE IF NOT EXISTS example (
    //         id TEXT PRIMARY KEY,
    //         name TEXT NOT NULL,
    //         created_at INTEGER NOT NULL,
    //         updated_at INTEGER NOT NULL
    //     )",
    //     [],
    // )
    // .context("Failed to create example table")?;
    
    Ok(())"#
            .to_string()
    } else {
        let mut content = String::new();
        for sql in up_sql {
            if sql.starts_with("--") {
                content.push_str(&format!("    {}\n", sql));
            } else {
                content.push_str(&format!(
                    "    conn.execute(\n        {:?},\n        [],\n    )\n    .context(\"Failed to execute migration\")?;\n\n",
                    sql
                ));
            }
        }
        content.push_str("    Ok(())");
        content
    };

    let down_content = if down_sql.is_empty() {
        r#"    // TODO: Implement rollback
    // Example:
    // conn.execute("DROP TABLE IF EXISTS example", [])
    //     .context("Failed to drop example table")?;
    
    Ok(())"#
            .to_string()
    } else {
        let mut content = String::new();
        for sql in down_sql {
            if sql.starts_with("--") {
                content.push_str(&format!("    {}\n", sql));
            } else {
                content.push_str(&format!(
                    "    conn.execute(\n        {:?},\n        [],\n    )\n    .context(\"Failed to execute rollback\")?;\n\n",
                    sql
                ));
            }
        }
        content.push_str("    Ok(())");
        content
    };

    let content = format!(
        r#"use anyhow::{{Context, Result}};
use rusqlite::Connection;

/// Migration {:03}: {}
pub fn up(conn: &Connection) -> Result<()> {{
{}

}}

/// Rollback: {}
pub fn down(conn: &Connection) -> Result<()> {{
{}

}}
"#,
        next_version,
        desc.replace("_", " "),
        up_content,
        format!("Undo {}", desc.replace("_", " ")),
        down_content
    );

    // Write file
    fs::write(&file_path, content)
        .with_context(|| format!("Failed to write migration file: {}", file_path.display()))?;

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("✓ Created migration file: {}", file_path.display());
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();
    println!("The migration will be automatically discovered on the next build.");

    Ok(())
}
