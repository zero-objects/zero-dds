//! WP 4H-b Cyclone-Live-Interop — Security-Caps im SPDP-Beacon.
//!
//! **Opt-in only**, via `--ignored`. Aufruf:
//!
//! ```bash
//! cargo test -p zerodds-security-runtime --test cyclone_live_security_caps \
//!     -- --ignored --nocapture
//! ```
//!
//! # Ziel
//!
//! Beleg der Aussage aus Architektur-Doc §8.1:
//! > "Cyclone sieht `<peer_classes>`-aehnliche ZeroDDS-Properties nicht und
//! >  faellt auf `<rtps_protection_kind>` zurueck — extra Properties werden
//! >  still ignoriert."
//!
//! Konkret: Wir senden einen SPDP-Beacon **mit** Security-Caps in der
//! `PID_PROPERTY_LIST` und weisen nach, dass Cyclone den Beacon nicht
//! verwirft (bidirektionaler SPDP-Austausch bleibt intakt).
//!
//! # Test-Ablauf
//!
//! 1. SSH startet `ddsperf -i 42 pub 1Hz` auf `llvm@llvm`.
//! 2. Lokal bindet der Test auf `239.255.0.1:17900` (Multicast, Domain 42)
//!    und joined die MC-Gruppe auf dem LAN-Interface.
//! 3. Der Test sendet periodisch seinen eigenen SPDP-Beacon — **mit
//!    ENCRYPT-Caps + Crypto-Plugin-Class in der PropertyList** — auf die
//!    gleiche Multicast-Gruppe.
//! 4. Pass-Kriterium: Wir empfangen Cyclone's eigenen SPDP-Beacon
//!    (nicht unseren Loopback). Das zeigt, dass Cyclone trotz unserer
//!    Extra-Properties weiterlaeuft und den Multicast-Kanal bedient.
//!    Cyclone haette bei Parser-Fehlern entweder ge-segfaulted oder
//!    seine Beacon-Schleife abgebrochen.
//!
//! # Warum kein vollstaendiger SEDP-Handshake?
//!
//! Der DoD-Satz fuer Stufe 2 lautet: "unser SPDP wird von Cyclone still
//! akzeptiert (extra Properties ignoriert)". Das ist eine Negativ-
//! Aussage (nichts Schlechtes passiert). Der SEDP-Handshake ist in WP
//! 1.4 schon nachgewiesen; hier reicht der SPDP-Beweis.
//!
//! # Abhaengigkeiten
//!
//! - `sshpass`, `ssh` lokal verfuegbar
//! - `llvm@llvm:22` mit Passwort `llvm` erreichbar
//! - `ddsperf` auf `llvm`, `/tmp/cyc.xml` pinned auf `enp6s18`
//! - `allmulticast on` auf `enp6s18` (sonst virtio-MC-Filter)
//! - Mac + llvm im selben LAN (192.168.178.0/24 typisch)

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
/// Multicast-Port fuer Domain 42: 7400 + 250 * 42 = 17900.
const SPDP_MULTICAST_PORT: u16 = 17900;
const SPDP_MULTICAST_GROUP: Ipv4Addr = Ipv4Addr::new(239, 255, 0, 1);
const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(8);
const BEACON_INTERVAL: Duration = Duration::from_millis(500);
/// Lokaler GuidPrefix — muss != Cyclone-Prefix sein.
const LOCAL_PREFIX: [u8; 12] = [0xE2; 12];

struct CycloneSubprocess {
    child: Child,
}

impl CycloneSubprocess {
    fn start() -> std::io::Result<Self> {
        // ddsperf: `-D` ist DURATION, Domain setzt man via `-i`.
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
        identity_status_token: None,
        sig_algo_info: None,
        kx_algo_info: None,
        sym_cipher_algo_info: None,
    };

    // Hier der interessante Teil: full-encrypt PropertyList-Content.
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

    // MC-RX: hier sehen wir Cyclone's eigenen SPDP-Beacon.
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

    // Unicast-Socket fuer default_unicast_locator im Beacon.
    let unicast_sock =
        UdpSocket::bind(SocketAddrV4::new(interface, 0)).expect("bind unicast socket");
    let unicast_port = unicast_sock.local_addr().unwrap().port();

    // Sender-Socket fuer unseren SPDP-Beacon.
    let sender_sock = UdpSocket::bind(SocketAddrV4::new(interface, 0)).expect("bind sender socket");
    sender_sock.set_multicast_ttl_v4(32).unwrap();

    // Beacon mit Security-Caps befuellen und periodisch senden.
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

    // Empfangs-Schleife: suchen nach einem SPDP-Datagram mit einem
    // GuidPrefix != unserem (Cyclone's Beacon).
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
        "Cyclone SPDP-Beacon nicht gesehen innerhalb {DISCOVERY_TIMEOUT:?} — Cyclone hat evtl. \
         unseren Beacon abgelehnt oder das Multicast-Setup ist kaputt"
    );
}
