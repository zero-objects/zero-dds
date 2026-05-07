// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-dashboard` — Live-Monitoring-Dashboard fuer ZeroDDS.
//!
//! Startet einen HTTP-Server, der eine SPA + JSON-Endpunkte ausliefert.
//! Im Standard-Modus mit synthetischem Demo-Backend; alternativ via
//! Built-in-Topics-Reader an eine echte DcpsRuntime gehaengt.

#![allow(clippy::print_stderr, clippy::print_stdout)]

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::process::ExitCode;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use zerodds_dashboard::state::{
    DashboardState, DiscoveryEdge, HistogramSnapshot, ParticipantInfo, RecordingStatus, TopicInfo,
};
use zerodds_dashboard::{ServerError, run_blocking};

fn print_help() {
    println!(
        "zerodds-dashboard — Live-Monitoring-Dashboard

USAGE:
  zerodds-dashboard [--bind ADDR:PORT] [--prometheus ADDR:PORT] [--demo]

OPTIONS:
  --bind ADDR:PORT        HTTP-Bind fuer SPA + JSON, default 127.0.0.1:8089
  --prometheus ADDR:PORT  Sidecar-Server fuer /metrics-Scrape (zerodds-monitor-1.0 §6.3),
                          z.B. 127.0.0.1:9464. Standardmaessig deaktiviert.
  --demo                  Synthetisches Demo-Backend mit Fake-Daten
                          fuer UI-Entwicklung
"
    );
}

fn parse_args() -> (SocketAddr, Option<SocketAddr>, bool) {
    let mut bind = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 8089));
    let mut prometheus: Option<SocketAddr> = None;
    let mut demo = false;
    let argv: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < argv.len() {
        match argv[i].as_str() {
            "--bind" => {
                if let Some(v) = argv.get(i + 1) {
                    if let Ok(a) = v.parse() {
                        bind = a;
                    } else {
                        eprintln!("invalid --bind value: {v}");
                    }
                }
                i += 2;
            }
            "--prometheus" => {
                if let Some(v) = argv.get(i + 1) {
                    match v.parse() {
                        Ok(a) => prometheus = Some(a),
                        Err(e) => eprintln!("invalid --prometheus value: {v} ({e})"),
                    }
                }
                i += 2;
            }
            "--demo" => {
                demo = true;
                i += 1;
            }
            "--help" | "-h" | "help" => {
                print_help();
                std::process::exit(0);
            }
            other => {
                eprintln!("unknown flag: {other}");
                i += 1;
            }
        }
    }
    (bind, prometheus, demo)
}

/// Synthetische Daten — fuer UI-Entwicklung ohne Live-Domain.
fn install_demo_data(state: &Arc<DashboardState>) {
    state.set_participants(vec![
        ParticipantInfo {
            guid_prefix_hex: "010f000001020304050607".into(),
            name: "talker".into(),
            domain_id: 0,
            vendor_id_hex: "01.0F".into(),
        },
        ParticipantInfo {
            guid_prefix_hex: "010f0000aabbccddeeff00".into(),
            name: "listener".into(),
            domain_id: 0,
            vendor_id_hex: "01.0F".into(),
        },
        ParticipantInfo {
            guid_prefix_hex: "010f00001122334455667".into(),
            name: "imu-publisher".into(),
            domain_id: 0,
            vendor_id_hex: "01.0F".into(),
        },
    ]);
    state.set_topics(vec![
        TopicInfo {
            name: "rt/chatter".into(),
            type_name: "std_msgs::msg::String".into(),
            publishers: 1,
            subscribers: 2,
            sample_rate_hz: 10.0,
        },
        TopicInfo {
            name: "rt/imu".into(),
            type_name: "sensor_msgs::msg::Imu".into(),
            publishers: 1,
            subscribers: 1,
            sample_rate_hz: 100.0,
        },
    ]);
    state.set_histograms(vec![
        HistogramSnapshot {
            name: "dds.write.latency".into(),
            count: 1234,
            mean_ns: 35_000,
            min_ns: 12_000,
            max_ns: 980_000,
            p50_ns: 28_000,
            p99_ns: 220_000,
        },
        HistogramSnapshot {
            name: "dds.read.latency".into(),
            count: 1234,
            mean_ns: 41_000,
            min_ns: 18_000,
            max_ns: 1_100_000,
            p50_ns: 33_000,
            p99_ns: 280_000,
        },
        HistogramSnapshot {
            name: "dds.heartbeat.rtt".into(),
            count: 56,
            mean_ns: 800_000,
            min_ns: 500_000,
            max_ns: 2_400_000,
            p50_ns: 750_000,
            p99_ns: 2_100_000,
        },
    ]);
    state.set_edges(vec![
        DiscoveryEdge {
            from_guid: "010f000001020304050607".into(),
            to_guid: "010f0000aabbccddeeff00".into(),
            topic: "rt/chatter".into(),
        },
        DiscoveryEdge {
            from_guid: "010f00001122334455667".into(),
            to_guid: "010f0000aabbccddeeff00".into(),
            topic: "rt/imu".into(),
        },
    ]);
    state.set_recording(RecordingStatus {
        active: false,
        output_path: None,
        frames: 0,
    });
}

