//! Delta and rate computation with optional EMA smoothing.

use crate::model::port::{PortRates, PortStats, QueueStats};

/// Compute delta handling u64 wrap.
#[inline]
pub fn delta(current: u64, previous: u64) -> u64 {
    if current >= previous {
        current - previous
    } else {
        (u64::MAX - previous) + current + 1
    }
}

/// Raw rate = delta / elapsed_secs. Then optionally smooth with EMA.
pub fn smooth_rate(raw_rate: f64, previous_smoothed: f64, alpha: f64) -> f64 {
    alpha * raw_rate + (1.0 - alpha) * previous_smoothed
}

/// Compute port rates from current and previous stats.
pub fn compute_port_rates(
    current: &PortStats,
    previous: &PortStats,
    elapsed_secs: f64,
    prev_rates: &PortRates,
    alpha: f64,
) -> PortRates {
    let raw_rx_pps = delta(current.ipackets, previous.ipackets) as f64 / elapsed_secs;
    let raw_tx_pps = delta(current.opackets, previous.opackets) as f64 / elapsed_secs;
    let raw_rx_bps = (delta(current.ibytes, previous.ibytes) as f64 * 8.0) / elapsed_secs;
    let raw_tx_bps = (delta(current.obytes, previous.obytes) as f64 * 8.0) / elapsed_secs;
    let raw_rx_missed = delta(current.imissed, previous.imissed) as f64 / elapsed_secs;
    let raw_rx_nombuf = delta(current.rx_nombuf, previous.rx_nombuf) as f64 / elapsed_secs;
    let raw_ierrors = delta(current.ierrors, previous.ierrors) as f64 / elapsed_secs;
    let raw_oerrors = delta(current.oerrors, previous.oerrors) as f64 / elapsed_secs;

    PortRates {
        rx_pps: smooth_rate(raw_rx_pps, prev_rates.rx_pps, alpha),
        tx_pps: smooth_rate(raw_tx_pps, prev_rates.tx_pps, alpha),
        rx_bps: smooth_rate(raw_rx_bps, prev_rates.rx_bps, alpha),
        tx_bps: smooth_rate(raw_tx_bps, prev_rates.tx_bps, alpha),
        rx_missed_pps: smooth_rate(raw_rx_missed, prev_rates.rx_missed_pps, alpha),
        rx_nombuf_pps: smooth_rate(raw_rx_nombuf, prev_rates.rx_nombuf_pps, alpha),
        ierrors_pps: smooth_rate(raw_ierrors, prev_rates.ierrors_pps, alpha),
        oerrors_pps: smooth_rate(raw_oerrors, prev_rates.oerrors_pps, alpha),
    }
}

/// Build queue stats from xstat (name, value) pairs and previous snapshot; compute rates.
pub fn compute_queue_rates(
    current_xstats: &[(String, u64)],
    previous_xstats: &[(String, u64)],
    elapsed_secs: f64,
    num_queues: usize,
    alpha: f64,
) -> Vec<QueueStats> {
    let mut queues = vec![QueueStats::default(); num_queues];

    for (name, val) in current_xstats {
        let prev = previous_xstats
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| *v)
            .unwrap_or(0);
        let d = delta(*val, prev);
        let rate = d as f64 / elapsed_secs;

        if name.starts_with("rx_q") && name.ends_with("_packets") {
            if let Some(q) = parse_queue_id(name, "rx_q", "_packets") {
                if q < num_queues {
                    let prev_s = queues[q].rx_pps;
                    queues[q].rx_packets = *val;
                    queues[q].rx_pps = smooth_rate(rate, prev_s, alpha);
                }
            }
        } else if name.starts_with("rx_q") && name.ends_with("_bytes") {
            if let Some(q) = parse_queue_id(name, "rx_q", "_bytes") {
                if q < num_queues {
                    let prev_s = queues[q].rx_bps;
                    queues[q].rx_bytes = *val;
                    let bps = (d as f64 * 8.0) / elapsed_secs;
                    queues[q].rx_bps = smooth_rate(bps, prev_s, alpha);
                }
            }
        } else if name.starts_with("tx_q") && name.ends_with("_packets") {
            if let Some(q) = parse_queue_id(name, "tx_q", "_packets") {
                if q < num_queues {
                    let prev_s = queues[q].tx_pps;
                    queues[q].tx_packets = *val;
                    queues[q].tx_pps = smooth_rate(rate, prev_s, alpha);
                }
            }
        } else if name.starts_with("tx_q") && name.ends_with("_bytes") {
            if let Some(q) = parse_queue_id(name, "tx_q", "_bytes") {
                if q < num_queues {
                    let prev_s = queues[q].tx_bps;
                    queues[q].tx_bytes = *val;
                    let bps = (d as f64 * 8.0) / elapsed_secs;
                    queues[q].tx_bps = smooth_rate(bps, prev_s, alpha);
                }
            }
        }
    }

    queues
}

fn parse_queue_id(name: &str, prefix: &str, suffix: &str) -> Option<usize> {
    let s = name.strip_prefix(prefix)?.strip_suffix(suffix)?;
    s.parse::<usize>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delta_wrap() {
        assert_eq!(delta(10, 5), 5);
        assert_eq!(delta(0, u64::MAX), 1);
        assert_eq!(delta(100, 90), 10);
    }

    #[test]
    fn test_smooth_rate() {
        assert!((smooth_rate(100.0, 0.0, 1.0) - 100.0).abs() < 1e-6);
        assert!((smooth_rate(100.0, 0.0, 0.0) - 0.0).abs() < 1e-6);
    }
}
