//! Unit tests for dpdk-top core logic.
//!
//! These tests validate parsing, formatting, rate computation, and data
//! structures without requiring a live DPDK process or Unix socket.

mod format {
    use dpdk_top::ui::format::{format_bps, format_int, format_rate};

    #[test]
    fn format_rate_ranges() {
        assert_eq!(format_rate(-1.0), "--");
        assert_eq!(format_rate(0.0), "0");
        assert_eq!(format_rate(42.0), "42");
        assert_eq!(format_rate(999.0), "999");
        assert_eq!(format_rate(1_500.0), "1.5K");
        assert_eq!(format_rate(12_345.0), "12.3K");
        assert_eq!(format_rate(999_999.0), "1000.0K");
        assert_eq!(format_rate(1_234_567.0), "1.23M");
        assert_eq!(format_rate(1_234_567_890.0), "1.23G");
    }

    #[test]
    fn format_bps_ranges() {
        assert_eq!(format_bps(-1.0), "--");
        assert_eq!(format_bps(0.0), "0 Mbps");
        assert_eq!(format_bps(500_000_000.0), "500 Mbps");
        assert_eq!(format_bps(1_000_000_000.0), "1.00 Gbps");
        assert_eq!(format_bps(10_000_000_000.0), "10.00 Gbps");
        assert_eq!(format_bps(100_000_000_000.0), "100.00 Gbps");
    }

    #[test]
    fn format_rate_at_boundary_1000() {
        let s = format_rate(1000.0);
        assert_eq!(s, "1.0K");
    }

    #[test]
    fn format_rate_at_boundary_1m() {
        let s = format_rate(1_000_000.0);
        assert_eq!(s, "1.00M");
    }

    #[test]
    fn format_int_thousands_separator() {
        assert_eq!(format_int(0), "0");
        assert_eq!(format_int(5), "5");
        assert_eq!(format_int(42), "42");
        assert_eq!(format_int(999), "999");
        assert_eq!(format_int(1_000), "1,000");
        assert_eq!(format_int(1_234), "1,234");
        assert_eq!(format_int(16_384), "16,384");
        assert_eq!(format_int(100_000), "100,000");
        assert_eq!(format_int(1_234_567), "1,234,567");
        assert_eq!(format_int(1_234_567_890), "1,234,567,890");
    }
}

mod rates {
    use dpdk_telemetry::rates::{compute_port_rates, compute_queue_rates, delta, smooth_rate};
    use dpdk_telemetry::model::port::{PortRates, PortStats};

    #[test]
    fn delta_normal() {
        assert_eq!(delta(100, 50), 50);
        assert_eq!(delta(0, 0), 0);
        assert_eq!(delta(1_000_000, 999_999), 1);
    }

    #[test]
    fn delta_wraps_on_overflow() {
        assert_eq!(delta(0, u64::MAX), 1);
        assert_eq!(delta(5, u64::MAX - 4), 10);
        assert_eq!(delta(0, u64::MAX - 99), 100);
    }

    #[test]
    fn smooth_rate_alpha_1_is_raw() {
        assert!((smooth_rate(500.0, 100.0, 1.0) - 500.0).abs() < 1e-6);
    }

    #[test]
    fn smooth_rate_alpha_0_keeps_previous() {
        assert!((smooth_rate(500.0, 100.0, 0.0) - 100.0).abs() < 1e-6);
    }

    #[test]
    fn smooth_rate_alpha_half_averages() {
        assert!((smooth_rate(100.0, 0.0, 0.5) - 50.0).abs() < 1e-6);
    }

    #[test]
    fn compute_port_rates_basic() {
        let prev = PortStats {
            ipackets: 1000,
            opackets: 500,
            ibytes: 500_000,
            obytes: 250_000,
            ..Default::default()
        };
        let curr = PortStats {
            ipackets: 2000,
            opackets: 600,
            ibytes: 1_000_000,
            obytes: 300_000,
            ..Default::default()
        };
        let rates = compute_port_rates(&curr, &prev, 1.0, &PortRates::default(), 1.0);

        assert!((rates.rx_pps - 1000.0).abs() < 1e-6);
        assert!((rates.tx_pps - 100.0).abs() < 1e-6);
        assert!((rates.rx_bps - 4_000_000.0).abs() < 1e-6); // 500KB * 8
        assert!((rates.tx_bps - 400_000.0).abs() < 1e-6);
    }

