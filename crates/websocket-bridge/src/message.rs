// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Message layer (§6.1 send algorithm + §6.2 receive algorithm).
//!
//! WebSocket frames are atomic; a logical message can, however, be
//! fragmented into multiple frames with FIN=0 continuation. We
//! encapsulate the send-sequencing and receive-reassembly logic here.

use alloc::vec::Vec;

use crate::frame::{Frame, Opcode};
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
}

impl core::fmt::Display for SendError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidFrameLimit => write!(f, "InvalidFrameLimit"),
            Self::InvalidUtf8 => write!(f, "InvalidUtf8"),
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
/// `mask` (4-byte) is applied to each frame if non-zero
/// (client path). The server path passes `[0;4]` as the mask or
/// `apply_mask=false` (handled via the empty mask).
///
/// # Errors
/// See [`SendError`].
pub fn fragment_message(
    is_text: bool,
    payload: &[u8],
    max_frame_payload: usize,
    mask: [u8; 4],
) -> Result<Vec<Frame>, SendError> {
    if max_frame_payload == 0 {
        return Err(SendError::InvalidFrameLimit);
    }
    if is_text {
        crate::utf8::validate(payload).map_err(|_| SendError::InvalidUtf8)?;
    }

    if payload.is_empty() {
        // Spec §5.6 allows empty text/binary messages → one frame
        // with FIN=1, empty payload.
        return Ok(alloc::vec![Frame {
            fin: true,
            rsv1: false,
            rsv2: false,
            rsv3: false,
            opcode: if is_text {
                Opcode::Text
            } else {
                Opcode::Binary
            },
            masking_key: if mask == [0; 4] { None } else { Some(mask) },
            payload: alloc::vec![],
        }]);
    }

    let mut frames = Vec::new();
    let mut offset = 0;
    let mut first = true;

    while offset < payload.len() {
        let chunk_end = (offset + max_frame_payload).min(payload.len());
        let chunk = &payload[offset..chunk_end];
        let is_last = chunk_end == payload.len();

        let opcode = if first {
            if is_text {
                Opcode::Text
            } else {
                Opcode::Binary
            }
        } else {
            Opcode::Continuation
        };

        frames.push(Frame {
            fin: is_last,
            rsv1: false,
            rsv2: false,
            rsv3: false,
            opcode,
            masking_key: if mask == [0; 4] { None } else { Some(mask) },
            payload: chunk.to_vec(),
        });

        offset = chunk_end;
        first = false;
    }

    Ok(frames)
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

    #[test]
    fn fragment_empty_message_yields_one_frame() {
        let f = fragment_message(true, b"", 100, [0; 4]).expect("ok");
        assert_eq!(f.len(), 1);
        assert!(f[0].fin);
        assert_eq!(f[0].opcode, Opcode::Text);
    }

    #[test]
    fn fragment_message_within_limit_single_frame() {
        let f = fragment_message(false, b"hello", 100, [0; 4]).expect("ok");
        assert_eq!(f.len(), 1);
        assert!(f[0].fin);
        assert_eq!(f[0].opcode, Opcode::Binary);
    }

    #[test]
    fn fragment_message_splits_into_text_plus_continuations() {
        let f = fragment_message(true, b"abcdefghij", 3, [0; 4]).expect("ok");
        assert_eq!(f.len(), 4);
        assert_eq!(f[0].opcode, Opcode::Text);
        assert!(!f[0].fin);
        assert_eq!(f[1].opcode, Opcode::Continuation);
        assert_eq!(f[2].opcode, Opcode::Continuation);
        assert_eq!(f[3].opcode, Opcode::Continuation);
        assert!(f[3].fin);
    }

    #[test]
    fn fragment_text_rejects_invalid_utf8() {
        let bad = [0xff, 0xfe];
        assert_eq!(
            fragment_message(true, &bad, 100, [0; 4]),
            Err(SendError::InvalidUtf8)
        );
    }

    #[test]
    fn fragment_zero_limit_rejected() {
        assert_eq!(
            fragment_message(false, b"x", 0, [0; 4]),
            Err(SendError::InvalidFrameLimit)
        );
    }

    #[test]
    fn fragment_with_mask_sets_mask_field() {
        let f = fragment_message(false, b"x", 100, [1, 2, 3, 4]).expect("ok");
        assert_eq!(f[0].masking_key, Some([1, 2, 3, 4]));
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
    fn reassembler_single_frame_message_complete() {
        let mut r = Reassembler::new();
        let msg = r
            .feed(&binary_frame(true, Opcode::Text, b"hello".to_vec()))
            .expect("ok")
            .expect("complete");
        assert!(msg.is_text);
        assert_eq!(msg.payload, b"hello");
    }

    #[test]
    fn reassembler_continuation_sequence_reassembles() {
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
    fn reassembler_continuation_without_preceding_text_rejected() {
        let mut r = Reassembler::new();
        assert_eq!(
            r.feed(&binary_frame(true, Opcode::Continuation, b"x".to_vec())),
            Err(ReceiveError::UnexpectedContinuation)
        );
    }

    #[test]
    fn reassembler_interleaved_text_during_pending_rejected() {
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
    fn reassembler_rejects_invalid_utf8_in_text() {
        let mut r = Reassembler::new();
        let result = r.feed(&binary_frame(true, Opcode::Text, alloc::vec![0xff]));
        assert!(matches!(result, Err(ReceiveError::InvalidUtf8(_))));
    }

    #[test]
    fn reassembler_rejects_message_above_limit() {
        let mut r = Reassembler::with_limit(5);
        let result = r.feed(&binary_frame(true, Opcode::Binary, alloc::vec![0; 10]));
        assert_eq!(result, Err(ReceiveError::MessageTooLarge));
    }

    #[test]
    fn reassembler_passes_through_control_frames() {
        let mut r = Reassembler::new();
        let msg = r
            .feed(&binary_frame(true, Opcode::Ping, b"abc".to_vec()))
            .expect("ok")
            .expect("ping");
        assert!(!msg.is_text);
        assert_eq!(msg.payload, b"abc");
    }

    #[test]
    fn reassembler_has_pending_during_continuation() {
        let mut r = Reassembler::new();
        let _ = r
            .feed(&binary_frame(false, Opcode::Binary, b"x".to_vec()))
            .expect("ok");
        assert!(r.has_pending());
    }

    #[test]
    fn fragment_send_then_reassemble_round_trip() {
        let original = b"the quick brown fox jumps";
        let frames = fragment_message(true, original, 4, [0; 4]).expect("ok");
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
}
