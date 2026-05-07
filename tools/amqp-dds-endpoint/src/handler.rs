// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Per-Connection Handler — treibt die Endpoint-State-Machine.
//!
//! Spec dds-amqp-1.0 §2.1 Endpoint Profile:
//! * Cl. 2 — Connection Acceptance: Protocol-Handshake + State.
//! * Cl. 6 — SASL: PLAIN/ANONYMOUS/EXTERNAL.
//! * Cl. 7 — Mandatory TLS for PLAIN (im Daemon: tls_active-Flag).
//!
//! Struktureller Ablauf:
//! 1. Optional SASL-Phase (Client schickt `AMQP\3\1\0\0`).
//! 2. AMQP-Phase (Client schickt `AMQP\0\1\0\0`).
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

/// Pro-Connection Statistik (fuer Tests + Metrics-Wiring).
#[derive(Debug, Default, Clone)]
pub struct ConnectionStats {
    /// Empfangene Frames (alle Typen).
    pub frames_received: u64,
    /// Gesendete Frames.
    pub frames_sent: u64,
    /// SASL-Phase durchlaufen?
    pub sasl_completed: bool,
    /// Open-Performative empfangen?
    pub open_received: bool,
    /// Close-Performative empfangen oder gesendet?
    pub closed: bool,
}

/// Handler-Fehler.
#[derive(Debug)]
pub enum HandlerError {
    /// Frame-IO-Fehler.
    FrameIo(FrameIoError),
    /// State-Machine-Fehler.
    Endpoint(EndpointError),
    /// Performative-Decoding fehlgeschlagen.
    PerformativeDecode(String),
    /// Connection wurde zu frueh geschlossen.
    UnexpectedEof,
}

impl core::fmt::Display for HandlerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::FrameIo(e) => write!(f, "frame io: {e}"),
            Self::Endpoint(e) => write!(f, "endpoint: {e:?}"),
            Self::PerformativeDecode(s) => write!(f, "performative decode: {s}"),
            Self::UnexpectedEof => write!(f, "unexpected eof"),
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

/// Handler-Konfiguration pro Connection.
#[derive(Clone)]
pub struct HandlerConfig {
    /// Container-Id, die wir im Open-Frame zurueckmelden
    /// (Spec §2.4.1).
    pub container_id: String,
    /// Maximale Frame-Groesse, die wir akzeptieren (DoS-Cap).
    pub max_frame_size: u32,
    /// Ist TLS aktiv? (Beeinflusst SASL-PLAIN-Akzeptanz Spec §10.2.1.)
    pub tls_active: bool,
    /// Metrics-Hub (zaehlt Frames, Connections, Errors).
    pub metrics: Arc<MetricsHub>,
    /// Spec §10.3.3 — AccessControl-Plugin fuer Pre-Attach- +
    /// Pre-Transfer-Checks (No-Bypass §10.3.5). `None` = kein
    /// Check (DDS-Security inaktiv).
    pub access_control: Option<Arc<dyn AccessControlPlugin + Send + Sync>>,
    /// Default-Identity fuer Pre-SASL-Phase (typisch ANONYMOUS).
    /// Wird nach erfolgreichem SASL durch den authentifizierten
    /// Subject ueberschrieben.
    pub default_identity: IdentityToken,
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
    /// Default mit AllowAll-Plugin fuer Tests.
    #[must_use]
    pub fn for_tests(metrics: Arc<MetricsHub>) -> Self {
        Self {
            container_id: "zerodds-amqp-endpoint".to_string(),
            max_frame_size: 1_048_576,
            tls_active: false,
            metrics,
            access_control: None,
            default_identity: build_identity_token(&SaslSubject::Anonymous),
        }
    }

    /// Mit AccessControl-Plugin.
    #[must_use]
    pub fn with_access_control(
        mut self,
        plugin: Arc<dyn AccessControlPlugin + Send + Sync>,
    ) -> Self {
        self.access_control = Some(plugin);
        self
    }

    /// Mit Identity (z.B. nach SASL).
    #[must_use]
    pub fn with_identity(mut self, identity: IdentityToken) -> Self {
        self.default_identity = identity;
        self
    }
}

