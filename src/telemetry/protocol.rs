//! Telemetry JSON request/response types and parsing.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;

use crate::model::mempool::MempoolInfo;
use crate::model::port::{LinkStatus, PortInfo, PortStats};

/// Response from /ethdev/list: { "/ethdev/list": [0, 1, ...] }. Key may have leading space.
pub fn parse_ethdev_list(json: &str) -> Result<Vec<u16>> {
    let map: HashMap<String, serde_json::Value> =
        serde_json::from_str(json).context("Parse /ethdev/list")?;
    let v = map
        .get("/ethdev/list")
        .or_else(|| {
            map.keys()
                .find(|k| k.trim().contains("ethdev/list"))
                .and_then(|k| map.get(k))
        })
        .and_then(|val| val.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_u64().map(|n| n as u16))
                .collect::<Vec<u16>>()
        })
        .unwrap_or_default();
    Ok(v)
}

/// Response from /ethdev/info,<port>. Wrapped as { "/ethdev/info,0": { ... } }.
pub fn parse_ethdev_info(json: &str, port_id: u16) -> Result<PortInfo> {
    let key = format!("/ethdev/info,{}", port_id);
    let map: HashMap<String, EthdevInfoRaw> =
        serde_json::from_str(json).context("Parse ethdev info")?;
    let raw = map
        .get(&key)
        .or_else(|| map.get("/ethdev/info"))
        .context("ethdev info key not found")?;

    let link_status = if raw.link_status.as_deref() == Some("up") || raw.dev_started == Some(1) {
        LinkStatus::Up
    } else if raw.link_status.as_deref() == Some("down") {
        LinkStatus::Down
    } else {
        LinkStatus::Unknown
    };

    Ok(PortInfo {
        name: raw.device_name.clone().unwrap_or_default(),
        pci: raw
            .device_name
            .clone()
            .unwrap_or_else(|| format!("port_{}", port_id)),
        driver: raw.driver_name.clone().unwrap_or_default(),
        mac: raw.mac_addr.clone().unwrap_or_default(),
        mtu: raw.mtu.unwrap_or(0),
        link_speed_mbps: raw.link_speed.unwrap_or(0),
        link_status,
        nb_rx_queues: raw.nb_rx_queues.unwrap_or(0),
        nb_tx_queues: raw.nb_tx_queues.unwrap_or(0),
    })
}

#[derive(Deserialize)]
struct EthdevInfoRaw {
    #[serde(alias = "name")]
    device_name: Option<String>,
    driver_name: Option<String>,
    mac_addr: Option<String>,
    mtu: Option<u16>,
    link_speed: Option<u32>,
    link_status: Option<String>,
    dev_started: Option<u8>,
    nb_rx_queues: Option<u16>,
    nb_tx_queues: Option<u16>,
}

/// Response from /ethdev/stats,<port>.
pub fn parse_ethdev_stats(json: &str, port_id: u16) -> Result<PortStats> {
    let key = format!("/ethdev/stats,{}", port_id);
    let map: HashMap<String, PortStats> =
        serde_json::from_str(json).context("Parse ethdev stats")?;
    let stats = map
        .get(&key)
        .or_else(|| map.get("/ethdev/stats"))
        .context("ethdev stats key not found")?;
    Ok(stats.clone())
}

/// Response from /ethdev/xstats,<port>.
/// Handles two formats:
///   - Array: [{"name":"rx_good_packets","value":123}, ...]  (DPDK 21.x+)
///   - Dict:  {"rx_good_packets": 123, ...}                  (ENA and some PMDs)
pub fn parse_ethdev_xstats(json: &str) -> Result<Vec<(String, u64)>> {
    let map: HashMap<String, serde_json::Value> =
        serde_json::from_str(json).context("Parse xstats")?;

    let val = map
        .get("/ethdev/xstats")
        .or_else(|| {
            map.keys()
                .find(|k| k.contains("ethdev/xstats"))
                .and_then(|k| map.get(k))
        })
        .context("xstats key not found")?;

    match val {
        serde_json::Value::Array(arr) => Ok(arr
            .iter()
            .filter_map(|entry| {
                let name = entry.get("name")?.as_str()?;
                let value = entry.get("value")?.as_u64()?;
                Some((name.to_string(), value))
            })
            .collect()),
        serde_json::Value::Object(obj) => Ok(obj
            .iter()
            .filter_map(|(name, val)| {
                let value = val.as_u64()?;
                Some((name.clone(), value))
            })
            .collect()),
        _ => Ok(Vec::new()),
    }
}

/// Response from /mempool/list: { "/mempool/list": ["name1", ...] }. Key may have leading space.
pub fn parse_mempool_list(json: &str) -> Result<Vec<String>> {
    let map: HashMap<String, serde_json::Value> =
        serde_json::from_str(json).context("Parse /mempool/list")?;
    let v = map
        .get("/mempool/list")
        .or_else(|| {
            map.keys()
                .find(|k| k.trim().contains("mempool/list"))
                .and_then(|k| map.get(k))
        })
        .and_then(|val| val.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect::<Vec<String>>()
        })
        .unwrap_or_default();
    Ok(v)
}

/// Response from /mempool/info,<name>.
pub fn parse_mempool_info(json: &str, name: &str) -> Result<MempoolInfo> {
    let key = format!("/mempool/info,{}", name);
    let map: HashMap<String, MempoolInfoRaw> =
        serde_json::from_str(json).context("Parse mempool info")?;
    let raw = map
        .get(&key)
        .or_else(|| map.get("/mempool/info"))
        .context("mempool info key not found")?;

    let size = raw.size.or(raw.count).or(raw.populated_size).unwrap_or(0);
    let free_count = raw
        .free_count
        .or_else(|| {
            let pool = raw.common_pool_count.unwrap_or(0);
            let cache = raw.total_cache_count.unwrap_or(0);
            Some(pool + cache)
        })
        .unwrap_or(0);

    Ok(MempoolInfo {
        name: name.to_string(),
        size,
        free_count,
        cache_size: raw.cache_size.unwrap_or(0),
        element_size: raw.elt_size.or(raw.element_size).unwrap_or(0),
        flags: raw.flags.unwrap_or(0),
    })
}

#[derive(Deserialize)]
struct MempoolInfoRaw {
    size: Option<u64>,
    #[serde(rename = "count")]
    count: Option<u64>,
    populated_size: Option<u64>,
    free_count: Option<u64>,
    common_pool_count: Option<u64>,
    total_cache_count: Option<u64>,
    cache_size: Option<u32>,
    elt_size: Option<u32>,
    element_size: Option<u32>,
    flags: Option<u32>,
}
