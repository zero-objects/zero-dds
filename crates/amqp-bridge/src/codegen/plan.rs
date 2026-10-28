// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS-AMQP 1.0 §7 — the type mapping, read off the model.
//!
//! A [`TypePlan`] is what §7 says about one composite type: which AMQP
//! descriptor it carries (§7.2.1), which body form its extensibility
//! puts it in (§7.2.2), and, for each field in IDL declaration order,
//! which AMQP type code its IDL type maps to (§7.1).
//!
//! The plan is the single reading of §7. Three things consume it and
//! none of them re-reads the clause:
//!
//! 1. [`crate::codegen::encode`] — the Rust reference, which turns a
//!    sample into the described composite of §8.1.3 and into the JSON
//!    document of §8.1.2.
//! 2. The three language emitters, which write `toAmqpValue<T>` and
//!    `toJsonString<T>` that make the same decisions in C++, Java and
//!    TypeScript.
//! 3. `crates/amqp-endpoint/`, whose `encode_dds_to_amqp_body` has no
//!    type information of its own and falls back to a hex field
//!    without one.
//!
//! ## What §7.1's table is total over, and what it is not
//!
//! §7.1: "The mapping is total over IDL primitives; unmapped IDL types
//! SHALL cause the encoder to raise an `UnsupportedType` error."
//! [`FieldEncoding`] covers every row of the primitive table plus
//! sequences, arrays, maps, enumerations and nested composites.
//! [`PlanError::UnsupportedType`] is what a type outside that reaches,
//! and it names the type rather than encoding it as something else.

use alloc::boxed::Box;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use zerodds_ast::{
    Ast, CharWidth, Extensibility, PrimitiveKind, TypeKind, TypeNode, ValuetypeKind,
};
use zerodds_ast_core::Id;

/// How deep a type walk follows aliases, sequences and arrays.
///
/// A recursive IDL type reaches this only through a `sequence`, which
/// the walk descends once per nesting level. The bound is here so a
/// model built by hand with a cycle stops with an error rather than
/// overflowing the stack.
const MAX_TYPE_DEPTH: u32 = 64;

/// The type mapping refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    /// §7.1 — an IDL type outside the mapping. Carries the type's
    /// scoped name, or its kind where it has no name.
    UnsupportedType(String),
    /// The subject of a plan is not a composite. §7.2 defines the
    /// mapping for a `struct`, a `union` and a `valuetype`; a plan of a
    /// `long` is a question with no answer.
    NotAComposite(String),
    /// A type reference the model did not resolve.
    Unresolved(String),
    /// The walk hit `MAX_TYPE_DEPTH`.
    TooDeep,
}

impl core::fmt::Display for PlanError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnsupportedType(name) => {
                write!(f, "DDS-AMQP 1.0 §7.1 has no mapping for `{name}`")
            }
            Self::NotAComposite(name) => write!(
                f,
                "`{name}` is not a struct, union or valuetype, so §7.2 has no body form for it"
            ),
            Self::Unresolved(name) => write!(f, "`{name}` did not resolve in the model"),
            Self::TooDeep => f.write_str("the type walk exceeded its depth bound"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for PlanError {}

/// The AMQP descriptor a composite carries. §7.2.1.
///
/// Both forms are built from the type's X-Types `TypeIdentifier`, which
/// the caller supplies: deriving it here would need the whole
/// TypeObject machinery and would produce a second answer beside
/// `zerodds-types`'.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Descriptor {
    /// §7.2.1.1 truncated form — the first eight octets of the
    /// equivalence hash as a big-endian `ulong`.
    Ulong(u64),
    /// §7.2.1.2 full-identifier form — `dds:type:` followed by the
    /// lower-case hexadecimal of the identifier bytes, as a `symbol`.
    Symbol(String),
}

impl Descriptor {
    /// §7.2.1.1 — the truncated form of an identifier.
    ///
    /// An identifier shorter than eight octets is padded on the right
    /// with zeros rather than refused: the clause reads "the first eight
    /// octets", and every `TypeIdentifier` variant this can be asked
    /// about is at least fourteen.
    #[must_use]
    pub fn truncated(identifier: &[u8]) -> Self {
        let mut bytes = [0_u8; 8];
        for (slot, byte) in bytes.iter_mut().zip(identifier.iter()) {
            *slot = *byte;
        }
        Self::Ulong(u64::from_be_bytes(bytes))
    }

