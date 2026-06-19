// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Cross-ORB OTS conformance: ZeroDDS decodes a **live JacORB 3.9
// PropagationContext** captured from a real transaction.
//
// Provenance (codepit, JDK8, JacORB /opt/jacorb): a JacORB
// `org.jacorb.transaction.TransactionService` (the runnable OTS) created a
// transaction via `TransactionFactory.create(0)`; the live `Control` yielded a
// real `Coordinator` + `Terminator`; the `PropagationContext` was built exactly
// as `TransactionCurrentImpl.begin()` does (current TransIdentity = {coordinator,
// terminator, otid}, no parents, tk_null any) and serialized via
// `org.omg.CosTransactions.PropagationContextHelper.write` (big-endian).
//
// This is the exact byte sequence JacORB's `ClientContextTransferInterceptor`
// puts into IIOP service context id=0 on a transactional invocation. Unlike the
// in-crate byte-identity golden (which uses NIL coordinator/terminator refs),
// this carries **full live IOR object references** (IDL:omg.org/CosTransactions/
// Coordinator:1.0 + Terminator:1.0, each with an IIOP profile, IPv6 alternate
// addresses, and a JacORB codeset component) — exercising the IOR-bearing
// TransIdentity decode path against a real foreign ORB.
//
// Capture program: see the session log / `DumpOts.java`; the otid was set to
// formatID=7, bqual_length=3, tid=[0xAA,0xBB,0xCC].

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Cross-ORB OTS: ZeroDDS decodes a live JacORB 3.9 PropagationContext.

use zerodds_cdr::{BufferReader, Endianness};
use zerodds_corba_cos_transactions::PropagationContext;

/// Live JacORB 3.9 PropagationContext (big-endian, 628 bytes) — captured from a
/// real TransactionService coordinator. Full coordinator + terminator IORs.
const JACORB_LIVE_PROPCTX: &str = "000000000000002c49444c3a6f6d672e6f72672f436f735472616e73616374696f6e732f436f6f7264696e61746f723a312e30000000000100000000000000f000010200000000103139322e3136382e3137382e313135008e45000000000020333733323638343636372f01043c4b3c0e0f49241e100630463814141b484c1b000000040000000300000028000000000000001e326130323a3930383a633231333a396230303a303a303a303a31316238008e4500000003000000320000000000000027326130323a3930383a633231333a396230303a393530633a663630333a323563343a6137626200008e4500000000000000000008000000004a4143000000000100000028000000000001000100000003000100010001000f05010001000101090000000205010001000101000000002b49444c3a6f6d672e6f72672f436f735472616e73616374696f6e732f5465726d696e61746f723a312e3000000000000100000000000000f000010200000000103139322e3136382e3137382e313135008e45000000000020333733323638343636372f02043c4b3c0e0f49241e100630463814141b484c1b000000040000000300000028000000000000001e326130323a3930383a633231333a396230303a303a303a303a31316238008e4500000003000000320000000000000027326130323a3930383a633231333a396230303a393530633a663630333a323563343a6137626200008e4500000000000000000008000000004a4143000000000100000028000000000001000100000003000100010001000f0501000100010109000000020501000100010100000000070000000300000003aabbcc000000000000000000";

fn hex_to_bytes(s: &str) -> Vec<u8> {
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap())
        .collect()
}

#[test]
fn decodes_live_jacorb_propagation_context_with_real_iors() {
    let bytes = hex_to_bytes(JACORB_LIVE_PROPCTX);
    let mut r = BufferReader::new(&bytes, Endianness::Big);
    let ctx = PropagationContext::decode(&mut r).expect("decode live JacORB PropagationContext");

    // timeout 0, flat transaction (no parents).
    assert_eq!(ctx.timeout, 0, "timeout");
    assert!(ctx.parents.is_empty(), "flat transaction has no parents");

    // The otid JacORB carried: formatID=7, bqual_length=3, tid=[AA,BB,CC].
    assert_eq!(ctx.current.otid.format_id, 7, "otid formatID");
    assert_eq!(ctx.current.otid.bequeath_length, 3, "otid bequeath_length");
    assert_eq!(
        ctx.current.otid.tid,
        vec![0xAA, 0xBB, 0xCC],
        "otid transaction id"
    );

    // Coordinator + Terminator are FULL live IORs (non-nil) — the live-capture
    // path that nil-ref tests never exercise. Each must carry its OMG type id.
    assert!(
        !ctx.current.coord_ior.is_empty(),
        "live coordinator IOR must be non-nil"
    );
    assert!(
        !ctx.current.term_ior.is_empty(),
        "live terminator IOR must be non-nil"
    );
    let coord_id = String::from_utf8_lossy(&ctx.current.coord_ior);
    assert!(
        coord_id.contains("IDL:omg.org/CosTransactions/Coordinator:1.0"),
        "coordinator IOR type id, got: {coord_id:?}"
    );
    let term_id = String::from_utf8_lossy(&ctx.current.term_ior);
    assert!(
        term_id.contains("IDL:omg.org/CosTransactions/Terminator:1.0"),
        "terminator IOR type id"
    );
}

#[test]
fn live_context_re_encodes_and_round_trips() {
    // Decode the live JacORB context, re-encode, decode again → structurally
    // stable (the IOR bytes are re-serialized canonically, otid + refs preserved).
    let bytes = hex_to_bytes(JACORB_LIVE_PROPCTX);
    let mut r = BufferReader::new(&bytes, Endianness::Big);
    let ctx = PropagationContext::decode(&mut r).expect("decode");

    let mut w = zerodds_cdr::BufferWriter::new(Endianness::Big);
    ctx.encode(&mut w).expect("re-encode");
    let re = w.into_bytes();
    let mut r2 = BufferReader::new(&re, Endianness::Big);
    let ctx2 = PropagationContext::decode(&mut r2).expect("decode round-trip");

    assert_eq!(
        ctx, ctx2,
        "live JacORB context must survive a ZeroDDS round-trip"
    );
}

