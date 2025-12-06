mod config;
mod ssh;
mod tailscale;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "hal")]
#[command(about = "Homelab Automation Layer - CLI tool for managing homelab infrastructure", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Connect to a host via SSH (tries local IP, then Tailscale)
    Ssh {
        /// Hostname to connect to
        hostname: String,
        /// Username for SSH connection (if not provided, will prompt or use default)
        #[arg(long, short = 'u')]
        user: Option<String>,
        /// Remove offending host keys from known_hosts before connecting
        #[arg(long, short = 'f')]
        fix_keys: bool,
        /// Additional SSH arguments
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        ssh_args: Vec<String>,
    },
    /// Install Tailscale on the system
    Tailscale {
        #[command(subcommand)]
        command: TailscaleCommands,
    },
}

#[derive(Subcommand)]
enum TailscaleCommands {
    /// Install Tailscale
    Install,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Ssh {
            hostname,
            user,
            fix_keys,
            ssh_args,
        } => {
            let homelab_dir = config::find_homelab_dir()?;
            let config = config::load_env_config(&homelab_dir)?;
            ssh::ssh_to_host(&hostname, user, fix_keys, &ssh_args, &config)?;
        }
        Commands::Tailscale { command } => match command {
            TailscaleCommands::Install => tailscale::install_tailscale()?,
        },
    }

    Ok(())
}
