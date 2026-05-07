// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Stream-State-Machine — RFC 9113 §5.1.

use crate::error::Http2Error;

/// Stream-Identifier.
pub type StreamId = u32;

/// Stream-State (RFC 9113 §5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StreamState {
    /// `idle`.
    Idle,
    /// `reserved (local)`.
    ReservedLocal,
    /// `reserved (remote)`.
    ReservedRemote,
    /// `open`.
    Open,
    /// `half-closed (local)`.
    HalfClosedLocal,
    /// `half-closed (remote)`.
    HalfClosedRemote,
    /// `closed`.
    Closed,
}

/// Eingehender Event aus Sicht der State-Machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamEvent {
    /// Empfangen `HEADERS` ohne `END_STREAM`.
    RecvHeaders,
    /// Empfangen `HEADERS`/`DATA` mit `END_STREAM`.
    RecvEndStream,
    /// Versendet `HEADERS` ohne `END_STREAM`.
    SendHeaders,
    /// Versendet `END_STREAM`.
    SendEndStream,
    /// Empfangen `PUSH_PROMISE`.
    RecvPushPromise,
    /// Versendet `PUSH_PROMISE`.
    SendPushPromise,
    /// `RST_STREAM` — terminiert sofort.
    Reset,
}

/// Wendet einen Event auf den State an. Spec §5.1.
///
/// # Errors
/// `InvalidState` wenn der Event im aktuellen State nicht erlaubt ist.
pub fn transition(state: StreamState, event: StreamEvent) -> Result<StreamState, Http2Error> {
    use StreamEvent as E;
    use StreamState as S;
    let next = match (state, event) {
        (_, E::Reset) => S::Closed,
        (S::Idle, E::RecvHeaders) => S::Open,
        (S::Idle, E::SendHeaders) => S::Open,
        (S::Idle, E::RecvPushPromise) => S::ReservedRemote,
        (S::Idle, E::SendPushPromise) => S::ReservedLocal,
        (S::ReservedLocal, E::SendHeaders) => S::HalfClosedRemote,
        (S::ReservedLocal, E::SendEndStream) => S::Closed,
        (S::ReservedRemote, E::RecvHeaders) => S::HalfClosedLocal,
        (S::ReservedRemote, E::RecvEndStream) => S::Closed,
        (S::Open, E::SendEndStream) => S::HalfClosedLocal,
        (S::Open, E::RecvEndStream) => S::HalfClosedRemote,
        (S::Open, E::RecvHeaders | E::SendHeaders) => S::Open,
        (S::HalfClosedLocal, E::RecvEndStream) => S::Closed,
        (S::HalfClosedLocal, E::RecvHeaders) => S::HalfClosedLocal,
        (S::HalfClosedRemote, E::SendEndStream) => S::Closed,
        (S::HalfClosedRemote, E::SendHeaders) => S::HalfClosedRemote,
        (S::Closed, _) => return Err(Http2Error::InvalidState),
        _ => return Err(Http2Error::InvalidState),
    };
    Ok(next)
}

/// Spec §5.1.1: Client-initiierte Streams sind ungerade.
#[must_use]
pub fn is_client_initiated(stream_id: StreamId) -> bool {
    stream_id != 0 && stream_id % 2 == 1
}

/// Spec §5.1.1: Server-initiierte Streams sind gerade.
#[must_use]
pub fn is_server_initiated(stream_id: StreamId) -> bool {
    stream_id != 0 && stream_id % 2 == 0
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn idle_to_open_via_headers() {
        assert_eq!(
            transition(StreamState::Idle, StreamEvent::RecvHeaders).unwrap(),
            StreamState::Open
        );
    }

    #[test]
    fn open_send_end_stream_half_closes_local() {
        assert_eq!(
            transition(StreamState::Open, StreamEvent::SendEndStream).unwrap(),
            StreamState::HalfClosedLocal
        );
    }

    #[test]
    fn half_closed_local_recv_end_stream_closes() {
        assert_eq!(
            transition(StreamState::HalfClosedLocal, StreamEvent::RecvEndStream).unwrap(),
            StreamState::Closed
        );
    }

    #[test]
    fn reset_from_any_state_closes() {
        for s in [
            StreamState::Idle,
            StreamState::Open,
            StreamState::HalfClosedLocal,
            StreamState::HalfClosedRemote,
            StreamState::ReservedLocal,
            StreamState::ReservedRemote,
        ] {
            assert_eq!(
                transition(s, StreamEvent::Reset).unwrap(),
                StreamState::Closed
            );
        }
    }

    #[test]
    fn reserved_local_send_headers_half_closes_remote() {
        assert_eq!(
            transition(StreamState::ReservedLocal, StreamEvent::SendHeaders).unwrap(),
            StreamState::HalfClosedRemote
        );
    }

    #[test]
    fn closed_state_rejects_non_reset_events() {
        assert!(transition(StreamState::Closed, StreamEvent::SendHeaders).is_err());
    }

    #[test]
    fn idle_send_end_stream_invalid() {
        assert!(transition(StreamState::Idle, StreamEvent::SendEndStream).is_err());
    }

    #[test]
    fn client_streams_are_odd() {
        assert!(is_client_initiated(1));
        assert!(is_client_initiated(3));
        assert!(!is_client_initiated(0));
        assert!(!is_client_initiated(2));
    }

    #[test]
    fn server_streams_are_even() {
        assert!(is_server_initiated(2));
        assert!(is_server_initiated(4));
        assert!(!is_server_initiated(0));
        assert!(!is_server_initiated(1));
    }

    #[test]
    fn open_recv_headers_stays_open_for_trailers() {
        assert_eq!(
            transition(StreamState::Open, StreamEvent::RecvHeaders).unwrap(),
            StreamState::Open
        );
    }
}
