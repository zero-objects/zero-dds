//! Cyclone DDS against an SPDP beacon that carries ZeroDDS' security
//! capabilities (`zerodds-security-1.2` §14, coverage item CV-22).
//!
//! **Opt-in** — marked `#[ignore]`, and without `INTEROP_REQUIRE=1` (or
//! `INTEROP_HOST_AVAILABLE=1`) it reports a skip instead of running. The
//! interop pillar's job `vendor-live-dcps`
//! (`ci/interop-omg-weekly.yml`) sets it and has Cyclone's `ddsperf` on the
//! path:
//!
//! ```bash
//! INTEROP_REQUIRE=1 cargo test -p zerodds-security-runtime \
//!     --test cyclone_live_security_caps -- --ignored --nocapture
//! ```
//!
//! `DDSPERF` names the binary when it is not `ddsperf` on the path.
//!
//! # What is shown
//!
//! Architecture document §8.1 says Cyclone does not know ZeroDDS'
//! capability properties and ignores them. The beacon here carries the
//! capabilities of a full-encrypt participant in `PID_PROPERTY_LIST`
//! (`advertise_security_caps`) and announces the SEDP endpoints. Cyclone
//! accepted it when Cyclone addresses this participant: a datagram from
//! Cyclone's GUID prefix arrives at the metatraffic unicast locator the
//! beacon names, which Cyclone learns from the beacon alone. Cyclone's own
//! multicast beacons do not show that: it sends them whether it took ours
//! or not.
//!
//! The domain is leased (`zerodds-test-domains`), so a run on the CI runner
//! host and one on another host of the LAN never share it.
//!
//! Before, the test started `ddsperf` over SSH on a lab host and passed when
//! it saw Cyclone's multicast beacon.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
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

const SPDP_MULTICAST_GROUP: Ipv4Addr = Ipv4Addr::new(239, 255, 0, 1);
/// How long the test waits for Cyclone's beacon and its directed reply.
const TEST_TIMEOUT: Duration = Duration::from_secs(15);
const BEACON_INTERVAL: Duration = Duration::from_millis(500);
/// Local GuidPrefix — must be != the Cyclone prefix.
const LOCAL_PREFIX: [u8; 12] = [0xE2; 12];

/// RTPS 2.5 §9.6.1.4.1: the SPDP multicast port of `domain`.
fn spdp_multicast_port(domain: i32) -> u16 {
    u16::try_from(7400 + 250 * domain).expect("SPDP port of a leased domain")
}

struct CycloneSubprocess {
    child: Child,
}

