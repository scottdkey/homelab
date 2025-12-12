use anyhow::{Context, Result};
use clap::Subcommand;
use serde_json;
use std::path::PathBuf;
use std::process::Command;

#[derive(Subcommand)]
pub enum BuildCommands {
    /// Build iOS app (always signed)
    Ios {
        /// Push to App Store Connect after building
        #[arg(long)]
        push: bool,
    },
    /// Build macOS app (always signed)
    Mac,
    /// Build Android app (always signed)
    Android,
    /// Build Web app (Rust server + Svelte frontend)
    Web {
        /// Build for production release
        #[arg(long)]
        release: bool,
        /// Run the container after building
        #[arg(long)]
        run: bool,
        /// Build Docker container
        #[arg(long)]
        docker: bool,
        /// Push Docker image to GitHub Container Registry
        #[arg(long)]
        push: bool,
    },
    /// Build CLI binary
    Cli,
}

pub fn handle_build(command: BuildCommands) -> Result<()> {
    match command {
        BuildCommands::Ios { push } => {
            // Always build and sign
            build_and_sign_ios()?;
            println!("✓ iOS build complete");

            if push {
                push_ios_to_app_store()?;
            }
        }
        BuildCommands::Mac => {
            // Always build and sign
            build_and_sign_mac()?;
            println!("✓ macOS build complete");
        }
        BuildCommands::Android => {
            // Always build and sign
            build_android()?;
            sign_android()?;
            println!("✓ Android build complete");
        }
        BuildCommands::Web {
            release,
            run,
            docker,
            push,
        } => {
            if docker {
                build_web_docker(release, push)?;
            } else {
                build_web(release)?;
                if run {
                    run_web_prod()?;
                }
            }
            println!("✓ Web build complete");
        }
        BuildCommands::Cli => {
            build_cli()?;
            println!("✓ CLI build complete");
        }
    }

    Ok(())
}

fn build_ios() -> Result<()> {
    println!("Building iOS app with Fastlane...");

    let status = Command::new("fastlane")
        .args(["ios", "ios_build_app"])
        .current_dir("fastlane")
        .status()
        .context("Failed to execute Fastlane iOS build")?;

    if !status.success() {
        anyhow::bail!("iOS build failed. Check Fastlane output for details.");
    }

    Ok(())
}

fn build_mac() -> Result<()> {
    println!("Building macOS app with Fastlane...");

    let status = Command::new("fastlane")
        .args(["mac", "mac_build_app"])
        .current_dir("fastlane")
        .status()
        .context("Failed to execute Fastlane macOS build")?;

    if !status.success() {
        anyhow::bail!("macOS build failed. Check Fastlane output for details.");
    }

    Ok(())
}

fn build_android() -> Result<()> {
    println!("Building Android JNI library...");

    // Build for all Android targets
    let targets = [
        "aarch64-linux-android",
        "armv7-linux-androideabi",
        "i686-linux-android",
        "x86_64-linux-android",
    ];

    for target in &targets {
        println!("Building for target: {}", target);
        let status = Command::new("cargo")
            .args(["build", "--lib", "--release", "--target", target])
            .status()
            .context(format!("Failed to build for {}", target))?;

        if !status.success() {
            anyhow::bail!("Failed to build for {}", target);
        }
    }

    println!("Copying JNI libraries to Android project...");
    let jni_libs = PathBuf::from("halvor-android/src/main/jniLibs");

    // Create directories
    for (arch, target) in [
        ("arm64-v8a", "aarch64-linux-android"),
        ("armeabi-v7a", "armv7-linux-androideabi"),
        ("x86", "i686-linux-android"),
        ("x86_64", "x86_64-linux-android"),
    ] {
        let lib_dir = jni_libs.join(arch);
        std::fs::create_dir_all(&lib_dir)
            .context(format!("Failed to create directory: {}", lib_dir.display()))?;

        let src_lib = PathBuf::from("target")
            .join(target)
            .join("release")
            .join("libhalvor.so");

        let dst_lib = lib_dir.join("libhalvor_jni.so");

        std::fs::copy(&src_lib, &dst_lib).context(format!(
            "Failed to copy library from {} to {}",
            src_lib.display(),
            dst_lib.display()
        ))?;
    }

    println!("Building Android app...");
    let gradle_dir = PathBuf::from("halvor-android");
    let status = Command::new("./gradlew")
        .arg("build")
        .current_dir(&gradle_dir)
        .status()
        .context("Failed to execute Gradle build")?;

    if !status.success() {
        anyhow::bail!("Android Gradle build failed");
    }

    Ok(())
}

