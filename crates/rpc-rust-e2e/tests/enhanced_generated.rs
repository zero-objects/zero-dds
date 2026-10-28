// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Rust binding generated for the Enhanced Service Mapping
//! (DDS-RPC 1.0 §7.5.1.2): its octets against the clause, and a client
//! calling a dispatcher over a real `DcpsRuntime`.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use zerodds_dcps::dds_type::{DdsType, RawBytes};
use zerodds_dcps::factory::DomainParticipantFactory;
use zerodds_dcps::qos::DomainParticipantQos;
use zerodds_rpc::common_types::RemoteExceptionCode;
use zerodds_rpc::error::RpcError;
use zerodds_rpc::qos_profile::RpcQos;
use zerodds_rpc::replier::{FnHandler, Replier, ReplierHandler};
use zerodds_rpc::requester::Requester;
use zerodds_rpc::topic_naming::{AnnotatedTopics, BindingStyle};
use zerodds_rpc_rust_e2e::enhanced_bindings::enh::{
    Base2, Base2Dispatcher, Base2Error, Calc, Calc_Reply, Calc_Request, Calc_add_In, Calc_add_Out,
    Calc_add_Result, Calc_scale_Result, CalcClient, CalcDispatcher, CalcError, Derived2,
    Derived2_Request, Derived2Dispatcher, Derived2Error, Fast, Fast_square_In, FastClient,
    FastDispatcher, FastError, TooBig,
};
use zerodds_wire::{Endianness, Reader, WireError, Writer};

/// §7.5.1.2.1.2: the member id is the HASH of the member name, 28 bits of
/// it (the EMHEADER holds no more). The HASH is §7.5.1.1.2's.
fn member_id(name: &str) -> u32 {
    zerodds_rpc::rpc_hash::rpc_member_hash(name) & 0x0FFF_FFFF
}

/// Encodes with the writer an XCDR2 little-endian sample is written with.
fn encode_with(write: impl FnOnce(&mut Writer) -> Result<(), WireError>) -> Vec<u8> {
    let mut writer = Writer::with_max_align(Endianness::Little, 4);
    write(&mut writer).expect("encode");
    writer.finish()
}

/// Decodes with the reader an XCDR2 little-endian sample is read with.
fn decode_with<T>(bytes: &[u8], read: impl FnOnce(&mut Reader) -> Result<T, WireError>) -> T {
    let mut reader = Reader::with_max_align(bytes, Endianness::Little, 4);
    read(&mut reader).expect("decode")
}

fn u32le(value: u32) -> [u8; 4] {
    value.to_le_bytes()
}

/// §7.5.1.2.1.1: a `@Choice` is read as `@mutable` with every member
/// optional and exactly one present, so `Calc_Request` is a PL_CDR2 object:
/// DHEADER, an EMHEADER with length code 4 (NEXTINT follows) and the
/// member id, the NEXTINT, and the `Calc_add_In` structure.
#[test]
fn a_request_is_a_mutable_structure_with_one_member() {
    let request = Calc_Request::Add(Calc_add_In { a: 1, b: 2 });
    let bytes = encode_with(|w| request.encode(w));
    // The structure: two `long`s. The member: EMHEADER, NEXTINT, 8 octets.
    let mut expected = Vec::new();
    expected.extend(u32le(16));
    expected.extend(u32le((4 << 28) | member_id("add")));
    expected.extend(u32le(8));
    expected.extend(u32le(1));
    expected.extend(u32le(2));
    assert_eq!(bytes, expected);
    assert_eq!(decode_with(&bytes, Calc_Request::decode), request);
}

/// §7.5.1.2.2: with `@Mutable` the `In` structure is itself
/// `@Extensibility(MUTABLE_EXTENSIBILITY)`: a DHEADER and an EMHEADER per
/// member, here length code 2 for a `long`.
#[test]
fn a_mutable_operation_has_mutable_structures() {
    let value = Fast_square_In { x: 7 };
    let bytes = encode_with(|w| value.encode(w));
    let mut expected = Vec::new();
    expected.extend(u32le(8));
    expected.extend(u32le((2 << 28) | member_id("x")));
    expected.extend(u32le(7));
    assert_eq!(bytes, expected);
    assert_eq!(decode_with(&bytes, Fast_square_In::decode), value);
    // An `@Autoid` structure without `@Mutable` is final: no headers.
    let plain = Calc_add_In { a: 1, b: 2 };
    let bytes = encode_with(|w| plain.encode(w));
    assert_eq!(bytes.len(), 8);
}

