// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `wireprobe-corba-idl` — the corpus's IDL, as CORBA IDL.
//!
//! An ORB's front end is not an XTypes one: it refuses `@final`, it has
//! no `bitmask`, no `bitset` and no `map`, and it has no `int8`. This
//! prints the same type as CORBA IDL, from the model rather than by
//! editing the text, so a construct with no CORBA form is refused by
//! name instead of being handed to the ORB to refuse for a reason the
//! run would then misread. See `corba_idl.rs`.
//!
//! ```text
//! wireprobe-corba-idl --idl robot.idl --type common::Robot --out gen.idl
//! ```
//!
//! Exit 3 is a construct CORBA IDL does not have, which the probes read
//! as "this case is not one the ORBs can take" rather than as a failure.
#![forbid(unsafe_code)]
#![allow(clippy::print_stderr, clippy::print_stdout, clippy::exit)]

use std::path::PathBuf;
use zerodds_wire_probe::ProbeError;
use zerodds_wire_probe::corba_idl::{corba_idl, cpp_name, java_name};
use zerodds_wire_probe::encode::model_from_idl;

const USAGE: &str = "\
wireprobe-corba-idl — one corpus type, printed as CORBA IDL

    --idl <file>            the IDL source
    --type <ScopedName>     the top-level type
    --out <file>            where the IDL goes (default: stdout)
    --print-cpp-name        print the C++ name of --type and stop
    --print-java-name       print the Java name of --type and stop
";

/// The construct is the target's and this probe has not implemented it.
///
/// Apart from the no-form code for the reason the octet ledger keeps
/// `no-form` apart from `refused`: a limit of the target and a gap of
/// ours are two different answers, and a caller that cannot tell them
/// apart reports one as the other.
const NOT_FILLED: i32 = 6;

/// Its front end took the case and its own generated code does not
/// build. A measurement of the toolchain.
const GENERATOR_DEFECT: i32 = 7;

/// It carries the case and a documented mapping rule makes the octets
/// differ. Also a measurement, and also not a missing form.
const MAPPING_DIVERGENCE: i32 = 8;

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    match run(&argv) {
        Ok(()) => {}
        Err(Failure::GeneratorDefect(message)) => {
            eprintln!("wireprobe-corba-idl: {message}");
            std::process::exit(GENERATOR_DEFECT);
        }
        Err(Failure::MappingDivergence(message)) => {
            eprintln!("wireprobe-corba-idl: {message}");
            std::process::exit(MAPPING_DIVERGENCE);
        }
        Err(Failure::Usage(message)) => {
            eprintln!("{message}\n\n{USAGE}");
            std::process::exit(1);
        }
        // Kept apart from a usage error so a corpus run can tell "this
        // case has no CORBA form" from "the probe was called wrongly".
        Err(Failure::NoCorbaForm(message)) => {
            eprintln!("wireprobe-corba-idl: {message}");
            std::process::exit(3);
        }
        Err(Failure::NotFilled(message)) => {
            eprintln!("wireprobe-corba-idl: {message}");
            std::process::exit(NOT_FILLED);
        }
    }
}

enum Failure {
    /// Its own generated code does not build.
    GeneratorDefect(String),
    /// It carries the construct and the octets differ by a mapping rule.
    MappingDivergence(String),
    Usage(String),
    NoCorbaForm(String),
    /// The target has the construct and this probe has not written it.
    NotFilled(String),
}

impl From<String> for Failure {
    fn from(message: String) -> Self {
        Self::Usage(message)
    }
}

fn run(argv: &[String]) -> Result<(), Failure> {
    let mut idl = None;
    let mut type_name = None;
    let mut out: Option<PathBuf> = None;
    let mut print_cpp = false;
    let mut print_java = false;

    let mut index = 0;
    while index < argv.len() {
        let flag = &argv[index];
        let value = || {
            argv.get(index + 1)
                .cloned()
                .ok_or_else(|| format!("`{flag}` takes a value"))
        };
        match flag.as_str() {
            "--idl" => {
                idl = Some(PathBuf::from(value()?));
                index += 2;
            }
            "--type" => {
                type_name = Some(value()?);
                index += 2;
            }
            "--out" => {
                out = Some(PathBuf::from(value()?));
                index += 2;
            }
            "--print-cpp-name" => {
                print_cpp = true;
                index += 1;
            }
            "--print-java-name" => {
                print_java = true;
                index += 1;
            }
            "--help" | "-h" => {
                println!("{USAGE}");
                return Ok(());
            }
            other => return Err(format!("`{other}` is not an option").into()),
        }
    }

    let idl = idl.ok_or_else(|| String::from("--idl is required"))?;
    let type_name = type_name.ok_or_else(|| String::from("--type is required"))?;
    let source =
        std::fs::read_to_string(&idl).map_err(|error| format!("{}: {error}", idl.display()))?;
    let ast = model_from_idl(&source, &idl.display().to_string())
        .map_err(|error| format!("{}: {error}", idl.display()))?;

    let segments: Vec<String> = type_name
        .split("::")
        .filter(|segment| !segment.is_empty())
        .map(std::string::ToString::to_string)
        .collect();
    let type_ = ast
        .find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(segments))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)
        .ok_or_else(|| format!("`{type_name}` is not in the model"))?;

    if print_cpp {
        println!("{}", cpp_name(&ast, type_));
        return Ok(());
    }
    if print_java {
        println!("{}", java_name(&ast, type_));
        return Ok(());
    }

    let text = corba_idl(&ast, type_).map_err(|error| match error {
        ProbeError::Unfilled { .. } => Failure::NotFilled(format!("{error}")),
        ProbeError::GeneratorDefect { .. } => Failure::GeneratorDefect(format!("{error}")),
        ProbeError::MappingDivergence { .. } => Failure::MappingDivergence(format!("{error}")),
        other => Failure::NoCorbaForm(format!("{other}")),
    })?;
    match out {
        Some(path) => std::fs::write(&path, text)
            .map_err(|error| format!("{}: {error}", path.display()).into()),
        None => {
            print!("{text}");
            Ok(())
        }
    }
}
