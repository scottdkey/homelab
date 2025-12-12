// Use modules and types from lib.rs
use anyhow::Result;
use clap::Parser;
use halvor::Commands;

#[derive(Parser)]
#[command(name = "halvor")]
#[command(about = "Homelab Automation Layer - CLI tool for managing homelab infrastructure", long_about = None)]
#[command(version = halvor::commands::utils::get_version_string())]
struct Cli {
    /// Hostname to operate on (defaults to localhost if not provided)
    #[arg(long, short = 'H', value_name = "HOSTNAME", global = true)]
    hostname: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

fn main() -> Result<()> {
    // Handle version flags before parsing (to show channel info)
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 2 && (args[1] == "--version" || args[1] == "-V") {
        halvor::commands::utils::print_version_with_channel();
        return Ok(());
    }

    // Check for updates (non-blocking, only in production mode)
    halvor::commands::utils::check_for_updates();

    let cli = Cli::parse();
    halvor::commands::handle_command(cli.hostname, cli.command)?;

    Ok(())
}
