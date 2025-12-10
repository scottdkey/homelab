use crate::config;
use crate::smb;
use anyhow::Result;

pub fn handle_smb(hostname: &str, uninstall: bool) -> Result<()> {
    let homelab_dir = config::find_homelab_dir()?;
    let config = config::load_env_config(&homelab_dir)?;
    if uninstall {
        smb::uninstall_smb_mounts(hostname, &config)?;
    } else {
        smb::setup_smb_mounts(hostname, &config)?;
    }
    Ok(())
}
