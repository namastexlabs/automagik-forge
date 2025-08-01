#!/usr/bin/env node

const { execSync, spawn } = require("child_process");
const path = require("path");
const fs = require("fs");
const zlib = require("zlib");

// Fallback ZIP extraction using Node.js when unzip is not available
function extractZipWithNode(zipPath, extractDir) {
  const buffer = fs.readFileSync(zipPath);
  let offset = 0;
  
  // Simple ZIP parser - look for file entries
  while (offset < buffer.length - 30) {
    // Look for local file header signature (0x04034b50)
    if (buffer.readUInt32LE(offset) === 0x04034b50) {
      const filenameLength = buffer.readUInt16LE(offset + 26);
      const extraFieldLength = buffer.readUInt16LE(offset + 28);
      const compressedSize = buffer.readUInt32LE(offset + 18);
      const uncompressedSize = buffer.readUInt32LE(offset + 22);
      const compressionMethod = buffer.readUInt16LE(offset + 8);
      
      offset += 30; // Skip local file header
      
      const filename = buffer.toString('utf8', offset, offset + filenameLength);
      offset += filenameLength + extraFieldLength;
      
      const fileData = buffer.slice(offset, offset + compressedSize);
      offset += compressedSize;
      
      // Skip directories
      if (filename.endsWith('/')) continue;
      
      const outputPath = path.join(extractDir, filename);
      fs.mkdirSync(path.dirname(outputPath), { recursive: true });
      
      if (compressionMethod === 0) {
        // No compression
        fs.writeFileSync(outputPath, fileData);
      } else if (compressionMethod === 8) {
        // Deflate compression
        const decompressed = zlib.inflateRawSync(fileData);
        fs.writeFileSync(outputPath, decompressed);
      } else {
        throw new Error(`Unsupported compression method: ${compressionMethod}`);
      }
    } else {
      offset++;
    }
  }
}

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

// Handle help and version flags first
if (process.argv.includes("--help") || process.argv.includes("-h")) {
  console.log(`
automagik-forge v${require("../package.json").version}

Usage: npx automagik-forge [options]

Options:
  --help, -h           Show this help message
  --version, -v        Show version
  --mcp               Run only MCP server (STDIO mode)
  --mcp-sse           Run only MCP server (SSE + STDIO mode)

Without options: Runs both backend server and MCP SSE server concurrently

Environment Variables:
  BACKEND_PORT        Backend server port (default: 8887)
  MCP_SSE_PORT        MCP SSE server port (default: 8889)
  HOST               Server host (default: 127.0.0.1)

Examples:
  npx automagik-forge              # Start both servers
  npx automagik-forge --mcp        # MCP server only (STDIO)
  npx automagik-forge --mcp-sse    # MCP server only (SSE + STDIO)
`);
  process.exit(0);
}

if (process.argv.includes("--version") || process.argv.includes("-v")) {
  console.log(require("../package.json").version);
  process.exit(0);
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

  // extract with fallback methods
  if (platform === "win32") {
    const unzipCmd = `powershell -Command "Expand-Archive -Path '${zipPath}' -DestinationPath '${extractDir}' -Force"`;
    execSync(unzipCmd, { stdio: "inherit" });
  } else {
    // Try unzip first, fallback to Node.js extraction if not available
    try {
      execSync(`unzip -qq -o "${zipPath}" -d "${extractDir}"`, { stdio: "inherit" });
    } catch (unzipError) {
      console.log("⚠️  unzip not found, using Node.js extraction...");
      try {
        // Fallback to Node.js extraction using yauzl if available, or basic implementation
        extractZipWithNode(zipPath, extractDir);
      } catch (nodeError) {
        console.error("❌ Extraction failed. Please install unzip:");
        console.error("  Ubuntu/Debian: sudo apt-get install unzip");
        console.error("  RHEL/CentOS:   sudo yum install unzip");
        console.error("  Alpine:        apk add unzip");
        console.error("\nOriginal error:", unzipError.message);
        process.exit(1);
      }
    }
  }

  // perms & launch
  if (platform !== "win32") {
    try {
      fs.chmodSync(binPath, 0o755);
    } catch { }
  }
  return launch(binPath);
}

if (isMcpMode || isMcpSseMode) {
  extractAndRun("mcp_task_server", (bin) => {
    const mcpArgs = isMcpSseMode ? ["--mcp-sse"] : ["--mcp"];
    console.log(`Starting MCP server with ${isMcpSseMode ? 'SSE + STDIO' : 'STDIO'} transport...`);
    
    // Environment variables are already loaded from .env file
    
    const proc = spawn(bin, mcpArgs, { 
      stdio: ["pipe", "pipe", "pipe"],
      env: { ...process.env }
    });
    process.stdin.pipe(proc.stdin);
    proc.stdout.pipe(process.stdout);
    proc.stderr.pipe(process.stderr);

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
  console.log(`📦 Extracting automagik-forge and mcp_task_server...`);
  
  // Environment variables are loaded from .env file
  // Use safe defaults (localhost only) unless overridden
  const mcpSsePort = process.env.MCP_SSE_PORT || "8889";
  const backendPort = process.env.BACKEND_PORT || process.env.PORT || "8887";
  const host = process.env.HOST || "127.0.0.1";
  
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
      env: { 
        ...process.env,
        BACKEND_PORT: backendPort,
        PORT: backendPort,
        HOST: host
      }
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
    extractAndRun("mcp_task_server", (mcpBin) => {
      console.log(`🚀 Starting MCP SSE server on http://${host}:${mcpSsePort}/sse...`);
      mcpServerProc = spawn(mcpBin, ["--mcp-sse"], { 
        stdio: ["pipe", "pipe", "pipe"],
        env: { 
          ...process.env,
          MCP_SSE_PORT: mcpSsePort,
          HOST: host
        }
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
