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
    /// Layout: GUID followed by RTPS SequenceNumber high/low words.
    #[must_use]
    pub fn to_bytes(&self, little_endian: bool) -> [u8; SAMPLE_IDENTITY_WIRE_SIZE] {
        let mut out = [0u8; SAMPLE_IDENTITY_WIRE_SIZE];
        out[..16].copy_from_slice(&self.writer_guid);
        let sequence = crate::wire_types::SequenceNumber::from_high_low(
            (self.sequence_number >> 32) as i32,
            self.sequence_number as u32,
        );
        let sn = if little_endian {
            sequence.to_bytes_le()
        } else {
            sequence.to_bytes_be()
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
        let sequence = if little_endian {
            crate::wire_types::SequenceNumber::from_bytes_le(sn)
        } else {
            crate::wire_types::SequenceNumber::from_bytes_be(sn)
        };
        let (high, low) = sequence.split();
        let sequence_number = (u64::from(high as u32) << 32) | u64::from(low);
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
    let mut pl = ParameterList::with_byte_order(little_endian);
    pl.push(related_sample_identity_param(id, little_endian));
    pl
}

/// eProsima's vendor-specific `PID_RELATED_SAMPLE_IDENTITY` (Fast DDS
/// `ParameterTypes.hpp`: `PID_RELATED_SAMPLE_IDENTITY = 0x800f`). Fast DDS
/// 2.6 writes and reads only this PID, not 0x0083; rmw_fastrtps correlates
/// ROS 2 service requests and replies with it. The value has the layout of
/// [`SampleIdentityBytes`]. A vendor-specific PID means something only in
/// the context of the sender's vendor (DDSI-RTPS 2.5 §9.6.2.2.1), so a
/// reader honours it from eProsima (and from this stack, which writes it
/// for Fast DDS peers) only.
pub const PID_EPROSIMA_RELATED_SAMPLE_IDENTITY: u16 = 0x800f;

/// Builds the eProsima form of the related sample identity
/// ([`PID_EPROSIMA_RELATED_SAMPLE_IDENTITY`]).
#[must_use]
pub fn eprosima_related_sample_identity_param(
    id: SampleIdentityBytes,
    little_endian: bool,
) -> Parameter {
    Parameter::new(
        PID_EPROSIMA_RELATED_SAMPLE_IDENTITY,
        id.to_bytes(little_endian).to_vec(),
    )
}

/// Reads [`PID_EPROSIMA_RELATED_SAMPLE_IDENTITY`] from an inline-QoS list —
/// the exact PID, since the vendor bit is what tells it apart from 0x000f.
/// `None` if absent or shorter than 24 bytes (Fast DDS ignores it then too).
#[must_use]
pub fn find_eprosima_related_sample_identity(
    pl: &ParameterList,
    little_endian: bool,
) -> Option<SampleIdentityBytes> {
    pl.parameters
        .iter()
        .find(|p| p.id == PID_EPROSIMA_RELATED_SAMPLE_IDENTITY)
        .and_then(|p| SampleIdentityBytes::from_bytes(&p.value, little_endian).ok())
}

/// Spec §9.6.4.9 — `PID_STATUS_INFO` bits.
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
/// (DDSI-RTPS 2.5 §9.6.4.9).
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

/// QoS values a DATA carries in-line that override those of its writer
/// (DDSI-RTPS 2.5 §8.3.8.2: "the inlineQos element contains QoS values that
/// override those of the RTPS Writer and should be used to process the
/// update"). Of the policies of Table 8.85 that may appear in-line, these
/// are the two whose value a reader applies to the single sample it takes:
/// the writer's `OWNERSHIP_STRENGTH` (the arbitration of EXCLUSIVE
/// ownership) and its `LIFESPAN` (the sample's expiry).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InlineQosOverride {
    /// `PID_OWNERSHIP_STRENGTH` (0x0006).
    pub ownership_strength: Option<i32>,
    /// `PID_LIFESPAN` (0x002b).
    pub lifespan: Option<zerodds_qos::Duration>,
}

