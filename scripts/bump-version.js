#!/usr/bin/env node

const fs = require('fs');
const path = require('path');

// Parse arguments
const bumpType = process.argv[2]; // patch, minor, major, prerelease

if (!bumpType || !['patch', 'minor', 'major', 'prerelease'].includes(bumpType)) {
  console.error('❌ Usage: node scripts/bump-version.js <patch|minor|major|prerelease>');
  process.exit(1);
}

// Standard semantic version bumping with beta pre-releases
function bumpVersion(version, type) {
  // Parse version with potential pre-release identifier (e.g., 0.2.8-beta.1)
  const match = version.match(/^(\d+)\.(\d+)\.(\d+)(?:-(.+))?$/);
  if (!match) {
    throw new Error(`Invalid version format: ${version}`);
  }
  
  const [, major, minor, patch, prerelease] = match;
  const majorNum = parseInt(major);
  const minorNum = parseInt(minor);
  const patchNum = parseInt(patch);
  
  switch (type) {
    case 'prerelease':
      if (prerelease) {
        // Already a pre-release, increment the pre-release number
        const betaMatch = prerelease.match(/^beta\.(\d+)$/);
        if (betaMatch) {
          const betaNum = parseInt(betaMatch[1]);
          return `${majorNum}.${minorNum}.${patchNum}-beta.${betaNum + 1}`;
        } else {
          // If not beta format, start with beta.1
          return `${majorNum}.${minorNum}.${patchNum}-beta.1`;
        }
      } else {
        // Stable version, create first pre-release
        return `${majorNum}.${minorNum}.${patchNum}-beta.1`;
      }
    case 'patch':
      return `${majorNum}.${minorNum}.${patchNum + 1}`;
    case 'minor':
      return `${majorNum}.${minorNum + 1}.0`;
    case 'major':
      return `${majorNum + 1}.0.0`;
    default:
      throw new Error(`Unknown bump type: ${type}`);
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