/// §7.5.1.2.1.1: a mutable structure's members come in any order and a
/// member a reader does not know is skipped by its length.
#[test]
fn a_mutable_structure_skips_a_member_it_does_not_know() {
    let mut bytes = Vec::new();
    // DHEADER over: an unknown member (LC 4, 3 octets), then `x`.
    let mut body = Vec::new();
    body.extend(u32le((4 << 28) | 0x0000_0123));
    body.extend(u32le(3));
    body.extend([1, 2, 3, 0]); // three octets and the padding to 4
    body.extend(u32le((2 << 28) | member_id("x")));
    body.extend(u32le(9));
    bytes.extend(u32le(body.len() as u32));
    bytes.extend(body);
    // The unknown member's extent is 3 octets; the padding is skipped by
    // the next EMHEADER's alignment.
    let decoded = decode_with(&bytes, Fast_square_In::decode);
    assert_eq!(decoded, Fast_square_In { x: 9 });
}

/// §7.5.1.2.5 rule 5: the reply has a `remoteEx` member.
#[test]
fn a_reply_carries_remote_ex() {
    let reply = Calc_Reply::RemoteEx(RemoteExceptionCode::UnknownOperation);
    let bytes = encode_with(|w| reply.encode(w));
    assert_eq!(
        bytes,
        zerodds_rpc::enhanced::encode_remote_ex_reply(
            RemoteExceptionCode::UnknownOperation,
            "remoteEx"
        ),
        "the same octets the runtime writes for a refused request"
    );
    assert_eq!(decode_with(&bytes, Calc_Reply::decode), reply);
}

/// §7.5.1.2.3 rules 5 and 6: the `Result` structure has `result` and a
/// member per exception, `<lowercase name>_ex`.
#[test]
fn a_result_is_a_choice_of_the_result_and_the_exceptions() {
    let failed = Calc_scale_Result::TooBig(TooBig { limit: 3 });
    let bytes = encode_with(|w| failed.encode(w));
    let emheader = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    assert_eq!(emheader & 0x0FFF_FFFF, member_id("toobig_ex"));
    assert_eq!(decode_with(&bytes, Calc_scale_Result::decode), failed);
    let ok = Calc_add_Result::Ok(Calc_add_Out { return_: 5 });
    let bytes = encode_with(|w| ok.encode(w));
    let emheader = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    assert_eq!(emheader & 0x0FFF_FFFF, member_id("result"));
}

/// §7.5.1.2.6: the request of a derived interface has a member for the
/// inherited operation, under the base interface's type names.
#[test]
fn a_derived_request_has_the_inherited_operation() {
    let request = Derived2_Request::Ident(
        zerodds_rpc_rust_e2e::enhanced_bindings::enh::Base2_ident_In { dummy: 0 },
    );
    let bytes = encode_with(|w| request.encode(w));
    let emheader = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    assert_eq!(emheader & 0x0FFF_FFFF, member_id("ident"));
    assert_eq!(decode_with(&bytes, Derived2_Request::decode), request);
}

/// A member the interface does not have is decoded as `UnknownOp`, not
/// refused: the service can answer `REMOTE_EX_UNKNOWN_OPERATION`.
#[test]
fn a_member_the_interface_does_not_have_is_unknown_operation() {
    let mut bytes = Vec::new();
    let mut body = Vec::new();
    body.extend(u32le((4 << 28) | 0x00AB_CDEF));
    body.extend(u32le(4));
    body.extend(u32le(0));
    bytes.extend(u32le(body.len() as u32));
    bytes.extend(body);
    assert_eq!(
        decode_with(&bytes, Calc_Request::decode),
        Calc_Request::UnknownOp(0x00AB_CDEF)
    );
}

struct CalcImpl {
    memory: Mutex<i32>,
}

impl Calc for CalcImpl {
    fn add(&self, a: i32, b: i32) -> Result<i32, CalcError> {
        Ok(a + b)
    }
    fn scale(&self, k: i32) -> Result<i32, CalcError> {
        if k > 100 {
            return Err(CalcError::TooBig(TooBig { limit: 100 }));
        }
        Ok(k * 2)
    }
    fn swap(&self, v: &mut i32) -> Result<(), CalcError> {
        std::mem::swap(&mut *self.memory.lock().unwrap(), v);
        Ok(())
    }
    fn ping(&self) -> Result<(), CalcError> {
        Ok(())
    }
}

struct FastImpl;

