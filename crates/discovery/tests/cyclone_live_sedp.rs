//! WP 1.4 T6b — Live-Interop-Test gegen echte Cyclone-DDS-Instanz.
//!
//! **Opt-in only** — `#[ignore]` markiert. Aufruf:
//!
//! ```bash
//! cargo test -p zerodds-discovery --test cyclone_live_sedp -- --ignored --nocapture
//! ```
//!
//! # Voraussetzungen
//!
//! - SSH-Zugriff auf `llvm@llvm` mit Passwort `llvm` (Lab-Setup)
//! - `sshpass` lokal installiert
//! - Cyclone DDS 0.10.2 (`ddsperf` Binary) auf `llvm`
//! - `/tmp/cyc.xml` auf `llvm` pinnt Cyclone auf `enp6s18`
//! - Lokaler Host im selben LAN wie `llvm`
//! - **Multicast-Durchlass zwischen PVE-VM und Client.** PVE hat eine
//!   2-Level-Bridge-Hierarchie (`vmbr0` + `fwbr113i0`). Beide brauchen
//!   `multicast_querier=1` + `multicast_snooping=0`, sonst filtert die
//!   Bridge Multicast. Details in Memory `reference_pve_multicast_setup`.
//!
//! # Test-Ablauf
//!
//! 1. Lokal: Unicast-Socket (fuer SEDP-Antworten) + Multicast-Socket
//!    (fuer SPDP-Beacons) + Sender-Socket fuer unseren Beacon binden.
//! 2. SpdpBeacon-Sender-Thread: alle 500ms eigenes SPDP-Datagram mit
//!    allen SEDP-Endpoint-Flags auf 239.255.0.1:17900 schicken. Damit
//!    sieht Cyclone uns als matched peer und schickt SEDP-Publications
//!    via Unicast an unseren `default_unicast_locator`.
//! 3. SSH-Subprocess startet `ddsperf -D 42 pub 1Hz` auf `llvm`
//!    → Cyclone sendet SPDP-Beacons + SEDP-Publications.
//! 4. Lokal poll-loopen beide Sockets (Multicast + Unicast). SPDP →
//!    `SedpStack::on_participant_discovered`; SEDP → `handle_datagram`
//!    → Publications landen im Cache.
//! 5. Assert: Cyclone-SPDP-Discovery + Beacon-Outbound verifiziert.
//!    Publications werden optional mitgeloggt.
//! 6. Teardown: SSH-pkill auf `ddsperf`, Sender-Thread stoppen.
//!
//! # VM-Kernel-Multicast-Fix (erforderlich auf llvm, 2026-04-19)
//!
//! Auf der VM muss `ip link set dev enp6s18 allmulticast on` gesetzt
//! sein (oder persistent via `/etc/systemd/network/...`). Grund:
//! virtio-net MC-Hashfilter droppt Frames auf L2 bevor `IP_ADD_MEMBERSHIP`
//! greift. Ohne allmulti sieht die VM `tcpdump -p` 0 Pakete, mit promisc
//! alle, Kernel-Counter steigt trotzdem — bekanntes virtio-Sync-Problem.
//! Details in Memory `reference_pve_multicast_setup`.
//!
//! # Reliable-SEDP AckNack-Loop (funktioniert)
//!
//! Der Test tickt den SedpStack periodisch. `add_writer_proxy` scheduled
//! ein preemptives AckNack mit `final=false`, das dem Cyclone-Writer
//! signalisiert "Reader ist bereit, schick alles ab Anfang". Jeder
//! AckNack/NackFrag ist mit INFO_DST auf Cyclone's GuidPrefix geroutet
//! (RTPS 2.5 §8.3.7.6 — ohne INFO_DST verwirft Cyclone ACKNACK als
//! "not a connection"). Cyclone antwortet mit Heartbeat + DATA.
//! Heartbeat-Dispatch im SedpStack akzeptiert auch reader_id=UNKNOWN,
//! da Cyclone das haeufig so setzt und die Adressierung via INFO_DST
//! macht.
//!
//! # Warum "#[ignore]"?
//!
//! - Cyclone-Binary nicht im CI
//! - SSH-Zugang + Passwort pflegeintensiv
//! - Netzwerk-Abhaengigkeit (LAN-Setup)
//!
//! Der deterministische Teil ist in `cyclone_sedp_replay.rs` ohne
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
/// Multicast-Port fuer Domain 42: 7400 + 250 * 42 = 17900.
const SPDP_MULTICAST_PORT: u16 = 17900;
const SPDP_MULTICAST_GROUP: Ipv4Addr = Ipv4Addr::new(239, 255, 0, 1);
/// Timeout fuer den gesamten Test-Run.
const TEST_TIMEOUT: Duration = Duration::from_secs(20);
/// Wie lange wir auf SPDP warten, bevor wir abbrechen.
const SPDP_DISCOVERY_TIMEOUT: Duration = Duration::from_secs(8);
/// Beacon-Sende-Intervall.
const BEACON_INTERVAL: Duration = Duration::from_millis(500);
/// Lokaler GuidPrefix — muss != Cyclone-Prefix sein, sonst kollidiert.
const LOCAL_PREFIX: [u8; 12] = [0xEE; 12];

struct CycloneSubprocess {
    child: Child,
}

