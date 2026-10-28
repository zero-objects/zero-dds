// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Type nodes. Spec §3.2.

use crate::facet::Facets;
use crate::node::annotation::AnnotationDefNode;
use crate::node::constant::ConstExprNode;
use crate::node::interface::{InterfaceNode, OperationNode};
use crate::node::member::{BitValueNode, BitfieldNode, CaseNode, EnumeratorNode, MemberNode};
use crate::node::module::ModuleNode;
use alloc::string::String;
use alloc::vec::Vec;
use zerodds_ast_core::{Id, NodeId, Ref};

/// Built-in scalar kinds. Spec §3.2, primitive row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PrimitiveKind {
    /// `boolean`
    Boolean,
    /// `octet`
    Octet,
    /// `char`
    Char,
    /// `wchar`
    WChar,
    /// `int8`
    Int8,
    /// `uint8`
    UInt8,
    /// `short`
    Int16,
    /// `unsigned short`
    UInt16,
    /// `long`
    Int32,
    /// `unsigned long`
    UInt32,
    /// `long long`
    Int64,
    /// `unsigned long long`
    UInt64,
    /// `float`
    Float32,
    /// `double`
    Float64,
    /// `long double`.
    ///
    /// Representable regardless of any target language's ability to hold
    /// it: the model describes the type, it does not evaluate it. The
    /// known Rust `f128` gap constrains backends, not this model.
    Float128,
}

impl PrimitiveKind {
    /// Every primitive, in specification order.
    ///
    /// Used by conformance tests to assert the set is complete rather
    /// than merely claimed complete.
    pub const ALL: [Self; 15] = [
        Self::Boolean,
        Self::Octet,
        Self::Char,
        Self::WChar,
        Self::Int8,
        Self::UInt8,
        Self::Int16,
        Self::UInt16,
        Self::Int32,
        Self::UInt32,
        Self::Int64,
        Self::UInt64,
        Self::Float32,
        Self::Float64,
        Self::Float128,
    ];

    /// IDL keyword spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Boolean => "boolean",
            Self::Octet => "octet",
            Self::Char => "char",
            Self::WChar => "wchar",
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
            Self::Float128 => "long double",
        }
    }

    /// Whether the primitive is an integral type. Bitset holder types
    /// and enum bit bounds both need this distinction.
    #[must_use]
    pub const fn is_integral(self) -> bool {
        matches!(
            self,
            Self::Octet
                | Self::Int8
                | Self::UInt8
                | Self::Int16
                | Self::UInt16
                | Self::Int32
                | Self::UInt32
                | Self::Int64
                | Self::UInt64
        )
    }
}

/// Narrow or wide character element.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CharWidth {
    /// `char` / `string`
    Char,
    /// `wchar` / `wstring`
    WChar,
}

/// Collection bound.
///
/// Retains the expression, not only its value, so the declaration can be
/// rendered back to source. Spec §3.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Bound {
    /// No bound.
    Unbounded,
    /// Bounded by a constant expression.
    Const(Id<ConstExprNode>),
}

impl Bound {
    /// The bounding expression, if any.
    #[must_use]
    pub const fn expr(self) -> Option<Id<ConstExprNode>> {
        match self {
            Self::Unbounded => None,
            Self::Const(id) => Some(id),
        }
    }

    /// Whether the collection is bounded.
    #[must_use]
    pub const fn is_bounded(self) -> bool {
        matches!(self, Self::Const(_))
    }
}

/// Valuetype modifier. Spec §3.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValuetypeKind {
    /// Plain valuetype.
    Regular,
    /// `abstract valuetype`
    Abstract,
    /// `custom valuetype`
    Custom,
    /// `truncatable`
    Truncatable,
}

impl ValuetypeKind {
    /// Every modifier, for completeness assertions.
    pub const ALL: [Self; 4] = [
        Self::Regular,
        Self::Abstract,
        Self::Custom,
        Self::Truncatable,
    ];
}

/// Whether a declaration's body has arrived.
///
/// A forward declaration is not a separate kind: it is a node whose body
/// is still `Forward`, so handles taken before the definition arrives
/// stay valid when it does. Spec §3.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Completeness {
    /// Declared, body not yet seen.
    Forward,
    /// Body present.
    Complete,
}

