//! Top-level application state.

use crate::engine::alerts::Alert;
use crate::engine::history::RingBuffer;
use crate::model::{MempoolState, PortRates, PortState};
use std::path::PathBuf;
use std::sync::RwLock;

/// Default history length: 1 hour at 1s resolution.
const PORT_HISTORY_LEN: usize = 3600;
/// Queue history: 5 minutes.
const QUEUE_HISTORY_LEN: usize = 300;
/// Mempool history: 5 minutes.
const MEMPOOL_HISTORY_LEN: usize = 300;

/// Application state shared between poller and TUI.
pub struct AppState {
    pub socket_path: PathBuf,
    pub poll_interval_secs: f64,
    pub smooth_alpha: f64,
    pub selected_port_id: RwLock<u16>,

    pub ports: RwLock<Vec<PortState>>,
    pub mempools: RwLock<Vec<MempoolState>>,
    /// Port history: per-port ring of PortRates.
    pub port_history: RwLock<Vec<RingBuffer<PortRates, PORT_HISTORY_LEN>>>,
    /// Mempool utilization history (per-mempool).
    pub mempool_history: RwLock<Vec<RingBuffer<f64, MEMPOOL_HISTORY_LEN>>>,

    pub connected: RwLock<bool>,
    pub last_poll_time: RwLock<Option<std::time::Instant>>,
    pub alerts: RwLock<Vec<Alert>>,
}

impl AppState {
    pub fn new(
        socket_path: PathBuf,
        poll_interval_secs: f64,
        smooth_alpha: f64,
        selected_port_id: u16,
    ) -> Self {
        Self {
            socket_path,
            poll_interval_secs,
            smooth_alpha,
            selected_port_id: RwLock::new(selected_port_id),
            ports: RwLock::new(Vec::new()),
            mempools: RwLock::new(Vec::new()),
            port_history: RwLock::new(Vec::new()),
            mempool_history: RwLock::new(Vec::new()),
            connected: RwLock::new(false),
            last_poll_time: RwLock::new(None),
            alerts: RwLock::new(Vec::new()),
        }
    }
}