impl CycloneSubprocess {
    /// Startet `ddsperf -D <domain> pub 1Hz` ueber SSH auf dem
    /// Remote-Host. Haelt den Subprocess-Handle, damit `Drop` das
    /// Remote-ddsperf killt.
    fn start() -> std::io::Result<Self> {
        // CYCLONEDDS_URI pinnt Cyclone auf enp6s18 (VM-LAN-Interface).
        // Ohne das waehlt Cyclone irgendein Interface (ggf. loopback)
        // und Multicast erreicht den Mac nicht.
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
        // Best-effort: pkill ddsperf remote, dann lokales Child
        // entkoppeln.
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

/// Liest `ipconfig getifaddr en0` auf macOS um die eigene LAN-IP zu
/// ermitteln. Der Test braucht eine echte IP (kein UNSPECIFIED), weil
/// Cyclone SEDP an den im Beacon angekuendigten Unicast-Locator schickt.
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

/// Baut unsere ParticipantBuiltinTopicData mit allen SEDP-Flags.
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
        // Metatraffic-Locator = unser Unicast-Socket. Cyclone schickt
        // SEDP-Publications genau dorthin, nachdem es unseren Beacon
        // matched. Ohne diese PID routet Cyclone SEDP nicht zurueck.
        metatraffic_unicast_locator: Some(Locator::udp_v4(
            local_ip.octets(),
            u32::from(unicast_port),
        )),
        metatraffic_multicast_locator: Some(Locator::udp_v4(
            SPDP_MULTICAST_GROUP.octets(),
            u32::from(SPDP_MULTICAST_PORT),
        )),
        // Domain 42 passend zu Cyclones `-D 42`. Ohne DOMAIN_ID
        // gehoert Cyclone den Beacon als Domain-0 zaehlen → no-match.
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

    // Multicast-Recv-Socket fuer SPDP-Beacons von Cyclone.
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

    // Unicast-Recv-Socket fuer SEDP-Antworten. Bindet auf eine freie
    // Portnummer auf dem LAN-Interface — genau diese Adresse
    // annoncieren wir als default_unicast_locator im SPDP-Beacon.
    let unicast_sock =
        UdpSocket::bind(SocketAddrV4::new(interface, 0)).expect("bind unicast socket");
    unicast_sock
        .set_read_timeout(Some(Duration::from_millis(100)))
        .unwrap();
    let unicast_port = unicast_sock.local_addr().unwrap().port();
    eprintln!("unicast bound on {interface}:{unicast_port}");

    // Beacon-Sender-Socket. Bindet auf das LAN-Interface — das Kernel-
    // Multicast-Routing folgt bei gebundener Source-IP dem passenden
    // Interface. `set_multicast_if_v4` ist in std::net nicht verfuegbar;
    // explizites bind reicht hier.
    let sender_sock = UdpSocket::bind(SocketAddrV4::new(interface, 0)).expect("bind sender socket");
    sender_sock
        .set_multicast_ttl_v4(32)
        .expect("set multicast ttl");

    // Eigene Participant-Daten + Beacon.
    let our_data = build_local_participant(interface, unicast_port);
    let our_prefix = our_data.guid.prefix;
    let mut beacon = SpdpBeacon::new(our_data);

    // Sender-Thread: sendet periodisch unseren Beacon auf Multicast.
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

    // ddsperf auf llvm starten.
    let _cyclone = CycloneSubprocess::start().expect("start ddsperf over ssh");
    eprintln!("started remote ddsperf");

    // Haupt-Loop: beide Sockets pollen.
    let spdp = SpdpReader::new();
    let mut stack = SedpStack::new(GuidPrefix::from_bytes(LOCAL_PREFIX), VendorId::ZERODDS);
    let mut cyclone_discovered = false;
    let mut publications_seen = 0usize;
    let mut cyclone_unicast_inbound = 0usize;
    let mut buf = vec![0u8; 65535];

    let start = Instant::now();
    while start.elapsed() < TEST_TIMEOUT {
        // Beide Sockets nacheinander pollen (kurze Timeouts).
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

            // SPDP-Beacon? → Participant-Discovery.
            if let Ok(p) = spdp.parse_datagram(datagram) {
                // Eigene Beacons ignorieren (Loopback kann passieren).
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

            // Kein SPDP → evtl. SEDP. Nur verarbeiten, wenn Cyclone
            // bereits entdeckt (sonst fehlt der WriterProxy).
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

        // Reader-tick: ACKNACK/NACK_FRAG an Cyclone schicken, damit der
        // reliable Writer seine DATA-Publications liefert.
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

    // Sender-Thread stoppen vor Assertion, damit Remote-ddsperf nicht
    // noch ewig weiter laeuft.
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

    // Akzeptanz:
    // 1. Cyclone-SPDP-Discovery (Cyclone-Beacon byte-genau geparst).
    // 2. Unser Beacon wurde raus geschickt (tcpdump bestaetigt korrekten
    //    Multicast-Frame auf enp6s18).
    // 3. Cyclone antwortet auf unserem Unicast-Port — Beweis, dass
    //    Cyclone uns als matched peer mit den neuen METATRAFFIC_*_LOCATOR
    //    + DOMAIN_ID PIDs akzeptiert hat. Ohne die neuen PIDs sieht
    //    Cyclone uns nicht.
    //
    // Reliable-SEDP Publications kommen erst mit Reader-tick + AckNack
    // (separater Schritt, siehe Header-Kommentar).
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
