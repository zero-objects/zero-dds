// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Live valuetype cross-ORB (§15.3.4): ZeroDDS invokes a REAL operation with
//! a `valuetype` parameter on a FOREIGN server (JacORB). Proves the
//! valuetype wire not just as a byte capture, but as an operation parameter
//! end-to-end over GIOP (marshal → invoke → foreign server unmarshals + echoes →
//! ZeroDDS decodes).
//!
//! The server, `interface ValueEcho { Point echo(in Point p); }` in
//! JacORB, is `competitors/jacorb/valuetype/`; the test builds and starts it
//! (`tests/support/jacorb.rs`) and runs in the integration job
//! `integration-bridges-vendor-corba`.

use std::rc::Rc;

use zerodds_corba_interop::runtime::{IiopCorbaConnection, object_reference_from_ior};
use zerodds_corba_rust::value_wire::{ValueMarshal, ValueReader, ValueRegistry, ValueWriter};
use zerodds_corba_rust::{CorbaConnection, ValueBase};
use zerodds_wire::{Endianness, Reader, Writer};

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
    fn marshal_state(&self, w: &mut Writer) -> Result<(), zerodds_wire::WireError> {
        w.write_i32(self.x);
        w.write_i32(self.y);
        Ok(())
    }
}
fn registry() -> ValueRegistry {
    let mut reg = ValueRegistry::new();
    reg.register(
        "IDL:Point:1.0",
        Box::new(|r: &mut Reader<'_>| {
            let x = r.read_i32()?;
            let y = r.read_i32()?;
            Ok(Rc::new(Point { x, y }) as Rc<dyn core::any::Any>)
        }),
    );
    reg
}

#[path = "support/jacorb.rs"]
mod jacorb;

#[test]
#[ignore = "needs JacORB 3.9 and a Java 8 JDK (integration-bridges-vendor-corba)"]
fn corbarust_13_a_jacorb_server_echoes_a_valuetype_parameter() {
    let fixture =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("competitors/jacorb/valuetype");
    let (_server, ior) = jacorb::start(&jacorb::Spec {
        name: "jacorb-valueecho",
        idl: vec![fixture.join("ValueEcho.idl")],
        java: vec![
            fixture.join("ValueEchoServer.java"),
            fixture.join("PointImpl.java"),
        ],
        main: "ValueEchoServer",
        props: &[],
        ior_to_file: false,
        stdout_prefix: "VALUEECHO_IOR=",
    });
    let oref = object_reference_from_ior(ior.trim()).expect("parse IOR");
    let conn = IiopCorbaConnection::new();

    // In-arg: marshal Point(42,-7) as a valuetype into the request body.
    let mut w = Writer::new(Endianness::Big);
    let p: Rc<dyn ValueMarshal> = Rc::new(Point { x: 42, y: -7 });
    ValueWriter::new().write(&mut w, Some(&p)).unwrap();

    let (reply, reply_e) = conn
        .invoke(&oref, "echo", Endianness::Big, &w.finish())
        .expect("foreign echo invoke");

    // Reply: the Point re-marshalled back by the foreign ORB.
    let mut r = Reader::new(&reply, reply_e);
    let got = ValueReader::new()
        .read(&mut r, 0, &registry())
        .unwrap()
        .expect("non-null reply value");
    assert_eq!(
        *got.downcast_ref::<Point>().unwrap(),
        Point { x: 42, y: -7 }
    );
}
