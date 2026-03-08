//! JSON streaming output for scripting and exporters.

use anyhow::Result;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::engine::poller::Poller;
use crate::model::state::AppState;

pub fn run_json_stream(socket_path: &Path, interval_secs: f64, smooth_alpha: f64) -> Result<()> {
    let state = Arc::new(AppState::new(
        socket_path.to_path_buf(),
        interval_secs,
        smooth_alpha,
        0,
    ));
    let shutdown = Arc::new(AtomicBool::new(false));
    let _handle = Poller::new(Arc::clone(&state), shutdown.clone(), interval_secs).spawn()?;

    let ctrlc_shutdown = shutdown.clone();
    ctrlc::set_handler(move || {
        ctrlc_shutdown.store(true, Ordering::Relaxed);
    })?;

    while !shutdown.load(Ordering::Relaxed) {
        std::thread::sleep(Duration::from_secs_f64(interval_secs));
        let ports = state.ports.read().unwrap();
        let mempools = state.mempools.read().unwrap();
        let mut rows = vec![];
        for port in ports.iter() {
            rows.push(serde_json::json!({
                "id": port.id,
                "pci": port.info.pci,
                "link": format!("{:?}", port.info.link_status),
                "rx_pps": port.rates.rx_pps,
                "tx_pps": port.rates.tx_pps,
                "rx_mbps": port.rates.rx_bps / 1e6,
                "tx_mbps": port.rates.tx_bps / 1e6,
            }));
        }
        let mp: Vec<_> = mempools
            .iter()
            .map(|m| {
                serde_json::json!({
                    "name": m.name,
                    "size": m.size,
                    "in_use": m.in_use,
                    "utilization": m.utilization_pct
                })
            })
            .collect();
        let out = serde_json::json!({
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "ports": rows,
            "mempools": mp,
        });
        println!("{}", out);
    }
    Ok(())
}
