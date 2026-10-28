// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The construct table. Spec §8.2.
//!
//! Every kind of type, and every facet property that a profile can
//! forbid, gets an explicit verdict in every profile. There is no
//! catch-all arm: a construct that has not been ruled on is a compile
//! error here rather than a silent pass at validation time, which is
//! what makes the coverage claim checkable instead of asserted.

use crate::facet::VendorKind;
use crate::node::type_::TypeKind;
use crate::profile::Profile;

/// Whether a profile admits a construct, and why not when it does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// The profile admits it.
    Allowed,
    /// The profile forbids it.
    Rejected {
        /// Short rule name.
        rule: &'static str,
        /// The paragraph the rule comes from.
        spec_reference: &'static str,
        /// Why the construct does not fit.
        detail: &'static str,
    },
}

impl Verdict {
    /// Whether the construct is admitted.
    #[must_use]
    pub const fn is_allowed(self) -> bool {
        matches!(self, Self::Allowed)
    }
}

/// A discriminator over `TypeKind`, so the table can be exhaustive
/// without constructing payloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ConstructKind {
    /// A built-in scalar other than the ones called out below.
    Primitive,
    /// `int8` or `uint8`, which IDL 4.2 introduced.
    NarrowInteger,
    /// `long double`.
    LongDouble,
    /// `fixed<d,s>`.
    Fixed,
    /// `string`.
    String,
    /// `wstring`.
    WideString,
    /// `sequence<T>`.
    Sequence,
    /// A one-dimensional array.
    Array1D,
    /// An array of two dimensions or more.
    ArrayMultiD,
    /// `map<K,V>`.
    Map,
    /// `struct`.
    Structure,
    /// `union`.
    Union,
    /// `enum`.
    Enumeration,
    /// `bitmask`.
    Bitmask,
    /// `bitset`.
    Bitset,
    /// `typedef`.
    Alias,
    /// `valuetype`.
    Valuetype,
    /// A boxed value.
    ValueBox,
    /// `native`.
    Native,
    /// `any`.
    Any,
    /// `ValueBase`.
    ValueBase,
    /// An interface used as a data type.
    ObjectRef,
    /// A type introduced by an `@annotation` declaration.
    AnnotationType,
}

impl ConstructKind {
    /// Every construct the model can hold.
    pub const ALL: [Self; 23] = [
        Self::Primitive,
        Self::NarrowInteger,
        Self::LongDouble,
        Self::Fixed,
        Self::String,
        Self::WideString,
        Self::Sequence,
        Self::Array1D,
        Self::ArrayMultiD,
        Self::Map,
        Self::Structure,
        Self::Union,
        Self::Enumeration,
        Self::Bitmask,
        Self::Bitset,
        Self::Alias,
        Self::Valuetype,
        Self::ValueBox,
        Self::Native,
        Self::Any,
        Self::ValueBase,
        Self::ObjectRef,
        Self::AnnotationType,
    ];

