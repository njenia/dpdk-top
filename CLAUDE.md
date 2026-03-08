# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**dpdk-top** is a real-time TUI monitor for DPDK applications. It connects to running DPDK processes via Unix socket telemetry and displays live port stats, throughput rates, per-queue distribution, mempool utilization, and historical charts — no DPDK linking required. Supports auto-discovery and switching between multiple DPDK instances.

## Build & Run

```bash
cargo build --release
cargo run --release -- -s /var/run/dpdk/rte/dpdk_telemetry.v2
cargo run --release -- --once    # one-shot JSON
```

## Tests

```bash
cargo test          # all tests (36 unit + integration)
cargo clippy        # lint
cargo fmt           # format
```

## Architecture

```
telemetry/   →   model/   →   engine/   →   ui/ / output/
(socket I/O)    (structs)   (processing)   (rendering)
```

- **`telemetry/`** — Unix socket client, JSON protocol parser, auto-discovery of sockets
- **`model/`** — `AppState` (Arc + RwLock), `PortStats`, `MempoolInfo` structs
- **`engine/`** — Background poller thread, delta/EMA rate computation, ring buffer history, alerts
- **`ui/`** — TUI views (Dashboard, Port, Graphs, Mempools, Xstats, Help), keybindings, widgets
- **`output/`** — `--once` and `--json` non-TUI output modes
- **`lib.rs`** — Re-exports modules for integration tests

## Concurrency

- `main.rs` creates one `Poller` thread per discovered DPDK instance
- Each `Poller` owns an `Arc<AppState>` and updates it via `RwLock`
- TUI runs on the main thread, reads from whichever `AppState` is selected
- Shutdown via `Arc<AtomicBool>`

## Key design decisions

- EMA smoothing (default alpha=0.8) prevents spiky rate displays
- First-poll seeding: skip rate computation on first sample to avoid spike from full cumulative counter
- xstats parser handles both array format (DPDK 21.x+) and dict format (ENA PMD)
- `RingBuffer<T, N>` uses `MaybeUninit` for zero-alloc fixed-capacity history
