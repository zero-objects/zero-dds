// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The C# type of an IDL type, §7.2.4 and §7.14.3, with what a setter has
//! to check about a value of it.

use super::{Gen, Idl4CsharpError, Platform, Settings};
use zerodds_ast::{
    Bound, CharWidth, ConstValue, PrimitiveKind, RangeConstraint, TypeKind, TypeNode,
};
use zerodds_ast_core::{Id, Ref};

/// What a type is in C#, beyond its spelling.
#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    /// `bool`.
    Bool,
    /// `char` (`char` and `wchar`, §7.2.4.1.3, §7.2.4.1.4).
    Char {
        /// Whether it is a `wchar`.
        wide: bool,
    },
    /// `sbyte` to `ulong`, Table 7.2.
    Integer {
        /// 8, 16, 32 or 64.
        bits: u8,
        /// Whether the C# type is signed.
        signed: bool,
    },
    /// `float`.
    Float,
    /// `double`.
    Double,
    /// `decimal` for `long double` (Table 7.3).
    LongDouble,
    /// `decimal` for `fixed<digits, scale>` (§7.2.4.2.4).
    Fixed {
        /// Total digits.
        digits: u8,
        /// Digits after the point.
        scale: u8,
    },
    /// `string` for `string` and `wstring` (§7.2.4.2.2, §7.2.4.2.3).
    Str {
        /// The bound, if any.
        bound: Option<u64>,
        /// Whether it is a `wstring`.
        wide: bool,
    },
    /// `Omg.Types.ISequence<T>` (§7.2.4.2.1).
    Sequence {
        /// The element.
        element: Box<CType>,
        /// The bound, if any.
        bound: Option<u64>,
    },
    /// A C# array (§7.2.4.4): the element and every dimension.
    Array {
        /// The element.
        element: Box<CType>,
        /// The dimensions, outermost first.
        dims: Vec<u64>,
    },
    /// `System.Collections.Generic.IDictionary<TKey, TValue>` (§7.14.3.1).
    Map {
        /// The key.
        key: Box<CType>,
        /// The value.
        value: Box<CType>,
        /// The bound, if any.
        bound: Option<u64>,
    },
    /// A class with a public default constructor and a copy constructor:
    /// a structure, a union, a value type.
    Class,
    /// A C# `struct`: a bit set (§7.14.3.2), a structure under
    /// `struct_type = "struct"` (§8.1.3).
    Struct,
    /// A C# `enum` (§7.2.4.3.3); the enumerator a default constructor
    /// starts a member at.
    Enum {
        /// The qualified enumerator.
        default: String,
    },
    /// `System.Collections.BitArray` for a bit mask (§7.14.3.3).
    BitArray {
        /// `@bit_bound`.
        bound: u16,
    },
    /// The `any` of §7.3 or A.1.4.
    Any,
    /// An object reference: an interface, `Object`, `ValueBase`.
    Reference,
}

/// A C# type: its spelling and its shape.
#[derive(Debug, Clone, PartialEq)]
pub struct CType {
    /// The C# spelling, qualified with `global::`.
    pub text: String,
    /// What it is.
    pub shape: Shape,
    /// `@range`, `@min`, `@max` collected through the typedefs it was
    /// reached by (§7.16.2.1).
    pub range: Option<Range>,
    /// The IDL kind of a primitive, for its literals.
    pub primitive: Option<PrimitiveKind>,
}

/// The bounds `@range`, `@min` and `@max` set, as evaluated values.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Range {
    /// The least value admitted.
    pub min: Option<ConstValue>,
    /// The greatest value admitted.
    pub max: Option<ConstValue>,
}

impl CType {
    fn of(text: &str, shape: Shape) -> Self {
        Self {
            text: text.to_string(),
            shape,
            range: None,
            primitive: None,
        }
    }

