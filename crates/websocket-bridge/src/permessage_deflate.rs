// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! permessage-deflate extension — RFC 7692 §7.
//!
//! Spec §7: extension negotiation via the `Sec-WebSocket-Extensions` header.
//! Four parameters:
//! * `server_no_context_takeover` (boolean)
//! * `client_no_context_takeover` (boolean)
//! * `server_max_window_bits` (8..15)
//! * `client_max_window_bits` (8..15)
//!
//! Wire format: the RSV1 bit in the frame header of the first frame marks a
//! compressed message (Spec §6). The message payload is DEFLATE-compressed
//! with the tail `00 00 FF FF` stripped (Spec §7.2.1).
//!
//! The module holds the negotiation of both sides ([`parse_deflate_offer`],
//! [`agree`], [`accept_offers`] for the server, [`render_offer`] and
//! [`accept_response`] for the client) and, with `std`, the payload
//! transformation of §7.2: [`Deflater`] and [`Inflater`].
//! [`connection`](crate::connection) applies them in the frame path.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Spec §7 tail marker — stripped on send, appended on receive.
pub const DEFLATE_TAIL: [u8; 4] = [0x00, 0x00, 0xff, 0xff];

/// The registered name of the extension (Spec §7).
pub const EXTENSION_NAME: &str = "permessage-deflate";

/// Negotiation parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PermessageDeflateParams {
    /// `server_no_context_takeover`.
    pub server_no_takeover: bool,
    /// `client_no_context_takeover`.
    pub client_no_takeover: bool,
    /// `server_max_window_bits` (8..=15).
    pub server_max_window_bits: u8,
    /// `client_max_window_bits` (8..=15).
    pub client_max_window_bits: u8,
}

impl Default for PermessageDeflateParams {
    fn default() -> Self {
        Self {
            server_no_takeover: false,
            client_no_takeover: false,
            server_max_window_bits: 15,
            client_max_window_bits: 15,
        }
    }
}

/// Negotiation error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NegotiationError {
    /// Unknown parameter.
    UnknownParam(String),
    /// Window bits outside 8..=15.
    InvalidWindowBits(u8),
    /// A boolean parameter has a value (should be parameterless).
    BooleanWithValue(String),
    /// A parameter appears more than once (Spec §7).
    DuplicateParam(String),
    /// A response sets a parameter in a way the offer excludes (Spec §7:
    /// "the client does not support the configuration that the response
    /// represents").
    NotOffered(String),
}

impl core::fmt::Display for NegotiationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnknownParam(p) => write!(f, "unknown parameter: {p}"),
            Self::InvalidWindowBits(b) => write!(f, "invalid window_bits: {b}"),
            Self::BooleanWithValue(p) => write!(f, "boolean param `{p}` has value"),
            Self::DuplicateParam(p) => write!(f, "parameter `{p}` appears twice"),
            Self::NotOffered(p) => write!(f, "`{p}` in the response contradicts the offer"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for NegotiationError {}

/// One `permessage-deflate` element of a client's
/// `Sec-WebSocket-Extensions` offer (Spec §7.1), as the client wrote it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DeflateOffer {
    /// `server_no_context_takeover`: the server must not use context
    /// takeover (§7.1.1.1).
    pub server_no_context_takeover: bool,
    /// `client_no_context_takeover`: a hint that the client will not use
    /// context takeover (§7.1.1.2).
    pub client_no_context_takeover: bool,
    /// `server_max_window_bits=<n>`: the largest LZ77 window the server may
    /// compress with (§7.1.2.1).
    pub server_max_window_bits: Option<u8>,
    /// `client_max_window_bits`: `None` when the offer does not carry the
    /// parameter, `Some(None)` without a value, `Some(Some(n))` with one
    /// (§7.1.2.2).
    pub client_max_window_bits: Option<Option<u8>>,
}

/// `1*DIGIT` "without leading zeroes" between 8 and 15 (Spec §7.1.2).
fn window_bits(v: &str) -> Result<u8, NegotiationError> {
    let plain = !v.is_empty() && v.len() <= 2 && v.bytes().all(|b| b.is_ascii_digit());
    if !plain || v.starts_with('0') {
        return Err(NegotiationError::InvalidWindowBits(0));
    }
    let bits: u8 = v
        .parse()
        .map_err(|_| NegotiationError::InvalidWindowBits(0))?;
    if !(8..=15).contains(&bits) {
        return Err(NegotiationError::InvalidWindowBits(bits));
    }
    Ok(bits)
}

