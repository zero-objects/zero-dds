// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Outbound Bridge-Client + Reconnect-Loop.
//!
//! Spec dds-amqp-1.0:
//! * §2.2 Bridge Profile Cl. 2 — Outbound-Connection zu einem
//!   konfigurierten upstream Broker.
//! * §2.2 Cl. 4 — SASL-Outbound (Initiator-Side).
//! * §2.2 Cl. 5 — SHALL NOT initiate PLAIN ueber unverschluesselten
//!   Transport.
//! * §2.2 Cl. 7 — Exponential Backoff-Reconnect.
//! * §10.8 Reconnect Behaviour — KEEP_LAST-Eviction-Counter via
//!   `transfers.dropped.reconnect-overflow`.

use std::io::{self, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use zerodds_amqp_bridge::extended_types::AmqpExtValue;
use zerodds_amqp_bridge::frame::FrameType;
use zerodds_amqp_bridge::performatives;
use zerodds_amqp_endpoint::sasl::{SaslMechanism, SaslState};
use zerodds_amqp_endpoint::{ConnectionState, MetricsHub};

use crate::frame_io::{
    AmqpProtocol, FrameIoError, read_frame, read_protocol_header, write_frame,
    write_protocol_header,
};
use crate::handler::HandlerError;

/// Spec §2.2 Cl. 7 — Default-Backoff-Pacing.
pub const DEFAULT_BACKOFF_INITIAL_MS: u64 = 1_000;
/// Spec §2.2 Cl. 7 — Backoff-Multiplikator.
pub const DEFAULT_BACKOFF_MULT: u64 = 2;
/// Spec §2.2 Cl. 7 — Backoff-Cap.
pub const DEFAULT_BACKOFF_CAP_MS: u64 = 60_000;

/// Spec §2.2 — Outbound-Bridge-Konfiguration.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// Upstream-Adresse (z.B. `"broker.example:5672"`).
    pub upstream_addr: String,
    /// Container-Id, die wir im Open senden.
    pub container_id: String,
    /// Max-Frame-Size (DoS-Cap).
    pub max_frame_size: u32,
    /// Ist TLS aktiv? (Beeinflusst PLAIN-Auswahl Spec §10.2.1.)
    pub tls_active: bool,
    /// SASL-Credential fuer PLAIN (Username, Password). Falls
    /// `None` und PLAIN trotzdem ausgewaehlt wird, faellt
    /// `select_outbound` auf ANONYMOUS zurueck.
    pub plain_credentials: Option<(String, String)>,
    /// Read-/Write-Timeout pro Connection.
    pub io_timeout: Option<Duration>,
}

/// Spec §2.2 Cl. 7 + §10.8 — Reconnect-Konfiguration.
#[derive(Debug, Clone)]
pub struct ReconnectConfig {
    /// Initialer Backoff (Default 1s).
    pub initial_ms: u64,
    /// Multiplikator (Default 2).
    pub multiplier: u64,
    /// Backoff-Cap (Default 60s).
    pub cap_ms: u64,
    /// Maximale Reconnect-Versuche; `None` = unbegrenzt.
    pub max_attempts: Option<u32>,
}

impl Default for ReconnectConfig {
    fn default() -> Self {
        Self {
            initial_ms: DEFAULT_BACKOFF_INITIAL_MS,
            multiplier: DEFAULT_BACKOFF_MULT,
            cap_ms: DEFAULT_BACKOFF_CAP_MS,
            max_attempts: None,
        }
    }
}

impl ReconnectConfig {
    /// Spec §2.2 Cl. 7 — naechster Backoff-Wert; sub-millisekunden-
    /// genau aber gecapped.
    #[must_use]
    pub fn next_backoff_ms(&self, attempt: u32) -> u64 {
        if attempt == 0 {
            return self.initial_ms;
        }
        // initial * mult^attempt, gecapped.
        let mut v = self.initial_ms;
        for _ in 0..attempt {
            v = v.saturating_mul(self.multiplier);
            if v >= self.cap_ms {
                return self.cap_ms;
            }
        }
        v
    }
}

/// Outbound-Verbindungs-Resultat.
#[derive(Debug, Clone)]
pub struct OutboundSession {
    /// Connection-State (sollte `Opened` sein nach erfolgreichem
    /// Handshake).
    pub state: ConnectionState,
    /// Container-Id, die der Broker im Open zurueckmeldete.
    pub remote_container_id: Option<String>,
    /// SASL-Mechanismus, mit dem authentifiziert wurde.
    pub sasl_mechanism: Option<SaslMechanism>,
}

