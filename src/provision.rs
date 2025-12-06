use crate::config::EnvConfig;
use anyhow::{Context, Result};
use std::process::{Command, Stdio};

pub fn provision_host(hostname: &str, portainer_host: bool, config: &EnvConfig) -> Result<()> {
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

    // Build the provisioning script
    let script = build_provision_script(portainer_host)?;

    // Copy the appropriate docker-compose file
    if portainer_host {
        copy_portainer_compose(&target_host, "portainer.docker-compose.yml")?;
    } else {
        copy_portainer_compose(&target_host, "portainer-agent.docker-compose.yml")?;
    }

    // Execute the script via SSH
    execute_provision_script(&target_host, &script)?;

    println!();
    println!("✓ Provisioning complete for {}", hostname);

    Ok(())
}

fn build_provision_script(portainer_host: bool) -> Result<String> {
    let mut script = String::from("#!/bin/bash\nset -e\n\n");

    // Check and install Docker
    script.push_str(
        r#"
echo "=== Checking Docker installation ==="
if ! command -v docker &> /dev/null; then
    echo "Docker not found. Installing Docker..."
    if command -v apt-get &> /dev/null; then
        # Debian/Ubuntu - detect which one using /etc/os-release
        if grep -qi "^ID=debian" /etc/os-release 2>/dev/null; then
            # Debian
            echo "Detected Debian, using Debian Docker repository"
            # Remove any existing Docker repository configuration
            sudo rm -f /etc/apt/sources.list.d/docker.list
            sudo apt-get update
            sudo apt-get install -y ca-certificates curl gnupg
            sudo install -m 0755 -d /etc/apt/keyrings
            curl -fsSL https://download.docker.com/linux/debian/gpg | sudo gpg --dearmor -o /etc/apt/keyrings/docker.gpg
            sudo chmod a+r /etc/apt/keyrings/docker.gpg
            DEBIAN_CODENAME=$(grep VERSION_CODENAME /etc/os-release 2>/dev/null | cut -d= -f2 || lsb_release -cs 2>/dev/null || echo "bookworm")
            echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.gpg] https://download.docker.com/linux/debian ${DEBIAN_CODENAME} stable" | sudo tee /etc/apt/sources.list.d/docker.list > /dev/null
        else
            # Ubuntu (or default to Ubuntu if detection fails)
            echo "Detected Ubuntu, using Ubuntu Docker repository"
            # Remove any existing Docker repository configuration
            sudo rm -f /etc/apt/sources.list.d/docker.list
            sudo apt-get update
            sudo apt-get install -y ca-certificates curl gnupg lsb-release
            sudo install -m 0755 -d /etc/apt/keyrings
            curl -fsSL https://download.docker.com/linux/ubuntu/gpg | sudo gpg --dearmor -o /etc/apt/keyrings/docker.gpg
            sudo chmod a+r /etc/apt/keyrings/docker.gpg
            echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.gpg] https://download.docker.com/linux/ubuntu $(lsb_release -cs) stable" | sudo tee /etc/apt/sources.list.d/docker.list > /dev/null
        fi
        sudo apt-get update
        sudo apt-get install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin
    elif command -v yum &> /dev/null; then
        # RHEL/CentOS
        sudo yum install -y yum-utils
        sudo yum-config-manager --add-repo https://download.docker.com/linux/centos/docker-ce.repo
        sudo yum install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin
        sudo systemctl start docker
        sudo systemctl enable docker
    elif command -v dnf &> /dev/null; then
        # Fedora
        sudo dnf install -y dnf-plugins-core
        sudo dnf config-manager --add-repo https://download.docker.com/linux/fedora/docker-ce.repo
        sudo dnf install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin
        sudo systemctl start docker
        sudo systemctl enable docker
    elif command -v brew &> /dev/null; then
        # macOS
        brew install --cask docker
        echo "Please start Docker Desktop manually"
    else
        echo "Unsupported package manager. Please install Docker manually."
        exit 1
    fi
    echo "✓ Docker installed"
else
    echo "✓ Docker already installed"
