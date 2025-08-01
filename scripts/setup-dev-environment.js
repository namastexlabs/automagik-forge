#!/usr/bin/env node

const fs = require("fs");
const path = require("path");
const net = require("net");
const { loadEnv } = require("./load-env");

const PORTS_FILE = path.join(__dirname, "..", ".dev-ports.json");
const DEV_ASSETS_SEED = path.join(__dirname, "..", "dev_assets_seed");
const DEV_ASSETS = path.join(__dirname, "..", "dev_assets");

/**
 * Check if a port is available
 */
function isPortAvailable(port) {
  return new Promise((resolve) => {
    const sock = net.createConnection({ port, host: "localhost" });
    sock.on("connect", () => {
      sock.destroy();
      resolve(false);
    });
    sock.on("error", () => resolve(true));
  });
}

/**
 * Find a free port starting from a given port
 */
async function findFreePort(startPort = 3000) {
  let port = startPort;
  while (!(await isPortAvailable(port))) {
    port++;
    if (port > 65535) {
      throw new Error("No available ports found");
    }
  }
  return port;
}

/**
 * Load existing ports from file
 */
function loadPorts() {
  try {
    if (fs.existsSync(PORTS_FILE)) {
      const data = fs.readFileSync(PORTS_FILE, "utf8");
      return JSON.parse(data);
    }
  } catch (error) {
    console.warn("Failed to load existing ports:", error.message);
  }
  return null;
}

/**
 * Save ports to file
 */
function savePorts(ports) {
  try {
    fs.writeFileSync(PORTS_FILE, JSON.stringify(ports, null, 2));
  } catch (error) {
    console.error("Failed to save ports:", error.message);
    throw error;
  }
}

/**
 * Verify that saved ports are still available
 */
async function verifyPorts(ports) {
  // Check if ports object has all required properties
  if (!ports.mcpSse) {
    if (process.argv[2] === "get") {
      console.log("Port structure outdated, missing mcpSse property, reallocating...");
    }
    return false;
  }

  const frontendAvailable = await isPortAvailable(ports.frontend);
  const backendAvailable = await isPortAvailable(ports.backend);
  const mcpSseAvailable = await isPortAvailable(ports.mcpSse);

  if (process.argv[2] === "get" && (!frontendAvailable || !backendAvailable || !mcpSseAvailable)) {
    console.log(
      `Port availability check failed: frontend:${ports.frontend}=${frontendAvailable}, backend:${ports.backend}=${backendAvailable}, mcpSse:${ports.mcpSse}=${mcpSseAvailable}`
    );
  }

  return frontendAvailable && backendAvailable && mcpSseAvailable;
}

/**
 * Allocate ports for development
 */
