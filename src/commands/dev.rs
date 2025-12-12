use anyhow::{Context, Result};
use clap::Subcommand;
use std::io::{self, Write};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::Command as StdCommand;
use std::process::{Command, Stdio};

#[derive(Subcommand)]
pub enum DevCommands {
    /// macOS development mode
    Mac,
    /// iOS development mode
    Ios,
    /// Web development mode
    Web {
        /// Run in bare metal mode (Rust server + Svelte dev, no Docker)
        #[arg(long)]
        bare_metal: bool,
        /// Run in production mode (Docker container)
        #[arg(long)]
        prod: bool,
        /// Port for the web server
        #[arg(long, default_value = "3000")]
        port: u16,
        /// Directory containing built Svelte app (for production mode)
        #[arg(long)]
        static_dir: Option<PathBuf>,
    },
    /// CLI development mode (with watch)
    Cli,
}

pub async fn handle_dev(command: DevCommands) -> Result<()> {
    match command {
        DevCommands::Mac => {
            dev_mac()?;
        }
        DevCommands::Ios => {
            dev_ios()?;
        }
        DevCommands::Web {
            bare_metal,
            prod,
            port,
            static_dir,
        } => {
            if prod {
                handle_web_prod().await?;
            } else if bare_metal {
                handle_web_bare_metal(port, static_dir).await?;
            } else {
                handle_web_docker(port).await?;
            }
        }
        DevCommands::Cli => {
            dev_cli().await?;
        }
    }

    Ok(())
}

fn dev_mac() -> Result<()> {
    use std::process::Command as StdCommand;

    println!("Starting macOS development mode...");

    let swift_dir = PathBuf::from("halvor-swift");
    let xcode_proj = swift_dir.join("HalvorApp.xcodeproj");

    // Create Xcode project if it doesn't exist
    if !xcode_proj.exists() {
        println!("Xcode project not found. Creating it...");
        let create_script = swift_dir.join("scripts/create-xcode-project.sh");
        if create_script.exists() {
            let status = StdCommand::new("bash")
                .arg(&create_script)
                .current_dir(&swift_dir)
                .status()
                .context("Failed to create Xcode project")?;

            if !status.success() {
                println!("⚠️  Failed to create Xcode project");
            }
        }
    }

    // Build the app (disable signing for dev builds)
    let status = StdCommand::new("xcodebuild")
        .args([
            "-project",
            "HalvorApp.xcodeproj",
            "-scheme",
            "HalvorApp-macOS",
            "-configuration",
            "Debug",
            "-derivedDataPath",
            "build",
            "CODE_SIGN_IDENTITY=",
            "CODE_SIGNING_REQUIRED=NO",
            "CODE_SIGNING_ALLOWED=NO",
        ])
        .current_dir(&swift_dir)
        .status()
        .context("Failed to build macOS app")?;

    if !status.success() {
        anyhow::bail!("macOS build failed");
    }

    // Open the app
    let app_path = swift_dir.join("build/Build/Products/Debug/HalvorApp-macOS.app");
    if app_path.exists() {
        StdCommand::new("open")
            .arg(&app_path)
            .status()
            .context("Failed to open macOS app")?;
    }

    Ok(())
}

