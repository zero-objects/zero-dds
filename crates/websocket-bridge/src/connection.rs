// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! One end of a WebSocket connection after the opening handshake —
//! RFC 6455 §5 to §8 and §10.7, RFC 7692 §6.
//!
//! [`Receiver`] turns the octets the peer sent into events: it checks each
//! frame against the masking rule of §5.1, the reserved bits and opcodes of
//! §5.2 and the size limits of §10.4, reassembles fragmented messages
//! (§5.4, §6.2), decompresses the messages of a negotiated
//! `permessage-deflate` (RFC 7692 §6.2), validates the UTF-8 of text
//! messages (§8.1, §8.2) and the body of a Close frame (§5.5.1, §7.4).
//! Every violation is a [`Failure`] with the status code the connection is
//! failed with (§7.1.7, §7.4.1).
//!
//! [`Sender`] builds the octets this end sends: messages (fragmented,
//! compressed and, for a client, masked with a new key per frame), Ping
//! and Pong, and the closing handshake of §7.1 — a Close is answered with
//! a Close, and no frame follows the Close this end sent (§5.5.1).

use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::close::{CloseBodyError, CloseCode, CloseHandshake, CloseState, parse_close_body};
use crate::codec::{CodecError, FrameHeader, decode_with_limit, encode, peek_header};
use crate::frame::{Frame, Opcode};
use crate::masking::{MaskingKeyProvider, MaskingViolation, Role, check_masking};
use crate::message::{Message, Reassembler, ReceiveError, SendError, split_into_frames};
#[cfg(feature = "std")]
use crate::permessage_deflate::{DeflateError, Deflater, Inflater, PermessageDeflateParams};
use crate::utf8::StreamingValidator;

/// The default limit for the payload of one frame: 16 MiB.
pub const DEFAULT_MAX_FRAME_SIZE: usize = 16 * 1024 * 1024;
/// The default limit for a message after reassembly and decompression:
/// 16 MiB.
pub const DEFAULT_MAX_MESSAGE_SIZE: usize = 16 * 1024 * 1024;

/// RFC 6455 §10.4 — the limits a receiver protects itself with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// The largest payload of one frame, as its header announces it.
    pub max_frame_size: usize,
    /// The largest message, after reassembly and decompression.
    pub max_message_size: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_frame_size: DEFAULT_MAX_FRAME_SIZE,
            max_message_size: DEFAULT_MAX_MESSAGE_SIZE,
        }
    }
}

/// What the peer sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// A complete text or binary message (§6.2), reassembled and
    /// decompressed; a text message is valid UTF-8.
    Message(Message),
    /// A Ping with its application data (§5.5.2); it is answered with
    /// [`Sender::pong`].
    Ping(Vec<u8>),
    /// A Pong with its application data (§5.5.3).
    Pong(Vec<u8>),
    /// A Close frame (§5.5.1) with its status code and reason; `None` for
    /// a Close frame without body. It is answered with
    /// [`Sender::close_received`]; the receiver delivers nothing after it.
    Close(Option<(u16, String)>),
}

/// Invalid data from the peer (§10.7): the reason the connection is failed
/// (§7.1.7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// A frame header that violates §5.2 or §5.5, or announces a payload
    /// over the limit (§10.4).
    Frame(CodecError),
    /// A frame against the masking rule of §5.1.
    Masking(MaskingViolation),
    /// A reserved bit that no negotiated extension defines (§5.2): RSV2,
    /// RSV3, RSV1 without `permessage-deflate`, or RSV1 on a control frame
    /// or a continuation frame (RFC 7692 §6.1).
    ReservedBit,
    /// A reserved opcode (§5.2).
    ReservedOpcode(u8),
    /// A message against §5.4 or §6.2, with malformed UTF-8 (§8.1, §8.2)
    /// or over the limit (§10.4).
    Message(ReceiveError),
    /// A Close frame with an invalid body (§5.5.1, §7.4).
    CloseBody(CloseBodyError),
    /// A compressed message that does not decompress (RFC 7692 §7.2.2).
    #[cfg(feature = "std")]
    Deflate(DeflateError),
}

impl Failure {
    /// RFC 6455 §7.4.1 — the status code of the Close frame the connection
    /// is failed with: 1002 for a protocol error, 1007 for data that does
    /// not fit the message type, 1009 for a frame or message over the
    /// limit.
    #[must_use]
    pub const fn close_code(&self) -> CloseCode {
        match self {
            Self::Frame(e) => e.close_code(),
            Self::Masking(v) => v.close_code(),
            Self::ReservedBit | Self::ReservedOpcode(_) => CloseCode::ProtocolError,
            Self::Message(e) => e.close_code(),
            Self::CloseBody(e) => e.close_code(),
            #[cfg(feature = "std")]
            Self::Deflate(DeflateError::TooLarge) => CloseCode::MessageTooBig,
            #[cfg(feature = "std")]
            Self::Deflate(_) => CloseCode::InvalidPayloadData,
        }
    }

    /// The Close frame that fails the connection (§7.1.7, §10.7): the
    /// status code and, as the reason, what was invalid.
    #[must_use]
    pub fn close_frame(&self) -> Frame {
        let mut reason = self.to_string();
        // A control frame carries at most 125 octets, two of them the
        // status code (§5.5); the reason is ASCII.
        reason.truncate(123);
        Frame::close(self.close_code().to_u16(), &reason)
    }
}

impl core::fmt::Display for Failure {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Frame(e) => write!(f, "{e}"),
            Self::Masking(MaskingViolation::UnmaskedFrameFromClient) => {
                f.write_str("a frame from the client is not masked")
            }
            Self::Masking(MaskingViolation::MaskedFrameFromServer) => {
                f.write_str("a frame from the server is masked")
            }
            Self::ReservedBit => f.write_str("a reserved bit is set that no extension defines"),
            Self::ReservedOpcode(op) => write!(f, "reserved opcode 0x{op:x}"),
            Self::Message(ReceiveError::UnexpectedContinuation) => {
                f.write_str("a continuation frame without a message to continue")
            }
            Self::Message(ReceiveError::InterleavedDataFrame) => {
                f.write_str("a new data frame inside a fragmented message")
            }
            Self::Message(ReceiveError::InvalidUtf8(_)) => {
                f.write_str("a text message is not UTF-8")
            }
            Self::Message(ReceiveError::MessageTooLarge) => {
                f.write_str("the message exceeds the limit")
            }
            Self::CloseBody(e) => write!(f, "{e}"),
            #[cfg(feature = "std")]
            Self::Deflate(e) => write!(f, "{e}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Failure {}

/// The receiving side of one end of a connection.
#[derive(Debug)]
pub struct Receiver {
    role: Role,
    limits: Limits,
    reassembler: Reassembler,
    #[cfg(feature = "std")]
    inflater: Option<Inflater>,
    /// A data message is in progress; `true` for a compressed one.
    message: Option<bool>,
    /// The UTF-8 check of the text frame at the start of the buffer while
    /// it is not complete.
    partial: Option<PartialText>,
    /// A Close frame was received or the connection failed.
    closed: bool,
}

/// The payload octets of an incomplete text frame that are checked, and
/// the UTF-8 state of the message behind them.
#[derive(Debug)]
struct PartialText {
    checked: usize,
    utf8: StreamingValidator,
}

impl Receiver {
    /// The receiver of `role`: [`Role::Server`] expects masked frames,
    /// [`Role::Client`] unmasked ones (§5.1).
    #[must_use]
    pub fn new(role: Role, limits: Limits) -> Self {
        Self {
            role,
            limits,
            reassembler: Reassembler::with_limit(limits.max_message_size),
            #[cfg(feature = "std")]
            inflater: None,
            message: None,
            partial: None,
            closed: false,
        }
    }

    /// The opening handshake agreed on `permessage-deflate` with `params`
    /// (RFC 7692 §7): RSV1 on the first frame of a data message marks a
    /// compressed message, which is decompressed.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn with_permessage_deflate(mut self, params: &PermessageDeflateParams) -> Self {
        self.inflater = Some(Inflater::for_receiver(self.role, params));
        self
    }

    /// Whether a Close frame was received or the connection failed: no
    /// further event comes.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.closed
    }

