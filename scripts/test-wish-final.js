#!/usr/bin/env node

const { spawn } = require('child_process');

console.log('🧪 Testing Wish System - Final Test...\n');

let messageId = 1;
let testData = {
  projectId: null,
  taskId1: null,
  taskId2: null,
  wishId: 'test-refactor-auth'
};

function send(mcpProcess, message) {
  const jsonStr = JSON.stringify(message);
  console.log('📤 SEND:', jsonStr);
  mcpProcess.stdin.write(jsonStr + '\n');
}

function testCreateTasks(mcpProcess) {
  console.log('\n=== Testing Task Creation with wish_id ===');
  
  // Get existing projects first
  send(mcpProcess, {
    "jsonrpc": "2.0",
    "id": messageId++,
    "method": "tools/call",
    "params": {
      "name": "list_projects",
      "arguments": {}
    }
  });
}

const mcpProcess = spawn('cargo', ['run', '--manifest-path', 'backend/Cargo.toml', '--', '--mcp'], {
  stdio: ['pipe', 'pipe', 'inherit'],
  env: { ...process.env, PORT: '8893', DISABLE_WORKTREE_ORPHAN_CLEANUP: '1' }
});

let step = 0;
let responses = [];

mcpProcess.stdout.on('data', (data) => {
  const response = data.toString().trim();
  
  // Skip log lines, only process JSON responses
  if (!response.startsWith('{')) {
    return;
  }
  
  console.log(`📥 RECV:`, response);
  responses.push(response);
  
  try {
    const parsed = JSON.parse(response);
    
    if (step === 0) {
      // Initialize
      send(mcpProcess, {
        "jsonrpc": "2.0",
        "id": messageId++,
        "method": "initialize",
        "params": {
          "protocolVersion": "2024-11-05",
          "capabilities": {},
          "clientInfo": {"name": "wish-test", "version": "1.0.0"}
        }
      });
      step++;
    } else if (step === 1) {
      // After initialize, get projects
      testCreateTasks(mcpProcess);
      step++;
    } else if (step === 2 && parsed.result?.content) {
      // Parse project list response
      const content = JSON.parse(parsed.result.content[0].text);
      if (content.projects && content.projects.length > 0) {
        testData.projectId = content.projects[0].id;
        console.log(`💾 Using project: ${testData.projectId}`);
        
        // Create first task with wish_id
        send(mcpProcess, {
          "jsonrpc": "2.0",
          "id": messageId++,
          "method": "tools/call",
          "params": {
            "name": "create_task",
            "arguments": {
              "project_id": testData.projectId,
              "title": "First task in wish",
              "description": "Testing wish system - first task",
              "wish_id": testData.wishId
            }
          }
        });
        step++;
      }
    } else if (step === 3 && parsed.result?.content) {
      // Parse first task creation response
      const content = JSON.parse(parsed.result.content[0].text);
      if (content.task_id) {
        testData.taskId1 = content.task_id;
        console.log(`💾 First task created: ${testData.taskId1}`);
        
        // Create second task with same wish_id (should succeed for grouping)
        send(mcpProcess, {
          "jsonrpc": "2.0",
          "id": messageId++,
          "method": "tools/call",
          "params": {
            "name": "create_task",
            "arguments": {
              "project_id": testData.projectId,
              "title": "Second task in same wish",
              "description": "Testing wish system - second task in same wish",
              "wish_id": testData.wishId
            }
          }
        });
        step++;
      }
    } else if (step === 4 && parsed.result?.content) {
      // Parse second task creation response
      const content = JSON.parse(parsed.result.content[0].text);
      if (content.task_id) {
        testData.taskId2 = content.task_id;
        console.log(`💾 Second task created: ${testData.taskId2}`);
        
        // List tasks filtered by wish_id
        send(mcpProcess, {
          "jsonrpc": "2.0",
          "id": messageId++,
          "method": "tools/call",
          "params": {
            "name": "list_tasks",
            "arguments": {
              "project_id": testData.projectId,
              "wish_id": testData.wishId
            }
          }
        });
        step++;
      }
    } else if (step === 5 && parsed.result?.content) {
      // Parse list tasks response
      const content = JSON.parse(parsed.result.content[0].text);
      console.log(`✅ Found ${content.count} tasks with wish_id '${testData.wishId}'`);
      
      if (content.count === 2) {
        console.log('\n🎉 WISH SYSTEM TEST SUCCESSFUL!');
        console.log('   ✅ Created multiple tasks with same wish_id');
        console.log('   ✅ Tasks grouped properly by wish_id');
        console.log('   ✅ Filtering by wish_id works correctly');
      } else {
        console.log(`❌ Expected 2 tasks, got ${content.count}`);
      }
      
      setTimeout(() => {
        mcpProcess.kill();
        process.exit(0);
      }, 500);
    }
  } catch (e) {
    console.log('⚠️ Non-JSON response or parsing error:', e.message);
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
}, 20000);