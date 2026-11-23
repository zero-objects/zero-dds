// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS-XRCE 1.0 §10.4 against another implementation: the eProsima
//! Micro-XRCE-DDS Agent 3.0.1 is the upstream agent of a ZeroDDS agent
//! ("XRCE Agents appear as Clients to other XRCE Agents"). The ZeroDDS agent
//! is `daemon::server::start` with an `upstream:` block; its clients are
//! `zerodds-xrce-client` over UDP; the objects they create, the sample they
//! write and the one they read live in the eProsima agent, whose answers come
//! back through the ZeroDDS agent.
//!
//! `MICRO_XRCE_AGENT_BIN` names the `MicroXRCEAgent` executable
//! (`tools/wire-capture/xrce-sessions/build-foreign.sh` builds it, the
//! interop image has it). The test is `#[ignore]` in the crate tier and runs
//! in the `vendor-live-xrce` job of `ci/interop-omg-weekly.yml`.

#![cfg(feature = "daemon")]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

mod udp_client;

use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use zerodds_dcps::runtime::RuntimeConfig;
use zerodds_xrce::object_id::ObjectId;
use zerodds_xrce::object_info::ResultStatusCode;
use zerodds_xrce::object_kind::ObjectKind;
use zerodds_xrce::object_repr::{ObjectVariant, ObjectVariantExtra};
use zerodds_xrce::submessages::read_data::{DataDeliveryControl, DeliveryControlForm};
use zerodds_xrce::submessages::write_data::DataFormat;
use zerodds_xrce_agent::daemon::config::{DaemonConfig, ListenerConfig};
use zerodds_xrce_agent::daemon::federation::{UpstreamConfig, UpstreamLocator};
use zerodds_xrce_agent::daemon::server;
use zerodds_xrce_client::{ClientEvent, XrceClient};

use udp_client::{UdpTransport, connect_udp, status_of};

/// The eProsima agent, stopped when the test ends.
struct Foreign(Child);