    /// §7.2.1.2 — the full-identifier form of an identifier.
    #[must_use]
    pub fn full(identifier: &[u8]) -> Self {
        let mut out = String::from("dds:type:");
        for byte in identifier {
            out.push(char::from_digit(u32::from(byte >> 4), 16).unwrap_or('0'));
            out.push(char::from_digit(u32::from(byte & 0x0F), 16).unwrap_or('0'));
        }
        Self::Symbol(out)
    }
}

/// The body form §7.2.2 puts a composite in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyForm {
    /// §7.2.2 — an AMQP `list` whose elements are the fields in IDL
    /// declaration order. The form for `@final` and `@appendable`.
    List,
    /// §7.2.2.5 — an AMQP `map` keyed by the member IDs, for a
    /// `@mutable` composite. "The list-positional form is invalid for
    /// `@mutable` types because their member-set evolves across
    /// versions."
    Map,
    /// §7.2.3 — a two-element list of discriminator and active branch.
    Union,
}

/// The AMQP type code an IDL type maps to. §7.1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldEncoding {
    /// `boolean` — `0x41` / `0x42`.
    Boolean,
    /// `octet` and `uint8` — `0x50`.
    Ubyte,
    /// `int8` — `0x51`.
    Byte,
    /// `unsigned short` — `0x60`.
    Ushort,
    /// `short` — `0x61`.
    Short,
    /// `unsigned long` — `0x70`.
    Uint,
    /// `long` — `0x71`.
    Int,
    /// `unsigned long long` — `0x80`.
    Ulong,
    /// `long long` — `0x81`.
    Long,
    /// `float` — `0x72`.
    Float,
    /// `double` — `0x82`.
    Double,
    /// `long double` — `0x82`, narrowed. §7.1.2.
    LongDouble,
    /// `char` — `0x73`, the ASCII subset. §7.1.4.1.
    Char,
    /// `wchar` — `0x73`, the UTF-32 code point. §7.1.4.2.
    WChar,
    /// `string` — `0xA1` / `0xB1`, UTF-8 verbatim.
    Str,
    /// `wstring` — `0xA1` / `0xB1`, transcoded to UTF-8. §7.1.5.2.
    WStr,
    /// A `sequence<octet>` or `octet[]` — AMQP `binary`, and Base64 in
    /// JSON mode. §8.1.2.
    Binary,
    /// An enumeration — the symbolic name as a `symbol`, and a JSON
    /// string of the same name. §8.1.2: "`enum`: JSON string of the
    /// symbolic name."
    Enumeration {
        /// The type's scoped name.
        name: String,
        /// The enumerator names, by declaration order.
        literals: Vec<String>,
    },
    /// A `sequence<T>` — an AMQP `list`. §7.1.7.
    Sequence(Box<FieldEncoding>),
    /// A `T[N]` — an AMQP `list`, like a sequence. §7.1.7.
    Array(Box<FieldEncoding>),
    /// A `map<K, V>` — an AMQP `map`. §7.1.8.
    Map(Box<FieldEncoding>, Box<FieldEncoding>),
    /// A nested `struct`, `union` or `valuetype` — a described
    /// composite of its own. §7.2.2.4.
    Composite(String),
    /// `DDS::Time_t` — an AMQP `timestamp` of milliseconds, with the
    /// nanosecond remainder carried beside it. §7.1.3.
    Time,
}

/// One field of a composite, in IDL declaration order. §7.2.2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldPlan {
    /// The IDL member name, which is the JSON object's key. §8.1.2.
    pub name: String,
    /// The member ID, which is the map key of a `@mutable` body.
    /// §7.2.2.5.
    pub id: u32,
    /// Whether the member is `@optional`. §7.2.2.1: an absent one is
    /// the AMQP `null` at its positional slot, and §8.1.2 omits it from
    /// the JSON object rather than writing `null`.
    pub optional: bool,
    /// What §7.1 maps the member's type to.
    pub encoding: FieldEncoding,
}

