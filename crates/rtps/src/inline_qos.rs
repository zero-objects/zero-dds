// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Inline-QoS helpers for DDS-RPC and SEDP (DDS-RPC 1.0 §7.8.2).
//!
//! Inline QoS is a [`crate::parameter_list::ParameterList`] in the body
//! of a DATA/DATA_FRAG submessage (Q flag, Spec §9.4.5.3). Phase 2 already uses
//! it for `PID_KEY_HASH`, `PID_STATUS_INFO` etc. — this module
//! adds helpers for the RPC-specific inline-QoS PIDs:
//!
//! * `PID_RELATED_SAMPLE_IDENTITY = 0x0083` (DDS-RPC 1.0 §7.8.2): is
//!   set by the reply writer in the inline QoS of every reply DATA; the value
//!   is the `request_id` (`SampleIdentity` = 16 byte writer_guid +
//!   8 byte sequence_number) of the correlated request. Encoding:
//!   XCDR2-Final, alignment cap=4, i.e. 24 bytes without padding.
//!
//! We encapsulate encode/decode here so that `zerodds-rpc` can work without a
//! hard dependency on RTPS internals like the `Parameter` layout.

use crate::error::WireError;
use crate::parameter_list::{
    MUST_UNDERSTAND_BIT, Parameter, ParameterList, VENDOR_SPECIFIC_BIT, pid,
};

/// Spec constant: 16 byte writer GUID + 8 byte sequence number.
pub const SAMPLE_IDENTITY_WIRE_SIZE: usize = 24;

/// Wire representation of a `SampleIdentity` (DDS-RPC 1.0 §7.5.1.1.1).
///
/// Identical to the layout in `zerodds-rpc::common_types::SampleIdentity`, but
/// modeled here as a pure byte helper — `zerodds-rtps` must not depend on
/// `zerodds-rpc` (crate dependency direction).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SampleIdentityBytes {
    /// 16 byte writer-GUID.
    pub writer_guid: [u8; 16],
    /// 64-bit sequence number.
    pub sequence_number: u64,
}

impl SampleIdentityBytes {
    /// Constructor.
    #[must_use]
    pub const fn new(writer_guid: [u8; 16], sequence_number: u64) -> Self {
        Self {
            writer_guid,
            sequence_number,
        }
    }

    /// XCDR2-Final encoder with the given endianness.
    /// Layout: 16 byte GUID + 8 byte u64 (alignment cap=4 ⇒ no padding).
    #[must_use]
    pub fn to_bytes(&self, little_endian: bool) -> [u8; SAMPLE_IDENTITY_WIRE_SIZE] {
        let mut out = [0u8; SAMPLE_IDENTITY_WIRE_SIZE];
        out[..16].copy_from_slice(&self.writer_guid);
        let sn = if little_endian {
            self.sequence_number.to_le_bytes()
        } else {
            self.sequence_number.to_be_bytes()
        };
        out[16..].copy_from_slice(&sn);
        out
    }

    /// XCDR2-Final decoder.
    ///
    /// # Errors
    /// `WireError::UnexpectedEof` if the buffer is shorter than 24 bytes.
    pub fn from_bytes(bytes: &[u8], little_endian: bool) -> Result<Self, WireError> {
        if bytes.len() < SAMPLE_IDENTITY_WIRE_SIZE {
            return Err(WireError::UnexpectedEof {
                needed: SAMPLE_IDENTITY_WIRE_SIZE,
                offset: 0,
            });
        }
        let mut writer_guid = [0u8; 16];
        writer_guid.copy_from_slice(&bytes[..16]);
        let mut sn = [0u8; 8];
        sn.copy_from_slice(&bytes[16..24]);
        let sequence_number = if little_endian {
            u64::from_le_bytes(sn)
        } else {
            u64::from_be_bytes(sn)
        };
        Ok(Self {
            writer_guid,
            sequence_number,
        })
    }
}

/// Builds a `Parameter` with `PID_RELATED_SAMPLE_IDENTITY` (0x0083) and
/// the 24-byte XCDR2 encoding of the `SampleIdentity` as the value.
#[must_use]
pub fn related_sample_identity_param(id: SampleIdentityBytes, little_endian: bool) -> Parameter {
    Parameter::new(
        pid::RELATED_SAMPLE_IDENTITY,
        id.to_bytes(little_endian).to_vec(),
    )
}