impl Drop for Foreign {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// `MicroXRCEAgent udp4 --port <port>`.
fn eprosima_agent(port: u16) -> Foreign {
    let bin = std::env::var("MICRO_XRCE_AGENT_BIN").expect(
        "MICRO_XRCE_AGENT_BIN must name the MicroXRCEAgent executable of the eProsima \
         Micro-XRCE-DDS Agent 3.0.1 (tools/wire-capture/xrce-sessions/build-foreign.sh)",
    );
    let child = Command::new(&bin)
        .args(["udp4", "--port", &port.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap_or_else(|e| panic!("{bin} does not start: {e}"));
    Foreign(child)
}

/// The ZeroDDS agent with the eProsima agent as upstream agent. Its DDS
/// runtime cannot start (the multicast allowlist does not hold the SPDP
/// group): whatever the clients get, the eProsima agent made it.
fn relay(upstream: std::net::SocketAddr) -> server::DaemonHandle {
    let cfg = DaemonConfig {
        discovery_enabled: false,
        listeners: vec![ListenerConfig::Udp {
            bind: "127.0.0.1:0".to_string(),
        }],
        upstream: Some(UpstreamConfig {
            keepalive: Duration::from_millis(200),
            link_timeout: Duration::from_millis(2000),
            reconnect: Duration::from_millis(200),
            ..UpstreamConfig::new(UpstreamLocator::Udp(upstream))
        }),
        ..DaemonConfig::default()
    };
    let runtime = RuntimeConfig {
        multicast_allowlist: vec![std::net::Ipv4Addr::new(239, 255, 0, 99)],
        ..RuntimeConfig::default()
    };
    server::start(cfg, runtime, None).expect("daemon start")
}

const TOPIC: &str = "FederatedForeign";
const TYPE: &str = "FederatedForeignType";

/// CREATEs `oid` with the XML of a Fast DDS profile, the form the eProsima
/// agent reads, and waits for its STATUS.
fn create(
    client: &mut XrceClient<UdpTransport>,
    start: Instant,
    oid: ObjectId,
    xml: &str,
    extra: ObjectVariantExtra,
) -> ResultStatusCode {
    let rid = client
        .create_object(
            oid,
            ObjectVariant::ByXmlString(xml.to_string()),
            extra,
            false,
            false,
            true,
        )
        .unwrap();
    status_of(client, start, rid, "CREATE at the eProsima agent")
}

fn under(parent: ObjectId) -> ObjectVariantExtra {
    ObjectVariantExtra {
        domain_id: None,
        parent: Some(parent),
    }
}

/// A participant, the topic and, by `role`, a publisher and DataWriter or a
/// subscriber and DataReader; every CREATE answered `STATUS_OK` by the eProsima
/// agent. Returns the endpoint.
fn endpoints(
    client: &mut XrceClient<UdpTransport>,
    start: Instant,
    domain: i32,
    writer: bool,
) -> ObjectId {
    let participant = ObjectId::new(0x001, ObjectKind::Participant).unwrap();
    let topic = ObjectId::new(0x001, ObjectKind::Topic).unwrap();
    let topic_xml =
        format!("<dds><topic><name>{TOPIC}</name><dataType>{TYPE}</dataType></topic></dds>");
    let endpoint_topic = format!(
        "<topic><kind>NO_KEY</kind><name>{TOPIC}</name><dataType>{TYPE}</dataType></topic>"
    );
    let (container, endpoint, xml) = if writer {
        (
            ObjectId::new(0x001, ObjectKind::Publisher).unwrap(),
            ObjectId::new(0x001, ObjectKind::DataWriter).unwrap(),
            format!("<dds><data_writer>{endpoint_topic}</data_writer></dds>"),
        )
    } else {
        (
            ObjectId::new(0x001, ObjectKind::Subscriber).unwrap(),
            ObjectId::new(0x001, ObjectKind::DataReader).unwrap(),
            format!("<dds><data_reader>{endpoint_topic}</data_reader></dds>"),
        )
    };
    let steps = [
        (
            participant,
            "<dds><participant><rtps><name>zerodds_relayed</name></rtps></participant></dds>"
                .to_string(),
            ObjectVariantExtra {
                domain_id: Some(i16::try_from(domain).unwrap()),
                parent: None,
            },
        ),
        (topic, topic_xml, under(participant)),
        (container, String::new(), under(participant)),
        (endpoint, xml, under(container)),
    ];
    for (oid, xml, extra) in steps {
        let code = create(client, start, oid, &xml, extra);
        assert_eq!(
            code,
            ResultStatusCode::Ok,
            "CREATE {:04x} at the eProsima agent",
            oid.raw()
        );
    }
    endpoint
}

/// Two clients of the ZeroDDS agent create their objects in the eProsima
/// agent through it, one writes a sample, the other reads it: the CREATEs,
/// the WRITE_DATA and the READ_DATA go up unchanged, the STATUS and the DATA
/// of the eProsima agent come back. The READ_DATA carries its
/// DataDeliveryControl without DHEADER, the form the eProsima agent reads.
#[test]
#[ignore = "needs the eProsima Micro-XRCE-DDS Agent (MICRO_XRCE_AGENT_BIN); runs in vendor-live-xrce"]
fn xrce_10_4_the_eprosima_agent_is_the_upstream_agent_of_a_zerodds_agent() {
    let start = Instant::now();
    let lease = zerodds_test_domains::lease_domain();
    let port = {
        let probe = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        probe.local_addr().unwrap().port()
    };
    let _foreign = eprosima_agent(port);
    std::thread::sleep(Duration::from_millis(500));
    let zerodds = relay(std::net::SocketAddr::from(([127, 0, 0, 1], port)));
    let address = zerodds.udp_local_addr().unwrap();

    let mut reading = connect_udp(address, [0x10, 0x4F, 0xE0, 0x02], start);
    reading.set_delivery_control_form(DeliveryControlForm::Bare);
    let reader = endpoints(&mut reading, start, lease.id(), false);
    reading
        .request_read(
            reader,
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
    let mut writing = connect_udp(address, [0x10, 0x4F, 0xE0, 0x01], start);
    let writer = endpoints(&mut writing, start, lease.id(), true);

    // The reader matches the writer inside the eProsima agent at a time of
    // its own: the writer writes the sample again until it is read.
    let sample = [0x5A, 0x10, 0x04, 0xE0, 0x01, 0x02, 0x03, 0x04];
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut read = Vec::new();
    let mut last_write: Option<Instant> = None;
    while read.is_empty() && Instant::now() < deadline {
        if last_write.is_none_or(|t| t.elapsed() >= Duration::from_millis(300)) {
            writing.request_write(writer, &sample, true).unwrap();
            last_write = Some(Instant::now());
        }
        for c in [&mut writing, &mut reading] {
            c.tick(start.elapsed()).unwrap();
        }
        for e in writing.poll(start.elapsed()).unwrap() {
            if let ClientEvent::Status { code, .. } = e {
                assert!(code.is_success(), "a WRITE_DATA refused: {code:?}");
            }
        }
        for e in reading.poll(start.elapsed()).unwrap() {
            if let ClientEvent::Data { payload, .. } = e {
                read.push(payload);
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        !read.is_empty(),
        "no sample of the eProsima agent reached the reader through the ZeroDDS agent"
    );
    assert!(read.iter().all(|p| *p == sample), "{read:?}");
    drop((writing, reading, zerodds));
}
