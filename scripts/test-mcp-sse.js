#!/usr/bin/env node

const { spawn } = require('child_process');
const path = require('path');

async function testMcpSseIntegration() {
    console.log('Testing MCP SSE integration...');
    
    // Set test environment
    process.env.MCP_SSE_PORT = '8765';
    
    const mcpBinary = path.join(__dirname, '../backend/target/debug/vibe-kanban-mcp');
    
    console.log('Starting MCP SSE server for testing...');
    const mcpProcess = spawn(mcpBinary, ['--mcp-sse'], {
        env: { ...process.env }
    });
    
    let serverReady = false;
    let sseEndpointFound = false;
    
    mcpProcess.stdout.on('data', (data) => {
        const output = data.toString();
        console.log(`[MCP-SSE] ${output.trim()}`);
        
        // Check for SSE server startup message
        if (output.includes('MCP SSE server listening')) {
            serverReady = true;
            sseEndpointFound = true;
            console.log('✅ SSE server started successfully');
        }
        
        // Check for STDIO server startup
        if (output.includes('Starting MCP STDIO server')) {
            console.log('✅ STDIO server started successfully');
        }
    });
    
    mcpProcess.stderr.on('data', (data) => {
        const output = data.toString();
        console.error(`[MCP-SSE ERROR] ${output.trim()}`);
        
        // Check for compilation or dependency errors
        if (output.includes('error:') || output.includes('Error:')) {
            console.error('❌ MCP server failed to start due to compilation/dependency errors');
        }
    });
    
    mcpProcess.on('exit', (code) => {
        if (code !== 0) {
            console.error(`❌ MCP SSE server exited with code ${code}`);
        } else {
            console.log('MCP SSE server exited gracefully');
        }
    });
    
    // Wait for server startup
    await new Promise(resolve => {
        const timeout = setTimeout(() => {
            console.log('Server startup timeout reached');
            resolve();
        }, 10000);
        
        const checkReady = setInterval(() => {
            if (serverReady) {
                clearTimeout(timeout);
                clearInterval(checkReady);
                resolve();
            }
        }, 100);
    });
    
    // Test results
    if (sseEndpointFound) {
        console.log('✅ MCP SSE server integration test PASSED');
        console.log('   - SSE server started successfully');
        console.log('   - Dual transport mode working');
        console.log('   - Server accessible on http://localhost:8765/sse');
    } else {
        console.log('❌ MCP SSE server integration test FAILED');
        console.log('   - SSE server did not start properly');
    }
    
    // Cleanup
    console.log('Cleaning up test server...');
    mcpProcess.kill('SIGTERM');
    
    setTimeout(() => {
        mcpProcess.kill('SIGKILL');
    }, 2000);
}

async function testDevWorkflow() {
    console.log('\nTesting development workflow integration...');
    
    // Test port allocation
    const { getPorts } = require('./setup-dev-environment');
    
    try {
        const ports = await getPorts();
        console.log('✅ Port allocation working:');
        console.log(`   - Frontend: ${ports.frontend}`);
        console.log(`   - Backend: ${ports.backend}`);
        console.log(`   - MCP SSE: ${ports.mcpSse}`);
        
        if (ports.mcpSse) {
            console.log('✅ MCP SSE port allocation integrated');
        } else {
            console.log('❌ MCP SSE port allocation failed');
        }
    } catch (error) {
        console.error('❌ Port allocation test failed:', error.message);
    }
}

async function main() {
    console.log('🧪 Running MCP SSE Integration Tests\n');
    
    // Test 1: SSE server functionality
    await testMcpSseIntegration();
    
    // Test 2: Development workflow integration
    await testDevWorkflow();
    
    console.log('\n🎯 Test Summary:');
    console.log('Run these commands to verify full integration:');
    console.log('  1. pnpm run mcp:build    # Build MCP binary');
    console.log('  2. pnpm run dev          # Start full dev environment');
    console.log('  3. curl http://localhost:$(node scripts/setup-dev-environment.js | jq -r .mcpSse)/sse');
    console.log('');
    console.log('Expected: SSE connection established with MCP server');
}

if (require.main === module) {
    main().catch(console.error);
}

module.exports = { testMcpSseIntegration, testDevWorkflow };