/// Builds an inline-QoS `ParameterList` that carries only the
/// `PID_RELATED_SAMPLE_IDENTITY` — good enough for the reply DATA
/// of a simple RPC operation. Callers can add further parameters
/// (`PID_KEY_HASH` etc.) via `push`.
#[must_use]
pub fn reply_inline_qos(id: SampleIdentityBytes, little_endian: bool) -> ParameterList {
    let mut pl = ParameterList::new();
    pl.push(related_sample_identity_param(id, little_endian));
    pl
}

/// Spec §9.6.3.9 — `PID_STATUS_INFO` bits.
pub mod status_info {
    /// Bit 0: the sample was marked NOT_ALIVE_DISPOSED via `dispose`.
    pub const DISPOSED: u32 = 0x0000_0001;
    /// Bit 1: the sample was marked NOT_ALIVE_NO_WRITERS via
    /// `unregister_instance`.
    pub const UNREGISTERED: u32 = 0x0000_0002;
    /// Bit 2: the sample was filtered by the writer via a content filter.
    pub const FILTERED: u32 = 0x0000_0004;
}

/// Builds a `Parameter` with `PID_STATUS_INFO` (0x0071). The value is a
/// 4-byte status word (bits per [`status_info`]). The spec requires
/// **big-endian** encoding independent of the RTPS header endianness
/// (DDSI-RTPS 2.5 §9.6.3.9).
#[must_use]
pub fn status_info_param(bits: u32) -> Parameter {
    Parameter::new(pid::STATUS_INFO, bits.to_be_bytes().to_vec())
}

/// Reads `PID_STATUS_INFO` from an inline-QoS list. Returns the
/// 4-byte status word, or `None` if the PID is missing / the value
/// is not 4 bytes.
#[must_use]
pub fn find_status_info(pl: &ParameterList) -> Option<u32> {
    let p = pl.find(pid::STATUS_INFO)?;
    if p.value.len() != 4 {
        return None;
    }
    let mut b = [0u8; 4];
    b.copy_from_slice(&p.value);
    Some(u32::from_be_bytes(b))
}

/// Reads `PID_KEY_HASH` from an inline-QoS list (Spec §9.6.4.8 +
/// XTypes 1.3 §7.6.8). Returns the 16-byte identity of the instance,
/// or `None` if the PID is missing / the value has an invalid
/// length.
#[must_use]
pub fn find_key_hash(pl: &ParameterList) -> Option<[u8; 16]> {
    let p = pl.find(pid::KEY_HASH)?;
    if p.value.len() != 16 {
        return None;
    }
    let mut b = [0u8; 16];
    b.copy_from_slice(&p.value);
    Some(b)
}

/// Builds an inline-QoS `ParameterList` for a lifecycle marker —
/// `PID_KEY_HASH` (16 byte) + `PID_STATUS_INFO` (4 byte). Sent by the
/// writer on `dispose`/`unregister_instance`.
#[must_use]
pub fn lifecycle_inline_qos(key_hash: [u8; 16], status_bits: u32) -> ParameterList {
    let mut pl = ParameterList::new();
    pl.push(Parameter::new(pid::KEY_HASH, key_hash.to_vec()));
    pl.push(status_info_param(status_bits));
    pl
}

/// Spec §8.7.9 — `PID_ORIGINAL_WRITER_INFO` as inline QoS. 24-byte
/// value: 16 byte original GUID + 8 byte SequenceNumber (signed i64).
/// Set by the persistence service when it forwards a historical sample
/// on behalf of another writer.
#[must_use]
pub fn original_writer_info_param(
    original_guid: [u8; 16],
    original_sn: i64,
    little_endian: bool,
) -> Parameter {
    let mut value = alloc::vec::Vec::with_capacity(24);
    value.extend_from_slice(&original_guid);
    let sn_bytes = if little_endian {
        original_sn.to_le_bytes()
    } else {
        original_sn.to_be_bytes()
    };
    value.extend_from_slice(&sn_bytes);
    Parameter::new(pid::ORIGINAL_WRITER_INFO, value)
}

/// Reads `PID_ORIGINAL_WRITER_INFO` from an inline-QoS list.
///
/// # Errors
/// `WireError::UnexpectedEof` if the value slice is below 24 bytes.
pub fn find_original_writer_info(
    pl: &ParameterList,
    little_endian: bool,
) -> Result<Option<([u8; 16], i64)>, WireError> {
    let target = pid::ORIGINAL_WRITER_INFO;
    for p in &pl.parameters {
        let masked = p.id & !(MUST_UNDERSTAND_BIT | VENDOR_SPECIFIC_BIT);
        if masked == target {
            if p.value.len() < 24 {
                return Err(WireError::UnexpectedEof {
                    needed: 24,
                    offset: 0,
                });
            }
            let mut g = [0u8; 16];
            g.copy_from_slice(&p.value[..16]);
            let mut s = [0u8; 8];
            s.copy_from_slice(&p.value[16..24]);
            let sn = if little_endian {
                i64::from_le_bytes(s)
            } else {
                i64::from_be_bytes(s)
            };
            return Ok(Some((g, sn)));
        }
    }
    Ok(None)
}

