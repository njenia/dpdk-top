//! Alert evaluation (built-in and custom rules).

use serde::Serialize;
use std::time::Instant;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AlertSeverity {
    Warning,
    Critical,
}

#[derive(Clone, Debug)]
pub struct Alert {
    pub kind: String,
    pub severity: AlertSeverity,
    pub message: String,
    pub port_id: Option<u16>,
    pub value: Option<f64>,
    pub since: Instant,
}

impl Alert {
    pub fn warning(kind: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            severity: AlertSeverity::Warning,
            message: message.into(),
            port_id: None,
            value: None,
            since: Instant::now(),
        }
    }

    pub fn critical(kind: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            severity: AlertSeverity::Critical,
            message: message.into(),
            port_id: None,
            value: None,
            since: Instant::now(),
        }
    }
}

/// Evaluate mempool-only alerts.
pub fn evaluate_mempool_alerts(utilization_pct: f64) -> Vec<Alert> {
    let mut out = Vec::new();
    if utilization_pct > 98.0 {
        out.push(
            Alert::critical(
                "mempool_critical",
                format!("Mempool utilization {:.1}%", utilization_pct),
            )
            .with_value(utilization_pct),
        );
    } else if utilization_pct > 90.0 {
        out.push(
            Alert::warning(
                "mempool_high",
                format!("Mempool utilization {:.1}%", utilization_pct),
            )
            .with_value(utilization_pct),
        );
    }
    out
}

/// Evaluate port-related built-in alerts.
pub fn evaluate_port_alerts(
    rx_missed_pps: f64,
    rx_nombuf_pps: f64,
    link_up: bool,
    port_id: u16,
) -> Vec<Alert> {
    let mut out = Vec::new();
    if rx_missed_pps > 0.0 {
        out.push(
            Alert::warning(
                "rx_missed_rising",
                format!("rx_missed: {:.0}/s on port {}", rx_missed_pps, port_id),
            )
            .with_port(port_id)
            .with_value(rx_missed_pps),
        );
    }
    if rx_nombuf_pps > 0.0 {
        out.push(
            Alert::critical(
                "rx_nombuf_rising",
                format!("rx_nombuf: {:.0}/s on port {}", rx_nombuf_pps, port_id),
            )
            .with_port(port_id)
            .with_value(rx_nombuf_pps),
        );
    }
    if !link_up {
        out.push(
            Alert::critical("link_down", format!("Port {} link down", port_id)).with_port(port_id),
        );
    }
    out
}

impl Alert {
    fn with_port(mut self, port_id: u16) -> Self {
        self.port_id = Some(port_id);
        self
    }
    fn with_value(mut self, value: f64) -> Self {
        self.value = Some(value);
        self
    }
}

/// For JSON output.
#[derive(Serialize)]
pub struct AlertJson {
    pub r#type: String,
    pub port: Option<u16>,
    pub value: Option<f64>,
    pub duration_secs: u64,
}
