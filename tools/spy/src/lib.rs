// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-spy` library — argument parsing.
//!
//! Crate `zerodds-spy`. Safety classification: **COMFORT**.
//! User-facing topic spy tool; CLI frontend only, no runtime paths.

#![allow(clippy::module_name_repetitions)]

use std::time::Duration;

/// Default Domain.
pub const DEFAULT_DOMAIN: u32 = 0;
/// Default Hex-Snippet-Bytes pro Sample.
pub const DEFAULT_HEX_BYTES: usize = 32;

/// Sub-command of the spy CLI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// `subscribe` — subscribe to TOPIC and dump samples.
    Subscribe(SubscribeArgs),
}

/// Arguments for `subscribe`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscribeArgs {
    /// DDS Domain.
    pub domain: u32,
    /// Topic-Name.
    pub topic: String,
    /// Maximum number of samples before auto-stop; `None` = unlimited.
    pub max_samples: Option<u64>,
    /// Maximum lifetime; `None` = until SIGINT.
    pub duration: Option<Duration>,
    /// How many bytes per sample to print as hex (0 = only metadata).
    pub hex_bytes: usize,
    /// Optionaler Typname-Override (`--type`). `None` = type-following:
    /// the spy discovers the real `type_name` of the writers via discovery
    /// and attaches per type. Set = subscribe to this type directly
    /// (skips discovery; useful for silent / late-joining writers).
    pub type_name: Option<String>,
    /// Decode samples to typed JSON on stdout (needs `type_file`).
    pub decode: bool,
    /// Out-of-band IDL file providing the types to decode against (`--type-file`).
    pub type_file: Option<String>,
    /// The wire dialect `--decode` reads in (`--dialect`): the one peer's
    /// answers to the questions implementations answer differently, as
    /// `zerodds_ast_layout::Dialect::parse` spells them. `None` leaves it to
    /// the configuration's `wire.codec_dialect`, whose default is the
    /// specification's. The decoder does not guess it from the bytes: Fast
    /// DDS closes a PL_CDR1 parameter list with a bare `0x3f02` where the
    /// specification's reading sets the must-understand bit, and a decoder
    /// in the wrong dialect reports the framing rather than accepting it.
    pub dialect: Option<zerodds_ast_layout::Dialect>,
}

impl Default for SubscribeArgs {
    fn default() -> Self {
        Self {
            domain: DEFAULT_DOMAIN,
            topic: String::new(),
            max_samples: None,
            duration: None,
            hex_bytes: DEFAULT_HEX_BYTES,
            type_name: None,
            decode: false,
            type_file: None,
            dialect: None,
        }
    }
}

/// Parse error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// No subcommand.
    Missing,
    /// Unknown subcommand.
    Unknown(String),
    /// Required argument missing.
    MissingArg(&'static str),
    /// Value not parseable.
    BadValue {
        /// Which flag.
        flag: &'static str,
        /// What was entered.
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

/// Parses `args` (typically `env::args().skip(1)`) into a [`Command`].
///
/// # Errors
/// See [`ParseError`].
pub fn parse_args(args: &[String]) -> Result<Command, ParseError> {
    // Default subcommand: `subscribe` when the first token starts with `-`
    // begins or is a topic name. Allows both
    // `zerodds-spy subscribe -t Foo` as well as directly
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
            "--type" => {
                i += 1;
                out.type_name = Some(rest.get(i).ok_or(ParseError::MissingArg("type"))?.clone());
            }
            "--decode" => {
                out.decode = true;
            }
            "--type-file" => {
                i += 1;
                out.type_file = Some(
                    rest.get(i)
                        .ok_or(ParseError::MissingArg("type-file"))?
                        .clone(),
                );
            }
            "--dialect" => {
                i += 1;
                let v = rest.get(i).ok_or(ParseError::MissingArg("dialect"))?;
                out.dialect = Some(zerodds_ast_layout::Dialect::parse(v).ok_or_else(|| {
                    ParseError::BadValue {
                        flag: "dialect",
                        got: v.clone(),
                    }
                })?);
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

/// Formats a byte slice as hex, max `limit` bytes with a `…` suffix
/// if longer.
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
    fn parse_type_override() {
        let cmd = parse_args(&s(&["-t", "Track", "--type", "cuas::Track"])).unwrap();
        let Command::Subscribe(a) = cmd;
        assert_eq!(a.topic, "Track");
        assert_eq!(a.type_name.as_deref(), Some("cuas::Track"));
    }

    #[test]
    fn parse_no_type_override_defaults_none() {
        let cmd = parse_args(&s(&["-t", "Track"])).unwrap();
        let Command::Subscribe(a) = cmd;
        assert_eq!(a.type_name, None);
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
    fn parse_dialect() {
        let Command::Subscribe(args) =
            parse_args(&s(&["-t", "T", "--decode", "--dialect", "fastdds"])).unwrap();
        assert_eq!(args.dialect, Some(zerodds_ast_layout::Dialect::FastDds));
        let Command::Subscribe(args) = parse_args(&s(&["-t", "T"])).unwrap();
        assert_eq!(args.dialect, None);
        assert!(matches!(
            parse_args(&s(&["-t", "T", "--dialect", "nope"])),
            Err(ParseError::BadValue {
                flag: "dialect",
                ..
            })
        ));
    }

    #[test]
    fn parse_no_args_rejected() {
        assert!(matches!(parse_args(&[]), Err(ParseError::Missing)));
    }

    #[test]
    fn parse_unknown_subcommand_rejected() {
        // "publish" is neither a subcommand nor a flag → Unknown.
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
