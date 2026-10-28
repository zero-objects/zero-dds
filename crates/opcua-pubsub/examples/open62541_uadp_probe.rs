// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The ZeroDDS UADP decoder against a foreign publisher.
//!
//! Joins the multicast group open62541's `tutorial_pubsub_publish` sends to and
//! decodes the NetworkMessages it puts on the wire. The expected values are the
//! foreign example's own configuration
//! (`open62541/examples/pubsub/tutorial_pubsub_publish.c`, mirrored in
//! `tutorial_pubsub_subscribe.c:81-85`): PublisherId UInt16 2234, WriterGroupId
//! 100, DataSetWriterId 62541, one Variant-encoded DateTime field.
//!
//! Usage: `open62541_uadp_probe [group:port] [count] [interface-address]`.
//!
//! The interface address is the local address to join the group on. 224.0.0.x
//! is link-local, so joining on the unspecified address leaves the choice to
//! the kernel, which picks a route the publisher may not be sending over.

#![allow(clippy::print_stdout)]

use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::process::ExitCode;
use std::time::Duration;

use zerodds_opcua_gateway::data_value::VariantValue;
use zerodds_opcua_pubsub::uadp::dataset_message::{DataSetData, DataSetMessageKind};
use zerodds_opcua_pubsub::uadp::network_message::{NetworkMessage, PublisherId};
use zerodds_opcua_pubsub::{UaDecode, UaReader};

fn fail(what: &str) -> ExitCode {
    println!("PROBE-FAIL: {what}");
    ExitCode::FAILURE
}

fn main() -> ExitCode {
    let group_port = std::env::args()
        .nth(1)
        .unwrap_or_else(|| String::from("224.0.0.22:4840"));
    let want: usize = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(3);

    let Some((group, port)) = group_port.split_once(':') else {
        return fail("expected <group>:<port>");
    };
    let (Ok(group), Ok(port)) = (group.parse::<Ipv4Addr>(), port.parse::<u16>()) else {
        return fail("could not parse <group>:<port>");
    };

    let socket = match UdpSocket::bind(SocketAddr::from(([0, 0, 0, 0], port))) {
        Ok(s) => s,
        Err(e) => return fail(&format!("bind: {e}")),
    };
    let interface = std::env::args()
        .nth(3)
        .and_then(|s| s.parse::<Ipv4Addr>().ok())
        .unwrap_or(Ipv4Addr::UNSPECIFIED);
    if let Err(e) = socket.join_multicast_v4(&group, &interface) {
        return fail(&format!("join_multicast_v4 on {interface}: {e}"));
    }
    if let Err(e) = socket.set_read_timeout(Some(Duration::from_secs(10))) {
        return fail(&format!("set_read_timeout: {e}"));
    }
    println!("listening on {group}:{port} via {interface} for {want} NetworkMessages");

    let mut seen = 0usize;
    let mut buf = [0u8; 4096];
    while seen < want {
        let (n, from) = match socket.recv_from(&mut buf) {
            Ok(v) => v,
            Err(e) => return fail(&format!("recv_from (foreign publisher silent?): {e}")),
        };
        let datagram = &buf[..n];
        let mut r = UaReader::new(datagram);
        let nm = match NetworkMessage::decode(&mut r) {
            Ok(nm) => nm,
            Err(e) => {
                return fail(&format!(
                    "decoding {n} foreign bytes from {from} failed: {e}"
                ));
            }
        };

        match nm.publisher_id {
            Some(PublisherId::UInt16(2234)) => {}
            other => return fail(&format!("expected PublisherId UInt16(2234), got {other:?}")),
        }
        match nm.group_header.as_ref().and_then(|g| g.writer_group_id) {
            Some(100) => {}
            other => return fail(&format!("expected WriterGroupId 100, got {other:?}")),
        }
        let Some(dsm) = nm.messages.first() else {
            return fail("the foreign NetworkMessage carried no DataSetMessage");
        };
        if dsm.writer_id != 62541 {
            return fail(&format!(
                "expected DataSetWriterId 62541, got {}",
                dsm.writer_id
            ));
        }
        if dsm.kind != DataSetMessageKind::KeyFrame {
            return fail(&format!("expected a KeyFrame, got {:?}", dsm.kind));
        }
        let DataSetData::Variant(fields) = &dsm.data else {
            return fail("expected Variant-encoded fields");
        };
        let ticks = match fields.first().and_then(|v| v.value.first()) {
            Some(VariantValue::DateTime(t)) => *t,
            other => return fail(&format!("expected a DateTime field, got {other:?}")),
        };
        // The foreign publisher stamps the server's current time; anything
        // before the year 2000 in DateTime ticks means we read the wrong bytes.
        if ticks < 125_911_584_000_000_000 {
            return fail(&format!("DateTime ticks {ticks} predate the year 2000"));
        }
        seen += 1;
        println!(
            "  #{seen} {n} bytes from {from}: publisher=2234 group=100 writer=62541 \
             config={:?}/{:?} DateTime={ticks}",
            dsm.config_major_version, dsm.config_minor_version
        );
    }

    println!("PROBE-PASS: {seen} foreign UADP NetworkMessages decoded");
    ExitCode::SUCCESS
}
