use std::path::{Path, PathBuf};
use std::process::Command;

/// On Windows, pnpm is a `.cmd` script and cannot be spawned directly.
/// Use `pnpm.cmd` so the OS can resolve it without going through cmd.exe.
fn pnpm_bin() -> &'static str {
    if cfg!(windows) {
        "pnpm.cmd"
    } else {
        "pnpm"
    }
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
    // 8b. Hash-based skip: compute a hash of the client source + build type.
    //     If it matches the previously saved hash, skip the entire build.
    // ---------------------------------------------------------------------------
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR not set"));
    let hash_file = out_dir.join("client-code.hash");
    let meta_file = out_dir.join("client-build.meta");

    // Build-type string encodes which features are active so that switching
    // bun ↔ node ↔ compile always triggers a rebuild.
    let build_type = match (feature_bun, feature_node, feature_compile) {
        (_, _, true) => "bun+compile",
        (true, _, _) => "bun",
        _ => "node",
    };

    let current_hash = compute_client_hash(client_dir, build_type);

    if let Some(saved_hash) = read_saved_hash(&hash_file) {
        if saved_hash == current_hash {
            println!(
                "cargo:warning=Client source unchanged (hash={:#018x}, build_type={}), skipping build.",
                current_hash, build_type
            );
            // Re-emit any cargo instructions that were saved from the previous
            // build; without this the Rust crate would compile without the
            // NODE_BUNDLE_STRATEGY env-var and node_bundled cfg flag.
            re_emit_saved_meta(&meta_file);
            return;
        }
    }

    println!(
        "cargo:warning=Client hash changed (new={:#018x}, build_type={}), rebuilding...",
        current_hash, build_type
    );

    // ---------------------------------------------------------------------------
    // 9. Run `pnpm run build` (SvelteKit build step, adapter-aware via SVELTE_ADAPTER).
    // ---------------------------------------------------------------------------
    println!(
        "cargo:warning=Building SvelteKit client (adapter={})...",
        svelte_adapter
    );

    let install_success = Command::new(pnpm_bin())
        .arg("install")
        .current_dir(client_dir)
        .status()
        .expect("Failed to run pnpm install");

    if !install_success.success() {
        panic!("Client install failed");
    }

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
                save_meta(
                    &meta_file,
                    "cargo:rustc-cfg=node_bundled\ncargo:rustc-env=NODE_BUNDLE_STRATEGY=bundled\n",
                );
            }
            BundleStrategy::Embedded(reason) => {
                // Option A fallback: embed the full build/server directory at compile time.
                // node_bundled cfg is NOT emitted; Rust source selects the embedded path.
                println!("cargo:rustc-env=NODE_BUNDLE_STRATEGY=embedded");
                println!(
                    "cargo:warning=Node bundle failed ({}), falling back to embedded build/ strategy",
                    reason
                );
                save_meta(
                    &meta_file,
                    "cargo:rustc-env=NODE_BUNDLE_STRATEGY=embedded\n",
                );
            }
        }
    } else {
        // No node-specific cargo instructions; write an empty meta so that a
        // stale meta from a previous node build doesn't get re-emitted.
        save_meta(&meta_file, "");
    }

    // ---------------------------------------------------------------------------
    // 11. Save the new hash so subsequent builds can skip if nothing changed.
    // ---------------------------------------------------------------------------
    write_hash(&hash_file, current_hash);
    println!(
        "cargo:warning=Saved client hash {:#018x} to {}",
        current_hash,
        hash_file.display()
    );
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

// ---------------------------------------------------------------------------
// FNV-1a 64-bit hash helpers — no external crates needed.
// ---------------------------------------------------------------------------

