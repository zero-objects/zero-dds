// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `IOP::Codec` and `IOP::CodecFactory` — CORBA 3.3 Part 2 §7.8.
//!
//! A [`Codec`] turns an `any` into the CDR encapsulation the formats of
//! IOR components and service contexts use, and back: [`Codec::encode`]
//! writes the `TypeCode` and the value, [`Codec::encode_value`] the value
//! alone; [`Codec::decode`] and [`Codec::decode_value`] read them.
//! [`CodecFactory::create_codec`] makes one for an [`Encoding`]; the ORB
//! offers the factory as the initial reference `"CodecFactory"` and to
//! ORB initializers as `ORBInitInfo::codec_factory` (Part 1 §16.9.2.5).
//!
//! The factory supports `ENCODING_CDR_ENCAPS` 1.0, 1.1 and 1.2, and 1.3
//! and 1.4, the GIOP versions after 1.2 (§7.8.2.1: "for all future
//! versions of GIOP as they arise"). What differs between them is which
//! types exist: a type Table 7.2 (§7.9) ties to a later GIOP version —
//! `long long`, `wchar`, `wstring`, `fixed` and the other extended types
//! before 1.1, value types and abstract interfaces before 1.2, component
//! TypeCodes before 1.3 — raises `InvalidTypeForEncoding` on encode and
//! makes the octets undecodable on decode. `wchar` and `wstring` take
//! their GIOP 1.2 form in every version that has them: "Wchar values in
//! encapsulations are assumed to be encoded using GIOP version 1.2 and
//! later CDR", and the same for `wstring` (Part 2 §9.3.1.6, §9.3.2.7).
//!
//! An encapsulation starts with the byte-order octet (§9.3.3); this codec
//! writes big-endian (octet 0) and reads both orders. Alignment counts
//! from the byte-order octet.

use alloc::vec::Vec;

use zerodds_corba_giop::Version;
use zerodds_corba_giop::feature::{self, Feature};
use zerodds_corba_rt::{
    AbstractValue, AnyValue, CompletionStatus, CorbaAny, SystemException, SystemExceptionKind,
    TypeCode, ValueRef,
};
use zerodds_wire::{Endianness, Reader, Writer};

/// `IOP::EncodingFormat`.
pub type EncodingFormat = i16;

/// `IOP::ENCODING_CDR_ENCAPS` (§7.8.2): the CDR encapsulation.
pub const ENCODING_CDR_ENCAPS: EncodingFormat = 0;

/// The initial reference of the [`CodecFactory`] (§7.8).
pub const CODEC_FACTORY_INITIAL_REFERENCE: &str = "CodecFactory";

/// `IOP::Encoding` (§7.8.2.1): the format and the version of a codec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Encoding {
    /// `format`.
    pub format: EncodingFormat,
    /// `major_version`.
    pub major_version: u8,
    /// `minor_version`.
    pub minor_version: u8,
}

impl Encoding {
    /// `ENCODING_CDR_ENCAPS` of GIOP `major.minor`.
    #[must_use]
    pub const fn cdr_encaps(major_version: u8, minor_version: u8) -> Self {
        Self {
            format: ENCODING_CDR_ENCAPS,
            major_version,
            minor_version,
        }
    }
}

/// What the operations of `IOP::Codec` raise (§7.8.1.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecError {
    /// `IOP::Codec::InvalidTypeForEncoding`: `encode` or `encode_value`
    /// was given a type that does not exist in the codec's encoding.
    InvalidTypeForEncoding,
    /// `IOP::Codec::FormatMismatch`: the octets cannot be decoded into an
    /// `any`.
    FormatMismatch,
    /// `IOP::Codec::TypeMismatch`: the `TypeCode` given to
    /// `decode_value` does not match the octets.
    TypeMismatch,
    /// A system exception: `MARSHAL` for a value that does not fit its
    /// own `TypeCode` (a union member other than the selected case, value
    /// state that does not match its type).
    System(SystemException),
}

