#!/usr/bin/env node

const fs = require('fs');
const path = require('path');

// Parse arguments
const bumpType = process.argv[2]; // patch, minor, major, prerelease

if (!bumpType || !['patch', 'minor', 'major', 'prerelease'].includes(bumpType)) {
  console.error('❌ Usage: node scripts/bump-version.js <patch|minor|major|prerelease>');
  process.exit(1);
}

// Semantic version bumping function with pre-release support
function bumpVersion(version, type) {
  // Handle pre-release versions (e.g., 0.2.7.1)
  const parts = version.split('.');
  
  if (parts.length === 4) {
    // Current version is a pre-release (0.2.7.1)
    const [major, minor, patch, prerelease] = parts.map(Number);
    
    switch (type) {
      case 'major':
        return `${major + 1}.0.0`;
      case 'minor':
        return `${major}.${minor + 1}.0`;
      case 'patch':
        return `${major}.${minor}.${patch + 1}`;
      case 'prerelease':
        return `${major}.${minor}.${patch}.${prerelease + 1}`;
      default:
        throw new Error(`Invalid bump type: ${type}`);
    }
  } else if (parts.length === 3) {
    // Current version is stable (0.2.7)
    const [major, minor, patch] = parts.map(Number);
    
    switch (type) {
      case 'major':
        return `${major + 1}.0.0`;
      case 'minor':
        return `${major}.${minor + 1}.0`;
      case 'patch':
        return `${major}.${minor}.${patch + 1}`;
      case 'prerelease':
        return `${major}.${minor}.${patch}.1`;
      default:
        throw new Error(`Invalid bump type: ${type}`);
    }
  } else {
    throw new Error(`Invalid version format: ${version}`);
  }
}

// Get current version from root package.json
const rootPackagePath = path.join(__dirname, '..', 'package.json');
const rootPackage = JSON.parse(fs.readFileSync(rootPackagePath, 'utf8'));
const currentVersion = rootPackage.version;
const newVersion = bumpVersion(currentVersion, bumpType);

console.log(`🔄 Bumping ${bumpType} version: ${currentVersion} → ${newVersion}`);

// Files to update
const filesToUpdate = [
  {
    path: path.join(__dirname, '..', 'package.json'),
    type: 'json',
    key: 'version'
  },
  {
    path: path.join(__dirname, '..', 'frontend', 'package.json'),
    type: 'json',
    key: 'version'
  },
  {
    path: path.join(__dirname, '..', 'npx-cli', 'package.json'),
    type: 'json',
    key: 'version'
  },
  {
    path: path.join(__dirname, '..', 'backend', 'Cargo.toml'),
    type: 'toml',
    key: 'version'
  }
];

// Update each file
for (const file of filesToUpdate) {
  try {
    if (file.type === 'json') {
      const content = JSON.parse(fs.readFileSync(file.path, 'utf8'));
      content[file.key] = newVersion;
      fs.writeFileSync(file.path, JSON.stringify(content, null, 2) + '\n');
      console.log(`✅ Updated ${path.relative(process.cwd(), file.path)}`);
    } else if (file.type === 'toml') {
      let content = fs.readFileSync(file.path, 'utf8');
      // Update only the first version line (under [package])
      content = content.replace(/^version = "[^"]*"/m, `version = "${newVersion}"`);
      fs.writeFileSync(file.path, content);
      console.log(`✅ Updated ${path.relative(process.cwd(), file.path)}`);
    }
  } catch (error) {
    console.error(`❌ Failed to update ${file.path}:`, error.message);
    process.exit(1);
  }
}

console.log(`🎉 Successfully bumped all versions to ${newVersion}!`);