/// What kind of type this is.
///
/// Spec §3.2 — the table there is normative and this enum mirrors it row
/// for row.
///
/// Exhaustive on purpose, for the same reason as [`crate::NodeKind`]: a
/// consumer mapping types onto another representation has to account for
/// each one, and a catch-all would let a new kind pass as nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeKind {
    /// Built-in scalar.
    Primitive(PrimitiveKind),
    /// `fixed<digits, scale>`.
    Fixed {
        /// Total digits.
        digits: u8,
        /// Digits after the point.
        scale: u8,
    },
    /// `string` or `wstring`.
    String {
        /// Character width.
        width: CharWidth,
        /// Optional bound.
        bound: Bound,
    },
    /// `sequence<T>` or `sequence<T, N>`.
    Sequence {
        /// Element type.
        element: Ref<TypeNode>,
        /// Optional bound.
        bound: Bound,
    },
    /// `T name[d1][d2]`.
    Array {
        /// Element type.
        element: Ref<TypeNode>,
        /// Dimensions, outermost first.
        dims: Vec<Id<ConstExprNode>>,
    },
    /// `map<K, V>` or `map<K, V, N>`.
    Map {
        /// Key type.
        key: Ref<TypeNode>,
        /// Value type.
        value: Ref<TypeNode>,
        /// Optional bound.
        bound: Bound,
    },
    /// `struct`.
    Structure {
        /// Base type for single inheritance.
        base: Option<Ref<TypeNode>>,
        /// Members in declaration order.
        members: Vec<Id<MemberNode>>,
    },
    /// `union`.
    Union {
        /// Discriminator type.
        discriminator: Ref<TypeNode>,
        /// Cases in declaration order.
        cases: Vec<Id<CaseNode>>,
        /// The `default:` case, if declared.
        default_case: Option<Id<CaseNode>>,
    },
    /// `enum`.
    Enumeration {
        /// Enumerators in declaration order.
        enumerators: Vec<Id<EnumeratorNode>>,
    },
    /// `bitmask`.
    Bitmask {
        /// Bit values in declaration order.
        bits: Vec<Id<BitValueNode>>,
    },
    /// `bitset`.
    Bitset {
        /// Inherited bitset.
        base: Option<Ref<TypeNode>>,
        /// Bitfields in declaration order.
        fields: Vec<Id<BitfieldNode>>,
    },
    /// `typedef`.
    Alias {
        /// The aliased type.
        aliased: Ref<TypeNode>,
    },
    /// `valuetype`.
    Valuetype {
        /// Modifier.
        kind: ValuetypeKind,
        /// Base valuetypes.
        bases: Vec<Ref<TypeNode>>,
        /// State members in declaration order.
        state: Vec<Id<MemberNode>>,
        /// Initialisers.
        initializers: Vec<Id<OperationNode>>,
        /// Supported interfaces.
        supported: Vec<Ref<InterfaceNode>>,
    },
    /// Boxed value.
    ValueBox {
        /// The boxed type.
        boxed: Ref<TypeNode>,
    },
    /// `native` — opaque, name only.
    Native,
    /// `ValueBase`. OMG IDL 4.2 §7.4.7, rules (131) and (132).
    ///
    /// The abstract base every valuetype has, written as a type: a value
    /// of any valuetype, whichever one arrives. It carries no reference
    /// for the same reason `Object` carries none — rule (131) is
    /// `<value_base_type> ::= "ValueBase"` and names nothing — and no
    /// state, because the base declares none.
    ///
    /// CORBA CDR encodes one as a chunked value with its repository id
    /// ahead of it. DDS-XTypes has neither valuetypes nor this, so the
    /// layout planner refuses it by name.
    ValueBase,
    /// `any`. OMG IDL 4.2 §7.4.2, rules (69) and (70).
    ///
    /// A value of any type, carrying its own type description. Building
    /// Block Any is one of the fifteen and this project takes all
    /// fifteen, so it is in the model — but it has no wire form under
    /// DDS-XTypes, which has no `any` at all. CORBA CDR writes a
    /// `TypeCode` and then the value; that is the only encoding in which
    /// it means anything, and the layout planner refuses it by name for
    /// every other one.
    ///
    /// It carries no reference: what an `any` holds is decided at run
    /// time and is not part of the declaration.
    Any,
    /// An object reference.
    ///
    /// Either the generic one — `Object`, OMG IDL 4.2 §7.4.6 rules (117)
    /// and (118) — or an interface used as a data type.
    ObjectRef {
        /// The referenced interface, absent for the generic `Object`.
        ///
        /// Rule (117) is `<object_type> ::= "Object"` and names no
        /// interface at all, so there is nothing to point at. `None`
        /// says that, where a reference to an invented `CORBA::Object`
        /// declaration would say the file had one.
        interface: Option<Ref<InterfaceNode>>,
    },
    /// A type introduced by an `@annotation` declaration.
    AnnotationType {
        /// The declaration.
        definition: Id<AnnotationDefNode>,
    },
}

