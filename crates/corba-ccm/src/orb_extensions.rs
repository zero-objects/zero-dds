// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CORBA 3.3 ORB-Extensions — Stub-Layer fuer:
//! - §16 Portable Interceptors (Part 1)
//! - §17 CORBA Messaging (Part 1)
//! - §18 Compression (Part 1) + Part 2 §12 ZIOP
//! - Part 2 §11 MIOP (Multicast Inter-ORB Protocol)
//! - Part 2 §8 Inter-ORB Bridges
//! - Part 2 §9.8/§9.9 BiDirectional GIOP
//!
//! ZeroDDS hat keinen vollen ORB; diese Module liefern Configuration-
//! + Datenmodell-Layer als Stub fuer Migrations-Tooling.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

// ===========================================================================
// §16 Portable Interceptors
// ===========================================================================

/// Spec §16 — Interceptor-Point fuer Client-Side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientInterceptionPoint {
    /// `send_request` (Spec §16.4.2).
    SendRequest,
    /// `send_poll`.
    SendPoll,
    /// `receive_reply`.
    ReceiveReply,
    /// `receive_exception`.
    ReceiveException,
    /// `receive_other`.
    ReceiveOther,
}

/// Spec §16 — Interceptor-Point fuer Server-Side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerInterceptionPoint {
    /// `receive_request_service_contexts` (Spec §16.4.3).
    ReceiveRequestServiceContexts,
    /// `receive_request`.
    ReceiveRequest,
    /// `send_reply`.
    SendReply,
    /// `send_exception`.
    SendException,
    /// `send_other`.
    SendOther,
}

/// Spec §16.4.2 — `ClientRequestInterceptor`-Trait.
pub trait ClientRequestInterceptor: Send + Sync {
    /// Spec-konformer Interceptor-Name.
    fn name(&self) -> &str;
    /// Wird an einem `point` aufgerufen.
    fn intercept(&self, point: ClientInterceptionPoint, op: &str);
}

/// Spec §16.4.3 — `ServerRequestInterceptor`-Trait.
pub trait ServerRequestInterceptor: Send + Sync {
    /// Spec-konformer Interceptor-Name.
    fn name(&self) -> &str;
    /// Wird an einem `point` aufgerufen.
    fn intercept(&self, point: ServerInterceptionPoint, op: &str);
}

/// Spec §16.4.4 — `IORInterceptor`-Trait fuer ObjectReference-Erstellung.
pub trait IorInterceptor: Send + Sync {
    /// Interceptor-Name.
    fn name(&self) -> &str;
    /// Wird beim `establish_components`-Call aufgerufen.
    /// Returns optional Tagged-Components (siehe `corba-ior::tags`).
    fn establish_components(&self) -> Vec<u32>;
}

/// Spec §16.4.x — Interceptor-Registry pro ORB.
#[derive(Default)]
pub struct InterceptorRegistry {
    client: Vec<Arc<dyn ClientRequestInterceptor>>,
    server: Vec<Arc<dyn ServerRequestInterceptor>>,
    ior: Vec<Arc<dyn IorInterceptor>>,
}

impl core::fmt::Debug for InterceptorRegistry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("InterceptorRegistry")
            .field("client_count", &self.client.len())
            .field("server_count", &self.server.len())
            .field("ior_count", &self.ior.len())
            .finish()
    }
}

impl InterceptorRegistry {
    /// Konstruktor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Spec §16.4.x — `add_client_request_interceptor`.
    pub fn add_client(&mut self, c: Arc<dyn ClientRequestInterceptor>) {
        self.client.push(c);
    }

    /// Spec §16.4.x — `add_server_request_interceptor`.
    pub fn add_server(&mut self, s: Arc<dyn ServerRequestInterceptor>) {
        self.server.push(s);
    }

    /// Spec §16.4.x — `add_ior_interceptor`.
    pub fn add_ior(&mut self, i: Arc<dyn IorInterceptor>) {
        self.ior.push(i);
    }

    /// Anzahl Client-Interceptors.
    #[must_use]
    pub fn client_count(&self) -> usize {
        self.client.len()
    }

    /// Anzahl Server-Interceptors.
    #[must_use]
    pub fn server_count(&self) -> usize {
        self.server.len()
    }

    /// Anzahl IOR-Interceptors.
    #[must_use]
    pub fn ior_count(&self) -> usize {
        self.ior.len()
    }

    /// Spec §16.4.2 — Accessor fuer Pipeline-Walks im
    /// Connection-Send/Receive-Pfad.
    #[must_use]
    pub fn client_interceptors(&self) -> &[Arc<dyn ClientRequestInterceptor>] {
        &self.client
    }

    /// Spec §16.4.3 — Accessor fuer Pipeline-Walks im
    /// Acceptor-/POA-Dispatch.
    #[must_use]
    pub fn server_interceptors(&self) -> &[Arc<dyn ServerRequestInterceptor>] {
        &self.server
    }

    /// Spec §16.4.4 — Accessor fuer IOR-Build-Pfad.
    #[must_use]
    pub fn ior_interceptors(&self) -> &[Arc<dyn IorInterceptor>] {
        &self.ior
    }

    /// Spec §16.4.2 — Walk durch alle Client-Interceptors am Point
    /// `point` mit dem Operation-Namen `op`. Wird im
    /// `corba-iiop::Connection::run_client_pipeline` aufgerufen.
    pub fn walk_client(&self, point: ClientInterceptionPoint, op: &str) {
        for ic in &self.client {
            ic.intercept(point, op);
        }
    }

    /// Spec §16.4.3 — Walk durch alle Server-Interceptors am Point
    /// `point` mit dem Operation-Namen `op`.
    pub fn walk_server(&self, point: ServerInterceptionPoint, op: &str) {
        for ic in &self.server {
            ic.intercept(point, op);
        }
    }

