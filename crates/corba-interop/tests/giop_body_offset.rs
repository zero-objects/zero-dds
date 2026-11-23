// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CORBA 3.3 Part 2 §9.4.2.1 through `IiopCorbaConnection` and
//! `CorbaServer`: a GIOP 1.0 or 1.1 request body follows the header
//! without padding, and CDR aligns its arguments from the first octet of
//! the message. An octet followed by an 8-octet argument (`long long`
//! from GIOP 1.1, Table 7.2; `double` in every version) goes over IIOP
//! 1.0, 1.1 and 1.2 profiles, in both byte orders, with two object keys
//! whose lengths put the body at offsets 4 apart.
//!
//! Each call checks both sides apart: the 8-octet value stands on a
//! multiple of 8 counted from the message start (the client wrote it at
//! the body's offset), and the servant received the value (the server
//! read it there). A client and a server that both counted from the
//! body's first octet would agree with each other and fail the first
//! check.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    dead_code,
    unused_imports
)]

use std::sync::{Arc, Mutex};

include!(concat!(env!("OUT_DIR"), "/corba_gen.rs"));

use corba_gen::{
    Aligned, AlignedAmiHandler, AlignedError, AlignedLate, AlignedStub, dispatch_aligned,
};
use zerodds_corba_iiop::{IiopProfileBody, IiopVersion};
use zerodds_corba_interop::runtime::{AmiClient, CorbaServer, IiopCorbaConnection};
use zerodds_corba_rust::{CorbaConnection, CorbaException, ObjectReference, SkeletonResult};
use zerodds_wire::{Endianness, Reader};

const TAG: u8 = 0x5a;
const LONG_LONG: i64 = 0x0102_0304_0506_0708;
const DOUBLE: f64 = 1.000_000_000_000_2;

/// What the servant was called with.
#[derive(Default)]
struct Calls {
    long_long: Mutex<Vec<(u8, i64)>>,
    double: Mutex<Vec<(u8, f64)>>,
}

struct AlignedServant(Arc<Calls>);

impl Aligned for AlignedServant {
    fn after_octet(&self, tag: u8, value: i64) -> Result<i64, CorbaException> {
        self.0.long_long.lock().unwrap().push((tag, value));
        Ok(value)
    }
    fn after_octet_double(&self, tag: u8, value: f64) -> Result<f64, CorbaException> {
        self.0.double.lock().unwrap().push((tag, value));
        Ok(value)
    }
    fn late(&self, value: i64) -> Result<(), AlignedError> {
        Err(AlignedError::AlignedLate(AlignedLate {
            value,
            ratio: DOUBLE,
        }))
    }
}

/// One request as the server got it: its operation, the body's offset
/// in the message, the body's octets and its byte order.
type Seen = Arc<Mutex<Vec<(String, usize, Vec<u8>, Endianness)>>>;

fn reference(port: u16, version: IiopVersion, key: &[u8]) -> ObjectReference {
    let profile = IiopProfileBody::new(version, "127.0.0.1".into(), port, key.to_vec());
    ObjectReference {
        type_id: "IDL:Aligned:1.0".into(),
        iiop_profile: profile.encode_encapsulation(Endianness::Big).unwrap(),
        policy_overrides: Vec::new(),
        profiles: Vec::new(),
    }
}

/// The position of `value` in `body`: exactly one.
fn position_of(body: &[u8], value: [u8; 8]) -> usize {
    let at: Vec<usize> = body
        .windows(8)
        .enumerate()
        .filter(|(_, w)| *w == value)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(at.len(), 1, "the value once in {body:02x?}");
    at[0]
}

fn bytes_of(value: i64, e: Endianness) -> [u8; 8] {
    match e {
        Endianness::Big => value.to_be_bytes(),
        Endianness::Little => value.to_le_bytes(),
    }
}

fn double_bytes_of(value: f64, e: Endianness) -> [u8; 8] {
    match e {
        Endianness::Big => value.to_be_bytes(),
        Endianness::Little => value.to_le_bytes(),
    }
}

