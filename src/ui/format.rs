//! Human-readable formatting for rates and sizes.

/// Format packets/sec or similar: 847, 312.4K, 1.24M, 1.24G.
pub fn format_rate(rate: f64) -> String {
    if rate < 0.0 {
        return "--".to_string();
    }
    if (0.0..1.0).contains(&rate) {
        return format!("{:.0}", rate);
    }
    if rate < 1_000.0 {
        return format!("{:.0}", rate);
    }
    if rate < 1_000_000.0 {
        return format!("{:.1}K", rate / 1_000.0);
    }
    if rate < 1_000_000_000.0 {
        return format!("{:.2}M", rate / 1_000_000.0);
    }
    format!("{:.2}G", rate / 1_000_000_000.0)
}

/// Format bits per second as Mbps or Gbps.
pub fn format_bps(bps: f64) -> String {
    if bps < 0.0 {
        return "--".to_string();
    }
    let mbps = bps / 1_000_000.0;
    if mbps >= 1000.0 {
        format!("{:.2} Gbps", bps / 1_000_000_000.0)
    } else {
        format!("{:.0} Mbps", mbps)
    }
}

/// Format integer with thousands separator.
pub fn format_int(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out.chars().rev().collect()
}
