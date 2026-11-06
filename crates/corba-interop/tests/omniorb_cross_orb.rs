// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr
)]
//! ZeroDDS against omniORB 4.3, both directions, each side generated from
//! the same IDL by its own compiler (`competitors/interop.idl`,
//! `competitors/features.idl`; omniidl for omniORB, `build.rs` for ZeroDDS):
//! omniORB's servants and clients are the oracle of the GIOP/IIOP requests
//! and replies ZeroDDS writes and reads. The cells of the former
//! `competitors/run_interop.sh` for omniORB, the AMI client against a
//! foreign server, and the operations of `Features` and `Shapes`, which the
//! generated stub and skeleton had been checked with only against each
//! other.
//!
//! The omniORB programs come from `competitors/build_peers.sh omniorb`
//! (`ZERODDS_CORBA_PEERS`); the job `integration-bridges-vendor-corba` runs
//! the tests (`#[ignore]` in the crate tier, which has no omniORB).

use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use zerodds_corba_interop::runtime::{
    AmiClient, CorbaServer, IiopCorbaConnection, object_reference, object_reference_from_ior,
};
use zerodds_corba_rust::{CorbaConnection, CorbaException};
use zerodds_wire::{Endianness, Reader, Writer};

#[path = "support/peers.rs"]
mod peers;

include!(concat!(env!("OUT_DIR"), "/corba_gen.rs"));
use corba_gen::{
    Color, Features, FeaturesStub, OutOfRange, Point, Shapes, ShapesError, ShapesStub, Variant,
    dispatch_features, dispatch_shapes,
};

fn omniorb_server(name: &str) -> peers::Server {
    peers::Server::start(Command::new(peers::peer(name)))
}

fn omniorb_client(name: &str, args: &[&str]) -> String {
    let mut command = Command::new(peers::peer(name));
    command.args(args);
    let output = peers::run_client(command);
    assert!(output.contains("ALL OK"), "{name}:\n{output}");
    output
}

#[test]
#[ignore = "needs omniORB 4.3 (integration-bridges-vendor-corba)"]
fn corbarust_13_zerodds_calls_echo_and_bench_on_an_omniorb_server() {
    let mut server = omniorb_server("omniorb_interop_server");
    let echo = server.ior("ECHO_IOR=");
    let bench = server.ior("BENCH_IOR=");
    peers::zerodds_client("echo", &echo);
    peers::zerodds_client("bench", &bench);
}

#[test]
#[ignore = "needs omniORB 4.3 (integration-bridges-vendor-corba)"]
fn corbarust_13_an_omniorb_client_calls_echo_and_bench_on_a_zerodds_server() {
    let mut server = peers::zerodds_server();
    let echo = server.ior("ECHO_IOR=");
    let bench = server.ior("BENCH_IOR=");
    omniorb_client("omniorb_interop_client", &["echo", &echo]);
    omniorb_client("omniorb_interop_client", &["bench", &bench]);
}

#[test]
#[ignore = "needs omniORB 4.3 (integration-bridges-vendor-corba)"]
fn corbarust_13_an_omniorb_client_drives_a_zerodds_naming_context() {
    let mut server = peers::zerodds_server();
    let naming = server.ior("NAMING_IOR=");
    omniorb_client("omniorb_naming_client", &[&naming]);
}

#[test]
#[ignore = "needs omniORB 4.3 (integration-bridges-vendor-corba)"]
fn corbarust_13_zerodds_binds_and_resolves_in_omninames() {
    let data = peers::scratch("omninames");
    let port = peers::free_port().to_string();
    // omniNames writes its log, the root context's IOR in it, to stderr.
    let mut command = Command::new("sh");
    command
        .args([
            "-c",
            "exec omniNames \"$@\" 2>&1",
            "omniNames",
            "-start",
            &port,
            "-datadir",
        ])
        .arg(&data)
        .args(["-ORBendPoint", &format!("giop:tcp:127.0.0.1:{port}")]);
    let mut names = peers::Server::start(command);
    let root = names.value("Root context is ");
    peers::zerodds_client("naming", &root);
}

