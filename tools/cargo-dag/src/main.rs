// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `cargo-dag` — emittiert die topologische Publish-Reihenfolge der
//! Workspace-Crates.

#![warn(missing_docs)]
#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::path::PathBuf;
use std::process::ExitCode;

use zerodds_cargo_dag::{CrateInfo, collect_crates, only_publishable, topo_sort};

fn print_help() {
    println!(
        "cargo-dag — emittiert die topologische Publish-Reihenfolge

USAGE:
  cargo-dag [WORKSPACE_ROOT] [--only-publishable] [--format {{flat,verbose,json}}]

OPTIONS:
  WORKSPACE_ROOT       Pfad zum Repo-Root (default: .)
  --only-publishable   filtere `publish = false` raus
  --format flat        Eine Zeile pro Crate (default)
  --format verbose     Crate + Pfad + Dep-Count pro Zeile
  --format json        Array of {{name, path, blocked, deps}}
"
    );
}

#[derive(Clone, Copy)]
enum Format {
    Flat,
    Verbose,
    Json,
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().collect();
    let mut root = PathBuf::from(".");
    let mut publishable_only = false;
    let mut fmt = Format::Flat;
    let mut i = 1;
    while i < argv.len() {
        match argv[i].as_str() {
            "--only-publishable" => {
                publishable_only = true;
                i += 1;
            }
            "--format" => {
                fmt = match argv.get(i + 1).map(String::as_str) {
                    Some("flat") => Format::Flat,
                    Some("verbose") => Format::Verbose,
                    Some("json") => Format::Json,
                    Some(other) => {
                        eprintln!("unknown --format: {other}");
                        return ExitCode::from(2);
                    }
                    None => {
                        eprintln!("--format needs value");
                        return ExitCode::from(2);
                    }
                };
                i += 2;
            }
            "--help" | "-h" | "help" => {
                print_help();
                return ExitCode::SUCCESS;
            }
            other if !other.starts_with("--") => {
                root = PathBuf::from(other);
                i += 1;
            }
            other => {
                eprintln!("unknown flag: {other}");
                return ExitCode::from(2);
            }
        }
    }
    let crates = match collect_crates(&root) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("collect: {e}");
            return ExitCode::from(1);
        }
    };
    let ordered = match topo_sort(&crates) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("topo: {e}");
            return ExitCode::from(1);
        }
    };
    let final_list = if publishable_only {
        only_publishable(&ordered)
    } else {
        ordered
    };
    print_list(&final_list, fmt);
    ExitCode::SUCCESS
}

fn print_list(list: &[CrateInfo], fmt: Format) {
    match fmt {
        Format::Flat => {
            for c in list {
                println!("{}", c.name);
            }
        }
        Format::Verbose => {
            println!("# total: {}", list.len());
            for c in list {
                let mark = if c.publish_blocked { " [blocked]" } else { "" };
                println!(
                    "{:<40} {} deps={}{}",
                    c.name,
                    c.path.display(),
                    c.deps.len(),
                    mark
                );
            }
        }
        Format::Json => {
            print!("[");
            for (i, c) in list.iter().enumerate() {
                if i > 0 {
                    print!(",");
                }
                print!(
                    r#"{{"name":"{}","path":"{}","blocked":{},"deps":["#,
                    c.name,
                    c.path.display().to_string().replace('\\', "/"),
                    c.publish_blocked
                );
                for (j, d) in c.deps.iter().enumerate() {
                    if j > 0 {
                        print!(",");
                    }
                    print!("\"{d}\"");
                }
                print!("]}}");
            }
            println!("]");
        }
    }
}
