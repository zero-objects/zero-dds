// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-bench` library — CLI parsing + statistics primitives.
//!
//! Das eigentliche Bench-Backend (DcpsRuntime + Loopback-Topic) lebt
//! in `main.rs`. Diese Crate exportiert die argument-Strukturen und
//! die Quantil-Berechnung damit beide unit-test-bar sind ohne ein
//! tatsächliches DCPS-Setup zu starten.

#![allow(clippy::module_name_repetitions)]

use std::time::Duration;

/// Default-Domain (DDS DOMAIN_ID 0).
pub const DEFAULT_DOMAIN: u32 = 0;
/// Default Bench-Topic.
pub const DEFAULT_TOPIC: &str = "zerodds/bench/loopback";
/// Default Payload-Größe in Bytes.
pub const DEFAULT_PAYLOAD: usize = 64;
/// Default Bench-Duration.
pub const DEFAULT_DURATION_SECS: u64 = 5;

/// Sub-command des Bench-CLIs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// `latency` — RTT-style ping/echo Loopback-Latency.
    Latency(BenchArgs),
    /// `throughput` — Burst-Sender, msgs/s + MB/s.
    Throughput(BenchArgs),
    /// `info` — druckt Backend-Capabilities und exit.
    Info,
}

/// Gemeinsame Argumente für Latency- und Throughput-Bench.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BenchArgs {
    /// DDS-Domain.
    pub domain: u32,
    /// Topic-Name.
    pub topic: String,
    /// Payload-Größe pro Sample.
    pub payload: usize,
    /// Run-Duration.
    pub duration: Duration,
}

impl Default for BenchArgs {
    fn default() -> Self {
        Self {
            domain: DEFAULT_DOMAIN,
            topic: DEFAULT_TOPIC.to_string(),
            payload: DEFAULT_PAYLOAD,
            duration: Duration::from_secs(DEFAULT_DURATION_SECS),
        }
    }
}

/// Parse-Fehler beim CLI-args.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// Kein subcommand angegeben.
    Missing,
    /// Unbekanntes subcommand.
    Unknown(String),
    /// Required-arg fehlt.
    MissingArg(&'static str),
    /// Wert ist nicht parse-bar.
    BadValue {
        /// Welche flag.
        flag: &'static str,
        /// Was eingegeben war.
        got: String,
    },
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing => write!(f, "no sub-command given"),
            Self::Unknown(s) => write!(f, "unknown sub-command: {s}"),
            Self::MissingArg(a) => write!(f, "missing required arg: {a}"),
            Self::BadValue { flag, got } => write!(f, "bad value for --{flag}: {got}"),
        }
    }
}

impl std::error::Error for ParseError {}

/// Parst `args` (typisch `env::args().skip(1)`) zu einem [`Command`].
///
/// # Errors
/// Siehe [`ParseError`].
pub fn parse_args(args: &[String]) -> Result<Command, ParseError> {
    let sub = args.first().ok_or(ParseError::Missing)?;
    match sub.as_str() {
        "latency" => parse_bench(&args[1..]).map(Command::Latency),
        "throughput" => parse_bench(&args[1..]).map(Command::Throughput),
        "info" => {
            if args.len() > 1 {
                return Err(ParseError::Unknown(args[1].clone()));
            }
            Ok(Command::Info)
        }
        other => Err(ParseError::Unknown(other.to_string())),
    }
}

fn parse_bench(rest: &[String]) -> Result<BenchArgs, ParseError> {
    let mut out = BenchArgs::default();
    let mut i = 0;
    while i < rest.len() {
        match rest[i].as_str() {
            "--domain" | "-d" => {
                i += 1;
                let v = rest.get(i).ok_or(ParseError::MissingArg("domain"))?;
                out.domain = v.parse().map_err(|_| ParseError::BadValue {
                    flag: "domain",
                    got: v.clone(),
                })?;
            }
            "--topic" | "-t" => {
                i += 1;
                out.topic = rest.get(i).ok_or(ParseError::MissingArg("topic"))?.clone();
            }
            "--payload" | "-p" => {
                i += 1;
                let v = rest.get(i).ok_or(ParseError::MissingArg("payload"))?;
                out.payload = v.parse().map_err(|_| ParseError::BadValue {
                    flag: "payload",
                    got: v.clone(),
                })?;
            }
            "--duration" => {
                i += 1;
                let v = rest.get(i).ok_or(ParseError::MissingArg("duration"))?;
                out.duration =
                    zerodds_cli_common::parse_duration(v).map_err(|_| ParseError::BadValue {
                        flag: "duration",
                        got: v.clone(),
                    })?;
            }
            other => return Err(ParseError::Unknown(other.to_string())),
        }
        i += 1;
    }
    Ok(out)
}

