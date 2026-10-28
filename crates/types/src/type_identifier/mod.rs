// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! XTypes 1.3 TypeIdentifier (Spec §7.3.4.2).
//!
//! TypeIdentifier is a CDR union with an `octet` discriminator. It
//! identifies a type either **directly** (primitives, plain
//! collections) or **indirectly** via a 14-byte SHA-256 hash of the
//! serialized TypeObject (EK_MINIMAL / EK_COMPLETE).
//!
//! # Wire-Encoding (XCDR2 LE)
//!
//! ```text
//! TypeIdentifier {
//!     octet _d;            // 1 byte discriminator
//!     // body depending on _d:
//!     // TK_NONE..TK_CHAR16:          no body
//!     // TI_STRING8_SMALL/LE_SMALL:   { octet bound; }
//!     // TI_STRING8_LARGE/LE_LARGE:   { uint32 bound; }  // 4-byte aligned
//!     // TI_PLAIN_SEQUENCE_SMALL:     { PlainCollectionHeader; octet bound; @external TypeIdentifier elem; }
//!     // TI_PLAIN_SEQUENCE_LARGE:     { PlainCollectionHeader; uint32 bound; @external TypeIdentifier elem; }
//!     // TI_PLAIN_ARRAY_SMALL:        { PlainCollectionHeader; seq<octet,5> dims; @external TypeIdentifier elem; }
//!     // TI_PLAIN_ARRAY_LARGE:        { PlainCollectionHeader; seq<uint32,5> dims; @external TypeIdentifier elem; }
//!     // TI_PLAIN_MAP_SMALL:          { PlainCollectionHeader; octet bound;
//!     //                                @external TypeIdentifier elem;
//!     //                                CollectionElementFlag key_flags;
//!     //                                @external TypeIdentifier key; }
//!     // TI_PLAIN_MAP_LARGE:          { ... uint32 bound ... }
//!     // TI_STRONGLY_CONNECTED_COMPONENT: { SCC_ID scc_id; }
//!     // EK_MINIMAL / EK_COMPLETE:    { octet hash[14]; }
//! }
//! ```

pub mod kinds;

use alloc::boxed::Box;
use alloc::vec::Vec;

use zerodds_wire::{Reader, WireError, Writer};

use self::kinds::{
    EK_BOTH, EK_COMPLETE, EK_MINIMAL, EQUIVALENCE_HASH_LEN, TI_PLAIN_ARRAY_LARGE,
    TI_PLAIN_ARRAY_SMALL, TI_PLAIN_MAP_LARGE, TI_PLAIN_MAP_SMALL, TI_PLAIN_SEQUENCE_LARGE,
    TI_PLAIN_SEQUENCE_SMALL, TI_STRING8_LARGE, TI_STRING8_SMALL, TI_STRING16_LARGE,
    TI_STRING16_SMALL, TI_STRONGLY_CONNECTED_COMPONENT, TK_BOOLEAN, TK_BYTE, TK_CHAR8, TK_CHAR16,
    TK_FLOAT32, TK_FLOAT64, TK_FLOAT128, TK_INT8, TK_INT16, TK_INT32, TK_INT64, TK_NONE, TK_UINT8,
    TK_UINT16, TK_UINT32, TK_UINT64,
};

/// 14-byte SHA256 hash of a serialized TypeObject.
///
/// Spec §7.3.1.2: the hash is formed from the **first 14 bytes** of the
/// SHA-256 over the XCDR2-serialized TypeObject.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EquivalenceHash(pub [u8; EQUIVALENCE_HASH_LEN]);

impl EquivalenceHash {
    /// Null hash (placeholder).
    pub const ZERO: Self = Self([0; EQUIVALENCE_HASH_LEN]);
}

/// Collection element flags (Annex B). A 16-bit bitmask with only one
/// relevant bit (TRY_CONSTRUCT) for TypeIdentifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CollectionElementFlag(pub u16);

impl CollectionElementFlag {
    /// TryConstruct = DISCARD (the default TryConstructKind, §7.3.4.5), set on
    /// the element of every plain collection by Cyclone + FastDDS
    /// (byte-verified). Bit position is shared with the member-flag types.
    pub const TRY_CONSTRUCT1: u16 = 1 << 0;
    /// `TRY_CONSTRUCT2`, the second bit of the try-construct pair.
    pub const TRY_CONSTRUCT2: u16 = 1 << 1;
    /// `IS_EXTERNAL`: the elements of the collection are `@external`
    /// (Annex B, `MemberFlag` position 2; `CollectionElementFlag` is
    /// "T1, T2, X").
    pub const IS_EXTERNAL: u16 = 1 << 2;

    /// The element flags as emitted on the wire: a bare DISCARD.
    #[must_use]
    pub const fn discard() -> Self {
        Self(Self::TRY_CONSTRUCT1)
    }
}

/// Equivalence kind of the element of a PlainCollection (Annex B).
///
/// EK_MINIMAL = the TI is strongly-hashed minimal, EK_COMPLETE =
/// complete, EK_BOTH = identical for both, `None` = the element is
/// itself primitive or plain (no strong hash).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EquivalenceKind {
    /// The element is a primitive or otherwise plain — no hash.
    None,
    /// The element TypeIdentifier is EK_MINIMAL.
    Minimal,
    /// The element TypeIdentifier is EK_COMPLETE.
    Complete,
    /// Minimal + complete are the same (e.g. for fully primitive types).
    Both,
}

impl EquivalenceKind {
    /// Encoded as `octet` (Annex B EquivalenceKind).
    #[must_use]
    pub const fn to_u8(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Minimal => EK_MINIMAL,
            Self::Complete => EK_COMPLETE,
            Self::Both => 0xF3,
        }
    }

    /// Decode.
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v {
            EK_MINIMAL => Self::Minimal,
            EK_COMPLETE => Self::Complete,
            0xF3 => Self::Both,
            _ => Self::None,
        }
    }
}

/// Header for plain collections (Annex B).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PlainCollectionHeader {
    /// Equivalence kind of the element TypeIdentifier.
    pub equiv_kind: u8,
    /// Try-construct flags.
    pub element_flags: CollectionElementFlag,
}

impl PlainCollectionHeader {
    /// Builds the header the way the vendors do (byte-verified against
    /// Cyclone + FastDDS): the given `equiv_kind` plus a bare DISCARD in the
    /// element flags. `equiv_kind` is `EK_BOTH` (0xF3) for a fully-descriptive
    /// element (primitive/string/nested-plain), or `EK_MINIMAL`/`EK_COMPLETE`
    /// when the element is a strong hash.
    #[must_use]
    pub const fn for_element(equiv_kind: u8) -> Self {
        Self {
            equiv_kind,
            element_flags: CollectionElementFlag::discard(),
        }
    }
}

