//! Mempool data structures.

use serde::Deserialize;

/// Mempool info from /mempool/info.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct MempoolInfo {
    #[serde(default)]
    pub name: String,
    #[serde(rename = "count", default)]
    pub size: u64,
    #[serde(rename = "free_count", default)]
    pub free_count: u64,
    #[serde(default)]
    pub cache_size: u32,
    #[serde(default)]
    pub element_size: u32,
    #[serde(default)]
    pub flags: u32,
}

/// In-use count: size - free_count. We also get it from some endpoints as "count" meaning in-use.
#[derive(Clone, Debug, Default)]
pub struct MempoolState {
    pub name: String,
    pub size: u64,
    pub in_use: u64,
    pub free_count: u64,
    pub cache_size: u32,
    pub element_size: u32,
    pub utilization_pct: f64,
}

impl MempoolState {
    pub fn from_info(info: &MempoolInfo) -> Self {
        let in_use = info.size.saturating_sub(info.free_count);
        let utilization_pct = if info.size > 0 {
            (in_use as f64 / info.size as f64) * 100.0
        } else {
            0.0
        };
        Self {
            name: info.name.clone(),
            size: info.size,
            in_use,
            free_count: info.free_count,
            cache_size: info.cache_size,
            element_size: info.element_size,
            utilization_pct,
        }
    }
}
