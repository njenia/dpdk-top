//! dpdk-top — real-time DPDK telemetry monitoring TUI.
//!
//! This library crate exposes internal modules for testing.
//! The binary entry point is in `main.rs`.

#![allow(dead_code)]

pub mod engine;
pub mod model;
pub mod output;
pub mod ui;

pub use dpdk_telemetry;
