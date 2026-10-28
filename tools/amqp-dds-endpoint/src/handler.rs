// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Per-connection handler — drives the endpoint state machine.
//!
//! Spec dds-amqp-1.0 §2.1 Endpoint Profile:
//! * Cl. 2 — Connection Acceptance: Protocol-Handshake + State.
//! * Cl. 6 — SASL: PLAIN/ANONYMOUS/EXTERNAL.
//! * Cl. 7 — Mandatory TLS for PLAIN (im Daemon: tls_active-Flag).
//!
//! Structural flow:
//! 1. Optional SASL phase (client sends `AMQP\3\1\0\0`).
//! 2. AMQP phase (client sends `AMQP\0\1\0\0`).
//! 3. Open / Begin / Attach / Transfer / ... bis Close.

use std::io::{Read, Write};
use std::sync::Arc;

use zerodds_amqp_bridge::extended_types::AmqpExtValue;
use zerodds_amqp_bridge::frame::FrameType;
use zerodds_amqp_bridge::performatives;
use zerodds_amqp_bridge::types::AmqpValue;
use zerodds_amqp_endpoint::security::SaslSubject;
use zerodds_amqp_endpoint::security::{
    AccessControlPlugin, AccessDecision, AccessOp, IdentityToken, build_identity_token,
};
use zerodds_amqp_endpoint::session::InboundFrameKind;
use zerodds_amqp_endpoint::{ConnectionState, EndpointError, MetricsHub, advance_connection};

use crate::frame_io::{
    AmqpProtocol, FrameIoError, read_frame, read_protocol_header, write_frame,
    write_protocol_header,
};

/// Per-connection statistics (for tests + metrics wiring).
#[derive(Debug, Default, Clone)]
pub struct ConnectionStats {
    /// Received frames (all types).
    pub frames_received: u64,
    /// Sent frames.
    pub frames_sent: u64,
    /// SASL phase completed?
    pub sasl_completed: bool,
    /// Open performative received?
    pub open_received: bool,
    /// Close performative received or sent?
    pub closed: bool,
}

/// Handler error.
#[derive(Debug)]
pub enum HandlerError {
    /// Frame-IO error.
    FrameIo(FrameIoError),
    /// State-Machine error.
    Endpoint(EndpointError),
    /// Performative decoding failed.
    PerformativeDecode(String),
    /// Connection was closed too early.
    UnexpectedEof,
    /// SASL refused the client (outcome `auth` was sent): the reason.
    Sasl(String),
}

impl core::fmt::Display for HandlerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::FrameIo(e) => write!(f, "frame io: {e}"),
            Self::Endpoint(e) => write!(f, "endpoint: {e:?}"),
            Self::PerformativeDecode(s) => write!(f, "performative decode: {s}"),
            Self::UnexpectedEof => write!(f, "unexpected eof"),
            Self::Sasl(s) => write!(f, "sasl: {s}"),
        }
    }
}

impl std::error::Error for HandlerError {}

impl From<FrameIoError> for HandlerError {
    fn from(e: FrameIoError) -> Self {
        Self::FrameIo(e)
    }
}

impl From<EndpointError> for HandlerError {
    fn from(e: EndpointError) -> Self {
        Self::Endpoint(e)
    }
}

/// Handler configuration per connection.
#[derive(Clone)]
pub struct HandlerConfig {
    /// Container-Id that we report back in the Open frame
    /// (Spec §2.4.1).
    pub container_id: String,
    /// Maximum frame size that we accept (DoS cap).
    pub max_frame_size: u32,
    /// Is TLS active? (Affects SASL-PLAIN acceptance Spec §10.2.1.)
    pub tls_active: bool,
    /// Metrics hub (counts frames, connections, errors).
    pub metrics: Arc<MetricsHub>,
    /// Spec §10.3.3 — AccessControl plugin for pre-attach +
    /// Pre-transfer checks (no-bypass §10.3.5). `None` = no
    /// Check (DDS security inactive).
    pub access_control: Option<Arc<dyn AccessControlPlugin + Send + Sync>>,
    /// Default identity for the pre-SASL phase (typically ANONYMOUS).
    /// Is set after successful SASL by the authenticated
    /// subject overridden.
    pub default_identity: IdentityToken,
    /// Which SASL mechanisms are offered and what they are checked against.
    pub sasl: crate::auth::SaslPolicy,
    /// DER of the client certificate of a TLS connection, the identity
    /// `EXTERNAL` stands for.
    pub client_cert: Option<Vec<u8>>,
}

