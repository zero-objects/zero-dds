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

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::sync::Arc;
use std::time::{Duration, Instant};

use zerodds_amqp_bridge::extended_types::AmqpExtValue;
use zerodds_amqp_bridge::frame::FrameType;
use zerodds_amqp_bridge::performatives;
use zerodds_amqp_bridge::types::AmqpValue;
use zerodds_amqp_endpoint::link::LinkRole;
use zerodds_amqp_endpoint::peer::{
    Answer, Attached, Container, Delivery, LinkId, LinkInfo, Peer, PeerConfig, SendError, condition,
};
use zerodds_amqp_endpoint::security::SaslSubject;
use zerodds_amqp_endpoint::security::{
    AccessControlPlugin, AccessDecision, AccessOp, IdentityToken, build_identity_token,
};
use zerodds_amqp_endpoint::session::InboundFrameKind;
use zerodds_amqp_endpoint::terminus::{
    AmqpError, DeliveryState, ExpiryPolicy, OUTCOME_SYMBOLS, Outcome, Source, Target, Terminus,
};
use zerodds_amqp_endpoint::transactions::{
    Coordinator, CoordinatorRequest, TransactionError, TransactionalState, TxnCapability, TxnId,
    coordinator_request,
};
use zerodds_amqp_endpoint::txn_resource::{Discharged, TxnTable};
use zerodds_amqp_endpoint::{EndpointError, MetricsHub, TxnLimits};

use crate::frame_io::{
    AmqpProtocol, FrameIoError, FrameReader, Polled, Transport, read_frame, read_protocol_header,
    write_frame, write_protocol_header,
};
use crate::nodes::{Message, Nodes, Put, Resolved};

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
    /// This endpoint closed the connection with an error (AMQP 1.0 Part 2
    /// §2.4.3): condition and description.
    Closed(String),
}

impl core::fmt::Display for HandlerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::FrameIo(e) => write!(f, "frame io: {e}"),
            Self::Endpoint(e) => write!(f, "endpoint: {e:?}"),
            Self::PerformativeDecode(s) => write!(f, "performative decode: {s}"),
            Self::UnexpectedEof => write!(f, "unexpected eof"),
            Self::Sasl(s) => write!(f, "sasl: {s}"),
            Self::Closed(s) => write!(f, "closed with {s}"),
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
    /// The nodes links attach to (AMQP 1.0 Part 3 §3.3), shared by the
    /// connections of one listener.
    pub nodes: Arc<Nodes>,
    /// The idle time-out of the connection (Part 2 §2.4.5): closed when
    /// the peer sends no frame for that long; `open.idle-time-out` states
    /// half of it. `None`: none.
    pub idle_time_out: Option<Duration>,
    /// `open.channel-max` (Part 2 §2.7.1): the highest channel number,
    /// one less than the sessions a connection may have.
    pub channel_max: u16,
    /// `begin.handle-max` (Part 2 §2.7.2): the highest handle, one less
    /// than the links a session may have.
    pub handle_max: u32,
    /// How long one read of the socket waits before the connection looks
    /// at its timers and at the nodes its links send from.
    pub poll_interval: Duration,
    /// The bounds of the transactions of a connection (AMQP 1.0 Part 4):
    /// how many it holds undischarged, and how long one may stay so.
    pub transactions: TxnLimits,
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
        let limits = zerodds_amqp_endpoint::ResourceLimits::default();
        Self {
            container_id: "zerodds-amqp-endpoint".to_string(),
            max_frame_size: 1_048_576,
            tls_active: false,
            metrics,
            access_control: None,
            default_identity: build_identity_token(&SaslSubject::Anonymous),
            sasl: crate::auth::SaslPolicy::default(),
            client_cert: None,
            nodes: Arc::new(Nodes::new(true)),
            idle_time_out: Some(Duration::from_millis(limits.idle_timeout_ms)),
            channel_max: u16::try_from(limits.max_sessions_per_connection.saturating_sub(1))
                .unwrap_or(u16::MAX),
            handle_max: limits.max_links_per_session.saturating_sub(1),
            poll_interval: Duration::from_millis(20),
            transactions: TxnLimits::default(),
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
pub fn handle_connection<S: Transport>(
    stream: &mut S,
    cfg: &HandlerConfig,
) -> Result<ConnectionStats, HandlerError> {
    cfg.metrics.on_connection_open();
    let mut stats = ConnectionStats::default();
    let result = serve(stream, cfg, &mut stats);
    cfg.metrics.on_connection_close();
    result?;
    stats.closed = true;
    Ok(stats)
}

/// AMQP 1.0 §2.2: "If the requested protocol version is not supported, the
/// server MUST send a protocol header with a supported protocol version and
/// then close the socket", and the same for a header the server cannot
/// parse. `supported` is the header this endpoint expects at this point.
/// The outgoing stream is closed behind the header and the incoming one
/// is read to its end ("both peers MUST close their outgoing stream and
/// SHOULD read the incoming stream until it is terminated"): a socket
/// closed over unread octets would be reset, and the reset can overtake
/// the header.
fn refuse_header<S: Transport>(
    stream: &mut S,
    supported: AmqpProtocol,
    error: FrameIoError,
) -> HandlerError {
    if matches!(
        error,
        FrameIoError::InvalidProtocolMagic(_)
            | FrameIoError::UnsupportedProtocolId(_)
            | FrameIoError::UnsupportedVersion { .. }
    ) {
        // The answer is owed to the peer; the socket is closed either way.
        let answered = write_protocol_header(stream, supported)
            .and_then(|()| Ok(stream.flush()?))
            .and_then(|()| Ok(stream.close_outgoing()?))
            .and_then(|()| Ok(stream.set_read_timeout(Some(REFUSED_PEER_WAIT))?));
        match answered {
            Ok(()) => {
                let mut discard = [0u8; 1024];
                let mut left = REFUSED_PEER_OCTETS;
                while left > 0 {
                    match stream.read(&mut discard) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => left = left.saturating_sub(n),
                    }
                }
            }
            Err(e) => eprintln!("the supported protocol header was not written: {e}"),
        }
    }
    HandlerError::FrameIo(error)
}

/// How long a peer whose header was refused is read before the socket is
/// closed.
const REFUSED_PEER_WAIT: Duration = Duration::from_secs(1);
/// How many octets of such a peer are read and discarded at most.
const REFUSED_PEER_OCTETS: usize = 64 * 1024;

