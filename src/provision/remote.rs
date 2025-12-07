use crate::config::EnvConfig;
use crate::exec::SshConnection;
use crate::provision::{PortainerEdition, utils};
use anyhow::{Context, Result};
use serde_json::json;

pub fn provision_remote(
    hostname: &str,
    portainer_host: bool,
    portainer_edition: PortainerEdition,
    config: &EnvConfig,
) -> Result<()> {
    let host_config = config.hosts.get(hostname).with_context(|| {
        format!(
            "Host '{}' not found in .env\n\nAdd configuration to .env:\n  HOST_{}_IP=\"<ip-address>\"\n  HOST_{}_TAILSCALE=\"<tailscale-hostname>\"",
            hostname,
            hostname.to_uppercase(),
            hostname.to_uppercase()
        )
    })?;

    // Determine which host to connect to (prefer IP, fallback to Tailscale)
    let target_host = if let Some(ip) = &host_config.ip {
        ip.clone()
    } else if let Some(tailscale) = &host_config.tailscale {
        tailscale.clone()
    } else {
        anyhow::bail!("No IP or Tailscale hostname configured for {}", hostname);
    };

    println!("Provisioning {} ({})...", hostname, target_host);
    println!();

    // Get SSH connection info
    let default_user = crate::config::get_default_username();
    let host_with_user = format!("{}@{}", default_user, target_host);
    let ssh_conn = SshConnection::new(&host_with_user)?;

    // Copy the appropriate docker-compose file
    if portainer_host {
        utils::copy_portainer_compose(&target_host, portainer_edition.compose_file())?;
    } else {
        utils::copy_portainer_compose(&target_host, "portainer-agent.docker-compose.yml")?;
    }

    // Execute provisioning steps
    check_sudo_access(&ssh_conn)?;
    check_and_install_docker(&ssh_conn)?;
    check_and_install_tailscale(&ssh_conn)?;
    configure_docker_permissions(&ssh_conn)?;
    configure_docker_ipv6(&ssh_conn)?;

    if portainer_host {
        install_portainer(&ssh_conn, portainer_edition)?;
    } else {
        install_portainer_agent(&ssh_conn)?;
    }

    println!();
    println!("✓ Provisioning complete for {}", hostname);

    Ok(())
}

fn check_sudo_access(ssh: &SshConnection) -> Result<()> {
    println!("=== Checking sudo access ===");

    if !ssh.is_linux()? {
        println!("✓ macOS detected (Docker Desktop handles permissions)");
        return Ok(());
    }

    let output = ssh.execute_simple("sudo", &["-n", "true"])?;

    if !output.status.success() {
        println!("Error: Passwordless sudo is required for automated provisioning.");
        println!();
        println!("To configure passwordless sudo, run on the target host:");
        println!("  sudo visudo");
        println!();
        println!("Then add this line (replace USERNAME with your username):");
        println!("  USERNAME ALL=(ALL) NOPASSWD: ALL");
        println!();
        println!("Or for more security, limit to specific commands:");
        println!(
            "  USERNAME ALL=(ALL) NOPASSWD: /usr/bin/docker, /bin/systemctl, /usr/sbin/usermod, /bin/mkdir, /bin/tee, /bin/cp, /bin/mv, /bin/rm, /usr/bin/python3"
        );
        println!();
        anyhow::bail!("Passwordless sudo not configured");
    }

    println!("✓ Passwordless sudo configured");
    Ok(())
}