impl Fast for FastImpl {
    fn square(&self, x: i32) -> Result<i32, FastError> {
        Ok(x * x)
    }
}

struct Ticking {
    stop: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Ticking {
    fn spawn<TIn: DdsType + Send + 'static, TOut: DdsType + Send + 'static>(
        replier: Replier<TIn, TOut>,
    ) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let handle = thread::spawn(move || {
            while !flag.load(Ordering::Acquire) {
                let _ = replier.tick();
                thread::sleep(Duration::from_millis(2));
            }
        });
        Self {
            stop,
            handle: Some(handle),
        }
    }
}

impl Drop for Ticking {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// The generated Enhanced client calls the generated dispatcher: no
/// header, the reply correlated by its related sample identity, the
/// exception as the `Result` member, the `@Mutable` operation, and an
/// operation the service does not have as `REMOTE_EX_UNKNOWN_OPERATION`.
#[test]
fn the_generated_enhanced_client_calls_the_generated_dispatcher() {
    let lease = zerodds_test_domains::lease_domain();
    let participant = DomainParticipantFactory::instance()
        .create_participant(lease.id(), DomainParticipantQos::default())
        .expect("a real participant on a leased domain");
    let qos = RpcQos::default_enhanced();

    let calc = CalcDispatcher::new(CalcImpl {
        memory: Mutex::new(5),
    })
    .serve(&participant, &qos)
    .expect("a replier");
    let _calc = Ticking::spawn(calc);
    let fast = FastDispatcher::new(FastImpl)
        .serve(&participant, &qos)
        .expect("a replier");
    let _fast = Ticking::spawn(fast);

    let client = CalcClient::new(&participant, &qos).expect("a client");
    assert_eq!(client.add(2, 3).expect("add"), 5);
    assert_eq!(client.scale(21).expect("scale"), 42);
    match client.scale(1000) {
        Err(CalcError::TooBig(exception)) => assert_eq!(exception.limit, 100),
        other => panic!("expected TooBig, got {other:?}"),
    }
    let mut v = 9;
    client.swap(&mut v).expect("swap");
    assert_eq!(v, 5, "the service's own value came back");
    client.ping().expect("ping");

    let fast = FastClient::new(&participant, &qos).expect("a client");
    assert_eq!(fast.square(12).expect("the @Mutable operation"), 144);
}

/// §7.7.2: a request for an operation the interface does not have is
/// answered `REMOTE_EX_UNKNOWN_OPERATION`, where the Basic mapping's
/// §7.7.1.1 says `REMOTE_EX_UNSUPPORTED`.
#[test]
fn an_unknown_member_is_answered_unknown_operation() {
    use zerodds_rpc::replier::ReplierHandler;
    let dispatcher = CalcDispatcher::new(CalcImpl {
        memory: Mutex::new(0),
    });
    assert_eq!(
        dispatcher.handle(Calc_Request::UnknownOp(0x00AB_CDEF)),
        Err(RemoteExceptionCode::UnknownOperation)
    );
}

/// §7.5.1.2.6: the request choice of a derived interface has the inherited
/// operations, flattened in, and the derived service implements the base
/// trait: the derived dispatcher answers an inherited member from the base
/// operation, with the base's `Result` type (`Base2_ident_Result`).
#[test]
fn a_derived_dispatcher_serves_an_inherited_member() {
    use zerodds_rpc::replier::ReplierHandler;
    use zerodds_rpc_rust_e2e::enhanced_bindings::enh::{
        Base2_ident_In, Base2_ident_Out, Base2_ident_Result, Derived2_Reply, Derived2_more_In,
        Derived2_more_Out, Derived2_more_Result,
    };
    struct Both;
    impl Base2 for Both {
        fn ident(&self) -> Result<i32, Base2Error> {
            Ok(1)
        }
    }
    impl Derived2 for Both {
        fn more(&self) -> Result<i32, Derived2Error> {
            Ok(2)
        }
    }
    let dispatcher = Derived2Dispatcher::new(Both);
    assert_eq!(
        dispatcher.handle(Derived2_Request::Ident(Base2_ident_In { dummy: 0 })),
        Ok(Derived2_Reply::Ident(Base2_ident_Result::Ok(
            Base2_ident_Out { return_: 1 }
        )))
    );
    assert_eq!(
        dispatcher.handle(Derived2_Request::More(Derived2_more_In { dummy: 0 })),
        Ok(Derived2_Reply::More(Derived2_more_Result::Ok(
            Derived2_more_Out { return_: 2 }
        )))
    );
    let _ = Base2Dispatcher::new(Both);
    let _: Option<RpcError> = None;
}

/// XTypes 1.3 §7.2.4.4.8: "Each non-optional member in a T1 object that is
/// not present in the T2 object takes its default value." A `@Mutable`
/// structure whose body has no member `x` is read with `x` = 0, not refused.
#[test]
fn an_absent_member_of_a_mutable_structure_takes_its_default() {
    // DHEADER 0: a mutable object with no member.
    let bytes = 0u32.to_le_bytes();
    let decoded = decode_with(&bytes, Fast_square_In::decode);
    assert_eq!(decoded, Fast_square_In { x: 0 });
}

/// One `@Choice` member of the wire: an `EMHEADER` with length code 4,
/// the `NEXTINT` and the member's octets.
fn choice_member(id: u32, body: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend(u32le((4 << 28) | id));
    out.extend(u32le(body.len() as u32));
    out.extend(body);
    out
}

/// A `@Choice` object: the `DHEADER` and the members.
fn choice_object(members: &[Vec<u8>]) -> Vec<u8> {
    let body: Vec<u8> = members.concat();
    let mut out = Vec::new();
    out.extend(u32le(body.len() as u32));
    out.extend(body);
    out
}

/// The octets of a `Calc_add_In`: two `long`s.
fn add_in(a: u32, b: u32) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend(u32le(a));
    out.extend(u32le(b));
    out
}

