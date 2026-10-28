// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The ZeroDDS OPC-UA client against a foreign server.
//!
//! Drives [`zerodds_opcua_server::client::Client`] over a real `opc.tcp`
//! socket against open62541's `tutorial_server_variable`. Every assertion is
//! about a value the foreign server chose, not one this tree produced: the
//! variable `ns=1;s="the.answer"` holds `Int32 42`
//! (`open62541/examples/tutorial_server_variable.c:25`), and the
//! ObjectsFolder `i=85` carries it as a forward reference.
//!
//! Usage: `open62541_probe [host:port]` — exits non-zero on any mismatch.

#![allow(clippy::print_stdout)]

use std::process::ExitCode;

use zerodds_opcua_gateway::data_value::{DataValue, Variant, VariantValue};
use zerodds_opcua_gateway::node_id::{NodeId, NodeIdentifier};
use zerodds_opcua_server::client::{Client, TcpTransport};

fn fail(what: &str) -> ExitCode {
    println!("PROBE-FAIL: {what}");
    ExitCode::FAILURE
}

fn main() -> ExitCode {
    let addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| String::from("127.0.0.1:4840"));
    let url = format!("opc.tcp://{addr}");

    // 1. Session-less discovery: Hello + OpenSecureChannel + GetEndpoints.
    let mut disc = match TcpTransport::connect(&addr) {
        Ok(t) => t,
        Err(e) => return fail(&format!("connect: {e}")),
    };
    let mut c = Client::new();
    if let Err(e) = c.open_channel(&mut disc, &url) {
        return fail(&format!("open_channel: {e}"));
    }
    let endpoints = match c.get_endpoints(&mut disc, &url) {
        Ok(e) => e,
        Err(e) => return fail(&format!("get_endpoints: {e}")),
    };
    println!("endpoints={}", endpoints.len());
    for ep in &endpoints {
        println!(
            "  endpoint url={:?} policy={:?} tokens={}",
            ep.endpoint_url,
            ep.security_policy_uri,
            ep.user_identity_tokens.len()
        );
    }
    if endpoints.is_empty() {
        return fail("the foreign server returned no endpoint");
    }
    drop(disc);

    // 2. Full session: Hello + OPN + CreateSession + ActivateSession.
    let mut t = match TcpTransport::connect(&addr) {
        Ok(t) => t,
        Err(e) => return fail(&format!("reconnect: {e}")),
    };
    let mut c = Client::new();
    if let Err(e) = c.connect(&mut t, &url) {
        return fail(&format!("connect handshake: {e}"));
    }
    println!("session_id={:?}", c.session_id());

    // 3. Read the foreign server's own variable.
    let answer = match NodeId::string(1, "the.answer") {
        Ok(n) => n,
        Err(e) => return fail(&format!("node id: {e}")),
    };
    let values = match c.read_values(&mut t, &[answer]) {
        Ok(v) => v,
        Err(e) => return fail(&format!("read: {e}")),
    };
    let Some(dv) = values.first() else {
        return fail("empty ReadResponse");
    };
    println!("read status={:?} value={:?}", dv.status, dv.value);
    match dv.value.as_ref().and_then(|v| v.value.first()) {
        Some(VariantValue::Int32(43)) => {
            println!("read: Int32(43) — the value the foreign server last wrote");
        }
        other => {
            return fail(&format!(
                "expected Int32(43) from the foreign server, got {other:?}"
            ));
        }
    }

    // 3b. Write through the foreign server's own Write service and read back.
    let Ok(answer) = NodeId::string(1, "the.answer") else {
        return fail("node id");
    };
    let written = DataValue {
        value: Some(Variant::scalar(VariantValue::Int32(4242))),
        status: None,
        source_timestamp: None,
        server_timestamp: None,
        source_pico_sec: None,
        server_pico_sec: None,
    };
    let results = match c.write_values(&mut t, &[(answer.clone(), written)]) {
        Ok(r) => r,
        Err(e) => return fail(&format!("write: {e}")),
    };
    println!("write results={results:?}");
    if results.first() != Some(&0) {
        return fail(&format!(
            "the foreign server rejected the write: {results:?}"
        ));
    }
    let back = match c.read_values(&mut t, &[answer]) {
        Ok(v) => v,
        Err(e) => return fail(&format!("read back: {e}")),
    };
    match back
        .first()
        .and_then(|d| d.value.as_ref())
        .and_then(|v| v.value.first())
    {
        Some(VariantValue::Int32(4242)) => {
            println!("read back: Int32(4242) — the foreign server kept what we wrote");
        }
        other => {
            return fail(&format!(
                "expected Int32(4242) after the write, got {other:?}"
            ));
        }
    }

    // 4. Browse the ObjectsFolder and find the same node by its identifier.
    let refs = match c.browse(&mut t, NodeId::numeric(0, 85)) {
        Ok(r) => r,
        Err(e) => return fail(&format!("browse: {e}")),
    };
    println!("browse(i=85) references={}", refs.len());
    let mut found = false;
    for r in &refs {
        println!(
            "  ref id={:?} browse_name={:?} class={:?}",
            r.node_id.node_id.identifier_type, r.browse_name.name, r.node_class
        );
        if let NodeIdentifier::String(s) = &r.node_id.node_id.identifier_type {
            if s == "the.answer" {
                found = true;
            }
        }
    }
    if !found {
        return fail("the ObjectsFolder browse did not carry ns=1;s=the.answer");
    }

    println!("PROBE-PASS");
    ExitCode::SUCCESS
}