async function allocatePorts() {
  // Load .env variables first
  loadEnv();
  
  // Check for environment variables first (from .env)
  const envFrontendPort = process.env.FRONTEND_PORT ? parseInt(process.env.FRONTEND_PORT) : null;
  const envBackendPort = process.env.BACKEND_PORT ? parseInt(process.env.BACKEND_PORT) : null;
  const envMcpSsePort = process.env.MCP_SSE_PORT ? parseInt(process.env.MCP_SSE_PORT) : null;
  
  // If env ports are specified and available, use them
  if (envFrontendPort && envBackendPort && envMcpSsePort) {
    const frontendAvailable = await isPortAvailable(envFrontendPort);
    const backendAvailable = await isPortAvailable(envBackendPort);
    const mcpSseAvailable = await isPortAvailable(envMcpSsePort);
    
    if (frontendAvailable && backendAvailable && mcpSseAvailable) {
      const envPorts = {
        frontend: envFrontendPort,
        backend: envBackendPort,
        mcpSse: envMcpSsePort,
        timestamp: new Date().toISOString(),
      };
      
      if (process.argv[2] === "get") {
        console.log("Using ports from .env file:");
        console.log(`Frontend: ${envPorts.frontend}`);
        console.log(`Backend: ${envPorts.backend}`);
        console.log(`MCP SSE: ${envPorts.mcpSse}`);
      }
      
      savePorts(envPorts);
      return envPorts;
    } else {
      if (process.argv[2] === "get") {
        console.log("Some .env ports are not available, falling back to dynamic allocation...");
      }
    }
  }

  // Try to load existing ports if no .env ports or they're not available
  const existingPorts = loadPorts();

  if (existingPorts) {
    // Verify existing ports are still available
    if (await verifyPorts(existingPorts)) {
      if (process.argv[2] === "get") {
        console.log("Reusing existing dev ports:");
        console.log(`Frontend: ${existingPorts.frontend}`);
        console.log(`Backend: ${existingPorts.backend}`);
        console.log(`MCP SSE: ${existingPorts.mcpSse || 'Not allocated'}`);
      }
      return existingPorts;
    } else {
      if (process.argv[2] === "get") {
        console.log(
          "Existing ports are no longer available, finding new ones..."
        );
      }
    }
  }

  // Find new free ports as last resort
  const frontendPort = await findFreePort(3000);
  const backendPort = await findFreePort(frontendPort + 1);
  const mcpSsePort = await findFreePort(8765);

  const ports = {
    frontend: frontendPort,
    backend: backendPort,
    mcpSse: mcpSsePort,
    timestamp: new Date().toISOString(),
  };

  savePorts(ports);

  if (process.argv[2] === "get") {
    console.log("Allocated new dev ports:");
    console.log(`Frontend: ${ports.frontend}`);
    console.log(`Backend: ${ports.backend}`);
    console.log(`MCP SSE: ${ports.mcpSse}`);
  }

  return ports;
}

/**
 * Get ports (allocate if needed)
 */
async function getPorts() {
  const ports = await allocatePorts();
  copyDevAssets();
  return ports;
}

/**
 * Copy dev_assets_seed to dev_assets
 */
function copyDevAssets() {
  try {
    if (!fs.existsSync(DEV_ASSETS)) {
      // Copy dev_assets_seed to dev_assets
      fs.cpSync(DEV_ASSETS_SEED, DEV_ASSETS, { recursive: true });

      if (process.argv[2] === "get") {
        console.log("Copied dev_assets_seed to dev_assets");
      }
    }
  } catch (error) {
    console.error("Failed to copy dev assets:", error.message);
  }
}

/**
 * Clear saved ports
 */
function clearPorts() {
  try {
    if (fs.existsSync(PORTS_FILE)) {
      fs.unlinkSync(PORTS_FILE);
      console.log("Cleared saved dev ports");
    } else {
      console.log("No saved ports to clear");
    }
  } catch (error) {
    console.error("Failed to clear ports:", error.message);
  }
}

// CLI interface
if (require.main === module) {
  const command = process.argv[2];

  switch (command) {
    case "get":
      getPorts()
        .then((ports) => {
          console.log(JSON.stringify(ports));
        })
        .catch(console.error);
      break;

    case "clear":
      clearPorts();
      break;

    case "frontend":
      getPorts()
        .then((ports) => {
          console.log(JSON.stringify(ports.frontend, null, 2));
        })
        .catch(console.error);
      break;

    case "backend":
      getPorts()
        .then((ports) => {
          console.log(JSON.stringify(ports.backend, null, 2));
        })
        .catch(console.error);
      break;

    default:
      console.log("Usage:");
      console.log(
        "  node setup-dev-environment.js get     - Setup dev environment (ports + assets)"
      );
      console.log(
        "  node setup-dev-environment.js frontend - Get frontend port only"
      );
      console.log(
        "  node setup-dev-environment.js backend  - Get backend port only"
      );
      console.log(
        "  node setup-dev-environment.js clear    - Clear saved ports"
      );
      break;
  }
}

module.exports = { getPorts, clearPorts, findFreePort };
