// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! XML → TypeObject Bridge (Cluster C4.5-b).
//!
//! Konvertiert das interne XML-Datenmodell aus [`xtypes_def`] in das
//! XTypes-1.3-TypeObject-Format aus `zerodds-types`. Wire-/Hash-Kompatibel
//! mit dem IDL-Lowering aus `zerodds-idl::semantics::to_typeobject`.
//!
//! # Spec-Quellen
//!
//! - OMG XTypes 1.3 §7.3.4.x — TypeObject + Minimal/Complete-Variants.
//! - OMG XTypes 1.3 §7.3.4.5 — MinimalStructType / EnumType / UnionType.
//! - OMG XTypes 1.3 Annex A — XML-Mapping.
//!
//! # Scope C4.5-b (diese Stufe)
//!
//! - Nur `MinimalTypeObject`. `CompleteTypeObject` nicht implementiert.
//! - Top-Level-Mapping (struct, enum, union, typedef, bitmask, bitset).
//! - Member-Typen via [`TypeRef::Primitive`] (direkt) oder
//!   [`TypeRef::Named`] (Lookup in [`TypeLibrary`] mit Hash-Vorberechnung).
//! - Modifier `arrayDimensions`/`sequenceMaxLength`/`stringMaxLength`
//!   wickeln den Member-TypeIdentifier in PlainArray-/PlainSequence-/
//!   String-Bound-Variants.
//! - Member-IDs: `@id` aus XSD wenn gesetzt, sonst sequentiell ab 1
//!   (AUTOID_SEQUENTIAL, Spec §7.3.1.2.1.1).
//! - Extensibility: `final`/`appendable`/`mutable` aus
//!   [`Extensibility`] auf `StructTypeFlag`/`UnionTypeFlag` mappen.
//!
//! # Bewusst nicht im Crate
//!
//! - `CompleteTypeObject` (kommt in Phase 6).
//! - Forward-Declarations / Cross-Library-Refs.
//! - Inheritance via `baseType` — der Bridge setzt zwar `base_type` als
//!   `EquivalenceHashMinimal`, aber Cycle-Detection bleibt Aufgabe des
//!   bestehenden [`crate::inheritance`]-Moduls.
//! - `@autoid(HASH)` — XML-Schema kennt keine entsprechende Annotation.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

use zerodds_types::builder::{Extensibility as TypeExt, TypeObjectBuilder};
use zerodds_types::hash::compute_minimal_hash;
use zerodds_types::type_object::flags::{
    CollectionElementFlag, CollectionTypeFlag, EnumLiteralFlag, EnumTypeFlag, StructTypeFlag,
    UnionDiscriminatorFlag, UnionMemberFlag, UnionTypeFlag,
};
use zerodds_types::type_object::minimal::{
    CommonCollectionElement, CommonDiscriminatorMember, CommonEnumeratedHeader,
    CommonEnumeratedLiteral, MinimalArrayType, MinimalCollectionElement,
    MinimalDiscriminatorMember, MinimalEnumeratedHeader, MinimalEnumeratedLiteral,
    MinimalEnumeratedType, MinimalSequenceType, MinimalUnionMember, MinimalUnionType,
};
use zerodds_types::{MinimalTypeObject, PrimitiveKind, TypeIdentifier, TypeObject};

use crate::xtypes_def::{
    BitField, BitValue, BitmaskType, BitsetType, EnumLiteral, EnumType, Extensibility,
    PrimitiveType as XmlPrimitive, StructMember, StructType, TypeDef, TypeLibrary, TypeRef,
    TypedefType, UnionDiscriminator, UnionType,
};

/// Fehler beim XML→TypeObject-Mapping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgeError {
    /// Ein XSD-Konstrukt ist (in C4.5-b) noch nicht abbildbar — etwa
    /// inline-Maps oder Self-Recursion.
    UnsupportedXsdConstruct(String),
    /// Eine `TypeRef::Named`-Referenz konnte nicht aufgeloest werden,
    /// weil sie nicht in der mitgelieferten [`TypeLibrary`] (oder via
    /// `bridge_with_resolver`) auftaucht.
    UnresolvedReference(String),
    /// Ein numerischer Wert (z.B. ein Union-Discriminator-Label) ist
    /// nicht parsebar.
    InvalidLiteral(String),
    /// Die EquivalenceHash-Berechnung ist fehlgeschlagen (Buffer-Overflow
    /// im internen Encoder).
    HashFailed(String),
    /// Modul-Eintraege sind kein Top-Level-Type — das XML hat ein
    /// `<module>` an einer Stelle, an der nur ein Type stehen darf.
    ModuleAtTopLevel(String),
}