/// What §7 says about one composite type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypePlan {
    /// The type's scoped name in the source's `::` form.
    pub scoped_name: String,
    /// The descriptor of §7.2.1.
    pub descriptor: Descriptor,
    /// The body form of §7.2.2.
    pub body: BodyForm,
    /// The fields, in IDL declaration order. For a union this is the
    /// discriminator followed by the branches.
    pub fields: Vec<FieldPlan>,
}

impl TypePlan {
    /// The last name segment, which is what a generated helper is named
    /// after: `toAmqpValue_M_T` takes the whole scoped name, and a
    /// nested composite's own file takes this.
    #[must_use]
    pub fn simple_name(&self) -> &str {
        self.scoped_name.rsplit("::").next().unwrap_or("")
    }

    /// The scoped name with `_` between segments, for a helper name.
    ///
    /// Each segment's own underscores are doubled first, so `A::B_C` and
    /// `A::B::C` do not both become `A_B_C`. A plain `join("_")` mapped
    /// both to one name, which is a silent collision between two
    /// generated helpers.
    #[must_use]
    pub fn helper_suffix(&self) -> String {
        self.scoped_name
            .split("::")
            .map(|segment| segment.replace('_', "__"))
            .collect::<Vec<_>>()
            .join("_")
    }
}

/// The plan for a composite type in the model.
///
/// `identifier` is the type's X-Types `TypeIdentifier` bytes and
/// `full_descriptor` chooses between §7.2.1.1's truncated form and
/// §7.2.1.2's full one — the `descriptor_form` setting of Annex A.
///
/// # Errors
/// [`PlanError`] — the subject is not a composite, a member's type has
/// no §7.1 mapping, or a reference did not resolve.
pub fn plan_of(
    ast: &Ast,
    id: Id<TypeNode>,
    identifier: &[u8],
    full_descriptor: bool,
) -> Result<TypePlan, PlanError> {
    let node = ast
        .type_(id)
        .map_err(|_| PlanError::Unresolved("the plan subject".to_string()))?;
    let scoped_name = scoped_name_of(ast, id);
    let descriptor = if full_descriptor {
        Descriptor::full(identifier)
    } else {
        Descriptor::truncated(identifier)
    };
    let (body, fields) = match &node.kind {
        TypeKind::Structure { members, .. } => (BodyForm::List, struct_fields(ast, members)?),
        TypeKind::Valuetype { kind, state, .. } => {
            // A `custom valuetype` marshals itself, so §7.2.2's
            // positional body is not the encoding it uses; the mapping
            // has nothing to say about what its `marshal` writes.
            if matches!(kind, ValuetypeKind::Custom) {
                return Err(PlanError::UnsupportedType(scoped_name));
            }
            (BodyForm::List, struct_fields(ast, state)?)
        }
        TypeKind::Union {
            discriminator,
            cases,
            ..
        } => (BodyForm::Union, union_fields(ast, discriminator, cases)?),
        _ => return Err(PlanError::NotAComposite(scoped_name)),
    };
    // §7.2.2.5 — a `@mutable` composite is a map keyed by member ID.
    // The discriminator and branch of a union are not members and have
    // no IDs, so the union form wins over the mutable one.
    let body = match (body, ast.effective_extensibility(id)) {
        (BodyForm::List, Extensibility::Mutable) => BodyForm::Map,
        (other, _) => other,
    };
    Ok(TypePlan {
        scoped_name,
        descriptor,
        body,
        fields,
    })
}

fn struct_fields(
    ast: &Ast,
    members: &[Id<zerodds_ast::MemberNode>],
) -> Result<Vec<FieldPlan>, PlanError> {
    let mut out = Vec::new();
    for member in members {
        let node = ast
            .member(*member)
            .map_err(|_| PlanError::Unresolved("a member".to_string()))?;
        let target = node
            .type_
            .resolved()
            .ok_or_else(|| PlanError::Unresolved(node.name.clone()))?;
        out.push(FieldPlan {
            name: node.name.clone(),
            id: node
                .facets
                .xtypes
                .as_ref()
                .and_then(zerodds_ast::XTypesFacet::effective_member_id)
                .unwrap_or(0),
            optional: node
                .facets
                .xtypes
                .as_ref()
                .is_some_and(|facet| facet.is_optional),
            encoding: encoding_of(ast, target, 0)?,
        });
    }
    Ok(out)
}