#[test]
fn corba_p2_9_4_2_an_8_octet_argument_after_an_octet_is_aligned_from_the_message_start() {
    let calls = Arc::new(Calls::default());
    let seen: Seen = Arc::default();
    let server = CorbaServer::new();
    for key in [&b"Aligned"[..], &b"Aligned-key"[..]] {
        let servant = Arc::new(AlignedServant(Arc::clone(&calls)));
        let seen = Arc::clone(&seen);
        server.register(key, move |op, body, e| {
            seen.lock()
                .unwrap()
                .push((op.to_string(), body.offset(), body.bytes().to_vec(), e));
            dispatch_aligned(&*servant, op, body, e)
        });
    }
    let acceptor = server.serve("127.0.0.1:0".parse().unwrap()).unwrap();
    let port = acceptor.listen_addr().port();
    let conn = Arc::new(IiopCorbaConnection::new());

    let mut expected_long_long = 0;
    let mut expected_double = 0;
    for key in [&b"Aligned"[..], &b"Aligned-key"[..]] {
        for version in [IiopVersion::V1_0, IiopVersion::V1_1, IiopVersion::V1_2] {
            let target = reference(port, version, key);
            // The generated stub writes big-endian.
            let stub = AlignedStub::new(target.clone(), conn.clone());
            assert_eq!(stub.after_octet_double(TAG, DOUBLE).unwrap(), DOUBLE);
            expected_double += 1;
            if version != IiopVersion::V1_0 {
                assert_eq!(stub.after_octet(TAG, LONG_LONG).unwrap(), LONG_LONG);
                expected_long_long += 1;
            }
            // Little-endian requests through the connection, read by the
            // generated skeleton.
            let (reply, re) = conn
                .invoke(&target, "after_octet_double", Endianness::Little, &|w| {
                    w.write_u8(TAG);
                    w.write_f64(DOUBLE);
                    Ok(())
                })
                .unwrap();
            assert_eq!(Reader::new(&reply, re).read_f64().unwrap(), DOUBLE);
            expected_double += 1;
            if version != IiopVersion::V1_0 {
                let (reply, re) = conn
                    .invoke(&target, "after_octet", Endianness::Little, &|w| {
                        w.write_u8(TAG);
                        w.write_i64(LONG_LONG);
                        Ok(())
                    })
                    .unwrap();
                assert_eq!(Reader::new(&reply, re).read_i64().unwrap(), LONG_LONG);
                expected_long_long += 1;
            }
        }
    }
    acceptor.shutdown();

    // The server read every value right.
    let long_long = calls.long_long.lock().unwrap();
    assert_eq!(long_long.len(), expected_long_long);
    assert!(long_long.iter().all(|c| *c == (TAG, LONG_LONG)));
    let double = calls.double.lock().unwrap();
    assert_eq!(double.len(), expected_double);
    assert!(double.iter().all(|c| *c == (TAG, DOUBLE)));

    // The client wrote every value on a multiple of 8 from the message
    // start, after the octet and its padding.
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), expected_long_long + expected_double);
    let mut residues = std::collections::BTreeSet::new();
    for (op, offset, body, e) in seen.iter() {
        assert_eq!(body[0], TAG, "{op}");
        let value = match op.as_str() {
            "after_octet" => bytes_of(LONG_LONG, *e),
            "after_octet_double" => double_bytes_of(DOUBLE, *e),
            other => panic!("{other}"),
        };
        let at = position_of(body, value);
        assert_eq!((offset + at) % 8, 0, "{op} at body offset {offset}");
        residues.insert(offset % 8);
    }
    // The two keys put a GIOP 1.0/1.1 body on a multiple of 8 and 4
    // past one: the value sits at body position 4 or 8, and a reader
    // that counted from the body's first octet would have missed it.
    assert_eq!(
        residues,
        std::collections::BTreeSet::from([0, 4]),
        "body offsets modulo 8"
    );
}

/// A server request interceptor that adds a reply service context of
/// `len` octets: it moves a GIOP 1.0/1.1 reply body by that much.
struct ReplyContext(usize);

impl zerodds_corba_ccm::portable_interceptor::ServerRequestInterceptor for ReplyContext {
    fn name(&self) -> &str {
        "reply-context"
    }
    fn send_reply(
        &self,
        info: &mut zerodds_corba_ccm::portable_interceptor::RequestInfo,
    ) -> zerodds_corba_ccm::portable_interceptor::PointResult {
        info.add_reply_service_context(0x5a44_0002, vec![7; self.0], false)?;
        Ok(())
    }
}

struct ReplyContextInit;

impl zerodds_corba_ccm::portable_interceptor::OrbInitializer for ReplyContextInit {
    fn pre_init(
        &self,
        info: &mut zerodds_corba_ccm::portable_interceptor::OrbInitInfo<'_>,
    ) -> Result<(), zerodds_corba_ccm::portable_interceptor::InitializerError> {
        if let Some(len) = info
            .orb_id()
            .strip_prefix("reply-offset-")
            .and_then(|n| n.parse().ok())
        {
            info.add_server_request_interceptor(Arc::new(ReplyContext(len)))?;
        }
        Ok(())
    }
}

