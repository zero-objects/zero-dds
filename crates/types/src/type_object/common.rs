// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Gemeinsam genutzte Wire-Types fuer TypeObject (Minimal + Complete).
//!
//! XTypes §7.3.4.5 (CommonStructMember, NameHash, MemberId).

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_cdr::{BufferReader, BufferWriter, DecodeError, EncodeError};

use crate::type_identifier::TypeIdentifier;

use super::flags::{StructMemberFlag, UnionMemberFlag};

/// 32-bit Member-ID (§7.3.4.5). Wird entweder explizit via `@id(n)`
/// vergeben oder aus dem Member-Namen gehasht (`@autoid(HASH)`).
pub type MemberId = u32;

/// 4-byte Name-Hash (§7.3.4.5 — "MD5(name)[0..4]").
///
/// Wird im MinimalTypeObject statt des vollen Namens gespeichert, um
/// die Payload klein zu halten. Im CompleteTypeObject liegt der volle
/// Name zusaetzlich vor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct NameHash(pub [u8; 4]);

impl NameHash {
    /// Berechnet den 4-byte NameHash aus einem Member-/Literal-Namen.
    ///
    /// Spec §7.3.4.5: "the `name_hash` is computed as the first 4
    /// octets of the MD5 hash of the name, interpreted as ASCII/UTF-8".
    #[must_use]
    pub fn from_name(name: &str) -> Self {
        let digest = zerodds_foundation::md5(name.as_bytes());
        let out: [u8; 4] = [digest[0], digest[1], digest[2], digest[3]];
        Self(out)
    }

    /// Encoded als `octet[4]` (4 byte, keine Laenge, kein Padding).
    ///
    /// # Errors
    /// Buffer-Overflow.
    pub fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        w.write_bytes(&self.0)
    }

    /// Decoder.
    ///
    /// # Errors
    /// Buffer-Underflow.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let bytes = r.read_bytes(4)?;
        // `read_bytes(n)` gibt garantiert genau n Bytes zurueck, sonst
        // UnexpectedEof. Der try_into auf [u8; 4] ist daher infallibel.
        let Ok(out): Result<[u8; 4], _> = bytes.try_into() else {
            return Err(DecodeError::UnexpectedEof {
                needed: 4,
                offset: 0,
            });
        };
        Ok(Self(out))
    }
}

/// CommonStructMember (§7.3.4.5.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonStructMember {
    /// Member-ID (4 byte).
    pub member_id: MemberId,
    /// Flags (IS_KEY, IS_OPTIONAL, etc.).
    pub member_flags: StructMemberFlag,
    /// Typ des Members (kann rekursiv sein).
    pub member_type_id: TypeIdentifier,
}

impl CommonStructMember {
    /// Encoded als `{ u32 member_id; u16 member_flags; TypeIdentifier member_type_id; }`.
    ///
    /// # Errors
    /// Buffer-Overflow.
    pub fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        w.write_u32(self.member_id)?;
        w.write_u16(self.member_flags.0)?;
        self.member_type_id.encode_into(w)
    }

    /// Decoder.
    ///
    /// # Errors
    /// Buffer-Underflow.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let member_id = r.read_u32()?;
        let member_flags = StructMemberFlag(r.read_u16()?);
        let member_type_id = TypeIdentifier::decode_from(r)?;
        Ok(Self {
            member_id,
            member_flags,
            member_type_id,
        })
    }
}

/// CommonUnionMember (§7.3.4.5.3). Enthaelt zusaetzlich die Label-Liste.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonUnionMember {
    /// Member-ID.
    pub member_id: MemberId,
    /// Flags (IS_DEFAULT fuer default-case).
    pub member_flags: UnionMemberFlag,
    /// Typ des Members.
    pub type_id: TypeIdentifier,
    /// Case-Labels als `i32`-Sequence (Spec §7.3.4.5.3.2: `long[]`).
    pub label_seq: alloc::vec::Vec<i32>,
}

