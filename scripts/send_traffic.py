#!/usr/bin/env python3
"""Send UDP traffic at a target rate to DPDK-bound ENIs for testing dpdk-top.

RSS queue distribution:
  ENA uses Toeplitz hash of (src_ip, dst_ip, src_port, dst_port) to spread
  traffic across queues. To get real distribution, we must vary the UDP
  source port as seen by the NIC. We do this by pre-binding multiple sockets
  to different local ports — the kernel then stamps each packet with that
  src_port in the UDP header, giving different Toeplitz hashes → different queues.

Multi-ENI mode (--multi):
  Sends to all 3 DPDK ENIs simultaneously using threads, each at --pps rate.
  Default targets are the 3 secondary ENIs on the test EC2 instance.
"""

import argparse
import errno
import os
import random
import socket
import threading
import time

# Default targets for the 3 DPDK-bound ENIs on the EC2 test instance
DEFAULT_TARGETS = [
    "172.31.44.155",  # ENI 1 (pmd0, 00:06.0)
    "172.31.42.123",  # ENI 2 (pmd1, 00:07.0)
    "172.31.35.183",  # ENI 3 (pmd2, 00:08.0)
]

SNDBUF = 8 * 1024 * 1024


def make_socket(src_port: int) -> socket.socket:
    s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    s.setsockopt(socket.SOL_SOCKET, socket.SO_SNDBUF, SNDBUF)
    s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    try:
        s.bind(("", src_port))
    except OSError:
        s.bind(("", 0))
    s.setblocking(False)
    return s


def build_flows(base_port: int, num_socks: int, num_dsts: int, rss: bool):
    """Return (sockets, flows, mode_str)."""
    if rss:
        src_base = random.randint(10000, 50000 - num_socks)
        sockets = [make_socket(src_base + i) for i in range(num_socks)]
        flow_count = max(num_socks, num_dsts) * 4
        flows = [
            (sockets[i % num_socks], base_port + (i % num_dsts))
            for i in range(flow_count)
        ]
        random.shuffle(flows)
        mode_str = (
            f"RSS {num_socks} src-ports × {num_dsts} dst-ports"
        )
    else:
        s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        s.setsockopt(socket.SOL_SOCKET, socket.SO_SNDBUF, SNDBUF)
        s.setblocking(False)
        sockets = [s]
        flows = [(s, base_port)]
        mode_str = f"single flow → :{base_port}"
    return sockets, flows, mode_str


class ENISender:
    """Sends traffic to a single target ENI at a fixed pps rate."""

    def __init__(self, target_ip: str, flows, pps: int, size: int, label: str):
        self.target_ip = target_ip
        self.flows = flows
        self.num_flows = len(flows)
        self.interval = 1.0 / pps
        self.size = size
        self.label = label
        self.payload = os.urandom(size)

        self.sent = 0
        self.errors = 0
        self._stop = threading.Event()

    def stop(self):
        self._stop.set()

    def run(self):
        flow_idx = 0
        next_send = time.monotonic()

        while not self._stop.is_set():
            now = time.monotonic()
            if now >= next_send:
                sock, dst_port = self.flows[flow_idx % self.num_flows]
                flow_idx += 1
                try:
                    sock.sendto(self.payload, (self.target_ip, dst_port))
                    self.sent += 1
                except BlockingIOError:
                    self.errors += 1
                except OSError as e:
                    if e.errno == errno.ENOBUFS:
                        self.errors += 1
                        time.sleep(0.001)
                    else:
                        raise

                next_send += self.interval
                if next_send < now - 0.1:
                    next_send = now
            else:
                sleep_for = next_send - now
                if sleep_for > 0.0005:
                    time.sleep(sleep_for * 0.8)


def run_multi(targets, pps, size, base_port, rss, num_flows, num_dsts):
    """Send to multiple ENIs simultaneously, one thread per target."""
    senders = []
    all_sockets = []

    for i, ip in enumerate(targets):
        socks, flows, mode_str = build_flows(base_port, num_flows, num_dsts, rss)
        all_sockets.extend(socks)
        sender = ENISender(ip, flows, pps, size, label=f"ENI{i+1} {ip}")
        senders.append(sender)

    threads = [
        threading.Thread(target=s.run, daemon=True, name=s.label)
        for s in senders
    ]

    print(f"Targets: {len(targets)} ENIs")
    for i, ip in enumerate(targets):
        print(f"  ENI{i+1}  {ip}")
    print(f"Rate:    {pps:,} pps × {size}B = {pps * size * 8 / 1e6:.1f} Mbps per ENI")
    print(f"Total:   {pps * len(targets):,} pps = {pps * len(targets) * size * 8 / 1e6:.1f} Mbps combined")
    mode_label = f"RSS {num_flows} src-ports × {num_dsts} dst-ports" if rss else f"single flow → :{base_port}"
    print(f"Mode:    {mode_label}")
    print("Press Ctrl+C to stop\n")

    t0 = time.monotonic()
    last_report = t0
    prev_sent = [0] * len(senders)

    for t in threads:
        t.start()

    try:
        while True:
            time.sleep(2.0)
            now = time.monotonic()
            elapsed = now - t0
            window = now - last_report

            header = f"  {'ENI':<20} {'total pkts':>12}  {'pps':>8}  {'Mbps':>7}  {'errs':>5}"
            print(header)
            total_sent = 0
            total_pps = 0.0
            for i, s in enumerate(senders):
                cur = s.sent
                delta = cur - prev_sent[i]
                prev_sent[i] = cur
                pps_actual = delta / window
                mbps = pps_actual * size * 8 / 1e6
                total_sent += cur
                total_pps += pps_actual
                print(f"  {s.label:<20} {cur:>12,}  {pps_actual:>8,.0f}  {mbps:>7.1f}  {s.errors:>5}")
            total_mbps = total_pps * size * 8 / 1e6
            print(f"  {'TOTAL':<20} {total_sent:>12,}  {total_pps:>8,.0f}  {total_mbps:>7.1f}")
            print()
            last_report = now

    except KeyboardInterrupt:
        for s in senders:
            s.stop()
        elapsed = time.monotonic() - t0
        print(f"\nDone: {sum(s.sent for s in senders):,} total packets in {elapsed:.1f}s")

    for s in all_sockets:
        s.close()


