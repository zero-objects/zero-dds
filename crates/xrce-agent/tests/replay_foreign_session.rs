// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Replays `tools/wire-capture/xrce-sessions/client-to-agent.hex` against
//! [`SessionEngine`] and checks that every `CREATE_CLIENT`/`CREATE`/
//! `DELETE` this crate's engine answers gets the same reply kind, result
//! status code and object id the real **eProsima Micro-XRCE-DDS Agent
//! 3.0.1** gave in `agent-to-client.hex` (see
//! `tools/wire-capture/xrce-sessions/README.md` for provenance — nothing
//! in that directory was written by this tree). `crates/xrce/tests/
//! foreign_frames.rs` already proves the capture decodes at the wire
//! level; this test drives the *session engine* with it.
//!
//! ## Fields excluded from comparison, and why
//!
//! - **`WRITE_DATA`/`READ_DATA` have no reply to correlate.** The real
//!   capture's `STATUS` count (35) equals exactly `CREATE` (30) +
//!   `DELETE` (5) — `WRITE_DATA` (11) and `READ_DATA` (4) get no
//!   synchronous `STATUS` from the real agent at all. DDS-XRCE 1.0 §7.8.4.1
//!   asks for `STATUS_OK` on a successful write, so this engine does answer
//!   a `WRITE_DATA` (and the replay ignores that extra `STATUS`: it is
//!   compared only for the request_ids the capture answers). They are
//!   still replayed through the engine below (so `FRAGMENT` reassembly —
//!   `BigHelloWorld`'s oversized `WRITE_DATA`/`DATA` — runs against real
//!   captured bytes, not just this crate's own synthetic unit tests),
//!   just not correlated to an expected reply.
//! - **`DATA` payload content is excluded.** [`NullDdsPort`] has no DDS
//!   data plane: the real capture's samples cross from one client's
//!   `WRITE_DATA` to a *different* client's `READ_DATA` through the real
//!   agent's own Fast DDS participant, which a `NullDdsPort`-backed
//!   engine structurally cannot reproduce. This matches the task's own
//!   framing — "every request gets the reply kind, status and object
//!   ids" — which does not include sample payload bytes.
//! - **Sequence numbers, `implementation_status`, timestamps.** Sequence
//!   numbers depend on this engine's own send/retransmission history,
//!   not on a different vendor's internal bookkeeping.
//!   `implementation_status` (`ResultStatus`'s second byte, §7.7.7) is
//!   vendor-specific/opaque by the spec's own design — comparing it
//!   across vendors compares noise, not conformance.
//!   `TIMESTAMP`/`TIMESTAMP_REPLY` (512 each, purely periodic pings) are
//!   real wall-clock values from the capture host; this engine's own
//!   wall-clock echo behavior is already covered by
//!   `session.rs`'s unit test
//!   `timestamp_reply_echoes_originate_and_uses_real_wall_clock`, so
//!   nothing here re-asserts a clock value nobody can reproduce.
//! - **`ACKNACK`/`HEARTBEAT` are protocol-timing artifacts**, not
//!   requests with a comparable reply. This engine runs its own
//!   independent `§8.4.14` reliability state machine over the replayed
//!   bytes (real regression coverage for the `FRAGMENT`/sequencing/
//!   retransmission fixes `session.rs`'s own tests cover in isolation),
//!   but its retransmission *timing* is not expected to match the real
//!   agent's.
//!
//! ## Client correlation
//!
//! Every `MessageHeader` in this capture uses `session_id = 0x81`
//! (`>= 128`), so `client_key` never appears on the wire — exactly the
//! "authenticate by source address" mode `zerodds-xrce-agentd`'s own
//! `PeerTable` handles at the transport layer. This test tracks "the
//! most recently `CREATE_CLIENT`'d key" the same way, which is correct
//! because the capture's 8 sessions run strictly one at a time (the
//! `README`'s "run in this order" — never two clients' traffic
//! interleaved) — confirmed by the 8 `CREATE_CLIENT`/`STATUS_AGENT`
//! counts matching the 5 documented example scenarios' 8 total client
//! processes (`SubscribeHelloWorld` + `PublishHelloWorld` = 2,
//! `...BestEffort` = 2, `...BigHelloWorld` = 2, `BinaryEntityCreation`
//! = 1, `TimeSync` = 1). Both streams are split into 8 ordinal segments
//! (client-to-agent bounded by `CREATE_CLIENT`, agent-to-client bounded
//! by `STATUS_AGENT`) so a `request_id` — a small per-client counter —
//! is only ever compared within its own client's segment, never against
//! a different client's request that happens to reuse the same two
//! bytes.