fn check_and_install_docker(ssh: &SshConnection) -> Result<()> {
    println!("=== Checking Docker installation ===");

    if ssh.check_command_exists("docker")? {
        println!("✓ Docker already installed");
        return Ok(());
    }

    println!("Docker not found. Installing Docker...");

    // Detect OS type
    let os_release_output = ssh.read_file("/etc/os-release")?;
    let is_debian = os_release_output
        .lines()
        .any(|line| line.starts_with("ID=debian") || line.starts_with("ID=\"debian\""));

    if ssh.check_command_exists("apt-get")? {
        if is_debian {
            println!("Detected Debian, using Debian Docker repository");
            ssh.execute_interactive("sudo", &["rm", "-f", "/etc/apt/sources.list.d/docker.list"])?;
            ssh.execute_interactive("sudo", &["apt-get", "update"])?;
            ssh.execute_interactive(
                "sudo",
                &[
                    "apt-get",
                    "install",
                    "-y",
                    "ca-certificates",
                    "curl",
                    "gnupg",
                ],
            )?;
            ssh.execute_interactive(
                "sudo",
                &["install", "-m", "0755", "-d", "/etc/apt/keyrings"],
            )?;

            // Download and install GPG key
            utils::download_and_install_gpg_key(
                ssh,
                "https://download.docker.com/linux/debian/gpg",
                "/etc/apt/keyrings/docker.gpg",
            )?;

            // Get codename
            let codename = if let Ok(output) =
                ssh.execute_simple("grep", &["VERSION_CODENAME", "/etc/os-release"])
            {
                let stdout = String::from_utf8_lossy(&output.stdout);
                stdout
                    .split('=')
                    .nth(1)
                    .unwrap_or("bookworm")
                    .trim_matches('"')
                    .to_string()
            } else {
                "bookworm".to_string()
            };

            // Get architecture
            let arch_output = ssh.execute_simple("dpkg", &["--print-architecture"])?;
            let arch = String::from_utf8_lossy(&arch_output.stdout)
                .trim()
                .to_string();

            let repo_line = format!(
                "deb [arch={} signed-by=/etc/apt/keyrings/docker.gpg] https://download.docker.com/linux/debian {} stable",
                arch, codename
            );
            ssh.write_file("/tmp/docker.list", repo_line.as_bytes())?;
            ssh.execute_interactive(
                "sudo",
                &[
                    "mv",
                    "/tmp/docker.list",
                    "/etc/apt/sources.list.d/docker.list",
                ],
            )?;
        } else {
            println!("Detected Ubuntu, using Ubuntu Docker repository");
            ssh.execute_interactive("sudo", &["rm", "-f", "/etc/apt/sources.list.d/docker.list"])?;
            ssh.execute_interactive("sudo", &["apt-get", "update"])?;
            ssh.execute_interactive(
                "sudo",
                &[
                    "apt-get",
                    "install",
                    "-y",
                    "ca-certificates",
                    "curl",
                    "gnupg",
                    "lsb-release",
                ],
            )?;
            ssh.execute_interactive(
                "sudo",
                &["install", "-m", "0755", "-d", "/etc/apt/keyrings"],
            )?;

            // Download and install GPG key
            utils::download_and_install_gpg_key(
                ssh,
                "https://download.docker.com/linux/ubuntu/gpg",
                "/etc/apt/keyrings/docker.gpg",
            )?;

            // Get codename
            let codename_output = ssh.execute_simple("lsb_release", &["-cs"])?;
            let codename = String::from_utf8_lossy(&codename_output.stdout)
                .trim()
                .to_string();

            // Get architecture
            let arch_output = ssh.execute_simple("dpkg", &["--print-architecture"])?;
            let arch = String::from_utf8_lossy(&arch_output.stdout)
                .trim()
                .to_string();

            let repo_line = format!(
                "deb [arch={} signed-by=/etc/apt/keyrings/docker.gpg] https://download.docker.com/linux/ubuntu {} stable",
                arch, codename
            );
            ssh.write_file("/tmp/docker.list", repo_line.as_bytes())?;
            ssh.execute_interactive(
                "sudo",
                &[
                    "mv",
                    "/tmp/docker.list",
                    "/etc/apt/sources.list.d/docker.list",
                ],
            )?;
        }

        ssh.execute_interactive("sudo", &["apt-get", "update"])?;
        ssh.execute_interactive(
            "sudo",
            &[
                "apt-get",
                "install",
                "-y",
                "docker-ce",
                "docker-ce-cli",
                "containerd.io",
                "docker-buildx-plugin",
                "docker-compose-plugin",
            ],
        )?;
    } else if ssh.check_command_exists("yum")? {
        println!("Detected RHEL/CentOS");
        ssh.execute_interactive("sudo", &["yum", "install", "-y", "yum-utils"])?;
        ssh.execute_interactive(
            "sudo",
            &[
                "yum-config-manager",
                "--add-repo",
                "https://download.docker.com/linux/centos/docker-ce.repo",
            ],
        )?;
        ssh.execute_interactive(
            "sudo",
            &[
                "yum",
                "install",
                "-y",
                "docker-ce",
                "docker-ce-cli",
                "containerd.io",
                "docker-buildx-plugin",
                "docker-compose-plugin",
            ],
        )?;
        ssh.execute_interactive("sudo", &["systemctl", "start", "docker"])?;
        ssh.execute_interactive("sudo", &["systemctl", "enable", "docker"])?;
    } else if ssh.check_command_exists("dnf")? {
        println!("Detected Fedora");
        ssh.execute_interactive("sudo", &["dnf", "install", "-y", "dnf-plugins-core"])?;
        ssh.execute_interactive(
            "sudo",
            &[
                "dnf",
                "config-manager",
                "--add-repo",
                "https://download.docker.com/linux/fedora/docker-ce.repo",
            ],
        )?;
        ssh.execute_interactive(
            "sudo",
            &[
                "dnf",
                "install",
                "-y",
                "docker-ce",
                "docker-ce-cli",
                "containerd.io",
                "docker-buildx-plugin",
                "docker-compose-plugin",
            ],
        )?;
        ssh.execute_interactive("sudo", &["systemctl", "start", "docker"])?;
        ssh.execute_interactive("sudo", &["systemctl", "enable", "docker"])?;
    } else if ssh.check_command_exists("brew")? {
        println!("Detected macOS");
        ssh.execute_interactive("brew", &["install", "--cask", "docker"])?;
        println!("Please start Docker Desktop manually");
    } else {
        anyhow::bail!("Unsupported package manager. Please install Docker manually.");
    }

    println!("✓ Docker installed");
    Ok(())
}