    /// Spec §16.4.4 — Walk durch alle IOR-Interceptors. Liefert
    /// die akkumulierten TaggedComponent-Tags, die in den IOR-Build
    /// einfliessen.
    #[must_use]
    pub fn walk_ior(&self) -> Vec<u32> {
        let mut tags = Vec::new();
        for ic in &self.ior {
            tags.extend(ic.establish_components());
        }
        tags
    }
}

/// Spec §16.5 — `PolicyFactory`-Trait fuer Policy-Erzeugung.
pub trait PolicyFactory: Send + Sync {
    /// Policy-Type (Spec verwendet `PolicyType` als u32).
    fn policy_type(&self) -> u32;
    /// Erzeugt eine Policy aus einem Any-Wert (CDR-encoded).
    ///
    /// # Errors
    /// `()` bei invalid encoding.
    #[allow(clippy::result_unit_err)]
    fn create_policy(&self, value: &[u8]) -> Result<Vec<u8>, ()>;
}

/// Spec §16.6 — `PICurrent` (Per-Invocation-Slot-Storage).
#[derive(Debug, Clone, Default)]
pub struct PiCurrent {
    slots: BTreeMap<u32, Vec<u8>>,
}

impl PiCurrent {
    /// Konstruktor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Spec §16.6 — `set_slot(slot_id, value)`.
    pub fn set_slot(&mut self, slot_id: u32, value: Vec<u8>) {
        self.slots.insert(slot_id, value);
    }

    /// Spec §16.6 — `get_slot(slot_id)`.
    #[must_use]
    pub fn get_slot(&self, slot_id: u32) -> Option<&[u8]> {
        self.slots.get(&slot_id).map(Vec::as_slice)
    }
}

// ===========================================================================
// §17 CORBA Messaging (Part 1)
// ===========================================================================

/// Spec §17 — Messaging-Policy-Type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessagingPolicy {
    /// `RebindPolicy` — Rebinding-Verhalten (Spec §17.3).
    Rebind,
    /// `SyncScopePolicy` — Synchronization-Scope (Spec §17.4).
    SyncScope,
    /// `RequestPriorityPolicy` — Request-Priority (Spec §17.5).
    RequestPriority,
    /// `ReplyPriorityPolicy`.
    ReplyPriority,
    /// `RoutingPolicy` — TII (Time-Independent-Invocation) (Spec §17.7).
    Routing,
    /// `MaxHopsPolicy`.
    MaxHops,
    /// `RequestStartTimePolicy` / `RequestEndTimePolicy`.
    RequestTime,
    /// `ReplyStartTimePolicy` / `ReplyEndTimePolicy`.
    ReplyTime,
    /// `RelativeRoundtripTimeoutPolicy`.
    RelativeRoundtripTimeout,
    /// `RoutingTypeRange`.
    RoutingTypeRange,
}

impl MessagingPolicy {
    /// Spec §17 — Wire-Wert (`PolicyType` ist u32, vendor-defined).
    /// Folgt der Reihenfolge im OMG `Messaging.idl` (formal/2011-11-02
    /// §B.5.1, Policy-Types 23..32).
    #[must_use]
    pub const fn policy_type(self) -> u32 {
        match self {
            Self::Rebind => 23,
            Self::SyncScope => 24,
            Self::RequestPriority => 25,
            Self::ReplyPriority => 26,
            Self::Routing => 30,
            Self::MaxHops => 32,
            Self::RequestTime => 27,
            Self::ReplyTime => 28,
            Self::RelativeRoundtripTimeout => 31,
            Self::RoutingTypeRange => 33,
        }
    }
}

/// Spec §17.1 — AMI (Asynchronous Method Invocation) Reply-Handler-Style.
/// `ReplyHandlerStyle` markiert den Code-Generator-Pfad fuer den
/// Stub-Code (Callback vs. Polling).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmiReplyHandler {
    /// Callback-Style — Reply-Handler-Object.
    Callback,
    /// Polling-Style — Poller-Object.
    Polling,
}

/// Spec §17.5 — `AmiReplySink`-Trait fuer asynchrone Reply-Verarbeitung.
///
/// Adapter-Trait, der die drei AMI-Callback-Methoden aus dem Spec-IDL
/// (`handle_reply` / `handle_excep` / `handle_other`) konkret macht.
/// Die `dispatch_async_reply`-Funktion mappt einen GIOP-Reply
/// auf die passende Callback-Methode.
pub trait AmiReplySink: Send + Sync {
    /// Spec §17.5.1 — `handle_reply(...)` fuer `NoException`.
    fn handle_reply(&self, request_id: u32, body: &[u8]);
    /// Spec §17.5.1 — `handle_excep(...)` fuer User-/System-Exception.
    fn handle_excep(&self, request_id: u32, body: &[u8]);
    /// Spec §17.5.1 — `handle_other(...)` fuer LocationForward etc.
    fn handle_other(&self, request_id: u32, body: &[u8]);
}

/// Spec §17.5 — Mappt einen `corba_giop::Reply` auf die passende
/// AMI-Callback-Methode. Das ist die Bridge zwischen GIOP-Wire und
/// der `AmiReplySink`-Surface.
pub fn dispatch_async_reply<S: AmiReplySink + ?Sized>(sink: &S, reply: &zerodds_corba_giop::Reply) {
    use zerodds_corba_giop::ReplyStatusType;
    match reply.reply_status {
        ReplyStatusType::NoException => sink.handle_reply(reply.request_id, &reply.body),
        ReplyStatusType::UserException | ReplyStatusType::SystemException => {
            sink.handle_excep(reply.request_id, &reply.body);
        }
        ReplyStatusType::LocationForward
        | ReplyStatusType::LocationForwardPerm
        | ReplyStatusType::NeedsAddressingMode => {
            sink.handle_other(reply.request_id, &reply.body);
        }
    }
}