/// The parameters of one `permessage-deflate` element, each at most once
/// (Spec §7: a parameter that is not defined, has an invalid value or
/// appears twice declines the offer, or fails the connection when it is in
/// a response). `bare_client_bits` says whether `client_max_window_bits`
/// may come without a value: in an offer yes, in a response no.
fn parse_element(element: &str, bare_client_bits: bool) -> Result<DeflateOffer, NegotiationError> {
    let mut parts = element.split(';');
    let name = parts.next().unwrap_or_default().trim();
    if !name.eq_ignore_ascii_case(EXTENSION_NAME) {
        return Err(NegotiationError::UnknownParam(name.to_string()));
    }
    let mut out = DeflateOffer::default();
    let mut seen: Vec<&str> = Vec::new();
    for part in parts {
        let part = part.trim();
        if part.is_empty() {
            return Err(NegotiationError::UnknownParam(String::new()));
        }
        let (k, v) = match part.split_once('=') {
            Some((k, v)) => {
                let v = v.trim();
                // A value is a token or a quoted string that holds one
                // (RFC 6455 §9.1).
                let v = v
                    .strip_prefix('"')
                    .and_then(|v| v.strip_suffix('"'))
                    .unwrap_or(v);
                (k.trim(), Some(v))
            }
            None => (part, None),
        };
        if seen.contains(&k) {
            return Err(NegotiationError::DuplicateParam(k.to_string()));
        }
        match (k, v) {
            ("server_no_context_takeover", None) => out.server_no_context_takeover = true,
            ("client_no_context_takeover", None) => out.client_no_context_takeover = true,
            ("server_no_context_takeover" | "client_no_context_takeover", Some(_)) => {
                return Err(NegotiationError::BooleanWithValue(k.to_string()));
            }
            ("server_max_window_bits", Some(v)) => {
                out.server_max_window_bits = Some(window_bits(v)?);
            }
            ("server_max_window_bits", None) => {
                return Err(NegotiationError::InvalidWindowBits(0));
            }
            ("client_max_window_bits", Some(v)) => {
                out.client_max_window_bits = Some(Some(window_bits(v)?));
            }
            ("client_max_window_bits", None) if bare_client_bits => {
                out.client_max_window_bits = Some(None);
            }
            ("client_max_window_bits", None) => {
                return Err(NegotiationError::InvalidWindowBits(0));
            }
            (other, _) => return Err(NegotiationError::UnknownParam(other.to_string())),
        }
        seen.push(k);
    }
    Ok(out)
}

/// Parses one `permessage-deflate` element of a client's offer, e.g.
/// `permessage-deflate; client_max_window_bits; server_max_window_bits=10`
/// (Spec §7.1).
///
/// # Errors
/// [`NegotiationError`] for what makes a server decline the offer (Spec
/// §7): an undefined parameter, an invalid value, a parameter twice.
pub fn parse_deflate_offer(element: &str) -> Result<DeflateOffer, NegotiationError> {
    parse_element(element, true)
}

/// What this implementation, as the server, answers to one offer (Spec
/// §7.1): the agreed parameters.
///
/// * `server_no_context_takeover` is agreed to when asked for (§7.1.1.1).
/// * `client_no_context_takeover` is put into the response when the client
///   named it: the client does not take its context over anyway, and the
///   server then keeps no window between messages (§7.1.1.2).
/// * `server_max_window_bits=n` is agreed to with the same value. For 8
///   the server keeps to it by sending its messages uncompressed
///   ([`Deflater::new`]); the client's messages are compressed all the
///   same.
/// * `client_max_window_bits=n` is answered with the same value, which
///   lets the server inflate with a window of that size; without a value
///   the response leaves the parameter out (the client may use up to 15
///   bits), and so it does when the offer does not carry it (§7.1.2.2:
///   the response MUST NOT include it then).
#[must_use]
pub fn agree(offer: &DeflateOffer) -> PermessageDeflateParams {
    PermessageDeflateParams {
        server_no_takeover: offer.server_no_context_takeover,
        client_no_takeover: offer.client_no_context_takeover,
        server_max_window_bits: offer.server_max_window_bits.unwrap_or(15),
        client_max_window_bits: offer.client_max_window_bits.flatten().unwrap_or(15),
    }
}

/// The server's choice among the elements of `Sec-WebSocket-Extensions`
/// (RFC 6455 §9.1: the client lists them in order of preference): the
/// parameters [`agree`] gives the first `permessage-deflate` offer that is
/// valid. An offer that is not is declined (Spec §7), and the next one is
/// looked at (Spec §5.1); elements of other extensions are not this
/// module's. `None`: no offer is accepted, the response names no
/// `permessage-deflate`.
#[must_use]
pub fn accept_offers<S: AsRef<str>>(elements: &[S]) -> Option<PermessageDeflateParams> {
    elements
        .iter()
        .map(AsRef::as_ref)
        .filter(|e| {
            e.split(';')
                .next()
                .is_some_and(|n| n.trim().eq_ignore_ascii_case(EXTENSION_NAME))
        })
        .find_map(|e| parse_deflate_offer(e).ok().as_ref().map(agree))
}

/// The `Sec-WebSocket-Extensions` element of a client's offer (Spec §7.1).
#[must_use]
pub fn render_offer(offer: &DeflateOffer) -> String {
    let mut s = String::from(EXTENSION_NAME);
    if offer.server_no_context_takeover {
        s.push_str("; server_no_context_takeover");
    }
    if offer.client_no_context_takeover {
        s.push_str("; client_no_context_takeover");
    }
    if let Some(bits) = offer.server_max_window_bits {
        s.push_str(&alloc::format!("; server_max_window_bits={bits}"));
    }
    match offer.client_max_window_bits {
        Some(Some(bits)) => s.push_str(&alloc::format!("; client_max_window_bits={bits}")),
        Some(None) => s.push_str("; client_max_window_bits"),
        None => {}
    }
    s
}