/// Spec §8.7.7 / §9.6.2.2.5 — `PID_DIRECTED_WRITE` as inline QoS.
/// Marks a sample as a point-to-point send to a single
/// target reader (16 byte GUID). Other readers that receive the sample
/// (e.g. via multicast) MUST discard it.
#[must_use]
pub fn directed_write_param(target_reader_guid: [u8; 16]) -> Parameter {
    Parameter::new(pid::DIRECTED_WRITE, target_reader_guid.to_vec())
}

/// Reads `PID_DIRECTED_WRITE` from an inline-QoS list.
///
/// Returns `Ok(None)` if the PID is not set (the sample is not
/// directed). Otherwise returns the 16-byte target GUID.
///
/// # Errors
/// `WireError::UnexpectedEof` if the value slice is below 16 bytes.
pub fn find_directed_write(pl: &ParameterList) -> Result<Option<[u8; 16]>, WireError> {
    let target = pid::DIRECTED_WRITE;
    for p in &pl.parameters {
        let masked = p.id & !(MUST_UNDERSTAND_BIT | VENDOR_SPECIFIC_BIT);
        if masked == target {
            if p.value.len() < 16 {
                return Err(WireError::UnexpectedEof {
                    needed: 16,
                    offset: 0,
                });
            }
            let mut g = [0u8; 16];
            g.copy_from_slice(&p.value[..16]);
            return Ok(Some(g));
        }
    }
    Ok(None)
}

/// Spec §8.7.7 — receiver-side filter: returns `true` if the sample
/// is addressed to the reader with `own_reader_guid` (or no
/// directed write is set). `false` if a PID_DIRECTED_WRITE
/// is present and the value does NOT match own_guid — the
/// caller MUST then discard the sample.
///
/// # Errors
/// Wire decoding error from `find_directed_write`.
pub fn directed_write_matches_reader(
    pl: &ParameterList,
    own_reader_guid: [u8; 16],
) -> Result<bool, WireError> {
    Ok(match find_directed_write(pl)? {
        None => true,
        Some(target) => target == own_reader_guid,
    })
}