impl CommonUnionMember {
    /// Encode.
    ///
    /// # Errors
    /// Buffer-Overflow.
    pub fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        w.write_u32(self.member_id)?;
        w.write_u16(self.member_flags.0)?;
        self.type_id.encode_into(w)?;
        // sequence<long>: u32 Laenge + N*i32
        let len =
            u32::try_from(self.label_seq.len()).map_err(|_| EncodeError::ValueOutOfRange {
                message: "union label sequence length exceeds u32::MAX",
            })?;
        w.write_u32(len)?;
        for l in &self.label_seq {
            w.write_u32(*l as u32)?;
        }
        Ok(())
    }

    /// Decoder.
    ///
    /// # Errors
    /// Buffer-Underflow.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let member_id = r.read_u32()?;
        let member_flags = UnionMemberFlag(r.read_u16()?);
        let type_id = TypeIdentifier::decode_from(r)?;
        let len = r.read_u32()? as usize;
        let cap = safe_capacity(len, 4, r.remaining());
        let mut label_seq = alloc::vec::Vec::with_capacity(cap);
        for _ in 0..len {
            label_seq.push(r.read_u32()? as i32);
        }
        Ok(Self {
            member_id,
            member_flags,
            type_id,
            label_seq,
        })
    }
}

// ============================================================================
// Complete-TypeObject-Annotations (§7.3.4.5.4)
// ============================================================================

/// Voller qualified Type-Name, z.B. "::sensors::Chatter". Alias fuer
/// `String` — im Wire als CDR-String (u32 Laenge + UTF-8 + null-term).
pub type QualifiedTypeName = String;

/// Placement-Kind einer `@verbatim`-Annotation (§7.3.4.5.4 §PL_*).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerbatimPlacement {
    /// Vor der Typ-Deklaration.
    Before,
    /// Nach der Typ-Deklaration.
    After,
    /// Innerhalb des Header-Blocks (z.B. `#include`).
    BeginFile,
    /// Ende der Datei.
    EndFile,
    /// Andere Platzierung (Forward-Compat).
    Other(String),
}

/// `@verbatim(language, text, placement)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedVerbatimAnnotation {
    /// Platzierung im Code-Gen-Output.
    pub placement: VerbatimPlacement,
    /// Zielsprache (z.B. "c++").
    pub language: String,
    /// Literal-Text.
    pub text: String,
}

impl AppliedVerbatimAnnotation {
    fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        let placement_str = match &self.placement {
            VerbatimPlacement::Before => "BEFORE_DECLARATION",
            VerbatimPlacement::After => "AFTER_DECLARATION",
            VerbatimPlacement::BeginFile => "BEGIN_FILE",
            VerbatimPlacement::EndFile => "END_FILE",
            VerbatimPlacement::Other(s) => s.as_str(),
        };
        w.write_string(placement_str)?;
        w.write_string(&self.language)?;
        w.write_string(&self.text)
    }

    fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let placement_str = r.read_string()?;
        let placement = match placement_str.as_str() {
            "BEFORE_DECLARATION" => VerbatimPlacement::Before,
            "AFTER_DECLARATION" => VerbatimPlacement::After,
            "BEGIN_FILE" => VerbatimPlacement::BeginFile,
            "END_FILE" => VerbatimPlacement::EndFile,
            _ => VerbatimPlacement::Other(placement_str),
        };
        let language = r.read_string()?;
        let text = r.read_string()?;
        Ok(Self {
            placement,
            language,
            text,
        })
    }
}

/// AppliedBuiltinTypeAnnotations (§7.3.4.5.4): `@verbatim` auf Typ-Level.
///
/// Wire: `sequence<AppliedVerbatimAnnotation, 1>` (0 oder 1 Eintrag =
/// "absent"/"present"). Weitere Builtin-Type-Annotations (`@unit`,
/// `@min`, `@max`, `@hash_id`) gehoeren zum Member-Scope, nicht Typ.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AppliedBuiltinTypeAnnotations {
    /// Optionale `@verbatim`-Direktive.
    pub verbatim: Option<AppliedVerbatimAnnotation>,
}