/// `StronglyConnectedComponentId` — Annex B, verbatim:
///
/// ```idl
/// @extensibility(APPENDABLE) @nested
/// struct StronglyConnectedComponentId {
///     TypeObjectHashId sc_component_id;
///     long             scc_length;
///     long             scc_index;
/// };
/// ```
///
/// with
///
/// ```idl
/// @extensibility(FINAL) @nested
/// union TypeObjectHashId switch (octet) {
///     case EK_COMPLETE:
///     case EK_MINIMAL:
///         EquivalenceHash hash;
/// };
/// ```
///
/// Two things follow from that and neither was here before. The
/// `TypeObjectHashId` is a union, so the fourteen hash octets are
/// preceded by their **equivalence kind** — and §7.3.4.6 tells the two
/// forms apart by exactly that: "those with discriminator
/// `TI_STRONG_COMPONENT` where the contained `TypeObjectHashId` has
/// discriminator `EK_MINIMAL`". Without the field a minimal and a
/// complete component identifier were the same value. And the structure
/// is `APPENDABLE` in a file whose head says "shall be serialized with
/// XCDR encoding version 2", so §7.4.2 gives it a `DHEADER`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StronglyConnectedComponentId {
    /// The `TypeObjectHashId` discriminator: `EK_MINIMAL` or
    /// `EK_COMPLETE`.
    pub equivalence_kind: u8,
    /// The component's 14-octet hash.
    pub hash: EquivalenceHash,
    /// Number of types in the component.
    pub scc_length: i32,
    /// Index of this type in the component, numbered from one
    /// (§7.3.4.9.2 step 4b); 0 in an `SCCIdentifier`, which names the
    /// component without naming a type in it (§7.3.4.9.3).
    pub scc_index: i32,
}

/// TypeIdentifier — XTypes §7.3.4.2.
///
/// Identifies primitive and plain types directly, composite types
/// (struct, union, etc.) indirectly (via a 14-byte hash). Plain
/// collections can recursively contain a TypeIdentifier, hence `Box`
/// for the nested variants.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum TypeIdentifier {
    /// TK_NONE — spec sentinel for "no TypeIdentifier known".
    #[default]
    None,
    /// Primitive without a body — the discriminator carries all info.
    Primitive(PrimitiveKind),
    /// `string<Bound>` with 8-bit characters, Bound <= 255 bytes.
    String8Small {
        /// Max length (0 = unbounded).
        bound: u8,
    },
    /// `string<Bound>` with Bound > 255.
    String8Large {
        /// Max length.
        bound: u32,
    },
    /// `wstring<Bound>` with Bound <= 255 chars.
    String16Small {
        /// Max length.
        bound: u8,
    },
    /// `wstring<Bound>`.
    String16Large {
        /// Max length.
        bound: u32,
    },
    /// `sequence<T, N>` with N <= 255.
    PlainSequenceSmall {
        /// Header (EquivKind of the element + flags).
        header: PlainCollectionHeader,
        /// Maximum bound (0 = unbounded).
        bound: u8,
        /// Element TypeIdentifier.
        element: Box<TypeIdentifier>,
    },
    /// `sequence<T, N>` with N > 255.
    PlainSequenceLarge {
        /// Header.
        header: PlainCollectionHeader,
        /// Max bound.
        bound: u32,
        /// Element TypeIdentifier.
        element: Box<TypeIdentifier>,
    },
    /// `T[D1, D2, ...]` with all dimensions <= 255.
    PlainArraySmall {
        /// Header.
        header: PlainCollectionHeader,
        /// Array dimensions.
        array_bounds: Vec<u8>,
        /// Element TypeIdentifier.
        element: Box<TypeIdentifier>,
    },
    /// `T[D1, D2, ...]` with at least one dimension > 255.
    PlainArrayLarge {
        /// Header.
        header: PlainCollectionHeader,
        /// Array dimensions.
        array_bounds: Vec<u32>,
        /// Element TypeIdentifier.
        element: Box<TypeIdentifier>,
    },
    /// `map<K, V, N>` with N <= 255.
    PlainMapSmall {
        /// Header.
        header: PlainCollectionHeader,
        /// Max size.
        bound: u8,
        /// Value TypeIdentifier.
        element: Box<TypeIdentifier>,
        /// Key flags.
        key_flags: CollectionElementFlag,
        /// Key TypeIdentifier.
        key: Box<TypeIdentifier>,
    },
    /// `map<K, V, N>` with N > 255.
    PlainMapLarge {
        /// Header.
        header: PlainCollectionHeader,
        /// Max size.
        bound: u32,
        /// Value TypeIdentifier.
        element: Box<TypeIdentifier>,
        /// Key flags.
        key_flags: CollectionElementFlag,
        /// Key TypeIdentifier.
        key: Box<TypeIdentifier>,
    },
    /// Strongly connected component (recursive types) — §7.3.4.9.
    StronglyConnectedComponent(StronglyConnectedComponentId),
    /// 14-byte hash of the MinimalTypeObject.
    EquivalenceHashMinimal(EquivalenceHash),
    /// 14-byte hash of the CompleteTypeObject.
    EquivalenceHashComplete(EquivalenceHash),
    /// The `default:` arm of Annex B's `union TypeIdentifier`:
    /// `ExtendedTypeDefn extended_defn`, under the comment
    /// `// ============ Future extensibility ============`.
    Extended {
        /// The discriminator the stream carried.
        discriminator: u8,
        /// The `ExtendedTypeDefn` member list, verbatim — the
        /// `DHEADER` excluded, because that is a length this encoder
        /// writes and a byte order this value does not carry.
        ///
        /// Annex B's `default: ExtendedTypeDefn extended_defn;` is a
        /// `@extensibility(MUTABLE) @nested struct` — "Empty. Available
        /// for future extension" — so under encoding version 2 the arm
        /// is a `DHEADER` over a member list this revision has no member
        /// of. Holding it verbatim is what lets the identifier be
        /// written back as it arrived; reading it and dropping it moved
        /// every field after this one in the stream, and reported
        /// nothing.
        body: alloc::vec::Vec<u8>,
    },
}

/// Primitive kind (no body in the TypeIdentifier, only the discriminator).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimitiveKind {
    /// `bool`.
    Boolean,
    /// `octet` / `byte`.
    Byte,
    /// `int8`.
    Int8,
    /// `int16`.
    Int16,
    /// `int32`.
    Int32,
    /// `int64`.
    Int64,
    /// `uint8`.
    UInt8,
    /// `uint16`.
    UInt16,
    /// `uint32`.
    UInt32,
    /// `uint64`.
    UInt64,
    /// `float32`.
    Float32,
    /// `float64`.
    Float64,
    /// `float128`.
    Float128,
    /// `char` (8-bit).
    Char8,
    /// `wchar` (16-bit).
    Char16,
}