fn dev_ios() -> Result<()> {
    use std::process::Command as StdCommand;

    println!("Starting iOS development mode...");

    let swift_dir = PathBuf::from("halvor-swift");
    let xcode_proj = swift_dir.join("HalvorApp.xcodeproj");

    // Create Xcode project if it doesn't exist
    if !xcode_proj.exists() {
        println!("Xcode project not found. Creating it...");
        let create_script = swift_dir.join("scripts/create-xcode-project.sh");
        if create_script.exists() {
            let status = StdCommand::new("bash")
                .arg(&create_script)
                .current_dir(&swift_dir)
                .status()
                .context("Failed to create Xcode project")?;

            if !status.success() {
                println!("⚠️  Failed to create Xcode project");
            }
        }
    }

    // List available devices and let user choose
    let devices = list_available_devices()?;

    if devices.is_empty() {
        anyhow::bail!(
            "No iOS devices or simulators found. Please create a simulator or connect a device."
        );
    }

    println!("\nAvailable iOS devices:");
    for (index, device) in devices.iter().enumerate() {
        let status = if device.booted { " (booted)" } else { "" };
        println!(
            "  {}. {} - {} ({}){}",
            index + 1,
            device.name,
            device.runtime,
            device.id,
            status
        );
    }

    print!("\nSelect device (1-{}): ", devices.len());
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let selection: usize = input
        .trim()
        .parse()
        .context("Invalid selection. Please enter a number.")?;

    if selection < 1 || selection > devices.len() {
        anyhow::bail!(
            "Invalid selection. Please choose a number between 1 and {}",
            devices.len()
        );
    }

    let selected_device = &devices[selection - 1];
    let sim_id = &selected_device.id;

    println!("Using device: {} ({})", selected_device.name, sim_id);

    // Build the app (disable signing for simulator builds)
    let status = StdCommand::new("xcodebuild")
        .args([
            "-project",
            "HalvorApp.xcodeproj",
            "-scheme",
            "HalvorApp-iOS",
            "-configuration",
            "Debug",
            "-sdk",
            "iphonesimulator",
            "-derivedDataPath",
            "build",
            "-destination",
            &format!("id={}", sim_id),
            "CODE_SIGN_IDENTITY=",
            "CODE_SIGNING_REQUIRED=NO",
            "CODE_SIGNING_ALLOWED=NO",
        ])
        .current_dir(&swift_dir)
        .status()
        .context("Failed to build iOS app")?;

    if !status.success() {
        anyhow::bail!("iOS build failed");
    }

    // Boot simulator
    StdCommand::new("xcrun")
        .args(["simctl", "boot", &sim_id])
        .status()
        .ok(); // Ignore errors (might already be booted)

    // Install app
    let app_path = swift_dir.join("build/Build/Products/Debug-iphonesimulator/HalvorApp-iOS.app");
    if app_path.exists() {
        let status = StdCommand::new("xcrun")
            .args(["simctl", "install", &sim_id, app_path.to_str().unwrap()])
            .status()
            .context("Failed to install iOS app")?;

        if !status.success() {
            anyhow::bail!("Failed to install iOS app");
        }

        // Launch app
        let status = StdCommand::new("xcrun")
            .args(["simctl", "launch", &sim_id, "dev.scottkey.halvor.ios"])
            .status()
            .context("Failed to launch iOS app")?;

        if !status.success() {
            anyhow::bail!("Failed to launch iOS app");
        }
    }

    Ok(())
}

#[derive(Debug)]
struct Device {
    id: String,
    name: String,
    runtime: String,
    booted: bool,
}

fn list_available_devices() -> Result<Vec<Device>> {
    use std::process::Command as StdCommand;

    // Get list of all devices (including physical devices)
    let output = StdCommand::new("xcrun")
        .args(["xcrun", "simctl", "list", "devices", "available", "--json"])
        .output()
        .context("Failed to list devices")?;

    if !output.status.success() {
        // Fallback to non-JSON output
        return list_available_devices_legacy();
    }

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).context("Failed to parse device list JSON")?;

    let mut devices = Vec::new();

    // Parse JSON structure: { "devices": { "iOS 18.0": [...], ... } }
    if let Some(devices_obj) = json.get("devices").and_then(|d| d.as_object()) {
        for (runtime, device_list) in devices_obj {
            if let Some(device_array) = device_list.as_array() {
                for device in device_array {
                    if let Some(id) = device.get("udid").and_then(|u| u.as_str()) {
                        if let Some(name) = device.get("name").and_then(|n| n.as_str()) {
                            let state = device
                                .get("state")
                                .and_then(|s| s.as_str())
                                .unwrap_or("Shutdown");
                            let booted = state == "Booted";

                            devices.push(Device {
                                id: id.to_string(),
                                name: name.to_string(),
                                runtime: runtime.clone(),
                                booted,
                            });
                        }
                    }
                }
            }
        }
    }

    // Also check for physical devices
    let physical_output = StdCommand::new("xcrun")
        .args(["xcrun", "devicectl", "list", "devices", "--json"])
        .output()
        .ok();

    // Sort devices: booted first, then by name
    devices.sort_by(|a, b| match (a.booted, b.booted) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.cmp(&b.name),
    });

    Ok(devices)
}

fn list_available_devices_legacy() -> Result<Vec<Device>> {
    use std::process::Command as StdCommand;

    // Fallback to parsing text output
    let output = StdCommand::new("xcrun")
        .args(["simctl", "list", "devices", "available"])
        .output()
        .context("Failed to list devices")?;

    let output_str = String::from_utf8_lossy(&output.stdout);
    let mut devices = Vec::new();
    let mut current_runtime = String::new();

    for line in output_str.lines() {
        // Check if this is a runtime header (e.g., "-- iOS 18.0 --")
        if line.contains("--") && line.contains("iOS") {
            if let Some(start) = line.find("iOS") {
                if let Some(end) = line[start..].find("--") {
                    current_runtime = line[start..start + end].trim().to_string();
                }
            }
        } else if line.contains("(") && line.contains(")") {
            // Parse device line: "    iPhone 17 (XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX) (Booted)"
            let trimmed = line.trim();
            if let Some(name_end) = trimmed.find('(') {
                let name = trimmed[..name_end].trim().to_string();

                if let Some(id_start) = trimmed[name_end..].find('(') {
                    let id_part = &trimmed[name_end + id_start + 1..];
                    if let Some(id_end) = id_part.find(')') {
                        let id = id_part[..id_end].trim().to_string();
                        let booted = trimmed.contains("Booted");

                        if id.len() == 36 && id.matches('-').count() == 4 {
                            devices.push(Device {
                                id,
                                name,
                                runtime: current_runtime.clone(),
                                booted,
                            });
                        }
                    }
                }
            }
        }
    }

    // Sort devices: booted first, then by name
    devices.sort_by(|a, b| match (a.booted, b.booted) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.cmp(&b.name),
    });

    Ok(devices)
}

