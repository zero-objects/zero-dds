// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `wireprobe-fill` — the C that puts one JSON sample into a vendor's
//! generated structure.
//!
//! The foreign probes are shell scripts around their vendor's own code
//! generator and compiler. This supplies the one piece the vendor does
//! not: the statements that fill `sample`. `#include`d into the probe
//! template.
//!
//! ```text
//! wireprobe-fill --vendor cyclone --idl robot.idl --type common::Robot \
//!                --values robot.json --out fill.inc
//! ```
#![forbid(unsafe_code)]
#![allow(clippy::print_stderr, clippy::print_stdout, clippy::exit)]

use std::path::PathBuf;
use zerodds_wire_probe::ProbeError;
use zerodds_wire_probe::corba_cpp::fill_corba_cpp;
use zerodds_wire_probe::corba_idl::{cpp_name as corba_cpp_name, java_name};
use zerodds_wire_probe::corba_java::fill_corba_java;
use zerodds_wire_probe::encode::model_from_idl;
use zerodds_wire_probe::fill::{Vendor, c_name, fill};
use zerodds_wire_probe::fill_cpp::{cpp_name, fill_fastdds};
use zerodds_wire_probe::fill_ros::{fill_ros, ros_c_name};
use zerodds_wire_probe::fill_rust::{fill_hdds, rust_path};

const USAGE: &str = "\
wireprobe-fill — the C that fills one sample, for a foreign probe

    --vendor cyclone|rti|fastdds|ros|omniorb|tao|jacorb|hdds
                            which generated API to write against
                            (`ros` is rosidl's, not a DDS vendor's;
                            omniorb and tao share the OMG IDL-to-C++
                            mapping, jacorb the IDL-to-Java one)
    --idl <file>            the IDL source
    --type <ScopedName>     the top-level type
    --values <file>         the sample, as JSON
    --defaults              the type's own default sample, instead of --values
    --out <file>            where the statements go (default: stdout)
    --print-c-name          print the generated name of --type and stop
                            (with --vendor fastdds, the C++ one;
                            with --vendor ros, rosidl's; with omniorb
                            or tao the OMG C++ one, with jacorb Java,
                            with hdds the Rust path)
";

/// The construct has no form in the target this fill was asked for.
///
/// The same code `wireprobe-corba-idl` and `wireprobe-ros-package`
/// use, and the same thing the octet ledger calls `no-form`: the
/// target's specification does not have the construct, so nothing was
/// refused — there was nothing to refuse. `tools/corba-oracle/
/// corba-sweep.sh` already reads it that way.
const NO_FORM: i32 = 3;

/// The construct is the target's and this probe has not implemented it.
///
/// Distinct from [`NO_FORM`], because a reader who cannot tell them
/// apart cannot tell a limit of CORBA from a gap of ours — which is
/// exactly what happened when both left through the same door.
const NOT_FILLED: i32 = 6;

/// Its front end took the case and its own generated code does not
/// build. A measurement of the toolchain.
const GENERATOR_DEFECT: i32 = 7;

/// The sample holds a value the declared type cannot — a refusal about
/// the value, not about the type, and not a usage error either.
const VALUE_MISMATCH: i32 = 5;

/// It carries the case and a documented mapping rule makes the octets
/// differ. Also a measurement, and also not a missing form.
const MAPPING_DIVERGENCE: i32 = 8;

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    match run(&argv) {
        Ok(()) => {}
        // A refusal is not a usage error and does not get the usage
        // text: it is an answer about the type, and the caller that
        // reads it is a sweep, not a person who mistyped a flag.
        Err(Failure::NoForm(message)) => {
            eprintln!("wireprobe-fill: {message}");
            std::process::exit(NO_FORM);
        }
        Err(Failure::NotFilled(message)) => {
            eprintln!("wireprobe-fill: {message}");
            std::process::exit(NOT_FILLED);
        }
        Err(Failure::GeneratorDefect(message)) => {
            eprintln!("wireprobe-fill: {message}");
            std::process::exit(GENERATOR_DEFECT);
        }
        Err(Failure::MappingDivergence(message)) => {
            eprintln!("wireprobe-fill: {message}");
            std::process::exit(MAPPING_DIVERGENCE);
        }
        Err(Failure::Mismatch(message)) => {
            eprintln!("wireprobe-fill: {message}");
            std::process::exit(VALUE_MISMATCH);
        }
        Err(Failure::Usage(message)) => {
            eprintln!("{message}\n\n{USAGE}");
            std::process::exit(1);
        }
    }
}

