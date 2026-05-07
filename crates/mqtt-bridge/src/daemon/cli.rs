// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CLI fuer `zerodds-mqtt-bridged`. Spec §2.

use std::string::String;
use std::vec::Vec;

/// Geparste CLI-Args.
#[derive(Debug, Clone, Default)]
pub struct CliArgs {
    /// `--config <FILE>`.
    pub config: Option<String>,
    /// `--broker <URL>`.
    pub broker: Option<String>,
    /// `--client-id <ID>`.
    pub client_id: Option<String>,
    /// `--domain <ID>`.
    pub domain: Option<i32>,
    /// `--username`.
    pub username: Option<String>,
    /// `--password`.
    pub password: Option<String>,
    /// `--tls-ca`.
    pub tls_ca: Option<String>,
    /// `--tls-cert`.
    pub tls_cert: Option<String>,
    /// `--tls-key`.
    pub tls_key: Option<String>,
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

/// Fehler beim CLI-Parse.
#[derive(Debug, Clone)]
pub enum CliError {
    /// Unbekannter Flag.
    UnknownFlag(String),
    /// Flag braucht Value.
    MissingValue(String),
    /// Wert nicht parsbar.
    InvalidValue {
        /// Flag-Name.
        flag: String,
        /// Roher Wert.
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
zerodds-mqtt-bridged 1.0 — DDS↔MQTT-5-Bridge-Daemon

USAGE:
    zerodds-mqtt-bridged [OPTIONS]

OPTIONS:
    --config <FILE>        Path zur Config-File (YAML)
    --broker <URL>         MQTT-Broker-URL (mqtt://, mqtts://)
    --client-id <ID>       MQTT-Client-Id
    --domain <ID>          DDS-Domain-ID (Default 0)
    --username <USER>      MQTT-Username
    --password <PASS>      MQTT-Password
    --tls-ca <FILE>        CA-Cert — L5-stub
    --tls-cert <FILE>      Client-Cert — L5-stub
    --tls-key <FILE>       Client-Key — L5-stub
    --topic <DDS:MQTT>     Topic-Override (mehrfach)
    --log-level <LEVEL>    trace/debug/info/warn/error
    --metrics <ADDR>       Prometheus-Listen — L5-stub
    --version              Versions-Info
    --help                 Hilfe

EXIT-CODES:
    0   normaler Shutdown
    1   Config-Fehler
    2   Broker-Connect-Fehler
    3   DDS-Discovery-Fehler
    4   TLS-Fehler
    5   Auth-Fehler
";

/// Versions-String.
pub const VERSION_TEXT: &str = "zerodds-mqtt-bridged 1.0";

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
            "--broker" => out.broker = Some(take_value(&mut i, "--broker")?),
            "--client-id" => out.client_id = Some(take_value(&mut i, "--client-id")?),
            "--domain" => {
                let v = take_value(&mut i, "--domain")?;
                out.domain = Some(v.parse().map_err(|_| CliError::InvalidValue {
                    flag: "--domain".to_string(),
                    value: v,
                })?);
            }
            "--username" => out.username = Some(take_value(&mut i, "--username")?),
            "--password" => out.password = Some(take_value(&mut i, "--password")?),
            "--tls-ca" => out.tls_ca = Some(take_value(&mut i, "--tls-ca")?),
            "--tls-cert" => out.tls_cert = Some(take_value(&mut i, "--tls-cert")?),
            "--tls-key" => out.tls_key = Some(take_value(&mut i, "--tls-key")?),
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
        assert!(parse(&[]).unwrap().broker.is_none());
    }

    #[test]
    fn broker_url_parsed() {
        let p = parse(&args(&["--broker", "mqtt://x:1883"])).unwrap();
        assert_eq!(p.broker.as_deref(), Some("mqtt://x:1883"));
    }

    #[test]
    fn equals_form() {
        let p = parse(&args(&["--client-id=zb-001"])).unwrap();
        assert_eq!(p.client_id.as_deref(), Some("zb-001"));
    }

    #[test]
    fn domain_int() {
        assert_eq!(parse(&args(&["--domain", "42"])).unwrap().domain, Some(42));
    }

    #[test]
    fn unknown_flag_rejected() {
        assert!(matches!(
            parse(&args(&["--xx"])).unwrap_err(),
            CliError::UnknownFlag(_)
        ));
    }

    #[test]
    fn help_short_form() {
        assert!(parse(&args(&["-h"])).unwrap().help);
    }
}
