// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! E2E test for TypeLookup service wiring (XTypes 1.3 §7.6.3.3.4).
//!
//! Verifies F-DCPS-typelookup-wiring: two `DcpsRuntime`s exchange a
//! TypeObject via the TL_SVC_* builtin endpoints.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use std::sync::Arc;
use std::time::Duration;

use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig};
use zerodds_types::TypeIdentifier;
use zerodds_types::builder::TypeObjectBuilder;
use zerodds_types::type_object::TypeObject;
use zerodds_types::{MinimalTypeObject, PrimitiveKind};

fn sample_type() -> MinimalTypeObject {
    MinimalTypeObject::Struct(
        TypeObjectBuilder::struct_type("::SampleStruct")
            .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
            .member(
                "b",
                TypeIdentifier::Primitive(PrimitiveKind::Float64),
                |m| m,
            )
            .build_minimal(),
    )
}

fn runtime_with_random_port(domain: i32, prefix_byte: u8) -> Arc<DcpsRuntime> {
    use zerodds_rtps::wire_types::GuidPrefix;
    let cfg = RuntimeConfig {
        spdp_period: Duration::from_secs(60),
        ..RuntimeConfig::default()
    };
    let prefix = GuidPrefix::from_bytes([prefix_byte; 12]);
    DcpsRuntime::start(domain, prefix, cfg).expect("runtime start")
}

#[test]
fn announces_type_lookup_endpoints_in_spdp() {
    let rt = runtime_with_random_port(common::unique_domain(), 1);
    let mask = rt.announced_builtin_endpoint_set();
    assert_eq!(
        mask & 0xf000,
        0xf000,
        "all four TypeLookup endpoint roles (bits 12..15) must be announced"
    );
}

#[test]
fn register_type_object_exposes_hash() {
    let rt = runtime_with_random_port(common::unique_domain(), 2);
    let obj = TypeObject::Minimal(sample_type());
    let hash = rt.register_type_object(obj.clone()).expect("register");
    // Hash should be deterministic.
    let hash2 = rt.register_type_object(obj).expect("re-register");
    assert_eq!(hash, hash2, "deterministic hash");
}

#[test]
fn type_lookup_request_to_unknown_peer_returns_none() {
    let rt = runtime_with_random_port(common::unique_domain(), 3);
    use zerodds_rtps::wire_types::GuidPrefix;
    let unknown = GuidPrefix::from_bytes([0xAA; 12]);
    let result = rt
        .send_type_lookup_request(unknown, &[])
        .expect("call returns Result");
    assert!(result.is_none(), "unknown peer → None");
}

#[test]
fn server_handles_inbound_get_types_request_via_dispatch() {
    // The server runs the default configuration and is asked the way the
    // measured vendors ask, in CDR2_LE. It answers in the default encoding.
    let (payload, hash) = get_types_reply(
        RuntimeConfig::default(),
        zerodds_types::type_lookup::TypeLookupEncoding::Xcdr2,
    );
    assert_eq!(
        &payload[..4],
        &[0, 7, 0, 0],
        "CDR2_LE with the FINAL envelope: XTypes 1.4 Beta 1 §7.6.3.3.3"
    );
    let mut reader = zerodds_types::framing::xcdr2_reader(&payload[4..]);
    check_get_types_reply(&mut reader, hash);
}

/// A request in CDR_LE is read like one in CDR2_LE: the identifier names
/// the encoding. The reply keeps the default encoding.
#[test]
fn a_get_types_request_in_cdr_le_is_answered() {
    let (payload, hash) = get_types_reply(
        RuntimeConfig::default(),
        zerodds_types::type_lookup::TypeLookupEncoding::Xcdr1,
    );
    assert_eq!(&payload[..4], &[0, 7, 0, 0]);
    let mut reader = zerodds_types::framing::xcdr2_reader(&payload[4..]);
    check_get_types_reply(&mut reader, hash);
}

