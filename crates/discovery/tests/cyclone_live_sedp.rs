//! WP 1.4 T6b — live-interop test against a real Cyclone DDS instance.
//!
//! **Opt-in** — marked `#[ignore]`, and without `INTEROP_REQUIRE=1` (or
//! `INTEROP_HOST_AVAILABLE=1`) it reports a skip instead of running. The
//! interop CI job sets it and has Cyclone's `ddsperf` on the path:
//!
//! ```bash
//! INTEROP_REQUIRE=1 cargo test -p zerodds-discovery --test cyclone_live_sedp -- --ignored --nocapture
//! ```
//!
//! `DDSPERF` names the binary when it is not `ddsperf` on the path.
//!
//! # Test flow
//!
//! 1. Locally: bind a unicast socket (for SEDP replies) + a multicast
//!    socket (for SPDP beacons) + a sender socket for our beacon, all on
//!    the interface a UDP connect probe picks.
//! 2. SpdpBeacon sender thread: every 500ms send our own SPDP datagram
//!    with all SEDP endpoint flags to 239.255.0.1:17900. This way
//!    Cyclone sees us as a matched peer and sends SEDP publications via
//!    unicast to our `default_unicast_locator`.
//! 3. Start `ddsperf -i 42 -D 18 pub 1Hz` on this host → Cyclone sends
//!    SPDP beacons + SEDP publications.
//! 4. Locally poll-loop both sockets (multicast + unicast). SPDP →
//!    `SedpStack::on_participant_discovered`; SEDP → `handle_datagram`
//!    → publications land in the cache.
//! 5. Assert: Cyclone SPDP discovery, its unicast reply, and at least one
//!    SEDP publication after the preemptive AckNack.
//! 6. Teardown: the child is killed on drop.
//!
//! Before, the test ran `ddsperf` over SSH on a lab host, with its host
//! name and password in this file, and found the local address through
//! macOS' `ipconfig`; it could only run from a Mac on that LAN.
//!
//! # Reliable SEDP AckNack loop
//!
//! The test ticks the SedpStack periodically. `add_writer_proxy`
//! schedules a preemptive AckNack with `final=false`, signaling to the
//! Cyclone writer "reader is ready, send everything from the start".
//! Each AckNack/NackFrag is routed with INFO_DST to Cyclone's
//! GuidPrefix (RTPS 2.5 §8.3.7.6 — without INFO_DST Cyclone discards
//! ACKNACK as "not a connection"). Cyclone replies with Heartbeat +
//! DATA. Heartbeat dispatch in the SedpStack also accepts
//! reader_id=UNKNOWN, since Cyclone often sets it that way and does the
//! addressing via INFO_DST.
//!
//! The deterministic part is in `cyclone_sedp_replay.rs` without
//! `#[ignore]`.

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

use core::time::Duration;
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Instant;

use zerodds_discovery::sedp::SedpStack;
use zerodds_discovery::spdp::{SpdpBeacon, SpdpReader};
use zerodds_rtps::participant_data::{
    Duration as DdsDuration, ParticipantBuiltinTopicData, endpoint_flag,
};
use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix, Locator, ProtocolVersion, VendorId};

const CYCLONE_DOMAIN: u32 = 42;
/// Multicast port for domain 42: 7400 + 250 * 42 = 17900.
const SPDP_MULTICAST_PORT: u16 = 17900;
const SPDP_MULTICAST_GROUP: Ipv4Addr = Ipv4Addr::new(239, 255, 0, 1);
/// Timeout for the entire test run.
const TEST_TIMEOUT: Duration = Duration::from_secs(20);
/// How long we wait for SPDP before aborting.
const SPDP_DISCOVERY_TIMEOUT: Duration = Duration::from_secs(8);
/// Beacon send interval.
const BEACON_INTERVAL: Duration = Duration::from_millis(500);
/// Local GuidPrefix — must be != Cyclone prefix, otherwise it collides.
const LOCAL_PREFIX: [u8; 12] = [0xEE; 12];

struct CycloneSubprocess {
    child: Child,
}