/// CORBA 3.3 Part 2 §9.4.3, OPEN-ITEMS 133 in the reply direction: a GIOP
/// 1.0/1.1 reply body follows the reply header without padding, and the
/// reply service contexts an interceptor adds decide its offset. The
/// server places the body on a multiple of eight (a padding service
/// context when needed): the returned `double` and `long long` stand on
/// a multiple of eight from the message start, and the generated stub
/// reads them. Context lengths 1 to 8 put the body at both multiples of
/// four.
#[test]
fn corba_p2_9_4_3_a_giop_1_0_reply_body_with_reply_contexts_is_aligned_from_the_message_start() {
    use std::io::{Read, Write};
    use zerodds_corba_giop::{
        Message, Request, ResponseFlags, ServiceContextList, TargetAddress, Version,
        decode_message_ctx, encode_message,
    };
    zerodds_corba_ccm::portable_interceptor::register_orb_initializer(Arc::new(ReplyContextInit));
    for len in 1..=8usize {
        let orb = zerodds_corba_interop::runtime::orb_init(
            &mut Vec::new(),
            &format!("reply-offset-{len}"),
        )
        .unwrap();
        let calls = Arc::new(Calls::default());
        let server = CorbaServer::new();
        server.set_orb(orb);
        let servant = Arc::new(AlignedServant(Arc::clone(&calls)));
        server.register(b"Aligned", move |op, body, e| {
            dispatch_aligned(&*servant, op, body, e)
        });
        let acceptor = server.serve("127.0.0.1:0".parse().unwrap()).unwrap();
        let port = acceptor.listen_addr().port();
        for version in [Version::V1_0, Version::V1_1] {
            // The raw reply: where the 8-octet value stands.
            let mut s = std::net::TcpStream::connect(acceptor.listen_addr()).unwrap();
            let request = Message::Request(Request {
                request_id: 1,
                response_flags: ResponseFlags::SYNC_WITH_TARGET,
                target: TargetAddress::Key(b"Aligned".to_vec()),
                operation: "after_octet_double".into(),
                requesting_principal: Some(Vec::new()),
                service_context: ServiceContextList::default(),
                body: Vec::new(),
            });
            // An empty-key request whose body we append at its offset.
            let mut frame = encode_message(version, Endianness::Big, false, &request).unwrap();
            let offset = frame.len();
            let mut w = zerodds_corba_giop::body_writer(offset, Endianness::Big);
            w.write_u8(TAG);
            w.write_f64(DOUBLE);
            let tail = zerodds_corba_giop::finish_body(w, offset);
            frame.extend_from_slice(&tail);
            let size = u32::try_from(frame.len() - 12).unwrap();
            frame[8..12].copy_from_slice(&size.to_be_bytes());
            s.write_all(&frame).unwrap();
            let mut header = [0u8; 12];
            s.read_exact(&mut header).unwrap();
            let n = u32::from_be_bytes([header[8], header[9], header[10], header[11]]) as usize;
            let mut reply = header.to_vec();
            reply.resize(12 + n, 0);
            s.read_exact(&mut reply[12..]).unwrap();
            let (msg, _, _) = decode_message_ctx(&reply).unwrap();
            assert!(matches!(msg, Message::Reply(_)), "{msg:?}");
            let at = position_of(&reply, double_bytes_of(DOUBLE, Endianness::Big));
            assert_eq!(
                at % 8,
                0,
                "GIOP {version:?}, context of {len}: double at {at}"
            );
            // The generated stub through an IIOP profile of that version.
            let iiop = IiopVersion::new(1, version.minor);
            let stub = AlignedStub::new(
                reference(port, iiop, b"Aligned"),
                Arc::new(IiopCorbaConnection::new()),
            );
            assert_eq!(stub.after_octet_double(TAG, DOUBLE).unwrap(), DOUBLE);
            if version == Version::V1_1 {
                assert_eq!(stub.after_octet(TAG, LONG_LONG).unwrap(), LONG_LONG);
            }
        }
        acceptor.shutdown();
    }
}

/// A peer that answers every request as another ORB without a padding
/// context does, in GIOP `version` (1.0 or 1.1) and the byte order `e`: a
/// `NO_EXCEPTION` reply with one reply service context of four octets,
/// whose body (the `double` [`DOUBLE`]) follows the reply header directly,
/// on four modulo eight, with the `double` aligned from the message start.
/// Returns the port and the offsets the reply bodies stood at. One thread
/// per connection; the peer runs until the test ends.
fn double_reply_peer(
    version: zerodds_corba_giop::Version,
    e: Endianness,
) -> (u16, Arc<Mutex<Vec<usize>>>) {
    use std::io::{Read, Write};
    use zerodds_corba_giop::{
        Message, Reply, ReplyStatusType, ServiceContext, ServiceContextList, decode_message_ctx,
        encode_message,
    };
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let offsets = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&offsets);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else {
                break;
            };
            let seen = Arc::clone(&seen);
            std::thread::spawn(move || {
                loop {
                    let mut header = [0u8; 12];
                    if s.read_exact(&mut header).is_err() {
                        break;
                    }
                    // GIOP 1.0 byte_order and the GIOP 1.1 flags: bit 0.
                    let size = [header[8], header[9], header[10], header[11]];
                    let n = if header[6] & 1 == 1 {
                        u32::from_le_bytes(size)
                    } else {
                        u32::from_be_bytes(size)
                    } as usize;
                    let mut frame = header.to_vec();
                    frame.resize(12 + n, 0);
                    if s.read_exact(&mut frame[12..]).is_err() {
                        break;
                    }
                    let Ok((Message::Request(request), _, _)) = decode_message_ctx(&frame) else {
                        continue;
                    };
                    let reply = Message::Reply(Reply {
                        request_id: request.request_id,
                        reply_status: ReplyStatusType::NoException,
                        service_context: ServiceContextList(vec![ServiceContext {
                            context_id: 0x4f4d_0001,
                            context_data: vec![1, 2, 3, 4],
                        }]),
                        body: Vec::new(),
                    });
                    let mut out = encode_message(version, e, false, &reply).unwrap();
                    let offset = out.len();
                    seen.lock().unwrap().push(offset);
                    let mut w = zerodds_corba_giop::body_writer(offset, e);
                    w.write_f64(DOUBLE);
                    out.extend_from_slice(&zerodds_corba_giop::finish_body(w, offset));
                    let size = u32::try_from(out.len() - 12).unwrap();
                    out[8..12].copy_from_slice(&match e {
                        Endianness::Big => size.to_be_bytes(),
                        Endianness::Little => size.to_le_bytes(),
                    });
                    let at = position_of(&out, double_bytes_of(DOUBLE, e));
                    assert_eq!(at % 8, 0, "the peer aligns from the message start");
                    if s.write_all(&out).is_err() {
                        break;
                    }
                }
            });
        }
    });
    (port, offsets)
}

