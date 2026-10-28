// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Regenerates `endpoints/testdata/xrce-client/xrce_client_*.bin`.
//!
//! The files are committed, and tests compare against them. They are written
//! only on request: `cargo run -p zerodds-endpoint-golden --
//! --write-xrce-client-fixtures [<dir>]`. A run without the flag (what the
//! endpoint tests do to get the other golden files) never touches them. The
//! test `committed_fixtures_equal_what_the_code_produces` fails when the
//! committed files and the code differ; the way out is that command and a
//! commit of the files, on purpose.
//!
//! Every `endpoints/{ada,ada-83,c,cpp,csharp,d,elixir,fsharp,go,java,
//! julia,kotlin,lua,nim,node,ocaml,python,swift,typescript,zig}` port and
//! `endpoints/xrce-agent-demo` checks its own XRCE-client encoder against
//! these six files. Since commit `939faa45298` the Rust endpoint
//! (`endpoints/rust/src/xrce_client.rs`) is a type alias over the real
//! `crates/xrce-client` (`zerodds-xrce-client`) — so the fixtures below
//! are produced by *running* that crate's [`XrceClient`] state machine
//! through a real connect + CREATE + DELETE + READ_DATA + WRITE_DATA
//! session, via a capturing [`ClientTransport`], and writing back exactly
//! the bytes it put on the wire. Nothing here hand-assembles a
//! submessage: every byte is what `zerodds_xrce_client::XrceClient` and
//! the `zerodds_xrce` codec it delegates to actually produced.
//!
//! ## Session script and clause mapping
//!
//! `ClientKey([0x01, 0x02, 0x03, 0x04])`, `session_id = 0x01` (`<= 127`,
//! so every `MessageHeader` after this carries the ClientKey — Spec
//! §8.3.2.1). The CREATE_CLIENT itself is sent where there is no session
//! yet: its header carries `SESSION_ID_NONE_WITH_CLIENT_KEY` (0x00) and the
//! session it asks for is in the `CLIENT_Representation`.
//!
//! 1. **`xrce_client_create_client.bin`** — [`XrceClient::connect`]
//!    (DDS-XRCE 1.0 §7.8.2.1/§8.3.5.1, `CREATE_CLIENT`). Travels on
//!    `STREAMID_NONE`, `sequence_nr = 0` (there is no session yet to run
//!    reliability on — see `XrceClient::send_control`'s doc). The body is
//!    a real `CLIENT_Representation` (`crates/xrce/src/handshake.rs`,
//!    byte-oracled against eProsima Micro-XRCE-DDS 3.0.1's own
//!    `CREATE_CLIENT` in `tools/wire-capture/xrce-sessions/
//!    client-to-agent.hex` line 1) — cookie `"XRCE"` + version `{1,0}` +
//!    vendor `{0,0}` + this `client_key` + `session_id` + no properties +
//!    `requested_mtu = Some(508)` — not the single ASCII-cookie-plus-key
//!    blob a previous generator wrote by hand.
//!
//!    A synthetic `STATUS_AGENT` (§8.3.5.5) is then fed back through
//!    [`XrceClient::poll`] — the same construction
//!    `zerodds-xrce-client`'s own `connect_and_ack` test helper uses — so
//!    the client really reaches `ClientState::Connected` before any of
//!    the five operations below runs; the reply itself is not one of the
//!    six recorded fixtures.
//!
//! 2. **`xrce_client_create.bin`** — [`XrceClient::create_object`]
//!    (§7.8.3/§8.3.5.2, `CREATE`), `request_id = 1` (the first request
//!    after the handshake — `CREATE_CLIENT` does not consume a counter
//!    slot). Creates `DataWriter` raw id `0x001` named `"SensorWriter"`
//!    (`ObjectVariant::ByReference`) under `Publisher` raw id `0x001`.
//!    The representation is the real **two-level** wire `ObjectVariant`
//!    (§7.7.3: outer `ObjectKind` byte + inner format-discriminated
//!    payload + the `DATAWRITER_Representation` trailing `publisher_id`)
//!    that `crates/xrce/src/object_repr.rs::encode_for_kind` builds —
//!    byte-oracled there against eProsima's own `CREATE(OBJK_SUBSCRIBER)`
//!    capture — not the single-level `RepresentationFormat` blob (no
//!    outer kind byte, no parent id) a previous version of this module
//!    wrote.
//!
//! 3. **`xrce_client_delete.bin`** — [`XrceClient::delete_object`]
//!    (§7.8.3/§8.3.5.3, `DELETE`), `request_id = 2`, removing the same
//!    `DataWriter` raw id `0x001` `CREATE`d above.
//!
//! 4. **`xrce_client_read_data.bin`** — [`XrceClient::request_read`]
//!    (§7.8.5/§8.3.5.9.2, `READ_DATA`), `request_id = 3`, pulling
//!    `DataReader` raw id `0x002` on `preferred_stream_id = 0x01`,
//!    `DataFormat::Data`, no content filter, no delivery control.
//!
//! 5. **`xrce_client_write_data.bin`** — [`XrceClient::request_write`]
//!    (§7.8.4/§8.3.5.9.1, `WRITE_DATA`, `FORMAT_DATA`), `request_id = 4`,
//!    on the same `DataWriter` raw id `0x001`, payload `[0x19, 0x00]`
//!    (kept from the previous fixture set: an opaque, XRCE-client-facing
//!    `serialized_data` blob the client itself never interprets).
//!
//! 6. **`xrce_client_create_topic.bin`** — a second [`XrceClient::
//!    create_object`] call, `request_id = 5`, appended after fixture 5
//!    so the first four request/sequence numbers above are untouched.
//!    Creates `Topic` raw id `0x010` named `"HelloWorldTopic"`
//!    (`ObjectVariant::ByReference`) under `Participant` raw id `0x001`
//!    — since commit `31c62df2a63` (`xrce: OBJK_TOPIC_Representation
//!    carries its participant's ObjectId`), `OBJK_TOPIC`'s own
//!    `..._Representation` carries a trailing parent `ObjectId` exactly
//!    like a `Publisher`'s or `Subscriber`'s does
//!    (`crates/xrce/src/object_repr.rs`,
//!    `ObjectVariantExtra::has_parent`). This fixture exists so every
//!    language port under `endpoints/` can check its own topic-CREATE
//!    encoding against a real, `XrceClient`-produced oracle rather than
//!    only the pre-existing `DataWriter`/`Publisher` case above, which a
//!    port could satisfy by coincidence without actually reading
//!    `ObjectKind::Topic` as a parent-carrying kind.
//!
//! Fixtures 2-6 are all submitted with `reliable: true`, so
//! `send_maybe_fragmented` puts them on `STREAMID_BUILTIN_RELIABLE`
//! (`0x80`) and — unlike a `reliable: false` call, whose `sequence_nr`
//! this client always leaves at `0` — `ReliableStreamState::submit`
//! hands out a real, strictly increasing sequence number per message:
//! `0` (`CREATE`), `1` (`DELETE`), `2` (`READ_DATA`), `3` (`WRITE_DATA`),
//! `4` (the topic `CREATE`). That increasing sequence is the point of
//! running a real client at all: the fixtures this module replaces put
//! all four original operations on the best-effort stream with
//! hand-picked, non-monotonic sequence numbers — a shape no
//! `reliable: true` (or, for that matter, `reliable: false`) call of
//! this client can produce.
//!
//! These six fixtures are read independently, each as one wire example
//! of its own submessage kind — not as one application-consistent
//! object lifecycle. In particular `xrce_client_write_data.bin` targets
//! the `DataWriter` that `xrce_client_delete.bin` already removed, and
//! the topic `CREATE` names a `Participant` id no fixture ever actually
//! creates; nothing about the wire bytes of any one message depends on
//! another one having run, or not run, first.

