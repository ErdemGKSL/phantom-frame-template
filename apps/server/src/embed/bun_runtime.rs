use anyhow::{Context, Result};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use tracing::info;

const BUNDLE_JS: &[u8] = include_bytes!("../../../client/dist/bundle.js");

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





/// Production-only frontend runner when `bun_compile` is disabled.
///
/// Extracts the bundled client and runs it directly with bun.
pub fn run_frontend_bun(frontend_port: u16) -> Result<()> {
    let temp_dir = get_project_temp_dir();
    std::fs::create_dir_all(&temp_dir)
        .context("Failed to create bundle directory")?;

    let bundle_path = temp_dir.join("bundle.js");
    
    info!("Extracting frontend bundle to {:?}", bundle_path);
    let mut file = std::fs::File::create(&bundle_path)
        .context("Failed to create bundle file")?;
    file.write_all(BUNDLE_JS)
        .context("Failed to write bundle")?;
    drop(file);

    info!("Starting frontend via bun at {:?}", bundle_path);
    let mut child = Command::new("bun")
        .arg(&bundle_path)
        .current_dir(&temp_dir)
        .env("PORT", frontend_port.to_string())
        .env("HOST", "127.0.0.1")
        .env("NODE_ENV", "production")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to spawn bun frontend")?;

    info!("Frontend (bun) started with PID: {:?}", child.id());

    // Stream logs in background; keep process running after dropping handle.
    if let Some(stdout) = child.stdout.take() {
        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().flatten() {
                let cleaned = strip_ansi_codes(&line);
                if !cleaned.trim().is_empty() {
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

    Ok(())
}
