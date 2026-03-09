//! # dpdk-telemetry
//!
//! A Rust client library for reading DPDK application telemetry via Unix socket.
//! No DPDK headers or shared libraries required — just filesystem access to the
//! telemetry socket (typically `/var/run/dpdk/rte/dpdk_telemetry.v2`).
//!
//! ## Quick start
//!
//! ```no_run
//! use dpdk_telemetry::{TelemetrySocket, discovery, protocol};
//!
//! // Auto-discover running DPDK instances
//! let sockets = discovery::discover_sockets().unwrap();
//!
//! // Connect to the first one
//! let mut sock = TelemetrySocket::connect(&sockets[0]).unwrap();
//!
//! // List ports
//! let port_ids = protocol::parse_ethdev_list(&sock.request("/ethdev/list").unwrap()).unwrap();
//!
//! // Get stats for port 0
//! let stats = protocol::parse_ethdev_stats(
//!     &sock.request("/ethdev/stats,0").unwrap(), 0
//! ).unwrap();
//! println!("RX packets: {}", stats.ipackets);
//! ```

pub mod alerts;
pub mod discovery;
pub mod history;
pub mod model;
pub mod protocol;
pub mod rates;
pub mod socket;

pub use socket::TelemetrySocket;