/// Spec §17.7 — Persistent Request Store fuer Time-Independent
/// Invocations (TII). In-Memory; Backing-Layer kann auf-Platte sein.
#[cfg(feature = "std")]
#[derive(Debug, Default)]
pub struct PersistentRequestStore {
    inner: std::sync::Mutex<BTreeMap<u32, PersistentRequestEntry>>,
}

/// Eintrag im Persistent-Request-Store.
#[cfg(feature = "std")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistentRequestEntry {
    /// Body des urspruenglichen Requests (CDR-encoded).
    pub body: Vec<u8>,
    /// Spec §17.7 — Deadline fuer den Request (Epoch-Sekunden).
    pub deadline_secs: u64,
}

#[cfg(feature = "std")]
impl PersistentRequestStore {
    /// Konstruktor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Spec §17.7 — speichert einen Request fuer asynchrone
    /// Verarbeitung.
    pub fn add(&self, request_id: u32, body: Vec<u8>, deadline_secs: u64) {
        if let Ok(mut g) = self.inner.lock() {
            g.insert(
                request_id,
                PersistentRequestEntry {
                    body,
                    deadline_secs,
                },
            );
        }
    }

    /// Spec §17.7 — holt einen Request raus (entfernt aus dem Store).
    #[must_use]
    pub fn poll(&self, request_id: u32) -> Option<PersistentRequestEntry> {
        self.inner
            .lock()
            .ok()
            .and_then(|mut g| g.remove(&request_id))
    }

    /// Spec §17.7 — entfernt alle Eintraege mit `deadline_secs < now`.
    /// Liefert die request_ids der abgelaufenen Eintraege.
    pub fn timeout_expired(&self, now_secs: u64) -> Vec<u32> {
        let Ok(mut g) = self.inner.lock() else {
            return Vec::new();
        };
        let expired: Vec<u32> = g
            .iter()
            .filter(|(_, e)| e.deadline_secs < now_secs)
            .map(|(k, _)| *k)
            .collect();
        for k in &expired {
            g.remove(k);
        }
        expired
    }

    /// Anzahl Eintraege.
    pub fn len(&self) -> usize {
        self.inner.lock().map_or(0, |g| g.len())
    }

    /// `true` wenn keine Eintraege.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

// ===========================================================================
// §18 Compression (Part 1) + Part 2 §12 ZIOP
// ===========================================================================

/// Spec §18 / Part 2 §12 — Compression-Algorithm-Identifier
/// (CORBA 3.3 ZIOP-Spec normiert "vendor-defined" Algorithmen).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionAlgorithm {
    /// `None` — keine Kompression.
    None,
    /// `Zlib` (RFC 1950).
    Zlib,
    /// `Gzip` (RFC 1952).
    Gzip,
    /// `Lzma` (XZ).
    Lzma,
    /// `Deflate` (raw, RFC 1951).
    Deflate,
}

impl CompressionAlgorithm {
    /// Spec-Wire-Wert (vendor-defined; wir nehmen 0=None, 1=Zlib, ...).
    #[must_use]
    pub const fn to_u8(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Zlib => 1,
            Self::Gzip => 2,
            Self::Lzma => 3,
            Self::Deflate => 4,
        }
    }

    /// Konvertierung vom Wire-Wert.
    ///
    /// # Errors
    /// `()` wenn Wert unbekannt.
    #[allow(clippy::result_unit_err)]
    pub const fn from_u8(v: u8) -> Result<Self, ()> {
        match v {
            0 => Ok(Self::None),
            1 => Ok(Self::Zlib),
            2 => Ok(Self::Gzip),
            3 => Ok(Self::Lzma),
            4 => Ok(Self::Deflate),
            _ => Err(()),
        }
    }

    /// Spec §18 — Komprimiert `input` mit dem gewaehlten Algorithmus.
    ///
    /// Backend-Mapping:
    /// * [`Self::None`] — passthrough (Bytes werden geclont).
    /// * [`Self::Zlib`] — RFC 1950 (zlib-wrapped deflate) via `flate2`.
    /// * [`Self::Gzip`] — RFC 1952 via `flate2`.
    /// * [`Self::Deflate`] — RFC 1951 (raw deflate) via `flate2`.
    /// * [`Self::Lzma`] — XZ; nicht abgedeckt (extra `xz2`/`liblzma`-
    ///   Build-Risiko unverhaeltnismaessig). Liefert
    ///   [`CompressionError::Unsupported`].
    ///
    /// # Errors
    /// I/O-Fehler des Compression-Backends bzw. [`CompressionError::Unsupported`].
    #[cfg(feature = "std")]
    pub fn compress(self, input: &[u8]) -> Result<Vec<u8>, CompressionError> {
        use std::io::Write;
        match self {
            Self::None => Ok(input.to_vec()),
            Self::Zlib => {
                let mut e =
                    flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
                e.write_all(input).map_err(CompressionError::from)?;
                e.finish().map_err(CompressionError::from)
            }
            Self::Gzip => {
                let mut e =
                    flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
                e.write_all(input).map_err(CompressionError::from)?;
                e.finish().map_err(CompressionError::from)
            }
            Self::Deflate => {
                let mut e =
                    flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
                e.write_all(input).map_err(CompressionError::from)?;
                e.finish().map_err(CompressionError::from)
            }
            Self::Lzma => Err(CompressionError::Unsupported(Self::Lzma)),
        }
    }

    /// Spec §18 — Dekomprimiert `input` analog zu [`Self::compress`].
    ///
    /// # Errors
    /// I/O-Fehler bzw. [`CompressionError::Unsupported`] fuer LZMA.
    #[cfg(feature = "std")]
    pub fn decompress(self, input: &[u8]) -> Result<Vec<u8>, CompressionError> {
        use std::io::Read;
        match self {
            Self::None => Ok(input.to_vec()),
            Self::Zlib => {
                let mut d = flate2::read::ZlibDecoder::new(input);
                let mut out = Vec::new();
                d.read_to_end(&mut out).map_err(CompressionError::from)?;
                Ok(out)
            }
            Self::Gzip => {
                let mut d = flate2::read::GzDecoder::new(input);
                let mut out = Vec::new();
                d.read_to_end(&mut out).map_err(CompressionError::from)?;
                Ok(out)
            }
            Self::Deflate => {
                let mut d = flate2::read::DeflateDecoder::new(input);
                let mut out = Vec::new();
                d.read_to_end(&mut out).map_err(CompressionError::from)?;
                Ok(out)
            }
            Self::Lzma => Err(CompressionError::Unsupported(Self::Lzma)),
        }
    }
}

