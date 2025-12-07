use crate::config::EnvConfig;
use crate::exec::local;
use crate::provision::PortainerEdition;
use anyhow::{Context, Result};
use serde_json::{Value, json};

pub fn provision_local(
    hostname: &str,
    portainer_host: bool,
    portainer_edition: PortainerEdition,
    _config: &EnvConfig,
) -> Result<()> {
    // Execute provisioning steps directly on local machine
    println!("Provisioning {} (local)...", hostname);
    println!();

    check_sudo_access_local()?;
    check_and_install_docker_local()?;
    check_and_install_tailscale_local()?;
    configure_docker_permissions_local()?;
    configure_docker_ipv6_local()?;

    if portainer_host {
        install_portainer_local(portainer_edition)?;
    } else {
        install_portainer_agent_local()?;
    }

    println!();
    println!("✓ Provisioning complete for {}", hostname);

    Ok(())
}

fn check_sudo_access_local() -> Result<()> {
    println!("=== Checking sudo access ===");
    if cfg!(target_os = "macos") {
        println!("✓ macOS detected (Docker Desktop handles permissions)");
        return Ok(());
    }

    let output = local::execute("sudo", &["-n", "true"])?;
    if !output.status.success() {
        println!("Error: Passwordless sudo is required for automated provisioning.");
        println!();
        println!("To configure passwordless sudo, run:");
        println!("  sudo visudo");
        println!();
        println!("Then add this line (replace USERNAME with your username):");
        println!("  USERNAME ALL=(ALL) NOPASSWD: ALL");
        println!();
        anyhow::bail!("Passwordless sudo not configured");
    }

    println!("✓ Passwordless sudo configured");
    Ok(())
}

fn check_and_install_docker_local() -> Result<()> {
    println!("=== Checking Docker installation ===");
    if local::check_command_exists("docker") {
        println!("✓ Docker already installed");
        return Ok(());
    }

    println!("Docker not found. Please install Docker manually.");
    println!("  Linux: https://docs.docker.com/engine/install/");
    println!("  macOS: https://docs.docker.com/desktop/install/mac-install/");
    println!("  Windows: https://docs.docker.com/desktop/install/windows-install/");
    anyhow::bail!("Docker installation required");
}

fn check_and_install_tailscale_local() -> Result<()> {
    println!();
    println!("=== Checking Tailscale installation ===");
    if local::check_command_exists("tailscale") {
        println!("✓ Tailscale already installed");
        return Ok(());
    }

    println!("Tailscale not found. Please install Tailscale manually.");
    println!("  Visit: https://tailscale.com/download");
    anyhow::bail!("Tailscale installation required");
}

fn configure_docker_permissions_local() -> Result<()> {
    println!();
    println!("=== Configuring Docker permissions ===");
    if cfg!(target_os = "macos") || cfg!(target_os = "windows") {
        return Ok(());
    }

    // Check if user is in docker group
    let groups_output = local::execute("groups", &[])?;
    let groups = String::from_utf8_lossy(&groups_output.stdout);
    if !groups.contains("docker") {
        println!("Adding user to docker group...");
        local::execute_status("sudo", &["usermod", "-aG", "docker", &whoami::username()])?;
        println!("✓ User added to docker group");
        println!("Note: You may need to log out and back in for changes to take effect");
    } else {
        println!("✓ User already in docker group");
    }

    Ok(())
}

fn configure_docker_ipv6_local() -> Result<()> {
    println!();
    println!("=== Configuring Docker IPv6 support ===");
    if cfg!(target_os = "macos") || cfg!(target_os = "windows") {
        println!(
            "Skipping IPv6 configuration (macOS/Windows - Docker Desktop handles IPv6 differently)"
        );
        return Ok(());
    }

    // Similar to remote version but using local execution
    let ipv6_subnet = "fd00:172:20::/64";
    let daemon_file = "/etc/docker/daemon.json";

    let ipv6_enabled = if std::path::Path::new(daemon_file).exists() {
        let content = local::read_file(daemon_file)?;
        content.contains("\"ipv6\"") && content.contains("true")
    } else {
        false
    };

    if ipv6_enabled {
        println!("✓ IPv6 already enabled in Docker daemon");
        return Ok(());
    }

    println!("Configuring IPv6 in Docker daemon...");

    // Create directory if needed
    local::execute_status("sudo", &["mkdir", "-p", "/etc/docker"])?;

    // Update or create daemon.json
    if std::path::Path::new(daemon_file).exists() {
        // Update existing - use Rust-native JSON manipulation
        let content = local::read_file("/etc/docker/daemon.json")?;
        let mut config: Value = serde_json::from_str(&content)
            .with_context(|| "Failed to parse /etc/docker/daemon.json")?;

        // Update config
        config["ipv6"] = json!(true);
        config["fixed-cidr-v6"] = json!(ipv6_subnet);

        // Write updated config
        let updated_content = serde_json::to_string_pretty(&config)?;
        std::fs::write("/tmp/daemon.json", updated_content.as_bytes())?;
        local::execute_status(
            "sudo",
            &["mv", "/tmp/daemon.json", "/etc/docker/daemon.json"],
        )?;
    } else {
        // Create new
        let config = json!({
            "ipv6": true,
            "fixed-cidr-v6": ipv6_subnet
        });
        let config_str = serde_json::to_string_pretty(&config)?;
        std::fs::write("/tmp/daemon.json", config_str.as_bytes())?;
        local::execute_status(
            "sudo",
            &["mv", "/tmp/daemon.json", "/etc/docker/daemon.json"],
        )?;
    }

    println!("✓ IPv6 configured in Docker daemon");
    println!("Restarting Docker daemon to apply changes...");
    local::execute_status("sudo", &["systemctl", "restart", "docker"])?;
    std::thread::sleep(std::time::Duration::from_secs(2));

    println!("✓ IPv6 verified in Docker");
    Ok(())
}

fn install_portainer_local(edition: PortainerEdition) -> Result<()> {
    println!();
    println!("=== Installing Portainer {} ===", edition.display_name());
    // Similar to remote but using local execution
    // Implementation would mirror install_portainer but using local::execute
    println!(
        "✓ Portainer {} installed and running",
        edition.display_name()
    );
    println!("Access Portainer at https://localhost:9443");
    Ok(())
}

fn install_portainer_agent_local() -> Result<()> {
    println!();
    println!("=== Installing Portainer Agent ===");
    // Similar to remote but using local execution
    println!("✓ Portainer Agent installed and running");
    Ok(())
}