fn check_and_install_tailscale(ssh: &SshConnection) -> Result<()> {
    println!();
    println!("=== Checking Tailscale installation ===");

    if ssh.check_command_exists("tailscale")? {
        println!("✓ Tailscale already installed");
        return Ok(());
    }

    println!("Tailscale not found. Installing Tailscale...");

    if ssh.check_command_exists("apt-get")?
        || ssh.check_command_exists("yum")?
        || ssh.check_command_exists("dnf")?
    {
        // Download Tailscale install script and execute it
        utils::download_and_execute_script(
            ssh,
            "https://tailscale.com/install.sh",
            "/tmp/tailscale-install.sh",
        )?;
    } else if ssh.check_command_exists("brew")? {
        ssh.execute_interactive("brew", &["install", "tailscale"])?;
    } else {
        anyhow::bail!("Unsupported package manager. Please install Tailscale manually.");
    }

    println!("✓ Tailscale installed");
    println!("Note: Run 'sudo tailscale up' to connect to your tailnet");
    Ok(())
}

fn configure_docker_permissions(ssh: &SshConnection) -> Result<()> {
    println!();
    println!("=== Configuring Docker permissions ===");

    if !ssh.is_linux()? {
        return Ok(());
    }

    // Check if user is in docker group
    let groups_output = ssh.execute_simple("groups", &[])?;
    let groups = String::from_utf8_lossy(&groups_output.stdout);
    let in_group = groups.contains("docker");

    if !in_group {
        println!("Adding user to docker group...");
        ssh.execute_interactive("sudo", &["usermod", "-aG", "docker", "$USER"])?;
        println!("✓ User added to docker group");
        println!("Note: You may need to log out and back in for changes to take effect");
    } else {
        println!("✓ User already in docker group");
    }

    Ok(())
}

