use std::{net::SocketAddr, str::FromStr, sync::Arc};

use rmcp::{transport::{stdio, sse_server::SseServer}, ServiceExt};
use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
use tokio_util::sync::CancellationToken;
use tracing_subscriber::{prelude::*, EnvFilter};
use vibe_kanban::{mcp::task_server::TaskServer, sentry_layer, utils::asset_dir};

fn main() -> anyhow::Result<()> {
    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    let enable_sse = args.contains(&"--mcp-sse".to_string());
    let enable_stdio = args.contains(&"--mcp".to_string()) || enable_sse;
    
    let (stdio_mode, sse_mode) = if enable_sse {
        (enable_stdio, true)   // STDIO if --mcp flag present, SSE always
    } else {
        (enable_stdio, true)   // STDIO only if --mcp flag, SSE by default
    };

    let environment = if cfg!(debug_assertions) {
        "dev"
    } else {
        "production"
    };
    let _guard = sentry::init(("https://1065a1d276a581316999a07d5dffee26@o4509603705192449.ingest.de.sentry.io/4509605576441937", sentry::ClientOptions {
        release: sentry::release_name!(),
        environment: Some(environment.into()),
        ..Default::default()
    }));
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
                        .with_filter(EnvFilter::new("debug")),
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

            let task_server = TaskServer::new(pool);
            let service = Arc::new(task_server);
            
            let mut join_set = tokio::task::JoinSet::new();
            let shutdown_token = CancellationToken::new();
            
            // Start STDIO transport if requested
            if stdio_mode {
                let service_clone = service.clone();
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
            
            // Start SSE transport if requested
            if sse_mode {
                let service_clone = service.clone();
                let token = shutdown_token.clone();
                let sse_port = get_sse_port();
                join_set.spawn(async move {
                    tokio::select! {
                        result = run_sse_server(service_clone, sse_port) => {
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

async fn run_sse_server(service: Arc<TaskServer>, port: u16) -> anyhow::Result<()> {
    let bind_addr = SocketAddr::from(([0, 0, 0, 0], port));
    
    match SseServer::serve(bind_addr).await {
        Ok(sse_server) => {
            tracing::info!("MCP SSE server listening on http://{}/sse", bind_addr);
            
            let cancellation_token = sse_server.with_service({
                let service = service.clone();
                move || service.as_ref().clone()
            });
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
        .unwrap_or(8765) // Default fallback port
}
