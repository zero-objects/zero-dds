// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! W3C trace context as RTPS vendor PID 0x0D00 (Spec §4).

use std::fmt;

use zerodds_foundation::tracing::{SpanContext, SpanId, TraceId};

/// Vendor PID value for the ZeroDDS trace-context inline QoS.
pub const PID_VENDOR_TRACE_CONTEXT: u16 = 0x0D00;

/// W3C-traceparent-Header (Version 00).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraceParent {
    /// Trace-ID (16 byte).
    pub trace_id: TraceId,
    /// Parent span ID (8 bytes).
    pub parent_id: SpanId,
    /// Trace-Flags (`0x01` = sampled).
    pub flags: u8,
}

impl TraceParent {
    /// Constructor.
    #[must_use]
    pub fn new(trace_id: TraceId, parent_id: SpanId, flags: u8) -> Self {
        Self {
            trace_id,
            parent_id,
            flags,
        }
    }

    /// Sampling-Bit (`0x01`).
    #[must_use]
    pub fn is_sampled(&self) -> bool {
        self.flags & 0x01 != 0
    }
}

impl fmt::Display for TraceParent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "00-{}-{}-{:02x}",
            self.trace_id.to_hex(),
            self.parent_id.to_hex(),
            self.flags
        )
    }
}

impl std::str::FromStr for TraceParent {
    type Err = TraceContextError;

    /// W3C Trace Context §3.2: the value of a `traceparent` header.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_traceparent(s)
    }
}

/// W3C tracestate header (Version 00, key=value;... per vendor).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TraceState {
    /// Raw state string (e.g. `"dds=topic:Foo;version:1.0"`).
    pub raw: String,
}

impl TraceState {
    /// Constructor with a raw string.
    #[must_use]
    pub fn new(raw: impl Into<String>) -> Self {
        Self { raw: raw.into() }
    }

    /// Empty?
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.raw.is_empty()
    }
}

/// PID 0x0D00 Payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraceContextPid {
    /// Mandatory field: traceparent.
    pub traceparent: TraceParent,
    /// Optional: tracestate.
    pub tracestate: Option<TraceState>,
}

impl TraceContextPid {
    /// Constructor.
    #[must_use]
    pub fn new(traceparent: TraceParent, tracestate: Option<TraceState>) -> Self {
        Self {
            traceparent,
            tracestate,
        }
    }

    /// Encoding per spec §4.2: two CDR strings (length+bytes,
    /// NUL-terminated, 4-byte-aligned).
    pub fn encode_inline_qos(&self, out: &mut Vec<u8>) {
        let tp = self.traceparent.to_string();
        encode_cdr_string(&tp, out);
        let ts = match &self.tracestate {
            Some(s) => s.raw.clone(),
            None => String::new(),
        };
        encode_cdr_string(&ts, out);
    }

    /// Decoding from the PID payload.
    pub fn decode_inline_qos(bytes: &[u8]) -> Result<Self, TraceContextError> {
        let (tp_str, rest) = decode_cdr_string(bytes)?;
        let traceparent = parse_traceparent(&tp_str)?;
        let tracestate = if rest.is_empty() {
            None
        } else {
            let (ts_str, _tail) = decode_cdr_string(rest)?;
            if ts_str.is_empty() {
                None
            } else {
                Some(TraceState::new(ts_str))
            }
        };
        Ok(Self::new(traceparent, tracestate))
    }

    /// Convenience: create from a span context.
    #[must_use]
    pub fn from_span_context(ctx: &SpanContext, vendor_state: Option<&str>) -> Self {
        Self {
            traceparent: TraceParent::new(ctx.trace_id, ctx.span_id, 0x01),
            tracestate: vendor_state.map(TraceState::new),
        }
    }