impl core::fmt::Debug for HandlerConfig {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("HandlerConfig")
            .field("container_id", &self.container_id)
            .field("max_frame_size", &self.max_frame_size)
            .field("tls_active", &self.tls_active)
            .field("access_control_present", &self.access_control.is_some())
            .field(
                "default_identity_subject",
                &self.default_identity.subject_name,
            )
            .finish()
    }
}

impl HandlerConfig {
    /// Default with AllowAll plugin for tests.
    #[must_use]
    pub fn for_tests(metrics: Arc<MetricsHub>) -> Self {
        Self {
            container_id: "zerodds-amqp-endpoint".to_string(),
            max_frame_size: 1_048_576,
            tls_active: false,
            metrics,
            access_control: None,
            default_identity: build_identity_token(&SaslSubject::Anonymous),
            sasl: crate::auth::SaslPolicy::default(),
            client_cert: None,
        }
    }

    /// With AccessControl plugin.
    #[must_use]
    pub fn with_access_control(
        mut self,
        plugin: Arc<dyn AccessControlPlugin + Send + Sync>,
    ) -> Self {
        self.access_control = Some(plugin);
        self
    }

    /// With identity (e.g. after SASL).
    #[must_use]
    pub fn with_identity(mut self, identity: IdentityToken) -> Self {
        self.default_identity = identity;
        self
    }
}

/// Handle a single AMQP-1.0 connection blocking on `stream`.
///
/// Spec §2.1 Cl. 2 — executes:
/// 1. Protocol-Header-Exchange (optional SASL → AMQP).
/// 2. SASL mechanism negotiation (if a SASL header).
/// 3. Open / Begin / Attach / Transfer / ... bis Close.
///
/// # Errors
/// See [`HandlerError`].
pub fn handle_connection<S: Read + Write>(
    stream: &mut S,
    cfg: &HandlerConfig,
) -> Result<ConnectionStats, HandlerError> {
    cfg.metrics.on_connection_open();
    let mut stats = ConnectionStats::default();

    // Spec §2.2 — read the first protocol header. The client can choose SASL
    // (0x03) or AMQP (0x00).
    let first = read_protocol_header(stream)?;
    let mut conn_cfg = cfg.clone();
    match first.protocol {
        AmqpProtocol::Sasl => {
            // SASL phase: the server accepts the SASL header and
            // starts negotiation.
            let subject = do_sasl_phase(stream, cfg, &mut stats)?;
            conn_cfg.default_identity = build_identity_token(&subject);
            // After successful SASL comes the second
            // Protocol header — this time AMQP.
            let second = read_protocol_header(stream)?;
            if second.protocol != AmqpProtocol::Amqp {
                return Err(HandlerError::FrameIo(FrameIoError::UnsupportedProtocolId(
                    second.protocol.as_bytes()[4],
                )));
            }
            // Server sends its AMQP header back.
            write_protocol_header(stream, AmqpProtocol::Amqp)?;
        }
        AmqpProtocol::Amqp if !cfg.sasl.allows_no_sasl() => {
            // AMQP 1.0 §5.3.1: a server that requires SASL answers the AMQP
            // header with the SASL header and closes.
            write_protocol_header(stream, AmqpProtocol::Sasl)?;
            cfg.metrics.on_connection_close();
            return Err(HandlerError::Sasl(
                "the client skipped SASL, which this endpoint requires".to_string(),
            ));
        }
        AmqpProtocol::Amqp => {
            // Direct AMQP phase: server confirms with its own header.
            write_protocol_header(stream, AmqpProtocol::Amqp)?;
        }
    }
    let cfg = &conn_cfg;

    // Both AMQP headers exchanged — drive state machine.
    let mut state = ConnectionState::Start;
    state = advance_connection(state, InboundFrameKind::Header)?;
    state = advance_connection(state, InboundFrameKind::Header)?;

    // Open-Begin-Attach-Loop bis Close.
    do_amqp_phase(stream, cfg, &mut stats, &mut state)?;

    cfg.metrics.on_connection_close();
    stats.closed = true;
    Ok(stats)
}