    /// Classify a type kind.
    ///
    /// Exhaustive on purpose: `TypeKind` is `#[non_exhaustive]` to its
    /// consumers but not within this crate, so adding a kind without
    /// ruling on it here is a compile error rather than a construct that
    /// silently passes every profile.
    #[must_use]
    pub fn of(kind: &TypeKind) -> Self {
        use crate::node::type_::{CharWidth, PrimitiveKind};
        match kind {
            TypeKind::Primitive(PrimitiveKind::Int8 | PrimitiveKind::UInt8) => Self::NarrowInteger,
            TypeKind::Primitive(PrimitiveKind::Float128) => Self::LongDouble,
            TypeKind::Primitive(_) => Self::Primitive,
            TypeKind::Fixed { .. } => Self::Fixed,
            TypeKind::String {
                width: CharWidth::Char,
                ..
            } => Self::String,
            TypeKind::String {
                width: CharWidth::WChar,
                ..
            } => Self::WideString,
            TypeKind::Sequence { .. } => Self::Sequence,
            TypeKind::Array { dims, .. } if dims.len() > 1 => Self::ArrayMultiD,
            TypeKind::Array { .. } => Self::Array1D,
            TypeKind::Map { .. } => Self::Map,
            TypeKind::Structure { .. } => Self::Structure,
            TypeKind::Union { .. } => Self::Union,
            TypeKind::Enumeration { .. } => Self::Enumeration,
            TypeKind::Bitmask { .. } => Self::Bitmask,
            TypeKind::Bitset { .. } => Self::Bitset,
            TypeKind::Alias { .. } => Self::Alias,
            TypeKind::Valuetype { .. } => Self::Valuetype,
            TypeKind::ValueBox { .. } => Self::ValueBox,
            TypeKind::Native => Self::Native,
            TypeKind::Any => Self::Any,
            TypeKind::ValueBase => Self::ValueBase,
            TypeKind::ObjectRef { .. } => Self::ObjectRef,
            TypeKind::AnnotationType { .. } => Self::AnnotationType,
        }
    }

    /// Source spelling, for diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Primitive => "a primitive",
            Self::NarrowInteger => "int8/uint8",
            Self::LongDouble => "long double",
            Self::Fixed => "fixed<>",
            Self::String => "string",
            Self::WideString => "wstring",
            Self::Sequence => "sequence<>",
            Self::Array1D => "a one-dimensional array",
            Self::ArrayMultiD => "a multi-dimensional array",
            Self::Map => "map<>",
            Self::Structure => "struct",
            Self::Union => "union",
            Self::Enumeration => "enum",
            Self::Bitmask => "bitmask",
            Self::Bitset => "bitset",
            Self::Alias => "typedef",
            Self::Valuetype => "valuetype",
            Self::ValueBox => "a value box",
            Self::Native => "native",
            Self::Any => "any",
            Self::ValueBase => "ValueBase",
            Self::ObjectRef => "an object reference",
            Self::AnnotationType => "an annotation type",
        }
    }
}

/// A facet property a profile can forbid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FacetProperty {
    /// `@key`.
    Key,
    /// `@optional`.
    Optional,
    /// `@external`.
    External,
    /// `@must_understand`.
    MustUnderstand,
    /// `@non_serialized`.
    NonSerialized,
    /// `@try_construct`.
    TryConstruct,
    /// `@appendable` or `@mutable` extensibility.
    NonFinalExtensibility,
    /// An explicit member id: `@id`, `@hashid`, `@autoid`.
    ExplicitMemberId,
    /// `@default` on a member.
    DefaultValue,
    /// `@range`, `@min`, `@max`.
    RangeConstraint,
    /// `@unit`.
    Unit,
    /// `@verbatim`.
    Verbatim,
    /// A ROS 2 default value, which ROS allows on any field.
    Ros2DefaultValue,
    /// A vendor facet of any kind.
    VendorDirective,
}

impl FacetProperty {
    /// Every property a profile rules on.
    pub const ALL: [Self; 14] = [
        Self::Key,
        Self::Optional,
        Self::External,
        Self::MustUnderstand,
        Self::NonSerialized,
        Self::TryConstruct,
        Self::NonFinalExtensibility,
        Self::ExplicitMemberId,
        Self::DefaultValue,
        Self::RangeConstraint,
        Self::Unit,
        Self::Verbatim,
        Self::Ros2DefaultValue,
        Self::VendorDirective,
    ];

    /// Source spelling, for diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Key => "@key",
            Self::Optional => "@optional",
            Self::External => "@external",
            Self::MustUnderstand => "@must_understand",
            Self::NonSerialized => "@non_serialized",
            Self::TryConstruct => "@try_construct",
            Self::NonFinalExtensibility => "@appendable or @mutable",
            Self::ExplicitMemberId => "an explicit member id",
            Self::DefaultValue => "@default",
            Self::RangeConstraint => "@range/@min/@max",
            Self::Unit => "@unit",
            Self::Verbatim => "@verbatim",
            Self::Ros2DefaultValue => "a ROS 2 default value",
            Self::VendorDirective => "a vendor directive",
        }
    }
}

