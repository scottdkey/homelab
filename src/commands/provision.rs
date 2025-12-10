use crate::config;
use crate::provision;
use anyhow::Result;

pub fn handle_provision(
    hostname: &str,
    portainer_host: bool,
    portainer_edition: &str,
) -> Result<()> {
    let homelab_dir = config::find_homelab_dir()?;
    let config = config::load_env_config(&homelab_dir)?;
    provision::provision_host(hostname, portainer_host, portainer_edition, &config)?;
    Ok(())
}
