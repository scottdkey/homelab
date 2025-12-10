pub mod core;
pub mod generated;
pub mod migrations;

use crate::config_manager;
use anyhow::{Context, Result};
use rusqlite::Connection;
use std::path::PathBuf;

const DB_FILE_NAME: &str = "halvor.db";

/// Get the database file path (in the config directory)
pub fn get_db_path() -> Result<PathBuf> {
    let config_dir = config_manager::get_config_dir()?;
    Ok(config_dir.join(DB_FILE_NAME))
}

/// Initialize the database and run migrations
///
/// This function automatically runs all pending migrations when the database is first accessed.
/// Migrations are run sequentially in order, ensuring the database schema is always up to date.
pub fn init_db() -> Result<Connection> {
    let db_path = get_db_path()?;
    let conn = Connection::open(&db_path)
        .with_context(|| format!("Failed to open database: {}", db_path.display()))?;

    // Run migrations to set up/update schema
    // This happens automatically on every database access to ensure schema is current
    migrations::run_migrations(&conn)?;

    Ok(conn)
}

/// Get a database connection
pub fn get_connection() -> Result<Connection> {
    init_db()
}

/// Get a database client for custom SQL queries
pub fn get_client() -> Result<core::DbClient> {
    let conn = get_connection()?;
    Ok(core::DbClient::new(conn))
}

// Re-export generated functions for convenience
pub use generated::*;
