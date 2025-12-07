#!/bin/bash

# Script to detect IPv6 subnet for Docker network configuration
# Run on the Docker host (bellerophon)
# Usage: ./scripts/detect-ipv6-subnet.sh
# Or remotely: ssh bellerophon 'bash -s' < scripts/detect-ipv6-subnet.sh

set -e

echo "=== IPv6 Subnet Detection ==="
echo ""

# Check if Docker daemon has IPv6 enabled
echo "1. Checking Docker daemon IPv6 configuration..."
if docker info 2>/dev/null | grep -q "IPv6: true"; then
    echo "   ✓ IPv6 is enabled in Docker daemon"
    
    # Try to get the fixed CIDR from daemon config
    if [ -f /etc/docker/daemon.json ]; then
        FIXED_CIDR=$(grep -o '"fixed-cidr-v6":\s*"[^"]*"' /etc/docker/daemon.json 2>/dev/null | sed 's/.*"fixed-cidr-v6":\s*"\([^"]*\)".*/\1/' || echo "")
        if [ -n "$FIXED_CIDR" ]; then
            echo "   Found in daemon.json: $FIXED_CIDR"
        fi
    fi
else
    echo "   ✗ IPv6 is not enabled in Docker daemon"
    echo ""
    echo "   To enable IPv6, add to /etc/docker/daemon.json:"
    echo "   {"
    echo "     \"ipv6\": true,"
    echo "     \"fixed-cidr-v6\": \"2001:db8:1::/64\""
    echo "   }"
    echo "   Then restart Docker: sudo systemctl restart docker"
    exit 1
fi
echo ""

# Check existing vpn_network if it exists
echo "2. Checking existing vpn_network configuration..."
if docker network inspect vpn_network >/dev/null 2>&1; then
    echo "   ✓ vpn_network exists"
    
    # Get IPv6 subnet from existing network
    EXISTING_IPV6=$(docker network inspect vpn_network --format '{{range .IPAM.Config}}{{if .Subnet}}{{if eq (index (split .Subnet ":") 0) "2001"}}{{.Subnet}}{{end}}{{end}}{{end}}' 2>/dev/null || echo "")
    
    if [ -n "$EXISTING_IPV6" ]; then
        echo "   Existing IPv6 subnet: $EXISTING_IPV6"
        echo ""
        echo "   Use this subnet in your compose files:"
        echo "   - subnet: $EXISTING_IPV6"
    else
        echo "   No IPv6 subnet configured in existing network"
    fi
else
    echo "   vpn_network does not exist yet"
fi
echo ""

# Check host IPv6 configuration
echo "3. Checking host IPv6 configuration..."
HOST_IPV6=$(ip -6 addr show | grep "inet6" | grep -v "::1" | grep -v "fe80" | head -1 | awk '{print $2}' | cut -d/ -f1 || echo "")
if [ -n "$HOST_IPV6" ]; then
    echo "   Host has IPv6 address: $HOST_IPV6"
    # Extract subnet (assume /64)
    HOST_SUBNET=$(echo "$HOST_IPV6" | sed 's/\([0-9a-f:]*\):.*/\1::\/64/')
    echo "   Possible subnet: $HOST_SUBNET"
else
    echo "   No global IPv6 address found on host"
fi
echo ""

# Check Docker default bridge network IPv6
echo "4. Checking Docker default bridge IPv6..."
BRIDGE_IPV6=$(docker network inspect bridge --format '{{range .IPAM.Config}}{{if .Subnet}}{{if eq (index (split .Subnet ":") 0) "2001"}}{{.Subnet}}{{end}}{{end}}{{end}}' 2>/dev/null || echo "")
if [ -n "$BRIDGE_IPV6" ]; then
    echo "   Default bridge IPv6 subnet: $BRIDGE_IPV6"
    echo "   You can use a different /64 from the same /48 range"
fi
echo ""

# Recommendations
echo "=== Recommendations ==="
echo ""

if [ -n "$FIXED_CIDR" ]; then
    echo "✓ Use the subnet from Docker daemon configuration:"
    echo "  - subnet: $FIXED_CIDR"
elif [ -n "$EXISTING_IPV6" ]; then
    echo "✓ Use the existing network subnet:"
    echo "  - subnet: $EXISTING_IPV6"
elif [ -n "$HOST_SUBNET" ]; then
    echo "⚠ Use a unique local address (ULA) or subnet from your host:"
    echo "  - subnet: fd00::/64  (ULA - recommended if no global IPv6)"
    echo "  - subnet: $HOST_SUBNET  (if you have global IPv6)"
else
    echo "⚠ No IPv6 detected. Use a unique local address (ULA):"
    echo "  - subnet: fd00::/64"
    echo ""
    echo "  Or enable IPv6 in Docker daemon first (see step 1)"
fi
echo ""

echo "=== Quick Setup ==="
echo ""
echo "If IPv6 is not enabled, run on bellerophon:"
echo "  sudo mkdir -p /etc/docker"
echo "  echo '{\"ipv6\": true, \"fixed-cidr-v6\": \"fd00::/64\"}' | sudo tee /etc/docker/daemon.json"
echo "  sudo systemctl restart docker"
echo ""
echo "Then update compose files to use: fd00::/64"