    /// Convenience: back to a span context (parent_span_id =
    /// the transmitted span ID; the receiver creates a child).
    #[must_use]
    pub fn to_span_context(&self) -> SpanContext {
        SpanContext {
            trace_id: self.traceparent.trace_id,
            span_id: self.traceparent.parent_id,
            parent_span_id: None,
        }
    }
}

/// Error in the trace-context codec.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TraceContextError {
    /// CDR string incomplete or length prefix invalid.
    InvalidCdrString,
    /// `traceparent` does not match the W3C format.
    InvalidTraceParent,
    /// Trace ID or span ID is all-zero (the W3C spec forbids invalid).
    InvalidIdentifiers,
}

impl fmt::Display for TraceContextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCdrString => f.write_str("invalid CDR string in PID 0x0D00 payload"),
            Self::InvalidTraceParent => f.write_str("traceparent does not match W3C format"),
            Self::InvalidIdentifiers => f.write_str("trace-id or span-id is all-zero"),
        }
    }
}

impl std::error::Error for TraceContextError {}

fn encode_cdr_string(s: &str, out: &mut Vec<u8>) {
    let bytes = s.as_bytes();
    let len = bytes.len() as u32 + 1; // +1 for the NUL terminator
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(bytes);
    out.push(0);
    // 4-byte-Alignment
    while !out.len().is_multiple_of(4) {
        out.push(0);
    }
}

fn decode_cdr_string(bytes: &[u8]) -> Result<(String, &[u8]), TraceContextError> {
    if bytes.len() < 4 {
        return Err(TraceContextError::InvalidCdrString);
    }
    let len = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
    if len == 0 || bytes.len() < 4 + len {
        return Err(TraceContextError::InvalidCdrString);
    }
    let payload = &bytes[4..4 + len - 1]; // without NUL terminator
    let s = std::str::from_utf8(payload)
        .map_err(|_| TraceContextError::InvalidCdrString)?
        .to_string();
    let mut consumed = 4 + len;
    while !consumed.is_multiple_of(4) {
        consumed += 1;
    }
    let rest = if consumed <= bytes.len() {
        &bytes[consumed..]
    } else {
        &[]
    };
    Ok((s, rest))
}

/// W3C Trace Context §3.2: a `traceparent` header value. Version `00` is
/// `00-<32 hex>-<16 hex>-<2 hex>` and nothing else; a higher version is read
/// by the fields of `00` and may go on after a dash (§3.2.4), its flags
/// reduced to the sampled flag (§4.3); version `ff` is invalid. The hex is
/// lower case (`HEXDIGLC`); an all-zero trace-id or parent-id is invalid
/// (§3.2.2.3, §3.2.2.4).
fn parse_traceparent(s: &str) -> Result<TraceParent, TraceContextError> {
    let b = s.as_bytes();
    let lower_hex = |field: &[u8]| field.iter().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f'));
    if b.len() < 55 || b[2] != b'-' || b[35] != b'-' || b[52] != b'-' || !lower_hex(&b[..2]) {
        return Err(TraceContextError::InvalidTraceParent);
    }
    let higher = match &b[..2] {
        b"00" if b.len() == 55 => false,
        b"00" | b"ff" => return Err(TraceContextError::InvalidTraceParent),
        _ if b.len() == 55 || b[55] == b'-' => true,
        _ => return Err(TraceContextError::InvalidTraceParent),
    };
    if !lower_hex(&b[3..35]) || !lower_hex(&b[36..52]) || !lower_hex(&b[53..55]) {
        return Err(TraceContextError::InvalidTraceParent);
    }
    let trace_id = parse_hex_16(&s[3..35])?;
    let parent_id = parse_hex_8(&s[36..52])?;
    let flags =
        u8::from_str_radix(&s[53..55], 16).map_err(|_| TraceContextError::InvalidTraceParent)?;
    let flags = if higher { flags & 0x01 } else { flags };

    let trace_id_obj = TraceId(trace_id);
    let parent_id_obj = SpanId(parent_id);
    if !trace_id_obj.is_valid() || !parent_id_obj.is_valid() {
        return Err(TraceContextError::InvalidIdentifiers);
    }
    Ok(TraceParent::new(trace_id_obj, parent_id_obj, flags))
}

