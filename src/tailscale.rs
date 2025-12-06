use crate::config;
use anyhow::Result;
use std::process::Command;

pub fn install_tailscale() -> Result<()> {
    let os = config::get_os();
    let arch = config::get_arch();

    println!("Installing Tailscale on {} ({})...", os, arch);

    match os {
        "macos" => install_tailscale_macos(),
        "linux" => install_tailscale_linux(),
        "windows" => install_tailscale_windows(),
        _ => {
            anyhow::bail!(
                "Unsupported operating system: {}\nPlease install Tailscale manually from: https://tailscale.com/download",
                os
            );
        }
    }
}

fn install_tailscale_macos() -> Result<()> {
    // Check for Homebrew
    if which::which("brew").is_ok() {
        println!("Detected macOS...");
        println!("Installing via Homebrew...");
        let status = Command::new("brew")
            .args(["install", "tailscale"])
            .status()?;

        if status.success() {
            println!("✓ Tailscale installed via Homebrew");
            println!();
            println!("To start Tailscale, run:");
            println!("  sudo tailscaled");
            println!("  tailscale up");
            Ok(())
        } else {
            anyhow::bail!("Failed to install Tailscale via Homebrew");
        }
    } else {
        anyhow::bail!(
            "Homebrew not found. Please install Homebrew first:\n  /bin/bash -c \"$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)\""
        );
    }
}

fn install_tailscale_linux() -> Result<()> {
    println!("Detected Linux...");

    // Try to detect package manager
    let install_script = if which::which("apt-get").is_ok() {
        println!("Installing via apt (Debian/Ubuntu)...");
        Some("curl -fsSL https://tailscale.com/install.sh | sh")
    } else if which::which("yum").is_ok() {
        println!("Installing via yum (RHEL/CentOS)...");
        Some("curl -fsSL https://tailscale.com/install.sh | sh")
    } else if which::which("dnf").is_ok() {
        println!("Installing via dnf (Fedora)...");
        Some("curl -fsSL https://tailscale.com/install.sh | sh")
    } else {
        None
    };

    if let Some(script) = install_script {
        // Download and execute the install script
        let status = Command::new("sh").arg("-c").arg(script).status()?;

        if status.success() {
            println!("✓ Tailscale installed");
            println!();
            println!("To start Tailscale, run:");
            println!("  sudo tailscale up");
            Ok(())
        } else {
            anyhow::bail!("Failed to install Tailscale");
        }
    } else {
        anyhow::bail!(
            "Unsupported Linux distribution. Please install Tailscale manually:\n  Visit: https://tailscale.com/download"
        );
    }
}

fn install_tailscale_windows() -> Result<()> {
    println!("Detected Windows...");
    println!("Please install Tailscale manually from: https://tailscale.com/download/windows");
    println!();
    println!("Or use winget:");
    println!("  winget install Tailscale.Tailscale");
    Ok(())
}