/// Reject with a CORBA 3.3 reason.
const fn corba_no(detail: &'static str) -> Verdict {
    Verdict::Rejected {
        rule: "construct exists in CORBA 3.3",
        spec_reference: "CORBA 3.3 §7.4.1",
        detail,
    }
}

/// Reject with an XTypes reason.
const fn xtypes_no(detail: &'static str) -> Verdict {
    Verdict::Rejected {
        rule: "construct exists in DDS-XTypes 1.3",
        spec_reference: "DDS-XTypes 1.3 §7.2.2",
        detail,
    }
}

/// Reject with a ROS 2 reason.
const fn ros_no(detail: &'static str) -> Verdict {
    Verdict::Rejected {
        rule: "construct expressible in a ROS 2 interface file",
        spec_reference: "ROS 2 interface definition rules",
        detail,
    }
}

/// Reject with an IDL 4.2 reason.
const fn idl_no(detail: &'static str) -> Verdict {
    Verdict::Rejected {
        rule: "construct exists in IDL 4.2",
        spec_reference: "OMG IDL 4.2 §7.4",
        detail,
    }
}

/// Reject with a FlatData reason.
const fn flat_no(detail: &'static str) -> Verdict {
    Verdict::Rejected {
        rule: "plain type",
        spec_reference: "zerodds-zero-copy-1.0 §3",
        detail,
    }
}

/// Reject with a DDS-XML reason.
const fn xml_no(detail: &'static str) -> Verdict {
    Verdict::Rejected {
        rule: "construct expressible in DDS-XML",
        spec_reference: "DDS-XML 1.0 §7.4",
        detail,
    }
}

