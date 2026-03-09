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

// ---------------------------------------------------------------------------
// Runner selection
// ---------------------------------------------------------------------------

/// Describes how to invoke `<runner> run dev` for a given package manager.
#[cfg(node)]
struct Runner {
    /// Executable name (looked up in PATH)
    program: &'static str,
    /// Arguments to pass before "dev", e.g. `["run"]` → `<program> run dev`
    args: &'static [&'static str],
    /// Human-readable label used in log messages
    label: &'static str,
}

/// Ordered list of runners tried when the `node` feature is active.
/// The first one whose executable is found in PATH wins.
#[cfg(node)]
const NODE_RUNNERS: &[Runner] = &[
    // Node 22+ built-in script runner (no extra tool required)
    Runner {
        program: "node",
        args: &["--run"],
        label: "node --run dev",
    },
    // npm is always available alongside node
    Runner {
        program: "npm",
        args: &["run"],
        label: "npm run dev",
    },
    // pnpm — common alternative
    Runner {
        program: "pnpm",
        args: &["run"],
        label: "pnpm run dev",
    },
    // npx as last resort (runs vite directly without a package manager)
    Runner {
        program: "npx",
        args: &["--yes", "--"],
        label: "npx -- vite dev",
    },
];

/// Check whether an executable exists in PATH by running `<program> --version`.
#[cfg(node)]
fn executable_in_path(program: &str) -> bool {
    Command::new(program)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Core spawn logic
// ---------------------------------------------------------------------------

/// Spawn the Vite dev server using the appropriate package manager for the
/// active feature (`bun` or `node`) and return the child process together
/// with a channel that fires once Vite reports it is listening.
pub fn run_dev_server() -> Result<(Child, mpsc::Receiver<()>)> {
    let client_dir = std::env::current_dir()
        .context("Failed to get current directory")?
        .join("apps")
        .join("client");

    if !client_dir.exists() {
        anyhow::bail!("Client directory not found at {:?}", client_dir);
    }

    // Exactly one of `bun` or `node` must be active. build.rs enforces mutual
    // exclusion; this compile_error! catches the (impossible in practice) case
    // where neither is set.
    #[cfg(not(any(bun, node)))]
    compile_error!(
        "Either the 'bun' or 'node' Cargo feature must be enabled. \
         The 'bun' feature is on by default; pass --no-default-features --features node \
         to switch to the node adapter."
    );

    // ------------------------------------------------------------------
    // bun feature: always use `bun run dev`
    // ------------------------------------------------------------------
    #[cfg(bun)]
    let (program, args, label) = {
        info!("Starting development server with bun run dev");
        ("bun", vec!["run", "dev"], "bun run dev")
    };

    // ------------------------------------------------------------------
    // node feature: probe PATH for the best available runner
    // ------------------------------------------------------------------
    #[cfg(node)]
    let (program, args, label) = {
        let runner = NODE_RUNNERS
            .iter()
            .find(|r| executable_in_path(r.program))
            .with_context(|| {
                "No suitable Node.js package manager found in PATH. \
                 Tried: node --run, npm run, pnpm run, npx. \
                 Please install Node.js (https://nodejs.org) and ensure it is in your PATH."
            })?;

        info!("Starting development server with {}", runner.label);

        let mut full_args: Vec<&str> = runner.args.to_vec();
        full_args.push("dev");
        (runner.program, full_args, runner.label)
    };

    let mut cmd = Command::new(program);
    for arg in &args {
        cmd.arg(arg);
    }

    let mut child = cmd
        .current_dir(&client_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("Failed to spawn dev server via `{}`", label))?;

    info!(
        "Development server ({}) started with PID: {:?}",
        label,
        child.id()
    );

    let (tx, rx) = mpsc::channel();

    if let Some(stdout) = child.stdout.take() {
        let tx_clone = tx.clone();
        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                if let Ok(line) = line {
                    let cleaned = strip_ansi_codes(&line);
                    if !cleaned.trim().is_empty() {
                        // Vite prints "Local:" when it is ready to accept connections
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

// ---------------------------------------------------------------------------
// DevServer RAII guard
// ---------------------------------------------------------------------------

pub struct DevServer {
    child: Option<Child>,
}

impl DevServer {
    pub fn start() -> Result<Self> {
        let (child, rx) = run_dev_server()?;

        // Block until Vite reports it is listening ("Local:" line)
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
