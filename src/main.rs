pub mod backup;
mod commands;
pub mod config;
pub mod config_manager;
pub mod crypto;
pub mod db;
pub mod docker;
pub mod exec;
pub mod networking;
pub mod npm;
pub mod portainer;
pub mod provision;
pub mod smb;
pub mod ssh;
pub mod sync;
pub mod tailscale;
pub mod update;
pub mod vpn;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "halvor")]
#[command(about = "Homelab Automation Layer - CLI tool for managing homelab infrastructure", long_about = None)]
#[command(version = commands::utils::get_version_string())]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Install Tailscale on the system
    Tailscale {
        #[command(subcommand)]
        command: TailscaleCommands,
    },
    /// Install Docker on the system
    Docker {
        #[command(subcommand)]
        command: DockerCommands,
    },
    /// Install Portainer on the system
    Portainer {
        #[command(subcommand)]
        command: PortainerCommands,
    },
    /// Provision a remote host (install Docker, Tailscale, Portainer)
    Provision {
        /// Hostname to provision
        hostname: String,
        /// Install Portainer host instead of Portainer Agent
        #[arg(long)]
        portainer_host: bool,
        /// Portainer edition to install (ce or be). Only used with --portainer-host
        #[arg(long, default_value = "ce")]
        portainer_edition: String,
    },
    /// Setup and mount SMB shares on a remote host
    Smb {
        /// Hostname to setup SMB mounts on
        hostname: String,
        /// Unmount and remove SMB mounts
        #[arg(long)]
        uninstall: bool,
    },
    /// Backup and restore Docker Compose data
    Backup {
        /// Hostname to backup/restore on
        hostname: String,
        #[command(subcommand)]
        command: BackupCommands,
    },
    /// Automatically create proxy hosts in Nginx Proxy Manager from compose file
    Npm {
        /// Hostname where Nginx Proxy Manager is running
        hostname: String,
        /// Docker compose file to read services from (e.g., media.docker-compose.yml)
        #[arg(default_value = "")]
        compose_file: String,
        /// Create proxy host for a specific service (e.g., portainer:9000 or npm:81)
        #[arg(long)]
        service: Option<String>,
    },
    /// Build and push VPN container image to GitHub Container Registry
    Vpn {
        #[command(subcommand)]
        command: VpnCommands,
    },
    /// Configure HAL settings (environment file location, etc.)
    /// Usage: halvor config [hostname] [command]
    Config {
        /// Optional positional argument - can be hostname or command
        #[arg(value_name = "HOSTNAME_OR_COMMAND")]
        arg: Option<String>,
        /// Show verbose output (including passwords)
        #[arg(short, long)]
        verbose: bool,
        /// Show database configuration instead of .env
        #[arg(long)]
        db: bool,
        #[command(subcommand)]
        command: Option<ConfigCommands>,
    },
    /// Sync encrypted data between hal installations
    Sync {
        /// Hostname to sync with
        hostname: String,
        /// Pull data from remote instead of pushing
        #[arg(long)]
        pull: bool,
    },
    /// Manage database migrations
    Migrate {
        #[command(subcommand)]
        command: MigrateCommands,
    },
    /// Check for and install updates
    Update {
        /// Use experimental channel for updates (versionless, continuously updated)
        #[arg(long)]
        experimental: bool,
        /// Force download and install the latest version (skips version check)
        #[arg(long)]
        force: bool,
    },
    /// Uninstall halvor (and old hal) from the system
    Uninstall {
        /// Skip confirmation prompt
        #[arg(long)]
        yes: bool,
    },
    /// List available servers/hosts
    List {
        /// Show verbose information about each server
        #[arg(long)]
        verbose: bool,
    },
}

#[derive(Subcommand)]
enum TailscaleCommands {
    /// Install Tailscale
    Install {
        /// Hostname to install Tailscale on (defaults to localhost)
        #[arg(default_value = "localhost")]
        hostname: String,
    },
}

#[derive(Subcommand)]
enum DockerCommands {
    /// Install Docker
    Install {
        /// Hostname to install Docker on (defaults to localhost)
        #[arg(default_value = "localhost")]
        hostname: String,
    },
}

#[derive(Subcommand)]
enum PortainerCommands {
    /// Install Portainer (host or agent)
    Install {
        /// Hostname to install Portainer on (defaults to localhost)
        #[arg(default_value = "localhost")]
        hostname: String,
        /// Portainer edition to install (ce or be)
        #[arg(long, default_value = "ce")]
        edition: String,
        /// Install Portainer host (with UI) instead of agent
        #[arg(long)]
        host: bool,
    },
}