impl fmt::Display for BridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedXsdConstruct(s) => {
                write!(f, "unsupported XSD construct: {s}")
            }
            Self::UnresolvedReference(s) => write!(f, "unresolved reference: {s}"),
            Self::InvalidLiteral(s) => write!(f, "invalid literal: {s}"),
            Self::HashFailed(s) => write!(f, "TypeIdentifier hashing failed: {s}"),
            Self::ModuleAtTopLevel(s) => {
                write!(f, "<module> not allowed at this position: {s}")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for BridgeError {}

// ============================================================================
// Public API
// ============================================================================

/// Konvertiert ein einzelnes XmlType-[`TypeDef`] in einen [`TypeObject`]
/// (Minimal-Variant).
///
/// Named-Member-Refs werden als Null-Hash-Placeholder
/// (`TypeIdentifier::EquivalenceHashMinimal([0; 14])`) abgelegt — der
/// Caller, der eine ganze Library auf einmal aufloesen will, sollte
/// stattdessen [`bridge_library`] benutzen.
///
/// # Errors
/// `BridgeError::UnsupportedXsdConstruct` fuer Konstrukte, die in dieser
/// Stufe nicht abgebildet werden; `BridgeError::ModuleAtTopLevel` wenn
/// ein `<module>` direkt uebergeben wurde (Module sind keine Types).
pub fn xml_type_to_typeobject(xml: &TypeDef) -> Result<TypeObject, BridgeError> {
    let mto = xml_type_to_minimal_typeobject(xml)?;
    Ok(TypeObject::Minimal(mto))
}

/// Wie [`xml_type_to_typeobject`], aber direkt als
/// [`MinimalTypeObject`] (ohne Discriminator-Wrapper).
///
/// # Errors
/// Siehe [`xml_type_to_typeobject`].
pub fn xml_type_to_minimal_typeobject(xml: &TypeDef) -> Result<MinimalTypeObject, BridgeError> {
    let resolver = NullResolver;
    bridge_typedef(xml, &resolver)
}

/// Konvertiert eine ganze [`TypeLibrary`] in eine Map
/// `Name → MinimalTypeObject`. Named-Refs zwischen Types innerhalb der
/// Library werden in einem zweistufigen Pass aufgeloest:
///
/// 1. Pre-Pass: jeder Top-Level-Type wird mit Null-Hash-Placeholdern
///    abgebildet, dann sein EquivalenceHashMinimal berechnet.
/// 2. Final-Pass: alle TypeIdentifier-Refs auf Named-Types werden mit
///    den Pre-Pass-Hashes ersetzt.
///
/// `<module>`-Eintraege werden rekursiv abgeflacht. Der Map-Key fuer
/// nested types ist der scoped Name (`Module::Inner`).
///
/// # Errors
/// `BridgeError` fuer alle Fehler aus den einzelnen Type-Mappings.
pub fn bridge_library(
    lib: &TypeLibrary,
) -> Result<BTreeMap<String, MinimalTypeObject>, BridgeError> {
    let flat = flatten(&lib.types, "");

    // Pre-Pass: alle Types mit Null-Hash bauen, dann EquivalenceHashMinimal.
    let pre_resolver = NullResolver;
    let mut pre: BTreeMap<String, MinimalTypeObject> = BTreeMap::new();
    let mut pre_hashes: BTreeMap<String, TypeIdentifier> = BTreeMap::new();
    for (scoped, td) in &flat {
        let mto = bridge_typedef(td, &pre_resolver)?;
        let h = compute_minimal_hash(&mto)
            .map_err(|e| BridgeError::HashFailed(alloc::format!("{e:?}")))?;
        pre_hashes.insert(scoped.clone(), TypeIdentifier::EquivalenceHashMinimal(h));
        pre.insert(scoped.clone(), mto);
    }

    // Final-Pass: alle Named-Refs ueber pre_hashes ersetzen.
    let resolver = MapResolver { named: &pre_hashes };
    let mut out: BTreeMap<String, MinimalTypeObject> = BTreeMap::new();
    for (scoped, td) in &flat {
        let mto = bridge_typedef(td, &resolver)?;
        out.insert(scoped.clone(), mto);
    }
    Ok(out)
}

// ============================================================================
// Resolver — Strategie fuer Named-References
// ============================================================================

/// Strategie um `TypeRef::Named(...)` auf einen [`TypeIdentifier`]
/// abzubilden. Implementierungen bestimmen, wie unaufloesbare Refs
/// behandelt werden (Null-Hash vs. Fehler).
trait NameResolver {
    fn resolve(&self, name: &str) -> TypeIdentifier;
}

/// Liefert immer `EquivalenceHashMinimal([0; 14])`. Wird im Pre-Pass
/// und bei Single-Type-Bridge benutzt, wenn keine Library bekannt ist.
struct NullResolver;

impl NameResolver for NullResolver {
    fn resolve(&self, _name: &str) -> TypeIdentifier {
        TypeIdentifier::EquivalenceHashMinimal(zerodds_types::EquivalenceHash([0; 14]))
    }
}

/// Liefert die in `named` registrierten Hashes; Fallback Null-Hash.
struct MapResolver<'a> {
    named: &'a BTreeMap<String, TypeIdentifier>,
}

impl NameResolver for MapResolver<'_> {
    fn resolve(&self, name: &str) -> TypeIdentifier {
        self.named
            .get(name)
            .cloned()
            .unwrap_or(TypeIdentifier::EquivalenceHashMinimal(
                zerodds_types::EquivalenceHash([0; 14]),
            ))
    }
}

// ============================================================================
// Flattening
// ============================================================================

/// zerodds-lint: recursion-depth 32
///
/// XML-Module-Hierarchien sind selten tiefer als ~8 Ebenen
/// (`org::omg::dds::core::policy`-Stil). Cap auf 32 deckt auch
/// pathologisch verschachtelte Test-Fixtures.
fn flatten<'a>(types: &'a [TypeDef], prefix: &str) -> Vec<(String, &'a TypeDef)> {
    let mut out: Vec<(String, &TypeDef)> = Vec::new();
    for t in types {
        match t {
            TypeDef::Module(m) => {
                let new_prefix = if prefix.is_empty() {
                    m.name.clone()
                } else {
                    alloc::format!("{prefix}::{}", m.name)
                };
                out.extend(flatten(&m.types, &new_prefix));
            }
            other => {
                let scoped = if prefix.is_empty() {
                    other.name().to_string()
                } else {
                    alloc::format!("{prefix}::{}", other.name())
                };
                out.push((scoped, other));
            }
        }
    }
    out
}

// ============================================================================
// Top-Level Dispatcher
// ============================================================================

fn bridge_typedef<R: NameResolver>(
    xml: &TypeDef,
    res: &R,
) -> Result<MinimalTypeObject, BridgeError> {
    match xml {
        TypeDef::Struct(s) => bridge_struct(s, res),
        TypeDef::Enum(e) => Ok(MinimalTypeObject::Enumerated(bridge_enum(e))),
        TypeDef::Union(u) => bridge_union(u, res),
        TypeDef::Typedef(t) => bridge_typedef_alias(t, res),
        TypeDef::Bitmask(b) => Ok(MinimalTypeObject::Bitmask(bridge_bitmask(b))),
        TypeDef::Bitset(b) => bridge_bitset(b),
        TypeDef::Module(m) => Err(BridgeError::ModuleAtTopLevel(m.name.clone())),
        TypeDef::Include(_) | TypeDef::ForwardDcl(_) | TypeDef::Const(_) => {
            Err(BridgeError::UnsupportedXsdConstruct(alloc::format!(
                "non-bridgeable XML element kind: {}",
                xml.name()
            )))
        }
    }
}

// ============================================================================
// Struct
// ============================================================================

fn bridge_struct<R: NameResolver>(
    s: &StructType,
    res: &R,
) -> Result<MinimalTypeObject, BridgeError> {
    let ext = map_extensibility(s.extensibility.unwrap_or_default());
    let mut builder = TypeObjectBuilder::struct_type(s.name.clone()).extensibility(ext);

    if let Some(base) = &s.base_type {
        builder = builder.base(res.resolve(base));
    }

    for m in &s.members {
        let ti = wrap_member_type(m, res)?;
        let key = m.key;
        let optional = m.optional;
        let must_understand = m.must_understand;
        let explicit_id = m.id;
        let name = m.name.clone();
        builder = builder.member(name, ti, |mut mb| {
            if key {
                mb = mb.key();
            }
            if optional {
                mb = mb.optional();
            }
            if must_understand {
                mb = mb.must_understand();
            }
            if let Some(id) = explicit_id {
                mb = mb.id(id);
            }
            mb
        });
    }
    Ok(MinimalTypeObject::Struct(builder.build_minimal()))
}