/// The **actual** service-context-id=0 payload JacORB's
/// `ClientContextTransferInterceptor` transmits on a live transactional
/// invocation — captured at the ZeroDDS server. Where `PropagationContextHelper.
/// write` emits the bare struct, JacORB's interceptor uses `Codec.encode(any)`,
/// so the payload is a byte-order octet, then a full `TypeCode` (note the
/// embedded member names `PropagationContext`/`TransIdentity`/`Coordinator`/
/// `otid_t`/`formatID`), then the value. ZeroDDS must accept this non-spec form
/// to interoperate with a live JacORB OTS; the otid is JacORB-generated.
const JACORB_LIVE_SC0: &str = "000000000000000f00000264000000000000003349444c3a6f6d672e6f72672f436f735472616e73616374696f6e732f50726f7061676174696f6e436f6e746578743a312e3000000000001350726f7061676174696f6e436f6e746578740000000000040000000874696d656f757400000000050000000863757272656e74000000000f0000019c000000000000002e49444c3a6f6d672e6f72672f436f735472616e73616374696f6e732f5472616e734964656e746974793a312e300000000000000e5472616e734964656e746974790000000000000300000006636f6f72640000000000000e00000044000000000000002c49444c3a6f6d672e6f72672f436f735472616e73616374696f6e732f436f6f7264696e61746f723a312e30000000000c436f6f7264696e61746f7200000000057465726d000000000000000e00000043000000000000002b49444c3a6f6d672e6f72672f436f735472616e73616374696f6e732f5465726d696e61746f723a312e3000000000000b5465726d696e61746f720000000000056f746964000000000000000f00000088000000000000002749444c3a6f6d672e6f72672f436f735472616e73616374696f6e732f6f7469645f743a312e300000000000076f7469645f7400000000000300000009666f726d6174494400000000000000030000000d627175616c5f6c656e67746800000000000000030000000474696400000000130000000c000000000000000a0000000000000008706172656e747300000000130000001000000000fffffffffffffe40000000000000001d696d706c656d656e746174696f6e5f73706563696669635f64617461000000000000000b0000001e0000002c49444c3a6f6d672e6f72672f436f735472616e73616374696f6e732f436f6f7264696e61746f723a312e30000000000100000000000000f000010200000000103139322e3136382e3137382e313135009e53000000000020333539373933333633302f01043d154923340a2201100630463814141b484c1b000000040000000300000028000000000000001e326130323a3930383a633231333a396230303a303a303a303a31316238009e5300000003000000320000000000000027326130323a3930383a633231333a396230303a393530633a663630333a323563343a6137626200009e5300000000000000000008000000004a4143000000000100000028000000000001000100000003000100010001000f05010001000101090000000205010001000101000000002b49444c3a6f6d672e6f72672f436f735472616e73616374696f6e732f5465726d696e61746f723a312e3000000000000100000000000000f000010200000000103139322e3136382e3137382e313135009e53000000000020333539373933333633302f02043d154923340a2201100630463814141b484c1b000000040000000300000028000000000000001e326130323a3930383a633231333a396230303a303a303a303a31316238009e5300000003000000320000000000000027326130323a3930383a633231333a396230303a393530633a663630333a323563343a6137626200009e5300000000000000000008000000004a4143000000000100000028000000000001000100000003000100010001000f0501000100010109000000020501000100010100000000000000000000000000000000000000000e0000003c000000000000002849444c3a6f6d672e6f72672f436f735472616e73616374696f6e732f436f6e74726f6c3a312e300000000008436f6e74726f6c000000002849444c3a6f6d672e6f72672f436f735472616e73616374696f6e732f436f6e74726f6c3a312e30000000000100000000000000f000010200000000103139322e3136382e3137382e313135009e53000000000020333539373933333633302f03043d154923340a2201100630463814141b484c1b000000040000000300000028000000000000001e326130323a3930383a633231333a396230303a303a303a303a31316238009e5300000003000000320000000000000027326130323a3930383a633231333a396230303a393530633a663630333a323563343a6137626200009e5300000000000000000008000000004a4143000000000100000028000000000001000100000003000100010001000f0501000100010109000000020501000100010100";

#[test]
fn decodes_jacorb_interceptor_any_wrapped_service_context() {
    // The non-spec, TypeCode-wrapped form JacORB's interceptor actually sends on
    // a live `Current.begin()` transaction. `from_service_context_data` must skip
    // the leading TypeCode and decode the value (liberal-in, spec-out).
    let data = hex_to_bytes(JACORB_LIVE_SC0);
    let ctx = PropagationContext::from_service_context_data(&data)
        .expect("decode JacORB any-wrapped SC id=0");
    // JacORB's transaction default timeout (seconds) propagated over the wire.
    assert_eq!(ctx.timeout, 30, "JacORB default transaction timeout");
    assert!(ctx.parents.is_empty(), "flat transaction (no parents)");
    // The live coordinator IOR (real object reference, ~300 bytes incl. IIOP
    // profile + IPv6 alternates) — the structural proof the value decoded at the
    // right offset past the TypeCode.
    assert!(
        ctx.current.coord_ior.len() > 100,
        "live coordinator IOR present, got {} bytes",
        ctx.current.coord_ior.len()
    );
    assert!(
        String::from_utf8_lossy(&ctx.current.coord_ior)
            .contains("IDL:omg.org/CosTransactions/Coordinator:1.0"),
        "coordinator IOR carries the OMG Coordinator type id"
    );
}