/// §7.2.3 — the discriminator, then one entry per branch.
///
/// The branches are listed so a generator knows which member each label
/// selects; the encoding writes exactly one of them, which is what "the
/// active branch's value" means.
fn union_fields(
    ast: &Ast,
    discriminator: &zerodds_ast_core::Ref<TypeNode>,
    cases: &[Id<zerodds_ast::CaseNode>],
) -> Result<Vec<FieldPlan>, PlanError> {
    let target = discriminator
        .resolved()
        .ok_or_else(|| PlanError::Unresolved("the discriminator".to_string()))?;
    let mut out = alloc::vec![FieldPlan {
        name: "discriminator".to_string(),
        id: 0,
        optional: false,
        encoding: encoding_of(ast, target, 0)?,
    }];
    for case in cases {
        let node = ast
            .case(*case)
            .map_err(|_| PlanError::Unresolved("a case".to_string()))?;
        let member = ast
            .member(node.member)
            .map_err(|_| PlanError::Unresolved("a case member".to_string()))?;
        let branch = member
            .type_
            .resolved()
            .ok_or_else(|| PlanError::Unresolved(member.name.clone()))?;
        out.push(FieldPlan {
            name: member.name.clone(),
            id: member
                .facets
                .xtypes
                .as_ref()
                .and_then(zerodds_ast::XTypesFacet::effective_member_id)
                .unwrap_or(0),
            optional: false,
            encoding: encoding_of(ast, branch, 0)?,
        });
    }
    Ok(out)
}

/// §7.1 — the AMQP code an IDL type maps to.
///
/// zerodds-lint: recursion-depth 64 (alias, sequence, array and map
/// nesting; bounded by `MAX_TYPE_DEPTH`, which returns
/// `PlanError::TooDeep`). A nested composite is a leaf of this walk —
/// it is named, not descended into — so a recursive IDL type cannot
/// cycle here; only the encoders that follow the plan graph can, and
/// `MAX_SAMPLE_DEPTH` in `super::encode` bounds those.
fn encoding_of(ast: &Ast, id: Id<TypeNode>, depth: u32) -> Result<FieldEncoding, PlanError> {
    if depth >= MAX_TYPE_DEPTH {
        return Err(PlanError::TooDeep);
    }
    let node = ast
        .type_(id)
        .map_err(|_| PlanError::Unresolved("a member type".to_string()))?;
    match &node.kind {
        TypeKind::Alias { aliased } => {
            let target = aliased
                .resolved()
                .ok_or_else(|| PlanError::Unresolved(node.name.clone()))?;
            encoding_of(ast, target, depth + 1)
        }
        TypeKind::Primitive(primitive) => primitive_encoding(*primitive)
            .ok_or_else(|| PlanError::UnsupportedType(scoped_name_of(ast, id))),
        TypeKind::String {
            width: CharWidth::Char,
            ..
        } => Ok(FieldEncoding::Str),
        TypeKind::String {
            width: CharWidth::WChar,
            ..
        } => Ok(FieldEncoding::WStr),
        TypeKind::Enumeration { enumerators } => {
            let mut literals = Vec::new();
            for enumerator in enumerators {
                let enumerator = ast
                    .enumerator(*enumerator)
                    .map_err(|_| PlanError::Unresolved("an enumerator".to_string()))?;
                literals.push(enumerator.name.clone());
            }
            Ok(FieldEncoding::Enumeration {
                name: scoped_name_of(ast, id),
                literals,
            })
        }
        TypeKind::Sequence { element, .. } => {
            let target = element
                .resolved()
                .ok_or_else(|| PlanError::Unresolved("a sequence element".to_string()))?;
            let inner = encoding_of(ast, target, depth + 1)?;
            // §8.1.2 gives an octet sequence its own row: Base64,
            // rather than a JSON array of numbers.
            if inner == FieldEncoding::Ubyte {
                return Ok(FieldEncoding::Binary);
            }
            Ok(FieldEncoding::Sequence(Box::new(inner)))
        }
        TypeKind::Array { element, .. } => {
            let target = element
                .resolved()
                .ok_or_else(|| PlanError::Unresolved("an array element".to_string()))?;
            let inner = encoding_of(ast, target, depth + 1)?;
            if inner == FieldEncoding::Ubyte {
                return Ok(FieldEncoding::Binary);
            }
            Ok(FieldEncoding::Array(Box::new(inner)))
        }
        TypeKind::Map { key, value, .. } => {
            let key_target = key
                .resolved()
                .ok_or_else(|| PlanError::Unresolved("a map key".to_string()))?;
            let value_target = value
                .resolved()
                .ok_or_else(|| PlanError::Unresolved("a map value".to_string()))?;
            Ok(FieldEncoding::Map(
                Box::new(encoding_of(ast, key_target, depth + 1)?),
                Box::new(encoding_of(ast, value_target, depth + 1)?),
            ))
        }
        TypeKind::Structure { .. } | TypeKind::Union { .. } | TypeKind::Valuetype { .. } => {
            let scoped = scoped_name_of(ast, id);
            // §7.1.3 — `DDS::Time_t` is a timestamp and not an ordinary
            // two-member structure. Recognised by name because that is
            // what it is: a declared IDL structure in the `DDS` module.
            if is_time(&scoped) {
                return Ok(FieldEncoding::Time);
            }
            Ok(FieldEncoding::Composite(scoped))
        }
        // §7.1: everything else raises `UnsupportedType` rather than
        // being encoded as the nearest thing. A `bitmask` and a
        // `bitset` have no AMQP code, an `any` and a `TypeCode` carry
        // their own type description that AMQP has no room for, and a
        // `fixed` is a decimal whose scale the AMQP decimals do not
        // carry.
        _ => Err(PlanError::UnsupportedType(scoped_name_of(ast, id))),
    }
}

