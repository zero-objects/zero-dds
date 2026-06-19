// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Connection/session state machine for the DDS-AMQP endpoint.
//!
//! Spec source: dds-amqp-1.0-beta1.pdf §6.1 Direct-Embed Topology +
//! `amqp-1.0-transport` §2.4 connection state diagram.

use alloc::string::String;

use crate::limits::ResourceLimits;
use crate::sasl::SaslState;

/// Spec §2.4 connection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConnectionState {
    /// Initial state before the header exchange.
    #[default]
    Start,
    /// AMQP header received.
    HdrRcvd,
    /// AMQP header exchanged in both directions.
    HdrExch,
    /// Open performative received.
    OpenRcvd,
    /// Open performative in both directions — the connection is open.
    Opened,
    /// Close performative received — cleanup phase.
    CloseRcvd,
    /// Close performative sent — cleanup phase.
    CloseSent,
    /// Connection finished.
    End,
}

/// Spec §2.5 session state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SessionState {
    /// Initial state before the Begin performative.
    #[default]
    Unmapped,
    /// Begin performative received.
    BeginRcvd,
    /// Begin performative in both directions — the session is open.
    Mapped,
    /// End performative received.
    EndRcvd,
    /// End performative sent.
    EndSent,
    /// Session closed.
    Discarded,
}

/// Endpoint configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndpointConfig {
    /// Container id (Spec §2.4.1) — uniquely identifies the endpoint.
    pub container_id: String,
    /// Resource limits + DoS caps.
    pub limits: ResourceLimits,
    /// Current SASL negotiation (None before the SASL phase).
    pub sasl: Option<SaslState>,
}

/// Endpoint error from the state machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EndpointError {
    /// Spec §2.4 — illegal state transition.
    IllegalStateTransition {
        /// Previous state.
        from: ConnectionState,
        /// What was attempted (e.g. "open").
        attempted: &'static str,
    },
    /// Resource limit exceeded.
    ResourceLimitExceeded(&'static str),
    /// Idle timeout exceeded.
    IdleTimeout,
    /// Connection was abruptly closed by the peer.
    ConnectionAborted,
}

/// Inbound frame class — simplified; a concrete listener
/// dispatches based on the performative descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InboundFrameKind {
    /// AMQP header (4-byte magic + 4-byte version).
    Header,
    /// Open performative.
    Open,
    /// Begin performative.
    Begin,
    /// Attach performative.
    Attach,
    /// Flow performative.
    Flow,
    /// Transfer performative.
    Transfer,
    /// Disposition performative.
    Disposition,
    /// Detach performative.
    Detach,
    /// End performative.
    End,
    /// Close performative.
    Close,
}

/// Spec §2.4 — advance the connection state machine on an incoming
/// frame.
///
/// # Errors
/// `IllegalStateTransition` if the state machine does not accept
/// the frame in the current state.
pub fn advance_connection(
    state: ConnectionState,
    frame: InboundFrameKind,
) -> Result<ConnectionState, EndpointError> {
    use ConnectionState as C;
    use InboundFrameKind as F;
    let next = match (state, frame) {
        (C::Start, F::Header) => C::HdrRcvd,
        (C::HdrRcvd, F::Header) => C::HdrExch,
        // Server may permit OPEN-RCVD directly after HdrExch.
        (C::HdrExch, F::Open) => C::OpenRcvd,
        (C::OpenRcvd, F::Open) => C::Opened,
        // Within Opened, Begin/Attach/etc are allowed; the state
        // stays Opened until Close arrives.
        (
            C::Opened,
            F::Begin | F::Attach | F::Flow | F::Transfer | F::Disposition | F::Detach | F::End,
        ) => C::Opened,
        (C::Opened, F::Close) => C::CloseRcvd,
        (C::CloseRcvd, _) => C::End,
        (
            from,
            F::Header
            | F::Open
            | F::Begin
            | F::Attach
            | F::Flow
            | F::Transfer
            | F::Disposition
            | F::Detach
            | F::End
            | F::Close,
        ) => {
            return Err(EndpointError::IllegalStateTransition {
                from,
                attempted: frame_label(frame),
            });
        }
    };
    Ok(next)
}

const fn frame_label(f: InboundFrameKind) -> &'static str {
    match f {
        InboundFrameKind::Header => "header",
        InboundFrameKind::Open => "open",
        InboundFrameKind::Begin => "begin",
        InboundFrameKind::Attach => "attach",
        InboundFrameKind::Flow => "flow",
        InboundFrameKind::Transfer => "transfer",
        InboundFrameKind::Disposition => "disposition",
        InboundFrameKind::Detach => "detach",
        InboundFrameKind::End => "end",
        InboundFrameKind::Close => "close",
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn header_exchange_advances_to_hdr_exch() {
        let s = advance_connection(ConnectionState::Start, InboundFrameKind::Header).expect("ok");
        assert_eq!(s, ConnectionState::HdrRcvd);
        let s = advance_connection(s, InboundFrameKind::Header).expect("ok");
        assert_eq!(s, ConnectionState::HdrExch);
    }

    #[test]
    fn open_advances_to_opened_state() {
        let s = advance_connection(ConnectionState::HdrExch, InboundFrameKind::Open).expect("ok");
        assert_eq!(s, ConnectionState::OpenRcvd);
        let s = advance_connection(s, InboundFrameKind::Open).expect("ok");
        assert_eq!(s, ConnectionState::Opened);
    }

    #[test]
    fn close_in_opened_advances_to_closercvd_then_end() {
        let s = advance_connection(ConnectionState::Opened, InboundFrameKind::Close).expect("ok");
        assert_eq!(s, ConnectionState::CloseRcvd);
        let s = advance_connection(s, InboundFrameKind::End).expect("ok");
        assert_eq!(s, ConnectionState::End);
    }

    #[test]
    fn open_in_start_state_yields_illegal_transition() {
        let r = advance_connection(ConnectionState::Start, InboundFrameKind::Open);
        assert!(matches!(
            r,
            Err(EndpointError::IllegalStateTransition { .. })
        ));
    }

    #[test]
    fn begin_attach_flow_transfer_keep_opened_state() {
        let s = ConnectionState::Opened;
        for f in [
            InboundFrameKind::Begin,
            InboundFrameKind::Attach,
            InboundFrameKind::Flow,
            InboundFrameKind::Transfer,
            InboundFrameKind::Disposition,
            InboundFrameKind::Detach,
        ] {
            let next = advance_connection(s, f).expect("ok");
            assert_eq!(next, ConnectionState::Opened);
        }
    }

    #[test]
    fn session_state_default_is_unmapped() {
        assert_eq!(SessionState::default(), SessionState::Unmapped);
    }
}