    #[test]
    fn compute_port_rates_with_elapsed() {
        let prev = PortStats {
            ipackets: 0,
            ..Default::default()
        };
        let curr = PortStats {
            ipackets: 5000,
            ..Default::default()
        };
        let rates = compute_port_rates(&curr, &prev, 2.0, &PortRates::default(), 1.0);
        assert!((rates.rx_pps - 2500.0).abs() < 1e-6);
    }

    #[test]
    fn compute_queue_rates_parses_queue_names() {
        let prev = vec![
            ("rx_q0_packets".to_string(), 100u64),
            ("rx_q0_bytes".to_string(), 50_000u64),
            ("rx_q1_packets".to_string(), 200u64),
            ("tx_q0_packets".to_string(), 50u64),
        ];
        let curr = vec![
            ("rx_q0_packets".to_string(), 200u64),
            ("rx_q0_bytes".to_string(), 100_000u64),
            ("rx_q1_packets".to_string(), 500u64),
            ("tx_q0_packets".to_string(), 150u64),
        ];

        let queues = compute_queue_rates(&curr, &prev, 1.0, 4, 1.0);

        assert_eq!(queues.len(), 4);
        assert!((queues[0].rx_pps - 100.0).abs() < 1e-6);
        assert!((queues[0].rx_bps - 400_000.0).abs() < 1e-6); // 50KB * 8
        assert!((queues[1].rx_pps - 300.0).abs() < 1e-6);
        assert!((queues[0].tx_pps - 100.0).abs() < 1e-6);
        // Q2 and Q3 should be zero
        assert!((queues[2].rx_pps).abs() < 1e-6);
        assert!((queues[3].rx_pps).abs() < 1e-6);
    }

    #[test]
    fn compute_port_rates_includes_error_fields() {
        let prev = PortStats {
            imissed: 100,
            rx_nombuf: 0,
            ierrors: 10,
            oerrors: 5,
            ..Default::default()
        };
        let curr = PortStats {
            imissed: 200,
            rx_nombuf: 50,
            ierrors: 30,
            oerrors: 15,
            ..Default::default()
        };
        let rates = compute_port_rates(&curr, &prev, 1.0, &PortRates::default(), 1.0);

        assert!((rates.rx_missed_pps - 100.0).abs() < 1e-6);
        assert!((rates.rx_nombuf_pps - 50.0).abs() < 1e-6);
        assert!((rates.ierrors_pps - 20.0).abs() < 1e-6);
        assert!((rates.oerrors_pps - 10.0).abs() < 1e-6);
    }

    #[test]
    fn compute_port_rates_counter_wrap() {
        let prev = PortStats {
            ipackets: u64::MAX - 99,
            ..Default::default()
        };
        let curr = PortStats {
            ipackets: 100,
            ..Default::default()
        };
        let rates = compute_port_rates(&curr, &prev, 1.0, &PortRates::default(), 1.0);
        assert!((rates.rx_pps - 200.0).abs() < 1e-6);
    }

    #[test]
    fn ema_smoothing_across_multiple_polls() {
        let alpha = 0.3;
        let prev = PortStats::default();
        let curr = PortStats {
            ipackets: 1000,
            ..Default::default()
        };

        let rates1 = compute_port_rates(&curr, &prev, 1.0, &PortRates::default(), alpha);
        // raw = 1000, smoothed = 0.3 * 1000 + 0.7 * 0 = 300
        assert!((rates1.rx_pps - 300.0).abs() < 1e-6);

        let curr2 = PortStats {
            ipackets: 2000,
            ..Default::default()
        };
        let rates2 = compute_port_rates(&curr2, &curr, 1.0, &rates1, alpha);
        // raw = 1000, smoothed = 0.3 * 1000 + 0.7 * 300 = 300 + 210 = 510
        assert!((rates2.rx_pps - 510.0).abs() < 1e-6);
    }

