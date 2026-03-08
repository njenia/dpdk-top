//! Port and queue data structures for DPDK ethdev stats.

use serde::Deserialize;
use std::collections::HashMap;

/// Static/slow-changing port info from /ethdev/info.
#[derive(Clone, Debug, Default)]
pub struct PortInfo {
    pub name: String,
    pub pci: String,
    pub driver: String,
    pub mac: String,
    pub mtu: u16,
    pub link_speed_mbps: u32,
    pub link_status: LinkStatus,
    pub nb_rx_queues: u16,
    pub nb_tx_queues: u16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LinkStatus {
    #[default]
    Unknown,
    Down,
    Up,
}

/// Raw stats from /ethdev/stats.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct PortStats {
    #[serde(default)]
    pub ipackets: u64,
    #[serde(default)]
    pub opackets: u64,
    #[serde(default)]
    pub ibytes: u64,
    #[serde(default)]
    pub obytes: u64,
    #[serde(default)]
    pub imissed: u64,
    #[serde(default)]
    pub ierrors: u64,
    #[serde(default)]
    pub oerrors: u64,
    #[serde(default)]
    pub rx_nombuf: u64,
}

/// Computed rates for a port.
#[derive(Clone, Debug, Default)]
pub struct PortRates {
    pub rx_pps: f64,
    pub tx_pps: f64,
    pub rx_bps: f64,
    pub tx_bps: f64,
    pub rx_missed_pps: f64,
    pub rx_nombuf_pps: f64,
    pub ierrors_pps: f64,
    pub oerrors_pps: f64,
}

/// Per-queue stats (from xstats names like rx_q0_packets).
#[derive(Clone, Debug, Default)]
pub struct QueueStats {
    pub rx_packets: u64,
    pub rx_bytes: u64,
    pub tx_packets: u64,
    pub tx_bytes: u64,
    pub rx_pps: f64,
    pub rx_bps: f64,
    pub tx_pps: f64,
    pub tx_bps: f64,
}

/// Full state for one port.
#[derive(Clone, Debug)]
pub struct PortState {
    pub id: u16,
    pub info: PortInfo,
    pub stats_current: PortStats,
    pub stats_previous: PortStats,
    pub rates: PortRates,
    pub queue_stats: Vec<QueueStats>,
    /// xstat name -> (value, rate)
    pub xstats: HashMap<String, (u64, f64)>,
}

impl PortState {
    pub fn new(id: u16) -> Self {
        Self {
            id,
            info: PortInfo::default(),
            stats_current: PortStats::default(),
            stats_previous: PortStats::default(),
            rates: PortRates::default(),
            queue_stats: Vec::new(),
            xstats: HashMap::new(),
        }
    }
}