impl CycloneSubprocess {
    /// Starts `ddsperf -i <domain> -D 18 pub 1Hz` on this host (`-i` is
    /// the domain, `-D` the duration in seconds). `DDSPERF` names the
    /// binary when it is not `ddsperf` on the path.
    fn start() -> std::io::Result<Self> {
        let binary = std::env::var_os("DDSPERF").unwrap_or_else(|| "ddsperf".into());
        let child = Command::new(binary)
            .args(["-i", &CYCLONE_DOMAIN.to_string(), "-D", "18", "pub", "1Hz"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        Ok(Self { child })
    }
}

impl Drop for CycloneSubprocess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The local address a packet to another network would leave from: a UDP
/// connect sends nothing and only asks the routing table. Cyclone sends
/// SEDP to the unicast locator our beacon announces, so it has to be a
/// real address, not UNSPECIFIED.
fn detect_local_interface() -> Ipv4Addr {
    let probe = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0))
        .and_then(|s| {
            s.connect(SocketAddrV4::new(Ipv4Addr::new(10, 255, 255, 255), 9))
                .map(|()| s)
        })
        .and_then(|s| s.local_addr());
    match probe {
        Ok(std::net::SocketAddr::V4(addr)) => *addr.ip(),
        _ => Ipv4Addr::UNSPECIFIED,
    }
}

/// Builds our ParticipantBuiltinTopicData with all SEDP flags.
fn build_local_participant(local_ip: Ipv4Addr, unicast_port: u16) -> ParticipantBuiltinTopicData {
    let flags = endpoint_flag::PARTICIPANT_ANNOUNCER
        | endpoint_flag::PARTICIPANT_DETECTOR
        | endpoint_flag::PUBLICATIONS_ANNOUNCER
        | endpoint_flag::PUBLICATIONS_DETECTOR
        | endpoint_flag::SUBSCRIPTIONS_ANNOUNCER
        | endpoint_flag::SUBSCRIPTIONS_DETECTOR;
    ParticipantBuiltinTopicData {
        guid: Guid::new(GuidPrefix::from_bytes(LOCAL_PREFIX), EntityId::PARTICIPANT),
        protocol_version: ProtocolVersion::V2_5,
        vendor_id: VendorId::ZERODDS,
        default_unicast_locators: vec![Locator::udp_v4(local_ip.octets(), u32::from(unicast_port))],
        default_multicast_locators: vec![Locator::udp_v4(
            SPDP_MULTICAST_GROUP.octets(),
            u32::from(SPDP_MULTICAST_PORT),
        )],
        // Metatraffic locator = our unicast socket. Cyclone sends SEDP
        // publications exactly there after it matches our beacon.
        // Without this PID, Cyclone does not route SEDP back.
        metatraffic_unicast_locators: vec![Locator::udp_v4(
            local_ip.octets(),
            u32::from(unicast_port),
        )],
        metatraffic_multicast_locators: vec![Locator::udp_v4(
            SPDP_MULTICAST_GROUP.octets(),
            u32::from(SPDP_MULTICAST_PORT),
        )],
        // Domain 42 matching Cyclone's `-D 42`. Without DOMAIN_ID,
        // Cyclone counts the beacon as domain 0 → no match.
        domain_id: Some(CYCLONE_DOMAIN),
        domain_tag: Default::default(),
        builtin_endpoint_set: flags,
        lease_duration: DdsDuration::from_secs(30),
        user_data: Vec::new(),
        properties: Default::default(),
        identity_token: None,
        permissions_token: None,
        participant_security_info: None,
        identity_status_token: None,
        sig_algo_info: None,
        kx_algo_info: None,
        sym_cipher_algo_info: None,
        available_builtin_endpoints_ext: 0,
        expects_inline_qos: false,
        manual_liveliness_count: None,
        builtin_endpoint_qos: None,
        entity_name: None,
    }
}

