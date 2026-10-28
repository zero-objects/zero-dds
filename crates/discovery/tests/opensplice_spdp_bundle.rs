//! OpenSplice announces every participant its DDSI2 service hosts in one
//! SPDP datagram (`crates/rtps/tests/fixtures/opensplice/spdp_bundle.hex`,
//! captured, provenance in the file). The participant with the SEDP
//! detectors stands last in it; a reader of the first sample alone never
//! gave the local SEDP writers a proxy to announce to.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use zerodds_discovery::sedp::SedpStack;
use zerodds_discovery::spdp::{SpdpError, SpdpReader, SpdpSample};
use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix, VendorId};

const FIXTURE: &str = include_str!("../../rtps/tests/fixtures/opensplice/spdp_bundle.hex");

fn datagram() -> Vec<u8> {
    let hex: String = FIXTURE
        .lines()
        .map(|line| line.split('#').next().unwrap_or(""))
        .collect::<Vec<_>>()
        .concat()
        .split_whitespace()
        .collect();
    (0..hex.len() / 2)
        .map(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).expect("valid hex"))
        .collect()
}

fn prefix(entity: u8) -> GuidPrefix {
    let hi = if entity == 0xd0 { 0x02 } else { 0x00 };
    GuidPrefix::from_bytes([
        0x52, 0x9a, 0xd8, 0x98, 0x00, 0x00, hi, entity, 0x00, 0x00, 0x00, 0x01,
    ])
}

#[test]
fn every_participant_of_the_bundle_is_read_with_its_own_endpoint_set() {
    let samples = SpdpReader::new().samples(&datagram()).expect("SPDP");
    let read: Vec<(GuidPrefix, u32)> = samples
        .iter()
        .map(|s| match s {
            SpdpSample::Announced(p) => {
                assert_eq!(p.sender_prefix, p.data.guid.prefix);
                assert_eq!(p.sender_vendor, VendorId([0x01, 0x02]));
                (p.sender_prefix, p.data.builtin_endpoint_set)
            }
            other => panic!("an announcement, not {other:?}"),
        })
        .collect();
    assert_eq!(
        read,
        vec![
            (prefix(0xd0), 0x0000_0415),
            (prefix(0x7d), 0x0000_0415),
            (prefix(0x58), 0x0000_0415),
            (prefix(0x04), 0x0000_0415),
            (prefix(0x81), 0x0000_3c3f),
        ]
    );
}

#[test]
fn parse_datagram_refuses_the_bundle() {
    assert_eq!(
        SpdpReader::new().parse_datagram(&datagram()),
        Err(SpdpError::Bundled(5))
    );
}

#[test]
fn the_participant_with_the_detectors_gets_the_local_sedp_writers_proxies() {
    let local = GuidPrefix::from_bytes([
        0x91, 0x83, 0x92, 0x48, 0xfb, 0x11, 0x0c, 0x00, 0x32, 0x97, 0x32, 0x5e,
    ]);
    let mut stack = SedpStack::new(local, VendorId::ZERODDS);
    for sample in SpdpReader::new().samples(&datagram()).expect("SPDP") {
        if let SpdpSample::Announced(p) = sample {
            stack.on_participant_discovered(&p);
        }
    }
    let readers = |proxies: &[zerodds_rtps::reader_proxy::ReaderProxy]| {
        proxies
            .iter()
            .map(|p| p.remote_reader_guid)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        readers(stack.pub_writer().inner().reader_proxies()),
        vec![Guid::new(
            prefix(0x81),
            EntityId::SEDP_BUILTIN_PUBLICATIONS_READER
        )]
    );
    assert_eq!(
        readers(stack.sub_writer().inner().reader_proxies()),
        vec![Guid::new(
            prefix(0x81),
            EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_READER
        )]
    );
}

#[test]
fn one_participants_sample_cut_out_of_the_bundle_reads_as_a_beacon() {
    let reader = SpdpReader::new();
    let bundle = datagram();
    let alone = reader
        .sample_datagram(&bundle, prefix(0x81))
        .expect("the last participant's sample");
    let p = reader.parse_datagram(&alone).expect("one sample");
    assert_eq!(p.data.guid.prefix, prefix(0x81));
    assert_eq!(p.data.builtin_endpoint_set, 0x0000_3c3f);
}
