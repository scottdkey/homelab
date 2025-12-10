use crate::config;
use crate::npm;
use anyhow::Result;

pub fn handle_npm(hostname: &str, compose_file: &str, service: Option<&str>) -> Result<()> {
    let homelab_dir = config::find_homelab_dir()?;
    let config = config::load_env_config(&homelab_dir)?;
    let rt = tokio::runtime::Runtime::new()?;
    if let Some(service_spec) = service {
        rt.block_on(npm::setup_single_proxy_host(
            hostname,
            service_spec,
            &config,
        ))?;
    } else if !compose_file.is_empty() {
        rt.block_on(npm::setup_proxy_hosts(hostname, compose_file, &config))?;
    } else {
        anyhow::bail!("Either --service or compose_file must be provided");
    }
    Ok(())
}
