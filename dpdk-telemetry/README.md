# dpdk-telemetry

Rust client library for reading DPDK application telemetry via Unix socket. No DPDK headers or shared libraries required — just filesystem access to the telemetry socket.

## Features

- **Socket client** — connects via `SOCK_SEQPACKET` (Linux) with `SOCK_STREAM` fallback (macOS), handles the DPDK banner automatically
- **Auto-discovery** — finds running DPDK instances by scanning `/var/run/dpdk/`, `/run/dpdk/`, and `~/.dpdk/`
- **Protocol parsers** — `parse_ethdev_stats`, `parse_ethdev_info`, `parse_ethdev_xstats`, `parse_mempool_info`, etc., handling quirks across DPDK versions and PMDs
- **Rate computation** — delta calculation with u64 wrap handling, EMA smoothing, per-queue rate breakdown
- **Model types** — `PortStats`, `PortInfo`, `PortRates`, `QueueStats`, `MempoolInfo`, `MempoolState`
- **Alerts** — built-in threshold evaluation for rx_missed, rx_nombuf, link state, and mempool utilization

## Quick start

```rust
use dpdk_telemetry::{TelemetrySocket, discovery, protocol};

// Auto-discover running DPDK instances
let sockets = discovery::discover_sockets().unwrap();

// Connect to the first one
let mut sock = TelemetrySocket::connect(&sockets[0]).unwrap();

// List ports
let port_ids = protocol::parse_ethdev_list(
    &sock.request("/ethdev/list").unwrap()
).unwrap();

// Get stats for port 0
let stats = protocol::parse_ethdev_stats(
    &sock.request("/ethdev/stats,0").unwrap(), 0
).unwrap();
println!("RX packets: {}", stats.ipackets);
```

## Use cases

- Prometheus/OpenTelemetry exporters for DPDK metrics
- CI/CD health checks asserting throughput and zero drops
- Custom monitoring dashboards (web, TUI, or headless)
- Alerting sidecars for VNF/CNF containers

## Related

This library is extracted from [dpdk-top](https://github.com/njenia/dpdk-top), a real-time TUI monitor for DPDK applications.

## License

Apache-2.0 OR MIT
