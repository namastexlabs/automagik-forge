#!/bin/bash

set -e  # Exit on any error

# Detect platform and architecture
PLATFORM=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

# Map to platform directory names used by the CLI
case "$PLATFORM" in
    linux)
        case "$ARCH" in
            x86_64) PLATFORM_DIR="linux-x64" ;;
            aarch64) PLATFORM_DIR="linux-arm64" ;;
            *) echo "❌ Unsupported Linux architecture: $ARCH"; exit 1 ;;
        esac
        ;;
    darwin)
        case "$ARCH" in
            x86_64) PLATFORM_DIR="macos-x64" ;;
            arm64) PLATFORM_DIR="macos-arm64" ;;
            *) echo "❌ Unsupported macOS architecture: $ARCH"; exit 1 ;;
        esac
        ;;
    *)
        echo "❌ Unsupported platform: $PLATFORM"
        exit 1
        ;;
esac

echo "🔍 Detected platform: $PLATFORM_DIR"

echo "🧹 Cleaning previous builds..."
rm -rf npx-cli/dist
mkdir -p "npx-cli/dist/$PLATFORM_DIR"

echo "🔨 Building frontend..."
(cd frontend && npm run build)

echo "🔨 Building Rust binaries..."
cargo build --release --manifest-path backend/Cargo.toml
cargo build --release --bin mcp_task_server --manifest-path backend/Cargo.toml

echo "📦 Creating distribution package..."

# Copy the main binary
cp target/release/vibe-kanban vibe-kanban
cp target/release/mcp_task_server vibe-kanban-mcp

zip vibe-kanban.zip vibe-kanban
zip vibe-kanban-mcp.zip vibe-kanban-mcp

rm vibe-kanban vibe-kanban-mcp

mv vibe-kanban.zip "npx-cli/dist/$PLATFORM_DIR/vibe-kanban.zip"
mv vibe-kanban-mcp.zip "npx-cli/dist/$PLATFORM_DIR/vibe-kanban-mcp.zip"

echo "✅ NPM package ready!"
echo "📁 Files created:"
echo "   - npx-cli/dist/$PLATFORM_DIR/vibe-kanban.zip"
echo "   - npx-cli/dist/$PLATFORM_DIR/vibe-kanban-mcp.zip"