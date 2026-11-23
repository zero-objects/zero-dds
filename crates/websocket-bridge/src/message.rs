// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Message layer (§6.1 send algorithm + §6.2 receive algorithm).
//!
//! WebSocket frames are atomic; a logical message can, however, be
//! fragmented into multiple frames with FIN=0 continuation. We
//! encapsulate the send-sequencing and receive-reassembly logic here.

use alloc::vec::Vec;

use crate::close::CloseCode;
use crate::frame::{Frame, Opcode};
use crate::masking::MaskingKeyProvider;
use crate::utf8::{StreamingValidator, Utf8Error};

// ---------------------------------------------------------------------------
// §6.1 Sending Data — frame sequencing for continuations
// ---------------------------------------------------------------------------

/// Send errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendError {
    /// `max_frame_payload` is 0 (caller bug).
    InvalidFrameLimit,
    /// The text message is not valid UTF-8 (Spec §6.1).
    InvalidUtf8,
    /// The masking key source delivered no key (Spec §5.3): the frame is
    /// not sent.
    Entropy,
    /// A Close frame was sent or the connection is closed (Spec §5.5.1:
    /// "after sending a Close frame, an endpoint MUST NOT send any more
    /// data frames").
    Closing,
    /// A control frame with more than 125 octets of payload (Spec §5.5).
    ControlFrameTooLong,
    /// A status code that must not be sent in a Close frame (Spec §7.4).
    StatusNotAllowed,
    /// The message could not be compressed (RFC 7692 §7.2.1).
    Deflate,
}

impl core::fmt::Display for SendError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidFrameLimit => write!(f, "InvalidFrameLimit"),
            Self::InvalidUtf8 => write!(f, "InvalidUtf8"),
            Self::Entropy => write!(f, "Entropy"),
            Self::Closing => write!(f, "Closing"),
            Self::ControlFrameTooLong => write!(f, "ControlFrameTooLong"),
            Self::StatusNotAllowed => write!(f, "StatusNotAllowed"),
            Self::Deflate => write!(f, "Deflate"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for SendError {}

/// Send algorithm per Spec §6.1.
///
/// Splits a logical message into a frame sequence:
/// - First frame: `Text`/`Binary` opcode, FIN=0 if further frames
///   follow.
/// - Following frames: `Continuation` opcode, FIN=0 except the last.
/// - Last frame: FIN=1.
///
/// For text messages, UTF-8 validation is performed over the
/// entire payload.
///
/// `max_frame_payload` = 0 is rejected as `InvalidFrameLimit`;
/// `usize::MAX` effectively disables splitting.
///
/// `keys` is the client's masking key source: with `Some`, every frame is
/// masked with a key of its own, drawn for it (Spec §5.3, §10.3: "Clients
/// MUST choose a new masking key for each frame"); `None` is the server,
/// whose frames carry no mask (Spec §5.1).
///
/// # Errors
/// See [`SendError`].
pub fn fragment_message(
    is_text: bool,
    payload: &[u8],
    max_frame_payload: usize,
    keys: Option<&mut dyn MaskingKeyProvider>,
) -> Result<Vec<Frame>, SendError> {
    if is_text {
        crate::utf8::validate(payload).map_err(|_| SendError::InvalidUtf8)?;
    }
    let opcode = if is_text {
        Opcode::Text
    } else {
        Opcode::Binary
    };
    split_into_frames(opcode, false, payload, max_frame_payload, keys)
}

/// The frames of one message: `opcode` and `rsv1` on the first, the
/// continuation opcode on the others, FIN on the last, a masking key of
/// its own on each when `keys` is given. An empty payload is one frame
/// (Spec §5.6).
pub(crate) fn split_into_frames(
    opcode: Opcode,
    rsv1: bool,
    payload: &[u8],
    max_frame_payload: usize,
    mut keys: Option<&mut dyn MaskingKeyProvider>,
) -> Result<Vec<Frame>, SendError> {
    if max_frame_payload == 0 {
        return Err(SendError::InvalidFrameLimit);
    }
    let mut frames = Vec::with_capacity(payload.len() / max_frame_payload + 1);
    let mut offset = 0usize;
    loop {
        let chunk_end = offset.saturating_add(max_frame_payload).min(payload.len());
        let first = offset == 0;
        let masking_key = match keys.as_mut() {
            Some(k) => Some(k.next_key().map_err(|_| SendError::Entropy)?),
            None => None,
        };
        frames.push(Frame {
            fin: chunk_end == payload.len(),
            rsv1: rsv1 && first,
            rsv2: false,
            rsv3: false,
            opcode: if first { opcode } else { Opcode::Continuation },
            masking_key,
            payload: payload[offset..chunk_end].to_vec(),
        });
        offset = chunk_end;
        if offset == payload.len() {
            return Ok(frames);
        }
    }
}

// ---------------------------------------------------------------------------
// §6.2 Receiving Data — reassembly + UTF-8 validation
// ---------------------------------------------------------------------------

/// Receive errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReceiveError {
    /// Continuation frame without a preceding text/binary frame.
    UnexpectedContinuation,
    /// New text/binary frame during a continuation sequence.
    InterleavedDataFrame,
    /// UTF-8 error in a text message.
    InvalidUtf8(Utf8Error),
    /// The reassembly buffer exceeds `max_message_size`.
    MessageTooLarge,
}

