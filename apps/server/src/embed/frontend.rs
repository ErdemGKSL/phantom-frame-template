use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use tracing;

#[cfg(target_os = "windows")]
const APP_BINARY: &[u8] = include_bytes!("../../../client/build/client.exe");

#[cfg(not(target_os = "windows"))]
const APP_BINARY: &[u8] = include_bytes!("../../../client/build/client");

pub fn run_frontend_binary() -> std::io::Result<()> {
    // Create temp directory for executable
    let temp_dir = std::env::temp_dir().join("phantom-frame-app");
    std::fs::create_dir_all(&temp_dir)?;
    
    let exe_path = temp_dir.join("client.exe");
    
    // Write embedded binary to temp location
    let mut file = std::fs::File::create(&exe_path)?;
    file.write_all(APP_BINARY)?;
    drop(file);
    
    // Spawn the executable with piped output
    let mut child = Command::new(&exe_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    
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
