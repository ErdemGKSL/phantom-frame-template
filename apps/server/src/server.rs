use anyhow::Result;
use axum::{Extension, Router};
use phantom_frame::CreateProxyConfig;
use std::sync::Arc;
use tracing::{info, instrument};

use crate::{api, env::Environment, middleware::RedirectTrailingSlashLayer, AppState};

#[instrument(skip_all, fields(port = %port, frontend_port = %frontend_port))]
pub async fn start_server(
    port: u16,
    frontend_port: u16,
    environment: Environment,
    #[cfg(not(debug_assertions))] assets_layer: crate::embed::AssetsLayer,
) -> Result<()> {
    info!("Initializing server");

    // Create proxy once so the RefreshTrigger and the Router share the same cache.
    let (proxy_router, refresh_frontend) =
        phantom_frame::create_proxy(create_proxy_config(frontend_port, environment)?);

    let state = Arc::new(AppState::new(refresh_frontend));

    // Create Axum router with proxy
    #[cfg(not(debug_assertions))]
    let app = Router::new()
        .merge(api::api_router())
        .merge(proxy_router)
        .layer(assets_layer)
        .layer(Extension(state))
        .layer(RedirectTrailingSlashLayer);

    #[cfg(debug_assertions)]
    let app = Router::new()
        .merge(api::api_router())
        .merge(proxy_router)
        .layer(Extension(state))
        .layer(RedirectTrailingSlashLayer);

    // Start server
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port))
        .await?;

    info!("Server running on http://127.0.0.1:{}", port);
    axum::serve(listener, app).await?;

    Ok(())
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
