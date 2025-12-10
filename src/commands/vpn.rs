use crate::config;
use crate::vpn;
use anyhow::Result;

pub enum VpnCommand {
    Build {
        github_user: String,
        tag: Option<String>,
    },
    Deploy {
        hostname: String,
    },
    Verify {
        hostname: String,
    },
}

pub fn handle_vpn(command: VpnCommand) -> Result<()> {
    let config_dir = config::find_homelab_dir()?;
    let config = config::load_env_config(&config_dir)?;

    match command {
        VpnCommand::Build { github_user, tag } => {
            let build_hostname = "localhost";
            vpn::build_and_push_vpn_image(build_hostname, &github_user, tag.as_deref(), &config)?;
        }
        VpnCommand::Deploy { hostname } => {
            vpn::deploy_vpn(&hostname, &config)?;
        }
        VpnCommand::Verify { hostname } => {
            vpn::verify_vpn(&hostname, &config)?;
        }
    }
    Ok(())
}
