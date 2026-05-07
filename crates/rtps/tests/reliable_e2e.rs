//! WP 1.1 T4 — E2E-Integration ReliableWriter ↔ ReliableReader mit
//! simuliertem Packet-Loss.
//!
//! Der Test laesst beide Endpunkte durch einen in-process LossyChannel
//! miteinander reden. Datagrams werden mit konfigurierbarer Drop-Rate
//! verworfen. Trotz Loss muessen **alle** Samples in-order beim Reader
//! ankommen — die Reliable-State-Maschine sorgt via ACKNACK-Round-Trips
//! fuer Re-Sends.

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

use zerodds_rtps::datagram::{ParsedSubmessage, decode_datagram};
use zerodds_rtps::reader_proxy::ReaderProxy;
use zerodds_rtps::reliable_reader::{
    DEFAULT_HEARTBEAT_RESPONSE_DELAY, DeliveredSample, ReliableReader, ReliableReaderConfig,
};
use zerodds_rtps::reliable_writer::{ReliableWriter, ReliableWriterConfig};
use zerodds_rtps::wire_types::{Locator, SequenceNumber, VendorId};
use zerodds_rtps::writer_proxy::WriterProxy;

mod common;
use common::{XorShift32, pattern_for, test_reader_guid, test_writer_guid};

/// In-process-Kanal mit konfigurierbarer Drop-Rate in Prozent (0–100).
#[derive(Debug)]
struct LossyChannel {
    drop_percent: u32,
    rng: XorShift32,
    queue: std::collections::VecDeque<Vec<u8>>,
    dropped: usize,
}

impl LossyChannel {
    fn new(drop_percent: u32, seed: u32) -> Self {
        assert!(drop_percent <= 100);
        Self {
            drop_percent,
            rng: XorShift32::new(seed),
            queue: std::collections::VecDeque::new(),
            dropped: 0,
        }
    }
    fn send(&mut self, datagram: Vec<u8>) {
        let r = self.rng.next_u32() % 100;
        if r < self.drop_percent {
            self.dropped += 1;
        } else {
            self.queue.push_back(datagram);
        }
    }
    fn recv(&mut self) -> Option<Vec<u8>> {
        self.queue.pop_front()
    }
    fn dropped(&self) -> usize {
        self.dropped
    }
}

fn make_writer() -> ReliableWriter {
    make_writer_with_frag_size(zerodds_rtps::reliable_writer::DEFAULT_FRAGMENT_SIZE)
}

fn make_writer_with_frag_size(fragment_size: u32) -> ReliableWriter {
    let reader_proxy = ReaderProxy::new(
        test_reader_guid(),
        vec![Locator::udp_v4([127, 0, 0, 1], 7410)],
        vec![],
        true,
    );
    ReliableWriter::new(ReliableWriterConfig {
        guid: test_writer_guid(),
        vendor_id: VendorId::ZERODDS,
        reader_proxies: vec![reader_proxy],
        max_samples: 1024,
        history_kind: zerodds_rtps::history_cache::HistoryKind::KeepAll,
        heartbeat_period: Duration::from_millis(50),
        fragment_size,
        mtu: zerodds_rtps::message_builder::DEFAULT_MTU,
    })
}

fn make_reader() -> ReliableReader {
    let writer_proxy = WriterProxy::new(
        test_writer_guid(),
        vec![Locator::udp_v4([127, 0, 0, 1], 7420)],
        vec![],
        true,
    );
    ReliableReader::new(ReliableReaderConfig {
        guid: test_reader_guid(),
        vendor_id: VendorId::ZERODDS,
        writer_proxies: vec![writer_proxy],
        max_samples_per_proxy: 1024,
        heartbeat_response_delay: DEFAULT_HEARTBEAT_RESPONSE_DELAY,
        assembler_caps: zerodds_rtps::fragment_assembler::AssemblerCaps::default(),
    })
}

/// Dispatcht Submessages von Writer→Reader.
fn dispatch_w2r(datagram: &[u8], r: &mut ReliableReader, now: Duration) -> Vec<DeliveredSample> {
    let mut delivered = Vec::new();
    let parsed = decode_datagram(datagram).expect("decode w->r datagram");
    for sub in parsed.submessages {
        match sub {
            ParsedSubmessage::Data(d) => delivered.extend(r.handle_data(&d)),
            ParsedSubmessage::Heartbeat(h) => {
                // Flags (F/L) werden aus dem Submessage-Header via
                // decode_datagram mitgefuehrt — kein Hardcoden mehr.
                r.handle_heartbeat(&h, now);
            }
            ParsedSubmessage::Gap(g) => delivered.extend(r.handle_gap(&g)),
            ParsedSubmessage::DataFrag(df) => delivered.extend(r.handle_data_frag(&df, now)),
            ParsedSubmessage::AckNack(_)
            | ParsedSubmessage::HeartbeatFrag(_)
            | ParsedSubmessage::NackFrag(_)
            | ParsedSubmessage::HeaderExtension(_)
            | ParsedSubmessage::InfoSource(_)
            | ParsedSubmessage::InfoReply(_)
            | ParsedSubmessage::InfoTimestamp(_)
            | ParsedSubmessage::Unknown { .. } => {}
        }
    }
    delivered
}