fn do_sasl_phase<S: Read + Write>(
    stream: &mut S,
    cfg: &HandlerConfig,
    stats: &mut ConnectionStats,
) -> Result<SaslSubject, HandlerError> {
    use zerodds_amqp_endpoint::sasl::SaslMechanism;
    // Server sends its SASL header back.
    write_protocol_header(stream, AmqpProtocol::Sasl)?;

    // sasl-mechanisms (AMQP 1.0 §5.3.3.1): what this connection may use.
    let offered = cfg.sasl.offered(cfg.tls_active, cfg.client_cert.is_some());
    let mechs = AmqpExtValue::List(vec![AmqpExtValue::Array(
        offered
            .iter()
            .map(|m| AmqpExtValue::Symbol(m.name().to_string()))
            .collect(),
    )]);
    send_sasl(stream, stats, 0x40, &mechs)?;

    // sasl-init (§5.3.3.2): [mechanism, initial-response, hostname].
    let fields = read_sasl(stream, cfg, stats, 0x41)?;
    let mechanism = match fields.first() {
        Some(AmqpExtValue::Symbol(m)) => m.clone(),
        _ => return refuse(stream, stats, "sasl-init without a mechanism"),
    };
    let response = match fields.get(1) {
        Some(AmqpExtValue::Binary(b)) => b.clone(),
        _ => Vec::new(),
    };
    let chosen = SaslMechanism::from_name(&mechanism).filter(|m| offered.contains(m));
    let (subject, additional) = match chosen {
        None => {
            return refuse(
                stream,
                stats,
                &format!("mechanism {mechanism} is not offered on this connection"),
            );
        }
        Some(SaslMechanism::Anonymous) => (SaslSubject::Anonymous, None),
        Some(SaslMechanism::Plain) => {
            let Some((user, password)) = crate::auth::parse_plain(&response) else {
                return refuse(stream, stats, "malformed PLAIN response");
            };
            let ok = cfg
                .sasl
                .store
                .as_ref()
                .is_some_and(|s| s.verify_plain(&user, &password));
            if !ok {
                return refuse(
                    stream,
                    stats,
                    &format!("PLAIN credentials of {user} rejected"),
                );
            }
            (SaslSubject::Plain { authcid: user }, None)
        }
        Some(SaslMechanism::External) => {
            let Some(cert) = cfg.client_cert.clone() else {
                return refuse(stream, stats, "EXTERNAL without a client certificate");
            };
            let subject_dn = format!("mtls:{}", crate::auth::sha256_hex(&cert));
            (
                SaslSubject::External {
                    certificate: cert,
                    subject_dn,
                },
                None,
            )
        }
        Some(SaslMechanism::ScramSha256) => {
            let Some(store) = cfg.sasl.store.as_ref() else {
                return refuse(stream, stats, "SCRAM-SHA-256 without a credential store");
            };
            let Some(nonce) = crate::auth::server_nonce() else {
                return refuse(stream, stats, "no random source for the SCRAM nonce");
            };
            let client_first = String::from_utf8_lossy(&response).into_owned();
            let (exchange, server_first) =
                match zerodds_amqp_endpoint::scram::ScramServerExchange::start(
                    &client_first,
                    &nonce,
                    |u| store.secrets(u),
                ) {
                    Ok(v) => v,
                    Err(e) => return refuse(stream, stats, &format!("SCRAM: {e}")),
                };
            // sasl-challenge (§5.3.3.3) / sasl-response (§5.3.3.4).
            send_sasl(
                stream,
                stats,
                0x42,
                &AmqpExtValue::List(vec![AmqpExtValue::Binary(server_first.into_bytes())]),
            )?;
            let fields = read_sasl(stream, cfg, stats, 0x43)?;
            let client_final = match fields.first() {
                Some(AmqpExtValue::Binary(b)) => String::from_utf8_lossy(b).into_owned(),
                _ => return refuse(stream, stats, "sasl-response without a response"),
            };
            match exchange.finish(&client_final) {
                zerodds_amqp_endpoint::scram::ScramStep::Success {
                    server_final,
                    username,
                } => (
                    SaslSubject::ScramSha256 { authcid: username },
                    Some(server_final.into_bytes()),
                ),
                zerodds_amqp_endpoint::scram::ScramStep::Failure(e) => {
                    return refuse(stream, stats, &format!("SCRAM: {e}"));
                }
            }
        }
    };
    // sasl-outcome (§5.3.3.6): code ok, and SCRAM's server-final as
    // additional-data.
    let mut outcome = vec![AmqpExtValue::Ubyte(0)];
    if let Some(data) = additional {
        outcome.push(AmqpExtValue::Binary(data));
    }
    send_sasl(stream, stats, 0x44, &AmqpExtValue::List(outcome))?;
    stats.sasl_completed = true;
    Ok(subject)
}