impl AppliedBuiltinTypeAnnotations {
    /// Encode als `sequence<T, 1>`.
    ///
    /// # Errors
    /// Buffer-Overflow.
    pub fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        match &self.verbatim {
            None => w.write_u32(0),
            Some(v) => {
                w.write_u32(1)?;
                v.encode_into(w)
            }
        }
    }

    /// Decode.
    ///
    /// # Errors
    /// Buffer-Underflow.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let len = r.read_u32()?;
        let verbatim = if len == 0 {
            None
        } else {
            Some(AppliedVerbatimAnnotation::decode_from(r)?)
        };
        // Etwaige weitere Eintraege (forward-compat) einfach skippen: wir
        // akzeptieren bis zu `len` Verbatims aber speichern nur den ersten.
        for _ in 1..len {
            let _ = AppliedVerbatimAnnotation::decode_from(r)?;
        }
        Ok(Self { verbatim })
    }
}

/// AppliedAnnotationParameter (§7.3.4.5.4): ein benannter Parameter
/// einer Annotation-Instanz. Der Parameter-Name wird als 4-byte-Hash
/// gespeichert (spart Payload).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedAnnotationParameter {
    /// Hash des Parameter-Namens.
    pub paramname_hash: NameHash,
    /// Parameter-Wert als opaque bytes (Discriminator-gefuehrt).
    pub value: Vec<u8>,
}

impl AppliedAnnotationParameter {
    fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        self.paramname_hash.encode_into(w)?;
        let len = u32::try_from(self.value.len()).map_err(|_| EncodeError::ValueOutOfRange {
            message: "annotation parameter value exceeds u32::MAX bytes",
        })?;
        w.write_u32(len)?;
        w.write_bytes(&self.value)
    }

    fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let paramname_hash = NameHash::decode_from(r)?;
        let len = r.read_u32()? as usize;
        let value = r.read_bytes(len)?.to_vec();
        Ok(Self {
            paramname_hash,
            value,
        })
    }
}

/// AppliedAnnotation: Instanz einer Custom-Annotation auf Typ/Member.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedAnnotation {
    /// Typ der Annotation (TypeIdentifier auf die Annotation-Definition).
    pub annotation_typeid: TypeIdentifier,
    /// Gesetzte Parameter.
    pub param_seq: Vec<AppliedAnnotationParameter>,
}

impl AppliedAnnotation {
    /// Encode.
    ///
    /// # Errors
    /// Buffer-Overflow.
    pub fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        self.annotation_typeid.encode_into(w)?;
        encode_seq(w, &self.param_seq, |w, p| p.encode_into(w))
    }

    /// Decode.
    ///
    /// # Errors
    /// Buffer-Underflow.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let annotation_typeid = TypeIdentifier::decode_from(r)?;
        let param_seq = decode_seq(r, AppliedAnnotationParameter::decode_from)?;
        Ok(Self {
            annotation_typeid,
            param_seq,
        })
    }
}

/// Optionales `sequence<AppliedAnnotation>` — wire: `sequence<T, 1>`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OptionalAppliedAnnotationSeq(pub Option<Vec<AppliedAnnotation>>);

impl OptionalAppliedAnnotationSeq {
    /// Encode.
    ///
    /// # Errors
    /// Buffer-Overflow.
    pub fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        match &self.0 {
            None => w.write_u32(0),
            Some(seq) => {
                w.write_u32(1)?;
                encode_seq(w, seq, |w, a| a.encode_into(w))
            }
        }
    }

    /// Decode.
    ///
    /// # Errors
    /// Buffer-Underflow.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let len = r.read_u32()?;
        if len == 0 {
            return Ok(Self(None));
        }
        let seq = decode_seq(r, AppliedAnnotation::decode_from)?;
        for _ in 1..len {
            // skip forward-compat duplicate inner seqs
            let _ = decode_seq(r, AppliedAnnotation::decode_from)?;
        }
        Ok(Self(Some(seq)))
    }
}