fn map_extensibility(e: Extensibility) -> TypeExt {
    match e {
        Extensibility::Final => TypeExt::Final,
        Extensibility::Appendable => TypeExt::Appendable,
        Extensibility::Mutable => TypeExt::Mutable,
    }
}

// ============================================================================
// Enum
// ============================================================================

fn bridge_enum(e: &EnumType) -> MinimalEnumeratedType {
    let bit_bound: u16 = e.bit_bound.unwrap_or(32).min(64) as u16;
    // Auto-Numbering: Spec §7.3.1.2.4.1 — wenn `value` fehlt, wird
    // `prev + 1` verwendet, beginnend bei 0.
    let mut prev: i32 = -1;
    let literal_seq: Vec<MinimalEnumeratedLiteral> = e
        .enumerators
        .iter()
        .map(|l: &EnumLiteral| {
            let value = l.value.unwrap_or(prev.saturating_add(1));
            prev = value;
            MinimalEnumeratedLiteral {
                common: CommonEnumeratedLiteral {
                    value,
                    flags: EnumLiteralFlag::default(),
                },
                detail: zerodds_types::type_object::common::NameHash::from_name(&l.name),
            }
        })
        .collect();
    MinimalEnumeratedType {
        enum_flags: EnumTypeFlag::default(),
        header: MinimalEnumeratedHeader {
            common: CommonEnumeratedHeader { bit_bound },
        },
        literal_seq,
    }
}

// ============================================================================
// Union
// ============================================================================

fn bridge_union<R: NameResolver>(u: &UnionType, res: &R) -> Result<MinimalTypeObject, BridgeError> {
    let disc_ti = type_ref_to_identifier(&u.discriminator, res);
    let mut next_seq: u32 = 1;
    let mut member_seq: Vec<MinimalUnionMember> = Vec::with_capacity(u.cases.len());
    for case in &u.cases {
        let m = &case.member;
        let mut labels: Vec<i32> = Vec::with_capacity(case.discriminators.len());
        let mut is_default = false;
        for d in &case.discriminators {
            match d {
                UnionDiscriminator::Default => {
                    is_default = true;
                }
                UnionDiscriminator::Value(s) => {
                    let v = parse_label(s)?;
                    labels.push(v);
                }
            }
        }
        let id = m.id.unwrap_or_else(|| {
            let v = next_seq;
            next_seq += 1;
            v
        });
        let ti = wrap_member_type(m, res)?;
        let mut flags: u16 = 0;
        if is_default {
            flags |= UnionMemberFlag::IS_DEFAULT;
        }
        member_seq.push(MinimalUnionMember {
            common: zerodds_types::type_object::common::CommonUnionMember {
                member_id: id,
                member_flags: UnionMemberFlag(flags),
                type_id: ti,
                label_seq: labels,
            },
            detail: zerodds_types::type_object::common::NameHash::from_name(&m.name),
        });
    }
    // XML-Union hat aktuell keinen Extensibility-Marker — Default
    // Appendable (Spec §7.2.2.4) → IS_APPENDABLE. Reuse die Struct-
    // Flag-Bitpositionen, die der Builder fuer Union-Flags spiegelt.
    let _ = &u.name;
    let union_flags_bits = StructTypeFlag::IS_APPENDABLE;
    Ok(MinimalTypeObject::Union(MinimalUnionType {
        union_flags: UnionTypeFlag(union_flags_bits),
        discriminator: MinimalDiscriminatorMember {
            common: CommonDiscriminatorMember {
                member_flags: UnionDiscriminatorFlag::default(),
                type_id: disc_ti,
            },
        },
        member_seq,
    }))
}

fn parse_label(s: &str) -> Result<i32, BridgeError> {
    let trimmed = s.trim();
    if let Some(rest) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        return i64::from_str_radix(rest, 16)
            .ok()
            .and_then(|v| i32::try_from(v).ok())
            .ok_or_else(|| BridgeError::InvalidLiteral(s.to_string()));
    }
    trimmed
        .parse::<i32>()
        .map_err(|_| BridgeError::InvalidLiteral(s.to_string()))
}

// ============================================================================
// Typedef → Alias / PlainSequence / PlainArray
// ============================================================================

fn bridge_typedef_alias<R: NameResolver>(
    t: &TypedefType,
    res: &R,
) -> Result<MinimalTypeObject, BridgeError> {
    // Wenn arrayDimensions gesetzt ist, ist das Typedef ein
    // MinimalArrayType. Wenn sequenceMaxLength gesetzt ist, ist es ein
    // MinimalSequenceType. Sonst ein MinimalAliasType.
    if !t.array_dimensions.is_empty() {
        let element =
            type_ref_to_identifier_with_string_bound(&t.type_ref, t.string_max_length, res);
        return Ok(MinimalTypeObject::Array(MinimalArrayType {
            collection_flag: CollectionTypeFlag::default(),
            bound_seq: t.array_dimensions.clone(),
            element: MinimalCollectionElement {
                common: CommonCollectionElement {
                    element_flags: CollectionElementFlag::default(),
                    type_id: element,
                },
            },
        }));
    }
    if let Some(bound) = t.sequence_max_length {
        let element =
            type_ref_to_identifier_with_string_bound(&t.type_ref, t.string_max_length, res);
        return Ok(MinimalTypeObject::Sequence(MinimalSequenceType {
            collection_flag: CollectionTypeFlag::default(),
            bound,
            element: MinimalCollectionElement {
                common: CommonCollectionElement {
                    element_flags: CollectionElementFlag::default(),
                    type_id: element,
                },
            },
        }));
    }
    let related = type_ref_to_identifier_with_string_bound(&t.type_ref, t.string_max_length, res);
    let alias = TypeObjectBuilder::alias(t.name.clone(), related).build_minimal();
    Ok(MinimalTypeObject::Alias(alias))
}

// ============================================================================
// Bitmask
// ============================================================================

fn bridge_bitmask(b: &BitmaskType) -> zerodds_types::type_object::minimal::MinimalBitmaskType {
    let bit_bound: u16 = b.bit_bound.unwrap_or(32).min(64) as u16;
    let mut prev: i32 = -1;
    let mut builder = TypeObjectBuilder::bitmask(b.name.clone()).bit_bound(bit_bound);
    for v in &b.bit_values {
        let pos = match v.position {
            Some(p) => {
                prev = p as i32;
                p
            }
            None => {
                let p = (prev.saturating_add(1)).max(0) as u32;
                prev = p as i32;
                p
            }
        };
        let p_u16 = pos as u16;
        let _ = v as &BitValue;
        builder = builder.flag(v.name.clone(), p_u16);
    }
    builder.build_minimal()
}

