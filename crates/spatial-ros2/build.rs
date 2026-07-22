// SPDX-License-Identifier: Apache-2.0
//! Generates the ROS 2 message Rust types from the vendored `ros_msgs.idl`
//! using ZeroDDS' own IDL->Rust codegen — the same pipeline that produces the
//! SpatialDDS types, so both sides of the semantic mapping are ZeroDDS-generated
//! and carry ZeroDDS' XCDR encode/decode.
// A build script legitimately fails the build by panicking on a bad IDL / env.
#![allow(clippy::expect_used, clippy::unwrap_used)]
use std::path::PathBuf;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_rust::{RustGenOptions, generate_rust_module};

/// The ROS 2 top-level packages carried by `ros_msgs.idl`. The codegen emits
/// crate-root-relative paths (`std_msgs::msg::Header`); included into this
/// crate they must be `crate::`-anchored so edition-2024 path resolution does
/// not read them as extern crates.
const ROS_PACKAGES: &[&str] = &[
    "builtin_interfaces",
    "std_msgs",
    "geometry_msgs",
    "tf2_msgs",
    "sensor_msgs",
    "vision_msgs",
    "geographic_msgs",
];

fn main() {
    println!("cargo:rerun-if-changed=idl/ros_msgs.idl");
    let idl = std::fs::read_to_string("idl/ros_msgs.idl").expect("read ros_msgs.idl");
    let ast = zerodds_idl::parse(&idl, &ParserConfig::default()).expect("parse ROS msg IDL");
    let opts = RustGenOptions {
        cdr_only: true,
        ..RustGenOptions::default()
    };
    let rust = generate_rust_module(&ast, &opts).expect("generate Rust");
    // Strip the crate-level `#![allow(..)]` inner attributes — they cannot sit
    // after other items once this is `include!`d at the crate root; the
    // `#[allow]` on the include site covers them.
    let mut rust: String = rust
        .lines()
        .filter(|l| !l.trim_start().starts_with("#!["))
        .collect::<Vec<_>>()
        .join("\n");
    for pkg in ROS_PACKAGES {
        rust = rust.replace(&format!("{pkg}::"), &format!("crate::{pkg}::"));
    }
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("ros_msgs.rs");
    std::fs::write(&out, rust).expect("write generated module");
}