/// CompleteTypeDetail (§7.3.4.5.4): ann_builtin + ann_custom + type_name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteTypeDetail {
    /// Builtin-Annotations (z.B. `@verbatim`).
    pub ann_builtin: AppliedBuiltinTypeAnnotations,
    /// Custom-Annotations (optional).
    pub ann_custom: OptionalAppliedAnnotationSeq,
    /// Vollqualifizierter Typ-Name (z.B. "::sensors::Chatter").
    pub type_name: QualifiedTypeName,
}

impl CompleteTypeDetail {
    /// Encode.
    ///
    /// # Errors
    /// Buffer-Overflow.
    pub fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        self.ann_builtin.encode_into(w)?;
        self.ann_custom.encode_into(w)?;
        w.write_string(&self.type_name)
    }

    /// Decode.
    ///
    /// # Errors
    /// Buffer-Underflow.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let ann_builtin = AppliedBuiltinTypeAnnotations::decode_from(r)?;
        let ann_custom = OptionalAppliedAnnotationSeq::decode_from(r)?;
        let type_name = r.read_string()?;
        Ok(Self {
            ann_builtin,
            ann_custom,
            type_name,
        })
    }
}

/// AppliedBuiltinMemberAnnotations (§7.3.4.5.4) — Member-spezifische
/// Builtin-Annotations.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AppliedBuiltinMemberAnnotations {
    /// `@unit("...")`.
    pub unit: Option<String>,
    /// `@min(val)` als opaque bytes (discriminator-led).
    pub min: Option<Vec<u8>>,
    /// `@max(val)`.
    pub max: Option<Vec<u8>>,
    /// `@hashid("...")`.
    pub hash_id: Option<String>,
    /// `@default(val)` (XTypes 1.3 §7.2.4.4.4.4.9). Wert ist als String
    /// gespeichert (Caller konvertiert zum Member-Typ); Wire-Form
    /// liegt am Ende des AppliedBuiltinMemberAnnotations-Records, sodass
    /// Decoder ohne `default_value`-Wissen am vorletzten String-Feld
    /// (`hash_id`) korrekt enden — neue Decoder lesen den Trailer; alte
    /// Decoder lassen ihn liegen.
    pub default_value: Option<String>,
}

impl AppliedBuiltinMemberAnnotations {
    fn write_opt_string(w: &mut BufferWriter, s: &Option<String>) -> Result<(), EncodeError> {
        match s {
            None => w.write_u32(0),
            Some(v) => {
                w.write_u32(1)?;
                w.write_string(v)
            }
        }
    }

    fn read_opt_string(r: &mut BufferReader<'_>) -> Result<Option<String>, DecodeError> {
        let len = r.read_u32()?;
        if len == 0 {
            return Ok(None);
        }
        // XTypes-Spec §7.3.4.8: AppliedBuiltinMemberAnnotations-Felder
        // (unit, min, max, hash_id) sind skalar-optional, nicht Sequenz.
        // len > 1 ist protokollwidrig — strictly rejecten statt still
        // Mehrfach-Eintraege zu verwerfen (verhinderte bisher die
        // Diagnose von fehlerhaften Peer-Encodern).
        if len != 1 {
            return Err(DecodeError::LengthExceeded {
                announced: len as usize,
                remaining: 1,
                offset: 0,
            });
        }
        let out = r.read_string()?;
        Ok(Some(out))
    }

    fn write_opt_bytes(w: &mut BufferWriter, b: &Option<Vec<u8>>) -> Result<(), EncodeError> {
        match b {
            None => w.write_u32(0),
            Some(v) => {
                w.write_u32(1)?;
                let len = u32::try_from(v.len()).map_err(|_| EncodeError::ValueOutOfRange {
                    message: "annotation value exceeds u32::MAX",
                })?;
                w.write_u32(len)?;
                w.write_bytes(v)
            }
        }
    }