async fn dev_cli() -> Result<()> {
    use std::process::Command as StdCommand;

    println!("Starting CLI in development mode with watch...");

    // Check if cargo-watch is installed
    let watch_available = StdCommand::new("cargo")
        .args(["watch", "--version"])
        .output()
        .is_ok();

    if !watch_available {
        println!("⚠️  cargo-watch not found. Installing...");
        let status = StdCommand::new("cargo")
            .args(["install", "cargo-watch"])
            .status()
            .context("Failed to install cargo-watch")?;

        if !status.success() {
            anyhow::bail!(
                "Failed to install cargo-watch. Please install manually: cargo install cargo-watch"
            );
        }
        println!("✓ cargo-watch installed");
    }

    // Run cargo watch to rebuild on changes
    println!("🔄 Watching for changes... (Press Ctrl+C to stop)");
    println!("💡 The CLI will automatically rebuild and run when you make changes");

    // Note: cargo watch will run until interrupted (Ctrl+C)
    // We use spawn() and wait so it runs in the foreground
    // The command needs to be split properly: cargo watch -x "run --bin halvor"
    let mut child = StdCommand::new("cargo")
        .args(["watch", "-x", "run --bin halvor"])
        .spawn()
        .context("Failed to start cargo watch. Make sure cargo-watch is installed: cargo install cargo-watch")?;

    // Wait for the process to finish (will be interrupted by user with Ctrl+C)
    match child.wait() {
        Ok(status) => {
            if !status.success() {
                eprintln!("⚠️  cargo watch exited with non-zero status");
            }
        }
        Err(e) => {
            eprintln!("⚠️  Error waiting for cargo watch: {}", e);
        }
    }

    Ok(())
}

async fn handle_web_bare_metal(port: u16, static_dir: Option<PathBuf>) -> Result<()> {
    use crate::services::web;

    // Determine static directory
    let static_path = if let Some(dir) = static_dir {
        dir
    } else {
        // Default to halvor-web build directory or dev mode
        let web_dir = PathBuf::from("halvor-web");
        let build_dir = web_dir.join("build");

        if build_dir.exists() {
            build_dir
        } else {
            // Development mode - serve from src and proxy to Vite dev server
            println!("⚠️  No build directory found. Starting Svelte dev server...");

            // Start Svelte dev server in background
            let mut svelte_dev = StdCommand::new("npm")
                .arg("run")
                .arg("dev")
                .current_dir(&web_dir)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?;

            println!("📦 Svelte dev server starting on http://localhost:5173");
            println!("🌐 Rust API server will proxy to Svelte dev server");

            // For dev mode, we'll proxy to Vite
            // For now, just serve the API and let user access Vite directly
            let addr = SocketAddr::from(([127, 0, 0, 1], port));
            println!("🚀 Starting Rust API server on http://{}", addr);
            println!("💡 Access Svelte app at http://localhost:5173");
            println!("💡 API available at http://{}/api/*", addr);

            // Start Rust server (this will block)
            web::start_server(addr, web_dir, None).await?;

            // Cleanup
            let _ = svelte_dev.kill();
            return Ok(());
        }
    };

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    web::start_server(addr, static_path, None).await?;

    Ok(())
}

async fn handle_web_docker(_port: u16) -> Result<()> {
    println!("Starting web development in Docker...");

    // Build and start Docker container
    let status = Command::new("docker-compose")
        .arg("up")
        .arg("--build")
        .current_dir("halvor-web")
        .status()?;

    if !status.success() {
        anyhow::bail!("Docker dev failed");
    }

    Ok(())
}

async fn handle_web_prod() -> Result<()> {
    println!("Starting web app in production mode (Docker)...");
    let web_dir = PathBuf::from("halvor-web");
    let status = Command::new("docker-compose")
        .args(["up", "halvor-web-prod"])
        .current_dir(&web_dir)
        .status()
        .context("Failed to start production Docker container")?;

    if !status.success() {
        anyhow::bail!("Failed to run web production container");
    }

    Ok(())
}