/// Outbound-Fehler.
#[derive(Debug)]
pub enum ClientError {
    /// TCP-/IO-Fehler.
    Io(io::Error),
    /// Frame-IO-Fehler.
    FrameIo(FrameIoError),
    /// Handler-Fehler aus der State-Machine.
    Handler(HandlerError),
    /// Broker-seitiger Reject (SASL outcome != ok, oder Close
    /// vor Open).
    BrokerReject(String),
    /// Spec §2.2 Cl. 5 — PLAIN ueber unverschluesseltem Transport
    /// abgelehnt.
    PlainRejectedNoTls,
    /// Broker bot keinen akzeptablen SASL-Mechanismus an.
    NoAcceptableSaslMechanism,
    /// Reconnect-Loop hat `max_attempts` ueberschritten.
    ReconnectExhausted(u32),
}

impl core::fmt::Display for ClientError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "io error: {e}"),
            Self::FrameIo(e) => write!(f, "frame io: {e}"),
            Self::Handler(e) => write!(f, "handler: {e}"),
            Self::BrokerReject(s) => write!(f, "broker reject: {s}"),
            Self::PlainRejectedNoTls => write!(
                f,
                "SASL PLAIN refused over unencrypted transport (Spec §2.2 Cl. 5)"
            ),
            Self::NoAcceptableSaslMechanism => write!(f, "no acceptable SASL mechanism offered"),
            Self::ReconnectExhausted(n) => {
                write!(f, "reconnect attempts exhausted after {n} tries")
            }
        }
    }
}

impl std::error::Error for ClientError {}

impl From<io::Error> for ClientError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<FrameIoError> for ClientError {
    fn from(e: FrameIoError) -> Self {
        Self::FrameIo(e)
    }
}
impl From<HandlerError> for ClientError {
    fn from(e: HandlerError) -> Self {
        Self::Handler(e)
    }
}

/// Spec §2.2 Cl. 2 — einmaliger Outbound-Connect.
///
/// Liefert einen connecteten `TcpStream` + `OutboundSession` nach
/// erfolgreichem AMQP-Open-Handshake. SASL-Phase ist symmetrisch
/// zur Server-Seite, nur dass wir der Initiator sind.
///
/// # Errors
/// Siehe [`ClientError`].
pub fn connect_outbound(cfg: &ClientConfig) -> Result<(TcpStream, OutboundSession), ClientError> {
    let mut stream = tcp_connect(&cfg.upstream_addr)?;
    if let Some(t) = cfg.io_timeout {
        stream.set_read_timeout(Some(t))?;
        stream.set_write_timeout(Some(t))?;
    }
    let session = drive_outbound_handshake(&mut stream, cfg)?;
    Ok((stream, session))
}

