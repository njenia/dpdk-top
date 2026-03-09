//! One-shot mode: print one snapshot with computed rates and exit.

use anyhow::Result;
use std::path::Path;
use std::time::Duration;

use dpdk_telemetry::protocol::*;
use dpdk_telemetry::TelemetrySocket;

pub fn run_once(socket_path: &Path, interval_secs: f64, _smooth: f64) -> Result<()> {
    let mut sock = TelemetrySocket::connect(socket_path)?;
    let port_ids = parse_ethdev_list(&sock.request("/ethdev/list")?)?;
    let mempool_names = parse_mempool_list(&sock.request("/mempool/list")?)?;

    let mut prev_stats = vec![];
    for id in &port_ids {
        let s = parse_ethdev_stats(&sock.request(&format!("/ethdev/stats,{}", id))?, *id)?;
        prev_stats.push((*id, s));
    }
    std::thread::sleep(Duration::from_secs_f64(interval_secs));

    let elapsed = interval_secs.max(0.001);
    let mut ports_json = vec![];
    for (id, prev) in &prev_stats {
        let stats = parse_ethdev_stats(&sock.request(&format!("/ethdev/stats,{}", id))?, *id)?;
        let rx_pps = (stats.ipackets.saturating_sub(prev.ipackets)) as f64 / elapsed;
        let tx_pps = (stats.opackets.saturating_sub(prev.opackets)) as f64 / elapsed;
        let rx_mbps = (stats.ibytes.saturating_sub(prev.ibytes) as f64 * 8.0) / elapsed / 1e6;
        let tx_mbps = (stats.obytes.saturating_sub(prev.obytes) as f64 * 8.0) / elapsed / 1e6;
        ports_json.push(serde_json::json!({
            "id": *id,
            "rx_pps": rx_pps,
            "tx_pps": tx_pps,
            "rx_mbps": rx_mbps,
            "tx_mbps": tx_mbps,
            "ipackets": stats.ipackets,
            "opackets": stats.opackets,
        }));
    }

    let mut mempools_json = vec![];
    for name in &mempool_names {
        let info = parse_mempool_info(&sock.request(&format!("/mempool/info,{}", name))?, name)?;
        let in_use = info.size.saturating_sub(info.free_count);
        let util = if info.size > 0 {
            (in_use as f64 / info.size as f64) * 100.0
        } else {
            0.0
        };
        mempools_json.push(serde_json::json!({
            "name": name,
            "size": info.size,
            "in_use": in_use,
            "utilization": util
        }));
    }

    let out = serde_json::json!({
        "ports": ports_json,
        "mempools": mempools_json,
    });
    println!("{}", serde_json::to_string_pretty(&out)?);
    Ok(())
}
