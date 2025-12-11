use rust_embed::RustEmbed;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::Response,
};
use tower::{Layer, Service};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tracing;

#[cfg(target_os = "windows")]
const APP_BINARY: &[u8] = include_bytes!("../../client/build/client.exe");

#[cfg(not(target_os = "windows"))]
const APP_BINARY: &[u8] = include_bytes!("../../client/build/client");

#[derive(RustEmbed)]
#[folder = "../client/static"]
struct Assets;

pub fn run_frontend_binary() -> std::io::Result<()> {
    // Create temp directory for executable
    let temp_dir = std::env::temp_dir().join("phantom-frame-app");
    std::fs::create_dir_all(&temp_dir)?;
    
    let exe_path = temp_dir.join("client.exe");
    
    // Write embedded binary to temp location
    let mut file = std::fs::File::create(&exe_path)?;
    file.write_all(APP_BINARY)?;
    drop(file);
    
    // Spawn the executable with piped output
    let mut child = Command::new(&exe_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    
    tracing::info!("Frontend binary started at {:?}", exe_path);
    
    // Spawn threads to read and log stdout/stderr
    if let Some(stdout) = child.stdout.take() {
        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                if let Ok(line) = line {
                    tracing::info!(target: "frontend", "{}", line);
                }
            }
        });
    }
    
    if let Some(stderr) = child.stderr.take() {
        std::thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines() {
                if let Ok(line) = line {
                    tracing::warn!(target: "frontend", "{}", line);
                }
            }
        });
    }
    
    Ok(())
}

#[derive(Clone)]
pub struct AssetsLayer;

impl<S> Layer<S> for AssetsLayer {
    type Service = AssetsMiddleware<S>;

    fn layer(&self, inner: S) -> AssetsMiddleware<S> {
        AssetsMiddleware { inner }
    }
}

#[derive(Clone)]
pub struct AssetsMiddleware<S> {
    inner: S,
}

impl<S> Service<Request<Body>> for AssetsMiddleware<S>
where
    S: Service<Request<Body>, Response = Response> + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let path = req.uri().path().trim_start_matches('/');
        
        // Try to serve from assets first
        if let Some(asset) = Assets::get(path) {
            let mime = get_mime_type(path);
            let response = Response::builder()
                .status(StatusCode::OK)
                .header("content-type", mime)
                .body(Body::from(asset.data.to_vec()))
                .unwrap();
            
            return Box::pin(async move { Ok(response) });
        }

        // Fall through to inner service (proxy, api routes, etc.)
        let future = self.inner.call(req);
        Box::pin(async move { future.await })
    }
}

fn get_mime_type(path: &str) -> &'static str {
    match path.split('.').last() {
        Some("html") => "text/html; charset=utf-8",
        Some("css") => "text/css",
        Some("js") => "application/javascript",
        Some("json") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        _ => "application/octet-stream",
    }
}