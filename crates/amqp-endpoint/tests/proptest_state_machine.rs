//! Property tests for the AMQP connection state machine.
//!
//! Spec OASIS amqp-1.0-transport §2.4 — semantic invariants:
//! 1. **End is absorbing:** once the state reaches `End`, every
//!    further frame MUST return an `End` or be rejected with an
//!    error transition.
//! 2. **Determinism:** `advance_connection(s, f)` is a pure
//!    function — the same input yields the same output.
//! 3. **State-walk sequence invariant:** a random sequence of
//!    frames either lets the state machine run through
//!    successfully OR is rejected with `IllegalStateTransition` —
//!    never a panic, never undefined behavior.

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

    /// Determinism: the same input yields the same output.
    #[test]
    fn advance_is_deterministic(state in arb_state(), frame in arb_frame()) {
        let r1 = advance_connection(state, frame);
        let r2 = advance_connection(state, frame);
        prop_assert_eq!(r1, r2);
    }

    /// From every state + frame, `advance_connection` must return
    /// either an Ok or an IllegalStateTransition — never a
    /// panic, never other error variants.
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

    /// State-walk robustness: a random frame sequence of 0..50
    /// frames does not put the state machine into a panicking
    /// state. Each step returns either Ok(s) or Err(...).
    /// After Err the walk aborts.
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
        // State reached — no panic, nothing unexpected.
        let _ = state;
    }

    /// End-absorbing invariant: the spec leaves End as the terminal
    /// state; after a Close receive it goes to CloseRcvd→End. From End
    /// there is no valid forward path; every further frame MUST
    /// return IllegalStateTransition.
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
    // From CloseRcvd every frame goes to End (cleanup phase).
    let s = advance_connection(s, InboundFrameKind::End).unwrap();
    assert_eq!(s, ConnectionState::End);
}