impl PrimitiveKind {
    /// Discriminator byte in TypeIdentifier.
    #[must_use]
    pub const fn to_u8(self) -> u8 {
        match self {
            Self::Boolean => TK_BOOLEAN,
            Self::Byte => TK_BYTE,
            Self::Int8 => TK_INT8,
            Self::Int16 => TK_INT16,
            Self::Int32 => TK_INT32,
            Self::Int64 => TK_INT64,
            Self::UInt8 => TK_UINT8,
            Self::UInt16 => TK_UINT16,
            Self::UInt32 => TK_UINT32,
            Self::UInt64 => TK_UINT64,
            Self::Float32 => TK_FLOAT32,
            Self::Float64 => TK_FLOAT64,
            Self::Float128 => TK_FLOAT128,
            Self::Char8 => TK_CHAR8,
            Self::Char16 => TK_CHAR16,
        }
    }

    /// Attempts to read a primitive kind from a discriminator byte.
    #[must_use]
    pub const fn from_u8(v: u8) -> Option<Self> {
        Some(match v {
            TK_BOOLEAN => Self::Boolean,
            TK_BYTE => Self::Byte,
            TK_INT8 => Self::Int8,
            TK_INT16 => Self::Int16,
            TK_INT32 => Self::Int32,
            TK_INT64 => Self::Int64,
            TK_UINT8 => Self::UInt8,
            TK_UINT16 => Self::UInt16,
            TK_UINT32 => Self::UInt32,
            TK_UINT64 => Self::UInt64,
            TK_FLOAT32 => Self::Float32,
            TK_FLOAT64 => Self::Float64,
            TK_FLOAT128 => Self::Float128,
            TK_CHAR8 => Self::Char8,
            TK_CHAR16 => Self::Char16,
            _ => return None,
        })
    }
}

// ============================================================================
// Wire-Encoding
// ============================================================================

