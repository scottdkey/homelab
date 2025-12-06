# HAL - Homelab Automation Layer

A Rust-based CLI tool for managing your homelab infrastructure, with support for SSH connections via local IP or Tailscale.

**HAL** stands for **Homelab Automation Layer** - your intelligent assistant for homelab operations.

## Features

- **SSH Connection**: Connect to hosts via local IP first, with automatic fallback to Tailscale
- **Environment-based Configuration**: Host configurations stored in `.env` file
- **Development Mode**: Auto-build and install on file changes

## Installation

### Automatic Installation (Recommended)

The install scripts will automatically detect if Rust is installed and install it if needed, then build and install hal.

**On Unix/macOS/Linux:**
```bash
./install.sh
```

**On Windows (PowerShell):**
```powershell
.\install.ps1
```

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

## Configuration

1. Copy `.env.example` to `.env`:
   ```bash
   cp .env.example .env
   ```

2. Edit `.env` with your host configurations:
   ```bash
   # Tailscale configuration
   TAILNET_BASE="ts.net"

   # Host configurations
   HOST_bellerophon_IP="10.10.10.14"
   HOST_bellerophon_TAILSCALE="bellerophon"
   ```

## Usage

### SSH to a host

```bash
hal ssh bellerophon
```

With additional SSH arguments:

```bash
hal ssh bellerophon -L 8080:localhost:8080
```

**Fix SSH host key issues:**

If you encounter "host key verification failed" or "host ID mismatch" errors, use the `--fix-keys` flag to remove offending host keys from your `known_hosts` file:

```bash
hal ssh bellerophon --fix-keys
```

This will:
- Prompt you to confirm removal for each configured host address (IP, Tailscale hostname, etc.)
- Remove the offending host keys from `~/.ssh/known_hosts`
- Then attempt to connect via SSH

You can also use the short form:

```bash
hal ssh bellerophon -f
```

### Install Tailscale

Install Tailscale on your system (supports macOS, Linux, and Windows):

```bash
hal tailscale install
```

This will:
- Detect your operating system
- Use the appropriate package manager (Homebrew on macOS, apt/yum/dnf on Linux)
- Provide instructions for starting Tailscale

## Development

### Building

```bash
cargo build --release
```

### Running tests

```bash
cargo test
```

### Development mode with auto-rebuild

```bash
make dev
```

This uses `cargo-watch` to automatically rebuild and reinstall when files change.

## Project Structure

```
.
├── Cargo.toml          # Rust project configuration
├── Makefile            # Development commands
├── install.sh          # Unix/macOS/Linux installation script
├── install.ps1         # Windows PowerShell installation script
├── src/
│   ├── main.rs        # Main CLI application
│   └── *.sh           # Original bash scripts (archived)
├── .env               # Environment configuration (gitignored)
├── .env.example       # Example environment configuration
└── README.md          # This file
```

## Requirements

- Rust (latest stable) - automatically installed by install scripts if not present
- `cargo-watch` (for development mode, installed automatically)
- SSH client
- Tailscale (optional, for Tailscale connections) - can be installed via `hal tailscale install`

