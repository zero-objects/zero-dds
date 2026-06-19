// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! End-to-end: generated CORBA stub → IIOP runtime → generated skeleton.
//!
//! Proves that the codegen (`generate_corba_rust_module`) produces real
//! GIOP/CDR marshalling — not just that it compiles, but that it roundtrips over
//! a real TCP loopback connection.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use zerodds_corba_interop::runtime::{
    CorbaServer, IiopCorbaConnection, object_reference, object_reference_from_ior,
    stringify_object_reference,
};
use zerodds_corba_rust::{CorbaConnection, CorbaException};

// The build.rs-generated module (Echo + Features: trait/stub/dispatch).
include!(concat!(env!("OUT_DIR"), "/corba_gen.rs"));
use corba_gen::CosNaming::{
    NameComponent, NamingContext, NamingContextError, NamingContextStub, NotFound,
    dispatch_namingcontext,
};
use corba_gen::{
    Color, Echo, EchoStub, Features, FeaturesStub, OutOfRange, Point, Shapes, ShapesError,
    ShapesStub, Variant, dispatch_echo, dispatch_features, dispatch_shapes,
};

/// Server implementation of the generated `Echo` trait.
struct EchoImpl;
impl Echo for EchoImpl {
    fn ping(&self, msg: String) -> Result<String, CorbaException> {
        Ok(msg)
    }
}

