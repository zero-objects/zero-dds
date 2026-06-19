// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]
//! Cross-ORB interop server: a ZeroDDS CORBA server that prints its objects as
//! stringified IORs so foreign ORBs (omniORB/TAO/JacORB) can call it.
//!
//! Registers the feature-matrix servants (Echo / Bench) via the generated
//! `dispatch_<iface>` and prints one `<NAME>_IOR=IOR:…` line per servant to
//! stdout. Runs until SIGTERM/SIGKILL.
//!
//! Usage: `interop_server [host] [port]` (default: 127.0.0.1 0=ephemeral).

use std::sync::Arc;

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use zerodds_corba_interop::runtime::{
    CorbaServer, object_reference, object_reference_from_ior, stringify_object_ref,
    stringify_object_reference,
};
use zerodds_corba_rust::{CorbaException, ObjectReference};

include!(concat!(env!("OUT_DIR"), "/corba_gen.rs"));
use corba_gen::CosNaming::{NamingContext, NamingContextError, NotFound, dispatch_namingcontext};
use corba_gen::{
    Bench, BenchError, Echo, Features, RangeError, dispatch_bench, dispatch_echo, dispatch_features,
};

const NAMINGCONTEXT_TYPE_ID: &str = "IDL:omg.org/CosNaming/NamingContext:1.0";

// ---- CosNaming::NamingContext servant (real OMG wire) ----------------------

/// Shared server runtime for the NamingContext federation: allows sub-contexts
/// created at runtime (`new_context`/`bind_new_context`) to be published as
/// their own CORBA objects (object key + IOR), so that a foreign ORB can narrow
/// and call them directly. `endpoint` is only set after `serve()` (real
/// host/port) — sub-context creation happens only after the first client call
/// anyway.
#[derive(Clone)]
struct NamingRuntime {
    server: CorbaServer,
    endpoint: ::std::sync::Arc<Mutex<(String, u16)>>,
    next_key: ::std::sync::Arc<AtomicU64>,
    // Context identity (Arc pointer) → published IOR. Guarantees that the same
    // model node (whether reached via new_context, bind_new_context or resolve)
    // always yields the SAME object reference.
    published: ::std::sync::Arc<Mutex<HashMap<usize, ObjectReference>>>,
}