    #[test]
    fn compute_queue_rates_tx_bytes() {
        let prev = vec![
            ("tx_q0_bytes".to_string(), 1_000_000u64),
            ("tx_q0_packets".to_string(), 100u64),
        ];
        let curr = vec![
            ("tx_q0_bytes".to_string(), 2_000_000u64),
            ("tx_q0_packets".to_string(), 200u64),
        ];
        let queues = compute_queue_rates(&curr, &prev, 1.0, 2, 1.0);
        assert!((queues[0].tx_bps - 8_000_000.0).abs() < 1e-6); // 1MB * 8
        assert!((queues[0].tx_pps - 100.0).abs() < 1e-6);
    }

    #[test]
    fn compute_queue_rates_ignores_oob_queue_ids() {
        let prev = vec![("rx_q99_packets".to_string(), 0u64)];
        let curr = vec![("rx_q99_packets".to_string(), 1000u64)];
        let queues = compute_queue_rates(&curr, &prev, 1.0, 4, 1.0);
        assert_eq!(queues.len(), 4);
        assert!(queues.iter().all(|q| q.rx_pps < 1e-6));
    }

    #[test]
    fn compute_queue_rates_missing_previous_stat() {
        let prev: Vec<(String, u64)> = vec![];
        let curr = vec![("rx_q0_packets".to_string(), 500u64)];
        let queues = compute_queue_rates(&curr, &prev, 1.0, 2, 1.0);
        assert!((queues[0].rx_pps - 500.0).abs() < 1e-6);
    }
}

mod protocol {
    use dpdk_telemetry::protocol::*;

    #[test]
    fn parse_ethdev_list_basic() {
        let json = r#"{"/ethdev/list": [0, 1, 2]}"#;
        let ids = parse_ethdev_list(json).unwrap();
        assert_eq!(ids, vec![0, 1, 2]);
    }

    #[test]
    fn parse_ethdev_list_empty() {
        let json = r#"{"/ethdev/list": []}"#;
        let ids = parse_ethdev_list(json).unwrap();
        assert!(ids.is_empty());
    }

