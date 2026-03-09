// ---------------------------------------------------------------------------
// embed/node_runtime.rs — production frontend runner for the 'node' feature.
//
// Two compile-time strategies are selected via the `node_bundled` cfg flag,
// which build.rs emits after attempting the bundle step:
//
//   node_bundled  (Option C — preferred)
//     build.rs ran `bun run bundle:node` successfully, producing a single
//     dist/bundle.node.js.  At startup we extract that file to a temp
//     directory and run it with `node bundle.node.js`.
//
//   !node_bundled (Option A — fallback)
//     bun was unavailable or the bundle step failed.  The full build/server/
//     directory plus the top-level server entry-points (index.js, handler.js,
//     env.js) were embedded via RustEmbed.  At startup we extract them to a
//     temp directory and run `node index.js`.
//
// In both cases:
//   - Static client assets (build/client/) are served by static_assets.rs,
//     shared with the bun runtime.
//   - We wait up to 30 s for the process to listen on frontend_port.
// ---------------------------------------------------------------------------

use anyhow::{Context, Result};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::process::{Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::{Duration, Instant};
use tracing::info;

use super::static_assets::extract_client_assets;

// ---------------------------------------------------------------------------
// Option C — bundled path: single node-targeted bundle
// ---------------------------------------------------------------------------

#[cfg(node_bundled)]
const BUNDLE_NODE_JS: &[u8] = include_bytes!("../../../client/dist/bundle.node.js");

// ---------------------------------------------------------------------------
// Option A — embedded path: full build/server directory
// ---------------------------------------------------------------------------

#[cfg(not(node_bundled))]
mod embedded {
    use rust_embed::RustEmbed;

    /// The SvelteKit server-side output directory (build/server/).
    /// Contains the SSR renderer chunks referenced by index.js.
    #[derive(RustEmbed)]
    #[folder = "../client/build/server"]
    pub struct ServerAssets;

    /// Top-level entry-point files that sit directly in build/:
    ///   index.js, handler.js, env.js
    #[derive(RustEmbed)]
    #[folder = "../client/build"]
    #[include = "*.js"]
    pub struct BuildRootAssets;
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn get_project_temp_dir() -> std::path::PathBuf {
    let project_name = env!("WORKSPACE_NAME");
    std::env::temp_dir().join(project_name)
}

fn strip_ansi_codes(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            if let Some('[') = chars.next() {
                while let Some(c) = chars.next() {
                    if c.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else {
            result.push(ch);
        }
    }
    result
}

/// Spawn `node <entry>`, stream its logs, and wait until it is ready.
fn spawn_and_wait(
    entry: &std::path::Path,
    temp_dir: &std::path::Path,
    frontend_port: u16,
    rust_port: u16,
) -> Result<()> {
    info!("Starting frontend via node at {:?}", entry);

    let mut child = Command::new("node")
        .arg(entry)
        .current_dir(temp_dir)
        .env("PORT", frontend_port.to_string())
        .env("HOST", "127.0.0.1")
        .env("NODE_ENV", "production")
        .env("PUBLIC_RUST_SERVER_PORT", rust_port.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to spawn node frontend")?;

    info!("Frontend (node) started with PID: {:?}", child.id());

    let ready = Arc::new(AtomicBool::new(false));
    let ready_clone = ready.clone();

    if let Some(stdout) = child.stdout.take() {
        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().flatten() {
                let cleaned = strip_ansi_codes(&line);
                if !cleaned.trim().is_empty() {
                    if cleaned.contains("Listening on") {
                        ready_clone.store(true, Ordering::SeqCst);
                    }
                    tracing::info!(target: "frontend", "{}", cleaned);
                }
            }
        });
    }

    if let Some(stderr) = child.stderr.take() {
        std::thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().flatten() {
                let cleaned = strip_ansi_codes(&line);
                if !cleaned.trim().is_empty() {
                    tracing::warn!(target: "frontend", "{}", cleaned);
                }
            }
        });
    }

    info!(
        "Waiting for frontend to be ready on port {}...",
        frontend_port
    );
    let start = Instant::now();
    let timeout = Duration::from_secs(30);

    while !ready.load(Ordering::SeqCst) && start.elapsed() < timeout {
        if TcpStream::connect(("127.0.0.1", frontend_port)).is_ok() {
            info!("Frontend port {} is now available", frontend_port);
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }

    if start.elapsed() >= timeout {
        anyhow::bail!(
            "Frontend failed to start within {} seconds",
            timeout.as_secs()
        );
    }

    info!("Frontend is ready after {:?}", start.elapsed());
    Ok(())
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Production frontend runner for the `node` feature.
///
/// Selects the bundled strategy (Option C) when build.rs succeeded in
/// producing `dist/bundle.node.js`, or the embedded strategy (Option A)
/// otherwise.
pub fn run_frontend_node(frontend_port: u16, rust_port: u16) -> Result<()> {
    let temp_dir = get_project_temp_dir();
    std::fs::create_dir_all(&temp_dir).context("Failed to create temp directory")?;

    // Extract shared static client assets (build/client/) to temp_dir/client/
    extract_client_assets(&temp_dir)
        .context("Failed to extract static client assets for node runtime")?;

    // -----------------------------------------------------------------
    // Option C — bundled: single dist/bundle.node.js
    // -----------------------------------------------------------------
    #[cfg(node_bundled)]
    {
        let bundle_strategy = env!("NODE_BUNDLE_STRATEGY"); // "bundled"
        info!(
            "Node runtime using bundled strategy ({}) — extracting dist/bundle.node.js",
            bundle_strategy
        );

        let bundle_path = temp_dir.join("bundle.node.js");
        let mut file =
            std::fs::File::create(&bundle_path).context("Failed to create bundle.node.js")?;
        file.write_all(BUNDLE_NODE_JS)
            .context("Failed to write bundle.node.js")?;
        drop(file);

        return spawn_and_wait(&bundle_path, &temp_dir, frontend_port, rust_port);
    }

    // -----------------------------------------------------------------
    // Option A — embedded: full build/server directory + entry files
    // -----------------------------------------------------------------
    #[cfg(not(node_bundled))]
    {
        use embedded::{BuildRootAssets, ServerAssets};

        let bundle_strategy = env!("NODE_BUNDLE_STRATEGY"); // "embedded"
        info!(
            "Node runtime using embedded strategy ({}) — extracting build/ server files",
            bundle_strategy
        );

        // Extract build/server/**  →  temp_dir/server/**
        let server_dir = temp_dir.join("server");
        std::fs::create_dir_all(&server_dir).context("Failed to create server asset directory")?;

        for asset_path in ServerAssets::iter() {
            let asset_path = asset_path.as_ref();
            let destination = server_dir.join(asset_path);
            if let Some(parent) = destination.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("Failed to create server asset dir {:?}", parent))?;
            }
            let asset = ServerAssets::get(asset_path)
                .with_context(|| format!("Failed to load embedded server asset {}", asset_path))?;
            std::fs::write(&destination, asset.data.as_ref())
                .with_context(|| format!("Failed to write server asset to {:?}", destination))?;
        }

        // Extract build/*.js  →  temp_dir/*.js  (index.js, handler.js, env.js)
        for asset_path in BuildRootAssets::iter() {
            let asset_path = asset_path.as_ref();
            // BuildRootAssets only captures *.js at the root of build/,
            // so the paths should be flat (e.g. "index.js").
            let destination = temp_dir.join(asset_path);
            if let Some(parent) = destination.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("Failed to create build root dir {:?}", parent))?;
            }
            let asset = BuildRootAssets::get(asset_path).with_context(|| {
                format!("Failed to load embedded build root asset {}", asset_path)
            })?;
            std::fs::write(&destination, asset.data.as_ref()).with_context(|| {
                format!("Failed to write build root asset to {:?}", destination)
            })?;
        }

        let entry = temp_dir.join("index.js");
        if !entry.exists() {
            anyhow::bail!(
                "index.js not found in temp dir after extracting embedded assets. \
                 Expected it at {:?}",
                entry
            );
        }

        spawn_and_wait(&entry, &temp_dir, frontend_port, rust_port)
    }
}
