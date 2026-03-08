//! Auto-detect DPDK telemetry socket paths.

use anyhow::Result;
use std::path::PathBuf;

#[cfg(unix)]
use std::os::unix::fs::FileTypeExt;

const SOCKET_NAME: &str = "dpdk_telemetry.v2";

fn is_socket(path: &std::path::Path) -> bool {
    #[cfg(unix)]
    {
        path.symlink_metadata()
            .map(|m| m.file_type().is_socket())
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        false
    }
}

fn has_telemetry_name(path: &std::path::Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|name| name == SOCKET_NAME || name.starts_with("dpdk_telemetry.v2"))
        .unwrap_or(false)
}

/// Scan /var/run/dpdk/ and ~/.dpdk/ for dpdk_telemetry.v2 sockets.
pub fn discover_sockets() -> Result<Vec<PathBuf>> {
    let mut found = Vec::new();

    let search_dirs = ["/var/run/dpdk", "/run/dpdk"];

    for base in &search_dirs {
        if let Ok(entries) = std::fs::read_dir(base) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if let Ok(sub) = std::fs::read_dir(&path) {
                        for e in sub.flatten() {
                            let p = e.path();
                            if has_telemetry_name(&p) && is_socket(&p) {
                                found.push(p);
                            }
                        }
                    }
                }
            }
        }
    }

    if let Some(home) = dirs::home_dir() {
        let fallback = home.join(".dpdk").join("rte").join(SOCKET_NAME);
        if is_socket(&fallback) && !found.contains(&fallback) {
            found.push(fallback);
        }
    }

    // Resolve symlinks to deduplicate (e.g. /run/dpdk and /var/run/dpdk are the same on Linux)
    let mut resolved = Vec::new();
    for p in found {
        let canonical = p.canonicalize().unwrap_or_else(|_| p.clone());
        if !resolved
            .iter()
            .any(|(_, c): &(PathBuf, PathBuf)| c == &canonical)
        {
            resolved.push((p, canonical));
        }
    }
    let mut result: Vec<PathBuf> = resolved.into_iter().map(|(orig, _)| orig).collect();
    result.sort();
    Ok(result)
}