#[test]
fn echo_roundtrip_via_generated_stub_and_skeleton() {
    let key: &[u8] = b"Echo";

    // Server: registry + generated skeleton, IIOP accept loop.
    let server = CorbaServer::new();
    let servant = Arc::new(EchoImpl);
    server.register(key, move |op, body, e| {
        dispatch_echo(&*servant, op, body, e)
    });
    let acceptor = server.serve("127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = acceptor.listen_addr();

    // Client: generated stub over IiopCorbaConnection.
    let conn: Arc<dyn CorbaConnection + Send + Sync> = Arc::new(IiopCorbaConnection::new());
    let ior = object_reference("IDL:Echo:1.0", &addr.ip().to_string(), addr.port(), key);
    let stub = EchoStub::new(ior, conn);

    assert_eq!(stub.ping("hello".to_string()).unwrap(), "hello");
    assert_eq!(stub.ping(String::new()).unwrap(), "");
    let big = "x".repeat(1024);
    assert_eq!(stub.ping(big.clone()).unwrap(), big);

    acceptor.shutdown();
}

use std::sync::Mutex;

/// Servant for the full feature matrix (interior mutability for the rw attribute + oneway).
struct FeaturesImpl {
    label: Mutex<String>,
    fired: Mutex<Vec<i32>>,
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
    fn divmod(
        &self,
        a: i32,
        b: i32,
        quotient: &mut i32,
        remainder: &mut i32,
    ) -> Result<(), CorbaException> {
        *quotient = a / b;
        *remainder = a % b;
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
    fn fire(&self, signal: i32) -> Result<(), CorbaException> {
        self.fired.lock().unwrap().push(signal);
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

#[test]
fn feature_matrix_roundtrip_via_codegen() {
    let key: &[u8] = b"Features";
    let server = CorbaServer::new();
    let servant = Arc::new(FeaturesImpl {
        label: Mutex::new("init".to_string()),
        fired: Mutex::new(Vec::new()),
    });
    let s2 = Arc::clone(&servant);
    server.register(key, move |op, body, e| dispatch_features(&*s2, op, body, e));
    let acceptor = server.serve("127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = acceptor.listen_addr();

    let conn: Arc<dyn CorbaConnection + Send + Sync> = Arc::new(IiopCorbaConnection::new());
    let ior = object_reference("IDL:Features:1.0", &addr.ip().to_string(), addr.port(), key);
    let f = FeaturesStub::new(ior, conn);

    // Primitive
    assert_eq!(f.add(2, 3).unwrap(), 5);
    assert_eq!(f.scale(2.5, 4.0).unwrap(), 10.0);
    assert!(!f.toggle(true).unwrap());
    assert_eq!(f.xor_byte(0xF0, 0x0F).unwrap(), 0xFF);
    assert_eq!(f.neg_short(-7).unwrap(), 7);
    assert_eq!(f.uadd(10, 20).unwrap(), 30);
    assert_eq!(f.add64(1_000_000_000_000, 1).unwrap(), 1_000_000_000_001);
    // String + Sequence
    assert_eq!(
        f.concat("foo".to_string(), "bar".to_string()).unwrap(),
        "foobar"
    );
    // wstring (UTF-16 wire, distinct from string): Unicode roundtrip over IIOP.
    let ws = zerodds_cdr::WString::from("wide-wörld-€-🌍");
    assert_eq!(f.wecho(ws.clone()).unwrap(), ws);
    // any (TypeCode + Value): roundtrip multiple variants over IIOP.
    for av in [
        zerodds_cdr::AnyValue::Long(-99),
        zerodds_cdr::AnyValue::Double(2.5),
        zerodds_cdr::AnyValue::Str("any-string".to_string()),
        zerodds_cdr::AnyValue::Boolean(true),
    ] {
        let any = zerodds_cdr::CorbaAny(av);
        assert_eq!(f.aecho(any.clone()).unwrap(), any);
    }
    assert_eq!(f.reverse(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    assert_eq!(
        f.echo_seq(vec!["a".to_string(), "b".to_string()]).unwrap(),
        vec!["a".to_string(), "b".to_string()]
    );
    // out
    let mut q = 0;
    let mut r = 0;
    f.divmod(17, 5, &mut q, &mut r).unwrap();
    assert_eq!((q, r), (3, 2));
    // inout
    let mut x = 41;
    f.increment(&mut x).unwrap();
    assert_eq!(x, 42);
    let mut a = 1;
    let mut b = 2;
    f.swap(&mut a, &mut b).unwrap();
    assert_eq!((a, b), (2, 1));
    // oneway
    f.fire(99).unwrap();
    // Attribute
    assert_eq!(f.answer().unwrap(), 42);
    assert_eq!(f.label().unwrap(), "init");
    f.set_label("changed".to_string()).unwrap();
    assert_eq!(f.label().unwrap(), "changed");
    // oneway is fire-and-forget (SYNC_NONE): server processing runs
    // asynchronously. Observe deterministically via a bounded poll (no fixed
    // sleep that goes flaky under load) — waits as long as needed, fails
    // only if the event NEVER arrives.
    let mut delivered = false;
    for _ in 0..200 {
        if servant.fired.lock().unwrap().contains(&99) {
            delivered = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(delivered, "oneway fire(99) was not delivered");

    acceptor.shutdown();
}

/// Servant for the constructed types (struct/enum/union/exception).
struct ShapesImpl;
impl Shapes for ShapesImpl {
    fn translate(&self, p: Point, dx: i32, dy: i32) -> Result<Point, CorbaException> {
        Ok(Point {
            x: p.x + dx,
            y: p.y + dy,
        })
    }
    fn sum_point(&self, p: Point) -> Result<i32, CorbaException> {
        Ok(p.x + p.y)
    }
    fn split(&self, p: Point, x: &mut i32, y: &mut i32) -> Result<(), CorbaException> {
        *x = p.x;
        *y = p.y;
        Ok(())
    }
    fn bump(&self, p: &mut Point) -> Result<(), CorbaException> {
        p.x += 1;
        p.y += 1;
        Ok(())
    }
    fn next_color(&self, c: Color) -> Result<Color, CorbaException> {
        Ok(match c {
            Color::RED => Color::GREEN,
            Color::GREEN => Color::BLUE,
            Color::BLUE => Color::RED,
        })
    }
    fn wrap_long(&self, v: i32) -> Result<Variant, CorbaException> {
        Ok(Variant::As_long(v))
    }
    fn unwrap_long(&self, v: Variant) -> Result<i32, CorbaException> {
        match v {
            Variant::As_long(n) => Ok(n),
            Variant::As_string(s) => Ok(s.len() as i32),
            Variant::As_flag(b) => Ok(i32::from(b)),
        }
    }
    fn checked(&self, idx: i32, limit: i32) -> Result<i32, ShapesError> {
        if idx < limit {
            Ok(idx)
        } else {
            // Typed user exception via the codegen — no hand-marshalling.
            Err(ShapesError::OutOfRange(OutOfRange {
                requested: idx,
                limit,
            }))
        }
    }
}

#[test]
fn shapes_matrix_roundtrip_via_codegen() {
    let key: &[u8] = b"Shapes";
    let server = CorbaServer::new();
    let servant = Arc::new(ShapesImpl);
    server.register(key, move |op, body, e| {
        dispatch_shapes(&*servant, op, body, e)
    });
    let acceptor = server.serve("127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = acceptor.listen_addr();

    let conn: Arc<dyn CorbaConnection + Send + Sync> = Arc::new(IiopCorbaConnection::new());
    let ior = object_reference("IDL:Shapes:1.0", &addr.ip().to_string(), addr.port(), key);
    let s = ShapesStub::new(ior, conn);

    // struct in + return
    let t = s.translate(Point { x: 1, y: 2 }, 10, 20).unwrap();
    assert_eq!((t.x, t.y), (11, 22));
    assert_eq!(s.sum_point(Point { x: 3, y: 4 }).unwrap(), 7);
    // struct out
    let mut ox = 0;
    let mut oy = 0;
    s.split(Point { x: 5, y: 6 }, &mut ox, &mut oy).unwrap();
    assert_eq!((ox, oy), (5, 6));
    // struct inout
    let mut p = Point { x: 7, y: 8 };
    s.bump(&mut p).unwrap();
    assert_eq!((p.x, p.y), (8, 9));
    // enum
    assert_eq!(s.next_color(Color::RED).unwrap(), Color::GREEN);
    assert_eq!(s.next_color(Color::BLUE).unwrap(), Color::RED);
    // union (both directions)
    match s.wrap_long(123).unwrap() {
        Variant::As_long(n) => assert_eq!(n, 123),
        other => panic!("unexpected Variant: {other:?}"),
    }
    assert_eq!(s.unwrap_long(Variant::As_long(99)).unwrap(), 99);
    assert_eq!(
        s.unwrap_long(Variant::As_string("abcd".to_string()))
            .unwrap(),
        4
    );
    assert_eq!(s.unwrap_long(Variant::As_flag(true)).unwrap(), 1);
    // Typed user exception via the codegen (no hand-decode):
    assert_eq!(s.checked(3, 10).unwrap(), 3);
    match s.checked(15, 10) {
        Err(ShapesError::OutOfRange(exc)) => assert_eq!(
            exc,
            OutOfRange {
                requested: 15,
                limit: 10
            }
        ),
        other => panic!("expected ShapesError::OutOfRange, got {other:?}"),
    }

    acceptor.shutdown();
}

use corba_gen::{NameError, Naming, NamingError, NamingStub, dispatch_naming};

/// Live NameService: wires the (so far in-memory spec-model-only)
/// `zerodds_corba_cosnaming::NamingContext` to the real GIOP/IIOP runtime.
/// bind/resolve carry object references as IORs; unknown names → typed
/// NameError. Composes object refs + typed exceptions + the live stack.
struct NamingImpl {
    ctx: zerodds_corba_cosnaming::NamingContext,
}

fn name_err(reason: impl core::fmt::Debug) -> NamingError {
    NamingError::NameError(NameError {
        reason: format!("{reason:?}"),
    })
}

impl Naming for NamingImpl {
    fn bind(
        &self,
        name: String,
        obj: zerodds_corba_rust::ObjectReference,
    ) -> Result<(), NamingError> {
        let ior = stringify_object_reference(&obj).map_err(NamingError::System)?;
        let nm = self.ctx.to_name(&name).map_err(name_err)?;
        self.ctx
            .bind(
                &nm,
                zerodds_corba_cosnaming::ObjectRef::from_stringified(ior),
            )
            .map_err(name_err)
    }

    fn resolve(&self, name: String) -> Result<zerodds_corba_rust::ObjectReference, NamingError> {
        match self.ctx.resolve_str(&name).map_err(name_err)? {
            zerodds_corba_cosnaming::context::ResolveResult::Object(o) => {
                object_reference_from_ior(&o.stringified).map_err(NamingError::System)
            }
            zerodds_corba_cosnaming::context::ResolveResult::Context(_) => {
                Err(name_err("resolved to a sub-context, not an object"))
            }
        }
    }
}

#[test]
fn live_nameservice_bind_resolve_via_codegen() {
    let server = CorbaServer::new();
    // The live object to register (Echo).
    let echo = Arc::new(EchoImpl);
    server.register(b"Echo", move |op, body, e| {
        dispatch_echo(&*echo, op, body, e)
    });
    // Live NameService over the in-memory NamingContext.
    let naming_servant = Arc::new(NamingImpl {
        ctx: zerodds_corba_cosnaming::NamingContext::new(),
    });
    server.register(b"Naming", move |op, body, e| {
        dispatch_naming(&*naming_servant, op, body, e)
    });
    let acceptor = server.serve("127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = acceptor.listen_addr();
    let (host, port) = (addr.ip().to_string(), addr.port());

    let naming = NamingStub::new(
        object_reference("IDL:Naming:1.0", &host, port, b"Naming"),
        Arc::new(IiopCorbaConnection::new()),
    );
    let echo_ref = object_reference("IDL:Echo:1.0", &host, port, b"Echo");

    // bind the Echo reference under "echo", then resolve.
    naming.bind("echo".to_string(), echo_ref).unwrap();
    let resolved = naming.resolve("echo".to_string()).unwrap();

    // The resolved reference must be LIVE — call Echo::ping through it.
    let echo_stub = EchoStub::new(resolved, Arc::new(IiopCorbaConnection::new()));
    assert_eq!(
        echo_stub.ping("via-naming".to_string()).unwrap(),
        "via-naming"
    );

    // Unknown name → typed NameError (raised over the wire).
    match naming.resolve("does-not-exist".to_string()) {
        Err(NamingError::NameError(_)) => {}
        other => panic!("expected NameError, got {other:?}"),
    }

    acceptor.shutdown();
}

use corba_gen::{EventBus, EventBusStub, dispatch_eventbus};

/// Live EventChannel: wires the (so far in-memory spec-model-only) CosEvent
/// `EventChannel` (pull model) to the real GIOP/IIOP runtime. Suppliers
/// push `any` events, consumers pull them (FIFO). Composes CorbaAny + the
/// cos-event add-on + the live runtime.
struct EventBusImpl {
    pull_consumer: std::sync::Arc<zerodds_corba_cos_event::channel::ProxyPullConsumer>,
    pull_supplier: std::sync::Arc<zerodds_corba_cos_event::channel::ProxyPullSupplier>,
}

impl EventBusImpl {
    fn new() -> Self {
        let ch = zerodds_corba_cos_event::EventChannel::new();
        let pc = ch.for_suppliers().obtain_pull_consumer();
        let ps = ch.for_consumers().obtain_pull_supplier();
        ps.connect_pull_consumer().unwrap();
        pc.connect_pull_supplier(ps.clone()).unwrap();
        Self {
            pull_consumer: pc,
            pull_supplier: ps,
        }
    }
}

fn evt_exc(msg: &'static str) -> CorbaException {
    CorbaException::SystemException {
        minor: 0,
        message: msg,
    }
}

impl EventBus for EventBusImpl {
    fn push(&self, event: zerodds_cdr::CorbaAny) -> Result<(), CorbaException> {
        let mut w = zerodds_cdr::BufferWriter::new(zerodds_cdr::Endianness::Big);
        zerodds_cdr::CdrEncode::encode(&event, &mut w).map_err(|_| evt_exc("encode any"))?;
        self.pull_consumer
            .forward_event(zerodds_corba_cos_event::AnyEvent::new(
                "any".to_string(),
                w.into_bytes(),
            ))
            .map_err(|_| evt_exc("event channel disconnected"))
    }

    fn try_pull(&self, event: &mut zerodds_cdr::CorbaAny) -> Result<bool, CorbaException> {
        use zerodds_corba_cos_event::comm::PullSupplier;
        match self.pull_supplier.try_pull() {
            Ok((e, true)) => {
                let mut r = zerodds_cdr::BufferReader::new(&e.data, zerodds_cdr::Endianness::Big);
                *event = <zerodds_cdr::CorbaAny as zerodds_cdr::CdrDecode>::decode(&mut r)
                    .map_err(|_| evt_exc("decode any"))?;
                Ok(true)
            }
            Ok((_, false)) => Ok(false),
            Err(_) => Err(evt_exc("event channel disconnected")),
        }
    }
}

// ===========================================================================
// Live notification (CosNotification): wires the cos-notify channel
// (StructuredEvent + StructuredProxy hierarchy) live to the GIOP/IIOP runtime.
// ===========================================================================
use corba_gen::{NotifyBus, NotifyBusStub, dispatch_notifybus};

struct NotifyBusImpl {
    push_consumer: std::sync::Arc<zerodds_corba_cos_notify::StructuredProxyPushConsumer>,
    pull_supplier: std::sync::Arc<zerodds_corba_cos_notify::StructuredProxyPullSupplier>,
}

impl NotifyBusImpl {
    fn new() -> Self {
        let ch = zerodds_corba_cos_notify::EventChannel::new();
        let pc = ch.for_suppliers().obtain_structured_push_consumer();
        let ps = ch.for_consumers().obtain_structured_pull_supplier();
        pc.connect_structured_push_supplier().unwrap();
        ps.connect_structured_pull_consumer().unwrap();
        Self {
            push_consumer: pc,
            pull_supplier: ps,
        }
    }
}

impl NotifyBus for NotifyBusImpl {
    fn push_structured(
        &self,
        domain: String,
        type_name: String,
        body: zerodds_cdr::CorbaAny,
    ) -> Result<(), CorbaException> {
        self.push_consumer
            .push_structured_event(zerodds_corba_cos_notify::StructuredEvent::new(
                domain, type_name, "", body,
            ));
        Ok(())
    }

    fn try_pull_structured(
        &self,
        domain: &mut String,
        type_name: &mut String,
        body: &mut zerodds_cdr::CorbaAny,
    ) -> Result<bool, CorbaException> {
        match self.pull_supplier.try_pull_structured_event() {
            Ok((ev, true)) => {
                *domain = ev.event_type().domain_name.clone();
                *type_name = ev.event_type().type_name.clone();
                *body = ev.remainder_of_body;
                Ok(true)
            }
            Ok((_, false)) => Ok(false),
            Err(_) => Err(evt_exc("notify channel disconnected")),
        }
    }
}

/// Live CosNotification: push StructuredEvents (domain/type + any payload) over
/// real IIOP + pull them back FIFO — the cos-notify channel wired live.
#[test]
fn live_notification_push_pull_via_codegen() {
    let server = CorbaServer::new();
    let bus = Arc::new(NotifyBusImpl::new());
    server.register(b"NotifyBus", move |op, body, e| {
        dispatch_notifybus(&*bus, op, body, e)
    });
    let acceptor = server.serve("127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = acceptor.listen_addr();
    let stub = NotifyBusStub::new(
        object_reference(
            "IDL:NotifyBus:1.0",
            &addr.ip().to_string(),
            addr.port(),
            b"NotifyBus",
        ),
        Arc::new(IiopCorbaConnection::new()),
    );

    let events = [
        ("Telecom", "CallEvent", zerodds_cdr::AnyValue::Long(42)),
        (
            "Finance",
            "Trade",
            zerodds_cdr::AnyValue::Str("buy".to_string()),
        ),
    ];
    for (d, t, v) in &events {
        stub.push_structured(
            (*d).to_string(),
            (*t).to_string(),
            zerodds_cdr::CorbaAny(v.clone()),
        )
        .unwrap();
    }
    for (d, t, v) in &events {
        let (mut gd, mut gt, mut gb) = (
            String::new(),
            String::new(),
            zerodds_cdr::CorbaAny::default(),
        );
        assert!(
            stub.try_pull_structured(&mut gd, &mut gt, &mut gb).unwrap(),
            "event available"
        );
        assert_eq!(
            (gd.as_str(), gt.as_str(), &gb),
            (*d, *t, &zerodds_cdr::CorbaAny(v.clone()))
        );
    }
    let (mut d, mut t, mut b) = (
        String::new(),
        String::new(),
        zerodds_cdr::CorbaAny::default(),
    );
    assert!(
        !stub.try_pull_structured(&mut d, &mut t, &mut b).unwrap(),
        "empty queue → false"
    );

    acceptor.shutdown();
}

#[test]
fn live_eventchannel_push_pull_via_codegen() {
    let server = CorbaServer::new();
    let bus = Arc::new(EventBusImpl::new());
    server.register(b"EventBus", move |op, body, e| {
        dispatch_eventbus(&*bus, op, body, e)
    });
    let acceptor = server.serve("127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = acceptor.listen_addr();

    let stub = EventBusStub::new(
        object_reference(
            "IDL:EventBus:1.0",
            &addr.ip().to_string(),
            addr.port(),
            b"EventBus",
        ),
        Arc::new(IiopCorbaConnection::new()),
    );

    // Supplier pushes three any events over IIOP.
    let events = [
        zerodds_cdr::CorbaAny(zerodds_cdr::AnyValue::Long(7)),
        zerodds_cdr::CorbaAny(zerodds_cdr::AnyValue::Str("evt".to_string())),
        zerodds_cdr::CorbaAny(zerodds_cdr::AnyValue::Double(1.5)),
    ];
    for ev in &events {
        stub.push(ev.clone()).unwrap();
    }

    // Consumer pulls them back FIFO.
    for expected in &events {
        let mut got = zerodds_cdr::CorbaAny::default();
        assert!(
            stub.try_pull(&mut got).unwrap(),
            "event should be available"
        );
        assert_eq!(&got, expected);
    }
    // Empty queue → try_pull false.
    let mut empty = zerodds_cdr::CorbaAny::default();
    assert!(!stub.try_pull(&mut empty).unwrap(), "empty queue → false");

    acceptor.shutdown();
}

use corba_gen::{TypeRepo, TypeRepoStub, dispatch_typerepo};

/// Live interface repository: wires the (in-memory) corba-ir `Repository`
/// to the real GIOP runtime (contains/ids).
struct TypeRepoImpl {
    repo: zerodds_corba_ir::Repository,
}

impl TypeRepoImpl {
    fn new() -> Self {
        let mut repo = zerodds_corba_ir::Repository::new();
        for (id, name) in [("IDL:Echo:1.0", "Echo"), ("IDL:Bench:1.0", "Bench")] {
            repo.register(zerodds_corba_ir::Definition::new(
                id,
                name,
                "1.0",
                zerodds_corba_ir::DefinitionKind::Interface,
            ))
            .unwrap();
        }
        Self { repo }
    }
}

impl TypeRepo for TypeRepoImpl {
    fn contains(&self, repo_id: String) -> Result<bool, CorbaException> {
        Ok(self.repo.lookup_id(&repo_id).is_some())
    }
    fn ids(&self) -> Result<Vec<String>, CorbaException> {
        Ok(self.repo.ids())
    }
}

#[test]
fn live_interface_repository_via_codegen() {
    let server = CorbaServer::new();
    let ir = Arc::new(TypeRepoImpl::new());
    server.register(b"TypeRepo", move |op, body, e| {
        dispatch_typerepo(&*ir, op, body, e)
    });
    let acceptor = server.serve("127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = acceptor.listen_addr();

    let repo = TypeRepoStub::new(
        object_reference(
            "IDL:TypeRepo:1.0",
            &addr.ip().to_string(),
            addr.port(),
            b"TypeRepo",
        ),
        Arc::new(IiopCorbaConnection::new()),
    );

    assert!(repo.contains("IDL:Echo:1.0".to_string()).unwrap());
    assert!(repo.contains("IDL:Bench:1.0".to_string()).unwrap());
    assert!(!repo.contains("IDL:Missing:1.0".to_string()).unwrap());
    let mut ids = repo.ids().unwrap();
    ids.sort();
    assert_eq!(
        ids,
        vec!["IDL:Bench:1.0".to_string(), "IDL:Echo:1.0".to_string()]
    );

    acceptor.shutdown();
}

use corba_gen::{Calculator, CalculatorStub, dispatch_calculator};
use std::sync::atomic::{AtomicBool, Ordering};

/// Anonymous component context (CCM §8.1.7).
struct AnonContext;
impl zerodds_corba_ccm::context::ComponentContext for AnonContext {
    fn get_caller_principal(&self) -> Option<Vec<u8>> {
        None
    }
}

/// CCM component executor: toggles the shared `active` flag in the
/// container lifecycle (ccm_activate/passivate/remove, CIF §8.1.5).
struct CalcExecutor {
    active: Arc<AtomicBool>,
}
impl zerodds_corba_ccm::cif::ComponentExecutor for CalcExecutor {
    fn set_context(&mut self, _ctx: Box<dyn zerodds_corba_ccm::context::ComponentContext>) {}
    fn ccm_activate(&mut self) -> Result<(), zerodds_corba_ccm::cif::CifError> {
        self.active.store(true, Ordering::SeqCst);
        Ok(())
    }
    fn ccm_passivate(&mut self) -> Result<(), zerodds_corba_ccm::cif::CifError> {
        self.active.store(false, Ordering::SeqCst);
        Ok(())
    }
    fn ccm_remove(&mut self) -> Result<(), zerodds_corba_ccm::cif::CifError> {
        self.active.store(false, Ordering::SeqCst);
        Ok(())
    }
}

/// Provided facet `Calculator` of the component — reachable over GIOP, but only
/// when the component is active in the lifecycle (otherwise a CORBA system exception).
struct CalculatorServant {
    active: Arc<AtomicBool>,
}
impl Calculator for CalculatorServant {
    fn add(&self, a: i32, b: i32) -> Result<i32, CorbaException> {
        if self.active.load(Ordering::SeqCst) {
            Ok(a + b)
        } else {
            Err(CorbaException::SystemException {
                minor: 0,
                message: "CORBA OBJECT_NOT_EXIST: component facet not active",
            })
        }
    }
}

#[test]
fn live_ccm_component_facet_via_codegen() {
    let active = Arc::new(AtomicBool::new(false));

    // Facet servant (always registered, but lifecycle-gated).
    let server = CorbaServer::new();
    let servant = Arc::new(CalculatorServant {
        active: Arc::clone(&active),
    });
    server.register(b"Calculator", move |op, body, e| {
        dispatch_calculator(&*servant, op, body, e)
    });
    let acceptor = server.serve("127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = acceptor.listen_addr();
    let calc = CalculatorStub::new(
        object_reference(
            "IDL:Calculator:1.0",
            &addr.ip().to_string(),
            addr.port(),
            b"Calculator",
        ),
        Arc::new(IiopCorbaConnection::new()),
    );

    // CCM container: install the component + drive it through the lifecycle.
    let container = zerodds_corba_ccm::container::Container::new(
        zerodds_corba_ccm::container::ContainerType::Session,
    );
    container
        .install_component(
            "calc-1".to_string(),
            Box::new(CalcExecutor {
                active: Arc::clone(&active),
            }),
            Box::new(AnonContext),
        )
        .unwrap();

    // Configured (not yet active): the facet call must fail.
    assert!(
        calc.add(2, 3).is_err(),
        "before ccm_activate the facet must not serve"
    );

    // activate → ccm_activate sets active=true → facet live over GIOP.
    container.activate("calc-1").unwrap();
    assert_eq!(calc.add(2, 3).unwrap(), 5);
    assert_eq!(calc.add(40, 2).unwrap(), 42);

    // passivate → facet inactive again.
    container.passivate("calc-1").unwrap();
    assert!(
        calc.add(2, 3).is_err(),
        "after ccm_passivate the facet must not serve"
    );

    container.remove("calc-1").unwrap();
    acceptor.shutdown();
}

// ===========================================================================
// CosNaming: REAL OMG NamingContext wire interface (formal/2004-10-03),
// delegating to the in-memory model zerodds_corba_cosnaming::NamingContext.
// Replaces the earlier simplified `Naming{bind(string)/resolve(string)}` stub
// with the standardized `Name = sequence<NameComponent{id,kind}>` wire.
// ===========================================================================

struct NamingContextServant {
    root: Arc<zerodds_corba_cosnaming::NamingContext>,
}

fn cosname_to_model(
    n: &corba_gen::CosNaming::CosName,
) -> Vec<zerodds_corba_cosnaming::NameComponent> {
    n.iter()
        .map(|c| zerodds_corba_cosnaming::NameComponent::with_kind(c.id.clone(), c.kind.clone()))
        .collect()
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
        NamingError::CannotProceed { .. } => {
            NamingContextError::System(CorbaException::SystemException {
                minor: 0,
                message: "CORBA CANNOT_PROCEED",
            })
        }
    }
}

fn sys_exc(message: &'static str) -> NamingContextError {
    NamingContextError::System(CorbaException::SystemException { minor: 0, message })
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
            // Sub-context resolve: follow-up (live sub-context federation).
            zerodds_corba_cosnaming::context::ResolveResult::Context(_) => {
                Err(sys_exc("CORBA NO_IMPLEMENT: sub-context resolve"))
            }
        }
    }

    fn unbind(&self, n: corba_gen::CosNaming::CosName) -> Result<(), NamingContextError> {
        self.root
            .unbind(&cosname_to_model(&n))
            .map_err(|e| map_naming_err(e, &n))
    }

    // new_context/bind_new_context require live-registered sub-context
    // servants (server handle + endpoint) — follow-up.
    fn new_context(&self) -> Result<zerodds_corba_rust::ObjectReference, CorbaException> {
        // No raises → the error type is CorbaException (not NamingContextError).
        Err(CorbaException::SystemException {
            minor: 0,
            message: "CORBA NO_IMPLEMENT: new_context",
        })
    }
    fn bind_new_context(
        &self,
        _n: corba_gen::CosNaming::CosName,
    ) -> Result<zerodds_corba_rust::ObjectReference, NamingContextError> {
        Err(sys_exc("CORBA NO_IMPLEMENT: bind_new_context"))
    }
    fn destroy(&self) -> Result<(), NamingContextError> {
        self.root
            .destroy()
            .map_err(|e| map_naming_err(e, &Vec::new()))
    }
}

/// e2e over the REAL CosNaming wire: ZeroDDS client (NamingContextStub) ↔
/// ZeroDDS server (NamingContextServant) over real IIOP loopback. Covers
/// Name=sequence<NameComponent>, object-ref binding, rebind/unbind and the
/// typed NotFound exception.
#[test]
fn live_cosnaming_real_wire_bind_resolve_via_codegen() {
    let key: &[u8] = b"NameService";
    let server = CorbaServer::new();
    let servant = Arc::new(NamingContextServant {
        root: Arc::new(zerodds_corba_cosnaming::NamingContext::new()),
    });
    server.register(key, move |op, body, e| {
        dispatch_namingcontext(&*servant, op, body, e)
    });
    let acceptor = server.serve("127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = acceptor.listen_addr();

    let conn: Arc<dyn CorbaConnection + Send + Sync> = Arc::new(IiopCorbaConnection::new());
    // type_id = real OMG RepositoryId, so foreign ORBs can narrow.
    let ior = object_reference(
        "IDL:omg.org/CosNaming/NamingContext:1.0",
        &addr.ip().to_string(),
        addr.port(),
        key,
    );
    let stub = NamingContextStub::new(ior.clone(), conn);

    let name = vec![NameComponent {
        id: "greeting".to_string(),
        kind: "obj".to_string(),
    }];
    // bind → resolve roundtrip (name sequence + object ref over the real wire).
    stub.bind(name.clone(), ior.clone()).unwrap();
    let got = stub.resolve(name.clone()).unwrap();
    assert!(
        !got.iiop_profile.is_empty(),
        "resolve returned an empty ref"
    );
    // rebind overwrites without AlreadyBound.
    stub.rebind(name.clone(), ior.clone()).unwrap();
    // unbind, then resolve → typed NotFound exception over the CosNaming wire.
    stub.unbind(name.clone()).unwrap();
    match stub.resolve(name.clone()) {
        Err(NamingContextError::NotFound(_)) => {}
        other => panic!("expected NotFound after unbind, got {other:?}"),
    }
    // Multi-level name (sequence<NameComponent> with kind) binds/resolves.
    let multi = vec![
        NameComponent {
            id: "a".to_string(),
            kind: String::new(),
        },
        NameComponent {
            id: "b".to_string(),
            kind: "leaf".to_string(),
        },
    ];
    // Binding a multi-level name fails because "a" is not a context →
    // NotFound(missing_node) is the model's spec-correct response.
    match stub.bind(multi, ior.clone()) {
        Ok(()) | Err(NamingContextError::NotFound(_)) => {}
        other => panic!("unexpected: {other:?}"),
    }

    acceptor.shutdown();
}

/// Codeset negotiation (§13.10.2.5) + BOM wstring (§15.3.1.6) through the real
/// GIOP codec pipeline: a request carries the `IOP::CodeSets` service context
/// (default UTF-8/UTF-16) AND a wstring body with non-ASCII; both must
/// arrive intact in both byte orders. Proves that the chosen
/// transmission codeset survives on the wire and that the wstring BOM decodes
/// correctly regardless of endianness (so that omniORB/TAO can read it).
#[test]
fn codeset_context_and_bom_wstring_survive_giop_pipeline() {
    use zerodds_cdr::{BufferReader, BufferWriter, CdrDecode, CdrEncode, Endianness, WString};
    use zerodds_corba_giop::{
        CodeSetContext, Message, Request, ResponseFlags, ServiceContextList, TargetAddress,
        Version, code_set_ids, decode_message_ctx, encode_message,
    };

    let text = WString::from("grüße €dwig 🌍");
    for e in [Endianness::Big, Endianness::Little] {
        let mut bw = BufferWriter::new(e);
        text.encode(&mut bw).unwrap();
        let body = bw.into_bytes();

        let cs = CodeSetContext::default_pair()
            .to_service_context(e)
            .unwrap();
        let req = Message::Request(Request {
            request_id: 1,
            response_flags: ResponseFlags::SYNC_WITH_TARGET,
            target: TargetAddress::Key(vec![b'X']),
            operation: "wecho".into(),
            requesting_principal: None,
            service_context: ServiceContextList(vec![cs]),
            body,
        });

        let wire = encode_message(Version::V1_2, e, false, &req).unwrap();
        let (decoded, decoded_e, _) = decode_message_ctx(&wire).unwrap();
        let Message::Request(r) = decoded else {
            panic!("expected Request");
        };

        // 1. CodeSetContext survives + is the default pair.
        let got = CodeSetContext::from_service_context_list(&r.service_context)
            .unwrap()
            .expect("CodeSets context present");
        assert_eq!(got.char_data, code_set_ids::UTF_8, "TCSC = UTF-8 ({e:?})");
        assert_eq!(
            got.wchar_data,
            code_set_ids::UTF_16,
            "TCSW = UTF-16 ({e:?})"
        );

        // 2. wstring body (BOM-carried) decodes correctly.
        let mut br = BufferReader::new(&r.body, decoded_e);
        assert_eq!(
            WString::decode(&mut br).unwrap(),
            text,
            "wstring roundtrip ({e:?})"
        );
    }
}
