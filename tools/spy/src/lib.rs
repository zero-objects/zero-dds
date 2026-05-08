// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-spy` library — argument parsing.

#![allow(clippy::module_name_repetitions)]

use std::time::Duration;

/// Default Domain.
pub const DEFAULT_DOMAIN: u32 = 0;
/// Default Hex-Snippet-Bytes pro Sample.
pub const DEFAULT_HEX_BYTES: usize = 32;

/// Sub-command des Spy-CLIs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// `subscribe` — abonniere TOPIC und dump samples.
    Subscribe(SubscribeArgs),
}

/// Argumente für `subscribe`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscribeArgs {
    /// DDS Domain.
    pub domain: u32,
    /// Topic-Name.
    pub topic: String,
    /// Maximale Anzahl Samples bevor Auto-Stop; `None` = unlimitiert.
    pub max_samples: Option<u64>,
    /// Maximale Lebensdauer; `None` = bis SIGINT.
    pub duration: Option<Duration>,
    /// Wieviele Bytes pro Sample als Hex drucken (0 = nur metadata).
    pub hex_bytes: usize,
}

impl Default for SubscribeArgs {
    fn default() -> Self {
        Self {
            domain: DEFAULT_DOMAIN,
            topic: String::new(),
            max_samples: None,
            duration: None,
            hex_bytes: DEFAULT_HEX_BYTES,
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

/// Parst `args` (typisch `env::args().skip(1)`) zu einem [`Command`].
///
/// # Errors
/// Siehe [`ParseError`].
pub fn parse_args(args: &[String]) -> Result<Command, ParseError> {
    // Default-subcommand: `subscribe` wenn der erste Token mit `-`
    // anfaengt oder ein Topic-Name ist. Erlaubt sowohl
    // `zerodds-spy subscribe -t Foo` als auch direkt
    // `zerodds-spy -t Foo`.
    let first = args.first().ok_or(ParseError::Missing)?;
    let (sub, rest) = if first.starts_with('-') || (!first.is_empty() && !is_subcommand_name(first))
    {
        ("subscribe", args)
    } else {
        (first.as_str(), &args[1..])
    };
    match sub {
        "subscribe" => parse_subscribe(rest).map(Command::Subscribe),
        other => Err(ParseError::Unknown(other.to_string())),
    }
}

const fn is_subcommand_name(s: &str) -> bool {
    matches!(s.as_bytes(), b"subscribe")
}

fn parse_subscribe(rest: &[String]) -> Result<SubscribeArgs, ParseError> {
    let mut out = SubscribeArgs::default();
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
            "--count" | "-n" => {
                i += 1;
                let v = rest.get(i).ok_or(ParseError::MissingArg("count"))?;
                out.max_samples = Some(v.parse().map_err(|_| ParseError::BadValue {
                    flag: "count",
                    got: v.clone(),
                })?);
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
            "--hex" | "-x" => {
                i += 1;
                let v = rest.get(i).ok_or(ParseError::MissingArg("hex"))?;
                out.hex_bytes = v.parse().map_err(|_| ParseError::BadValue {
                    flag: "hex",
                    got: v.clone(),
                })?;
            }
            other => return Err(ParseError::Unknown(other.to_string())),
        }
        i += 1;
    }
    Ok(out)
}

/// Formatiert ein Byte-Slice als Hex, max `limit` Bytes mit `…`-Suffix
/// wenn länger.
#[must_use]
pub fn format_hex_snippet(bytes: &[u8], limit: usize) -> String {
    if limit == 0 {
        return String::new();
    }
    let take = bytes.len().min(limit);
    let mut out = String::with_capacity(take * 3);
    for (i, b) in bytes[..take].iter().enumerate() {
        if i > 0 && i % 4 == 0 {
            out.push(' ');
        }
        out.push_str(&format!("{b:02x}"));
    }
    if bytes.len() > limit {
        out.push_str(&format!(" …(+{}B)", bytes.len() - limit));
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn s(args: &[&str]) -> Vec<String> {
        args.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn parse_subscribe_explicit() {
        let cmd = parse_args(&s(&["subscribe", "-t", "Foo"])).unwrap();
        let Command::Subscribe(a) = cmd;
        assert_eq!(a.topic, "Foo");
    }

    #[test]
    fn parse_subscribe_implicit() {
        let cmd = parse_args(&s(&["-t", "Bar"])).unwrap();
        let Command::Subscribe(a) = cmd;
        assert_eq!(a.topic, "Bar");
    }

    #[test]
    fn parse_subscribe_full() {
        let cmd = parse_args(&s(&[
            "subscribe",
            "-d",
            "5",
            "-t",
            "X",
            "-n",
            "10",
            "--duration",
            "30s",
            "-x",
            "16",
        ]))
        .unwrap();
        let Command::Subscribe(a) = cmd;
        assert_eq!(a.domain, 5);
        assert_eq!(a.topic, "X");
        assert_eq!(a.max_samples, Some(10));
        assert_eq!(a.duration, Some(Duration::from_secs(30)));
        assert_eq!(a.hex_bytes, 16);
    }

    #[test]
    fn parse_no_args_rejected() {
        assert!(matches!(parse_args(&[]), Err(ParseError::Missing)));
    }

    #[test]
    fn parse_unknown_subcommand_rejected() {
        // "publish" ist kein Subcommand und kein Flag → Unknown.
        let err = parse_args(&s(&["publish", "-t", "Foo"])).unwrap_err();
        assert!(matches!(err, ParseError::Unknown(_)));
    }

    #[test]
    fn parse_bad_count_rejected() {
        assert!(matches!(
            parse_args(&s(&["subscribe", "-n", "abc"])),
            Err(ParseError::BadValue { .. })
        ));
    }

    #[test]
    fn format_hex_snippet_basic() {
        let s = format_hex_snippet(&[0xde, 0xad, 0xbe, 0xef, 0x01, 0x02], 4);
        assert!(s.starts_with("deadbeef"));
        assert!(s.contains("…(+2B)"));
    }

    #[test]
    fn format_hex_snippet_zero_limit() {
        assert_eq!(format_hex_snippet(&[1, 2, 3], 0), "");
    }

    #[test]
    fn format_hex_snippet_no_truncation() {
        let s = format_hex_snippet(&[0x12, 0x34], 4);
        assert_eq!(s, "1234");
    }
}
