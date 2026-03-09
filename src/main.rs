//! dpdk-top — real-time DPDK telemetry monitoring TUI.
//!
//! Connects to a running DPDK process via the telemetry Unix socket and displays
//! live interface stats, rates, per-queue distribution, mempool utilization, and more.

#![allow(dead_code)]

mod engine;
mod model;
mod output;
mod ui;

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use dpdk_telemetry::discovery::discover_sockets;

use engine::poller::Poller;
use model::state::AppState;
use ui::app::run_tui;

/// Real-time terminal-based monitoring for DPDK applications.
#[derive(Parser, Debug)]
#[command(name = "dpdk-top", version, about)]
struct Args {
    /// Telemetry socket path (default: auto-detect from /var/run/dpdk/)
    #[arg(short, long, value_name = "PATH")]
    socket: Option<PathBuf>,

    /// Poll interval in seconds
    #[arg(short, long, value_name = "SECS", default_value = "1.0")]
    interval: f64,

    /// Start focused on a specific port
    #[arg(long, value_name = "PORT_ID")]
    port: Option<u16>,

    /// Disable colors (for piping / accessibility)
    #[arg(long)]
    no_color: bool,

    /// Output JSON to stdout instead of TUI (for scripting)
    #[arg(long)]
    json: bool,

    /// Print stats once and exit (with computed rates)
    #[arg(long)]
    once: bool,

    /// Watch a specific xstat counter over time
    #[arg(long, value_name = "XSTAT")]
    watch: Option<String>,

    /// Alert rule (repeatable), e.g. "rx_missed_errors>0"
    #[arg(short = 'a', long = "alert", value_name = "RULE")]
    alerts: Vec<String>,

    /// EMA smoothing alpha (0.0–1.0, 1.0 = no smoothing)
    #[arg(long, value_name = "ALPHA", default_value = "0.8")]
    smooth: f64,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let socket_paths: Vec<PathBuf> = match &args.socket {
        Some(p) => vec![p.clone()],
        None => {
            let sockets = discover_sockets()?;
            match sockets.len() {
                0 => anyhow::bail!(
                    "No DPDK application found.\n\n\
                     dpdk-top auto-scans /var/run/dpdk/ for running DPDK processes.\n\
                     Make sure a DPDK app (e.g. testpmd, l3fwd) is running with telemetry enabled.\n\n\
                     Tip: try running with sudo if the app runs as root:\n\
                     \x20 sudo dpdk-top\n\n\
                     Or point to a socket manually:\n\
                     \x20 dpdk-top -s /var/run/dpdk/rte/dpdk_telemetry.v2"
                ),
                1 => {
                    eprintln!("Auto-detected DPDK socket: {}", sockets[0].display());
                    sockets
                }
                n => {
                    eprintln!("Found {} DPDK processes — use [/] in TUI to switch between them:", n);
                    for (i, s) in sockets.iter().enumerate() {
                        eprintln!("  [{}] {}", i + 1, s.display());
                    }
                    sockets
                }
            }
        }
    };

    let primary_path = &socket_paths[0];

    if args.once {
        return output::oneshot::run_once(primary_path, args.interval, args.smooth);
    }

    if args.json {
        return output::json::run_json_stream(primary_path, args.interval, args.smooth);
    }

    if let Some(xstat) = &args.watch {
        return ui::app::run_watch_mode(primary_path, args.interval, xstat);
    }

    let shutdown = Arc::new(AtomicBool::new(false));

    let instances: Vec<Arc<AppState>> = socket_paths
        .into_iter()
        .map(|path| {
            let state = Arc::new(AppState::new(
                path,
                args.interval,
                args.smooth,
                args.port.unwrap_or(0),
            ));
            let poller = Poller::new(Arc::clone(&state), shutdown.clone(), args.interval);
            let _ = poller.spawn();
            state
        })
        .collect();

    run_tui(instances, shutdown, args.no_color)?;

    Ok(())
}