impl ReceiveError {
    /// Spec §7.4.1 — the status code the connection is failed with.
    ///
    /// §8.1 and §8.2 demand that malformed UTF-8 in a text message
    /// fails the connection; §7.4.1 assigns 1007 "Invalid frame
    /// payload data" to exactly that case. A framing violation is a
    /// protocol error (1002), an oversized message is 1009.
    #[must_use]
    pub const fn close_code(&self) -> CloseCode {
        match self {
            Self::UnexpectedContinuation | Self::InterleavedDataFrame => CloseCode::ProtocolError,
            Self::InvalidUtf8(_) => CloseCode::InvalidPayloadData,
            Self::MessageTooLarge => CloseCode::MessageTooBig,
        }
    }

    /// The Close frame that fails the connection for this error
    /// (Spec §7.1 + §7.4.1).
    #[must_use]
    pub fn close_frame(&self) -> Frame {
        Frame::close(self.close_code().to_u16(), "")
    }
}

impl core::fmt::Display for ReceiveError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnexpectedContinuation => write!(f, "UnexpectedContinuation"),
            Self::InterleavedDataFrame => write!(f, "InterleavedDataFrame"),
            Self::InvalidUtf8(e) => write!(f, "InvalidUtf8({e})"),
            Self::MessageTooLarge => write!(f, "MessageTooLarge"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ReceiveError {}

/// Complete logical message (after reassembly).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    /// `true` if a `Text` message.
    pub is_text: bool,
    /// Total payload (concatenated after FIN=1).
    pub payload: Vec<u8>,
}

/// Receive-algorithm reassembler.
pub struct Reassembler {
    /// `Some` if a continuation sequence is active.
    pending: Option<PendingMessage>,
    /// Spec §10.1: the caller sets this value to avoid DoS.
    /// `usize::MAX` disables the limit.
    pub max_message_size: usize,
}

struct PendingMessage {
    is_text: bool,
    buffer: Vec<u8>,
    utf8: StreamingValidator,
}

impl core::fmt::Debug for Reassembler {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Reassembler")
            .field("pending", &self.pending.is_some())
            .field("max_message_size", &self.max_message_size)
            .finish()
    }
}

impl Default for Reassembler {
    fn default() -> Self {
        Self::new()
    }
}

impl Reassembler {
    /// Constructor with a `usize::MAX` limit (no DoS protection —
    /// the caller MUST set `max_message_size` if externally reachable).
    #[must_use]
    pub fn new() -> Self {
        Self {
            pending: None,
            max_message_size: usize::MAX,
        }
    }

    /// Constructor with an explicit DoS cap.
    #[must_use]
    pub fn with_limit(max_message_size: usize) -> Self {
        Self {
            pending: None,
            max_message_size,
        }
    }

