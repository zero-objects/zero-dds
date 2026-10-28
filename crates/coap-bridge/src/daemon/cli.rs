// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CLI for `zerodds-coap-bridged`. Spec §2.

use std::string::String;
use std::vec::Vec;

/// Parsed CLI args.
#[derive(Debug, Clone, Default)]
pub struct CliArgs {
    /// `--bind <ADDR>`.
    pub bind: Option<String>,
    /// `--domain <ID>`.
    pub domain: Option<i32>,
    /// `--dtls-psk-id <ID>` — DTLS PSK identity (Spec §7.1).
    pub dtls_psk_id: Option<String>,
    /// `--dtls-psk <SECRET>` — DTLS PSK secret, hex (Spec §7.1).
    pub dtls_psk: Option<String>,
    /// `--dtls-cert <FILE>` — DTLS server certificate, PEM (Spec §7.1).
    pub dtls_cert: Option<String>,
    /// `--dtls-key <FILE>` — DTLS server key, PKCS#8 PEM (Spec §7.1).
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

pub use zerodds_config::cli::CliError;
use zerodds_config::cli::Flags;

/// Help text.
pub const HELP_TEXT: &str = "\
zerodds-coap-bridged 1.0 — DDS↔CoAP bridge daemon

USAGE:
    zerodds-coap-bridged [OPTIONS]

OPTIONS:
    --bind <ADDR>          UDP bind address (default 0.0.0.0:5683)
    --domain <ID>          DDS domain ID (default 0)
    --dtls-psk-id <ID>     DTLS PSK identity (enables coaps://)
    --dtls-psk <SECRET>    DTLS PSK secret, hex (enables coaps://)
    --dtls-cert <FILE>     DTLS server cert, PEM (enables coaps://)
    --dtls-key <FILE>      DTLS server key, PKCS#8 PEM
    --topic <DDS:URI>      Topic override (repeatable)
    --log-level <LEVEL>    trace/debug/info/warn/error
    --metrics <ADDR>       Prometheus listener (enables metrics)
    --version              Version info
    --help                 Help

Each option above is `--set` of its file key (`--bind X` is
`--set coap.bind=X`), so it wins over the files. The file is
coap-bridged.yaml with coap-bridged.d/ in the system configuration
directory unless --config names another.

EXIT CODES:
    0   normal shutdown
    1   config error
    2   bind error (port in use)
    3   DDS discovery error
    4   DTLS setup error
";

/// Version string.
pub const VERSION_TEXT: &str = "zerodds-coap-bridged 1.0";

fn unpaired(given: &str, needs: &str) -> CliError {
    CliError::Unpaired {
        given: given.to_string(),
        needs: needs.to_string(),
    }
}

impl CliArgs {
    /// The convenience flags as `PATH=VALUE` assignments, in the order the
    /// daemon applies them.
    ///
    /// # Errors
    /// [`CliError::Unpaired`] when only one flag of a pair is given
    /// (`--dtls-cert`/`--dtls-key`, `--dtls-psk-id`/`--dtls-psk`).
    pub fn sets(&self) -> Result<Vec<String>, CliError> {
        let mut out = Vec::new();
        if let Some(v) = &self.bind {
            out.push(format!("coap.bind={v}"));
        }
        if let Some(v) = self.domain {
            out.push(format!("domain={v}"));
        }
        if let Some(v) = &self.log_level {
            out.push(format!("log_level={v}"));
        }
        match (&self.dtls_cert, &self.dtls_key) {
            (Some(cert), Some(key)) => {
                out.push(format!("coap.dtls.cert_file={cert}"));
                out.push(format!("coap.dtls.key_file={key}"));
                out.push("coap.dtls.enabled=true".to_string());
            }
            (None, None) => {}
            (Some(_), None) => return Err(unpaired("--dtls-cert", "--dtls-key")),
            (None, Some(_)) => return Err(unpaired("--dtls-key", "--dtls-cert")),
        }
        match (&self.dtls_psk_id, &self.dtls_psk) {
            (Some(id), Some(secret)) => {
                out.push(format!("+coap.dtls.psk={id}={secret}"));
                out.push("coap.dtls.enabled=true".to_string());
            }
            (None, None) => {}
            (Some(_), None) => return Err(unpaired("--dtls-psk-id", "--dtls-psk")),
            (None, Some(_)) => return Err(unpaired("--dtls-psk", "--dtls-psk-id")),
        }
        if let Some(v) = &self.metrics {
            out.push(format!("metrics.listen={v}"));
            out.push("metrics.enabled=true".to_string());
        }
        for topic in &self.topics {
            out.push(format!("+topics={topic}"));
        }
        Ok(out)
    }
}

/// Parses args.
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
            "--bind" => out.bind = Some(flags.value(&flag)?),
            "--domain" => out.domain = Some(flags.parsed(&flag)?),
            "--dtls-psk-id" => out.dtls_psk_id = Some(flags.value(&flag)?),
            "--dtls-psk" => out.dtls_psk = Some(flags.value(&flag)?),
            "--dtls-cert" => out.dtls_cert = Some(flags.value(&flag)?),
            "--dtls-key" => out.dtls_key = Some(flags.value(&flag)?),
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
