use anyhow::Result;
use axum::{Extension, Router};
use phantom_frame::CreateProxyConfig;
use std::sync::Arc;
use tracing::{info, instrument};

use crate::{env::Environment, AppState};

#[instrument(skip_all, fields(port = %port, frontend_port = %frontend_port))]
pub async fn start_server(
    port: u16,
    frontend_port: u16,
    environment: Environment,
    #[cfg(not(debug_assertions))] assets_layer: crate::embed::AssetsLayer,
) -> Result<()> {
    info!("Initializing server");
    // Create application state
    let state = Arc::new(create_app_state(frontend_port, environment).await?);

    // Create Axum router with proxy
    #[cfg(not(debug_assertions))]
    let app = Router::new()
        .merge(create_proxy_router(frontend_port, environment).await?)
        .layer(assets_layer)
        .layer(Extension(state));

    #[cfg(debug_assertions)]
    let app = Router::new()
        .merge(create_proxy_router(frontend_port, environment).await?)
        .layer(Extension(state));

    // Start server
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port))
        .await?;

    info!("Server running on http://127.0.0.1:{}", port);
    axum::serve(listener, app).await?;

    Ok(())
}

#[instrument(skip_all, fields(frontend_port = %frontend_port))]
async fn create_app_state(frontend_port: u16, environment: Environment) -> Result<AppState> {
    info!("Creating application state");
    let (_, refresh_frontend) =
        phantom_frame::create_proxy(create_proxy_config(frontend_port, environment)?);

    Ok(AppState { refresh_frontend })
}

#[instrument(skip_all, fields(frontend_port = %frontend_port))]
pub async fn create_proxy_router(
    frontend_port: u16,
    environment: Environment,
) -> Result<Router> {
    info!("Creating proxy router");
    let proxy_config = create_proxy_config(frontend_port, environment)?;
    let (proxy_app, _) = phantom_frame::create_proxy(proxy_config);

    Ok(proxy_app)
}

#[instrument(skip_all, fields(frontend_port = %frontend_port))]
fn create_proxy_config(frontend_port: u16, environment: Environment) -> Result<CreateProxyConfig> {
    info!("Creating proxy configuration");
    let proxy_config = CreateProxyConfig::new(format!("http://localhost:{}", frontend_port))
        .with_cache_key_fn(|req| format!("{}::{}", req.method, req.path))
        .with_exclude_paths(vec![
            "POST *".to_string(),
            "PUT *".to_string(),
            "DELETE *".to_string(),
            "PATCH *".to_string(),
        ])
        .with_websocket_enabled(matches!(environment, Environment::Development));

    Ok(proxy_config)
}
