// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-opcua-bridge` with `zerodds-opcua-pubsub`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-opcua-pubsub` (`src/pubsub_glue.rs`,
//! `src/daemon/server.rs`): a `daemon::Publisher` over a
//! `transport::UdpTransport`, built from a `writer::WriterGroup` with a
//! `WriterGroupConfig` whose `NetworkMessageContentMask` names the
//! PublisherId and a `PublisherId::UInt16`, a `PublishedDataSet` of
//! the flattened DDS sample and one `DataSetWriter`; `publish_cycle` for a
//! plain and `publish_cycle_secured` with a `security::SecurityKey` for a
//! protected NetworkMessage (feature `pubsub-security`).
//!
//! So the crate relies on: one publish cycle sending one UADP NetworkMessage
//! to the configured destination, its header naming the configured
//! PublisherId with its DataType when the mask names it; a protected message announcing its
//! SecurityHeader.
//!
//! Oracles: OPC 10000-14 v1.05.06 §7.2.4.4.2 Table 154 (UADPVersion 1 in bits
//! 0-3; UADPFlags bit 4 PublisherId, bit 7 ExtendedFlags1; ExtendedFlags1
//! bits 0-2 the PublisherId type, 001 UInt16, bit 4 SecurityHeader, bit 7
//! ExtendedFlags2; the PublisherId after the flags); OPC 10000-6 §5.2.2.2
//! (a UInt16 little-endian).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::net::{SocketAddr, UdpSocket};
use std::time::Duration;

use zerodds_opcua_gateway::data_value::{DataValue, Variant, VariantValue};
use zerodds_opcua_pubsub::config::{
    ConfigurationVersion, DataSetWriterConfig, NetworkMessageContentMask, WriterGroupConfig,
};
use zerodds_opcua_pubsub::daemon::Publisher;
use zerodds_opcua_pubsub::transport::UdpTransport;
use zerodds_opcua_pubsub::uadp::network_message::PublisherId;
use zerodds_opcua_pubsub::writer::{DataSetWriter, PublishedDataSet, WriterGroup};

const PUBLISHER_ID: u16 = 0x1234;

/// A publisher as `pubsub_glue.rs` builds one, sending to `destination`.
fn publisher(destination: SocketAddr) -> Publisher<UdpTransport> {
    let local: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let transport = UdpTransport::bind(local, destination).unwrap();
    let mut config = WriterGroupConfig::new("Telemetry-wg", 7);
    config.network_message_content_mask = NetworkMessageContentMask::from_bits(
        config.network_message_content_mask.bits() | NetworkMessageContentMask::PUBLISHER_ID,
    );
    let group = WriterGroup::new(config, PublisherId::UInt16(PUBLISHER_ID));
    let mut dataset = PublishedDataSet::new("Telemetry");
    dataset.add_field(
        "value",
        DataValue::new_value(Variant::scalar(VariantValue::Int32(21)), 0, 0),
    );
    let mut publisher = Publisher::new(transport, group);
    publisher.add_dataset(dataset);
    publisher.add_writer(DataSetWriter::new(
        DataSetWriterConfig::new("Telemetry-w", 3, "Telemetry"),
        ConfigurationVersion::default(),
    ));
    publisher
}

fn receiver() -> (UdpSocket, SocketAddr) {
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let addr = socket.local_addr().unwrap();
    (socket, addr)
}

/// The header octets Table 154 fixes: version, flags, PublisherId.
fn check_header(message: &[u8]) -> u8 {
    assert_eq!(message[0] & 0x0f, 1, "UADPVersion 1");
    assert_ne!(message[0] & 0x10, 0, "PublisherId enabled");
    assert_ne!(
        message[0] & 0x80,
        0,
        "ExtendedFlags1 present for a UInt16 id"
    );
    let flags1 = message[1];
    assert_eq!(flags1 & 0x07, 0b001, "PublisherId of DataType UInt16");
    let at = if flags1 & 0x80 != 0 { 3 } else { 2 };
    assert_eq!(
        u16::from_le_bytes([message[at], message[at + 1]]),
        PUBLISHER_ID
    );
    flags1
}

#[test]
fn uapubsub_7_2_4_4_2_one_cycle_sends_a_networkmessage_with_the_publisher_id() {
    let (socket, addr) = receiver();
    let mut publisher = publisher(addr);
    publisher.publish_cycle(None).unwrap();
    let mut buf = [0u8; 2048];
    let n = socket.recv(&mut buf).expect("one NetworkMessage");
    let flags1 = check_header(&buf[..n]);
    assert_eq!(flags1 & 0x10, 0, "no SecurityHeader on a plain message");
}

#[cfg(feature = "pubsub-security")]
#[test]
fn uapubsub_7_2_4_4_2_a_protected_networkmessage_announces_its_security_header() {
    use zerodds_opcua_pubsub::security::{SecurityKey, SecurityPolicy};

    let policy = SecurityPolicy::Aes256Ctr;
    let key = SecurityKey::from_blob(policy, 11, &vec![0x5a; policy.key_material_len()]).unwrap();
    let (socket, addr) = receiver();
    let mut publisher = publisher(addr);
    publisher.publish_cycle_secured(None, policy, &key).unwrap();
    let mut buf = [0u8; 2048];
    let n = socket.recv(&mut buf).expect("one NetworkMessage");
    let flags1 = check_header(&buf[..n]);
    assert_ne!(flags1 & 0x10, 0, "SecurityHeader enabled");
}
