// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CLI for `zerodds-durability-svc`.
//!
//! Every flag below is `--set` of its file key (`--domain N` is
//! `--set domain=N`), so it wins over the config file, exactly like every
//! other ZeroDDS daemon's convenience flags. The file is
//! `durability-svc.yaml` with `durability-svc.d/` in the system
//! configuration directory unless `--config` names another (see
//! `zerodds_dcps::cli::HELP`).

use std::string::String;
use std::vec::Vec;

/// Parsed CLI args.
#[derive(Debug, Clone, Default)]
pub struct CliArgs {
    /// `--domain <ID>`.
    pub domain: Option<i32>,
    /// `--store <KIND>`.
    pub store: Option<String>,
    /// `--path <PATH>`.
    pub path: Option<String>,
    /// `--auto`.
    pub auto: bool,
    /// `--topic <NAME>` (repeatable).
    pub topics: Vec<String>,
    /// `--version`.
    pub version: bool,
    /// `--help`.
    pub help: bool,
}

pub use zerodds_config::cli::CliError;
use zerodds_config::cli::Flags;

/// Help text.
pub const HELP_TEXT: &str = "\
zerodds-durability-svc — ZeroDDS Durability-Service daemon (ADR 0009)

USAGE:
  zerodds-durability-svc [OPTIONS]

OPTIONS:
  --domain N        DDS domain id. One daemon per domain.
  --store KIND      Cold-store adapter: sqlite (default) | file | lakehouse | postgres.
                    lakehouse (DuckDB) is opt-in: the cargo feature lakehouse.
                    postgres is opt-in: the cargo feature postgres.
  --path PATH       Store location. sqlite/lakehouse: a db file (omit = in-memory,
                    NOT durable across restart). file: a directory (required).
                    postgres: a libpq connection string (required), e.g.
                    postgres://zerodds@db/zerodds.
  --auto            Auto-serve every topic whose writers declare TRANSIENT or
                    PERSISTENT durability (no application-side code needed).
  --topic NAME      Explicitly serve a topic (repeatable). Combine with --auto
                    or use instead of it. At least one of --auto / --topic
                    (from any config source) is required.
  --version         Version info.
  --help            This help.

Each option above is `--set` of its file key (`--topic X` is
`--set +durability.topics=X`), so it wins over the file. The file is
durability-svc.yaml with durability-svc.d/ in the system configuration
directory unless --config names another. Per-topic type/keyed/contract
overrides are config-only — see zerodds-durability-svc(5).
";

/// Version string.
pub const VERSION_TEXT: &str = "zerodds-durability-svc 1.0";

impl CliArgs {
    /// The convenience flags as `PATH=VALUE` assignments, in the order the
    /// daemon applies them.
    #[must_use]
    pub fn sets(&self) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(v) = self.domain {
            out.push(format!("domain={v}"));
        }
        if let Some(v) = &self.store {
            out.push(format!("store={v}"));
        }
        if let Some(v) = &self.path {
            out.push(format!("path={v}"));
        }
        if self.auto {
            out.push("auto=true".to_string());
        }
        for topic in &self.topics {
            out.push(format!("+durability.topics={topic}"));
        }
        out
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
            "--store" => out.store = Some(flags.value(&flag)?),
            "--path" => out.path = Some(flags.value(&flag)?),
            "--auto" => out.auto = true,
            "--topic" => out.topics.push(flags.value(&flag)?),
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
    fn topic_repeats() {
        let p = parse(&args(&["--topic", "A", "--topic", "B"])).unwrap();
        assert_eq!(p.topics, vec!["A".to_string(), "B".to_string()]);
    }

    #[test]
    fn sets_maps_topic_to_plus_durability_topics() {
        let p = parse(&args(&["--topic", "A"])).unwrap();
        assert_eq!(p.sets(), vec!["+durability.topics=A".to_string()]);
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