/// The client's reading of the server's response to `offer` (Spec §7): the
/// agreed parameters.
///
/// # Errors
/// [`NegotiationError`] for a response on which the client MUST fail the
/// connection: an undefined parameter, an invalid value, a parameter
/// twice, or a configuration the offer excludes — the offer asked for
/// `server_no_context_takeover` and the response lacks it, the response's
/// `server_max_window_bits` is larger than the offered one or missing
/// where one was offered, or it names `client_max_window_bits` although
/// the offer did not (§7.1.2.2).
pub fn accept_response(
    offer: &DeflateOffer,
    response: &str,
) -> Result<PermessageDeflateParams, NegotiationError> {
    let r = parse_element(response, false)?;
    if offer.server_no_context_takeover && !r.server_no_context_takeover {
        return Err(NegotiationError::NotOffered(
            "server_no_context_takeover".to_string(),
        ));
    }
    if let Some(asked) = offer.server_max_window_bits {
        match r.server_max_window_bits {
            Some(bits) if bits <= asked => {}
            _ => {
                return Err(NegotiationError::NotOffered(
                    "server_max_window_bits".to_string(),
                ));
            }
        }
    }
    if r.client_max_window_bits.is_some() && offer.client_max_window_bits.is_none() {
        return Err(NegotiationError::NotOffered(
            "client_max_window_bits".to_string(),
        ));
    }
    Ok(PermessageDeflateParams {
        server_no_takeover: r.server_no_context_takeover,
        client_no_takeover: r.client_no_context_takeover,
        server_max_window_bits: r.server_max_window_bits.unwrap_or(15),
        client_max_window_bits: r.client_max_window_bits.flatten().unwrap_or(15),
    })
}

/// Parses a `Sec-WebSocket-Extensions` value. Spec §7.1.
///
/// Format: `permessage-deflate; param1; param2=value; ...`. The result
/// holds the window sizes with their default of 15 where the element names
/// none; [`parse_deflate_offer`] keeps what the element said.
///
/// # Errors
/// See [`NegotiationError`].
pub fn parse_offer(offer: &str) -> Result<PermessageDeflateParams, NegotiationError> {
    let o = parse_deflate_offer(offer)?;
    Ok(PermessageDeflateParams {
        server_no_takeover: o.server_no_context_takeover,
        client_no_takeover: o.client_no_context_takeover,
        server_max_window_bits: o.server_max_window_bits.unwrap_or(15),
        client_max_window_bits: o.client_max_window_bits.flatten().unwrap_or(15),
    })
}

/// Render the server accept header value.
#[must_use]
pub fn render_accept(params: &PermessageDeflateParams) -> String {
    let mut s = String::from("permessage-deflate");
    if params.server_no_takeover {
        s.push_str("; server_no_context_takeover");
    }
    if params.client_no_takeover {
        s.push_str("; client_no_context_takeover");
    }
    if params.server_max_window_bits != 15 {
        s.push_str(&alloc::format!(
            "; server_max_window_bits={}",
            params.server_max_window_bits
        ));
    }
    if params.client_max_window_bits != 15 {
        s.push_str(&alloc::format!(
            "; client_max_window_bits={}",
            params.client_max_window_bits
        ));
    }
    s
}

/// Append the tail after DEFLATE decompression. Spec §7.2.2.
#[must_use]
pub fn append_tail(payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(payload.len() + 4);
    out.extend_from_slice(payload);
    out.extend_from_slice(&DEFLATE_TAIL);
    out
}

/// Strip the tail before sending compressed frames. Spec §7.2.1.
#[must_use]
pub fn strip_tail(payload: &[u8]) -> &[u8] {
    if payload.ends_with(&DEFLATE_TAIL) {
        &payload[..payload.len() - DEFLATE_TAIL.len()]
    } else {
        payload
    }
}

// ---------------------------------------------------------------------------
// §7.2 Message Payload Transformation
// ---------------------------------------------------------------------------

/// A message could not be compressed or decompressed (Spec §7.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeflateError {
    /// The payload is not DEFLATE data (Spec §7.2.2). The connection is
    /// failed with 1007: the payload does not fit the message type.
    Corrupt,
    /// The decompressed message is longer than the limit the caller gave
    /// (RFC 6455 §10.4). The connection is failed with 1009.
    TooLarge,
    /// The compressor did not end the message at an empty block without
    /// compression (Spec §7.2.1 step 2).
    NoTail,
}

impl core::fmt::Display for DeflateError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Corrupt => f.write_str("the payload is not DEFLATE data"),
            Self::TooLarge => f.write_str("the decompressed message exceeds the limit"),
            Self::NoTail => f.write_str("the compressed data has no 00 00 ff ff tail"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for DeflateError {}

/// The window the decompressor is opened with for `bits` of the
/// negotiation. Its range is 9 to 15; a stream of an 8-bit window is
/// inflated with the 9-bit one, which holds every distance the smaller one
/// allows.
#[cfg(feature = "std")]
const fn inflate_window(bits: u8) -> u8 {
    if bits < 9 {
        9
    } else if bits > 15 {
        15
    } else {
        bits
    }
}

/// The compressor of one direction of a connection (Spec §7.2.1): raw
/// DEFLATE with the negotiated LZ77 window, taken over from message to
/// message unless the agreed parameters forbid it.
#[cfg(feature = "std")]
pub struct Deflater {
    z: flate2::Compress,
    no_context_takeover: bool,
    scratch: Vec<u8>,
}

#[cfg(feature = "std")]
impl core::fmt::Debug for Deflater {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Deflater")
            .field("no_context_takeover", &self.no_context_takeover)
            .finish_non_exhaustive()
    }
}

