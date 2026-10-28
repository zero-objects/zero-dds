// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CLI for `zerodds-opcua-bridged`. Spec §2.

use std::string::String;
use std::vec::Vec;

/// Parsed CLI args.
#[derive(Debug, Clone, Default)]
pub struct CliArgs {
    /// `--bind <ADDR>` — the `opc.tcp` server bind address.
    pub bind: Option<String>,
    /// `--domain <ID>`.
    pub domain: Option<i32>,
    /// `--idl <FILE>` (multi).
    pub idl: Vec<String>,
    /// `--gateway-config <FILE>`.
    pub gateway_config: Option<String>,
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
zerodds-opcua-bridged 1.0 — the OMG DDS-OPCUA Gateway 1.0 daemon

USAGE:
    zerodds-opcua-bridged [OPTIONS]

OPTIONS:
    --bind <ADDR>             opc.tcp bind address (default 0.0.0.0:4840)
    --domain <ID>             DDS domain ID (default 0)
    --idl <FILE>              IDL file to compile (repeatable)
    --gateway-config <FILE>   Spec §10 XML gateway config
    --log-level <LEVEL>       trace/debug/info/warn/error
    --metrics <ADDR>          Prometheus listener (enables metrics)
    --version                 Version info
    --help                    Help

Each option above is `--set` of its file key (`--bind X` is
`--set opcua.server.bind=X`), so it wins over the files. The file is
opcua-bridged.yaml with opcua-bridged.d/ in the system configuration
directory unless --config names another.

EXIT CODES:
    0   normal shutdown
    1   config error
    2   bind error (port in use)
    3   DDS discovery error
    4   security (SecureChannel/certificate) setup error
";

/// Version string.
pub const VERSION_TEXT: &str = "zerodds-opcua-bridged 1.0";

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
            "--idl" => out.idl.push(flags.value(&flag)?),
            "--gateway-config" => out.gateway_config = Some(flags.value(&flag)?),
            "--log-level" => out.log_level = Some(flags.value(&flag)?),
            "--metrics" => out.metrics = Some(flags.value(&flag)?),
            other => return Err(CliError::UnknownFlag(other.to_string())),
        }
    }
    Ok(out)
}

impl CliArgs {
    /// The convenience flags as `PATH=VALUE` assignments, in the order the
    /// daemon applies them.
    ///
    /// # Errors
    /// Never — kept `Result` for symmetry with the other bridges' `sets()`.
    pub fn sets(&self) -> Result<Vec<String>, CliError> {
        let mut out = Vec::new();
        if let Some(v) = &self.bind {
            out.push(std::format!("opcua.server.bind={v}"));
        }
        if let Some(v) = self.domain {
            out.push(std::format!("domain={v}"));
        }
        if let Some(v) = &self.log_level {
            out.push(std::format!("log_level={v}"));
        }
        for idl in &self.idl {
            out.push(std::format!("+opcua.idl={idl}"));
        }
        if let Some(v) = &self.gateway_config {
            out.push(std::format!("opcua.gateway_config={v}"));
        }
        if let Some(v) = &self.metrics {
            out.push(std::format!("metrics.listen={v}"));
            out.push("metrics.enabled=true".to_string());
        }
        Ok(out)
    }
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
        let p = parse(&args(&["--bind", "0.0.0.0:4840"])).unwrap();
        assert_eq!(p.bind.as_deref(), Some("0.0.0.0:4840"));
    }

    #[test]
    fn equals_form() {
        let p = parse(&args(&["--bind=127.0.0.1:4841"])).unwrap();
        assert_eq!(p.bind.as_deref(), Some("127.0.0.1:4841"));
    }

    #[test]
    fn domain_parses_int() {
        assert_eq!(parse(&args(&["--domain", "12"])).unwrap().domain, Some(12));
    }

    #[test]
    fn idl_is_repeatable() {
        let p = parse(&args(&["--idl", "a.idl", "--idl", "b.idl"])).unwrap();
        assert_eq!(p.idl, vec!["a.idl".to_string(), "b.idl".to_string()]);
    }

    #[test]
    fn unknown_flag_rejected() {
        assert!(matches!(
            parse(&args(&["--nope"])).unwrap_err(),
            CliError::UnknownFlag(_)
        ));
    }

    #[test]
    fn sets_render_idl_as_appends() {
        let p = parse(&args(&["--idl", "a.idl"])).unwrap();
        assert_eq!(p.sets().unwrap(), vec!["+opcua.idl=a.idl".to_string()]);
    }

    #[test]
    fn help_short_form() {
        assert!(parse(&args(&["-h"])).unwrap().help);
    }
}
