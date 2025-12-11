use crate::env::get_enviroment;
use tracing::info;
use tracing_subscriber;

mod embed;
mod env;
mod server;

#[derive(Clone)]
pub struct AppState {
    pub refresh_frontend: phantom_frame::cache::RefreshTrigger,
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    
    // Initialize tracing subscriber with fallback
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"))
        )
        .init();
    
    let environment = get_enviroment();
    info!("Starting server in {:?} mode", environment);

    let frontend_port = match environment {
        env::Environment::Development => 5173,
        env::Environment::Production => 13472,
    };

    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(3030);
    
    info!("Frontend port: {}", frontend_port);
    info!("Server port: {}", port);

    #[cfg(debug_assertions)]
    let _dev_server = embed::DevServer::start().expect("Failed to start dev server");

    #[cfg(not(debug_assertions))]
    {
        embed::run_frontend_binary().expect("Failed to run frontend binary");
    }

    #[cfg(not(debug_assertions))]
    let result = server::start_server(port, frontend_port, environment, embed::AssetsLayer).await;

    #[cfg(debug_assertions)]
    let result = server::start_server(port, frontend_port, environment).await;

    if let Err(e) = result {
        tracing::error!("Server error: {}", e);
        std::process::exit(1);
    }
}