// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Feeds `endpoints/testdata/xrce-client/xrce_client_*.bin` — the six
//! fixtures every `endpoints/{ada,ada-83,c,cpp,csharp,d,elixir,fsharp,go,
//! java,julia,kotlin,lua,nim,node,ocaml,python,swift,typescript,zig}`
//! port (and `endpoints/xrce-agent-demo`) checks its own XRCE-client
//! encoder against — into this crate's own decoders, proving the agent
//! side of `zerodds-xrce`/`zerodds-xrce-agent` accepts exactly what those
//! ports' shared client fixtures carry.
//!
//! `endpoints/golden-gen`'s `xrce_client_fixtures` module produces these
//! bytes by actually running `zerodds_xrce_client::XrceClient`; this test
//! is the other end of that same wire — it does not regenerate anything,
//! it only decodes and, where the plain (non-`daemon`) [`XrceAgent`] has
//! a matching operation, drives that operation with the decoded fields.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::path::PathBuf;

use zerodds_xrce::encoding::Endianness;
use zerodds_xrce::handshake::{DEFAULT_MTU, XRCE_VERSION};
use zerodds_xrce::header::{ClientKey, StreamId};
use zerodds_xrce::object_id::ObjectId;
use zerodds_xrce::object_info::BaseObjectRequest;
use zerodds_xrce::object_kind::ObjectKind;
use zerodds_xrce::object_repr::ObjectVariant;
use zerodds_xrce::object_store::{CreateOutcome, CreationMode};
use zerodds_xrce::submessages::{
    CreateClientPayload, CreatePayload, DataFormat, DeletePayload, Message, ReadDataPayload,
    WriteDataPayload,
};

use zerodds_xrce_agent::XrceAgent;

fn fixture(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../endpoints/testdata/xrce-client")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn client_key() -> ClientKey {
    ClientKey([0x01, 0x02, 0x03, 0x04])
}

/// DDS-XRCE 1.0 §7.8.2.1/§8.3.5.1: `CREATE_CLIENT`, `STREAMID_NONE`, and a
/// real `CLIENT_Representation` body (not an ad hoc "cookie+version+key"
/// blob).
#[test]
fn create_client_fixture_decodes_into_a_real_client_representation() {
    let bytes = fixture("xrce_client_create_client.bin");
    let msg = Message::decode(&bytes).expect("decodes as a real XRCE message");
    assert_eq!(msg.header.stream_id, StreamId::NONE);
    assert_eq!(msg.submessages.len(), 1);
    let payload = CreateClientPayload::try_from_submessage(&msg.submessages[0]).expect("payload");
    let rep = payload.representation;
    assert_eq!(rep.version, XRCE_VERSION);
    assert_eq!(rep.vendor_id, [0x00, 0x00]);
    assert_eq!(rep.client_key, client_key());
    assert_eq!(rep.session_id, 0x01);
    assert_eq!(rep.properties, None);
    assert_eq!(rep.requested_mtu, Some(DEFAULT_MTU));
    assert_eq!(
        rep.encode().unwrap(),
        msg.submessages[0].body,
        "the typed codec re-encodes the fixture body octet for octet"
    );
}

/// §7.8.3/§8.3.5.2 `CREATE`: the real two-level `ObjectVariant` (§7.7.3 —
/// outer `ObjectKind` byte + `DATAWRITER_Representation.publisher_id`),
/// and the agent really accepts and stores what it decodes.
#[test]
fn create_fixture_decodes_into_an_object_the_agent_accepts() {
    let bytes = fixture("xrce_client_create.bin");
    let msg = Message::decode(&bytes).expect("decodes");
    assert_eq!(msg.submessages.len(), 1);
    let payload = CreatePayload::try_from_submessage(&msg.submessages[0]).expect("payload");
    let (base, n) = BaseObjectRequest::decode(&payload.representation).expect("base");
    assert_eq!(base.request_id, [0x00, 0x01]);
    assert_eq!(
        base.object_id,
        ObjectId::new(0x001, ObjectKind::DataWriter).unwrap()
    );

    let (kind, variant, extra, consumed) =
        ObjectVariant::decode_for_kind(&payload.representation[n..], Endianness::Little)
            .expect("real two-level ObjectVariant, not a single-level RepresentationFormat blob");
    assert_eq!(kind, ObjectKind::DataWriter);
    assert_eq!(variant, ObjectVariant::ByReference("SensorWriter".into()));
    assert_eq!(
        extra.parent,
        Some(ObjectId::new(0x001, ObjectKind::Publisher).unwrap()),
        "DATAWRITER_Representation carries its publisher_id"
    );
    assert_eq!(consumed, payload.representation.len() - n);

    let mut agent = XrceAgent::new();
    agent.register_client(client_key());
    let outcome = agent
        .create_object(
            client_key(),
            base.object_id,
            variant,
            CreationMode::default(),
        )
        .expect("the agent accepts the decoded CREATE");
    assert_eq!(outcome, CreateOutcome::Created);
}

