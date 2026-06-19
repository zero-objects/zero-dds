// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CLI argument parser for `zerodds-ws-bridged`.
//!
//! Spec: `zerodds-ws-bridge-1.0.md` §2.
//!
//! Deliberately hand-written — no `clap` dep in the workspace, no
//! `getopts` heritage. Accepted forms:
//!
//! * `--flag value`
//! * `--flag=value`
//! * `--bool-flag`            (no-arg, for `--help` / `--version`)
//! * repetitions for `--topic` (multi-value)

use std::string::String;
use std::vec::Vec;

/// Parsed CLI args.
#[derive(Debug, Clone, Default)]
pub struct CliArgs {
    /// `--config <FILE>` — path to the YAML config.
    pub config: Option<String>,
    /// `--listen <ADDR>` — override for the bind address.
    pub listen: Option<String>,
    /// `--domain <ID>` — DDS domain id override.
    pub domain: Option<i32>,
    /// `--topic <NAME[:KEY]>` — single-topic overrides (multiple).
    pub topics: Vec<String>,
    /// `--tls-cert <FILE>` — TLS cert file. L5 stub.
    pub tls_cert: Option<String>,
    /// `--tls-key <FILE>` — TLS key file. L5 stub.
    pub tls_key: Option<String>,
    /// `--auth-token <SECRET>` — bearer token auth. L5 stub.
    pub auth_token: Option<String>,
    /// `--log-level <LEVEL>` — `trace|debug|info|warn|error`.
    pub log_level: Option<String>,
    /// `--metrics <ADDR>` — Prometheus listen address. L5 stub.
    pub metrics: Option<String>,
    /// `--version` — print version info + exit.
    pub version: bool,
    /// `--help` — help text + exit.
    pub help: bool,
}

/// Error during CLI parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliError {
    /// Unknown flag.
    UnknownFlag(String),
    /// The flag needs a value but has none.
    MissingValue(String),
    /// Value not parseable (e.g. `--domain abc`).
    InvalidValue {
        /// Flag name.
        flag: String,
        /// Raw value.
        value: String,
    },
}

impl core::fmt::Display for CliError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnknownFlag(name) => write!(f, "unknown flag: {name}"),
            Self::MissingValue(name) => write!(f, "flag {name} requires a value"),
            Self::InvalidValue { flag, value } => {
                write!(f, "flag {flag} got invalid value: {value}")
            }
        }
    }
}

impl std::error::Error for CliError {}

/// Help text per Spec §2.
pub const HELP_TEXT: &str = "\
zerodds-ws-bridged 1.0 — DDS↔WebSocket bridge daemon

USAGE:
    zerodds-ws-bridged [OPTIONS]

OPTIONS:
    --config <FILE>         Path to the config file (YAML)
    --listen <ADDR>         Bind address (default 0.0.0.0:8080)
    --domain <ID>           DDS domain id (default 0)
    --topic <NAME>          Single-topic override (multiple)
    --tls-cert <FILE>       TLS cert (PEM); enables wss:// — L5 stub
    --tls-key <FILE>        TLS key (PEM) — L5 stub
    --auth-token <SECRET>   Bearer token auth — L5 stub
    --log-level <LEVEL>     trace/debug/info/warn/error (default info)
    --metrics <ADDR>        Prometheus scrape endpoint — L5 stub
    --version               Version info
    --help                  Help

EXIT CODES:
    0   normal shutdown (SIGTERM/SIGINT)
    1   config error
    2   bind error (port in use)
    3   DDS discovery error
    4   TLS error

Spec: docs/specs/zerodds-ws-bridge-1.0.md
";

/// Version string.
pub const VERSION_TEXT: &str = "zerodds-ws-bridged 1.0";

/// Parses the CLI args (typically from `std::env::args().skip(1).collect()`).
///
/// # Errors
/// See [`CliError`].
pub fn parse(args: &[String]) -> Result<CliArgs, CliError> {
    let mut out = CliArgs::default();
    let mut i = 0;
    while i < args.len() {
        let raw = &args[i];
        // Split `--flag=value`.
        let (flag, inline) = match raw.split_once('=') {
            Some((k, v)) => (k.to_string(), Some(v.to_string())),
            None => (raw.clone(), None),
        };
        let take_value = |i: &mut usize, flag: &str| -> Result<String, CliError> {
            if let Some(v) = inline.clone() {
                Ok(v)
            } else {
                *i += 1;
                args.get(*i)
                    .cloned()
                    .ok_or_else(|| CliError::MissingValue(flag.to_string()))
            }
        };
        match flag.as_str() {
            "--help" | "-h" => out.help = true,
            "--version" | "-V" => out.version = true,
            "--config" => out.config = Some(take_value(&mut i, "--config")?),
            "--listen" => out.listen = Some(take_value(&mut i, "--listen")?),
            "--domain" => {
                let v = take_value(&mut i, "--domain")?;
                out.domain = Some(v.parse().map_err(|_| CliError::InvalidValue {
                    flag: "--domain".to_string(),
                    value: v,
                })?);
            }
            "--topic" => {
                out.topics.push(take_value(&mut i, "--topic")?);
            }
            "--tls-cert" => out.tls_cert = Some(take_value(&mut i, "--tls-cert")?),
            "--tls-key" => out.tls_key = Some(take_value(&mut i, "--tls-key")?),
            "--auth-token" => out.auth_token = Some(take_value(&mut i, "--auth-token")?),
            "--log-level" => out.log_level = Some(take_value(&mut i, "--log-level")?),
            "--metrics" => out.metrics = Some(take_value(&mut i, "--metrics")?),
            other => return Err(CliError::UnknownFlag(other.to_string())),
        }
        i += 1;
    }
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn args(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn empty_args_yield_default() {
        let p = parse(&[]).unwrap();
        assert!(p.config.is_none());
        assert!(!p.help);
    }

    #[test]
    fn help_and_version_no_arg() {
        assert!(parse(&args(&["--help"])).unwrap().help);
        assert!(parse(&args(&["--version"])).unwrap().version);
    }

    #[test]
    fn space_separated_value() {
        let p = parse(&args(&["--config", "/tmp/c.yaml"])).unwrap();
        assert_eq!(p.config.as_deref(), Some("/tmp/c.yaml"));
    }

    #[test]
    fn equals_form_value() {
        let p = parse(&args(&["--listen=0.0.0.0:9000"])).unwrap();
        assert_eq!(p.listen.as_deref(), Some("0.0.0.0:9000"));
    }

    #[test]
    fn domain_parses_int() {
        assert_eq!(parse(&args(&["--domain", "7"])).unwrap().domain, Some(7));
    }

    #[test]
    fn domain_rejects_non_int() {
        let err = parse(&args(&["--domain", "abc"])).unwrap_err();
        assert!(matches!(err, CliError::InvalidValue { .. }));
    }

    #[test]
    fn topic_repeats_collected() {
        let p = parse(&args(&["--topic", "A", "--topic", "B"])).unwrap();
        assert_eq!(p.topics, vec!["A".to_string(), "B".to_string()]);
    }

    #[test]
    fn unknown_flag_rejected() {
        let err = parse(&args(&["--nope"])).unwrap_err();
        assert!(matches!(err, CliError::UnknownFlag(_)));
    }

    #[test]
    fn missing_value_rejected() {
        let err = parse(&args(&["--config"])).unwrap_err();
        assert!(matches!(err, CliError::MissingValue(_)));
    }
}