#[derive(Subcommand)]
enum VpnCommands {
    /// Build and push VPN container image to GitHub Container Registry
    Build {
        /// GitHub username or organization
        #[arg(long)]
        github_user: String,
        /// Image tag (if not provided, pushes both 'latest' and git hash)
        #[arg(long)]
        tag: Option<String>,
    },
    /// Deploy VPN to a remote host (injects PIA credentials from local .env)
    Deploy {
        /// Hostname to deploy VPN to
        hostname: String,
    },
    /// Verify VPN is working correctly
    Verify {
        /// Hostname where VPN is running
        hostname: String,
    },
}

#[derive(Subcommand)]
enum BackupCommands {
    /// Create a backup of all Docker volumes on the host
    Create,
    /// List available backups
    List,
    /// Restore from a backup
    Restore {
        /// Optional: specific backup name (timestamp). If not provided, lists available backups
        #[arg(short, long)]
        backup: Option<String>,
    },
}

// Export command enums for use in command modules
#[derive(Subcommand, Clone)]
pub enum MigrateCommands {
    /// Run the next pending migration (migrate forward one)
    Up,
    /// Rollback the last applied migration (migrate backward one)
    Down,
    /// Show migration status
    Status,
    /// Generate a new migration file
    Generate {
        /// Migration description (e.g., "add users table")
        description: Vec<String>,
    },
    /// Alias for generate
    #[command(name = "g")]
    GenerateShort {
        /// Migration description (e.g., "add users table")
        description: Vec<String>,
    },
}

#[derive(Subcommand, Clone)]
pub enum ConfigCommands {
    /// Show current configuration
    Show,
    /// Initialize or update HAL configuration (interactive)
    Init,
    /// Set the environment file path
    SetEnv {
        /// Path to the .env file
        path: String,
    },
    /// Set release channel to stable
    #[command(name = "stable")]
    SetStable,
    /// Set release channel to experimental
    #[command(name = "experimental")]
    SetExperimental,
    /// Create new configuration
    Create {
        #[command(subcommand)]
        command: CreateConfigCommands,
    },
    /// Create example .env file
    Env,
    /// Backup SQLite database
    Db {
        #[command(subcommand)]
        command: DbCommands,
    },
    /// Set backup location (for current system if no hostname provided)
    Backup {
        /// Hostname to set backup location for (only used when called without hostname)
        hostname: Option<String>,
    },
    /// Commit host configuration to database (from .env to DB)
    Commit,
    /// Write host configuration back to .env file (from DB to .env)
    #[command(name = "backup")]
    BackupToEnv,
    /// Delete host configuration
    Delete {
        /// Also delete from .env file
        #[arg(long)]
        from_env: bool,
    },
    /// Set IP address for hostname
    Ip {
        /// IP address
        value: String,
    },
    /// Set hostname for hostname (primary hostname)
    Hostname {
        /// Hostname value
        value: String,
    },
    /// Set Tailscale hostname (optional, different from primary hostname)
    Tailscale {
        /// Tailscale hostname value
        value: String,
    },
    /// Set backup path for hostname
    BackupPath {
        /// Backup path
        value: String,
    },
    /// Show differences between .env and database configurations
    Diff,
}

#[derive(Subcommand, Clone)]
pub enum CreateConfigCommands {
    /// Create app configuration (backup location, etc.)
    App,
    /// Create SMB server configuration
    Smb {
        /// Server name
        server_name: Option<String>,
    },
    /// Create SSH host configuration
    Ssh {
        /// Hostname
        hostname: Option<String>,
    },
}

#[derive(Subcommand, Clone)]
pub enum DbCommands {
    /// Backup the SQLite database
    Backup {
        /// Path to save backup (defaults to current directory with timestamp)
        #[arg(long)]
        path: Option<String>,
    },
    /// Generate Rust structs from database schema
    Generate,
}

fn main() -> Result<()> {
    // Handle version flags before parsing (to show channel info)
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 2 && (args[1] == "--version" || args[1] == "-V") {
        commands::utils::print_version_with_channel();
        return Ok(());
    }

    // Check for updates (non-blocking, only in production mode)
    commands::utils::check_for_updates();

    let cli = Cli::parse();
    commands::handle_command(cli.command)?;

    Ok(())
}