/// A second `CREATE`, `xrce_client_create_topic.bin`: `OBJK_TOPIC`'s
/// own `..._Representation` carries a trailing `participant_id` exactly
/// like `OBJK_PUBLISHER`'s carries its own parent (commit `31c62df2a63`,
/// `xrce: OBJK_TOPIC_Representation carries its participant's
/// ObjectId`) — checked here against the same kind of independent
/// fixture the `DataWriter` case above already is, so a decoder that
/// only special-cased `DataWriter`/`Publisher` would fail this test.
#[test]
fn create_topic_fixture_decodes_into_an_object_the_agent_accepts() {
    let bytes = fixture("xrce_client_create_topic.bin");
    let msg = Message::decode(&bytes).expect("decodes");
    assert_eq!(msg.submessages.len(), 1);
    let payload = CreatePayload::try_from_submessage(&msg.submessages[0]).expect("payload");
    let (base, n) = BaseObjectRequest::decode(&payload.representation).expect("base");
    assert_eq!(base.request_id, [0x00, 0x05]);
    assert_eq!(
        base.object_id,
        ObjectId::new(0x010, ObjectKind::Topic).unwrap()
    );

    let (kind, variant, extra, consumed) =
        ObjectVariant::decode_for_kind(&payload.representation[n..], Endianness::Little)
            .expect("real two-level ObjectVariant, not a single-level RepresentationFormat blob");
    assert_eq!(kind, ObjectKind::Topic);
    assert_eq!(
        variant,
        ObjectVariant::ByReference("HelloWorldTopic".into())
    );
    assert_eq!(
        extra.parent,
        Some(ObjectId::new(0x001, ObjectKind::Participant).unwrap()),
        "OBJK_TOPIC_Representation carries its participant_id"
    );
    assert_eq!(consumed, payload.representation.len() - n);

    let mut agent = XrceAgent::new();
    agent.register_client(client_key());
    let outcome = agent
        .create_object(
            client_key(),
            base.object_id,
            variant,
            CreationMode::default(),
        )
        .expect("the agent accepts the decoded topic CREATE");
    assert_eq!(outcome, CreateOutcome::Created);
}

/// §7.8.3/§8.3.5.3 `DELETE`: removes the exact object `CREATE` above
/// added.
#[test]
fn delete_fixture_decodes_and_removes_the_object_create_added() {
    let mut agent = XrceAgent::new();
    agent.register_client(client_key());
    let writer = ObjectId::new(0x001, ObjectKind::DataWriter).unwrap();
    agent
        .create_object(
            client_key(),
            writer,
            ObjectVariant::ByReference("SensorWriter".into()),
            CreationMode::default(),
        )
        .expect("seed the object DELETE will remove");

    let bytes = fixture("xrce_client_delete.bin");
    let msg = Message::decode(&bytes).expect("decodes");
    assert_eq!(msg.submessages.len(), 1);
    let payload = DeletePayload::try_from_submessage(&msg.submessages[0]).expect("payload");
    let (base, _) = BaseObjectRequest::decode(&payload.representation).expect("base");
    assert_eq!(base.request_id, [0x00, 0x02]);
    assert_eq!(base.object_id, writer);

    let removed = agent
        .delete_object(client_key(), base.object_id)
        .expect("known client");
    assert!(removed);
}

/// §7.8.5/§8.3.5.9.2 `READ_DATA`: request id, target `DataReader`, and a
/// minimal `ReadSpecification` (no filter, no delivery control).
#[test]
fn read_data_fixture_decodes_into_a_real_read_specification() {
    let bytes = fixture("xrce_client_read_data.bin");
    let msg = Message::decode(&bytes).expect("decodes");
    assert_eq!(msg.submessages.len(), 1);
    let payload = ReadDataPayload::try_from_submessage(&msg.submessages[0]).expect("payload");
    assert_eq!(payload.base.request_id, [0x00, 0x03]);
    assert_eq!(
        payload.base.object_id,
        ObjectId::new(0x002, ObjectKind::DataReader).unwrap()
    );
    assert_eq!(payload.read_specification.preferred_stream_id, 0x01);
    assert_eq!(payload.read_specification.data_format, DataFormat::Data);
    assert!(
        payload
            .read_specification
            .content_filter_expression
            .is_none()
    );
    assert!(payload.read_specification.delivery_control.is_none());
}

/// §7.8.4/§8.3.5.9.1 `WRITE_DATA` (`FORMAT_DATA`): request id, target
/// `DataWriter` (the same one `CREATE`/`DELETE` above use), opaque
/// sample bytes.
#[test]
fn write_data_fixture_decodes_into_the_sample_bytes_the_client_sent() {
    let bytes = fixture("xrce_client_write_data.bin");
    let msg = Message::decode(&bytes).expect("decodes");
    assert_eq!(msg.submessages.len(), 1);
    let payload = WriteDataPayload::try_from_submessage(&msg.submessages[0]).expect("payload");
    assert_eq!(payload.base.request_id, [0x00, 0x04]);
    assert_eq!(
        payload.base.object_id,
        ObjectId::new(0x001, ObjectKind::DataWriter).unwrap()
    );
    assert_eq!(payload.serialized_data, vec![0x19, 0x00]);
}

/// Every fixture's `MessageHeader` carries the same `ClientKey` and
/// `session_id` — a sanity check that the six files are one client's
/// session, not six independently hand-picked headers. The CREATE_CLIENT is
/// the one exception: it is sent where there is no session yet, so its header
/// carries SESSION_ID_NONE_WITH_CLIENT_KEY (0x00, DDS-XRCE 1.0 §8.3.2.1) and
/// the session it asks for (0x01) is in its CLIENT_Representation.
#[test]
fn every_fixture_shares_the_same_client_key_and_session_id() {
    for name in [
        "xrce_client_create_client.bin",
        "xrce_client_create.bin",
        "xrce_client_delete.bin",
        "xrce_client_read_data.bin",
        "xrce_client_write_data.bin",
        "xrce_client_create_topic.bin",
    ] {
        let bytes = fixture(name);
        let msg = Message::decode(&bytes).unwrap_or_else(|e| panic!("{name} decodes: {e:?}"));
        let want = if name == "xrce_client_create_client.bin" {
            0x00
        } else {
            0x01
        };
        assert_eq!(msg.header.session_id.0, want, "{name}");
        assert_eq!(msg.header.client_key, Some(client_key()), "{name}");
    }
}