    #[cfg(feature = "std")]
    fn compression(&self) -> bool {
        self.inflater.is_some()
    }

    #[cfg(not(feature = "std"))]
    #[allow(clippy::unused_self)]
    fn compression(&self) -> bool {
        false
    }

    /// Takes the next event out of `buf`, the octets received and not yet
    /// consumed; the octets of the frames it read are removed. `Ok(None)`:
    /// `buf` holds no complete frame (or frames that complete no message);
    /// call again when more octets have arrived. After a Close frame the
    /// receiver reads nothing more (§5.5.1).
    ///
    /// # Errors
    /// [`Failure`] for invalid data. The connection is failed: the caller
    /// sends [`Sender::fail`] and closes; the receiver reads nothing more.
    /// A header that is invalid is refused as soon as it is there, before
    /// its payload is buffered.
    pub fn feed(&mut self, buf: &mut Vec<u8>) -> Result<Option<Event>, Failure> {
        loop {
            if self.closed {
                return Ok(None);
            }
            let Some(frame) = self.next_frame(buf).inspect_err(|_| self.closed = true)? else {
                return Ok(None);
            };
            if let Some(event) = self.frame(frame).inspect_err(|_| self.closed = true)? {
                return Ok(Some(event));
            }
        }
    }

    fn next_frame(&mut self, buf: &mut Vec<u8>) -> Result<Option<Frame>, Failure> {
        let header = match peek_header(buf) {
            Ok(h) => h,
            Err(e) if e.is_incomplete() => return Ok(None),
            Err(e) => return Err(Failure::Frame(e)),
        };
        check_masking(header.masked, self.role).map_err(Failure::Masking)?;
        // §5.2: "MUST be 0 unless an extension is negotiated that defines
        // meanings for non-zero values." permessage-deflate defines RSV1
        // on the first frame of a data message (RFC 7692 §6.1).
        let rsv1_defined =
            self.compression() && matches!(header.opcode, Opcode::Text | Opcode::Binary);
        if header.rsv2 || header.rsv3 || (header.rsv1 && !rsv1_defined) {
            return Err(Failure::ReservedBit);
        }
        if let Opcode::Reserved(op) = header.opcode {
            return Err(Failure::ReservedOpcode(op));
        }
        match decode_with_limit(buf, self.limits.max_frame_size) {
            Ok((frame, used)) => {
                buf.drain(..used);
                self.partial = None;
                Ok(Some(frame))
            }
            Err(e) if e.is_incomplete() => {
                self.check_partial_text(&header, buf)?;
                Ok(None)
            }
            Err(e) => Err(Failure::Frame(e)),
        }
    }

    /// §8.1, §8.2: the octets of a text message are checked as they
    /// arrive. A frame of an uncompressed text message that is not
    /// complete yet is validated up to the octets that are there, so
    /// malformed UTF-8 fails the connection with the octet that makes it
    /// malformed and not when the rest of the frame has come. (The
    /// payload of a compressed message is DEFLATE data; its text is
    /// checked after decompression.)
    fn check_partial_text(&mut self, header: &FrameHeader, buf: &[u8]) -> Result<(), Failure> {
        let start = match (header.opcode, self.message) {
            (Opcode::Text, None) if !header.rsv1 => Some(StreamingValidator::new()),
            (Opcode::Continuation, Some(false)) => self.reassembler.text_state(),
            _ => None,
        };
        let (Some(start), Some(payload)) = (start, buf.get(header.header_len..)) else {
            return Ok(());
        };
        let partial = self.partial.get_or_insert(PartialText {
            checked: 0,
            utf8: start,
        });
        let Some(fresh) = payload.get(partial.checked..) else {
            return Ok(());
        };
        let mut octets = fresh.to_vec();
        if header.masked {
            // The masking key is the four octets before the payload; the
            // octet at payload index i is XORed with key[i mod 4] (§5.3).
            let key = &buf[header.header_len - 4..header.header_len];
            for (i, octet) in octets.iter_mut().enumerate() {
                *octet ^= key[(partial.checked + i) % 4];
            }
        }
        partial.checked += octets.len();
        partial
            .utf8
            .feed(&octets)
            .map_err(|e| Failure::Message(ReceiveError::InvalidUtf8(e)))
    }

    fn frame(&mut self, frame: Frame) -> Result<Option<Event>, Failure> {
        match frame.opcode {
            Opcode::Ping => Ok(Some(Event::Ping(frame.payload))),
            Opcode::Pong => Ok(Some(Event::Pong(frame.payload))),
            Opcode::Close => {
                let body = parse_close_body(&frame.payload).map_err(Failure::CloseBody)?;
                self.closed = true;
                Ok(Some(Event::Close(body)))
            }
            Opcode::Text | Opcode::Binary | Opcode::Continuation => self.data(frame),
            Opcode::Reserved(op) => Err(Failure::ReservedOpcode(op)),
        }
    }

    #[cfg_attr(not(feature = "std"), allow(unused_mut))]
    fn data(&mut self, mut frame: Frame) -> Result<Option<Event>, Failure> {
        let first = frame.opcode != Opcode::Continuation;
        let compressed = match (first, self.message) {
            (true, Some(_)) => return Err(Failure::Message(ReceiveError::InterleavedDataFrame)),
            (false, None) => return Err(Failure::Message(ReceiveError::UnexpectedContinuation)),
            (true, None) => frame.rsv1,
            (false, Some(compressed)) => compressed,
        };
        self.message = if frame.fin { None } else { Some(compressed) };
        #[cfg(feature = "std")]
        if compressed {
            let room = self
                .limits
                .max_message_size
                .saturating_sub(self.reassembler.pending_len());
            let Some(inflater) = self.inflater.as_mut() else {
                return Err(Failure::ReservedBit);
            };
            frame.payload = inflater
                .decompress(&frame.payload, frame.fin, room)
                .map_err(Failure::Deflate)?;
            frame.rsv1 = false;
        }
        #[cfg(not(feature = "std"))]
        if compressed {
            return Err(Failure::ReservedBit);
        }
        match self.reassembler.feed(&frame) {
            Ok(Some(message)) => Ok(Some(Event::Message(message))),
            Ok(None) => Ok(None),
            Err(e) => Err(Failure::Message(e)),
        }
    }
}

/// The sending side of one end of a connection. Its methods return the
/// octets to write, whole frames in the order they are to be sent; a
/// caller with several writers holds one lock over the call and the write
/// (the compression context of RFC 7692 follows the order of the
/// messages).
pub struct Sender {
    /// The masking key source of a client (§5.3); `None` for a server.
    keys: Option<Box<dyn MaskingKeyProvider + Send>>,
    max_frame_payload: usize,
    #[cfg(feature = "std")]
    deflater: Option<Deflater>,
    close: CloseHandshake,
}