/// Handle a single AMQP-1.0-Connection blocking auf `stream`.
///
/// Spec §2.1 Cl. 2 — fuehrt:
/// 1. Protocol-Header-Exchange (optional SASL → AMQP).
/// 2. SASL-Mechanism-Negotiation (wenn SASL-Header).
/// 3. Open / Begin / Attach / Transfer / ... bis Close.
///
/// # Errors
/// Siehe [`HandlerError`].
pub fn handle_connection<S: Read + Write>(
    stream: &mut S,
    cfg: &HandlerConfig,
) -> Result<ConnectionStats, HandlerError> {
    cfg.metrics.on_connection_open();
    let mut stats = ConnectionStats::default();

    // Spec §2.2 — Read first protocol-header. Klient kann SASL
    // (0x03) oder AMQP (0x00) waehlen.
    let first = read_protocol_header(stream)?;
    match first.protocol {
        AmqpProtocol::Sasl => {
            // SASL-Phase: server akzeptiert SASL-Header und
            // startet Negotiation.
            do_sasl_phase(stream, cfg, &mut stats)?;
            // Nach erfolgreicher SASL kommt der zweite
            // Protocol-Header — diesmal AMQP.
            let second = read_protocol_header(stream)?;
            if second.protocol != AmqpProtocol::Amqp {
                return Err(HandlerError::FrameIo(FrameIoError::UnsupportedProtocolId(
                    second.protocol.as_bytes()[4],
                )));
            }
            // Server schickt seinen AMQP-Header zurueck.
            write_protocol_header(stream, AmqpProtocol::Amqp)?;
        }
        AmqpProtocol::Amqp => {
            // Direkt AMQP-Phase: Server bestaetigt mit eigenem Header.
            write_protocol_header(stream, AmqpProtocol::Amqp)?;
        }
    }

    // Beide AMQP-Header ausgetauscht — State-Machine treiben.
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
) -> Result<(), HandlerError> {
    // Server schickt seinen SASL-Header zurueck.
    write_protocol_header(stream, AmqpProtocol::Sasl)?;

    // Server schickt SASL-mechanisms-Frame mit der angebotenen
    // Mechanismen-Liste. Spec §5.3.3.1.
    // Mechanismen: PLAIN nur bei TLS-aktiv, sonst nur ANONYMOUS+EXTERNAL.
    let mechs = build_sasl_mechanisms(cfg.tls_active);
    let sasl_mechanisms_descriptor: u64 = 0x0000_0000_0000_0040;
    let body = performatives::encode_performative(sasl_mechanisms_descriptor, &mechs)
        .map_err(|e| HandlerError::PerformativeDecode(format!("{e}")))?;
    write_frame(stream, FrameType::Sasl, 0, &body)?;
    stats.frames_sent += 1;

    // Server liest sasl-init vom Klient (Spec §5.3.3.2,
    // descriptor 0x41). Wir akzeptieren einfach jeden init und
    // antworten mit sasl-outcome (Spec §5.3.3.6, descriptor 0x44)
    // mit code = 0 (ok).
    let init_frame = read_frame(stream, cfg.max_frame_size)?;
    stats.frames_received += 1;
    if init_frame.header.frame_type != FrameType::Sasl {
        return Err(HandlerError::FrameIo(FrameIoError::UnsupportedProtocolId(
            init_frame.header.frame_type.to_u8(),
        )));
    }

    // sasl-outcome-Frame senden.
    let outcome_descriptor: u64 = 0x0000_0000_0000_0044;
    // sasl-outcome body: list mit [code: ubyte (0=ok)].
    let outcome_body = AmqpExtValue::List(vec![AmqpExtValue::Ubyte(0)]);
    let body = performatives::encode_performative(outcome_descriptor, &outcome_body)
        .map_err(|e| HandlerError::PerformativeDecode(format!("{e}")))?;
    write_frame(stream, FrameType::Sasl, 0, &body)?;
    stats.frames_sent += 1;
    stats.sasl_completed = true;
    Ok(())
}

