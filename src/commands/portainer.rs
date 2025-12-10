use crate::config;
use crate::portainer;
use anyhow::Result;

pub fn handle_portainer(hostname: &str, edition: &str, host: bool) -> Result<()> {
    let homelab_dir = config::find_homelab_dir()?;
    let config = config::load_env_config(&homelab_dir)?;
    if host {
        portainer::install_portainer_host(hostname, edition, &config)?;
    } else {
        portainer::install_portainer_agent(hostname, edition, &config)?;
    }
    Ok(())
}