impl core::fmt::Debug for Sender {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Sender")
            .field("masks", &self.keys.is_some())
            .field("max_frame_payload", &self.max_frame_payload)
            .field("close", &self.close.state())
            .finish_non_exhaustive()
    }
}

impl Sender {
    /// The sender of a server: its frames carry no mask (§5.1).
    #[must_use]
    pub fn server() -> Self {
        Self {
            keys: None,
            max_frame_payload: usize::MAX,
            #[cfg(feature = "std")]
            deflater: None,
            close: CloseHandshake::new(),
        }
    }

    /// The sender of a client: every frame is masked with a new key from
    /// the operating system's random source (§5.3, §10.3).
    #[cfg(feature = "std")]
    #[must_use]
    pub fn client() -> Self {
        Self::client_with_keys(Box::new(crate::masking::OsRngProvider))
    }

    /// The sender of a client with its own masking key source.
    #[must_use]
    pub fn client_with_keys(keys: Box<dyn MaskingKeyProvider + Send>) -> Self {
        Self {
            keys: Some(keys),
            ..Self::server()
        }
    }

    /// Messages with more payload than `octets` are sent in fragments of
    /// that size (§5.4). Zero is refused when a message is sent.
    #[must_use]
    pub fn with_max_frame_payload(mut self, octets: usize) -> Self {
        self.max_frame_payload = octets;
        self
    }

    /// The opening handshake agreed on `permessage-deflate` with `params`
    /// (RFC 7692 §7): messages are sent compressed. With a window of 8
    /// bits for this direction they are sent uncompressed
    /// ([`Deflater::new`]).
    #[cfg(feature = "std")]
    #[must_use]
    pub fn with_permessage_deflate(mut self, params: &PermessageDeflateParams) -> Self {
        let role = if self.keys.is_some() {
            Role::Client
        } else {
            Role::Server
        };
        self.deflater = Deflater::for_sender(role, params);
        self
    }

    /// The state of the closing handshake (§7.1).
    #[must_use]
    pub fn close_state(&self) -> CloseState {
        self.close.state()
    }

    /// The closing handshake with the status codes sent and received.
    #[must_use]
    pub fn close_handshake(&self) -> &CloseHandshake {
        &self.close
    }

    fn open(&self) -> Result<(), SendError> {
        if self.close.state() == CloseState::Open {
            Ok(())
        } else {
            Err(SendError::Closing)
        }
    }

    fn wire(&mut self, mut frame: Frame) -> Result<Vec<u8>, SendError> {
        if let Some(keys) = self.keys.as_mut() {
            frame.masking_key = Some(keys.next_key().map_err(|_| SendError::Entropy)?);
        }
        encode(&frame).map_err(|_| SendError::ControlFrameTooLong)
    }

    /// The frames of a text (`is_text`) or binary message (§6.1).
    ///
    /// # Errors
    /// [`SendError::Closing`] after a Close frame was sent or received
    /// (§5.5.1), [`SendError::InvalidUtf8`] for a text message that is not
    /// UTF-8, [`SendError::Entropy`] when the client's key source delivers
    /// no key, [`SendError::InvalidFrameLimit`] for a fragment size of
    /// zero, [`SendError::Deflate`] when the message does not compress.
    pub fn message(&mut self, is_text: bool, payload: &[u8]) -> Result<Vec<u8>, SendError> {
        self.open()?;
        if is_text {
            crate::utf8::validate(payload).map_err(|_| SendError::InvalidUtf8)?;
        }
        let opcode = if is_text {
            Opcode::Text
        } else {
            Opcode::Binary
        };
        #[cfg(feature = "std")]
        let compressed = match self.deflater.as_mut() {
            // An empty message is sent as it is: there is nothing to
            // compress, and an uncompressed message leaves the window
            // alone (RFC 7692 §6, §7.2.3.2).
            Some(deflater) if !payload.is_empty() => {
                Some(deflater.compress(payload).map_err(|_| SendError::Deflate)?)
            }
            _ => None,
        };
        #[cfg(not(feature = "std"))]
        let compressed: Option<Vec<u8>> = None;
        let body = compressed.as_deref().unwrap_or(payload);
        let keys = self
            .keys
            .as_mut()
            .map(|k| &mut **k as &mut dyn MaskingKeyProvider);
        let frames = split_into_frames(
            opcode,
            compressed.is_some(),
            body,
            self.max_frame_payload,
            keys,
        )?;
        let mut out = Vec::with_capacity(body.len() + 14 * frames.len());
        for frame in &frames {
            out.extend_from_slice(&encode(frame).map_err(|_| SendError::ControlFrameTooLong)?);
        }
        Ok(out)
    }

    /// A Ping frame (§5.5.2).
    ///
    /// # Errors
    /// [`SendError::ControlFrameTooLong`] for more than 125 octets,
    /// [`SendError::Closing`], [`SendError::Entropy`].
    pub fn ping(&mut self, payload: &[u8]) -> Result<Vec<u8>, SendError> {
        self.open()?;
        self.wire(Frame::ping(payload.to_vec()))
    }

    /// The Pong frame that answers a Ping with `payload` (§5.5.3: the same
    /// application data).
    ///
    /// # Errors
    /// As [`Self::ping`].
    pub fn pong(&mut self, payload: &[u8]) -> Result<Vec<u8>, SendError> {
        self.open()?;
        self.wire(Frame::pong(payload.to_vec()))
    }

    /// Starts the closing handshake (§7.1.2): the Close frame with
    /// `status` and `reason`. No frame follows it (§5.5.1).
    ///
    /// # Errors
    /// [`SendError::StatusNotAllowed`] for a status code that must not be
    /// sent (§7.4), [`SendError::ControlFrameTooLong`] for a reason over
    /// 123 octets, [`SendError::Closing`] when a Close was already sent or
    /// received.
    pub fn close(&mut self, status: u16, reason: &str) -> Result<Vec<u8>, SendError> {
        self.open()?;
        if crate::close::validate_wire_status_code(status).is_err() {
            return Err(SendError::StatusNotAllowed);
        }
        if reason.len() > 123 {
            return Err(SendError::ControlFrameTooLong);
        }
        let wire = self.wire(Frame::close(status, reason))?;
        self.close
            .initiator_send_close(status)
            .map_err(|()| SendError::Closing)?;
        Ok(wire)
    }

    /// The peer's Close frame arrived ([`Event::Close`]). §5.5.1: "If an
    /// endpoint receives a Close frame and did not previously send a Close
    /// frame, the endpoint MUST send a Close frame in response", with the
    /// status code it received. Returns that frame, or `None` when this
    /// end had sent its Close before: the peer's frame is the response and
    /// the handshake is complete (§7.1.4). The connection is closed after
    /// it either way.
    ///
    /// # Errors
    /// [`SendError::Entropy`] when the client's key source delivers no
    /// key.
    pub fn close_received(
        &mut self,
        body: Option<&(u16, String)>,
    ) -> Result<Option<Vec<u8>>, SendError> {
        // §7.1.5: a Close frame without a status code is read as 1005.
        let received = body.map_or(1005, |(code, _)| *code);
        match self.close.state() {
            CloseState::Open => {
                let frame = match body {
                    Some((code, _)) => Frame::close(*code, ""),
                    None => Frame {
                        payload: Vec::new(),
                        ..Frame::close(1000, "")
                    },
                };
                let wire = self.wire(frame)?;
                self.close
                    .responder_recv_close(received)
                    .map_err(|()| SendError::Closing)?;
                self.close
                    .responder_send_close_response()
                    .map_err(|()| SendError::Closing)?;
                Ok(Some(wire))
            }
            CloseState::ClosingInitiator => {
                self.close
                    .recv_close_response(received)
                    .map_err(|()| SendError::Closing)?;
                Ok(None)
            }
            CloseState::ClosingResponder | CloseState::Closed | CloseState::Failed => Ok(None),
        }
    }

