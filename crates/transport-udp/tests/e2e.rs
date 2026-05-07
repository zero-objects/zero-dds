//! W4 E2E-Test: write → UDP → read.
//!
//! Verbindet `BestEffortWriter` und `BestEffortReader` ueber zwei
//! `UdpTransport`-Instanzen auf der Loopback-Schnittstelle und
//! verifiziert, dass mehrere Datagrams in-order ankommen.

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

use std::net::Ipv4Addr;
use std::time::Duration;

use zerodds_rtps::reader::BestEffortReader;
use zerodds_rtps::wire_types::{EntityId, GuidPrefix, SequenceNumber};
use zerodds_rtps::writer::BestEffortWriter;
use zerodds_transport::Transport;
use zerodds_transport_udp::UdpTransport;

const PARTICIPANT_PREFIX: GuidPrefix = GuidPrefix([
    0xAA, 0xBB, 0xCC, 0xDD, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
]);

fn make_endpoints() -> (UdpTransport, UdpTransport) {
    let writer_xport = UdpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).expect("writer bind");
    let reader_xport = UdpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0).expect("reader bind");
    let writer_xport = writer_xport
        .with_timeout(Some(Duration::from_secs(2)))
        .expect("writer timeout");
    let reader_xport = reader_xport
        .with_timeout(Some(Duration::from_secs(2)))
        .expect("reader timeout");
    (writer_xport, reader_xport)
}

#[test]
fn e2e_write_udp_read_single_datagram() {
    let writer_id = EntityId::user_writer_with_key([0x10, 0x20, 0x30]);
    let reader_id = EntityId::user_reader_with_key([0xA0, 0xB0, 0xC0]);

    let (writer_xport, reader_xport) = make_endpoints();
    let dest = reader_xport.local_locator();

    let mut writer = BestEffortWriter::new(PARTICIPANT_PREFIX, writer_id, reader_id);
    let reader = BestEffortReader::new(PARTICIPANT_PREFIX, reader_id);

    let payload = b"hello rtps over udp";
    let datagram = writer.write(payload).expect("encode");
    writer_xport.send(&dest, &datagram).expect("send");

    let received = reader_xport.recv().expect("recv");
    let samples = reader.recv_datagram(&received.data).expect("decode");
    assert_eq!(samples.len(), 1);
    assert_eq!(samples[0].payload.as_ref(), payload.as_slice());
    assert_eq!(samples[0].writer_sn, SequenceNumber(1));
    assert_eq!(samples[0].writer_id, writer_id);
}

#[test]
fn e2e_write_udp_read_multiple_in_order() {
    let writer_id = EntityId::user_writer_with_key([0x10, 0x20, 0x30]);
    let reader_id = EntityId::user_reader_with_key([0xA0, 0xB0, 0xC0]);

    let (writer_xport, reader_xport) = make_endpoints();
    let dest = reader_xport.local_locator();

    let mut writer = BestEffortWriter::new(PARTICIPANT_PREFIX, writer_id, reader_id);
    let reader = BestEffortReader::new(PARTICIPANT_PREFIX, reader_id);

    let payloads: Vec<Vec<u8>> = (0u8..5).map(|i| vec![i, i + 1, i + 2]).collect();
    for p in &payloads {
        let datagram = writer.write(p).expect("encode");
        writer_xport.send(&dest, &datagram).expect("send");
    }

    for (i, expected) in payloads.iter().enumerate() {
        let recv = reader_xport.recv().expect("recv");
        let samples = reader.recv_datagram(&recv.data).expect("decode");
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].payload.as_ref(), expected.as_slice());
        assert_eq!(samples[0].writer_sn, SequenceNumber((i + 1) as i64));
    }
}

#[test]
fn e2e_reader_drops_message_to_other_endpoint() {
    let writer_id = EntityId::user_writer_with_key([0x10, 0x20, 0x30]);
    let our_reader = EntityId::user_reader_with_key([0xA0, 0xB0, 0xC0]);
    let other_reader = EntityId::user_reader_with_key([0x99, 0x99, 0x99]);

    let (writer_xport, reader_xport) = make_endpoints();
    let dest = reader_xport.local_locator();

    // Writer richtet an other_reader.
    let mut writer = BestEffortWriter::new(PARTICIPANT_PREFIX, writer_id, other_reader);
    let datagram = writer.write(b"not for us").expect("encode");
    writer_xport.send(&dest, &datagram).expect("send");

    // Unser Reader sieht das Datagram, aber filtert es weg.
    let recv = reader_xport.recv().expect("recv");
    let our_reader_obj = BestEffortReader::new(PARTICIPANT_PREFIX, our_reader);
    let samples = our_reader_obj.recv_datagram(&recv.data).expect("decode");
    assert!(
        samples.is_empty(),
        "Datagram an other_reader darf nicht delivert werden"
    );
}
