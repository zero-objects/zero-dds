// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! A ZeroDDS UADP publisher for a foreign subscriber.
//!
//! Sends NetworkMessages shaped exactly the way open62541's
//! `tutorial_pubsub_subscribe` filters for
//! (`examples/pubsub/tutorial_pubsub_subscribe.c:81-85`): PublisherId UInt16
//! 2234, WriterGroupId 100, DataSetWriterId 62541, a KeyFrame carrying the
//! four Variant fields its DataSetMetaData declares — DateTime, Int32, Int64,
//! Boolean. The foreign subscriber maps them onto `ns=1;i=50000..50003`, where
//! a foreign client reads them back.
//!
//! Usage: `open62541_uadp_publisher [group:port] [count] [int32] [int64]`.

#![allow(clippy::print_stdout)]

use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::process::ExitCode;
use std::thread::sleep;
use std::time::Duration;

use zerodds_opcua_gateway::data_value::{Variant, VariantValue};
use zerodds_opcua_pubsub::uadp::dataset_message::DataSetMessage;
use zerodds_opcua_pubsub::uadp::network_message::{GroupHeader, NetworkMessage, PublisherId};
use zerodds_opcua_pubsub::{UaEncode, UaWriter};

fn fail(what: &str) -> ExitCode {
    println!("PUBLISHER-FAIL: {what}");
    ExitCode::FAILURE
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let group_port = args
        .next()
        .unwrap_or_else(|| String::from("224.0.0.22:4840"));
    let count: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(20);
    let int32: i32 = args
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1_234_567);
    let int64: i64 = args
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(-9_876_543_210);

    let Some((group, port)) = group_port.split_once(':') else {
        return fail("expected <group>:<port>");
    };
    let (Ok(group), Ok(port)) = (group.parse::<Ipv4Addr>(), port.parse::<u16>()) else {
        return fail("could not parse <group>:<port>");
    };

    let socket = match UdpSocket::bind(SocketAddr::from(([0, 0, 0, 0], 0))) {
        Ok(s) => s,
        Err(e) => return fail(&format!("bind: {e}")),
    };
    if let Err(e) = socket.set_multicast_loop_v4(true) {
        return fail(&format!("set_multicast_loop_v4: {e}"));
    }
    let destination = SocketAddr::from((group, port));

    let fields = vec![
        Variant::scalar(VariantValue::DateTime(134_311_993_591_997_027)),
        Variant::scalar(VariantValue::Int32(int32)),
        Variant::scalar(VariantValue::Int64(int64)),
        Variant::scalar(VariantValue::Boolean(true)),
    ];
    let message = NetworkMessage {
        publisher_id: Some(PublisherId::UInt16(2234)),
        data_set_class_id: None,
        group_header: Some(GroupHeader {
            writer_group_id: Some(100),
            group_version: None,
            network_message_number: None,
            sequence_number: None,
        }),
        timestamp: None,
        pico_seconds: None,
        promoted_fields: Vec::new(),
        payload_header: true,
        messages: vec![DataSetMessage::key_frame_variant(62541, fields)],
    };

    let mut w = UaWriter::new();
    if let Err(e) = message.encode(&mut w) {
        return fail(&format!("encode: {e}"));
    }
    let datagram = w.into_vec();
    println!("publishing {} bytes to {destination}", datagram.len());

    for _ in 0..count {
        if let Err(e) = socket.send_to(&datagram, destination) {
            return fail(&format!("send_to: {e}"));
        }
        sleep(Duration::from_millis(100));
    }
    println!("PUBLISHER-DONE: {count} NetworkMessages sent");
    ExitCode::SUCCESS
}
