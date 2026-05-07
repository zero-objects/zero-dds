#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr
)]

//! Annex C C.1.5 — Settlement-Mode Reliable.
//!
//! Spec §C.1.5: ein DDS-DataWriter mit RELIABILITY=RELIABLE
//! produziert AMQP-Settlement-Roundtrip (snd-settle-mode=unsettled
//! → Disposition → settle).
//!
//! Wir verifizieren das Settlement-Tracking ohne DDS-Side-Bridge:
//! ein Sender-Link wird mit Unsettled-Mode angelegt, Transfer
//! sendet, Disposition fuer Settlement empfaengt.

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

    // Drei Dispositions empfangen — pending wieder bei 0.
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
    // Credit aufgebraucht — naechster deliver() liefert Err.
    assert_eq!(link.deliver(), Err(DeliverError::NoCredit));
    assert_eq!(link.pending_settlements, 2);
}