/// Every reply body of a [`double_reply_peer`] stood on four modulo eight.
fn assert_on_four_modulo_eight(offsets: &Mutex<Vec<usize>>) {
    let offsets = offsets.lock().unwrap().clone();
    assert!(!offsets.is_empty());
    assert!(
        offsets.iter().all(|o| o % 8 == 4),
        "the reply bodies stood at {offsets:?}"
    );
}

/// CORBA 3.3 Part 2 §9.4.3, OPEN-ITEMS 133 for the stubs: another
/// ORB's GIOP 1.0/1.1 reply puts its body right after the reply header,
/// here on four modulo eight (one reply service context of four octets),
/// and aligns the returned `double` from the message start. The
/// generated stub reads the reply at the body's offset
/// (`CorbaConnection::invoke_aligned`) and gets the value; read from the
/// body's first octet (`invoke`), the same octets give another one.
#[test]
fn corba_p2_9_4_3_a_reply_body_of_another_orb_on_four_modulo_eight_is_read_at_its_offset() {
    use zerodds_corba_giop::Version;
    for version in [Version::V1_0, Version::V1_1] {
        let (port, offsets) = double_reply_peer(version, Endianness::Big);
        let iiop = IiopVersion::new(1, version.minor);
        let target = reference(port, iiop, b"Aligned");

        // The generated stub: the value.
        let stub = AlignedStub::new(target.clone(), Arc::new(IiopCorbaConnection::new()));
        assert_eq!(
            stub.after_octet_double(TAG, DOUBLE).unwrap(),
            DOUBLE,
            "GIOP {version:?}"
        );

        // The body read from its first octet: four octets of padding and
        // half the value.
        let (body, e) = IiopCorbaConnection::new()
            .invoke(
                &target,
                "after_octet_double",
                Endianness::Big,
                &zerodds_corba_rust::no_arguments,
            )
            .unwrap();
        let from_zero = Reader::new(&body, e).read_f64().unwrap();
        assert_ne!(from_zero.to_bits(), DOUBLE.to_bits(), "GIOP {version:?}");

        assert_on_four_modulo_eight(&offsets);
    }
}

/// The calls of an [`AlignedAmiHandler`]: the result, or the exception.
#[derive(Default)]
struct AmiResults(Mutex<Vec<Result<f64, CorbaException>>>);

impl AlignedAmiHandler for AmiResults {
    fn after_octet_double(&self, __return: f64) {
        self.0.lock().unwrap().push(Ok(__return));
    }
    fn after_octet_double_excep(&self, __excep: CorbaException) {
        self.0.lock().unwrap().push(Err(__excep));
    }
}

/// CORBA 3.3 Part 2 §9.4.3 and CORBA Messaging §22, OPEN-ITEMS 133 for
/// AMI: another ORB's GIOP 1.0/1.1 reply with its body on four modulo
/// eight, in either byte order. The generated poller of
/// `sendp_after_octet_double` and the reply handler of
/// `sendc_after_octet_double` get the `double` the peer aligned from the
/// message start; the `AsyncReply` of the `AmiClient` keeps the body's
/// offset and the reply's byte order, and read from the body's first
/// octet the same octets give another value.
#[test]
fn corba_p2_9_4_3_an_ami_reply_of_another_orb_on_four_modulo_eight_is_read_at_its_offset() {
    use zerodds_corba_giop::Version;
    for version in [Version::V1_0, Version::V1_1] {
        for e in [Endianness::Big, Endianness::Little] {
            let (port, offsets) = double_reply_peer(version, e);
            let target = reference(port, IiopVersion::new(1, version.minor), b"Aligned");
            let stub = AlignedStub::new(target.clone(), Arc::new(IiopCorbaConnection::new()));
            let mut client = AmiClient::connect(&target).unwrap();

            // Polling model (§22.6).
            let poller = stub
                .sendp_after_octet_double(&mut client, TAG, DOUBLE)
                .unwrap();
            let got = poller.get_reply(&mut client).unwrap();
            assert_eq!(got.to_bits(), DOUBLE.to_bits(), "GIOP {version:?} {e:?}");

            // Callback model (§22.5).
            let handler = Arc::new(AmiResults::default());
            stub.sendc_after_octet_double(&mut client, handler.clone(), TAG, DOUBLE)
                .unwrap();
            client.perform_all().unwrap();
            let calls = std::mem::take(&mut *handler.0.lock().unwrap());
            match calls.as_slice() {
                [Ok(v)] => assert_eq!(v.to_bits(), DOUBLE.to_bits(), "GIOP {version:?} {e:?}"),
                other => panic!("GIOP {version:?} {e:?}: {other:?}"),
            }

            // The channel's reply: the body at its offset, the reply's
            // byte order.
            let id = client
                .send_poll("after_octet_double", &zerodds_corba_rust::no_arguments)
                .unwrap();
            let (body, reply_e) = client.get_reply(id).unwrap().unwrap();
            assert_eq!(body.offset() % 8, 4, "GIOP {version:?} {e:?}");
            assert_eq!(reply_e, e);
            let from_zero = Reader::new(body.bytes(), reply_e).read_f64().unwrap();
            assert_ne!(
                from_zero.to_bits(),
                DOUBLE.to_bits(),
                "GIOP {version:?} {e:?}"
            );

            assert_on_four_modulo_eight(&offsets);
        }
    }
}

