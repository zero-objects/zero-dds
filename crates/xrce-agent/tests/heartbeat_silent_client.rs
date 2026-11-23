// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS-XRCE 1.0 §8.3.5.12 against the daemon over UDP loopback: "the XRCE
//! Agent will only send HEARTBEAT messages when it has some indication that
//! the XRCE Client is active and not in a sleep cycle. This is to avoid
//! awakening the XRCE Client unnecessarily."
//!
//! A reading client gets a DATA on its reliable stream and then stops
//! answering (its socket is read raw, nothing is acknowledged). The
//! datagrams it receives are counted: HEARTBEATs within the active window,
//! none after it. When the client speaks again, the HEARTBEAT comes with the
//! answer: with a heartbeat period of five minutes, the agent's tick sends no
//! other HEARTBEAT for the length of the test, so the one that arrives is
//! the one sent with the answer.

#![cfg(feature = "daemon")]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

mod udp_client;

use std::time::{Duration, Instant};

use zerodds_dcps::runtime::RuntimeConfig;
use zerodds_xrce::header::StreamId;
use zerodds_xrce::submessages::read_data::DataDeliveryControl;
use zerodds_xrce::submessages::write_data::DataFormat;
use zerodds_xrce::submessages::{HeartbeatPayload, Message, SubmessageId};
use zerodds_xrce_agent::daemon::config::{DaemonConfig, ListenerConfig};
use zerodds_xrce_agent::daemon::server;

use udp_client::{connect_udp, create_endpoints, status_of};

/// The datagrams `socket` receives within `span`, decoded.
fn datagrams(socket: &std::net::UdpSocket, span: Duration) -> Vec<Message> {
    let mut out = Vec::new();
    let end = Instant::now() + span;
    let mut buf = [0u8; 65_507];
    while Instant::now() < end {
        match socket.recv(&mut buf) {
            Ok(n) => out.push(Message::decode(&buf[..n]).expect("decode")),
            Err(_) => std::thread::sleep(Duration::from_millis(5)),
        }
    }
    out
}

fn heartbeats(messages: &[Message]) -> Vec<HeartbeatPayload> {
    messages
        .iter()
        .flat_map(|m| m.submessages.iter())
        .filter(|sm| sm.header.submessage_id == SubmessageId::Heartbeat)
        .map(|sm| HeartbeatPayload::try_from_submessage(sm).unwrap())
        .collect()
}

/// The agent of `cfg` with a writer client and a reading client that has a
/// continuous READ_DATA running, acknowledged up to here; from the return on
/// the reading client is silent and its socket is read raw.
struct SilentReader {
    daemon: server::DaemonHandle,
    writer_client: zerodds_xrce_client::XrceClient<udp_client::UdpTransport>,
    writer: zerodds_xrce::object_id::ObjectId,
    reader_client: zerodds_xrce_client::XrceClient<udp_client::UdpTransport>,
    socket: std::net::UdpSocket,
    start: Instant,
    _lease: zerodds_test_domains::DomainLease,
}

fn silent_reader(heartbeat_period_ms: u64, client_active_window_ms: u64, key: u8) -> SilentReader {
    let start = Instant::now();
    let lease = zerodds_test_domains::lease_domain();
    let cfg = DaemonConfig {
        discovery_enabled: false,
        listeners: vec![ListenerConfig::Udp {
            bind: "127.0.0.1:0".to_string(),
        }],
        forward_unknown_topic_types: true,
        heartbeat_period_ms,
        client_active_window_ms,
        ..DaemonConfig::default()
    };
    let daemon = server::start(cfg, RuntimeConfig::default(), None).expect("daemon start");
    let agent = daemon.udp_local_addr().expect("udp listener");

    let mut writer_client = connect_udp(agent, [0x8C, 0x51, key, 0x01], start);
    let w = create_endpoints(
        &mut writer_client,
        start,
        lease.id(),
        "HeartbeatSilentClient",
        "Opaque",
    );
    let mut reader_client = connect_udp(agent, [0x8C, 0x51, key, 0x02], start);
    let r = create_endpoints(
        &mut reader_client,
        start,
        lease.id(),
        "HeartbeatSilentClient",
        "Opaque",
    );
    // A successful READ_DATA has no STATUS (§8.3.5.9): the DATA answers it.
    reader_client
        .request_read(
            r.reader,
            0x80,
            DataFormat::Data,
            None,
            Some(DataDeliveryControl {
                max_samples: zerodds_xrce::MAX_SAMPLES_UNLIMITED,
                max_elapsed_time: 0,
                max_bytes_per_second: 0,
                min_pace_period: 0,
            }),
            true,
        )
        .unwrap();
    // The reader client acknowledges what the HEARTBEATs of the agent ask
    // for up to here.
    for _ in 0..10 {
        reader_client.tick(start.elapsed()).unwrap();
        reader_client.poll(start.elapsed()).unwrap();
        std::thread::sleep(Duration::from_millis(20));
    }
    // From here the reader client is silent: its socket is read raw.
    let socket = reader_client.transport_mut().0.try_clone().unwrap();
    SilentReader {
        daemon,
        writer_client,
        writer: w.writer,
        reader_client,
        socket,
        start,
        _lease: lease,
    }
}