    #[test]
    fn parse_ethdev_stats_basic() {
        let json = r#"{"/ethdev/stats,0": {
            "ipackets": 12345,
            "opackets": 6789,
            "ibytes": 1234500,
            "obytes": 678900,
            "imissed": 0,
            "ierrors": 0,
            "oerrors": 0,
            "rx_nombuf": 0
        }}"#;
        let stats = parse_ethdev_stats(json, 0).unwrap();
        assert_eq!(stats.ipackets, 12345);
        assert_eq!(stats.opackets, 6789);
        assert_eq!(stats.ibytes, 1234500);
    }

    #[test]
    fn parse_ethdev_stats_missing_fields_default_to_zero() {
        let json = r#"{"/ethdev/stats,0": {"ipackets": 100}}"#;
        let stats = parse_ethdev_stats(json, 0).unwrap();
        assert_eq!(stats.ipackets, 100);
        assert_eq!(stats.opackets, 0);
        assert_eq!(stats.ibytes, 0);
    }

    #[test]
    fn parse_ethdev_info_link_up() {
        let json = r#"{"/ethdev/info,0": {
            "name": "0000:00:06.0",
            "driver_name": "net_ena",
            "mac_addr": "0E:06:DC:99:DC:0B",
            "mtu": 1500,
            "link_speed": 10000,
            "link_status": "up",
            "nb_rx_queues": 4,
            "nb_tx_queues": 4
        }}"#;
        let info = parse_ethdev_info(json, 0).unwrap();
        assert_eq!(info.name, "0000:00:06.0");
        assert_eq!(info.driver, "net_ena");
        assert_eq!(info.nb_rx_queues, 4);
        assert_eq!(info.link_status, dpdk_telemetry::model::port::LinkStatus::Up);
    }

    #[test]
    fn parse_ethdev_info_dev_started_as_proxy() {
        let json = r#"{"/ethdev/info,0": {
            "name": "test_port",
            "dev_started": 1
        }}"#;
        let info = parse_ethdev_info(json, 0).unwrap();
        assert_eq!(info.link_status, dpdk_telemetry::model::port::LinkStatus::Up);
    }

    #[test]
    fn parse_ethdev_info_missing_fields() {
        let json = r#"{"/ethdev/info,0": {}}"#;
        let info = parse_ethdev_info(json, 0).unwrap();
        assert_eq!(info.link_status, dpdk_telemetry::model::port::LinkStatus::Unknown);
        assert_eq!(info.nb_rx_queues, 0);
        assert_eq!(info.mtu, 0);
    }

    #[test]
    fn parse_xstats_array_format() {
        let json = r#"{"/ethdev/xstats,0": [
            {"name": "rx_good_packets", "value": 12345},
            {"name": "rx_q0_packets", "value": 5000},
            {"name": "tx_good_packets", "value": 100}
        ]}"#;
        let xstats = parse_ethdev_xstats(json).unwrap();
        assert_eq!(xstats.len(), 3);
        assert!(xstats
            .iter()
            .any(|(n, v)| n == "rx_good_packets" && *v == 12345));
        assert!(xstats
            .iter()
            .any(|(n, v)| n == "rx_q0_packets" && *v == 5000));
    }

    #[test]
    fn parse_xstats_dict_format() {
        let json = r#"{"/ethdev/xstats,0": {
            "rx_good_packets": 12345,
            "rx_q0_packets": 5000,
            "tx_good_packets": 100
        }}"#;
        let xstats = parse_ethdev_xstats(json).unwrap();
        assert_eq!(xstats.len(), 3);
        assert!(xstats
            .iter()
            .any(|(n, v)| n == "rx_good_packets" && *v == 12345));
    }

    #[test]
    fn parse_mempool_list_basic() {
        let json = r#"{"/mempool/list": ["mb_pool_0", "mb_pool_1"]}"#;
        let names = parse_mempool_list(json).unwrap();
        assert_eq!(names, vec!["mb_pool_0", "mb_pool_1"]);
    }

    #[test]
    fn parse_mempool_info_with_size_and_free() {
        let json = r#"{"/mempool/info,mb_pool_0": {
            "size": 16384,
            "free_count": 12000,
            "cache_size": 256,
            "elt_size": 2176
        }}"#;
        let info = parse_mempool_info(json, "mb_pool_0").unwrap();
        assert_eq!(info.size, 16384);
        assert_eq!(info.free_count, 12000);
        assert_eq!(info.element_size, 2176);
    }

    #[test]
    fn parse_mempool_info_with_count_and_common_pool() {
        let json = r#"{"/mempool/info,mb_pool_0": {
            "count": 16384,
            "common_pool_count": 10000,
            "total_cache_count": 2000,
            "cache_size": 256,
            "element_size": 2176
        }}"#;
        let info = parse_mempool_info(json, "mb_pool_0").unwrap();
        assert_eq!(info.size, 16384);
        assert_eq!(info.free_count, 12000); // 10000 + 2000
    }

    #[test]
    fn parse_ethdev_info_link_down() {
        let json = r#"{"/ethdev/info,0": {
            "name": "0000:00:07.0",
            "link_status": "down",
            "nb_rx_queues": 2,
            "nb_tx_queues": 2
        }}"#;
        let info = parse_ethdev_info(json, 0).unwrap();
        assert_eq!(info.link_status, dpdk_telemetry::model::port::LinkStatus::Down);
        assert_eq!(info.nb_rx_queues, 2);
    }

    #[test]
    fn parse_ethdev_list_leading_space_key() {
        let json = r#"{ " /ethdev/list": [0, 3, 5]}"#;
        let ids = parse_ethdev_list(json).unwrap();
        assert_eq!(ids, vec![0, 3, 5]);
    }

    #[test]
    fn parse_mempool_list_leading_space_key() {
        let json = r#"{ " /mempool/list": ["pool_a"]}"#;
        let names = parse_mempool_list(json).unwrap();
        assert_eq!(names, vec!["pool_a"]);
    }

    #[test]
    fn parse_mempool_info_populated_size_fallback() {
        let json = r#"{"/mempool/info,pool_x": {
            "populated_size": 8192,
            "free_count": 4096,
            "elt_size": 2048
        }}"#;
        let info = parse_mempool_info(json, "pool_x").unwrap();
        assert_eq!(info.size, 8192);
        assert_eq!(info.free_count, 4096);
        assert_eq!(info.element_size, 2048);
    }

    #[test]
    fn parse_xstats_non_array_non_dict_returns_empty() {
        let json = r#"{"/ethdev/xstats,0": "unexpected_string"}"#;
        let xstats = parse_ethdev_xstats(json).unwrap();
        assert!(xstats.is_empty());
    }

    #[test]
    fn parse_ethdev_stats_large_counters() {
        let json = r#"{"/ethdev/stats,0": {
            "ipackets": 18446744073709551000,
            "opackets": 0,
            "ibytes": 18446744073709551000,
            "obytes": 0,
            "imissed": 0,
            "ierrors": 0,
            "oerrors": 0,
            "rx_nombuf": 0
        }}"#;
        let stats = parse_ethdev_stats(json, 0).unwrap();
        assert!(stats.ipackets > 1_000_000_000_000);
    }

    #[test]
    fn parse_ethdev_list_malformed_returns_error() {
        let result = parse_ethdev_list("not json at all");
        assert!(result.is_err());
    }

    #[test]
    fn parse_mempool_info_no_free_count_uses_pool_counts() {
        let json = r#"{"/mempool/info,p": {
            "size": 1024,
            "common_pool_count": 500,
            "total_cache_count": 100
        }}"#;
        let info = parse_mempool_info(json, "p").unwrap();
        assert_eq!(info.size, 1024);
        assert_eq!(info.free_count, 600); // 500 + 100
    }
}