    fn read_opt_bytes(r: &mut BufferReader<'_>) -> Result<Option<Vec<u8>>, DecodeError> {
        let len = r.read_u32()?;
        if len == 0 {
            return Ok(None);
        }
        // Siehe read_opt_string: skalar-optional, len > 1 ist
        // Protokoll-Fehler, nicht stiller Datenverlust.
        if len != 1 {
            return Err(DecodeError::LengthExceeded {
                announced: len as usize,
                remaining: 1,
                offset: 0,
            });
        }
        let inner_len = r.read_u32()? as usize;
        let out = r.read_bytes(inner_len)?.to_vec();
        Ok(Some(out))
    }

    /// Encode.
    ///
    /// # Errors
    /// Buffer-Overflow.
    pub fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        Self::write_opt_string(w, &self.unit)?;
        Self::write_opt_bytes(w, &self.min)?;
        Self::write_opt_bytes(w, &self.max)?;
        Self::write_opt_string(w, &self.hash_id)?;
        Self::write_opt_string(w, &self.default_value)
    }

    /// Decode.
    ///
    /// # Errors
    /// Buffer-Underflow.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let unit = Self::read_opt_string(r)?;
        let min = Self::read_opt_bytes(r)?;
        let max = Self::read_opt_bytes(r)?;
        let hash_id = Self::read_opt_string(r)?;
        // `default_value` ist neuer Trailer (§7.2.4.4.4.4.9). Falls
        // Reader-Buffer leer ist, ist es ein Legacy-Encoder ohne den
        // Trailer — return None, keine Fehler.
        let default_value = if r.remaining() >= 4 {
            Self::read_opt_string(r).ok().flatten()
        } else {
            None
        };
        Ok(Self {
            unit,
            min,
            max,
            hash_id,
            default_value,
        })
    }
}

/// CompleteMemberDetail: `name` + `ann_builtin` + `ann_custom`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteMemberDetail {
    /// Voller Member-Name.
    pub name: String,
    /// Builtin-Member-Annotations.
    pub ann_builtin: AppliedBuiltinMemberAnnotations,
    /// Custom-Annotations.
    pub ann_custom: OptionalAppliedAnnotationSeq,
}

impl CompleteMemberDetail {
    /// Encode.
    ///
    /// # Errors
    /// Buffer-Overflow.
    pub fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        w.write_string(&self.name)?;
        self.ann_builtin.encode_into(w)?;
        self.ann_custom.encode_into(w)
    }

    /// Decode.
    ///
    /// # Errors
    /// Buffer-Underflow.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let name = r.read_string()?;
        let ann_builtin = AppliedBuiltinMemberAnnotations::decode_from(r)?;
        let ann_custom = OptionalAppliedAnnotationSeq::decode_from(r)?;
        Ok(Self {
            name,
            ann_builtin,
            ann_custom,
        })
    }
}

/// Hilfsroutine: sequence<T> encode via Callback.
pub(crate) fn encode_seq<T, F>(
    w: &mut BufferWriter,
    items: &[T],
    mut f: F,
) -> Result<(), EncodeError>
where
    F: FnMut(&mut BufferWriter, &T) -> Result<(), EncodeError>,
{
    let len = u32::try_from(items.len()).map_err(|_| EncodeError::ValueOutOfRange {
        message: "sequence length exceeds u32::MAX",
    })?;
    w.write_u32(len)?;
    for it in items {
        f(w, it)?;
    }
    Ok(())
}

/// DoS-Cap fuer Vec-Pre-Allocation beim Decode. Der Wert ist die
/// Obergrenze in Elementen, die wir initial allozieren. Grosse Sequenzen
/// werden inkrementell durch `push()` gewachsen.
///
/// 16 MiB / min_elem_size ist eine grobzuegige Heuristik — niemand
/// sendet legitim 16M TypeIdentifiers in einem getTypes-Reply, aber
/// Vec::with_capacity(u32_wire as usize) wuerde bei u32::MAX ~16 GB
/// reservieren = OOM-Vektor.
pub const DECODE_PREALLOC_CAP: usize = 4096;

