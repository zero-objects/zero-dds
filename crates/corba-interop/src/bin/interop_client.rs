// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::panic
)]
//! Cross-ORB interop client: calls the ZeroDDS-generated stubs through a
//! stringified IOR (produced by any ORB).
//!
//! Usage:
//!   interop_client echo     <IOR>   — calls Echo::ping("zerodds↔orb")
//!   interop_client features <IOR>   — exercises the feature matrix
//!
//! Exit code 0 = all assertions green, 1 = mismatch (interop error).

use std::sync::Arc;

use zerodds_corba_interop::runtime::{IiopCorbaConnection, object_reference_from_ior};
use zerodds_corba_rust::CorbaConnection;

include!(concat!(env!("OUT_DIR"), "/corba_gen.rs"));
use corba_gen::CosNaming::{NameComponent, NamingContext, NamingContextStub};
use corba_gen::{Bench, BenchError, BenchStub, Echo, EchoStub, Features, FeaturesStub, RangeError};

fn conn() -> Arc<dyn CorbaConnection + Send + Sync> {
    Arc::new(IiopCorbaConnection::new())
}

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_default();
    let ior = std::env::args().nth(2).expect("IOR argument missing");
    let obj = object_reference_from_ior(&ior).expect("IOR parse");

    match mode.as_str() {
        "echo" => {
            let stub = EchoStub::new(obj, conn());
            let r = stub.ping("zerodds<->orb".to_string()).unwrap();
            assert_eq!(r, "zerodds<->orb", "Echo-Mismatch");
            let big = "x".repeat(4096);
            assert_eq!(stub.ping(big.clone()).unwrap(), big, "Echo-4k-Mismatch");
            println!("OK echo: ping roundtrip (klein + 4k)");
        }
        "bench" => {
            // Cross-ORB matrix: identical to the omniORB/TAO/JacORB servant.
            let self_ref = obj.clone();
            let b = BenchStub::new(obj, conn());
            check("add", b.add(2, 3).unwrap(), 5);
            check_f64(
                "scale (double, 8-aligned)",
                b.scale(2.5, 4.0).unwrap(),
                10.0,
            );
            check(
                "add64 (long long, 8-aligned)",
                b.add64(1_000_000_000_000, 1).unwrap(),
                1_000_000_000_001,
            );
            check(
                "next_char (char = 1 Byte)",
                b.next_char(b'A').unwrap(),
                b'B',
            );
            check_s(
                "concat",
                b.concat("foo".into(), "bar".into()).unwrap(),
                "foobar",
            );
            // wstring (UTF-16 wire with BOM §15.3.1.6): non-ASCII roundtrip
            // against the foreign-ORB server. Proves codeset negotiation.
            let w = zerodds_cdr::WString::from("wíde-€-Ω");
            check("wecho (wstring/UTF-16 BOM)", b.wecho(w.clone()).unwrap(), w);
            // Structured any (§15.3.5 TypeCode) against the foreign-ORB server:
            // struct AnyPair{long k; string v;} (RepositoryId IDL:AnyPair:1.0,
            // must match the foreign IDL TypeCode) + sequence<long>.
            let pair = zerodds_cdr::CorbaAny(zerodds_cdr::AnyValue::Struct {
                repo_id: "IDL:AnyPair:1.0".into(),
                name: "AnyPair".into(),
                members: vec![
                    ("k".into(), zerodds_cdr::AnyValue::Long(7)),
                    ("v".into(), zerodds_cdr::AnyValue::Str("seven".into())),
                ],
            });
            check(
                "aecho(struct AnyPair)",
                b.aecho(pair.clone()).unwrap(),
                pair,
            );
            let seq = zerodds_cdr::CorbaAny(zerodds_cdr::AnyValue::Seq {
                element: zerodds_cdr::TypeCode::Long,
                items: vec![
                    zerodds_cdr::AnyValue::Long(10),
                    zerodds_cdr::AnyValue::Long(20),
                    zerodds_cdr::AnyValue::Long(30),
                ],
            });
            check("aecho(sequence<long>)", b.aecho(seq.clone()).unwrap(), seq);
            check_v("reverse", b.reverse(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
            let mut q = 0;
            let mut r = 0;
            b.divmod(17, 5, &mut q, &mut r).unwrap();
            check("divmod.q", q, 3);
            check("divmod.r", r, 2);
            let mut x = 41;
            b.increment(&mut x).unwrap();
            check("increment", x, 42);
            // Object reference roundtrip (wire = IOR): echo of our own Bench ref.
            // Verification by USE, not by byte equality — foreign ORBs (TAO)
            // re-marshal the IOR equivalently, but not byte-identically
            // (component order/encapsulation). The echoed ref must be a live,
            // callable reference.
            let echoed = b.echo_ref(self_ref.clone()).unwrap();
            let rb = BenchStub::new(echoed, conn());
            check("echo_ref (Object/IOR, live call)", rb.add(2, 3).unwrap(), 5);
            // Typed user exception (raises RangeError) — fields must roundtrip.
            check("checked(ok)", b.checked(3, 10).unwrap(), 3);
            match b.checked(15, 10) {
                Err(BenchError::RangeError(RangeError { requested, limit })) => {
                    check("checked raises RangeError.requested", requested, 15);
                    check("checked raises RangeError.limit", limit, 10);
                }
                other => panic!("erwartete RangeError, bekam {other:?}"),
            }
            println!("OK bench: alle Operationen roundtrip (inkl. Object-Ref + typed exception)");
        }
        "features" => {
            let f = FeaturesStub::new(obj, conn());
            check("add", f.add(2, 3).unwrap(), 5);
            check_f64("scale", f.scale(2.5, 4.0).unwrap(), 10.0);
            check("toggle", f.toggle(true).unwrap(), false);
            check("xor_byte", f.xor_byte(0xF0, 0x0F).unwrap(), 0xFF);
            check("neg_short", f.neg_short(-7).unwrap(), 7);
            check("uadd", f.uadd(10, 20).unwrap(), 30);
            check(
                "add64",
                f.add64(1_000_000_000_000, 1).unwrap(),
                1_000_000_000_001,
            );
            check_s(
                "concat",
                f.concat("foo".into(), "bar".into()).unwrap(),
                "foobar",
            );
            check_v("reverse", f.reverse(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
            check_vs(
                "echo_seq",
                f.echo_seq(vec!["a".into(), "b".into()]).unwrap(),
                vec!["a".to_string(), "b".to_string()],
            );
            let mut q = 0;
            let mut r = 0;
            f.divmod(17, 5, &mut q, &mut r).unwrap();
            check("divmod.q", q, 3);
            check("divmod.r", r, 2);
            let mut x = 41;
            f.increment(&mut x).unwrap();
            check("increment", x, 42);
            let mut a = 1;
            let mut b = 2;
            f.swap(&mut a, &mut b).unwrap();
            check("swap.a", a, 2);
            check("swap.b", b, 1);
            f.fire(99).unwrap();
            check("answer", f.answer().unwrap(), 42);
            f.set_label("changed".into()).unwrap();
            check_s("label", f.label().unwrap(), "changed");
            println!("OK features: 18 Operationen/Attribute roundtrip");
        }
        "naming" => {
            // Drives a CosNaming::NamingContext (foreign daemon or ZeroDDS)
            // over the real OMG wire: bind/resolve/rebind/unbind + the typed
            // NotFound exception (cross-ORB, exact RepositoryId
            // IDL:omg.org/CosNaming/NamingContext/NotFound:1.0 thanks to #4(3)).
            use corba_gen::CosNaming::NamingContextError;
            let nc = NamingContextStub::new(obj.clone(), conn());
            // Single-level name → binds at root level (no intermediate context
            // needed), works against ZeroDDS AND foreign daemons.
            let name = vec![NameComponent {
                id: "zerodds-interop".into(),
                kind: "obj".into(),
            }];
            // We bind an object ref (the NamingContext itself) and resolve it.
            nc.bind(name.clone(), obj.clone())
                .expect("bind in NamingContext");
            let got = nc.resolve(name.clone()).expect("resolve");
            check(
                "resolve returns a non-nil ref",
                got.iiop_profile.is_empty(),
                false,
            );
            // rebind overwrites without AlreadyBound.
            nc.rebind(name.clone(), obj.clone()).expect("rebind");
            // unbind removes the binding.
            nc.unbind(name.clone()).expect("unbind");
            // resolve after unbind → typed NotFound exception over the real
            // CosNaming wire (cross-ORB: the foreign daemon throws NotFound, our
            // stub decodes it via the exact RepositoryId).
            match nc.resolve(name.clone()) {
                Err(NamingContextError::NotFound(_)) => {
                    check("resolve(unbound) → NotFound", true, true)
                }
                other => panic!("expected NotFound after unbind, got {other:?}"),
            }
            // Federation (sub-context graph, spec §2.5.4.7/.8): bind_new_context
            // creates a sub-context + binds it; through the returned
            // NamingContext ref we bind a leaf; a compound name from the root
            // traverses into the sub-context. Drives both the ZeroDDS server and
            // foreign daemons (omniNames/TAO/JacORB).
            let ctx_name = vec![NameComponent {
                id: "sub".into(),
                kind: "ctx".into(),
            }];
            // Idempotency: a persistent daemon (JacORB writes its naming DB to
            // disk and reloads it on restart) may still carry `sub/ctx` from an
            // earlier run → bind_new_context would throw AlreadyBound. A prior
            // unbind (NotFound on a fresh daemon ignored) makes the test
            // re-runnable against all daemons.
            let _ = nc.unbind(ctx_name.clone());
            let sub_ref = nc
                .bind_new_context(ctx_name.clone())
                .expect("bind_new_context");
            let sub = NamingContextStub::new(sub_ref, conn());
            let leaf = vec![NameComponent {
                id: "leaf".into(),
                kind: "obj".into(),
            }];
            sub.bind(leaf.clone(), obj.clone())
                .expect("bind leaf in sub-context");
            // resolve directly via the sub-context ref:
            let via_sub = sub.resolve(leaf.clone()).expect("resolve via sub-ref");
            check(
                "federation: resolve via sub-context-ref",
                via_sub.iiop_profile.is_empty(),
                false,
            );
            // compound resolve from the root (traversal into the sub-context):
            let compound = vec![
                NameComponent {
                    id: "sub".into(),
                    kind: "ctx".into(),
                },
                NameComponent {
                    id: "leaf".into(),
                    kind: "obj".into(),
                },
            ];
            let via_root = nc
                .resolve(compound)
                .expect("compound resolve from the root");
            check(
                "federation: compound-name resolve from the root",
                via_root.iiop_profile.is_empty(),
                false,
            );
            println!(
                "OK naming: bind/resolve/rebind/unbind + NotFound + federation (bind_new_context + compound-resolve) over CosNaming wire"
            );
        }
        other => {
            eprintln!("unknown mode: {other}");
            std::process::exit(2);
        }
    }
}

fn check<T: PartialEq + std::fmt::Debug>(name: &str, got: T, want: T) {
    assert!(got == want, "{name}: got {got:?}, want {want:?}");
    println!("  ✓ {name}");
}
fn check_f64(name: &str, got: f64, want: f64) {
    assert!((got - want).abs() < 1e-9, "{name}: got {got}, want {want}");
    println!("  ✓ {name}");
}
fn check_s(name: &str, got: String, want: &str) {
    assert!(got == want, "{name}: got {got:?}, want {want:?}");
    println!("  ✓ {name}");
}
fn check_v(name: &str, got: Vec<i32>, want: Vec<i32>) {
    assert!(got == want, "{name}: got {got:?}, want {want:?}");
    println!("  ✓ {name}");
}
fn check_vs(name: &str, got: Vec<String>, want: Vec<String>) {
    assert!(got == want, "{name}: got {got:?}, want {want:?}");
    println!("  ✓ {name}");
}
