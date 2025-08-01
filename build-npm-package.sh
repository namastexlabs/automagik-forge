#!/bin/bash

set -e  # Exit on any error

echo "🚀 Starting complete NPM package build process..."

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

echo "🗄️  Preparing database for SQLX compilation..."
npm run prepare-db

echo "✅ Database preparation complete! SQLX errors should now be resolved."

echo "🔍 Running quality checks..."
npm run check

echo "✅ Quality checks passed!"

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
cp target/release/automagik-forge automagik-forge
cp target/release/mcp_task_server mcp_task_server

zip automagik-forge.zip automagik-forge
zip mcp_task_server.zip mcp_task_server

rm automagik-forge mcp_task_server

mv automagik-forge.zip "npx-cli/dist/$PLATFORM_DIR/automagik-forge.zip"
mv mcp_task_server.zip "npx-cli/dist/$PLATFORM_DIR/mcp_task_server.zip"

echo "📦 Creating NPM package..."
cd npx-cli

# Create the NPM package
npm pack

# Get the generated package name
PACKAGE_FILE=$(ls -t automagik-forge-*.tgz | head -n1)

# Move it to the root directory for easy access
mv "$PACKAGE_FILE" "../$PACKAGE_FILE"

cd ..

echo ""
echo "✅ Complete NPM package build successful!"
echo "📁 Files created:"
echo "   - npx-cli/dist/$PLATFORM_DIR/automagik-forge.zip"
echo "   - npx-cli/dist/$PLATFORM_DIR/mcp_task_server.zip"
echo "   - $PACKAGE_FILE (ready for distribution)"
echo ""
echo "🎉 Your NPM package is ready!"
echo "📦 Install locally with: npm install -g ./$PACKAGE_FILE"
echo "🔗 Or publish with: npm publish $PACKAGE_FILE"
echo ""
echo "🚨 IMPORTANT: This script only builds for the current platform ($PLATFORM_DIR)."
echo "🚨           To support all platforms including macOS ARM64, use:"
echo "🚨           ./build-all-platforms.sh (for current platform only)"
echo "🚨           OR run the GitHub Actions pre-release workflow for all platforms."