/// Sichere Allokation: `Vec::with_capacity(len.min(remaining_bytes /
/// min_elem_size).min(CAP))`. Verhindert "30-byte-PID erzwingt 4 GB RAM".
#[must_use]
pub(crate) fn safe_capacity(len: usize, min_elem_size: usize, remaining_bytes: usize) -> usize {
    let by_bytes = if min_elem_size == 0 {
        DECODE_PREALLOC_CAP
    } else {
        remaining_bytes.saturating_div(min_elem_size)
    };
    len.min(by_bytes).min(DECODE_PREALLOC_CAP)
}

/// Hilfsroutine: sequence<T> decode via Callback.
///
/// Verwendet [`safe_capacity`] fuer DoS-Schutz: selbst wenn die
/// wire-Laenge `u32::MAX` ist, allokieren wir initial hoechstens
/// `DECODE_PREALLOC_CAP` Eintraege. Die echte Schleife baut trotzdem
/// bis `len` hoch, bricht aber spaetestens ab wenn `read_*` ueber den
/// verfuegbaren Puffer hinaus liest.
pub(crate) fn decode_seq<T, F>(
    r: &mut BufferReader<'_>,
    mut f: F,
) -> Result<alloc::vec::Vec<T>, DecodeError>
where
    F: FnMut(&mut BufferReader<'_>) -> Result<T, DecodeError>,
{
    let len = r.read_u32()? as usize;
    let cap = safe_capacity(len, 1, r.remaining());
    let mut out = alloc::vec::Vec::with_capacity(cap);
    for _ in 0..len {
        out.push(f(r)?);
    }
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod safe_capacity_tests {
    use super::*;

    #[test]
    fn safe_capacity_clamps_by_remaining_bytes() {
        assert_eq!(safe_capacity(1_000_000_000, 4, 100), 25);
    }

    #[test]
    fn safe_capacity_caps_at_prealloc_cap() {
        let cap = safe_capacity(usize::MAX, 1, usize::MAX);
        assert_eq!(cap, DECODE_PREALLOC_CAP);
    }

    #[test]
    fn safe_capacity_returns_len_when_small() {
        assert_eq!(safe_capacity(10, 4, 1000), 10);
    }

    #[test]
    fn safe_capacity_handles_zero_elem_size() {
        assert_eq!(safe_capacity(usize::MAX, 0, 100), DECODE_PREALLOC_CAP);
    }

    #[test]
    fn decode_seq_truncates_preallocation_for_large_lengths() {
        let mut bytes = alloc::vec::Vec::new();
        bytes.extend_from_slice(&u32::MAX.to_le_bytes());
        let mut r = BufferReader::new(&bytes, zerodds_cdr::Endianness::Little);
        let res: Result<alloc::vec::Vec<u8>, _> = decode_seq(&mut r, |rr| rr.read_u8());
        assert!(res.is_err());
    }

    fn roundtrip_verbatim(placement: VerbatimPlacement) {
        let a = AppliedVerbatimAnnotation {
            placement: placement.clone(),
            language: alloc::string::String::from("c++"),
            text: alloc::string::String::from("// example"),
        };
        let mut w = BufferWriter::new(zerodds_cdr::Endianness::Little);
        a.encode_into(&mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, zerodds_cdr::Endianness::Little);
        let decoded = AppliedVerbatimAnnotation::decode_from(&mut r).unwrap();
        assert_eq!(decoded, a);
    }

    #[test]
    fn verbatim_placement_roundtrip_before() {
        roundtrip_verbatim(VerbatimPlacement::Before);
    }

    #[test]
    fn verbatim_placement_roundtrip_after() {
        roundtrip_verbatim(VerbatimPlacement::After);
    }

    #[test]
    fn verbatim_placement_roundtrip_begin_file() {
        roundtrip_verbatim(VerbatimPlacement::BeginFile);
    }

    #[test]
    fn verbatim_placement_roundtrip_end_file() {
        roundtrip_verbatim(VerbatimPlacement::EndFile);
    }

    #[test]
    fn verbatim_placement_roundtrip_other_forward_compat() {
        roundtrip_verbatim(VerbatimPlacement::Other(alloc::string::String::from(
            "CUSTOM_PLACEMENT",
        )));
    }
}
