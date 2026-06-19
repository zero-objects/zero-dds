//! WP 1.4 T6b — live-interop test against a real Cyclone DDS instance.
//!
//! **Opt-in only** — marked `#[ignore]`. Invocation:
//!
//! ```bash
//! cargo test -p zerodds-discovery --test cyclone_live_sedp -- --ignored --nocapture
//! ```
//!
//! # Prerequisites
//!
//! - SSH access to `llvm@llvm` with password `llvm` (lab setup)
//! - `sshpass` installed locally
//! - Cyclone DDS 0.10.2 (`ddsperf` binary) on the bench host
//! - `/tmp/cyc.xml` on the bench host pins Cyclone to `enp6s18`
//! - Local host on the same LAN as the bench host
//! - **Multicast pass-through between the virtualized host's VM and the
//!   client.** The virtualization host has a 2-level bridge hierarchy
//!   (`vmbr0` + `fwbr113i0`). Both need `multicast_querier=1` +
//!   `multicast_snooping=0`, otherwise the bridge filters multicast.
//!   Details in memory `reference_pve_multicast_setup`.
//!
//! # Test flow
//!
//! 1. Locally: bind a unicast socket (for SEDP replies) + a multicast
//!    socket (for SPDP beacons) + a sender socket for our beacon.
//! 2. SpdpBeacon sender thread: every 500ms send our own SPDP datagram
//!    with all SEDP endpoint flags to 239.255.0.1:17900. This way
//!    Cyclone sees us as a matched peer and sends SEDP publications via
//!    unicast to our `default_unicast_locator`.
//! 3. SSH subprocess starts `ddsperf -D 42 pub 1Hz` on the bench host
//!    → Cyclone sends SPDP beacons + SEDP publications.
//! 4. Locally poll-loop both sockets (multicast + unicast). SPDP →
//!    `SedpStack::on_participant_discovered`; SEDP → `handle_datagram`
//!    → publications land in the cache.
//! 5. Assert: Cyclone SPDP discovery + beacon outbound verified.
//!    Publications are optionally logged along the way.
//! 6. Teardown: SSH pkill on `ddsperf`, stop the sender thread.
//!
//! # VM kernel multicast fix (required on the bench host, 2026-04-19)
//!
//! On the VM, `ip link set dev enp6s18 allmulticast on` must be set
//! (or persistently via `/etc/systemd/network/...`). Reason: the
//! virtio-net MC hash filter drops frames at L2 before
//! `IP_ADD_MEMBERSHIP` takes effect. Without allmulti the VM sees
//! `tcpdump -p` 0 packets, with promisc all of them, yet the kernel
//! counter still increments — a known virtio sync issue. Details in
//! memory `reference_pve_multicast_setup`.
//!
//! # Reliable SEDP AckNack loop (working)
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
//! # Why "#[ignore]"?
//!
//! - Cyclone binary not in CI
//! - SSH access + password are maintenance-heavy
//! - network dependency (LAN setup)
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

const SSH_USER: &str = "llvm";
const SSH_PASS: &str = "llvm";
const SSH_HOST: &str = "llvm";
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
    /// Starts `ddsperf -D <domain> pub 1Hz` over SSH on the remote
    /// host. Holds the subprocess handle so `Drop` kills the remote
    /// ddsperf.
    fn start() -> std::io::Result<Self> {
        // CYCLONEDDS_URI pins Cyclone to enp6s18 (the VM LAN interface).
        // Without it, Cyclone picks some arbitrary interface (possibly
        // loopback) and multicast never reaches the macOS host.
        let remote_cmd = format!(
            "CYCLONEDDS_URI=file:///tmp/cyc.xml timeout 18 ddsperf -D {CYCLONE_DOMAIN} pub 1Hz > /tmp/cyclone_live.log 2>&1"
        );
        let child = Command::new("sshpass")
            .arg("-p")
            .arg(SSH_PASS)
            .arg("ssh")
            .arg("-o")
            .arg("StrictHostKeyChecking=no")
            .arg("-o")
            .arg("ConnectTimeout=5")
            .arg(format!("{SSH_USER}@{SSH_HOST}"))
            .arg(&remote_cmd)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        Ok(Self { child })
    }
}

