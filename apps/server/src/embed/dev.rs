use anyhow::{Context, Result};
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use tracing::{info, warn};

fn strip_ansi_codes(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    
    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            // Skip ESC character
            if let Some('[') = chars.next() {
                // Skip until we find a letter (the final character of the escape sequence)
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

pub fn run_dev_server() -> Result<(Child, mpsc::Receiver<()>)> {
    info!("Starting development server with bun run dev");

    let client_dir = std::env::current_dir()
        .context("Failed to get current directory")?
        .join("apps")
        .join("client");

    if !client_dir.exists() {
        anyhow::bail!("Client directory not found at {:?}", client_dir);
    }

    let mut child = Command::new("bun")
        .arg("run")
        .arg("dev")
        .current_dir(&client_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to spawn bun dev server")?;

    info!("Development server started with PID: {:?}", child.id());

    let (tx, rx) = mpsc::channel();

    // Spawn threads to read and log stdout/stderr
    if let Some(stdout) = child.stdout.take() {
        let tx_clone = tx.clone();
        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                if let Ok(line) = line {
                    let cleaned = strip_ansi_codes(&line);
                    if !cleaned.trim().is_empty() {
                        if cleaned.contains("Local:") {
                            tx_clone.send(()).ok();
                        }
                        tracing::info!(target: "dev-frontend", "{}", cleaned);
                    }
                }
            }
        });
    }

    if let Some(stderr) = child.stderr.take() {
        std::thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines() {
                if let Ok(line) = line {
                    let cleaned = strip_ansi_codes(&line);
                    if !cleaned.trim().is_empty() {
                        tracing::warn!(target: "dev-frontend", "{}", cleaned);
                    }
                }
            }
        });
    }

    Ok((child, rx))
}

pub struct DevServer {
    child: Option<Child>,
}

impl DevServer {
    pub fn start() -> Result<Self> {
        let (child, rx) = run_dev_server()?;
        
        // Wait for the dev server to log "Local:"
        rx.recv().ok();
        
        Ok(Self { child: Some(child) })
    }
}

impl Drop for DevServer {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            info!("Stopping development server");
            if let Err(e) = child.kill() {
                warn!("Failed to kill dev server process: {}", e);
            } else {
                info!("Development server stopped");
            }
        }
    }
}
