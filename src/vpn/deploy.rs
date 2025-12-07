use crate::exec::SshConnection;
use anyhow::{Context, Result};
use std::env;

pub fn deploy_vpn(hostname: &str, config: &crate::config::EnvConfig) -> Result<()> {
    let homelab_dir = crate::config::find_homelab_dir()?;

    // Load PIA credentials from local .env
    dotenv::from_path(homelab_dir.join(".env")).context("Failed to load .env file")?;

    let pia_username = env::var("PIA_USERNAME").context("PIA_USERNAME not found in .env file")?;
    let pia_password = env::var("PIA_PASSWORD").context("PIA_PASSWORD not found in .env file")?;

    // Get host configuration
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

    println!("Deploying VPN to {} ({})...", hostname, target_host);
    println!();

    // Read compose file - use local build version for now (avoids registry auth issues)
    // User can switch to portainer version after making image public
    let compose_file = homelab_dir
        .join("compose")
        .join("openvpn-pia.docker-compose.yml");
    if !compose_file.exists() {
        anyhow::bail!("VPN compose file not found at {}", compose_file.display());
    }

    let compose_content = std::fs::read_to_string(&compose_file)
        .with_context(|| format!("Failed to read compose file: {}", compose_file.display()))?;

    // Don't substitute - let docker-compose read from .env file using --env-file

    // Determine username for SSH and VPN config path
    let default_user = crate::config::get_default_username();
    // Allow VPN_USER to override the username for config path (useful for Portainer)
    // If not set, uses the SSH user (default_user)
    let vpn_user = env::var("VPN_USER").unwrap_or_else(|_| default_user.clone());
    let host_with_user = format!("{}@{}", default_user, target_host);
    let ssh_conn = SshConnection::new(&host_with_user)?;

    // Check if files already exist - if so, skip deployment
    // Check for both .ovpn and .opvn (typo) variants
    // Use /home/$USER/config/vpn (USER can be set via VPN_USER env var)
    let vpn_config_dir = format!("/home/{}/config/vpn", vpn_user);
    let auth_exists = ssh_conn.file_exists(&format!("{}/auth.txt", vpn_config_dir))?;
    let config_exists = ssh_conn.file_exists(&format!("{}/ca-montreal.ovpn", vpn_config_dir))?
        || ssh_conn.file_exists(&format!("{}/ca-montreal.opvn", vpn_config_dir))?;
    let files_exist = auth_exists && config_exists;

    if files_exist {
        println!("✓ VPN configuration files already exist on remote system");
        println!("  Skipping file copy (files are already in place)");
    } else {
        println!("VPN configuration files not found, attempting to copy...");

        // Copy OpenVPN config files to remote system
        let openvpn_dir = homelab_dir.join("openvpn");
        let auth_file = openvpn_dir.join("auth.txt");
        let config_file = openvpn_dir.join("ca-montreal.ovpn");

        if !auth_file.exists() {
            anyhow::bail!("OpenVPN auth file not found at {}", auth_file.display());
        }
        if !config_file.exists() {
            anyhow::bail!("OpenVPN config file not found at {}", config_file.display());
        }

        // Copy files using scp, then move to $HOME/config/vpn
        // Read auth file and write directly
        let auth_content = std::fs::read(&auth_file)
            .with_context(|| format!("Failed to read auth file: {}", auth_file.display()))?;

        // Create directory and write file
        ssh_conn.mkdir_p(&vpn_config_dir)?;
        ssh_conn.write_file(&format!("{}/auth.txt", vpn_config_dir), &auth_content)?;
        ssh_conn.execute_shell_interactive(&format!("chmod 600 {}/auth.txt", vpn_config_dir))?;
        println!("✓ Copied auth.txt to remote system");

        // Copy config file
        let config_content = std::fs::read(&config_file)
            .with_context(|| format!("Failed to read config file: {}", config_file.display()))?;

        ssh_conn.write_file(
            &format!("{}/ca-montreal.ovpn", vpn_config_dir),
            &config_content,
        )?;
        ssh_conn
            .execute_shell_interactive(&format!("chmod 644 {}/ca-montreal.ovpn", vpn_config_dir))?;
        println!("✓ Copied ca-montreal.ovpn to remote system");
    }

    // Copy compose file to remote system (keep in home directory for user access)
    ssh_conn.mkdir_p("$HOME/vpn")?;
    ssh_conn.write_file("$HOME/vpn/docker-compose.yml", compose_content.as_bytes())?;
    println!("✓ Copied VPN compose file to remote system");

    // Create .env file on remote system with PIA credentials
    let env_content = format!(
        "PIA_USERNAME={}\nPIA_PASSWORD={}\n",
        pia_username, pia_password
    );
    ssh_conn.write_file("$HOME/vpn/.env", env_content.as_bytes())?;
    println!("✓ Created .env file on remote system");

    println!();
    println!(
        "✓ VPN configuration files copied to {} ({})",
        hostname, target_host
    );
    println!("  Files copied:");
    println!("    - ~/vpn/docker-compose.yml (Portainer compose file)");
    println!("    - ~/vpn/.env (PIA credentials)");
    println!(
        "    - /home/{}/config/vpn/auth.txt (OpenVPN authentication)",
        vpn_user
    );
    println!(
        "    - /home/{}/config/vpn/ca-montreal.ovpn (OpenVPN configuration)",
        vpn_user
    );
    println!();
    println!("  Note: Set USER environment variable in Portainer to match the username");
    println!("        Example: USER={}", vpn_user);
    println!();
    println!("  You can now deploy the VPN manually using Portainer or docker-compose.");

    Ok(())
}