/// FNV-1a 64-bit offset basis and prime.
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Feed a byte slice into a running FNV-1a accumulator.
fn fnv1a_update(mut hash: u64, data: &[u8]) -> u64 {
    for &byte in data {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Compute a deterministic FNV-1a hash over all client source files plus the
/// active build-type string.
///
/// Files are discovered by walking `client_dir` recursively, skipping the
/// directories that contain generated or dependency output:
///   - `node_modules/`
///   - `build/`
///   - `dist/`
///   - `.svelte-kit/`
///
/// For each file the relative path (with forward slashes) and the full file
/// contents are both fed into the hash, in a stable sorted order so that
/// adding or renaming a file is always detected.
fn compute_client_hash(client_dir: &Path, build_type: &str) -> u64 {
    // Collect (relative_path_string, absolute_path) for every eligible file.
    let mut entries: Vec<(String, PathBuf)> = Vec::new();
    collect_files(client_dir, client_dir, &mut entries);
    // Sort by relative path so the order is deterministic across platforms.
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut hash = FNV_OFFSET;

    // Mix in the build-type first so a feature switch changes the hash even
    // when no source file has been modified.
    hash = fnv1a_update(hash, build_type.as_bytes());
    // Separator to avoid accidental collisions between the type and file data.
    hash = fnv1a_update(hash, b"\0BUILD_TYPE\0");

    for (rel_path, abs_path) in &entries {
        // Include the path so renames/deletions are detected.
        hash = fnv1a_update(hash, rel_path.as_bytes());
        hash = fnv1a_update(hash, b"\0PATH\0");

        match std::fs::read(abs_path) {
            Ok(contents) => {
                hash = fnv1a_update(hash, &contents);
            }
            Err(e) => {
                // If we can't read a file, mix in the error string so the
                // hash still changes and a rebuild is triggered.
                hash = fnv1a_update(hash, e.to_string().as_bytes());
            }
        }
        hash = fnv1a_update(hash, b"\0FILE\0");
    }

    hash
}

/// Recursively walk `dir`, skipping ignored subdirectories, and push
/// `(relative_path, absolute_path)` pairs into `out`.
fn collect_files(base: &Path, dir: &Path, out: &mut Vec<(String, PathBuf)>) {
    const SKIP_DIRS: &[&str] = &["node_modules", "build", "dist", ".svelte-kit"];

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let file_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };

        if path.is_dir() {
            if SKIP_DIRS.contains(&file_name.as_str()) {
                continue;
            }
            collect_files(base, &path, out);
        } else if path.is_file() {
            // Build a relative path with forward slashes for cross-platform
            // determinism (Windows uses `\` by default).
            let rel = path
                .strip_prefix(base)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            out.push((rel, path));
        }
    }
}

/// Read the previously saved hash from `hash_file`. Returns `None` if the
/// file does not exist or cannot be parsed.
fn read_saved_hash(hash_file: &Path) -> Option<u64> {
    let text = std::fs::read_to_string(hash_file).ok()?;
    // The file contains a single hex string written by `write_hash`.
    u64::from_str_radix(text.trim().trim_start_matches("0x"), 16).ok()
}

/// Write `hash` as a hex string to `hash_file`, creating the file if needed.
fn write_hash(hash_file: &Path, hash: u64) {
    std::fs::write(hash_file, format!("{:#018x}\n", hash))
        .unwrap_or_else(|e| println!("cargo:warning=Failed to write hash file: {}", e));
}

/// Persist cargo instruction lines that must be re-emitted on a skip.
/// Each line in `contents` is a `cargo:...` directive.
fn save_meta(meta_file: &Path, contents: &str) {
    std::fs::write(meta_file, contents)
        .unwrap_or_else(|e| println!("cargo:warning=Failed to write meta file: {}", e));
}

/// Re-emit every `cargo:...` line saved by a previous build.
/// Called when the hash matches and the client build is skipped.
fn re_emit_saved_meta(meta_file: &Path) {
    match std::fs::read_to_string(meta_file) {
        Ok(contents) => {
            for line in contents.lines() {
                let line = line.trim();
                if !line.is_empty() {
                    println!("{}", line);
                }
            }
        }
        Err(_) => {
            // No meta file yet (e.g. first build was interrupted). Safe to
            // ignore — the hash also won't match in that case normally, but
            // be defensive.
        }
    }
}
