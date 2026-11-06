// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! What a type defined in Rust is in IDL terms.
//!
//! A [`TypeDesc`] is the declaration the IDL would have written: the
//! scoped name, the extensibility, the members with their annotations,
//! or the enumerators. The member types are not in it; they come from the
//! Rust types of the members ([`super::DdsTypeInfo::Members`]), each of
//! which describes itself as a [`ValueDesc`] ([`super::DdsMember::DESC`]).
//! Nothing here decides an octet: the description becomes IDL and the IDL
//! becomes the model the layout planner plans, as a `.idl` file would.

use alloc::borrow::Cow;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use super::{DdsMember, ExtensibilityKind, MemberList};

/// XTypes 1.3 §7.2.2.4.4.4.1 (`@final`, `@appendable`, `@mutable`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Extensibility {
    /// `@final`.
    Final,
    /// `@appendable`.
    Appendable,
    /// `@mutable`.
    Mutable,
}

impl Extensibility {
    /// The IDL annotation.
    #[must_use]
    pub const fn annotation(self) -> &'static str {
        match self {
            Self::Final => "@final",
            Self::Appendable => "@appendable",
            Self::Mutable => "@mutable",
        }
    }
}

/// XTypes 1.3 §7.3.1.2.1.6 (`@autoid`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoId {
    /// `@autoid(SEQUENTIAL)`.
    Sequential,
    /// `@autoid(HASH)`.
    Hash,
}

/// An IDL primitive type, by the Rust type that holds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Primitive {
    /// `boolean`, `bool`.
    Boolean,
    /// `octet`, `u8`.
    Octet,
    /// `char`, `char` (U+0000 to U+00FF).
    Char,
    /// `int8`, `i8`.
    Int8,
    /// `uint8`, `u8`.
    UInt8,
    /// `short`, `i16`.
    Int16,
    /// `unsigned short`, `u16`.
    UInt16,
    /// `long`, `i32`.
    Int32,
    /// `unsigned long`, `u32`.
    UInt32,
    /// `long long`, `i64`.
    Int64,
    /// `unsigned long long`, `u64`.
    UInt64,
    /// `float`, `f32`.
    Float32,
    /// `double`, `f64`.
    Float64,
}

impl Primitive {
    /// The IDL spelling.
    #[must_use]
    pub const fn idl(self) -> &'static str {
        match self {
            Self::Boolean => "boolean",
            Self::Octet => "octet",
            Self::Char => "char",
            Self::Int8 => "int8",
            Self::UInt8 => "uint8",
            Self::Int16 => "short",
            Self::UInt16 => "unsigned short",
            Self::Int32 => "long",
            Self::UInt32 => "unsigned long",
            Self::Int64 => "long long",
            Self::UInt64 => "unsigned long long",
            Self::Float32 => "float",
            Self::Float64 => "double",
        }
    }
}

/// The IDL type one Rust type stands for at a member position.
///
/// Nested descriptions are function pointers and not values, so a type
/// that reaches itself through a sequence or an `Option` has a finite
/// description.
#[derive(Debug, Clone, Copy)]
pub enum ValueDesc {
    /// A primitive.
    Primitive(Primitive),
    /// A `string`, with its bound when the type carries one. A member's
    /// own `bound` replaces it.
    String {
        /// `string<N>`'s `N`.
        bound: Option<u32>,
    },
    /// A `sequence<T>` or `sequence<T, N>`.
    Sequence {
        /// The element.
        element: fn() -> ValueDesc,
        /// `N`. A member's own `bound` replaces it.
        bound: Option<u32>,
    },
    /// One dimension of an array; `[[T; 3]; 2]` is `T x[2][3]`.
    Array {
        /// The element, itself an array for the inner dimensions.
        element: fn() -> ValueDesc,
        /// This dimension's length.
        len: u32,
    },
    /// An `@optional` member's value (`Option<T>`).
    Optional {
        /// The value when it is present.
        inner: fn() -> ValueDesc,
    },
    /// A struct or an enumeration with a description of its own.
    Type {
        /// Its description.
        desc: &'static TypeDesc,
    },
}

/// `@hashid` on a member, XTypes 1.3 §7.3.1.2.1.5.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashId {
    /// `@hashid`: the hash of the member's own name.
    Name,
    /// `@hashid("text")`: the hash of `text`.
    Text(&'static str),
}

/// One member as the IDL declares it, without its type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemberDesc {
    /// The member's IDL name.
    pub name: &'static str,
    /// `@key`.
    pub key: bool,
    /// `@id(N)`.
    pub id: Option<u32>,
    /// `@hashid`.
    pub hash_id: Option<HashId>,
    /// The bound of the member's `string` or outermost `sequence`.
    pub bound: Option<u32>,
}

