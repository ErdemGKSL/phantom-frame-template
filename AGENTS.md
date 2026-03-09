# phantom-frame-template — Agent Guide

This document describes the project structure, how the pieces fit together, and the SSR cache invalidation pattern. Read it before making changes.

---

## Project Layout

```
phantom-frame-template/
├── Cargo.toml                  — Cargo workspace root (resolver = "3")
├── Cargo.lock
└── apps/
    ├── server/                 — Rust binary crate (the only workspace member)
    └── client/                 — SvelteKit frontend (managed by bun, not Cargo)
```

---

## Server Crate (`apps/server/`)

**Framework:** Axum 0.8 on Tokio.

### Source modules

```
apps/server/src/
├── main.rs          — Entry point; defines AppState; resolves ports; starts embed + server
├── server.rs        — Builds the Axum Router; wires api_router + proxy; creates AppState
├── env.rs           — Environment enum (Development / Production)
├── api/
│   └── mod.rs       — Custom API routes (see below)
└── embed/
    ├── mod.rs       — Conditional re-exports based on cfg flags
    ├── dev.rs       — Dev mode: spawns `pnpm run dev` and waits for Vite to be ready
    ├── bun_runtime.rs   — Release (default): embeds dist/bundle.js, runs it via bun
    ├── frontend.rs      — Release (bun_compile feature): embeds compiled binary, spawns it
    └── static_assets.rs — Release: embeds build/client/ via rust-embed as a Tower layer
```

### AppState (`main.rs`)

```rust
pub struct AppState {
    pub refresh_frontend: phantom_frame::cache::RefreshTrigger,
    pub counter: Arc<AtomicUsize>,
}
```

- `refresh_frontend` — handle to the phantom-frame proxy cache; used to invalidate cached SSR responses (see Cache Invalidation section).
- `counter` — example in-memory atomic counter shared across all requests.

`AppState` is wrapped in `Arc` and distributed to handlers via Axum's `Extension` layer.

### Router construction (`server.rs`)

The final Axum `Router` is assembled as:

```
api::api_router()           — /api/* routes handled by Rust
  .merge(proxy_router)      — everything else proxied to SvelteKit
  [.layer(AssetsLayer)]     — release only: serves embedded static files
  .layer(Extension(state))  — injects Arc<AppState> into all handlers
```

The proxy is registered **after** the api router so `/api/*` paths are matched first and never forwarded to SvelteKit.

### Proxy configuration (`server.rs`)

```rust
CreateProxyConfig::new("http://localhost:{frontend_port}")
    .with_cache_key_fn(|req| format!("{}::{}", req.method, req.path))
    .with_exclude_paths(vec!["POST *", "PUT *", "DELETE *", "PATCH *"])
    .with_websocket_enabled(/* true in dev, false in prod */)
```

- Cache key format: `METHOD::PATH` — e.g. `GET::/`, `GET::/__data.json`
- Only GET/HEAD responses are cached; mutating methods bypass the cache entirely.
- WebSocket proxying is enabled in development for Vite HMR.

---

## API Module (`apps/server/src/api/mod.rs`)

All custom Rust API routes live here. Register new routes in `api_router()`.

Current routes:

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/counter` | Returns current counter value as `{ "value": <n> }` |
| GET | `/api/increment` | Atomically increments counter, returns new value, and invalidates the SSR cache for the main page |

To add a new route:

1. Write a handler function in `api/mod.rs` (or a submodule).
2. Add it to `api_router()` with the appropriate `routing::get` / `post` / etc.
3. Access shared state via `Extension(state): Extension<Arc<AppState>>`.
4. If the mutation should be visible in SSR-rendered pages, call `trigger_by_key_match` for the affected cache keys (see below).

---

## Frontend (`apps/client/`)

**Framework:** SvelteKit 2 + Svelte 5, built with Vite 7, served via `svelte-adapter-bun` or `@sveltejs/adapter-node`.

```
apps/client/src/
├── app.html
├── lib/
│   └── index.ts             — $lib barrel (extend as needed)
└── routes/
    ├── +layout.svelte       — Root layout; imports Tailwind CSS
    └── +page.svelte         — Main page; fetches counter from /api/counter on mount
```

The client communicates with the Rust server via plain `fetch` calls to `/api/*`. In development, Vite proxies those requests; in production, Axum routes them directly.

### Build pipeline

```
cargo build --release
  └─ build.rs:
       pnpm run build   → SvelteKit build → apps/client/build/
       bun run bundle  → bun bundles build/index.js → dist/bundle.js
       (bun_compile feature only)
       bun run compile → dist/client binary
```

In development (`cargo run`), the build script is skipped and the Rust binary spawns `pnpm run dev` automatically.

---

## SSR Cache Invalidation

### How the cache works

phantom-frame caches every proxied GET/HEAD response keyed by `METHOD::PATH`. When the same URL is requested again, the cached response is returned directly — the SvelteKit process is not hit at all. This means if server-side state changes (e.g. the counter increments), any SSR-rendered page that displays that state will serve stale HTML until its cache entry is evicted.

### The `RefreshTrigger` API

`AppState.refresh_frontend` is a `phantom_frame::cache::RefreshTrigger`. It has two methods:

```rust
// Evict all cached entries
state.refresh_frontend.trigger();

// Evict entries whose cache key matches a pattern (supports wildcards)
state.refresh_frontend.trigger_by_key_match("GET::/");
```

`trigger_by_key_match` accepts glob-style patterns. Examples:

| Pattern | What it evicts |
|---------|---------------|
| `"GET::/"` | The main page HTML |
| `"GET::/__data.json"` | SvelteKit's data endpoint for the root route |
| `"GET::/posts/*"` | All cached responses under `/posts/` |
| `"GET::*"` | All cached GET responses |

### Cache key format

The key function is defined in `server.rs`:

```rust
.with_cache_key_fn(|req| format!("{}::{}", req.method, req.path))
```

So the key for a `GET /` request is `GET::/`, and for `GET /__data.json` it is `GET::/__data.json`. SvelteKit fetches `__data.json` alongside the HTML when navigating client-side, so both keys should be invalidated together whenever the underlying data changes.

### Pattern: invalidate after mutation

Any API handler that mutates state visible in SSR pages should invalidate the relevant cache keys before returning:

```rust
async fn my_mutation_handler(Extension(state): Extension<Arc<AppState>>) -> Json<Value> {
    // ... perform the mutation ...

    state.refresh_frontend.trigger_by_key_match("GET::/");
    state.refresh_frontend.trigger_by_key_match("GET::/__data.json");

    Json(json!({ "ok": true }))
}
```

This is already done in `increment_counter` (`api/mod.rs:23-24`). Follow the same pattern for any new mutation routes that affect SSR-rendered content.

---

## Environment & Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `PORT` | `3030` | Port the Axum server listens on |
| `RUST_LOG` | `info` | Tracing filter (e.g. `debug`, `server=trace`) |

Environment (dev vs prod) is determined by `debug_assertions` — i.e. `cargo run` is development, `cargo build --release` is production. There is no runtime env var for this.