fi
"#,
    );

    // Check and install Tailscale
    script.push_str(
        r#"
echo ""
echo "=== Checking Tailscale installation ==="
if ! command -v tailscale &> /dev/null; then
    echo "Tailscale not found. Installing Tailscale..."
    if command -v apt-get &> /dev/null; then
        curl -fsSL https://tailscale.com/install.sh | sh
    elif command -v yum &> /dev/null; then
        curl -fsSL https://tailscale.com/install.sh | sh
    elif command -v dnf &> /dev/null; then
        curl -fsSL https://tailscale.com/install.sh | sh
    elif command -v brew &> /dev/null; then
        brew install tailscale
    else
        echo "Unsupported package manager. Please install Tailscale manually."
        exit 1
    fi
    echo "✓ Tailscale installed"
    echo "Note: Run 'sudo tailscale up' to connect to your tailnet"
else
    echo "✓ Tailscale already installed"
fi
"#,
    );

    // Add user to docker group if needed (Linux only)
    script.push_str(
        r#"
echo ""
echo "=== Configuring Docker permissions ==="
if [ "$(uname)" != "Darwin" ]; then
    if ! groups | grep -q docker; then
        echo "Adding user to docker group..."
        sudo usermod -aG docker $USER
        echo "✓ User added to docker group"
        echo "Note: You may need to log out and back in for changes to take effect"
    else
        echo "✓ User already in docker group"
    fi
fi
"#,
    );

    // Install Portainer
    if portainer_host {
        script.push_str(
            r#"
echo ""
echo "=== Installing Portainer CE ==="
# Remove any existing Portainer containers and volumes
echo "Removing any existing Portainer instances..."
if docker ps -a --format '{{.Names}}' 2>/dev/null | grep -q '^portainer$'; then
    docker stop portainer 2>/dev/null || sudo docker stop portainer 2>/dev/null
    docker rm portainer 2>/dev/null || sudo docker rm portainer 2>/dev/null
    echo "✓ Removed existing Portainer container"
fi
# Also remove any portainer_agent containers
if docker ps -a --format '{{.Names}}' 2>/dev/null | grep -q '^portainer_agent$'; then
    docker stop portainer_agent 2>/dev/null || sudo docker stop portainer_agent 2>/dev/null
    docker rm portainer_agent 2>/dev/null || sudo docker rm portainer_agent 2>/dev/null
    echo "✓ Removed existing Portainer Agent container"
fi

# Use docker-compose file that was copied to the portainer directory
# Use the home directory of the current user
PORTAINER_DIR="$HOME/portainer"
if [ ! -d "$PORTAINER_DIR" ]; then
    mkdir -p "$PORTAINER_DIR"
fi
if [ -f "$PORTAINER_DIR/docker-compose.yml" ]; then
    cd "$PORTAINER_DIR"
    # Stop and remove any existing containers from compose
    if command -v docker &> /dev/null && docker compose version &> /dev/null; then
        docker compose down 2>/dev/null || sudo docker compose down 2>/dev/null
        # Start with docker compose
        if docker compose up -d 2>/dev/null; then
            echo "✓ Portainer CE installed and running (via docker compose)"
        else
            sudo docker compose up -d
            echo "✓ Portainer CE installed and running (via docker compose)"
        fi
    elif command -v docker-compose &> /dev/null; then
        docker-compose down 2>/dev/null || sudo docker-compose down 2>/dev/null
        # Start with docker-compose
        if docker-compose up -d 2>/dev/null; then
            echo "✓ Portainer CE installed and running (via docker-compose)"
        else
            sudo docker-compose up -d
            echo "✓ Portainer CE installed and running (via docker-compose)"
        fi
    else
        echo "Error: docker compose not available"
        exit 1
    fi
    echo "Access Portainer at https://localhost:9443"
else
    echo "Error: docker-compose.yml not found at $PORTAINER_DIR/docker-compose.yml"
    exit 1
fi
"#,
        );
    } else {
        script.push_str(
            r#"
echo ""
echo "=== Installing Portainer Agent ==="
# Remove any existing Portainer containers
echo "Removing any existing Portainer instances..."
if docker ps -a --format '{{.Names}}' 2>/dev/null | grep -q '^portainer$'; then
    docker stop portainer 2>/dev/null || sudo docker stop portainer 2>/dev/null
    docker rm portainer 2>/dev/null || sudo docker rm portainer 2>/dev/null
    echo "✓ Removed existing Portainer container"
fi
if docker ps -a --format '{{.Names}}' 2>/dev/null | grep -q '^portainer_agent$'; then
    docker stop portainer_agent 2>/dev/null || sudo docker stop portainer_agent 2>/dev/null
    docker rm portainer_agent 2>/dev/null || sudo docker rm portainer_agent 2>/dev/null
    echo "✓ Removed existing Portainer Agent container"
fi

# Use docker-compose file that was copied to the portainer directory
# Use the home directory of the current user
PORTAINER_DIR="$HOME/portainer"
if [ ! -d "$PORTAINER_DIR" ]; then
    mkdir -p "$PORTAINER_DIR"
fi
if [ -f "$PORTAINER_DIR/docker-compose.yml" ]; then
    cd "$PORTAINER_DIR"
    # Stop and remove any existing containers from compose
    if command -v docker &> /dev/null && docker compose version &> /dev/null; then
        docker compose down 2>/dev/null || sudo docker compose down 2>/dev/null
        # Start with docker compose
        if docker compose up -d 2>/dev/null; then
            echo "✓ Portainer Agent installed and running (via docker compose)"
        else
            sudo docker compose up -d
            echo "✓ Portainer Agent installed and running (via docker compose)"
        fi
    elif command -v docker-compose &> /dev/null; then
        docker-compose down 2>/dev/null || sudo docker-compose down 2>/dev/null
        # Start with docker-compose
        if docker-compose up -d 2>/dev/null; then
            echo "✓ Portainer Agent installed and running (via docker-compose)"
        else
            sudo docker-compose up -d
            echo "✓ Portainer Agent installed and running (via docker-compose)"
        fi
    else
        echo "Error: docker compose not available"
        exit 1
    fi
    echo "Add this agent to your Portainer instance using the agent endpoint"
else
    echo "Error: docker-compose.yml not found at $PORTAINER_DIR/docker-compose.yml"
    exit 1
fi
"#,
        );
    }

    script.push_str("\necho \"\n=== Provisioning complete ===\"\n");

    Ok(script)
}

