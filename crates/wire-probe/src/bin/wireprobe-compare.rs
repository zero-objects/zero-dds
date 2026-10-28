// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `wireprobe-compare` — did the value that came back out equal the one
//! that went in.
//!
//! The last step of a pairwise run: A wrote the bytes, B read them, and
//! this says whether B's values are the sample. Against the **input**,
//! not against A's own reading of its own bytes — two implementations
//! that misread a construct in the same way would agree with each other
//! and disagree with the sample, and only the second comparison shows
//! it.
//!
//! ```text
//! wireprobe-compare --idl robot.idl --type common::Robot \
//!                   --expected robot.json --found robot.read.json
//! ```
//!
//! Both sides are put into the canonical form of
//! `crates/wire-probe/src/canonical.rs` first, so a difference in how a
//! `double` was printed is not reported as a difference in what was
//! read.
//!
//! | exit | meaning |
//! |---|---|
//! | 0 | the values are the same |
//! | 1 | they differ — the first place is printed |
//! | 2 | one of them is not a value of this type at all |
#![forbid(unsafe_code)]
#![allow(clippy::print_stderr, clippy::print_stdout, clippy::exit)]

use std::path::PathBuf;
use zerodds_wire_probe::canonical::{canonicalise, difference, text};
use zerodds_wire_probe::encode::model_from_idl;

const USAGE: &str = "\
wireprobe-compare — one sample against another, in canonical form

    --idl <file>            the IDL source
    --type <ScopedName>     the top-level type
    --expected <file>       the sample that went in
    --found <file>          the values that came back
    --quiet                 say nothing on success
";

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    match run(&argv) {
        Ok(true) => std::process::exit(0),
        Ok(false) => std::process::exit(1),
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    }
}

fn run(argv: &[String]) -> Result<bool, String> {
    let mut idl: Option<PathBuf> = None;
    let mut type_name = None;
    let mut expected: Option<PathBuf> = None;
    let mut found: Option<PathBuf> = None;
    let mut quiet = false;

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
            "--expected" => {
                expected = Some(PathBuf::from(value()?));
                index += 2;
            }
            "--found" => {
                found = Some(PathBuf::from(value()?));
                index += 2;
            }
            "--quiet" => {
                quiet = true;
                index += 1;
            }
            "--help" | "-h" => {
                println!("{USAGE}");
                return Ok(true);
            }
            other => return Err(format!("`{other}` is not an option\n\n{USAGE}")),
        }
    }

    let idl = idl.ok_or("--idl is required")?;
    let type_name = type_name.ok_or("--type is required")?;
    let expected = expected.ok_or("--expected is required")?;
    let found = found.ok_or("--found is required")?;

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

    let left = read(&ast, type_, &expected, &type_name)?;
    let right = read(&ast, type_, &found, &type_name)?;

    match difference(&left, &right, &type_name) {
        None => {
            if !quiet {
                println!("same {}", text(&left));
            }
            Ok(true)
        }
        Some(first) => {
            println!("differs {first}");
            Ok(false)
        }
    }
}

fn read(
    ast: &zerodds_ast::Ast,
    type_: zerodds_ast_core::Id<zerodds_ast::TypeNode>,
    path: &PathBuf,
    type_name: &str,
) -> Result<serde_json::Value, String> {
    let text =
        std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))?;
    canonicalise(ast, type_, &value, type_name)
        .map_err(|error| format!("{}: {error}", path.display()))
}
