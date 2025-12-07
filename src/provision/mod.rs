// Provision module - organized into submodules for maintainability
mod local;
mod remote;
mod utils;

use crate::config::EnvConfig;
use anyhow::{Context, Result};

/// Portainer edition type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortainerEdition {
    Ce,
    Be,
}

impl PortainerEdition {
    pub fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "ce" => Ok(PortainerEdition::Ce),
            "be" | "business" | "business-edition" => Ok(PortainerEdition::Be),
            _ => anyhow::bail!("Invalid portainer edition: {}. Must be 'ce' or 'be'", s),
        }
    }

    pub fn compose_file(&self) -> &'static str {
        match self {
            PortainerEdition::Ce => "portainer.docker-compose.yml",
            PortainerEdition::Be => "portainer-be.docker-compose.yml",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            PortainerEdition::Ce => "Community Edition",
            PortainerEdition::Be => "Business Edition",
        }
    }
}

/// Main entry point for provisioning a host
pub fn provision_host(
    hostname: &str,
    portainer_host: bool,
    portainer_edition: &str,
    config: &EnvConfig,
) -> Result<()> {
    let edition = if portainer_host {
        PortainerEdition::from_str(portainer_edition)
            .with_context(|| format!("Invalid portainer edition: {}", portainer_edition))?
    } else {
        // Edition doesn't matter if not installing host
        PortainerEdition::Ce
    };

    // Check if we're running locally or remotely
    let is_local = utils::is_local_execution(hostname, config)?;

    if is_local {
        println!("Detected local execution on {}", hostname);
        println!();
        local::provision_local(hostname, portainer_host, edition, config)
    } else {
        println!("Detected remote execution - provisioning via SSH");
        println!();
        remote::provision_remote(hostname, portainer_host, edition, config)
    }
}
