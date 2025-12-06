.PHONY: install dev build clean setup-dev

# Install hal globally
install:
	cargo install --path . --force

# Setup development dependencies
setup-dev:
	@echo "Installing cargo-watch for development..."
	cargo install cargo-watch --quiet

# Development mode: watch for changes and auto-install
dev: setup-dev
	@echo "Starting development mode..."
	@echo "Watching for changes and auto-installing..."
	@echo "Press Ctrl+C to stop"
	cargo watch -x 'install --path . --force'

# Build the binary
build:
	cargo build --release

# Clean build artifacts
clean:
	cargo clean

