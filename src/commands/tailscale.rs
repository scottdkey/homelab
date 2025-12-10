use crate::config;
use crate::tailscale;
use anyhow::Result;

pub fn handle_tailscale(hostname: &str) -> Result<()> {
    if hostname == "localhost" {
        tailscale::install_tailscale()?;
    } else {
        let homelab_dir = config::find_homelab_dir()?;
        let config = config::load_env_config(&homelab_dir)?;
        tailscale::install_tailscale_on_host(hostname, &config)?;
    }
    Ok(())
}