use std::path::Path;
use std::time::Duration;

use zerodds_xrce::handshake::AgentRepresentation;
use zerodds_xrce::header::{ClientKey, MessageHeader, SessionId, StreamId};
use zerodds_xrce::object_id::ObjectId;
use zerodds_xrce::object_info::ResultStatus;
use zerodds_xrce::object_kind::ObjectKind;
use zerodds_xrce::object_repr::{ObjectVariant, ObjectVariantExtra};
use zerodds_xrce::serial_number::SerialNumber16;
use zerodds_xrce::submessages::{DataFormat, Message, StatusAgentPayload};
use zerodds_xrce_client::{ClientError, ClientTransport, XrceClient};

/// In-memory [`ClientTransport`]: records every datagram `XrceClient`
/// sends (in send order), and lets [`ack_connect`] queue up one datagram
/// for [`XrceClient::poll`] to read back.
#[derive(Default)]
struct CaptureTransport {
    sent: Vec<Vec<u8>>,
    inbox: Vec<Vec<u8>>,
}

impl ClientTransport for CaptureTransport {
    fn send(&mut self, payload: &[u8]) -> Result<(), ClientError> {
        self.sent.push(payload.to_vec());
        Ok(())
    }

    fn try_recv(&mut self) -> Result<Option<Vec<u8>>, ClientError> {
        Ok(if self.inbox.is_empty() {
            None
        } else {
            Some(self.inbox.remove(0))
        })
    }
}