    /// Whether the C# type is a value type: what `System.Nullable<T>`
    /// takes (Table 7.5, Table 7.8).
    #[must_use]
    pub fn is_value_type(&self) -> bool {
        matches!(
            self.shape,
            Shape::Bool
                | Shape::Char { .. }
                | Shape::Integer { .. }
                | Shape::Float
                | Shape::Double
                | Shape::LongDouble
                | Shape::Fixed { .. }
                | Shape::Enum { .. }
                | Shape::Struct
        )
    }

    /// The type an `@optional` or `@external` member of this type has:
    /// "IDL elements annotated with @optional whose type T maps to a C#
    /// value type shall map to `System.Nullable<T>`. IDL types mapped to
    /// reference types shall remain unchanged."
    #[must_use]
    pub fn nullable(&self) -> Self {
        if self.is_value_type() {
            Self {
                text: format!("global::System.Nullable<{}>", self.text),
                ..self.clone()
            }
        } else {
            self.clone()
        }
    }
}

/// The C# type of an IDL integer type, Table 7.2.
fn integer(kind: PrimitiveKind) -> CType {
    use PrimitiveKind as P;
    let (text, bits, signed) = match kind {
        P::Int8 => ("sbyte", 8, true),
        P::Octet | P::UInt8 => ("byte", 8, false),
        P::Int16 => ("short", 16, true),
        P::UInt16 => ("ushort", 16, false),
        P::Int32 => ("int", 32, true),
        P::UInt32 => ("uint", 32, false),
        P::Int64 => ("long", 64, true),
        _ => ("ulong", 64, false),
    };
    CType {
        primitive: Some(kind),
        ..CType::of(text, Shape::Integer { bits, signed })
    }
}