#[cfg(feature = "std")]
impl Deflater {
    /// A compressor with an LZ77 window of `2^window_bits` octets, 9 to
    /// 15. With `no_context_takeover` each message starts with an empty
    /// window.
    ///
    /// `None` for 8 bits (and for a value outside the 8 to 15 of Spec
    /// §7.1.2): the DEFLATE compressor has no 256-octet window, and the
    /// next larger one is what the parameter forbids. A sender without a
    /// compressor sends its messages uncompressed, which uses no window at
    /// all and which Spec §6 leaves to it for every message.
    #[must_use]
    pub fn new(window_bits: u8, no_context_takeover: bool) -> Option<Self> {
        if !(9..=15).contains(&window_bits) {
            return None;
        }
        Some(Self {
            z: flate2::Compress::new_with_window_bits(
                flate2::Compression::default(),
                false,
                window_bits,
            ),
            no_context_takeover,
            scratch: alloc::vec![0u8; 16 * 1024],
        })
    }

    /// The compressor of the sending side of `role` under `params`: the
    /// server compresses with `server_max_window_bits` and
    /// `server_no_context_takeover`, the client with the `client_` pair.
    /// `None` as for [`Self::new`].
    #[must_use]
    pub fn for_sender(
        role: crate::masking::Role,
        params: &PermessageDeflateParams,
    ) -> Option<Self> {
        match role {
            crate::masking::Role::Server => {
                Self::new(params.server_max_window_bits, params.server_no_takeover)
            }
            crate::masking::Role::Client => {
                Self::new(params.client_max_window_bits, params.client_no_takeover)
            }
        }
    }

    /// Spec §7.2.1 — the payload of the compressed message for `data`:
    /// DEFLATE, ended at a byte boundary by an empty block without
    /// compression, whose `00 00 ff ff` is removed.
    ///
    /// # Errors
    /// [`DeflateError::NoTail`] if the compressor's output does not end
    /// with that block.
    pub fn compress(&mut self, data: &[u8]) -> Result<Vec<u8>, DeflateError> {
        if data.is_empty() {
            // The empty block without compression, less its `00 00 ff ff`
            // (Spec §7.2.3.6); the window is untouched.
            return Ok(alloc::vec![0x00]);
        }
        if self.no_context_takeover {
            self.z.reset();
        }
        let mut out = Vec::with_capacity(data.len() / 2 + 16);
        let mut input = data;
        loop {
            let (before_in, before_out) = (self.z.total_in(), self.z.total_out());
            self.z
                .compress(input, &mut self.scratch, flate2::FlushCompress::Sync)
                .map_err(|_| DeflateError::NoTail)?;
            let consumed = usize::try_from(self.z.total_in() - before_in).unwrap_or(0);
            let produced = usize::try_from(self.z.total_out() - before_out).unwrap_or(0);
            input = &input[consumed..];
            out.extend_from_slice(&self.scratch[..produced]);
            // The flush is complete when the compressor left room in the
            // output buffer; until then it has more to write.
            if input.is_empty() && produced < self.scratch.len() {
                break;
            }
            if consumed == 0 && produced == 0 {
                break;
            }
        }
        if !out.ends_with(&DEFLATE_TAIL) {
            return Err(DeflateError::NoTail);
        }
        out.truncate(out.len() - DEFLATE_TAIL.len());
        Ok(out)
    }
}

/// The decompressor of one direction of a connection (Spec §7.2.2).
#[cfg(feature = "std")]
pub struct Inflater {
    z: flate2::Decompress,
    window_bits: u8,
    no_context_takeover: bool,
    /// The last octets of what was decompressed, at most one window (with
    /// `no_context_takeover`: of the current message): the LZ77 window a
    /// new DEFLATE stream is given after a block with BFINAL (Spec §7.2.1
    /// allows such blocks inside and between messages).
    history: Vec<u8>,
    scratch: Vec<u8>,
}

#[cfg(feature = "std")]
impl core::fmt::Debug for Inflater {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Inflater")
            .field("window_bits", &self.window_bits)
            .field("no_context_takeover", &self.no_context_takeover)
            .finish_non_exhaustive()
    }
}

#[cfg(feature = "std")]
impl Inflater {
    /// A decompressor with an LZ77 window of `2^window_bits` octets. With
    /// `no_context_takeover` each message starts with an empty window.
    #[must_use]
    pub fn new(window_bits: u8, no_context_takeover: bool) -> Self {
        let window_bits = inflate_window(window_bits);
        Self {
            z: flate2::Decompress::new_with_window_bits(false, window_bits),
            window_bits,
            no_context_takeover,
            history: Vec::new(),
            scratch: alloc::vec![0u8; 16 * 1024],
        }
    }