fn tcp_connect(addr: &str) -> io::Result<TcpStream> {
    // Falls mehrere AddrInfos (DNS-Round-Robin), nehmen wir den
    // ersten, der akzeptiert.
    let addrs: Vec<_> = addr.to_socket_addrs()?.collect();
    let mut last_err: Option<io::Error> = None;
    for a in addrs {
        match TcpStream::connect_timeout(&a, Duration::from_secs(10)) {
            Ok(s) => return Ok(s),
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.unwrap_or_else(|| {
        io::Error::new(io::ErrorKind::AddrNotAvailable, "no resolvable address")
    }))
}

fn drive_outbound_handshake<S: Read + Write>(
    stream: &mut S,
    cfg: &ClientConfig,
) -> Result<OutboundSession, ClientError> {
    // Spec §2.2 — Senden SASL-Header zuerst (Initiator-Wahl); der
    // Broker antwortet entweder mit SASL-Header oder verlangt
    // direkt AMQP. Wir starten konservativ mit SASL.
    write_protocol_header(stream, AmqpProtocol::Sasl)?;
    let server_proto = read_protocol_header(stream)?;
    let mechanism = if server_proto.protocol == AmqpProtocol::Sasl {
        Some(do_outbound_sasl(stream, cfg)?)
    } else {
        // Server skipped SASL — wir senden direkt AMQP-Header.
        None
    };

    // AMQP-Header-Exchange.
    write_protocol_header(stream, AmqpProtocol::Amqp)?;
    let amqp_proto = read_protocol_header(stream)?;
    if amqp_proto.protocol != AmqpProtocol::Amqp {
        return Err(ClientError::FrameIo(FrameIoError::UnsupportedProtocolId(
            amqp_proto.protocol.as_bytes()[4],
        )));
    }

    // Open-Performative senden.
    let open = performatives::open(&cfg.container_id)
        .map_err(|e| ClientError::Handler(HandlerError::PerformativeDecode(format!("{e}"))))?;
    write_frame(stream, FrameType::Amqp, 0, &open)?;

    // Open-Reply lesen.
    let frame = read_frame(stream, cfg.max_frame_size)?;
    let remote_container_id = extract_container_id(&frame.body);

    Ok(OutboundSession {
        state: ConnectionState::Opened,
        remote_container_id,
        sasl_mechanism: mechanism,
    })
}

fn do_outbound_sasl<S: Read + Write>(
    stream: &mut S,
    cfg: &ClientConfig,
) -> Result<SaslMechanism, ClientError> {
    // Server schickt sasl-mechanisms-Frame.
    let mechs_frame = read_frame(stream, cfg.max_frame_size)?;
    if mechs_frame.header.frame_type != FrameType::Sasl {
        return Err(ClientError::FrameIo(FrameIoError::UnsupportedProtocolId(
            mechs_frame.header.frame_type.to_u8(),
        )));
    }
    // Body decodieren: descriptor 0x40 + list[array<symbol>].
    let (_descriptor, body, _) =
        zerodds_amqp_bridge::performatives::decode_performative(&mechs_frame.body)
            .map_err(|e| ClientError::Handler(HandlerError::PerformativeDecode(format!("{e}"))))?;
    let offered = parse_offered_mechanisms(&body);

    // Spec §2.2 Cl. 5 + §10.2.1 — PLAIN nur bei TLS-aktiv.
    let chosen = SaslState::select_outbound(&offered, cfg.tls_active)
        .ok_or(ClientError::NoAcceptableSaslMechanism)?;

    // PLAIN-Schutzschicht: select_outbound filtert PLAIN bereits
    // ohne TLS, aber wir doppeln mit explizitem Reject fuer den
    // Fall einer Konfig-Inkonsistenz.
    if chosen == SaslMechanism::Plain && !cfg.tls_active {
        return Err(ClientError::PlainRejectedNoTls);
    }

    // sasl-init-Frame senden (descriptor 0x41).
    let init_descriptor: u64 = 0x0000_0000_0000_0041;
    let init_body = build_sasl_init(chosen, cfg);
    let init_payload = performatives::encode_performative(init_descriptor, &init_body)
        .map_err(|e| ClientError::Handler(HandlerError::PerformativeDecode(format!("{e}"))))?;
    write_frame(stream, FrameType::Sasl, 0, &init_payload)?;

    // sasl-outcome lesen (descriptor 0x44, code 0=ok).
    let outcome_frame = read_frame(stream, cfg.max_frame_size)?;
    let (descriptor, outcome_body, _) =
        zerodds_amqp_bridge::performatives::decode_performative(&outcome_frame.body)
            .map_err(|e| ClientError::Handler(HandlerError::PerformativeDecode(format!("{e}"))))?;
    if descriptor != 0x0000_0000_0000_0044 {
        return Err(ClientError::BrokerReject(format!(
            "expected sasl-outcome (0x44), got descriptor 0x{descriptor:x}"
        )));
    }
    let code = extract_outcome_code(&outcome_body);
    if code != Some(0) {
        return Err(ClientError::BrokerReject(format!(
            "sasl outcome code {code:?}"
        )));
    }
    Ok(chosen)
}

fn parse_offered_mechanisms(body: &AmqpExtValue) -> Vec<SaslMechanism> {
    let mut out = Vec::new();
    if let AmqpExtValue::List(items) = body {
        if let Some(AmqpExtValue::Array(arr)) = items.first() {
            for sym in arr {
                if let AmqpExtValue::Symbol(s) = sym {
                    if let Some(m) = SaslMechanism::from_name(s) {
                        out.push(m);
                    }
                }
            }
        } else if let Some(AmqpExtValue::Symbol(s)) = items.first() {
            // Manche Broker schicken einzelnen symbol statt array.
            if let Some(m) = SaslMechanism::from_name(s) {
                out.push(m);
            }
        }
    }
    out
}

fn build_sasl_init(mech: SaslMechanism, cfg: &ClientConfig) -> AmqpExtValue {
    // sasl-init body: list[mechanism: symbol, initial-response: binary?, hostname: string?].
    let mut items: Vec<AmqpExtValue> = Vec::new();
    items.push(AmqpExtValue::Symbol(mech.name().to_string()));
    let response = match (mech, &cfg.plain_credentials) {
        (SaslMechanism::Plain, Some((user, pw))) => {
            // RFC 4616: PLAIN-Form ist `\0username\0password`.
            let mut buf: Vec<u8> = Vec::new();
            buf.push(0);
            buf.extend(user.as_bytes());
            buf.push(0);
            buf.extend(pw.as_bytes());
            AmqpExtValue::Binary(buf)
        }
        (SaslMechanism::Anonymous, _) => AmqpExtValue::Binary(b"anonymous".to_vec()),
        (SaslMechanism::External, _) => AmqpExtValue::Binary(Vec::new()),
        (SaslMechanism::Plain, None) => AmqpExtValue::Binary(Vec::new()),
        (SaslMechanism::ScramSha256, Some((user, _pw))) => {
            // RFC 5802/7677 SCRAM-SHA-256 client-first-message:
            //   `n,,n=<saslprep(user)>,r=<client-nonce>`.
            // Phase-A: initial-response mit leerem nonce — der Caller
            // muss die folgenden SCRAM-Server-First/Client-Final/
            // Server-Final-Steps separat ueber das sasl-Mechanism-
            // Frame-Handling abwickeln (kein Single-Round-Trip).
            // Phase-B implementiert den vollen Loop in build_sasl_step().
            let body = format!("n,,n={user},r=");
            AmqpExtValue::Binary(body.into_bytes())
        }
        (SaslMechanism::ScramSha256, None) => {
            // Ohne credentials kein client-first-message — leerer
            // Initial-Response, Server wird mit auth-failed antworten.
            AmqpExtValue::Binary(Vec::new())
        }
    };
    items.push(response);
    AmqpExtValue::List(items)
}

fn extract_outcome_code(body: &AmqpExtValue) -> Option<u8> {
    if let AmqpExtValue::List(items) = body {
        if let Some(AmqpExtValue::Ubyte(code)) = items.first() {
            return Some(*code);
        }
    }
    None
}

fn extract_container_id(performative_body: &[u8]) -> Option<String> {
    let (_descriptor, body, _) =
        zerodds_amqp_bridge::performatives::decode_performative(performative_body).ok()?;
    if let AmqpExtValue::List(items) = body {
        if let Some(AmqpExtValue::Str(s)) = items.first() {
            return Some(s.clone());
        }
    }
    None
}

/// Spec §2.2 Cl. 7 — Reconnect-Loop mit exponential Backoff.
///
/// Versucht `connect_outbound` so lange, bis er erfolgreich ist
/// oder `max_attempts` erreicht ist. Wartezeit zwischen Versuchen
/// folgt `next_backoff_ms`.
///
/// `shutdown_signal` erlaubt kooperatives Abbrechen aus
/// einem anderen Thread.
///
/// # Errors
/// `ReconnectExhausted` wenn das Cap erreicht ist; sonst der
/// letzte connect-Fehler.
pub fn connect_with_reconnect(
    cfg: &ClientConfig,
    reconnect: &ReconnectConfig,
    shutdown_signal: &Arc<AtomicBool>,
    metrics: &Arc<MetricsHub>,
) -> Result<(TcpStream, OutboundSession), ClientError> {
    let mut attempt: u32 = 0;
    let mut last_err: Option<ClientError> = None;
    loop {
        if shutdown_signal.load(Ordering::Relaxed) {
            return Err(last_err.unwrap_or(ClientError::ReconnectExhausted(attempt)));
        }
        if let Some(max) = reconnect.max_attempts {
            if attempt >= max {
                return Err(ClientError::ReconnectExhausted(attempt));
            }
        }
        match connect_outbound(cfg) {
            Ok(ok) => return Ok(ok),
            Err(e) => {
                metrics.on_decode_error(); // generic error tally; spec
                // hat keine eigene reconnect-failure metric.
                last_err = Some(e);
                let wait_ms = reconnect.next_backoff_ms(attempt);
                attempt = attempt.saturating_add(1);
                thread::sleep(Duration::from_millis(wait_ms));
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::sync::atomic::AtomicBool;

    fn cfg(addr: &str) -> ClientConfig {
        ClientConfig {
            upstream_addr: addr.into(),
            container_id: "client-test".into(),
            max_frame_size: 65_536,
            tls_active: false,
            plain_credentials: None,
            io_timeout: Some(Duration::from_secs(2)),
        }
    }

    // --- Backoff ---

    #[test]
    fn backoff_starts_at_initial() {
        let r = ReconnectConfig::default();
        assert_eq!(r.next_backoff_ms(0), 1_000);
    }

    #[test]
    fn backoff_doubles_until_cap() {
        let r = ReconnectConfig::default();
        assert_eq!(r.next_backoff_ms(1), 2_000);
        assert_eq!(r.next_backoff_ms(2), 4_000);
        assert_eq!(r.next_backoff_ms(3), 8_000);
        assert_eq!(r.next_backoff_ms(4), 16_000);
        assert_eq!(r.next_backoff_ms(5), 32_000);
        assert_eq!(r.next_backoff_ms(6), 60_000); // capped
        assert_eq!(r.next_backoff_ms(20), 60_000); // still capped
    }

    #[test]
    fn backoff_respects_custom_cap() {
        let r = ReconnectConfig {
            initial_ms: 100,
            multiplier: 3,
            cap_ms: 5_000,
            max_attempts: None,
        };
        assert_eq!(r.next_backoff_ms(0), 100);
        assert_eq!(r.next_backoff_ms(1), 300);
        assert_eq!(r.next_backoff_ms(2), 900);
        assert_eq!(r.next_backoff_ms(3), 2_700);
        assert_eq!(r.next_backoff_ms(4), 5_000); // capped
    }

    #[test]
    fn backoff_with_unit_multiplier_stays_at_initial() {
        let r = ReconnectConfig {
            initial_ms: 500,
            multiplier: 1,
            cap_ms: 60_000,
            max_attempts: None,
        };
        assert_eq!(r.next_backoff_ms(0), 500);
        assert_eq!(r.next_backoff_ms(5), 500);
    }

    // --- SASL parsing ---

    #[test]
    fn parse_offered_mechanisms_array_form() {
        let body = AmqpExtValue::List(vec![AmqpExtValue::Array(vec![
            AmqpExtValue::Symbol("PLAIN".into()),
            AmqpExtValue::Symbol("ANONYMOUS".into()),
        ])]);
        let mechs = parse_offered_mechanisms(&body);
        assert_eq!(mechs.len(), 2);
        assert!(mechs.contains(&SaslMechanism::Plain));
        assert!(mechs.contains(&SaslMechanism::Anonymous));
    }

    #[test]
    fn parse_offered_mechanisms_single_symbol() {
        let body = AmqpExtValue::List(vec![AmqpExtValue::Symbol("EXTERNAL".into())]);
        let mechs = parse_offered_mechanisms(&body);
        assert_eq!(mechs, vec![SaslMechanism::External]);
    }

    #[test]
    fn parse_offered_mechanisms_unknown_filtered() {
        let body = AmqpExtValue::List(vec![AmqpExtValue::Array(vec![
            AmqpExtValue::Symbol("BOGUS".into()),
            AmqpExtValue::Symbol("ANONYMOUS".into()),
        ])]);
        let mechs = parse_offered_mechanisms(&body);
        assert_eq!(mechs, vec![SaslMechanism::Anonymous]);
    }

    // --- SASL-init build ---

    #[test]
    fn sasl_init_plain_includes_credentials() {
        let mut c = cfg("x:1");
        c.plain_credentials = Some(("alice".into(), "secret".into()));
        let body = build_sasl_init(SaslMechanism::Plain, &c);
        let items = match body {
            AmqpExtValue::List(v) => v,
            _ => panic!(),
        };
        assert_eq!(items[0], AmqpExtValue::Symbol("PLAIN".into()));
        let response = match &items[1] {
            AmqpExtValue::Binary(b) => b,
            _ => panic!(),
        };
        // RFC 4616: "\0alice\0secret".
        assert_eq!(response, &b"\0alice\0secret".to_vec());
    }

    #[test]
    fn sasl_init_anonymous_uses_marker() {
        let body = build_sasl_init(SaslMechanism::Anonymous, &cfg("x:1"));
        let items = match body {
            AmqpExtValue::List(v) => v,
            _ => panic!(),
        };
        assert_eq!(items[0], AmqpExtValue::Symbol("ANONYMOUS".into()));
    }

    #[test]
    fn sasl_init_external_has_empty_response() {
        let body = build_sasl_init(SaslMechanism::External, &cfg("x:1"));
        let items = match body {
            AmqpExtValue::List(v) => v,
            _ => panic!(),
        };
        assert_eq!(items[1], AmqpExtValue::Binary(Vec::new()));
    }

    // --- E2E ---

    /// E2E: Wir setzen einen Mini-AMQP-Server in einem Thread auf,
    /// connectierern als Client, verifizieren Open/Open-Roundtrip.
    #[test]
    fn outbound_connect_to_local_server() {
        // Wir nutzen unseren eigenen handler::handle_connection
        // als Server.
        use crate::handler::{HandlerConfig, handle_connection};

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        listener.set_nonblocking(false).unwrap();

        // Server-Thread.
        let server_metrics = Arc::new(MetricsHub::new());
        let server_metrics_clone = server_metrics.clone();
        let server = thread::spawn(move || {
            if let Ok((mut sock, _)) = listener.accept() {
                let _ = sock.set_read_timeout(Some(Duration::from_secs(2)));
                let _ = sock.set_write_timeout(Some(Duration::from_secs(2)));
                let cfg = HandlerConfig::for_tests(server_metrics_clone);
                let _ = handle_connection(&mut sock, &cfg);
            }
        });

        // Client-Connect.
        let client_cfg = cfg(&format!("127.0.0.1:{port}"));
        let metrics = Arc::new(MetricsHub::new());
        let shutdown = Arc::new(AtomicBool::new(false));
        let result = connect_with_reconnect(
            &client_cfg,
            &ReconnectConfig {
                max_attempts: Some(1),
                ..ReconnectConfig::default()
            },
            &shutdown,
            &metrics,
        );
        assert!(result.is_ok(), "connect failed: {result:?}");
        let (mut stream, session) = result.unwrap();
        assert_eq!(session.state, ConnectionState::Opened);
        assert!(session.remote_container_id.is_some());

        // Saubere Trennung: Client schickt Close.
        let close = performatives::close().unwrap();
        write_frame(&mut stream, FrameType::Amqp, 0, &close).unwrap();
        drop(stream);
        let _ = server.join();
    }

    #[test]
    fn reconnect_exhausts_with_max_attempts() {
        // Connect zu nicht existierender Adresse; max_attempts=2.
        let cfg = cfg("127.0.0.1:1"); // port 1 fast garantiert reject
        let metrics = Arc::new(MetricsHub::new());
        let shutdown = Arc::new(AtomicBool::new(false));
        let r = ReconnectConfig {
            initial_ms: 1, // sehr klein fuer Test-Geschwindigkeit
            multiplier: 1,
            cap_ms: 1,
            max_attempts: Some(2),
        };
        let err = connect_with_reconnect(&cfg, &r, &shutdown, &metrics).unwrap_err();
        assert!(matches!(err, ClientError::ReconnectExhausted(_)));
    }

    #[test]
    fn reconnect_aborts_on_shutdown_signal() {
        let cfg = cfg("127.0.0.1:1");
        let metrics = Arc::new(MetricsHub::new());
        let shutdown = Arc::new(AtomicBool::new(false));
        // Setze shutdown nach ca. 50ms in einem anderen Thread.
        let s = shutdown.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(50));
            s.store(true, Ordering::Relaxed);
        });
        let r = ReconnectConfig {
            initial_ms: 200, // groesser als shutdown-delay, sodass Loop schlaeft
            multiplier: 1,
            cap_ms: 200,
            max_attempts: None, // unbegrenzt; nur shutdown bricht ab
        };
        let err = connect_with_reconnect(&cfg, &r, &shutdown, &metrics);
        // Im Shutdown-Pfad bekommen wir entweder Reconnect-Exhausted
        // oder den letzten Connect-Fehler (Io). Beide sind ok.
        assert!(err.is_err());
    }
}
