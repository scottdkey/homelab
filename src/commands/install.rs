use crate::config;
use crate::services;
use anyhow::{Result, Context};

/// Handle install command
/// hostname: None = local, Some(hostname) = remote host
pub fn handle_install(
    hostname: Option<&str>,
    service: &str,
    edition: &str,
    host: bool,
) -> Result<()> {
    let config = config::load_config()?;
    let target_host = hostname.unwrap_or("localhost");

    match service.to_lowercase().as_str() {
        "docker" => {
            services::docker::install_docker(target_host, &config)?;
        }
        "tailscale" => {
            if target_host == "localhost" {
                services::tailscale::install_tailscale()?;
            } else {
                services::tailscale::install_tailscale_on_host(target_host, &config)?;
            }
        }
        "portainer" => {
            if host {
                services::portainer::install_portainer_host(target_host, edition, &config)?;
            } else {
                services::portainer::install_portainer_agent(target_host, edition, &config)?;
            }
        }
        "npm" => {
            anyhow::bail!(
                "NPM installation not yet implemented. Use 'halvor {} npm' to configure proxy hosts",
                target_host
            );
        }
        "cli" => {
            install_cli_dependencies()?;
        }
        _ => {
            anyhow::bail!(
                "Unknown service: {}. Supported services: docker, tailscale, portainer, npm, cli",
                service
            );
        }
    }

    Ok(())
}

/// Install CLI development dependencies
fn install_cli_dependencies() -> Result<()> {
    println!("Installing CLI development dependencies...");

    // Install cargo-watch if not already installed
    use std::process::Command;

    let watch_installed = Command::new("cargo")
        .args(["watch", "--version"])
        .output()
        .is_ok();

    if !watch_installed {
        println!("Installing cargo-watch...");
        let status = Command::new("cargo")
            .args(["install", "cargo-watch"])
            .status()
            .context("Failed to install cargo-watch")?;

        if !status.success() {
            anyhow::bail!("Failed to install cargo-watch");
        }
        println!("✓ cargo-watch installed");
    } else {
        println!("✓ cargo-watch already installed");
    }

    // Fetch Rust dependencies
    println!("Fetching Rust dependencies...");
    let status = Command::new("cargo")
        .args(["fetch"])
        .status()
        .context("Failed to fetch Rust dependencies")?;

    if !status.success() {
        anyhow::bail!("Failed to fetch Rust dependencies");
    }

    println!("✓ CLI dependencies installed");
    Ok(())
}
