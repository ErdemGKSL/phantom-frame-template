use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use axum::body::Body;
use axum::http::{Request, Response, StatusCode, header};
use tower::{Layer, Service};

/// Tower [`Layer`] that redirects any request whose path ends with a trailing
/// slash (other than a bare `/`) to the same path without the slash.
///
/// Example: `GET /about/` → `301 /about`
/// Query strings are preserved: `GET /about/?x=1` → `301 /about?x=1`
#[derive(Clone, Copy)]
pub struct RedirectTrailingSlashLayer;

impl<S> Layer<S> for RedirectTrailingSlashLayer {
    type Service = RedirectTrailingSlash<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RedirectTrailingSlash { inner }
    }
}

#[derive(Clone)]
pub struct RedirectTrailingSlash<S> {
    inner: S,
}

impl<S> Service<Request<Body>> for RedirectTrailingSlash<S>
where
    S: Service<Request<Body>, Response = Response<Body>> + Send + Clone + 'static,
    S::Future: Send + 'static,
{
    type Response = Response<Body>;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let path = req.uri().path();

        if path.len() > 1 && path.ends_with('/') {
            // Strip trailing slash, keep query string
            let new_path = &path[..path.len() - 1];
            let new_uri = if let Some(query) = req.uri().query() {
                format!("{}?{}", new_path, query)
            } else {
                new_path.to_string()
            };

            let response = Response::builder()
                .status(StatusCode::MOVED_PERMANENTLY)
                .header(header::LOCATION, new_uri)
                .body(Body::empty())
                .unwrap();

            return Box::pin(async move { Ok(response) });
        }

        let future = self.inner.call(req);
        Box::pin(async move { future.await })
    }
}