#[test]
#[ignore = "needs Cyclone DDS ddsperf on this host; the interop CI job runs it"]
fn cyclone_live_sedp_discovery() {
    if std::env::var("INTEROP_REQUIRE").as_deref() != Ok("1")
        && std::env::var("INTEROP_HOST_AVAILABLE").as_deref() != Ok("1")
    {
        eprintln!("INTEROP-SKIP[cyclone_live_sedp]: opt in with INTEROP_REQUIRE=1 and ddsperf");
        return;
    }
    let interface = detect_local_interface();
    assert!(
        !interface.is_unspecified(),
        "could not detect a local IPv4 address; the test needs a real one \
         for Cyclone to send SEDP unicast back"
    );
    eprintln!("local interface: {interface}");

    // Multicast recv socket for SPDP beacons from Cyclone. Shared with the
    // Cyclone on this host, which binds the same port: without
    // SO_REUSEADDR on both, the second bind fails and ddsperf exits
    // ("failed to bind to ANY:17900: address in use").
    let spdp_sock: UdpSocket = {
        let s = socket2::Socket::new(
            socket2::Domain::IPV4,
            socket2::Type::DGRAM,
            Some(socket2::Protocol::UDP),
        )
        .expect("spdp socket");
        s.set_reuse_address(true).expect("SO_REUSEADDR");
        s.bind(&SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, SPDP_MULTICAST_PORT).into())
            .expect("bind spdp multicast port");
        s.into()
    };
    spdp_sock
        .set_read_timeout(Some(Duration::from_millis(100)))
        .unwrap();
    spdp_sock
        .join_multicast_v4(&SPDP_MULTICAST_GROUP, &interface)
        .expect("join spdp multicast group");
    eprintln!("joined {SPDP_MULTICAST_GROUP}:{SPDP_MULTICAST_PORT}");

    // Unicast recv socket for SEDP replies. Binds to a free port
    // number on the LAN interface — we announce exactly this address as
    // default_unicast_locator in the SPDP beacon.
    let unicast_sock =
        UdpSocket::bind(SocketAddrV4::new(interface, 0)).expect("bind unicast socket");
    unicast_sock
        .set_read_timeout(Some(Duration::from_millis(100)))
        .unwrap();
    let unicast_port = unicast_sock.local_addr().unwrap().port();
    eprintln!("unicast bound on {interface}:{unicast_port}");

    // Beacon sender socket. Binds to the LAN interface — with a bound
    // source IP, kernel multicast routing follows the matching
    // interface. `set_multicast_if_v4` is not available in std::net;
    // an explicit bind is enough here.
    let sender_sock = UdpSocket::bind(SocketAddrV4::new(interface, 0)).expect("bind sender socket");
    sender_sock
        .set_multicast_ttl_v4(32)
        .expect("set multicast ttl");

    // Our own participant data + beacon.
    let our_data = build_local_participant(interface, unicast_port);
    let our_prefix = our_data.guid.prefix;
    let mut beacon = SpdpBeacon::new(our_data);

    // Sender thread: periodically sends our beacon on multicast.
    let stop = Arc::new(AtomicBool::new(false));
    let stop_sender = Arc::clone(&stop);
    let beacon_sent = Arc::new(AtomicBool::new(false));
    let beacon_sent_clone = Arc::clone(&beacon_sent);
    let mc_dest = SocketAddrV4::new(SPDP_MULTICAST_GROUP, SPDP_MULTICAST_PORT);
    let send_handle = thread::spawn(move || {
        while !stop_sender.load(Ordering::Relaxed) {
            match beacon.serialize() {
                Ok(d) => {
                    if let Err(e) = sender_sock.send_to(&d, mc_dest) {
                        eprintln!("beacon send error: {e}");
                    } else {
                        beacon_sent_clone.store(true, Ordering::Relaxed);
                    }
                }
                Err(e) => {
                    eprintln!("beacon serialize error: {e:?}");
                    break;
                }
            }
            thread::sleep(BEACON_INTERVAL);
        }
    });

    let _cyclone = CycloneSubprocess::start().expect("start ddsperf");
    eprintln!("started ddsperf");

    // Main loop: poll both sockets.
    let spdp = SpdpReader::new();
    let mut stack = SedpStack::new(GuidPrefix::from_bytes(LOCAL_PREFIX), VendorId::ZERODDS);
    let mut cyclone_discovered = false;
    let mut publications_seen = 0usize;
    let mut cyclone_unicast_inbound = 0usize;
    let mut buf = vec![0u8; 65535];

    let start = Instant::now();
    while start.elapsed() < TEST_TIMEOUT {
        // Poll both sockets in turn (short timeouts).
        for (name, sock) in [("mc", &spdp_sock), ("uc", &unicast_sock)] {
            let n = match sock.recv(&mut buf) {
                Ok(n) => n,
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    continue;
                }
                Err(e) => {
                    eprintln!("{name} recv error: {e}");
                    continue;
                }
            };
            let datagram = &buf[..n];
            if name == "uc" {
                cyclone_unicast_inbound += 1;
            }

            // SPDP beacon? → participant discovery.
            if let Ok(p) = spdp.parse_datagram(datagram) {
                // Ignore our own beacons (loopback can happen).
                if p.sender_prefix == our_prefix {
                    continue;
                }
                if !cyclone_discovered {
                    eprintln!(
                        "discovered cyclone on {name}: prefix={:?}, vendor={:?}, endpoint_set={:#x}, uc={:?}",
                        p.sender_prefix,
                        p.sender_vendor,
                        p.data.builtin_endpoint_set,
                        p.data.default_unicast_locators
                    );
                    stack.on_participant_discovered(&p);
                    cyclone_discovered = true;
                }
                continue;
            }

            // Not SPDP → possibly SEDP. Only process if Cyclone has
            // already been discovered (otherwise the WriterProxy is
            // missing).
            if cyclone_discovered {
                match stack.handle_datagram(datagram, Duration::from_secs(1)) {
                    Ok(events) => {
                        if !events.new_publications.is_empty() {
                            for p in &events.new_publications {
                                eprintln!(
                                    "sedp pub on {name}: topic={}, type={}",
                                    p.topic_name, p.type_name
                                );
                            }
                            publications_seen += events.new_publications.len();
                        }
                    }
                    Err(e) => eprintln!("{name} sedp handle_datagram error: {e:?}"),
                }
            }
        }

        // Reader tick: send ACKNACK/NACK_FRAG to Cyclone so the
        // reliable writer delivers its DATA publications.
        let now_rel = start.elapsed();
        match stack.tick(now_rel) {
            Ok(outbound) => {
                for dg in outbound {
                    for loc in dg.targets.iter() {
                        if loc.kind != zerodds_rtps::wire_types::LocatorKind::UdpV4 {
                            continue;
                        }
                        let ip = loc.ipv4();
                        let Ok(port) = u16::try_from(loc.port) else {
                            continue;
                        };
                        let addr =
                            SocketAddrV4::new(Ipv4Addr::new(ip[0], ip[1], ip[2], ip[3]), port);
                        if let Err(e) = unicast_sock.send_to(&dg.bytes, addr) {
                            eprintln!("acknack send error to {addr}: {e}");
                        }
                    }
                }
            }
            Err(e) => eprintln!("stack.tick error: {e:?}"),
        }

        if publications_seen >= 4 {
            break;
        }

        if start.elapsed() > SPDP_DISCOVERY_TIMEOUT && !cyclone_discovered {
            stop.store(true, Ordering::Relaxed);
            let _ = send_handle.join();
            panic!(
                "no cyclone SPDP beacon received within {:?} \
                 (beacon_sent={})",
                SPDP_DISCOVERY_TIMEOUT,
                beacon_sent.load(Ordering::Relaxed)
            );
        }
    }

    // Stop the sender thread before the assertions.
    stop.store(true, Ordering::Relaxed);
    let _ = send_handle.join();

    eprintln!(
        "test finished: cyclone_discovered={cyclone_discovered}, \
         cyclone_unicast_inbound={cyclone_unicast_inbound}, \
         publications_seen={publications_seen}, \
         beacon_sent={}, \
         pub_reader_unknown_src={}",
        beacon_sent.load(Ordering::Relaxed),
        stack.pub_reader().inner().unknown_src_count()
    );

    // Acceptance:
    // 1. Cyclone SPDP discovery (Cyclone beacon parsed byte-exact).
    // 2. Our beacon was sent out.
    // 3. Cyclone replies on our unicast port — proof that Cyclone
    //    accepted us as a matched peer with the new METATRAFFIC_*_LOCATOR
    //    + DOMAIN_ID PIDs. Without the new PIDs, Cyclone does not see us.
    //
    // Reliable SEDP publications only arrive with the reader tick +
    // AckNack (a separate step, see the header comment).
    assert!(
        cyclone_discovered,
        "expected Cyclone SPDP discovery within {TEST_TIMEOUT:?}"
    );
    assert!(
        beacon_sent.load(Ordering::Relaxed),
        "expected beacon to be sent at least once"
    );
    assert!(
        cyclone_unicast_inbound >= 1,
        "expected Cyclone to unicast-reply to our metatraffic_unicast_locator, got {cyclone_unicast_inbound} packets"
    );
    assert!(
        publications_seen >= 1,
        "expected at least 1 SEDP publication after preemptive AckNack, got {publications_seen}"
    );
    thread::sleep(Duration::from_millis(100));
}
