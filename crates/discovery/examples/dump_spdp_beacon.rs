//! Dump ein SPDP-Beacon-Datagramm als raw bytes nach stdout.
//!
//! Aufruf:
//! ```bash
//! cargo run --example dump_spdp_beacon -- 192.168.178.60 54321 42 > /tmp/beacon.bin
//! ```
//!
//! Verwendet fuer Multicast-Sanity-Check: Binary nach llvm kopieren,
//! dort via Python periodisch auf 239.255.0.1:17900 senden, pruefen
//! ob Cyclone auf llvm uns als matched peer sieht.

#![allow(clippy::expect_used, clippy::print_stderr)]

use std::io::Write;

use zerodds_discovery::SpdpBeacon;
use zerodds_rtps::participant_data::{
    Duration as DdsDuration, ParticipantBuiltinTopicData, endpoint_flag,
};
use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix, Locator, ProtocolVersion, VendorId};

fn main() {
    let mut args = std::env::args().skip(1);
    let ip_str = args
        .next()
        .expect("usage: dump_spdp_beacon <ip> <uc_port> <domain>");
    let uc_port: u16 = args.next().expect("uc_port").parse().expect("uc_port num");
    let domain: u32 = args.next().expect("domain").parse().expect("domain num");

    let ip: [u8; 4] = ip_str
        .split('.')
        .map(|s| s.parse().expect("octet"))
        .collect::<Vec<u8>>()
        .try_into()
        .expect("4 octets");

    let flags = endpoint_flag::PARTICIPANT_ANNOUNCER
        | endpoint_flag::PARTICIPANT_DETECTOR
        | endpoint_flag::PUBLICATIONS_ANNOUNCER
        | endpoint_flag::PUBLICATIONS_DETECTOR
        | endpoint_flag::SUBSCRIPTIONS_ANNOUNCER
        | endpoint_flag::SUBSCRIPTIONS_DETECTOR;
    // mc-port fuer Domain: 7400 + 250 * domain
    let mc_port = 7400 + 250 * domain;

    let data = ParticipantBuiltinTopicData {
        guid: Guid::new(GuidPrefix::from_bytes([0xEE; 12]), EntityId::PARTICIPANT),
        protocol_version: ProtocolVersion::V2_5,
        vendor_id: VendorId::ZERODDS,
        default_unicast_locator: Some(Locator::udp_v4(ip, u32::from(uc_port))),
        default_multicast_locator: Some(Locator::udp_v4([239, 255, 0, 1], mc_port)),
        metatraffic_unicast_locator: Some(Locator::udp_v4(ip, u32::from(uc_port))),
        metatraffic_multicast_locator: Some(Locator::udp_v4([239, 255, 0, 1], mc_port)),
        domain_id: Some(domain),
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

    let mut beacon = SpdpBeacon::new(data);
    let datagram = beacon.serialize().expect("serialize beacon");
    eprintln!("wrote {} beacon bytes to stdout", datagram.len());
    std::io::stdout()
        .write_all(&datagram)
        .expect("write stdout");
}
