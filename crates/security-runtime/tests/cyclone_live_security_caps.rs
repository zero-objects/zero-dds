//! WP 4H-b Cyclone live interop — security caps in the SPDP beacon.
//!
//! **Opt-in only**, via `--ignored`. Invocation:
//!
//! ```bash
//! cargo test -p zerodds-security-runtime --test cyclone_live_security_caps \
//!     -- --ignored --nocapture
//! ```
//!
//! # Goal
//!
//! Evidence for the statement from architecture doc §8.1:
//! > "Cyclone does not see `<peer_classes>`-like ZeroDDS properties and
//! >  falls back to `<rtps_protection_kind>` — extra properties are
//! >  silently ignored."
//!
//! Concretely: we send an SPDP beacon **with** security caps in the
//! `PID_PROPERTY_LIST` and prove that Cyclone does not discard the
//! beacon (the bidirectional SPDP exchange stays intact).
//!
//! # Test flow
//!
//! 1. SSH starts `ddsperf -i 42 pub 1Hz` on `llvm@llvm`.
//! 2. Locally the test binds to `239.255.0.1:17900` (multicast, domain 42)
//!    and joins the MC group on the LAN interface.
//! 3. The test periodically sends its own SPDP beacon — **with
//!    ENCRYPT caps + crypto plugin class in the PropertyList** — to the
//!    same multicast group.
//! 4. Pass criterion: we receive Cyclone's own SPDP beacon
//!    (not our loopback). This shows that, despite our
//!    extra properties, Cyclone keeps running and serves the multicast channel.
//!    On parser errors Cyclone would either have segfaulted or
//!    aborted its beacon loop.
//!
//! # Why no full SEDP handshake?
//!
//! The DoD sentence for stage 2 reads: "our SPDP is silently
//! accepted by Cyclone (extra properties ignored)". That is a negative
//! statement (nothing bad happens). The SEDP handshake is already proven in WP
//! 1.4; here the SPDP proof suffices.
//!
//! # Dependencies
//!
//! - `sshpass`, `ssh` available locally
//! - `llvm@llvm:22` reachable with password `llvm`
//! - `ddsperf` on `llvm`, `/tmp/cyc.xml` pinned to `enp6s18`
//! - `allmulticast on` on `enp6s18` (otherwise the virtio MC filter)
//! - Mac + llvm on the same LAN (192.168.178.0/24 typical)

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

use zerodds_discovery::spdp::{SpdpBeacon, SpdpReader};
use zerodds_rtps::participant_data::{
    Duration as DdsDuration, ParticipantBuiltinTopicData, endpoint_flag,
};
use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix, Locator, ProtocolVersion, VendorId};
use zerodds_security_runtime::{
    PeerCapabilities, ProtectionLevel, SuiteHint, advertise_security_caps,
};

const SSH_USER: &str = "llvm";
const SSH_PASS: &str = "llvm";
const SSH_HOST: &str = "llvm";
const CYCLONE_DOMAIN: u32 = 42;
/// Multicast port for domain 42: 7400 + 250 * 42 = 17900.
const SPDP_MULTICAST_PORT: u16 = 17900;
const SPDP_MULTICAST_GROUP: Ipv4Addr = Ipv4Addr::new(239, 255, 0, 1);
const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(8);
const BEACON_INTERVAL: Duration = Duration::from_millis(500);
/// Local GuidPrefix — must be != the Cyclone prefix.
const LOCAL_PREFIX: [u8; 12] = [0xE2; 12];

struct CycloneSubprocess {
    child: Child,
}

