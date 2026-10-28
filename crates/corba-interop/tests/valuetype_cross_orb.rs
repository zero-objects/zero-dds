// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr
)]
//! Live valuetype cross-ORB (§15.3.4): ZeroDDS invokes a REAL operation with
//! a `valuetype` parameter on a FOREIGN server (JacORB). Proves the
//! valuetype wire not just as a byte capture, but as an operation parameter
//! end-to-end over GIOP (marshal → invoke → foreign server unmarshals + echoes →
//! ZeroDDS decodes).
//!
//! Runs only with a live `interface ValueEcho { Point echo(in Point p); }` server
//! whose IOR is provided via `VALUEECHO_IOR` (the Linux test host). Ignored by default.

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

#[test]
#[ignore = "needs live foreign ValueEcho server via VALUEECHO_IOR env (Linux test host)"]
fn valuetype_echo_against_foreign_server() {
    let ior = std::env::var("VALUEECHO_IOR").expect("VALUEECHO_IOR env");
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
    eprintln!("cross-ORB live valuetype OK: echo(Point(42,-7)) roundtrip");
}
