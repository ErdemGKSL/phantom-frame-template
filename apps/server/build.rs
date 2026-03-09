use std::path::Path;
use std::process::Command;

/// On Windows, pnpm is a `.cmd` script and cannot be spawned directly.
/// Use `pnpm.cmd` so the OS can resolve it without going through cmd.exe.
fn pnpm_bin() -> &'static str {
    if cfg!(windows) { "pnpm.cmd" } else { "pnpm" }
}

fn main() {
    // ---------------------------------------------------------------------------
    // 1. Declare all custom cfgs so rustc's check-cfg doesn't warn about them.
    // ---------------------------------------------------------------------------
    println!("cargo:rustc-check-cfg=cfg(bun)");
    println!("cargo:rustc-check-cfg=cfg(node)");
    println!("cargo:rustc-check-cfg=cfg(compile_frontend)");
    println!("cargo:rustc-check-cfg=cfg(node_bundled)");

    // ---------------------------------------------------------------------------
    // 2. Read active features.
    // ---------------------------------------------------------------------------
    let feature_bun = std::env::var("CARGO_FEATURE_BUN").is_ok();
    let feature_node = std::env::var("CARGO_FEATURE_NODE").is_ok();
    let feature_compile = std::env::var("CARGO_FEATURE_COMPILE").is_ok();

    // ---------------------------------------------------------------------------
    // 3. Mutual-exclusion guard: bun and node cannot both be active.
    // ---------------------------------------------------------------------------
    if feature_bun && feature_node {
        println!(
            "cargo:error=Cannot enable both 'bun' and 'node' features simultaneously. \
             Use --no-default-features --features node to switch to the node adapter."
        );
        std::process::exit(1);
    }

    // compile requires bun (Cargo.toml declares compile = ["bun"], so this is a
    // defensive check — it cannot normally be violated, but guard anyway).
    if feature_compile && !feature_bun {
        println!(
            "cargo:error=The 'compile' feature requires the 'bun' feature. \
             Enable it with --features compile (which implies bun)."
        );
        std::process::exit(1);
    }

    println!(
        "cargo:warning=Features → bun={} node={} compile={}",
        feature_bun, feature_node, feature_compile
    );

    // ---------------------------------------------------------------------------
    // 4. Emit rustc cfgs that Rust source files can use with #[cfg(...)].
    // ---------------------------------------------------------------------------
    if feature_bun {
        println!("cargo:rustc-cfg=bun");
    }
    if feature_node {
        println!("cargo:rustc-cfg=node");
    }
    if feature_compile {
        println!("cargo:rustc-cfg=compile_frontend");
    }

    // ---------------------------------------------------------------------------
    // 5. Resolve workspace project name (template self-customisation logic).
    // ---------------------------------------------------------------------------
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
        let folder_name = workspace_toml_path
            .canonicalize()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .and_then(|d| d.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_else(|| template_name.to_string());

        if folder_name != template_name {
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

    // ---------------------------------------------------------------------------
    // 6. Skip the client build in debug / non-release mode.
    // ---------------------------------------------------------------------------
    let profile = std::env::var("PROFILE").unwrap();
    println!("cargo:warning=Building with profile: {}", profile);

    if profile != "release" {
        println!("cargo:warning=Skipping client build in non-release mode");
        return;
    }

    // ---------------------------------------------------------------------------
    // 7. Register rerun triggers.
    // ---------------------------------------------------------------------------
    println!("cargo:rerun-if-changed=../client/src");
    println!("cargo:rerun-if-changed=../client/static");
    println!("cargo:rerun-if-changed=../client/package.json");
    println!("cargo:rerun-if-changed=../client/vite.config.ts");
    println!("cargo:rerun-if-changed=../client/svelte.config.js");
    println!("cargo:rerun-if-changed=../client/build/client");
    println!("cargo:rerun-if-changed=../client/dist/bundle.js");
    println!("cargo:rerun-if-changed=../client/dist/bundle.node.js");
    println!("cargo:rerun-if-changed=../client/dist/client");

    let client_dir = Path::new("../client");
    if !client_dir.exists() {
        panic!("Client directory not found at {:?}", client_dir);
    }

    // ---------------------------------------------------------------------------
    // 8. Determine which SvelteKit adapter to use and tell the JS build about it.
    // ---------------------------------------------------------------------------
    let svelte_adapter = if feature_node { "node" } else { "bun" };
    println!("cargo:warning=Using SvelteKit adapter: {}", svelte_adapter);

    // ---------------------------------------------------------------------------
    // 9. Run `pnpm run build` (SvelteKit build step, adapter-aware via SVELTE_ADAPTER).
    // ---------------------------------------------------------------------------
    println!(
        "cargo:warning=Building SvelteKit client (adapter={})...",
        svelte_adapter
    );
    let build_status = Command::new(pnpm_bin())
        .arg("run")
        .arg("build")
        .env("SVELTE_ADAPTER", svelte_adapter)
        .current_dir(client_dir)
        .status()
        .expect("Failed to run pnpm run build");

    if !build_status.success() {
        panic!("Client build failed");
    }

    // ---------------------------------------------------------------------------
    // 10. Post-build bundling / compilation steps, depending on active features.
    // ---------------------------------------------------------------------------
    if feature_bun && !feature_compile {
        // Default bun mode: bundle build/index.js into a single dist/bundle.js
        // that can be run directly with `bun bundle.js`.
        println!("cargo:warning=Bundling client for bun runtime...");
        let status = Command::new("bun")
            .arg("run")
            .arg("bundle")
            .current_dir(client_dir)
            .status()
            .expect("Failed to run bun run bundle");

        if !status.success() {
            panic!("Client bundle (bun) failed");
        }
        println!("cargo:warning=Bun bundle complete (dist/bundle.js)");
    }

    if feature_compile {
        // compile mode: bun build --compile produces a self-contained native binary.
        // The bundle step is not needed; we go straight to compile.
        println!("cargo:warning=Compiling client to self-contained binary...");
        let status = Command::new("bun")
            .arg("run")
            .arg("compile")
            .current_dir(client_dir)
            .status()
            .expect("Failed to run bun run compile");

        if !status.success() {
            panic!("Client compile failed");
        }
        println!("cargo:warning=Bun compile complete (dist/client[.exe])");
    }

    if feature_node {
        // node mode: attempt Option C first — use bun to bundle for node target,
        // producing a single dist/bundle.node.js that can be run with `node`.
        // If bun is unavailable or the bundle step fails, fall back to Option A:
        // embed the full build/ server directory and run `node index.js` directly.
        println!("cargo:warning=Attempting node bundle (Option C: bun build --target=node)...");

        let node_bundle_result = try_node_bundle(client_dir);

        match node_bundle_result {
            BundleStrategy::Bundled => {
                // Option C succeeded: a single dist/bundle.node.js was produced.
                println!("cargo:rustc-cfg=node_bundled");
                println!("cargo:rustc-env=NODE_BUNDLE_STRATEGY=bundled");
                println!("cargo:warning=Node bundle complete (dist/bundle.node.js) — using bundled strategy");
            }
            BundleStrategy::Embedded(reason) => {
                // Option A fallback: embed the full build/server directory at compile time.
                // node_bundled cfg is NOT emitted; Rust source selects the embedded path.
                println!("cargo:rustc-env=NODE_BUNDLE_STRATEGY=embedded");
                println!(
                    "cargo:warning=Node bundle failed ({}), falling back to embedded build/ strategy",
                    reason
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helper types and functions
// ---------------------------------------------------------------------------

enum BundleStrategy {
    /// Option C: bun successfully produced dist/bundle.node.js
    Bundled,
    /// Option A fallback: reason why Option C was skipped/failed
    Embedded(String),
}

fn try_node_bundle(client_dir: &Path) -> BundleStrategy {
    // First check whether bun is available at all on this machine.
    let bun_available = Command::new("bun")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !bun_available {
        return BundleStrategy::Embedded("bun not found in PATH".to_string());
    }

    let status = Command::new("bun")
        .arg("run")
        .arg("bundle:node")
        .current_dir(client_dir)
        .status();

    match status {
        Ok(s) if s.success() => BundleStrategy::Bundled,
        Ok(s) => BundleStrategy::Embedded(format!(
            "bundle:node exited with code {}",
            s.code().unwrap_or(-1)
        )),
        Err(e) => BundleStrategy::Embedded(format!("failed to spawn bun: {}", e)),
    }
}