/// Reads `PID_RELATED_SAMPLE_IDENTITY` from an inline-QoS list.
///
/// Masks the must-understand bit + vendor bit. Returns `Ok(None)` if
/// the PID is not present (inline QoS without RPC correlation —
/// e.g. a user-sample DATA that has nothing to do with RPC).
///
/// # Errors
/// `WireError::UnexpectedEof` if the value slice is below 24 bytes.
pub fn find_related_sample_identity(
    pl: &ParameterList,
    little_endian: bool,
) -> Result<Option<SampleIdentityBytes>, WireError> {
    let target = pid::RELATED_SAMPLE_IDENTITY;
    for p in &pl.parameters {
        let masked = p.id & !(MUST_UNDERSTAND_BIT | VENDOR_SPECIFIC_BIT);
        if masked == target {
            return Ok(Some(SampleIdentityBytes::from_bytes(
                &p.value,
                little_endian,
            )?));
        }
    }
    Ok(None)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use alloc::sync::Arc;

    fn sample_id() -> SampleIdentityBytes {
        SampleIdentityBytes::new([0xAB; 16], 0x0102_0304_0506_0708)
    }

    #[test]
    fn sample_identity_layout_is_24_bytes() {
        let id = sample_id();
        let bytes_le = id.to_bytes(true);
        let bytes_be = id.to_bytes(false);
        assert_eq!(bytes_le.len(), SAMPLE_IDENTITY_WIRE_SIZE);
        assert_eq!(bytes_be.len(), SAMPLE_IDENTITY_WIRE_SIZE);
        // First 16 bytes = GUID (endianness-independent).
        assert_eq!(&bytes_le[..16], &[0xAB; 16]);
        assert_eq!(&bytes_be[..16], &[0xAB; 16]);
        // Last 8 bytes = sequence number, different per endianness.
        assert_eq!(&bytes_le[16..], &0x0102_0304_0506_0708u64.to_le_bytes());
        assert_eq!(&bytes_be[16..], &0x0102_0304_0506_0708u64.to_be_bytes());
    }

    #[test]
    fn sample_identity_roundtrip_le() {
        let id = sample_id();
        let bytes = id.to_bytes(true);
        assert_eq!(SampleIdentityBytes::from_bytes(&bytes, true).unwrap(), id);
    }

    #[test]
    fn sample_identity_roundtrip_be() {
        let id = sample_id();
        let bytes = id.to_bytes(false);
        assert_eq!(SampleIdentityBytes::from_bytes(&bytes, false).unwrap(), id);
    }

    #[test]
    fn sample_identity_too_short_is_eof_error() {
        let res = SampleIdentityBytes::from_bytes(&[0u8; 10], true);
        assert!(matches!(res, Err(WireError::UnexpectedEof { .. })));
    }

    #[test]
    fn related_sample_identity_param_uses_pid_0x0083() {
        let p = related_sample_identity_param(sample_id(), true);
        assert_eq!(p.id, 0x0083);
        assert_eq!(p.id, pid::RELATED_SAMPLE_IDENTITY);
        assert_eq!(p.value.len(), SAMPLE_IDENTITY_WIRE_SIZE);
    }

    #[test]
    fn reply_inline_qos_contains_only_pid_0x0083() {
        let pl = reply_inline_qos(sample_id(), true);
        assert_eq!(pl.parameters.len(), 1);
        assert_eq!(pl.parameters[0].id, pid::RELATED_SAMPLE_IDENTITY);
    }

    #[test]
    fn find_related_sample_identity_finds_pid() {
        let id = sample_id();
        let pl = reply_inline_qos(id, true);
        let got = find_related_sample_identity(&pl, true).unwrap();
        assert_eq!(got, Some(id));
    }

    #[test]
    fn find_related_sample_identity_missing_returns_none() {
        let pl = ParameterList::new();
        let got = find_related_sample_identity(&pl, true).unwrap();
        assert_eq!(got, None);
    }

    #[test]
    fn find_related_sample_identity_with_must_understand_bit_works() {
        // Cyclone likes to set inline-QoS PIDs with the MU bit. Masking must
        // take effect.
        let id = sample_id();
        let mut pl = ParameterList::new();
        pl.push(Parameter::new(
            MUST_UNDERSTAND_BIT | pid::RELATED_SAMPLE_IDENTITY,
            id.to_bytes(true).to_vec(),
        ));
        let got = find_related_sample_identity(&pl, true).unwrap();
        assert_eq!(got, Some(id));
    }

    #[test]
    fn find_related_sample_identity_truncated_value_is_error() {
        let mut pl = ParameterList::new();
        pl.push(Parameter::new(
            pid::RELATED_SAMPLE_IDENTITY,
            alloc::vec![0u8; 8],
        ));
        let res = find_related_sample_identity(&pl, true);
        assert!(matches!(res, Err(WireError::UnexpectedEof { .. })));
    }

    #[test]
    fn roundtrip_in_data_submessage_inline_qos() {
        // E2E: DataSubmessage with Q flag + PID 0x0083 in inline QoS,
        // then roundtrip through write_body / read_body_with_flags.
        use crate::submessages::{DATA_FLAG_INLINE_QOS, DataSubmessage};
        use crate::wire_types::{EntityId, SequenceNumber};

        let id = sample_id();
        let inline = reply_inline_qos(id, true);
        let payload: Arc<[u8]> = Arc::from(alloc::vec![1u8, 2, 3, 4].as_slice());
        let data = DataSubmessage {
            extra_flags: 0,
            reader_id: EntityId::user_reader_with_key([1, 2, 3]),
            writer_id: EntityId::user_writer_with_key([4, 5, 6]),
            writer_sn: SequenceNumber(42),
            inline_qos: Some(inline),
            key_flag: false,
            non_standard_flag: false,
            serialized_payload: payload.clone(),
        };
        let (body, flags) = data.write_body(true);
        assert_ne!(flags & DATA_FLAG_INLINE_QOS, 0);

        let decoded = DataSubmessage::read_body_with_flags(&body, true, flags).unwrap();
        assert!(decoded.inline_qos.is_some());
        let pl = decoded.inline_qos.unwrap();
        let back = find_related_sample_identity(&pl, true).unwrap();
        assert_eq!(back, Some(id));
        // Payload byte-identical.
        assert_eq!(&decoded.serialized_payload[..], &[1, 2, 3, 4]);
    }

    // ---- Spec §8.7.7 Directed Write ----

    #[test]
    fn directed_write_param_carries_target_guid() {
        let target = [0xCA; 16];
        let p = directed_write_param(target);
        assert_eq!(p.id, pid::DIRECTED_WRITE);
        assert_eq!(p.value, target.to_vec());
    }

    #[test]
    fn find_directed_write_returns_target_when_present() {
        let mut pl = ParameterList::new();
        let target = [0xAB; 16];
        pl.push(directed_write_param(target));
        assert_eq!(find_directed_write(&pl).unwrap(), Some(target));
    }

    #[test]
    fn find_directed_write_returns_none_when_absent() {
        let pl = ParameterList::new();
        assert_eq!(find_directed_write(&pl).unwrap(), None);
    }

    #[test]
    fn find_directed_write_rejects_truncated_value() {
        let mut pl = ParameterList::new();
        pl.push(Parameter::new(pid::DIRECTED_WRITE, alloc::vec![0; 8]));
        let r = find_directed_write(&pl);
        assert!(matches!(r, Err(WireError::UnexpectedEof { .. })));
    }

    #[test]
    fn directed_write_matches_reader_returns_true_for_matching_target() {
        let mut pl = ParameterList::new();
        let me = [0xAB; 16];
        pl.push(directed_write_param(me));
        assert!(directed_write_matches_reader(&pl, me).unwrap());
    }

    #[test]
    fn directed_write_matches_reader_returns_false_for_other_target() {
        let mut pl = ParameterList::new();
        pl.push(directed_write_param([0xAB; 16]));
        let other = [0xCD; 16];
        assert!(!directed_write_matches_reader(&pl, other).unwrap());
    }

    #[test]
    fn directed_write_matches_reader_returns_true_when_no_directed_write() {
        let pl = ParameterList::new();
        // No directed write → every reader matches.
        assert!(directed_write_matches_reader(&pl, [0; 16]).unwrap());
    }

    // ---- Spec §8.7.9 OriginalWriterInfo ----

    #[test]
    fn original_writer_info_param_24_byte_layout_le() {
        let p = original_writer_info_param([0xAB; 16], 0x0102_0304_0506_0708, true);
        assert_eq!(p.id, pid::ORIGINAL_WRITER_INFO);
        assert_eq!(p.value.len(), 24);
        assert_eq!(&p.value[..16], &[0xAB; 16]);
        assert_eq!(&p.value[16..], &0x0102_0304_0506_0708i64.to_le_bytes());
    }

    #[test]
    fn original_writer_info_roundtrip_le() {
        let mut pl = ParameterList::new();
        pl.push(original_writer_info_param([0xCD; 16], 42, true));
        let back = find_original_writer_info(&pl, true).unwrap();
        assert_eq!(back, Some(([0xCD; 16], 42)));
    }

    #[test]
    fn original_writer_info_roundtrip_be() {
        let mut pl = ParameterList::new();
        pl.push(original_writer_info_param([0xCD; 16], 42, false));
        let back = find_original_writer_info(&pl, false).unwrap();
        assert_eq!(back, Some(([0xCD; 16], 42)));
    }

    #[test]
    fn find_original_writer_info_returns_none_when_absent() {
        let pl = ParameterList::new();
        assert_eq!(find_original_writer_info(&pl, true).unwrap(), None);
    }

    #[test]
    fn find_original_writer_info_rejects_truncated() {
        let mut pl = ParameterList::new();
        pl.push(Parameter::new(
            pid::ORIGINAL_WRITER_INFO,
            alloc::vec![0; 16],
        ));
        let r = find_original_writer_info(&pl, true);
        assert!(matches!(r, Err(WireError::UnexpectedEof { .. })));
    }

    #[test]
    fn vendor_trace_context_pid_constant() {
        assert_eq!(pid::VENDOR_TRACE_CONTEXT, 0x0D00);
    }

    #[test]
    fn vendor_trace_context_param_roundtrip() {
        let mut pl = ParameterList::new();
        let payload = alloc::vec![1, 2, 3, 4, 5, 6, 7, 8];
        pl.push(Parameter::new(pid::VENDOR_TRACE_CONTEXT, payload.clone()));
        let bytes = pl.to_bytes(true);
        let pl2 = ParameterList::from_bytes(&bytes, true).expect("decode");
        let p = pl2.find(pid::VENDOR_TRACE_CONTEXT).expect("present");
        assert_eq!(p.value, payload);
    }
}
