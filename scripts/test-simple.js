#!/usr/bin/env node

const { spawn } = require('child_process');

console.log('🧪 Simple MCP Test...\n');

// Start MCP server
const mcpProcess = spawn('cargo', ['run', '--manifest-path', 'backend/Cargo.toml', '--', '--mcp'], {
  stdio: ['pipe', 'pipe', 'inherit'],
  env: { ...process.env, PORT: '8892', DISABLE_WORKTREE_ORPHAN_CLEANUP: '1' }
});

let responseCount = 0;

mcpProcess.stdout.on('data', (data) => {
  const response = data.toString().trim();
  console.log(`📥 Response ${++responseCount}:`, response);
  
  if (responseCount === 1) {
    // Send initialize after first response
    console.log('📤 Sending initialize...');
    mcpProcess.stdin.write(`{"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "test", "version": "1.0.0"}}}\n`);
  } else if (responseCount === 2) {
    // Send list_tools after initialize response
    console.log('📤 Sending list_tools...');
    mcpProcess.stdin.write(`{"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}\n`);
  } else if (responseCount === 3) {
    console.log('✅ Test successful - MCP server responding');
    mcpProcess.kill();
    process.exit(0);
  }
});

mcpProcess.on('exit', (code) => {
  console.log(`\n🔴 MCP server exited with code: ${code}`);
  process.exit(0);
});

// Safety timeout
setTimeout(() => {
  console.log('\n⏰ Test timeout');
  mcpProcess.kill();
  process.exit(1);
}, 10000);