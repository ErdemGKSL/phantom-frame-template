#[cfg(debug_assertions)]
pub mod dev;

#[cfg(not(debug_assertions))]
pub mod frontend;
#[cfg(not(debug_assertions))]
pub mod static_assets;

#[allow(unused)]
#[cfg(debug_assertions)]
pub use dev::{run_dev_server, DevServer};

#[cfg(not(debug_assertions))]
pub use frontend::run_frontend_binary;
#[cfg(not(debug_assertions))]
pub use static_assets::AssetsLayer;
