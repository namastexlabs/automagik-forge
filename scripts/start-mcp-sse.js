#!/usr/bin/env node

const { spawn } = require('child_process');
const path = require('path');
const { getPorts } = require('./setup-dev-environment');
const { loadEnv } = require('./load-env');

async function startMcpSseServer() {
    try {
        // Load environment variables
        loadEnv();
        
        // Get allocated ports
        const ports = await getPorts();
        
        // Set environment variables
        process.env.MCP_SSE_PORT = ports.mcpSse.toString();
        process.env.FRONTEND_PORT = ports.frontend.toString();
        process.env.BACKEND_PORT = ports.backend.toString();
        
        const mcpBinary = path.join(__dirname, '../target/debug/mcp_task_server');
        
        console.log(`Starting MCP SSE server on port ${ports.mcpSse}...`);
        
        const mcpProcess = spawn(mcpBinary, ['--mcp-sse'], {
            stdio: 'pipe',
            env: { ...process.env }
        });
        
        mcpProcess.stdout.on('data', (data) => {
            console.log(`[MCP-SSE] ${data.toString().trim()}`);
        });
        
        mcpProcess.stderr.on('data', (data) => {
            console.error(`[MCP-SSE] ${data.toString().trim()}`);
        });
        
        mcpProcess.on('exit', (code) => {
            if (code !== 0) {
                console.warn(`MCP SSE server exited with code ${code}`);
            } else {
                console.log('MCP SSE server shutdown gracefully');
            }
        });
        
        // Handle graceful shutdown
        process.on('SIGINT', () => {
            console.log('Shutting down MCP SSE server...');
            mcpProcess.kill('SIGTERM');
            setTimeout(() => {
                mcpProcess.kill('SIGKILL');
            }, 5000);
        });
        
        process.on('SIGTERM', () => {
            mcpProcess.kill('SIGTERM');
        });
        
        return mcpProcess;
    } catch (error) {
        console.error('Failed to start MCP SSE server:', error);
        process.exit(1);
    }
}

if (require.main === module) {
    startMcpSseServer();
}

module.exports = { startMcpSseServer };