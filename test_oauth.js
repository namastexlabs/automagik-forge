#!/usr/bin/env node

const http = require('http');

async function testOAuthDiscovery() {
    console.log('Testing OAuth 2.1 Discovery Endpoint...');
    
    return new Promise((resolve, reject) => {
        const options = {
            hostname: 'localhost',
            port: 3001,
            path: '/.well-known/oauth-authorization-server',
            method: 'GET',
            headers: {
                'Accept': 'application/json'
            }
        };

        const req = http.request(options, (res) => {
            let data = '';

            res.on('data', (chunk) => {
                data += chunk;
            });

            res.on('end', () => {
                try {
                    const discovery = JSON.parse(data);
                    console.log('✅ OAuth Discovery Response:');
                    console.log(JSON.stringify(discovery, null, 2));
                    
                    // Validate required fields
                    const requiredFields = ['issuer', 'authorization_endpoint', 'token_endpoint'];
                    const missingFields = requiredFields.filter(field => !discovery[field]);
                    
                    if (missingFields.length > 0) {
                        console.log('❌ Missing required fields:', missingFields);
                        reject(new Error(`Missing required fields: ${missingFields.join(', ')}`));
                    } else {
                        console.log('✅ All required OAuth 2.1 discovery fields present');
                        resolve(discovery);
                    }
                } catch (error) {
                    console.log('❌ Failed to parse discovery response:', error.message);
                    console.log('Raw response:', data);
                    reject(error);
                }
            });
        });

        req.on('error', (error) => {
            console.log('❌ Request failed:', error.message);
            reject(error);
        });

        req.setTimeout(5000, () => {
            req.destroy();
            reject(new Error('Request timeout'));
        });

        req.end();
    });
}

async function testMCPServer() {
    console.log('\nTesting MCP Server SSE endpoint...');
    
    return new Promise((resolve, reject) => {
        const options = {
            hostname: 'localhost',
            port: 8889,
            path: '/sse',
            method: 'GET',
            headers: {
                'Accept': 'text/event-stream',
                'Cache-Control': 'no-cache'
            }
        };

        const req = http.request(options, (res) => {
            console.log('✅ MCP SSE endpoint accessible');
            console.log('Status:', res.statusCode);
            console.log('Headers:', res.headers);
            
            // Close immediately after confirming connection
            req.destroy();
            resolve();
        });

        req.on('error', (error) => {
            if (error.code === 'ECONNREFUSED') {
                console.log('⚠️  MCP server not running on port 8889');
            } else {
                console.log('❌ MCP server request failed:', error.message);
            }
            reject(error);
        });

        req.setTimeout(5000, () => {
            req.destroy();
            reject(new Error('MCP server request timeout'));
        });

        req.end();
    });
}

async function main() {
    console.log('Phase 2B MCP OAuth Integration Test\n');
    
    try {
        // Test OAuth discovery
        await testOAuthDiscovery();
        
        // Test MCP server (optional)
        try {
            await testMCPServer();
        } catch (error) {
            console.log('Note: MCP server test failed, but OAuth endpoints are working');
        }
        
        console.log('\n🎉 Phase 2B OAuth Integration Test Completed Successfully!');
        console.log('\nNext steps for MCP clients:');
        console.log('1. Use /.well-known/oauth-authorization-server for discovery');
        console.log('2. Redirect users to /oauth/authorize for GitHub authentication');
        console.log('3. Exchange authorization code at /oauth/token for Bearer token');
        console.log('4. Use Bearer token in Authorization header for authenticated MCP requests');
        
    } catch (error) {
        console.log('\n❌ Test failed:', error.message);
        process.exit(1);
    }
}

if (require.main === module) {
    main();
}

module.exports = { testOAuthDiscovery, testMCPServer };