// ============================================================================
// Bitset
// ============================================================================

fn bridge_bitset(b: &BitsetType) -> Result<MinimalTypeObject, BridgeError> {
    use zerodds_types::type_identifier::kinds::{
        TK_INT8, TK_INT16, TK_INT32, TK_INT64, TK_UINT8, TK_UINT16, TK_UINT32, TK_UINT64,
    };
    // Bit-Position laeuft kumulativ: jedes Feld setzt prev + bitcount.
    let mut next_pos: u16 = 0;
    let mut builder = TypeObjectBuilder::bitset(b.name.clone());
    for f in &b.bit_fields {
        let bitcount = parse_bitset_mask_bits(&f.mask)?;
        let holder = match &f.type_ref {
            TypeRef::Primitive(p) => holder_kind_byte(*p).ok_or_else(|| {
                BridgeError::UnsupportedXsdConstruct(alloc::format!(
                    "bitset holder type: {}",
                    p.as_xml()
                ))
            })?,
            TypeRef::Named(_) => {
                return Err(BridgeError::UnsupportedXsdConstruct(
                    "bitset holder must be a primitive integer".to_string(),
                ));
            }
        };
        // Defensive: dass die TK_-Konstanten existieren.
        debug_assert!(matches!(
            holder,
            TK_INT8 | TK_INT16 | TK_INT32 | TK_INT64 | TK_UINT8 | TK_UINT16 | TK_UINT32 | TK_UINT64
        ));
        let pos = next_pos;
        next_pos = next_pos.saturating_add(u16::from(bitcount));
        builder = builder.field(f.name.clone(), pos, bitcount, holder);
        let _ = f as &BitField;
    }
    Ok(MinimalTypeObject::Bitset(builder.build_minimal()))
}

fn holder_kind_byte(p: XmlPrimitive) -> Option<u8> {
    use zerodds_types::type_identifier::kinds::*;
    Some(match p {
        XmlPrimitive::Octet => TK_BYTE,
        XmlPrimitive::Short => TK_INT16,
        XmlPrimitive::UShort => TK_UINT16,
        XmlPrimitive::Long => TK_INT32,
        XmlPrimitive::ULong => TK_UINT32,
        XmlPrimitive::LongLong => TK_INT64,
        XmlPrimitive::ULongLong => TK_UINT64,
        XmlPrimitive::Boolean => TK_BOOLEAN,
        XmlPrimitive::Char => TK_CHAR8,
        XmlPrimitive::WChar => TK_CHAR16,
        _ => return None,
    })
}

/// Parst eine Bitmask wie `0x0F` zu der Anzahl gesetzter Bits.
fn parse_bitset_mask_bits(mask: &str) -> Result<u8, BridgeError> {
    let trimmed = mask.trim();
    let (rest, radix) = if let Some(r) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        (r, 16u32)
    } else if let Some(r) = trimmed
        .strip_prefix("0b")
        .or_else(|| trimmed.strip_prefix("0B"))
    {
        (r, 2u32)
    } else {
        (trimmed, 10u32)
    };
    let v = u64::from_str_radix(rest, radix)
        .map_err(|_| BridgeError::InvalidLiteral(alloc::format!("bitset mask {mask}")))?;
    let bits = u8::try_from(v.count_ones())
        .map_err(|_| BridgeError::InvalidLiteral(alloc::format!("bitset mask too wide: {mask}")))?;
    if bits == 0 {
        return Err(BridgeError::InvalidLiteral(alloc::format!(
            "bitset mask must have >= 1 bit: {mask}"
        )));
    }
    Ok(bits)
}

// ============================================================================
// TypeRef → TypeIdentifier
// ============================================================================

/// Wickelt einen Struct-Member in seinen finalen TypeIdentifier — inkl.
/// `arrayDimensions`, `sequenceMaxLength` und `stringMaxLength`.
fn wrap_member_type<R: NameResolver>(
    m: &StructMember,
    res: &R,
) -> Result<TypeIdentifier, BridgeError> {
    let inner = type_ref_to_identifier_with_string_bound(&m.type_ref, m.string_max_length, res);

    // arrayDimensions hat Vorrang vor sequenceMaxLength.
    if !m.array_dimensions.is_empty() {
        return Ok(wrap_array(inner, &m.array_dimensions));
    }
    if let Some(bound) = m.sequence_max_length {
        return Ok(wrap_sequence(inner, bound));
    }
    Ok(inner)
}

fn wrap_sequence(element: TypeIdentifier, bound: u32) -> TypeIdentifier {
    if bound <= 255 {
        TypeIdentifier::PlainSequenceSmall {
            header: zerodds_types::PlainCollectionHeader::default(),
            bound: bound as u8,
            element: alloc::boxed::Box::new(element),
        }
    } else {
        TypeIdentifier::PlainSequenceLarge {
            header: zerodds_types::PlainCollectionHeader::default(),
            bound,
            element: alloc::boxed::Box::new(element),
        }
    }
}

fn wrap_array(element: TypeIdentifier, dims: &[u32]) -> TypeIdentifier {
    let any_large = dims.iter().any(|d| *d > 255);
    if any_large {
        TypeIdentifier::PlainArrayLarge {
            header: zerodds_types::PlainCollectionHeader::default(),
            array_bounds: dims.to_vec(),
            element: alloc::boxed::Box::new(element),
        }
    } else {
        TypeIdentifier::PlainArraySmall {
            header: zerodds_types::PlainCollectionHeader::default(),
            array_bounds: dims.iter().map(|d| *d as u8).collect(),
            element: alloc::boxed::Box::new(element),
        }
    }
}

fn type_ref_to_identifier<R: NameResolver>(t: &TypeRef, res: &R) -> TypeIdentifier {
    type_ref_to_identifier_with_string_bound(t, None, res)
}

fn type_ref_to_identifier_with_string_bound<R: NameResolver>(
    t: &TypeRef,
    string_max_length: Option<u32>,
    res: &R,
) -> TypeIdentifier {
    match t {
        TypeRef::Primitive(p) => primitive_to_identifier(*p, string_max_length),
        TypeRef::Named(n) => res.resolve(n),
    }
}

