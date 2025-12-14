#[cfg(debug_assertions)]
pub mod dev;

#[cfg(not(debug_assertions))]
#[cfg(bun_compile)]
pub mod frontend;

#[cfg(not(debug_assertions))]
#[cfg(not(bun_compile))]
pub mod bun_runtime;
#[cfg(not(debug_assertions))]
pub mod static_assets;

#[allow(unused)]
#[cfg(debug_assertions)]
pub use dev::{run_dev_server, DevServer};

// #[cfg(not(debug_assertions))]
// #[cfg(bun_compile)]
// pub fn run_frontend(frontend_port: u16) -> anyhow::Result<()> {
// 	frontend::run_frontend_binary(frontend_port)
// }

#[cfg(not(debug_assertions))]
#[cfg(bun_compile)]
pub use frontend::run_frontend_binary as run_frontend;

#[cfg(not(debug_assertions))]
#[cfg(not(bun_compile))]
pub use bun_runtime::run_frontend_bun as run_frontend;

#[cfg(not(debug_assertions))]
pub use static_assets::AssetsLayer;