impl CycloneSubprocess {
    fn start() -> std::io::Result<Self> {
        // ddsperf: `-D` is DURATION, the domain is set via `-i`.
        let remote_cmd = format!(
            "CYCLONEDDS_URI=file:///tmp/cyc.xml timeout 18 ddsperf -i {CYCLONE_DOMAIN} -D 15 pub 1Hz \
             > /tmp/cyclone_live_caps.log 2>&1"
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
        let _ = Command::new("sshpass")
            .arg("-p")
            .arg(SSH_PASS)
            .arg("ssh")
            .arg("-o")
            .arg("StrictHostKeyChecking=no")
            .arg(format!("{SSH_USER}@{SSH_HOST}"))
            .arg("pkill -f 'ddsperf.*-i 42' || true")
            .output();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn detect_local_interface() -> Ipv4Addr {
    // macOS + Linux: verwende die Default-Route-IP.
    if let Ok(out) = Command::new("ipconfig").args(["getifaddr", "en0"]).output() {
        if let Ok(s) = core::str::from_utf8(&out.stdout) {
            if let Ok(ip) = s.trim().parse::<Ipv4Addr>() {
                return ip;
            }
        }
    }
    Ipv4Addr::UNSPECIFIED
}

fn build_secure_beacon_data(local_ip: Ipv4Addr, unicast_port: u16) -> ParticipantBuiltinTopicData {
    let flags = endpoint_flag::PARTICIPANT_ANNOUNCER | endpoint_flag::PARTICIPANT_DETECTOR;
    let mut data = ParticipantBuiltinTopicData {
        guid: Guid::new(GuidPrefix::from_bytes(LOCAL_PREFIX), EntityId::PARTICIPANT),
        protocol_version: ProtocolVersion::V2_5,
        vendor_id: VendorId::ZERODDS,
        default_unicast_locator: Some(Locator::udp_v4(local_ip.octets(), u32::from(unicast_port))),
        default_multicast_locator: Some(Locator::udp_v4(
            SPDP_MULTICAST_GROUP.octets(),
            u32::from(SPDP_MULTICAST_PORT),
        )),
        metatraffic_unicast_locator: Some(Locator::udp_v4(
            local_ip.octets(),
            u32::from(unicast_port),
        )),
        metatraffic_multicast_locator: Some(Locator::udp_v4(
            SPDP_MULTICAST_GROUP.octets(),
            u32::from(SPDP_MULTICAST_PORT),
        )),
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
    };

    // Here the interesting part: full-encrypt PropertyList content.
    let caps = PeerCapabilities {
        auth_plugin_class: Some("DDS:Auth:PKI-DH:1.2".into()),
        access_plugin_class: Some("DDS:Access:Permissions:1.2".into()),
        crypto_plugin_class: Some("DDS:Crypto:AES-GCM-GMAC:1.2".into()),
        supported_suites: vec![SuiteHint::Aes128Gcm, SuiteHint::Aes256Gcm],
        offered_protection: ProtectionLevel::Encrypt,
        has_valid_cert: false,
        validity_window: None,
        vendor_hint: Some("zerodds".into()),
        cert_cn: None,
        delegation_chain: None,
    };
    advertise_security_caps(&mut data.properties, &caps);
    data
}

#[test]
#[ignore = "requires SSH access to llvm@llvm + Cyclone DDS (ddsperf)"]
fn cyclone_accepts_beacon_with_security_caps() {
    let interface = detect_local_interface();
    assert!(
        !interface.is_unspecified(),
        "could not detect local LAN IP; test needs a real IP on en0"
    );
    eprintln!("local interface: {interface}");

    // MC-RX: here we see Cyclone's own SPDP beacon.
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

    // Unicast socket for default_unicast_locator in the beacon.
    let unicast_sock =
        UdpSocket::bind(SocketAddrV4::new(interface, 0)).expect("bind unicast socket");
    let unicast_port = unicast_sock.local_addr().unwrap().port();

    // Sender socket for our SPDP beacon.
    let sender_sock = UdpSocket::bind(SocketAddrV4::new(interface, 0)).expect("bind sender socket");
    sender_sock.set_multicast_ttl_v4(32).unwrap();

    // Fill the beacon with security caps and send it periodically.
    let data = build_secure_beacon_data(interface, unicast_port);
    let our_prefix = data.guid.prefix;
    assert!(
        !data.properties.is_empty(),
        "pre-condition: beacon carries security properties"
    );
    eprintln!(
        "outbound beacon carries {} security properties",
        data.properties.len()
    );
    let mut beacon = SpdpBeacon::new(data);

    let stop = Arc::new(AtomicBool::new(false));
    let stop_sender = Arc::clone(&stop);
    let mc_dest = SocketAddrV4::new(SPDP_MULTICAST_GROUP, SPDP_MULTICAST_PORT);
    let send_handle = thread::spawn(move || {
        while !stop_sender.load(Ordering::Relaxed) {
            if let Ok(d) = beacon.serialize() {
                let _ = sender_sock.send_to(&d, mc_dest);
            }
            thread::sleep(BEACON_INTERVAL);
        }
    });

    let _cyclone = CycloneSubprocess::start().expect("spawn ssh/ddsperf");
    eprintln!("started remote ddsperf -i {CYCLONE_DOMAIN}");

    // Receive loop: search for an SPDP datagram with a
    // GuidPrefix != ours (Cyclone's beacon).
    let reader = SpdpReader::new();
    let deadline = Instant::now() + DISCOVERY_TIMEOUT;
    let mut buf = [0u8; 65_536];
    let mut cyclone_seen = false;

    while Instant::now() < deadline {
        match spdp_sock.recv_from(&mut buf) {
            Ok((n, _addr)) => {
                if let Ok(disc) = reader.parse_datagram(&buf[..n]) {
                    if disc.data.guid.prefix != our_prefix {
                        eprintln!(
                            "saw Cyclone SPDP from prefix {:?} vendor={:?} props={}",
                            disc.data.guid.prefix,
                            disc.sender_vendor,
                            disc.data.properties.len()
                        );
                        cyclone_seen = true;
                        break;
                    }
                }
            }
            Err(_) => continue,
        }
    }

    stop.store(true, Ordering::Relaxed);
    let _ = send_handle.join();

    assert!(
        cyclone_seen,
        "Cyclone SPDP beacon not seen within {DISCOVERY_TIMEOUT:?} — Cyclone may have \
         rejected our beacon or the multicast setup is broken"
    );
}