fn primitive_to_identifier(p: XmlPrimitive, bound: Option<u32>) -> TypeIdentifier {
    match p {
        XmlPrimitive::Boolean => TypeIdentifier::Primitive(PrimitiveKind::Boolean),
        XmlPrimitive::Octet => TypeIdentifier::Primitive(PrimitiveKind::Byte),
        XmlPrimitive::Char => TypeIdentifier::Primitive(PrimitiveKind::Char8),
        XmlPrimitive::WChar => TypeIdentifier::Primitive(PrimitiveKind::Char16),
        XmlPrimitive::Short => TypeIdentifier::Primitive(PrimitiveKind::Int16),
        XmlPrimitive::UShort => TypeIdentifier::Primitive(PrimitiveKind::UInt16),
        XmlPrimitive::Long => TypeIdentifier::Primitive(PrimitiveKind::Int32),
        XmlPrimitive::ULong => TypeIdentifier::Primitive(PrimitiveKind::UInt32),
        XmlPrimitive::LongLong => TypeIdentifier::Primitive(PrimitiveKind::Int64),
        XmlPrimitive::ULongLong => TypeIdentifier::Primitive(PrimitiveKind::UInt64),
        XmlPrimitive::Float => TypeIdentifier::Primitive(PrimitiveKind::Float32),
        XmlPrimitive::Double => TypeIdentifier::Primitive(PrimitiveKind::Float64),
        XmlPrimitive::LongDouble => TypeIdentifier::Primitive(PrimitiveKind::Float128),
        XmlPrimitive::String => string_id(false, bound.unwrap_or(0)),
        XmlPrimitive::WString => string_id(true, bound.unwrap_or(0)),
    }
}

