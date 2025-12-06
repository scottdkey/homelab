#!/bin/bash

# Cross-platform installation script for hal
# Detects OS and installs Rust if needed, then installs hal

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Get the directory where this script is located
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$SCRIPT_DIR"

echo "Installing hal (Homelab Automation Layer)..."

# Check if Rust is installed
if ! command -v rustc &> /dev/null || ! command -v cargo &> /dev/null; then
    echo -e "${YELLOW}Rust is not installed. Installing Rust...${NC}"
    
    # Install rustup
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    
    # Source cargo env
    export PATH="$HOME/.cargo/bin:$PATH"
    
    echo -e "${GREEN}✓ Rust installed successfully${NC}"
    echo "Please restart your terminal or run: source \$HOME/.cargo/env"
else
    echo -e "${GREEN}✓ Rust is already installed${NC}"
    # Ensure cargo is in PATH
    export PATH="$HOME/.cargo/bin:$PATH"
fi

# Determine installation directory
if [ -w /usr/local/bin ]; then
    INSTALL_DIR="/usr/local/bin"
elif [ -w "$HOME/.local/bin" ]; then
    INSTALL_DIR="$HOME/.local/bin"
    mkdir -p "$INSTALL_DIR"
else
    echo -e "${RED}Error: Cannot write to /usr/local/bin or ~/.local/bin${NC}"
    echo "Please run with sudo or create ~/.local/bin directory"
    exit 1
fi

INSTALL_PATH="$INSTALL_DIR/hal"

# Check if hal already exists
if [ -e "$INSTALL_PATH" ]; then
    echo -e "${YELLOW}hal already exists at $INSTALL_PATH${NC}"
    read -p "Overwrite? (y/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo "Installation cancelled."
        exit 0
    fi
    rm -f "$INSTALL_PATH"
fi

# Build and install hal
echo "Building hal..."
cd "$PROJECT_DIR"
cargo build --release

# Create a wrapper script that sets HOMELAB_DIR
cat > "$INSTALL_PATH" << EOF
#!/bin/bash
# Wrapper script for hal
export HOMELAB_DIR="$PROJECT_DIR"
exec "$PROJECT_DIR/target/release/hal" "\$@"
EOF

chmod +x "$INSTALL_PATH"
echo -e "${GREEN}✓ hal installed to $INSTALL_PATH${NC}"

# Check if install directory is in PATH
if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    echo ""
    echo -e "${YELLOW}Warning: $INSTALL_DIR is not in your PATH${NC}"
    echo "Add this to your ~/.bashrc, ~/.zshrc, or ~/.profile:"
    echo "  export PATH=\"\$PATH:$INSTALL_DIR\""
fi

echo ""
echo -e "${GREEN}Installation complete!${NC}"
echo "Try: hal ssh bellerophon"