/// Dispatcht ACKNACKs von Reader→Writer.
fn dispatch_r2w(datagram: &[u8], w: &mut ReliableWriter) {
    let parsed = decode_datagram(datagram).expect("decode r->w datagram");
    let src = test_reader_guid();
    for sub in parsed.submessages {
        match sub {
            ParsedSubmessage::AckNack(ack) => {
                let base = ack.reader_sn_state.bitmap_base;
                let requested: Vec<_> = ack.reader_sn_state.iter_set().collect();
                w.handle_acknack(src, base, requested);
            }
            ParsedSubmessage::NackFrag(nf) => w.handle_nackfrag(src, &nf),
            _ => {}
        }
    }
}

fn run_e2e(total_samples: usize, drop_percent: u32, seed: u32) -> Vec<DeliveredSample> {
    let mut w = make_writer();
    let mut r = make_reader();
    let mut ch_w2r = LossyChannel::new(drop_percent, seed);
    let mut ch_r2w = LossyChannel::new(drop_percent, seed.wrapping_add(1));
    let mut delivered = Vec::new();

    // Phase 1: Alle Samples schreiben. Jeder write gibt eine Liste
    // von Datagrammen zurueck (DATA oder mehrere DATA_FRAG), die wir
    // in Reihenfolge in den Kanal kippen.
    for i in 1..=total_samples {
        let dgs = w.write(&[i as u8]).expect("write");
        for dg in dgs {
            ch_w2r.send(dg.bytes);
        }
    }

    // Phase 2: Schritt-Simulation bis alle Samples angekommen oder
    // Deadlock. 50ms-Schritte, max 2000 Schritte = 100 simulierte
    // Sekunden — bei 10% Loss und 50 Samples reicht das lockerleicht.
    let max_steps = 2000;
    for step in 0..max_steps {
        let now = Duration::from_millis((step as u64) * 50);

        // W → R dispatch
        while let Some(dg) = ch_w2r.recv() {
            delivered.extend(dispatch_w2r(&dg, &mut r, now));
        }
        // R → W dispatch (ACKNACKs)
        while let Some(dg) = ch_r2w.recv() {
            dispatch_r2w(&dg, &mut w);
        }

        // Writer-Tick (HEARTBEATs + Resends)
        for dg in w.tick(now).expect("writer tick") {
            ch_w2r.send(dg.bytes);
        }
        // Reader-Tick (ACKNACK/NACK_FRAG wenn faellig)
        for dg in r.tick(now).expect("reader tick") {
            ch_r2w.send(dg);
        }

        if delivered.len() == total_samples {
            eprintln!(
                "e2e: delivered {} samples in {} steps (drops w2r={}, r2w={})",
                delivered.len(),
                step + 1,
                ch_w2r.dropped(),
                ch_r2w.dropped(),
            );
            break;
        }
    }
    delivered
}

#[test]
fn e2e_zero_loss_delivers_all_in_order() {
    let delivered = run_e2e(50, 0, 42);
    assert_eq!(delivered.len(), 50);
    for (i, s) in delivered.iter().enumerate() {
        assert_eq!(s.sequence_number, SequenceNumber(i as i64 + 1));
        assert_eq!(s.payload.as_ref(), &[(i + 1) as u8]);
    }
}

#[test]
fn e2e_10_percent_loss_delivers_all_in_order() {
    let delivered = run_e2e(50, 10, 1337);
    assert_eq!(delivered.len(), 50);
    for (i, s) in delivered.iter().enumerate() {
        assert_eq!(s.sequence_number, SequenceNumber(i as i64 + 1));
    }
}

#[test]
fn e2e_30_percent_loss_delivers_all_in_order() {
    let delivered = run_e2e(30, 30, 2025);
    assert_eq!(delivered.len(), 30);
    for (i, s) in delivered.iter().enumerate() {
        assert_eq!(s.sequence_number, SequenceNumber(i as i64 + 1));
    }
}

