// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-snitch` library — argument parsing.
//!
//! Crate `zerodds-snitch`. Safety classification: **COMFORT**.
//! User-facing Discovery-Probe; nur CLI-Frontend.

#![allow(clippy::module_name_repetitions)]

use std::time::Duration;

/// Default Domain.
pub const DEFAULT_DOMAIN: u32 = 0;
/// Default Probe-Duration.
pub const DEFAULT_DURATION_SECS: u64 = 5;

/// Sub-command des Snitch-CLIs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// `probe` — startet kurzen Discovery-Run und druckt Snapshot.
    Probe(ProbeArgs),
}

/// Argumente für `probe`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeArgs {
    /// DDS Domain.
    pub domain: u32,
    /// Probe-Duration.
    pub duration: Duration,
    /// Output-Format.
    pub format: ProbeFormat,
}

/// Output-Format für `probe`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeFormat {
    /// Mensch-lesbarer Text-Tree.
    Text,
    /// JSON-Lines (eine Zeile pro Participant).
    Json,
}

impl Default for ProbeArgs {
    fn default() -> Self {
        Self {
            domain: DEFAULT_DOMAIN,
            duration: Duration::from_secs(DEFAULT_DURATION_SECS),
            format: ProbeFormat::Text,
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

/// Parst `args` zu einem [`Command`]. `probe` ist implizit wenn das
/// erste Token mit `-` anfängt.
///
/// # Errors
/// Siehe [`ParseError`].
pub fn parse_args(args: &[String]) -> Result<Command, ParseError> {
    let first = args.first().ok_or(ParseError::Missing)?;
    let (sub, rest) = if first.starts_with('-') {
        ("probe", args)
    } else {
        (first.as_str(), &args[1..])
    };
    match sub {
        "probe" => parse_probe(rest).map(Command::Probe),
        other => Err(ParseError::Unknown(other.to_string())),
    }
}

fn parse_probe(rest: &[String]) -> Result<ProbeArgs, ParseError> {
    let mut out = ProbeArgs::default();
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
                    "text" => ProbeFormat::Text,
                    "json" => ProbeFormat::Json,
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn s(args: &[&str]) -> Vec<String> {
        args.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn parse_probe_default() {
        let cmd = parse_args(&s(&["probe"])).unwrap();
        assert_eq!(cmd, Command::Probe(ProbeArgs::default()));
    }

    #[test]
    fn parse_probe_implicit() {
        let cmd = parse_args(&s(&["-d", "5"])).unwrap();
        let Command::Probe(p) = cmd;
        assert_eq!(p.domain, 5);
    }

    #[test]
    fn parse_probe_full() {
        let cmd = parse_args(&s(&[
            "probe",
            "-d",
            "5",
            "--duration",
            "10s",
            "--format",
            "json",
        ]))
        .unwrap();
        let Command::Probe(p) = cmd;
        assert_eq!(p.domain, 5);
        assert_eq!(p.duration, Duration::from_secs(10));
        assert_eq!(p.format, ProbeFormat::Json);
    }

    #[test]
    fn parse_no_args_rejected() {
        assert!(matches!(parse_args(&[]), Err(ParseError::Missing)));
    }

    #[test]
    fn parse_unknown_subcommand_rejected() {
        let err = parse_args(&s(&["wat"])).unwrap_err();
        assert!(matches!(err, ParseError::Unknown(_)));
    }

    #[test]
    fn parse_bad_format_rejected() {
        assert!(matches!(
            parse_args(&s(&["probe", "--format", "xml"])),
            Err(ParseError::BadValue { .. })
        ));
    }
}
