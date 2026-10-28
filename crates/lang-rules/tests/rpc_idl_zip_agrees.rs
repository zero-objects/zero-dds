// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The IDL files of `omg-dds-rpc-idl.zip` (DDS-RPC 1.0, `idl/robot.idl`) written
//! out for the Basic Service Mapping agree with the names the generators take
//! for the same interface (§7.5.1.1.4 to §7.5.1.1.7): every structure, union and
//! constant of `robot.idl` that belongs to `RobotControl` is a name the model
//! gives, and the other way round.
//!
//! `robot.idl` is the copy in `tests/omg-dds-rpc-idl/`, byte for byte as the OMG published it;
//! `omg-dds-rpc-idl.sha256` beside it holds its SHA-256. When the zip itself is at hand
//! (`ZD_DDS_RPC_IDL_ZIP`, the cache directory of the workspace or of a sibling checkout named
//! `zerodds`, `~/.cache/zerodds-spec/`), the copy is also compared with the zip's file.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::print_stderr
)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use zerodds_lang_rules::adapter::interface;

const ZIP: &str = "omg-dds-rpc-idl.zip";

fn copy_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/omg-dds-rpc-idl")
}

fn locate_zip() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(p) = std::env::var("ZD_DDS_RPC_IDL_ZIP") {
        candidates.push(PathBuf::from(p));
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    candidates.push(root.join("internal/standards/cache/omg").join(ZIP));
    candidates.push(
        root.join("../zerodds/internal/standards/cache/omg")
            .join(ZIP),
    );
    if let Ok(home) = std::env::var("HOME") {
        candidates.push(Path::new(&home).join(".cache/zerodds-spec").join(ZIP));
    }
    candidates.into_iter().find(|c| c.is_file())
}

fn sha256(path: &Path) -> String {
    for tool in [&["sha256sum"][..], &["shasum", "-a", "256"][..]] {
        if let Ok(out) = Command::new(tool[0]).args(&tool[1..]).arg(path).output() {
            if out.status.success() {
                return String::from_utf8_lossy(&out.stdout)
                    .split_whitespace()
                    .next()
                    .unwrap_or_default()
                    .to_string();
            }
        }
    }
    panic!("neither sha256sum nor shasum is available");
}

/// `robot.idl` as the OMG published it: the copy, checked against its recorded SHA-256 and,
/// with the zip at hand, against the zip's file.
fn robot_idl() -> String {
    let copy = copy_dir().join("robot.idl");
    let recorded = std::fs::read_to_string(copy_dir().join("omg-dds-rpc-idl.sha256"))
        .expect("the sha256 list");
    let expected = recorded
        .lines()
        .filter_map(|l| l.split_once("  "))
        .find(|(_, name)| name.trim() == "robot.idl")
        .map(|(sum, _)| sum.to_string())
        .expect("robot.idl is in the sha256 list");
    assert_eq!(
        sha256(&copy),
        expected,
        "robot.idl differs from the OMG file recorded in omg-dds-rpc-idl.sha256"
    );
    let bytes = std::fs::read(&copy).expect("robot.idl");
    if let Some(zip) = locate_zip() {
        let out = Command::new("unzip")
            .args(["-p"])
            .arg(&zip)
            .arg("idl/robot.idl")
            .output()
            .expect("unzip");
        assert!(out.status.success(), "unzip -p {}", zip.display());
        assert_eq!(out.stdout, bytes, "the copy is not the zip's idl/robot.idl");
    } else {
        eprintln!("{ZIP} not at hand: the copy was checked against its recorded sum only");
    }
    String::from_utf8(bytes).expect("robot.idl is ASCII")
}

/// The names `robot.idl` declares for `RobotControl` under `#ifdef BASIC`.
fn names_of_the_zip() -> BTreeSet<String> {
    let text = robot_idl();
    let mut names = BTreeSet::new();
    for line in text.lines() {
        let words: Vec<&str> = line.split_whitespace().collect();
        let name = match words.as_slice() {
            ["struct", name, ..] | ["union", name, ..] => Some(*name),
            ["const", "long", name, ..] => Some(*name),
            _ => None,
        };
        if let Some(name) = name {
            if name.starts_with("RobotControl_") {
                names.insert(name.to_string());
            }
        }
    }
    names
}

#[test]
fn the_idl_of_the_spec_names_what_the_generators_name() {
    let source = "
        module robot {
          exception TooFast {};
          enum Command { START_COMMAND, STOP_COMMAND };
          struct Status { string msg; };
          @DDSService
          interface RobotControl {
            void  command(in Command com);
            float setSpeed(in float speed) raises (TooFast);
            float getSpeed();
            void  getStatus(out Status status);
          };
        };
    ";
    let parsed = zerodds_idl_parse::parse(source);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "robot.idl");
    let mut ast = lowered.ast;
    ast.resolve_all_by_name().expect("resolve");
    let service = interface::services(&ast)
        .expect("services")
        .into_iter()
        .find(|s| s.name == "RobotControl")
        .expect("RobotControl");

    let mut ours: BTreeSet<String> = BTreeSet::new();
    let name = service.name.as_str();
    ours.insert(interface::request_type_name(name));
    ours.insert(interface::reply_type_name(name));
    ours.insert(interface::call_union_name(name));
    ours.insert(interface::return_union_name(name));
    for operation in service.all_operations() {
        ours.insert(interface::in_struct_name(name, &operation.name));
        ours.insert(interface::out_struct_name(name, &operation.name));
        ours.insert(interface::result_union_name(name, &operation.name));
        ours.insert(interface::operation_hash_constant(name, &operation.name));
    }
    assert_eq!(names_of_the_zip(), ours);
}
