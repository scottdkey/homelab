# Development Guide

## Building

```bash
cargo build --release
```

## Running tests

```bash
cargo test
```

## Development mode with auto-rebuild

```bash
make dev
```

This uses `cargo-watch` to automatically rebuild and reinstall when files change.

## Project Structure

```
.
├── Cargo.toml          # Rust project configuration
├── Makefile            # Development commands
├── scripts/            # Installation and setup scripts
│   ├── install.sh      # Unix/macOS/Linux installation script
│   ├── install.ps1     # Windows PowerShell installation script
│   ├── setup-ssh-hosts.sh
│   └── setup-ssh-keys.sh
├── src/
│   ├── main.rs        # Main CLI application
│   ├── config.rs      # Configuration management
│   ├── config_manager.rs
│   ├── provision.rs   # Host provisioning
│   ├── vpn.rs         # VPN deployment
│   ├── smb.rs         # SMB mount management
│   ├── backup.rs      # Backup/restore operations
│   ├── npm.rs         # Nginx Proxy Manager integration
│   └── tailscale.rs   # Tailscale operations
├── openvpn-container/ # VPN Docker container
├── compose/           # Docker Compose files
├── .env               # Environment configuration (gitignored)
├── .env.example       # Example environment configuration
└── README.md          # Main README
```

## Requirements

- Rust (latest stable) - automatically installed by install scripts if not present
- `cargo-watch` (for development mode, installed automatically)
- SSH client
- Tailscale (optional, for Tailscale connections) - can be installed via `hal tailscale install`

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Run tests: `cargo test`
5. Submit a pull request