fn string_id(wide: bool, bound: u32) -> TypeIdentifier {
    if wide {
        if bound <= 255 {
            TypeIdentifier::String16Small { bound: bound as u8 }
        } else {
            TypeIdentifier::String16Large { bound }
        }
    } else if bound <= 255 {
        TypeIdentifier::String8Small { bound: bound as u8 }
    } else {
        TypeIdentifier::String8Large { bound }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::xtypes_def::{
        BitField, BitValue, BitmaskType, BitsetType, EnumLiteral, EnumType, ModuleEntry,
        StructMember, StructType, TypedefType, UnionCase, UnionType,
    };

    fn primitive(p: XmlPrimitive) -> TypeRef {
        TypeRef::Primitive(p)
    }

    fn make_struct() -> StructType {
        StructType {
            name: "Position".into(),
            extensibility: Some(Extensibility::Final),
            base_type: None,
            members: alloc::vec![
                StructMember {
                    name: "x".into(),
                    type_ref: primitive(XmlPrimitive::Float),
                    key: true,
                    ..Default::default()
                },
                StructMember {
                    name: "y".into(),
                    type_ref: primitive(XmlPrimitive::Float),
                    ..Default::default()
                },
            ],
        }
    }

    // ---- Struct ----------------------------------------------------------

    #[test]
    fn struct_two_members_lower_to_minimal_struct() {
        let s = make_struct();
        let mto = xml_type_to_minimal_typeobject(&TypeDef::Struct(s)).unwrap();
        match mto {
            MinimalTypeObject::Struct(st) => {
                assert_eq!(st.member_seq.len(), 2);
                assert!(st.struct_flags.has(StructTypeFlag::IS_FINAL));
                assert!(matches!(
                    st.member_seq[0].common.member_type_id,
                    TypeIdentifier::Primitive(PrimitiveKind::Float32)
                ));
            }
            other => panic!("expected struct, got {other:?}"),
        }
    }

    #[test]
    fn struct_explicit_id_is_preserved() {
        let s = StructType {
            name: "X".into(),
            extensibility: None,
            base_type: None,
            members: alloc::vec![StructMember {
                name: "a".into(),
                type_ref: primitive(XmlPrimitive::Long),
                id: Some(42),
                ..Default::default()
            }],
        };
        let mto = xml_type_to_minimal_typeobject(&TypeDef::Struct(s)).unwrap();
        if let MinimalTypeObject::Struct(st) = mto {
            assert_eq!(st.member_seq[0].common.member_id, 42);
        } else {
            panic!("not a struct");
        }
    }

    #[test]
    fn struct_autoid_sequential_starts_at_1() {
        let s = StructType {
            name: "X".into(),
            extensibility: None,
            base_type: None,
            members: alloc::vec![
                StructMember {
                    name: "a".into(),
                    type_ref: primitive(XmlPrimitive::Long),
                    ..Default::default()
                },
                StructMember {
                    name: "b".into(),
                    type_ref: primitive(XmlPrimitive::Long),
                    ..Default::default()
                },
            ],
        };
        let mto = xml_type_to_minimal_typeobject(&TypeDef::Struct(s)).unwrap();
        if let MinimalTypeObject::Struct(st) = mto {
            assert_eq!(st.member_seq[0].common.member_id, 1);
            assert_eq!(st.member_seq[1].common.member_id, 2);
        } else {
            panic!("not a struct");
        }
    }

    #[test]
    fn struct_member_flags_key_optional_must_understand() {
        use zerodds_types::type_object::flags::StructMemberFlag;
        let s = StructType {
            name: "X".into(),
            extensibility: None,
            base_type: None,
            members: alloc::vec![StructMember {
                name: "a".into(),
                type_ref: primitive(XmlPrimitive::Long),
                key: true,
                optional: true,
                must_understand: true,
                ..Default::default()
            }],
        };
        let mto = xml_type_to_minimal_typeobject(&TypeDef::Struct(s)).unwrap();
        if let MinimalTypeObject::Struct(st) = mto {
            let f = st.member_seq[0].common.member_flags;
            assert!(f.has(StructMemberFlag::IS_KEY));
            assert!(f.has(StructMemberFlag::IS_OPTIONAL));
            assert!(f.has(StructMemberFlag::IS_MUST_UNDERSTAND));
        } else {
            panic!("not a struct");
        }
    }

    #[test]
    fn struct_string_member_with_bound_uses_string_small() {
        let s = StructType {
            name: "X".into(),
            extensibility: None,
            base_type: None,
            members: alloc::vec![StructMember {
                name: "name".into(),
                type_ref: primitive(XmlPrimitive::String),
                string_max_length: Some(64),
                ..Default::default()
            }],
        };
        let mto = xml_type_to_minimal_typeobject(&TypeDef::Struct(s)).unwrap();
        if let MinimalTypeObject::Struct(st) = mto {
            assert_eq!(
                st.member_seq[0].common.member_type_id,
                TypeIdentifier::String8Small { bound: 64 }
            );
        } else {
            panic!();
        }
    }

    #[test]
    fn struct_member_with_array_dimensions_wraps_into_plain_array() {
        let s = StructType {
            name: "X".into(),
            extensibility: None,
            base_type: None,
            members: alloc::vec![StructMember {
                name: "a".into(),
                type_ref: primitive(XmlPrimitive::Long),
                array_dimensions: alloc::vec![3, 4],
                ..Default::default()
            }],
        };
        let mto = xml_type_to_minimal_typeobject(&TypeDef::Struct(s)).unwrap();
        if let MinimalTypeObject::Struct(st) = mto {
            assert!(matches!(
                st.member_seq[0].common.member_type_id,
                TypeIdentifier::PlainArraySmall { .. }
            ));
        } else {
            panic!();
        }
    }

    #[test]
    fn struct_member_with_sequence_bound_wraps_into_plain_sequence() {
        let s = StructType {
            name: "X".into(),
            extensibility: None,
            base_type: None,
            members: alloc::vec![StructMember {
                name: "a".into(),
                type_ref: primitive(XmlPrimitive::Long),
                sequence_max_length: Some(100),
                ..Default::default()
            }],
        };
        let mto = xml_type_to_minimal_typeobject(&TypeDef::Struct(s)).unwrap();
        if let MinimalTypeObject::Struct(st) = mto {
            assert!(matches!(
                st.member_seq[0].common.member_type_id,
                TypeIdentifier::PlainSequenceSmall { bound: 100, .. }
            ));
        } else {
            panic!();
        }
    }

    #[test]
    fn struct_member_with_large_array_uses_plain_array_large() {
        let s = StructType {
            name: "X".into(),
            extensibility: None,
            base_type: None,
            members: alloc::vec![StructMember {
                name: "a".into(),
                type_ref: primitive(XmlPrimitive::Long),
                array_dimensions: alloc::vec![300, 4],
                ..Default::default()
            }],
        };
        let mto = xml_type_to_minimal_typeobject(&TypeDef::Struct(s)).unwrap();
        if let MinimalTypeObject::Struct(st) = mto {
            assert!(matches!(
                st.member_seq[0].common.member_type_id,
                TypeIdentifier::PlainArrayLarge { .. }
            ));
        } else {
            panic!();
        }
    }

    #[test]
    fn struct_extensibility_mutable_sets_flag() {
        let mut s = make_struct();
        s.extensibility = Some(Extensibility::Mutable);
        let mto = xml_type_to_minimal_typeobject(&TypeDef::Struct(s)).unwrap();
        if let MinimalTypeObject::Struct(st) = mto {
            assert!(st.struct_flags.has(StructTypeFlag::IS_MUTABLE));
        } else {
            panic!();
        }
    }

    // ---- Enum ------------------------------------------------------------

    #[test]
    fn enum_with_explicit_values() {
        let e = EnumType {
            name: "Color".into(),
            bit_bound: Some(16),
            enumerators: alloc::vec![
                EnumLiteral {
                    name: "RED".into(),
                    value: Some(0),
                },
                EnumLiteral {
                    name: "GREEN".into(),
                    value: Some(1),
                },
                EnumLiteral {
                    name: "BLUE".into(),
                    value: Some(2),
                },
            ],
        };
        let mto = xml_type_to_minimal_typeobject(&TypeDef::Enum(e)).unwrap();
        if let MinimalTypeObject::Enumerated(en) = mto {
            assert_eq!(en.header.common.bit_bound, 16);
            assert_eq!(en.literal_seq.len(), 3);
            assert_eq!(en.literal_seq[2].common.value, 2);
        } else {
            panic!();
        }
    }

    #[test]
    fn enum_auto_numbering_starts_at_zero() {
        let e = EnumType {
            name: "X".into(),
            bit_bound: None,
            enumerators: alloc::vec![
                EnumLiteral {
                    name: "A".into(),
                    value: None,
                },
                EnumLiteral {
                    name: "B".into(),
                    value: None,
                },
                EnumLiteral {
                    name: "C".into(),
                    value: Some(10),
                },
                EnumLiteral {
                    name: "D".into(),
                    value: None,
                },
            ],
        };
        let mto = xml_type_to_minimal_typeobject(&TypeDef::Enum(e)).unwrap();
        if let MinimalTypeObject::Enumerated(en) = mto {
            assert_eq!(en.literal_seq[0].common.value, 0);
            assert_eq!(en.literal_seq[1].common.value, 1);
            assert_eq!(en.literal_seq[2].common.value, 10);
            assert_eq!(en.literal_seq[3].common.value, 11);
            // Default bit_bound = 32.
            assert_eq!(en.header.common.bit_bound, 32);
        } else {
            panic!();
        }
    }

    // ---- Union -----------------------------------------------------------

    #[test]
    fn union_with_int_disc_and_default_branch() {
        let u = UnionType {
            name: "U".into(),
            discriminator: primitive(XmlPrimitive::Long),
            cases: alloc::vec![
                UnionCase {
                    discriminators: alloc::vec![
                        UnionDiscriminator::Value("1".into()),
                        UnionDiscriminator::Value("2".into()),
                    ],
                    member: StructMember {
                        name: "a".into(),
                        type_ref: primitive(XmlPrimitive::LongLong),
                        ..Default::default()
                    },
                },
                UnionCase {
                    discriminators: alloc::vec![UnionDiscriminator::Default],
                    member: StructMember {
                        name: "b".into(),
                        type_ref: primitive(XmlPrimitive::String),
                        string_max_length: Some(64),
                        ..Default::default()
                    },
                },
            ],
        };
        let mto = xml_type_to_minimal_typeobject(&TypeDef::Union(u)).unwrap();
        if let MinimalTypeObject::Union(un) = mto {
            assert_eq!(un.member_seq.len(), 2);
            assert_eq!(un.member_seq[0].common.label_seq, alloc::vec![1_i32, 2]);
            assert!(un.member_seq[1].common.member_flags.0 & UnionMemberFlag::IS_DEFAULT != 0);
        } else {
            panic!();
        }
    }

    #[test]
    fn union_label_hex_parsed() {
        let u = UnionType {
            name: "U".into(),
            discriminator: primitive(XmlPrimitive::Long),
            cases: alloc::vec![UnionCase {
                discriminators: alloc::vec![UnionDiscriminator::Value("0x10".into())],
                member: StructMember {
                    name: "a".into(),
                    type_ref: primitive(XmlPrimitive::Long),
                    ..Default::default()
                },
            }],
        };
        let mto = xml_type_to_minimal_typeobject(&TypeDef::Union(u)).unwrap();
        if let MinimalTypeObject::Union(un) = mto {
            assert_eq!(un.member_seq[0].common.label_seq, alloc::vec![16_i32]);
        } else {
            panic!();
        }
    }

    #[test]
    fn union_label_invalid_returns_invalid_literal() {
        let u = UnionType {
            name: "U".into(),
            discriminator: primitive(XmlPrimitive::Long),
            cases: alloc::vec![UnionCase {
                discriminators: alloc::vec![UnionDiscriminator::Value("not-an-int".into())],
                member: StructMember {
                    name: "a".into(),
                    type_ref: primitive(XmlPrimitive::Long),
                    ..Default::default()
                },
            }],
        };
        let err = xml_type_to_minimal_typeobject(&TypeDef::Union(u)).unwrap_err();
        assert!(matches!(err, BridgeError::InvalidLiteral(_)));
    }

    // ---- Typedef ---------------------------------------------------------

    #[test]
    fn typedef_to_alias() {
        let t = TypedefType {
            name: "Count".into(),
            type_ref: primitive(XmlPrimitive::ULongLong),
            ..Default::default()
        };
        let mto = xml_type_to_minimal_typeobject(&TypeDef::Typedef(t)).unwrap();
        if let MinimalTypeObject::Alias(a) = mto {
            assert_eq!(
                a.body.common.related_type,
                TypeIdentifier::Primitive(PrimitiveKind::UInt64)
            );
        } else {
            panic!("not an alias");
        }
    }

    #[test]
    fn typedef_with_array_dims_lowers_to_array_type() {
        let t = TypedefType {
            name: "Vec3".into(),
            type_ref: primitive(XmlPrimitive::Float),
            array_dimensions: alloc::vec![3],
            ..Default::default()
        };
        let mto = xml_type_to_minimal_typeobject(&TypeDef::Typedef(t)).unwrap();
        match mto {
            MinimalTypeObject::Array(a) => {
                assert_eq!(a.bound_seq, alloc::vec![3]);
                assert_eq!(
                    a.element.common.type_id,
                    TypeIdentifier::Primitive(PrimitiveKind::Float32)
                );
            }
            other => panic!("expected array, got {other:?}"),
        }
    }

    #[test]
    fn typedef_with_sequence_bound_lowers_to_sequence_type() {
        let t = TypedefType {
            name: "Bytes".into(),
            type_ref: primitive(XmlPrimitive::Octet),
            sequence_max_length: Some(1024),
            ..Default::default()
        };
        let mto = xml_type_to_minimal_typeobject(&TypeDef::Typedef(t)).unwrap();
        match mto {
            MinimalTypeObject::Sequence(s) => {
                assert_eq!(s.bound, 1024);
            }
            _ => panic!(),
        }
    }

    // ---- Bitmask ---------------------------------------------------------

    #[test]
    fn bitmask_three_flags() {
        let b = BitmaskType {
            name: "Perms".into(),
            bit_bound: Some(8),
            bit_values: alloc::vec![
                BitValue {
                    name: "READ".into(),
                    position: Some(0),
                },
                BitValue {
                    name: "WRITE".into(),
                    position: Some(1),
                },
                BitValue {
                    name: "EXEC".into(),
                    position: Some(2),
                },
            ],
        };
        let mto = xml_type_to_minimal_typeobject(&TypeDef::Bitmask(b)).unwrap();
        if let MinimalTypeObject::Bitmask(bm) = mto {
            assert_eq!(bm.bit_bound, 8);
            assert_eq!(bm.flag_seq.len(), 3);
            assert_eq!(bm.flag_seq[0].common.position, 0);
            assert_eq!(bm.flag_seq[2].common.position, 2);
        } else {
            panic!();
        }
    }

    #[test]
    fn bitmask_auto_position() {
        let b = BitmaskType {
            name: "X".into(),
            bit_bound: None,
            bit_values: alloc::vec![
                BitValue {
                    name: "A".into(),
                    position: None,
                },
                BitValue {
                    name: "B".into(),
                    position: None,
                },
                BitValue {
                    name: "C".into(),
                    position: Some(10),
                },
                BitValue {
                    name: "D".into(),
                    position: None,
                },
            ],
        };
        let mto = xml_type_to_minimal_typeobject(&TypeDef::Bitmask(b)).unwrap();
        if let MinimalTypeObject::Bitmask(bm) = mto {
            assert_eq!(bm.flag_seq[0].common.position, 0);
            assert_eq!(bm.flag_seq[1].common.position, 1);
            assert_eq!(bm.flag_seq[2].common.position, 10);
            assert_eq!(bm.flag_seq[3].common.position, 11);
            assert_eq!(bm.bit_bound, 32);
        } else {
            panic!();
        }
    }

    // ---- Bitset ----------------------------------------------------------

    #[test]
    fn bitset_with_two_fields() {
        let b = BitsetType {
            name: "Packed".into(),
            bit_fields: alloc::vec![
                BitField {
                    name: "header".into(),
                    type_ref: primitive(XmlPrimitive::ULong),
                    mask: "0x0F".into(),
                },
                BitField {
                    name: "body".into(),
                    type_ref: primitive(XmlPrimitive::ULong),
                    mask: "0xFFFFFFF0".into(),
                },
            ],
        };
        let mto = xml_type_to_minimal_typeobject(&TypeDef::Bitset(b)).unwrap();
        if let MinimalTypeObject::Bitset(bs) = mto {
            assert_eq!(bs.field_seq.len(), 2);
            assert_eq!(bs.field_seq[0].common.bitcount, 4);
            assert_eq!(bs.field_seq[0].common.position, 0);
            assert_eq!(bs.field_seq[1].common.bitcount, 28);
            assert_eq!(bs.field_seq[1].common.position, 4);
        } else {
            panic!();
        }
    }

    #[test]
    fn bitset_invalid_mask_rejected() {
        let b = BitsetType {
            name: "X".into(),
            bit_fields: alloc::vec![BitField {
                name: "f".into(),
                type_ref: primitive(XmlPrimitive::ULong),
                mask: "garbage".into(),
            }],
        };
        let err = xml_type_to_minimal_typeobject(&TypeDef::Bitset(b)).unwrap_err();
        assert!(matches!(err, BridgeError::InvalidLiteral(_)));
    }

    #[test]
    fn bitset_zero_mask_rejected() {
        let b = BitsetType {
            name: "X".into(),
            bit_fields: alloc::vec![BitField {
                name: "f".into(),
                type_ref: primitive(XmlPrimitive::ULong),
                mask: "0x00".into(),
            }],
        };
        let err = xml_type_to_minimal_typeobject(&TypeDef::Bitset(b)).unwrap_err();
        assert!(matches!(err, BridgeError::InvalidLiteral(_)));
    }

    // ---- Module / Library -----------------------------------------------

    #[test]
    fn module_at_top_level_is_error() {
        let m = TypeDef::Module(ModuleEntry {
            name: "M".into(),
            types: alloc::vec![],
        });
        let err = xml_type_to_minimal_typeobject(&m).unwrap_err();
        assert!(matches!(err, BridgeError::ModuleAtTopLevel(_)));
    }

    #[test]
    fn library_resolves_named_refs_between_types() {
        // struct Point { float x; float y; };
        // struct Line { Point a; Point b; };
        let lib = TypeLibrary {
            name: "Lib".into(),
            types: alloc::vec![
                TypeDef::Struct(StructType {
                    name: "Point".into(),
                    extensibility: None,
                    base_type: None,
                    members: alloc::vec![
                        StructMember {
                            name: "x".into(),
                            type_ref: primitive(XmlPrimitive::Float),
                            ..Default::default()
                        },
                        StructMember {
                            name: "y".into(),
                            type_ref: primitive(XmlPrimitive::Float),
                            ..Default::default()
                        },
                    ],
                }),
                TypeDef::Struct(StructType {
                    name: "Line".into(),
                    extensibility: None,
                    base_type: None,
                    members: alloc::vec![
                        StructMember {
                            name: "a".into(),
                            type_ref: TypeRef::Named("Point".into()),
                            ..Default::default()
                        },
                        StructMember {
                            name: "b".into(),
                            type_ref: TypeRef::Named("Point".into()),
                            ..Default::default()
                        },
                    ],
                }),
            ],
        };
        let map = bridge_library(&lib).unwrap();
        let line = map.get("Line").expect("Line registered");
        if let MinimalTypeObject::Struct(st) = line {
            // Beide Member sollten auf denselben Hash zeigen.
            assert_eq!(
                st.member_seq[0].common.member_type_id,
                st.member_seq[1].common.member_type_id
            );
            assert!(matches!(
                st.member_seq[0].common.member_type_id,
                TypeIdentifier::EquivalenceHashMinimal(_)
            ));
        } else {
            panic!("Line not a struct");
        }
        // Beide Types in der Map.
        assert!(map.contains_key("Point"));
    }

    #[test]
    fn library_module_flattens_to_scoped_name() {
        let lib = TypeLibrary {
            name: "L".into(),
            types: alloc::vec![TypeDef::Module(ModuleEntry {
                name: "Inner".into(),
                types: alloc::vec![TypeDef::Struct(StructType {
                    name: "S".into(),
                    extensibility: None,
                    base_type: None,
                    members: alloc::vec![],
                })],
            })],
        };
        let map = bridge_library(&lib).unwrap();
        assert!(map.contains_key("Inner::S"));
    }

    #[test]
    fn library_unknown_named_ref_falls_back_to_zero_hash() {
        let lib = TypeLibrary {
            name: "L".into(),
            types: alloc::vec![TypeDef::Struct(StructType {
                name: "S".into(),
                extensibility: None,
                base_type: None,
                members: alloc::vec![StructMember {
                    name: "missing".into(),
                    type_ref: TypeRef::Named("DoesNotExist".into()),
                    ..Default::default()
                }],
            })],
        };
        let map = bridge_library(&lib).unwrap();
        let s = map.get("S").unwrap();
        if let MinimalTypeObject::Struct(st) = s {
            // Null-Hash als Fallback.
            assert_eq!(
                st.member_seq[0].common.member_type_id,
                TypeIdentifier::EquivalenceHashMinimal(zerodds_types::EquivalenceHash([0; 14]))
            );
        } else {
            panic!();
        }
    }

    // ---- TypeIdentifier-Hash --------------------------------------------

    #[test]
    fn typeobject_can_be_hashed() {
        let s = make_struct();
        let to = xml_type_to_typeobject(&TypeDef::Struct(s)).unwrap();
        let h = zerodds_types::compute_hash(&to).expect("hash");
        // Deterministisch: zweimal dasselbe Ergebnis.
        let h2 = zerodds_types::compute_hash(&to).unwrap();
        assert_eq!(h, h2);
    }

    #[test]
    fn xml_struct_matches_idl_struct_hash() {
        // Cross-Check: ein XML-Struct und eine direkt im Builder
        // formulierte aequivalente Definition sollten denselben Hash
        // haben (sofern Member-IDs/Flags/Types identisch sind).
        let xml_struct = StructType {
            name: "Sensor".into(),
            extensibility: Some(Extensibility::Appendable),
            base_type: None,
            members: alloc::vec![
                StructMember {
                    name: "id".into(),
                    type_ref: primitive(XmlPrimitive::LongLong),
                    key: true,
                    ..Default::default()
                },
                StructMember {
                    name: "temp".into(),
                    type_ref: primitive(XmlPrimitive::Float),
                    ..Default::default()
                },
            ],
        };
        let xml_mto = xml_type_to_minimal_typeobject(&TypeDef::Struct(xml_struct)).unwrap();
        let xml_hash = compute_minimal_hash(&xml_mto).unwrap();

        // Aequivalentes via Builder direkt:
        let builder_struct = TypeObjectBuilder::struct_type("Sensor")
            .extensibility(TypeExt::Appendable)
            .member("id", TypeIdentifier::Primitive(PrimitiveKind::Int64), |m| {
                m.key()
            })
            .member(
                "temp",
                TypeIdentifier::Primitive(PrimitiveKind::Float32),
                |m| m,
            )
            .build_minimal();
        let builder_mto = MinimalTypeObject::Struct(builder_struct);
        let builder_hash = compute_minimal_hash(&builder_mto).unwrap();
        assert_eq!(
            xml_hash, builder_hash,
            "XML-bridge und Builder muessen byte-identische TypeObjects produzieren"
        );
    }

    // ---- Display-Impl der BridgeError ------------------------------------

    #[test]
    fn bridge_error_display() {
        let e = BridgeError::UnsupportedXsdConstruct("foo".into());
        assert!(e.to_string().contains("foo"));
        let e = BridgeError::UnresolvedReference("Bar".into());
        assert!(e.to_string().contains("Bar"));
        let e = BridgeError::InvalidLiteral("x".into());
        assert!(e.to_string().contains("x"));
        let e = BridgeError::HashFailed("oom".into());
        assert!(e.to_string().contains("oom"));
        let e = BridgeError::ModuleAtTopLevel("M".into());
        assert!(e.to_string().contains("M"));
    }
}