/// `ClientKey` every endpoint port's own xrce-client fixture test checks
/// against.
fn client_key() -> ClientKey {
    ClientKey([0x01, 0x02, 0x03, 0x04])
}

/// Feeds a real `STATUS_AGENT` (§8.3.5.5) into `client`'s transport and
/// polls it — the same construction `zerodds-xrce-client`'s own
/// `connect_and_ack` unit test uses. This is the only way [`XrceClient`]
/// ever leaves `Connecting` for `Connected`, so every operation recorded
/// after this call is a real, connected client's own encoding, not a
/// hand-waved "assume it is connected".
fn ack_connect(client: &mut XrceClient<CaptureTransport>) {
    let agent_rep = AgentRepresentation {
        timestamp: None,
        version: [0x01, 0x00],
        vendor_id: [0x00, 0x00],
        properties: None,
    }
    .encode()
    .expect("agent representation encodes");
    let mut body = ResultStatus::ok().encode().to_vec();
    body.extend_from_slice(&agent_rep);
    let sm = StatusAgentPayload {
        representation: body,
    }
    .into_submessage()
    .expect("status_agent submessage");
    let header = MessageHeader {
        session_id: SessionId(0x80),
        stream_id: StreamId::NONE,
        sequence_nr: SerialNumber16::new(0),
        client_key: None,
    };
    let msg = Message::new(header, std::vec![sm]).expect("status_agent message");
    client
        .transport_mut()
        .inbox
        .push(msg.encode().expect("encode status_agent"));
    client
        .poll(Duration::ZERO)
        .expect("poll delivers the STATUS_AGENT");
}