/// Why a fill did not happen, kept apart so the exit code can say it.
enum Failure {
    /// Its own generated code does not build.
    GeneratorDefect(String),
    /// It carries the construct and the octets differ by a mapping rule.
    MappingDivergence(String),
    /// The sample holds a value the declared type cannot.
    Mismatch(String),
    /// The target has no form for the construct.
    NoForm(String),
    /// The target has one and this probe has not written it.
    NotFilled(String),
    /// The command line.
    Usage(String),
}

impl From<String> for Failure {
    fn from(message: String) -> Self {
        Self::Usage(message)
    }
}

impl From<&str> for Failure {
    fn from(message: &str) -> Self {
        Self::Usage(message.to_string())
    }
}

fn run(argv: &[String]) -> Result<(), Failure> {
    let mut vendor = None;
    let mut idl = None;
    let mut type_name = None;
    let mut values = None;
    // Whether the sample is the type's own default rather than a file.
    //
    // The Fast DDS decode probe needs one: it fills a sample and
    // serialises it once to read the encapsulation flag `fastddsgen`
    // chose, because an `@external` member of a *default-constructed*
    // C++ sample is a null `eprosima::fastcdr::external` that Fast DDS
    // refuses to serialise. The values never survive — the sample is
    // reset before it becomes the deserialisation target — and the read
    // direction has no values file to offer, so the type's own default
    // is what it asks for.
    let mut defaults = false;
    let mut out: Option<PathBuf> = None;
    let mut print_c_name = false;
    // Fast DDS's statements are C++ and come out of a separate backend,
    // `fill_cpp`, rather than a third `Vendor` in the C one. `rosidl`'s
    // are C, but against structures that are not the IDL-to-C mapping
    // `Vendor` covers, so they come out of `fill_ros` the same way.
    let mut fastdds = false;
    let mut ros = false;
    // The two ORB mappings are two more backends, for the same reason:
    // the shape of a statement is decided by the language mapping, and
    // the OMG IDL-to-C++ and IDL-to-Java mappings are neither of the
    // two above. omniORB and TAO share one, because CORBA 3.3 part 3
    // §1.2 is a specification and both generate against it.
    let mut corba_cpp = false;
    let mut corba_java = false;
    // HDDS is the fifth mapping: `hddsgen` emits plain Rust structs
    // with `pub` fields, so its backend writes one expression rather
    // than a run of statements.
    let mut hdds = false;

    let mut index = 0;
    while index < argv.len() {
        let flag = &argv[index];
        let value = || {
            argv.get(index + 1)
                .cloned()
                .ok_or_else(|| format!("`{flag}` takes a value"))
        };
        match flag.as_str() {
            "--vendor" => {
                let name = value()?;
                if name == "fastdds" {
                    fastdds = true;
                } else if name == "ros" {
                    ros = true;
                } else if name == "omniorb" || name == "tao" {
                    corba_cpp = true;
                } else if name == "jacorb" {
                    corba_java = true;
                } else if name == "hdds" {
                    hdds = true;
                } else {
                    vendor = Some(
                        Vendor::parse(&name).ok_or_else(|| format!("`{name}` is not a vendor"))?,
                    );
                }
                index += 2;
            }
            "--idl" => {
                idl = Some(PathBuf::from(value()?));
                index += 2;
            }
            "--type" => {
                type_name = Some(value()?);
                index += 2;
            }
            "--defaults" => {
                defaults = true;
                index += 1;
            }
            "--values" => {
                values = Some(PathBuf::from(value()?));
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
            "--help" | "-h" => {
                println!("{USAGE}");
                return Ok(());
            }
            other => return Err(Failure::Usage(format!("`{other}` is not an option"))),
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

    if print_c_name {
        if fastdds {
            println!("{}", cpp_name(&ast, type_));
        } else if ros {
            println!("{}", ros_c_name(&ast, type_));
        } else if corba_cpp {
            println!("{}", corba_cpp_name(&ast, type_));
        } else if corba_java {
            println!("{}", java_name(&ast, type_));
        } else if hdds {
            println!("{}", rust_path(&ast, type_));
        } else {
            // The plain C name is the vendor's too, since
            // `identifier-target-keyword-typename`: a type declared at
            // the outermost scope keeps its own spelling, and RTI
            // renames a C keyword there where Cyclone does not. A
            // default would hand one of the two the other's header.
            let vendor = vendor.ok_or("--vendor is required with --print-c-name")?;
            println!("{}", c_name(&ast, type_, vendor));
        }
        return Ok(());
    }

    if !fastdds && !ros && !corba_cpp && !corba_java && !hdds && vendor.is_none() {
        return Err(Failure::Usage(String::from("--vendor is required")));
    }
    // `--defaults` and `--values` are the two ways to a sample, and
    // exactly one of them has to be given: a caller that passed both
    // would get the file it named silently ignored.
    let sample: serde_json::Value = match (defaults, values) {
        (true, Some(_)) => {
            return Err(Failure::Usage(String::from(
                "--defaults and --values are two ways to the same sample; give one",
            )));
        }
        (true, None) => zerodds_wire_probe::default_of_type(&ast, type_, "--defaults")
            .map_err(|error| Failure::Usage(format!("{error}")))?,
        (false, Some(values)) => {
            let text = std::fs::read_to_string(&values)
                .map_err(|error| format!("{}: {error}", values.display()))?;
            serde_json::from_str(&text).map_err(|error| format!("{}: {error}", values.display()))?
        }
        (false, None) => return Err(Failure::Usage(String::from("--values is required"))),
    };

    let source = if let Some(vendor) = vendor {
        fill(&ast, type_, &sample, vendor)
    } else if ros {
        fill_ros(&ast, type_, &sample)
    } else if corba_cpp {
        fill_corba_cpp(&ast, type_, &sample)
    } else if corba_java {
        fill_corba_java(&ast, type_, &sample)
    } else if hdds {
        fill_hdds(&ast, type_, &sample)
    } else {
        fill_fastdds(&ast, type_, &sample)
    }
    .map_err(|error| match error {
        // The two the caller has to tell apart. Everything else is a
        // fault in the run rather than an answer about the type.
        ProbeError::Unsupported { .. } => Failure::NoForm(format!("{error}")),
        ProbeError::GeneratorDefect { .. } => Failure::GeneratorDefect(format!("{error}")),
        ProbeError::MappingDivergence { .. } => Failure::MappingDivergence(format!("{error}")),
        ProbeError::Unfilled { .. } => Failure::NotFilled(format!("{error}")),
        // A value the declared type cannot hold is an answer about the
        // sample, not a mistyped flag: it gets the bare sentence, never
        // the usage text — `primitive-char-above-latin1`'s `.bin.err`
        // used to carry the whole flag table under one line of refusal.
        ProbeError::Mismatch { .. } => Failure::Mismatch(format!("{error}")),
        other => Failure::Usage(format!("{other}")),
    })?;
    match out {
        Some(path) => std::fs::write(&path, source)
            .map_err(|error| Failure::Usage(format!("{}: {error}", path.display()))),
        None => {
            print!("{source}");
            Ok(())
        }
    }
}
