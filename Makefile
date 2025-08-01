# Automagik Forge - Build and Publishing Automation
# Usage:
#   make bump VERSION=0.2.0    # Bump version across all files
#   make build                 # Build the project
#   make publish               # Build and publish to NPM

.PHONY: help bump bump-patch bump-minor bump-major bump-prerelease bump-manual build publish publish-stable publish-prerelease clean check-version version

# Default target
help:
	@echo "Automagik Forge Build Automation"
	@echo ""
	@echo "Available targets:"
	@echo "  bump (bump-patch)   - Bump patch version automatically (e.g., 0.2.6 → 0.2.7)"
	@echo "  bump-minor          - Bump minor version automatically (e.g., 0.2.6 → 0.3.0)"
	@echo "  bump-major          - Bump major version automatically (e.g., 0.2.6 → 1.0.0)"
	@echo "  bump-prerelease     - Bump pre-release version (e.g., 0.2.7 → 0.2.7.1, 0.2.7.1 → 0.2.7.2)"
	@echo "  bump VERSION=x.y.z  - Bump to specific version manually"
	@echo "  build               - Build frontend and Rust binaries"
	@echo "  publish             - Build and publish stable release to NPM"
	@echo "  publish-prerelease  - Build and publish pre-release to NPM"
	@echo "  clean               - Clean build artifacts"
	@echo "  version             - Show current versions across all files"
	@echo "  help                - Show this help message"
	@echo ""
	@echo "Examples:"
	@echo "  make bump-prerelease # Create pre-release (for bug fixes)"
	@echo "  make publish-prerelease # Publish pre-release to test"
	@echo "  make bump           # Stable patch version (when bugs confirmed fixed)"
	@echo "  make publish        # Publish stable version to NPM"

# Check if VERSION is provided for bump target
check-version:
	@if [ -z "$(VERSION)" ]; then \
		echo "❌ Error: VERSION is required. Usage: make bump VERSION=x.y.z"; \
		exit 1; \
	fi
	@echo "🔄 Bumping version to $(VERSION)"

# Default bump is patch version (backward compatibility)
bump:
	@if [ -n "$(VERSION)" ]; then \
		$(MAKE) bump-manual VERSION=$(VERSION); \
	else \
		$(MAKE) bump-patch; \
	fi

# Automatic semantic version bumps
bump-patch:
	@echo "🔄 Auto-bumping patch version..."
	@node scripts/bump-version.js patch

bump-minor:
	@echo "🔄 Auto-bumping minor version..."
	@node scripts/bump-version.js minor

bump-major:
	@echo "🔄 Auto-bumping major version..."
	@node scripts/bump-version.js major

bump-prerelease:
	@echo "🔄 Auto-bumping pre-release version..."
	@node scripts/bump-version.js prerelease

# Manual version bump (legacy support)
bump-manual: check-version
	@echo "📝 Updating version in all package files..."
	@# Update root package.json
	@sed -i 's/"version": "[^"]*"/"version": "$(VERSION)"/' package.json
	@# Update frontend package.json
	@sed -i 's/"version": "[^"]*"/"version": "$(VERSION)"/' frontend/package.json
	@# Update npx-cli package.json
	@sed -i 's/"version": "[^"]*"/"version": "$(VERSION)"/' npx-cli/package.json
	@# Update backend Cargo.toml (only the first version under [package])
	@sed -i '0,/version = "[^"]*"/s//version = "$(VERSION)"/' backend/Cargo.toml
	@echo "✅ Version bumped to $(VERSION) across all files"
	@echo "📋 Updated files:"
	@echo "   - package.json"
	@echo "   - frontend/package.json"
	@echo "   - npx-cli/package.json"
	@echo "   - backend/Cargo.toml"

# Build the project
build:
	@echo "🚀 Building Automagik Forge..."
	@echo "🧹 Cleaning previous builds..."
	@rm -rf npx-cli/dist
	@echo "🔨 Building frontend..."
	@cd frontend && npm run build
	@echo "🔨 Building Rust binaries..."
	@cargo build --release --manifest-path backend/Cargo.toml
	@cargo build --release --bin mcp_task_server --manifest-path backend/Cargo.toml
	@echo "📦 Creating distribution package..."
	@./build-npm-package.sh
	@echo "✅ Build complete!"

# Clean build artifacts
clean:
	@echo "🧹 Cleaning build artifacts..."
	@rm -rf target/
	@rm -rf frontend/dist/
	@rm -rf npx-cli/dist/
	@rm -f automagik-forge automagik-forge-mcp
	@rm -f *.zip
	@echo "✅ Clean complete!"

# Build and publish stable release to NPM
publish: publish-stable

# Publish stable version (no pre-release tag)
publish-stable: build
	@echo "📦 Publishing STABLE release to NPM..."
	@CURRENT_VERSION=$$(node -p "require('./package.json').version"); \
	if [[ $$CURRENT_VERSION =~ \.[0-9]+$$ ]]; then \
		echo "❌ Error: Current version ($$CURRENT_VERSION) appears to be a pre-release. Use 'make bump' to create a stable version first."; \
		exit 1; \
	fi
	@cd npx-cli && npm publish
	@echo "🎉 Successfully published STABLE version to npm!"
	@echo "📋 Users can now install with: npx automagik-forge"

# Publish pre-release version (with beta tag)
publish-prerelease: build
	@echo "📦 Publishing PRE-RELEASE to NPM with beta tag..."
	@CURRENT_VERSION=$$(node -p "require('./package.json').version"); \
	if [[ ! $$CURRENT_VERSION =~ \.[0-9]+$$ ]]; then \
		echo "❌ Error: Current version ($$CURRENT_VERSION) is not a pre-release. Use 'make bump-prerelease' first."; \
		exit 1; \
	fi
	@cd npx-cli && npm publish --tag beta
	@echo "🎉 Successfully published PRE-RELEASE version to npm!"
	@echo "📋 Users can test with: npx automagik-forge@beta"
	@echo "📋 Or specific version: npx automagik-forge@$$(node -p "require('./package.json').version")"

# Development helpers
dev:
	@echo "🚀 Starting development environment..."
	@npm run dev

test:
	@echo "🧪 Running tests..."
	@npm run check

# Version info
version:
	@echo "Current versions:"
	@echo "  Root:     $(shell grep '"version"' package.json | head -1 | sed 's/.*"version": "\([^"]*\)".*/\1/')"
	@echo "  Frontend: $(shell grep '"version"' frontend/package.json | head -1 | sed 's/.*"version": "\([^"]*\)".*/\1/')"
	@echo "  NPX CLI:  $(shell grep '"version"' npx-cli/package.json | head -1 | sed 's/.*"version": "\([^"]*\)".*/\1/')"
	@echo "  Backend:  $(shell grep 'version =' backend/Cargo.toml | head -1 | sed 's/.*version = "\([^"]*\)".*/\1/')"