/// CORBA 3.3 Part 2 §9.4.3 through the DII (Part 1 §7.2), OPEN-ITEMS 133:
/// the result of another ORB's GIOP 1.0/1.1 reply on four modulo eight,
/// in either byte order. `Request::invoke` keeps the body's offset and the
/// reply's byte order; read through `result_body`, the `double` is the
/// peer's.
#[test]
fn corba_p2_9_4_3_a_dii_result_of_another_orb_on_four_modulo_eight_is_read_at_its_offset() {
    use zerodds_corba_giop::Version;
    for version in [Version::V1_0, Version::V1_1] {
        for e in [Endianness::Big, Endianness::Little] {
            let (port, offsets) = double_reply_peer(version, e);
            let target = reference(port, IiopVersion::new(1, version.minor), b"Aligned");
            let mut request = zerodds_corba_ccm::dynamic_api::Request::new("after_octet_double");
            assert!(request.result_body().is_none());
            request
                .invoke(&IiopCorbaConnection::new(), &target, Endianness::Big)
                .unwrap();
            assert_eq!(request.result_offset % 8, 4, "GIOP {version:?} {e:?}");
            assert_eq!(request.result_endianness, e);
            let result = request.result_body().unwrap();
            let got = result.reader(request.result_endianness).read_f64().unwrap();
            assert_eq!(got.to_bits(), DOUBLE.to_bits(), "GIOP {version:?} {e:?}");
            assert_on_four_modulo_eight(&offsets);
        }
    }
}

/// One call as a POA servant or a DSI servant read it: the operation,
/// the body's offset in the message, the octet and the value.
type ReadCall = (String, usize, u8, i64, f64);
/// Every such call.
type Read = Arc<Mutex<Vec<ReadCall>>>;

/// A POA servant (`zerodds_corba_poa::Servant`) of the operations of
/// `Aligned`, reading its arguments as `Servant::invoke` hands them.
#[derive(Debug)]
struct PoaAligned(Read);

/// Reads `after_octet` (`octet`, `long long`) or `after_octet_double`
/// (`octet`, `double`) at the body's offset and writes the value back.
fn read_after_octet(
    operation: &str,
    body: &zerodds_corba_giop::AlignedBody,
    e: Endianness,
    read: &Mutex<Vec<ReadCall>>,
) -> Vec<u8> {
    let mut r = body.reader(e);
    let mut w = zerodds_wire::Writer::new(e);
    let Ok(tag) = r.read_u8() else {
        return Vec::new();
    };
    match operation {
        "after_octet" => {
            let Ok(value) = r.read_i64() else {
                return Vec::new();
            };
            read.lock()
                .unwrap()
                .push((operation.into(), body.offset(), tag, value, 0.0));
            w.write_i64(value);
        }
        "after_octet_double" => {
            let Ok(value) = r.read_f64() else {
                return Vec::new();
            };
            read.lock()
                .unwrap()
                .push((operation.into(), body.offset(), tag, 0, value));
            w.write_f64(value);
        }
        _ => return Vec::new(),
    }
    w.finish()
}

impl zerodds_corba_poa::Servant for PoaAligned {
    fn primary_interface(&self) -> String {
        "IDL:Aligned:1.0".into()
    }

    fn invoke(
        &self,
        operation: &str,
        body: &zerodds_corba_giop::AlignedBody,
        e: Endianness,
    ) -> Vec<u8> {
        read_after_octet(operation, body, e, &self.0)
    }
}

/// A DSI servant of the same operations, reading what `dispatch_dsi`
/// puts in the `ServerRequest`.
struct DsiAligned(Read);

impl zerodds_corba_ccm::dynamic_api::DsiServant for DsiAligned {
    fn dynamic_invoke(&self, req: &mut zerodds_corba_ccm::dynamic_api::ServerRequest) {
        let reply = read_after_octet(&req.operation, &req.input(), req.endianness, &self.0);
        req.set_result(reply);
    }
    fn primary_interface(&self) -> String {
        "IDL:Aligned:1.0".into()
    }
}