impl Gen<'_> {
    /// The C# type of the type `reference` names, with what the typedefs
    /// on the way annotate.
    pub(super) fn cs_type(
        &self,
        reference: &Ref<TypeNode>,
        settings: &Settings,
        at: &str,
    ) -> Result<CType, Idl4CsharpError> {
        let id = reference
            .resolved()
            .ok_or_else(|| Idl4CsharpError::Unresolved {
                what: "a type".to_string(),
                at: at.to_string(),
            })?;
        self.cs_type_of(id, settings, at, 0)
    }

    pub(super) fn cs_type_of(
        &self,
        id: Id<TypeNode>,
        settings: &Settings,
        at: &str,
        depth: usize,
    ) -> Result<CType, Idl4CsharpError> {
        if depth > super::MAX_DEPTH {
            return Err(Idl4CsharpError::Unevaluated {
                what: "a chain of typedefs deeper than the mapping follows".to_string(),
                at: at.to_string(),
            });
        }
        let node = self
            .ast
            .type_(id)
            .map_err(|_| Idl4CsharpError::Unresolved {
                what: "a type".to_string(),
                at: at.to_string(),
            })?;
        let no_form = |construct: &str| Idl4CsharpError::NoCsharpForm {
            construct: construct.to_string(),
            at: at.to_string(),
        };
        Ok(match &node.kind {
            TypeKind::Primitive(kind) => primitive(*kind),
            TypeKind::Fixed { digits, scale } => CType::of(
                "decimal",
                Shape::Fixed {
                    digits: *digits,
                    scale: *scale,
                },
            ),
            TypeKind::String { width, bound } => CType::of(
                "string",
                Shape::Str {
                    bound: self.bound(*bound, at)?,
                    wide: *width == CharWidth::WChar,
                },
            ),
            TypeKind::Sequence { element, bound } => {
                let element = self.cs_type(element, settings, at)?;
                CType {
                    text: format!("global::Omg.Types.ISequence<{}>", element.text),
                    shape: Shape::Sequence {
                        element: Box::new(element),
                        bound: self.bound(*bound, at)?,
                    },
                    range: None,
                    primitive: None,
                }
            }
            TypeKind::Array { element, dims } => {
                let element = self.cs_type(element, settings, at)?;
                let mut lengths = Vec::new();
                for dim in dims {
                    let length =
                        self.ast
                            .eval_integral(*dim)
                            .map_err(|_| Idl4CsharpError::Unevaluated {
                                what: "an array dimension".to_string(),
                                at: at.to_string(),
                            })?;
                    lengths.push(u64::try_from(length).map_err(|_| {
                        Idl4CsharpError::Unevaluated {
                            what: "an array dimension that is not positive".to_string(),
                            at: at.to_string(),
                        }
                    })?);
                }
                // A multi-dimensional IDL array is a rectangular C# array:
                // "a C# array of the mapped element type".
                let commas = ",".repeat(lengths.len().saturating_sub(1));
                CType {
                    text: format!("{}[{commas}]", element.text),
                    shape: Shape::Array {
                        element: Box::new(element),
                        dims: lengths,
                    },
                    range: None,
                    primitive: None,
                }
            }
            TypeKind::Map { key, value, bound } => {
                let key = self.cs_type(key, settings, at)?;
                let value = self.cs_type(value, settings, at)?;
                CType {
                    text: format!(
                        "global::System.Collections.Generic.IDictionary<{}, {}>",
                        key.text, value.text
                    ),
                    shape: Shape::Map {
                        key: Box::new(key),
                        value: Box::new(value),
                        bound: self.bound(*bound, at)?,
                    },
                    range: None,
                    primitive: None,
                }
            }
            TypeKind::Alias { aliased } => {
                let aliased = aliased
                    .resolved()
                    .ok_or_else(|| Idl4CsharpError::Unresolved {
                        what: format!("the type typedef `{}` names", node.name),
                        at: at.to_string(),
                    })?;
                let mut inner = self.cs_type_of(aliased, settings, at, depth + 1)?;
                if let Some(range) = node.facets.xtypes.as_ref().and_then(|x| x.range) {
                    inner.range = Some(self.merge_range(inner.range.take(), range, at)?);
                }
                inner
            }
            TypeKind::Structure { .. } => {
                let shape = if self.struct_type_of(id) {
                    Shape::Struct
                } else {
                    Shape::Class
                };
                CType::of(&self.qualified(id)?, shape)
            }
            TypeKind::Union { .. } | TypeKind::Valuetype { .. } | TypeKind::ValueBox { .. } => {
                CType::of(&self.qualified(id)?, Shape::Class)
            }
            TypeKind::Bitset { .. } => CType::of(&self.qualified(id)?, Shape::Struct),
            TypeKind::Enumeration { enumerators } => {
                let qualified = self.qualified(id)?;
                let first = self.default_enumerator(id, enumerators, at)?;
                CType::of(
                    &qualified,
                    Shape::Enum {
                        default: format!("{qualified}.{first}"),
                    },
                )
            }
            TypeKind::Bitmask { .. } => CType::of(
                "global::System.Collections.BitArray",
                Shape::BitArray {
                    bound: self.ast.effective_bit_bound(id),
                },
            ),
            TypeKind::Any => CType::of(
                match self.options.platform {
                    Platform::Dds => "global::Omg.Types.Any",
                    Platform::Corba => "global::Corba.Any",
                },
                Shape::Any,
            ),
            TypeKind::ObjectRef { interface } => match interface.as_ref().and_then(Ref::resolved) {
                Some(target) => CType::of(&self.interface_name(target)?, Shape::Reference),
                // `Object` itself: A.1.3 maps it to `Corba.Object` and
                // `Corba.IObject`; outside the CORBA platform there is
                // nothing it could be.
                None => match self.options.platform {
                    Platform::Corba => CType::of("global::Corba.IObject", Shape::Reference),
                    Platform::Dds => return Err(no_form("Object outside the CORBA platform")),
                },
            },
            // OMG IDL 4.2 §7.4.7.4.6 leaves the form of ValueBase to each
            // mapping and IDL4 to C# names none; every value type is an
            // object, which is what a reference to any of them is.
            TypeKind::ValueBase => CType::of("object", Shape::Reference),
            // §7.2.4.5: "compliant implementations may provide the
            // necessary mechanisms to map native types". The natives of the
            // CORBA module are `object` on the CORBA platform; others have
            // no form.
            TypeKind::Native if self.in_corba_module(node.module) => {
                CType::of("object", Shape::Reference)
            }
            TypeKind::Native => return Err(no_form("a native type")),
            TypeKind::AnnotationType { .. } => return Err(no_form("an annotation type as data")),
        })
    }

    fn bound(&self, bound: Bound, at: &str) -> Result<Option<u64>, Idl4CsharpError> {
        match bound {
            Bound::Unbounded => Ok(None),
            Bound::Const(expr) => {
                let value =
                    self.ast
                        .eval_integral(expr)
                        .map_err(|_| Idl4CsharpError::Unevaluated {
                            what: "a bound".to_string(),
                            at: at.to_string(),
                        })?;
                u64::try_from(value)
                    .map(Some)
                    .map_err(|_| Idl4CsharpError::Unevaluated {
                        what: "a bound that is not positive".to_string(),
                        at: at.to_string(),
                    })
            }
        }
    }

    /// The enumerator a default-constructed member of an enumeration
    /// holds: `@default_literal`, or the first.
    fn default_enumerator(
        &self,
        id: Id<TypeNode>,
        enumerators: &[Id<zerodds_ast::EnumeratorNode>],
        at: &str,
    ) -> Result<String, Idl4CsharpError> {
        let settings = self.settings_of_type(id);
        let chosen = enumerators
            .iter()
            .find(|e| {
                self.ast.enumerator(**e).is_ok_and(|node| {
                    node.facets
                        .xtypes
                        .as_ref()
                        .is_some_and(|x| x.is_default_literal)
                })
            })
            .or_else(|| enumerators.first())
            .ok_or_else(|| Idl4CsharpError::Unresolved {
                what: "an enumeration without enumerators".to_string(),
                at: at.to_string(),
            })?;
        let node = self
            .ast
            .enumerator(*chosen)
            .map_err(|_| Idl4CsharpError::Unresolved {
                what: "an enumerator".to_string(),
                at: at.to_string(),
            })?;
        Ok(settings.naming.member(&node.name))
    }

    /// The bounds a typedef or member annotates over what the typedefs
    /// under it annotated: a bound set here replaces the one below.
    pub(super) fn merge_range(
        &self,
        under: Option<Range>,
        constraint: RangeConstraint,
        at: &str,
    ) -> Result<Range, Idl4CsharpError> {
        let eval = |expr| {
            self.ast
                .eval_const(expr)
                .map_err(|_| Idl4CsharpError::Unevaluated {
                    what: "the value of @range, @min or @max".to_string(),
                    at: at.to_string(),
                })
        };
        let mut out = under.unwrap_or_default();
        if let Some(min) = constraint.min {
            out.min = Some(eval(min)?);
        }
        if let Some(max) = constraint.max {
            out.max = Some(eval(max)?);
        }
        Ok(out)
    }
}

/// The C# type of an IDL primitive, Tables 7.2 and 7.3.
fn primitive(kind: PrimitiveKind) -> CType {
    use PrimitiveKind as P;
    let mut out = match kind {
        P::Boolean => CType::of("bool", Shape::Bool),
        P::Char => CType::of("char", Shape::Char { wide: false }),
        P::WChar => CType::of("char", Shape::Char { wide: true }),
        P::Float32 => CType::of("float", Shape::Float),
        P::Float64 => CType::of("double", Shape::Double),
        P::Float128 => CType::of("decimal", Shape::LongDouble),
        _ => integer(kind),
    };
    out.primitive = Some(kind);
    out
}