    /// Spec §6.2 — process a frame and return a completed message if
    /// applicable (FIN=1 + valid UTF-8 for text).
    ///
    /// Control frames (Close/Ping/Pong) are NOT reassembled
    /// (Spec §5.5: control frames MUST NOT be fragmented). The caller
    /// handles them separately; we simply return them as
    /// `Ok(Some(_))` with `is_text=false`.
    ///
    /// # Errors
    /// See [`ReceiveError`].
    pub fn feed(&mut self, frame: &Frame) -> Result<Option<Message>, ReceiveError> {
        match frame.opcode {
            Opcode::Text | Opcode::Binary => {
                if self.pending.is_some() {
                    return Err(ReceiveError::InterleavedDataFrame);
                }
                let is_text = frame.opcode == Opcode::Text;
                if frame.fin {
                    // Single-frame message, validate UTF-8 inline.
                    if is_text {
                        crate::utf8::validate(&frame.payload).map_err(ReceiveError::InvalidUtf8)?;
                    }
                    if frame.payload.len() > self.max_message_size {
                        return Err(ReceiveError::MessageTooLarge);
                    }
                    Ok(Some(Message {
                        is_text,
                        payload: frame.payload.clone(),
                    }))
                } else {
                    let mut utf8 = StreamingValidator::new();
                    if is_text {
                        utf8.feed(&frame.payload)
                            .map_err(ReceiveError::InvalidUtf8)?;
                    }
                    if frame.payload.len() > self.max_message_size {
                        return Err(ReceiveError::MessageTooLarge);
                    }
                    self.pending = Some(PendingMessage {
                        is_text,
                        buffer: frame.payload.clone(),
                        utf8,
                    });
                    Ok(None)
                }
            }
            Opcode::Continuation => {
                let mut p = self
                    .pending
                    .take()
                    .ok_or(ReceiveError::UnexpectedContinuation)?;
                if p.is_text {
                    p.utf8
                        .feed(&frame.payload)
                        .map_err(ReceiveError::InvalidUtf8)?;
                }
                if p.buffer.len().saturating_add(frame.payload.len()) > self.max_message_size {
                    return Err(ReceiveError::MessageTooLarge);
                }
                p.buffer.extend_from_slice(&frame.payload);
                if frame.fin {
                    if p.is_text {
                        p.utf8.finalize().map_err(ReceiveError::InvalidUtf8)?;
                    }
                    Ok(Some(Message {
                        is_text: p.is_text,
                        payload: p.buffer,
                    }))
                } else {
                    self.pending = Some(p);
                    Ok(None)
                }
            }
            // Control frames: not fragmentable, return directly.
            Opcode::Close | Opcode::Ping | Opcode::Pong => Ok(Some(Message {
                is_text: false,
                payload: frame.payload.clone(),
            })),
            // Reserved opcodes (Spec §5.2): MUST result in failing
            // the WebSocket connection. We treat this as the
            // UnexpectedContinuation equivalent.
            Opcode::Reserved(_) => Err(ReceiveError::UnexpectedContinuation),
        }
    }

    /// The UTF-8 state of the text message in progress, after the frames
    /// fed so far; `None` when no text message is in progress. A receiver
    /// continues it over the octets of a frame that is not complete yet.
    pub(crate) fn text_state(&self) -> Option<StreamingValidator> {
        self.pending
            .as_ref()
            .filter(|p| p.is_text)
            .map(|p| p.utf8.clone())
    }

    /// The octets of the message in progress that are buffered.
    #[must_use]
    pub fn pending_len(&self) -> usize {
        self.pending.as_ref().map_or(0, |p| p.buffer.len())
    }