impl CodecError {
    /// The repository id of the exception.
    #[must_use]
    pub const fn repository_id(&self) -> &'static str {
        match self {
            Self::InvalidTypeForEncoding => "IDL:omg.org/IOP/Codec/InvalidTypeForEncoding:1.0",
            Self::FormatMismatch => "IDL:omg.org/IOP/Codec/FormatMismatch:1.0",
            Self::TypeMismatch => "IDL:omg.org/IOP/Codec/TypeMismatch:1.0",
            Self::System(e) => e.repository_id(),
        }
    }
}

impl core::fmt::Display for CodecError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.repository_id())
    }
}

/// `IOP::CodecFactory::UnknownEncoding` (§7.8.2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnknownEncoding;

impl UnknownEncoding {
    /// The repository id of the exception.
    pub const REPOSITORY_ID: &'static str = "IDL:omg.org/IOP/CodecFactory/UnknownEncoding:1.0";
}

impl core::fmt::Display for UnknownEncoding {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(Self::REPOSITORY_ID)
    }
}

/// `IOP::Codec` (§7.8.1) for one [`Encoding`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Codec {
    encoding: Encoding,
}

impl Codec {
    /// The encoding of this codec.
    #[must_use]
    pub const fn encoding(&self) -> Encoding {
        self.encoding
    }

    fn version(&self) -> Version {
        Version::new(self.encoding.major_version, self.encoding.minor_version)
    }

    /// `encode`: the `TypeCode` and the value of `data` in a CDR
    /// encapsulation.
    ///
    /// # Errors
    /// [`CodecError::InvalidTypeForEncoding`] for a type the encoding
    /// does not have; `MARSHAL` for a value that does not fit its
    /// `TypeCode`.
    pub fn encode(&self, data: &CorbaAny) -> Result<Vec<u8>, CodecError> {
        self.check_value(&data.0)?;
        let mut w = encapsulation();
        data.encode(&mut w).map_err(|_| marshal())?;
        Ok(w.finish())
    }

    /// `decode`: the `any` of an encapsulation [`Self::encode`] wrote.
    ///
    /// # Errors
    /// [`CodecError::FormatMismatch`] for an empty sequence, a byte-order
    /// octet other than 0 or 1, octets that do not decode into an `any`,
    /// octets left after it, and an `any` of a type the encoding does not
    /// have.
    pub fn decode(&self, data: &[u8]) -> Result<CorbaAny, CodecError> {
        let mut r = open(data)?;
        let any = CorbaAny::decode(&mut r).map_err(|_| CodecError::FormatMismatch)?;
        if r.remaining() != 0 || self.check_value(&any.0).is_err() {
            return Err(CodecError::FormatMismatch);
        }
        Ok(any)
    }

    /// `encode_value`: the value of `data` alone, without its `TypeCode`,
    /// in a CDR encapsulation.
    ///
    /// # Errors
    /// As [`Self::encode`].
    pub fn encode_value(&self, data: &CorbaAny) -> Result<Vec<u8>, CodecError> {
        self.check_value(&data.0)?;
        let mut w = encapsulation();
        data.encode_value(&mut w).map_err(|_| marshal())?;
        Ok(w.finish())
    }

    /// `decode_value`: the value of type `tc` in an encapsulation
    /// [`Self::encode_value`] wrote.
    ///
    /// # Errors
    /// [`CodecError::FormatMismatch`] for an empty sequence or a
    /// byte-order octet other than 0 or 1; [`CodecError::TypeMismatch`]
    /// when the octets are not a value of `tc`, when octets are left
    /// after it, and for a `tc` with a type the encoding does not have.
    pub fn decode_value(&self, data: &[u8], tc: &TypeCode) -> Result<CorbaAny, CodecError> {
        let mut r = open(data)?;
        if !type_exists(tc, self.version()) {
            return Err(CodecError::TypeMismatch);
        }
        let any = CorbaAny::decode_value(tc, &mut r).map_err(|_| CodecError::TypeMismatch)?;
        if r.remaining() != 0 || self.check_value(&any.0).is_err() {
            return Err(CodecError::TypeMismatch);
        }
        Ok(any)
    }

    /// Every type in `value`, its own and those of the `any`, `TypeCode`
    /// and value-type instances inside it, exists in the encoding.
    fn check_value(&self, value: &AnyValue) -> Result<(), CodecError> {
        let version = self.version();
        if type_exists(&value.type_code(), version) && nested_types_exist(value, version) {
            Ok(())
        } else {
            Err(CodecError::InvalidTypeForEncoding)
        }
    }
}