/// §7.1's primitive table, row for row.
const fn primitive_encoding(primitive: PrimitiveKind) -> Option<FieldEncoding> {
    Some(match primitive {
        PrimitiveKind::Boolean => FieldEncoding::Boolean,
        PrimitiveKind::Octet | PrimitiveKind::UInt8 => FieldEncoding::Ubyte,
        PrimitiveKind::Int8 => FieldEncoding::Byte,
        PrimitiveKind::UInt16 => FieldEncoding::Ushort,
        PrimitiveKind::Int16 => FieldEncoding::Short,
        PrimitiveKind::UInt32 => FieldEncoding::Uint,
        PrimitiveKind::Int32 => FieldEncoding::Int,
        PrimitiveKind::UInt64 => FieldEncoding::Ulong,
        PrimitiveKind::Int64 => FieldEncoding::Long,
        PrimitiveKind::Float32 => FieldEncoding::Float,
        PrimitiveKind::Float64 => FieldEncoding::Double,
        PrimitiveKind::Float128 => FieldEncoding::LongDouble,
        PrimitiveKind::Char => FieldEncoding::Char,
        PrimitiveKind::WChar => FieldEncoding::WChar,
    })
}

/// Whether a scoped name is the DDS `Time_t`. §7.1.3.
fn is_time(scoped: &str) -> bool {
    matches!(scoped, "DDS::Time_t" | "DDS::Duration_t" | "Time_t")
}

/// The scoped name of a type node, in the source's `::` form.
fn scoped_name_of(ast: &Ast, id: Id<TypeNode>) -> String {
    let Ok(node) = ast.type_(id) else {
        return String::new();
    };
    ast.scoped_name_of(zerodds_ast::AnyId::Type(id, node.node_id))
        .map_or_else(
            || {
                if node.name.is_empty() {
                    format!("{:?}", core::mem::discriminant(&node.kind))
                } else {
                    node.name.clone()
                }
            },
            |scoped| scoped.segments().join("::"),
        )
}