    /// Fails the connection for `failure` (§7.1.7, §10.7): the Close frame
    /// with its status code, or `None` when a Close frame was already sent.
    /// The handshake is [`CloseState::Failed`] after it.
    pub fn fail(&mut self, failure: &Failure) -> Option<Vec<u8>> {
        let wire = if self.close.state() == CloseState::Open {
            let code = failure.close_code().to_u16();
            let wire = self.wire(failure.close_frame()).ok();
            if wire.is_some() {
                // The state is Open, which is the one the call needs.
                let _sent = self.close.initiator_send_close(code);
            }
            wire
        } else {
            None
        };
        self.close.fail(failure.to_string());
        wire
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::codec::decode;
    use crate::masking::{ClosureMaskingKeyProvider, EntropyError};

    const KEY: [u8; 4] = [0x37, 0xfa, 0x21, 0x3d];

    fn server() -> Receiver {
        Receiver::new(Role::Server, Limits::default())
    }

    /// The wire octets of `frame` as a client sends it.
    fn masked(frame: Frame) -> Vec<u8> {
        encode(&frame.with_mask(KEY)).expect("encode")
    }

    /// A frame of `opcode` with FIN as given, masked.
    fn part(opcode: Opcode, fin: bool, payload: &[u8]) -> Vec<u8> {
        masked(Frame {
            fin,
            opcode,
            ..Frame::binary(payload.to_vec())
        })
    }

    /// Feeds `wire` and expects the failure with `code`.
    fn fails(receiver: &mut Receiver, wire: &[u8], code: u16) -> Failure {
        let mut buf = wire.to_vec();
        let failure = receiver.feed(&mut buf).expect_err("invalid data");
        assert_eq!(failure.close_code().to_u16(), code, "{failure}");
        // The Close frame that fails the connection carries the code.
        let close = failure.close_frame();
        assert_eq!(close.opcode, Opcode::Close);
        assert_eq!(&close.payload[..2], &code.to_be_bytes());
        assert!(close.payload.len() <= 125);
        // Nothing is read after the failure.
        let mut more = masked(Frame::text("x"));
        assert_eq!(receiver.feed(&mut more), Ok(None));
        assert!(receiver.is_closed());
        failure
    }

    fn all_events(receiver: &mut Receiver, wire: &[u8]) -> Vec<Event> {
        let mut buf = wire.to_vec();
        let mut events = Vec::new();
        while let Some(event) = receiver.feed(&mut buf).expect("valid data") {
            events.push(event);
        }
        events
    }

    fn text(s: &str) -> Event {
        Event::Message(Message {
            is_text: true,
            payload: s.as_bytes().to_vec(),
        })
    }

    #[test]
    fn ws_5_1_the_server_fails_an_unmasked_frame_with_1002() {
        // §5.1: "The server MUST close the connection upon receiving a
        // frame that is not masked." The RFC's own unmasked "Hello"
        // (§5.7).
        let failure = fails(
            &mut server(),
            &[0x81, 0x05, 0x48, 0x65, 0x6c, 0x6c, 0x6f],
            1002,
        );
        assert_eq!(
            failure,
            Failure::Masking(MaskingViolation::UnmaskedFrameFromClient)
        );
        // The masked one of §5.7 is delivered.
        let wire = [
            0x81, 0x85, 0x37, 0xfa, 0x21, 0x3d, 0x7f, 0x9f, 0x4d, 0x51, 0x58,
        ];
        assert_eq!(all_events(&mut server(), &wire), [text("Hello")]);
    }

    #[test]
    fn ws_5_1_the_client_fails_a_masked_frame_with_1002() {
        // §5.1: "A client MUST close a connection if it detects a masked
        // frame."
        let mut client = Receiver::new(Role::Client, Limits::default());
        let failure = fails(&mut client, &masked(Frame::text("Hello")), 1002);
        assert_eq!(
            failure,
            Failure::Masking(MaskingViolation::MaskedFrameFromServer)
        );
        let mut client = Receiver::new(Role::Client, Limits::default());
        assert_eq!(
            all_events(&mut client, &encode(&Frame::text("Hello")).unwrap()),
            [text("Hello")]
        );
    }

    #[test]
    fn ws_5_1_an_unmasked_frame_is_refused_from_its_header() {
        // Two octets of a frame that announces 100 octets of payload: the
        // violation is known, the payload is not waited for.
        let mut buf = alloc::vec![0x82u8, 100];
        assert_eq!(
            server().feed(&mut buf),
            Err(Failure::Masking(MaskingViolation::UnmaskedFrameFromClient))
        );
    }

    #[test]
    fn ws_5_2_a_reserved_bit_without_an_extension_fails_with_1002() {
        // §5.2: "If a nonzero value is received and none of the negotiated
        // extensions defines the meaning of such a nonzero value, the
        // receiving endpoint MUST _Fail the WebSocket Connection_."
        for bit in [0x40u8, 0x20, 0x10] {
            let mut wire = masked(Frame::text("x"));
            wire[0] |= bit;
            assert_eq!(fails(&mut server(), &wire, 1002), Failure::ReservedBit);
            // On a control frame too.
            let mut wire = masked(Frame::ping(Vec::new()));
            wire[0] |= bit;
            assert_eq!(fails(&mut server(), &wire, 1002), Failure::ReservedBit);
        }
    }

    #[test]
    fn ws_5_2_a_reserved_opcode_fails_with_1002() {
        // §5.2: "If an unknown opcode is received, the receiving endpoint
        // MUST _Fail the WebSocket Connection_."
        for op in [0x3u8, 0x4, 0x5, 0x6, 0x7, 0xb, 0xc, 0xd, 0xe, 0xf] {
            let wire = part(Opcode::Reserved(op), true, b"");
            assert_eq!(
                fails(&mut server(), &wire, 1002),
                Failure::ReservedOpcode(op)
            );
        }
    }

    #[test]
    fn ws_5_2_a_length_that_is_not_minimal_fails_with_1002() {
        // §5.2: "the minimal number of bytes MUST be used to encode the
        // length". Without the check the stream would stall: the frame
        // never decodes.
        let wire = [0x82, 0xfe, 0x00, 0x05, 1, 2, 3, 4, 0, 0, 0, 0, 0];
        assert_eq!(
            fails(&mut server(), &wire, 1002),
            Failure::Frame(CodecError::NonMinimalLength)
        );
        // The most significant bit of a 64-bit length.
        let mut wire = alloc::vec![0x82u8, 0xff, 0x80, 0, 0, 0, 0, 0, 0, 0];
        wire.extend_from_slice(&KEY);
        assert_eq!(
            fails(&mut server(), &wire, 1002),
            Failure::Frame(CodecError::PayloadLengthMsbSet)
        );
    }

    #[test]
    fn ws_5_4_a_fragmented_message_is_delivered_whole() {
        // §5.4, the example of §5.7: "Hel" + "lo".
        let mut wire = part(Opcode::Text, false, b"Hel");
        wire.extend(part(Opcode::Continuation, true, b"lo"));
        assert_eq!(all_events(&mut server(), &wire), [text("Hello")]);
        // Octet by octet, as a slow connection delivers it.
        let mut receiver = server();
        let mut buf = Vec::new();
        let mut events = Vec::new();
        for octet in &wire {
            buf.push(*octet);
            while let Some(event) = receiver.feed(&mut buf).unwrap() {
                events.push(event);
            }
        }
        assert_eq!(events, [text("Hello")]);
        assert!(buf.is_empty());
    }

    #[test]
    fn ws_5_4_control_frames_may_come_between_the_fragments() {
        // §5.4: "Control frames MAY be injected in the middle of a
        // fragmented message."
        let mut wire = part(Opcode::Binary, false, &[1, 2]);
        wire.extend(masked(Frame::ping(b"p".to_vec())));
        wire.extend(part(Opcode::Continuation, false, &[3]));
        wire.extend(masked(Frame::pong(b"q".to_vec())));
        wire.extend(part(Opcode::Continuation, true, &[4]));
        assert_eq!(
            all_events(&mut server(), &wire),
            [
                Event::Ping(b"p".to_vec()),
                Event::Pong(b"q".to_vec()),
                Event::Message(Message {
                    is_text: false,
                    payload: alloc::vec![1, 2, 3, 4],
                }),
            ]
        );
    }

    #[test]
    fn ws_5_4_a_continuation_without_a_message_fails_with_1002() {
        let wire = part(Opcode::Continuation, true, b"x");
        assert_eq!(
            fails(&mut server(), &wire, 1002),
            Failure::Message(ReceiveError::UnexpectedContinuation)
        );
        // Also after a message that was complete.
        let mut receiver = server();
        assert_eq!(
            all_events(&mut receiver, &masked(Frame::text("done"))),
            [text("done")]
        );
        fails(
            &mut receiver,
            &part(Opcode::Continuation, false, b"x"),
            1002,
        );
    }

    #[test]
    fn ws_5_4_a_new_data_frame_inside_a_message_fails_with_1002() {
        // §5.4: "The fragments of one message MUST NOT be interleaved
        // between the fragments of another message".
        let mut receiver = server();
        assert_eq!(
            all_events(&mut receiver, &part(Opcode::Text, false, b"a")),
            []
        );
        assert_eq!(
            fails(&mut receiver, &part(Opcode::Text, true, b"b"), 1002),
            Failure::Message(ReceiveError::InterleavedDataFrame)
        );
    }

    #[test]
    fn ws_5_5_a_control_frame_that_is_fragmented_or_long_fails_with_1002() {
        // §5.5: "All control frames MUST have a payload length of 125
        // bytes or less and MUST NOT be fragmented."
        let mut wire = alloc::vec![0x09u8, 0x80];
        wire.extend_from_slice(&KEY);
        assert_eq!(
            fails(&mut server(), &wire, 1002),
            Failure::Frame(CodecError::FragmentedControlFrame)
        );
        let mut wire = alloc::vec![0x89u8, 0xfe, 0x00, 0x7e];
        wire.extend_from_slice(&KEY);
        assert_eq!(
            fails(&mut server(), &wire, 1002),
            Failure::Frame(CodecError::ControlFrameTooLong)
        );
    }

    #[test]
    fn ws_5_5_1_a_close_is_delivered_with_its_status_and_ends_the_reading() {
        let mut receiver = server();
        let mut buf = masked(Frame::close(1000, "bye"));
        buf.extend(masked(Frame::text("late")));
        assert_eq!(
            receiver.feed(&mut buf),
            Ok(Some(Event::Close(Some((1000, "bye".to_string())))))
        );
        // §5.5.1: nothing after the Close is processed.
        assert!(receiver.is_closed());
        assert_eq!(receiver.feed(&mut buf), Ok(None));
        // A Close without body carries no status code.
        let mut receiver = server();
        let close = Frame {
            payload: Vec::new(),
            ..Frame::close(1000, "")
        };
        assert_eq!(
            all_events(&mut receiver, &masked(close)),
            [Event::Close(None)]
        );
    }

    #[test]
    fn ws_5_5_1_an_invalid_close_body_fails_the_connection() {
        // One octet: no status code. 1005 and 1006 MUST NOT be set in a
        // Close frame (§7.4.1); 999 and 5000 are outside every range.
        let one = Frame {
            payload: alloc::vec![0x03],
            ..Frame::close(1000, "")
        };
        assert_eq!(
            fails(&mut server(), &masked(one), 1002),
            Failure::CloseBody(CloseBodyError::TruncatedStatus)
        );
        for code in [999u16, 1004, 1005, 1006, 1015, 1016, 2999, 5000] {
            assert_eq!(
                fails(&mut server(), &masked(Frame::close(code, "")), 1002),
                Failure::CloseBody(CloseBodyError::StatusNotAllowed(code))
            );
        }
        // A reason that is not UTF-8 is 1007.
        let bad = Frame {
            payload: alloc::vec![0x03, 0xe8, 0xff, 0xfe],
            ..Frame::close(1000, "")
        };
        assert_eq!(
            fails(&mut server(), &masked(bad), 1007),
            Failure::CloseBody(CloseBodyError::ReasonNotUtf8)
        );
    }

    #[test]
    fn ws_5_5_1_a_close_is_answered_with_a_close_of_the_same_status() {
        // §5.5.1: "If an endpoint receives a Close frame and did not
        // previously send a Close frame, the endpoint MUST send a Close
        // frame in response. [...] it typically echos the status code it
        // received."
        let mut receiver = server();
        let mut sender = Sender::server();
        let events = all_events(&mut receiver, &masked(Frame::close(4001, "done")));
        let [Event::Close(body)] = &events[..] else {
            panic!("{events:?}");
        };
        let wire = sender
            .close_received(body.as_ref())
            .expect("a response")
            .expect("the first Close of this end");
        let (frame, used) = decode(&wire).expect("one frame");
        assert_eq!(used, wire.len());
        assert_eq!(frame.opcode, Opcode::Close);
        assert_eq!(frame.masking_key, None);
        assert_eq!(frame.payload, 4001u16.to_be_bytes());
        assert_eq!(sender.close_state(), CloseState::Closed);
        assert_eq!(sender.close_handshake().received_code(), Some(4001));
        assert_eq!(sender.close_handshake().sent_code(), Some(4001));
        // A Close without status code is answered with one without.
        let mut sender = Sender::server();
        let wire = sender.close_received(None).unwrap().unwrap();
        assert_eq!(wire, [0x88, 0x00]);
        assert_eq!(sender.close_handshake().received_code(), Some(1005));
    }

    #[test]
    fn ws_5_5_1_no_frame_follows_the_close_this_end_sent() {
        // §5.5.1: "After sending a Close frame, an endpoint MUST NOT send
        // any more data frames".
        let mut sender = Sender::server();
        let wire = sender.close(1001, "going away").expect("the Close");
        let (frame, _) = decode(&wire).unwrap();
        assert_eq!(frame, Frame::close(1001, "going away"));
        assert_eq!(sender.close_state(), CloseState::ClosingInitiator);
        assert_eq!(sender.message(true, b"late"), Err(SendError::Closing));
        assert_eq!(sender.ping(b""), Err(SendError::Closing));
        assert_eq!(sender.pong(b""), Err(SendError::Closing));
        assert_eq!(sender.close(1000, ""), Err(SendError::Closing));
        // §7.1.4: the peer's Close completes the handshake; this end sends
        // no second one.
        assert_eq!(
            sender.close_received(Some(&(1001, String::new()))),
            Ok(None)
        );
        assert_eq!(sender.close_state(), CloseState::Closed);
        assert!(sender.close_handshake().is_closed());
    }

    #[test]
    fn ws_7_4_a_status_code_that_must_not_be_sent_is_refused() {
        for code in [999u16, 1005, 1006, 1015, 5000] {
            assert_eq!(
                Sender::server().close(code, ""),
                Err(SendError::StatusNotAllowed)
            );
        }
        let long = "x".repeat(124);
        assert_eq!(
            Sender::server().close(1000, &long),
            Err(SendError::ControlFrameTooLong)
        );
    }

    #[test]
    fn ws_7_1_7_a_failed_connection_sends_its_close_once() {
        // §7.1.7, §10.7: the Close frame with the status code of the
        // failure; a Close this end already sent is not followed by one.
        let failure = Failure::ReservedBit;
        let mut sender = Sender::server();
        let wire = sender.fail(&failure).expect("the Close");
        let (frame, _) = decode(&wire).unwrap();
        assert_eq!(frame.opcode, Opcode::Close);
        assert_eq!(&frame.payload[..2], &1002u16.to_be_bytes());
        assert_eq!(sender.close_state(), CloseState::Failed);
        assert_eq!(sender.close_handshake().sent_code(), Some(1002));
        assert_eq!(sender.fail(&failure), None);
        assert_eq!(sender.message(false, b"x"), Err(SendError::Closing));

        let mut sender = Sender::server();
        sender.close(1000, "").unwrap();
        assert_eq!(sender.fail(&failure), None);
        assert_eq!(sender.close_state(), CloseState::Failed);
    }

    #[test]
    fn ws_8_2_malformed_utf8_from_the_client_fails_with_1007() {
        // §8.2: "When a server receives a Text-frame from the client that
        // contains malformed UTF-8 data [...] the server MUST fail the
        // WebSocket Connection" — §7.4.1 gives it 1007. The octets are the
        // ones of Autobahn case 6.3.1.
        let bad = [
            0xce, 0xba, 0xe1, 0xbd, 0xb9, 0xcf, 0x83, 0xce, 0xbc, 0xce, 0xb5, 0xed, 0xa0, 0x80,
            0x65, 0x64, 0x69, 0x74, 0x65, 0x64,
        ];
        let failure = fails(&mut server(), &part(Opcode::Text, true, &bad), 1007);
        assert!(matches!(
            failure,
            Failure::Message(ReceiveError::InvalidUtf8(_))
        ));
        // The same octets as a binary message are data.
        assert_eq!(
            all_events(&mut server(), &part(Opcode::Binary, true, &bad)),
            [Event::Message(Message {
                is_text: false,
                payload: bad.to_vec(),
            })]
        );
        // Split over fragments, the message fails with the fragment that
        // makes it invalid, before the last one arrives.
        let mut receiver = server();
        assert_eq!(
            all_events(&mut receiver, &part(Opcode::Text, false, &bad[..11])),
            []
        );
        fails(
            &mut receiver,
            &part(Opcode::Continuation, false, &bad[11..14]),
            1007,
        );
        // A code point cut by the end of the message is malformed too.
        let mut receiver = server();
        assert_eq!(
            all_events(&mut receiver, &part(Opcode::Text, false, &[0xce])),
            []
        );
        fails(&mut receiver, &part(Opcode::Continuation, true, b""), 1007);
    }

    #[test]
    fn ws_8_2_malformed_utf8_fails_with_the_octet_that_makes_it_malformed() {
        // The frame announces 20 octets; the surrogate ed a0 80 at 11 to
        // 13 is malformed with its second octet (Autobahn cases 6.4.2 to
        // 6.4.4 send such a frame in pieces and expect the failure before
        // the rest).
        let bad = [
            0xce, 0xba, 0xe1, 0xbd, 0xb9, 0xcf, 0x83, 0xce, 0xbc, 0xce, 0xb5, 0xed, 0xa0, 0x80,
            0x65, 0x64, 0x69, 0x74, 0x65, 0x64,
        ];
        let wire = part(Opcode::Text, true, &bad);
        let header = wire.len() - bad.len();
        // Up to the octet before: no event, no failure, nothing consumed;
        // the code point cut after ed is not malformed yet.
        let mut receiver = server();
        let mut buf = Vec::new();
        for octet in &wire[..header + 12] {
            buf.push(*octet);
            assert_eq!(receiver.feed(&mut buf), Ok(None));
        }
        assert_eq!(buf.len(), header + 12);
        // The octet a0 behind ed: the frame is 7 octets short and fails.
        buf.push(wire[header + 12]);
        let failure = receiver.feed(&mut buf).expect_err("malformed UTF-8");
        assert_eq!(failure.close_code().to_u16(), 1007);
        assert!(receiver.is_closed());

        // In a continuation frame the check goes on from the state of the
        // frames before: e1 bd in the first frame, the b9 that completes
        // the code point and then a lone continuation octet in the second.
        let mut receiver = server();
        let mut buf = part(Opcode::Text, false, &[0xe1, 0xbd]);
        let second = part(Opcode::Continuation, true, &[0xb9, b'a', 0x80, b'b', b'c']);
        let header = second.len() - 5;
        buf.extend_from_slice(&second[..header + 2]);
        assert_eq!(receiver.feed(&mut buf), Ok(None));
        buf.push(second[header + 2]);
        let failure = receiver.feed(&mut buf).expect_err("malformed UTF-8");
        assert_eq!(failure.close_code().to_u16(), 1007);

        // A valid text frame fed octet by octet is the message, a code
        // point cut at every position among them.
        let mut receiver = server();
        let wire = part(Opcode::Text, true, "a\u{e9}\u{20ac}\u{1f600}b".as_bytes());
        let mut buf = Vec::new();
        let mut events = Vec::new();
        for octet in &wire {
            buf.push(*octet);
            events.extend(receiver.feed(&mut buf).expect("valid"));
        }
        assert_eq!(events, [text("a\u{e9}\u{20ac}\u{1f600}b")]);

        // The same malformed octets in a binary frame that is not
        // complete are not looked at.
        let mut receiver = server();
        let wire = part(Opcode::Binary, true, &bad);
        let mut buf = wire[..wire.len() - 1].to_vec();
        assert_eq!(receiver.feed(&mut buf), Ok(None));
        // A client checks the unmasked frame of a server the same way.
        let mut client = Receiver::new(Role::Client, Limits::default());
        let wire = encode(&Frame::text("x")).unwrap();
        let mut buf = alloc::vec![wire[0], 0x05, b'a', 0xff];
        assert_eq!(
            client.feed(&mut buf).map_err(|f| f.close_code().to_u16()),
            Err(1007)
        );
    }

    #[test]
    fn ws_8_1_malformed_utf8_from_the_server_fails_with_1007() {
        let mut client = Receiver::new(Role::Client, Limits::default());
        let wire = encode(&Frame {
            opcode: Opcode::Text,
            ..Frame::binary(alloc::vec![0xc3, 0x28])
        })
        .unwrap();
        fails(&mut client, &wire, 1007);
    }

    #[test]
    fn ws_6_2_a_code_point_split_over_fragments_is_valid() {
        // U+00E9 (c3 a9) cut between two frames.
        let mut wire = part(Opcode::Text, false, &[b'a', 0xc3]);
        wire.extend(part(Opcode::Continuation, true, &[0xa9, b'b']));
        assert_eq!(all_events(&mut server(), &wire), [text("a\u{e9}b")]);
    }

    #[test]
    fn ws_10_4_a_frame_over_the_limit_fails_with_1009_from_its_header() {
        let limits = Limits {
            max_frame_size: 1024,
            max_message_size: 4096,
        };
        // Fourteen octets: a masked header that announces 2^40 octets.
        let mut wire = alloc::vec![0x82u8, 0xff];
        wire.extend_from_slice(&(1u64 << 40).to_be_bytes());
        wire.extend_from_slice(&KEY);
        let mut receiver = Receiver::new(Role::Server, limits);
        assert_eq!(
            fails(&mut receiver, &wire, 1009),
            Failure::Frame(CodecError::FrameTooLarge {
                announced: 1 << 40,
                limit: 1024
            })
        );
        // A frame at the limit passes.
        let mut receiver = Receiver::new(Role::Server, limits);
        let events = all_events(&mut receiver, &part(Opcode::Binary, true, &[7u8; 1024]));
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn ws_10_4_a_message_of_many_small_frames_over_the_limit_fails_with_1009() {
        let limits = Limits {
            max_frame_size: 1024,
            max_message_size: 4096,
        };
        let mut receiver = Receiver::new(Role::Server, limits);
        let mut wire = part(Opcode::Binary, false, &[0u8; 1024]);
        for _ in 0..3 {
            wire.extend(part(Opcode::Continuation, false, &[0u8; 1024]));
        }
        assert_eq!(all_events(&mut receiver, &wire), []);
        assert_eq!(
            fails(
                &mut receiver,
                &part(Opcode::Continuation, true, &[0u8; 1]),
                1009
            ),
            Failure::Message(ReceiveError::MessageTooLarge)
        );
    }

    #[test]
    fn ws_5_3_every_frame_of_a_client_carries_a_key_of_its_own() {
        // §5.3: all frames from the client are masked; §10.3: a new key
        // for each frame. Data frames, fragments and control frames.
        let mut n = 0u32;
        let keys = ClosureMaskingKeyProvider(move || {
            n += 1;
            Ok(n.to_be_bytes())
        });
        let mut sender = Sender::client_with_keys(Box::new(keys)).with_max_frame_payload(2);
        let mut wire = sender.message(true, b"abcde").unwrap();
        wire.extend(sender.ping(b"p").unwrap());
        wire.extend(sender.pong(b"p").unwrap());
        wire.extend(sender.close(1000, "").unwrap());
        let mut keys = Vec::new();
        let mut rest = &wire[..];
        while !rest.is_empty() {
            let (frame, used) = decode(rest).unwrap();
            keys.push(frame.masking_key.expect("masked"));
            rest = &rest[used..];
        }
        assert_eq!(
            keys,
            [
                [0, 0, 0, 1],
                [0, 0, 0, 2],
                [0, 0, 0, 3],
                [0, 0, 0, 4],
                [0, 0, 0, 5],
                [0, 0, 0, 6]
            ]
        );
        // A server reads what the client sent; a client refuses it.
        let events = all_events(&mut server(), &wire);
        assert_eq!(events[0], text("abcde"));
        assert_eq!(events[3], Event::Close(Some((1000, String::new()))));
    }

    #[test]
    fn ws_5_3_a_client_without_a_key_sends_no_frame() {
        let keys = ClosureMaskingKeyProvider(|| Err(EntropyError));
        let mut sender = Sender::client_with_keys(Box::new(keys));
        assert_eq!(sender.message(false, b"x"), Err(SendError::Entropy));
        assert_eq!(sender.ping(b""), Err(SendError::Entropy));
        assert_eq!(sender.close(1000, ""), Err(SendError::Entropy));
        // The Close was not sent: the handshake has not begun.
        assert_eq!(sender.close_state(), CloseState::Open);
    }

    #[cfg(feature = "std")]
    #[test]
    fn ws_5_3_the_client_sender_draws_its_keys_from_the_operating_system() {
        let mut sender = Sender::client();
        let mut keys = Vec::new();
        for _ in 0..64 {
            let wire = sender.message(false, b"x").unwrap();
            keys.push(decode(&wire).unwrap().0.masking_key.expect("masked"));
        }
        keys.sort_unstable();
        keys.dedup();
        // 64 keys of 32 bits: two equal ones have a probability of 5e-7.
        assert!(keys.len() >= 63, "{} distinct keys of 64", keys.len());
    }

    #[cfg(feature = "std")]
    mod deflate {
        use super::*;
        use crate::permessage_deflate::PermessageDeflateParams;

        fn pair(params: &PermessageDeflateParams) -> (Sender, Receiver) {
            (
                Sender::server().with_permessage_deflate(params),
                Receiver::new(Role::Client, Limits::default()).with_permessage_deflate(params),
            )
        }

        #[test]
        fn rfc7692_6_1_the_frames_of_the_rfc_are_received_as_hello() {
            let params = PermessageDeflateParams::default();
            // §7.2.3.1 unfragmented, §7.2.3.3 a block without compression,
            // §7.2.3.4 a block with BFINAL, §7.2.3.5 two blocks.
            let frames: [&[u8]; 4] = [
                &[0xc1, 0x07, 0xf2, 0x48, 0xcd, 0xc9, 0xc9, 0x07, 0x00],
                &[
                    0xc1, 0x0b, 0x00, 0x05, 0x00, 0xfa, 0xff, 0x48, 0x65, 0x6c, 0x6c, 0x6f, 0x00,
                ],
                &[0xc1, 0x08, 0xf3, 0x48, 0xcd, 0xc9, 0xc9, 0x07, 0x00, 0x00],
                &[
                    0xc1, 0x0d, 0xf2, 0x48, 0x05, 0x00, 0x00, 0x00, 0xff, 0xff, 0xca, 0xc9, 0xc9,
                    0x07, 0x00,
                ],
            ];
            for wire in frames {
                let (_, mut client) = pair(&params);
                assert_eq!(
                    all_events(&mut client, wire),
                    [text("Hello")],
                    "{wire:02x?}"
                );
            }
            // §7.2.3.1 fragmented: RSV1 on the first frame only.
            let (_, mut client) = pair(&params);
            let wire = [
                0x41, 0x03, 0xf2, 0x48, 0xcd, 0x80, 0x04, 0xc9, 0xc9, 0x07, 0x00,
            ];
            assert_eq!(all_events(&mut client, &wire), [text("Hello")]);
        }

        #[test]
        fn rfc7692_7_2_3_2_the_second_hello_refers_to_the_window_of_the_first() {
            let (_, mut client) = pair(&PermessageDeflateParams::default());
            let mut wire = alloc::vec![0xc1u8, 0x07, 0xf2, 0x48, 0xcd, 0xc9, 0xc9, 0x07, 0x00];
            // An uncompressed message between the two leaves the window
            // alone.
            wire.extend_from_slice(&[0x81, 0x01, b'x']);
            wire.extend_from_slice(&[0xc1, 0x05, 0xf2, 0x00, 0x11, 0x00, 0x00]);
            assert_eq!(
                all_events(&mut client, &wire),
                [text("Hello"), text("x"), text("Hello")]
            );
            // With server_no_context_takeover the receiver has no window
            // to refer to: the second payload is invalid.
            let params = PermessageDeflateParams {
                server_no_takeover: true,
                ..PermessageDeflateParams::default()
            };
            let (_, mut client) = pair(&params);
            let mut buf = wire.clone();
            assert_eq!(client.feed(&mut buf), Ok(Some(text("Hello"))));
            assert_eq!(client.feed(&mut buf), Ok(Some(text("x"))));
            let failure = client.feed(&mut buf).expect_err("no window");
            assert_eq!(failure, Failure::Deflate(DeflateError::Corrupt));
            assert_eq!(failure.close_code().to_u16(), 1007);
        }

        #[test]
        fn rfc7692_6_1_rsv1_on_a_control_or_continuation_frame_fails_with_1002() {
            // §6.1: "An endpoint MUST NOT set the "Per-Message Compressed"
            // bit of control frames and non-first fragments of a data
            // message. An endpoint receiving such a frame MUST _Fail the
            // WebSocket Connection_."
            let params = PermessageDeflateParams::default();
            let (_, mut client) = pair(&params);
            assert_eq!(
                fails(&mut client, &[0xc9, 0x00], 1002),
                Failure::ReservedBit
            );
            let (_, mut client) = pair(&params);
            let wire = [
                0x41, 0x03, 0xf2, 0x48, 0xcd, 0xc0, 0x04, 0xc9, 0xc9, 0x07, 0x00,
            ];
            assert_eq!(fails(&mut client, &wire, 1002), Failure::ReservedBit);
            // RSV2 and RSV3 stay undefined.
            let (_, mut client) = pair(&params);
            assert_eq!(
                fails(&mut client, &[0xa1, 0x00], 1002),
                Failure::ReservedBit
            );
        }

        #[test]
        fn rfc7692_6_a_sent_message_is_compressed_and_received_again() {
            // The sender's octets through the receiver of the other end,
            // with and without context takeover, fragmented and not.
            let message: Vec<u8> = (0..20_000u32)
                .flat_map(|i| (i % 97).to_le_bytes())
                .collect();
            for (no_takeover, fragment) in [
                (false, usize::MAX),
                (true, usize::MAX),
                (false, 100),
                (true, 7),
            ] {
                let params = PermessageDeflateParams {
                    server_no_takeover: no_takeover,
                    ..PermessageDeflateParams::default()
                };
                let (sender, mut client) = pair(&params);
                let mut sender = sender.with_max_frame_payload(fragment);
                for round in 0..3 {
                    let wire = sender.message(false, &message).unwrap();
                    assert!(
                        wire.len() < message.len() / 4,
                        "round {round}: {}",
                        wire.len()
                    );
                    // RSV1 on the first frame, on no other.
                    let mut rest = &wire[..];
                    let mut first = true;
                    while !rest.is_empty() {
                        let (frame, used) = decode(rest).unwrap();
                        assert_eq!(frame.rsv1, first);
                        first = false;
                        rest = &rest[used..];
                    }
                    assert_eq!(
                        all_events(&mut client, &wire),
                        [Event::Message(Message {
                            is_text: false,
                            payload: message.clone(),
                        })]
                    );
                }
                // An empty message goes out as it is.
                assert_eq!(sender.message(true, b"").unwrap(), [0x81, 0x00]);
                // Control frames are not compressed (§6: "PMCEs operate
                // only on data messages").
                assert_eq!(sender.ping(b"hello").unwrap()[0], 0x89);
            }
        }

        #[test]
        fn rfc7692_7_2_1_context_takeover_makes_the_repeated_message_shorter() {
            let message =
                b"{\"topic\":\"Chat::Message\",\"op\":\"publish\",\"data\":{\"text\":\"hello\"}}";
            let (mut sender, _) = pair(&PermessageDeflateParams::default());
            let first = sender.message(true, message).unwrap().len();
            let second = sender.message(true, message).unwrap().len();
            assert!(second < first / 2, "{first} then {second}");
            // server_no_context_takeover: every message starts with an
            // empty window, the octets are the same each time.
            let params = PermessageDeflateParams {
                server_no_takeover: true,
                ..PermessageDeflateParams::default()
            };
            let (mut sender, _) = pair(&params);
            let first = sender.message(true, message).unwrap();
            let second = sender.message(true, message).unwrap();
            assert_eq!(first, second);
        }

        #[test]
        fn rfc7692_8_a_message_that_inflates_over_the_limit_fails_with_1009() {
            // A megabyte of zeros compresses to about a kilobyte. The
            // receiver's message limit is 64 KiB: it stops there.
            let (mut sender, _) = pair(&PermessageDeflateParams::default());
            let wire = sender.message(false, &alloc::vec![0u8; 1 << 20]).unwrap();
            assert!(wire.len() < 4096);
            let limits = Limits {
                max_frame_size: 1 << 20,
                max_message_size: 64 * 1024,
            };
            let mut client = Receiver::new(Role::Client, limits)
                .with_permessage_deflate(&PermessageDeflateParams::default());
            assert_eq!(
                fails(&mut client, &wire, 1009),
                Failure::Deflate(DeflateError::TooLarge)
            );
        }

        #[test]
        fn rfc7692_6_2_a_compressed_text_message_is_utf8_after_decompression() {
            // §6.1: the compressed payload is not subject to the UTF-8
            // rule, the decompressed message is.
            let params = PermessageDeflateParams::default();
            let mut deflater = Deflater::for_sender(Role::Server, &params).unwrap();
            let payload = deflater.compress(&[0xc3, 0x28]).unwrap();
            let wire = encode(&Frame {
                rsv1: true,
                opcode: Opcode::Text,
                ..Frame::binary(payload)
            })
            .unwrap();
            let (_, mut client) = pair(&params);
            let failure = fails(&mut client, &wire, 1007);
            assert!(matches!(
                failure,
                Failure::Message(ReceiveError::InvalidUtf8(_))
            ));
        }

        #[test]
        fn rfc7692_7_1_2_a_sender_with_a_window_of_8_bits_sends_uncompressed() {
            let params = PermessageDeflateParams {
                server_max_window_bits: 8,
                ..PermessageDeflateParams::default()
            };
            let (mut sender, mut client) = pair(&params);
            let wire = sender.message(true, b"Hello").unwrap();
            assert_eq!(wire, [0x81, 0x05, b'H', b'e', b'l', b'l', b'o']);
            assert_eq!(all_events(&mut client, &wire), [text("Hello")]);
            // The other direction is compressed.
            let mut client_tx = Sender::client().with_permessage_deflate(&params);
            let wire = client_tx.message(true, b"Hello").unwrap();
            assert_eq!(wire[0], 0xc1);
        }

        #[test]
        fn rfc7692_7_the_roles_use_the_parameters_of_their_direction() {
            // The client compresses with the client_ pair, the server
            // inflates with it; and the other way round.
            let params = PermessageDeflateParams {
                server_no_takeover: false,
                client_no_takeover: true,
                server_max_window_bits: 15,
                client_max_window_bits: 9,
            };
            let mut client_tx = Sender::client().with_permessage_deflate(&params);
            let mut server_rx =
                Receiver::new(Role::Server, Limits::default()).with_permessage_deflate(&params);
            let message: Vec<u8> = (0..5000u32).flat_map(|i| (i % 251).to_le_bytes()).collect();
            for _ in 0..3 {
                let wire = client_tx.message(false, &message).unwrap();
                assert_eq!(
                    all_events(&mut server_rx, &wire),
                    [Event::Message(Message {
                        is_text: false,
                        payload: message.clone(),
                    })]
                );
            }
        }
    }
}
