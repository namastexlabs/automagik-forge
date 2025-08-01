#!/usr/bin/env node

const { execSync, spawn } = require('child_process');
const fs = require('fs');
const path = require('path');

console.log('🚀 Preparing database for SQLx...');

// Get project root and backend directory
const projectRoot = path.join(__dirname, '..');
const backendDir = path.join(projectRoot, 'backend');

console.log(`📁 Project root: ${projectRoot}`);
console.log(`📁 Backend directory: ${backendDir}`);

// Ensure backend directory exists
if (!fs.existsSync(backendDir)) {
  console.error(`❌ Backend directory does not exist: ${backendDir}`);
  process.exit(1);
}

// Change to backend directory for all operations
process.chdir(backendDir);
console.log(`📂 Working directory: ${process.cwd()}`);

// Create temporary database file path
const dbFile = path.join(backendDir, 'prepare_db.sqlite');

// Remove existing files
if (fs.existsSync(dbFile)) {
  console.log('🧹 Removing existing temporary database...');
  fs.unlinkSync(dbFile);
}

// Remove existing .env to avoid conflicts
const envFile = path.join(backendDir, '.env');
if (fs.existsSync(envFile)) {
  console.log('🧹 Removing existing .env file...');
  fs.unlinkSync(envFile);
}

try {
  // Get absolute path (cross-platform) 
  const dbPath = path.resolve(dbFile);
  // Use SQLite URL format - try multiple formats for compatibility
  const databaseUrl = `sqlite:${dbPath}`;
  const altDatabaseUrl = `sqlite://${dbPath}`;
  const fileDatabaseUrl = `sqlite:///${dbPath}`;
  
  console.log(`🗄️  Database file path: ${dbPath}`);
  console.log(`🔗 Database URL: ${databaseUrl}`);
  
  // Create empty database file to ensure it exists
  console.log('📁 Creating database file...');
  fs.writeFileSync(dbFile, '');
  
  // Verify file was created and is accessible
  try {
    fs.accessSync(dbFile, fs.constants.R_OK | fs.constants.W_OK);
    console.log('✅ Database file created and accessible');
  } catch (accessErr) {
    console.error('❌ Cannot access database file:', accessErr.message);
    process.exit(1);
  }
  
  // Check if sqlx CLI is available
  console.log('🔍 Checking SQLX CLI availability...');
  try {
    const version = execSync('cargo sqlx --version', { stdio: 'pipe', encoding: 'utf8' });
    console.log(`✅ SQLX CLI available: ${version.trim()}`);
  } catch (err) {
    console.log('📦 Installing SQLX CLI...');
    execSync('cargo install sqlx-cli --no-default-features --features sqlite', { stdio: 'inherit' });
    console.log('✅ SQLX CLI installed successfully');
  }
  
  console.log('🔧 Running database migrations...');
  
  // Try different database URL formats for compatibility
  const urlsToTry = [databaseUrl, altDatabaseUrl, fileDatabaseUrl];
  let migrationSuccess = false;
  let workingUrl = null;
  
  for (const testUrl of urlsToTry) {
    console.log(`🔄 Trying database URL: ${testUrl}`);
    
    try {
      const sqlxEnv = { 
        ...process.env, 
        DATABASE_URL: testUrl,
        SQLX_OFFLINE: 'false'  // Force online mode for preparation
      };
      
      execSync('cargo sqlx migrate run', {
        stdio: 'pipe', // Use pipe first to avoid spam on failure
        env: sqlxEnv,
        cwd: backendDir
      });
      
      console.log(`✅ Migrations successful with URL: ${testUrl}`);
      migrationSuccess = true;
      workingUrl = testUrl;
      break;
      
    } catch (migrationErr) {
      console.log(`❌ Failed with URL: ${testUrl}`);
      if (testUrl === urlsToTry[urlsToTry.length - 1]) {
        // This is the last attempt, show the error
        console.error('Migration error:', migrationErr.message);
      }
    }
  }
  
  if (!migrationSuccess) {
    console.error('❌ All database URL formats failed');
    process.exit(1);
  }
  
  // Verify database was created and has content
  if (fs.existsSync(dbFile)) {
    const stats = fs.statSync(dbFile);
    console.log(`✅ Database file created: ${stats.size} bytes`);
    
    if (stats.size === 0) {
      console.error('❌ Database file is empty - migrations may have failed');
      process.exit(1);
    }
  } else {
    console.error('❌ Database file was not created');
    process.exit(1);
  }
  
  console.log('🔧 Generating SQLX query cache...');
  
  // Use the working URL for query preparation
  const finalSqlxEnv = { 
    ...process.env, 
    DATABASE_URL: workingUrl,
    SQLX_OFFLINE: 'false'  // Force online mode for preparation
  };
  
  execSync('cargo sqlx prepare --workspace', {
    stdio: 'inherit', 
    env: finalSqlxEnv,
    cwd: backendDir
  });
  
  // Verify sqlx-data.json was created and has content
  const sqlxDataFile = path.join(backendDir, 'sqlx-data.json');
  if (fs.existsSync(sqlxDataFile)) {
    const stats = fs.statSync(sqlxDataFile);
    console.log(`✅ SQLX query cache created: ${stats.size} bytes`);
    
    if (stats.size < 100) {
      console.warn('⚠️  Query cache file seems unusually small');
    }
    
    // Validate the JSON structure
    try {
      const data = JSON.parse(fs.readFileSync(sqlxDataFile, 'utf8'));
      console.log(`📊 Query cache contains ${data.queries ? data.queries.length : 0} queries`);
    } catch (parseErr) {
      console.error('❌ Invalid JSON in query cache file');
      process.exit(1);
    }
  } else {
    console.error('❌ SQLX query cache was not created');
    process.exit(1);
  }
  
  console.log('🎉 Database preparation completed successfully!');
  console.log('✅ SQLX compilation errors should now be resolved');
  
} catch (error) {
  console.error('❌ Database preparation failed:', error.message);
  
  // More detailed error information
  if (error.status) console.error('Exit code:', error.status);
  if (error.signal) console.error('Signal:', error.signal);
  if (error.stdout) console.error('STDOUT:', error.stdout.toString());
  if (error.stderr) console.error('STDERR:', error.stderr.toString());
  
  process.exit(1);
} finally {
  // Clean up temporary files
  if (fs.existsSync(dbFile)) {
    console.log('🧹 Cleaning up temporary database file...');
    try {
      fs.unlinkSync(dbFile);
      console.log('✅ Cleanup completed');
    } catch (cleanupErr) {
      console.warn('⚠️  Could not remove temporary database file:', cleanupErr.message);
    }
  }
  
  // Remove .env file if we created it
  const envFile = path.join(backendDir, '.env');
  if (fs.existsSync(envFile)) {
    try {
      const envContent = fs.readFileSync(envFile, 'utf8');
      if (envContent.includes('# DATABASE_URL is not set to force SQLX offline mode')) {
        fs.unlinkSync(envFile);
        console.log('🧹 Removed temporary .env file');
      }
    } catch (envErr) {
      console.warn('⚠️  Could not check/remove .env file:', envErr.message);
    }
  }
}