/// A big-endian encapsulation with its byte-order octet written.
fn encapsulation() -> Writer {
    let mut w = Writer::new(Endianness::Big);
    w.write_u8(0);
    w
}

/// A reader after the byte-order octet of `data`.
fn open(data: &[u8]) -> Result<Reader<'_>, CodecError> {
    let endianness = match data.first() {
        Some(0) => Endianness::Big,
        Some(1) => Endianness::Little,
        _ => return Err(CodecError::FormatMismatch),
    };
    let mut r = Reader::new(data, endianness);
    r.skip_to(1).map_err(|_| CodecError::FormatMismatch)?;
    Ok(r)
}

fn marshal() -> CodecError {
    CodecError::System(SystemException::new(
        SystemExceptionKind::Marshal,
        0,
        CompletionStatus::No,
    ))
}

/// Whether every kind in `tc` exists in GIOP `version` (Table 7.2).
///
/// The `TypeCode`s still to look at wait in a list on the heap, not in
/// nested calls: the stack this takes does not grow with how deep `tc`
/// nests, for a `TypeCode` of the application as for one [`Codec::decode`]
/// read from a peer's octets.
fn type_exists(tc: &TypeCode, version: Version) -> bool {
    let mut pending: Vec<&TypeCode> = alloc::vec![tc];
    while let Some(tc) = pending.pop() {
        if let Some(f) = Feature::of_tckind(tc.tckind())
            && !feature::support(f, version).is_valid()
        {
            return false;
        }
        match tc {
            TypeCode::Sequence { element, .. } | TypeCode::Array { element, .. } => {
                pending.push(element);
            }
            TypeCode::Alias { content, .. } | TypeCode::ValueBox { content, .. } => {
                pending.push(content);
            }
            TypeCode::Struct { members, .. } => pending.extend(members.iter().map(|(_, t)| t)),
            TypeCode::Union {
                discriminator,
                members,
                ..
            } => {
                pending.push(discriminator);
                pending.extend(members.iter().map(|m| &m.member_type));
            }
            TypeCode::Value {
                concrete_base,
                members,
                ..
            } => {
                pending.push(concrete_base);
                pending.extend(members.iter().map(|m| &m.member_type));
            }
            _ => {}
        }
    }
    true
}

/// Whether the types `value` carries inside it — a nested `any`, a
/// `TypeCode` value, the instances of value types — exist in `version`.
///
/// As in [`type_exists`], the parts still to look at wait in a list on
/// the heap.
fn nested_types_exist(value: &AnyValue, version: Version) -> bool {
    let mut pending: Vec<&AnyValue> = alloc::vec![value];
    while let Some(value) = pending.pop() {
        match value {
            AnyValue::Any(inner) => {
                if !type_exists(&inner.0.type_code(), version) {
                    return false;
                }
                pending.push(&inner.0);
            }
            AnyValue::TypeCodeValue(tc) => {
                if !type_exists(tc, version) {
                    return false;
                }
            }
            AnyValue::Seq { items, .. } | AnyValue::Array { items, .. } => pending.extend(items),
            AnyValue::Struct { members, .. } | AnyValue::Except { members, .. } => {
                pending.extend(members.iter().map(|(_, v)| v));
            }
            AnyValue::Union { value, .. } => pending.extend(value.as_deref()),
            AnyValue::Value {
                value: ValueRef::Instance(instance),
                ..
            }
            | AnyValue::Abstract {
                value: AbstractValue::Value(ValueRef::Instance(instance)),
                ..
            } => {
                if !type_exists(&instance.type_code, version) {
                    return false;
                }
                pending.extend(instance.members.iter().map(|(_, v)| v));
            }
            _ => {}
        }
    }
    true
}

/// `IOP::CodecFactory` (§7.8.2).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CodecFactory;

