// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CLI fuer `zerodds-coap-bridged`. Spec §2.

use std::string::String;
use std::vec::Vec;

/// Geparste CLI-Args.
#[derive(Debug, Clone, Default)]
pub struct CliArgs {
    /// `--config <FILE>`.
    pub config: Option<String>,
    /// `--bind <ADDR>`.
    pub bind: Option<String>,
    /// `--domain <ID>`.
    pub domain: Option<i32>,
    /// `--dtls-psk-id <ID>` — L5-Stub.
    pub dtls_psk_id: Option<String>,
    /// `--dtls-psk <SECRET>` — L5-Stub.
    pub dtls_psk: Option<String>,
    /// `--dtls-cert <FILE>` — L5-Stub.
    pub dtls_cert: Option<String>,
    /// `--dtls-key <FILE>` — L5-Stub.
    pub dtls_key: Option<String>,
    /// `--topic` (multi).
    pub topics: Vec<String>,
    /// `--log-level`.
    pub log_level: Option<String>,
    /// `--metrics`.
    pub metrics: Option<String>,
    /// `--version`.
    pub version: bool,
    /// `--help`.
    pub help: bool,
}

/// CLI-Fehler.
#[derive(Debug, Clone)]
pub enum CliError {
    /// Unbekannter Flag.
    UnknownFlag(String),
    /// Fehlender Value.
    MissingValue(String),
    /// Wert nicht parsbar.
    InvalidValue {
        /// Flag.
        flag: String,
        /// Wert.
        value: String,
    },
}

impl core::fmt::Display for CliError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnknownFlag(n) => write!(f, "unknown flag: {n}"),
            Self::MissingValue(n) => write!(f, "flag {n} needs a value"),
            Self::InvalidValue { flag, value } => write!(f, "bad value for {flag}: {value}"),
        }
    }
}

impl std::error::Error for CliError {}

/// Help-Text.
pub const HELP_TEXT: &str = "\
zerodds-coap-bridged 1.0 — DDS↔CoAP-Bridge-Daemon

USAGE:
    zerodds-coap-bridged [OPTIONS]

OPTIONS:
    --config <FILE>        Path zur Config-File (YAML)
    --bind <ADDR>          UDP-Bind-Address (Default 0.0.0.0:5683)
    --domain <ID>          DDS-Domain-ID (Default 0)
    --dtls-psk-id <ID>     DTLS-PSK-Identity — L5-stub
    --dtls-psk <SECRET>    DTLS-PSK-Secret — L5-stub
    --dtls-cert <FILE>     DTLS-Server-Cert — L5-stub
    --dtls-key <FILE>      DTLS-Server-Key — L5-stub
    --topic <DDS:URI>      Topic-Override (mehrfach)
    --log-level <LEVEL>    trace/debug/info/warn/error
    --metrics <ADDR>       Prometheus-Listen — L5-stub
    --version              Versions-Info
    --help                 Hilfe

EXIT-CODES:
    0   normaler Shutdown
    1   Config-Fehler
    2   Bind-Fehler (Port belegt)
    3   DDS-Discovery-Fehler
    4   DTLS-Setup-Fehler
";

/// Versions-String.
pub const VERSION_TEXT: &str = "zerodds-coap-bridged 1.0";

/// Parst Args.
///
/// # Errors
/// [`CliError`].
pub fn parse(args: &[String]) -> Result<CliArgs, CliError> {
    let mut out = CliArgs::default();
    let mut i = 0;
    while i < args.len() {
        let raw = &args[i];
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
            "--bind" => out.bind = Some(take_value(&mut i, "--bind")?),
            "--domain" => {
                let v = take_value(&mut i, "--domain")?;
                out.domain = Some(v.parse().map_err(|_| CliError::InvalidValue {
                    flag: "--domain".to_string(),
                    value: v,
                })?);
            }
            "--dtls-psk-id" => out.dtls_psk_id = Some(take_value(&mut i, "--dtls-psk-id")?),
            "--dtls-psk" => out.dtls_psk = Some(take_value(&mut i, "--dtls-psk")?),
            "--dtls-cert" => out.dtls_cert = Some(take_value(&mut i, "--dtls-cert")?),
            "--dtls-key" => out.dtls_key = Some(take_value(&mut i, "--dtls-key")?),
            "--topic" => out.topics.push(take_value(&mut i, "--topic")?),
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
    fn empty_args_default() {
        assert!(parse(&[]).unwrap().bind.is_none());
    }

    #[test]
    fn bind_addr_parsed() {
        let p = parse(&args(&["--bind", "0.0.0.0:5683"])).unwrap();
        assert_eq!(p.bind.as_deref(), Some("0.0.0.0:5683"));
    }

    #[test]
    fn equals_form() {
        let p = parse(&args(&["--bind=127.0.0.1:5684"])).unwrap();
        assert_eq!(p.bind.as_deref(), Some("127.0.0.1:5684"));
    }

    #[test]
    fn domain_parses_int() {
        assert_eq!(parse(&args(&["--domain", "12"])).unwrap().domain, Some(12));
    }

    #[test]
    fn unknown_flag_rejected() {
        assert!(matches!(
            parse(&args(&["--nope"])).unwrap_err(),
            CliError::UnknownFlag(_)
        ));
    }

    #[test]
    fn help_short_form() {
        assert!(parse(&args(&["-h"])).unwrap().help);
    }
}