impl SilentReader {
    /// Writes one sample; it reaches the silent reader as a DATA on its
    /// reliable stream, which nothing acknowledges.
    fn write_sample(&mut self) {
        let wrid = self
            .writer_client
            .request_write(self.writer, &[1, 0, 0, 0], true)
            .unwrap();
        assert!(status_of(&mut self.writer_client, self.start, wrid, "WRITE_DATA").is_success());
    }
}

#[test]
fn xrce_8_3_5_12_a_silent_udp_client_gets_heartbeats_in_the_window_and_none_after_it() {
    let mut s = silent_reader(100, 600, 0x00);
    std::thread::sleep(Duration::from_millis(800));
    let _ = datagrams(&s.socket, Duration::from_millis(50));

    s.write_sample();
    // The DATA arrives and opens the window: HEARTBEATs every 100 ms.
    let first = datagrams(&s.socket, Duration::from_millis(500));
    assert!(
        first.iter().any(|m| m.header.stream_id == StreamId(0x80)),
        "the DATA on the reliable stream: {first:?}"
    );
    let early = heartbeats(&first);
    assert!(early.len() >= 2, "HEARTBEATs within the window: {early:?}");
    assert!(early.iter().all(|h| h.stream_id == 0x80));
    // After the window (600 ms after the DATA), nothing at all.
    std::thread::sleep(Duration::from_millis(400));
    let _ = datagrams(&s.socket, Duration::from_millis(10));
    let late = datagrams(&s.socket, Duration::from_millis(1500));
    assert!(
        late.is_empty(),
        "no datagram to the silent client: {late:?}"
    );
    drop(s.daemon);
}

/// With a heartbeat period of five minutes the tick of the agent sends at
/// most the first HEARTBEAT of the stream during this test; the HEARTBEAT
/// after the client's TIMESTAMP can only be the one the agent sends with
/// its answer. A bound of two seconds does not depend on the load of the
/// host.
#[test]
fn xrce_8_3_5_12_a_silent_udp_client_that_speaks_again_gets_the_heartbeat_with_the_answer() {
    let mut s = silent_reader(300_000, 1000, 0x01);
    s.write_sample();
    let first = datagrams(&s.socket, Duration::from_millis(500));
    assert!(
        first.iter().any(|m| m.header.stream_id == StreamId(0x80)),
        "the DATA on the reliable stream: {first:?}"
    );
    assert!(
        heartbeats(&first).len() <= 1,
        "the period holds back every HEARTBEAT but the first: {first:?}"
    );
    // The window closes 1000 ms after the DATA; the client stays silent past
    // it.
    std::thread::sleep(Duration::from_millis(1200));
    let quiet = datagrams(&s.socket, Duration::from_millis(100));
    assert!(quiet.is_empty(), "nothing to the silent client: {quiet:?}");

    // The client speaks again (a TIMESTAMP): the answer and the HEARTBEAT of
    // the DATA it has not acknowledged.
    s.reader_client
        .send_timestamp(zerodds_xrce::submessages::timestamp::TimePoint {
            seconds: 1,
            nanoseconds: 0,
        })
        .unwrap();
    let woken = datagrams(&s.socket, Duration::from_secs(2));
    assert!(
        woken
            .iter()
            .flat_map(|m| m.submessages.iter())
            .any(|sm| sm.header.submessage_id == SubmessageId::TimestampReply),
        "the answer to the TIMESTAMP: {woken:?}"
    );
    let hb = heartbeats(&woken);
    assert_eq!(hb.len(), 1, "one HEARTBEAT with the answer: {woken:?}");
    assert_eq!(hb[0].stream_id, 0x80);
    drop(s.daemon);
}