impl MemberDesc {
    /// A member with no annotation.
    #[must_use]
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            key: false,
            id: None,
            hash_id: None,
            bound: None,
        }
    }

    /// `@key`.
    #[must_use]
    pub const fn key(mut self) -> Self {
        self.key = true;
        self
    }

    /// `@id(id)`.
    #[must_use]
    pub const fn id(mut self, id: u32) -> Self {
        self.id = Some(id);
        self
    }

    /// `@hashid` or `@hashid("text")`.
    #[must_use]
    pub const fn hash_id(mut self, hash_id: HashId) -> Self {
        self.hash_id = Some(hash_id);
        self
    }

    /// The bound of the member's `string` or outermost `sequence`.
    #[must_use]
    pub const fn bound(mut self, bound: u32) -> Self {
        self.bound = Some(bound);
        self
    }
}

/// One enumerator: its IDL name, and `@value` when it has one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnumeratorDesc {
    /// The enumerator's IDL name.
    pub name: &'static str,
    /// `@value(N)`.
    pub value: Option<i32>,
}

/// One case of a union, as the IDL declares it, without its type.
#[derive(Debug, Clone, Copy)]
pub struct CaseDesc {
    /// The branch member's IDL name.
    pub name: &'static str,
    /// The values of its `case` labels, as [`super::Discriminator::label`]
    /// gives them: a function, because a label of an enumeration is the
    /// enumerator's wire value, which only its Rust type knows.
    pub labels: fn() -> Vec<i128>,
    /// `default:`.
    pub default: bool,
    /// `@id(N)`.
    pub id: Option<u32>,
    /// `@hashid`.
    pub hash_id: Option<HashId>,
    /// The bound of the branch's `string` or outermost `sequence`.
    pub bound: Option<u32>,
}

impl CaseDesc {
    /// A case with the labels `labels` gives.
    #[must_use]
    pub const fn new(name: &'static str, labels: fn() -> Vec<i128>) -> Self {
        Self {
            name,
            labels,
            default: false,
            id: None,
            hash_id: None,
            bound: None,
        }
    }

    /// `default:`, after the case's own labels.
    #[must_use]
    pub const fn default_case(mut self) -> Self {
        self.default = true;
        self
    }

    /// `@id(id)`.
    #[must_use]
    pub const fn id(mut self, id: u32) -> Self {
        self.id = Some(id);
        self
    }

    /// `@hashid` or `@hashid("text")`.
    #[must_use]
    pub const fn hash_id(mut self, hash_id: HashId) -> Self {
        self.hash_id = Some(hash_id);
        self
    }

    /// The bound of the branch's `string` or outermost `sequence`.
    #[must_use]
    pub const fn bound(mut self, bound: u32) -> Self {
        self.bound = Some(bound);
        self
    }
}

/// One argument of a generic type's instantiation.
#[derive(Debug, Clone, Copy)]
pub enum TypeArg {
    /// A type parameter: the IDL type its Rust type stands for.
    Type(ValueDesc),
    /// A `const` parameter of an integer type.
    Int(i128),
    /// A `const` parameter of type `bool`.
    Bool(bool),
    /// A `const` parameter of type `char`.
    Char(char),
}

/// The parameters of a generic type and its instantiation's arguments.
#[derive(Debug, Clone, Copy)]
pub struct TypeArgs {
    /// The parameters' names, as the name template writes them (`{T}`).
    pub params: &'static [&'static str],
    /// The arguments, in the order of `params`.
    pub values: fn() -> Vec<TypeArg>,
}