fn build_web(release: bool) -> Result<()> {
    println!("Building web application...");

    // Step 1: Build the Rust server binary (with SQLite and web-server features)
    println!("Building Rust web server...");
    let mut cargo_args = vec!["build", "--bin", "halvor"];

    if release {
        cargo_args.push("--release");
        println!("Building in release mode...");
    }

    let status = Command::new("cargo")
        .args(&cargo_args)
        .status()
        .context("Failed to build Rust web server")?;

    if !status.success() {
        anyhow::bail!("Rust server build failed");
    }

    // Step 2: Build the Svelte frontend
    println!("Building Svelte frontend...");
    let web_dir = PathBuf::from("halvor-web");

    // Check if node_modules exists, install if needed
    if !web_dir.join("node_modules").exists() {
        println!("Installing npm dependencies...");
        let status = Command::new("npm")
            .arg("install")
            .current_dir(&web_dir)
            .status()
            .context("Failed to install npm dependencies")?;

        if !status.success() {
            anyhow::bail!("npm install failed");
        }
    }

    // Build Svelte app
    let mut npm_args = vec!["run", "build"];
    if release {
        npm_args.push("--");
        npm_args.push("--mode");
        npm_args.push("production");
    }

    let status = Command::new("npm")
        .args(&npm_args)
        .current_dir(&web_dir)
        .status()
        .context("Failed to build Svelte app")?;

    if !status.success() {
        anyhow::bail!("Svelte build failed");
    }

    println!("✓ Web build complete!");
    println!(
        "  - Rust server: target/{}/halvor",
        if release { "release" } else { "debug" }
    );
    println!("  - Svelte app: halvor-web/build/");
    println!("\nTo run the server:");
    println!("  halvor dev web --bare-metal");
    if release {
        println!("  or: cargo run --release --bin halvor -- dev web --bare-metal");
    } else {
        println!("  or: cargo run --bin halvor -- dev web --bare-metal");
    }

    Ok(())
}

fn build_web_docker(release: bool, push: bool) -> Result<()> {
    use crate::services::docker::build::{
        build_image, check_docker_auth, generate_ghcr_tags, get_git_hash, get_github_user,
        push_images, DockerBuildConfig,
    };
    use std::path::PathBuf;

    println!("Building Docker container for web application...");

    // Get GitHub user and git hash
    let github_user = get_github_user();
    if github_user == "unknown" {
        println!(
            "⚠️  Warning: Could not determine GitHub user. Set GITHUB_USER environment variable."
        );
        println!("   Using 'unknown' as image name prefix.");
    }

    let git_hash = get_git_hash();

    // Generate image tags
    let tags = generate_ghcr_tags(&github_user, "halvor-web", release, &git_hash);

    println!("Building Docker image...");
    for tag in &tags {
        println!("  Tag: {}", tag);
    }
    println!("  Release mode: {}", release);
    println!();

    // Build Docker image using docker_build module
    let dockerfile = PathBuf::from("halvor-web/Dockerfile");
    let context = PathBuf::from(".");

    let build_config = DockerBuildConfig::new(dockerfile, context)
        .with_target("production")
        .with_build_arg("BUILD_TYPE", if release { "release" } else { "debug" })
        .with_tags(tags.clone());

    println!("Building Docker image...");
    build_image(&build_config)?;

    println!("✓ Docker image built successfully");
    for tag in &tags {
        println!("  - {}", tag);
    }
    println!();

    if push {
        println!("Pushing to GitHub Container Registry...");
        check_docker_auth()?;
        push_images(&tags)?;

        println!();
        println!("✓ All images pushed successfully");
        println!();
        println!("To use this image:");
        println!("  docker run -p 13000:13000 {}", tags[0]);
    } else {
        println!("To push the image:");
        println!("  halvor build web --docker --release --push");
    }

    Ok(())
}

fn run_web_prod() -> Result<()> {
    println!("Starting web app in production mode (Docker)...");
    let web_dir = PathBuf::from("halvor-web");
    let status = Command::new("docker-compose")
        .args(["up", "--build"])
        .current_dir(&web_dir)
        .status()
        .context("Failed to start Docker container")?;

    if !status.success() {
        anyhow::bail!("Failed to run web production container");
    }

    Ok(())
}

