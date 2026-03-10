use anyhow::{Context as _, Result as AnyhowResult};
use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::Response,
};
use rust_embed::RustEmbed;
use std::fs;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::task::{Context, Poll};
use tower::{Layer, Service};

#[derive(RustEmbed)]
#[folder = "../client/build/client"]
struct ClientAssets;

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
    type Future =
        Pin<Box<dyn Future<Output = std::result::Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<std::result::Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let path = req.uri().path().trim_start_matches('/');

        // Try to serve from assets first
        if is_safe_asset_path(path) {
            if let Some(asset) = ClientAssets::get(path) {
                let mime = get_mime_type(path);
                let mut response = Response::builder()
                    .status(StatusCode::OK)
                    .header("content-type", mime);

                if path.starts_with("_app/immutable/") {
                    response =
                        response.header("cache-control", "public,max-age=31536000,immutable");
                }

                let response = response.body(Body::from(asset.data.to_vec())).unwrap();

                return Box::pin(async move { Ok(response) });
            }
        }

        // Fall through to inner service (proxy, api routes, etc.)
        let future = self.inner.call(req);
        Box::pin(async move { future.await })
    }
}

pub fn extract_client_assets(output_dir: &Path) -> AnyhowResult<()> {
    let client_dir = output_dir.join("client");
    fs::create_dir_all(&client_dir).with_context(|| {
        format!(
            "Failed to create client asset directory at {:?}",
            client_dir
        )
    })?;

    for asset_path in ClientAssets::iter() {
        let asset_path = asset_path.as_ref();
        let destination = client_dir.join(asset_path);

        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create asset parent directory at {:?}", parent)
            })?;
        }

        let asset = ClientAssets::get(asset_path)
            .with_context(|| format!("Failed to load embedded asset {}", asset_path))?;

        fs::write(&destination, asset.data.as_ref())
            .with_context(|| format!("Failed to write asset to {:?}", destination))?;
    }

    Ok(())
}

fn is_safe_asset_path(path: &str) -> bool {
    !path.is_empty() && !path.contains("..")
}

fn get_mime_type(path: &str) -> &'static str {
    match path.rsplit('.').next() {
        Some("html") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") | Some("mjs") => "application/javascript; charset=utf-8",
        Some("json") | Some("map") => "application/json; charset=utf-8",
        Some("txt") => "text/plain; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("avif") => "image/avif",
        Some("ico") => "image/x-icon",
        Some("wasm") => "application/wasm",
        Some("xml") => "application/xml; charset=utf-8",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("otf") => "font/otf",
        _ => "application/octet-stream",
    }
}