impl TypeIdentifier {
    /// Encoded as XCDR2 little-endian bytes (TypeIdentifier without an
    /// encapsulation header — that is provided by the caller, e.g. the
    /// TYPE_INFORMATION PID or the TypeLookup RPC).
    ///
    /// # Errors
    /// `WireError` on buffer overflow (very unlikely for normal types;
    /// the 2^32 limit for large kinds).
    pub fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        let d = self.discriminator();
        w.write_u8(d);
        match self {
            Self::None | Self::Primitive(_) => Ok(()),
            // The `ExtendedTypeDefn` member list goes back the way it
            // came, behind a `DHEADER` this encoder writes. Holding the
            // list rather than the framed bytes keeps the value free of
            // a byte order: `encode_appendable` writes the length in the
            // writer's, whatever that is.
            Self::Extended { body, .. } => crate::framing::encode_extended_members(w, body),
            Self::String8Small { bound } | Self::String16Small { bound } => {
                w.write_u8(*bound);
                Ok(())
            }
            Self::String8Large { bound } | Self::String16Large { bound } => {
                w.write_u32(*bound);
                Ok(())
            }
            Self::PlainSequenceSmall {
                header,
                bound,
                element,
            } => {
                encode_collection_header(w, *header)?;
                w.write_u8(*bound);
                element.encode_into(w)
            }
            Self::PlainSequenceLarge {
                header,
                bound,
                element,
            } => {
                encode_collection_header(w, *header)?;
                w.write_u32(*bound);
                element.encode_into(w)
            }
            Self::PlainArraySmall {
                header,
                array_bounds,
                element,
            } => {
                encode_collection_header(w, *header)?;
                // sequence<octet> — 4-byte length + bytes (XCDR2 LE).
                let len = u32::try_from(array_bounds.len()).map_err(|_| WireError::OutOfRange {
                    at: w.position(),
                    expected: alloc::string::String::from(
                        "plain array dimensions length exceeds u32::MAX",
                    ),
                })?;
                w.write_u32(len);
                w.write_bytes(array_bounds);
                element.encode_into(w)
            }
            Self::PlainArrayLarge {
                header,
                array_bounds,
                element,
            } => {
                encode_collection_header(w, *header)?;
                let len = u32::try_from(array_bounds.len()).map_err(|_| WireError::OutOfRange {
                    at: w.position(),
                    expected: alloc::string::String::from(
                        "plain array dimensions length exceeds u32::MAX",
                    ),
                })?;
                w.write_u32(len);
                for dim in array_bounds {
                    w.write_u32(*dim);
                }
                element.encode_into(w)
            }
            Self::PlainMapSmall {
                header,
                bound,
                element,
                key_flags,
                key,
            } => {
                encode_collection_header(w, *header)?;
                w.write_u8(*bound);
                element.encode_into(w)?;
                w.write_u16(key_flags.0);
                key.encode_into(w)
            }
            Self::PlainMapLarge {
                header,
                bound,
                element,
                key_flags,
                key,
            } => {
                encode_collection_header(w, *header)?;
                w.write_u32(*bound);
                element.encode_into(w)?;
                w.write_u16(key_flags.0);
                key.encode_into(w)
            }
            Self::StronglyConnectedComponent(scc) => {
                // XTypes 1.3 §7.3.4.9.2 step 4b numbers a component's
                // types "starting with index 1 for the first type", so a
                // type's index is 1..=scc_length and a component holds at
                // least one type. §7.3.4.9.3: "There are situations where
                // an SCC needs to be identified without referencing a
                // concrete type inside the SCC. In this situation a
                // TypeIdentifier is constructed the same way as for any
                // of the types in the SCC except the scc_index field is
                // set to 0" — the SCCIdentifier — so 0 is written too.
                //
                // The bound still exists for the reason it always did: a
                // negative value would otherwise leave here as a huge
                // u32 through two's complement.
                if scc.scc_length < 1 || scc.scc_index < 0 || scc.scc_index > scc.scc_length {
                    return Err(WireError::OutOfRange {
                        at: w.position(),
                        expected: alloc::string::String::from("SCC scc_length/scc_index invalid"),
                    });
                }
                if scc.equivalence_kind != EK_MINIMAL && scc.equivalence_kind != EK_COMPLETE {
                    return Err(WireError::OutOfRange {
                        at: w.position(),
                        expected: alloc::format!(
                            "a TypeObjectHashId discriminator of EK_MINIMAL or EK_COMPLETE, \
                             and 0x{:02x} is neither",
                            scc.equivalence_kind
                        ),
                    });
                }
                // `@extensibility(APPENDABLE)` under encoding version 2:
                // a DHEADER over the `TypeObjectHashId`, `scc_length`
                // and `scc_index` that follow.
                crate::framing::encode_appendable(w, |w| {
                    w.write_u8(scc.equivalence_kind);
                    w.write_bytes(&scc.hash.0);
                    w.write_u32(scc.scc_length as u32);
                    w.write_u32(scc.scc_index as u32);
                    Ok(())
                })
            }
            Self::EquivalenceHashMinimal(h) | Self::EquivalenceHashComplete(h) => {
                w.write_bytes(&h.0);
                Ok(())
            }
        }
    }

    /// Maximum recursion depth when wire-decoding a nested
    /// `TypeIdentifier` (plain collections can reference recursively).
    /// Protects against stack overflow on pathological datagrams.
    pub const MAX_DECODE_DEPTH: usize = 16;

    /// Decode from XCDR2 little-endian bytes. The reader must be
    /// positioned at the discriminator. Internal recursion is capped
    /// (see [`Self::MAX_DECODE_DEPTH`]).
    ///
    /// # Errors
    /// `WireError` on buffer underflow, an inconsistent length field
    /// or when the maximum recursion depth is exceeded.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, WireError> {
        Self::decode_with_depth(r, 0)
    }

    fn decode_with_depth(r: &mut Reader<'_>, depth: usize) -> Result<Self, WireError> {
        if depth >= Self::MAX_DECODE_DEPTH {
            return Err(WireError::OutOfRange {
                at: r.position(),
                expected: alloc::format!(
                    "a TypeIdentifier nested fewer than {} deep",
                    Self::MAX_DECODE_DEPTH
                ),
            });
        }
        let d = r.read_u8()?;
        Ok(match d {
            TK_NONE => Self::None,
            TK_BOOLEAN | TK_BYTE | TK_INT8 | TK_INT16 | TK_INT32 | TK_INT64 | TK_UINT8
            | TK_UINT16 | TK_UINT32 | TK_UINT64 | TK_FLOAT32 | TK_FLOAT64 | TK_FLOAT128
            | TK_CHAR8 | TK_CHAR16 => {
                // Primitive discriminator → no body. The outer match
                // arm ensures `from_u8` returns `Some(..)`; a kind added
                // to the list and forgotten in `from_u8` would otherwise
                // become a primitive of the wrong width, so it is
                // refused by name instead.
                let Some(p) = PrimitiveKind::from_u8(d) else {
                    return Err(WireError::OutOfRange {
                        at: r.position() - 1,
                        expected: alloc::format!(
                            "a primitive TypeKind `PrimitiveKind::from_u8` knows, \
                             and 0x{d:02x} is one this build lists but cannot decode"
                        ),
                    });
                };
                Self::Primitive(p)
            }
            TI_STRING8_SMALL => Self::String8Small {
                bound: r.read_u8()?,
            },
            TI_STRING8_LARGE => Self::String8Large {
                bound: r.read_u32()?,
            },
            TI_STRING16_SMALL => Self::String16Small {
                bound: r.read_u8()?,
            },
            TI_STRING16_LARGE => Self::String16Large {
                bound: r.read_u32()?,
            },
            TI_PLAIN_SEQUENCE_SMALL => {
                let header = decode_collection_header(r)?;
                let bound = r.read_u8()?;
                let element = Box::new(Self::decode_with_depth(r, depth + 1)?);
                Self::PlainSequenceSmall {
                    header,
                    bound,
                    element,
                }
            }
            TI_PLAIN_SEQUENCE_LARGE => {
                let header = decode_collection_header(r)?;
                let bound = r.read_u32()?;
                let element = Box::new(Self::decode_with_depth(r, depth + 1)?);
                Self::PlainSequenceLarge {
                    header,
                    bound,
                    element,
                }
            }
            TI_PLAIN_ARRAY_SMALL => {
                let header = decode_collection_header(r)?;
                let n = r.read_u32()? as usize;
                let array_bounds = r.read_bytes(n)?.to_vec();
                let element = Box::new(Self::decode_with_depth(r, depth + 1)?);
                Self::PlainArraySmall {
                    header,
                    array_bounds,
                    element,
                }
            }
            TI_PLAIN_ARRAY_LARGE => {
                let header = decode_collection_header(r)?;
                let n = r.read_u32()? as usize;
                let cap = crate::type_object::common::safe_capacity(n, 4, r.remaining());
                let mut array_bounds = Vec::with_capacity(cap);
                for _ in 0..n {
                    array_bounds.push(r.read_u32()?);
                }
                let element = Box::new(Self::decode_with_depth(r, depth + 1)?);
                Self::PlainArrayLarge {
                    header,
                    array_bounds,
                    element,
                }
            }
            TI_PLAIN_MAP_SMALL => {
                let header = decode_collection_header(r)?;
                let bound = r.read_u8()?;
                let element = Box::new(Self::decode_with_depth(r, depth + 1)?);
                let key_flags = CollectionElementFlag(r.read_u16()?);
                let key = Box::new(Self::decode_with_depth(r, depth + 1)?);
                Self::PlainMapSmall {
                    header,
                    bound,
                    element,
                    key_flags,
                    key,
                }
            }
            TI_PLAIN_MAP_LARGE => {
                let header = decode_collection_header(r)?;
                let bound = r.read_u32()?;
                let element = Box::new(Self::decode_with_depth(r, depth + 1)?);
                let key_flags = CollectionElementFlag(r.read_u16()?);
                let key = Box::new(Self::decode_with_depth(r, depth + 1)?);
                Self::PlainMapLarge {
                    header,
                    bound,
                    element,
                    key_flags,
                    key,
                }
            }
            TI_STRONGLY_CONNECTED_COMPONENT => crate::framing::decode_appendable(r, |r| {
                let at = r.position();
                let equivalence_kind = r.read_u8()?;
                if equivalence_kind != EK_MINIMAL && equivalence_kind != EK_COMPLETE {
                    return Err(WireError::OutOfRange {
                        at,
                        expected: alloc::format!(
                            "a TypeObjectHashId discriminator of EK_MINIMAL or \
                                 EK_COMPLETE, and 0x{equivalence_kind:02x} is neither"
                        ),
                    });
                }
                let hash_bytes = r.read_bytes(EQUIVALENCE_HASH_LEN)?;
                let Ok(h): Result<[u8; EQUIVALENCE_HASH_LEN], _> = hash_bytes.try_into() else {
                    return Err(WireError::Truncated {
                        at: r.position(),
                        needed: EQUIVALENCE_HASH_LEN,
                        available: 0,
                    });
                };
                let scc_length = r.read_u32()? as i32;
                let scc_index = r.read_u32()? as i32;
                Ok(Self::StronglyConnectedComponent(
                    StronglyConnectedComponentId {
                        equivalence_kind,
                        hash: EquivalenceHash(h),
                        scc_length,
                        scc_index,
                    },
                ))
            })?,
            EK_MINIMAL | EK_COMPLETE => {
                let hash_bytes = r.read_bytes(EQUIVALENCE_HASH_LEN)?;
                let Ok(h): Result<[u8; EQUIVALENCE_HASH_LEN], _> = hash_bytes.try_into() else {
                    return Err(WireError::Truncated {
                        at: r.position(),
                        needed: EQUIVALENCE_HASH_LEN,
                        available: 0,
                    });
                };
                if d == EK_MINIMAL {
                    Self::EquivalenceHashMinimal(EquivalenceHash(h))
                } else {
                    Self::EquivalenceHashComplete(EquivalenceHash(h))
                }
            }
            // Annex B gives `union TypeIdentifier` a case for
            // `EK_COMPLETE` and one for `EK_MINIMAL` and none for
            // `EK_BOTH = 0xF3`; `EK_BOTH` belongs in a
            // `PlainCollectionHeader`, where a fully descriptive element
            // needs no equivalence form, and not at this discriminator.
            // Reading it as the `default:` arm took the fourteen hash
            // octets that follow it in a stream that carries one and
            // gave them back as nothing.
            EK_BOTH => {
                return Err(WireError::OutOfRange {
                    at: r.position() - 1,
                    expected: alloc::string::String::from(
                        "a TypeIdentifier discriminator Annex B has a case for, and \
                         EK_BOTH (0xF3) is only a PlainCollectionHeader's equivalence kind",
                    ),
                });
            }
            // `default: ExtendedTypeDefn extended_defn;`, which is
            // `@extensibility(MUTABLE)` — a DHEADER over a member list,
            // aligned to four after the octet discriminator. Reading the
            // discriminator alone left the body in the stream, and at an
            // embedded position — a plain sequence's element type, say —
            // that moved every field after it with no error anywhere.
            other => Self::Extended {
                discriminator: other,
                body: crate::framing::decode_extended_members(r)?,
            },
        })
    }

    /// The discriminator byte for this TypeIdentifier.
    #[must_use]
    pub const fn discriminator(&self) -> u8 {
        match self {
            Self::None => TK_NONE,
            Self::Primitive(p) => p.to_u8(),
            Self::String8Small { .. } => TI_STRING8_SMALL,
            Self::String8Large { .. } => TI_STRING8_LARGE,
            Self::String16Small { .. } => TI_STRING16_SMALL,
            Self::String16Large { .. } => TI_STRING16_LARGE,
            Self::PlainSequenceSmall { .. } => TI_PLAIN_SEQUENCE_SMALL,
            Self::PlainSequenceLarge { .. } => TI_PLAIN_SEQUENCE_LARGE,
            Self::PlainArraySmall { .. } => TI_PLAIN_ARRAY_SMALL,
            Self::PlainArrayLarge { .. } => TI_PLAIN_ARRAY_LARGE,
            Self::PlainMapSmall { .. } => TI_PLAIN_MAP_SMALL,
            Self::PlainMapLarge { .. } => TI_PLAIN_MAP_LARGE,
            Self::StronglyConnectedComponent(_) => TI_STRONGLY_CONNECTED_COMPONENT,
            Self::EquivalenceHashMinimal(_) => EK_MINIMAL,
            Self::EquivalenceHashComplete(_) => EK_COMPLETE,
            Self::Extended { discriminator, .. } => *discriminator,
        }
    }

    /// Short form: encode into a fresh XCDR2 writer, little-endian.
    ///
    /// # Errors
    /// `WireError` on overflow.
    pub fn to_bytes_le(&self) -> Result<Vec<u8>, WireError> {
        let mut w = crate::framing::xcdr2_writer();
        self.encode_into(&mut w)?;
        Ok(w.finish())
    }

    /// Short form: decode from LE bytes.
    ///
    /// # Errors
    /// `WireError`.
    pub fn from_bytes_le(bytes: &[u8]) -> Result<Self, WireError> {
        let mut r = crate::framing::xcdr2_reader(bytes);
        Self::decode_from(&mut r)
    }
}