fn build_and_sign_ios() -> Result<()> {
    // Build the app first
    build_ios()?;

    // Then sign it
    sign_ios()?;

    Ok(())
}

fn sign_ios() -> Result<()> {
    println!("Signing iOS app with Fastlane...");

    // First, ensure the app was built
    let app_path =
        PathBuf::from("halvor-swift/build/Build/Products/Release-iphoneos/HalvorApp-iOS.app");
    if !app_path.exists() {
        anyhow::bail!(
            "iOS app not found at {}. Build step must have failed.",
            app_path.display()
        );
    }

    // Run Fastlane to sign the app
    let status = Command::new("fastlane")
        .args(["ios", "sign_app"])
        .current_dir("fastlane")
        .status()
        .context("Failed to execute Fastlane iOS signing")?;

    if !status.success() {
        anyhow::bail!("iOS signing failed. Check Fastlane output for details.");
    }

    println!("✓ iOS app signed successfully");
    Ok(())
}

fn build_and_sign_mac() -> Result<()> {
    // Build the app first
    build_mac()?;

    // Then sign it
    sign_mac()?;

    Ok(())
}

fn sign_mac() -> Result<()> {
    println!("Signing macOS app with Fastlane...");

    // First, ensure the app was built
    let app_path = PathBuf::from("halvor-swift/build/Build/Products/Release/HalvorApp-macOS.app");
    if !app_path.exists() {
        anyhow::bail!(
            "macOS app not found at {}. Build step must have failed.",
            app_path.display()
        );
    }

    let status = Command::new("fastlane")
        .args(["mac", "sign_app"])
        .current_dir("fastlane")
        .status()
        .context("Failed to execute Fastlane macOS signing")?;

    if !status.success() {
        anyhow::bail!("macOS signing failed. Check Fastlane output for details.");
    }

    println!("✓ macOS app signed successfully");
    Ok(())
}

fn sign_android() -> Result<()> {
    println!("Signing Android app...");
    let gradle_dir = PathBuf::from("halvor-android");
    let status = Command::new("./gradlew")
        .args(["assembleRelease", "bundleRelease"])
        .current_dir(&gradle_dir)
        .status()
        .context("Failed to execute Gradle signing")?;

    if !status.success() {
        anyhow::bail!("Android signing failed");
    }

    Ok(())
}

fn build_cli() -> Result<()> {
    println!("Building CLI binary...");
    let status = Command::new("cargo")
        .args(["build", "--release", "--bin", "halvor"])
        .status()
        .context("Failed to build CLI")?;

    if !status.success() {
        anyhow::bail!("CLI build failed");
    }

    println!("CLI binary built at: target/release/halvor");
    Ok(())
}