/// `wire.type_lookup_encoding = xcdr1` answers in CDR_LE, the reading of
/// XTypes 1.3 Table 60 and DDSI-RTPS 2.5 Table 10.2.
#[test]
fn the_setting_xcdr1_answers_in_cdr_le() {
    let config = RuntimeConfig {
        type_lookup_encoding: zerodds_types::type_lookup::TypeLookupEncoding::Xcdr1,
        ..RuntimeConfig::default()
    };
    let (payload, hash) = get_types_reply(
        config,
        zerodds_types::type_lookup::TypeLookupEncoding::Xcdr2,
    );
    assert_eq!(&payload[..4], &[0, 1, 0, 0]);
    let mut reader =
        zerodds_types::framing::xcdr1_reader(&payload[4..], zerodds_wire::Endianness::Little);
    check_get_types_reply(&mut reader, hash);
}

/// The requests the runtime sends take the same setting: CDR2_LE by
/// default, CDR_LE under `xcdr1`.
#[test]
fn a_request_is_written_in_the_configured_encoding() {
    use zerodds_types::type_lookup::{
        GetTypesRequest, RequestHeader, TypeLookupCall, TypeLookupEncoding, TypeLookupRequest,
    };
    let request = TypeLookupRequest {
        header: RequestHeader::default(),
        data: TypeLookupCall::GetTypes(GetTypesRequest::default()),
    };
    let default = RuntimeConfig::default().type_lookup_encoding;
    assert_eq!(default, TypeLookupEncoding::Xcdr2);
    assert_eq!(
        &request.to_payload(default).expect("encode")[..4],
        &[0, 7, 0, 0]
    );
    assert_eq!(
        &request
            .to_payload(TypeLookupEncoding::Xcdr1)
            .expect("encode")[..4],
        &[0, 1, 0, 0]
    );
}

