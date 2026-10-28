// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `wireprobe-ros-package` — the `rosidl` interface package one corpus
//! type becomes.
//!
//! `rosidl` will not take the corpus IDL as written: it allows one
//! message per file at `<package>/msg/<Name>.idl`, exactly two module
//! levels, and no `union`, `enum`, `map`, `bitmask`, `bitset` or
//! `fixed`. This writes the package a type does map onto, or refuses
//! with the construct that has no `rosidl` form.
//!
//! ```text
//! wireprobe-ros-package --idl robot.idl --type common::Robot --out gen/
//! ```
//!
//! It prints the root message's name on stdout, which is what the
//! generator driver and the probe template both need. `--print-c-name`
//! prints the C identifier `rosidl_generator_c` will give it instead.
//!
//! Exit 3 — and not 1 — when the type has no `rosidl` form, so a caller
//! can tell "ROS cannot express this case" from "this run is broken".
#![forbid(unsafe_code)]
#![allow(clippy::print_stderr, clippy::print_stdout, clippy::exit)]

use std::path::PathBuf;
use zerodds_wire_probe::encode::{ProbeError, model_from_idl};
use zerodds_wire_probe::ros_idl::{PACKAGE, non_native_byte_order_hazard, ros_package};

const USAGE: &str = "\
wireprobe-ros-package — the rosidl interface package for one type

    --idl <file>            the IDL source
    --type <ScopedName>     the top-level type
    --out <dir>             where <package>/msg/<Name>.idl files go
    --print-c-name          print the generated C name and stop
    --print-files           print one relative path per line and stop
    --print-be-hazard       print `<kind>\t<construct>` for what
                            Fast CDR 1.0.24 cannot write in a non-native
                            byte order, if the type reaches one, and
                            stop. The kind is
                            `not-a-function-of-the-sample`, which may
                            not be recorded, or `deterministic`, which
                            is a divergence like any other
";

/// The type has no `rosidl` form. Distinct from a broken run.
const NO_ROSIDL_FORM: i32 = 3;

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
        Err(Failure::NoRosidlForm(message)) => {
            eprintln!("wireprobe-ros-package: {message}");
            std::process::exit(NO_ROSIDL_FORM);
        }
        Err(Failure::NotFilled(message)) => {
            eprintln!("wireprobe-ros-package: {message}");
            std::process::exit(NOT_FILLED);
        }
        Err(Failure::GeneratorDefect(message)) => {
            eprintln!("wireprobe-ros-package: {message}");
            std::process::exit(GENERATOR_DEFECT);
        }
        Err(Failure::MappingDivergence(message)) => {
            eprintln!("wireprobe-ros-package: {message}");
            std::process::exit(MAPPING_DIVERGENCE);
        }
        Err(Failure::Usage(message)) => {
            eprintln!("{message}\n\n{USAGE}");
            std::process::exit(1);
        }
    }
}

enum Failure {
    /// Its own generated code does not build.
    GeneratorDefect(String),
    /// It carries the construct and the octets differ by a mapping rule.
    MappingDivergence(String),
    /// `rosidl` has no form for the type. Exit 3.
    NoRosidlForm(String),
    /// The target has the construct and this probe has not written it.
    NotFilled(String),
    /// Anything else. Exit 1.
    Usage(String),
}

impl From<String> for Failure {
    fn from(message: String) -> Self {
        Self::Usage(message)
    }
}

impl From<&str> for Failure {
    fn from(message: &str) -> Self {
        Self::Usage(message.to_owned())
    }
}

fn run(argv: &[String]) -> Result<(), Failure> {
    let mut idl = None;
    let mut type_name = None;
    let mut out: Option<PathBuf> = None;
    let mut print_c_name = false;
    let mut print_files = false;
    let mut print_be_hazard = false;

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
            "--print-c-name" => {
                print_c_name = true;
                index += 1;
            }
            "--print-files" => {
                print_files = true;
                index += 1;
            }
            "--print-be-hazard" => {
                print_be_hazard = true;
                index += 1;
            }
            "--help" | "-h" => {
                println!("{USAGE}");
                return Ok(());
            }
            other => return Err(format!("`{other}` is not an option").into()),
        }
    }

    let idl = idl.ok_or("--idl is required")?;
    let type_name = type_name.ok_or("--type is required")?;
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

    let package = match ros_package(&ast, type_) {
        Ok(package) => package,
        // One arm per statement, and the exit code carries which:
        // `Unsupported` says ROS's specification has no such construct,
        // `GeneratorDefect` says it has one and rosidl's own output
        // does not build, `Unfilled` says this program has not written
        // it. Anything else is this program failing and must not be
        // reported as a limit of ROS at all.
        Err(error @ ProbeError::Unsupported { .. }) => {
            return Err(Failure::NoRosidlForm(format!("{error}")));
        }
        Err(error @ ProbeError::GeneratorDefect { .. }) => {
            return Err(Failure::GeneratorDefect(format!("{error}")));
        }
        Err(error @ ProbeError::MappingDivergence { .. }) => {
            return Err(Failure::MappingDivergence(format!("{error}")));
        }
        Err(error @ ProbeError::Unfilled { .. }) => {
            return Err(Failure::NotFilled(format!("{error}")));
        }
        Err(other) => return Err(format!("{other}").into()),
    };

    if print_c_name {
        println!("{}", package.root_c_name());
        return Ok(());
    }
    if print_be_hazard {
        // Silence means the type is safe in either order. The caller
        // tests for a non-empty line rather than for an exit code,
        // because "no hazard" is not a failure.
        //
        // The kind comes first, tab-separated, so that the caller
        // decides on a name and not on a substring of the sentence:
        // one kind is refused and the other is recorded, and a shell
        // that grepped for a word in the prose would silently swap them
        // the day the prose is edited.
        if let Some((kind, where_)) = non_native_byte_order_hazard(&ast, type_) {
            println!("{}\t{where_}", kind.as_str());
        }
        return Ok(());
    }
    if print_files {
        for file in &package.files {
            println!("msg/{}.idl", file.name);
        }
        return Ok(());
    }

    let out = out.ok_or("--out is required")?;
    let msg = out.join(PACKAGE).join("msg");
    std::fs::create_dir_all(&msg).map_err(|error| format!("{}: {error}", msg.display()))?;
    for file in &package.files {
        let path = msg.join(format!("{}.idl", file.name));
        std::fs::write(&path, &file.text)
            .map_err(|error| format!("{}: {error}", path.display()))?;
    }
    // The root's name, which is what the caller needs to build the
    // `idl_tuples` entry and the type support symbol.
    println!("{}", package.root);
    Ok(())
}