fn send_sasl<S: Write>(
    stream: &mut S,
    stats: &mut ConnectionStats,
    descriptor: u64,
    body: &AmqpExtValue,
) -> Result<(), HandlerError> {
    let bytes = performatives::encode_performative(descriptor, body)
        .map_err(|e| HandlerError::PerformativeDecode(format!("{e}")))?;
    write_frame(stream, FrameType::Sasl, 0, &bytes)?;
    stats.frames_sent += 1;
    Ok(())
}

/// Reads one SASL frame and returns the fields of the performative
/// `descriptor` names.
fn read_sasl<S: Read>(
    stream: &mut S,
    cfg: &HandlerConfig,
    stats: &mut ConnectionStats,
    descriptor: u64,
) -> Result<Vec<AmqpExtValue>, HandlerError> {
    let frame = read_frame(stream, cfg.max_frame_size)?;
    stats.frames_received += 1;
    if frame.header.frame_type != FrameType::Sasl {
        return Err(HandlerError::FrameIo(FrameIoError::UnsupportedProtocolId(
            frame.header.frame_type.to_u8(),
        )));
    }
    let (got, body, _) = performatives::decode_performative(&frame.body)
        .map_err(|e| HandlerError::PerformativeDecode(format!("{e}")))?;
    if got != descriptor {
        return Err(HandlerError::PerformativeDecode(format!(
            "expected SASL performative {descriptor:#x}, got {got:#x}"
        )));
    }
    Ok(match body {
        AmqpExtValue::List(items) => items,
        other => vec![other],
    })
}

/// Sends sasl-outcome `auth` (code 1) and ends the connection
/// (DDS-AMQP 1.0 §2.1 Cl. 7).
fn refuse<S: Write, T>(
    stream: &mut S,
    stats: &mut ConnectionStats,
    reason: &str,
) -> Result<T, HandlerError> {
    send_sasl(
        stream,
        stats,
        0x44,
        &AmqpExtValue::List(vec![AmqpExtValue::Ubyte(1)]),
    )?;
    Err(HandlerError::Sasl(reason.to_string()))
}