// ============================================================================
// WP 1.2 T4 — Fragmentation-E2E mit Packet-Loss
// ============================================================================

/// Fuehrt E2E mit konfigurierbarer Payload-Groesse, fragment_size und
/// Drop-Rate aus. Prueft, dass alle N Samples byte-identisch beim Reader
/// ankommen.
fn run_frag_e2e(
    total_samples: usize,
    payload_size: usize,
    fragment_size: u32,
    drop_percent: u32,
    seed: u32,
) -> Vec<DeliveredSample> {
    let mut w = make_writer_with_frag_size(fragment_size);
    let mut r = make_reader();
    let mut ch_w2r = LossyChannel::new(drop_percent, seed);
    let mut ch_r2w = LossyChannel::new(drop_percent, seed.wrapping_add(1));
    let mut delivered = Vec::new();

    // Phase 1: Samples schreiben
    for i in 1..=total_samples {
        let payload = pattern_for(i, payload_size);
        let dgs = w.write(&payload).expect("write");
        for dg in dgs {
            ch_w2r.send(dg.bytes);
        }
    }

    // Phase 2: Schritt-Simulation — bei vielen Fragmenten und hohem
    // Loss sind mehr Runden noetig. 50ms-Schritte, max 5000 Schritte
    // = 250 simulierte Sekunden.
    let max_steps = 5000;
    for step in 0..max_steps {
        let now = Duration::from_millis((step as u64) * 50);

        while let Some(dg) = ch_w2r.recv() {
            delivered.extend(dispatch_w2r(&dg, &mut r, now));
        }
        while let Some(dg) = ch_r2w.recv() {
            dispatch_r2w(&dg, &mut w);
        }

        for dg in w.tick(now).expect("writer tick") {
            ch_w2r.send(dg.bytes);
        }
        for dg in r.tick(now).expect("reader tick") {
            ch_r2w.send(dg);
        }

        if delivered.len() == total_samples {
            eprintln!(
                "frag-e2e: {} samples (size={}, frag_size={}, drop={}%) \
                 in {} steps (drops w2r={}, r2w={})",
                delivered.len(),
                payload_size,
                fragment_size,
                drop_percent,
                step + 1,
                ch_w2r.dropped(),
                ch_r2w.dropped(),
            );
            break;
        }
    }
    delivered
}

#[test]
fn e2e_frag_large_sample_zero_loss() {
    // 10 kB Sample, fragment_size 1400 → ~8 Fragmente, ein einziges Sample
    let delivered = run_frag_e2e(1, 10_000, 1400, 0, 7);
    assert_eq!(delivered.len(), 1);
    assert_eq!(delivered[0].payload.as_ref(), &*pattern_for(1, 10_000));
    assert_eq!(delivered[0].sequence_number, SequenceNumber(1));
}

#[test]
fn e2e_frag_large_sample_10_percent_loss() {
    let delivered = run_frag_e2e(1, 10_000, 1400, 10, 42);
    assert_eq!(delivered.len(), 1);
    assert_eq!(delivered[0].payload.as_ref(), &*pattern_for(1, 10_000));
}

#[test]
fn e2e_frag_large_sample_30_percent_loss() {
    let delivered = run_frag_e2e(1, 10_000, 1400, 30, 99);
    assert_eq!(delivered.len(), 1);
    assert_eq!(delivered[0].payload.as_ref(), &*pattern_for(1, 10_000));
}

#[test]
fn e2e_frag_many_samples_3_fragments_each_10_percent_loss() {
    // 20 Samples, jedes mit 3 Fragmenten (payload 10, frag_size 4)
    let delivered = run_frag_e2e(20, 10, 4, 10, 123);
    assert_eq!(delivered.len(), 20);
    for (i, s) in delivered.iter().enumerate() {
        assert_eq!(s.sequence_number, SequenceNumber(i as i64 + 1));
        assert_eq!(
            s.payload.as_ref(),
            &*pattern_for(i + 1, 10),
            "sample {}",
            i + 1
        );
    }
}

#[test]
fn e2e_frag_mixed_data_and_data_frag() {
    // Mix: payload_size=3 < fragment_size=4 → nimmt DATA-Pfad.
    // Wir testen implizit: Writer kann in ein-und-derselben Session
    // beides emittieren. Hier nur DATA-Pfad abgedeckt; die
    // DATA_FRAG-Pfade sind in den anderen Tests.
    let delivered = run_frag_e2e(5, 3, 4, 0, 1);
    assert_eq!(delivered.len(), 5);
    for (i, s) in delivered.iter().enumerate() {
        assert_eq!(s.payload.as_ref(), &*pattern_for(i + 1, 3));
    }
}
