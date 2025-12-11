use rust_embed::RustEmbed;
use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::Response,
};
use tower::{Layer, Service};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

#[derive(RustEmbed)]
#[folder = "../client/static"]
struct Assets;

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
