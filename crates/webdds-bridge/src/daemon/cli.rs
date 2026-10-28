// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CLI argument parser for `zerodds-webdds-bridged`.
//!
//! Hand-written, matching every other `zerodds-*-bridged` daemon: no
//! `clap` dependency. `--config`, `--set` and `--check-config` are
//! taken out before this parser runs (`zerodds_config::CommandLine`);
//! what remains are this daemon's convenience flags, which
//! [`CliArgs::apply_to`] adds to that command line so they sit in the flag
//! layer above every file.

use zerodds_config::{CommandLine, ConfigError};

/// Parsed CLI args.
#[derive(Debug, Clone, Default)]
pub struct CliArgs {
    /// `--listen <ADDR>` — HTTP bind address override.
    pub listen: Option<String>,
    /// `--domain <ID>` — DDS domain id override.
    pub domain: Option<i32>,
    /// `--type <NAME=FILE>` — a type source override (repeatable).
    pub types: Vec<String>,
    /// `--tls-cert <FILE>` — TLS cert file (PEM); enables `https://`.
    pub tls_cert: Option<String>,
    /// `--tls-key <FILE>` — TLS key file (PEM).
    pub tls_key: Option<String>,
    /// `--auth-token <SECRET>` — bearer token auth.
    pub auth_token: Option<String>,
    /// `--log-level <LEVEL>`.
    pub log_level: Option<String>,
    /// `--metrics <ADDR>` — Prometheus listen address.
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
zerodds-webdds-bridged 1.0 — OMG DDS-WEB 1.0 gateway (REST + SOAP)

USAGE:
    zerodds-webdds-bridged [OPTIONS]

OPTIONS:
    --listen <ADDR>          HTTP bind address (default 127.0.0.1:8080)
    --domain <ID>            DDS domain id (default 0)
    --type <NAME=FILE>       Type source override (multiple), e.g. M::Sensor=/path/t.idl
    --tls-cert <FILE>        TLS cert (PEM); enables https://
    --tls-key <FILE>         TLS key (PEM)
    --auth-token <SECRET>    Bearer token auth
    --log-level <LEVEL>      trace/debug/info/warn/error (default info)
    --metrics <ADDR>         Prometheus scrape endpoint
    --version                Version info
    --help                   Help

Each option above is `--set` of its file key, so it wins over the
files. The file is webdds-bridged.yaml with webdds-bridged.d/ in the
system configuration directory unless --config names another.

EXIT CODES:
    0   normal shutdown (SIGTERM/SIGINT)
    1   config error
    2   bind error (port in use)
    3   DDS discovery error
    4   TLS error

Spec: OMG Web-Enabled DDS 1.0, formal/2014-12-01.
";

/// Version string.
pub const VERSION_TEXT: &str = "zerodds-webdds-bridged 1.0";

impl CliArgs {
    /// The convenience flags as `PATH=VALUE` assignments, in the order
    /// the daemon applies them.
    #[must_use]
    pub fn sets(&self) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(v) = &self.listen {
            out.push(format!("listen={v}"));
        }
        if let Some(v) = self.domain {
            out.push(format!("domain={v}"));
        }
        if let Some(v) = &self.log_level {
            out.push(format!("log_level={v}"));
        }
        if let Some(v) = &self.auth_token {
            out.push("auth.mode=bearer".to_string());
            out.push(format!("auth.bearer_token={v}"));
        }
        if let Some(v) = &self.tls_cert {
            out.push(format!("tls.cert_file={v}"));
            out.push("tls.enabled=true".to_string());
        }
        if let Some(v) = &self.tls_key {
            out.push(format!("tls.key_file={v}"));
            out.push("tls.enabled=true".to_string());
        }
        if let Some(v) = &self.metrics {
            out.push(format!("metrics.listen={v}"));
            out.push("metrics.enabled=true".to_string());
        }
        out
    }

    /// Every `--type NAME=FILE` as `(name, file)`, in the order given; each
    /// is one entry appended to `types`. [`parse`] has refused a value
    /// without `=`.
    #[must_use]
    pub fn type_entries(&self) -> Vec<(&str, &str)> {
        self.types
            .iter()
            .filter_map(|entry| entry.split_once('='))
            .collect()
    }

    /// Adds the convenience flags to `command_line`: [`Self::sets`] as
    /// `--set` after any explicit one, then each `--type` as one entry
    /// appended to `types`.
    ///
    /// # Errors
    /// Whatever [`CommandLine::push_set`] or [`CommandLine::push_entry`]
    /// reports.
    pub fn apply_to(&self, command_line: &mut CommandLine) -> Result<(), ConfigError> {
        for assignment in self.sets() {
            command_line.push_set(&assignment)?;
        }
        for (name, file) in self.type_entries() {
            command_line.push_entry(
                "+types",
                &[("name", name), ("idl_file", file)],
                &format!("--type {name}={file}"),
            )?;
        }
        Ok(())
    }
}