/// Fehler im Compression-Codec.
#[cfg(feature = "std")]
#[derive(Debug)]
pub enum CompressionError {
    /// Backend-I/O-Fehler.
    Io(std::io::Error),
    /// Algorithmus ist im aktuellen Build nicht abgedeckt
    /// (Decision-Record: LZMA verlangt extra `xz2`/`liblzma`-Build).
    Unsupported(CompressionAlgorithm),
}

#[cfg(feature = "std")]
impl core::fmt::Display for CompressionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "compression io: {e}"),
            Self::Unsupported(a) => write!(f, "compression unsupported: {a:?}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for CompressionError {}

#[cfg(feature = "std")]
impl From<std::io::Error> for CompressionError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// Spec Part 2 §12 — ZIOP (Compressed-IIOP) Configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZiopConfig {
    /// Algorithm.
    pub algorithm: CompressionAlgorithm,
    /// Threshold (Bytes) — kein Compress unter diesem Wert.
    pub min_size_threshold: u32,
    /// Compression-Level (0-9 fuer Zlib/Deflate; 0 = kein, 9 = max).
    pub level: u8,
}

impl Default for ZiopConfig {
    fn default() -> Self {
        Self {
            algorithm: CompressionAlgorithm::None,
            min_size_threshold: 1024,
            level: 6,
        }
    }
}

// ===========================================================================
// Part 2 §11 MIOP (Multicast Inter-ORB Protocol)
// ===========================================================================

/// Spec Part 2 §11 — MIOP-Configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MiopConfig {
    /// IPv4-Multicast-Group-Adresse.
    pub group_addr_v4: [u8; 4],
    /// Port.
    pub port: u16,
    /// TTL (Hop-Count).
    pub ttl: u8,
    /// Loopback (lokale Replikation einschalten).
    pub loopback: bool,
}

impl Default for MiopConfig {
    fn default() -> Self {
        Self {
            // Spec MIOP — Default-Group im 239.x-Range.
            group_addr_v4: [239, 255, 0, 1],
            port: 5683,
            ttl: 1,
            loopback: false,
        }
    }
}

/// Spec Part 2 §11 — MIOP Magic-Bytes (`MIOP` ASCII).
pub const MIOP_MAGIC: [u8; 4] = *b"MIOP";

/// Spec Part 2 §11.4 — MIOP Packet-Version (`0x10` = MIOP/1.0).
pub const MIOP_VERSION_1_0: u8 = 0x10;

/// Spec Part 2 §11.4 — MIOP-Packet-Header (10 Bytes ohne Magic, plus
/// 4-Byte-Magic = 16 Bytes Header total). Kapselt einen GIOP-Frame
/// in einem UDP-Multicast-Datagram.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MiopFrameHeader {
    /// Spec — Packet-Version. Default `MIOP_VERSION_1_0`.
    pub version: u8,
    /// Spec — Flags-Byte. Bit 0 = Endian (0=BE,1=LE), Bit 1 = Last-Frag.
    pub flags: u8,
    /// Spec — Packet-Length (Anzahl Body-Bytes nach diesem Header).
    pub packet_length: u16,
    /// Spec — Eindeutiger Identifier fuer dieses Multi-Packet-Set.
    pub unique_id: u32,
    /// Spec — Index dieses Packets innerhalb des Sets (0-basiert).
    pub packet_number: u8,
    /// Spec — Anzahl Pakete im Set (>= 1).
    pub number_of_packets: u8,
}

impl MiopFrameHeader {
    /// Bytes-Length des Headers (Magic + 10-Byte-Body).
    pub const ENCODED_LEN: usize = 14;

    /// Konstruktor fuer Single-Packet-MIOP-Frame (typischer Fall:
    /// GIOP-Message passt in ein UDP-Datagram).
    #[must_use]
    pub const fn single_packet(unique_id: u32, packet_length: u16, little_endian: bool) -> Self {
        let mut flags: u8 = 0;
        if little_endian {
            flags |= 0x01;
        }
        // Last-Frag-Bit gesetzt (Bit 1).
        flags |= 0x02;
        Self {
            version: MIOP_VERSION_1_0,
            flags,
            packet_length,
            unique_id,
            packet_number: 0,
            number_of_packets: 1,
        }
    }

