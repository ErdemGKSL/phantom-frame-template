use std::process::Command;
use std::path::Path;

fn main() {
    let profile = std::env::var("PROFILE").unwrap();
    
    // Only run build in release mode
    if profile != "release" {
        return;
    }

    println!("cargo:rerun-if-changed=../client/src");
    println!("cargo:rerun-if-changed=../client/package.json");
    println!("cargo:rerun-if-changed=../client/vite.config.ts");
    println!("cargo:rerun-if-changed=../client/svelte.config.js");

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

    println!("Compiling client...");
    let compile_status = Command::new("bun")
        .arg("run")
        .arg("compile")
        .current_dir(client_dir)
        .status()
        .expect("Failed to run bun compile");

    if !compile_status.success() {
        panic!("Client compile failed");
    }

    println!("Client build and compile completed successfully");
}