/// Calls `after_octet` from GIOP 1.1 and `after_octet_double` in every
/// version over IIOP 1.0, 1.1 and 1.2 profiles of both keys, in both
/// byte orders, and checks the values that came back.
fn call_both_keys(port: u16) -> usize {
    let conn = IiopCorbaConnection::new();
    let mut calls = 0;
    for key in [&b"Aligned"[..], &b"Aligned-key"[..]] {
        for version in [IiopVersion::V1_0, IiopVersion::V1_1, IiopVersion::V1_2] {
            let target = reference(port, version, key);
            for e in [Endianness::Big, Endianness::Little] {
                let (reply, re) = conn
                    .invoke(&target, "after_octet_double", e, &|w| {
                        w.write_u8(TAG);
                        w.write_f64(DOUBLE);
                        Ok(())
                    })
                    .unwrap();
                assert_eq!(
                    Reader::new(&reply, re).read_f64().unwrap().to_bits(),
                    DOUBLE.to_bits(),
                    "{version:?} {e:?}"
                );
                calls += 1;
                // `long long` is a type of GIOP 1.1 and later (Table 7.2).
                if version != IiopVersion::V1_0 {
                    let (reply, re) = conn
                        .invoke(&target, "after_octet", e, &|w| {
                            w.write_u8(TAG);
                            w.write_i64(LONG_LONG);
                            Ok(())
                        })
                        .unwrap();
                    assert_eq!(
                        Reader::new(&reply, re).read_i64().unwrap(),
                        LONG_LONG,
                        "{version:?} {e:?}"
                    );
                    calls += 1;
                }
            }
        }
    }
    calls
}

/// The servant read every value right, from bodies at offsets 0 and 4
/// modulo 8 (where a reader from the body's first octet would have read
/// four octets of padding as part of the value).
fn assert_read_right(read: &Read, calls: usize) {
    let read = read.lock().unwrap();
    assert_eq!(read.len(), calls, "{read:?}");
    let mut residues = std::collections::BTreeSet::new();
    for (operation, offset, tag, long_long, double) in read.iter() {
        assert_eq!(*tag, TAG, "{operation} at {offset}");
        match operation.as_str() {
            "after_octet" => assert_eq!(*long_long, LONG_LONG, "at {offset}"),
            _ => assert_eq!(double.to_bits(), DOUBLE.to_bits(), "at {offset}"),
        }
        residues.insert(offset % 8);
    }
    assert_eq!(
        residues,
        std::collections::BTreeSet::from([0, 4]),
        "body offsets modulo 8"
    );
}

/// OPEN-ITEMS 133, CORBA 3.3 Part 2 §9.4.2.1: a POA servant
/// (`Poa::dispatch`, `Servant::invoke`, here behind
/// `zerodds_corba_interop::serve`) gets the request body at its offset
/// in the message and reads an 8-octet argument after an octet there,
/// under GIOP 1.0, 1.1 and 1.2, in both byte orders. The reply goes out
/// in the version of the request.
#[test]
fn corba_p2_9_4_2_1_a_poa_servant_reads_an_8_octet_argument_at_the_body_offset() {
    use zerodds_corba_poa::{IdAssignmentPolicy, Poa, PoaConfig, PoaManager, PolicySet, Servant};
    let read: Read = Arc::default();
    let manager = Arc::new(PoaManager::new());
    manager.activate().unwrap();
    let poa = Poa::new(PoaConfig {
        adapter_name: "RootPOA".to_string(),
        policies: PolicySet {
            id_assignment: IdAssignmentPolicy::User,
            ..PolicySet::default()
        },
        manager,
    })
    .unwrap();
    for key in [&b"Aligned"[..], &b"Aligned-key"[..]] {
        let servant: Box<dyn Servant> = Box::new(PoaAligned(Arc::clone(&read)));
        poa.activate_object_with_id(key.into(), servant).unwrap();
    }
    let acceptor =
        zerodds_corba_interop::serve("127.0.0.1:0".parse().unwrap(), Arc::new(poa)).unwrap();
    let calls = call_both_keys(acceptor.listen_addr().port());
    acceptor.shutdown();
    assert_read_right(&read, calls);
}

/// OPEN-ITEMS 133 for the DSI: `dispatch_dsi` puts the offset and the
/// byte order of the request body into the `ServerRequest`, and a
/// dynamic servant reading `input()` gets the 8-octet argument after an
/// octet right, under GIOP 1.0, 1.1 and 1.2.
#[test]
fn corba_p2_9_4_2_1_a_dsi_servant_reads_an_8_octet_argument_at_the_body_offset() {
    let read: Read = Arc::default();
    let server = CorbaServer::new();
    for key in [&b"Aligned"[..], &b"Aligned-key"[..]] {
        let dsi = Arc::new(DsiAligned(Arc::clone(&read)));
        server.register(key, move |op, body, e| {
            zerodds_corba_ccm::dynamic_api::dispatch_dsi(&*dsi, op, body, e)
        });
    }
    let acceptor = server.serve("127.0.0.1:0".parse().unwrap()).unwrap();
    let calls = call_both_keys(acceptor.listen_addr().port());
    acceptor.shutdown();
    assert_read_right(&read, calls);
}

