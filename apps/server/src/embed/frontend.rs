use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use tracing;
use anyhow::{Context, Result};

#[cfg(target_os = "windows")]
const APP_BINARY: &[u8] = include_bytes!("../../../client/dist/client.exe");

#[cfg(not(target_os = "windows"))]
const APP_BINARY: &[u8] = include_bytes!("../../../client/dist/client");

pub fn run_frontend_binary(frontend_port: u16) -> Result<()> {
    // Create temp directory for executable with project name
    let project_name = env!("WORKSPACE_NAME");
    let temp_dir = std::env::temp_dir().join(project_name);
    std::fs::create_dir_all(&temp_dir)?;
    
    #[cfg(target_os = "windows")]
    let exe_path = temp_dir.join("client.exe");
    
    #[cfg(not(target_os = "windows"))]
    let exe_path = temp_dir.join("client");
    
    tracing::info!("Extracting frontend binary to {:?}", exe_path);
    
    // Write embedded binary to temp location
    let mut file = std::fs::File::create(&exe_path)?;
    file.write_all(APP_BINARY)?;
    drop(file);
    
    // Set executable permissions on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&exe_path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&exe_path, perms)?;
    }
    
    // Spawn the executable with piped output
    let mut child = Command::new(&exe_path)
        .current_dir(&temp_dir)
        .env("PORT", frontend_port.to_string())
        .env("HOST", "127.0.0.1")
        .env("NODE_ENV", "production")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to spawn frontend binary")?;
    
    tracing::info!("Frontend binary started at {:?}", exe_path);
    
    tracing::info!("Frontend binary started at {:?}", exe_path);
    
    // Spawn threads to read and log stdout/stderr
    if let Some(stdout) = child.stdout.take() {
        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                if let Ok(line) = line {
                    tracing::info!(target: "frontend", "{}", line);
                }
            }
        });
    }
    
    if let Some(stderr) = child.stderr.take() {
        std::thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines() {
                if let Ok(line) = line {
                    tracing::warn!(target: "frontend", "{}", line);
                }
            }
        });
    }
    
    Ok(())
}
