// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `zerodds-lint` CLI.
//!
//! ```text
//! zerodds-lint check [--root <path>] [--fail-on-warning]
//! ```

// CLI-Binary darf logischerweise stdout/stderr nutzen; workspace-deny wird
// nur hier oben aufgehoben.
#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Result;
use clap::{Parser, Subcommand};

use zerodds_lint::runner::{self, RunConfig};

#[derive(Parser, Debug)]
#[command(name = "zerodds-lint", version, about = "ZeroDDS custom lint runner")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Lint den gesamten Workspace.
    Check {
        /// Workspace-Wurzel (default: aktuelles Verzeichnis nach oben suchen).
        #[arg(long)]
        root: Option<PathBuf>,
        /// Auch Warnings lassen den Run mit Exit-1 enden.
        #[arg(long)]
        fail_on_warning: bool,
    },
}

fn main() -> ExitCode {
    match real_main() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("zerodds-lint: {e:#}");
            ExitCode::from(2)
        }
    }
}

fn real_main() -> Result<ExitCode> {
    let cli = Cli::parse();
    match cli.command {
        Command::Check {
            root,
            fail_on_warning,
        } => {
            let root = match root {
                Some(p) => p,
                None => runner::locate_workspace_root(&std::env::current_dir()?)?,
            };
            let cfg = RunConfig {
                workspace_root: root,
                fail_on_warning,
            };
            let report = runner::run(&cfg)?;
            for d in &report.diagnostics {
                println!("{d}");
            }
            println!(
                "zerodds-lint: scanned {crates} crates, {files} files; \
                 {errs} errors, {warns} warnings",
                crates = report.crates_scanned,
                files = report.files_scanned,
                errs = report.error_count(),
                warns = report.warning_count(),
            );
            let failed =
                report.error_count() > 0 || (cfg.fail_on_warning && report.warning_count() > 0);
            Ok(if failed {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            })
        }
    }
}