impl CycloneSubprocess {
    /// `ddsperf -i <domain> -D 20 pub 1Hz` on this host (`-i` is the domain,
    /// `-D` the duration in seconds).
    fn start(domain: i32) -> std::io::Result<Self> {
        let binary = std::env::var_os("DDSPERF").unwrap_or_else(|| "ddsperf".into());
        let child = Command::new(binary)
            .args(["-i", &domain.to_string(), "-D", "20", "pub", "1Hz"])
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

/// The address a packet to another network would leave from: Cyclone sends
/// to the unicast locator the beacon announces, so it has to be a real
/// address.
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

fn build_secure_beacon_data(
    domain: i32,
    local_ip: Ipv4Addr,
    unicast_port: u16,
) -> ParticipantBuiltinTopicData {
    let flags = endpoint_flag::PARTICIPANT_ANNOUNCER
        | endpoint_flag::PARTICIPANT_DETECTOR
        | endpoint_flag::PUBLICATIONS_ANNOUNCER
        | endpoint_flag::PUBLICATIONS_DETECTOR
        | endpoint_flag::SUBSCRIPTIONS_ANNOUNCER
        | endpoint_flag::SUBSCRIPTIONS_DETECTOR;
    let multicast = Locator::udp_v4(
        SPDP_MULTICAST_GROUP.octets(),
        u32::from(spdp_multicast_port(domain)),
    );
    let unicast = Locator::udp_v4(local_ip.octets(), u32::from(unicast_port));
    let mut data = ParticipantBuiltinTopicData {
        guid: Guid::new(GuidPrefix::from_bytes(LOCAL_PREFIX), EntityId::PARTICIPANT),
        protocol_version: ProtocolVersion::V2_5,
        vendor_id: VendorId::ZERODDS,
        default_unicast_locators: vec![unicast],
        default_multicast_locators: vec![multicast],
        metatraffic_unicast_locators: vec![unicast],
        metatraffic_multicast_locators: vec![multicast],
        domain_id: Some(u32::try_from(domain).expect("leased domain")),
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
    };

    // The capabilities of a full-encrypt participant in the PropertyList.
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

/// The GUID prefix of an RTPS message's header (RTPS 2.5 §9.4.4: "RTPS",
/// version, vendor, then the prefix).
fn rtps_sender(datagram: &[u8]) -> Option<GuidPrefix> {
    if datagram.len() < 20 || &datagram[..4] != b"RTPS" {
        return None;
    }
    let mut prefix = [0u8; 12];
    prefix.copy_from_slice(&datagram[8..20]);
    Some(GuidPrefix::from_bytes(prefix))
}

#[test]
#[ignore = "needs Cyclone DDS ddsperf on this host; vendor-live-dcps runs it"]
fn cyclone_accepts_beacon_with_security_caps() {
    if std::env::var("INTEROP_REQUIRE").as_deref() != Ok("1")
        && std::env::var("INTEROP_HOST_AVAILABLE").as_deref() != Ok("1")
    {
        eprintln!(
            "INTEROP-SKIP[cyclone_live_security_caps]: opt in with INTEROP_REQUIRE=1 and ddsperf"
        );
        return;
    }
    let lease = zerodds_test_domains::lease_domain();
    let domain = lease.id();
    let port = spdp_multicast_port(domain);
    let interface = detect_local_interface();
    assert!(
        !interface.is_unspecified(),
        "could not detect a local IPv4 address; Cyclone needs a real one to send to"
    );
    eprintln!("domain {domain}, interface {interface}");

    // The SPDP multicast port, shared with the Cyclone on this host: without
    // SO_REUSEADDR on both, the second bind fails.
    let spdp_sock: UdpSocket = {
        let s = socket2::Socket::new(
            socket2::Domain::IPV4,
            socket2::Type::DGRAM,
            Some(socket2::Protocol::UDP),
        )
        .expect("spdp socket");
        s.set_reuse_address(true).expect("SO_REUSEADDR");
        s.bind(&SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port).into())
            .expect("bind the SPDP multicast port");
        s.into()
    };
    spdp_sock
        .set_read_timeout(Some(Duration::from_millis(100)))
        .unwrap();
    spdp_sock
        .join_multicast_v4(&SPDP_MULTICAST_GROUP, &interface)
        .expect("join the SPDP multicast group");

    // The metatraffic unicast locator the beacon announces.
    let unicast_sock =
        UdpSocket::bind(SocketAddrV4::new(interface, 0)).expect("bind unicast socket");
    unicast_sock
        .set_read_timeout(Some(Duration::from_millis(100)))
        .unwrap();
    let unicast_port = unicast_sock.local_addr().unwrap().port();

    let sender_sock = UdpSocket::bind(SocketAddrV4::new(interface, 0)).expect("bind sender socket");
    sender_sock.set_multicast_ttl_v4(32).unwrap();

    let data = build_secure_beacon_data(domain, interface, unicast_port);
    let our_prefix = data.guid.prefix;
    assert!(
        !data.properties.is_empty(),
        "pre-condition: the beacon carries the security properties"
    );
    eprintln!(
        "outbound beacon carries {} security properties",
        data.properties.len()
    );
    let mut beacon = SpdpBeacon::new(data);
    let stop = Arc::new(AtomicBool::new(false));
    let stop_sender = Arc::clone(&stop);
    let mc_dest = SocketAddrV4::new(SPDP_MULTICAST_GROUP, port);
    let send_handle = thread::spawn(move || {
        while !stop_sender.load(Ordering::Relaxed) {
            if let Ok(d) = beacon.serialize() {
                let _ = sender_sock.send_to(&d, mc_dest);
            }
            thread::sleep(BEACON_INTERVAL);
        }
    });

    let _cyclone = CycloneSubprocess::start(domain).expect("start ddsperf");

    let reader = SpdpReader::new();
    let mut cyclone_prefix: Option<GuidPrefix> = None;
    let mut addressed_to_us = 0usize;
    let mut buf = vec![0u8; 65_536];
    let start = Instant::now();
    while start.elapsed() < TEST_TIMEOUT && addressed_to_us == 0 {
        if let Ok(n) = spdp_sock.recv(&mut buf)
            && let Ok(disc) = reader.parse_datagram(&buf[..n])
            && disc.sender_prefix != our_prefix
            && disc.sender_vendor == VendorId::CYCLONE
        {
            if cyclone_prefix.is_none() {
                eprintln!(
                    "Cyclone's beacon: prefix {:?}, {} properties",
                    disc.sender_prefix,
                    disc.data.properties.len()
                );
            }
            cyclone_prefix = Some(disc.sender_prefix);
        }
        if let Ok(n) = unicast_sock.recv(&mut buf)
            && let (Some(from), Some(cyclone)) = (rtps_sender(&buf[..n]), cyclone_prefix)
            && from == cyclone
        {
            addressed_to_us += 1;
        }
    }
    stop.store(true, Ordering::Relaxed);
    let _ = send_handle.join();

    let cyclone = cyclone_prefix.unwrap_or_else(|| {
        panic!("no SPDP beacon of Cyclone on domain {domain} within {TEST_TIMEOUT:?}")
    });
    assert!(
        addressed_to_us >= 1,
        "Cyclone ({cyclone:?}) sent nothing to the unicast locator of the beacon with the \
         security properties within {TEST_TIMEOUT:?}: it did not take the participant"
    );
    eprintln!("Cyclone addressed the participant {addressed_to_us} time(s)");
}
