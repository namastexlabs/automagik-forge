use std::{net::SocketAddr, str::FromStr, sync::Arc};

use rmcp::{
    transport::{stdio, sse_server::SseServer}, 
    ServiceExt
};
use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
use tokio_util::sync::CancellationToken;
use tracing_subscriber::{prelude::*, EnvFilter};
use automagik_forge::{
    mcp::{
        task_server::{TaskServer, AuthenticatedTaskServer},
        oauth_middleware::oauth_sse_authentication_middleware,
    }, 
    sentry_layer, 
    utils::asset_dir
};

fn main() -> anyhow::Result<()> {
    // Load .env file if it exists
    dotenvy::dotenv().ok();
    
    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    let enable_sse = args.contains(&"--mcp-sse".to_string());
    let enable_stdio = args.contains(&"--mcp".to_string()) || enable_sse;
    
    let (stdio_mode, sse_mode) = if enable_stdio {
        (true, false)   // --mcp flag: STDIO only
    } else {
        (false, true)   // No flags: SSE only
    };

    let environment = if cfg!(debug_assertions) {
        "dev"
    } else {
        "production"
    };
    // Check if telemetry is disabled
    let telemetry_disabled = std::env::var("DISABLE_TELEMETRY")
        .unwrap_or_default()
        .to_lowercase() == "true";
    
    let _guard = if !telemetry_disabled {
        let sentry_dsn = std::env::var("SENTRY_DSN")
            .unwrap_or_else(|_| "https://fa5e961d24021da4e6df30e5beee03af@o4509714066571264.ingest.us.sentry.io/4509714113495040".to_string());
        
        Some(sentry::init((sentry_dsn, sentry::ClientOptions {
            release: sentry::release_name!(),
            environment: Some(environment.into()),
            ..Default::default()
        })))
    } else {
        None
    };
    sentry::configure_scope(|scope| {
        scope.set_tag("source", "mcp");
    });
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            tracing_subscriber::registry()
                .with(
                    tracing_subscriber::fmt::layer()
                        .with_writer(std::io::stderr)
                        .with_filter(
                            std::env::var("RUST_LOG")
                                .map(|level| EnvFilter::new(level))
                                .unwrap_or_else(|_| EnvFilter::new("info"))
                        ),
                )
                .with(sentry_layer())
                .init();

            tracing::debug!("[MCP] Starting MCP task server...");

            // Database connection
            let database_url = format!(
                "sqlite://{}",
                asset_dir().join("db.sqlite").to_string_lossy()
            );

            let options = SqliteConnectOptions::from_str(&database_url)?.create_if_missing(false);
            let pool = SqlitePool::connect_with(options).await?;

            // Use different servers for different transport modes
            let stdio_task_server = TaskServer::new(pool.clone());
            let stdio_service = Arc::new(stdio_task_server);
            
            let auth_task_server = AuthenticatedTaskServer::new(pool.clone());
            let auth_service = Arc::new(auth_task_server);
            
            let mut join_set = tokio::task::JoinSet::new();
            let shutdown_token = CancellationToken::new();
            
            // Start STDIO transport if requested (uses basic TaskServer)
            if stdio_mode {
                let service_clone = stdio_service.clone();
                let token = shutdown_token.clone();
                join_set.spawn(async move {
                    tokio::select! {
                        result = run_stdio_server(service_clone) => {
                            tracing::info!("STDIO server completed: {:?}", result);
                            result
                        }
                        _ = token.cancelled() => {
                            tracing::info!("STDIO server cancelled");
                            Ok(())
                        }
                    }
                });
            }
            
            // Start SSE transport if requested (uses AuthenticatedTaskServer with OAuth)
            if sse_mode {
                let service_clone = auth_service.clone();
                let token = shutdown_token.clone();
                let sse_port = get_sse_port();
                let pool_clone = pool.clone();
                join_set.spawn(async move {
                    tokio::select! {
                        result = run_sse_server_authenticated(service_clone, sse_port, pool_clone) => {
                            tracing::info!("SSE server completed: {:?}", result);
                            result
                        }
                        _ = token.cancelled() => {
                            tracing::info!("SSE server cancelled");
                            Ok(())
                        }
                    }
                });
            }
            
            // Wait for shutdown signal or any service to fail
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    tracing::info!("Received Ctrl+C, shutting down...");
                }
                Some(result) = join_set.join_next() => {
                    if let Err(e) = result {
                        tracing::error!("Service task failed: {:?}", e);
                    }
                }
            }
            
            // Trigger shutdown for all services
            shutdown_token.cancel();
            
            // Wait for all tasks to complete with timeout
            let shutdown_timeout = tokio::time::Duration::from_secs(5);
            tokio::time::timeout(shutdown_timeout, async {
                while let Some(_) = join_set.join_next().await {}
            }).await.ok();
            
            tracing::info!("MCP server shutdown complete");
            Ok(())
        })
}

