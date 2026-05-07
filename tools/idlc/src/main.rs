// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! IDL4 compiler: backends for C, C++, C#, Java, Python, Rust (T7.1).
//!
//! Phase-0-Spike: nur `--parse-only` ist implementiert. Code-Generator-
//! Backends (C/C++/C#/Java/Python/Rust) folgen in Phase 1.
//!
//! # Usage
//!
//! ```text
//! zerodds-idlc --parse-only <file.idl>           Parse mit OMG IDL 4.2 Base
//! zerodds-idlc --parse-only --rti <file.idl>     Parse mit RTI Connext-Delta
//! zerodds-idlc --version                         Versions-Info
//! zerodds-idlc --help                            Hilfe
//! ```
//!
//! Exit-Codes:
//!   0   parse erfolgreich, AST gedruckt
//!   1   parse-Fehler (Lex/Recognize/Build)
//!   2   CLI-Argumente ungueltig oder Datei nicht lesbar
//!   3   Backend (noch) nicht implementiert

#![allow(clippy::print_stderr, clippy::print_stdout)] // CLI-Tool: I/O auf stdio zulaessig.

use std::process::ExitCode;

use zerodds_idl::config::ParserConfig;
use zerodds_idl::grammar::deltas::RTI_CONNEXT;
use zerodds_idl::parser::{parse, parse_with_deltas};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("zerodds-idlc: {e}");
            ExitCode::from(e.exit_code())
        }
    }
}

#[derive(Debug)]
enum CliError {
    Usage(String),
    Io(String),
    Parse(String),
    NotImplemented(String),
}

impl CliError {
    fn exit_code(&self) -> u8 {
        match self {
            Self::Usage(_) | Self::Io(_) => 2,
            Self::Parse(_) => 1,
            Self::NotImplemented(_) => 3,
        }
    }
}

impl core::fmt::Display for CliError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Usage(m) | Self::Io(m) | Self::Parse(m) | Self::NotImplemented(m) => {
                f.write_str(m)
            }
        }
    }
}

#[derive(Default)]
struct CliOptions {
    parse_only: bool,
    rti: bool,
    file: Option<String>,
}

fn run(args: &[String]) -> Result<(), CliError> {
    let mut opts = CliOptions::default();
    for arg in args {
        match arg.as_str() {
            "-h" | "--help" => {
                print_help();
                return Ok(());
            }
            "-V" | "--version" => {
                println!("zerodds-idlc {VERSION}");
                return Ok(());
            }
            "--parse-only" => opts.parse_only = true,
            "--rti" => opts.rti = true,
            other if other.starts_with('-') => {
                return Err(CliError::Usage(format!("unknown flag: {other}")));
            }
            other => {
                if opts.file.is_some() {
                    return Err(CliError::Usage(
                        "only one input file supported in Phase 0".to_string(),
                    ));
                }
                opts.file = Some(other.to_string());
            }
        }
    }

    if !opts.parse_only {
        return Err(CliError::NotImplemented(
            "Phase 0 supports only --parse-only; code-gen backends follow in Phase 1".to_string(),
        ));
    }
    let path = opts
        .file
        .ok_or_else(|| CliError::Usage("missing input file (try --help)".to_string()))?;
    let src = std::fs::read_to_string(&path)
        .map_err(|e| CliError::Io(format!("cannot read {path}: {e}")))?;

    let cfg = ParserConfig::default();
    let ast = if opts.rti {
        parse_with_deltas(&src, &cfg, &[&RTI_CONNEXT])
    } else {
        parse(&src, &cfg)
    }
    .map_err(|e| CliError::Parse(format!("parse failed: {e}")))?;

    println!("{ast}");
    Ok(())
}

fn print_help() {
    println!(
        "zerodds-idlc {VERSION}\n\
         IDL4 compiler (Phase 0: parse-only)\n\n\
         USAGE:\n\
         \x20   zerodds-idlc --parse-only [--rti] <file.idl>\n\n\
         OPTIONS:\n\
         \x20   --parse-only       Parse + print AST (Phase 0 default)\n\
         \x20   --rti              Aktiviere RTI Connext-Grammar-Delta\n\
         \x20   -h, --help         Diese Hilfe\n\
         \x20   -V, --version      Versions-Info\n\n\
         EXIT-CODES:\n\
         \x20   0  Erfolg\n\
         \x20   1  Parse-Fehler\n\
         \x20   2  CLI/IO-Fehler\n\
         \x20   3  Feature noch nicht implementiert\n"
    );
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn run_with_no_args_errors_with_usage() {
        let result = run(&[]);
        assert!(matches!(result, Err(CliError::NotImplemented(_))));
    }

    #[test]
    fn run_with_unknown_flag_errors() {
        let result = run(&["--bogus".to_string()]);
        assert!(matches!(result, Err(CliError::Usage(_))));
    }

    #[test]
    fn run_with_help_succeeds() {
        let result = run(&["--help".to_string()]);
        assert!(result.is_ok());
    }

    #[test]
    fn run_with_version_succeeds() {
        let result = run(&["--version".to_string()]);
        assert!(result.is_ok());
    }

    #[test]
    fn run_parse_only_without_file_errors() {
        let result = run(&["--parse-only".to_string()]);
        assert!(matches!(result, Err(CliError::Usage(_))));
    }

    #[test]
    fn run_parse_only_with_missing_file_errors_io() {
        let result = run(&[
            "--parse-only".to_string(),
            "/nonexistent/file.idl".to_string(),
        ]);
        assert!(matches!(result, Err(CliError::Io(_))));
    }

    #[test]
    fn cli_error_exit_codes_are_distinct() {
        assert_eq!(CliError::Parse("x".into()).exit_code(), 1);
        assert_eq!(CliError::Usage("x".into()).exit_code(), 2);
        assert_eq!(CliError::Io("x".into()).exit_code(), 2);
        assert_eq!(CliError::NotImplemented("x".into()).exit_code(), 3);
    }
}
