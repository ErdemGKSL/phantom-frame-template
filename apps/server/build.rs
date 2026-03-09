use std::path::Path;
use std::process::Command;

fn main() {
    // Tell Rust's `check-cfg` system that `cfg(bun_compile)` is an expected custom cfg.
    println!("cargo:rustc-check-cfg=cfg(bun_compile)");

    // Read workspace project name from root Cargo.toml metadata
    let workspace_toml_path = std::path::Path::new("../../Cargo.toml");
    let workspace_toml =
        std::fs::read_to_string(workspace_toml_path).expect("Failed to read workspace Cargo.toml");

    let template_name = "phantom-frame-template";

    let raw_name = workspace_toml
        .lines()
        .skip_while(|line| !line.starts_with("[workspace.metadata]"))
        .skip(1)
        .find(|line| line.trim().starts_with("project_name"))
        .and_then(|line| line.split('=').nth(1))
        .map(|s| s.trim().trim_matches('"').to_string())
        .unwrap_or_else(|| template_name.to_string());

    let workspace_name = if raw_name == template_name {
        // Derive name from the actual folder name of the workspace root
        let folder_name = workspace_toml_path
            .canonicalize()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .and_then(|d| d.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_else(|| template_name.to_string());

        if folder_name != template_name {
            // Rewrite the project_name line in the workspace Cargo.toml
            let updated = workspace_toml.replacen(
                &format!("project_name = \"{}\"", template_name),
                &format!("project_name = \"{}\"", folder_name),
                1,
            );
            std::fs::write(workspace_toml_path, updated)
                .expect("Failed to update workspace Cargo.toml");
            println!(
                "cargo:warning=Renamed project_name from '{}' to '{}'",
                template_name, folder_name
            );
        }

        folder_name
    } else {
        raw_name
    };

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
    println!("cargo:rerun-if-changed=../client/static");
    println!("cargo:rerun-if-changed=../client/package.json");
    println!("cargo:rerun-if-changed=../client/vite.config.ts");
    println!("cargo:rerun-if-changed=../client/svelte.config.js");
    println!("cargo:rerun-if-changed=../client/build/client");
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