fn serve<S: Transport>(
    stream: &mut S,
    cfg: &HandlerConfig,
    stats: &mut ConnectionStats,
) -> Result<(), HandlerError> {
    // Spec §2.2 — read the first protocol header. The client can choose SASL
    // (0x03) or AMQP (0x00). The header this endpoint supports first is the
    // SASL one where SASL is required (§5.3.1), else the AMQP one.
    let expected = if cfg.sasl.allows_no_sasl() {
        AmqpProtocol::Amqp
    } else {
        AmqpProtocol::Sasl
    };
    let first = read_protocol_header(stream).map_err(|e| refuse_header(stream, expected, e))?;
    let mut conn_cfg = cfg.clone();
    match first.protocol {
        AmqpProtocol::Sasl => {
            // SASL phase: the server accepts the SASL header and
            // starts negotiation.
            let subject = do_sasl_phase(stream, cfg, stats)?;
            conn_cfg.default_identity = build_identity_token(&subject);
            // After successful SASL comes the second
            // Protocol header — this time AMQP.
            let second = read_protocol_header(stream)
                .map_err(|e| refuse_header(stream, AmqpProtocol::Amqp, e))?;
            if second.protocol != AmqpProtocol::Amqp {
                return Err(refuse_header(
                    stream,
                    AmqpProtocol::Amqp,
                    FrameIoError::UnsupportedProtocolId(second.protocol.as_bytes()[4]),
                ));
            }
            // Server sends its AMQP header back.
            write_protocol_header(stream, AmqpProtocol::Amqp)?;
        }
        AmqpProtocol::Amqp if !cfg.sasl.allows_no_sasl() => {
            // AMQP 1.0 §5.3.1: a server that requires SASL answers the AMQP
            // header with the SASL header and closes.
            write_protocol_header(stream, AmqpProtocol::Sasl)?;
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

    // Both AMQP headers exchanged: open, sessions and links until close.
    do_amqp_phase(stream, cfg, stats)
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
    if offered.is_empty() {
        // §5.3.3.1 sasl-server-mechanisms: "It is invalid for this list to
        // be null or empty." No mechanism of the configuration works on
        // this connection (PLAIN without TLS, EXTERNAL without a client
        // certificate): the connection ends behind the header.
        return Err(HandlerError::Sasl(
            "no SASL mechanism of the configuration can be offered on this connection".to_string(),
        ));
    }
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

/// How long the peer is given to answer a close of this endpoint before
/// the socket is closed (Part 2 §2.4.3: "a peer SHOULD implement a timeout
/// to give its partner a reasonable time to receive and process the
/// close").
const CLOSE_WAIT_MS: u64 = 2_000;

/// The AMQP phase of a connection (Part 2 §2.4 to §2.6): the frames go to
/// the connection endpoint [`Peer`], whose links attach to the nodes of
/// the configuration.
fn do_amqp_phase<S: Transport>(
    stream: &mut S,
    cfg: &HandlerConfig,
    stats: &mut ConnectionStats,
) -> Result<(), HandlerError> {
    let started = Instant::now();
    let now = || u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let mut peer = Peer::new(
        PeerConfig {
            container_id: cfg.container_id.clone(),
            max_frame_size: cfg.max_frame_size,
            idle_time_out_ms: cfg
                .idle_time_out
                .map(|d| u32::try_from(d.as_millis()).unwrap_or(u32::MAX)),
            channel_max: cfg.channel_max,
            handle_max: cfg.handle_max,
            ..PeerConfig::default()
        },
        now(),
    );
    let mut app = ConnectionNodes::new(cfg);
    let mut reader = FrameReader::default();
    stream
        .set_read_timeout(Some(cfg.poll_interval))
        .map_err(FrameIoError::Io)?;
    let mut close_deadline: Option<u64> = None;
    let result = loop {
        let mut wrote = Ok(());
        while let Some(frame) = peer.poll_output() {
            wrote = stream.write_all(&frame);
            if wrote.is_err() {
                break;
            }
            stats.frames_sent += 1;
        }
        if let Err(e) = wrote.and_then(|()| stream.flush()) {
            break Err(HandlerError::FrameIo(FrameIoError::Io(e)));
        }
        stats.open_received |= peer.open_received();
        if peer.is_ended() {
            break Ok(());
        }
        if peer.is_closing() {
            let deadline = *close_deadline.get_or_insert(now().saturating_add(CLOSE_WAIT_MS));
            if now() >= deadline {
                break Ok(());
            }
        }
        match reader.poll(stream, cfg.max_frame_size) {
            Ok(Polled::Frame(frame)) => {
                stats.frames_received += 1;
                if frame.header.frame_type == FrameType::Amqp {
                    peer.frame(frame.header.channel, &frame.body, now(), &mut app);
                } else {
                    // Part 2 §2.3.1: the type of a frame on an AMQP
                    // connection is 0x00.
                    peer.fail_connection(
                        condition::FRAMING_ERROR,
                        format!(
                            "frame of type {:#04x} behind the AMQP protocol header",
                            frame.header.frame_type.to_u8()
                        ),
                        &mut app,
                    );
                }
            }
            Ok(Polled::Idle) => {}
            // The peer closed the stream.
            Ok(Polled::Closed) => break Ok(()),
            Err(FrameIoError::Io(e)) => break Err(HandlerError::FrameIo(FrameIoError::Io(e))),
            Err(e) => {
                // Part 2 §2.7.1 max-frame-size: "A peer that receives an
                // oversized frame MUST close the connection with the
                // framing-error error-code"; §2.8.16 for octets that are
                // no frame header. No frame boundary is found after that.
                reader.discard_rest();
                peer.fail_connection(condition::FRAMING_ERROR, e.to_string(), &mut app);
            }
        }
        peer.tick(now(), &mut app);
        app.pump(&mut peer, now());
    };
    // Whatever way the connection ended: its links are gone, and what
    // they held unsettled goes back to the nodes.
    peer.connection_lost(&mut app);
    if let Some(fault) = peer.fault() {
        return Err(HandlerError::PerformativeDecode(fault.to_string()));
    }
    if let Some(error) = peer.local_close() {
        if matches!(
            error.condition.as_str(),
            condition::DECODE_ERROR | condition::NOT_IMPLEMENTED | condition::FRAMING_ERROR
        ) {
            cfg.metrics.on_decode_error();
        }
        return result.and(Err(HandlerError::Closed(error.to_string())));
    }
    result
}

/// A delivery this endpoint sent on a link and that is at no terminal
/// state.
struct Inflight {
    message: Message,
    /// Sent settled. Such a delivery is held only while the transaction it
    /// was acquired in is undischarged.
    settled: bool,
    /// The undischarged transaction the link acquired the message in
    /// (Part 4 §4.4.3).
    acquired_in: Option<TxnId>,
    /// The undischarged transaction the partner's outcome for it belongs
    /// to (Part 4 §4.4.2).
    retiring_in: Option<TxnId>,
}

impl Inflight {
    fn plain(message: Message) -> Self {
        Self {
            message,
            settled: false,
            acquired_in: None,
            retiring_in: None,
        }
    }
}

/// What one link of a connection is attached to.
struct LinkState {
    /// The address of the node.
    address: String,
    /// Container and link name: what `undeliverable-here` refers to.
    key: String,
    /// The node was created for this link and goes with it (Part 3
    /// §3.5.10 delete-on-close).
    dynamic: bool,
    /// Messages sent on the link and not yet at a terminal state, by
    /// delivery tag.
    inflight: BTreeMap<Vec<u8>, Inflight>,
    /// The transaction the messages this link sends are acquired in
    /// (Part 4 §4.4.3), set by the `txn-id` of a flow.
    txn: Option<TxnId>,
}

/// Work a controller asked for within a transaction (Part 4 §4.4).
enum Work {
    /// §4.4.1: a message posted to a node; the node gets it with the
    /// commit.
    Post { address: String, message: Message },
    /// §4.4.2: the outcome of a delivery this endpoint sent; it applies
    /// with the commit. `settled`: the controller settled the delivery
    /// with it.
    Retire {
        link: LinkId,
        tag: Vec<u8>,
        outcome: Outcome,
        settled: bool,
    },
    /// §4.4.3: a message a link acquired; it returns to its node with a
    /// rollback.
    Acquire { link: LinkId, tag: Vec<u8> },
}

/// A control link (Part 4 §4.1).
struct Control {
    /// The source of the link supports the outcome `rejected` (§4.2): an
    /// error of a declare or a discharge is that outcome. Without it the
    /// link is detached with the error.
    rejects: bool,
}

/// What a callback of the connection endpoint left to do once the
/// endpoint can be asked again.
enum Action {
    /// Destroy the link with the error, once none of `after` is in the
    /// middle of its transfers any more: a control link ended with a
    /// transaction error goes after the abort of what its transactions
    /// acquired (Part 4 §4.4.3).
    Detach {
        link: LinkId,
        error: AmqpError,
        after: Withdrawn,
    },
    /// Settle a delivery this endpoint sent, with the state.
    Settle {
        link: LinkId,
        tag: Vec<u8>,
        state: DeliveryState,
    },
    /// The acquisition of a delivery was rolled back: abort it if it is
    /// in the middle of its transfers, settle it as released otherwise
    /// (`settled`: it was sent settled, there is nothing to settle).
    Withdraw {
        link: LinkId,
        tag: Vec<u8>,
        settled: bool,
    },
    /// The answer to a discharge, once none of `after` is in the middle
    /// of its transfers any more (Part 4 §4.4.3: "the resource MUST abort
    /// or complete sending the remainder of the message before completing
    /// the discharge").
    Answer {
        control: LinkId,
        delivery_id: u32,
        state: DeliveryState,
        after: Withdrawn,
    },
}

/// Deliveries whose acquisition was rolled back, by link and delivery
/// tag.
type Withdrawn = Vec<(LinkId, Vec<u8>)>;

/// The access-control name of the coordinator: what an attach to it is
/// checked against.
pub const COORDINATOR_ADDRESS: &str = "$coordinator";

/// The links of one connection and the nodes they attach to; the
/// transactional resource of the connection (Part 4).
struct ConnectionNodes<'a> {
    cfg: &'a HandlerConfig,
    links: BTreeMap<LinkId, LinkState>,
    next_tag: u64,
    /// The control links.
    controls: BTreeMap<LinkId, Control>,
    /// The transactions declared over them, each with its work.
    txns: TxnTable<LinkId, Work>,
    /// Deliveries posted to a transaction whose transfers are not
    /// complete (Part 4 §4.4.1), by link and delivery-id, with the
    /// transactions their frames named so far.
    partial: BTreeMap<(LinkId, u32), Vec<TxnId>>,
    actions: Vec<Action>,
    /// The time of the connection, in milliseconds.
    now_ms: u64,
}

impl<'a> ConnectionNodes<'a> {
    fn new(cfg: &'a HandlerConfig) -> Self {
        Self {
            cfg,
            links: BTreeMap::new(),
            next_tag: 0,
            controls: BTreeMap::new(),
            txns: TxnTable::new(cfg.transactions),
            partial: BTreeMap::new(),
            actions: Vec::new(),
            now_ms: 0,
        }
    }
}

impl ConnectionNodes<'_> {
    /// Part 2 §2.8.15 resource-limit-exceeded: the listener creates no
    /// more nodes.
    fn no_room_for_a_node(&self) -> AmqpError {
        AmqpError::new(
            condition::RESOURCE_LIMIT_EXCEEDED,
            format!(
                "the listener holds {} created nodes, its bound",
                self.cfg.nodes.limits().max_nodes
            ),
        )
    }

    /// The coordinator of this listener (Part 4 §4.5.1, §4.5.7): local
    /// transactions, several of them at a time on one session, and a
    /// txn-id used across the sessions of the connection.
    fn coordinator() -> Coordinator {
        Coordinator {
            capabilities: vec![
                TxnCapability::LocalTransactions,
                TxnCapability::MultiTxnsPerSsn,
                TxnCapability::MultiSsnsPerTxn,
            ],
        }
    }

    /// What the callbacks left to do, the transactions past their time,
    /// then: sends from the nodes on every link that has credit (Part 2
    /// §2.6.7); a link asked to drain whose node has nothing for it is
    /// drained.
    fn pump(&mut self, peer: &mut Peer, now_ms: u64) {
        self.now_ms = now_ms;
        // Part 4 §4.5.8 transaction-timeout.
        for (txn, work) in self.txns.expire(now_ms) {
            self.rollback(&txn, work, false);
        }
        self.run_actions(peer);
        for (id, credit, drain) in peer.sendable() {
            let Some(state) = self.links.get_mut(&id) else {
                continue;
            };
            // A link that acquires in a transaction which is no longer
            // live (timed out, not yet discharged) acquires nothing.
            if state
                .txn
                .as_ref()
                .is_some_and(|t| self.txns.check(t).is_err())
            {
                continue;
            }
            let presettled = peer.link(id).is_some_and(|l| l.snd_settle_mode == 1);
            let transactional = state
                .txn
                .as_ref()
                .map(|t| DeliveryState::Transactional(TransactionalState::of(t)));
            let mut sent = 0;
            let mut empty = false;
            while sent < credit {
                let Some(mut message) = self.cfg.nodes.take(&state.address, &state.key) else {
                    empty = true;
                    break;
                };
                self.next_tag += 1;
                let tag = self.next_tag.to_be_bytes();
                match peer.send_with_state(
                    id,
                    &tag,
                    message.format,
                    &message.payload,
                    false,
                    transactional.as_ref(),
                ) {
                    Ok(_) => {
                        self.cfg.metrics.on_transfer_sent(!presettled);
                        if let Some(txn) = &state.txn {
                            // Part 4 §4.4.3: acquired in the transaction;
                            // a rollback returns it to the node, also
                            // when it was sent settled.
                            if self
                                .txns
                                .record(
                                    txn,
                                    Work::Acquire {
                                        link: id,
                                        tag: tag.to_vec(),
                                    },
                                )
                                .is_ok()
                            {
                                state.inflight.insert(
                                    tag.to_vec(),
                                    Inflight {
                                        message,
                                        settled: presettled,
                                        acquired_in: Some(txn.clone()),
                                        retiring_in: None,
                                    },
                                );
                            }
                        } else if !presettled {
                            state
                                .inflight
                                .insert(tag.to_vec(), Inflight::plain(message));
                        }
                        sent += 1;
                    }
                    Err(
                        SendError::MessageTooLarge | SendError::FrameSizeTooSmall | SendError::Tag,
                    ) => {
                        // This link can never carry the message; it stays
                        // in the node for the others.
                        message.undeliverable.insert(state.key.clone());
                        self.cfg.nodes.put_back(&state.address, message);
                    }
                    Err(
                        SendError::NoCredit
                        | SendError::Window
                        | SendError::InProgress
                        | SendError::NoLink,
                    ) => {
                        // Not now: the session window is closed, or the
                        // delivery before this one still has frames to
                        // send (Part 2 §2.5.6, §2.6.14). The engine goes on
                        // with the next flow.
                        self.cfg.nodes.put_back(&state.address, message);
                        break;
                    }
                }
            }
            if drain && empty {
                peer.drained(id, self);
            }
        }
    }

    /// Does what the callbacks left to do, and what doing it leaves to do
    /// (the end of a control link rolls its transactions back); an answer
    /// that still waits for a delivery to end stays.
    fn run_actions(&mut self, peer: &mut Peer) {
        let mut waiting = Vec::new();
        while !self.actions.is_empty() {
            for action in core::mem::take(&mut self.actions) {
                match action {
                    Action::Detach { link, error, after } => {
                        if after
                            .iter()
                            .any(|(l, tag)| peer.sending(*l) == Some(tag.as_slice()))
                        {
                            waiting.push(Action::Detach { link, error, after });
                        } else {
                            peer.detach(link, Some(error), self);
                        }
                    }
                    Action::Settle { link, tag, state } => {
                        peer.settle(link, &tag, Some(&state));
                    }
                    Action::Withdraw { link, tag, settled } => {
                        if peer.sending(link) == Some(tag.as_slice()) {
                            peer.abort(link);
                        } else if !settled {
                            // The message is back at its node: for this
                            // delivery it is released (Part 3 §3.4.4).
                            peer.settle(
                                link,
                                &tag,
                                Some(&DeliveryState::Outcome(Outcome::Released)),
                            );
                        }
                    }
                    Action::Answer {
                        control,
                        delivery_id,
                        state,
                        after,
                    } => {
                        if after
                            .iter()
                            .any(|(link, tag)| peer.sending(*link) == Some(tag.as_slice()))
                        {
                            waiting.push(Action::Answer {
                                control,
                                delivery_id,
                                state,
                                after,
                            });
                        } else {
                            peer.answer(control, delivery_id, Some(&state));
                        }
                    }
                }
            }
        }
        self.actions = waiting;
    }

    /// The node a delivery on `link` goes to, when the partner may send
    /// to it.
    fn target_of(&self, link: &LinkInfo) -> Result<&str, AmqpError> {
        let Some(state) = self.links.get(&link.id) else {
            return Err(AmqpError::new(
                condition::INTERNAL_ERROR,
                "delivery on a link without a node",
            ));
        };
        // Spec §10.3.5 No-Bypass — the check before each transfer.
        if !check_access(self.cfg, &state.address, AccessOp::ReceiveSample) {
            self.cfg.metrics.on_unauthorized();
            return Err(AmqpError::new(
                condition::UNAUTHORIZED_ACCESS,
                format!(
                    "{} may not send to {}",
                    self.cfg.default_identity.subject_name, state.address
                ),
            ));
        }
        Ok(&state.address)
    }

    /// Part 2 §2.8.15 resource-limit-exceeded for a node.
    fn node_full(&self, address: &str) -> AmqpError {
        let limits = self.cfg.nodes.limits();
        AmqpError::new(
            condition::RESOURCE_LIMIT_EXCEEDED,
            format!(
                "node {address} holds {} messages or {} octets, its bounds",
                limits.max_messages, limits.max_octets
            ),
        )
    }

    /// A message the partner sent to a node (Part 3 §3.3): the node takes
    /// it, or the outcome says why not.
    fn post(&mut self, link: &LinkInfo, delivery: Delivery) -> Outcome {
        let address = match self.target_of(link) {
            Ok(address) => address,
            Err(error) => return Outcome::Rejected(Some(error)),
        };
        self.cfg.metrics.on_transfer_received();
        let message = Message::new(delivery.message_format, delivery.payload);
        match self.cfg.nodes.put(address, message) {
            Put::Stored => Outcome::Accepted,
            Put::NoNode => Outcome::Rejected(Some(AmqpError::new(
                condition::NOT_FOUND,
                format!("node {address} is gone"),
            ))),
            Put::Full => Outcome::Rejected(Some(self.node_full(address))),
        }
    }

    /// Part 4 §4.4.1: a message posted in a transaction. It is held with
    /// the transaction and reaches the node with the commit; the answer
    /// is the outcome that applies then, in a transactional-state of the
    /// same transaction.
    fn post_in_txn(
        &mut self,
        link: &LinkInfo,
        delivery: Delivery,
        txn: &TxnId,
    ) -> Result<Answer, AmqpError> {
        if let Err(error) = self.txns.check(txn) {
            // No transaction to associate an outcome with: the delivery is
            // rejected with the transaction error. One sent settled has no
            // outcome to carry it; the link is destroyed with it.
            return if delivery.settled {
                Err(error)
            } else {
                Ok(Answer::outcome(Outcome::Rejected(Some(error))))
            };
        }
        let settled = delivery.settled;
        let outcome = match self.hold(link, delivery, txn) {
            Ok(()) => Outcome::Accepted,
            Err(error) => {
                if settled {
                    // §4.4.4, a delivery sent settled: "If the resource is
                    // unable to process the delivery it MUST NOT allow the
                    // successful discharge of the associated transaction."
                    self.txns.doom(
                        txn,
                        format!(
                            "a delivery sent settled in the transaction was not taken: {error}"
                        ),
                    );
                }
                Outcome::Rejected(Some(error))
            }
        };
        Ok(Answer::State(Some(DeliveryState::Transactional(
            TransactionalState {
                txn_id: txn.clone(),
                outcome: Some(outcome),
            },
        ))))
    }

    /// Holds a posted message with its transaction.
    fn hold(&mut self, link: &LinkInfo, delivery: Delivery, txn: &TxnId) -> Result<(), AmqpError> {
        if delivery.state_differs {
            // §4.4.1: "if delivery is split across several transfer frames
            // then all frames MUST be explicitly associated with the same
            // transaction".
            return Err(AmqpError::new(
                condition::INVALID_FIELD,
                format!("a transfer frame of the delivery did not name transaction {txn}"),
            ));
        }
        let address = self.target_of(link)?.to_string();
        if !self.cfg.nodes.exists(&address) {
            return Err(AmqpError::new(
                condition::NOT_FOUND,
                format!("node {address} is gone"),
            ));
        }
        // What the transaction holds for a node is bounded like the node.
        let limits = self.cfg.nodes.limits();
        let (held, octets) = self
            .txns
            .work(txn)
            .iter()
            .filter_map(|w| match w {
                Work::Post {
                    address: a,
                    message,
                } if *a == address => Some(message.payload.len() as u64),
                _ => None,
            })
            .fold((0usize, 0u64), |(n, sum), len| {
                (n + 1, sum.saturating_add(len))
            });
        if held >= limits.max_messages as usize
            || octets.saturating_add(delivery.payload.len() as u64) > limits.max_octets
        {
            return Err(self.node_full(&address));
        }
        self.cfg.metrics.on_transfer_received();
        let message = Message::new(delivery.message_format, delivery.payload);
        self.txns.record(txn, Work::Post { address, message })
    }

    /// A message to the coordinator (Part 4 §4.2, §4.3).
    fn coordinate(
        &mut self,
        link: &LinkInfo,
        delivery: &Delivery,
        rejects: bool,
    ) -> Result<Answer, AmqpError> {
        if delivery.settled {
            // §4.2: "If the coordinator receives a transfer that has been
            // settled by the sender, it SHOULD detach with an
            // amqp:illegal-state error."
            return Err(AmqpError::new(
                condition::ILLEGAL_STATE,
                "a message to the coordinator was sent settled",
            ));
        }
        if matches!(delivery.state, Some(DeliveryState::Transactional(_))) {
            // §4.1: "No transactional work is allowed on the control
            // link."
            return Err(AmqpError::new(
                condition::NOT_ALLOWED,
                "transactional work on the control link",
            ));
        }
        // §4.2, §4.3: an error is the outcome rejected where the source of
        // the link supports it; else the link is detached with it.
        let refuse = |error: AmqpError| {
            if rejects {
                Ok(DeliveryState::Outcome(Outcome::Rejected(Some(error))))
            } else {
                Err(error)
            }
        };
        let mut after = Vec::new();
        let answer = match coordinator_request(&delivery.payload) {
            Err(e) => refuse(AmqpError::new(e.condition(), e.to_string())),
            Ok(CoordinatorRequest::Declare(declare)) => {
                if declare.global_id.is_some() {
                    // §4.5.2: "This field MUST NOT be set if the
                    // coordinator does not have the
                    // distributed-transactions capability."
                    refuse(AmqpError::new(
                        condition::NOT_IMPLEMENTED,
                        "declare.global-id: the coordinator has no amqp:distributed-transactions",
                    ))
                } else {
                    match self.txns.declare(link.id, self.now_ms) {
                        Ok(txn) => Ok(DeliveryState::Declared(txn)),
                        Err(error) => refuse(error),
                    }
                }
            }
            Ok(CoordinatorRequest::Discharge(discharge)) => {
                let txn = &discharge.txn_id;
                if self.txns.owner(txn) == Some(&link.id)
                    && self.partial.values().any(|posted| posted.contains(txn))
                {
                    // §4.4.1: "It is an error for the controller to
                    // attempt to discharge a transaction against which a
                    // partial delivery has been posted. If this happens,
                    // the control link MUST be terminated with the
                    // transaction-rollback error." The transactions of the
                    // link are rolled back before it goes (§4.4.3).
                    let error = TransactionError::Rollback.error(format!(
                        "discharge of transaction {txn} while a delivery posted to it is incomplete"
                    ));
                    return self.end_control(link.id, error);
                }
                match self.txns.discharge(&link.id, txn, discharge.fail) {
                    Discharged::Commit(work) => match self.commit(txn, work) {
                        Ok(()) => Ok(DeliveryState::Outcome(Outcome::Accepted)),
                        Err((reason, waiting)) => {
                            after = waiting;
                            refuse(TransactionError::Rollback.error(reason))
                        }
                    },
                    Discharged::Rollback(work) => {
                        after = self.rollback(txn, work, true);
                        Ok(DeliveryState::Outcome(Outcome::Accepted))
                    }
                    Discharged::Refused { error, work } => {
                        after = self.rollback(txn, work, true);
                        refuse(error)
                    }
                }
            }
        };
        let state = match answer {
            Ok(state) => state,
            Err(error) => {
                // Without the outcome rejected the link is detached with
                // the error; a failed discharge completes after the abort
                // of what the transaction acquired (§4.4.3: "If the
                // discharge indicates the failure of the transaction the
                // resource MUST abort or complete sending the remainder of
                // the message before completing the discharge").
                after.extend(self.roll_back_control(link.id));
                if after.is_empty() {
                    return Err(error);
                }
                self.actions.push(Action::Detach {
                    link: link.id,
                    error,
                    after,
                });
                return Ok(Answer::Later);
            }
        };
        if after.is_empty() {
            return Ok(Answer::State(Some(state)));
        }
        self.actions.push(Action::Answer {
            control: link.id,
            delivery_id: delivery.delivery_id,
            state,
            after,
        });
        Ok(Answer::Later)
    }

    /// The links that acquire in `txn` acquire in none from now on
    /// (Part 4 §4.4.3: "When a transaction is discharged, the txn-id of
    /// any link endpoints will be cleared").
    fn clear_acquiring(&mut self, txn: &TxnId) {
        for state in self.links.values_mut() {
            if state.txn.as_ref() == Some(txn) {
                state.txn = None;
            }
        }
    }

    /// Applies the work of a committed transaction (Part 4 §4.3, §4.4).
    /// The posted messages reach their nodes all or none; if a node can
    /// no longer take one, nothing is applied and the work is undone.
    ///
    /// # Errors
    /// Why the transaction was rolled back instead, and the deliveries
    /// whose abort the answer waits for.
    fn commit(&mut self, txn: &TxnId, work: Vec<Work>) -> Result<(), (String, Withdrawn)> {
        let mut posts = Vec::new();
        let mut rest = Vec::new();
        for w in work {
            match w {
                Work::Post { address, message } => posts.push((address, message)),
                other => rest.push(other),
            }
        }
        if let Err(reason) = self.cfg.nodes.put_all(posts) {
            // §4.4.4: "If the resource can no longer apply the outcome
            // that it originally indicated would be the result of a
            // successful discharge then it MUST NOT allow the successful
            // discharge".
            let waiting = self.rollback(txn, rest, true);
            return Err((reason, waiting));
        }
        for w in rest {
            match w {
                Work::Post { .. } => {}
                Work::Retire {
                    link,
                    tag,
                    outcome,
                    settled,
                } => {
                    let Some(state) = self.links.get_mut(&link) else {
                        continue;
                    };
                    let Some(inflight) = state.inflight.remove(&tag) else {
                        continue;
                    };
                    Self::apply(self.cfg, state, inflight.message, &outcome);
                    if !settled && !inflight.settled {
                        // §4.4.2: "On a successful discharge, the resource
                        // will apply the given outcome and can immediately
                        // settle the transfers."
                        self.actions.push(Action::Settle {
                            link,
                            tag,
                            state: DeliveryState::Outcome(outcome),
                        });
                    }
                }
                Work::Acquire { link, tag } => {
                    let Some(state) = self.links.get_mut(&link) else {
                        continue;
                    };
                    let sent_settled = match state.inflight.get_mut(&tag) {
                        Some(inflight) => {
                            inflight.acquired_in = None;
                            inflight.settled
                        }
                        None => false,
                    };
                    if sent_settled {
                        // §4.4.4, a delivery sent settled by the resource:
                        // "In the event of a successful discharge the
                        // outcome applies at the resource": it is gone
                        // from the node.
                        state.inflight.remove(&tag);
                    }
                }
            }
        }
        self.clear_acquiring(txn);
        Ok(())
    }

    /// Undoes the work of a transaction (Part 4 §4.3, §4.4.4): posted
    /// messages reach no node; outcomes are not applied, the deliveries
    /// stay with the link, and one the controller settled takes the
    /// default outcome of the source, released; acquired messages return
    /// to their nodes. `discharged`: the transaction is gone (not only
    /// timed out). Returns the deliveries of rolled-back acquisitions,
    /// which may be in the middle of their transfers.
    fn rollback(&mut self, txn: &TxnId, work: Vec<Work>, discharged: bool) -> Withdrawn {
        let mut withdrawn = Vec::new();
        // The latest work first: a message that goes back to the head of
        // its node last was acquired first.
        for w in work.into_iter().rev() {
            match w {
                Work::Post { .. } => {}
                Work::Retire {
                    link, tag, settled, ..
                } => {
                    let Some(state) = self.links.get_mut(&link) else {
                        continue;
                    };
                    if let Some(inflight) = state.inflight.get_mut(&tag) {
                        inflight.retiring_in = None;
                    }
                    if settled && let Some(inflight) = state.inflight.remove(&tag) {
                        // §4.4.4: "the controller can no longer influence
                        // the state of the delivery as it is settled, and
                        // the resource MUST apply the default outcome."
                        Self::apply(self.cfg, state, inflight.message, &Outcome::Released);
                    }
                }
                Work::Acquire { link, tag } => {
                    let Some(state) = self.links.get_mut(&link) else {
                        continue;
                    };
                    if let Some(inflight) = state.inflight.remove(&tag) {
                        self.cfg.nodes.put_back(&state.address, inflight.message);
                        self.actions.push(Action::Withdraw {
                            link,
                            tag: tag.clone(),
                            settled: inflight.settled,
                        });
                        withdrawn.push((link, tag));
                    }
                }
            }
        }
        if discharged {
            self.clear_acquiring(txn);
            self.partial.retain(|_, posted| {
                posted.retain(|t| t != txn);
                !posted.is_empty()
            });
        }
        withdrawn
    }

    /// An outcome acts on the node of the link (Part 3 §3.4).
    fn apply(cfg: &HandlerConfig, state: &LinkState, mut message: Message, outcome: &Outcome) {
        match outcome {
            // Part 3 §3.4.2, §3.4.3: the message leaves the node.
            Outcome::Accepted | Outcome::Rejected(_) | Outcome::None => {}
            // §3.4.4: back to the node, to be sent again.
            Outcome::Released => cfg.nodes.put_back(&state.address, message),
            // §3.4.5.
            Outcome::Modified {
                delivery_failed,
                undeliverable_here,
            } => {
                if *delivery_failed {
                    message.count_failed_delivery();
                }
                if *undeliverable_here {
                    message.undeliverable.insert(state.key.clone());
                }
                cfg.nodes.put_back(&state.address, message);
            }
        }
        cfg.metrics.on_settled();
    }

    /// Terminates the control link of `txn` with
    /// amqp:transaction:rollback (Part 4 §4.4.2).
    fn terminate_control_of(&mut self, txn: &TxnId, reason: String) {
        if let Some(control) = self.txns.owner(txn).copied() {
            let after = self.roll_back_control(control);
            self.actions.push(Action::Detach {
                link: control,
                error: TransactionError::Rollback.error(reason),
                after,
            });
        }
    }

    /// Rolls back every transaction the control link `control` declared
    /// (Part 4 §4.1), ahead of its end. Returns the deliveries of
    /// rolled-back acquisitions, which may be in the middle of their
    /// transfers.
    fn roll_back_control(&mut self, control: LinkId) -> Withdrawn {
        let mut after = Vec::new();
        for (txn, work) in self.txns.close(&control) {
            after.extend(self.rollback(&txn, work, true));
        }
        after
    }

    /// Ends the control link `control` with `error`: its transactions are
    /// rolled back first, and the detach goes once no delivery they
    /// acquired is in the middle of its transfers (Part 4 §4.4.3).
    fn end_control(&mut self, control: LinkId, error: AmqpError) -> Result<Answer, AmqpError> {
        let after = self.roll_back_control(control);
        if after.is_empty() {
            return Err(error);
        }
        self.actions.push(Action::Detach {
            link: control,
            error,
            after,
        });
        Ok(Answer::Later)
    }

    /// The outcome of a delivery this endpoint sent acts on the node
    /// (Part 3 §3.4).
    fn retire(&mut self, link: &LinkInfo, tag: &[u8], state: &Outcome, settled: bool) {
        // A delivery settled without a terminal state takes the default
        // outcome of the source, released (Part 3 §3.5.3).
        let outcome = match state {
            Outcome::None if settled => &Outcome::Released,
            Outcome::None => return,
            terminal => terminal,
        };
        let Some(link_state) = self.links.get_mut(&link.id) else {
            return;
        };
        let Some(inflight) = link_state.inflight.get(tag) else {
            return;
        };
        if let Some(txn) = inflight.acquired_in.clone() {
            // Part 4 §4.4.4, a delivery acquired in a transaction: "An
            // outcome sent by the controller before the transaction has
            // discharged MUST be associated with the same transaction."
            // The rollback that follows the end of the control link
            // returns the message to its node.
            self.terminate_control_of(
                &txn,
                format!("an outcome outside transaction {txn} for a delivery acquired in it"),
            );
            return;
        }
        if let Some(txn) = inflight.retiring_in.clone() {
            // A later word of the controller replaces the outcome it gave
            // in the transaction.
            if let Some(work) = self.txns.work_mut(&txn) {
                work.retain(|w| !matches!(w, Work::Retire { link: l, tag: t, .. } if *l == link.id && t == tag));
            }
        }
        let Some(inflight) = link_state.inflight.remove(tag) else {
            return;
        };
        Self::apply(self.cfg, link_state, inflight.message, outcome);
    }

    /// Part 4 §4.4.2: the outcome of a delivery this endpoint sent, in a
    /// transaction: recorded, applied with the commit.
    fn retire_in_txn(
        &mut self,
        link: &LinkInfo,
        tag: &[u8],
        transactional: &TransactionalState,
        settled: bool,
    ) -> Result<(), AmqpError> {
        let txn = &transactional.txn_id;
        self.txns.check(txn)?;
        let Some(inflight) = self
            .links
            .get_mut(&link.id)
            .and_then(|state| state.inflight.get_mut(tag))
        else {
            return Ok(());
        };
        // §4.4.2: "the delivery MUST NOT be associated with a different
        // non-discharged transaction than the outcome. If this happens
        // then the control link MUST be terminated with a
        // transaction-rollback error."
        let other = [inflight.acquired_in.as_ref(), inflight.retiring_in.as_ref()]
            .into_iter()
            .flatten()
            .find(|t| *t != txn)
            .cloned();
        if let Some(other) = other {
            self.terminate_control_of(
                txn,
                format!("an outcome in transaction {txn} for a delivery of transaction {other}"),
            );
            return Ok(());
        }
        let outcome = match (&transactional.outcome, settled) {
            (Some(outcome), _) => outcome.clone(),
            // Settled without an outcome: the default outcome of the
            // source (Part 3 §3.5.3).
            (None, true) => Outcome::Released,
            // The delivery is named with the transaction and no outcome:
            // there is none to apply.
            (None, false) => return Ok(()),
        };
        let replaces = inflight.retiring_in.replace(txn.clone()).is_some();
        if replaces && let Some(work) = self.txns.work_mut(txn) {
            work.retain(
                |w| !matches!(w, Work::Retire { link: l, tag: t, .. } if *l == link.id && t == tag),
            );
        }
        self.txns.record(
            txn,
            Work::Retire {
                link: link.id,
                tag: tag.to_vec(),
                outcome,
                settled,
            },
        )
    }

    /// Part 4 §4.2: the attach of a control link.
    fn attach_control(&mut self, link: &LinkInfo) -> Result<Attached, AmqpError> {
        if link.role != LinkRole::Receiver {
            // §4.5.1: the coordinator is the target the controller sends
            // to.
            return Err(AmqpError::new(
                condition::NOT_IMPLEMENTED,
                "a coordinator as the target of a link this endpoint sends on",
            ));
        }
        if !check_access(self.cfg, COORDINATOR_ADDRESS, AccessOp::AttachReceiver) {
            self.cfg.metrics.on_unauthorized();
            return Err(AmqpError::new(
                condition::UNAUTHORIZED_ACCESS,
                format!(
                    "{} may not attach to the coordinator",
                    self.cfg.default_identity.subject_name
                ),
            ));
        }
        if self.cfg.transactions.max_transactions == 0 {
            return Err(AmqpError::new(
                condition::NOT_IMPLEMENTED,
                "the configuration admits no transactions",
            ));
        }
        // OUTCOME_SYMBOLS[1] is amqp:rejected:list.
        let rejects = link
            .source
            .as_ref()
            .is_some_and(|s| s.outcomes.iter().any(|o| o == OUTCOME_SYMBOLS[1]));
        self.controls.insert(link.id, Control { rejects });
        Ok(Attached {
            source: link.source.clone(),
            target: None,
            coordinator: Some(Self::coordinator()),
        })
    }
}

impl Container for ConnectionNodes<'_> {
    fn attach(&mut self, link: &LinkInfo) -> Result<Attached, AmqpError> {
        if link.coordinator.is_some() {
            return self.attach_control(link);
        }
        // The terminus at this endpoint: the target of a link the peer
        // sends on, the source of one it receives on.
        let (requested, op) = match link.role {
            LinkRole::Receiver => (
                link.target.as_ref().map(|t| &t.terminus),
                AccessOp::AttachReceiver,
            ),
            LinkRole::Sender => (
                link.source.as_ref().map(|s| &s.terminus),
                AccessOp::AttachSender,
            ),
        };
        let named = requested.and_then(|t| t.address.clone());
        // Spec §10.3.3 / §10.3.5 — the access check before the attach.
        let checked = named.as_deref().unwrap_or("<unknown>");
        if !check_access(self.cfg, checked, op) {
            self.cfg.metrics.on_unauthorized();
            return Err(AmqpError::new(
                condition::UNAUTHORIZED_ACCESS,
                format!(
                    "{} may not attach to {checked}",
                    self.cfg.default_identity.subject_name
                ),
            ));
        }
        let Some(requested) = requested else {
            return Err(if link.target_value.is_some() {
                AmqpError::new(
                    condition::NOT_IMPLEMENTED,
                    "the target is neither an amqp:target:list nor an amqp:coordinator:list",
                )
            } else {
                AmqpError::new(
                    condition::INVALID_FIELD,
                    "attach without the terminus at this endpoint",
                )
            });
        };
        let (address, dynamic) = if requested.dynamic {
            if requested.address.is_some() {
                // Part 3 §3.5.3, §3.5.4 dynamic: "In this case the address
                // field MUST NOT be set."
                return Err(AmqpError::new(
                    condition::INVALID_FIELD,
                    "a dynamic terminus with an address",
                ));
            }
            let Some(address) = self
                .cfg
                .nodes
                .create_dynamic(&link.remote_container, &link.name)
            else {
                return Err(self.no_room_for_a_node());
            };
            (address, true)
        } else {
            let Some(address) = named else {
                return Err(AmqpError::new(
                    condition::NOT_IMPLEMENTED,
                    "a terminus without an address is not offered",
                ));
            };
            match self.cfg.nodes.resolve(&address) {
                Resolved::Node => {}
                Resolved::NotFound => {
                    return Err(AmqpError::new(
                        condition::NOT_FOUND,
                        format!("no node {address}"),
                    ));
                }
                Resolved::Full => return Err(self.no_room_for_a_node()),
            }
            (address, false)
        };
        // The terminus as created (Part 2 §2.6.3): no state is kept past
        // the link, so durable none and expiry at the detach; a dynamic
        // node lives as long as its link (Part 3 §3.5.10).
        let terminus = Terminus {
            address: Some(address.clone()),
            durable: 0,
            expiry_policy: ExpiryPolicy::LinkDetach,
            timeout: 0,
            dynamic,
            dynamic_node_properties: if dynamic {
                vec![(
                    "lifetime-policy".to_string(),
                    AmqpExtValue::Described {
                        descriptor: DELETE_ON_CLOSE,
                        value: Box::new(AmqpExtValue::List(Vec::new())),
                    },
                )]
            } else {
                Vec::new()
            },
            capabilities: Vec::new(),
        };
        self.links.insert(
            link.id,
            LinkState {
                address,
                key: format!("{}/{}", link.remote_container, link.name),
                dynamic,
                inflight: BTreeMap::new(),
                txn: None,
            },
        );
        Ok(Attached::from(match link.role {
            LinkRole::Receiver => (link.source.clone(), Some(Target { terminus })),
            LinkRole::Sender => (
                Some(Source {
                    terminus,
                    // Part 3 §3.5.2: each message goes to one link.
                    distribution_mode: Some("move".to_string()),
                    // No filter is in place, whatever the peer asked for
                    // (§3.5.3: "the sending endpoint sets the filter
                    // actually in place").
                    filter: Vec::new(),
                    default_outcome: Some(Outcome::Released),
                    outcomes: OUTCOME_SYMBOLS.iter().map(|s| (*s).to_string()).collect(),
                }),
                link.target.clone(),
            ),
        }))
    }

    fn deliver(&mut self, link: &LinkInfo, delivery: Delivery) -> Result<Answer, AmqpError> {
        if let Some(control) = self.controls.get(&link.id) {
            let rejects = control.rejects;
            return self.coordinate(link, &delivery, rejects);
        }
        self.partial.remove(&(link.id, delivery.delivery_id));
        if delivery.state_differs {
            // §4.4.1: "if delivery is split across several transfer frames
            // then all frames MUST be explicitly associated with the same
            // transaction". A transaction only some frames named does not
            // get the delivery; the controller counted it as work of that
            // transaction, so the transaction does not commit (§4.4.4).
            let first = match &delivery.state {
                Some(DeliveryState::Transactional(t)) => Some(t.txn_id.clone()),
                _ => None,
            };
            for txn in delivery.txns.iter().filter(|t| Some(*t) != first.as_ref()) {
                self.txns.doom(
                    txn,
                    format!(
                        "a delivery on link {} named transaction {txn} on some of its transfer \
                         frames only",
                        link.name
                    ),
                );
            }
            if first.is_none() {
                // Not posted: the first frame named no transaction, a
                // later one did.
                let error = AmqpError::new(
                    condition::INVALID_FIELD,
                    "a transfer frame of the delivery named a transaction the first did not",
                );
                return if delivery.settled {
                    // No outcome carries the error.
                    Err(error)
                } else {
                    Ok(Answer::outcome(Outcome::Rejected(Some(error))))
                };
            }
        }
        match delivery.state.clone() {
            Some(DeliveryState::Transactional(transactional)) => {
                self.post_in_txn(link, delivery, &transactional.txn_id)
            }
            _ => Ok(Answer::outcome(self.post(link, delivery))),
        }
    }

    fn disposition(
        &mut self,
        link: &LinkInfo,
        tag: &[u8],
        state: Option<&DeliveryState>,
        settled: bool,
    ) -> Result<(), AmqpError> {
        match state {
            Some(DeliveryState::Transactional(transactional)) => {
                self.retire_in_txn(link, tag, transactional, settled)
            }
            other => {
                let outcome = other.map_or(Outcome::None, DeliveryState::outcome);
                self.retire(link, tag, &outcome, settled);
                Ok(())
            }
        }
    }

    fn acquire_in(&mut self, link: &LinkInfo, txn: Option<&TxnId>) -> Result<(), AmqpError> {
        // Part 4 §4.4.3: the transaction the messages this link sends
        // from now on are acquired in.
        if let Some(txn) = txn {
            self.txns.check(txn)?;
        }
        if let Some(state) = self.links.get_mut(&link.id) {
            state.txn = txn.cloned();
        }
        Ok(())
    }

    fn delivery_begun(&mut self, link: &LinkInfo, delivery_id: u32, state: Option<&DeliveryState>) {
        if let Some(DeliveryState::Transactional(transactional)) = state {
            self.partial
                .insert((link.id, delivery_id), vec![transactional.txn_id.clone()]);
        }
    }

    fn delivery_named(&mut self, link: &LinkInfo, delivery_id: u32, txn: &TxnId) {
        // §4.4.1: a later frame posted the delivery against `txn` as well;
        // until its last frame it is a partial delivery of `txn`.
        let posted = self.partial.entry((link.id, delivery_id)).or_default();
        if !posted.contains(txn) {
            posted.push(txn.clone());
        }
    }

    fn delivery_aborted(&mut self, link: &LinkInfo, delivery_id: u32) {
        self.partial.remove(&(link.id, delivery_id));
    }

    fn available(&mut self, link: &LinkInfo) -> u32 {
        // Part 2 §2.7.4: the messages of the node this link may get.
        self.links.get(&link.id).map_or(0, |state| {
            u32::try_from(self.cfg.nodes.available(&state.address, &state.key)).unwrap_or(u32::MAX)
        })
    }

    fn detached(&mut self, link: &LinkInfo, _unsettled: Vec<Vec<u8>>, _error: Option<&AmqpError>) {
        if self.controls.remove(&link.id).is_some() {
            // Part 4 §4.1: "If the control link is closed while there
            // exist non-discharged transactions it created, then all such
            // transactions are immediately rolled back".
            for (txn, work) in self.txns.close(&link.id) {
                self.rollback(&txn, work, true);
            }
            return;
        }
        self.partial.retain(|(id, _), _| *id != link.id);
        let Some(state) = self.links.remove(&link.id) else {
            return;
        };
        // The default outcome of the source for what the link leaves
        // unsettled: released. The oldest goes back last, to the head.
        for (_, inflight) in state.inflight.into_iter().rev() {
            // Part 4 §4.4.4: an outcome or an acquisition of a transaction
            // that can no longer be applied: the transaction does not
            // commit.
            for txn in [inflight.acquired_in, inflight.retiring_in]
                .into_iter()
                .flatten()
            {
                self.txns.doom(
                    &txn,
                    format!(
                        "link {} was detached with a delivery of the transaction unsettled",
                        link.name
                    ),
                );
            }
            self.cfg.nodes.put_back(&state.address, inflight.message);
        }
        if state.dynamic {
            self.cfg.nodes.delete(&state.address);
        }
    }
}