    /// Spec Part 2 §11.4 — Header-Encode (16 Bytes, Big-Endian-
    /// Wire-Order fuer u16/u32-Felder).
    pub fn encode(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&MIOP_MAGIC);
        out.push(self.version);
        out.push(self.flags);
        out.extend_from_slice(&self.packet_length.to_be_bytes());
        out.extend_from_slice(&self.unique_id.to_be_bytes());
        out.push(self.packet_number);
        out.push(self.number_of_packets);
    }

    /// Spec Part 2 §11.4 — Header-Decode aus den ersten 16 Bytes.
    ///
    /// # Errors
    /// `MiopError::TooShort`/`InvalidMagic`/`UnsupportedVersion`.
    pub fn decode(input: &[u8]) -> Result<(Self, usize), MiopError> {
        if input.len() < Self::ENCODED_LEN {
            return Err(MiopError::TooShort);
        }
        if input[0..4] != MIOP_MAGIC {
            return Err(MiopError::InvalidMagic);
        }
        let version = input[4];
        if version != MIOP_VERSION_1_0 {
            return Err(MiopError::UnsupportedVersion(version));
        }
        let flags = input[5];
        let packet_length = u16::from_be_bytes([input[6], input[7]]);
        let unique_id = u32::from_be_bytes([input[8], input[9], input[10], input[11]]);
        let packet_number = input[12];
        let number_of_packets = input[13];
        Ok((
            Self {
                version,
                flags,
                packet_length,
                unique_id,
                packet_number,
                number_of_packets,
            },
            Self::ENCODED_LEN,
        ))
    }

    /// Spec — `true` wenn das Last-Fragment-Bit gesetzt ist.
    #[must_use]
    pub const fn is_last_fragment(&self) -> bool {
        (self.flags & 0x02) != 0
    }

    /// Spec — `true` wenn Endian-Bit auf little-endian zeigt.
    #[must_use]
    pub const fn is_little_endian(&self) -> bool {
        (self.flags & 0x01) != 0
    }
}

/// MIOP-Codec-Errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MiopError {
    /// Buffer zu klein fuer Header.
    TooShort,
    /// Magic-Bytes != "MIOP".
    InvalidMagic,
    /// Unsupported MIOP-Version.
    UnsupportedVersion(u8),
}

/// Spec Part 2 §11 — MIOP-Sender. Adapter-Trait fuer den Multicast-
/// Sink. Konkrete Implementations leben in `transport-udp`.
pub trait MulticastSink: Send + Sync {
    /// Sendet ein UDP-Datagram an die Multicast-Group.
    ///
    /// # Errors
    /// Implementation-spezifisch (z.B. Socket-IO).
    fn send_datagram(&self, data: &[u8]) -> Result<(), MulticastSinkError>;
}

/// Multicast-Sink-Error (opaque).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MulticastSinkError(pub String);

/// Spec Part 2 §11 — MIOP-Sender, der einen GIOP-Message-Body in
/// MIOP-Frames verpackt und ueber einen `MulticastSink` versendet.
///
/// `mtu` ist die UDP-Datagram-Groesse (typisch 1472 bei 1500-MTU
/// minus 28-Byte-IP/UDP-Header). Wenn die GIOP-Bytes nicht in einen
/// einzelnen MIOP-Frame passen, wird fragmentiert.
pub struct MiopSender<S: MulticastSink> {
    sink: S,
    mtu: usize,
    next_unique_id: core::sync::atomic::AtomicU32,
}

impl<S: MulticastSink> MiopSender<S> {
    /// Konstruktor.
    #[must_use]
    pub fn new(sink: S, mtu: usize) -> Self {
        Self {
            sink,
            mtu,
            next_unique_id: core::sync::atomic::AtomicU32::new(1),
        }
    }

    /// Maximale Body-Groesse pro MIOP-Frame.
    #[must_use]
    pub const fn max_body_per_frame(&self) -> usize {
        self.mtu.saturating_sub(MiopFrameHeader::ENCODED_LEN)
    }

    /// Spec Part 2 §11.4 — sendet ein GIOP-Message-Body als ein- oder
    /// mehrteiliges MIOP-Set.
    ///
    /// # Errors
    /// `MulticastSinkError` bei IO-Failure des Sinks.
    pub fn send_giop(
        &self,
        giop_bytes: &[u8],
        little_endian: bool,
    ) -> Result<(), MulticastSinkError> {
        let max = self.max_body_per_frame();
        let unique_id = self
            .next_unique_id
            .fetch_add(1, core::sync::atomic::Ordering::Relaxed);
        if max == 0 || giop_bytes.len() <= max {
            // Single-Packet-Pfad.
            let header = MiopFrameHeader::single_packet(
                unique_id,
                u16::try_from(giop_bytes.len()).unwrap_or(u16::MAX),
                little_endian,
            );
            let mut datagram = Vec::with_capacity(MiopFrameHeader::ENCODED_LEN + giop_bytes.len());
            header.encode(&mut datagram);
            datagram.extend_from_slice(giop_bytes);
            return self.sink.send_datagram(&datagram);
        }
        // Multi-Packet-Pfad.
        let total_len = giop_bytes.len();
        let total_packets = total_len.div_ceil(max);
        let total_packets_u8 = u8::try_from(total_packets).unwrap_or(u8::MAX);
        for (idx, chunk) in giop_bytes.chunks(max).enumerate() {
            let mut flags: u8 = 0;
            if little_endian {
                flags |= 0x01;
            }
            let is_last = idx + 1 == total_packets;
            if is_last {
                flags |= 0x02;
            }
            let header = MiopFrameHeader {
                version: MIOP_VERSION_1_0,
                flags,
                packet_length: u16::try_from(chunk.len()).unwrap_or(u16::MAX),
                unique_id,
                packet_number: u8::try_from(idx).unwrap_or(u8::MAX),
                number_of_packets: total_packets_u8,
            };
            let mut datagram = Vec::with_capacity(MiopFrameHeader::ENCODED_LEN + chunk.len());
            header.encode(&mut datagram);
            datagram.extend_from_slice(chunk);
            self.sink.send_datagram(&datagram)?;
        }
        Ok(())
    }
}

// ===========================================================================
// Part 2 §8 Inter-ORB Bridges
// ===========================================================================

/// Spec Part 2 §8 — Bridge-Mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeMode {
    /// `Inline-Bridge` — embedded in einem ORB-Process.
    Inline,
    /// `Request-Level-Bridge` — separater Bridge-Process via DSI/DII.
    RequestLevel,
}