/// The verdict for one construct in one profile.
///
/// Every arm is written out. Adding a construct or a profile without
/// ruling on the pair is a compile error, which is the point.
#[must_use]
pub const fn construct_verdict(profile: Profile, construct: ConstructKind) -> Verdict {
    use ConstructKind as C;
    match profile {
        // IDL 4.2 is the widest data profile the model targets. It has
        // every construct except the ones DDS added on top.
        Profile::Idl42Core => match construct {
            C::Primitive
            | C::NarrowInteger
            | C::LongDouble
            | C::Fixed
            | C::String
            | C::WideString
            | C::Sequence
            | C::Array1D
            | C::ArrayMultiD
            | C::Map
            | C::Structure
            | C::Union
            | C::Enumeration
            | C::Bitmask
            | C::Bitset
            | C::Alias
            | C::Valuetype
            | C::ValueBox
            | C::Native
            | C::Any
            | C::ValueBase
            | C::ObjectRef
            | C::AnnotationType => Verdict::Allowed,
        },

        // XTypes 1.3 covers the data types; interface-shaped constructs
        // have no type identifier and cannot appear in a topic type.
        Profile::XTypes13 => match construct {
            C::Primitive
            | C::NarrowInteger
            | C::LongDouble
            | C::Fixed
            | C::String
            | C::WideString
            | C::Sequence
            | C::Array1D
            | C::ArrayMultiD
            | C::Map
            | C::Structure
            | C::Union
            | C::Enumeration
            | C::Bitmask
            | C::Bitset
            | C::Alias => Verdict::Allowed,
            C::Valuetype => xtypes_no("valuetype has no XTypes type identifier"),
            C::ValueBox => xtypes_no("a value box has no XTypes type identifier"),
            C::Native => xtypes_no("native is opaque and has no wire form"),
            C::Any => xtypes_no("XTypes has no `any`, and no type identifier for one"),
            C::ValueBase => xtypes_no("XTypes has no valuetype, so it has no base for one"),
            C::ObjectRef => xtypes_no("an object reference is not a data type"),
            C::AnnotationType => xtypes_no("an annotation type is metadata, not a topic type"),
        },

        // CORBA 3.3 predates the XTypes additions.
        Profile::Corba33 => match construct {
            C::Primitive
            | C::LongDouble
            | C::Fixed
            | C::String
            | C::WideString
            | C::Sequence
            | C::Array1D
            | C::ArrayMultiD
            | C::Structure
            | C::Union
            | C::Enumeration
            | C::Alias
            | C::Valuetype
            | C::ValueBox
            | C::Native
            | C::Any
            | C::ValueBase
            | C::ObjectRef => Verdict::Allowed,
            C::NarrowInteger => corba_no("int8 and uint8 were introduced by IDL 4.2"),
            C::Map => corba_no("map<> was introduced by XTypes and has no CORBA equivalent"),
            C::Bitmask => corba_no("bitmask was introduced by XTypes"),
            C::Bitset => corba_no("bitset was introduced by XTypes"),
            C::AnnotationType => corba_no("annotations were introduced by IDL 4.0"),
        },

        // A ROS 2 interface file expresses a flat message shape.
        Profile::Ros2 => match construct {
            C::Primitive
            | C::NarrowInteger
            | C::String
            | C::Sequence
            | C::Array1D
            | C::Structure
            | C::Alias => Verdict::Allowed,
            // `rosidl`'s IDL front end parses `enum` and its type model
            // then reports the name as unknown, so a message can name
            // one and nothing can carry it.
            // `crates/wire-probe/src/ros_idl.rs` refuses it in those
            // words; the table said `Allowed` and the two were a second
            // opinion about one specification.
            C::Enumeration => ros_no(
                "ROS 2's type model has no enumeration — its IDL parses `enum` \
                        and then reports the name as unknown",
            ),
            C::LongDouble => ros_no("ROS 2 has no long double"),
            C::Fixed => ros_no("ROS 2 has no fixed-point type"),
            C::WideString => ros_no("ROS 2 has no wide string"),
            C::ArrayMultiD => ros_no("ROS 2 arrays are one-dimensional"),
            C::Map => ros_no("ROS 2 message definitions have no map"),
            C::Union => ros_no("ROS 2 message definitions have no union"),
            C::Bitmask => ros_no("ROS 2 has no bitmask"),
            C::Bitset => ros_no("ROS 2 has no bitset"),
            C::Valuetype => ros_no("ROS 2 message definitions have no valuetype"),
            C::ValueBox => ros_no("ROS 2 message definitions have no value box"),
            C::Native => ros_no("ROS 2 has no native type"),
            C::Any => ros_no("ROS 2 message definitions have no `any`"),
            C::ValueBase => ros_no("ROS 2 message definitions have no valuetype"),
            C::ObjectRef => ros_no("ROS 2 has no object reference"),
            C::AnnotationType => ros_no("ROS 2 has no annotation type"),
        },

        // DDS-XML declares deployment over types; it names them rather
        // than defining every construct.
        Profile::DdsXml => match construct {
            C::Primitive
            | C::NarrowInteger
            | C::LongDouble
            | C::Fixed
            | C::String
            | C::WideString
            | C::Sequence
            | C::Array1D
            | C::ArrayMultiD
            | C::Map
            | C::Structure
            | C::Union
            | C::Enumeration
            | C::Bitmask
            | C::Bitset
            | C::Alias => Verdict::Allowed,
            C::Valuetype => xml_no("DDS-XML has no valuetype element"),
            C::ValueBox => xml_no("DDS-XML has no value box element"),
            C::Native => xml_no("DDS-XML cannot describe an opaque native type"),
            C::Any => xml_no("DDS-XML has no `any` element"),
            C::ValueBase => xml_no("DDS-XML has no valuetype element to have a base for"),
            C::ObjectRef => xml_no("DDS-XML has no object reference element"),
            C::AnnotationType => xml_no("DDS-XML has no annotation type element"),
        },

        // A plain type is fixed size with no indirection, so it can be
        // loaned rather than serialised.
        Profile::FlatData => match construct {
            C::Primitive
            | C::NarrowInteger
            | C::LongDouble
            | C::Fixed
            | C::Array1D
            | C::ArrayMultiD
            | C::Structure
            | C::Enumeration
            | C::Bitmask
            | C::Bitset
            | C::Alias => Verdict::Allowed,
            C::String => flat_no("a plain type cannot reach a string"),
            C::WideString => flat_no("a plain type cannot reach a wide string"),
            C::Sequence => flat_no("a plain type cannot reach a sequence"),
            C::Map => flat_no("a plain type cannot reach a map"),
            C::Union => flat_no("a union's active branch is not fixed"),
            C::Valuetype => flat_no("a valuetype is not loanable"),
            C::ValueBox => flat_no("a value box carries indirection"),
            C::Native => flat_no("native has no known size"),
            C::Any => flat_no("`any` decides what it holds at run time, so it has no fixed size"),
            C::ValueBase => flat_no("which valuetype arrives is decided at run time"),
            C::ObjectRef => flat_no("an object reference is a handle, not data"),
            C::AnnotationType => flat_no("an annotation type carries no sample data"),
        },

        // A vendor dialect admits everything the model can hold; what a
        // vendor rejects is its own extension surface, not the type
        // space, and is checked through its facet.
        Profile::Vendor(_) => match construct {
            C::Primitive
            | C::NarrowInteger
            | C::LongDouble
            | C::Fixed
            | C::String
            | C::WideString
            | C::Sequence
            | C::Array1D
            | C::ArrayMultiD
            | C::Map
            | C::Structure
            | C::Union
            | C::Enumeration
            | C::Bitmask
            | C::Bitset
            | C::Alias
            | C::Valuetype
            | C::ValueBox
            | C::Native
            | C::Any
            | C::ValueBase
            | C::ObjectRef
            | C::AnnotationType => Verdict::Allowed,
        },
    }
}