#![cfg(feature = "daemon")]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use zerodds_xrce::object_id::ObjectId;
use zerodds_xrce::object_info::{BaseObjectReply, ResultStatusCode};
use zerodds_xrce::submessages::{
    CreateClientPayload, Message, StatusAgentPayload, StatusPayload, SubmessageId,
};

use zerodds_xrce_agent::daemon::dds_port::{
    ContainerSpec, DdsPort, DdsPortError, Durability, EndpointQos, History, NullDdsPort, ObjectKey,
    SourceTime, TopicSpec, WriteOutcome,
};
use zerodds_xrce_agent::daemon::session::{SessionEngine, SessionEngineConfig, SessionEvent};

fn load_hex(name: &str) -> Vec<Vec<u8>> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/wire-capture/xrce-sessions")
        .join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {path:?}: {e}"))
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(hex_decode)
        .collect()
}

fn hex_decode(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("capture line is valid hex"))
        .collect()
}

/// Splits a decoded datagram stream into ordinal segments — a new
/// segment starts whenever a submessage of kind `boundary` appears (the
/// boundary-carrying message is the first element of its own new
/// segment).
fn segment_by<'a>(
    datagrams: impl Iterator<Item = &'a Vec<u8>>,
    boundary: SubmessageId,
) -> Vec<Vec<Message>> {
    let mut segments: Vec<Vec<Message>> = Vec::new();
    for bytes in datagrams {
        let Ok(msg) = Message::decode(bytes) else {
            continue;
        };
        let starts_new = msg
            .submessages
            .iter()
            .any(|sm| sm.header.submessage_id == boundary);
        if starts_new || segments.is_empty() {
            segments.push(Vec::new());
        }
        segments.last_mut().expect("just pushed").push(msg);
    }
    segments
}

/// What a `STATUS`'s `BaseObjectReply` says, minus the fields excluded
/// per the module doc.
#[derive(Debug, Clone, PartialEq, Eq)]
struct StatusSummary {
    code: ResultStatusCode,
    object_id: ObjectId,
}

/// Extracts the one client segment's correlatable `STATUS` replies,
/// keyed by `request_id` (safe within a single client's own segment —
/// see the module doc).
fn expected_statuses(segment: &[Message]) -> BTreeMap<[u8; 2], StatusSummary> {
    let mut out = BTreeMap::new();
    for msg in segment {
        for sm in &msg.submessages {
            if sm.header.submessage_id != SubmessageId::Status {
                continue;
            }
            let Ok(p) = StatusPayload::try_from_submessage(sm) else {
                continue;
            };
            let Ok((reply, _)) = BaseObjectReply::decode(&p.representation) else {
                continue;
            };
            let summary = StatusSummary {
                code: reply.result.status_code,
                object_id: reply.related_request.object_id,
            };
            if let Some(prev) = out.get(&reply.related_request.request_id) {
                // A request_id repeating within one segment is only ever
                // a real retransmission (see the module doc's note on
                // request_id [0, 2]) -- if a *different* reply ever
                // shows up under the same request_id, two distinct real
                // requests collided and this test's per-segment
                // correlation would silently compare the wrong pair.
                assert_eq!(
                    prev, &summary,
                    "request_id {:?} got two different STATUS replies within one segment \
                     -- can no longer be correlated by request_id alone",
                    reply.related_request.request_id
                );
            }
            out.insert(reply.related_request.request_id, summary);
        }
    }
    out
}

fn expected_status_agent_code(segment: &[Message]) -> Option<ResultStatusCode> {
    for msg in segment {
        for sm in &msg.submessages {
            if sm.header.submessage_id != SubmessageId::StatusAgent {
                continue;
            }
            let Ok(p) = StatusAgentPayload::try_from_submessage(sm) else {
                continue;
            };
            let Ok((result, _)) = p.result_and_position(zerodds_xrce::encoding::Endianness::Little)
            else {
                continue;
            };
            return Some(result.status_code);
        }
    }
    None
}

