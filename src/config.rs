use anyhow::{Context, Result};
use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};

pub struct HostConfig {
    pub ip: Option<String>,
    pub tailscale: Option<String>,
}

pub struct EnvConfig {
    pub tailnet_base: String,
    pub hosts: HashMap<String, HostConfig>,
}

pub fn find_homelab_dir() -> Result<PathBuf> {
    // Check for environment variable override
    if let Ok(dir) = env::var("HOMELAB_DIR") {
        return Ok(PathBuf::from(dir));
    }

    // Try to find .env file in current directory or parent directories
    let mut current = env::current_dir()?;
    loop {
        let env_file = current.join(".env");
        if env_file.exists() {
            return Ok(current);
        }
        if !current.pop() {
            break;
        }
    }

    // Fallback: use current directory
    Ok(env::current_dir()?)
}

pub fn load_env_config(homelab_dir: &Path) -> Result<EnvConfig> {
    let env_file = homelab_dir.join(".env");

    if !env_file.exists() {
        anyhow::bail!(
            "Error: .env file not found at {}\nCopy .env.example to .env and configure your settings.",
            env_file.display()
        );
    }

    // Load .env file
    dotenv::from_path(&env_file)
        .with_context(|| format!("Failed to load .env file from {}", env_file.display()))?;

    let tailnet_base = env::var("TAILNET_BASE").unwrap_or_else(|_| "ts.net".to_string());

    // Parse host configurations
    let mut hosts = HashMap::new();
    let env_vars: Vec<(String, String)> = env::vars().collect();

    for (key, value) in env_vars {
        if let Some(hostname) = key.strip_prefix("HOST_") {
            if let Some(rest) = hostname.strip_suffix("_IP") {
                let hostname_lower = rest.to_lowercase();
                let config = hosts.entry(hostname_lower).or_insert_with(|| HostConfig {
                    ip: None,
                    tailscale: None,
                });
                config.ip = Some(value);
            } else if let Some(rest) = hostname.strip_suffix("_TAILSCALE") {
                let hostname_lower = rest.to_lowercase();
                let config = hosts.entry(hostname_lower).or_insert_with(|| HostConfig {
                    ip: None,
                    tailscale: None,
                });
                config.tailscale = Some(value);
            }
        }
    }

    Ok(EnvConfig {
        tailnet_base,
        hosts,
    })
}

pub fn list_available_hosts(config: &EnvConfig) {
    println!("Available hosts (from .env):");
    let mut hosts: Vec<&String> = config.hosts.keys().collect();
    hosts.sort();
    for host in hosts {
        println!("  - {}", host);
    }
}

pub fn get_default_username() -> String {
    env::var("USER")
        .or_else(|_| env::var("USERNAME"))
        .unwrap_or_else(|_| "root".to_string())
}

pub fn get_os() -> &'static str {
    env::consts::OS
}

pub fn get_arch() -> &'static str {
    env::consts::ARCH
}