/// Parses the CLI args (typically from
/// `std::env::args().skip(1).collect()`).
///
/// # Errors
/// See [`CliError`].
pub fn parse(args: &[String]) -> Result<CliArgs, CliError> {
    let mut out = CliArgs::default();
    let mut flags = Flags::new(args);
    while let Some(flag) = flags.next_flag() {
        match flag.name() {
            "--help" | "-h" => out.help = true,
            "--version" | "-V" => out.version = true,
            "--listen" => out.listen = Some(flags.value(&flag)?),
            "--domain" => out.domain = Some(flags.parsed(&flag)?),
            "--type" => {
                let v = flags.value(&flag)?;
                match v.split_once('=') {
                    Some((name, file)) if !name.trim().is_empty() && !file.trim().is_empty() => {
                        out.types.push(v);
                    }
                    _ => {
                        return Err(CliError::InvalidValue {
                            flag: "--type".to_string(),
                            value: v,
                        });
                    }
                }
            }
            "--tls-cert" => out.tls_cert = Some(flags.value(&flag)?),
            "--tls-key" => out.tls_key = Some(flags.value(&flag)?),
            "--auth-token" => out.auth_token = Some(flags.value(&flag)?),
            "--log-level" => out.log_level = Some(flags.value(&flag)?),
            "--metrics" => out.metrics = Some(flags.value(&flag)?),
            other => return Err(CliError::UnknownFlag(other.to_string())),
        }
    }
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn args(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn empty_args_yield_default() {
        let p = parse(&[]).unwrap();
        assert!(p.listen.is_none());
        assert!(!p.help);
    }

    #[test]
    fn help_and_version_no_arg() {
        assert!(parse(&args(&["--help"])).unwrap().help);
        assert!(parse(&args(&["--version"])).unwrap().version);
    }

    #[test]
    fn space_and_equals_form_both_work() {
        assert_eq!(
            parse(&args(&["--listen", "0.0.0.0:1"]))
                .unwrap()
                .listen
                .as_deref(),
            Some("0.0.0.0:1")
        );
        assert_eq!(
            parse(&args(&["--listen=0.0.0.0:2"]))
                .unwrap()
                .listen
                .as_deref(),
            Some("0.0.0.0:2")
        );
    }

    #[test]
    fn domain_rejects_non_int() {
        let err = parse(&args(&["--domain", "abc"])).unwrap_err();
        assert!(matches!(err, CliError::InvalidValue { .. }));
    }

    #[test]
    fn unknown_flag_rejected() {
        assert!(matches!(
            parse(&args(&["--nope"])).unwrap_err(),
            CliError::UnknownFlag(_)
        ));
    }

    #[test]
    fn missing_value_rejected() {
        assert!(matches!(
            parse(&args(&["--listen"])).unwrap_err(),
            CliError::MissingValue(_)
        ));
    }

    #[test]
    fn auth_token_becomes_bearer_sets() {
        let p = parse(&args(&["--auth-token", "t"])).unwrap();
        assert_eq!(p.sets(), ["auth.mode=bearer", "auth.bearer_token=t"]);
    }

    #[test]
    fn type_override_becomes_a_type_entry() {
        let p = parse(&args(&["--type", "M::Sensor=/x.idl", "--type=B=/b.idl"])).unwrap();
        assert!(p.sets().is_empty());
        assert_eq!(p.type_entries(), [("M::Sensor", "/x.idl"), ("B", "/b.idl")]);
    }

    /// Measured on the Linux test host: the README quickstart
    /// `--type 'Demo::Sensor=…/sensor.idl'` ended the daemon with "invalid
    /// types[0]: expected a map of settings".
    #[test]
    fn type_flags_reach_the_daemon_config() {
        let p = parse(&args(&[
            "--domain",
            "7",
            "--type",
            "M::Sensor=/x.idl",
            "--type",
            "M::Other=/y.idl",
        ]))
        .unwrap();
        let mut command_line = CommandLine::default();
        p.apply_to(&mut command_line).unwrap();
        let mut config = zerodds_config::Config::default();
        command_line.apply(&mut config).unwrap();
        let cfg = crate::daemon::config::DaemonConfig::from_config(Some(&config)).unwrap();
        assert_eq!(cfg.domain, 7);
        let types: Vec<(&str, Option<&str>)> = cfg
            .types
            .iter()
            .map(|t| (t.type_name.as_str(), t.idl_file.as_deref()))
            .collect();
        assert_eq!(
            types,
            [("M::Sensor", Some("/x.idl")), ("M::Other", Some("/y.idl"))]
        );
    }

    #[test]
    fn type_without_name_or_file_is_refused() {
        for bad in ["M::Sensor", "=/x.idl", "M::Sensor="] {
            assert!(
                matches!(
                    parse(&args(&["--type", bad])),
                    Err(CliError::InvalidValue { .. })
                ),
                "{bad}"
            );
        }
    }
}
