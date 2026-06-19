// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Valuetype wire (§15.3.4) e2e over real GIOP/IIOP: a valuetype is marshalled
//! by the client, sent as the request body via `invoke`, decoded by the
//! server dispatcher through the `ValueRegistry` and re-marshalled; the client
//! decodes the reply. Demonstrates:
//! * single-value roundtrip (value_tag + RepositoryId + state),
//! * **value sharing**: two aliased refs in ONE body → a single instance on
//!   server decode (indirection through the GIOP body),
//! * **inheritance state flattening**: base state first, then the derived part.

use std::rc::Rc;

use zerodds_cdr::{BufferReader, BufferWriter, CdrDecode, CdrEncode, Endianness};
use zerodds_corba_interop::runtime::{CorbaServer, IiopCorbaConnection, object_reference};
use zerodds_corba_rust::value_wire::{ValueMarshal, ValueReader, ValueRegistry, ValueWriter};
use zerodds_corba_rust::{CorbaConnection, ValueBase};

// valuetype Point { public long x; public long y; }; — as emitted by the codegen.
#[derive(Debug, PartialEq, Eq)]
struct Point {
    x: i32,
    y: i32,
}
impl ValueBase for Point {
    fn repository_id(&self) -> &str {
        "IDL:Point:1.0"
    }
}
impl ValueMarshal for Point {
    fn marshal_state(&self, w: &mut BufferWriter) -> Result<(), zerodds_cdr::EncodeError> {
        self.x.encode(w)?;
        self.y.encode(w)
    }
}

// valuetype Extended : Base { ... }; state flattening: version (base) first.
#[derive(Debug, PartialEq, Eq)]
struct Extended {
    version: i32,
    name: String,
}
impl ValueBase for Extended {
    fn repository_id(&self) -> &str {
        "IDL:Extended:1.0"
    }
}
impl ValueMarshal for Extended {
    fn marshal_state(&self, w: &mut BufferWriter) -> Result<(), zerodds_cdr::EncodeError> {
        self.version.encode(w)?; // base state first (§15.3.4)
        self.name.encode(w)
    }
}

fn registry() -> ValueRegistry {
    let mut reg = ValueRegistry::new();
    reg.register(
        "IDL:Point:1.0",
        Box::new(|r: &mut BufferReader<'_>| {
            let x = i32::decode(r)?;
            let y = i32::decode(r)?;
            Ok(Rc::new(Point { x, y }) as Rc<dyn core::any::Any>)
        }),
    );
    reg.register(
        "IDL:Extended:1.0",
        Box::new(|r: &mut BufferReader<'_>| {
            let version = i32::decode(r)?;
            let name = String::decode(r)?;
            Ok(Rc::new(Extended { version, name }) as Rc<dyn core::any::Any>)
        }),
    );
    reg
}

