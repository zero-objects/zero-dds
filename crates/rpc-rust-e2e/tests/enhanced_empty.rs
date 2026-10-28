// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Enhanced Service Mapping with `@Empty` structures (DDS-RPC 1.0
//! §7.5.1.2.1.3): an operation with nothing to carry has an empty `In` and
//! an empty `Out`, and the dispatcher answers it.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use zerodds_rpc::replier::ReplierHandler;
use zerodds_rpc_rust_e2e::empty_bindings::emp::{
    Pinger, Pinger_Reply, Pinger_Request, Pinger_count_In, Pinger_ping_In, Pinger_ping_Out,
    Pinger_ping_Result, PingerDispatcher, PingerError,
};
use zerodds_wire::{Endianness, Reader, Writer};

/// An `@Empty` structure is no octets: it is not a mutable object, so it has
/// no DHEADER either.
#[test]
fn an_empty_structure_has_no_octets() {
    let mut writer = Writer::with_max_align(Endianness::Little, 4);
    Pinger_ping_In {}.encode(&mut writer).expect("encode");
    assert!(writer.finish().is_empty());
    let mut reader = Reader::with_max_align(&[], Endianness::Little, 4);
    assert_eq!(
        Pinger_ping_In::decode(&mut reader).unwrap(),
        Pinger_ping_In {}
    );
}

#[test]
fn the_dispatcher_answers_an_operation_with_empty_structures() {
    struct Impl;
    impl Pinger for Impl {
        fn ping(&self) -> Result<(), PingerError> {
            Ok(())
        }
        fn count(&self) -> Result<i32, PingerError> {
            Ok(5)
        }
    }
    let dispatcher = PingerDispatcher::new(Impl);
    assert_eq!(
        dispatcher.handle(Pinger_Request::Ping(Pinger_ping_In {})),
        Ok(Pinger_Reply::Ping(Pinger_ping_Result::Ok(
            Pinger_ping_Out {}
        )))
    );
    assert!(
        dispatcher
            .handle(Pinger_Request::Count(Pinger_count_In {}))
            .is_ok()
    );
}