/// §7.5.1.2.1.1: "exactly one member shall be present at any given
/// time", and §7.5.2 gives "two options active in a @Choice structure" as
/// the example of `REMOTE_EX_INVALID_ARGUMENT`. A request with two active
/// members is therefore not decoded, where it used to be read as its first
/// member.
#[test]
fn a_request_with_two_active_members_is_refused() {
    let one = choice_object(&[choice_member(member_id("add"), &add_in(1, 2))]);
    assert_eq!(
        decode_with(&one, Calc_Request::decode),
        Calc_Request::Add(Calc_add_In { a: 1, b: 2 })
    );
    let two = choice_object(&[
        choice_member(member_id("add"), &add_in(1, 2)),
        choice_member(member_id("add"), &add_in(3, 4)),
    ]);
    let mut reader = Reader::with_max_align(&two, Endianness::Little, 4);
    assert!(Calc_Request::decode(&mut reader).is_err());
    // The second member does not have to be a known one.
    let mixed = choice_object(&[
        choice_member(member_id("add"), &add_in(1, 2)),
        choice_member(0x0012_3456, &[0, 0, 0, 0]),
    ]);
    let mut reader = Reader::with_max_align(&mixed, Endianness::Little, 4);
    assert!(Calc_Request::decode(&mut reader).is_err());
}

/// The other side of "exactly one": a choice with no member is refused,
/// for the request and for the `Result`.
#[test]
fn a_choice_without_a_member_is_refused() {
    let empty = choice_object(&[]);
    let mut reader = Reader::with_max_align(&empty, Endianness::Little, 4);
    assert!(Calc_Request::decode(&mut reader).is_err());
    let mut reader = Reader::with_max_align(&empty, Endianness::Little, 4);
    assert!(Calc_scale_Result::decode(&mut reader).is_err());
}

/// A service answers a request that cannot be decoded with
/// `REMOTE_EX_INVALID_ARGUMENT` (§7.5.2). The request carries two active
/// members; the generated dispatcher's replier is asked over the real
/// runtime.
#[test]
fn the_generated_service_answers_two_active_members_with_invalid_argument() {
    let lease = zerodds_test_domains::lease_domain();
    let participant = DomainParticipantFactory::instance()
        .create_participant(lease.id(), DomainParticipantQos::default())
        .expect("a real participant on a leased domain");
    let qos = RpcQos::default_enhanced();
    let calc = CalcDispatcher::new(CalcImpl {
        memory: Mutex::new(0),
    })
    .serve(&participant, &qos)
    .expect("a replier");
    let _calc = Ticking::spawn(calc);

    let raw = Requester::<RawBytes, RawBytes>::with_options(
        &participant,
        "enh_Calc",
        "",
        &qos,
        BindingStyle::FunctionCall,
        &AnnotatedTopics::none(),
        &CalcClient::endpoint_options(),
    )
    .expect("a requester");
    let two = choice_object(&[
        choice_member(member_id("add"), &add_in(1, 2)),
        choice_member(member_id("add"), &add_in(3, 4)),
    ]);
    let err = raw
        .send_request_blocking(&RawBytes::new(two), Some(Duration::from_secs(10)))
        .unwrap_err();
    assert_eq!(
        err,
        RpcError::RemoteException(RemoteExceptionCode::InvalidArgument.as_u32())
    );
}

