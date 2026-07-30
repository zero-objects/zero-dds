// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-RPC C# PSM codegen tests (F.1 item 11 — csharp RPC parity with java).
//!
//! A `@service` interface previously degraded to a bare C# signature stub (a
//! regression vs the Java PSM). It now emits the five members — sync interface,
//! async interface, handler interface, requester, replier — with real
//! marshalling. These are marker-based assertions (robust to whitespace).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use zerodds_idl::config::ParserConfig;
use zerodds_idl_csharp::{CsGenOptions, generate_csharp};

fn g(idl: &str) -> String {
    // `full_4_2` enables the CORBA building blocks (incl. `oneway` operations).
    let ast = zerodds_idl::parse(idl, &ParserConfig::full_4_2()).expect("parse");
    generate_csharp(&ast, &CsGenOptions::default()).expect("gen")
}

#[test]
fn service_emits_five_members() {
    let cs = g("@service interface Calculator { long long add(in long long a, in long long b); };");
    assert!(cs.contains("public interface Calculator"), "{cs}");
    assert!(cs.contains("public interface CalculatorAsync"), "{cs}");
    assert!(cs.contains("public interface CalculatorService"), "{cs}");
    assert!(
        cs.contains("public sealed class CalculatorRequester"),
        "{cs}"
    );
    assert!(cs.contains("public sealed class CalculatorReplier"), "{cs}");
}

#[test]
fn sync_interface_carries_service_attribute() {
    let cs = g("@service interface Calc { void noop(); };");
    assert!(cs.contains("[Zerodds.Rpc.Service(\"Calc\")]"), "{cs}");
    assert!(cs.contains("public interface Calc"), "{cs}");
}

#[test]
fn async_interface_returns_task_of_return_type() {
    let cs = g("@service interface Calc { long long add(in long long a); };");
    assert!(
        cs.contains("System.Threading.Tasks.Task<long> AddAsync(long a)"),
        "{cs}"
    );
}

#[test]
fn sync_interface_returns_unboxed_primitive() {
    let cs = g("@service interface Calc { long long add(in long long a); };");
    assert!(cs.contains("long Add(long a);"), "{cs}");
}

#[test]
fn oneway_method_emits_oneway_attribute() {
    let cs = g("@service interface Logger { oneway void log(in string msg); };");
    assert!(cs.contains("[Zerodds.Rpc.Oneway]"), "{cs}");
    assert!(cs.contains("void Log(string msg);"), "{cs}");
}

#[test]
fn oneway_async_returns_task() {
    let cs = g("@service interface Logger { oneway void log(in string msg); };");
    assert!(
        cs.contains("System.Threading.Tasks.Task LogAsync(string msg)"),
        "{cs}"
    );
}

#[test]
fn out_param_uses_holder_pattern() {
    let cs = g("@service interface S { void result(out long v); };");
    assert!(cs.contains("Zerodds.Rpc.Holder<int> v"), "{cs}");
}

#[test]
fn inout_param_uses_holder_pattern() {
    let cs = g("@service interface S { void twice(inout long v); };");
    assert!(cs.contains("Zerodds.Rpc.Holder<int> v"), "{cs}");
}

#[test]
fn requester_implements_both_interfaces_and_holds_runtime() {
    let cs = g("@service interface Calc { void noop(); };");
    assert!(
        cs.contains("public sealed class CalcRequester : Calc, CalcAsync"),
        "{cs}"
    );
    assert!(
        cs.contains("private readonly Zerodds.Rpc.IRequester requester;"),
        "{cs}"
    );
}

#[test]
fn requester_marshals_object_tuple_and_sends_request() {
    let cs = g("@service interface Calc { long long add(in long long a, in long long b); };");
    assert!(
        cs.contains("requester.SendRequest(1, new object[] { a, b })"),
        "{cs}"
    );
}

#[test]
fn requester_oneway_uses_send_oneway() {
    let cs = g("@service interface Logger { oneway void log(in string m); };");
    assert!(
        cs.contains("requester.SendOneway(1, new object[] { m })"),
        "{cs}"
    );
}

#[test]
fn replier_constructor_takes_handler_and_dispatch_uses_method_ids() {
    let cs = g("@service interface Calc { void foo(); void bar(); };");
    assert!(
        cs.contains("public CalcReplier(Zerodds.Rpc.IReplier replier, CalcService handler)"),
        "{cs}"
    );
    assert!(cs.contains("case 1:"), "{cs}");
    assert!(cs.contains("case 2:"), "{cs}");
    assert!(cs.contains("RemoteExceptionCode.UnknownOperation"), "{cs}");
}

#[test]
fn replier_dispatch_calls_handler_and_packs_reply() {
    let cs = g("@service interface Calc { long long add(in long long a, in long long b); };");
    assert!(
        cs.contains("long __ret = handler.Add((long) __a[0], (long) __a[1]);"),
        "{cs}"
    );
    assert!(cs.contains("return new object?[] { __ret };"), "{cs}");
}

#[test]
fn non_service_interface_stays_plain_stub() {
    // Interface WITHOUT @service keeps the idiomatic plain-interface stub — the
    // RPC path must not hijack ordinary interfaces.
    let cs = g("interface Plain { long op(in long a); };");
    assert!(cs.contains("public interface Plain"), "{cs}");
    assert!(!cs.contains("Requester"), "{cs}");
    assert!(!cs.contains("Zerodds.Rpc.Service"), "{cs}");
}

#[test]
fn multi_service_emits_independent_class_sets() {
    let cs = g("@service interface Alpha { void a(); }; @service interface Beta { void b(); };");
    for anchor in [
        "public interface Alpha",
        "public sealed class AlphaRequester",
        "public sealed class AlphaReplier",
        "public interface Beta",
        "public sealed class BetaRequester",
        "public sealed class BetaReplier",
    ] {
        assert!(cs.contains(anchor), "missing {anchor}:\n{cs}");
    }
}
