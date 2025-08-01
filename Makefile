# Automagik Forge - Build and Publishing Automation
# Usage:
#   make bump VERSION=0.2.0    # Bump version across all files
#   make build                 # Build the project
#   make publish               # Build and publish to NPM

.PHONY: help bump release build publish clean version

# Default target
help:
	@echo "Automagik Forge Build Automation"
	@echo ""
	@echo "Available targets:"
	@echo "  bump        - Create alpha pre-release (0.2.16 → 0.2.16-alpha.0)"
	@echo "  release     - Convert pre-release to stable (0.2.16-alpha.0 → 0.2.17)"
	@echo "  build       - Build frontend and Rust binaries"
	@echo "  publish     - Publish to NPM (auto-detects stable/prerelease)"
	@echo "  clean       - Clean build artifacts"
	@echo "  version     - Show current versions"
	@echo ""
	@echo "Workflow:"
	@echo "  1. make bump     # Create pre-release"
	@echo "  2. make publish  # Test with beta users"
	@echo "  3. make release  # Promote to stable"
	@echo "  4. make publish  # Release to all users"

# Bump to pre-release version
bump:
	@echo "🔄 Creating pre-release version..."
	@CURRENT_VERSION=$$(node -p "require('./package.json').version"); \
	if echo "$$CURRENT_VERSION" | grep -q '\-alpha\.' ; then \
		node scripts/bump-version.js prerelease; \
	else \
		BASE_VERSION=$$(echo "$$CURRENT_VERSION" | sed 's/\.[0-9]*$$//'); \
		if echo "$$CURRENT_VERSION" | grep -qE '\..*\..*\..*\.' ; then \
			BASE_VERSION=$$(echo "$$CURRENT_VERSION" | sed 's/\.[0-9]*$$//'); \
		else \
			BASE_VERSION="$$CURRENT_VERSION"; \
		fi; \
		NEW_VERSION="$$BASE_VERSION-alpha.0"; \
		sed -i "s/\"version\": \"[^\"]*\"/\"version\": \"$$NEW_VERSION\"/" package.json; \
		sed -i "s/\"version\": \"[^\"]*\"/\"version\": \"$$NEW_VERSION\"/" frontend/package.json; \
		sed -i "s/\"version\": \"[^\"]*\"/\"version\": \"$$NEW_VERSION\"/" npx-cli/package.json; \
		sed -i "0,/version = \"[^\"]*\"/s//version = \"$$NEW_VERSION\"/" backend/Cargo.toml; \
		echo "✅ Version bumped to $$NEW_VERSION"; \
	fi

# Convert pre-release to stable release
release:
	@echo "🚀 Converting pre-release to stable version..."
	@CURRENT_VERSION=$$(node -p "require('./package.json').version"); \
	if ! echo "$$CURRENT_VERSION" | grep -qE '\-|\..*\..*\..*\.' ; then \
		echo "❌ Error: Current version ($$CURRENT_VERSION) is already stable."; \
		echo "   Nothing to release."; \
		exit 1; \
	fi; \
	if echo "$$CURRENT_VERSION" | grep -qE '\..*\..*\..*\.' ; then \
		BASE_VERSION=$$(echo "$$CURRENT_VERSION" | sed 's/\.[0-9]*$$//'); \
		PATCH=$$(echo "$$BASE_VERSION" | sed 's/.*\.//'); \
		MINOR=$$(echo "$$BASE_VERSION" | sed 's/\.[^.]*$$//' | sed 's/.*\.//'); \
		MAJOR=$$(echo "$$BASE_VERSION" | sed 's/\..*//'); \
		NEW_PATCH=$$(($$PATCH + 1)); \
		NEW_VERSION="$$MAJOR.$$MINOR.$$NEW_PATCH"; \
	else \
		NEW_VERSION=$$(echo "$$CURRENT_VERSION" | sed 's/-.*//'); \
	fi; \
	echo "📝 Updating version from $$CURRENT_VERSION to $$NEW_VERSION"; \
	sed -i "s/\"version\": \"[^\"]*\"/\"version\": \"$$NEW_VERSION\"/" package.json; \
	sed -i "s/\"version\": \"[^\"]*\"/\"version\": \"$$NEW_VERSION\"/" frontend/package.json; \
	sed -i "s/\"version\": \"[^\"]*\"/\"version\": \"$$NEW_VERSION\"/" npx-cli/package.json; \
	sed -i "0,/version = \"[^\"]*\"/s//version = \"$$NEW_VERSION\"/" backend/Cargo.toml; \
	echo "✅ Version updated to $$NEW_VERSION"


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

# Build and publish to NPM (auto-detects stable vs prerelease)
publish: build
	@echo "📦 Publishing to NPM..."
	@CURRENT_VERSION=$$(node -p "require('./package.json').version"); \
	if echo "$$CURRENT_VERSION" | grep -q '\-' ; then \
		echo "📦 Detected PRE-RELEASE version: $$CURRENT_VERSION"; \
		if echo "$$CURRENT_VERSION" | grep -q '\-alpha\.' ; then \
			TAG="alpha"; \
		elif echo "$$CURRENT_VERSION" | grep -q '\-beta\.' ; then \
			TAG="beta"; \
		else \
			TAG="next"; \
		fi; \
		cd npx-cli && npm publish --tag $$TAG; \
		echo "🎉 Successfully published PRE-RELEASE to npm!"; \
		echo "📋 Users can test with: npx automagik-forge@$$TAG"; \
		echo "📋 Or specific version: npx automagik-forge@$$CURRENT_VERSION"; \
	else \
		echo "📦 Detected STABLE version: $$CURRENT_VERSION"; \
		cd npx-cli && npm publish; \
		echo "🎉 Successfully published STABLE version to npm!"; \
		echo "📋 Users can now install with: npx automagik-forge"; \
	fi

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