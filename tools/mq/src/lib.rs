// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-mq` library — argument parsing.
//!
//! Crate `zerodds-mq`. Safety classification: **COMFORT**.
//! User-facing cross-domain Bridge; nur CLI-Frontend.

#![allow(clippy::module_name_repetitions)]

use std::time::Duration;

/// Default Source-Domain.
pub const DEFAULT_SRC_DOMAIN: u32 = 0;
/// Default Destination-Domain.
pub const DEFAULT_DST_DOMAIN: u32 = 1;

/// Sub-command des MQ-CLIs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// `bridge` — leitet ein Topic von Source-Domain in Destination-Domain.
    Bridge(BridgeArgs),
}

/// Argumente für `bridge`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgeArgs {
    /// Source-Domain.
    pub src_domain: u32,
    /// Destination-Domain.
    pub dst_domain: u32,
    /// Topic auf Source-Seite.
    pub src_topic: String,
    /// Topic auf Destination-Seite (default == src_topic).
    pub dst_topic: String,
    /// Maximale Lebensdauer; `None` = bis SIGINT.
    pub duration: Option<Duration>,
    /// Bidirektional (sub<->pub auf beiden Seiten).
    pub bidirectional: bool,
}

impl Default for BridgeArgs {
    fn default() -> Self {
        Self {
            src_domain: DEFAULT_SRC_DOMAIN,
            dst_domain: DEFAULT_DST_DOMAIN,
            src_topic: String::new(),
            dst_topic: String::new(),
            duration: None,
            bidirectional: false,
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

/// Parst `args` zu einem [`Command`]. `bridge` ist implizit wenn das
/// erste Token mit `-` anfängt.
///
/// # Errors
/// Siehe [`ParseError`].
pub fn parse_args(args: &[String]) -> Result<Command, ParseError> {
    let first = args.first().ok_or(ParseError::Missing)?;
    let (sub, rest) = if first.starts_with('-') {
        ("bridge", args)
    } else {
        (first.as_str(), &args[1..])
    };
    match sub {
        "bridge" => parse_bridge(rest).map(Command::Bridge),
        other => Err(ParseError::Unknown(other.to_string())),
    }
}

fn parse_bridge(rest: &[String]) -> Result<BridgeArgs, ParseError> {
    let mut out = BridgeArgs::default();
    let mut i = 0;
    while i < rest.len() {
        match rest[i].as_str() {
            "--src-domain" => {
                i += 1;
                let v = rest.get(i).ok_or(ParseError::MissingArg("src-domain"))?;
                out.src_domain = v.parse().map_err(|_| ParseError::BadValue {
                    flag: "src-domain",
                    got: v.clone(),
                })?;
            }
            "--dst-domain" => {
                i += 1;
                let v = rest.get(i).ok_or(ParseError::MissingArg("dst-domain"))?;
                out.dst_domain = v.parse().map_err(|_| ParseError::BadValue {
                    flag: "dst-domain",
                    got: v.clone(),
                })?;
            }
            "--topic" | "-t" => {
                i += 1;
                let topic = rest.get(i).ok_or(ParseError::MissingArg("topic"))?.clone();
                out.src_topic = topic.clone();
                if out.dst_topic.is_empty() {
                    out.dst_topic = topic;
                }
            }
            "--src-topic" => {
                i += 1;
                out.src_topic = rest
                    .get(i)
                    .ok_or(ParseError::MissingArg("src-topic"))?
                    .clone();
            }
            "--dst-topic" => {
                i += 1;
                out.dst_topic = rest
                    .get(i)
                    .ok_or(ParseError::MissingArg("dst-topic"))?
                    .clone();
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
            "--bidirectional" => {
                out.bidirectional = true;
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
    fn parse_bridge_minimal() {
        let cmd = parse_args(&s(&["bridge", "-t", "Foo"])).unwrap();
        let Command::Bridge(b) = cmd;
        assert_eq!(b.src_topic, "Foo");
        assert_eq!(b.dst_topic, "Foo");
        assert_eq!(b.src_domain, 0);
        assert_eq!(b.dst_domain, 1);
    }

    #[test]
    fn parse_bridge_implicit() {
        let cmd = parse_args(&s(&["-t", "Bar"])).unwrap();
        let Command::Bridge(b) = cmd;
        assert_eq!(b.src_topic, "Bar");
    }

    #[test]
    fn parse_bridge_full() {
        let cmd = parse_args(&s(&[
            "bridge",
            "--src-domain",
            "0",
            "--dst-domain",
            "5",
            "--src-topic",
            "X",
            "--dst-topic",
            "Y",
            "--duration",
            "10s",
            "--bidirectional",
        ]))
        .unwrap();
        let Command::Bridge(b) = cmd;
        assert_eq!(b.src_domain, 0);
        assert_eq!(b.dst_domain, 5);
        assert_eq!(b.src_topic, "X");
        assert_eq!(b.dst_topic, "Y");
        assert_eq!(b.duration, Some(Duration::from_secs(10)));
        assert!(b.bidirectional);
    }

    #[test]
    fn parse_bridge_topic_propagates_to_dst() {
        let cmd = parse_args(&s(&["bridge", "-t", "T1"])).unwrap();
        let Command::Bridge(b) = cmd;
        assert_eq!(b.src_topic, b.dst_topic);
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
    fn parse_bad_domain_rejected() {
        assert!(matches!(
            parse_args(&s(&["bridge", "--src-domain", "abc"])),
            Err(ParseError::BadValue { .. })
        ));
    }
}