    /// The decompressor of the receiving side of `role` under `params`:
    /// the server inflates what the client compressed with the `client_`
    /// pair, the client what the server compressed with the `server_` pair.
    #[must_use]
    pub fn for_receiver(role: crate::masking::Role, params: &PermessageDeflateParams) -> Self {
        match role {
            crate::masking::Role::Server => {
                Self::new(params.client_max_window_bits, params.client_no_takeover)
            }
            crate::masking::Role::Client => {
                Self::new(params.server_max_window_bits, params.server_no_takeover)
            }
        }
    }

    fn window(&self) -> usize {
        1usize << self.window_bits
    }

    fn remember(&mut self, produced: &[u8]) {
        let window = self.window();
        if produced.len() >= window {
            self.history.clear();
            self.history
                .extend_from_slice(&produced[produced.len() - window..]);
        } else {
            let keep = window - produced.len();
            if self.history.len() > keep {
                let cut = self.history.len() - keep;
                self.history.drain(..cut);
            }
            self.history.extend_from_slice(produced);
        }
    }

    /// Decompresses the payload of one frame of a compressed message; with
    /// `end_of_message`, the last one, behind which the `00 00 ff ff` of
    /// Spec §7.2.2 is appended. `limit` is the number of octets the caller
    /// still accepts for the message.
    ///
    /// # Errors
    /// [`DeflateError::Corrupt`] for a payload that is not DEFLATE data,
    /// [`DeflateError::TooLarge`] as soon as the output passes `limit`
    /// (the output is not produced beyond it).
    pub fn decompress(
        &mut self,
        payload: &[u8],
        end_of_message: bool,
        limit: usize,
    ) -> Result<Vec<u8>, DeflateError> {
        let mut out = Vec::new();
        self.run(payload, limit, &mut out)?;
        if end_of_message {
            self.run(&DEFLATE_TAIL, limit, &mut out)?;
        }
        self.remember(&out);
        if end_of_message && self.no_context_takeover {
            self.z = flate2::Decompress::new_with_window_bits(false, self.window_bits);
            self.history.clear();
        }
        Ok(out)
    }

