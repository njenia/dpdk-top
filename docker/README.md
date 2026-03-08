# Docker setup for dpdk-top testing

Run DPDK (testpmd) + dpdk-top in a container so you can test **dpdk-top** on a Mac or any host without a real DPDK setup.

## Is it reliable?

- **For testing dpdk-top:** Yes. The container runs real DPDK and testpmd, creates the telemetry socket, and dpdk-top connects to it. You get real telemetry data (ports, stats, mempools, rates).
- **For “taking over a network interface”:** No. In Docker on Mac (or typical cloud containers) you don’t have PCI passthrough or a real NIC to bind. This image uses **virtual devices** (`net_null0`, `net_null1`) so there are no real interfaces and no hugepages. For real NIC takeover you need bare-metal Linux (or a VM with PCI passthrough).

So: use this to develop and test dpdk-top; use real hardware or a proper Linux/DPDK host for performance or NIC binding.

## Prerequisites

- Docker (Docker Desktop on Mac is fine)
- Build and run from the **repository root** (parent of `docker/`)

## Build

From the repo root:

```bash
docker build -f docker/Dockerfile -t dpdk-top-demo .
```

Or with compose:

```bash
docker compose -f docker/docker-compose.yml build
```

## Run

Interactive TUI (testpmd runs in the background, dpdk-top in the foreground).  
**Required:** `--cap-add=IPC_LOCK` for testpmd mempool.

```bash
docker run -it --rm --cap-add=IPC_LOCK --name dpdk-top-demo dpdk-top-demo
```

Use **↑/↓** or **j/k** to change the selected port, **q** or **Esc** to quit (container exits).

### RX traffic in dpdk-top

Traffic is generated **inside the container** to a TAP device (dtap0) that testpmd reads from, so you should see non-zero **RX pps** and **RX Mbps** on port 0 as soon as the TUI is up. No Mac script needed.

## One-shot JSON (no TUI)

```bash
docker run --rm dpdk-top-demo dpdk-top -s /var/run/dpdk/rte/dpdk_telemetry.v2 --once
```

(Override entrypoint and run `dpdk-top` directly; testpmd must be running in another container or you’ll get connection errors. For one-shot from this image, you’d need a small wrapper that starts testpmd, waits, then runs `dpdk-top --once`.)

## What runs inside the container

1. **testpmd** in the background:
   - Cores: `-l 0-1`, 2 channels `-n 2`
   - No hugepages: `--no-huge -m 256`
   - No PCI: `--no-pci`
   - Port 0: `net_tap0` (dtap0); traffic is generated inside the container (ping to 10.0.0.1) so dpdk-top shows RX
   - Port 1: `net_null1`
   - Interactive mode, forwarding: `set fwd io` then `start`

2. **dpdk-top** in the foreground, connecting to `/var/run/dpdk/rte/dpdk_telemetry.v2`.

The telemetry socket is created inside the container; it is **not** visible on the Mac host. So you run dpdk-top **inside** this same container (as in the commands above), not from the host.