/// The PEM pair of the SSLIOP cells: a self-signed leaf (CA:FALSE, which
/// rustls insists on for an end entity; SAN localhost for the name the
/// client checks), and the same certificate and key in one file, which is
/// how omniORB takes its own identity.
fn tls_material() -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
    let dir = peers::scratch("ssliop");
    let (cert, key, keycert) = (
        dir.join("cert.pem"),
        dir.join("key.pem"),
        dir.join("keycert.pem"),
    );
    let status = Command::new("openssl")
        .args(["req", "-x509", "-newkey", "rsa:2048", "-keyout"])
        .arg(&key)
        .arg("-out")
        .arg(&cert)
        .args([
            "-days",
            "1",
            "-nodes",
            "-subj",
            "/CN=localhost",
            "-addext",
            "subjectAltName=DNS:localhost",
            "-addext",
            "basicConstraints=critical,CA:FALSE",
            "-addext",
            "keyUsage=critical,digitalSignature",
            "-addext",
            "extendedKeyUsage=serverAuth",
        ])
        .output()
        .expect("openssl");
    assert!(
        status.status.success(),
        "openssl: {}",
        String::from_utf8_lossy(&status.stderr)
    );
    let both = [std::fs::read(&cert).unwrap(), std::fs::read(&key).unwrap()].concat();
    std::fs::write(&keycert, both).unwrap();
    (cert, key, keycert)
}

#[test]
#[ignore = "needs omniORB 4.3 with its SSL transport (integration-bridges-vendor-corba)"]
fn corbarust_13_an_omniorb_client_calls_echo_over_ssliop_on_a_zerodds_server() {
    let (cert, key, keycert) = tls_material();
    let mut command = Command::new(env!("CARGO_BIN_EXE_ssliop_server"));
    command.arg(&cert).arg(&key);
    let mut server = peers::Server::start(command);
    let ior = server.ior("SSLIOP_IOR=");
    let mut client = Command::new(peers::peer("omniorb_ssliop_client"));
    client.arg(&ior).arg(&cert).arg(&keycert);
    peers::run_client(client);
}

#[test]
#[ignore = "needs omniORB 4.3 with its SSL transport (integration-bridges-vendor-corba)"]
fn corbarust_13_zerodds_calls_echo_over_ssliop_on_an_omniorb_server() {
    let (cert, _, keycert) = tls_material();
    let mut command = Command::new(peers::peer("omniorb_ssliop_server"));
    command.arg(&cert).arg(&keycert);
    let mut server = peers::Server::start(command);
    let ior = server.ior("SSLIOP_IOR=");
    let mut client = Command::new(env!("CARGO_BIN_EXE_ssliop_client"));
    client.arg(&ior).arg(&cert);
    peers::run_client(client);
}

fn two_longs(a: i32, b: i32) -> Vec<u8> {
    let mut w = Writer::new(Endianness::Big);
    w.write_i32(a);
    w.write_i32(b);
    w.finish()
}

/// CORBA Messaging §22: the AMI client is client-side only, so the server
/// sees ordinary GIOP requests; omniORB answering them, and the replies
/// finding their request ids, is the cell.
#[test]
#[ignore = "needs omniORB 4.3 (integration-bridges-vendor-corba)"]
fn corbarust_5_the_ami_client_calls_an_omniorb_server_by_callback_and_by_polling() {
    let mut server = omniorb_server("omniorb_interop_server");
    let ior = server.ior("BENCH_IOR=");
    let oref = object_reference_from_ior(&ior).expect("parse IOR");
    let mut client = AmiClient::connect(&oref).expect("connect to omniORB's Bench");

    let got = Arc::new(Mutex::new(None));
    let sink = Arc::clone(&got);
    client
        .send(
            "add",
            &two_longs(7, 5),
            Box::new(move |reply| {
                let (body, e) = reply.expect("omniORB's add reply");
                *sink.lock().unwrap() = Some(Reader::new(&body, e).read_i32().unwrap());
            }),
        )
        .unwrap();
    client.perform_all().unwrap();
    assert_eq!(*got.lock().unwrap(), Some(12), "add(7, 5) by callback");

    let id = client.send_poll("divmod", &two_longs(17, 5)).unwrap();
    let (body, e) = client
        .get_reply(id)
        .unwrap()
        .expect("omniORB's divmod reply");
    let mut r = Reader::new(&body, e);
    assert_eq!(
        (r.read_i32().unwrap(), r.read_i32().unwrap()),
        (3, 2),
        "divmod(17, 5) by polling"
    );
}

