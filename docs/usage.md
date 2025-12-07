# Usage Guide

## SSH to a Host

After setup, simply use standard SSH:

```bash
ssh maple
ssh bellerophon
```

With additional SSH arguments:

```bash
ssh maple -L 8080:localhost:8080
```

## Setup SMB Mounts

Setup and mount SMB shares on a remote host:

```bash
hal smb bellerophon
```

This will:
- Install SMB client utilities (`cifs-utils`)
- Create mount points at `/mnt/smb/{servername}/{sharename}`
- Mount SMB shares using credentials from `.env`
- Add entries to `/etc/fstab` for persistent mounts

**Uninstall SMB mounts:**
```bash
hal smb bellerophon --uninstall
```

## Backup and Restore Docker Volumes

**Create a backup:**
```bash
hal backup bellerophon create
```

This creates a timestamped backup of all Docker volumes and bind mounts in `/mnt/smb/maple/backups/{hostname}/{timestamp}/`.

**List available backups:**
```bash
hal backup bellerophon list
```

**Restore from a backup:**
```bash
hal backup bellerophon restore
```

If no backup name is specified, it will list available backups and prompt you to select one.

Or restore a specific backup:
```bash
hal backup bellerophon restore --backup 20240101_120000
```

## Automatically Setup Nginx Proxy Manager Hosts

Automatically create proxy hosts in Nginx Proxy Manager from a Docker Compose file:

```bash
hal npm bellerophon media.docker-compose.yml
```

This will:
- Parse the compose file to find services with exposed ports
- Connect to Nginx Proxy Manager API (requires `NPM_USERNAME` and `NPM_PASSWORD` in `.env`)
- Create proxy hosts for each service (e.g., `sonarr.local`, `radarr.local`)
- Forward traffic to the host where services are running

The command will:
- Skip services that already have proxy hosts configured
- Use the host's IP or Tailscale address for forwarding
- Create domains in the format `{servicename}.local`

## VPN Deployment

Build and deploy VPN containers:

```bash
hal vpn build
hal vpn deploy
```

See the [VPN documentation](vpn.md) for more details.

## Update Channels

HAL supports multiple update channels: stable, alpha, and beta.

### Stable Channel (Default)

The stable channel receives production releases. This is the default behavior:

```bash
hal update
```

### Alpha Channel

The alpha channel receives pre-release builds from the `alpha` branch. These are continuously updated and may be unstable:

```bash
hal update --alpha
```

When you first use `--alpha`, you'll be prompted to confirm switching to the alpha channel. Alpha releases are versionless (continuously updated) and allow you to test the latest features before they reach beta or stable.

**Note:** Alpha releases may contain bugs and breaking changes. Use at your own risk.

### Beta Channel

The beta channel receives pre-release builds from the `beta` branch. These are more stable than alpha but still pre-release:

```bash
hal update --beta
```

When you first use `--beta`, you'll be prompted to confirm switching to the beta channel. Beta releases are versionless (continuously updated) and provide early access to upcoming stable releases.

**Note:** Beta releases may contain bugs. Use with caution in production environments.

### Switching Channels

- Use `hal update` (without flags) to check for stable releases
- Use `hal update --alpha` to check for alpha releases
- Use `hal update --beta` to check for beta releases

The update command will automatically download and install the latest version from the selected channel.
