pub mod backup;
pub mod config;
pub mod docker;
pub mod list;
pub mod migrate;
pub mod npm;
pub mod portainer;
pub mod provision;
pub mod smb;
pub mod sync;
pub mod tailscale;
pub mod uninstall;
pub mod update;
pub mod utils;
pub mod vpn;

use crate::Commands;
use anyhow::Result;

/// Dispatch command to appropriate handler
pub fn handle_command(command: Commands) -> Result<()> {
    use crate::{
        BackupCommands, Commands::*, DockerCommands, PortainerCommands, TailscaleCommands,
        VpnCommands,
    };

    match command {
        Tailscale { command } => match command {
            TailscaleCommands::Install { hostname } => {
                tailscale::handle_tailscale(&hostname)?;
            }
        },
        Docker { command } => match command {
            DockerCommands::Install { hostname } => {
                docker::handle_docker(&hostname)?;
            }
        },
        Portainer { command } => match command {
            PortainerCommands::Install {
                hostname,
                edition,
                host,
            } => {
                portainer::handle_portainer(&hostname, &edition, host)?;
            }
        },
        Provision {
            hostname,
            portainer_host,
            portainer_edition,
        } => {
            provision::handle_provision(&hostname, portainer_host, &portainer_edition)?;
        }
        Smb {
            hostname,
            uninstall,
        } => {
            smb::handle_smb(&hostname, uninstall)?;
        }
        Backup { hostname, command } => {
            use backup::BackupCommand;
            let backup_cmd = match command {
                BackupCommands::Create => BackupCommand::Create,
                BackupCommands::List => BackupCommand::List,
                BackupCommands::Restore { backup } => BackupCommand::Restore { backup },
            };
            backup::handle_backup(&hostname, backup_cmd)?;
        }
        Npm {
            hostname,
            compose_file,
            service,
        } => {
            npm::handle_npm(&hostname, &compose_file, service.as_deref())?;
        }
        Vpn { command } => {
            use vpn::{VpnCommand, VpnCommand::*};
            let vpn_cmd = match command {
                VpnCommands::Build { github_user, tag } => Build { github_user, tag },
                VpnCommands::Deploy { hostname } => Deploy { hostname },
                VpnCommands::Verify { hostname } => Verify { hostname },
            };
            vpn::handle_vpn(vpn_cmd)?;
        }
        Update {
            experimental,
            force,
        } => {
            update::handle_update(experimental, force)?;
        }
        Config {
            arg,
            verbose,
            db,
            command,
        } => {
            config::handle_config(arg.as_deref(), verbose, db, command.as_ref())?;
        }
        Migrate { command } => {
            migrate::handle_migrate(command)?;
        }
        Sync { hostname, pull } => {
            sync::handle_sync(&hostname, pull)?;
        }
        Uninstall { yes } => {
            uninstall::handle_uninstall(yes)?;
        }
        List { verbose } => {
            list::handle_list(verbose)?;
        }
    }
    Ok(())
}