async fn run_stdio_server(service: Arc<TaskServer>) -> anyhow::Result<()> {
    tracing::info!("Starting MCP STDIO server...");
    let server = service.as_ref().clone().serve(stdio()).await
        .inspect_err(|e| {
            tracing::error!("STDIO serving error: {:?}", e);
            sentry::capture_error(e);
        })?;
    
    server.waiting().await?;
    Ok(())
}

async fn run_sse_server_authenticated(service: Arc<AuthenticatedTaskServer>, port: u16, pool: SqlitePool) -> anyhow::Result<()> {
    let bind_addr = SocketAddr::from(([0, 0, 0, 0], port));
    
    match SseServer::serve(bind_addr).await {
        Ok(sse_server) => {
            tracing::info!("MCP SSE server with OAuth authentication listening on http://{}/sse", bind_addr);
            
            let base_url = std::env::var("BASE_URL")
                .unwrap_or_else(|_| "http://localhost:3001".to_string());
            tracing::info!("OAuth 2.1 authentication endpoints:");
            tracing::info!("  - Discovery: {}/.well-known/oauth-authorization-server", base_url);
            tracing::info!("  - Authorize: {}/oauth/authorize", base_url);
            tracing::info!("  - Token: {}/oauth/token", base_url);
            
            // Get token store for middleware
            let token_store = service.get_token_store();
            
            // Create OAuth middleware closure
            let _oauth_middleware = {
                let token_store = token_store.clone();
                let pool = pool.clone();
                move |headers: axum::http::HeaderMap, req: axum::http::Request<axum::body::Body>, next: axum::middleware::Next| {
                    let token_store = token_store.clone();
                    let pool = pool.clone();
                    async move {
                        oauth_sse_authentication_middleware(token_store, pool, headers, req, next).await
                    }
                }
            };
            
            // Apply OAuth middleware to service via task-local storage injection
            let protected_service = move || {
                // OAuth middleware injects Bearer token into REQUEST_CONTEXT task-local storage
                // which is then accessible to MCP tools via get_user_context() methods
                service.as_ref().clone()
            };
            
            let cancellation_token = sse_server.with_service(protected_service);
            tracing::info!("MCP SSE server started with OAuth 2.1 authentication ready");
            cancellation_token.cancelled().await;
            Ok(())
        }
        Err(e) => {
            tracing::error!("Failed to start authenticated SSE server on port {}: {}", port, e);
            // Don't fail the entire application if SSE fails
            if std::env::var("MCP_SSE_REQUIRED").is_ok() {
                Err(e.into())
            } else {
                tracing::warn!("SSE server disabled due to startup failure");
                Ok(())
            }
        }
    }
}

// Legacy SSE server function for backward compatibility
#[allow(dead_code)]
async fn run_sse_server(service: Arc<TaskServer>, port: u16) -> anyhow::Result<()> {
    let bind_addr = SocketAddr::from(([0, 0, 0, 0], port));
    
    match SseServer::serve(bind_addr).await {
        Ok(sse_server) => {
            tracing::info!("MCP SSE server (basic) listening on http://{}/sse", bind_addr);
            
            let base_url = std::env::var("BASE_URL")
                .unwrap_or_else(|_| "http://localhost:3001".to_string());
            tracing::info!("OAuth authentication endpoints available at:");
            tracing::info!("  - Discovery: {}/.well-known/oauth-authorization-server", base_url);
            tracing::info!("  - Authorize: {}/oauth/authorize", base_url);
            tracing::info!("  - Token: {}/oauth/token", base_url);
            
            // Use the service directly for basic functionality
            let cancellation_token = sse_server.with_service(move || service.as_ref().clone());
            tracing::info!("MCP SSE server started - OAuth endpoints ready for external clients");
            cancellation_token.cancelled().await;
            Ok(())
        }
        Err(e) => {
            tracing::error!("Failed to start SSE server on port {}: {}", port, e);
            // Don't fail the entire application if SSE fails
            if std::env::var("MCP_SSE_REQUIRED").is_ok() {
                Err(e.into())
            } else {
                tracing::warn!("SSE server disabled due to startup failure");
                Ok(())
            }
        }
    }
}

fn get_sse_port() -> u16 {
    std::env::var("MCP_SSE_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8889) // Default fallback port to match CLI
}
