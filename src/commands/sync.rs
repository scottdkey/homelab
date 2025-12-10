use crate::config;
use crate::sync;
use anyhow::Result;

pub fn handle_sync(hostname: &str, pull: bool) -> Result<()> {
    let homelab_dir = config::find_homelab_dir()?;
    let config = config::load_env_config(&homelab_dir)?;
    sync::sync_data(hostname, pull, &config)?;
    Ok(())
}