fn encode_collection_header(
    w: &mut Writer,
    header: PlainCollectionHeader,
) -> Result<(), WireError> {
    w.write_u8(header.equiv_kind);
    {
        w.write_u16(header.element_flags.0);
        Ok(())
    }
}

fn decode_collection_header(r: &mut Reader<'_>) -> Result<PlainCollectionHeader, WireError> {
    let equiv_kind = r.read_u8()?;
    let element_flags = CollectionElementFlag(r.read_u16()?);
    Ok(PlainCollectionHeader {
        equiv_kind,
        element_flags,
    })
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn roundtrip(ti: TypeIdentifier) {
        let bytes = ti.to_bytes_le().unwrap();
        let decoded = TypeIdentifier::from_bytes_le(&bytes).unwrap();
        assert_eq!(ti, decoded);
    }

    #[test]
    fn primitive_none_roundtrips() {
        roundtrip(TypeIdentifier::None);
    }

    #[test]
    fn all_primitives_roundtrip() {
        for p in [
            PrimitiveKind::Boolean,
            PrimitiveKind::Byte,
            PrimitiveKind::Int8,
            PrimitiveKind::Int16,
            PrimitiveKind::Int32,
            PrimitiveKind::Int64,
            PrimitiveKind::UInt8,
            PrimitiveKind::UInt16,
            PrimitiveKind::UInt32,
            PrimitiveKind::UInt64,
            PrimitiveKind::Float32,
            PrimitiveKind::Float64,
            PrimitiveKind::Float128,
            PrimitiveKind::Char8,
            PrimitiveKind::Char16,
        ] {
            roundtrip(TypeIdentifier::Primitive(p));
        }
    }

    #[test]
    fn primitive_int32_discriminator_is_spec_value() {
        let ti = TypeIdentifier::Primitive(PrimitiveKind::Int32);
        let bytes = ti.to_bytes_le().unwrap();
        assert_eq!(bytes, [TK_INT32]);
    }

    #[test]
    fn string8_small_roundtrips() {
        roundtrip(TypeIdentifier::String8Small { bound: 64 });
        roundtrip(TypeIdentifier::String8Small { bound: 0 }); // unbounded
    }

    #[test]
    fn string8_large_roundtrips() {
        roundtrip(TypeIdentifier::String8Large { bound: 65_536 });
    }

    #[test]
    fn string16_small_and_large_roundtrip() {
        roundtrip(TypeIdentifier::String16Small { bound: 32 });
        roundtrip(TypeIdentifier::String16Large { bound: 100_000 });
    }

    #[test]
    fn plain_sequence_of_int32_roundtrips() {
        let ti = TypeIdentifier::PlainSequenceSmall {
            header: PlainCollectionHeader {
                equiv_kind: 0,
                element_flags: CollectionElementFlag(0),
            },
            bound: 10,
            element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Int32)),
        };
        roundtrip(ti);
    }

    #[test]
    fn plain_sequence_large_of_string_roundtrips() {
        let ti = TypeIdentifier::PlainSequenceLarge {
            header: PlainCollectionHeader {
                equiv_kind: 0,
                element_flags: CollectionElementFlag(0),
            },
            bound: 1_000_000,
            element: Box::new(TypeIdentifier::String8Small { bound: 255 }),
        };
        roundtrip(ti);
    }

    #[test]
    fn plain_array_small_3d_roundtrips() {
        let ti = TypeIdentifier::PlainArraySmall {
            header: PlainCollectionHeader::default(),
            array_bounds: alloc::vec![3, 4, 5],
            element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Float64)),
        };
        roundtrip(ti);
    }

    #[test]
    fn plain_array_large_roundtrips() {
        let ti = TypeIdentifier::PlainArrayLarge {
            header: PlainCollectionHeader::default(),
            array_bounds: alloc::vec![1_000, 2_000],
            element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Byte)),
        };
        roundtrip(ti);
    }

    #[test]
    fn plain_map_small_roundtrips() {
        let ti = TypeIdentifier::PlainMapSmall {
            header: PlainCollectionHeader::default(),
            bound: 100,
            element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Int64)),
            key_flags: CollectionElementFlag(0),
            key: Box::new(TypeIdentifier::String8Small { bound: 64 }),
        };
        roundtrip(ti);
    }

    #[test]
    fn equivalence_hash_minimal_roundtrips() {
        let hash = EquivalenceHash([
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
        ]);
        roundtrip(TypeIdentifier::EquivalenceHashMinimal(hash));
        roundtrip(TypeIdentifier::EquivalenceHashComplete(hash));
    }

    #[test]
    fn equivalence_hash_wire_is_discriminator_plus_14_bytes() {
        let hash = EquivalenceHash([0xAA; 14]);
        let bytes = TypeIdentifier::EquivalenceHashMinimal(hash)
            .to_bytes_le()
            .unwrap();
        assert_eq!(bytes.len(), 15);
        assert_eq!(bytes[0], EK_MINIMAL);
        assert_eq!(&bytes[1..], &[0xAA; 14]);
    }

    #[test]
    fn nested_sequence_of_sequence_roundtrips() {
        let inner = TypeIdentifier::PlainSequenceSmall {
            header: PlainCollectionHeader::default(),
            bound: 5,
            element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Int16)),
        };
        let outer = TypeIdentifier::PlainSequenceSmall {
            header: PlainCollectionHeader::default(),
            bound: 3,
            element: Box::new(inner),
        };
        roundtrip(outer);
    }

    /// Annex B, octet for octet:
    ///
    /// ```idl
    /// @extensibility(APPENDABLE) @nested
    /// struct StronglyConnectedComponentId {
    ///     TypeObjectHashId sc_component_id;
    ///     long             scc_length;
    ///     long             scc_index;
    /// };
    /// ```
    ///
    /// The discriminator `B0`, three octets of padding, a `DHEADER` of
    /// 24 (`@extensibility(APPENDABLE)` under encoding version 2), then
    /// the `TypeObjectHashId` — its own `EK_MINIMAL` discriminator
    /// before the fourteen hash octets — a padding octet, and the two
    /// `long`s. Thirty-two octets.
    ///
    /// The form this tree wrote had neither the `DHEADER` nor the
    /// equivalence kind: `B0` and straight into the hash, twenty-four
    /// octets. A round-trip test cannot see that — both halves agree
    /// with each other — so this asserts the octets.
    #[test]
    fn a_component_identifier_is_the_thirty_two_octets_of_annex_b() {
        let scc = TypeIdentifier::StronglyConnectedComponent(StronglyConnectedComponentId {
            equivalence_kind: EK_MINIMAL,
            hash: EquivalenceHash([
                0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D,
            ]),
            scc_length: 2,
            scc_index: 1,
        });
        let bytes = scc.to_bytes_le().unwrap();
        let expected: alloc::vec::Vec<u8> = alloc::vec![
            0xB0, 0x00, 0x00, 0x00, // TI_STRONGLY_CONNECTED_COMPONENT, then padding
            0x18, 0x00, 0x00, 0x00, // DHEADER = 24
            0xF1, // TypeObjectHashId: EK_MINIMAL
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D,
            0x00, // padding to the long
            0x02, 0x00, 0x00, 0x00, // scc_length
            0x01, 0x00, 0x00, 0x00, // scc_index
        ];
        assert_eq!(bytes, expected, "{bytes:02x?}");
        assert_eq!(TypeIdentifier::from_bytes_le(&bytes).unwrap(), scc);
    }

    /// The two equivalence forms of one component are different
    /// identifiers. §7.3.4.6 tells them apart by "the contained
    /// `TypeObjectHashId` has discriminator `EK_MINIMAL`", and with no
    /// field for it they were the same fourteen octets.
    #[test]
    fn a_minimal_and_a_complete_component_identifier_differ() {
        let of = |kind| {
            TypeIdentifier::StronglyConnectedComponent(StronglyConnectedComponentId {
                equivalence_kind: kind,
                hash: EquivalenceHash([0x33; 14]),
                scc_length: 1,
                scc_index: 1,
            })
        };
        let minimal = of(EK_MINIMAL).to_bytes_le().unwrap();
        let complete = of(EK_COMPLETE).to_bytes_le().unwrap();
        assert_ne!(minimal, complete);
        assert_eq!(minimal.len(), complete.len());
    }

    /// A `TypeObjectHashId` discriminator the union has no case for is
    /// refused on both sides rather than carried.
    #[test]
    fn a_component_identifier_takes_only_the_two_equivalence_kinds() {
        let scc = TypeIdentifier::StronglyConnectedComponent(StronglyConnectedComponentId {
            equivalence_kind: kinds::EK_BOTH,
            hash: EquivalenceHash::ZERO,
            scc_length: 1,
            scc_index: 1,
        });
        assert!(scc.to_bytes_le().is_err());

        let mut bytes = alloc::vec![0xB0_u8, 0, 0, 0, 24, 0, 0, 0, kinds::EK_BOTH];
        bytes.extend_from_slice(&[0u8; 14]);
        bytes.extend_from_slice(&[0, 1, 0, 0, 0, 1, 0, 0, 0]);
        assert!(TypeIdentifier::from_bytes_le(&bytes).is_err());
    }

    #[test]
    fn strongly_connected_component_roundtrips() {
        let scc = TypeIdentifier::StronglyConnectedComponent(StronglyConnectedComponentId {
            equivalence_kind: EK_MINIMAL,
            hash: EquivalenceHash([0x11; 14]),
            scc_length: 5,
            scc_index: 2,
        });
        roundtrip(scc);
    }

    #[test]
    fn scc_encode_rejects_negative_values() {
        let scc = TypeIdentifier::StronglyConnectedComponent(StronglyConnectedComponentId {
            equivalence_kind: EK_MINIMAL,
            hash: EquivalenceHash::ZERO,
            scc_length: -1,
            scc_index: 1,
        });
        assert!(scc.to_bytes_le().is_err());
    }

    #[test]
    fn scc_encode_rejects_index_out_of_bounds() {
        let scc = TypeIdentifier::StronglyConnectedComponent(StronglyConnectedComponentId {
            equivalence_kind: EK_MINIMAL,
            hash: EquivalenceHash::ZERO,
            scc_length: 3,
            scc_index: 4, // 1..=3 is the range; 4 is past the end
        });
        assert!(scc.to_bytes_le().is_err());
    }

    #[test]
    fn max_decode_depth_constant_is_reasonable() {
        // DoS guard: recursion when wire-decoding a nested
        // `TypeIdentifier` is bounded. The cap must be > 4 (for
        // realistic nested sequences) and < 64 (DoS protection).
        const _ASSERT_MIN: usize = TypeIdentifier::MAX_DECODE_DEPTH - 4;
        const _ASSERT_MAX: usize = 64 - TypeIdentifier::MAX_DECODE_DEPTH;
    }

    #[test]
    fn deeply_nested_but_bounded_sequence_decodes_ok() {
        // 3-deep sequence<sequence<sequence<int32>>> — within the cap.
        let l1 = TypeIdentifier::PlainSequenceSmall {
            header: PlainCollectionHeader::default(),
            bound: 5,
            element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Int32)),
        };
        let l2 = TypeIdentifier::PlainSequenceSmall {
            header: PlainCollectionHeader::default(),
            bound: 5,
            element: Box::new(l1),
        };
        let l3 = TypeIdentifier::PlainSequenceSmall {
            header: PlainCollectionHeader::default(),
            bound: 5,
            element: Box::new(l2),
        };
        roundtrip(l3);
    }

    /// Annex B's `default: ExtendedTypeDefn extended_defn;`. The arm is
    /// `@extensibility(MUTABLE)`, so it is an aligned `DHEADER` over a
    /// member list — here an empty one — and the identifier goes back
    /// exactly as it came.
    ///
    /// The bare discriminator octet this used to feed is not a
    /// `TypeIdentifier`: reading one and stopping left the body in the
    /// stream, which at an embedded position moved every field after it
    /// with no error anywhere.
    #[test]
    fn the_extended_arm_carries_its_body() {
        let bytes = [0xC7, 0, 0, 0, 0, 0, 0, 0];
        let decoded = TypeIdentifier::from_bytes_le(&bytes).unwrap();
        assert_eq!(
            decoded,
            TypeIdentifier::Extended {
                discriminator: 0xC7,
                body: alloc::vec![],
            }
        );
        assert_eq!(decoded.to_bytes_le().unwrap(), bytes);
    }

    /// A member list a later revision wrote and this one does not know
    /// survives the round trip verbatim.
    #[test]
    fn an_extended_arm_with_a_member_list_round_trips_verbatim() {
        let mut bytes = alloc::vec![0xC7_u8, 0, 0, 0];
        bytes.extend_from_slice(&8_u32.to_le_bytes());
        bytes.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02, 0x03, 0x04]);
        let decoded = TypeIdentifier::from_bytes_le(&bytes).unwrap();
        assert_eq!(
            decoded,
            TypeIdentifier::Extended {
                discriminator: 0xC7,
                body: alloc::vec![0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02, 0x03, 0x04],
            }
        );
        assert_eq!(decoded.to_bytes_le().unwrap(), bytes);
    }

    /// Annex B's `union TypeIdentifier` has `case EK_COMPLETE:` and
    /// `case EK_MINIMAL:` and no case for `EK_BOTH = 0xF3`; that value
    /// is a `PlainCollectionHeader`'s equivalence kind. Read as the
    /// `default:` arm it took the fourteen hash octets that follow it in
    /// a real stream and gave back one.
    #[test]
    fn ek_both_is_not_a_type_identifier_discriminator() {
        let mut bytes = alloc::vec![kinds::EK_BOTH];
        bytes.extend_from_slice(&[0u8; 14]);
        assert!(TypeIdentifier::from_bytes_le(&bytes).is_err());
    }

    // ---- Additional edge-case roundtrips --------------------------------

    #[test]
    fn plain_sequence_large_with_u32_max_bound_roundtrips() {
        let ti = TypeIdentifier::PlainSequenceLarge {
            header: PlainCollectionHeader::default(),
            bound: u32::MAX,
            element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Byte)),
        };
        roundtrip(ti);
    }

    #[test]
    fn plain_array_large_with_many_dimensions_roundtrips() {
        let ti = TypeIdentifier::PlainArrayLarge {
            header: PlainCollectionHeader::default(),
            array_bounds: alloc::vec![
                1_000, 2_000, 3_000, 4_000, 5_000, 6_000, 7_000, 8_000, 9_000, 10_000, 11_000,
                12_000,
            ],
            element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Float32)),
        };
        roundtrip(ti);
    }

    #[test]
    fn plain_array_small_with_single_dimension_roundtrips() {
        let ti = TypeIdentifier::PlainArraySmall {
            header: PlainCollectionHeader::default(),
            array_bounds: alloc::vec![250],
            element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Int32)),
        };
        roundtrip(ti);
    }

    #[test]
    fn plain_map_large_with_nested_map_value_roundtrips() {
        let inner = TypeIdentifier::PlainMapSmall {
            header: PlainCollectionHeader::default(),
            bound: 10,
            element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Int32)),
            key_flags: CollectionElementFlag(0),
            key: Box::new(TypeIdentifier::String8Small { bound: 8 }),
        };
        let outer = TypeIdentifier::PlainMapLarge {
            header: PlainCollectionHeader::default(),
            bound: 5_000,
            element: Box::new(inner),
            key_flags: CollectionElementFlag(0),
            key: Box::new(TypeIdentifier::String8Small { bound: 16 }),
        };
        roundtrip(outer);
    }

    #[test]
    fn strongly_connected_component_large_scc_length_and_index() {
        let scc = TypeIdentifier::StronglyConnectedComponent(StronglyConnectedComponentId {
            equivalence_kind: EK_MINIMAL,
            hash: EquivalenceHash([0x5A; 14]),
            scc_length: i32::MAX,
            scc_index: i32::MAX - 1,
        });
        roundtrip(scc);
    }

    #[test]
    fn extended_arms_cover_multiple_discriminators() {
        // Deliberately discriminator bytes not assigned to any
        // TK_/TI_/EK_. 0x01 (TK_BOOLEAN), 0x70ff (TI_STRING8_*) etc. are
        // avoided. 0x12-0x1F, 0x40, 0x50, 0xC7, 0xFE, 0xFF are free.
        for d in [0x12_u8, 0x1F, 0x40, 0x50, 0xC7, 0xFE, 0xFF] {
            let bytes = [d, 0, 0, 0, 0, 0, 0, 0];
            let decoded = TypeIdentifier::from_bytes_le(&bytes).unwrap();
            assert_eq!(
                decoded,
                TypeIdentifier::Extended {
                    discriminator: d,
                    body: alloc::vec![],
                }
            );
            assert_eq!(decoded.to_bytes_le().unwrap(), bytes);
        }
    }

    #[test]
    fn encode_first_byte_is_always_discriminator() {
        let samples: alloc::vec::Vec<TypeIdentifier> = alloc::vec![
            TypeIdentifier::None,
            TypeIdentifier::Primitive(PrimitiveKind::Int32),
            TypeIdentifier::String8Small { bound: 8 },
            TypeIdentifier::String8Large { bound: 10_000 },
            TypeIdentifier::String16Small { bound: 8 },
            TypeIdentifier::String16Large { bound: 10_000 },
            TypeIdentifier::PlainSequenceSmall {
                header: PlainCollectionHeader::default(),
                bound: 1,
                element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Int32)),
            },
            TypeIdentifier::PlainSequenceLarge {
                header: PlainCollectionHeader::default(),
                bound: 300,
                element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Int32)),
            },
            TypeIdentifier::PlainArraySmall {
                header: PlainCollectionHeader::default(),
                array_bounds: alloc::vec![2, 3],
                element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Int32)),
            },
            TypeIdentifier::PlainArrayLarge {
                header: PlainCollectionHeader::default(),
                array_bounds: alloc::vec![500, 500],
                element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Int32)),
            },
            TypeIdentifier::PlainMapSmall {
                header: PlainCollectionHeader::default(),
                bound: 1,
                element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Int32)),
                key_flags: CollectionElementFlag(0),
                key: Box::new(TypeIdentifier::String8Small { bound: 1 }),
            },
            TypeIdentifier::PlainMapLarge {
                header: PlainCollectionHeader::default(),
                bound: 1_000,
                element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Int32)),
                key_flags: CollectionElementFlag(0),
                key: Box::new(TypeIdentifier::String8Small { bound: 1 }),
            },
            TypeIdentifier::StronglyConnectedComponent(StronglyConnectedComponentId {
                equivalence_kind: EK_MINIMAL,
                hash: EquivalenceHash([0; 14]),
                scc_length: 1,
                scc_index: 1,
            }),
            TypeIdentifier::EquivalenceHashMinimal(EquivalenceHash([0x11; 14])),
            TypeIdentifier::EquivalenceHashComplete(EquivalenceHash([0x22; 14])),
            TypeIdentifier::Extended {
                discriminator: 0x77,
                body: alloc::vec![],
            },
        ];
        for ti in samples {
            let bytes = ti.to_bytes_le().unwrap();
            assert!(!bytes.is_empty());
            assert_eq!(bytes[0], ti.discriminator());
        }
    }

    #[test]
    fn primitive_kind_from_u8_rejects_unknown() {
        assert!(PrimitiveKind::from_u8(0xC7).is_none());
        assert!(PrimitiveKind::from_u8(0x00).is_none() || PrimitiveKind::from_u8(0x00).is_some());
    }

    #[test]
    fn primitive_kind_roundtrip_via_u8() {
        for p in [
            PrimitiveKind::Boolean,
            PrimitiveKind::Byte,
            PrimitiveKind::Int8,
            PrimitiveKind::Int16,
            PrimitiveKind::Int32,
            PrimitiveKind::Int64,
            PrimitiveKind::UInt8,
            PrimitiveKind::UInt16,
            PrimitiveKind::UInt32,
            PrimitiveKind::UInt64,
            PrimitiveKind::Float32,
            PrimitiveKind::Float64,
            PrimitiveKind::Float128,
            PrimitiveKind::Char8,
            PrimitiveKind::Char16,
        ] {
            assert_eq!(PrimitiveKind::from_u8(p.to_u8()), Some(p));
        }
    }

    #[test]
    fn equivalence_kind_roundtrip() {
        for k in [
            EquivalenceKind::None,
            EquivalenceKind::Minimal,
            EquivalenceKind::Complete,
            EquivalenceKind::Both,
        ] {
            let encoded = k.to_u8();
            let decoded = EquivalenceKind::from_u8(encoded);
            // None round-trips to None (encoded as 0 which isn't a known
            // discriminator → `from_u8` falls back to None).
            if k == EquivalenceKind::None {
                assert_eq!(decoded, EquivalenceKind::None);
            } else {
                assert_eq!(decoded, k);
            }
        }
    }

    #[test]
    fn equivalence_kind_from_u8_unknown_is_none() {
        assert_eq!(EquivalenceKind::from_u8(0xAB), EquivalenceKind::None);
    }

    #[test]
    fn equivalence_hash_zero_constant_is_zeroes() {
        assert_eq!(EquivalenceHash::ZERO.0, [0u8; 14]);
    }

    /// The `default:` arm is a discriminator, three octets of padding
    /// and a `DHEADER` — eight octets for an empty
    /// `ExtendedTypeDefn`, not one.
    ///
    /// This test asserted the one octet, which was the defect: the body
    /// the specification puts there was neither read nor written, and at
    /// an embedded position that shifted the whole rest of the stream.
    #[test]
    fn the_extended_arm_encodes_its_dheader_too() {
        let ti = TypeIdentifier::Extended {
            discriminator: 0xC7,
            body: alloc::vec![],
        };
        let bytes = ti.to_bytes_le().unwrap();
        assert_eq!(bytes, alloc::vec![0xC7, 0, 0, 0, 0, 0, 0, 0]);
    }
}