/// Statistik-Snapshot für Latency-Bench.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LatencyStats {
    /// Anzahl gesammelte Samples.
    pub samples: usize,
    /// Minimum (ns).
    pub min_ns: u64,
    /// Median (p50, ns).
    pub p50_ns: u64,
    /// 99th-Percentile (ns).
    pub p99_ns: u64,
    /// Maximum (ns).
    pub max_ns: u64,
}

/// Berechnet [`LatencyStats`] aus einem Vector von Round-Trip-Times in ns.
///
/// Sortiert in-place. Liefert `None` falls leer.
#[must_use]
pub fn compute_stats(rtts_ns: &mut [u64]) -> Option<LatencyStats> {
    if rtts_ns.is_empty() {
        return None;
    }
    rtts_ns.sort_unstable();
    let n = rtts_ns.len();
    let p50_idx = n / 2;
    let p99_idx = n.saturating_mul(99) / 100;
    Some(LatencyStats {
        samples: n,
        min_ns: rtts_ns[0],
        p50_ns: rtts_ns[p50_idx.min(n - 1)],
        p99_ns: rtts_ns[p99_idx.min(n - 1)],
        max_ns: rtts_ns[n - 1],
    })
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::missing_panics_doc
)]
mod tests {
    use super::*;

    fn s(args: &[&str]) -> Vec<String> {
        args.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn parse_latency_minimal() {
        let cmd = parse_args(&s(&["latency"])).unwrap();
        assert_eq!(cmd, Command::Latency(BenchArgs::default()));
    }

    #[test]
    fn parse_latency_full() {
        let cmd = parse_args(&s(&[
            "latency",
            "-d",
            "5",
            "-t",
            "Foo",
            "--payload",
            "1024",
            "--duration",
            "30s",
        ]))
        .unwrap();
        let Command::Latency(b) = cmd else {
            panic!("expected latency");
        };
        assert_eq!(b.domain, 5);
        assert_eq!(b.topic, "Foo");
        assert_eq!(b.payload, 1024);
        assert_eq!(b.duration, Duration::from_secs(30));
    }

    #[test]
    fn parse_throughput_smoke() {
        let cmd = parse_args(&s(&["throughput", "-p", "65536"])).unwrap();
        let Command::Throughput(b) = cmd else {
            panic!("expected throughput");
        };
        assert_eq!(b.payload, 65536);
    }

    #[test]
    fn parse_info() {
        let cmd = parse_args(&s(&["info"])).unwrap();
        assert_eq!(cmd, Command::Info);
    }

    #[test]
    fn parse_unknown_subcommand_rejected() {
        let err = parse_args(&s(&["unknown"])).unwrap_err();
        assert!(matches!(err, ParseError::Unknown(_)));
    }

    #[test]
    fn parse_no_args_rejected() {
        let err = parse_args(&[]).unwrap_err();
        assert!(matches!(err, ParseError::Missing));
    }

    #[test]
    fn parse_bad_payload_rejected() {
        let err = parse_args(&s(&["latency", "--payload", "abc"])).unwrap_err();
        assert!(matches!(err, ParseError::BadValue { .. }));
    }

    #[test]
    fn compute_stats_basic() {
        let mut rtts = vec![100u64, 200, 300, 400, 500];
        let stats = compute_stats(&mut rtts).unwrap();
        assert_eq!(stats.samples, 5);
        assert_eq!(stats.min_ns, 100);
        assert_eq!(stats.max_ns, 500);
        assert_eq!(stats.p50_ns, 300);
    }

    #[test]
    fn compute_stats_empty() {
        let mut empty: Vec<u64> = Vec::new();
        assert!(compute_stats(&mut empty).is_none());
    }

    #[test]
    fn compute_stats_single() {
        let mut single = vec![42u64];
        let stats = compute_stats(&mut single).unwrap();
        assert_eq!(stats.min_ns, 42);
        assert_eq!(stats.max_ns, 42);
        assert_eq!(stats.p50_ns, 42);
        assert_eq!(stats.p99_ns, 42);
    }
}