def run_single(target, pps, size, base_port, rss, num_flows, num_dsts):
    """Send to a single target."""
    sockets, flows, mode_str = build_flows(base_port, num_flows, num_dsts, rss)
    num_flow_count = len(flows)

    print(f"Target:  {target}")
    print(f"Rate:    {pps:,} pps  ×  {size}B  =  {pps * size * 8 / 1e6:.1f} Mbps")
    print(f"Mode:    {mode_str}")
    print("Press Ctrl+C to stop\n")

    interval = 1.0 / pps
    sent = 0
    errors = 0
    payload = os.urandom(size)
    t0 = time.monotonic()
    next_send = t0
    last_report = t0
    flow_idx = 0

    try:
        while True:
            now = time.monotonic()
            if now >= next_send:
                sock, dst_port = flows[flow_idx % num_flow_count]
                flow_idx += 1
                try:
                    sock.sendto(payload, (target, dst_port))
                    sent += 1
                except BlockingIOError:
                    errors += 1
                except OSError as e:
                    if e.errno == errno.ENOBUFS:
                        errors += 1
                        time.sleep(0.001)
                    else:
                        raise

                next_send += interval
                if next_send < now - 0.1:
                    next_send = now
            else:
                sleep_for = next_send - now
                if sleep_for > 0.0005:
                    time.sleep(sleep_for * 0.8)

            if now - last_report >= 2.0:
                elapsed = now - t0
                print(
                    f"  {sent:>12,} pkts | {sent/elapsed:>10,.0f} pps | "
                    f"{sent * size * 8 / elapsed / 1e6:>8.1f} Mbps | {errors} send errors"
                )
                last_report = now

    except KeyboardInterrupt:
        elapsed = time.monotonic() - t0
        print(f"\nDone: {sent:,} packets in {elapsed:.1f}s = {sent/elapsed:,.0f} pps avg")

    for s in sockets:
        s.close()


def main():
    parser = argparse.ArgumentParser(
        description="UDP traffic generator for dpdk-top testing",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # Single ENI
  python3 send_traffic.py --target 34.239.224.188 --pps 10000

  # Single ENI with RSS queue spread
  python3 send_traffic.py --target 34.239.224.188 --pps 10000 --rss

  # All 3 DPDK ENIs simultaneously (5K pps each = 15K pps total)
  python3 send_traffic.py --multi --pps 5000

  # All 3 ENIs with RSS spread (run from inside the VPC or EC2)
  python3 send_traffic.py --multi --pps 10000 --rss

  # Custom targets
  python3 send_traffic.py --multi --targets 10.0.0.1 10.0.0.2 10.0.0.3 --pps 5000
        """
    )
    parser.add_argument("--target",    default="34.239.224.188",
                        help="Destination IP for single-target mode (default: 34.239.224.188)")
    parser.add_argument("--multi",     action="store_true",
                        help="Send to all 3 DPDK ENIs simultaneously (one thread per ENI)")
    parser.add_argument("--targets",   nargs="+", metavar="IP",
                        help=f"Override ENI IPs for --multi (default: {' '.join(DEFAULT_TARGETS)})")
    parser.add_argument("--port",      type=int, default=9999,
                        help="Base destination UDP port (default: 9999)")
    parser.add_argument("--pps",       type=int, default=5000,
                        help="Target packets per second per ENI (default: 5000)")
    parser.add_argument("--size",      type=int, default=512,
                        help="Payload size in bytes (default: 512)")
    parser.add_argument("--rss",       action="store_true",
                        help="Bind multiple sockets to distinct src ports for RSS queue spread")
    parser.add_argument("--flows",     type=int, default=64,
                        help="Number of distinct src-port sockets in RSS mode (default: 64)")
    parser.add_argument("--dst-ports", type=int, default=64,
                        help="Number of distinct dst ports in RSS mode (default: 64)")
    args = parser.parse_args()

    if args.multi:
        targets = args.targets if args.targets else DEFAULT_TARGETS
        run_multi(targets, args.pps, args.size, args.port, args.rss, args.flows, args.dst_ports)
    else:
        run_single(args.target, args.pps, args.size, args.port, args.rss, args.flows, args.dst_ports)


if __name__ == "__main__":
    main()
