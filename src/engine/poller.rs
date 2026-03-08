//! Poller thread: periodic telemetry queries and state updates.

use anyhow::Result;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::engine::alerts::{evaluate_mempool_alerts, evaluate_port_alerts, Alert};
use crate::engine::rates;
use crate::model::port::{LinkStatus, PortState};
use crate::model::state::AppState;
use crate::telemetry::protocol::*;
use crate::telemetry::TelemetrySocket;

const DISCOVERY_INTERVAL_SECS: u64 = 10;

pub struct Poller {
    state: Arc<AppState>,
    shutdown: Arc<AtomicBool>,
    interval_secs: f64,
}

impl Poller {
    pub fn new(state: Arc<AppState>, shutdown: Arc<AtomicBool>, interval_secs: f64) -> Self {
        Self {
            state,
            shutdown,
            interval_secs,
        }
    }

    pub fn spawn(self) -> Result<std::thread::JoinHandle<()>> {
        let handle = std::thread::spawn(move || {
            let _ = self.run_loop();
        });
        Ok(handle)
    }

    fn run_loop(&self) -> Result<()> {
        let interval = Duration::from_secs_f64(self.interval_secs);
        let discovery_interval = Duration::from_secs(DISCOVERY_INTERVAL_SECS);
        let mut last_discovery;
        let mut last_poll = Instant::now();

        while !self.shutdown.load(std::sync::atomic::Ordering::Relaxed) {
            match TelemetrySocket::connect(&self.state.socket_path) {
                Ok(mut sock) => {
                    *self.state.connected.write().unwrap() = true;
                    self.discover(&mut sock)?;
                    last_discovery = Instant::now();

                    loop {
                        if self.shutdown.load(std::sync::atomic::Ordering::Relaxed) {
                            break;
                        }
                        if last_discovery.elapsed() >= discovery_interval {
                            self.discover(&mut sock)?;
                            last_discovery = Instant::now();
                        }
                        let now = Instant::now();
                        if now.duration_since(last_poll) >= interval {
                            let elapsed = last_poll.elapsed().as_secs_f64().max(0.001);
                            last_poll = now;
                            if let Err(e) = self.poll_once(&mut sock, elapsed) {
                                eprintln!("Poll error: {}", e);
                                break;
                            }
                            *self.state.last_poll_time.write().unwrap() = Some(now);
                        }
                        std::thread::sleep(Duration::from_millis(100));
                    }
                }
                Err(e) => {
                    *self.state.connected.write().unwrap() = false;
                    eprintln!("Telemetry connect failed: {}", e);
                }
            }
            std::thread::sleep(Duration::from_secs(2));
        }

        *self.state.connected.write().unwrap() = false;
        Ok(())
    }

    fn discover(&self, sock: &mut TelemetrySocket) -> Result<()> {
        let port_ids = parse_ethdev_list(&sock.request("/ethdev/list")?)?;
        let mempool_names = parse_mempool_list(&sock.request("/mempool/list")?)?;

        let mut ports = self.state.ports.write().unwrap();
        let mut port_history = self.state.port_history.write().unwrap();
        let mut mempools = self.state.mempools.write().unwrap();
        let mut mempool_history = self.state.mempool_history.write().unwrap();

        // Add new ports
        for &id in &port_ids {
            if !ports.iter().any(|p| p.id == id) {
                ports.push(PortState::new(id));
                port_history.push(crate::engine::history::RingBuffer::new());
            }
        }
        ports.retain(|p| port_ids.contains(&p.id));
        port_history.truncate(ports.len());

        // Fetch info for each port
        for port in ports.iter_mut() {
            let resp = sock.request(&format!("/ethdev/info,{}", port.id));
            if let Ok(r) = resp {
                if let Ok(info) = parse_ethdev_info(&r, port.id) {
                    port.info = info;
                }
            }
        }

        // Mempools
        let mut new_mempools = Vec::new();
        for name in &mempool_names {
            let resp = sock.request(&format!("/mempool/info,{}", name));
            if let Ok(r) = resp {
                if let Ok(info) = parse_mempool_info(&r, name) {
                    new_mempools.push(crate::model::MempoolState::from_info(&info));
                }
            }
        }
        *mempools = new_mempools;
        mempool_history.resize_with(mempools.len(), crate::engine::history::RingBuffer::new);

        Ok(())
    }

    fn poll_once(&self, sock: &mut TelemetrySocket, elapsed_secs: f64) -> Result<()> {
        let alpha = self.state.smooth_alpha;
        let mut all_alerts = Vec::<Alert>::new();

        let mut ports = self.state.ports.write().unwrap();
        let mut port_history = self.state.port_history.write().unwrap();

        for (pi, port) in ports.iter_mut().enumerate() {
            let stats_resp = sock.request(&format!("/ethdev/stats,{}", port.id));
            let stats_json = match stats_resp {
                Ok(s) => s,
                Err(_) => continue,
            };
            let current_stats = match parse_ethdev_stats(&stats_json, port.id) {
                Ok(s) => s,
                Err(_) => continue,
            };

            // If stats_current is still the zero baseline (first poll), just seed it
            // and skip rate computation to avoid a spurious spike from the full cumulative counter.
            if port.stats_current.ipackets == 0 && port.stats_current.ibytes == 0 {
                port.stats_current = current_stats;
                continue;
            }

            let prev_rates = port.rates.clone();
            port.rates = rates::compute_port_rates(
                &current_stats,
                &port.stats_current,
                elapsed_secs,
                &prev_rates,
                alpha,
            );
            port.stats_previous = std::mem::replace(&mut port.stats_current, current_stats);

            let link_up = port.info.link_status == LinkStatus::Up;
            all_alerts.extend(evaluate_port_alerts(
                port.rates.rx_missed_pps,
                port.rates.rx_nombuf_pps,
                link_up,
                port.id,
            ));

            if pi < port_history.len() {
                port_history[pi].push(port.rates.clone());
            }
        }

        for port in ports.iter_mut() {
            let xstats_resp = sock.request(&format!("/ethdev/xstats,{}", port.id));
            if let Ok(json) = xstats_resp {
                if let Ok(pairs) = parse_ethdev_xstats(&json) {
                    let prev_pairs: Vec<(String, u64)> = port
                        .xstats
                        .iter()
                        .map(|(k, (v, _))| (k.clone(), *v))
                        .collect();
                    let nq = (port.info.nb_rx_queues).max(port.info.nb_tx_queues) as usize;
                    port.queue_stats = rates::compute_queue_rates(
                        &pairs,
                        &prev_pairs,
                        elapsed_secs,
                        nq.max(1),
                        alpha,
                    );
                    port.xstats = pairs.into_iter().map(|(k, v)| (k, (v, 0.0))).collect();
                }
            }
        }

        drop(ports);
        drop(port_history);

        let mempools = self.state.mempools.write().unwrap();
        let mut mempool_history = self.state.mempool_history.write().unwrap();
        for (i, mp) in mempools.iter().enumerate() {
            if i < mempool_history.len() {
                mempool_history[i].push(mp.utilization_pct);
            }
            all_alerts.extend(evaluate_mempool_alerts(mp.utilization_pct));
        }
        drop(mempools);
        drop(mempool_history);

        *self.state.alerts.write().unwrap() = all_alerts;
        Ok(())
    }
}