/// What a described type is.
#[derive(Debug, Clone, Copy)]
pub enum TypeKind {
    /// A `struct`.
    Struct {
        /// Its extensibility.
        extensibility: Extensibility,
        /// `@autoid`, when written.
        autoid: Option<AutoId>,
        /// `@nested`.
        nested: bool,
        /// The members, in declaration order.
        members: &'static [MemberDesc],
        /// The members' value descriptions, from the Rust member types.
        values: fn(usize) -> Option<ValueDesc>,
        /// How many members the Rust member types name.
        count: usize,
    },
    /// An `enum`.
    Enum {
        /// The enumerators, in declaration order.
        enumerators: &'static [EnumeratorDesc],
        /// `@bit_bound`, when written.
        bit_bound: Option<u16>,
        /// `@final` or `@appendable`, when written (DDS-XTypes 1.3
        /// §7.2.3, Table 12: "Enumerated Types: These types may be final
        /// or appendable"); unwritten, the IDL default.
        extensibility: Option<Extensibility>,
    },
    /// A `union`.
    Union {
        /// Its extensibility.
        extensibility: Extensibility,
        /// `@autoid`, when written.
        autoid: Option<AutoId>,
        /// `@nested`.
        nested: bool,
        /// The discriminator's IDL type.
        discriminator: fn() -> ValueDesc,
        /// `switch (@key D)`: the discriminator is the union's key
        /// (DDS-XTypes 1.3 §7.2.2.4.4.4.8, "for union types, only the
        /// discriminator member can have the “is key” attribute").
        key: bool,
        /// The cases, in declaration order.
        cases: &'static [CaseDesc],
        /// The branches' value descriptions, from their Rust types.
        values: fn(usize) -> Option<ValueDesc>,
        /// How many branches the Rust branch types name.
        count: usize,
    },
}

/// A type's IDL declaration.
#[derive(Debug, Clone, Copy)]
pub struct TypeDesc {
    /// The scoped name, `app::Track`; for a generic type the template, in
    /// which `{T}` stands for the argument of the parameter `T`
    /// ([`TypeDesc::scoped_name`]).
    pub name: &'static str,
    /// A generic type's parameters and its instantiation's arguments.
    pub args: Option<TypeArgs>,
    /// What it is.
    pub kind: TypeKind,
}

impl TypeDesc {
    /// A struct over the member types `M` with the extensibility `E`.
    ///
    /// The value descriptions come from `M`, so a description written by
    /// hand cannot name another type for a member than the one the Rust
    /// code hands the serializer.
    #[must_use]
    pub const fn structure<M: MemberList, E: ExtensibilityKind>(
        name: &'static str,
        members: &'static [MemberDesc],
    ) -> Self {
        Self {
            name,
            args: None,
            kind: TypeKind::Struct {
                extensibility: E::KIND,
                autoid: None,
                nested: false,
                members,
                values: M::value_desc,
                count: M::LEN,
            },
        }
    }

    /// An enumeration.
    #[must_use]
    pub const fn enumeration(name: &'static str, enumerators: &'static [EnumeratorDesc]) -> Self {
        Self {
            name,
            args: None,
            kind: TypeKind::Enum {
                enumerators,
                bit_bound: None,
                extensibility: None,
            },
        }
    }

    /// A union over the branch types `M` with the extensibility `E` and the
    /// discriminator type `D`.
    #[must_use]
    pub const fn union<M: MemberList, E: ExtensibilityKind, D: DdsMember>(
        name: &'static str,
        cases: &'static [CaseDesc],
    ) -> Self {
        Self {
            name,
            args: None,
            kind: TypeKind::Union {
                extensibility: E::KIND,
                autoid: None,
                nested: false,
                discriminator: super::desc_of::<D>,
                key: false,
                cases,
                values: M::value_desc,
                count: M::LEN,
            },
        }
    }

    /// The parameters and arguments of a generic type's instantiation.
    #[must_use]
    pub const fn generic(mut self, args: TypeArgs) -> Self {
        self.args = Some(args);
        self
    }

    /// `@autoid(…)` on a struct or a union.
    #[must_use]
    pub const fn autoid(mut self, autoid: AutoId) -> Self {
        match self.kind {
            TypeKind::Struct {
                autoid: ref mut slot,
                ..
            }
            | TypeKind::Union {
                autoid: ref mut slot,
                ..
            } => *slot = Some(autoid),
            TypeKind::Enum { .. } => {}
        }
        self
    }

    /// `@nested` on a struct or a union.
    #[must_use]
    pub const fn nested(mut self) -> Self {
        match self.kind {
            TypeKind::Struct {
                nested: ref mut slot,
                ..
            }
            | TypeKind::Union {
                nested: ref mut slot,
                ..
            } => *slot = true,
            TypeKind::Enum { .. } => {}
        }
        self
    }

    /// `switch (@key D)` on a union: its discriminator is its key, and
    /// the union a keyed type (DDS-XTypes 1.3 §7.2.2.4.4.4.8). Nothing on
    /// a struct or an enumeration, whose key is on its members.
    #[must_use]
    pub const fn key_discriminator(mut self) -> Self {
        if let TypeKind::Union {
            key: ref mut slot, ..
        } = self.kind
        {
            *slot = true;
        }
        self
    }

