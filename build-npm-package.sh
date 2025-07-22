#!/bin/bash

set -e  # Exit on any error

echo "🧹 Cleaning previous builds..."
rm -rf npx-cli/dist
mkdir -p npx-cli/dist/macos-arm64

echo "🔨 Building frontend..."
(cd frontend && npm run build)

echo "🔨 Building Rust binaries..."
cargo build --release --manifest-path backend/Cargo.toml
cargo build --release --bin mcp_task_server --manifest-path backend/Cargo.toml

echo "📦 Creating distribution package..."

# Copy the main binary
cp target/release/automagik-forge automagik-forge
cp target/release/mcp_task_server automagik-forge-mcp

zip automagik-forge.zip automagik-forge
zip automagik-forge-mcp.zip automagik-forge-mcp

rm automagik-forge automagik-forge-mcp

mv automagik-forge.zip npx-cli/dist/macos-arm64/automagik-forge.zip
mv automagik-forge-mcp.zip npx-cli/dist/macos-arm64/automagik-forge-mcp.zip

echo "✅ NPM package ready!"
echo "📁 Files created:"
echo "   - npx-cli/dist/macos-arm64/automagik-forge.zip"
echo "   - npx-cli/dist/macos-arm64/automagik-forge-mcp.zip"