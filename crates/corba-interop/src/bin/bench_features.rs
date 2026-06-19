// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation
)]
//! Per-operation latency benchmark over the full CORBA feature matrix
//! (generated codegen path). Measures the self-roundtrip latency for each
//! IDL construct kind: `BenchStub::<op>` → CDR encode → IIOP/GIOP →
//! `dispatch_bench` skeleton → servant → reply → stub decode.
//!
//! Characterizes the per-feature cost (long/double/long long/char/string/
//! wstring/any-struct/any-seq/sequence/out-params/typed-exception/object-ref).
//!
//! Usage: `bench_features [iterations]` (default: 50000).

use std::sync::Arc;
use std::time::Instant;

use zerodds_corba_interop::runtime::{CorbaServer, IiopCorbaConnection, object_reference};
use zerodds_corba_rust::{CorbaConnection, CorbaException, ObjectReference};

include!(concat!(env!("OUT_DIR"), "/corba_gen.rs"));
use corba_gen::{Bench, BenchError, BenchStub, RangeError, dispatch_bench};

struct BenchImpl;
impl Bench for BenchImpl {
    fn add(&self, a: i32, b: i32) -> Result<i32, CorbaException> {
        Ok(a.wrapping_add(b))
    }
    fn scale(&self, x: f64, factor: f64) -> Result<f64, CorbaException> {
        Ok(x * factor)
    }
    fn add64(&self, a: i64, b: i64) -> Result<i64, CorbaException> {
        Ok(a.wrapping_add(b))
    }
    fn next_char(&self, c: u8) -> Result<u8, CorbaException> {
        Ok(c.wrapping_add(1))
    }
    fn concat(&self, a: String, b: String) -> Result<String, CorbaException> {
        Ok(format!("{a}{b}"))
    }
    fn wecho(&self, w: zerodds_cdr::WString) -> Result<zerodds_cdr::WString, CorbaException> {
        Ok(w)
    }
    fn aecho(&self, a: zerodds_cdr::CorbaAny) -> Result<zerodds_cdr::CorbaAny, CorbaException> {
        Ok(a)
    }
    fn reverse(&self, xs: Vec<i32>) -> Result<Vec<i32>, CorbaException> {
        let mut v = xs;
        v.reverse();
        Ok(v)
    }
    fn divmod(&self, a: i32, b: i32, q: &mut i32, r: &mut i32) -> Result<(), CorbaException> {
        *q = a / b;
        *r = a % b;
        Ok(())
    }
    fn increment(&self, x: &mut i32) -> Result<(), CorbaException> {
        *x += 1;
        Ok(())
    }
    fn echo_ref(&self, o: ObjectReference) -> Result<ObjectReference, CorbaException> {
        Ok(o)
    }
    fn checked(&self, idx: i32, limit: i32) -> Result<i32, BenchError> {
        if idx < limit {
            Ok(idx)
        } else {
            Err(BenchError::RangeError(RangeError {
                requested: idx,
                limit,
            }))
        }
    }
}

fn bench<F: FnMut()>(label: &str, n: usize, mut f: F) {
    for _ in 0..2000u32 {
        f();
    }
    let mut s: Vec<u64> = Vec::with_capacity(n);
    for _ in 0..n {
        let t0 = Instant::now();
        f();
        s.push(t0.elapsed().as_nanos() as u64);
    }
    s.sort_unstable();
    let us = |q: f64| s[((n as f64 * q) as usize).min(n - 1)] as f64 / 1000.0;
    println!(
        "  {label:<30} p50={:>6.2}us  p90={:>6.2}us  p99={:>7.2}us",
        us(0.50),
        us(0.90),
        us(0.99),
    );
}

fn main() {
    let n: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(50_000);

    let key: &[u8] = b"Bench";
    let server = CorbaServer::new();
    let servant = Arc::new(BenchImpl);
    server.register(key, move |op, body, e| {
        dispatch_bench(&*servant, op, body, e)
    });
    let acceptor = server.serve("127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = acceptor.listen_addr();

    let conn: Arc<dyn CorbaConnection + Send + Sync> = Arc::new(IiopCorbaConnection::new());
    let self_ref = object_reference("IDL:Bench:1.0", &addr.ip().to_string(), addr.port(), key);
    let b = BenchStub::new(self_ref.clone(), conn);

    // Pre-built arguments (outside the measurement loop).
    let w = zerodds_cdr::WString::from("wíde-€-Ω");
    let any_struct = zerodds_cdr::CorbaAny(zerodds_cdr::AnyValue::Struct {
        repo_id: "IDL:AnyPair:1.0".into(),
        name: "AnyPair".into(),
        members: vec![
            ("k".into(), zerodds_cdr::AnyValue::Long(7)),
            ("v".into(), zerodds_cdr::AnyValue::Str("seven".into())),
        ],
    });
    let any_seq = zerodds_cdr::CorbaAny(zerodds_cdr::AnyValue::Seq {
        element: zerodds_cdr::TypeCode::Long,
        items: vec![
            zerodds_cdr::AnyValue::Long(10),
            zerodds_cdr::AnyValue::Long(20),
            zerodds_cdr::AnyValue::Long(30),
        ],
    });

    println!("ZeroDDS CORBA Feature-Matrix per-Operation-Latenz (Codegen, IIOP loopback, N={n})");
    bench("add (long)", n, || {
        b.add(2, 3).unwrap();
    });
    bench("scale (double, 8-aligned)", n, || {
        b.scale(2.5, 4.0).unwrap();
    });
    bench("add64 (long long, 8-aligned)", n, || {
        b.add64(1_000_000_000_000, 1).unwrap();
    });
    bench("next_char (char, 1 byte)", n, || {
        b.next_char(b'A').unwrap();
    });
    bench("concat (string)", n, || {
        b.concat("foo".into(), "bar".into()).unwrap();
    });
    bench("wecho (wstring/UTF-16 BOM)", n, || {
        b.wecho(w.clone()).unwrap();
    });
    bench("aecho (any: struct TypeCode)", n, || {
        b.aecho(any_struct.clone()).unwrap();
    });
    bench("aecho (any: sequence<long>)", n, || {
        b.aecho(any_seq.clone()).unwrap();
    });
    bench("reverse (sequence<long> x3)", n, || {
        b.reverse(vec![1, 2, 3]).unwrap();
    });
    bench("divmod (2x out-param)", n, || {
        let mut q = 0;
        let mut r = 0;
        b.divmod(17, 5, &mut q, &mut r).unwrap();
    });
    bench("checked ok (raises-capable)", n, || {
        b.checked(3, 10).unwrap();
    });
    bench("checked raises (UserException)", n, || {
        let _ = b.checked(15, 10);
    });
    bench("echo_ref (Object/IOR)", n, || {
        b.echo_ref(self_ref.clone()).unwrap();
    });

    acceptor.shutdown();
}
