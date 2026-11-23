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
//!
//! `--config`, `--set` and `--check-config` are taken out before this parser
//! runs (`zerodds_config::CommandLine`); what remains are this daemon's
//! convenience flags, which [`CliArgs::sets`] turns into `--set` entries so
//! they sit in the flag layer above every file.

use std::string::String;
use std::vec::Vec;

/// Parsed CLI args.
#[derive(Debug, Clone, Default)]
pub struct CliArgs {
    /// `--listen <ADDR>` — override for the bind address.
    pub listen: Option<String>,
    /// `--domain <ID>` — DDS domain id override.
    pub domain: Option<i32>,
    /// `--topic <NAME[:WS_PATH]>` — one more topic entry per flag: the DDS
    /// topic name, and behind a single `:` the path of its WebSocket
    /// endpoint (§4.2) where it is not the default path.
    pub topics: Vec<String>,
    /// `--tls-cert <FILE>` — TLS cert file (PEM); enables `wss://`.
    pub tls_cert: Option<String>,
    /// `--tls-key <FILE>` — TLS key file (PEM).
    pub tls_key: Option<String>,
    /// `--auth-token <SECRET>` — bearer token auth.
    pub auth_token: Option<String>,
    /// `--log-level <LEVEL>` — `trace|debug|info|warn|error`.
    pub log_level: Option<String>,
    /// `--metrics <ADDR>` — Prometheus listen address.
    pub metrics: Option<String>,
    /// `--version` — print version info + exit.
    pub version: bool,
    /// `--help` — help text + exit.
    pub help: bool,
}

pub use zerodds_config::cli::CliError;
use zerodds_config::cli::Flags;

/// Help text per Spec §2.
pub const HELP_TEXT: &str = "\
zerodds-ws-bridged — DDS↔WebSocket bridge daemon

USAGE:
    zerodds-ws-bridged [OPTIONS]

OPTIONS:
    --config <FILE>         Base file, plus FILE.d/
    --set <PATH=VALUE>      One value above all files (repeatable)
    --check-config          Check the configuration, print every value
                            with its origin, exit (0 valid, 1 invalid,
                            2 a file that does not load)
    --listen <ADDR>         Bind address (default 127.0.0.1:8080)
    --domain <ID>           DDS domain id (default 0)
    --topic <NAME[:WS_PATH]>
                            One more topic (repeatable); WS_PATH is its
                            path, default /topics/<slug>
    --tls-cert <FILE>       TLS cert (PEM); enables wss://
    --tls-key <FILE>        TLS key (PEM)
    --auth-token <SECRET>   Bearer token auth
    --log-level <LEVEL>     trace/debug/info/warn/error (default info)
    --metrics <ADDR>        Prometheus scrape endpoint (default off)
    --version               Version info
    --help                  Help

Each option from --listen on is `--set` of its file key (`--listen X`
is `--set listen=X`), so it wins over the files. The file is
ws-bridged.yaml with ws-bridged.d/ in the system configuration
directory unless --config names another.

EXIT CODES:
    0   normal shutdown (SIGTERM/SIGINT)
    1   config error
    2   bind error (port in use)
    3   DDS error (the participant does not start)
    4   TLS error (certificate or key does not load)

Spec: docs/specs/zerodds-ws-bridge-1.0.md
";

/// The line `--version` prints: the program and the version of its crate.
pub const VERSION_TEXT: &str = concat!("zerodds-ws-bridged ", env!("CARGO_PKG_VERSION"));

impl CliArgs {
    /// The convenience flags as `PATH=VALUE` assignments, in the order the
    /// daemon applies them.
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
        for name in &self.topics {
            out.push(format!("+topics={name}"));
        }
        out
    }
}

/// Parses the CLI args (typically from `std::env::args().skip(1).collect()`).
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
            "--topic" => {
                out.topics.push(flags.value(&flag)?);
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
#[allow(clippy::expect_used, clippy::unwrap_used)]
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
    fn space_separated_value() {
        let p = parse(&args(&["--listen", "0.0.0.0:1"])).unwrap();
        assert_eq!(p.listen.as_deref(), Some("0.0.0.0:1"));
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
        let err = parse(&args(&["--listen"])).unwrap_err();
        assert!(matches!(err, CliError::MissingValue(_)));
    }

    /// Spec §2: the help names every option and every exit code of the
    /// specification, and the default of `--listen` is the one the daemon
    /// starts with.
    #[test]
    fn wsbridge_2_the_help_names_the_options_the_exit_codes_and_the_default() {
        let spec = include_str!("../../../../docs/specs/zerodds-ws-bridge-1.0.md");
        let section = &spec[spec.find("## §2 CLI surface").expect("§2")..];
        let block = &section[..section.find("## §3").expect("§3")];
        let mut options = 0;
        for line in block.lines() {
            let Some(rest) = line.trim_start().strip_prefix("--") else {
                continue;
            };
            let name = rest.split([' ', '=']).next().expect("a name");
            if !line.starts_with("  --") {
                continue;
            }
            options += 1;
            assert!(
                HELP_TEXT.contains(&format!("--{name} "))
                    || HELP_TEXT.contains(&format!("--{name}\n")),
                "the help does not name --{name}"
            );
        }
        assert_eq!(options, 13, "the options of §2");
        for code in ["0   normal", "1   config", "2   bind", "3   DDS", "4   TLS"] {
            assert!(block.contains(code), "§2 names `{code}`");
            assert!(HELP_TEXT.contains(code), "the help names `{code}`");
        }
        let default = crate::daemon::config::DaemonConfig::default_for_dev().listen;
        assert_eq!(default, "127.0.0.1:8080");
        assert!(HELP_TEXT.contains(&format!("(default {default})")));
        assert!(block.contains(&format!("(default {default})")));
    }

    /// Spec §2: every short form is `--set` of its file key.
    #[test]
    fn wsbridge_2_every_short_form_is_a_set_of_its_key() {
        let p = parse(&args(&[
            "--listen=[::1]:9",
            "--domain=7",
            "--log-level=debug",
            "--auth-token=t",
            "--tls-cert=/c.pem",
            "--tls-key=/k.pem",
            "--metrics=127.0.0.1:9",
            "--topic=Chat::Message:/chat",
            "--topic=Plain",
        ]))
        .unwrap();
        assert_eq!(
            p.sets(),
            [
                "listen=[::1]:9",
                "domain=7",
                "log_level=debug",
                "auth.mode=bearer",
                "auth.bearer_token=t",
                "tls.cert_file=/c.pem",
                "tls.enabled=true",
                "tls.key_file=/k.pem",
                "tls.enabled=true",
                "metrics.listen=127.0.0.1:9",
                "metrics.enabled=true",
                "+topics=Chat::Message:/chat",
                "+topics=Plain",
            ]
        );
    }

    #[test]
    fn convenience_flags_become_sets() {
        let p = parse(&args(&[
            "--listen",
            "0.0.0.0:1",
            "--auth-token",
            "t",
            "--topic",
            "A",
        ]))
        .unwrap();
        assert_eq!(
            p.sets(),
            [
                "listen=0.0.0.0:1",
                "auth.mode=bearer",
                "auth.bearer_token=t",
                "+topics=A"
            ]
        );
    }
}