fn copy_portainer_compose(host: &str, compose_filename: &str) -> Result<()> {
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

    // Determine username for SSH - try key-based auth first
    let default_user = crate::config::get_default_username();
    let host_with_user = format!("{}@{}", default_user, host);

    // Test if key-based auth works
    let test_cmd = format!(
        r#"ssh -o ConnectTimeout=1 -o BatchMode=yes -o PreferredAuthentications=publickey -o PasswordAuthentication=no -o StrictHostKeyChecking=no {} 'echo test' >/dev/null 2>&1"#,
        host_with_user
    );

    let test_status = Command::new("sh")
        .arg("-c")
        .arg(&test_cmd)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    let use_key_auth = test_status.is_ok() && test_status.unwrap().success();

    // Create directory first - use the actual $HOME of the user we're connecting as
    // This should work since we're connecting as that user
    let mkdir_command =
        r#"mkdir -p "$HOME/portainer" 2>/dev/null || mkdir -p "$(eval echo ~$USER)/portainer""#;

    let mut mkdir_cmd = Command::new("ssh");
    if use_key_auth {
        mkdir_cmd.args([
            "-o",
            "StrictHostKeyChecking=no",
            "-o",
            "PreferredAuthentications=publickey",
            "-o",
            "PasswordAuthentication=no",
            &host_with_user,
            "bash",
            "-c",
            mkdir_command,
        ]);
    } else {
        mkdir_cmd.args([
            "-o",
            "StrictHostKeyChecking=no",
            "-o",
            "PreferredAuthentications=publickey,keyboard-interactive,password",
            &host_with_user,
            "bash",
            "-c",
            mkdir_command,
        ]);
    }

    mkdir_cmd.stdout(Stdio::null());
    mkdir_cmd.stderr(Stdio::inherit());

    let mkdir_status = mkdir_cmd.status()?;
    if !mkdir_status.success() {
        anyhow::bail!("Failed to create $HOME/portainer directory on remote system");
    }

    // Now write the file - use $HOME of the user we're connecting as
    let setup_cmd = r#"cat > "$HOME/portainer/docker-compose.yml" || cat > "$(eval echo ~$USER)/portainer/docker-compose.yml""#;

    let mut cmd = Command::new("ssh");
    if use_key_auth {
        cmd.args([
            "-o",
            "StrictHostKeyChecking=no",
            "-o",
            "PreferredAuthentications=publickey",
            "-o",
            "PasswordAuthentication=no",
            &host_with_user,
            "bash",
            "-c",
            &setup_cmd,
        ]);
    } else {
        cmd.args([
            "-o",
            "StrictHostKeyChecking=no",
            "-o",
            "PreferredAuthentications=publickey,keyboard-interactive,password",
            &host_with_user,
            "bash",
            "-c",
            &setup_cmd,
        ]);
    }

    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::inherit());

    let mut child = cmd.spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        stdin.write_all(compose_content.as_bytes())?;
        stdin.flush()?;
        drop(stdin);
    }

    let status = child.wait()?;
    if !status.success() {
        anyhow::bail!("Failed to copy Portainer docker-compose file to remote system");
    }

    println!("✓ Copied {} to remote system", compose_filename);
    Ok(())
}

