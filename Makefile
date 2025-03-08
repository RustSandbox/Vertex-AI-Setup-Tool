# Makefile for hvertex project

# Detect the operating system
ifeq ($(OS),Windows_NT)
    PLATFORM := windows
    BINARY_NAME := hvertex.exe
    INSTALL_DIR := $(USERPROFILE)/AppData/Local/bin
else
    UNAME_S := $(shell uname -s)
    ifeq ($(UNAME_S),Darwin)
        PLATFORM := macos
    else ifeq ($(UNAME_S),Linux)
        PLATFORM := linux
    endif
    BINARY_NAME := hvertex
    INSTALL_DIR := $(HOME)/.local/bin
endif

# Default target
all: fmt clippy test build

# Format the code
fmt:
	@echo "🔍 Formatting code..."
	cargo fmt

# Run clippy linter
clippy:
	@echo "🔍 Running clippy..."
	cargo clippy -- -D warnings

# Run tests
test:
	@echo "🧪 Running tests..."
	cargo test

# Build the project
build:
	@echo "🏗️ Building project..."
	cargo build --release

# Build for specific platform
build-$(PLATFORM):
	@echo "🏗️ Building for $(PLATFORM)..."
	cargo build --release

# Install the binary to system path
install: build
	@echo "📦 Installing binary to $(INSTALL_DIR)..."
	@mkdir -p $(INSTALL_DIR)
	@cp target/release/$(BINARY_NAME) $(INSTALL_DIR)/
	@chmod +x $(INSTALL_DIR)/$(BINARY_NAME)
	@echo "✅ Installation complete! Binary installed to $(INSTALL_DIR)"

# Clean build artifacts
clean:
	@echo "🧹 Cleaning build artifacts..."
	cargo clean

# Run all checks (fmt, clippy, test)
check: fmt clippy test

# Development setup
dev-setup:
	@echo "🛠️ Setting up development environment..."
	rustup component add rustfmt
	rustup component add clippy

# Show help
help:
	@echo "Available targets:"
	@echo "  all          - Run all checks and build (default)"
	@echo "  fmt          - Format code using rustfmt"
	@echo "  clippy       - Run clippy linter"
	@echo "  test         - Run tests"
	@echo "  build        - Build the project"
	@echo "  build-$(PLATFORM) - Build for current platform"
	@echo "  install      - Install binary to system path"
	@echo "  clean        - Clean build artifacts"
	@echo "  check        - Run all checks (fmt, clippy, test)"
	@echo "  dev-setup    - Set up development environment"
	@echo "  help         - Show this help message"

.PHONY: all fmt clippy test build build-$(PLATFORM) install clean check dev-setup help 