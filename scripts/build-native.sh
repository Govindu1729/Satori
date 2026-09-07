#!/bin/bash
# Build script for Zen Agentic Extension Native Host
# Usage: ./scripts/build-native.sh [--release]

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
NATIVE_HOST_DIR="$PROJECT_DIR/native-host"

echo "=== Zen Agentic Native Host Build ==="
echo "Project directory: $PROJECT_DIR"

# Check if Rust is installed
if ! command -v cargo &> /dev/null; then
    echo "Error: Rust/Cargo not found. Please install Rust from https://rustup.rs/"
    exit 1
fi

echo "Rust version: $(rustc --version)"
cd "$NATIVE_HOST_DIR"

# Build based on flag
if [[ "$1" == "--release" ]]; then
    echo "Building release version..."
    cargo build --release
    BINARY_PATH="$NATIVE_HOST_DIR/target/release/zen-agentic-native"
else
    echo "Building debug version..."
    cargo build
    BINARY_PATH="$NATIVE_HOST_DIR/target/debug/zen-agentic-native"
fi

if [[ -f "$BINARY_PATH" ]]; then
    echo ""
    echo "✓ Build successful!"
    echo "Binary location: $BINARY_PATH"
    echo ""
    echo "To install, create native messaging manifest at:"
    echo "  macOS: ~/Library/Application Support/Zen/NativeMessagingHosts/zen_agentic_native.json"
    echo "  Linux: ~/.mozilla/native-messaging-hosts/zen_agentic_native.json"
    echo ""
    echo "Manifest content:"
    echo "{"
    echo "  \"name\": \"zen_agentic_native\","
    echo "  \"description\": \"Zen Agentic AI Native Host\","
    echo "  \"path\": \"$BINARY_PATH\","
    echo "  \"type\": \"stdio\","
    echo "  \"allowed_extensions\": [\"zen-agentic-ai@localhost\"]"
    echo "}"
else
    echo "Error: Build failed - binary not found"
    exit 1
fi