    fn run(
        &mut self,
        mut input: &[u8],
        limit: usize,
        out: &mut Vec<u8>,
    ) -> Result<(), DeflateError> {
        loop {
            let (before_in, before_out) = (self.z.total_in(), self.z.total_out());
            let status = self
                .z
                .decompress(input, &mut self.scratch, flate2::FlushDecompress::Sync)
                .map_err(|_| DeflateError::Corrupt)?;
            let consumed = usize::try_from(self.z.total_in() - before_in).unwrap_or(0);
            let produced = usize::try_from(self.z.total_out() - before_out).unwrap_or(0);
            input = &input[consumed..];
            if out.len().saturating_add(produced) > limit {
                return Err(DeflateError::TooLarge);
            }
            out.extend_from_slice(&self.scratch[..produced]);
            if status == flate2::Status::StreamEnd {
                // A block with BFINAL ended the DEFLATE stream; the next
                // block starts a new one at the byte boundary, over the
                // same LZ77 window (Spec §7.2.1).
                let mut window: Vec<u8> = Vec::with_capacity(self.history.len() + out.len());
                window.extend_from_slice(&self.history);
                window.extend_from_slice(out);
                let size = self.window();
                let tail = &window[window.len().saturating_sub(size)..];
                self.z = flate2::Decompress::new_with_window_bits(false, self.window_bits);
                if !tail.is_empty() {
                    self.z
                        .set_dictionary(tail)
                        .map_err(|_| DeflateError::Corrupt)?;
                }
                continue;
            }
            if input.is_empty() && produced < self.scratch.len() {
                return Ok(());
            }
            if consumed == 0 && produced == 0 {
                // No progress with input left: the data asks for octets
                // that are not there.
                return if input.is_empty() {
                    Ok(())
                } else {
                    Err(DeflateError::Corrupt)
                };
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn ws_9_parse_no_params_yields_defaults() {
        let p = parse_offer("permessage-deflate").unwrap();
        assert_eq!(p, PermessageDeflateParams::default());
    }

    #[test]
    fn ws_9_parse_no_takeover_flags() {
        let p = parse_offer(
            "permessage-deflate; server_no_context_takeover; client_no_context_takeover",
        )
        .unwrap();
        assert!(p.server_no_takeover);
        assert!(p.client_no_takeover);
    }

    #[test]
    fn ws_9_parse_window_bits() {
        let p =
            parse_offer("permessage-deflate; server_max_window_bits=12; client_max_window_bits=10")
                .unwrap();
        assert_eq!(p.server_max_window_bits, 12);
        assert_eq!(p.client_max_window_bits, 10);
    }

    #[test]
    fn ws_9_rejects_invalid_window_bits() {
        assert!(parse_offer("permessage-deflate; server_max_window_bits=7").is_err());
        assert!(parse_offer("permessage-deflate; server_max_window_bits=16").is_err());
    }

    #[test]
    fn ws_9_rejects_unknown_param() {
        assert!(matches!(
            parse_offer("permessage-deflate; foo"),
            Err(NegotiationError::UnknownParam(_))
        ));
    }

    #[test]
    fn ws_9_rejects_boolean_with_value() {
        assert!(matches!(
            parse_offer("permessage-deflate; server_no_context_takeover=yes"),
            Err(NegotiationError::BooleanWithValue(_))
        ));
    }

    #[test]
    fn ws_9_render_default_is_bare_extension_name() {
        let s = render_accept(&PermessageDeflateParams::default());
        assert_eq!(s, "permessage-deflate");
    }

    #[test]
    fn ws_9_render_includes_params() {
        let p = PermessageDeflateParams {
            server_no_takeover: true,
            client_no_takeover: false,
            server_max_window_bits: 12,
            client_max_window_bits: 15,
        };
        let s = render_accept(&p);
        assert!(s.contains("server_no_context_takeover"));
        assert!(s.contains("server_max_window_bits=12"));
        assert!(!s.contains("client_max_window_bits"));
    }

    #[test]
    fn ws_9_tail_round_trip() {
        let raw = b"hello";
        let with_tail = append_tail(raw);
        assert_eq!(with_tail, b"hello\x00\x00\xff\xff");
        let stripped = strip_tail(&with_tail);
        assert_eq!(stripped, raw);
    }

    #[test]
    fn ws_9_strip_tail_no_op_when_absent() {
        assert_eq!(strip_tail(b"hello"), b"hello");
    }

    #[test]
    fn ws_9_parameterless_client_max_window_bits_accepted() {
        let p = parse_offer("permessage-deflate; client_max_window_bits").unwrap();
        assert_eq!(p.client_max_window_bits, 15);
    }

    #[test]
    fn rfc7692_7_an_offer_with_an_undefined_invalid_or_repeated_parameter_is_declined() {
        // §7: "A server MUST decline an extension negotiation offer for
        // this extension if [...]".
        for (offer, err) in [
            (
                "permessage-deflate; foo",
                NegotiationError::UnknownParam("foo".to_string()),
            ),
            (
                "permessage-deflate; server_max_window_bits",
                NegotiationError::InvalidWindowBits(0),
            ),
            (
                "permessage-deflate; server_max_window_bits=7",
                NegotiationError::InvalidWindowBits(7),
            ),
            (
                "permessage-deflate; client_max_window_bits=16",
                NegotiationError::InvalidWindowBits(16),
            ),
            // "a decimal integer value without leading zeroes".
            (
                "permessage-deflate; server_max_window_bits=010",
                NegotiationError::InvalidWindowBits(0),
            ),
            (
                "permessage-deflate; client_max_window_bits=+9",
                NegotiationError::InvalidWindowBits(0),
            ),
            (
                "permessage-deflate; client_no_context_takeover=1",
                NegotiationError::BooleanWithValue("client_no_context_takeover".to_string()),
            ),
            (
                "permessage-deflate; server_no_context_takeover; server_no_context_takeover",
                NegotiationError::DuplicateParam("server_no_context_takeover".to_string()),
            ),
            (
                "permessage-deflate; client_max_window_bits; client_max_window_bits=10",
                NegotiationError::DuplicateParam("client_max_window_bits".to_string()),
            ),
        ] {
            assert_eq!(parse_deflate_offer(offer), Err(err), "{offer}");
            assert_eq!(accept_offers(&[offer]), None, "{offer}");
        }
        // A quoted value is the token it holds (RFC 6455 §9.1).
        assert_eq!(
            parse_deflate_offer("permessage-deflate; server_max_window_bits=\"10\"")
                .unwrap()
                .server_max_window_bits,
            Some(10)
        );
    }

    #[test]
    fn rfc7692_7_1_the_response_follows_the_offer() {
        let answer = |offer: &str| accept_offers(&[offer]).map(|p| render_accept(&p));
        // The examples of §7.1.3.
        assert_eq!(
            answer("permessage-deflate").as_deref(),
            Some("permessage-deflate")
        );
        // §7.1.2.2: a bare client_max_window_bits is not echoed (a
        // response needs a value), the client may use up to 15 bits.
        assert_eq!(
            answer("permessage-deflate; client_max_window_bits").as_deref(),
            Some("permessage-deflate")
        );
        // With a value the server takes it for its own window.
        assert_eq!(
            answer("permessage-deflate; client_max_window_bits=10").as_deref(),
            Some("permessage-deflate; client_max_window_bits=10")
        );
        // §7.1.1.1, §7.1.2.1: accepted "by including" the parameter.
        assert_eq!(
            answer("permessage-deflate; server_no_context_takeover; server_max_window_bits=10")
                .as_deref(),
            Some("permessage-deflate; server_no_context_takeover; server_max_window_bits=10")
        );
        // §7.1.1.2: the hint is taken.
        assert_eq!(
            answer("permessage-deflate; client_no_context_takeover").as_deref(),
            Some("permessage-deflate; client_no_context_takeover")
        );
    }

    #[test]
    fn rfc7692_5_1_an_invalid_offer_is_declined_for_the_next_one() {
        // §5.1: the offers are a list in order of preference; the first
        // one that is valid is answered.
        let agreed = accept_offers(&[
            "permessage-deflate; server_max_window_bits=16",
            "x-other-extension",
            "permessage-deflate; server_max_window_bits=9",
            "permessage-deflate",
        ])
        .expect("the second offer");
        assert_eq!(agreed.server_max_window_bits, 9);
        // The name is a token of HTTP, compared without case.
        assert!(accept_offers(&["Permessage-Deflate"]).is_some());
        assert_eq!(accept_offers::<&str>(&[]), None);
        assert_eq!(accept_offers(&["x-other-extension"]), None);
    }

    #[cfg(feature = "std")]
    #[test]
    fn rfc7692_7_1_2_1_a_window_of_8_bits_is_kept_by_not_compressing() {
        // server_max_window_bits=8 is agreed to with 8 (§7.1.2.1: "the
        // same or smaller value"). The compressor has no 256-octet window:
        // the server has none then and sends uncompressed, the client's
        // direction is compressed.
        let agreed =
            accept_offers(&["permessage-deflate; server_max_window_bits=8"]).expect("agreed");
        assert_eq!(
            render_accept(&agreed),
            "permessage-deflate; server_max_window_bits=8"
        );
        use crate::masking::Role;
        assert!(Deflater::for_sender(Role::Server, &agreed).is_none());
        assert!(Deflater::for_sender(Role::Client, &agreed).is_some());
        assert!(Deflater::new(8, false).is_none());
        assert!(Deflater::new(16, false).is_none());
        // What a client compressed with a window of 8 bits is inflated.
        let mut inflater = Inflater::new(8, false);
        assert_eq!(
            inflater
                .decompress(&[0xf2, 0x48, 0xcd, 0xc9, 0xc9, 0x07, 0x00], true, 16)
                .unwrap(),
            b"Hello"
        );
    }

    #[test]
    fn rfc7692_7_the_client_fails_on_a_response_outside_its_offer() {
        let offer = DeflateOffer {
            server_no_context_takeover: true,
            server_max_window_bits: Some(10),
            ..DeflateOffer::default()
        };
        assert_eq!(
            render_offer(&offer),
            "permessage-deflate; server_no_context_takeover; server_max_window_bits=10"
        );
        let agreed = accept_response(
            &offer,
            "permessage-deflate; server_no_context_takeover; server_max_window_bits=9; client_no_context_takeover",
        )
        .expect("within the offer");
        assert_eq!(
            agreed,
            PermessageDeflateParams {
                server_no_takeover: true,
                client_no_takeover: true,
                server_max_window_bits: 9,
                client_max_window_bits: 15,
            }
        );
        for (response, err) in [
            // The server ignores what the client required.
            (
                "permessage-deflate; server_max_window_bits=10",
                NegotiationError::NotOffered("server_no_context_takeover".to_string()),
            ),
            (
                "permessage-deflate; server_no_context_takeover",
                NegotiationError::NotOffered("server_max_window_bits".to_string()),
            ),
            (
                "permessage-deflate; server_no_context_takeover; server_max_window_bits=11",
                NegotiationError::NotOffered("server_max_window_bits".to_string()),
            ),
            // §7.1.2.2: not in the offer, MUST NOT be in the response.
            (
                "permessage-deflate; server_no_context_takeover; server_max_window_bits=10; client_max_window_bits=10",
                NegotiationError::NotOffered("client_max_window_bits".to_string()),
            ),
            (
                "permessage-deflate; server_no_context_takeover; server_max_window_bits=10; bar",
                NegotiationError::UnknownParam("bar".to_string()),
            ),
        ] {
            assert_eq!(accept_response(&offer, response), Err(err), "{response}");
        }
        // A response needs a value for client_max_window_bits.
        let offer = DeflateOffer {
            client_max_window_bits: Some(None),
            ..DeflateOffer::default()
        };
        assert_eq!(
            render_offer(&offer),
            "permessage-deflate; client_max_window_bits"
        );
        assert_eq!(
            accept_response(&offer, "permessage-deflate; client_max_window_bits"),
            Err(NegotiationError::InvalidWindowBits(0))
        );
        assert_eq!(
            accept_response(&offer, "permessage-deflate; client_max_window_bits=12")
                .unwrap()
                .client_max_window_bits,
            12
        );
    }

    #[cfg(feature = "std")]
    mod codec {
        use super::*;

        const HELLO: [u8; 7] = [0xf2, 0x48, 0xcd, 0xc9, 0xc9, 0x07, 0x00];

        #[test]
        fn rfc7692_7_2_1_hello_is_compressed_to_the_octets_of_the_rfc() {
            // §7.2.3.1 and §7.2.3.2: one block with the fixed Huffman code,
            // the tail removed; the second "Hello" refers to the first
            // with context takeover and is the same octets without.
            let mut with = Deflater::new(15, false).unwrap();
            assert_eq!(with.compress(b"Hello").unwrap(), HELLO);
            assert_eq!(
                with.compress(b"Hello").unwrap(),
                [0xf2, 0x00, 0x11, 0x00, 0x00]
            );
            let mut without = Deflater::new(15, true).unwrap();
            assert_eq!(without.compress(b"Hello").unwrap(), HELLO);
            assert_eq!(without.compress(b"Hello").unwrap(), HELLO);
            // §7.2.3.6: no data is the empty block without compression.
            assert_eq!(with.compress(b"").unwrap(), [0x00]);
        }

        #[test]
        fn rfc7692_7_2_2_the_payloads_of_the_rfc_decompress_to_hello() {
            let payloads: [&[u8]; 4] = [
                &HELLO,
                // §7.2.3.3: a block without compression.
                &[
                    0x00, 0x05, 0x00, 0xfa, 0xff, 0x48, 0x65, 0x6c, 0x6c, 0x6f, 0x00,
                ],
                // §7.2.3.4: a block with BFINAL.
                &[0xf3, 0x48, 0xcd, 0xc9, 0xc9, 0x07, 0x00, 0x00],
                // §7.2.3.5: two blocks.
                &[
                    0xf2, 0x48, 0x05, 0x00, 0x00, 0x00, 0xff, 0xff, 0xca, 0xc9, 0xc9, 0x07, 0x00,
                ],
            ];
            for payload in payloads {
                let mut inflater = Inflater::new(15, false);
                assert_eq!(
                    inflater.decompress(payload, true, 1024).unwrap(),
                    b"Hello",
                    "{payload:02x?}"
                );
                // §7.2.3.6: the empty message after it.
                assert_eq!(inflater.decompress(&[0x00], true, 1024).unwrap(), b"");
            }
            // In two fragments (§7.2.3.1): the tail comes with the last.
            let mut inflater = Inflater::new(15, false);
            let mut out = inflater.decompress(&HELLO[..3], false, 1024).unwrap();
            out.extend(inflater.decompress(&HELLO[3..], true, 1024).unwrap());
            assert_eq!(out, b"Hello");
        }

        #[test]
        fn rfc7692_7_2_1_the_window_survives_a_block_with_bfinal() {
            // "Hello" in a block with BFINAL (§7.2.3.4), then the second
            // "Hello" of §7.2.3.2, which is a reference into the window.
            let mut inflater = Inflater::new(15, false);
            let first = [0xf3, 0x48, 0xcd, 0xc9, 0xc9, 0x07, 0x00, 0x00];
            assert_eq!(inflater.decompress(&first, true, 1024).unwrap(), b"Hello");
            let second = [0xf2, 0x00, 0x11, 0x00, 0x00];
            assert_eq!(inflater.decompress(&second, true, 1024).unwrap(), b"Hello");
        }

        #[test]
        fn rfc7692_7_2_2_what_is_not_deflate_is_an_error() {
            let mut inflater = Inflater::new(15, false);
            // BTYPE 11 is reserved (RFC 1951 §3.2.3).
            assert_eq!(
                inflater.decompress(&[0x06, 0x00], true, 1024),
                Err(DeflateError::Corrupt)
            );
            // A reference into a window that holds nothing.
            let mut inflater = Inflater::new(15, false);
            assert_eq!(
                inflater.decompress(&[0xf2, 0x00, 0x11, 0x00, 0x00], true, 1024),
                Err(DeflateError::Corrupt)
            );
        }

        #[test]
        fn rfc7692_7_2_2_the_output_stops_at_the_limit() {
            let mut deflater = Deflater::new(15, false).unwrap();
            let payload = deflater.compress(&alloc::vec![7u8; 100_000]).unwrap();
            assert!(payload.len() < 1000);
            let mut inflater = Inflater::new(15, false);
            assert_eq!(
                inflater.decompress(&payload, true, 99_999),
                Err(DeflateError::TooLarge)
            );
            let mut inflater = Inflater::new(15, false);
            assert_eq!(
                inflater.decompress(&payload, true, 100_000).unwrap().len(),
                100_000
            );
        }

        /// 4096 octets without a repetition inside, twice: the second half
        /// is a reference 4096 octets back.
        fn far_reference() -> Vec<u8> {
            let mut x = 0x2545_f491_4f6c_dd1du64;
            let half: Vec<u8> = (0..4096)
                .map(|_| {
                    x ^= x << 13;
                    x ^= x >> 7;
                    x ^= x << 17;
                    (x >> 24) as u8
                })
                .collect();
            let mut data = half.clone();
            data.extend_from_slice(&half);
            data
        }

        #[test]
        fn rfc7692_7_1_2_the_compressor_keeps_to_the_negotiated_window() {
            // server_max_window_bits=9: no reference further back than 512
            // octets. The second half of the data repeats the first 4096
            // octets back: a 15-bit compressor writes it as references, a
            // 9-bit compressor cannot reach that far and writes the octets
            // again.
            let data = far_reference();
            let small = Deflater::new(9, false).unwrap().compress(&data).unwrap();
            assert!(
                small.len() > 8000,
                "{} octets with a 9-bit window",
                small.len()
            );
            let large = Deflater::new(15, false).unwrap().compress(&data).unwrap();
            assert!(
                large.len() < 4400,
                "{} octets with a 15-bit window",
                large.len()
            );
            for (bits, payload) in [(9, &small), (15, &large)] {
                assert_eq!(
                    Inflater::new(bits, false)
                        .decompress(payload, true, usize::MAX)
                        .unwrap(),
                    data
                );
            }
        }

        #[test]
        fn rfc7692_7_2_2_no_context_takeover_starts_each_message_with_an_empty_window() {
            let mut inflater = Inflater::new(15, true);
            assert_eq!(inflater.decompress(&HELLO, true, 1024).unwrap(), b"Hello");
            // The reference of §7.2.3.2 finds no window.
            assert_eq!(
                inflater.decompress(&[0xf2, 0x00, 0x11, 0x00, 0x00], true, 1024),
                Err(DeflateError::Corrupt)
            );
        }
    }
}
