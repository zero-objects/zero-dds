// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Connection/Session-State-Machine fuer den DDS-AMQP-Endpoint.
//!
//! Spec-Quelle: dds-amqp-1.0-beta1.pdf §6.1 Direct-Embed Topology +
//! `amqp-1.0-transport` §2.4 Connection-State-Diagramm.

use alloc::string::String;

use crate::limits::ResourceLimits;
use crate::sasl::SaslState;

/// Spec §2.4 Connection-State.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConnectionState {
    /// Initial state vor dem Header-Exchange.
    #[default]
    Start,
    /// AMQP-Header empfangen.
    HdrRcvd,
    /// AMQP-Header beidseitig ausgetauscht.
    HdrExch,
    /// Open-Performative empfangen.
    OpenRcvd,
    /// Open-Performative beidseitig — Connection ist offen.
    Opened,
    /// Close-Performative empfangen — Cleanup-Phase.
    CloseRcvd,
    /// Close-Performative gesendet — Cleanup-Phase.
    CloseSent,
    /// Connection abgeschlossen.
    End,
}

/// Spec §2.5 Session-State.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SessionState {
    /// Initial state vor Begin-Performative.
    #[default]
    Unmapped,
    /// Begin-Performative empfangen.
    BeginRcvd,
    /// Begin-Performative beidseitig — Session offen.
    Mapped,
    /// End-Performative empfangen.
    EndRcvd,
    /// End-Performative gesendet.
    EndSent,
    /// Session geschlossen.
    Discarded,
}

/// Endpoint-Konfiguration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndpointConfig {
    /// Container-Id (Spec §2.4.1) — uniquely identifies the endpoint.
    pub container_id: String,
    /// Resource-Limits + DoS-Caps.
    pub limits: ResourceLimits,
    /// Aktuelle SASL-Verhandlung (None vor SASL-Phase).
    pub sasl: Option<SaslState>,
}

/// Endpoint-Fehler aus der State-Machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EndpointError {
    /// Spec §2.4 — illegal state transition.
    IllegalStateTransition {
        /// Vorher-Zustand.
        from: ConnectionState,
        /// Was wir versucht haben (z.B. "open").
        attempted: &'static str,
    },
    /// Resource-Limit ueberschritten.
    ResourceLimitExceeded(&'static str),
    /// Idle-Timeout ueberschritten.
    IdleTimeout,
    /// Connection wurde vom Peer abrupt geschlossen.
    ConnectionAborted,
}

/// Eingehende Frame-Klasse — vereinfacht; ein konkreter Listener
/// dispatched anhand von Performative-Descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InboundFrameKind {
    /// AMQP-Header (4-Byte Magic + 4-Byte Version).
    Header,
    /// Open-Performative.
    Open,
    /// Begin-Performative.
    Begin,
    /// Attach-Performative.
    Attach,
    /// Flow-Performative.
    Flow,
    /// Transfer-Performative.
    Transfer,
    /// Disposition-Performative.
    Disposition,
    /// Detach-Performative.
    Detach,
    /// End-Performative.
    End,
    /// Close-Performative.
    Close,
}

/// Spec §2.4 — advance the connection state machine on an incoming
/// frame.
///
/// # Errors
/// `IllegalStateTransition` wenn die State-Machine den Frame im
/// aktuellen State nicht akzeptiert.
pub fn advance_connection(
    state: ConnectionState,
    frame: InboundFrameKind,
) -> Result<ConnectionState, EndpointError> {
    use ConnectionState as C;
    use InboundFrameKind as F;
    let next = match (state, frame) {
        (C::Start, F::Header) => C::HdrRcvd,
        (C::HdrRcvd, F::Header) => C::HdrExch,
        // Server may permit OPEN-RCVD direkt nach HdrExch.
        (C::HdrExch, F::Open) => C::OpenRcvd,
        (C::OpenRcvd, F::Open) => C::Opened,
        // Innerhalb von Opened sind Begin/Attach/etc erlaubt; State
        // bleibt Opened bis Close eintrifft.
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