/// `Point echo(in Point p)` over real IIOP: the client marshals a Point, the
/// server decodes + re-marshals it, the client decodes the reply.
#[test]
fn valuetype_echo_roundtrip() {
    let server = CorbaServer::new();
    server.register(b"ValueEcho", |op, body, e| {
        use zerodds_corba_rust::SkeletonResult;
        if op != "echo" {
            return SkeletonResult::BadOperation;
        }
        let mut r = BufferReader::new(body, e);
        let mut vr = ValueReader::new();
        let v = vr.read(&mut r, 0, &registry()).unwrap().expect("non-null");
        let p = v.downcast_ref::<Point>().expect("Point");
        // Re-marshal as the reply.
        let mut w = BufferWriter::new(e);
        let mut vw = ValueWriter::new();
        let echo: Rc<dyn ValueMarshal> = Rc::new(Point { x: p.x, y: p.y });
        vw.write(&mut w, Some(&echo)).unwrap();
        SkeletonResult::Reply(w.into_bytes())
    });
    let acceptor = server.serve("127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = acceptor.listen_addr();
    let ior = object_reference(
        "IDL:ValueEcho:1.0",
        &addr.ip().to_string(),
        addr.port(),
        b"ValueEcho",
    );
    let conn = IiopCorbaConnection::new();

    for e in [Endianness::Big, Endianness::Little] {
        let mut w = BufferWriter::new(e);
        let mut vw = ValueWriter::new();
        let p: Rc<dyn ValueMarshal> = Rc::new(Point { x: 42, y: -7 });
        vw.write(&mut w, Some(&p)).unwrap();
        let (reply, reply_e) = conn.invoke(&ior, "echo", e, &w.into_bytes()).unwrap();

        let mut r = BufferReader::new(&reply, reply_e);
        let mut vr = ValueReader::new();
        let got = vr.read(&mut r, 0, &registry()).unwrap().unwrap();
        assert_eq!(
            *got.downcast_ref::<Point>().unwrap(),
            Point { x: 42, y: -7 }
        );
    }
    acceptor.shutdown();
}

/// Value sharing through the GIOP body: the client writes the SAME instance
/// twice (the second = indirection); the server reads both with ONE
/// `ValueReader` and must resolve the same `Rc` instance. Reply = 1 if shared,
/// otherwise 0.
#[test]
fn valuetype_sharing_over_giop() {
    let server = CorbaServer::new();
    server.register(b"Share", |op, body, e| {
        use zerodds_corba_rust::SkeletonResult;
        if op != "pair" {
            return SkeletonResult::BadOperation;
        }
        let mut r = BufferReader::new(body, e);
        let mut vr = ValueReader::new();
        let reg = registry();
        let a = vr.read(&mut r, 0, &reg).unwrap().unwrap();
        let b = vr.read(&mut r, 0, &reg).unwrap().unwrap();
        let shared = u8::from(Rc::ptr_eq(&a, &b));
        SkeletonResult::Reply(vec![shared])
    });
    let acceptor = server.serve("127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = acceptor.listen_addr();
    let ior = object_reference(
        "IDL:Share:1.0",
        &addr.ip().to_string(),
        addr.port(),
        b"Share",
    );
    let conn = IiopCorbaConnection::new();

    for e in [Endianness::Big, Endianness::Little] {
        let mut w = BufferWriter::new(e);
        let mut vw = ValueWriter::new();
        let p: Rc<dyn ValueMarshal> = Rc::new(Point { x: 5, y: 9 });
        vw.write(&mut w, Some(&p)).unwrap();
        vw.write(&mut w, Some(&p)).unwrap(); // same instance → indirection
        let (reply, _e) = conn.invoke(&ior, "pair", e, &w.into_bytes()).unwrap();
        assert_eq!(reply, vec![1], "value sharing lost over GIOP ({e:?})");
    }
    acceptor.shutdown();
}

/// Inheritance-flattened value over GIOP: `Extended { version, name }` — base
/// state first. The roundtrip proves the §15.3.4 ordering end-to-end.
#[test]
fn valuetype_inheritance_flattened_roundtrip() {
    let server = CorbaServer::new();
    server.register(b"ExtEcho", |op, body, e| {
        use zerodds_corba_rust::SkeletonResult;
        if op != "echo" {
            return SkeletonResult::BadOperation;
        }
        let mut r = BufferReader::new(body, e);
        let mut vr = ValueReader::new();
        let v = vr.read(&mut r, 0, &registry()).unwrap().unwrap();
        let x = v.downcast_ref::<Extended>().unwrap();
        let mut w = BufferWriter::new(e);
        let mut vw = ValueWriter::new();
        let echo: Rc<dyn ValueMarshal> = Rc::new(Extended {
            version: x.version,
            name: x.name.clone(),
        });
        vw.write(&mut w, Some(&echo)).unwrap();
        SkeletonResult::Reply(w.into_bytes())
    });
    let acceptor = server.serve("127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = acceptor.listen_addr();
    let ior = object_reference(
        "IDL:ExtEcho:1.0",
        &addr.ip().to_string(),
        addr.port(),
        b"ExtEcho",
    );
    let conn = IiopCorbaConnection::new();

    let mut w = BufferWriter::new(Endianness::Big);
    let mut vw = ValueWriter::new();
    let v: Rc<dyn ValueMarshal> = Rc::new(Extended {
        version: 3,
        name: "rev2".to_string(),
    });
    vw.write(&mut w, Some(&v)).unwrap();
    let (reply, reply_e) = conn
        .invoke(&ior, "echo", Endianness::Big, &w.into_bytes())
        .unwrap();

    let mut r = BufferReader::new(&reply, reply_e);
    let mut vr = ValueReader::new();
    let got = vr.read(&mut r, 0, &registry()).unwrap().unwrap();
    assert_eq!(
        *got.downcast_ref::<Extended>().unwrap(),
        Extended {
            version: 3,
            name: "rev2".to_string()
        }
    );
    acceptor.shutdown();
}