#[test]
#[ignore = "needs omniORB 4.3 (integration-bridges-vendor-corba)"]
fn corbarust_13_the_generated_stubs_call_every_operation_of_features_and_shapes_on_an_omniorb_server()
 {
    let mut server = omniorb_server("omniorb_features_server");
    let conn: Arc<dyn CorbaConnection + Send + Sync> = Arc::new(IiopCorbaConnection::new());
    let f = FeaturesStub::new(
        object_reference_from_ior(&server.ior("FEATURES_IOR=")).unwrap(),
        Arc::clone(&conn),
    );
    let s = ShapesStub::new(
        object_reference_from_ior(&server.ior("SHAPES_IOR=")).unwrap(),
        conn,
    );

    assert_eq!(f.add(2, 3).unwrap(), 5);
    assert_eq!(f.scale(2.5, 4.0).unwrap(), 10.0);
    assert!(!f.toggle(true).unwrap());
    assert_eq!(f.xor_byte(0xF0, 0x0F).unwrap(), 0xFF);
    assert_eq!(f.neg_short(-7).unwrap(), 7);
    assert_eq!(f.uadd(10, 20).unwrap(), 30);
    assert_eq!(f.add64(1_000_000_000_000, 1).unwrap(), 1_000_000_000_001);
    assert_eq!(f.concat("foo".into(), "bar".into()).unwrap(), "foobar");
    let ws = zerodds_corba_rt::WString::from("wíde-€-Ω");
    assert_eq!(f.wecho(ws.clone()).unwrap(), ws);
    for value in [
        zerodds_corba_rt::AnyValue::Long(-99),
        zerodds_corba_rt::AnyValue::Double(2.5),
        zerodds_corba_rt::AnyValue::Str("any-string".to_string()),
        zerodds_corba_rt::AnyValue::Boolean(true),
    ] {
        let any = zerodds_corba_rt::CorbaAny(value);
        assert_eq!(f.aecho(any.clone()).unwrap(), any);
    }
    assert_eq!(f.reverse(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    assert_eq!(
        f.echo_seq(vec!["a".into(), "b".into()]).unwrap(),
        vec!["a".to_string(), "b".to_string()]
    );
    let (mut q, mut r) = (0, 0);
    f.divmod(17, 5, &mut q, &mut r).unwrap();
    assert_eq!((q, r), (3, 2));
    let mut x = 41;
    f.increment(&mut x).unwrap();
    assert_eq!(x, 42);
    let (mut a, mut b) = (1, 2);
    f.swap(&mut a, &mut b).unwrap();
    assert_eq!((a, b), (2, 1));
    f.fire(99).unwrap();
    assert_eq!(f.answer().unwrap(), 42);
    assert_eq!(f.label().unwrap(), "init");
    f.set_label("changed".into()).unwrap();
    assert_eq!(f.label().unwrap(), "changed");
    // The oneway, as omniORB's servant received it.
    server.wait_for_line("FIRED 99");

    let t = s.translate(Point { x: 1, y: 2 }, 10, 20).unwrap();
    assert_eq!((t.x, t.y), (11, 22));
    assert_eq!(s.sum_point(Point { x: 3, y: 4 }).unwrap(), 7);
    let (mut ox, mut oy) = (0, 0);
    s.split(Point { x: 5, y: 6 }, &mut ox, &mut oy).unwrap();
    assert_eq!((ox, oy), (5, 6));
    let mut p = Point { x: 7, y: 8 };
    s.bump(&mut p).unwrap();
    assert_eq!((p.x, p.y), (8, 9));
    assert_eq!(s.next_color(Color::RED).unwrap(), Color::GREEN);
    assert_eq!(s.next_color(Color::BLUE).unwrap(), Color::RED);
    match s.wrap_long(123).unwrap() {
        Variant::As_long(n) => assert_eq!(n, 123),
        other => panic!("omniORB answered {other:?}"),
    }
    assert_eq!(s.unwrap_long(Variant::As_long(99)).unwrap(), 99);
    assert_eq!(s.unwrap_long(Variant::As_string("abcd".into())).unwrap(), 4);
    // A discriminator no label takes selects the `default:` member.
    assert_eq!(s.unwrap_long(Variant::As_flag(7, true)).unwrap(), 1);
    assert_eq!(s.checked(3, 10).unwrap(), 3);
    match s.checked(15, 10) {
        Err(ShapesError::OutOfRange(e)) => assert_eq!(
            e,
            OutOfRange {
                requested: 15,
                limit: 10
            }
        ),
        other => panic!("expected OutOfRange, omniORB answered {other:?}"),
    }
}

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
    fn wecho(
        &self,
        w: zerodds_corba_rt::WString,
    ) -> Result<zerodds_corba_rt::WString, CorbaException> {
        Ok(w)
    }
    fn aecho(
        &self,
        a: zerodds_corba_rt::CorbaAny,
    ) -> Result<zerodds_corba_rt::CorbaAny, CorbaException> {
        Ok(a)
    }
    fn reverse(&self, xs: Vec<i32>) -> Result<Vec<i32>, CorbaException> {
        Ok(xs.into_iter().rev().collect())
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
        Ok(match v {
            Variant::As_long(n) => n,
            Variant::As_string(s) => s.len() as i32,
            Variant::As_flag(_, b) => i32::from(b),
        })
    }
    fn checked(&self, idx: i32, limit: i32) -> Result<i32, ShapesError> {
        if idx < limit {
            Ok(idx)
        } else {
            Err(ShapesError::OutOfRange(OutOfRange {
                requested: idx,
                limit,
            }))
        }
    }
}

#[test]
#[ignore = "needs omniORB 4.3 (integration-bridges-vendor-corba)"]
fn corbarust_13_an_omniorb_client_calls_every_operation_of_the_generated_features_and_shapes_skeletons()
 {
    let server = CorbaServer::new();
    let features = Arc::new(FeaturesImpl {
        label: Mutex::new("init".to_string()),
        fired: Mutex::new(Vec::new()),
    });
    let servant = Arc::clone(&features);
    server.register(b"Features", move |op, body, e| {
        dispatch_features(&*servant, op, body, e)
    });
    let shapes = Arc::new(ShapesImpl);
    server.register(b"Shapes", move |op, body, e| {
        dispatch_shapes(&*shapes, op, body, e)
    });
    let acceptor = server.serve("127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = acceptor.listen_addr();
    let host = addr.ip().to_string();
    let f = zerodds_corba_interop::runtime::stringify_object_reference(&object_reference(
        "IDL:Features:1.0",
        &host,
        addr.port(),
        b"Features",
    ))
    .unwrap();
    let s = zerodds_corba_interop::runtime::stringify_object_reference(&object_reference(
        "IDL:Shapes:1.0",
        &host,
        addr.port(),
        b"Shapes",
    ))
    .unwrap();
    omniorb_client("omniorb_features_client", &[&f, &s]);
    // omniORB's oneway `fire(99)`, as the generated skeleton delivered it.
    let deadline = Instant::now() + Duration::from_secs(5);
    while !features.fired.lock().unwrap().contains(&99) {
        assert!(
            Instant::now() < deadline,
            "the oneway fire(99) did not arrive"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    acceptor.shutdown();
}
