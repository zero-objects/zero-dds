// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CLI for `zerodds-zenoh-bridged`.

use std::string::String;
use std::vec::Vec;

/// Parsed CLI args.
#[derive(Debug, Clone, Default)]
pub struct CliArgs {
    /// `--domain <ID>`.
    pub domain: Option<i32>,
    /// `--mode <peer|client|router>`.
    pub mode: Option<String>,
    /// `--connect <ENDPOINT>` (repeatable).
    pub connect: Vec<String>,
    /// `--listen <ENDPOINT>` (repeatable).
    pub listen: Vec<String>,
    /// `--no-multicast-scouting`.
    pub no_multicast_scouting: bool,
    /// `--scope <SCOPE>`.
    pub scope: Option<String>,
    /// `--topic <DDS[:in|out|bidir]>` (repeatable).
    pub topics: Vec<String>,
    /// `--log-level <LEVEL>`.
    pub log_level: Option<String>,
    /// `--metrics <ADDR>`.
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
zerodds-zenoh-bridged 1.0 — DDS<->Zenoh bridge daemon

USAGE:
    zerodds-zenoh-bridged [OPTIONS]

OPTIONS:
    --domain <ID>            DDS domain ID (default 0)
    --mode <peer|client|router>  Zenoh session mode (default peer)
    --connect <ENDPOINT>      Zenoh endpoint to connect to (repeatable)
    --listen <ENDPOINT>       Zenoh endpoint to listen on (repeatable)
    --no-multicast-scouting   Disable Zenoh UDP multicast scouting
    --scope <SCOPE>           Zenoh key-expression scope (default: none,
                               matching zenoh-plugin-dds)
    --topic <DDS[:DIR]>       Topic override, DIR in in|out|bidir (repeatable)
    --log-level <LEVEL>       trace/debug/info/warn/error
    --metrics <ADDR>          Prometheus listener (enables metrics)
    --version                 Version info
    --help                    Help

Each option above is `--set` of its file key (`--domain X` is `--set
domain=X`), so it wins over the files. The file is zenoh-bridged.yaml with
zenoh-bridged.d/ in the system configuration directory unless --config
names another.

EXIT CODES:
    0   normal shutdown
    1   config error
    2   bind error (a metrics/admin listen address already in use)
    3   DDS discovery error
    4   Zenoh session error
";

/// Version string.
pub const VERSION_TEXT: &str = "zerodds-zenoh-bridged 1.0";

impl CliArgs {
    /// The convenience flags as `PATH=VALUE` assignments, in the order the
    /// daemon applies them.
    ///
    /// # Errors
    /// Never — kept `Result` for symmetry with the other bridge daemons'
    /// CLI, whose pairs of flags (TLS cert/key etc.) do need one.
    pub fn sets(&self) -> Result<Vec<String>, CliError> {
        let mut out = Vec::new();
        if let Some(v) = self.domain {
            out.push(format!("domain={v}"));
        }
        if let Some(v) = &self.mode {
            out.push(format!("zenoh.mode={v}"));
        }
        for ep in &self.connect {
            out.push(format!("+zenoh.connect={ep}"));
        }
        for ep in &self.listen {
            out.push(format!("+zenoh.listen={ep}"));
        }
        if self.no_multicast_scouting {
            out.push("zenoh.scouting.multicast.enabled=false".to_string());
        }
        if let Some(v) = &self.scope {
            out.push(format!("zenoh.scope={v}"));
        }
        if let Some(v) = &self.log_level {
            out.push(format!("log_level={v}"));
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
            "--domain" => out.domain = Some(flags.parsed(&flag)?),
            "--mode" => out.mode = Some(flags.value(&flag)?),
            "--connect" => out.connect.push(flags.value(&flag)?),
            "--listen" => out.listen.push(flags.value(&flag)?),
            "--no-multicast-scouting" => out.no_multicast_scouting = true,
            "--scope" => out.scope = Some(flags.value(&flag)?),
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
        assert!(parse(&[]).unwrap().domain.is_none());
    }

    #[test]
    fn domain_parses_int() {
        assert_eq!(parse(&args(&["--domain", "12"])).unwrap().domain, Some(12));
    }

    #[test]
    fn connect_is_repeatable() {
        let p = parse(&args(&["--connect", "tcp/a:1", "--connect", "tcp/b:2"])).unwrap();
        assert_eq!(
            p.connect,
            vec!["tcp/a:1".to_string(), "tcp/b:2".to_string()]
        );
    }

    #[test]
    fn sets_produces_plus_prefixed_list_entries() {
        let p = parse(&args(&["--connect", "tcp/a:1", "--topic", "T"])).unwrap();
        let sets = p.sets().unwrap();
        assert!(sets.contains(&"+zenoh.connect=tcp/a:1".to_string()));
        assert!(sets.contains(&"+topics=T".to_string()));
    }

    #[test]
    fn no_multicast_scouting_flag() {
        let p = parse(&args(&["--no-multicast-scouting"])).unwrap();
        assert!(p.no_multicast_scouting);
        assert!(
            p.sets()
                .unwrap()
                .contains(&"zenoh.scouting.multicast.enabled=false".to_string())
        );
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

    #[test]
    fn equals_form() {
        let p = parse(&args(&["--mode=client"])).unwrap();
        assert_eq!(p.mode.as_deref(), Some("client"));
    }
}
