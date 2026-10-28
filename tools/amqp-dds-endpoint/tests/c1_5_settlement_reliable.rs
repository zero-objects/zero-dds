#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr
)]

//! Annex C C.1.5 — Settlement-Mode Reliable.
//!
//! Spec §C.1.5: a DDS DataWriter with RELIABILITY=RELIABLE
//! produces an AMQP settlement roundtrip (snd-settle-mode=unsettled
//! → Disposition → settle).
//!
//! We verify settlement tracking without a DDS-side bridge:
//! a sender link is created with unsettled mode, transfer
//! sends, receives a disposition for settlement.

mod common;

use zerodds_amqp_endpoint::link::{DeliverError, LinkRole, LinkSession, SettlementMode};

#[test]
fn c1_5_reliable_unsettled_increments_pending_until_disposition() {
    let mut link = LinkSession::new("L".into(), 0, LinkRole::Sender, SettlementMode::Unsettled);
    link.grant_credit(3);

    // Drei Transfers senden — pending-Counter wachsend.
    link.deliver().unwrap();
    link.deliver().unwrap();
    link.deliver().unwrap();
    assert_eq!(link.pending_settlements, 3);
    assert_eq!(link.delivered, 3);
    assert_eq!(link.credit, 0);

    // Three dispositions received — pending back at 0.
    link.settle();
    link.settle();
    link.settle();
    assert_eq!(link.pending_settlements, 0);
}

#[test]
fn c1_5_credit_exhaustion_blocks_further_deliveries() {
    let mut link = LinkSession::new("L".into(), 0, LinkRole::Sender, SettlementMode::Unsettled);
    link.grant_credit(2);
    link.deliver().unwrap();
    link.deliver().unwrap();
    // Credit exhausted — the next deliver() returns Err.
    assert_eq!(link.deliver(), Err(DeliverError::NoCredit));
    assert_eq!(link.pending_settlements, 2);
}