/// Fake-Updater-Thread — bewegt die Histogramme leicht, damit das
/// Dashboard im Demo-Modus „lebt”.
fn spawn_demo_ticker(state: Arc<DashboardState>) {
    thread::spawn(move || {
        let mut tick: u64 = 0;
        loop {
            thread::sleep(Duration::from_secs(1));
            tick = tick.wrapping_add(1);
            let drift = (tick % 30) as i64 - 15;
            let mean_drift = drift * 200;
            state.set_histograms(vec![
                HistogramSnapshot {
                    name: "dds.write.latency".into(),
                    count: 1234 + tick,
                    mean_ns: (35_000 + mean_drift).max(1) as u64,
                    min_ns: 12_000,
                    max_ns: 980_000 + (drift.unsigned_abs() * 1_000),
                    p50_ns: (28_000 + mean_drift).max(1) as u64,
                    p99_ns: 220_000 + (drift.unsigned_abs() * 2_000),
                },
                HistogramSnapshot {
                    name: "dds.read.latency".into(),
                    count: 1234 + tick,
                    mean_ns: (41_000 + mean_drift).max(1) as u64,
                    min_ns: 18_000,
                    max_ns: 1_100_000,
                    p50_ns: (33_000 + mean_drift).max(1) as u64,
                    p99_ns: 280_000 + (drift.unsigned_abs() * 3_000),
                },
                HistogramSnapshot {
                    name: "dds.heartbeat.rtt".into(),
                    count: 56 + tick / 5,
                    mean_ns: 800_000,
                    min_ns: 500_000,
                    max_ns: 2_400_000,
                    p50_ns: 750_000,
                    p99_ns: 2_100_000,
                },
            ]);
        }
    });
}

fn main() -> ExitCode {
    let (bind, prometheus, demo) = parse_args();
    let state = Arc::new(DashboardState::new());
    if demo {
        install_demo_data(&state);
        spawn_demo_ticker(Arc::clone(&state));
    } else {
        eprintln!(
            "zerodds-dashboard: ohne --demo leerer State; \
             Live-Hook an DcpsRuntime via Built-in-Topics + Prometheus-Scrape (siehe README)."
        );
    }
    if let Some(addr) = prometheus {
        let registry = zerodds_monitor::default_registry();
        match zerodds_monitor::serve_prometheus(addr, registry) {
            Ok(_h) => {
                eprintln!("zerodds-dashboard: Prometheus-Scrape lauscht auf http://{addr}/metrics");
            }
            Err(e) => {
                eprintln!("prometheus server error: {e}");
                return ExitCode::from(1);
            }
        }
    }
    if let Err(e) = run_blocking(bind, state) {
        match e {
            ServerError::Io(e) => eprintln!("server error: {e}"),
        }
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}
