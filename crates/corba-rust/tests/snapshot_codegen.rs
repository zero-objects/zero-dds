// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Snapshot-Tests fuer den CORBA-Rust-Codegen.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use zerodds_corba_rust::{CorbaRustGenOptions, generate_corba_rust_module};
use zerodds_idl::config::ParserConfig;
use zerodds_idl::features::IdlFeatures;

fn run(idl: &str) -> String {
    // CORBA-Full-Profil — aktiviert oneway-Ops, valuetypes,
    // private state-members.
    let cfg = ParserConfig {
        features: IdlFeatures::corba_full(),
        ..ParserConfig::default()
    };
    let ast = zerodds_idl::parse(idl, &cfg).expect("parse");
    generate_corba_rust_module(&ast, &CorbaRustGenOptions::default()).expect("gen")
}

#[test]
fn snapshot_simple_interface() {
    let idl = r#"
        interface Calculator {
            long add(in long a, in long b);
            long sub(in long a, in long b);
        };
    "#;
    insta::assert_snapshot!(run(idl));
}

#[test]
fn snapshot_interface_with_attribute() {
    let idl = r#"
        interface Counter {
            readonly attribute long count;
            attribute string label;
            void increment();
        };
    "#;
    insta::assert_snapshot!(run(idl));
}

#[test]
fn snapshot_interface_with_oneway_op() {
    let idl = r#"
        interface Logger {
            oneway void log(in string message);
        };
    "#;
    insta::assert_snapshot!(run(idl));
}

#[test]
fn snapshot_interface_with_inout_param() {
    let idl = r#"
        interface Mutator {
            void update(inout long value);
            void produce(out long result);
        };
    "#;
    insta::assert_snapshot!(run(idl));
}

#[test]
fn snapshot_valuetype_with_state_member() {
    let idl = r#"
        valuetype Point {
            public long x;
            public long y;
            private string label;
        };
    "#;
    insta::assert_snapshot!(run(idl));
}

#[test]
fn snapshot_interface_inheritance() {
    let idl = r#"
        interface Base {
            void ping();
        };
        interface Derived : Base {
            long get_id();
        };
    "#;
    insta::assert_snapshot!(run(idl));
}

#[test]
fn snapshot_interface_with_raises() {
    let idl = r#"
        exception NotFound {
            string what;
        };
        exception Forbidden {};
        interface Vault {
            string lookup(in string key) raises (NotFound, Forbidden);
        };
    "#;
    insta::assert_snapshot!(run(idl));
}

#[test]
fn snapshot_valuetype_with_init_factory() {
    let idl = r#"
        valuetype Account {
            public string id;
            public double balance;
            factory create(in string id, in double initial_balance);
        };
    "#;
    insta::assert_snapshot!(run(idl));
}

#[test]
fn snapshot_valuetype_with_inheritance() {
    let idl = r#"
        valuetype Base {
            public long version;
        };
        valuetype Extended : Base {
            public string name;
        };
    "#;
    insta::assert_snapshot!(run(idl));
}

#[test]
fn snapshot_module_with_interface() {
    let idl = r#"
        module finance {
            interface Account {
                attribute double balance;
                double withdraw(in double amount);
            };
        };
    "#;
    insta::assert_snapshot!(run(idl));
}
