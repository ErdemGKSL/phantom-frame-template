// ---------------------------------------------------------------------------
// embed/mod.rs — conditional module declarations and re-exports.
//
// Compile-time cfg flags (emitted by build.rs):
//   bun              — active when Cargo feature 'bun' is enabled (default)
//   node             — active when Cargo feature 'node' is enabled
//   compile_frontend — active when Cargo feature 'compile' is enabled (implies bun)
//   node_bundled     — active when build.rs succeeded with Option C for node
//                      (bun build --target=node → dist/bundle.node.js).
//                      When absent, Option A (embedded build/ dir) is used instead.
// ---------------------------------------------------------------------------

// ---- dev (debug only) ------------------------------------------------------
#[cfg(debug_assertions)]
pub mod dev;

// ---- production modules (release only) -------------------------------------

// bun compile: self-contained native binary produced by `bun build --compile`
#[cfg(all(not(debug_assertions), compile_frontend))]
pub mod frontend;

// bun runtime (default): run dist/bundle.js with bun
#[cfg(all(not(debug_assertions), bun, not(compile_frontend)))]
pub mod bun_runtime;

// node runtime: run with node (bundled or embedded, selected by node_bundled cfg)
#[cfg(all(not(debug_assertions), node))]
pub mod node_runtime;

// static assets: served via rust-embed from build/client/ (all release modes)
#[cfg(not(debug_assertions))]
pub mod static_assets;

// ---------------------------------------------------------------------------
// Re-exports
// ---------------------------------------------------------------------------

#[allow(unused)]
#[cfg(debug_assertions)]
pub use dev::{run_dev_server, DevServer};

#[cfg(all(not(debug_assertions), compile_frontend))]
pub use frontend::run_frontend_binary as run_frontend;

#[cfg(all(not(debug_assertions), bun, not(compile_frontend)))]
pub use bun_runtime::run_frontend_bun as run_frontend;

#[cfg(all(not(debug_assertions), node))]
pub use node_runtime::run_frontend_node as run_frontend;

#[cfg(not(debug_assertions))]
pub use static_assets::AssetsLayer;