/// Runs the session documented in the module doc and returns every
/// datagram `XrceClient` sent, in send order:
/// `[CREATE_CLIENT, CREATE, DELETE, READ_DATA, WRITE_DATA, CREATE_TOPIC]`.
fn run_session() -> [Vec<u8>; 6] {
    let mut client = XrceClient::new(client_key(), 0x01, CaptureTransport::default());
    client.connect([0x01, 0x00]).expect("connect");
    ack_connect(&mut client);

    let writer = ObjectId::new(0x001, ObjectKind::DataWriter).expect("writer id");
    let publisher = ObjectId::new(0x001, ObjectKind::Publisher).expect("publisher id");
    let reader = ObjectId::new(0x002, ObjectKind::DataReader).expect("reader id");
    let topic = ObjectId::new(0x010, ObjectKind::Topic).expect("topic id");
    let participant = ObjectId::new(0x001, ObjectKind::Participant).expect("participant id");

    let r_create = client
        .create_object(
            writer,
            ObjectVariant::ByReference("SensorWriter".into()),
            ObjectVariantExtra {
                domain_id: None,
                parent: Some(publisher),
            },
            false,
            false,
            true,
        )
        .expect("create");
    assert_eq!(
        r_create, 1,
        "CREATE is the first request after the handshake"
    );

    let r_delete = client.delete_object(writer, true).expect("delete");
    assert_eq!(r_delete, 2, "DELETE is the second request");

    let r_read = client
        .request_read(reader, 0x01, DataFormat::Data, None, None, true)
        .expect("read");
    assert_eq!(r_read, 3, "READ_DATA is the third request");

    let r_write = client
        .request_write(writer, &[0x19, 0x00], true)
        .expect("write");
    assert_eq!(r_write, 4, "WRITE_DATA is the fourth request");

    let r_create_topic = client
        .create_object(
            topic,
            ObjectVariant::ByReference("HelloWorldTopic".into()),
            ObjectVariantExtra {
                domain_id: None,
                parent: Some(participant),
            },
            false,
            false,
            true,
        )
        .expect("create topic");
    assert_eq!(
        r_create_topic, 5,
        "the appended topic CREATE is the fifth request"
    );

    let sent = &client.transport_mut().sent;
    assert_eq!(
        sent.len(),
        6,
        "one datagram per operation, incl. CREATE_CLIENT"
    );
    [
        sent[0].clone(),
        sent[1].clone(),
        sent[2].clone(),
        sent[3].clone(),
        sent[4].clone(),
        sent[5].clone(),
    ]
}

/// Writes the six `xrce_client_*.bin` fixtures to
/// `endpoints/testdata/xrce-client/`, resolved relative to this crate's
/// own manifest directory so it does not depend on the caller's `dir`
/// argument (see `main.rs`) or working directory.
pub fn write_all() {
    let dir = fixture_dir();
    for (name, bytes) in fixtures() {
        std::fs::write(dir.join(name), bytes).expect("write xrce_client_*.bin fixture");
    }
    eprintln!("wrote xrce_client_*.bin to {}", dir.display());
}

/// The directory of the committed fixtures.
fn fixture_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../testdata/xrce-client")
}