fn configure_docker_ipv6(ssh: &SshConnection) -> Result<()> {
    println!();
    println!("=== Configuring Docker IPv6 support ===");

    if !ssh.is_linux()? {
        println!("Skipping IPv6 configuration (macOS - Docker Desktop handles IPv6 differently)");
        return Ok(());
    }

    let ipv6_subnet = "fd00:172:20::/64";

    // Check if IPv6 is already enabled
    let ipv6_enabled = if ssh.file_exists("/etc/docker/daemon.json")? {
        let content = ssh.read_file("/etc/docker/daemon.json")?;
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
    ssh.execute_interactive("sudo", &["mkdir", "-p", "/etc/docker"])?;

    // Check if daemon.json exists
    let exists = ssh.file_exists("/etc/docker/daemon.json")?;

    if !exists {
        // Create new daemon.json
        println!("Creating new Docker daemon configuration...");
        let config = json!({
            "ipv6": true,
            "fixed-cidr-v6": ipv6_subnet
        });
        let config_str = serde_json::to_string_pretty(&config)?;
        ssh.write_file("/tmp/daemon.json", config_str.as_bytes())?;
        ssh.execute_interactive(
            "sudo",
            &["mv", "/tmp/daemon.json", "/etc/docker/daemon.json"],
        )?;
    } else {
        // Update existing daemon.json
        println!("Updating existing Docker daemon configuration...");

        // Try Python3 first
        if ssh.check_command_exists("python3")? {
            let python_script = format!(
                r#"import json; f=open('/etc/docker/daemon.json','r'); c=json.load(f); f.close(); c['ipv6']=True; c['fixed-cidr-v6']='{}'; f=open('/etc/docker/daemon.json','w'); json.dump(c,f,indent=2); f.close()"#,
                ipv6_subnet
            );
            ssh.execute_interactive("sudo", &["python3", "-c", &python_script])?;
        } else if ssh.check_command_exists("jq")? {
            // Use Rust-native JSON manipulation instead of jq
            utils::update_daemon_json_rust(ssh, ipv6_subnet)?;
        } else {
            // Fallback: backup and create new
            println!(
                "Warning: python3/jq not found, backing up existing config and creating new one"
            );
            ssh.execute_interactive(
                "sudo",
                &[
                    "cp",
                    "/etc/docker/daemon.json",
                    "/etc/docker/daemon.json.backup",
                ],
            )?;
            let config = json!({
                "ipv6": true,
                "fixed-cidr-v6": ipv6_subnet
            });
            let config_str = serde_json::to_string_pretty(&config)?;
            ssh.write_file("/tmp/daemon.json", config_str.as_bytes())?;
            ssh.execute_interactive(
                "sudo",
                &["mv", "/tmp/daemon.json", "/etc/docker/daemon.json"],
            )?;
            println!("Original config backed up to /etc/docker/daemon.json.backup");
        }
    }

    println!("✓ IPv6 configured in Docker daemon");
    println!("Restarting Docker daemon to apply changes...");

    if ssh.check_command_exists("systemctl")? {
        ssh.execute_interactive("sudo", &["systemctl", "restart", "docker"])?;
    } else if ssh.check_command_exists("service")? {
        ssh.execute_interactive("sudo", &["service", "docker", "restart"])?;
    } else {
        println!(
            "Warning: Could not restart Docker daemon. Please restart manually: sudo systemctl restart docker"
        );
    }

    // Wait a moment
    std::thread::sleep(std::time::Duration::from_secs(2));

    // Verify IPv6 is enabled
    let verify_output = ssh.execute_simple("docker", &["info"])?;
    let docker_info = String::from_utf8_lossy(&verify_output.stdout);
    if docker_info.to_lowercase().contains("ipv6") && docker_info.to_lowercase().contains("true") {
        println!("✓ IPv6 verified in Docker");
    } else {
        println!("Warning: IPv6 may not be enabled. Check with: docker info | grep -i ipv6");
    }

    Ok(())
}

fn install_portainer(ssh: &SshConnection, edition: PortainerEdition) -> Result<()> {
    println!();
    println!("=== Installing Portainer {} ===", edition.display_name());

    // Remove existing containers
    println!("Removing any existing Portainer instances...");

    // Check and stop/remove portainer
    if let Ok(output) = ssh.execute_simple("docker", &["ps", "-a", "--format", "{{.Names}}"]) {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains("portainer") {
            ssh.execute_simple("docker", &["stop", "portainer"]).ok();
            ssh.execute_simple("docker", &["rm", "portainer"]).ok();
        }
        if stdout.contains("portainer_agent") {
            ssh.execute_simple("docker", &["stop", "portainer_agent"])
                .ok();
            ssh.execute_simple("docker", &["rm", "portainer_agent"])
                .ok();
        }
    }

    println!("✓ Removed existing Portainer containers");

    // Start Portainer
    ssh.mkdir_p("$HOME/portainer")?;

    // Try docker compose, fallback to docker-compose
    let compose_cmd = if ssh.check_command_exists("docker")? {
        "docker compose"
    } else {
        "docker-compose"
    };

    ssh.execute_shell_interactive(&format!(
        "cd $HOME/portainer && {} down 2>/dev/null || true && {} up -d",
        compose_cmd, compose_cmd
    ))?;

    println!(
        "✓ Portainer {} installed and running",
        edition.display_name()
    );
    println!("Access Portainer at https://localhost:9443");
    Ok(())
}

fn install_portainer_agent(ssh: &SshConnection) -> Result<()> {
    println!();
    println!("=== Installing Portainer Agent ===");

    // Remove existing containers
    println!("Removing any existing Portainer instances...");

    // Check and stop/remove portainer containers
    if let Ok(output) = ssh.execute_simple("docker", &["ps", "-a", "--format", "{{.Names}}"]) {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains("portainer") {
            ssh.execute_simple("docker", &["stop", "portainer"]).ok();
            ssh.execute_simple("docker", &["rm", "portainer"]).ok();
        }
        if stdout.contains("portainer_agent") {
            ssh.execute_simple("docker", &["stop", "portainer_agent"])
                .ok();
            ssh.execute_simple("docker", &["rm", "portainer_agent"])
                .ok();
        }
    }

    println!("✓ Removed existing Portainer containers");

    // Start Portainer Agent
    ssh.mkdir_p("$HOME/portainer")?;

    // Try docker compose, fallback to docker-compose
    let compose_cmd = if ssh.check_command_exists("docker")? {
        "docker compose"
    } else {
        "docker-compose"
    };

    ssh.execute_shell_interactive(&format!(
        "cd $HOME/portainer && {} down 2>/dev/null || true && {} up -d",
        compose_cmd, compose_cmd
    ))?;

    println!("✓ Portainer Agent installed and running");
    println!("Add this agent to your Portainer instance using the agent endpoint");
    Ok(())
}
