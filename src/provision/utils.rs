use crate::config::EnvConfig;
use crate::exec::{SshConnection, local};
use anyhow::{Context, Result};

/// Detect if we're running locally on the target host or remotely
pub fn is_local_execution(hostname: &str, config: &EnvConfig) -> Result<bool> {
    let host_config = config
        .hosts
        .get(hostname)
        .with_context(|| format!("Host '{}' not found in config", hostname))?;

    // Get target IP
    let target_ip = if let Some(ip) = &host_config.ip {
        ip.clone()
    } else {
        // If no IP configured, assume remote
        return Ok(false);
    };

    // Get local IP addresses
    let local_ips = get_local_ips()?;

    // Check if target IP matches any local IP
    Ok(local_ips.contains(&target_ip))
}

/// Get all local IP addresses
pub fn get_local_ips() -> Result<Vec<String>> {
    let mut ips = Vec::new();

    // Try to get IPs using platform-specific commands
    #[cfg(unix)]
    {
        // Use `hostname -I` on Linux or `ifconfig` on macOS
        if let Ok(output) = local::execute("hostname", &["-I"]) {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for ip in stdout.split_whitespace() {
                ips.push(ip.to_string());
            }
        }

        // Also try `ip addr` on Linux
        if let Ok(output) = local::execute("ip", &["addr", "show"]) {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if line.contains("inet ") && !line.contains("127.0.0.1") && !line.contains("::1") {
                    if let Some(ip_part) = line.split_whitespace().nth(1) {
                        if let Some(ip) = ip_part.split('/').next() {
                            ips.push(ip.to_string());
                        }
                    }
                }
            }
        }
    }

    #[cfg(windows)]
    {
        // Use `ipconfig` on Windows
        if let Ok(output) = local::execute("ipconfig", &[]) {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if line.contains("IPv4 Address") || line.contains("IPv4 地址") {
                    if let Some(ip_part) = line.split(':').nth(1) {
                        let ip = ip_part.trim();
                        if !ip.is_empty() {
                            ips.push(ip.to_string());
                        }
                    }
                }
            }
        }
    }

    Ok(ips)
}

/// Helper function to download and install GPG key
pub fn download_and_install_gpg_key(
    ssh: &SshConnection,
    url: &str,
    output_path: &str,
) -> Result<()> {
    // Download GPG key using reqwest (blocking)
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    let response = client.get(url).send()?;
    let gpg_data = response.bytes()?;

    // Write to temporary file on remote
    ssh.write_file("/tmp/docker.gpg.raw", &gpg_data)?;

    // Run gpg --dearmor on remote
    ssh.execute_interactive(
        "sudo",
        &["gpg", "--dearmor", "-o", output_path, "/tmp/docker.gpg.raw"],
    )?;

    // Set permissions
    ssh.execute_interactive("sudo", &["chmod", "a+r", output_path])?;

    // Clean up temp file
    ssh.execute_simple("rm", &["-f", "/tmp/docker.gpg.raw"])
        .ok();

    Ok(())
}

/// Helper function to download and execute a script
pub fn download_and_execute_script(
    ssh: &SshConnection,
    url: &str,
    script_path: &str,
) -> Result<()> {
    // Download script using reqwest (blocking)
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()?;
    let response = client.get(url).send()?;
    let script_content = response.text()?;

    // Write script to remote
    ssh.write_file(script_path, script_content.as_bytes())?;

    // Make executable
    ssh.execute_interactive("chmod", &["+x", script_path])?;

    // Execute script
    ssh.execute_shell_interactive(&format!("sh {}", script_path))?;

    // Clean up
    ssh.execute_simple("rm", &["-f", script_path]).ok();

    Ok(())
}

/// Helper function to update daemon.json using Rust-native JSON manipulation
pub fn update_daemon_json_rust(ssh: &SshConnection, ipv6_subnet: &str) -> Result<()> {
    use serde_json::{Value, json};

    // Read existing config
    let content = ssh.read_file("/etc/docker/daemon.json")?;
    let mut config: Value = serde_json::from_str(&content)
        .with_context(|| "Failed to parse /etc/docker/daemon.json")?;

    // Update config
    config["ipv6"] = json!(true);
    config["fixed-cidr-v6"] = json!(ipv6_subnet);

    // Write updated config
    let updated_content = serde_json::to_string_pretty(&config)?;
    ssh.write_file("/tmp/daemon.json", updated_content.as_bytes())?;
    ssh.execute_interactive(
        "sudo",
        &["mv", "/tmp/daemon.json", "/etc/docker/daemon.json"],
    )?;

    Ok(())
}

pub fn copy_portainer_compose(host: &str, compose_filename: &str) -> Result<()> {
    // Find the homelab directory to locate the compose file
    let homelab_dir = crate::config::find_homelab_dir()?;
    let compose_file = homelab_dir.join("compose").join(compose_filename);

    if !compose_file.exists() {
        anyhow::bail!(
            "Portainer docker-compose file not found at {}",
            compose_file.display()
        );
    }

    // Read the compose file
    let compose_content = std::fs::read_to_string(&compose_file)
        .with_context(|| format!("Failed to read compose file: {}", compose_file.display()))?;

    // Determine username for SSH
    let default_user = crate::config::get_default_username();
    let host_with_user = format!("{}@{}", default_user, host);
    let ssh_conn = SshConnection::new(&host_with_user)?;

    // Create directory first
    ssh_conn.mkdir_p("$HOME/portainer")?;

    // Write the file
    ssh_conn.write_file(
        "$HOME/portainer/docker-compose.yml",
        compose_content.as_bytes(),
    )?;

    println!("✓ Copied {} to remote system", compose_filename);
    Ok(())
}
