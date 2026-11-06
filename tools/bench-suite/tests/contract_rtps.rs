// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-bench-suite` with `zerodds-rtps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-rtps` (`benches/rtps_fragmented.rs`,
//! `benches/transports_e2e.rs`, `benches/recv*.rs`): a `ReliableWriter` built
//! from `ReliableWriterConfig` with one `ReaderProxy`, `HistoryKind::KeepAll`,
//! `max_samples = 1`, `DEFAULT_HEARTBEAT_PERIOD`, `DEFAULT_FRAGMENT_SIZE` and
//! `DEFAULT_MTU`, written once per iteration; the bench labels a payload with
//! `ceil(size / DEFAULT_FRAGMENT_SIZE)` fragments. The transport benches build
//! `wire_types::Locator::udp_v4`. Oracle: DDSI-RTPS 2.5 §8.4.14.1 (a sample
//! larger than the fragment size goes out as DATA_FRAG, fragments numbered
//! from 1), §9.4.5.4/§9.4.5.5 (DATA 0x15, DATA_FRAG 0x16); UDP over Ethernet
//! carries 1472 octets (1500 − 20 IPv4 − 8 UDP).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use zerodds_rtps::datagram::{ParsedSubmessage, decode_datagram};
use zerodds_rtps::history_cache::HistoryKind;
use zerodds_rtps::message_builder::DEFAULT_MTU;
use zerodds_rtps::reader_proxy::ReaderProxy;
use zerodds_rtps::reliable_writer::{
    DEFAULT_FRAGMENT_SIZE, DEFAULT_HEARTBEAT_PERIOD, ReliableWriter, ReliableWriterConfig,
};
use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix, Locator, VendorId};

fn writer() -> ReliableWriter {
    let reader = ReaderProxy::new(
        Guid::new(
            GuidPrefix::from_bytes([2; 12]),
            EntityId::user_reader_with_key([0x40, 0x50, 0x60]),
        ),
        vec![Locator::udp_v4([127, 0, 0, 1], 7410)],
        vec![],
        true,
    );
    ReliableWriter::new(ReliableWriterConfig {
        guid: Guid::new(
            GuidPrefix::from_bytes([1; 12]),
            EntityId::user_writer_with_key([0x10, 0x20, 0x30]),
        ),
        vendor_id: VendorId::ZERODDS,
        reader_proxies: vec![reader],
        max_samples: 1,
        history_kind: HistoryKind::KeepAll,
        heartbeat_period: DEFAULT_HEARTBEAT_PERIOD,
        fragment_size: DEFAULT_FRAGMENT_SIZE,
        mtu: DEFAULT_MTU,
    })
}

/// (DATA count, DATA_FRAG fragment numbers) over all datagrams of one write.
fn sent(payload: &[u8]) -> (usize, Vec<u32>, usize) {
    let datagrams = writer()
        .write(payload)
        .expect("a fresh writer accepts one sample");
    let (mut data, mut fragments, mut largest) = (0, Vec::new(), 0);
    for datagram in &datagrams {
        largest = largest.max(datagram.bytes.len());
        assert_eq!(
            datagram.targets.as_slice(),
            &[Locator::udp_v4([127, 0, 0, 1], 7410)]
        );
        for frame in decode_datagram(&datagram.bytes).unwrap().submessages {
            match frame.submessage {
                ParsedSubmessage::Data(_) => data += 1,
                ParsedSubmessage::DataFrag(frag) => {
                    for offset in 0..u32::from(frag.fragments_in_submessage) {
                        fragments.push(frag.fragment_starting_num.0 + offset);
                    }
                }
                _ => {}
            }
        }
    }
    (data, fragments, largest)
}

#[test]
fn the_defaults_fit_an_ethernet_udp_datagram() {
    assert_eq!(DEFAULT_MTU, 1500 - 20 - 8);
    assert!((DEFAULT_FRAGMENT_SIZE as usize) < DEFAULT_MTU);
    assert!(!DEFAULT_HEARTBEAT_PERIOD.is_zero());
}

#[test]
fn rtps_8_4_14_1_a_sample_up_to_the_fragment_size_goes_out_as_one_data() {
    let (data, fragments, largest) = sent(&[7_u8; 64]);
    assert_eq!((data, fragments.len()), (1, 0));
    assert!(largest <= DEFAULT_MTU);
}

#[test]
fn rtps_8_4_14_1_a_larger_sample_goes_out_in_ceil_size_over_fragment_size_fragments() {
    for size in [
        DEFAULT_FRAGMENT_SIZE as usize + 1,
        4 * DEFAULT_FRAGMENT_SIZE as usize,
        10_000,
    ] {
        let (data, fragments, largest) = sent(&vec![3_u8; size]);
        let expected = size.div_ceil(DEFAULT_FRAGMENT_SIZE as usize);
        assert_eq!(data, 0, "{size}: no DATA beside the fragments");
        assert_eq!(
            fragments,
            (1..=expected as u32).collect::<Vec<_>>(),
            "{size}"
        );
        assert!(
            largest <= DEFAULT_MTU,
            "{size}: a datagram of {largest} octets"
        );
    }
}
