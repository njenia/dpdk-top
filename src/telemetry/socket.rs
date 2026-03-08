//! Unix socket connection to DPDK telemetry.
//! DPDK uses SOCK_SEQPACKET; we try seqpacket first and fall back to stream (e.g. macOS).

use anyhow::{Context, Result};
use std::io::{Read, Write};
use std::path::Path;

enum TelemetrySocketInner {
    Seqpacket(uds::UnixSeqpacketConn),
    Stream(std::os::unix::net::UnixStream),
}

pub struct TelemetrySocket {
    inner: TelemetrySocketInner,
}

impl TelemetrySocket {
    pub fn connect(path: &Path) -> Result<Self> {
        let mut sock = {
            #[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "netbsd"))]
            {
                if let Ok(conn) = uds::UnixSeqpacketConn::connect(path) {
                    let _ = conn.set_read_timeout(Some(std::time::Duration::from_secs(5)));
                    let _ = conn.set_write_timeout(Some(std::time::Duration::from_secs(5)));
                    Self {
                        inner: TelemetrySocketInner::Seqpacket(conn),
                    }
                } else {
                    let stream =
                        std::os::unix::net::UnixStream::connect(path).with_context(|| {
                            format!("Failed to connect to telemetry socket: {}", path.display())
                        })?;
                    stream.set_read_timeout(Some(std::time::Duration::from_secs(5)))?;
                    stream.set_write_timeout(Some(std::time::Duration::from_secs(5)))?;
                    Self {
                        inner: TelemetrySocketInner::Stream(stream),
                    }
                }
            }
            #[cfg(not(any(target_os = "linux", target_os = "freebsd", target_os = "netbsd")))]
            {
                let stream = std::os::unix::net::UnixStream::connect(path).with_context(|| {
                    format!("Failed to connect to telemetry socket: {}", path.display())
                })?;
                stream.set_read_timeout(Some(std::time::Duration::from_secs(5)))?;
                stream.set_write_timeout(Some(std::time::Duration::from_secs(5)))?;
                Self {
                    inner: TelemetrySocketInner::Stream(stream),
                }
            }
        };

        sock.read_banner()?;
        Ok(sock)
    }

    /// DPDK sends a banner on connect, e.g. {"version":"DPDK 20.05.0","pid":123,"max_output_len":16384}. Consume it.
    fn read_banner(&mut self) -> Result<()> {
        let mut buf = [0u8; 256];
        match &mut self.inner {
            TelemetrySocketInner::Seqpacket(conn) => {
                let _ = conn.recv(&mut buf).context("Read telemetry banner")?;
            }
            TelemetrySocketInner::Stream(stream) => {
                let _ = stream.read(&mut buf).context("Read telemetry banner")?;
            }
        }
        Ok(())
    }

    /// Send a command (e.g. "/ethdev/list") and receive JSON response.
    /// Max response size 64KB per DPDK.
    pub fn request(&mut self, cmd: &str) -> Result<String> {
        match &mut self.inner {
            TelemetrySocketInner::Seqpacket(conn) => {
                conn.send(cmd.as_bytes())
                    .context("Write telemetry command")?;
                let mut buf = vec![0u8; 65536];
                let n = conn.recv(&mut buf).context("Read telemetry response")?;
                let s = std::str::from_utf8(&buf[..n]).context("Telemetry response not UTF-8")?;
                Ok(s.to_string())
            }
            TelemetrySocketInner::Stream(stream) => {
                stream
                    .write_all(cmd.as_bytes())
                    .context("Write telemetry command")?;
                stream.flush()?;
                let mut buf = vec![0u8; 65536];
                let n = stream.read(&mut buf).context("Read telemetry response")?;
                let s = std::str::from_utf8(&buf[..n]).context("Telemetry response not UTF-8")?;
                Ok(s.to_string())
            }
        }
    }
}
