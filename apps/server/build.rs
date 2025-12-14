use std::process::Command;
use std::path::Path;

fn main() {
    // Tell Rust's `check-cfg` system that `cfg(bun_compile)` is an expected custom cfg.
    println!("cargo:rustc-check-cfg=cfg(bun_compile)");
    
    // Read workspace package name from root Cargo.toml
    let workspace_toml = std::fs::read_to_string("../../Cargo.toml")
        .expect("Failed to read workspace Cargo.toml");
    let workspace_name = workspace_toml
        .lines()
        .skip_while(|line| !line.starts_with("[workspace.package]"))
        .skip(1)
        .find(|line| line.trim().starts_with("name"))
        .and_then(|line| line.split('=').nth(1))
        .map(|s| s.trim().trim_matches('"').to_string())
        .unwrap_or_else(|| "phantom-frame-template".to_string());
    
    println!("cargo:rustc-env=WORKSPACE_NAME={}", workspace_name);

    let profile = std::env::var("PROFILE").unwrap();
    println!("cargo:warning=Building with profile: {}", profile);
    
    // Only run build in release mode
    if profile != "release" {
        println!("cargo:warning=Skipping client build in non-release mode");
        return;
    }

    // Check if bun_compile feature is enabled
    let bun_compile = std::env::var("CARGO_FEATURE_BUN_COMPILE").is_ok();

    println!("cargo:warning=bun_compile feature enabled: {}", bun_compile);
    
    if bun_compile {
        // Emit cfg so Rust code can use #[cfg(bun_compile)]
        println!("cargo:rustc-cfg=bun_compile");
    }

    println!("cargo:rerun-if-changed=../client/src");
    println!("cargo:rerun-if-changed=../client/package.json");
    println!("cargo:rerun-if-changed=../client/vite.config.ts");
    println!("cargo:rerun-if-changed=../client/svelte.config.js");
    println!("cargo:rerun-if-changed=../client/dist/client");
    println!("cargo:rerun-if-changed=../client/dist/bundle.js");

    let client_dir = Path::new("../client");
    
    if !client_dir.exists() {
        panic!("Client directory not found at {:?}", client_dir);
    }

    println!("Building client...");
    let build_status = Command::new("bun")
        .arg("run")
        .arg("build")
        .current_dir(client_dir)
        .status()
        .expect("Failed to run bun build");

    if !build_status.success() {
        panic!("Client build failed");
    }

    println!("Bundling client...");
    let bundle_status = Command::new("bun")
        .arg("run")
        .arg("bundle")
        .current_dir(client_dir)
        .status()
        .expect("Failed to run bun bundle");

    if !bundle_status.success() {
        panic!("Client bundle failed");
    }

    if bun_compile {
        println!("Compiling client to binary...");
        let compile_status = Command::new("bun")
            .arg("run")
            .arg("compile")
            .current_dir(client_dir)
            .status()
            .expect("Failed to run bun compile");

        if !compile_status.success() {
            panic!("Client compile failed");
        }

        println!("Client build, bundle, and compile completed successfully");
    } else {
        println!("Client build and bundle completed (bundle ready for bun runtime)");
    }
}
