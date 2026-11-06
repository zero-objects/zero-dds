// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The generated service column against the mapping tables of
//! zerodds-corba-rust 1.1 (`docs/specs/zerodds-corba-rust-1.1.md`): §1
//! (module), §3 (interface), §3.1 (operation), §3.2 (attribute), §4.1
//! (valuetype), §5 (AMI), §10 (repository ids), and CCM 4.0 §6.9.1 for a
//! component. Each test checks the forms a table prescribes, not the
//! generator's earlier output; that the text is Rust and resolves is
//! `compile_check.rs`.

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

/// Parses, lowers and resolves — the three steps every consumer of the
/// rebuilt frontend takes.
///
/// The resolve pass is the caller's: `zerodds-idl-lower` leaves a name
/// it could not bind on the spot pending, and a model handed to a
/// generator without it has an unresolved type for every forward
/// reference. Asserted rather than assumed, because a generator over a
/// half-resolved model emits signatures that resolve to nothing and
/// says nothing about it.
fn model(idl: &str) -> zerodds_ast::Ast {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(
        parsed.diagnostics().iter().all(|d| !d.is_error()),
        "the IDL must parse: {:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "mapping.idl");
    assert!(
        lowered.diagnostics.iter().all(|d| !d.is_error()),
        "the IDL must lower: {:?}",
        lowered.diagnostics
    );
    let mut ast = lowered.ast;
    let report = ast.resolve_all_by_name().expect("a resolve pass");
    assert!(
        report.broken.is_empty(),
        "every name must resolve: {:?}",
        report.broken
    );
    ast
}

fn run(idl: &str) -> String {
    let ast = model(idl);
    generate_corba_rust_module(&ast, &CorbaRustGenOptions::default()).expect("gen")
}

/// `needle` occurs in `code`.
fn has(code: &str, needle: &str) {
    assert!(code.contains(needle), "missing `{needle}` in:\n{code}");
}

/// The text from the first `start` on.
fn from<'a>(code: &'a str, start: &str) -> &'a str {
    &code[code
        .find(start)
        .unwrap_or_else(|| panic!("no `{start}` in:\n{code}"))..]
}

/// `first` occurs ahead of `second` in `code`.
fn ahead(code: &str, first: &str, second: &str) {
    let a = code
        .find(first)
        .unwrap_or_else(|| panic!("no `{first}` in:\n{code}"));
    let b = code
        .find(second)
        .unwrap_or_else(|| panic!("no `{second}` in:\n{code}"));
    assert!(a < b, "`{first}` must come ahead of `{second}`:\n{code}");
}

const CORBA_RESULT: &str = "::core::result::Result";
const EXCEPTION: &str = "zerodds_corba_rust::CorbaException";

/// §1: an IDL `module` is a `pub mod`.
#[test]
fn corbarust_1_a_module_is_a_pub_mod() {
    let code = run("module finance { interface Account { double withdraw(in double amount); }; };");
    has(&code, "pub mod finance {");
    ahead(&code, "pub mod finance {", "pub trait Account");
    has(&code, "\"IDL:finance/Account:1.0\"");
}

/// §3: an interface is a trait, a client stub that sends the request with
/// `CorbaConnection::invoke`, and a skeleton `dispatch_<interface>`.
#[test]
fn corbarust_3_an_interface_is_a_trait_a_stub_and_a_dispatch_function() {
    let code = run("interface Calculator { long add(in long a, in long b); };");
    has(
        &code,
        "pub const CALCULATOR_REPOSITORY_ID: &str = \"IDL:Calculator:1.0\";",
    );
    has(&code, "pub trait Calculator");
    has(
        &code,
        &format!("fn add(&self, a: i32, b: i32) -> {CORBA_RESULT}<i32, {EXCEPTION}>;"),
    );
    has(&code, "pub struct CalculatorStub");
    has(&code, "impl Calculator for CalculatorStub");
    has(&code, "self.connection.invoke(&self.object_ref, \"add\"");
    has(&code, "pub fn dispatch_calculator(servant: &dyn Calculator");
}

/// §3.1: a `oneway` operation is sent with `invoke_oneway` and waits for
/// no reply.
#[test]
fn corbarust_3_1_a_oneway_operation_is_sent_without_waiting_for_a_reply() {
    let code = run("interface Logger { oneway void log(in string message); };");
    has(
        &code,
        &format!("fn log(&self, message: String) -> {CORBA_RESULT}<(), {EXCEPTION}>;"),
    );
    has(&code, "invoke_oneway(&self.object_ref, \"log\"");
    assert!(
        !code.contains(".invoke(&self.object_ref, \"log\""),
        "a oneway operation waits for no reply:\n{code}"
    );
}