impl Drop for CycloneSubprocess {
    fn drop(&mut self) {
        // Best-effort: pkill remote ddsperf, then detach the local
        // child.
        let _ = Command::new("sshpass")
            .arg("-p")
            .arg(SSH_PASS)
            .arg("ssh")
            .arg("-o")
            .arg("StrictHostKeyChecking=no")
            .arg(format!("{SSH_USER}@{SSH_HOST}"))
            .arg("pkill -f 'ddsperf.*-D 42' || true")
            .output();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Reads `ipconfig getifaddr en0` on macOS to determine the local LAN
/// IP. The test needs a real IP (not UNSPECIFIED), because Cyclone
/// sends SEDP to the unicast locator announced in the beacon.
fn detect_local_interface() -> Ipv4Addr {
    if let Ok(out) = Command::new("ipconfig").args(["getifaddr", "en0"]).output() {
        if let Ok(s) = core::str::from_utf8(&out.stdout) {
            if let Ok(ip) = s.trim().parse::<Ipv4Addr>() {
                return ip;
            }
        }
    }
    Ipv4Addr::UNSPECIFIED
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
        default_unicast_locator: Some(Locator::udp_v4(local_ip.octets(), u32::from(unicast_port))),
        default_multicast_locator: Some(Locator::udp_v4(
            SPDP_MULTICAST_GROUP.octets(),
            u32::from(SPDP_MULTICAST_PORT),
        )),
        // Metatraffic locator = our unicast socket. Cyclone sends SEDP
        // publications exactly there after it matches our beacon.
        // Without this PID, Cyclone does not route SEDP back.
        metatraffic_unicast_locator: Some(Locator::udp_v4(
            local_ip.octets(),
            u32::from(unicast_port),
        )),
        metatraffic_multicast_locator: Some(Locator::udp_v4(
            SPDP_MULTICAST_GROUP.octets(),
            u32::from(SPDP_MULTICAST_PORT),
        )),
        // Domain 42 matching Cyclone's `-D 42`. Without DOMAIN_ID,
        // Cyclone counts the beacon as domain 0 → no match.
        domain_id: Some(CYCLONE_DOMAIN),
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
    }
}

#[test]
#[ignore = "requires SSH access to llvm@llvm + Cyclone DDS 0.10.2"]
fn cyclone_live_sedp_discovery() {
    let interface = detect_local_interface();
    assert!(
        !interface.is_unspecified(),
        "could not detect local LAN IP (ipconfig getifaddr en0 failed); \
         test needs a real IP for Cyclone to send SEDP unicast back"
    );
    eprintln!("local interface: {interface}");

    // Multicast recv socket for SPDP beacons from Cyclone.
    let spdp_sock = UdpSocket::bind(SocketAddrV4::new(
        Ipv4Addr::UNSPECIFIED,
        SPDP_MULTICAST_PORT,
    ))
    .expect("bind spdp multicast port");
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

    // Start ddsperf on the bench host.
    let _cyclone = CycloneSubprocess::start().expect("start ddsperf over ssh");
    eprintln!("started remote ddsperf");

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
                        p.data.default_unicast_locator
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

    // Stop the sender thread before the assertion so the remote ddsperf
    // doesn't keep running forever.
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
    // 2. Our beacon was sent out (tcpdump confirms the correct
    //    multicast frame on enp6s18).
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
        "expected Cyclone to unicast-reply to our metatraffic_unicast_locator, got {cyclone_unicast_inbound} packets (set `ip link set enp6s18 allmulticast on` on llvm if needed)"
    );
    assert!(
        publications_seen >= 1,
        "expected at least 1 SEDP publication after preemptive AckNack, got {publications_seen}"
    );
    thread::sleep(Duration::from_millis(100));
}