fn execute_provision_script(host: &str, script: &str) -> Result<()> {
    use std::io::Write;

    // Try key-based authentication first with default username
    let default_user = crate::config::get_default_username();
    let host_with_user = format!("{}@{}", default_user, host);

    // First, try key-based authentication (silent test)
    let test_cmd = format!(
        r#"ssh -o ConnectTimeout=1 -o BatchMode=yes -o PreferredAuthentications=publickey -o PasswordAuthentication=no -o StrictHostKeyChecking=no {} 'echo test' >/dev/null 2>&1"#,
        host_with_user
    );

    let test_status = Command::new("sh")
        .arg("-c")
        .arg(&test_cmd)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    let use_key_auth = test_status.is_ok() && test_status.unwrap().success();

    // Determine username - use default if key auth works, otherwise prompt
    let username = if use_key_auth {
        default_user
    } else {
        // Key-based auth failed, prompt for username
        print!(
            "Username for {} (press Enter for '{}'): ",
            host, default_user
        );
        std::io::stdout().flush()?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        if input.trim().is_empty() {
            default_user
        } else {
            input.trim().to_string()
        }
    };

    let host_with_user = format!("{}@{}", username, host);

    // Use SSH to execute the script on the remote host
    // We'll use bash -s to pipe the script through SSH
    let mut cmd = Command::new("ssh");

    // Add options - prefer key-based auth if available, otherwise allow password
    // Use -tt to force TTY allocation even when stdin is piped (needed for sudo prompts)
    if use_key_auth {
        cmd.args([
            "-o",
            "PreferredAuthentications=publickey",
            "-o",
            "PasswordAuthentication=no",
            "-o",
            "StrictHostKeyChecking=no",
            "-tt", // Force pseudo-terminal allocation even when stdin is not a terminal
            &host_with_user,
            "bash",
        ]);
    } else {
        cmd.args([
            "-o",
            "PreferredAuthentications=keyboard-interactive,password,publickey",
            "-o",
            "StrictHostKeyChecking=no",
            "-tt", // Force pseudo-terminal allocation even when stdin is not a terminal
            &host_with_user,
            "bash",
        ]);
    }

    // Instead of piping the script, write it to a temp file on the remote system
    // This allows sudo password prompts to work properly
    let temp_script_path = format!("/tmp/hal-provision-{}.sh", std::process::id());

    // Write script to remote file by piping it through SSH
    // Don't use -tt here (only needed for execution with sudo prompts)
    let mut write_cmd = Command::new("ssh");
    if use_key_auth {
        write_cmd.args([
            "-o",
            "PreferredAuthentications=publickey",
            "-o",
            "PasswordAuthentication=no",
            "-o",
            "StrictHostKeyChecking=no",
            &host_with_user,
            "bash",
            "-c",
            &format!(
                "cat > {} && chmod +x {}",
                temp_script_path, temp_script_path
            ),
        ]);
    } else {
        write_cmd.args([
            "-o",
            "PreferredAuthentications=keyboard-interactive,password,publickey",
            "-o",
            "StrictHostKeyChecking=no",
            &host_with_user,
            "bash",
            "-c",
            &format!(
                "cat > {} && chmod +x {}",
                temp_script_path, temp_script_path
            ),
        ]);
    }

    write_cmd.stdin(Stdio::piped());
    write_cmd.stdout(Stdio::null()); // Suppress output during write
    write_cmd.stderr(Stdio::inherit()); // Show errors

    let mut write_child = write_cmd.spawn()?;
    if let Some(mut stdin) = write_child.stdin.take() {
        use std::io::Write;
        stdin.write_all(script.as_bytes())?;
        stdin.flush()?;
        drop(stdin); // Close stdin to signal EOF
    }

    let write_status = write_child.wait()?;
    if !write_status.success() {
        anyhow::bail!("Failed to write provisioning script to remote system");
    }

    // Now execute the script explicitly with bash
    let mut exec_cmd = Command::new("ssh");
    if use_key_auth {
        exec_cmd.args([
            "-o",
            "PreferredAuthentications=publickey",
            "-o",
            "PasswordAuthentication=no",
            "-o",
            "StrictHostKeyChecking=no",
            "-tt",
            &host_with_user,
            "bash",
            &temp_script_path,
        ]);
    } else {
        exec_cmd.args([
            "-o",
            "PreferredAuthentications=keyboard-interactive,password,publickey",
            "-o",
            "StrictHostKeyChecking=no",
            "-tt",
            &host_with_user,
            "bash",
            &temp_script_path,
        ]);
    }

    exec_cmd.stdin(Stdio::inherit());
    exec_cmd.stdout(Stdio::inherit());
    exec_cmd.stderr(Stdio::inherit());

    let status = exec_cmd.status()?;

    // Clean up the temporary script
    let _ = Command::new("ssh")
        .args([
            "-o",
            "StrictHostKeyChecking=no",
            "-o",
            "BatchMode=yes",
            &host_with_user,
            "rm",
            "-f",
            &temp_script_path,
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    if !status.success() {
        anyhow::bail!(
            "Provisioning script failed with exit code: {}",
            status.code().unwrap_or(1)
        );
    }

    Ok(())
}