mod history {
    use dpdk_telemetry::history::RingBuffer;

    #[test]
    fn empty_ring_buffer() {
        let r: RingBuffer<u32, 4> = RingBuffer::new();
        assert_eq!(r.len(), 0);
        assert_eq!(r.capacity(), 4);
        assert_eq!(r.iter().count(), 0);
    }

    #[test]
    fn push_within_capacity() {
        let mut r: RingBuffer<u32, 4> = RingBuffer::new();
        r.push(10);
        r.push(20);
        r.push(30);
        assert_eq!(r.len(), 3);
        let v: Vec<u32> = r.iter().copied().collect();
        assert_eq!(v, vec![10, 20, 30]);
    }

    #[test]
    fn push_wraps_around() {
        let mut r: RingBuffer<u32, 3> = RingBuffer::new();
        r.push(1);
        r.push(2);
        r.push(3);
        r.push(4);
        assert_eq!(r.len(), 3);
        let v: Vec<u32> = r.iter().copied().collect();
        assert_eq!(v, vec![2, 3, 4]);
    }

    #[test]
    fn push_many_wraps() {
        let mut r: RingBuffer<u32, 2> = RingBuffer::new();
        for i in 0..100 {
            r.push(i);
        }
        assert_eq!(r.len(), 2);
        let v: Vec<u32> = r.iter().copied().collect();
        assert_eq!(v, vec![98, 99]);
    }

    #[test]
    fn last_n_returns_newest() {
        let mut r: RingBuffer<u32, 5> = RingBuffer::new();
        for i in 1..=10 {
            r.push(i);
        }
        let last3: Vec<u32> = r.last_n(3).iter().map(|&&v| v).collect();
        assert_eq!(last3, vec![8, 9, 10]);
    }

    #[test]
    fn last_n_more_than_len() {
        let mut r: RingBuffer<u32, 10> = RingBuffer::new();
        r.push(1);
        r.push(2);
        let last = r.last_n(100);
        assert_eq!(last.len(), 2);
    }

    #[test]
    fn copy_last_n_works() {
        let mut r: RingBuffer<u32, 4> = RingBuffer::new();
        r.push(10);
        r.push(20);
        r.push(30);
        r.push(40);
        r.push(50);
        let mut out = [0u32; 3];
        r.copy_last_n(&mut out);
        assert_eq!(out, [30, 40, 50]);
    }

    #[test]
    fn is_empty_reflects_state() {
        let mut r: RingBuffer<u32, 4> = RingBuffer::new();
        assert!(r.is_empty());
        r.push(1);
        assert!(!r.is_empty());
    }

    #[test]
    fn last_n_zero_returns_empty() {
        let mut r: RingBuffer<u32, 4> = RingBuffer::new();
        r.push(1);
        assert!(r.last_n(0).is_empty());
    }

    #[test]
    fn zero_capacity_ring_buffer() {
        let mut r: RingBuffer<u32, 0> = RingBuffer::new();
        r.push(1); // should not panic
        assert_eq!(r.len(), 0);
        assert!(r.is_empty());
    }
}

mod mempool {
    use dpdk_telemetry::model::mempool::{MempoolInfo, MempoolState};

