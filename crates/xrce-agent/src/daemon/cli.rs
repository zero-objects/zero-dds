// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CLI for `zerodds-xrce-agentd`.
//!
//! `--config`/`--set`/`--check-config` are handled by
//! `zerodds_dcps::cli::Configured` (shared across every ZeroDDS daemon);
//! this module is the daemon's own convenience flags plus `--help`/
//! `--version`, exactly mirroring `crates/coap-bridge`'s `daemon::cli`.

use std::string::{String, ToString};
use std::vec::Vec;

/// Parsed CLI args.
#[derive(Debug, Clone, Default)]
pub struct CliArgs {
    /// `--listen <udp|tcp>:<ADDR>` (multi) — `--set +listeners=...`.
    pub listen: Vec<String>,
    /// `--log-level`.
    pub log_level: Option<String>,
    /// `--version`.
    pub version: bool,
    /// `--help`.
    pub help: bool,
}

pub use zerodds_config::cli::CliError;
use zerodds_config::cli::Flags;

/// Help text.
pub const HELP_TEXT: &str = "\
zerodds-xrce-agentd 1.0 — DDS-XRCE agent daemon

USAGE:
    zerodds-xrce-agentd [OPTIONS]

OPTIONS:
    --listen <udp|tcp>:<ADDR>   Add a listener (repeatable; default is one
                                UDP listener on 0.0.0.0:2018, eProsima's
                                own default agent port)
    --log-level <LEVEL>          trace/debug/info/warn/error
    --version                    Version info
    --help                       Help

Each option above is `--set` of its file key, so it wins over the files.
The file is xrce-agentd.yaml with xrce-agentd.d/ in the system
configuration directory unless --config names another.

EXIT CODES:
    0   normal shutdown
    1   config error
    2   bind error (listener port in use)
    3   DDS discovery/runtime error
";

/// Version string.
pub const VERSION_TEXT: &str = "zerodds-xrce-agentd 1.0";

impl CliArgs {
    /// The convenience flags as `PATH=VALUE` assignments, in the order the
    /// daemon applies them.
    ///
    /// # Errors
    /// None today (kept `Result` for symmetry with `crates/coap-bridge`,
    /// whose paired-flag validation lives here).
    pub fn sets(&self) -> Result<Vec<String>, CliError> {
        let mut out = Vec::new();
        for l in &self.listen {
            out.push(alloc_format(l));
        }
        if let Some(v) = &self.log_level {
            out.push(format!("log_level={v}"));
        }
        Ok(out)
    }
}

fn alloc_format(listen: &str) -> String {
    format!("+listeners={listen}")
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
            "--listen" => out.listen.push(flags.value(&flag)?),
            "--log-level" => out.log_level = Some(flags.value(&flag)?),
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
        assert!(parse(&[]).unwrap().listen.is_empty());
    }

    #[test]
    fn listen_repeatable() {
        let p = parse(&args(&[
            "--listen",
            "udp:0.0.0.0:2018",
            "--listen",
            "tcp:0.0.0.0:2019",
        ]))
        .unwrap();
        assert_eq!(p.listen, vec!["udp:0.0.0.0:2018", "tcp:0.0.0.0:2019"]);
    }

    /// A participant's DDS domain is the one its CREATE carries
    /// (`OBJK_PARTICIPANT_Representation.domain_id`); there is no agent-wide
    /// default to set.
    #[test]
    fn there_is_no_domain_flag() {
        assert!(matches!(
            parse(&args(&["--domain", "12"])).unwrap_err(),
            CliError::UnknownFlag(_)
        ));
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
    fn sets_produces_expected_assignments() {
        let p = parse(&args(&[
            "--listen",
            "udp:0.0.0.0:2018",
            "--log-level",
            "debug",
        ]))
        .unwrap();
        let sets = p.sets().unwrap();
        assert_eq!(sets, vec!["+listeners=udp:0.0.0.0:2018", "log_level=debug"]);
    }
}
