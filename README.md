# HAL - Homelab Automation Layer

[![Build Status](https://github.com/scottdkey/homelab/workflows/Build%20and%20Push/badge.svg)](https://github.com/scottdkey/homelab/actions/workflows/build.yml)
[![Release](https://github.com/scottdkey/homelab/workflows/Release/badge.svg)](https://github.com/scottdkey/homelab/actions/workflows/release.yml)
[![Docker Image](https://img.shields.io/badge/docker-ghcr.io%2Fscottdkey%2Fvpn-blue)](https://github.com/users/scottdkey/packages/container/package/vpn)

A Rust-based CLI tool for managing your homelab infrastructure, with scripts for SSH setup and automation.

**HAL** stands for **Homelab Automation Layer** - your intelligent assistant for homelab operations.

## Installation

### Automatic Installation (Recommended)

Download and run the install script from GitHub:

**On Unix/macOS/Linux:**
```bash
curl -fsSL https://raw.githubusercontent.com/scottdkey/homelab/main/scripts/install.sh | bash
```

Or download and run manually:
```bash
curl -O https://raw.githubusercontent.com/scottdkey/homelab/main/scripts/install.sh
chmod +x install.sh
./install.sh
```

**On Windows (PowerShell):**
```powershell
irm https://raw.githubusercontent.com/scottdkey/homelab/main/scripts/install.ps1 | iex
```

The install scripts will:
- Detect your platform (OS and architecture)
- Download the correct pre-built binary from GitHub releases
- Install to `/usr/local/bin` (Linux/macOS) or `~/.local/bin` (Windows)
- Set up PATH if needed

### Manual Installation

If you already have Rust installed:

```bash
cargo install --path .
```

Or using make:

```bash
make install
```

### Development Mode

For development, use the watch mode that automatically rebuilds and installs on changes:

```bash
make dev
```

This will:
- Watch for changes in the source code
- Automatically rebuild the project
- Install the binary globally

## Documentation

- **[Configuration Guide](docs/configuration.md)** - Setting up your environment file and managing configuration
- **[Setup Guide](docs/setup.md)** - SSH host configuration and key setup
- **[Usage Guide](docs/usage.md)** - Common commands and operations
- **[Development Guide](docs/development.md)** - Building, testing, and contributing
- **[Workflows](docs/workflows.md)** - GitHub Actions CI/CD documentation

## Quick Start

After installation:

1. **Configure HAL**: `hal config init`
2. **Setup SSH hosts**: `./scripts/setup-ssh-hosts.sh`
3. **Setup SSH keys**: `./scripts/setup-ssh-keys.sh <hostname>`
4. **Connect**: `ssh <hostname>`

See the [Configuration Guide](docs/configuration.md) for detailed setup instructions.