    #[test]
    fn mempool_state_from_info() {
        let info = MempoolInfo {
            name: "test_pool".to_string(),
            size: 1000,
            free_count: 400,
            cache_size: 32,
            element_size: 2176,
            flags: 0,
        };
        let state = MempoolState::from_info(&info);
        assert_eq!(state.in_use, 600);
        assert_eq!(state.free_count, 400);
        assert!((state.utilization_pct - 60.0).abs() < 0.01);
    }

    #[test]
    fn mempool_state_full() {
        let info = MempoolInfo {
            name: "full_pool".to_string(),
            size: 16384,
            free_count: 0,
            ..Default::default()
        };
        let state = MempoolState::from_info(&info);
        assert_eq!(state.in_use, 16384);
        assert!((state.utilization_pct - 100.0).abs() < 0.01);
    }

    #[test]
    fn mempool_state_empty() {
        let info = MempoolInfo {
            name: "empty_pool".to_string(),
            size: 16384,
            free_count: 16384,
            ..Default::default()
        };
        let state = MempoolState::from_info(&info);
        assert_eq!(state.in_use, 0);
        assert!((state.utilization_pct - 0.0).abs() < 0.01);
    }

    #[test]
    fn mempool_state_zero_size() {
        let info = MempoolInfo {
            name: "zero".to_string(),
            size: 0,
            free_count: 0,
            ..Default::default()
        };
        let state = MempoolState::from_info(&info);
        assert!((state.utilization_pct - 0.0).abs() < 0.01);
    }
}

mod alerts {
    use dpdk_telemetry::alerts::{evaluate_mempool_alerts, evaluate_port_alerts, AlertSeverity};

    #[test]
    fn mempool_below_90_no_alert() {
        assert!(evaluate_mempool_alerts(89.9).is_empty());
        assert!(evaluate_mempool_alerts(0.0).is_empty());
        assert!(evaluate_mempool_alerts(50.0).is_empty());
    }

    #[test]
    fn mempool_above_90_warning() {
        let alerts = evaluate_mempool_alerts(91.0);
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].severity, AlertSeverity::Warning);
        assert_eq!(alerts[0].kind, "mempool_high");
    }

    #[test]
    fn mempool_at_boundary_90_no_alert() {
        assert!(evaluate_mempool_alerts(90.0).is_empty());
    }

    #[test]
    fn mempool_above_98_critical() {
        let alerts = evaluate_mempool_alerts(99.5);
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].severity, AlertSeverity::Critical);
        assert_eq!(alerts[0].kind, "mempool_critical");
        assert!(alerts[0].value.unwrap() > 99.0);
    }

    #[test]
    fn mempool_between_98_and_90_is_warning_not_critical() {
        let alerts = evaluate_mempool_alerts(97.0);
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].severity, AlertSeverity::Warning);
    }

    #[test]
    fn port_all_healthy_no_alerts() {
        let alerts = evaluate_port_alerts(0.0, 0.0, true, 0);
        assert!(alerts.is_empty());
    }

    #[test]
    fn port_rx_missed_triggers_warning() {
        let alerts = evaluate_port_alerts(100.0, 0.0, true, 3);
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].severity, AlertSeverity::Warning);
        assert_eq!(alerts[0].kind, "rx_missed_rising");
        assert_eq!(alerts[0].port_id, Some(3));
    }

    #[test]
    fn port_rx_nombuf_triggers_critical() {
        let alerts = evaluate_port_alerts(0.0, 500.0, true, 1);
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].severity, AlertSeverity::Critical);
        assert_eq!(alerts[0].kind, "rx_nombuf_rising");
    }

    #[test]
    fn port_link_down_triggers_critical() {
        let alerts = evaluate_port_alerts(0.0, 0.0, false, 0);
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].severity, AlertSeverity::Critical);
        assert_eq!(alerts[0].kind, "link_down");
    }

    #[test]
    fn port_multiple_issues_produce_multiple_alerts() {
        let alerts = evaluate_port_alerts(50.0, 100.0, false, 2);
        assert_eq!(alerts.len(), 3); // rx_missed + rx_nombuf + link_down
    }
}

mod discovery {
    use dpdk_telemetry::discovery::discover_sockets;

    #[test]
    fn discover_sockets_does_not_panic() {
        let _ = discover_sockets();
    }
}
