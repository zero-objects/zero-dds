#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr
)]

//! Annex C C.1.6 — Settlement-Mode Best-Effort.
//!
//! Spec §C.1.6: RELIABILITY=BEST_EFFORT → AMQP pre-settled
//! (`snd-settle-mode=settled`); no disposition roundtrip.

mod common;

use zerodds_amqp_endpoint::link::{LinkRole, LinkSession, SettlementMode};

#[test]
fn c1_6_best_effort_settled_does_not_track_pending() {
    let mut link = LinkSession::new("L".into(), 0, LinkRole::Sender, SettlementMode::Settled);
    link.grant_credit(5);

    // Three pre-settled transfers — pending stays 0 (no roundtrip).
    link.deliver().unwrap();
    link.deliver().unwrap();
    link.deliver().unwrap();
    assert_eq!(link.delivered, 3);
    assert_eq!(link.pending_settlements, 0);
}

#[test]
fn c1_6_settle_call_on_pre_settled_link_is_no_op() {
    let mut link = LinkSession::new("L".into(), 0, LinkRole::Sender, SettlementMode::Settled);
    link.grant_credit(1);
    link.deliver().unwrap();
    // Disposition receipt still allowed (no-op because pending=0).
    link.settle();
    assert_eq!(link.pending_settlements, 0);
}
