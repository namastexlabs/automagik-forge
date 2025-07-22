#!/usr/bin/env node

const { execSync, spawn } = require("child_process");
const path = require("path");
const fs = require("fs");

// Load .env file from current working directory
function loadEnvFile() {
  const envPath = path.join(process.cwd(), '.env');
  if (fs.existsSync(envPath)) {
    const envContent = fs.readFileSync(envPath, 'utf8');
    envContent.split('\n').forEach(line => {
      const trimmed = line.trim();
      if (trimmed && !trimmed.startsWith('#')) {
        const [key, ...valueParts] = trimmed.split('=');
        if (key && valueParts.length > 0) {
          const value = valueParts.join('=');
          if (!process.env[key]) {
            process.env[key] = value;
          }
        }
      }
    });
  }
}

// Load environment variables from .env file
loadEnvFile();

// Detect true CPU arch on macOS (handles Rosetta)
function getUnderlyingArch() {
  const platform = process.platform;
  const nodeArch = process.arch;

  if (platform !== "darwin") {
    return nodeArch;
  }

  // If Node itself is arm64, we’re natively on Apple silicon
  if (nodeArch === "arm64") {
    return "arm64";
  }

  // Otherwise check for Rosetta translation
  try {
    const translated = execSync("sysctl -in sysctl.proc_translated", {
      encoding: "utf8",
    }).trim();
    if (translated === "1") {
      return "arm64";
    }
  } catch {
    // sysctl key not present → assume true Intel
  }

  return "x64";
}

const platform = process.platform;
const arch = getUnderlyingArch();

// Map to our build target names
function getPlatformDir() {
  if (platform === "linux" && arch === "x64") return "linux-x64";
  if (platform === "linux" && arch === "arm64") return "linux-arm64";
  if (platform === "win32" && arch === "x64") return "windows-x64";
  if (platform === "win32" && arch === "arm64") return "windows-arm64";
  if (platform === "darwin" && arch === "x64") return "macos-x64";
  if (platform === "darwin" && arch === "arm64") return "macos-arm64";

  console.error(`❌ Unsupported platform: ${platform}-${arch}`);
  console.error("Supported platforms:");
  console.error("  - Linux x64");
  console.error("  - Linux ARM64");
  console.error("  - Windows x64");
  console.error("  - Windows ARM64");
  console.error("  - macOS x64 (Intel)");
  console.error("  - macOS ARM64 (Apple Silicon)");
  process.exit(1);
}

function getBinaryName(base) {
  return platform === "win32" ? `${base}.exe` : base;
}

const platformDir = getPlatformDir();
const extractDir = path.join(__dirname, "..", "dist", platformDir);
const isMcpMode = process.argv.includes("--mcp");
const isMcpSseMode = process.argv.includes("--mcp-sse");

// ensure output dir
fs.mkdirSync(extractDir, { recursive: true });

function extractAndRun(baseName, launch) {
  const binName = getBinaryName(baseName);
  const binPath = path.join(extractDir, binName);
  const zipName = `${baseName}.zip`;
  const zipPath = path.join(extractDir, zipName);

  // clean old binary
  if (fs.existsSync(binPath)) fs.unlinkSync(binPath);
  if (!fs.existsSync(zipPath)) {
    console.error(`❌ ${zipName} not found at: ${zipPath}`);
    console.error(`Current platform: ${platform}-${arch} (${platformDir})`);
    process.exit(1);
  }

  // extract
  const unzipCmd =
    platform === "win32"
      ? `powershell -Command "Expand-Archive -Path '${zipPath}' -DestinationPath '${extractDir}' -Force"`
      : `unzip -qq -o "${zipPath}" -d "${extractDir}"`;
  execSync(unzipCmd, { stdio: "inherit" });

  // perms & launch
  if (platform !== "win32") {
    try {
      fs.chmodSync(binPath, 0o755);
    } catch { }
  }
  return launch(binPath);
}