/// A server runtime under `config` gets a `getTypes` request for a type it
/// registered, written in `request_encoding`, from a plain UDP socket; the
/// serialized payload of its reply and the hash asked for.
fn get_types_reply(
    config: RuntimeConfig,
    request_encoding: zerodds_types::type_lookup::TypeLookupEncoding,
) -> (Vec<u8>, zerodds_types::EquivalenceHash) {
    use zerodds_rtps::datagram::encode_data_datagram;
    use zerodds_rtps::header::RtpsHeader;
    use zerodds_rtps::submessages::DataSubmessage;
    use zerodds_rtps::wire_types::{EntityId, ProtocolVersion, SequenceNumber, VendorId};
    use zerodds_types::type_lookup::{
        GetTypesRequest, RequestHeader, TypeLookupCall, TypeLookupRequest, service_instance_name,
    };

    let config = RuntimeConfig {
        spdp_period: Duration::from_secs(60),
        ..config
    };
    let server_rt = DcpsRuntime::start(
        common::unique_domain(),
        zerodds_rtps::wire_types::GuidPrefix::from_bytes([4; 12]),
        config,
    )
    .expect("runtime start");
    let obj = TypeObject::Minimal(sample_type());
    let hash = server_rt
        .register_type_object(obj)
        .expect("server registers type");

    // Construct a valid GetTypesRequest payload directly.
    let type_ids = vec![TypeIdentifier::EquivalenceHashMinimal(hash)];
    let request_guid = zerodds_rtps::wire_types::Guid::new(
        zerodds_rtps::wire_types::GuidPrefix::from_bytes([42; 12]),
        EntityId::TL_SVC_REQ_WRITER,
    )
    .to_bytes();
    let request = TypeLookupRequest {
        header: RequestHeader {
            writer_guid: request_guid,
            sequence_number: 42,
            instance_name: service_instance_name(&server_rt.guid_prefix.0),
        },
        data: TypeLookupCall::GetTypes(GetTypesRequest { type_ids }),
    };
    let payload = request
        .to_payload(request_encoding)
        .expect("encode request");

    let header = RtpsHeader {
        protocol_version: ProtocolVersion::CURRENT,
        vendor_id: VendorId::ZERODDS,
        guid_prefix: zerodds_rtps::wire_types::GuidPrefix::from_bytes([42; 12]),
    };
    let data = DataSubmessage {
        extra_flags: 0,
        reader_id: EntityId::TL_SVC_REQ_READER,
        writer_id: EntityId::TL_SVC_REQ_WRITER,
        writer_sn: SequenceNumber::from_high_low(0, 1),
        inline_qos: None,
        // A whole sample: §8.3.7.2.5 Tab. 8.43 `D=1, K=0`.
        data_flag: true,
        key_flag: false,
        non_standard_flag: false,
        serialized_payload: zerodds_rtps::payload::Payload::from(payload.into_boxed_slice()),
    };
    let datagram = encode_data_datagram(header, &[data]).expect("encode");

    // The server locator has a 0.0.0.0 bind; construct a real loopback
    // locator with the bound port for the send.
    let local = server_rt.user_locator();
    let port = local.port;
    let socket = std::net::UdpSocket::bind("127.0.0.1:0").expect("peer socket");
    socket
        .set_read_timeout(Some(Duration::from_secs(3)))
        .expect("timeout");
    socket
        .send_to(&datagram, (std::net::Ipv4Addr::LOCALHOST, port as u16))
        .expect("send");
    let mut buffer = [0u8; 65536];
    let (len, _) = socket
        .recv_from(&mut buffer)
        .expect("actual TypeLookup reply");
    let parsed = zerodds_rtps::datagram::decode_datagram(&buffer[..len]).expect("reply datagram");
    let data = parsed
        .submessages
        .iter()
        .find_map(|frame| match &frame.submessage {
            zerodds_rtps::datagram::ParsedSubmessage::Data(data) => Some(data),
            _ => None,
        })
        .expect("reply DATA");
    let mut payload = data.serialized_payload.to_vec();
    server_rt.shutdown();
    // XTypes 1.3 §7.6.3.1.2: the options count the padding to four.
    assert_eq!(payload.len() % 4, 0, "the sample ends four-aligned");
    let padding = usize::from(payload[3] & 0x03);
    payload.truncate(payload.len() - padding);
    (payload, hash)
}

/// The reply [`get_types_reply`] gets: the request's RPC identity, and the
/// one object asked for.
fn check_get_types_reply(
    reader: &mut zerodds_wire::Reader<'_>,
    hash: zerodds_types::EquivalenceHash,
) {
    use zerodds_types::type_lookup::{GetTypesResult, TypeLookupReplyMessage, TypeLookupReturn};
    let request_guid = zerodds_rtps::wire_types::Guid::new(
        zerodds_rtps::wire_types::GuidPrefix::from_bytes([42; 12]),
        zerodds_rtps::wire_types::EntityId::TL_SVC_REQ_WRITER,
    )
    .to_bytes();
    let reply = TypeLookupReplyMessage::decode_from(reader).expect("standard RPC reply");
    assert_eq!(reader.remaining(), 0);
    assert_eq!(reply.header.writer_guid, request_guid);
    assert_eq!(
        reply.header.sequence_number, 42,
        "RPC identity is not DATA sequence 1"
    );
    assert_eq!(reply.header.remote_ex, 0);
    let TypeLookupReturn::GetTypes(GetTypesResult::Ok(result)) = reply.ret else {
        panic!("wrong operation/result");
    };
    assert_eq!(result.types.len(), 1);
    assert_eq!(
        result.types[0].type_identifier,
        TypeIdentifier::EquivalenceHashMinimal(hash)
    );
    assert_eq!(
        result.types[0].type_object,
        TypeObject::Minimal(sample_type())
    );
}

/// Leases a DDS domain no other process on this host holds — the
/// domain number is the UDP port number, so it is a host-wide resource.
#[path = "common/mod.rs"]
mod common;
