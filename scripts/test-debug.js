#!/usr/bin/env node

const { spawn } = require('child_process');

console.log('🧪 Debug MCP Server...\n');

const mcpProcess = spawn('cargo', ['run', '--manifest-path', 'backend/Cargo.toml', '--', '--mcp'], {
  stdio: ['pipe', 'pipe', 'pipe'],
  env: { ...process.env, PORT: '8894', DISABLE_WORKTREE_ORPHAN_CLEANUP: '1' }
});

console.log('📤 Sending initialize...');
mcpProcess.stdin.write('{"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "test", "version": "1.0.0"}}}\n');

mcpProcess.stdout.on('data', (data) => {
  console.log('📥 STDOUT:', data.toString());
});

mcpProcess.stderr.on('data', (data) => {
  console.log('📥 STDERR:', data.toString());
});

mcpProcess.on('exit', (code) => {
  console.log(`\n💥 Exit code: ${code}`);
  process.exit(0);
});

// Timeout after 5 seconds
setTimeout(() => {
  console.log('\n⏰ Timeout - killing process');
  mcpProcess.kill();
}, 5000);