if (isMcpMode || isMcpSseMode) {
  extractAndRun("automagik-forge-mcp", (bin) => {
    const mcpArgs = isMcpSseMode ? ["--mcp-sse"] : [];
    console.log(`Starting MCP server with ${isMcpSseMode ? 'SSE + STDIO' : 'STDIO'} transport...`);
    
    // Environment variables are already loaded from .env file
    
    const proc = spawn(bin, mcpArgs, { 
      stdio: ["pipe", "pipe", "pipe"],
      env: { ...process.env }
    });
    process.stdin.pipe(proc.stdin);
    proc.stdout.pipe(process.stdout);
    proc.stderr.pipe(process.stdout);

    proc.on("exit", (c) => process.exit(c || 0));
    proc.on("error", (e) => {
      console.error("❌ MCP server error:", e.message);
      process.exit(1);
    });
    process.on("SIGINT", () => {
      console.error("\n🛑 Shutting down MCP server...");
      proc.kill("SIGINT");
    });
    process.on("SIGTERM", () => proc.kill("SIGTERM"));
  });
} else {
  // Start both main backend server and MCP SSE server concurrently
  console.log(`📦 Extracting automagik-forge and automagik-forge-mcp...`);
  
  // Environment variables are loaded from .env file
  // Use defaults if not specified in .env
  const mcpSsePort = process.env.MCP_SSE_PORT || "23002";
  const backendPort = process.env.BACKEND_PORT || process.env.PORT || "23001";
  const host = process.env.HOST || "0.0.0.0";
  
  let mainServerProc, mcpServerProc;
  let shutdownInProgress = false;
  
  // Function to gracefully shutdown both servers
  const shutdown = (signal) => {
    if (shutdownInProgress) return;
    shutdownInProgress = true;
    
    console.log(`\n🛑 Shutting down servers (${signal})...`);
    
    if (mainServerProc && !mainServerProc.killed) {
      mainServerProc.kill(signal);
    }
    if (mcpServerProc && !mcpServerProc.killed) {
      mcpServerProc.kill(signal);
    }
    
    // Force exit after timeout
    setTimeout(() => {
      console.log("⏰ Force exit after timeout");
      process.exit(1);
    }, 5000);
  };
  
  // Extract and start main backend server
  extractAndRun("automagik-forge", (mainBin) => {
    console.log(`🚀 Starting main backend server on http://${host}:${backendPort}...`);
    mainServerProc = spawn(mainBin, [], { 
      stdio: ["pipe", "pipe", "pipe"],
      env: { ...process.env }
    });
    
    mainServerProc.stdout.on("data", (data) => {
      process.stdout.write(`[MAIN] ${data}`);
    });
    mainServerProc.stderr.on("data", (data) => {
      process.stderr.write(`[MAIN] ${data}`);
    });
    
    mainServerProc.on("exit", (code) => {
      if (!shutdownInProgress) {
        console.error(`❌ Main server exited with code ${code}`);
        shutdown("SIGTERM");
      }
    });
    
    mainServerProc.on("error", (e) => {
      console.error("❌ Main server error:", e.message);
      shutdown("SIGTERM");
    });
    
    // Extract and start MCP SSE server
    extractAndRun("automagik-forge-mcp", (mcpBin) => {
      console.log(`🚀 Starting MCP SSE server on http://${host}:${mcpSsePort}/sse...`);
      mcpServerProc = spawn(mcpBin, ["--mcp-sse"], { 
        stdio: ["pipe", "pipe", "pipe"],
        env: { ...process.env }
      });
      
      mcpServerProc.stdout.on("data", (data) => {
        process.stdout.write(`[MCP] ${data}`);
      });
      mcpServerProc.stderr.on("data", (data) => {
        process.stderr.write(`[MCP] ${data}`);
      });
      
      mcpServerProc.on("exit", (code) => {
        if (!shutdownInProgress) {
          console.error(`❌ MCP server exited with code ${code}`);
          shutdown("SIGTERM");
        }
      });
      
      mcpServerProc.on("error", (e) => {
        console.error("❌ MCP server error:", e.message);
        shutdown("SIGTERM");
      });
    });
  });
  
  // Handle shutdown signals
  process.on("SIGINT", () => shutdown("SIGINT"));
  process.on("SIGTERM", () => shutdown("SIGTERM"));
  process.on("exit", () => shutdown("SIGTERM"));
}