/// The verdict for one facet property in one profile.
#[must_use]
pub const fn facet_verdict(profile: Profile, property: FacetProperty) -> Verdict {
    use FacetProperty as F;
    match profile {
        Profile::Idl42Core => match property {
            F::Key
            | F::Optional
            | F::External
            | F::MustUnderstand
            | F::NonSerialized
            | F::TryConstruct
            | F::NonFinalExtensibility
            | F::ExplicitMemberId
            | F::DefaultValue
            | F::RangeConstraint
            | F::Unit
            | F::Verbatim => Verdict::Allowed,
            F::Ros2DefaultValue => {
                idl_no("a per-field default is a ROS 2 construct, spelled @default in IDL")
            }
            F::VendorDirective => idl_no("a vendor directive is outside IDL 4.2"),
        },

        Profile::XTypes13 => match property {
            F::Key
            | F::Optional
            | F::External
            | F::MustUnderstand
            | F::NonSerialized
            | F::TryConstruct
            | F::NonFinalExtensibility
            | F::ExplicitMemberId
            | F::DefaultValue
            | F::RangeConstraint
            | F::Unit
            | F::Verbatim => Verdict::Allowed,
            F::Ros2DefaultValue => xtypes_no("a ROS 2 default is not an XTypes annotation"),
            F::VendorDirective => xtypes_no("a vendor directive is outside XTypes 1.3"),
        },

        Profile::Corba33 => match property {
            F::Verbatim => Verdict::Allowed,
            F::Key => corba_no("@key is a DDS concept with no CORBA meaning"),
            F::Optional => corba_no("@optional was introduced by XTypes"),
            F::External => corba_no("@external was introduced by XTypes"),
            F::MustUnderstand => corba_no("@must_understand was introduced by XTypes"),
            F::NonSerialized => corba_no("@non_serialized was introduced by XTypes"),
            F::TryConstruct => corba_no("@try_construct was introduced by XTypes"),
            F::NonFinalExtensibility => {
                corba_no("CORBA structs do not evolve; extensibility is an XTypes concept")
            }
            F::ExplicitMemberId => corba_no("member ids were introduced by XTypes"),
            F::DefaultValue => corba_no("@default was introduced by XTypes"),
            F::RangeConstraint => corba_no("@range was introduced by XTypes"),
            F::Unit => corba_no("@unit was introduced by XTypes"),
            F::Ros2DefaultValue => corba_no("a ROS 2 default has no CORBA equivalent"),
            F::VendorDirective => corba_no("a vendor directive is outside CORBA 3.3"),
        },

        Profile::Ros2 => match property {
            F::Key | F::Ros2DefaultValue => Verdict::Allowed,
            F::Optional => ros_no("ROS 2 fields are always present"),
            F::External => ros_no("ROS 2 has no @external"),
            F::MustUnderstand => ros_no("ROS 2 has no @must_understand"),
            F::NonSerialized => ros_no("ROS 2 has no @non_serialized"),
            F::TryConstruct => ros_no("ROS 2 has no @try_construct"),
            F::NonFinalExtensibility => {
                ros_no("a ROS 2 message is final; evolution goes through a new type")
            }
            F::ExplicitMemberId => ros_no("ROS 2 fields carry no member id"),
            F::DefaultValue => {
                ros_no("a ROS 2 default is spelled in the .msg file, not as @default")
            }
            F::RangeConstraint => ros_no("ROS 2 has no @range"),
            F::Unit => ros_no("ROS 2 has no @unit"),
            F::Verbatim => ros_no("ROS 2 has no @verbatim"),
            F::VendorDirective => ros_no("a vendor directive is outside ROS 2"),
        },

        Profile::DdsXml => match property {
            F::Key
            | F::Optional
            | F::External
            | F::MustUnderstand
            | F::NonSerialized
            | F::TryConstruct
            | F::NonFinalExtensibility
            | F::ExplicitMemberId
            | F::DefaultValue
            | F::RangeConstraint
            | F::Unit => Verdict::Allowed,
            F::Verbatim => xml_no("DDS-XML has no @verbatim element"),
            F::Ros2DefaultValue => xml_no("a ROS 2 default has no DDS-XML element"),
            F::VendorDirective => xml_no("a vendor directive is outside DDS-XML 1.0"),
        },

        Profile::FlatData => match property {
            F::Key
            | F::MustUnderstand
            | F::ExplicitMemberId
            | F::DefaultValue
            | F::RangeConstraint
            | F::Unit
            | F::Verbatim
            | F::TryConstruct
            | F::NonSerialized
            | F::Ros2DefaultValue
            | F::VendorDirective => Verdict::Allowed,
            F::Optional => flat_no("an optional member makes the layout value-dependent"),
            F::External => flat_no("@external is indirection, which a loan cannot follow"),
            F::NonFinalExtensibility => {
                flat_no("a non-final aggregate carries a header, so its size is not fixed")
            }
        },

        Profile::Vendor(_) => match property {
            F::Key
            | F::Optional
            | F::External
            | F::MustUnderstand
            | F::NonSerialized
            | F::TryConstruct
            | F::NonFinalExtensibility
            | F::ExplicitMemberId
            | F::DefaultValue
            | F::RangeConstraint
            | F::Unit
            | F::Verbatim
            | F::Ros2DefaultValue
            | F::VendorDirective => Verdict::Allowed,
        },
    }
}

/// Every profile the table rules on, including one vendor dialect so the
/// exhaustiveness test covers that arm too.
pub const ALL_PROFILES: [Profile; 7] = [
    Profile::Idl42Core,
    Profile::XTypes13,
    Profile::Corba33,
    Profile::Ros2,
    Profile::DdsXml,
    Profile::FlatData,
    Profile::Vendor(VendorKind::Rti),
];