impl NamingRuntime {
    fn new(server: CorbaServer, host: String) -> Self {
        Self {
            server,
            endpoint: ::std::sync::Arc::new(Mutex::new((host, 0))),
            next_key: ::std::sync::Arc::new(AtomicU64::new(0)),
            published: ::std::sync::Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Publishes a (sub-)context as a standalone CORBA object and returns its
    /// IOR. Idempotent via the Arc identity.
    fn publish(
        &self,
        ctx: &::std::sync::Arc<zerodds_corba_cosnaming::NamingContext>,
    ) -> ObjectReference {
        let id = ::std::sync::Arc::as_ptr(ctx) as usize;
        let mut map = self.published.lock().unwrap();
        if let Some(existing) = map.get(&id) {
            return existing.clone();
        }
        let n = self.next_key.fetch_add(1, Ordering::Relaxed);
        let key = format!("NameService/ctx/{n}").into_bytes();
        let servant = ::std::sync::Arc::new(NamingContextServant {
            root: ::std::sync::Arc::clone(ctx),
            rt: self.clone(),
        });
        self.server.register(&key, move |op, body, e| {
            dispatch_namingcontext(&*servant, op, body, e)
        });
        let (host, port) = self.endpoint.lock().unwrap().clone();
        let objref = object_reference(NAMINGCONTEXT_TYPE_ID, &host, port, &key);
        map.insert(id, objref.clone());
        objref
    }
}

/// Cross-ORB NamingContext servant: delegates to the in-memory CosNaming model.
struct NamingContextServant {
    root: ::std::sync::Arc<zerodds_corba_cosnaming::NamingContext>,
    rt: NamingRuntime,
}

fn cosname_to_model(
    n: &corba_gen::CosNaming::CosName,
) -> Vec<zerodds_corba_cosnaming::NameComponent> {
    n.iter()
        .map(|c| zerodds_corba_cosnaming::NameComponent::with_kind(c.id.clone(), c.kind.clone()))
        .collect()
}

fn sys_exc(message: &'static str) -> NamingContextError {
    NamingContextError::System(CorbaException::SystemException { minor: 0, message })
}

fn map_naming_err(
    e: zerodds_corba_cosnaming::NamingError,
    name: &corba_gen::CosNaming::CosName,
) -> NamingContextError {
    use zerodds_corba_cosnaming::{NamingError, NotFoundReason as M};
    match e {
        NamingError::NotFound { why, .. } => {
            let w = match why {
                M::MissingNode => corba_gen::CosNaming::NotFoundReason::missing_node,
                M::NotContext => corba_gen::CosNaming::NotFoundReason::not_context,
                M::NotObject => corba_gen::CosNaming::NotFoundReason::not_object,
            };
            NamingContextError::NotFound(NotFound {
                why: w,
                rest_of_name: name.clone(),
            })
        }
        NamingError::InvalidName => {
            NamingContextError::InvalidName(corba_gen::CosNaming::InvalidName {})
        }
        NamingError::AlreadyBound => {
            NamingContextError::AlreadyBound(corba_gen::CosNaming::AlreadyBound {})
        }
        NamingError::NotEmpty => NamingContextError::NotEmpty(corba_gen::CosNaming::NotEmpty {}),
        NamingError::CannotProceed { .. } => sys_exc("CORBA CANNOT_PROCEED"),
    }
}

impl NamingContext for NamingContextServant {
    fn bind(
        &self,
        n: corba_gen::CosNaming::CosName,
        obj: zerodds_corba_rust::ObjectReference,
    ) -> Result<(), NamingContextError> {
        let s = stringify_object_reference(&obj).map_err(|_| sys_exc("CORBA MARSHAL"))?;
        self.root
            .bind(
                &cosname_to_model(&n),
                zerodds_corba_cosnaming::ObjectRef::from_stringified(s),
            )
            .map_err(|e| map_naming_err(e, &n))
    }
    fn rebind(
        &self,
        n: corba_gen::CosNaming::CosName,
        obj: zerodds_corba_rust::ObjectReference,
    ) -> Result<(), NamingContextError> {
        let s = stringify_object_reference(&obj).map_err(|_| sys_exc("CORBA MARSHAL"))?;
        self.root
            .rebind(
                &cosname_to_model(&n),
                zerodds_corba_cosnaming::ObjectRef::from_stringified(s),
            )
            .map_err(|e| map_naming_err(e, &n))
    }
    fn resolve(
        &self,
        n: corba_gen::CosNaming::CosName,
    ) -> Result<zerodds_corba_rust::ObjectReference, NamingContextError> {
        match self
            .root
            .resolve(&cosname_to_model(&n))
            .map_err(|e| map_naming_err(e, &n))?
        {
            zerodds_corba_cosnaming::context::ResolveResult::Object(o) => {
                object_reference_from_ior(&o.stringified).map_err(|_| sys_exc("CORBA MARSHAL"))
            }
            zerodds_corba_cosnaming::context::ResolveResult::Context(ctx) => {
                // Sub-context → published NamingContext object reference
                // (federation: the caller narrows it back to NamingContext).
                Ok(self.rt.publish(&ctx))
            }
        }
    }
    fn unbind(&self, n: corba_gen::CosNaming::CosName) -> Result<(), NamingContextError> {
        self.root
            .unbind(&cosname_to_model(&n))
            .map_err(|e| map_naming_err(e, &n))
    }
    fn new_context(&self) -> Result<zerodds_corba_rust::ObjectReference, CorbaException> {
        // No raises → error type is CorbaException (not NamingContextError).
        // Creates an empty, UNbound sub-context object (spec §2.5.4.7).
        let ctx = self.root.new_context();
        Ok(self.rt.publish(&ctx))
    }
    fn bind_new_context(
        &self,
        n: corba_gen::CosNaming::CosName,
    ) -> Result<zerodds_corba_rust::ObjectReference, NamingContextError> {
        // Spec §2.5.4.8: create an empty sub-context AND bind it under `n`.
        let name = cosname_to_model(&n);
        let ctx = self
            .root
            .bind_new_context(&name)
            .map_err(|e| map_naming_err(e, &n))?;
        Ok(self.rt.publish(&ctx))
    }
    fn destroy(&self) -> Result<(), NamingContextError> {
        self.root
            .destroy()
            .map_err(|e| map_naming_err(e, &Vec::new()))
    }
}

struct EchoImpl;
impl Echo for EchoImpl {
    fn ping(&self, msg: String) -> Result<String, CorbaException> {
        Ok(msg)
    }
}

/// Cross-ORB Bench servant (identical semantics to the omniORB servant).
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
        // Echo: decodes the (foreign-ORB) TypeCode+value and re-encodes it.
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
    fn echo_ref(
        &self,
        o: zerodds_corba_rust::ObjectReference,
    ) -> Result<zerodds_corba_rust::ObjectReference, CorbaException> {
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

/// Feature servant: covers the essential CORBA data types + param modes.
struct FeaturesImpl {
    label: std::sync::Mutex<String>,
}
impl Features for FeaturesImpl {
    fn add(&self, a: i32, b: i32) -> Result<i32, CorbaException> {
        Ok(a.wrapping_add(b))
    }
    fn scale(&self, x: f64, factor: f64) -> Result<f64, CorbaException> {
        Ok(x * factor)
    }
    fn toggle(&self, b: bool) -> Result<bool, CorbaException> {
        Ok(!b)
    }
    fn xor_byte(&self, a: u8, b: u8) -> Result<u8, CorbaException> {
        Ok(a ^ b)
    }
    fn neg_short(&self, s: i16) -> Result<i16, CorbaException> {
        Ok(-s)
    }
    fn uadd(&self, a: u32, b: u32) -> Result<u32, CorbaException> {
        Ok(a.wrapping_add(b))
    }
    fn add64(&self, a: i64, b: i64) -> Result<i64, CorbaException> {
        Ok(a.wrapping_add(b))
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
    fn echo_seq(&self, xs: Vec<String>) -> Result<Vec<String>, CorbaException> {
        Ok(xs)
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
    fn swap(&self, a: &mut i32, b: &mut i32) -> Result<(), CorbaException> {
        std::mem::swap(a, b);
        Ok(())
    }
    fn fire(&self, _signal: i32) -> Result<(), CorbaException> {
        Ok(())
    }
    fn answer(&self) -> Result<i32, CorbaException> {
        Ok(42)
    }
    fn label(&self) -> Result<String, CorbaException> {
        Ok(self.label.lock().unwrap().clone())
    }
    fn set_label(&self, value: String) -> Result<(), CorbaException> {
        *self.label.lock().unwrap() = value;
        Ok(())
    }
}

fn main() {
    let host = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1".to_string());
    let port: u16 = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    let server = CorbaServer::new();
    let echo = Arc::new(EchoImpl);
    server.register(b"Echo", move |op, body, e| {
        dispatch_echo(&*echo, op, body, e)
    });
    let feats = Arc::new(FeaturesImpl {
        label: std::sync::Mutex::new("init".to_string()),
    });
    server.register(b"Features", move |op, body, e| {
        dispatch_features(&*feats, op, body, e)
    });
    let bench = Arc::new(BenchImpl);
    server.register(b"Bench", move |op, body, e| {
        dispatch_bench(&*bench, op, body, e)
    });
    let naming_rt = NamingRuntime::new(server.clone(), host.clone());
    let naming = Arc::new(NamingContextServant {
        root: Arc::new(zerodds_corba_cosnaming::NamingContext::new()),
        rt: naming_rt.clone(),
    });
    server.register(b"NameService", move |op, body, e| {
        dispatch_namingcontext(&*naming, op, body, e)
    });

    let acceptor = server
        .serve(format!("{host}:{port}").parse().unwrap())
        .unwrap();
    let addr = acceptor.listen_addr();
    let h = addr.ip().to_string();
    let p = addr.port();
    // Mirror the real (ephemeral) endpoint into the naming runtime so that
    // sub-context IORs published at runtime carry the correct host/port.
    *naming_rt.endpoint.lock().unwrap() = (h.clone(), p);

    println!(
        "ECHO_IOR={}",
        stringify_object_ref("IDL:Echo:1.0", &h, p, b"Echo")
    );
    println!(
        "FEATURES_IOR={}",
        stringify_object_ref("IDL:Features:1.0", &h, p, b"Features")
    );
    println!(
        "BENCH_IOR={}",
        stringify_object_ref("IDL:Bench:1.0", &h, p, b"Bench")
    );
    // type_id = real OMG RepositoryId, so foreign ORBs (omniORB/TAO/JacORB)
    // can narrow the IOR to CosNaming::NamingContext.
    println!(
        "NAMING_IOR={}",
        stringify_object_ref(
            "IDL:omg.org/CosNaming/NamingContext:1.0",
            &h,
            p,
            b"NameService"
        )
    );
    println!("LISTENING={h}:{p}");
    use std::io::Write;
    std::io::stdout().flush().unwrap();

    // Block until killed.
    loop {
        std::thread::sleep(std::time::Duration::from_secs(3600));
    }
}