/// Spec Part 2 §8 — Bridge-Configuration.
#[derive(Debug, Clone)]
pub struct BridgeConfig {
    /// Bridge-Mode.
    pub mode: BridgeMode,
    /// Source-ORB-Identifier (z.B. `"corba"`).
    pub source_orb: String,
    /// Target-ORB-Identifier (z.B. `"dds"`).
    pub target_orb: String,
}

// ===========================================================================
// Part 2 §9.8/§9.9 Bi-Directional GIOP
// ===========================================================================

/// Spec Part 2 §9.8 — BiDir-Policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BiDirPolicy {
    /// `NORMAL` — Standard-IIOP, kein BiDir.
    Normal,
    /// `BOTH` — Server kann auch outgoing requests ueber dieselbe
    /// Connection senden (Callback-Style).
    Both,
}

/// Spec Part 2 §9.9 — BiDirectional Service-Context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BiDirServiceContext {
    /// Spec §9.9.1 — listen-points (host:port-Paare des Clients).
    pub listen_points: Vec<(String, u16)>,
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    // §16 Portable Interceptors
    struct DummyClient;
    impl ClientRequestInterceptor for DummyClient {
        fn name(&self) -> &str {
            "dummy-client"
        }
        fn intercept(&self, _: ClientInterceptionPoint, _: &str) {}
    }

    struct DummyServer;
    impl ServerRequestInterceptor for DummyServer {
        fn name(&self) -> &str {
            "dummy-server"
        }
        fn intercept(&self, _: ServerInterceptionPoint, _: &str) {}
    }

    struct DummyIor;
    impl IorInterceptor for DummyIor {
        fn name(&self) -> &str {
            "dummy-ior"
        }
        fn establish_components(&self) -> Vec<u32> {
            alloc::vec![]
        }
    }

    #[test]
    fn registry_add_increments_counts() {
        let mut r = InterceptorRegistry::new();
        r.add_client(Arc::new(DummyClient) as Arc<dyn ClientRequestInterceptor>);
        r.add_server(Arc::new(DummyServer) as Arc<dyn ServerRequestInterceptor>);
        r.add_ior(Arc::new(DummyIor) as Arc<dyn IorInterceptor>);
        assert_eq!(r.client_count(), 1);
        assert_eq!(r.server_count(), 1);
        assert_eq!(r.ior_count(), 1);
    }

    #[test]
    fn picurrent_set_get_slot() {
        let mut p = PiCurrent::new();
        p.set_slot(7, alloc::vec![0xab, 0xcd]);
        assert_eq!(p.get_slot(7), Some(&[0xab, 0xcd][..]));
        assert!(p.get_slot(99).is_none());
    }

    #[test]
    fn client_interception_points_distinct() {
        assert_ne!(
            ClientInterceptionPoint::SendRequest,
            ClientInterceptionPoint::ReceiveReply
        );
    }

    #[test]
    fn server_interception_points_distinct() {
        assert_ne!(
            ServerInterceptionPoint::ReceiveRequest,
            ServerInterceptionPoint::SendReply
        );
    }

    // §17 Messaging
    #[test]
    fn messaging_policies_distinct() {
        assert_ne!(MessagingPolicy::Rebind, MessagingPolicy::SyncScope);
    }

    #[test]
    fn ami_reply_handler_distinct() {
        assert_ne!(AmiReplyHandler::Callback, AmiReplyHandler::Polling);
    }

    // §18 + Part 2 §12 ZIOP
    #[test]
    fn compression_algorithm_round_trip() {
        for a in [
            CompressionAlgorithm::None,
            CompressionAlgorithm::Zlib,
            CompressionAlgorithm::Gzip,
            CompressionAlgorithm::Lzma,
            CompressionAlgorithm::Deflate,
        ] {
            assert_eq!(CompressionAlgorithm::from_u8(a.to_u8()).expect("ok"), a);
        }
    }

    #[test]
    fn compression_algorithm_unknown_rejected() {
        assert!(CompressionAlgorithm::from_u8(99).is_err());
    }

    #[test]
    fn ziop_config_default_no_compression() {
        let c = ZiopConfig::default();
        assert_eq!(c.algorithm, CompressionAlgorithm::None);
        assert_eq!(c.min_size_threshold, 1024);
    }

    // §18 Compression — Wire-up

    #[cfg(feature = "std")]
    #[test]
    fn compression_none_passes_through() {
        let input = b"hello, corba";
        let out = CompressionAlgorithm::None
            .compress(input)
            .expect("compress ok");
        assert_eq!(out, input);
        let back = CompressionAlgorithm::None
            .decompress(&out)
            .expect("decompress ok");
        assert_eq!(back, input);
    }

    #[cfg(feature = "std")]
    #[test]
    fn compression_zlib_round_trip() {
        let input = b"the quick brown fox jumps over the lazy dog".repeat(8);
        let compressed = CompressionAlgorithm::Zlib
            .compress(&input)
            .expect("compress ok");
        // Zlib-Output muss kuerzer sein als das wiederholte Input.
        assert!(compressed.len() < input.len());
        let back = CompressionAlgorithm::Zlib
            .decompress(&compressed)
            .expect("decompress ok");
        assert_eq!(back, input);
    }

    #[cfg(feature = "std")]
    #[test]
    fn compression_gzip_round_trip() {
        let input = b"OMG-CORBA-3.3 18 Compression spec".repeat(16);
        let compressed = CompressionAlgorithm::Gzip
            .compress(&input)
            .expect("compress ok");
        let back = CompressionAlgorithm::Gzip
            .decompress(&compressed)
            .expect("decompress ok");
        assert_eq!(back, input);
    }

    #[cfg(feature = "std")]
    #[test]
    fn compression_deflate_round_trip() {
        let input = b"deflate raw RFC1951".repeat(32);
        let compressed = CompressionAlgorithm::Deflate
            .compress(&input)
            .expect("compress ok");
        let back = CompressionAlgorithm::Deflate
            .decompress(&compressed)
            .expect("decompress ok");
        assert_eq!(back, input);
    }

    #[cfg(feature = "std")]
    #[test]
    fn compression_lzma_returns_unsupported() {
        let err = CompressionAlgorithm::Lzma
            .compress(b"x")
            .expect_err("must fail");
        assert!(matches!(
            err,
            CompressionError::Unsupported(CompressionAlgorithm::Lzma)
        ));
    }

    #[cfg(feature = "std")]
    #[test]
    fn compression_zlib_handles_large_block() {
        // 10 kB pseudo-random-Block: byte-pattern aus deterministischer
        // Index-Funktion damit der Test nicht von rand-deps abhaengt.
        let input: Vec<u8> = (0..10_000_u32)
            .map(|i| (i.wrapping_mul(2654435761) >> 24) as u8)
            .collect();
        let compressed = CompressionAlgorithm::Zlib
            .compress(&input)
            .expect("compress ok");
        let back = CompressionAlgorithm::Zlib
            .decompress(&compressed)
            .expect("decompress ok");
        assert_eq!(back, input);
    }

    // Part 2 §11 MIOP
    #[test]
    fn miop_config_default_uses_239_range() {
        let m = MiopConfig::default();
        assert_eq!(m.group_addr_v4[0], 239);
        assert_eq!(m.port, 5683);
        assert_eq!(m.ttl, 1);
    }

    // Part 2 §8 Bridges
    #[test]
    fn bridge_modes_distinct() {
        assert_ne!(BridgeMode::Inline, BridgeMode::RequestLevel);
    }

    #[test]
    fn bridge_config_construct() {
        let c = BridgeConfig {
            mode: BridgeMode::RequestLevel,
            source_orb: "corba".into(),
            target_orb: "dds".into(),
        };
        assert_eq!(c.source_orb, "corba");
    }

    // Part 2 §9.8/§9.9 BiDir
    #[test]
    fn bidir_policy_distinct() {
        assert_ne!(BiDirPolicy::Normal, BiDirPolicy::Both);
    }

    #[test]
    fn bidir_service_context_listen_points() {
        let sc = BiDirServiceContext {
            listen_points: alloc::vec![("client.example".into(), 8080)],
        };
        assert_eq!(sc.listen_points.len(), 1);
    }

    // §16 Portable Interceptors — Pipeline-Walks
    #[test]
    fn registry_walk_client_invokes_all_client_interceptors() {
        use core::sync::atomic::{AtomicUsize, Ordering};
        struct Counting {
            count: alloc::sync::Arc<AtomicUsize>,
        }
        impl ClientRequestInterceptor for Counting {
            fn name(&self) -> &str {
                "counting"
            }
            fn intercept(&self, _: ClientInterceptionPoint, _: &str) {
                self.count.fetch_add(1, Ordering::Relaxed);
            }
        }
        let count = alloc::sync::Arc::new(AtomicUsize::new(0));
        let mut r = InterceptorRegistry::new();
        r.add_client(alloc::sync::Arc::new(Counting {
            count: count.clone(),
        }) as Arc<dyn ClientRequestInterceptor>);
        r.add_client(alloc::sync::Arc::new(Counting {
            count: count.clone(),
        }) as Arc<dyn ClientRequestInterceptor>);
        r.walk_client(ClientInterceptionPoint::SendRequest, "op");
        assert_eq!(count.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn registry_walk_ior_collects_tags() {
        struct EmitTwo;
        impl IorInterceptor for EmitTwo {
            fn name(&self) -> &str {
                "emit-two"
            }
            fn establish_components(&self) -> Vec<u32> {
                alloc::vec![0xAAAA_AAAA, 0xBBBB_BBBB]
            }
        }
        let mut r = InterceptorRegistry::new();
        r.add_ior(Arc::new(EmitTwo) as Arc<dyn IorInterceptor>);
        let tags = r.walk_ior();
        assert_eq!(tags, alloc::vec![0xAAAA_AAAA, 0xBBBB_BBBB]);
    }

    // §17 Messaging — Policy-Type-Wire
    #[test]
    fn messaging_policy_wire_values_match_omg_messaging_idl() {
        assert_eq!(MessagingPolicy::Rebind.policy_type(), 23);
        assert_eq!(MessagingPolicy::SyncScope.policy_type(), 24);
        assert_eq!(MessagingPolicy::Routing.policy_type(), 30);
        assert_eq!(MessagingPolicy::RelativeRoundtripTimeout.policy_type(), 31);
    }

    // §17 AMI — Reply-Dispatch
    struct RecordingSink {
        replies: alloc::sync::Arc<std::sync::Mutex<Vec<(u32, &'static str)>>>,
    }
    impl AmiReplySink for RecordingSink {
        fn handle_reply(&self, request_id: u32, _body: &[u8]) {
            if let Ok(mut g) = self.replies.lock() {
                g.push((request_id, "reply"));
            }
        }
        fn handle_excep(&self, request_id: u32, _body: &[u8]) {
            if let Ok(mut g) = self.replies.lock() {
                g.push((request_id, "excep"));
            }
        }
        fn handle_other(&self, request_id: u32, _body: &[u8]) {
            if let Ok(mut g) = self.replies.lock() {
                g.push((request_id, "other"));
            }
        }
    }

    #[test]
    fn ami_handler_handles_no_exception_reply() {
        use zerodds_corba_giop::{Reply, ReplyStatusType, ServiceContextList};
        let replies = alloc::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = RecordingSink {
            replies: replies.clone(),
        };
        let r = Reply {
            request_id: 42,
            reply_status: ReplyStatusType::NoException,
            service_context: ServiceContextList::default(),
            body: alloc::vec![1, 2, 3],
        };
        dispatch_async_reply(&sink, &r);
        let g = replies.lock().unwrap();
        assert_eq!(*g, alloc::vec![(42_u32, "reply")]);
    }

    #[test]
    fn ami_handler_handles_user_exception_reply() {
        use zerodds_corba_giop::{Reply, ReplyStatusType, ServiceContextList};
        let replies = alloc::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = RecordingSink {
            replies: replies.clone(),
        };
        let r = Reply {
            request_id: 7,
            reply_status: ReplyStatusType::UserException,
            service_context: ServiceContextList::default(),
            body: alloc::vec![],
        };
        dispatch_async_reply(&sink, &r);
        let g = replies.lock().unwrap();
        assert_eq!(*g, alloc::vec![(7_u32, "excep")]);
    }

    // §17.7 Persistent-Request-Store fuer TII
    #[cfg(feature = "std")]
    #[test]
    fn persistent_request_store_add_poll_timeout() {
        let s = PersistentRequestStore::new();
        s.add(1, alloc::vec![0xAA], 100);
        s.add(2, alloc::vec![0xBB], 50);
        s.add(3, alloc::vec![0xCC], 200);
        assert_eq!(s.len(), 3);

        // Poll holt einen Eintrag heraus (entfernt aus Store).
        let e1 = s.poll(1).expect("present");
        assert_eq!(e1.body, alloc::vec![0xAA]);
        assert_eq!(e1.deadline_secs, 100);
        assert!(s.poll(1).is_none());
        assert_eq!(s.len(), 2);

        // Timeout @ now=120 → request 2 (deadline=50) ist expired.
        let expired = s.timeout_expired(120);
        assert_eq!(expired, alloc::vec![2]);
        assert_eq!(s.len(), 1);

        // Request 3 (deadline=200) bleibt drin.
        assert!(s.poll(3).is_some());
    }

    // Part 2 §11 MIOP — Frame-Codec
    #[test]
    fn miop_frame_encode_decode_roundtrip() {
        let h = MiopFrameHeader::single_packet(0xCAFE_BABE, 1234, true);
        let mut bytes = Vec::new();
        h.encode(&mut bytes);
        assert_eq!(bytes.len(), MiopFrameHeader::ENCODED_LEN);
        let (decoded, consumed) = MiopFrameHeader::decode(&bytes).expect("decode");
        assert_eq!(consumed, MiopFrameHeader::ENCODED_LEN);
        assert_eq!(decoded, h);
        assert!(decoded.is_last_fragment());
        assert!(decoded.is_little_endian());
    }

    #[test]
    fn miop_frame_decode_rejects_bad_magic_and_version() {
        let mut bad = alloc::vec![b'X', b'X', b'X', b'X'];
        bad.extend_from_slice(&[0u8; 10]);
        assert_eq!(
            MiopFrameHeader::decode(&bad).unwrap_err(),
            MiopError::InvalidMagic
        );

        let mut wrong_version = MIOP_MAGIC.to_vec();
        wrong_version.push(0xFF); // version
        wrong_version.extend_from_slice(&[0u8; 9]);
        assert_eq!(
            MiopFrameHeader::decode(&wrong_version).unwrap_err(),
            MiopError::UnsupportedVersion(0xFF)
        );

        let too_short = MIOP_MAGIC.to_vec();
        assert_eq!(
            MiopFrameHeader::decode(&too_short).unwrap_err(),
            MiopError::TooShort
        );
    }

    // Part 2 §11 MIOP — Sender (Single + Multi-Packet)
    struct MockSink {
        sent: alloc::sync::Arc<std::sync::Mutex<Vec<Vec<u8>>>>,
    }
    impl MulticastSink for MockSink {
        fn send_datagram(&self, data: &[u8]) -> Result<(), MulticastSinkError> {
            if let Ok(mut g) = self.sent.lock() {
                g.push(data.to_vec());
            }
            Ok(())
        }
    }

    #[test]
    fn miop_sender_single_packet_fits_mtu() {
        let sent = alloc::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = MockSink { sent: sent.clone() };
        let sender = MiopSender::new(sink, 256);
        let payload = alloc::vec![0xAB; 100];
        sender.send_giop(&payload, false).expect("send");

        let g = sent.lock().unwrap();
        assert_eq!(g.len(), 1, "single-packet path produces 1 datagram");
        assert!(g[0].starts_with(&MIOP_MAGIC));
        // Datagram = Header (14) + Payload (100).
        assert_eq!(g[0].len(), MiopFrameHeader::ENCODED_LEN + 100);
    }

    #[test]
    fn miop_sender_fragments_multi_packet_over_small_mtu() {
        let sent = alloc::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = MockSink { sent: sent.clone() };
        // MTU = 14 (Header) + 30 (Body) = 44.
        let sender = MiopSender::new(sink, 44);
        let payload = alloc::vec![0xCD; 100];
        sender.send_giop(&payload, true).expect("send");

        let g = sent.lock().unwrap();
        // 100 / 30 -> 4 Pakete (30+30+30+10).
        assert_eq!(g.len(), 4);
        // Letzter Frame muss Last-Fragment-Bit gesetzt haben.
        let (last_header, _) = MiopFrameHeader::decode(&g[3]).expect("decode");
        assert!(last_header.is_last_fragment());
        assert_eq!(last_header.packet_number, 3);
        assert_eq!(last_header.number_of_packets, 4);
        // Erster Frame: Last-Fragment NICHT gesetzt.
        let (first_header, _) = MiopFrameHeader::decode(&g[0]).expect("decode");
        assert!(!first_header.is_last_fragment());
        assert_eq!(first_header.packet_number, 0);
    }
}