fn do_amqp_phase<S: Read + Write>(
    stream: &mut S,
    cfg: &HandlerConfig,
    stats: &mut ConnectionStats,
    state: &mut ConnectionState,
) -> Result<(), HandlerError> {
    loop {
        let frame = match read_frame(stream, cfg.max_frame_size) {
            Ok(f) => f,
            Err(FrameIoError::Io(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                // Peer closed abruptly.
                return Ok(());
            }
            Err(e) => return Err(HandlerError::FrameIo(e)),
        };
        stats.frames_received += 1;

        // Empty-Frame = Heartbeat (§2.4.5).
        if frame.body.is_empty() {
            // Reply with own heartbeat? No — server is passive.
            continue;
        }

        // Decode performative.
        let kind = match classify_performative(&frame.body) {
            Some(k) => k,
            None => {
                // Unknown performative — ignore frame.
                cfg.metrics.on_decode_error();
                continue;
            }
        };

        // Drive state machine.
        *state = advance_connection(*state, kind)?;

        match kind {
            InboundFrameKind::Open => {
                stats.open_received = true;
                // The server sends its own open back — and the
                // Outbound Open advances the connection state to
                // Opened (Spec §2.4 OpenRcvd → Opened).
                let open = performatives::open(&cfg.container_id)
                    .map_err(|e| HandlerError::PerformativeDecode(format!("{e}")))?;
                write_frame(stream, FrameType::Amqp, 0, &open)?;
                stats.frames_sent += 1;
                *state = advance_connection(*state, InboundFrameKind::Open)?;
            }
            InboundFrameKind::Begin => {
                // Echo Begin on channel 0; remote-channel = 0.
                let begin = performatives::begin(Some(0), 0, 1024, 1024)
                    .map_err(|e| HandlerError::PerformativeDecode(format!("{e}")))?;
                write_frame(stream, FrameType::Amqp, frame.header.channel, &begin)?;
                stats.frames_sent += 1;
            }
            InboundFrameKind::Attach => {
                // Spec §10.3.3 / §10.3.5 — Pre-Attach-AccessControl-
                // Check. On Deny: detach with
                // `amqp:unauthorized-access` + Counter-Inkrement.
                let (link_name, target_addr, is_sender) = parse_attach(&frame.body);
                if !check_access(
                    cfg,
                    &target_addr,
                    if is_sender {
                        AccessOp::AttachReceiver
                    } else {
                        AccessOp::AttachSender
                    },
                ) {
                    cfg.metrics.on_unauthorized();
                    let detach = performatives::detach(0, true)
                        .map_err(|e| HandlerError::PerformativeDecode(format!("{e}")))?;
                    write_frame(stream, FrameType::Amqp, frame.header.channel, &detach)?;
                    stats.frames_sent += 1;
                    continue;
                }
                // Echo Attach with own handle 0 (sender direction).
                let attach = performatives::attach(&link_name, 0, true)
                    .map_err(|e| HandlerError::PerformativeDecode(format!("{e}")))?;
                write_frame(stream, FrameType::Amqp, frame.header.channel, &attach)?;
                stats.frames_sent += 1;
            }
            InboundFrameKind::Transfer => {
                // Spec §10.3.5 No-Bypass — Pre-Transfer-Check.
                // Inbound transfer = receiver side from the server's view;
                // we check ReceiveSample. On Deny: silently
                // count, but send Disposition rejected.
                if !check_access(cfg, "<transfer>", AccessOp::ReceiveSample) {
                    cfg.metrics.on_unauthorized();
                    continue;
                }
                cfg.metrics.on_transfer_received();
                // Pre-settled accept; no disposition.
            }
            InboundFrameKind::Close => {
                // Reply with own Close + advance state to End
                // (CloseRcvd → End per State-Machine).
                let close = performatives::close()
                    .map_err(|e| HandlerError::PerformativeDecode(format!("{e}")))?;
                write_frame(stream, FrameType::Amqp, 0, &close)?;
                stats.frames_sent += 1;
                *state = advance_connection(*state, InboundFrameKind::Close)?;
                return Ok(());
            }
            InboundFrameKind::End => {
                let end = performatives::end()
                    .map_err(|e| HandlerError::PerformativeDecode(format!("{e}")))?;
                write_frame(stream, FrameType::Amqp, frame.header.channel, &end)?;
                stats.frames_sent += 1;
            }
            // Flow / Disposition / Detach: silently ack.
            InboundFrameKind::Flow | InboundFrameKind::Disposition | InboundFrameKind::Detach => {}
            InboundFrameKind::Header => {
                // Should no longer arrive after the handshake — ignore.
            }
        }
    }
}

/// Spec §10.3.5 — Pre-Op-AccessControl-Check.
///
/// Returns `true` on `Allow` (or a missing plugin = default
/// Allow); `false` on `Deny`. Caller handles Deny (detach,
/// Drop, etc.).
fn check_access(cfg: &HandlerConfig, address: &str, op: AccessOp) -> bool {
    let Some(plugin) = cfg.access_control.as_ref() else {
        // No plugin = no check (DDS security inactive).
        return true;
    };
    matches!(
        plugin.check(&cfg.default_identity, address, op),
        AccessDecision::Allow
    )
}

/// Spec §2.6.1 — extract Attach body fields.
///
/// Returns `(link_name, target_address, is_sender)`. If the
/// body format is unexpected, we fall back to defaults
/// (link_name="link", target_address="<unknown>", is_sender=true).
fn parse_attach(body: &[u8]) -> (String, String, bool) {
    let default = ("link".to_string(), "<unknown>".to_string(), true);
    let Ok((_, body_value, _)) = zerodds_amqp_bridge::performatives::decode_performative(body)
    else {
        return default;
    };
    let AmqpExtValue::List(items) = body_value else {
        return default;
    };
    // Spec attach-list: [name, handle, role, snd-settle-mode, rcv-settle-mode,
    //                    source, target, ...]
    let link_name = items
        .first()
        .and_then(|v| match v {
            AmqpExtValue::Str(s) => Some(s.clone()),
            _ => None,
        })
        .unwrap_or_else(|| default.0.clone());
    // role: bool (false=sender, true=receiver)
    let is_sender_from_role = items
        .get(2)
        .map(|v| matches!(v, AmqpExtValue::Boolean(false)))
        .unwrap_or(default.2);
    // The target address is at index 6; in many implementations
    // this is a described composite with an address string. We
    // try to decode a string or a map with an "address" key
    // extract.
    let target_addr = items
        .get(6)
        .and_then(extract_address)
        .or_else(|| items.get(5).and_then(extract_address))
        .unwrap_or_else(|| default.1.clone());
    (link_name, target_addr, is_sender_from_role)
}

fn extract_address(v: &AmqpExtValue) -> Option<String> {
    match v {
        AmqpExtValue::Str(s) => Some(s.clone()),
        AmqpExtValue::Symbol(s) => Some(s.clone()),
        AmqpExtValue::List(items) => items.first().and_then(|x| match x {
            AmqpExtValue::Str(s) | AmqpExtValue::Symbol(s) => Some(s.clone()),
            _ => None,
        }),
        _ => None,
    }
}

/// Classifies a performative body as `InboundFrameKind` by
/// Descriptor-Code (Spec §2.7 Tab 2.7).
#[must_use]
pub fn classify_performative(body: &[u8]) -> Option<InboundFrameKind> {
    // Body begins with `0x00` (described) + descriptor + list body.
    // We read the descriptor (Ulong).
    let (descriptor, _, _) = zerodds_amqp_bridge::performatives::decode_performative(body).ok()?;
    descriptor_to_kind(descriptor)
}

const fn descriptor_to_kind(descriptor: u64) -> Option<InboundFrameKind> {
    use zerodds_amqp_bridge::performatives::descriptor as d;
    let kind = match descriptor {
        d::OPEN => InboundFrameKind::Open,
        d::BEGIN => InboundFrameKind::Begin,
        d::ATTACH => InboundFrameKind::Attach,
        d::FLOW => InboundFrameKind::Flow,
        d::TRANSFER => InboundFrameKind::Transfer,
        d::DISPOSITION => InboundFrameKind::Disposition,
        d::DETACH => InboundFrameKind::Detach,
        d::END => InboundFrameKind::End,
        d::CLOSE => InboundFrameKind::Close,
        _ => return None,
    };
    Some(kind)
}

// AmqpValue is referenced by the Vec helpers — keep the import alive.
const _: Option<AmqpValue> = None;

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn cfg() -> HandlerConfig {
        HandlerConfig::for_tests(Arc::new(MetricsHub::new()))
    }

    /// Round-trip helper: writes a sequence of bytes as
    /// "client input" and collects server output into a Vec.
    struct DuplexCursor {
        input: Cursor<Vec<u8>>,
        output: Vec<u8>,
    }
    impl Read for DuplexCursor {
        fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
            self.input.read(b)
        }
    }
    impl Write for DuplexCursor {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.output.write(b)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn duplex(input: Vec<u8>) -> DuplexCursor {
        DuplexCursor {
            input: Cursor::new(input),
            output: Vec::new(),
        }
    }

    #[test]
    fn descriptor_classification_covers_9_performatives() {
        use zerodds_amqp_bridge::performatives::descriptor as d;
        for (code, expected) in [
            (d::OPEN, InboundFrameKind::Open),
            (d::BEGIN, InboundFrameKind::Begin),
            (d::ATTACH, InboundFrameKind::Attach),
            (d::FLOW, InboundFrameKind::Flow),
            (d::TRANSFER, InboundFrameKind::Transfer),
            (d::DISPOSITION, InboundFrameKind::Disposition),
            (d::DETACH, InboundFrameKind::Detach),
            (d::END, InboundFrameKind::End),
            (d::CLOSE, InboundFrameKind::Close),
        ] {
            assert_eq!(descriptor_to_kind(code), Some(expected));
        }
        assert_eq!(descriptor_to_kind(0xFFFF), None);
    }

    #[test]
    fn handle_connection_open_close_round_trip() {
        // Client sequence: AMQP header + Open + Close.
        let mut input = Vec::new();
        input.extend(AmqpProtocol::Amqp.as_bytes()); // protocol header
        // Open-Performative.
        let open = performatives::open("client").unwrap();
        let header = zerodds_amqp_bridge::frame::FrameHeader {
            size: 8 + open.len() as u32,
            doff: 2,
            frame_type: FrameType::Amqp,
            channel: 0,
        };
        input.extend(zerodds_amqp_bridge::frame::encode_frame_header(header));
        input.extend(&open);
        // Close-Performative.
        let close = performatives::close().unwrap();
        let header = zerodds_amqp_bridge::frame::FrameHeader {
            size: 8 + close.len() as u32,
            doff: 2,
            frame_type: FrameType::Amqp,
            channel: 0,
        };
        input.extend(zerodds_amqp_bridge::frame::encode_frame_header(header));
        input.extend(&close);

        let mut io = duplex(input);
        let stats = handle_connection(&mut io, &cfg()).unwrap();
        assert!(stats.open_received);
        assert!(stats.closed);
        assert_eq!(stats.frames_received, 2);
        // Server has: AMQP header + Open reply + Close reply.
        assert!(stats.frames_sent >= 2);
        // Server output begins with AMQP header.
        assert_eq!(&io.output[0..4], b"AMQP");
    }

    #[test]
    fn handle_connection_invalid_magic_rejected() {
        let bad = b"NOPE\x00\x01\x00\x00";
        let mut io = duplex(bad.to_vec());
        let err = handle_connection(&mut io, &cfg()).unwrap_err();
        assert!(matches!(
            err,
            HandlerError::FrameIo(FrameIoError::InvalidProtocolMagic(_))
        ));
    }

    #[test]
    fn handle_connection_sasl_then_amqp() {
        // Client: SASL header → expects sasl-mechanisms +
        // sends sasl-init → expects sasl-outcome → sends
        // AMQP header → sends Open → sends Close.
        let mut input = Vec::new();
        input.extend(AmqpProtocol::Sasl.as_bytes());
        // sasl-init: descriptor 0x41, body list.
        let sasl_init_descriptor = 0x0000_0000_0000_0041u64;
        let init_body = AmqpExtValue::List(vec![AmqpExtValue::Symbol("ANONYMOUS".into())]);
        let init_payload =
            performatives::encode_performative(sasl_init_descriptor, &init_body).unwrap();
        let header = zerodds_amqp_bridge::frame::FrameHeader {
            size: 8 + init_payload.len() as u32,
            doff: 2,
            frame_type: FrameType::Sasl,
            channel: 0,
        };
        input.extend(zerodds_amqp_bridge::frame::encode_frame_header(header));
        input.extend(&init_payload);
        // Second protocol header: AMQP.
        input.extend(AmqpProtocol::Amqp.as_bytes());
        // Open + Close.
        let open = performatives::open("client").unwrap();
        let h = zerodds_amqp_bridge::frame::FrameHeader {
            size: 8 + open.len() as u32,
            doff: 2,
            frame_type: FrameType::Amqp,
            channel: 0,
        };
        input.extend(zerodds_amqp_bridge::frame::encode_frame_header(h));
        input.extend(&open);
        let close = performatives::close().unwrap();
        let h = zerodds_amqp_bridge::frame::FrameHeader {
            size: 8 + close.len() as u32,
            doff: 2,
            frame_type: FrameType::Amqp,
            channel: 0,
        };
        input.extend(zerodds_amqp_bridge::frame::encode_frame_header(h));
        input.extend(&close);

        let mut io = duplex(input);
        let stats = handle_connection(&mut io, &cfg()).unwrap();
        assert!(stats.sasl_completed);
        assert!(stats.open_received);
        assert!(stats.closed);
    }

    #[test]
    fn access_control_deny_attach_yields_unauthorized_metric() {
        use zerodds_amqp_endpoint::security::{
            AccessControlPlugin, AccessDecision, AccessOp, IdentityToken,
        };
        struct DenyAll;
        impl AccessControlPlugin for DenyAll {
            fn check(&self, _: &IdentityToken, _: &str, _: AccessOp) -> AccessDecision {
                AccessDecision::Deny
            }
        }

        let metrics = Arc::new(MetricsHub::new());
        let cfg = HandlerConfig::for_tests(metrics.clone()).with_access_control(Arc::new(DenyAll));

        // Build input: AMQP-Header, Open, Attach, Close.
        let mut input = Vec::new();
        input.extend(AmqpProtocol::Amqp.as_bytes());
        let open = performatives::open("c").unwrap();
        let h = zerodds_amqp_bridge::frame::FrameHeader {
            size: 8 + open.len() as u32,
            doff: 2,
            frame_type: FrameType::Amqp,
            channel: 0,
        };
        input.extend(zerodds_amqp_bridge::frame::encode_frame_header(h));
        input.extend(&open);

        // Attach (Sender, Handle 0).
        let attach = performatives::attach("L", 0, true).unwrap();
        let h = zerodds_amqp_bridge::frame::FrameHeader {
            size: 8 + attach.len() as u32,
            doff: 2,
            frame_type: FrameType::Amqp,
            channel: 0,
        };
        input.extend(zerodds_amqp_bridge::frame::encode_frame_header(h));
        input.extend(&attach);

        let close = performatives::close().unwrap();
        let h = zerodds_amqp_bridge::frame::FrameHeader {
            size: 8 + close.len() as u32,
            doff: 2,
            frame_type: FrameType::Amqp,
            channel: 0,
        };
        input.extend(zerodds_amqp_bridge::frame::encode_frame_header(h));
        input.extend(&close);

        let mut io = duplex(input);
        handle_connection(&mut io, &cfg).unwrap();
        // Spec §10.3.3 — DenyAll on attach produces
        // errors.unauthorized++.
        assert!(metrics.snapshot("errors.unauthorized").unwrap_or(0) >= 1);
    }

    #[test]
    fn access_control_allow_does_not_increment_unauthorized() {
        use zerodds_amqp_endpoint::security::AllowAll;
        let metrics = Arc::new(MetricsHub::new());
        let cfg = HandlerConfig::for_tests(metrics.clone()).with_access_control(Arc::new(AllowAll));

        let mut input = Vec::new();
        input.extend(AmqpProtocol::Amqp.as_bytes());
        let open = performatives::open("c").unwrap();
        let h = zerodds_amqp_bridge::frame::FrameHeader {
            size: 8 + open.len() as u32,
            doff: 2,
            frame_type: FrameType::Amqp,
            channel: 0,
        };
        input.extend(zerodds_amqp_bridge::frame::encode_frame_header(h));
        input.extend(&open);
        let close = performatives::close().unwrap();
        let h = zerodds_amqp_bridge::frame::FrameHeader {
            size: 8 + close.len() as u32,
            doff: 2,
            frame_type: FrameType::Amqp,
            channel: 0,
        };
        input.extend(zerodds_amqp_bridge::frame::encode_frame_header(h));
        input.extend(&close);

        let mut io = duplex(input);
        handle_connection(&mut io, &cfg).unwrap();
        assert_eq!(metrics.snapshot("errors.unauthorized"), Some(0));
    }

    #[test]
    fn metrics_counter_incremented_on_connection() {
        let m = Arc::new(MetricsHub::new());
        let cfg = HandlerConfig::for_tests(m.clone());
        let mut input = Vec::new();
        input.extend(AmqpProtocol::Amqp.as_bytes());
        let close = performatives::close().unwrap();
        let h = zerodds_amqp_bridge::frame::FrameHeader {
            size: 8 + close.len() as u32,
            doff: 2,
            frame_type: FrameType::Amqp,
            channel: 0,
        };
        // Open first, then close.
        let open = performatives::open("c").unwrap();
        let oh = zerodds_amqp_bridge::frame::FrameHeader {
            size: 8 + open.len() as u32,
            doff: 2,
            frame_type: FrameType::Amqp,
            channel: 0,
        };
        input.extend(zerodds_amqp_bridge::frame::encode_frame_header(oh));
        input.extend(&open);
        input.extend(zerodds_amqp_bridge::frame::encode_frame_header(h));
        input.extend(&close);
        let mut io = duplex(input);
        handle_connection(&mut io, &cfg).unwrap();
        // Connection opened+closed → active gauge at 0,
        // total at 1.
        assert_eq!(m.snapshot("connections.active"), Some(0));
        assert_eq!(m.snapshot("connections.total"), Some(1));
    }
}