fn parse_hex_16(hex: &str) -> Result<[u8; 16], TraceContextError> {
    let mut out = [0u8; 16];
    for (i, byte) in out.iter_mut().enumerate() {
        let pair = &hex
            .get(i * 2..i * 2 + 2)
            .ok_or(TraceContextError::InvalidTraceParent)?;
        *byte = u8::from_str_radix(pair, 16).map_err(|_| TraceContextError::InvalidTraceParent)?;
    }
    Ok(out)
}

fn parse_hex_8(hex: &str) -> Result<[u8; 8], TraceContextError> {
    let mut out = [0u8; 8];
    for (i, byte) in out.iter_mut().enumerate() {
        let pair = &hex
            .get(i * 2..i * 2 + 2)
            .ok_or(TraceContextError::InvalidTraceParent)?;
        *byte = u8::from_str_radix(pair, 16).map_err(|_| TraceContextError::InvalidTraceParent)?;
    }
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn sample_traceparent() -> TraceParent {
        TraceParent::new(
            TraceId([
                0x4b, 0xf9, 0x2f, 0x35, 0x77, 0xb3, 0x4d, 0xa6, 0xa3, 0xce, 0x92, 0x9d, 0x0e, 0x0e,
                0x47, 0x36,
            ]),
            SpanId([0x00, 0xf0, 0x67, 0xaa, 0x0b, 0xa9, 0x02, 0xb7]),
            0x01,
        )
    }

    #[test]
    fn traceparent_format_matches_w3c() {
        let tp = sample_traceparent();
        assert_eq!(
            tp.to_string(),
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"
        );
    }

    #[test]
    fn traceparent_sampled_bit() {
        let tp = TraceParent::new(TraceId([1; 16]), SpanId([1; 8]), 0x01);
        assert!(tp.is_sampled());
        let tp = TraceParent::new(TraceId([1; 16]), SpanId([1; 8]), 0x00);
        assert!(!tp.is_sampled());
    }

    #[test]
    fn pid_roundtrip_with_state() {
        let pid =
            TraceContextPid::new(sample_traceparent(), Some(TraceState::new("dds=topic:Foo")));
        let mut buf = Vec::new();
        pid.encode_inline_qos(&mut buf);
        let decoded = TraceContextPid::decode_inline_qos(&buf).expect("decode");
        assert_eq!(decoded, pid);
    }

    #[test]
    fn pid_roundtrip_without_state() {
        let pid = TraceContextPid::new(sample_traceparent(), None);
        let mut buf = Vec::new();
        pid.encode_inline_qos(&mut buf);
        let decoded = TraceContextPid::decode_inline_qos(&buf).expect("decode");
        assert_eq!(decoded, pid);
    }

    #[test]
    fn pid_decode_rejects_short_payload() {
        let buf = [0u8; 2];
        assert_eq!(
            TraceContextPid::decode_inline_qos(&buf),
            Err(TraceContextError::InvalidCdrString)
        );
    }

    #[test]
    fn pid_decode_rejects_invalid_traceparent_format() {
        let mut buf = Vec::new();
        encode_cdr_string("not-a-traceparent", &mut buf);
        encode_cdr_string("", &mut buf);
        match TraceContextPid::decode_inline_qos(&buf) {
            Err(TraceContextError::InvalidTraceParent) => {}
            other => panic!("expected InvalidTraceParent, got {other:?}"),
        }
    }

    #[test]
    fn pid_decode_rejects_zero_trace_id() {
        let mut buf = Vec::new();
        encode_cdr_string(
            "00-00000000000000000000000000000000-00f067aa0ba902b7-01",
            &mut buf,
        );
        encode_cdr_string("", &mut buf);
        assert_eq!(
            TraceContextPid::decode_inline_qos(&buf),
            Err(TraceContextError::InvalidIdentifiers)
        );
    }

    #[test]
    fn from_to_span_context_roundtrip() {
        let ctx = SpanContext::new_root(
            TraceId([
                0x4b, 0xf9, 0x2f, 0x35, 0x77, 0xb3, 0x4d, 0xa6, 0xa3, 0xce, 0x92, 0x9d, 0x0e, 0x0e,
                0x47, 0x36,
            ]),
            SpanId([0x00, 0xf0, 0x67, 0xaa, 0x0b, 0xa9, 0x02, 0xb7]),
        );
        let pid = TraceContextPid::from_span_context(&ctx, Some("dds=v:1"));
        let back = pid.to_span_context();
        assert_eq!(back.trace_id, ctx.trace_id);
        assert_eq!(back.span_id, ctx.span_id);
    }

    /// W3C Trace Context §3.2.2, §3.2.3: the header of the examples is
    /// read, the sampled flag with it.
    #[test]
    fn w3c_trace_context_3_2_2_a_traceparent_header_is_read() {
        let tp: TraceParent = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"
            .parse()
            .expect("the example of section 3.2.3");
        assert_eq!(tp, sample_traceparent());
        assert!(tp.is_sampled());
        let unsampled: TraceParent = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-00"
            .parse()
            .expect("the unsampled example");
        assert!(!unsampled.is_sampled());
    }

    /// W3C Trace Context §3.2.2: upper-case hex, version ff, wrong field
    /// lengths, a missing dash, all-zero identifiers and fields after a
    /// version-00 value are refused.
    #[test]
    fn w3c_trace_context_3_2_2_a_malformed_traceparent_is_refused() {
        for bad in [
            "00-4BF92F3577B34DA6A3CE929D0E0E4736-00f067aa0ba902b7-01",
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00F067AA0BA902B7-01",
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-0A",
            "ff-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
            "0-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
            "00-4bf92f3577b34da6a3ce929d0e0e473-00f067aa0ba902b7-01",
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b-01",
            "00-4bf92f3577b34da6a3ce929d0e0e4736_00f067aa0ba902b7-01",
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01-",
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01-ab",
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-+1",
            "",
        ] {
            assert_eq!(
                bad.parse::<TraceParent>(),
                Err(TraceContextError::InvalidTraceParent),
                "{bad}"
            );
        }
        for zero in [
            "00-00000000000000000000000000000000-00f067aa0ba902b7-01",
            "00-4bf92f3577b34da6a3ce929d0e0e4736-0000000000000000-01",
        ] {
            assert_eq!(
                zero.parse::<TraceParent>(),
                Err(TraceContextError::InvalidIdentifiers),
                "{zero}"
            );
        }
    }

    /// W3C Trace Context §3.2.4, §4.3: a higher version is read by the
    /// fields of version 00, followed by the end or a dash; only the
    /// sampled flag is kept.
    #[test]
    fn w3c_trace_context_3_2_4_a_higher_version_is_read_by_the_00_fields() {
        let tp: TraceParent =
            "cc-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-09-what-the-future-holds"
                .parse()
                .expect("a higher version with more fields");
        assert_eq!(tp.trace_id, sample_traceparent().trace_id);
        assert_eq!(tp.parent_id, sample_traceparent().parent_id);
        assert_eq!(tp.flags, 0x01);
        assert!(
            "cc-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"
                .parse::<TraceParent>()
                .is_ok()
        );
        assert_eq!(
            "cc-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01x".parse::<TraceParent>(),
            Err(TraceContextError::InvalidTraceParent)
        );
        assert_eq!(
            "cc-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7".parse::<TraceParent>(),
            Err(TraceContextError::InvalidTraceParent)
        );
    }

    #[test]
    fn pid_constant_is_0x0d00() {
        assert_eq!(PID_VENDOR_TRACE_CONTEXT, 0x0D00);
    }
}
