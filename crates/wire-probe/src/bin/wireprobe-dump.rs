// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `wireprobe-dump` — the C that prints one deserialised sample as the
//! canonical JSON.
//!
//! The mirror of `wireprobe-fill`. The foreign probes are shell scripts
//! around their vendor's own code generator and compiler; between them
//! the two programs supply the two pieces the vendor does not — the
//! statements that fill `sample` before serialising, and the statements
//! that print it after deserialising.
//!
//! ```text
//! wireprobe-dump --vendor cyclone --idl robot.idl --type common::Robot \
//!                --out dump.inc
//! ```
//!
//! No `--values`: what a dump prints is decided by the bytes the vendor
//! read, which is the whole point of the return direction.
#![forbid(unsafe_code)]
#![allow(clippy::print_stderr, clippy::print_stdout, clippy::exit)]

use std::path::PathBuf;
use zerodds_wire_probe::corba_cpp::dump_corba_cpp;
use zerodds_wire_probe::corba_java::dump_corba_java;
use zerodds_wire_probe::dump::dump;
use zerodds_wire_probe::dump_cpp::dump_fastdds;
use zerodds_wire_probe::dump_ros::dump_ros;
use zerodds_wire_probe::dump_rust::dump_hdds;
use zerodds_wire_probe::encode::model_from_idl;
use zerodds_wire_probe::fill::Vendor;

const USAGE: &str = "\
wireprobe-dump — the C that prints one sample, for a foreign probe

    --vendor cyclone|rti|fastdds|ros|omniorb|tao|jacorb|hdds
                            which generated API to read through
                            (`ros` is rosidl's, not a DDS vendor's;
                            omniorb and tao share the OMG IDL-to-C++
                            mapping, jacorb is the IDL-to-Java one;
                            hdds is the Rust expression that prints
                            hddsgen's generated type as JSON)
    --idl <file>            the IDL source
    --type <ScopedName>     the top-level type
    --out <file>            where the statements go (default: stdout)
";

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if let Err(message) = run(&argv) {
        eprintln!("{message}\n\n{USAGE}");
        std::process::exit(1);
    }
}

fn run(argv: &[String]) -> Result<(), String> {
    let mut vendor = None;
    let mut idl = None;
    let mut type_name = None;
    let mut out: Option<PathBuf> = None;
    // Fast DDS's statements are C++ and come out of a separate backend,
    // the same split `wireprobe-fill` has, and `rosidl`'s come out of
    // `dump_ros` for the same reason.
    let mut fastdds = false;
    let mut ros = false;
    let mut corba_cpp = false;
    let mut corba_java = false;
    // HDDS's dump is a Rust expression over `hddsgen`'s generated type,
    // a fifth mapping for the same reason `wireprobe-fill` has one.
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
                    // One backend for both: the OMG IDL-to-C++ mapping
                    // is a specification and the two ORBs generate the
                    // same API against it.
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
            "--out" => {
                out = Some(PathBuf::from(value()?));
                index += 2;
            }
            "--help" | "-h" => {
                println!("{USAGE}");
                return Ok(());
            }
            other => return Err(format!("`{other}` is not an option")),
        }
    }

    let idl = idl.ok_or("--idl is required")?;
    let type_name = type_name.ok_or("--type is required")?;
    if !fastdds && !ros && !corba_cpp && !corba_java && !hdds && vendor.is_none() {
        return Err(String::from("--vendor is required"));
    }
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

    let source = if let Some(vendor) = vendor {
        dump(&ast, type_, vendor)
    } else if ros {
        dump_ros(&ast, type_)
    } else if corba_cpp {
        dump_corba_cpp(&ast, type_)
    } else if corba_java {
        dump_corba_java(&ast, type_)
    } else if hdds {
        dump_hdds(&ast, type_)
    } else {
        dump_fastdds(&ast, type_)
    }
    .map_err(|error| format!("{error}"))?;
    match out {
        Some(path) => {
            std::fs::write(&path, source).map_err(|error| format!("{}: {error}", path.display()))
        }
        None => {
            print!("{source}");
            Ok(())
        }
    }
}
