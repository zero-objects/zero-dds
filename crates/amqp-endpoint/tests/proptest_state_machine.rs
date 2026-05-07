//! Property-Tests fuer AMQP Connection-State-Machine.
//!
//! Spec OASIS amqp-1.0-transport §2.4 — semantische Invarianten:
//! 1. **End ist absorbing:** Wenn der State `End` erreicht ist, MUSS
//!    jeder weitere Frame ein `End` zurueckliefern oder mit einer
//!    Fehler-Transition verworfen werden.
//! 2. **Determinismus:** `advance_connection(s, f)` ist eine reine
//!    Funktion — gleicher Input liefert gleichen Output.
//! 3. **State-Walk-Sequence-Invariante:** zufaellige Sequenz von
//!    Frames laesst die State-Machine entweder erfolgreich
//!    durchlaufen ODER mit `IllegalStateTransition` ablehnen — nie
//!    panic, nie undefiniertes Verhalten.

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

use proptest::prelude::*;
use zerodds_amqp_endpoint::session::{
    ConnectionState, EndpointError, InboundFrameKind, advance_connection,
};

fn arb_state() -> impl Strategy<Value = ConnectionState> {
    prop_oneof![
        Just(ConnectionState::Start),
        Just(ConnectionState::HdrRcvd),
        Just(ConnectionState::HdrExch),
        Just(ConnectionState::OpenRcvd),
        Just(ConnectionState::Opened),
        Just(ConnectionState::CloseRcvd),
        Just(ConnectionState::CloseSent),
        Just(ConnectionState::End),
    ]
}

fn arb_frame() -> impl Strategy<Value = InboundFrameKind> {
    prop_oneof![
        Just(InboundFrameKind::Header),
        Just(InboundFrameKind::Open),
        Just(InboundFrameKind::Begin),
        Just(InboundFrameKind::Attach),
        Just(InboundFrameKind::Flow),
        Just(InboundFrameKind::Transfer),
        Just(InboundFrameKind::Disposition),
        Just(InboundFrameKind::Detach),
        Just(InboundFrameKind::End),
        Just(InboundFrameKind::Close),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Determinismus: gleicher Input liefert gleichen Output.
    #[test]
    fn advance_is_deterministic(state in arb_state(), frame in arb_frame()) {
        let r1 = advance_connection(state, frame);
        let r2 = advance_connection(state, frame);
        prop_assert_eq!(r1, r2);
    }

    /// Aus jedem State + Frame muss `advance_connection` entweder
    /// ein Ok liefern oder ein IllegalStateTransition — niemals
    /// panic, niemals andere Error-Varianten.
    #[test]
    fn advance_returns_ok_or_illegal_transition(
        state in arb_state(),
        frame in arb_frame(),
    ) {
        let result = advance_connection(state, frame);
        match result {
            Ok(_) => {}
            Err(EndpointError::IllegalStateTransition { .. }) => {}
            Err(other) => prop_assert!(false, "unexpected error: {other:?}"),
        }
    }

    /// State-Walk Robustness: zufaellige Frame-Sequenz von 0..50
    /// Frames bringt die State-Machine nicht in einen panischen
    /// Zustand. Jeder Schritt liefert entweder Ok(s) oder Err(...).
    /// Nach Err bricht der Walk ab.
    #[test]
    fn random_frame_sequence_terminates_cleanly(
        frames in prop::collection::vec(arb_frame(), 0..50),
    ) {
        let mut state = ConnectionState::Start;
        for f in frames {
            match advance_connection(state, f) {
                Ok(next) => state = next,
                Err(_) => break,
            }
        }
        // State erreicht — kein panic, kein unexpected.
        let _ = state;
    }

    /// End-Absorbing-Invariant: die Spec laesst End als Terminal-
    /// State; nach Close-Receive geht es zu CloseRcvd→End. Aus End
    /// gibt es keinen valid forward-path; jeder weitere Frame MUSS
    /// IllegalStateTransition liefern.
    #[test]
    fn end_state_rejects_all_frames(frame in arb_frame()) {
        let result = advance_connection(ConnectionState::End, frame);
        prop_assert!(
            matches!(result, Err(EndpointError::IllegalStateTransition { .. })),
            "End must reject {frame:?}, got {result:?}"
        );
    }
}

/// Spec §2.4 — Open-Path: Start → HdrRcvd → HdrExch → OpenRcvd → Opened.
#[test]
fn happy_path_open_walk() {
    let mut s = ConnectionState::Start;
    for (frame, expected) in &[
        (InboundFrameKind::Header, ConnectionState::HdrRcvd),
        (InboundFrameKind::Header, ConnectionState::HdrExch),
        (InboundFrameKind::Open, ConnectionState::OpenRcvd),
        (InboundFrameKind::Open, ConnectionState::Opened),
    ] {
        s = advance_connection(s, *frame).unwrap();
        assert_eq!(s, *expected);
    }
}

/// Spec §2.4 — Close-Path: Opened → CloseRcvd → End.
#[test]
fn happy_path_close_walk() {
    let s = ConnectionState::Opened;
    let s = advance_connection(s, InboundFrameKind::Close).unwrap();
    assert_eq!(s, ConnectionState::CloseRcvd);
    // Aus CloseRcvd geht jeder Frame zu End (cleanup-phase).
    let s = advance_connection(s, InboundFrameKind::End).unwrap();
    assert_eq!(s, ConnectionState::End);
}
