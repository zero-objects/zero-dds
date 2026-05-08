// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-monitor-cli` library — argument parsing für das
//! `zerodds-monitor` Binary.
//!
//! Das Tool nutzt das Backend in `crates/monitor/` (publiziert als
//! `zerodds-monitor`). Die Trennung zwischen Tool- und Backend-
//! Crate ist nötig damit beide Konsumenten (Library-User und CLI-
//! User) eindeutig auflösen.

#![allow(clippy::module_name_repetitions)]

use std::time::Duration;

/// Default Listen-Address für `monitor serve`.
pub const DEFAULT_ADDR: &str = "127.0.0.1:9991";
/// Default Snapshot-Duration.
pub const DEFAULT_DURATION_SECS: u64 = 5;

/// Sub-command des Monitor-CLIs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// `snapshot` — startet Runtime, sammelt Metriken für
    /// `--duration`, druckt Registry-Inhalt.
    Snapshot(SnapshotArgs),
    /// `serve` — startet Runtime + HTTP-Server auf `--addr` für
    /// `--duration` (oder bis SIGINT).
    Serve(ServeArgs),
    /// `names` — druckt bekannte Metric-Namen mit HELP-Texten.
    Names,
}

/// Arguments für `snapshot`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotArgs {
    /// DDS-Domain.
    pub domain: u32,
    /// Sammel-Duration.
    pub duration: Duration,
    /// Output-Format.
    pub format: SnapshotFormat,
}

/// Snapshot-Output-Format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotFormat {
    /// Mensch-lesbarer Text-Tree.
    Text,
    /// Prometheus-Exposition (gleiches Format wie `/metrics`).
    Prometheus,
}

impl Default for SnapshotArgs {
    fn default() -> Self {
        Self {
            domain: 0,
            duration: Duration::from_secs(DEFAULT_DURATION_SECS),
            format: SnapshotFormat::Text,
        }
    }
}

/// Arguments für `serve`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServeArgs {
    /// DDS-Domain.
    pub domain: u32,
    /// Listen-Adresse.
    pub addr: String,
    /// Maximale Lebenszeit; `None` = bis SIGINT.
    pub duration: Option<Duration>,
}

impl Default for ServeArgs {
    fn default() -> Self {
        Self {
            domain: 0,
            addr: DEFAULT_ADDR.to_string(),
            duration: None,
        }
    }
}

/// Parse-Fehler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// Kein subcommand.
    Missing,
    /// Unbekanntes subcommand.
    Unknown(String),
    /// Required-arg fehlt.
    MissingArg(&'static str),
    /// Wert nicht parse-bar.
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

/// Parst die `args` Slice (ohne `argv[0]`) zu einem [`Command`].
///
/// # Errors
/// Siehe [`ParseError`].
pub fn parse_args(args: &[String]) -> Result<Command, ParseError> {
    let sub = args.first().ok_or(ParseError::Missing)?;
    match sub.as_str() {
        "snapshot" => parse_snapshot(&args[1..]).map(Command::Snapshot),
        "serve" => parse_serve(&args[1..]).map(Command::Serve),
        "names" => {
            if args.len() > 1 {
                return Err(ParseError::Unknown(args[1].clone()));
            }
            Ok(Command::Names)
        }
        other => Err(ParseError::Unknown(other.to_string())),
    }
}

fn parse_snapshot(rest: &[String]) -> Result<SnapshotArgs, ParseError> {
    let mut out = SnapshotArgs::default();
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
            "--duration" => {
                i += 1;
                let v = rest.get(i).ok_or(ParseError::MissingArg("duration"))?;
                out.duration =
                    zerodds_cli_common::parse_duration(v).map_err(|_| ParseError::BadValue {
                        flag: "duration",
                        got: v.clone(),
                    })?;
            }
            "--format" | "-f" => {
                i += 1;
                let v = rest.get(i).ok_or(ParseError::MissingArg("format"))?;
                out.format = match v.as_str() {
                    "text" => SnapshotFormat::Text,
                    "prometheus" | "prom" => SnapshotFormat::Prometheus,
                    other => {
                        return Err(ParseError::BadValue {
                            flag: "format",
                            got: other.to_string(),
                        });
                    }
                };
            }
            other => return Err(ParseError::Unknown(other.to_string())),
        }
        i += 1;
    }
    Ok(out)
}

fn parse_serve(rest: &[String]) -> Result<ServeArgs, ParseError> {
    let mut out = ServeArgs::default();
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
            "--addr" | "-a" => {
                i += 1;
                out.addr = rest.get(i).ok_or(ParseError::MissingArg("addr"))?.clone();
            }
            "--duration" => {
                i += 1;
                let v = rest.get(i).ok_or(ParseError::MissingArg("duration"))?;
                out.duration = Some(zerodds_cli_common::parse_duration(v).map_err(|_| {
                    ParseError::BadValue {
                        flag: "duration",
                        got: v.clone(),
                    }
                })?);
            }
            other => return Err(ParseError::Unknown(other.to_string())),
        }
        i += 1;
    }
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn s(args: &[&str]) -> Vec<String> {
        args.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn parse_snapshot_default() {
        let cmd = parse_args(&s(&["snapshot"])).unwrap();
        assert_eq!(cmd, Command::Snapshot(SnapshotArgs::default()));
    }

    #[test]
    fn parse_snapshot_full() {
        let cmd = parse_args(&s(&[
            "snapshot",
            "-d",
            "5",
            "--duration",
            "30s",
            "-f",
            "prom",
        ]))
        .unwrap();
        let Command::Snapshot(s_args) = cmd else {
            panic!("expected snapshot");
        };
        assert_eq!(s_args.domain, 5);
        assert_eq!(s_args.duration, Duration::from_secs(30));
        assert_eq!(s_args.format, SnapshotFormat::Prometheus);
    }

    #[test]
    fn parse_serve_with_addr() {
        let cmd = parse_args(&s(&["serve", "-a", "0.0.0.0:9000", "--duration", "1s"])).unwrap();
        let Command::Serve(srv) = cmd else {
            panic!("expected serve");
        };
        assert_eq!(srv.addr, "0.0.0.0:9000");
        assert_eq!(srv.duration, Some(Duration::from_secs(1)));
    }

    #[test]
    fn parse_names_no_args() {
        assert_eq!(parse_args(&s(&["names"])).unwrap(), Command::Names);
    }

    #[test]
    fn parse_no_args_rejected() {
        assert!(matches!(parse_args(&[]), Err(ParseError::Missing)));
    }

    #[test]
    fn parse_unknown_subcommand_rejected() {
        assert!(matches!(
            parse_args(&s(&["unknown"])),
            Err(ParseError::Unknown(_))
        ));
    }

    #[test]
    fn parse_bad_format_rejected() {
        assert!(matches!(
            parse_args(&s(&["snapshot", "--format", "json"])),
            Err(ParseError::BadValue { .. })
        ));
    }
}
