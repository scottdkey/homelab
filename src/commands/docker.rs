use crate::config;
use crate::docker;
use anyhow::Result;

pub fn handle_docker(hostname: &str) -> Result<()> {
    let homelab_dir = config::find_homelab_dir()?;
    let config = config::load_env_config(&homelab_dir)?;
    docker::install_docker(hostname, &config)?;
    Ok(())
}