impl CodecFactory {
    /// The encodings this factory makes codecs for:
    /// `ENCODING_CDR_ENCAPS` 1.0 to 1.4 (§7.8.2.1).
    pub const SUPPORTED: [Encoding; 5] = [
        Encoding::cdr_encaps(1, 0),
        Encoding::cdr_encaps(1, 1),
        Encoding::cdr_encaps(1, 2),
        Encoding::cdr_encaps(1, 3),
        Encoding::cdr_encaps(1, 4),
    ];

    /// `create_codec`: a codec of `enc`.
    ///
    /// # Errors
    /// [`UnknownEncoding`] for every encoding outside
    /// [`Self::SUPPORTED`].
    pub fn create_codec(&self, enc: Encoding) -> Result<Codec, UnknownEncoding> {
        if Self::SUPPORTED.contains(&enc) {
            Ok(Codec { encoding: enc })
        } else {
            Err(UnknownEncoding)
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use alloc::boxed::Box;
    use alloc::string::{String, ToString};
    use alloc::vec;
    use zerodds_corba_rt::{UnionLabel, UnionMember, WString};

    fn codec(minor: u8) -> Codec {
        CodecFactory
            .create_codec(Encoding::cdr_encaps(1, minor))
            .unwrap()
    }

    fn point() -> CorbaAny {
        CorbaAny(AnyValue::Struct {
            repo_id: "IDL:demo/Point:1.0".into(),
            name: "Point".into(),
            members: vec![
                ("x".into(), AnyValue::Long(7)),
                ("label".into(), AnyValue::Str("p".into())),
            ],
        })
    }

    #[test]
    fn corba_p2_7_8_1_encode_carries_typecode_and_value() {
        let bytes = codec(2)
            .encode(&CorbaAny(AnyValue::Long(0x0102_0304)))
            .unwrap();
        // Byte-order octet, pad to 4, tk_long (3), the value.
        assert_eq!(bytes, [0, 0, 0, 0, 0, 0, 0, 3, 1, 2, 3, 4]);
        let any = point();
        let bytes = codec(2).encode(&any).unwrap();
        assert_eq!(bytes[0], 0, "big-endian encapsulation");
        // tk_struct (15) after the byte-order octet and padding.
        assert_eq!(bytes[4..8], [0, 0, 0, 15]);
        assert_eq!(codec(2).decode(&bytes).unwrap(), any);
    }

    #[test]
    fn corba_p2_7_8_1_encode_value_carries_the_value_only() {
        let bytes = codec(2)
            .encode_value(&CorbaAny(AnyValue::Long(0x0102_0304)))
            .unwrap();
        assert_eq!(bytes, [0, 0, 0, 0, 1, 2, 3, 4]);
        let any = point();
        let bytes = codec(2).encode_value(&any).unwrap();
        // x, then "p" with its length and terminator.
        assert_eq!(bytes, [0, 0, 0, 0, 0, 0, 0, 7, 0, 0, 0, 2, b'p', 0]);
        let tc = any.0.type_code();
        assert_eq!(codec(2).decode_value(&bytes, &tc).unwrap(), any);
    }

    #[test]
    fn corba_p2_7_8_1_decode_reads_both_byte_orders() {
        // A little-endian encapsulation of the long 5, as another ORB
        // may write it.
        let le = [1, 0, 0, 0, 3, 0, 0, 0, 5, 0, 0, 0];
        assert_eq!(codec(2).decode(&le).unwrap(), CorbaAny(AnyValue::Long(5)));
        assert_eq!(
            codec(2)
                .decode_value(&[1, 0, 0, 0, 5, 0, 0, 0], &TypeCode::Long)
                .unwrap(),
            CorbaAny(AnyValue::Long(5))
        );
    }

    #[test]
    fn corba_p2_7_8_1_wstring_under_cdr_encaps_1_0_is_invalid_type_for_encoding() {
        let wide = CorbaAny(AnyValue::WStr(WString::from("ä")));
        assert_eq!(
            codec(0).encode(&wide).unwrap_err(),
            CodecError::InvalidTypeForEncoding
        );
        assert_eq!(
            codec(0).encode_value(&wide).unwrap_err(),
            CodecError::InvalidTypeForEncoding
        );
        // Inside a sequence, a struct and a nested any.
        for v in [
            AnyValue::Seq {
                element: TypeCode::WChar,
                items: vec![AnyValue::WChar(0x41)],
            },
            AnyValue::Struct {
                repo_id: "IDL:S:1.0".into(),
                name: "S".into(),
                members: vec![("n".into(), AnyValue::LongLong(1))],
            },
            AnyValue::Any(Box::new(wide.clone())),
            AnyValue::TypeCodeValue(TypeCode::Fixed {
                digits: 5,
                scale: 2,
            }),
        ] {
            assert_eq!(
                codec(0).encode(&CorbaAny(v.clone())).unwrap_err(),
                CodecError::InvalidTypeForEncoding,
                "{v:?}"
            );
            codec(1).encode(&CorbaAny(v)).unwrap();
        }
        // The 1.0 types pass.
        codec(0).encode(&point()).unwrap();
        // 1.1 has wstring; it is written in its GIOP 1.2 form (§9.3.2.7:
        // wstrings in encapsulations), length in octets, no terminator.
        let bytes = codec(1).encode_value(&wide).unwrap();
        assert_eq!(bytes, [0, 0, 0, 0, 0, 0, 0, 2, 0x00, 0xe4]);
        assert_eq!(
            codec(1)
                .decode_value(&bytes, &TypeCode::WString(0))
                .unwrap(),
            wide
        );
    }

    #[test]
    fn corba_p2_7_8_1_value_types_need_cdr_encaps_1_2() {
        let boxed = TypeCode::ValueBox {
            repo_id: "IDL:demo/Name:1.0".into(),
            name: "Name".into(),
            content: Box::new(TypeCode::String(0)),
        };
        let v = CorbaAny(AnyValue::Value {
            type_code: boxed.clone(),
            value: ValueRef::Null,
        });
        assert_eq!(
            codec(1).encode(&v).unwrap_err(),
            CodecError::InvalidTypeForEncoding
        );
        let bytes = codec(2).encode(&v).unwrap();
        assert_eq!(codec(2).decode(&bytes).unwrap(), v);
        // The 1.2 octets are not an any of the 1.1 encoding.
        assert_eq!(
            codec(1).decode(&bytes).unwrap_err(),
            CodecError::FormatMismatch
        );
        assert_eq!(
            codec(1)
                .decode_value(&codec(2).encode_value(&v).unwrap(), &boxed)
                .unwrap_err(),
            CodecError::TypeMismatch
        );
    }

    #[test]
    fn corba_p2_7_8_1_component_typecodes_need_cdr_encaps_1_3() {
        let tc = AnyValue::TypeCodeValue(TypeCode::Component {
            repo_id: "IDL:demo/C:1.0".into(),
            name: "C".into(),
        });
        assert_eq!(
            codec(2).encode(&CorbaAny(tc.clone())).unwrap_err(),
            CodecError::InvalidTypeForEncoding
        );
        let bytes = codec(3).encode(&CorbaAny(tc.clone())).unwrap();
        assert_eq!(codec(3).decode(&bytes).unwrap(), CorbaAny(tc.clone()));
        codec(4).encode(&CorbaAny(tc)).unwrap();
    }

    #[test]
    fn corba_p2_7_8_1_decode_of_garbage_is_format_mismatch() {
        let c = codec(2);
        assert_eq!(c.decode(&[]).unwrap_err(), CodecError::FormatMismatch);
        // Byte-order octet 2.
        assert_eq!(
            c.decode(&[2, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 1]).unwrap_err(),
            CodecError::FormatMismatch
        );
        // An unknown TCKind.
        assert_eq!(
            c.decode(&[0, 0, 0, 0, 0, 0, 0, 0x7f]).unwrap_err(),
            CodecError::FormatMismatch
        );
        // A long cut short.
        assert_eq!(
            c.decode(&[0, 0, 0, 0, 0, 0, 0, 3, 0, 0]).unwrap_err(),
            CodecError::FormatMismatch
        );
        // Octets after the any.
        assert_eq!(
            c.decode(&[0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 1, 9])
                .unwrap_err(),
            CodecError::FormatMismatch
        );
        assert_eq!(
            c.decode_value(&[], &TypeCode::Long).unwrap_err(),
            CodecError::FormatMismatch
        );
    }

    #[test]
    fn corba_p2_7_8_1_decode_value_with_a_foreign_typecode_is_type_mismatch() {
        let c = codec(2);
        let bytes = c.encode_value(&point()).unwrap();
        // Shorter than the TypeCode needs.
        let wide = TypeCode::Struct {
            repo_id: "IDL:demo/Wide:1.0".into(),
            name: "Wide".into(),
            members: vec![
                ("a".into(), TypeCode::LongLong),
                ("b".into(), TypeCode::LongLong),
            ],
            is_except: false,
        };
        assert_eq!(
            c.decode_value(&bytes, &wide).unwrap_err(),
            CodecError::TypeMismatch
        );
        // Octets left over.
        assert_eq!(
            c.decode_value(&bytes, &TypeCode::Long).unwrap_err(),
            CodecError::TypeMismatch
        );
        // A boolean that is not 0 or 1.
        assert_eq!(
            c.decode_value(&[0, 7], &TypeCode::Boolean).unwrap_err(),
            CodecError::TypeMismatch
        );
        // A type the encoding does not have.
        assert_eq!(
            codec(0)
                .decode_value(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1], &TypeCode::LongLong)
                .unwrap_err(),
            CodecError::TypeMismatch
        );
    }

    #[test]
    fn corba_p2_7_8_1_union_and_sequence_round_trip() {
        let union_tc = TypeCode::Union {
            repo_id: "IDL:demo/U:1.0".into(),
            name: "U".into(),
            discriminator: Box::new(TypeCode::Long),
            default_index: -1,
            members: vec![
                UnionMember {
                    label: UnionLabel::Long(1),
                    name: "n".into(),
                    member_type: TypeCode::Long,
                },
                UnionMember {
                    label: UnionLabel::Long(2),
                    name: "s".into(),
                    member_type: TypeCode::String(0),
                },
            ],
        };
        let u = CorbaAny(AnyValue::Union {
            type_code: union_tc.clone(),
            discriminator: UnionLabel::Long(2),
            value: Some(Box::new(AnyValue::Str("two".to_string()))),
        });
        let seq = CorbaAny(AnyValue::Seq {
            element: TypeCode::Short,
            items: vec![AnyValue::Short(-1), AnyValue::Short(2)],
        });
        for minor in 0..=4 {
            let c = codec(minor);
            for any in [&u, &seq] {
                assert_eq!(&c.decode(&c.encode(any).unwrap()).unwrap(), any);
                assert_eq!(
                    &c.decode_value(&c.encode_value(any).unwrap(), &any.0.type_code())
                        .unwrap(),
                    any
                );
            }
        }
        // A union value whose discriminator selects another member than
        // the value given: MARSHAL.
        let bad = CorbaAny(AnyValue::Union {
            type_code: union_tc,
            discriminator: UnionLabel::Long(1),
            value: Some(Box::new(AnyValue::Str("x".into()))),
        });
        assert!(matches!(
            codec(2).encode(&bad).unwrap_err(),
            CodecError::System(e) if e.kind == SystemExceptionKind::Marshal
        ));
    }

    #[test]
    fn corba_p2_7_8_2_factory_supports_cdr_encaps_1_0_to_1_4_and_refuses_others() {
        for minor in 0..=4 {
            let enc = Encoding::cdr_encaps(1, minor);
            assert_eq!(CodecFactory.create_codec(enc).unwrap().encoding(), enc);
        }
        for enc in [
            Encoding::cdr_encaps(1, 5),
            Encoding::cdr_encaps(2, 0),
            Encoding::cdr_encaps(0, 9),
            Encoding {
                format: 1,
                major_version: 1,
                minor_version: 2,
            },
        ] {
            assert_eq!(
                CodecFactory.create_codec(enc),
                Err(UnknownEncoding),
                "{enc:?}"
            );
        }
        assert_eq!(ENCODING_CDR_ENCAPS, 0);
        assert_eq!(
            UnknownEncoding.to_string(),
            "IDL:omg.org/IOP/CodecFactory/UnknownEncoding:1.0"
        );
        assert_eq!(
            CodecError::FormatMismatch.repository_id(),
            "IDL:omg.org/IOP/Codec/FormatMismatch:1.0"
        );
        let _ = String::new();
    }
}
