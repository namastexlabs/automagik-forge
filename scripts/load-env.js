#!/usr/bin/env node

const fs = require('fs');
const path = require('path');

/**
 * Load environment variables from .env files
 * Priority: .env.local > .env.production/.env.development > .env
 */
function loadEnv() {
  const envFiles = [
    '.env',
    process.env.NODE_ENV === 'production' ? '.env.production' : '.env.development',
    '.env.local'
  ].filter(Boolean);

  for (const envFile of envFiles) {
    const envPath = path.resolve(process.cwd(), envFile);
    if (fs.existsSync(envPath)) {
      const envContent = fs.readFileSync(envPath, 'utf8');
      
      envContent.split('\n').forEach(line => {
        const trimmed = line.trim();
        if (trimmed && !trimmed.startsWith('#')) {
          const [key, ...valueParts] = trimmed.split('=');
          if (key && valueParts.length > 0) {
            const value = valueParts.join('=').replace(/^["']|["']$/g, '');
            if (!process.env[key.trim()]) {
              process.env[key.trim()] = value;
            }
          }
        }
      });
    }
  }
}

// Auto-load if called directly
if (require.main === module) {
  loadEnv();
}

module.exports = { loadEnv };