impl TypeKind {
    /// Short name, for diagnostics.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Primitive(_) => "primitive",
            Self::Fixed { .. } => "fixed",
            Self::String { .. } => "string",
            Self::Sequence { .. } => "sequence",
            Self::Array { .. } => "array",
            Self::Map { .. } => "map",
            Self::Structure { .. } => "struct",
            Self::Union { .. } => "union",
            Self::Enumeration { .. } => "enum",
            Self::Bitmask { .. } => "bitmask",
            Self::Bitset { .. } => "bitset",
            Self::Alias { .. } => "typedef",
            Self::Valuetype { .. } => "valuetype",
            Self::ValueBox { .. } => "value box",
            Self::Native => "native",
            Self::Any => "any",
            Self::ValueBase => "ValueBase",
            Self::ObjectRef { .. } => "object reference",
            Self::AnnotationType { .. } => "annotation type",
        }
    }

    /// Whether this kind aggregates members that carry member ids —
    /// struct and valuetype state. Union cases carry ids through their
    /// member, which is why unions are not listed here.
    #[must_use]
    pub const fn is_member_aggregate(&self) -> bool {
        matches!(self, Self::Structure { .. } | Self::Valuetype { .. })
    }

    /// The members of an aggregate, empty for every other kind.
    #[must_use]
    pub fn members(&self) -> &[Id<MemberNode>] {
        match self {
            Self::Structure { members, .. } => members,
            Self::Valuetype { state, .. } => state,
            _ => &[],
        }
    }

    /// Every type this kind references directly, resolved or not.
    ///
    /// Drives cycle-safe traversal and layout dependency collection.
    #[must_use]
    pub fn type_refs(&self) -> Vec<&Ref<TypeNode>> {
        match self {
            Self::Sequence { element, .. } | Self::Array { element, .. } => {
                alloc::vec![element]
            }
            Self::Map { key, value, .. } => alloc::vec![key, value],
            Self::Alias { aliased } => alloc::vec![aliased],
            Self::ValueBox { boxed } => alloc::vec![boxed],
            Self::Union { discriminator, .. } => alloc::vec![discriminator],
            Self::Structure { base, .. } | Self::Bitset { base, .. } => {
                base.as_ref().map_or_else(Vec::new, |b| alloc::vec![b])
            }
            Self::Valuetype { bases, .. } => bases.iter().collect(),
            _ => Vec::new(),
        }
    }

    /// The same references, mutably and in the same order.
    ///
    /// The order is the slot numbering the undo log uses to name one
    /// reference of a kind that holds several, so the two functions have
    /// to stay in step; both are written as one match over every kind.
    pub fn type_refs_mut(&mut self) -> Vec<&mut Ref<TypeNode>> {
        match self {
            Self::Sequence { element, .. } | Self::Array { element, .. } => {
                alloc::vec![element]
            }
            Self::Map { key, value, .. } => alloc::vec![key, value],
            Self::Alias { aliased } => alloc::vec![aliased],
            Self::ValueBox { boxed } => alloc::vec![boxed],
            Self::Union { discriminator, .. } => alloc::vec![discriminator],
            Self::Structure { base, .. } | Self::Bitset { base, .. } => {
                base.as_mut().map_or_else(Vec::new, |b| alloc::vec![b])
            }
            Self::Valuetype { bases, .. } => bases.iter_mut().collect(),
            _ => Vec::new(),
        }
    }
}

impl TypeNode {
    /// The node's facets. A borrow helper, so a caller iterating types
    /// does not have to name the field through a temporary.
    #[must_use]
    pub const fn facets_ref(&self) -> &Facets {
        &self.facets
    }
}

/// A named type. Spec §3.2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeNode {
    /// Stable identity.
    pub node_id: NodeId,
    /// Simple name.
    pub name: String,
    /// Enclosing module.
    pub module: Id<ModuleNode>,
    /// The interface or value type this is declared in, when it is not
    /// declared directly in a module (OMG IDL 4.2 §7.4.3.4: an interface
    /// body holds type, constant and exception declarations). Its name is
    /// a segment of the scoped name and of the repository id:
    /// `CosNaming::NamingContext::NotFound`.
    pub scope: Option<String>,
    /// What kind of type.
    pub kind: TypeKind,
    /// Whether the body has arrived.
    pub completeness: Completeness,
    /// Per-source information. Spec §4.
    pub facets: Facets,
}