    /// The scoped name: [`Self::name`], with each `{P}` of a generic
    /// type's template replaced by the spelling of its argument
    /// ([`arg_spelling`]): `app::Pair_{T}` for `Pair<u32>` is
    /// `app::Pair_unsigned_long`.
    #[must_use]
    pub fn scoped_name(&self) -> Cow<'static, str> {
        let Some(args) = self.args else {
            return Cow::Borrowed(self.name);
        };
        let values = (args.values)();
        let mut name = String::from(self.name);
        for (param, value) in args.params.iter().zip(values) {
            name = name.replace(&format!("{{{param}}}"), &arg_spelling(value));
        }
        Cow::Owned(name)
    }

    /// `@bit_bound(bits)` on an enumeration.
    #[must_use]
    pub const fn bit_bound(mut self, bits: u16) -> Self {
        if let TypeKind::Enum {
            bit_bound: ref mut slot,
            ..
        } = self.kind
        {
            *slot = Some(bits);
        }
        self
    }

    /// `@final` or `@appendable` on an enumeration (DDS-XTypes 1.3
    /// §7.2.3, Table 12). `@mutable` is no extensibility of an enumeration;
    /// the derive refuses it before it gets here.
    #[must_use]
    pub const fn enum_extensibility(mut self, kind: Extensibility) -> Self {
        if let TypeKind::Enum {
            extensibility: ref mut slot,
            ..
        } = self.kind
        {
            *slot = Some(kind);
        }
        self
    }

    /// How many members the description names, or the enumerators of an
    /// enumeration.
    #[must_use]
    pub const fn member_count(&self) -> usize {
        match self.kind {
            TypeKind::Struct { members, .. } => members.len(),
            TypeKind::Union { cases, .. } => cases.len(),
            TypeKind::Enum { enumerators, .. } => enumerators.len(),
        }
    }

    /// How many member types the Rust type names; for an enumeration,
    /// its enumerators.
    #[must_use]
    pub const fn type_count(&self) -> usize {
        match self.kind {
            TypeKind::Struct { count, .. } | TypeKind::Union { count, .. } => count,
            TypeKind::Enum { enumerators, .. } => enumerators.len(),
        }
    }
}

/// How a generic type's argument is spelled in its instantiation's IDL
/// name: an identifier part, deterministic, from the argument's IDL type.
///
/// - a primitive: its IDL spelling, spaces as `_` (`unsigned_long`);
/// - `string`, `string<8>`: `string`, `string_8`;
/// - `sequence<T>`, `sequence<T, 4>`: `sequence_T`, `sequence_T_4`;
/// - one array dimension `T[3]`: `array_3_T`;
/// - `@optional T`: `optional_T`;
/// - a described type: its scoped name, `::` as `_` (`app_Point`);
/// - a `const` argument: the integer (`m` in front of a negative one),
///   `true`/`false`, `u` and the hex code point of a `char`.
#[must_use]
pub fn arg_spelling(arg: TypeArg) -> String {
    match arg {
        TypeArg::Type(value) => value_spelling(value, 0),
        TypeArg::Int(value) if value < 0 => format!("m{}", value.unsigned_abs()),
        TypeArg::Int(value) => format!("{value}"),
        TypeArg::Bool(value) => String::from(if value { "true" } else { "false" }),
        TypeArg::Char(value) => format!("u{:x}", u32::from(value)),
    }
}

/// How deep [`value_spelling`] follows nested collections.
const MAX_SPELLING_DEPTH: usize = 64;

/// zerodds-lint: recursion-depth 64 (one level per collection of the
/// argument; bounded by `MAX_SPELLING_DEPTH`)
fn value_spelling(value: ValueDesc, depth: usize) -> String {
    if depth > MAX_SPELLING_DEPTH {
        return String::from("nested");
    }
    match value {
        ValueDesc::Primitive(primitive) => primitive.idl().replace(' ', "_"),
        ValueDesc::String { bound: None } => String::from("string"),
        ValueDesc::String { bound: Some(bound) } => format!("string_{bound}"),
        ValueDesc::Sequence {
            element,
            bound: None,
        } => format!("sequence_{}", value_spelling(element(), depth + 1)),
        ValueDesc::Sequence {
            element,
            bound: Some(bound),
        } => format!("sequence_{}_{bound}", value_spelling(element(), depth + 1)),
        ValueDesc::Array { element, len } => {
            format!("array_{len}_{}", value_spelling(element(), depth + 1))
        }
        ValueDesc::Optional { inner } => format!("optional_{}", value_spelling(inner(), depth + 1)),
        ValueDesc::Type { desc } => desc.scoped_name().replace("::", "_"),
    }
}