/// §3.1: `out` and `inout` parameters are `&mut T`.
#[test]
fn corbarust_3_1_out_and_inout_parameters_are_mutable_references() {
    let code =
        run("interface Mutator { void update(inout long value); void produce(out long result); };");
    has(
        &code,
        &format!("fn update(&self, value: &mut i32) -> {CORBA_RESULT}<(), {EXCEPTION}>;"),
    );
    has(
        &code,
        &format!("fn produce(&self, result: &mut i32) -> {CORBA_RESULT}<(), {EXCEPTION}>;"),
    );
}

/// §3.1: with `raises (E1, E2)` the return is `Result<T, IError>`, and
/// `IError` has `System(CorbaException)` and one variant per raised
/// exception.
#[test]
fn corbarust_3_1_raises_returns_the_error_enum_of_the_interface() {
    let code = run(
        "exception NotFound { string what; }; exception Forbidden {}; \
         interface Vault { string lookup(in string key) raises (NotFound, Forbidden); };",
    );
    has(
        &code,
        &format!("fn lookup(&self, key: String) -> {CORBA_RESULT}<String, VaultError>;"),
    );
    let error = from(&code, "pub enum VaultError");
    has(error, &format!("System({EXCEPTION})"));
    has(error, "NotFound(NotFound)");
    has(error, "Forbidden(Forbidden)");
}

/// §3.2: a `readonly` attribute has a getter, a writable one a getter and
/// `set_<name>`.
#[test]
fn corbarust_3_2_a_readonly_attribute_has_a_getter_a_writable_one_also_a_setter() {
    let code = run("interface Counter { readonly attribute long count; attribute string label; };");
    has(
        &code,
        &format!("fn count(&self) -> {CORBA_RESULT}<i32, {EXCEPTION}>;"),
    );
    assert!(
        !code.contains("fn set_count"),
        "a readonly attribute has no setter:\n{code}"
    );
    has(
        &code,
        &format!("fn label(&self) -> {CORBA_RESULT}<String, {EXCEPTION}>;"),
    );
    has(
        &code,
        &format!("fn set_label(&self, value: String) -> {CORBA_RESULT}<(), {EXCEPTION}>;"),
    );
}

/// §4.1: a valuetype is a trait, a `<Name>Value` state carrier with every
/// state member, `ValueMarshal` in declaration order, and
/// `register_<name>_value`.
#[test]
fn corbarust_4_1_a_valuetype_is_a_trait_a_state_carrier_and_a_factory() {
    let code = run("valuetype Point { public long x; public long y; private string label; };");
    has(&code, "pub trait Point: zerodds_corba_rust::ValueBase");
    let carrier = from(&code, "pub struct PointValue");
    has(carrier, "pub x: i32,");
    has(carrier, "pub label: String,");
    let marshal = from(&code, "fn marshal_state");
    ahead(marshal, "w.write_i32(self.x);", "w.write_i32(self.y);");
    ahead(
        marshal,
        "w.write_i32(self.y);",
        "w.write_string(&self.label);",
    );
    has(&code, "pub fn register_point_value(");
}

/// §4.1: inherited state is flattened, the base's members first.
#[test]
fn corbarust_4_1_the_state_of_the_base_comes_first() {
    let code = run("valuetype Base { public long version; }; \
         valuetype Extended : Base { public string name; };");
    let extended = from(&code, "pub struct ExtendedValue");
    ahead(extended, "pub version: i32,", "pub name: String,");
    ahead(
        extended,
        "w.write_i32(self.version);",
        "w.write_string(&self.name);",
    );
}

#[test]
fn corbarust_10_an_exception_repository_id_has_its_scope_and_typeprefix() {
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
fn corbarust_5_ami_emits_the_handler_sendc_sendp_and_the_poller() {
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
fn corbarust_4_1_a_truncatable_valuetype_lists_its_base_ids() {
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
fn corbarust_5_no_ami_without_the_annotation() {
    // Without `@ami`, NO AMI code may be produced (gate correct).
    let code = run("interface Plain { long add(in long a); };");
    assert!(
        !code.contains("AmiHandler"),
        "AMI code emitted without @ami"
    );
    assert!(!code.contains("sendc_"), "sendc_ emitted without @ami");
}

/// CCM 4.0 §6.9.1 (p. 43) — "The `configuration_complete` operation
/// (inherited from `Components::CCMObject`) is invoked by the agent
/// effecting the configuration to signal the completion of the
/// configuration phase." The emitted component trait has to carry that
/// operation, or a generated component has no way of being told the
/// configuration phase is over.
#[test]
fn ccm_6_9_a_component_trait_carries_the_configuration_complete_lifecycle_operation() {
    let out = run("component Trader { attribute long limit; };");
    assert!(
        out.contains("pub trait Trader"),
        "no component trait was emitted:\n{out}"
    );
    assert!(
        out.contains("fn configuration_complete("),
        "§6.9 configuration_complete is missing:\n{out}"
    );
    assert!(
        out.contains("fn ccm_remove("),
        "the removal counterpart is missing:\n{out}"
    );
    assert!(
        out.contains("TRADER_REPOSITORY_ID"),
        "the component keeps no repository identifier:\n{out}"
    );
}