/// The repository id of `AlignedLate`.
const LATE_ID: &str = "IDL:AlignedLate:1.0";

/// The body of an `AlignedLate` written for `offset` in its message.
fn late_body(offset: usize, e: Endianness) -> Vec<u8> {
    let mut w = zerodds_corba_giop::body_writer(offset, e);
    w.write_string(LATE_ID);
    w.write_i64(LONG_LONG);
    w.write_f64(DOUBLE);
    zerodds_corba_giop::finish_body(w, offset)
}

/// CORBA 3.3 Part 2 §9.4.3, OPEN-ITEMS 133 for user exceptions: another
/// ORB's GIOP 1.0/1.1 `USER_EXCEPTION` reply puts its body on four
/// modulo eight (a reply service context of four octets), and the
/// `long long` and `double` after the repository id are aligned from the
/// message start. The generated stub reads them at the body's offset;
/// the same body read from its first octet gives other values.
#[test]
fn corba_p2_9_4_3_a_user_exception_of_another_orb_on_four_modulo_eight_is_read_at_its_offset() {
    use std::io::{Read, Write};
    use zerodds_corba_giop::{
        Message, Reply, ReplyStatusType, ServiceContext, ServiceContextList, Version,
        decode_message_ctx, encode_message,
    };
    for version in [Version::V1_0, Version::V1_1] {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let offsets = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&offsets);
        // A peer that answers every request with the exception, its body
        // right after the reply header. It runs until the test ends.
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut s) = stream else {
                    break;
                };
                let seen = Arc::clone(&seen);
                std::thread::spawn(move || {
                    loop {
                        let mut header = [0u8; 12];
                        if s.read_exact(&mut header).is_err() {
                            break;
                        }
                        let n = u32::from_be_bytes([header[8], header[9], header[10], header[11]])
                            as usize;
                        let mut frame = header.to_vec();
                        frame.resize(12 + n, 0);
                        s.read_exact(&mut frame[12..]).unwrap();
                        let Ok((Message::Request(request), _, _)) = decode_message_ctx(&frame)
                        else {
                            continue;
                        };
                        let reply = Message::Reply(Reply {
                            request_id: request.request_id,
                            reply_status: ReplyStatusType::UserException,
                            service_context: ServiceContextList(vec![ServiceContext {
                                context_id: 0x4f4d_0001,
                                context_data: vec![1, 2, 3, 4],
                            }]),
                            body: Vec::new(),
                        });
                        let mut out =
                            encode_message(version, Endianness::Big, false, &reply).unwrap();
                        let offset = out.len();
                        seen.lock().unwrap().push(offset);
                        out.extend_from_slice(&late_body(offset, Endianness::Big));
                        let size = u32::try_from(out.len() - 12).unwrap();
                        out[8..12].copy_from_slice(&size.to_be_bytes());
                        let at = position_of(&out, bytes_of(LONG_LONG, Endianness::Big));
                        assert_eq!(at % 8, 0, "the peer aligns from the message start");
                        s.write_all(&out).unwrap();
                    }
                });
            }
        });
        let target = reference(port, IiopVersion::new(1, version.minor), b"Aligned");

        // The generated stub: the members.
        let stub = AlignedStub::new(target.clone(), Arc::new(IiopCorbaConnection::new()));
        match stub.late(LONG_LONG) {
            Err(AlignedError::AlignedLate(late)) => {
                assert_eq!(late.value, LONG_LONG, "GIOP {version:?}");
                assert_eq!(late.ratio.to_bits(), DOUBLE.to_bits(), "GIOP {version:?}");
            }
            other => panic!("GIOP {version:?}: {other:?}"),
        }

        // The exception through the connection: its body keeps the offset;
        // read from its first octet, the `long long` is another value.
        let err = IiopCorbaConnection::new()
            .invoke(
                &target,
                "late",
                Endianness::Big,
                &zerodds_corba_rust::no_arguments,
            )
            .unwrap_err();
        let CorbaException::UserException {
            repository_id,
            body,
            endianness,
        } = err
        else {
            panic!("GIOP {version:?}: {err:?}");
        };
        assert_eq!(repository_id, LATE_ID);
        assert_eq!(body.offset() % 8, 4, "GIOP {version:?}");
        let mut from_zero = Reader::new(body.bytes(), endianness);
        from_zero.read_string().unwrap();
        assert_ne!(from_zero.read_i64().unwrap(), LONG_LONG, "GIOP {version:?}");

        let offsets = offsets.lock().unwrap().clone();
        assert!(!offsets.is_empty());
        assert!(
            offsets.iter().all(|o| o % 8 == 4),
            "the reply bodies stood at {offsets:?}"
        );
    }
}