    /// `true` if a continuation sequence is active (a reassembly
    /// that is not yet complete).
    #[must_use]
    pub fn has_pending(&self) -> bool {
        self.pending.is_some()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::masking::EntropyError;

    #[test]
    fn ws_6_1_fragment_empty_message_yields_one_frame() {
        let f = fragment_message(true, b"", 100, None).expect("ok");
        assert_eq!(f.len(), 1);
        assert!(f[0].fin);
        assert_eq!(f[0].opcode, Opcode::Text);
    }

    #[test]
    fn ws_6_1_fragment_message_within_limit_single_frame() {
        let f = fragment_message(false, b"hello", 100, None).expect("ok");
        assert_eq!(f.len(), 1);
        assert!(f[0].fin);
        assert_eq!(f[0].opcode, Opcode::Binary);
    }

    #[test]
    fn ws_6_1_fragment_message_splits_into_text_plus_continuations() {
        let f = fragment_message(true, b"abcdefghij", 3, None).expect("ok");
        assert_eq!(f.len(), 4);
        assert_eq!(f[0].opcode, Opcode::Text);
        assert!(!f[0].fin);
        assert_eq!(f[1].opcode, Opcode::Continuation);
        assert_eq!(f[2].opcode, Opcode::Continuation);
        assert_eq!(f[3].opcode, Opcode::Continuation);
        assert!(f[3].fin);
    }

    #[test]
    fn ws_6_1_fragment_text_rejects_invalid_utf8() {
        let bad = [0xff, 0xfe];
        assert_eq!(
            fragment_message(true, &bad, 100, None),
            Err(SendError::InvalidUtf8)
        );
    }

    #[test]
    fn ws_6_1_fragment_zero_limit_rejected() {
        assert_eq!(
            fragment_message(false, b"x", 0, None),
            Err(SendError::InvalidFrameLimit)
        );
    }

    /// A key source that counts: key `n` is the number `n`.
    fn counting_keys()
    -> crate::masking::ClosureMaskingKeyProvider<impl FnMut() -> Result<[u8; 4], EntropyError>>
    {
        let mut n = 0u32;
        crate::masking::ClosureMaskingKeyProvider(move || {
            let key = n.to_be_bytes();
            n += 1;
            Ok(key)
        })
    }

    #[test]
    fn ws_6_1_fragment_with_mask_sets_mask_field() {
        let mut keys = crate::masking::ClosureMaskingKeyProvider(|| Ok([1, 2, 3, 4]));
        let f = fragment_message(false, b"x", 100, Some(&mut keys)).expect("ok");
        assert_eq!(f[0].masking_key, Some([1, 2, 3, 4]));
        // The server's frames carry none (§5.1).
        let f = fragment_message(false, b"x", 100, None).expect("ok");
        assert_eq!(f[0].masking_key, None);
    }

    #[test]
    fn ws_10_3_each_frame_of_a_fragmented_message_has_its_own_key() {
        // §10.3: "Clients MUST choose a new masking key for each frame".
        // Four frames, four draws, four keys.
        let mut keys = counting_keys();
        let f = fragment_message(true, b"abcdefghij", 3, Some(&mut keys)).expect("ok");
        let drawn: Vec<Option<[u8; 4]>> = f.iter().map(|f| f.masking_key).collect();
        assert_eq!(
            drawn,
            [
                Some([0, 0, 0, 0]),
                Some([0, 0, 0, 1]),
                Some([0, 0, 0, 2]),
                Some([0, 0, 0, 3])
            ]
        );
    }

    #[test]
    fn ws_10_3_a_zero_key_still_sets_the_mask_bit() {
        // The key 00 00 00 00 is a key like any other: the frame goes out
        // with MASK set and the four key octets (§5.2), not unmasked.
        let mut keys = counting_keys();
        let f = fragment_message(false, b"x", 100, Some(&mut keys)).expect("ok");
        assert_eq!(f[0].masking_key, Some([0; 4]));
        let wire = crate::codec::encode(&f[0]).expect("encode");
        assert_eq!(wire, [0x82, 0x81, 0, 0, 0, 0, b'x']);
    }

    #[test]
    fn ws_5_3_without_a_key_no_frame_is_built() {
        let mut keys = crate::masking::ClosureMaskingKeyProvider(|| Err(EntropyError));
        assert_eq!(
            fragment_message(false, b"x", 100, Some(&mut keys)),
            Err(SendError::Entropy)
        );
    }

    fn binary_frame(fin: bool, opcode: Opcode, payload: Vec<u8>) -> Frame {
        Frame {
            fin,
            rsv1: false,
            rsv2: false,
            rsv3: false,
            opcode,
            masking_key: None,
            payload,
        }
    }

    #[test]
    fn ws_6_2_reassembler_single_frame_message_complete() {
        let mut r = Reassembler::new();
        let msg = r
            .feed(&binary_frame(true, Opcode::Text, b"hello".to_vec()))
            .expect("ok")
            .expect("complete");
        assert!(msg.is_text);
        assert_eq!(msg.payload, b"hello");
    }

    #[test]
    fn ws_6_2_reassembler_continuation_sequence_reassembles() {
        let mut r = Reassembler::new();
        let p1 = r
            .feed(&binary_frame(false, Opcode::Text, b"hel".to_vec()))
            .expect("ok");
        assert!(p1.is_none());
        let p2 = r
            .feed(&binary_frame(false, Opcode::Continuation, b"lo ".to_vec()))
            .expect("ok");
        assert!(p2.is_none());
        let msg = r
            .feed(&binary_frame(true, Opcode::Continuation, b"world".to_vec()))
            .expect("ok")
            .expect("complete");
        assert_eq!(msg.payload, b"hello world");
    }

    #[test]
    fn ws_6_2_reassembler_continuation_without_preceding_text_rejected() {
        let mut r = Reassembler::new();
        assert_eq!(
            r.feed(&binary_frame(true, Opcode::Continuation, b"x".to_vec())),
            Err(ReceiveError::UnexpectedContinuation)
        );
    }

    #[test]
    fn ws_6_2_reassembler_interleaved_text_during_pending_rejected() {
        let mut r = Reassembler::new();
        let _ = r
            .feed(&binary_frame(false, Opcode::Text, b"hel".to_vec()))
            .expect("ok");
        assert_eq!(
            r.feed(&binary_frame(false, Opcode::Text, b"new".to_vec())),
            Err(ReceiveError::InterleavedDataFrame)
        );
    }

    #[test]
    fn ws_6_2_reassembler_rejects_invalid_utf8_in_text() {
        let mut r = Reassembler::new();
        let result = r.feed(&binary_frame(true, Opcode::Text, alloc::vec![0xff]));
        assert!(matches!(result, Err(ReceiveError::InvalidUtf8(_))));
    }

    #[test]
    fn ws_6_2_reassembler_rejects_message_above_limit() {
        let mut r = Reassembler::with_limit(5);
        let result = r.feed(&binary_frame(true, Opcode::Binary, alloc::vec![0; 10]));
        assert_eq!(result, Err(ReceiveError::MessageTooLarge));
    }

    #[test]
    fn ws_6_2_reassembler_passes_through_control_frames() {
        let mut r = Reassembler::new();
        let msg = r
            .feed(&binary_frame(true, Opcode::Ping, b"abc".to_vec()))
            .expect("ok")
            .expect("ping");
        assert!(!msg.is_text);
        assert_eq!(msg.payload, b"abc");
    }

    #[test]
    fn ws_6_2_reassembler_has_pending_during_continuation() {
        let mut r = Reassembler::new();
        let _ = r
            .feed(&binary_frame(false, Opcode::Binary, b"x".to_vec()))
            .expect("ok");
        assert!(r.has_pending());
    }

    #[test]
    fn ws_6_2_fragment_send_then_reassemble_round_trip() {
        let original = b"the quick brown fox jumps";
        let frames = fragment_message(true, original, 4, None).expect("ok");
        let mut r = Reassembler::new();
        let mut completed: Option<Message> = None;
        for f in &frames {
            if let Some(m) = r.feed(f).expect("ok") {
                completed = Some(m);
            }
        }
        let msg = completed.expect("completed");
        assert_eq!(msg.payload, original);
    }

    #[test]
    fn ws_8_2_server_fails_connection_with_1007_on_malformed_utf8_from_client() {
        // Spec §8.2: "When a server receives a Text-frame from the
        // client that contains malformed UTF-8 data [...] the server
        // MUST fail the WebSocket Connection." §7.4.1 gives 1007
        // "Invalid frame payload data" as the status code.
        //
        // A client frame is masked (§5.3); the codec unmasks on
        // decode, so the reassembler sees the raw — invalid — bytes.
        let bad = alloc::vec![0x48, 0x69, 0xC3, 0x28]; // "Hi" + overlong pair
        let masked = Frame {
            fin: true,
            rsv1: false,
            rsv2: false,
            rsv3: false,
            opcode: Opcode::Text,
            masking_key: Some([0x37, 0xfa, 0x21, 0x3d]),
            payload: bad,
        };
        let wire = crate::codec::encode(&masked).expect("encode");
        let received = crate::codec::decode(&wire).expect("decode").0;
        assert_eq!(
            crate::masking::validate_masking(&received, crate::masking::Role::Server),
            Ok(()),
            "a client frame is masked"
        );
        let mut r = Reassembler::new();
        let err = r.feed(&received).expect_err("malformed UTF-8 fails");
        assert!(matches!(err, ReceiveError::InvalidUtf8(_)));
        assert_eq!(err.close_code(), CloseCode::InvalidPayloadData);
        assert_eq!(err.close_code().to_u16(), 1007);
        // The close frame put on the wire carries 1007 big-endian.
        let close = err.close_frame();
        assert_eq!(close.opcode, Opcode::Close);
        assert_eq!(&close.payload[..2], &1007u16.to_be_bytes());
    }

    #[test]
    fn ws_8_1_client_fails_connection_with_1007_on_malformed_utf8_from_server() {
        // Spec §8.1, the mirror image: a server frame is unmasked and
        // the client fails the connection with the same status code.
        let server_frame = Frame {
            fin: true,
            rsv1: false,
            rsv2: false,
            rsv3: false,
            opcode: Opcode::Text,
            masking_key: None,
            payload: alloc::vec![0xED, 0xA0, 0x80], // U+D800 surrogate
        };
        assert_eq!(
            crate::masking::validate_masking(&server_frame, crate::masking::Role::Client),
            Ok(()),
            "a server frame is not masked"
        );
        let mut r = Reassembler::new();
        let err = r.feed(&server_frame).expect_err("surrogate is rejected");
        assert!(matches!(err, ReceiveError::InvalidUtf8(_)));
        assert_eq!(err.close_code().to_u16(), 1007);
    }

    #[test]
    fn ws_8_2_malformed_utf8_split_across_continuation_frames_still_yields_1007() {
        // §8.1/§8.2 apply to a fragmented text message too — the
        // streaming validator catches the invalid sequence at the
        // fragment boundary.
        let first = Frame {
            fin: false,
            rsv1: false,
            rsv2: false,
            rsv3: false,
            opcode: Opcode::Text,
            masking_key: None,
            payload: alloc::vec![0xE2, 0x82], // start of U+20AC
        };
        let second = Frame {
            fin: true,
            rsv1: false,
            rsv2: false,
            rsv3: false,
            opcode: Opcode::Continuation,
            masking_key: None,
            payload: alloc::vec![0x28], // invalid continuation byte
        };
        let mut r = Reassembler::new();
        assert!(
            r.feed(&first)
                .expect("first fragment is still open")
                .is_none()
        );
        let err = r.feed(&second).expect_err("the sequence is invalid");
        assert_eq!(err.close_code().to_u16(), 1007);
    }

    #[test]
    fn ws_8_2_receive_error_close_codes_match_spec_7_4_1() {
        assert_eq!(
            ReceiveError::UnexpectedContinuation.close_code().to_u16(),
            1002
        );
        assert_eq!(
            ReceiveError::InterleavedDataFrame.close_code().to_u16(),
            1002
        );
        assert_eq!(ReceiveError::MessageTooLarge.close_code().to_u16(), 1009);
    }
}