impl InlineQosOverride {
    /// The overrides in `inline_qos`, in its byte order. A parameter of
    /// the wrong length is no override.
    #[must_use]
    pub fn of(inline_qos: Option<&ParameterList>) -> Self {
        let Some(pl) = inline_qos else {
            return Self::default();
        };
        Self {
            ownership_strength: pl.find(pid::OWNERSHIP_STRENGTH).and_then(|p| {
                let b: [u8; 4] = p.value.as_slice().try_into().ok()?;
                Some(if pl.little_endian {
                    i32::from_le_bytes(b)
                } else {
                    i32::from_be_bytes(b)
                })
            }),
            lifespan: pl
                .find(pid::LIFESPAN)
                .and_then(|p| crate::publication_data::decode_duration(&p.value, pl.little_endian)),
        }
    }
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

/// A lifecycle marker's inline QoS with `PID_STATUS_INFO` and **no**
/// `PID_KEY_HASH` — the form Cyclone 11.0.1 was measured to send, where the
/// instance is named by the `serializedKey` in the payload instead.
///
/// Only legitimate together with a non-empty `serializedPayload`: a marker
/// with neither the hash nor a key names no instance at all. See
/// [`crate::dialect::DisposeKeyForm::with_key_available`].
#[must_use]
pub fn lifecycle_inline_qos_status_only(status_bits: u32) -> ParameterList {
    let mut pl = ParameterList::new();
    pl.push(status_info_param(status_bits));
    pl
}

/// Wire size of a `PID_COHERENT_SET` value: an RTPS `SequenceNumber_t`
/// (`long high` + `unsigned long low`).
pub const COHERENT_SET_WIRE_SIZE: usize = 8;

/// Builds a `Parameter` with `PID_COHERENT_SET` (0x0056) — DDSI-RTPS 2.5
/// §8.7.6 / §9.6.4.2. The value is the `SequenceNumber_t` of the **first**
/// sample of the coherent set this DATA belongs to.
///
/// `SequenceNumber_t` is `{ long high; unsigned long low; }` (§9.3.2 Table
/// 9.3), so both halves follow the endianness of the enclosing submessage —
/// unlike `PID_STATUS_INFO`, which the spec pins to big-endian. Callers pass
/// the same `little_endian` flag they hand to `DataSubmessage::write_body`.
#[must_use]
pub fn coherent_set_param(set_first_sn: i64, little_endian: bool) -> Parameter {
    Parameter::new(
        pid::COHERENT_SET,
        coherent_set_bytes(set_first_sn, little_endian).to_vec(),
    )
}

/// `SequenceNumber_t` octets of a coherent-set marker (see
/// [`coherent_set_param`]).
#[must_use]
pub fn coherent_set_bytes(set_first_sn: i64, little_endian: bool) -> [u8; COHERENT_SET_WIRE_SIZE] {
    let high = (set_first_sn >> 32) as i32;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let low = (set_first_sn & 0xFFFF_FFFF) as u32;
    let mut out = [0u8; COHERENT_SET_WIRE_SIZE];
    if little_endian {
        out[0..4].copy_from_slice(&high.to_le_bytes());
        out[4..8].copy_from_slice(&low.to_le_bytes());
    } else {
        out[0..4].copy_from_slice(&high.to_be_bytes());
        out[4..8].copy_from_slice(&low.to_be_bytes());
    }
    out
}

/// Reads `PID_COHERENT_SET` from an inline-QoS list. Returns the
/// set-first `SequenceNumber` as an `i64`, or `None` if the PID is absent,
/// its value is not 8 octets, or the value is `SEQUENCENUMBER_UNKNOWN`:
/// such a DATA belongs to no coherent set and ends the writer's open one,
/// as one without the PID does (DDSI-RTPS 2.5 §8.7.6, Table 9.22
/// Example 2). Masks the must-understand and vendor bits — a peer may set
/// either on the PID.
#[must_use]
pub fn find_coherent_set(pl: &ParameterList, little_endian: bool) -> Option<i64> {
    for p in &pl.parameters {
        let masked = p.id & !(MUST_UNDERSTAND_BIT | VENDOR_SPECIFIC_BIT);
        if masked != pid::COHERENT_SET {
            continue;
        }
        if p.value.len() < COHERENT_SET_WIRE_SIZE {
            return None;
        }
        let (h, l) = (
            [p.value[0], p.value[1], p.value[2], p.value[3]],
            [p.value[4], p.value[5], p.value[6], p.value[7]],
        );
        let (high, low) = if little_endian {
            (i32::from_le_bytes(h), u32::from_le_bytes(l))
        } else {
            (i32::from_be_bytes(h), u32::from_be_bytes(l))
        };
        let sn = (i64::from(high) << 32) | i64::from(low);
        return (sn != crate::wire_types::SequenceNumber::UNKNOWN.0).then_some(sn);
    }
    None
}

/// Inline QoS of a coherent-set member DATA: a single `PID_COHERENT_SET`
/// (§8.7.6). Every DATA/DATA_FRAG between `begin_coherent_changes` and
/// `end_coherent_changes` carries it, with the same value.
#[must_use]
pub fn coherent_set_inline_qos(set_first_sn: i64, little_endian: bool) -> ParameterList {
    let mut pl = ParameterList::with_byte_order(little_endian);
    pl.push(coherent_set_param(set_first_sn, little_endian));
    pl
}

/// `PID_GROUP_COHERENT_SET` (0x0063, DDSI-RTPS 2.5 §8.7.6, §9.6.4.3): the
/// group sequence number of the first sample of a group coherent set,
/// across all writers of the publisher. Every DATA, the first DATA_FRAG
/// and the end-of-coherent-set DATA of the set carry it. CDR
/// `SequenceNumber_t`, the layout of `PID_COHERENT_SET`.
#[must_use]
pub fn group_coherent_set_param(group_first_gsn: i64, little_endian: bool) -> Parameter {
    Parameter::new(
        pid::GROUP_COHERENT_SET,
        coherent_set_bytes(group_first_gsn, little_endian).to_vec(),
    )
}

/// Reads `PID_GROUP_COHERENT_SET` from an inline-QoS list.
#[must_use]
pub fn find_group_coherent_set(pl: &ParameterList, little_endian: bool) -> Option<i64> {
    find_sequence_number(pl, pid::GROUP_COHERENT_SET, little_endian)
}

/// `PID_GROUP_SEQ_NUM` (0x0064, §8.7.5, §9.6.4.4): the sample's group
/// sequence number. Every DATA and the first DATA_FRAG of a writer of a
/// GROUP-scope publisher carry it. CDR `SequenceNumber_t`.
#[must_use]
pub fn group_seq_num_param(gsn: i64, little_endian: bool) -> Parameter {
    Parameter::new(
        pid::GROUP_SEQ_NUM,
        coherent_set_bytes(gsn, little_endian).to_vec(),
    )
}

/// Reads `PID_GROUP_SEQ_NUM` from an inline-QoS list.
#[must_use]
pub fn find_group_seq_num(pl: &ParameterList, little_endian: bool) -> Option<i64> {
    find_sequence_number(pl, pid::GROUP_SEQ_NUM, little_endian)
}

/// `PID_WRITER_GROUP_INFO` (0x0065, §8.7.5 Table 8.88, §9.6.4.5): the
/// `WriterGroupInfo_t` — its one member `writerSet`, the
/// [`GroupDigest`](crate::group_digest::GroupDigest) of the writers the
/// publisher had announced when the sample was written. Four octets,
/// which have no byte order.
#[must_use]
pub fn writer_group_info_param(writer_set: crate::group_digest::GroupDigest) -> Parameter {
    Parameter::new(pid::WRITER_GROUP_INFO, writer_set.to_bytes().to_vec())
}

/// `PID_SECURE_WRITER_GROUP_INFO` (0x0066, §9.6.4.6): the same
/// `WriterGroupInfo_t` as [`writer_group_info_param`], "reserved for DDS
/// Security. In the cases when it is used it shall be added anywhere that
/// the PID_WRITER_GROUP_INFO in-line QoS is required."
#[must_use]
pub fn secure_writer_group_info_param(writer_set: crate::group_digest::GroupDigest) -> Parameter {
    Parameter::new(
        pid::SECURE_WRITER_GROUP_INFO,
        writer_set.to_bytes().to_vec(),
    )
}

/// Reads `PID_WRITER_GROUP_INFO` from an inline-QoS list, or, where the
/// list has none, `PID_SECURE_WRITER_GROUP_INFO` (§9.6.4.6): the secure
/// parameter stands where the plain one is required.
#[must_use]
pub fn find_writer_group_info(pl: &ParameterList) -> Option<crate::group_digest::GroupDigest> {
    let find = |id: u16| {
        pl.parameters
            .iter()
            .find(|p| p.id & !(MUST_UNDERSTAND_BIT | VENDOR_SPECIFIC_BIT) == id)
            .and_then(|p| crate::group_digest::GroupDigest::read_from(&p.value).ok())
    };
    find(pid::WRITER_GROUP_INFO).or_else(|| find(pid::SECURE_WRITER_GROUP_INFO))
}

/// The group information of a sample (§8.7.5, §8.7.6): its
/// `PID_GROUP_SEQ_NUM`, `PID_WRITER_GROUP_INFO` (the empty group when
/// absent) and `PID_GROUP_COHERENT_SET`. `None` without a group sequence
/// number: the writer is not one of a GROUP-scope publisher's.
#[must_use]
pub fn find_change_group_info(pl: &ParameterList) -> Option<crate::writer_group::ChangeGroupInfo> {
    let gsn = find_group_seq_num(pl, pl.little_endian)?;
    Some(crate::writer_group::ChangeGroupInfo {
        gsn,
        writer_set: find_writer_group_info(pl).unwrap_or(crate::group_digest::GroupDigest::EMPTY),
        coherent_set_gsn: find_group_coherent_set(pl, pl.little_endian),
    })
}

/// The `SequenceNumber_t` value of the parameter `id`, the must-understand
/// and vendor bits masked off. `None` if absent or shorter than eight
/// octets.
fn find_sequence_number(pl: &ParameterList, id: u16, little_endian: bool) -> Option<i64> {
    for p in &pl.parameters {
        let masked = p.id & !(MUST_UNDERSTAND_BIT | VENDOR_SPECIFIC_BIT);
        if masked != id || p.value.len() < COHERENT_SET_WIRE_SIZE {
            continue;
        }
        let (h, l) = (
            [p.value[0], p.value[1], p.value[2], p.value[3]],
            [p.value[4], p.value[5], p.value[6], p.value[7]],
        );
        let (high, low) = if little_endian {
            (i32::from_le_bytes(h), u32::from_le_bytes(l))
        } else {
            (i32::from_be_bytes(h), u32::from_be_bytes(l))
        };
        return Some((i64::from(high) << 32) | i64::from(low));
    }
    None
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
    // The SN is an RTPS `SequenceNumber_t` (high `i32`, then low `u32`),
    // not a plain `i64` — the same layout `PID_COHERENT_SET` uses.
    value.extend_from_slice(&coherent_set_bytes(original_sn, little_endian));
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
            let (hi, lo) = (
                [p.value[16], p.value[17], p.value[18], p.value[19]],
                [p.value[20], p.value[21], p.value[22], p.value[23]],
            );
            let (high, low) = if little_endian {
                (i32::from_le_bytes(hi), u32::from_le_bytes(lo))
            } else {
                (i32::from_be_bytes(hi), u32::from_be_bytes(lo))
            };
            let sn = (i64::from(high) << 32) | i64::from(low);
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

/// Spec §8.7.7 — receiver-side filter: `true` if the sample is addressed
/// to the reader `own_reader_guid`, or is not directed. "The serialized
/// information denotes the GUIDs of the targeted reader(s)": one
/// `PID_DIRECTED_WRITE` per target reader, and the reader is addressed
/// when any of them names it. Only the first was read, so that a sample
/// directed at two readers was absorbed by the second.
///
/// # Errors
/// `WireError::UnexpectedEof` if a value slice is below 16 bytes.
pub fn directed_write_accepts(
    pl: &ParameterList,
    own_reader_guid: [u8; 16],
) -> Result<bool, WireError> {
    let target = pid::DIRECTED_WRITE;
    let mut directed = false;
    for p in &pl.parameters {
        if p.id & !(MUST_UNDERSTAND_BIT | VENDOR_SPECIFIC_BIT) != target {
            continue;
        }
        if p.value.len() < 16 {
            return Err(WireError::UnexpectedEof {
                needed: 16,
                offset: 0,
            });
        }
        if p.value[..16] == own_reader_guid {
            return Ok(true);
        }
        directed = true;
    }
    Ok(!directed)
}

/// Spec §8.7.7 — [`directed_write_accepts`]: `false` if the sample is
/// directed and none of its targets is `own_reader_guid`; the caller then
/// acknowledges and absorbs it as if it were a GAP.
///
/// # Errors
/// As [`directed_write_accepts`].
pub fn directed_write_matches_reader(
    pl: &ParameterList,
    own_reader_guid: [u8; 16],
) -> Result<bool, WireError> {
    directed_write_accepts(pl, own_reader_guid)
}

/// Reads `PID_RELATED_SAMPLE_IDENTITY` from an inline-QoS list.
///
/// Masks the must-understand bit only. A parameter with the
/// vendor-specific bit set (`0x8083`) belongs to the vendor that defined
/// it (DDSI-RTPS 2.5 §9.6.2.2.1) and is not DDS-RPC's `0x0083`; it is not
/// found here. Returns `Ok(None)` if the PID is not present (inline QoS
/// without RPC correlation — e.g. a user-sample DATA that has nothing to
/// do with RPC).
///
/// # Errors
/// `WireError::UnexpectedEof` if the value slice is below 24 bytes.
pub fn find_related_sample_identity(
    pl: &ParameterList,
    little_endian: bool,
) -> Result<Option<SampleIdentityBytes>, WireError> {
    let target = pid::RELATED_SAMPLE_IDENTITY;
    for p in &pl.parameters {
        let masked = p.id & !MUST_UNDERSTAND_BIT;
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

    // DDSI-RTPS 2.5 Table 9.23: Writer 1's second sample of a group
    // coherent set — PID_GROUP_SEQ_NUM 13, PID_COHERENT_SET 4,
    // PID_GROUP_COHERENT_SET 11.
    fn table_9_23_writer_1_second_sample(little_endian: bool) -> ParameterList {
        let mut pl = ParameterList::with_byte_order(little_endian);
        pl.push(group_seq_num_param(13, little_endian));
        pl.push(coherent_set_param(4, little_endian));
        pl.push(group_coherent_set_param(11, little_endian));
        pl
    }

    #[test]
    fn the_group_sequence_numbers_of_table_9_23_read_back_in_both_byte_orders() {
        for le in [true, false] {
            let pl = table_9_23_writer_1_second_sample(le);
            assert_eq!(find_group_seq_num(&pl, pl.little_endian), Some(13));
            assert_eq!(find_coherent_set(&pl, pl.little_endian), Some(4));
            assert_eq!(find_group_coherent_set(&pl, pl.little_endian), Some(11));
        }
    }

    #[test]
    fn group_seq_num_is_a_cdr_sequence_number_with_pid_0x0064() {
        let p = group_seq_num_param((2_i64 << 32) | 5, false);
        assert_eq!(p.id, 0x0064);
        assert_eq!(p.value, [0, 0, 0, 2, 0, 0, 0, 5]);
        let p = group_seq_num_param((2_i64 << 32) | 5, true);
        assert_eq!(p.value, [2, 0, 0, 0, 5, 0, 0, 0]);
    }

    #[test]
    fn writer_group_info_carries_the_four_digest_octets_with_pid_0x0065() {
        let digest = crate::group_digest::GroupDigest([0x6f, 0x62, 0x13, 0x62]);
        let p = writer_group_info_param(digest);
        assert_eq!(p.id, 0x0065);
        assert_eq!(p.value, [0x6f, 0x62, 0x13, 0x62]);
        let mut pl = ParameterList::new();
        pl.push(Parameter::new(
            MUST_UNDERSTAND_BIT | pid::WRITER_GROUP_INFO,
            p.value,
        ));
        assert_eq!(find_writer_group_info(&pl), Some(digest));
    }

    #[test]
    fn a_short_group_parameter_is_not_read() {
        let mut pl = ParameterList::new();
        pl.push(Parameter::new(pid::GROUP_SEQ_NUM, alloc::vec![0; 4]));
        pl.push(Parameter::new(pid::WRITER_GROUP_INFO, alloc::vec![0; 3]));
        assert_eq!(find_group_seq_num(&pl, true), None);
        assert_eq!(find_writer_group_info(&pl), None);
    }

    #[test]
    fn the_eprosima_pid_is_found_exactly_and_not_as_0x0083() {
        let id = sample_id();
        let mut pl = ParameterList::new();
        pl.push(eprosima_related_sample_identity_param(id, true));
        assert_eq!(pl.parameters[0].id, 0x800f);
        assert_eq!(find_eprosima_related_sample_identity(&pl, true), Some(id));
        assert_eq!(find_related_sample_identity(&pl, true).unwrap(), None);
        let standard = reply_inline_qos(id, true);
        assert_eq!(find_eprosima_related_sample_identity(&standard, true), None);
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
        assert_eq!(&bytes_le[16..], &[4, 3, 2, 1, 8, 7, 6, 5]);
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

    /// DDS-RPC 1.0 §7.8.2 defines 0x0083. A vendor-specific 0x8083 is some
    /// vendor's own parameter and is not read as the related sample identity.
    #[test]
    fn a_vendor_specific_0x8083_is_not_the_related_sample_identity() {
        let id = SampleIdentityBytes::new([3; 16], 9);
        let mut pl = ParameterList::new();
        pl.parameters.push(Parameter::new(
            VENDOR_SPECIFIC_BIT | pid::RELATED_SAMPLE_IDENTITY,
            id.to_bytes(true).to_vec(),
        ));
        assert_eq!(find_related_sample_identity(&pl, true).unwrap(), None);

        let mut pl = ParameterList::new();
        pl.parameters.push(Parameter::new(
            MUST_UNDERSTAND_BIT | pid::RELATED_SAMPLE_IDENTITY,
            id.to_bytes(true).to_vec(),
        ));
        assert_eq!(find_related_sample_identity(&pl, true).unwrap(), Some(id));
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
            data_flag: true,
            key_flag: false,
            non_standard_flag: false,
            serialized_payload: payload.clone().into(),
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
        // RTPS SequenceNumber_t: high i32 then low u32, each little-endian.
        assert_eq!(
            &p.value[16..],
            &[0x04, 0x03, 0x02, 0x01, 0x08, 0x07, 0x06, 0x05]
        );
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

    // ---- DDSI-RTPS 2.5 §8.7.6 / §9.6.4.2 PID_COHERENT_SET ----

    /// Table 9.18: `PID_COHERENT_SET` is 0x0056 and its value is a
    /// `SequenceNumber_t` — `{ long high; unsigned long low; }`, 8 octets, in
    /// the endianness of the enclosing submessage.
    #[test]
    fn coherent_set_param_is_pid_0x0056_with_sequence_number_layout() {
        let p = coherent_set_param(0x0000_0002_1234_5678, true);
        assert_eq!(p.id, 0x0056);
        assert_eq!(p.id, pid::COHERENT_SET);
        assert_eq!(p.value.len(), COHERENT_SET_WIRE_SIZE);
        assert_eq!(&p.value[0..4], &2i32.to_le_bytes(), "high half");
        assert_eq!(&p.value[4..8], &0x1234_5678u32.to_le_bytes(), "low half");

        let be = coherent_set_param(0x0000_0002_1234_5678, false);
        assert_eq!(&be.value[0..4], &2i32.to_be_bytes(), "high half BE");
        assert_eq!(
            &be.value[4..8],
            &0x1234_5678u32.to_be_bytes(),
            "low half BE"
        );
    }

    /// A small set-first SN — the normal case — is `high = 0`, `low = n`.
    #[test]
    fn coherent_set_small_sn_octets() {
        assert_eq!(coherent_set_bytes(1, true), [0, 0, 0, 0, 1, 0, 0, 0]);
        assert_eq!(coherent_set_bytes(1, false), [0, 0, 0, 0, 0, 0, 0, 1]);
    }

    #[test]
    fn coherent_set_roundtrip_both_endiannesses() {
        for le in [true, false] {
            let mut pl = ParameterList::new();
            pl.push(coherent_set_param(0x0000_0003_DEAD_BEEF, le));
            assert_eq!(find_coherent_set(&pl, le), Some(0x0000_0003_DEAD_BEEF));
        }
    }

    #[test]
    fn find_coherent_set_returns_none_when_absent() {
        assert_eq!(find_coherent_set(&ParameterList::new(), true), None);
    }

    /// Cyclone and RTI set the must-understand bit on inline-QoS PIDs; the
    /// lookup masks it (and the vendor bit) before comparing.
    #[test]
    fn find_coherent_set_masks_must_understand_bit() {
        let mut pl = ParameterList::new();
        pl.push(Parameter::new(
            MUST_UNDERSTAND_BIT | pid::COHERENT_SET,
            coherent_set_bytes(9, true).to_vec(),
        ));
        assert_eq!(find_coherent_set(&pl, true), Some(9));
    }

    /// Table 9.22, Example 2: `SEQUENCENUMBER_UNKNOWN` (high = -1, low = 0)
    /// names no set, in either byte order.
    #[test]
    fn find_coherent_set_reads_sequencenumber_unknown_as_no_set() {
        for le in [true, false] {
            let mut pl = ParameterList::new();
            pl.push(coherent_set_param(
                crate::wire_types::SequenceNumber::UNKNOWN.0,
                le,
            ));
            assert_eq!(find_coherent_set(&pl, le), None);
        }
    }

    /// A truncated value is not a set-first SN of zero — it is no marker.
    #[test]
    fn find_coherent_set_rejects_truncated_value() {
        let mut pl = ParameterList::new();
        pl.push(Parameter::new(pid::COHERENT_SET, alloc::vec![0u8; 4]));
        assert_eq!(find_coherent_set(&pl, true), None);
    }

    /// End to end through the DATA submessage: a coherent-set member survives
    /// `write_body` / `read_body_with_flags` with its marker intact.
    #[test]
    fn coherent_set_survives_data_submessage_roundtrip() {
        use crate::submessages::{DATA_FLAG_INLINE_QOS, DataSubmessage};
        use crate::wire_types::{EntityId, SequenceNumber};

        let payload: Arc<[u8]> = Arc::from(alloc::vec![7u8, 8, 9].as_slice());
        let data = DataSubmessage {
            extra_flags: 0,
            reader_id: EntityId::user_reader_with_key([1, 2, 3]),
            writer_id: EntityId::user_writer_with_key([4, 5, 6]),
            writer_sn: SequenceNumber(5),
            inline_qos: Some(coherent_set_inline_qos(4, true)),
            data_flag: true,
            key_flag: false,
            non_standard_flag: false,
            serialized_payload: payload.into(),
        };
        let (body, flags) = data.write_body(true);
        assert_ne!(flags & DATA_FLAG_INLINE_QOS, 0);
        let decoded = DataSubmessage::read_body_with_flags(&body, true, flags).unwrap();
        let pl = decoded.inline_qos.expect("inline QoS");
        assert_eq!(find_coherent_set(&pl, true), Some(4));
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

    /// DDSI-RTPS 2.5 §9.6.4 Table 9.20: the ParameterIds of PID_DIRECTED_WRITE
    /// (0x0057, GUID_t), PID_ORIGINAL_WRITER_INFO (0x0061, OriginalWriterInfo_t)
    /// and PID_SECURE_WRITER_GROUP_INFO (0x0066, WriterGroupInfo_t), and the
    /// octets each one's value takes (§9.6.4.6, §9.6.4.7).
    #[test]
    fn table_9_20_directed_write_original_writer_info_and_secure_group_info() {
        let guid = [7u8; 16];
        let directed = directed_write_param(guid);
        assert_eq!(directed.id, 0x0057);
        assert_eq!(directed.value, guid, "a GUID_t: sixteen octets");
        let original = original_writer_info_param(guid, 42, true);
        assert_eq!(original.id, 0x0061);
        assert_eq!(original.value.len(), 24, "GUID_t and SequenceNumber_t");
        assert_eq!(&original.value[..16], &guid);
        let secure = secure_writer_group_info_param(crate::group_digest::GroupDigest::EMPTY);
        assert_eq!(secure.id, 0x0066);
        assert_eq!(secure.value.len(), 4, "WriterGroupInfo_t: its writerSet");
        // And back from a list.
        let mut pl = ParameterList::new();
        pl.push(directed);
        pl.push(original);
        assert_eq!(find_directed_write(&pl).unwrap(), Some(guid));
        assert_eq!(
            find_original_writer_info(&pl, true).unwrap(),
            Some((guid, 42))
        );
    }
}