/// CORBA 3.3 Part 2 §9.4.3: a servant that passes on a user exception it
/// received in a GIOP 1.0/1.1 reply at four modulo eight. `CorbaServer`
/// writes the body on four modulo eight of its own 1.0/1.1 reply, where
/// the members stay aligned, and the stub reads them; a GIOP 1.2 reply,
/// whose body is on eight, cannot carry it and is `MARSHAL`,
/// `COMPLETED_YES`.
#[test]
fn corba_p2_9_4_3_a_relayed_user_exception_keeps_its_offset_modulo_eight() {
    let server = CorbaServer::new();
    server.register(b"Aligned", |_op, _body, e| {
        SkeletonResult::Exception(CorbaException::UserException {
            repository_id: LATE_ID.into(),
            body: zerodds_corba_rust::AlignedBody::new(&late_body(36, e), 36),
            endianness: e,
        })
    });
    let acceptor = server.serve("127.0.0.1:0".parse().unwrap()).unwrap();
    let port = acceptor.listen_addr().port();
    for version in [IiopVersion::V1_0, IiopVersion::V1_1] {
        let stub = AlignedStub::new(
            reference(port, version, b"Aligned"),
            Arc::new(IiopCorbaConnection::new()),
        );
        match stub.late(LONG_LONG) {
            Err(AlignedError::AlignedLate(late)) => {
                assert_eq!(late.value, LONG_LONG, "IIOP {version:?}");
                assert_eq!(late.ratio.to_bits(), DOUBLE.to_bits(), "IIOP {version:?}");
            }
            other => panic!("IIOP {version:?}: {other:?}"),
        }
    }
    let stub = AlignedStub::new(
        reference(port, IiopVersion::V1_2, b"Aligned"),
        Arc::new(IiopCorbaConnection::new()),
    );
    match stub.late(LONG_LONG) {
        Err(AlignedError::System(CorbaException::System(e))) => {
            assert_eq!(e.kind, zerodds_corba_rust::SystemExceptionKind::Marshal);
            assert_eq!(e.completed, zerodds_corba_rust::CompletionStatus::Yes);
        }
        other => panic!("IIOP 1.2: {other:?}"),
    }
    acceptor.shutdown();
}

/// The other byte order.
fn other_order(e: Endianness) -> Endianness {
    match e {
        Endianness::Big => Endianness::Little,
        Endianness::Little => Endianness::Big,
    }
}

/// A server request interceptor that only exists: with it, `CorbaServer`
/// builds the reply through the interceptors.
struct PassThrough;

impl zerodds_corba_ccm::portable_interceptor::ServerRequestInterceptor for PassThrough {
    fn name(&self) -> &str {
        "pass-through"
    }
}

struct PassThroughInit;

impl zerodds_corba_ccm::portable_interceptor::OrbInitializer for PassThroughInit {
    fn pre_init(
        &self,
        info: &mut zerodds_corba_ccm::portable_interceptor::OrbInitInfo<'_>,
    ) -> Result<(), zerodds_corba_ccm::portable_interceptor::InitializerError> {
        if info.orb_id() == "own-byte-order" {
            info.add_server_request_interceptor(Arc::new(PassThrough))?;
        }
        Ok(())
    }
}

/// CORBA 3.3 Part 2 §9.4.1, §9.4.3: a servant passes on a user exception
/// it received in the other byte order than the one of the request. The
/// octets of its body cannot be read in another byte order: `CorbaServer`
/// writes the reply in the exception's byte order, on its offset modulo
/// eight, and the generated stub reads the members, under GIOP 1.0, 1.1
/// and 1.2, without and with a server request interceptor.
#[test]
fn corba_p2_9_4_3_a_relayed_user_exception_goes_out_in_its_own_byte_order() {
    zerodds_corba_ccm::portable_interceptor::register_orb_initializer(Arc::new(PassThroughInit));
    for intercepted in [false, true] {
        let server = CorbaServer::new();
        if intercepted {
            server.set_orb(
                zerodds_corba_interop::runtime::orb_init(&mut Vec::new(), "own-byte-order")
                    .unwrap(),
            );
        }
        let orders = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&orders);
        server.register(b"Aligned", move |_op, _body, e| {
            let other = other_order(e);
            seen.lock().unwrap().push((e, other));
            SkeletonResult::Exception(CorbaException::UserException {
                repository_id: LATE_ID.into(),
                body: zerodds_corba_rust::AlignedBody::new(&late_body(40, other), 40),
                endianness: other,
            })
        });
        let acceptor = server.serve("127.0.0.1:0".parse().unwrap()).unwrap();
        let port = acceptor.listen_addr().port();
        for version in [IiopVersion::V1_0, IiopVersion::V1_1, IiopVersion::V1_2] {
            let stub = AlignedStub::new(
                reference(port, version, b"Aligned"),
                Arc::new(IiopCorbaConnection::new()),
            );
            match stub.late(LONG_LONG) {
                Err(AlignedError::AlignedLate(late)) => {
                    assert_eq!(late.value, LONG_LONG, "IIOP {version:?} {intercepted}");
                    assert_eq!(
                        late.ratio.to_bits(),
                        DOUBLE.to_bits(),
                        "IIOP {version:?} {intercepted}"
                    );
                }
                other => panic!("IIOP {version:?} {intercepted}: {other:?}"),
            }
        }
        // The servant answered in the other byte order each time.
        let orders = orders.lock().unwrap().clone();
        assert_eq!(orders.len(), 3);
        assert!(
            orders
                .iter()
                .all(|(request, exception)| request != exception)
        );
        acceptor.shutdown();
    }
}