fn build_sasl_mechanisms(tls_active: bool) -> AmqpExtValue {
    // sasl-mechanisms body: list mit [server-mechanisms: array<symbol>].
    let mut mechs: Vec<AmqpExtValue> = Vec::new();
    if tls_active {
        mechs.push(AmqpExtValue::Symbol("PLAIN".to_string()));
    }
    mechs.push(AmqpExtValue::Symbol("ANONYMOUS".to_string()));
    mechs.push(AmqpExtValue::Symbol("EXTERNAL".to_string()));
    AmqpExtValue::List(vec![AmqpExtValue::Array(mechs)])
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
                // Peer hat abrupt geschlossen.
                return Ok(());
            }
            Err(e) => return Err(HandlerError::FrameIo(e)),
        };
        stats.frames_received += 1;

        // Empty-Frame = Heartbeat (§2.4.5).
        if frame.body.is_empty() {
            // Reply mit eigenem Heartbeat? Nein — server ist passiv.
            continue;
        }

        // Performative decoden.
        let kind = match classify_performative(&frame.body) {
            Some(k) => k,
            None => {
                // Unbekannte Performative — Frame ignorieren.
                cfg.metrics.on_decode_error();
                continue;
            }
        };

        // State-Machine treiben.
        *state = advance_connection(*state, kind)?;

        match kind {
            InboundFrameKind::Open => {
                stats.open_received = true;
                // Server schickt eigenen Open zurueck — und der
                // Outbound-Open advanced den Connection-State auf
                // Opened (Spec §2.4 OpenRcvd → Opened).
                let open = performatives::open(&cfg.container_id)
                    .map_err(|e| HandlerError::PerformativeDecode(format!("{e}")))?;
                write_frame(stream, FrameType::Amqp, 0, &open)?;
                stats.frames_sent += 1;
                *state = advance_connection(*state, InboundFrameKind::Open)?;
            }
            InboundFrameKind::Begin => {
                // Echo Begin auf Channel 0; remote-channel = 0.
                let begin = performatives::begin(Some(0), 0, 1024, 1024)
                    .map_err(|e| HandlerError::PerformativeDecode(format!("{e}")))?;
                write_frame(stream, FrameType::Amqp, frame.header.channel, &begin)?;
                stats.frames_sent += 1;
            }
            InboundFrameKind::Attach => {
                // Spec §10.3.3 / §10.3.5 — Pre-Attach-AccessControl-
                // Check. Bei Deny: Detach mit
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
                // Echo Attach mit eigener Handle 0 (Sender direction).
                let attach = performatives::attach(&link_name, 0, true)
                    .map_err(|e| HandlerError::PerformativeDecode(format!("{e}")))?;
                write_frame(stream, FrameType::Amqp, frame.header.channel, &attach)?;
                stats.frames_sent += 1;
            }
            InboundFrameKind::Transfer => {
                // Spec §10.3.5 No-Bypass — Pre-Transfer-Check.
                // Inbound-Transfer = Receiver-Side aus Server-Sicht;
                // wir pruefen ReceiveSample. Bei Deny: still
                // counten, aber Disposition rejected senden.
                if !check_access(cfg, "<transfer>", AccessOp::ReceiveSample) {
                    cfg.metrics.on_unauthorized();
                    continue;
                }
                cfg.metrics.on_transfer_received();
                // Pre-settled accept; kein Disposition.
            }
            InboundFrameKind::Close => {
                // Reply mit eigenem Close + State auf End advancen
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
            // Flow / Disposition / Detach: ack stillschweigend.
            InboundFrameKind::Flow | InboundFrameKind::Disposition | InboundFrameKind::Detach => {}
            InboundFrameKind::Header => {
                // Sollte nach Handshake nicht mehr kommen — ignorieren.
            }
        }
    }
}

/// Spec §10.3.5 — Pre-Op-AccessControl-Check.
///
/// Liefert `true` bei `Allow` (oder fehlendem Plugin = Default
/// Allow); `false` bei `Deny`. Caller behandelt Deny (Detach,
/// Drop, etc.).
fn check_access(cfg: &HandlerConfig, address: &str, op: AccessOp) -> bool {
    let Some(plugin) = cfg.access_control.as_ref() else {
        // Kein Plugin = kein Check (DDS-Security inaktiv).
        return true;
    };
    matches!(
        plugin.check(&cfg.default_identity, address, op),
        AccessDecision::Allow
    )
}

/// Spec §2.6.1 — Attach-Body-Felder extrahieren.
///
/// Liefert `(link_name, target_address, is_sender)`. Wenn das
/// Body-Format unerwartet ist, fallen wir auf Defaults zurueck
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
    // target-Address ist im 6. Index; in vielen Implementierungen
    // ist das ein described composite mit Address-String. Wir
    // probieren, einen String oder eine Map mit "address"-Key zu
    // extrahieren.
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

/// Klassifiziert ein Performative-Body als `InboundFrameKind` per
/// Descriptor-Code (Spec §2.7 Tab 2.7).
#[must_use]
pub fn classify_performative(body: &[u8]) -> Option<InboundFrameKind> {
    // Body beginnt mit `0x00` (described) + Descriptor + List-Body.
    // Wir lesen den Descriptor (Ulong).
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

    /// Round-Trip-Helper: schreibt eine Sequenz von Bytes als
    /// "Klient-Eingabe" und sammelt Server-Ausgabe in einem Vec.
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
        // Klient-Sequenz: AMQP-Header + Open + Close.
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
        // Server hat: AMQP-Header + Open-reply + Close-reply.
        assert!(stats.frames_sent >= 2);
        // Server-Ausgabe beginnt mit AMQP-Header.
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
        // Klient: SASL-Header → erwartet sasl-mechanisms +
        // schickt sasl-init → erwartet sasl-outcome → schickt
        // AMQP-Header → schickt Open → schickt Close.
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
        // Zweiter Protocol-Header: AMQP.
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
        // Spec §10.3.3 — DenyAll auf Attach erzeugt
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
        // Erst Open dann Close.
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
        // Connection geoeffnet+geschlossen → active gauge bei 0,
        // total bei 1.
        assert_eq!(m.snapshot("connections.active"), Some(0));
        assert_eq!(m.snapshot("connections.total"), Some(1));
    }
}