fn push_ios_to_app_store() -> Result<()> {
    use std::path::PathBuf;
    println!("Pushing iOS app to App Store Connect...");

    // First, ensure the app was built and signed
    let app_path =
        PathBuf::from("halvor-swift/build/Build/Products/Release-iphoneos/HalvorApp-iOS.app");
    if !app_path.exists() {
        anyhow::bail!(
            "iOS app not found at {}. Build step must have failed.",
            app_path.display()
        );
    }

    // Create an archive (.ipa) from the app bundle
    println!("Creating IPA archive...");
    let ipa_dir = PathBuf::from("halvor-swift/build/ipa");
    std::fs::create_dir_all(&ipa_dir).context("Failed to create IPA directory")?;

    let payload_dir = ipa_dir.join("Payload");
    std::fs::create_dir_all(&payload_dir).context("Failed to create Payload directory")?;

    // Copy app bundle to Payload directory
    let dest_app = payload_dir.join("HalvorApp-iOS.app");
    if dest_app.exists() {
        std::fs::remove_dir_all(&dest_app).context("Failed to remove existing app in Payload")?;
    }

    // Use ditto to copy the app bundle (preserves symlinks and metadata)
    let status = Command::new("ditto")
        .args([&app_path, &dest_app])
        .status()
        .context("Failed to copy app bundle to Payload")?;

    if !status.success() {
        anyhow::bail!("Failed to copy app bundle to Payload directory");
    }

    // Create IPA file
    let ipa_path = ipa_dir.join("HalvorApp-iOS.ipa");
    if ipa_path.exists() {
        std::fs::remove_file(&ipa_path).with_context(|| {
            format!("Failed to remove existing IPA file: {}", ipa_path.display())
        })?;
    }

    // Ensure ipa_dir exists (in case it was deleted)
    std::fs::create_dir_all(&ipa_dir).with_context(|| {
        format!(
            "Failed to ensure IPA directory exists: {}",
            ipa_dir.display()
        )
    })?;

    // Use absolute path for the IPA file to avoid issues with zip
    let ipa_path_abs = std::fs::canonicalize(&ipa_dir)
        .context("Failed to get absolute path for IPA directory")?
        .join("HalvorApp-iOS.ipa");

    let ipa_path_str = ipa_path_abs
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("IPA path contains invalid UTF-8"))?;

    let status = Command::new("zip")
        .args(["-r", ipa_path_str, "Payload"])
        .current_dir(&ipa_dir)
        .status()
        .with_context(|| {
            format!(
                "Failed to create IPA archive at: {}",
                ipa_path_abs.display()
            )
        })?;

    if !status.success() {
        anyhow::bail!("Failed to create IPA archive");
    }

    println!("✓ IPA archive created: {}", ipa_path_abs.display());

    // Upload to App Store Connect using Fastlane
    // Pass the IPA path as an environment variable so Fastlane can use it
    // Also ensure environment variables from .envrc are passed through
    println!("Uploading to App Store Connect via Fastlane...");

    let mut fastlane_cmd = Command::new("fastlane");
    fastlane_cmd
        .args(["ios", "ios_upload_to_app_store"])
        .current_dir("fastlane")
        .env("IPA_PATH", ipa_path_str);

    // Pass through App Store Connect credentials from environment
    // Debug: Show which credentials are available
    eprintln!("Checking for App Store Connect credentials in environment...");
    let mut found_vars = Vec::new();
    let mut missing_vars = Vec::new();

    // Check for API key path - if it's a 1Password reference, download it
    // Helper function to download from 1Password
    fn download_api_key_from_1password() -> Result<Option<String>> {
        let vault = std::env::var("VAULT_NAME").unwrap_or_else(|_| "automations".to_string());
        let item = std::env::var("ITEM_NAME").unwrap_or_else(|_| "halvor".to_string());

        eprintln!(
            "  → Attempting to download API key from 1Password item '{}' in vault '{}'...",
            item, vault
        );

        // First, get the item JSON to find .p8 files
        let item_output = Command::new("op")
            .args(["item", "get", &item, "--vault", &vault, "--format", "json"])
            .output()
            .context("Failed to query 1Password item")?;

        if !item_output.status.success() {
            eprintln!(
                "  ⚠️  Could not access 1Password item. Make sure you're signed in: op signin"
            );
            return Ok(None);
        }

        // Parse JSON to find .p8 files
        let item_json: serde_json::Value = serde_json::from_slice(&item_output.stdout)
            .context("Failed to parse 1Password item JSON")?;

        // Look for files in the item
        let mut p8_files = Vec::new();
        if let Some(files) = item_json.get("files").and_then(|f| f.as_array()) {
            for file in files {
                if let Some(name) = file.get("name").and_then(|n| n.as_str()) {
                    if name.ends_with(".p8") {
                        p8_files.push(name.to_string());
                    }
                }
            }
        }

        if p8_files.is_empty() {
            eprintln!("  ⚠️  No .p8 files found in 1Password item");
            return Ok(None);
        }

        // Use the first .p8 file found (or prefer AuthKey.p8 if it exists)
        let file_name = if p8_files.iter().any(|f| f == "AuthKey.p8") {
            "AuthKey.p8"
        } else {
            &p8_files[0]
        };

        eprintln!("  → Found .p8 file: {}", file_name);
        let temp_key_path = std::env::temp_dir().join("app_store_connect_api_key.p8");

        let output = Command::new("op")
            .args([
                "item",
                "get",
                &item,
                "--vault",
                &vault,
                "--file",
                file_name,
                "--output",
                temp_key_path.to_str().unwrap(),
            ])
            .output()
            .context("Failed to download API key from 1Password")?;

        if output.status.success() && temp_key_path.exists() {
            eprintln!("  ✓ Downloaded API key file '{}' from 1Password", file_name);
            Ok(Some(temp_key_path.to_string_lossy().to_string()))
        } else {
            eprintln!("  ⚠️  Failed to download API key file from 1Password");
            Ok(None)
        }
    }

    let api_key_path = match std::env::var("APP_STORE_CONNECT_API_KEY_PATH") {
        Ok(path) if !path.is_empty() => {
            // Check if it's a 1Password reference (op://)
            if path.starts_with("op://") {
                println!("  → Downloading API key from 1Password...");
                // Download from 1Password to a temp file
                let temp_key_path = std::env::temp_dir().join("app_store_connect_api_key.p8");

                let status = Command::new("op")
                    .args(["read", &path, "--outfile", temp_key_path.to_str().unwrap()])
                    .status()
                    .context("Failed to download API key from 1Password")?;

                if !status.success() {
                    anyhow::bail!("Failed to download API key from 1Password. Make sure you're signed in: op signin");
                }

                println!("  ✓ Downloaded API key to temporary file");
                Some(temp_key_path.to_string_lossy().to_string())
            } else if std::path::Path::new(&path).exists() {
                Some(path)
            } else {
                println!("  ⚠️  API key path doesn't exist: {}", path);
                None
            }
        }
        _ => {
            // Try to download from 1Password using the helper function
            eprintln!("  → APP_STORE_CONNECT_API_KEY_PATH not set, attempting to download from 1Password...");
            match download_api_key_from_1password() {
                Ok(Some(path)) => {
                    eprintln!("  ✓ Successfully downloaded API key file");
                    Some(path)
                }
                Ok(None) => {
                    eprintln!("  ⚠️  Could not download API key file from 1Password (file not found or download failed)");
                    None
                }
                Err(e) => {
                    eprintln!("  ⚠️  Error downloading from 1Password: {}", e);
                    None
                }
            }
        }
    };

    // Set the API key path if we have it
    if let Some(ref key_path) = api_key_path {
        fastlane_cmd.env("APP_STORE_CONNECT_API_KEY_PATH", key_path);
        found_vars.push("APP_STORE_CONNECT_API_KEY_PATH");
        eprintln!("  ✓ Found: APP_STORE_CONNECT_API_KEY_PATH");
    } else {
        missing_vars.push("APP_STORE_CONNECT_API_KEY_PATH");
        eprintln!("  ✗ Missing: APP_STORE_CONNECT_API_KEY_PATH");
    }

    let credential_vars = [
        "APP_STORE_CONNECT_API_KEY_ID",
        "APP_STORE_CONNECT_API_ISSUER",
        "FASTLANE_USER",
        "APP_STORE_CONNECT_USERNAME",
        "FASTLANE_PASSWORD",
        "APP_STORE_CONNECT_PASSWORD",
    ];

    for var_name in &credential_vars {
        match std::env::var(var_name) {
            Ok(val) => {
                if !val.is_empty() {
                    fastlane_cmd.env(var_name, &val);
                    found_vars.push(*var_name);
                    println!("  ✓ Found: {}", var_name);
                } else {
                    missing_vars.push(*var_name);
                    println!("  ✗ Empty: {}", var_name);
                }
            }
            Err(_) => {
                missing_vars.push(*var_name);
                println!("  ✗ Missing: {}", var_name);
            }
        }
    }

    eprintln!();
    if found_vars.len() >= 3 {
        eprintln!("✓ All required API key credentials found!");
    } else if !found_vars.is_empty() {
        eprintln!(
            "⚠️  Found {} credential(s), but need 3 for API key auth or 2 for username/password",
            found_vars.len()
        );
        eprintln!("   Found: {}", found_vars.join(", "));
        eprintln!("   Missing: {}", missing_vars.join(", "));
    } else {
        eprintln!("⚠️  No App Store Connect credentials found in environment!");
        eprintln!("   Make sure:");
        eprintln!("   1. Variables are in your 1Password vault with exact names");
        eprintln!("   2. direnv is loaded (run 'direnv allow' in this directory)");
        eprintln!("   3. You're running halvor from a shell where direnv is active");
    }
    eprintln!();

    let status = fastlane_cmd
        .status()
        .context("Failed to execute Fastlane upload")?;

    if !status.success() {
        anyhow::bail!("Failed to upload to App Store Connect. Check Fastlane output for details.");
    }

    println!("✓ iOS app uploaded to App Store Connect successfully");
    println!("  IPA location: {}", ipa_path_abs.display());
    println!("  Check App Store Connect for processing status.");

    Ok(())
}