/// `amqp:delete-on-close:list` (Part 3 §3.5.10).
const DELETE_ON_CLOSE: u64 = 0x2b;

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
    impl Transport for DuplexCursor {
        fn set_read_timeout(&mut self, _: Option<Duration>) -> std::io::Result<()> {
            Ok(())
        }
        fn close_outgoing(&mut self) -> std::io::Result<()> {
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
    fn ddsamqp_2_1_handle_connection_open_close_round_trip() {
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
        // §2.2: answered with the supported header.
        assert_eq!(io.output, b"AMQP\x00\x01\x00\x00");
    }

    /// §2.2: a connection that ends on a refused header leaves the gauge of
    /// active connections where it was.
    #[test]
    fn amqp10_2_2_a_refused_header_closes_the_connection_in_the_metrics() {
        let metrics = Arc::new(MetricsHub::new());
        let cfg = HandlerConfig::for_tests(metrics.clone());
        let mut io = duplex(b"AMQP\x00\x02\x00\x00".to_vec());
        assert!(handle_connection(&mut io, &cfg).is_err());
        assert_eq!(metrics.snapshot("connections.active"), Some(0));
        assert_eq!(metrics.snapshot("connections.total"), Some(1));
    }

    #[test]
    fn ddsamqp_2_1_handle_connection_sasl_then_amqp() {
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

    /// AMQP 1.0 §5.3.3.1: "It is invalid for this list to be null or
    /// empty." A connection on which no configured mechanism works (PLAIN
    /// alone, without TLS) gets the SASL header and no sasl-mechanisms
    /// frame.
    #[test]
    fn amqp10_5_3_3_1_no_empty_list_of_mechanisms_is_sent() {
        let mut cfg = cfg();
        cfg.sasl.enabled = vec![zerodds_amqp_endpoint::sasl::SaslMechanism::Plain];
        cfg.tls_active = false;
        let mut io = duplex(AmqpProtocol::Sasl.as_bytes().to_vec());
        let err = handle_connection(&mut io, &cfg).unwrap_err();
        assert!(matches!(err, HandlerError::Sasl(_)), "{err}");
        assert_eq!(io.output, AmqpProtocol::Sasl.as_bytes());
    }

    #[test]
    fn ddsamqp_2_1_access_control_deny_attach_yields_unauthorized_metric() {
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

        // Begin, then attach (Sender, Handle 0).
        let begin = performatives::begin(None, 0, 100, 100).unwrap();
        let h = zerodds_amqp_bridge::frame::FrameHeader {
            size: 8 + begin.len() as u32,
            doff: 2,
            frame_type: FrameType::Amqp,
            channel: 0,
        };
        input.extend(zerodds_amqp_bridge::frame::encode_frame_header(h));
        input.extend(&begin);
        // The client as the receiver of the link: an attach without a
        // source, which the access check sees as `<unknown>`.
        let attach = performatives::attach("L", 0, false).unwrap();
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
    fn ddsamqp_2_1_access_control_allow_does_not_increment_unauthorized() {
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
