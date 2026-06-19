// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Snapshot tests for the CORBA Rust codegen.

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
    // CORBA full profile — enables oneway ops, valuetypes,
    // private state members.
    let cfg = ParserConfig {
        features: IdlFeatures::corba_full(),
        ..ParserConfig::default()
    };
    let ast = zerodds_idl::parse(idl, &cfg).expect("parse");
    generate_corba_rust_module(&ast, &CorbaRustGenOptions::default()).expect("gen")
}

#[test]
fn scope_aware_exception_repo_id_with_typeprefix() {
    // #4(3): Exception RepositoryId uses the definition scope (module) + typeprefix.
    let idl = r#"
        typeprefix CosNaming "omg.org";
        module CosNaming {
            exception NotFound { long why; };
            interface NamingContext {
                long resolve(in long n) raises(NotFound);
            };
        };
    "#;
    let code = run(idl);
    // Skeleton/stub must carry the fully qualified RepoId with the omg.org prefix.
    assert!(
        code.contains("IDL:omg.org/CosNaming/NotFound:1.0"),
        "scope+typeprefix RepoId missing; gen:\n{code}"
    );
    assert!(
        !code.contains("\"IDL:NotFound:1.0\""),
        "flat RepoId must no longer appear"
    );
}

#[test]
fn ami_codegen_emits_handler_sendc_sendp_poller() {
    // CORBA Messaging §22: `@ami` → ReplyHandler-Trait + sendc_/sendp_ + Poller.
    let idl = r#"
        interface Bank {
            @ami long deposit(in long amount);
            @ami long transfer(in long amount, out long balance);
        };
    "#;
    let code = run(idl);
    // Callback model: handler trait + success/fault methods.
    assert!(
        code.contains("pub trait BankAmiHandler"),
        "handler trait missing:\n{code}"
    );
    assert!(
        code.contains("fn deposit(&self, __return: i32)"),
        "deposit reply missing"
    );
    assert!(
        code.contains("fn deposit_excep(&self, __excep: zerodds_corba_rust::CorbaException)"),
        "deposit_excep missing"
    );
    assert!(
        code.contains("fn transfer(&self, __return: i32, balance: i32)"),
        "transfer reply (ret + out) missing"
    );
    // sendc_ (callback) + sendp_ (polling) on the stub.
    assert!(
        code.contains("pub fn sendc_deposit"),
        "sendc_deposit missing"
    );
    assert!(
        code.contains("pub fn sendp_deposit"),
        "sendp_deposit missing"
    );
    // Typed poller: return-only → i32, return+out → tuple.
    assert!(
        code.contains("pub struct BankDepositPoller"),
        "poller struct missing"
    );
    assert!(
        code.contains(
            "pub fn get_reply(&self, __channel: &mut dyn zerodds_corba_rust::AsyncCorbaChannel) -> ::core::result::Result<i32"
        ),
        "Deposit poller get_reply -> i32 missing"
    );
    assert!(
        code.contains("-> ::core::result::Result<(i32, i32)"),
        "Transfer poller get_reply -> (ret, out) tuple missing"
    );
}

#[test]
fn truncatable_valuetype_emits_base_ids() {
    // `valuetype Derived : truncatable Base` → chunked/truncatable base-id list.
    let idl = r#"
        valuetype Base { public long id; };
        valuetype Derived : truncatable Base { public string extra; };
    "#;
    let code = run(idl);
    assert!(
        code.contains("pub const DERIVED_BASE_IDS: &[&str] = &[\"IDL:Base:1.0\"];"),
        "truncatable base-id list missing:\n{code}"
    );
    // Non-truncatable Base gets no list.
    assert!(!code.contains("BASE_BASE_IDS"), "Base is not truncatable");
}

#[test]
fn no_ami_codegen_without_annotation() {
    // Without `@ami`, NO AMI code may be produced (gate correct).
    let code = run("interface Plain { long add(in long a); };");
    assert!(
        !code.contains("AmiHandler"),
        "AMI code emitted without @ami"
    );
    assert!(!code.contains("sendc_"), "sendc_ emitted without @ami");
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
