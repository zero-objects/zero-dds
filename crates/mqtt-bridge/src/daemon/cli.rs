// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CLI for `zerodds-mqtt-bridged`. Spec §2.

use std::string::String;
use std::vec::Vec;

/// Parsed CLI args.
#[derive(Debug, Clone, Default)]
pub struct CliArgs {
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

pub use zerodds_config::cli::CliError;
use zerodds_config::cli::Flags;

/// Help text.
pub const HELP_TEXT: &str = "\
zerodds-mqtt-bridged 1.0 — DDS↔MQTT-5 bridge daemon

USAGE:
    zerodds-mqtt-bridged [OPTIONS]

OPTIONS:
    --broker <URL>         MQTT broker URL (mqtt://, mqtts://, ws://, wss://)
    --client-id <ID>       MQTT client ID
    --domain <ID>          DDS domain ID (default 0)
    --username <USER>      MQTT username
    --password <PASS>      MQTT password
    --tls-ca <FILE>        CA cert of the broker (enables mqtts)
    --tls-cert <FILE>      Client cert (mTLS)
    --tls-key <FILE>       Client key (mTLS)
    --topic <DDS:MQTT>     Topic override (repeatable)
    --log-level <LEVEL>    trace/debug/info/warn/error
    --metrics <ADDR>       Prometheus listen (enables metrics)
    --version              Version info
    --help                 Help

Each option above is `--set` of its file key (`--broker X` is
`--set mqtt.broker_url=X`), so it wins over the files. The file is
mqtt-bridged.yaml with mqtt-bridged.d/ in the system configuration
directory unless --config names another.

EXIT CODES:
    0   normal shutdown
    1   config error
    2   broker connect error
    3   DDS discovery error
    4   TLS error
    5   auth error
";

/// Versions-String.
pub const VERSION_TEXT: &str = "zerodds-mqtt-bridged 1.0";

impl CliArgs {
    /// The convenience flags as `PATH=VALUE` assignments, in the order the
    /// daemon applies them.
    #[must_use]
    pub fn sets(&self) -> Vec<String> {
        let mut out = Vec::new();
        let mut push = |key: &str, value: &Option<String>| {
            if let Some(v) = value {
                out.push(format!("{key}={v}"));
            }
        };
        push("mqtt.broker_url", &self.broker);
        push("mqtt.client_id", &self.client_id);
        push("domain", &self.domain.map(|d| d.to_string()));
        push("mqtt.username", &self.username);
        push("mqtt.password", &self.password);
        push("mqtt.tls.ca_file", &self.tls_ca);
        push("mqtt.tls.cert_file", &self.tls_cert);
        push("mqtt.tls.key_file", &self.tls_key);
        push("log_level", &self.log_level);
        push("metrics.listen", &self.metrics);
        if self.tls_ca.is_some() || self.tls_cert.is_some() || self.tls_key.is_some() {
            out.push("mqtt.tls.enabled=true".to_string());
        }
        if self.metrics.is_some() {
            out.push("metrics.enabled=true".to_string());
        }
        for topic in &self.topics {
            out.push(format!("+topics={topic}"));
        }
        out
    }
}

/// Parst Args.
///
/// # Errors
/// [`CliError`].
pub fn parse(args: &[String]) -> Result<CliArgs, CliError> {
    let mut out = CliArgs::default();
    let mut flags = Flags::new(args);
    while let Some(flag) = flags.next_flag() {
        match flag.name() {
            "--help" | "-h" => out.help = true,
            "--version" | "-V" => out.version = true,
            "--broker" => out.broker = Some(flags.value(&flag)?),
            "--client-id" => out.client_id = Some(flags.value(&flag)?),
            "--domain" => out.domain = Some(flags.parsed(&flag)?),
            "--username" => out.username = Some(flags.value(&flag)?),
            "--password" => out.password = Some(flags.value(&flag)?),
            "--tls-ca" => out.tls_ca = Some(flags.value(&flag)?),
            "--tls-cert" => out.tls_cert = Some(flags.value(&flag)?),
            "--tls-key" => out.tls_key = Some(flags.value(&flag)?),
            "--topic" => out.topics.push(flags.value(&flag)?),
            "--log-level" => out.log_level = Some(flags.value(&flag)?),
            "--metrics" => out.metrics = Some(flags.value(&flag)?),
            other => return Err(CliError::UnknownFlag(other.to_string())),
        }
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

    /// `--tls-*`, `--topic` and `--metrics` were parsed and then dropped by
    /// the daemon; as `--set` entries they reach the configuration.
    #[test]
    fn every_convenience_flag_becomes_a_set() {
        let p = parse(&args(&[
            "--broker",
            "mqtts://b:8883",
            "--tls-ca",
            "/ca.pem",
            "--topic",
            "Chat::Message:chat/msg",
            "--metrics",
            "127.0.0.1:9",
        ]))
        .unwrap();
        let sets = p.sets();
        for want in [
            "mqtt.broker_url=mqtts://b:8883",
            "mqtt.tls.ca_file=/ca.pem",
            "mqtt.tls.enabled=true",
            "metrics.listen=127.0.0.1:9",
            "metrics.enabled=true",
            "+topics=Chat::Message:chat/msg",
        ] {
            assert!(sets.iter().any(|s| s == want), "missing {want} in {sets:?}");
        }
    }
}
