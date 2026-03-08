#!/bin/bash
set -e
# Keep testpmd stdin open so it stays in interactive mode (FIFO reader blocks until writer closes).
# Writer and testpmd must survive SIGHUP when we exec dpdk-top (use nohup + disown).
# Port 0: net_tap0 (dtap0) — we generate traffic to it from inside the container so dpdk-top sees RX.
# Port 1: net_null (no real peer).
mkdir -p /tmp/dpdk
mkfifo -m 600 /tmp/dpdk/testpmd_stdin 2>/dev/null || true
nohup sh -c 'exec 3>/tmp/dpdk/testpmd_stdin; sleep 99999' </dev/null &>/dev/null &
disown 2>/dev/null || true
nohup dpdk-testpmd -l 0-1 -n 2 --no-huge -m 256 --no-pci \
  --vdev=net_tap0,iface=dtap0 --vdev=net_null1 \
  -- -i --no-flush-rx --total-num-mbufs=8192 </tmp/dpdk/testpmd_stdin >>/tmp/testpmd.log 2>&1 &
TESTPMD_PID=$!
disown 2>/dev/null || true
# Wait for socket to appear (DPDK may use default path or file-prefix subdir)
SOCK=""
for i in $(seq 1 15); do
  SOCK=$(find /var/run/dpdk -name 'dpdk_telemetry.v2*' -type s 2>/dev/null | head -1)
  if [ -n "$SOCK" ] && [ -d /proc/"$TESTPMD_PID" ] 2>/dev/null; then
    sleep 1
    [ -d /proc/"$TESTPMD_PID" ] 2>/dev/null && break
  fi
  sleep 1
done
if [ -z "$SOCK" ] || ! [ -S "$SOCK" ]; then
  echo "Telemetry socket did not appear. testpmd may have failed."
  kill $TESTPMD_PID 2>/dev/null || true
  exit 1
fi
# Give testpmd time to create dtap0
sleep 2
# Configure TAP so we can send traffic to it (kernel will put packets on dtap0, testpmd reads them)
if [ -d /sys/class/net/dtap0 ]; then
  ip addr add 10.0.0.1/24 dev dtap0 2>/dev/null || true
  ip link set dtap0 up 2>/dev/null || true
  # Generate constant traffic to 10.0.0.1 so testpmd sees RX on port 0
  nohup sh -c 'while true; do ping -c 200 -i 0.2 10.0.0.1 2>/dev/null; done' </dev/null &>/dev/null &
  disown 2>/dev/null || true
fi
# Forward mode: io = receive on one port, send to other
printf 'set fwd io\n' >/tmp/dpdk/testpmd_stdin 2>/dev/null || true
sleep 1
printf 'start\n' >/tmp/dpdk/testpmd_stdin 2>/dev/null || true
sleep 1
# Run dpdk-top in foreground. Do not use exec: the shell must stay as parent
# so background testpmd and fifo-writer don't get SIGHUP and exit.
dpdk-top -s "$SOCK"