#[test]
fn replay_matches_the_real_agents_create_client_create_and_delete_replies() {
    let client_to_agent = load_hex("client-to-agent.hex");
    let agent_to_client = load_hex("agent-to-client.hex");
    assert_eq!(
        client_to_agent.len(),
        886,
        "client-to-agent.hex datagram count"
    );
    assert_eq!(
        agent_to_client.len(),
        886,
        "agent-to-client.hex datagram count"
    );

    let client_segments = segment_by(client_to_agent.iter(), SubmessageId::CreateClient);
    let agent_segments = segment_by(agent_to_client.iter(), SubmessageId::StatusAgent);
    assert_eq!(
        client_segments.len(),
        8,
        "8 CREATE_CLIENT-bounded segments (one per example process)"
    );
    assert_eq!(
        agent_segments.len(),
        8,
        "8 STATUS_AGENT-bounded segments, one per client_segments entry"
    );

    let mut engine = SessionEngine::new(
        NullDdsPort::new(),
        SessionEngineConfig {
            // The capture's processes share one ClientKey and session_id.
            restart_on_same_session: true,
            // The captured client names types the agent does not hold; the
            // default of the agent is to refuse such a topic (§7.7.3.7.3).
            forward_unknown_topic_types: true,
            // The captured client sends the profile elements of Fast DDS.
            ignore_unsupported_qos: true,
            // The capture's STATUS_AGENT has the ResultStatus in front.
            status_agent_with_result_status: true,
            ..SessionEngineConfig::default()
        },
    );
    let mut checked_statuses = 0usize;
    let mut checked_status_agents = 0usize;
    let mut now_ms: u64 = 0;

    for (seg_idx, (client_seg, agent_seg)) in client_segments
        .iter()
        .zip(agent_segments.iter())
        .enumerate()
    {
        let expected_hs = expected_status_agent_code(agent_seg);
        let expected = expected_statuses(agent_seg);

        let mut current_client: Option<[u8; 4]> = None;
        let mut actual_hs: Option<ResultStatusCode> = None;
        let mut actual: BTreeMap<[u8; 2], StatusSummary> = BTreeMap::new();

        for msg in client_seg {
            for sm in &msg.submessages {
                if sm.header.submessage_id == SubmessageId::CreateClient {
                    if let Ok(p) = CreateClientPayload::try_from_submessage(sm) {
                        current_client = Some(p.representation.client_key.0);
                    }
                }
            }
            let resolved = msg.header.client_key.map(|k| k.0).or(current_client);
            let now = Duration::from_millis(now_ms);
            now_ms += 1;
            let out = engine.handle_message(resolved, &msg.header, &msg.submessages, now);
            for ob in &out {
                for sm in &ob.submessages {
                    match sm.header.submessage_id {
                        SubmessageId::StatusAgent => {
                            if let Ok(p) = StatusAgentPayload::try_from_submessage(sm) {
                                if let Ok((result, _)) = p
                                    .result_and_position(zerodds_xrce::encoding::Endianness::Little)
                                {
                                    actual_hs = Some(result.status_code);
                                }
                            }
                        }
                        SubmessageId::Status => {
                            if let Ok(p) = StatusPayload::try_from_submessage(sm) {
                                if let Ok((reply, _)) = BaseObjectReply::decode(&p.representation) {
                                    actual.insert(
                                        reply.related_request.request_id,
                                        StatusSummary {
                                            code: reply.result.status_code,
                                            object_id: reply.related_request.object_id,
                                        },
                                    );
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        assert_eq!(
            actual_hs, expected_hs,
            "segment {seg_idx}: STATUS_AGENT result code must match the real agent's"
        );
        if expected_hs.is_some() {
            checked_status_agents += 1;
        }

        for (request_id, expected_summary) in &expected {
            let Some(actual_summary) = actual.get(request_id) else {
                panic!(
                    "segment {seg_idx}, request_id {request_id:?}: the real agent answered it \
                     ({expected_summary:?}) but this engine did not reply at all"
                );
            };
            assert_eq!(
                actual_summary, expected_summary,
                "segment {seg_idx}, request_id {request_id:?}: reply kind/status/object_id must \
                 match the real agent's"
            );
            checked_statuses += 1;
        }
    }

    assert_eq!(
        checked_status_agents, 8,
        "must have checked all 8 real CREATE_CLIENT handshakes"
    );
    // 34, not the capture's raw 35 STATUS submessages: request_id [0, 2]
    // (object_id 0xfffe) appears twice with byte-identical content --  a
    // real retransmission the capture recorded (the client's own ACKNACK
    // asked for it again), not two different replies to correlate. A
    // request_id keys this test's `expected`/`actual` maps, so the
    // duplicate collapses to one entry; nothing is silently skipped
    // (`expected_statuses` logs a `COLLISION` line if it ever sees two
    // *different* replies share a request_id, which would be a real bug).
    assert_eq!(
        checked_statuses, 34,
        "must have checked all 30 real CREATE + 5 real DELETE replies, minus the one exact \
         retransmitted duplicate (the capture's 35 raw STATUS submessages correlate to 34 \
         distinct request_ids)"
    );
}

/// What the replay asked the DDS side to create.
#[derive(Default)]
struct Recorded {
    domains: Vec<i32>,
    topics: Vec<(String, String)>,
    writers: Vec<(String, EndpointQos)>,
    readers: Vec<(String, EndpointQos)>,
    publishers: usize,
    subscribers: usize,
}

/// A [`DdsPort`] that accepts everything, like [`NullDdsPort`], and records
/// every participant, topic and endpoint it was asked for.
struct RecordingDdsPort {
    inner: NullDdsPort,
    topic_names: BTreeMap<ObjectKey, String>,
    seen: std::sync::Arc<std::sync::Mutex<Recorded>>,
}

impl DdsPort for RecordingDdsPort {
    fn create_participant(&mut self, key: ObjectKey, domain_id: i32) -> Result<(), DdsPortError> {
        self.seen.lock().unwrap().domains.push(domain_id);
        self.inner.create_participant(key, domain_id)
    }
    fn create_topic(
        &mut self,
        key: ObjectKey,
        participant: ObjectKey,
        spec: TopicSpec,
    ) -> Result<(), DdsPortError> {
        self.seen
            .lock()
            .unwrap()
            .topics
            .push((spec.topic_name.clone(), spec.type_name.clone()));
        self.topic_names.insert(key, spec.topic_name.clone());
        self.inner.create_topic(key, participant, spec)
    }
    fn create_publisher(
        &mut self,
        key: ObjectKey,
        participant: ObjectKey,
        spec: ContainerSpec,
    ) -> Result<(), DdsPortError> {
        self.seen.lock().unwrap().publishers += 1;
        self.inner.create_publisher(key, participant, spec)
    }
    fn create_subscriber(
        &mut self,
        key: ObjectKey,
        participant: ObjectKey,
        spec: ContainerSpec,
    ) -> Result<(), DdsPortError> {
        self.seen.lock().unwrap().subscribers += 1;
        self.inner.create_subscriber(key, participant, spec)
    }
    fn create_writer(
        &mut self,
        key: ObjectKey,
        topic: ObjectKey,
        publisher: ObjectKey,
        qos: EndpointQos,
    ) -> Result<(), DdsPortError> {
        let name = self.topic_names.get(&topic).cloned().unwrap_or_default();
        self.seen.lock().unwrap().writers.push((name, qos.clone()));
        self.inner.create_writer(key, topic, publisher, qos)
    }
    fn create_reader(
        &mut self,
        key: ObjectKey,
        topic: ObjectKey,
        subscriber: ObjectKey,
        qos: EndpointQos,
    ) -> Result<(), DdsPortError> {
        let name = self.topic_names.get(&topic).cloned().unwrap_or_default();
        self.seen.lock().unwrap().readers.push((name, qos.clone()));
        self.inner.create_reader(key, topic, subscriber, qos)
    }
    fn write(
        &mut self,
        writer: ObjectKey,
        payload: &[u8],
        big_endian: bool,
        dispose: bool,
        unregister: bool,
        source_time: Option<SourceTime>,
    ) -> Result<WriteOutcome, DdsPortError> {
        self.inner.write(
            writer,
            payload,
            big_endian,
            dispose,
            unregister,
            source_time,
        )
    }
    fn sample_length(
        &mut self,
        writer: ObjectKey,
        bytes: &[u8],
        big_endian: bool,
    ) -> Result<usize, DdsPortError> {
        self.inner.sample_length(writer, bytes, big_endian)
    }
    fn set_read_filter(
        &mut self,
        reader: ObjectKey,
        filter: Option<&str>,
    ) -> Result<(), DdsPortError> {
        self.inner.set_read_filter(reader, filter)
    }
    fn poll_samples(
        &mut self,
        reader: ObjectKey,
        max: usize,
    ) -> Vec<zerodds_xrce_agent::daemon::dds_port::PolledSample> {
        self.inner.poll_samples(reader, max)
    }
    fn delete(&mut self, key: ObjectKey) {
        self.inner.delete(key);
    }
    fn unregister_client(&mut self, client_key: [u8; 4]) {
        self.inner.unregister_client(client_key);
    }
}

/// The DDS entities the eProsima Micro-XRCE-DDS Client 3.0.1 examples
/// describe — XML in the Fast DDS element form (`<name>`, `<dataType>`,
/// `<kind>`), and `BinaryEntityCreation`'s `OBJK_*_Binary` blobs — are
/// created under the names, types and QoS those representations carry.
/// A DDS peer (Cyclone, Fast DDS) matches the agent's endpoints only by
/// topic and type name; an agent that reads neither creates topics no
/// peer ever matches.
#[test]
fn replay_creates_the_dds_entities_the_real_clients_describe() {
    let client_to_agent = load_hex("client-to-agent.hex");
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Recorded::default()));
    let port = RecordingDdsPort {
        inner: NullDdsPort::new(),
        topic_names: BTreeMap::new(),
        seen: std::sync::Arc::clone(&seen),
    };
    let mut engine = SessionEngine::new(
        port,
        SessionEngineConfig {
            restart_on_same_session: true,
            // The captured client names types the agent does not hold; the
            // default of the agent is to refuse such a topic (§7.7.3.7.3).
            forward_unknown_topic_types: true,
            // The captured client sends the profile elements of Fast DDS.
            ignore_unsupported_qos: true,
            // The capture's STATUS_AGENT has the ResultStatus in front.
            status_agent_with_result_status: true,
            ..SessionEngineConfig::default()
        },
    );
    let mut current_client: Option<[u8; 4]> = None;
    for (i, bytes) in client_to_agent.iter().enumerate() {
        let Ok(msg) = Message::decode(bytes) else {
            continue;
        };
        for sm in &msg.submessages {
            if sm.header.submessage_id == SubmessageId::CreateClient {
                if let Ok(p) = CreateClientPayload::try_from_submessage(sm) {
                    current_client = Some(p.representation.client_key.0);
                }
            }
        }
        let resolved = msg.header.client_key.map(|k| k.0).or(current_client);
        let now = Duration::from_millis(u64::try_from(i).unwrap());
        engine.handle_message(resolved, &msg.header, &msg.submessages, now);
    }
    let refused: Vec<_> = engine
        .drain_events()
        .into_iter()
        .filter(|e| matches!(e, SessionEvent::CreateRefused { .. }))
        .collect();
    assert!(refused.is_empty(), "CREATEs refused: {refused:?}");

    let seen = seen.lock().unwrap();
    assert!(seen.domains.iter().all(|d| *d == 0), "{:?}", seen.domains);
    // DDS-XRCE 1.0 §7.5, Table 6: the XRCE Publishers and Subscribers the
    // examples create have a DDS Publisher and Subscriber each.
    assert!(seen.publishers > 0, "no DDS Publisher was created");
    assert!(seen.subscribers > 0, "no DDS Subscriber was created");
    let mut topics = seen.topics.clone();
    topics.sort();
    topics.dedup();
    assert_eq!(
        topics,
        vec![
            (
                "BigHelloWorldTopic".to_string(),
                "BigHelloWorld".to_string()
            ),
            ("ExampleTopic".to_string(), "ExampleType".to_string()),
            ("HelloWorldTopic".to_string(), "HelloWorld".to_string()),
        ]
    );
    // The XML examples name no QoS: a DataWriter is RELIABLE, a
    // DataReader BEST_EFFORT (DDS 1.4 §2.2.3.14 defaults).
    for (topic, qos) in &seen.writers {
        if topic == "ExampleTopic" {
            continue;
        }
        assert_eq!(qos, &EndpointQos::writer_default(), "writer on {topic}");
    }
    for (topic, qos) in &seen.readers {
        if topic == "ExampleTopic" {
            continue;
        }
        assert_eq!(qos, &EndpointQos::reader_default(), "reader on {topic}");
    }
    // BinaryEntityCreation asks for RELIABLE, KEEP_LAST, TRANSIENT_LOCAL
    // (qos_flags 0x000b) on both of its endpoints.
    let binary = EndpointQos {
        reliable: true,
        history: History::KeepLast(1),
        durability: Durability::TransientLocal,
        ..EndpointQos::reader_default()
    };
    let example_writers: Vec<_> = seen
        .writers
        .iter()
        .filter(|(t, _)| t == "ExampleTopic")
        .collect();
    let example_readers: Vec<_> = seen
        .readers
        .iter()
        .filter(|(t, _)| t == "ExampleTopic")
        .collect();
    assert_eq!(example_writers.len(), 1);
    assert_eq!(example_readers.len(), 1);
    assert_eq!(example_writers[0].1, binary);
    assert_eq!(example_readers[0].1, binary);
    assert_eq!(seen.writers.len(), 4, "3 XML publishers + 1 binary");
    assert_eq!(seen.readers.len(), 4, "3 XML subscribers + 1 binary");
}