/// The octets of a `Result` of `scale` that carries an exception member
/// this version of the interface does not declare.
fn scale_result_with_unknown_member(id: u32) -> Vec<u8> {
    choice_object(&[choice_member(id, &[7, 0, 0, 0])])
}

/// §7.7.2.5.6: a `Result` member the interface does not declare decodes
/// to `UnknownEx`, and so does not make the whole reply undecodable.
#[test]
fn an_unknown_exception_member_of_a_result_decodes_as_unknown_ex() {
    let bytes = scale_result_with_unknown_member(0x0000_0777);
    assert_eq!(
        decode_with(&bytes, Calc_scale_Result::decode),
        Calc_scale_Result::UnknownEx(0x0000_0777)
    );
    // The known members still decode as before.
    let known = encode_with(|w| Calc_scale_Result::TooBig(TooBig { limit: 3 }).encode(w));
    assert_eq!(
        decode_with(&known, Calc_scale_Result::decode),
        Calc_scale_Result::TooBig(TooBig { limit: 3 })
    );
}

/// §7.7.2.5.6: "The language binding that provides function call/return
/// semantics may throw REMOTE_EX_UNKNOWN_EXCEPTION if the RPC call results
/// into an exception that client does not understand." A service of a
/// newer version of the interface answers `scale` with an exception member
/// this client does not have; the generated client reports
/// `RemoteExceptionCode::UnknownException`.
#[test]
fn the_generated_client_reports_unknown_exception_for_an_unknown_result_member() {
    let lease = zerodds_test_domains::lease_domain();
    let participant = DomainParticipantFactory::instance()
        .create_participant(lease.id(), DomainParticipantQos::default())
        .expect("a real participant on a leased domain");
    let qos = RpcQos::default_enhanced();

    // The reply of the newer service: `Calc_Reply` with the `scale`
    // member, whose `Result` holds a member this version does not know.
    let reply = choice_object(&[choice_member(
        member_id("scale"),
        &scale_result_with_unknown_member(0x0000_0777),
    )]);
    let handler: Arc<dyn ReplierHandler<RawBytes, RawBytes>> =
        Arc::new(FnHandler::new(move |_request: RawBytes| {
            Ok::<_, RemoteExceptionCode>(RawBytes::new(reply.clone()))
        }));
    let newer = Replier::<RawBytes, RawBytes>::with_options(
        &participant,
        "enh_Calc",
        "",
        &qos,
        BindingStyle::FunctionCall,
        &AnnotatedTopics::none(),
        &CalcClient::endpoint_options(),
        handler,
    )
    .expect("a replier");
    let _newer = Ticking::spawn(newer);

    let client = CalcClient::new(&participant, &qos).expect("a client");
    match client.scale(3) {
        Err(CalcError::Remote(RpcError::RemoteException(code))) => {
            assert_eq!(code, RemoteExceptionCode::UnknownException.as_u32());
        }
        other => panic!("expected REMOTE_EX_UNKNOWN_EXCEPTION, got {other:?}"),
    }
}

/// §7.5.2: an exception the operation does not declare is
/// `REMOTE_EX_UNKNOWN_EXCEPTION`, in the Enhanced mapping too: `TooBig` is
/// declared by `scale` alone, and `add` returning it is answered with the
/// code.
#[test]
fn an_exception_the_operation_does_not_declare_is_unknown_exception() {
    use zerodds_rpc::replier::ReplierHandler;
    struct Wrong;
    impl Calc for Wrong {
        fn add(&self, _a: i32, _b: i32) -> Result<i32, CalcError> {
            Err(CalcError::TooBig(TooBig { limit: 1 }))
        }
        fn scale(&self, k: i32) -> Result<i32, CalcError> {
            Ok(k)
        }
        fn swap(&self, _v: &mut i32) -> Result<(), CalcError> {
            Ok(())
        }
        fn ping(&self) -> Result<(), CalcError> {
            Ok(())
        }
    }
    let dispatcher = CalcDispatcher::new(Wrong);
    assert_eq!(
        dispatcher.handle(Calc_Request::Add(Calc_add_In { a: 1, b: 2 })),
        Err(RemoteExceptionCode::UnknownException)
    );
}