/// The six fixtures as the current code produces them, by file name.
fn fixtures() -> [(&'static str, Vec<u8>); 6] {
    let [
        create_client,
        create,
        delete,
        read_data,
        write_data,
        create_topic,
    ] = run_session();
    [
        ("xrce_client_create_client.bin", create_client),
        ("xrce_client_create.bin", create),
        ("xrce_client_delete.bin", delete),
        ("xrce_client_read_data.bin", read_data),
        ("xrce_client_write_data.bin", write_data),
        ("xrce_client_create_topic.bin", create_topic),
    ]
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    /// The exact fixtures this module writes, run against the same
    /// decoders `crates/xrce-agent` and the ports use — a change here
    /// that silently drifts the wire form is caught by `cargo test -p
    /// zerodds-endpoint-golden`, not only by a language port's own test
    /// noticing a mismatch later.
    #[test]
    fn session_produces_six_distinct_real_submessages() {
        use zerodds_xrce::SubmessageId;
        let [
            create_client,
            create,
            delete,
            read_data,
            write_data,
            create_topic,
        ] = run_session();
        let ids = [
            (&create_client, SubmessageId::CreateClient),
            (&create, SubmessageId::Create),
            (&delete, SubmessageId::Delete),
            (&read_data, SubmessageId::ReadData),
            (&write_data, SubmessageId::WriteData),
            (&create_topic, SubmessageId::Create),
        ];
        for (bytes, expected) in ids {
            let msg = Message::decode(bytes).expect("decodes as a real XRCE message");
            assert_eq!(msg.submessages.len(), 1);
            assert_eq!(msg.submessages[0].header.submessage_id, expected);
        }
    }

    /// `CREATE`'s representation is the real two-level `ObjectVariant`
    /// (outer `ObjectKind` + `DATAWRITER_Representation.publisher_id`),
    /// not the single-level form the previous generator wrote.
    #[test]
    fn create_fixture_carries_the_two_level_object_variant() {
        use zerodds_xrce::encoding::Endianness;
        use zerodds_xrce::submessages::CreatePayload;
        let [_, create, _, _, _, _] = run_session();
        let msg = Message::decode(&create).expect("decode");
        let payload = CreatePayload::try_from_submessage(&msg.submessages[0]).expect("payload");
        let (base, n) =
            zerodds_xrce::object_info::BaseObjectRequest::decode(&payload.representation)
                .expect("base");
        assert_eq!(base.request_id, [0x00, 0x01]);
        let (kind, variant, extra, consumed) =
            ObjectVariant::decode_for_kind(&payload.representation[n..], Endianness::Little)
                .expect("two-level decode");
        assert_eq!(kind, ObjectKind::DataWriter);
        assert_eq!(variant, ObjectVariant::ByReference("SensorWriter".into()));
        assert_eq!(
            extra.parent,
            Some(ObjectId::new(0x001, ObjectKind::Publisher).unwrap())
        );
        assert_eq!(consumed, payload.representation.len() - n);
    }

    /// `xrce_client_create_topic.bin`'s representation carries its
    /// participant's `ObjectId` as `OBJK_TOPIC_Representation`'s own
    /// trailing field (commit `31c62df2a63`), the same shape a
    /// `Publisher`'s or `Subscriber`'s representation carries — proven
    /// here by decoding the fixture this module actually wrote, not by
    /// re-deriving the rule.
    #[test]
    fn create_topic_fixture_carries_its_participants_object_id() {
        use zerodds_xrce::encoding::Endianness;
        use zerodds_xrce::submessages::CreatePayload;
        let [_, _, _, _, _, create_topic] = run_session();
        let msg = Message::decode(&create_topic).expect("decode");
        let payload = CreatePayload::try_from_submessage(&msg.submessages[0]).expect("payload");
        let (base, n) =
            zerodds_xrce::object_info::BaseObjectRequest::decode(&payload.representation)
                .expect("base");
        assert_eq!(base.request_id, [0x00, 0x05]);
        assert_eq!(
            base.object_id,
            ObjectId::new(0x010, ObjectKind::Topic).unwrap()
        );
        let (kind, variant, extra, consumed) =
            ObjectVariant::decode_for_kind(&payload.representation[n..], Endianness::Little)
                .expect("two-level decode");
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
    }

    /// Every reliable-stream message (everything after `CREATE_CLIENT`)
    /// carries a real, strictly increasing sequence number, on
    /// `STREAMID_BUILTIN_RELIABLE` — not the hand-picked, best-effort
    /// numbering the previous generator wrote.
    #[test]
    fn reliable_operations_get_real_increasing_sequence_numbers() {
        let [_, create, delete, read_data, write_data, create_topic] = run_session();
        let seqs: Vec<u16> = [&create, &delete, &read_data, &write_data, &create_topic]
            .into_iter()
            .map(|bytes| {
                let (header, _) = MessageHeader::read_from(bytes).expect("header");
                assert_eq!(header.stream_id, StreamId::BUILTIN_RELIABLE);
                header.sequence_nr.raw()
            })
            .collect();
        assert_eq!(seqs, std::vec![0, 1, 2, 3, 4]);
    }

    /// A change of the client that changes a byte has to come with the
    /// regenerated fixtures: the committed files equal what the code
    /// produces. Nothing here writes them.
    #[test]
    fn committed_fixtures_equal_what_the_code_produces() {
        let dir = fixture_dir();
        let mut committed: Vec<String> = std::fs::read_dir(&dir)
            .expect("fixture directory")
            .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
            .collect();
        committed.sort();
        let produced = fixtures();
        let mut names: Vec<String> = produced.iter().map(|(n, _)| (*n).to_string()).collect();
        names.sort();
        assert_eq!(committed, names, "files in {}", dir.display());
        for (name, bytes) in produced {
            let on_disk = std::fs::read(dir.join(name)).expect("read fixture");
            assert_eq!(
                on_disk, bytes,
                "{name} differs from the code; regenerate with --write-xrce-client-fixtures"
            );
        }
    }
}
