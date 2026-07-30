// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Materializes the idlc-generated Go `Ping`/`Pong` types (package `main`) for
//! the native-frontends go ping-pong example. Run:
//!   cargo run -p zerodds-endpoint-e2e --example gen-go > types.go

#![allow(clippy::print_stdout, clippy::expect_used)]

use zerodds_endpoint_e2e::IDL;
use zerodds_idl::config::ParserConfig;
use zerodds_idl_go::{GoGenOptions, generate_go_module};

fn main() {
    let ast = zerodds_idl::parse(IDL, &ParserConfig::default()).expect("parse IDL");
    let src = generate_go_module(
        &ast,
        &GoGenOptions {
            package_name: "main".to_string(),
        },
    )
    .expect("generate go module");
    print!("{src}");
}
