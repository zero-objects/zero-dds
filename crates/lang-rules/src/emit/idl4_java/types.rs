// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Java type of an IDL type, §7.2.4 and §7.14.3, with what a
//! modifier has to check about a value of it.

use super::{Gen, Idl4JavaError, Platform, Settings};
use zerodds_ast::{
    Bound, CharWidth, ConstValue, PrimitiveKind, RangeConstraint, TypeKind, TypeNode,
};
use zerodds_ast_core::{Id, Ref};

/// What a type is in Java, beyond its spelling.
#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    /// `boolean`.
    Boolean,
    /// `char`, narrow (an IDL `char`, 8 bits) or wide (`wchar`).
    Char,
    /// `byte`, `short`, `int` or `long`: `bits` the Java width, `unsigned`
    /// whether it holds an IDL unsigned type as its bit pattern.
    Integer {
        /// 8, 16, 32 or 64.
        bits: u8,
        /// Whether the IDL type is unsigned and the Java type of the same
        /// width (Table 7.2, `promote_integer_width` FALSE).
        unsigned: bool,
    },
    /// `float`.
    Float,
    /// `double`.
    Double,
    /// `java.math.BigInteger`: an `unsigned long long` with
    /// `promote_integer_width` TRUE (Table 8.2).
    BigInteger,
    /// `java.math.BigDecimal` for `long double` (Table 7.3).
    LongDouble,
    /// `java.math.BigDecimal` for `fixed<digits, scale>` (§7.2.4.2.4).
    Fixed {
        /// Total digits.
        digits: u8,
        /// Digits after the point.
        scale: u8,
    },
    /// `string` or `wstring` (§7.2.4.2.2, §7.2.4.2.3).
    Str {
        /// The bound, if any.
        bound: Option<u64>,
        /// Whether it is a `wstring`.
        wide: bool,
    },
    /// `sequence<T>` as a type-specific interface of Table 7.4 or as
    /// `java.util.List<E>` (§7.2.4.2.1).
    Sequence {
        /// The element.
        element: Box<JType>,
        /// The bound, if any.
        bound: Option<u64>,
        /// The class of Table 7.4, for a sequence of a basic type.
        specific: Option<&'static str>,
    },
    /// An array (§7.2.4.4): the element and every dimension.
    Array {
        /// The element.
        element: Box<JType>,
        /// The dimensions, outermost first.
        dims: Vec<u64>,
    },
    /// `map<K, V>` (§7.14.3.1).
    Map {
        /// The key.
        key: Box<JType>,
        /// The value.
        value: Box<JType>,
        /// The bound, if any.
        bound: Option<u64>,
    },
    /// A class with a public no-argument constructor: a structure, a
    /// union, a bit set, a value type.
    Class,
    /// A Java `enum` (§7.2.4.3.3); the constant a default constructor
    /// starts a member at.
    Enum {
        /// The qualified constant.
        default: String,
    },
    /// `java.util.BitSet` for a bit mask (§7.14.3.3).
    BitSet {
        /// `@bit_bound`.
        bound: u16,
    },
    /// The `any` of §7.3 or A.1.4.
    Any,
    /// An object reference: an interface, `Object`, `ValueBase`.
    Reference,
}

/// A Java type: its spelling, its boxed spelling and its shape.
#[derive(Debug, Clone, PartialEq)]
pub struct JType {
    /// The Java spelling, qualified.
    pub text: String,
    /// The spelling as a type argument: the box of a primitive.
    pub boxed: String,
    /// What it is.
    pub shape: Shape,
    /// `@range`, `@min`, `@max` collected through the typedefs it was
    /// reached by (§7.2.4.6: "Annotations on an IDL typedef shall be
    /// applied to uses of the typedef").
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

impl JType {
    fn plain(text: &str, boxed: &str, shape: Shape) -> Self {
        Self {
            text: text.to_string(),
            boxed: boxed.to_string(),
            shape,
            range: None,
            primitive: None,
        }
    }

    fn same(text: String, shape: Shape) -> Self {
        Self {
            boxed: text.clone(),
            text,
            shape,
            range: None,
            primitive: None,
        }
    }

    /// Whether the Java type is a primitive.
    #[must_use]
    pub fn is_primitive(&self) -> bool {
        matches!(
            self.shape,
            Shape::Boolean | Shape::Char | Shape::Integer { .. } | Shape::Float | Shape::Double
        )
    }

    /// The type as an `@optional` or `@external` member holds it: "Replaces
    /// type with boxed type, for Basic Types. No impact on other types."
    /// (Table 7.6, Table 7.9).
    #[must_use]
    pub fn boxed_type(&self) -> Self {
        Self {
            text: self.boxed.clone(),
            ..self.clone()
        }
    }
}

/// The Java primitive of an IDL integer type, Table 7.2 and Table 8.2.
fn integer(kind: PrimitiveKind, promote: bool) -> JType {
    use PrimitiveKind as P;
    let (bits, unsigned) = match kind {
        P::Int8 => (8, false),
        P::Octet | P::UInt8 => {
            if promote {
                (16, false)
            } else {
                (8, true)
            }
        }
        P::Int16 => (16, false),
        P::UInt16 => {
            if promote {
                (32, false)
            } else {
                (16, true)
            }
        }
        P::Int32 => (32, false),
        P::UInt32 => {
            if promote {
                (64, false)
            } else {
                (32, true)
            }
        }
        P::Int64 => (64, false),
        _ => (64, true),
    };
    let (text, boxed) = match bits {
        8 => ("byte", "Byte"),
        16 => ("short", "Short"),
        32 => ("int", "Integer"),
        _ => ("long", "Long"),
    };
    // `octet` is `byte` in both columns of Table 8.2 except `uint8`: the
    // table lists `uint8` as `short` under promotion, and `octet` not at
    // all, so `octet` stays the `byte` of §7.2.4.1.6.
    let (bits, unsigned, text, boxed) = if kind == P::Octet {
        (8, true, "byte", "Byte")
    } else {
        (bits, unsigned, text, boxed)
    };
    JType {
        primitive: Some(kind),
        ..JType::plain(text, boxed, Shape::Integer { bits, unsigned })
    }
}

/// The class of Table 7.4 a sequence of `element` maps to, if the
/// element is a basic type.
fn specific_sequence(element: &JType) -> Option<&'static str> {
    match &element.shape {
        Shape::Boolean => Some("BooleanSeq"),
        Shape::Char => Some("CharSeq"),
        Shape::Integer { bits: 8, .. } => Some("ByteSeq"),
        Shape::Integer { bits: 16, .. } => Some("ShortSeq"),
        Shape::Integer { bits: 32, .. } => Some("IntegerSeq"),
        Shape::Integer { .. } => Some("LongSeq"),
        Shape::Float => Some("FloatSeq"),
        Shape::Double => Some("DoubleSeq"),
        Shape::LongDouble => Some("BigDecimalSeq"),
        _ => None,
    }
}

/// The boxed type of Table 7.5 a map key or value takes: the table boxes
/// the unsigned types one width up, whatever `promote_integer_width`
/// says.
fn map_boxed(kind: Option<PrimitiveKind>, of: &JType) -> String {
    use PrimitiveKind as P;
    match kind {
        Some(P::UInt16) => "Integer".to_string(),
        Some(P::UInt32) => "Long".to_string(),
        Some(P::UInt64) => "java.math.BigInteger".to_string(),
        _ => of.boxed.clone(),
    }
}

impl Gen<'_> {
    /// The Java type of the type `reference` names, with what the
    /// typedefs on the way annotate.
    pub(super) fn java_type(
        &self,
        reference: &Ref<TypeNode>,
        settings: &Settings,
        at: &str,
    ) -> Result<JType, Idl4JavaError> {
        let id = reference
            .resolved()
            .ok_or_else(|| Idl4JavaError::Unresolved {
                what: "a type".to_string(),
                at: at.to_string(),
            })?;
        self.java_type_of(id, settings, at, 0)
    }

    pub(super) fn java_type_of(
        &self,
        id: Id<TypeNode>,
        settings: &Settings,
        at: &str,
        depth: usize,
    ) -> Result<JType, Idl4JavaError> {
        if depth > super::MAX_DEPTH {
            return Err(Idl4JavaError::Unevaluated {
                what: "a chain of typedefs deeper than the mapping follows".to_string(),
                at: at.to_string(),
            });
        }
        let node = self.ast.type_(id).map_err(|_| Idl4JavaError::Unresolved {
            what: "a type".to_string(),
            at: at.to_string(),
        })?;
        let no_form = |construct: &str| Idl4JavaError::NoJavaForm {
            construct: construct.to_string(),
            at: at.to_string(),
        };
        Ok(match &node.kind {
            TypeKind::Primitive(kind) => self.primitive(*kind, settings),
            TypeKind::Fixed { digits, scale } => JType::same(
                "java.math.BigDecimal".to_string(),
                Shape::Fixed {
                    digits: *digits,
                    scale: *scale,
                },
            ),
            TypeKind::String { width, bound } => {
                let wide = *width == CharWidth::WChar;
                JType::same(
                    settings.string_type.clone(),
                    Shape::Str {
                        bound: self.bound(*bound, at)?,
                        wide,
                    },
                )
            }
            TypeKind::Sequence { element, bound } => {
                let element = self.java_type(element, settings, at)?;
                let bound = self.bound(*bound, at)?;
                let specific = specific_sequence(&element);
                let text = match specific {
                    Some(class) => format!("org.omg.type.{class}"),
                    None => format!("java.util.List<{}>", element.boxed),
                };
                JType::same(
                    text,
                    Shape::Sequence {
                        element: Box::new(element),
                        bound,
                        specific,
                    },
                )
            }
            TypeKind::Array { element, dims } => {
                let element = self.java_type(element, settings, at)?;
                let mut lengths = Vec::new();
                for dim in dims {
                    let length =
                        self.ast
                            .eval_integral(*dim)
                            .map_err(|_| Idl4JavaError::Unevaluated {
                                what: "an array dimension".to_string(),
                                at: at.to_string(),
                            })?;
                    lengths.push(u64::try_from(length).map_err(|_| {
                        Idl4JavaError::Unevaluated {
                            what: "an array dimension that is not positive".to_string(),
                            at: at.to_string(),
                        }
                    })?);
                }
                let text = format!("{}{}", element.text, "[]".repeat(lengths.len()));
                JType::same(
                    text,
                    Shape::Array {
                        element: Box::new(element),
                        dims: lengths,
                    },
                )
            }
            TypeKind::Map { key, value, bound } => {
                let key_type = self.java_type(key, settings, at)?;
                let value_type = self.java_type(value, settings, at)?;
                let key_boxed = map_boxed(key_type.primitive, &key_type);
                let value_boxed = map_boxed(value_type.primitive, &value_type);
                JType::same(
                    format!("java.util.Map<{key_boxed}, {value_boxed}>"),
                    Shape::Map {
                        key: Box::new(key_type),
                        value: Box::new(value_type),
                        bound: self.bound(*bound, at)?,
                    },
                )
            }
            TypeKind::Alias { aliased } => {
                let aliased = aliased
                    .resolved()
                    .ok_or_else(|| Idl4JavaError::Unresolved {
                        what: format!("the type typedef `{}` names", node.name),
                        at: at.to_string(),
                    })?;
                let mut inner = self.java_type_of(aliased, settings, at, depth + 1)?;
                if let Some(range) = node.facets.xtypes.as_ref().and_then(|x| x.range) {
                    inner.range = Some(self.merge_range(inner.range.take(), range, at)?);
                }
                inner
            }
            TypeKind::Structure { .. }
            | TypeKind::Union { .. }
            | TypeKind::Bitset { .. }
            | TypeKind::Valuetype { .. } => JType::same(self.qualified(id)?, Shape::Class),
            TypeKind::Enumeration { enumerators } => {
                let qualified = self.qualified(id)?;
                let first = self.default_enumerator(id, enumerators, at)?;
                JType::same(
                    qualified.clone(),
                    Shape::Enum {
                        default: format!("{qualified}.{first}"),
                    },
                )
            }
            TypeKind::Bitmask { .. } => JType::same(
                "java.util.BitSet".to_string(),
                Shape::BitSet {
                    bound: self.ast.effective_bit_bound(id),
                },
            ),
            TypeKind::Any => {
                let text = match self.options.platform {
                    Platform::Dds => "org.omg.type.Any",
                    Platform::Corba => "org.omg.CORBA.Any",
                };
                JType::same(text.to_string(), Shape::Any)
            }
            TypeKind::ObjectRef { interface } => match interface.as_ref().and_then(Ref::resolved) {
                Some(target) => JType::same(self.interface_name(target)?, Shape::Reference),
                // `Object` itself: A.1.3 maps the CORBA Object interface to
                // `org.omg.CORBA.Object`; outside a CORBA platform there is
                // nothing it could be.
                None => match self.options.platform {
                    Platform::Corba => {
                        JType::same("org.omg.CORBA.Object".to_string(), Shape::Reference)
                    }
                    Platform::Dds => return Err(no_form("Object outside the CORBA platform")),
                },
            },
            // OMG IDL 4.2 §7.4.7.4.6 leaves the form of ValueBase to each
            // mapping and IDL4 to Java names none. Every mapped value type
            // is `java.io.Serializable` — its state is — which is the base
            // the CORBA mapping to Java gives ValueBase (formal/2008-01-11
            // §1.13.1).
            TypeKind::ValueBase => {
                JType::same("java.io.Serializable".to_string(), Shape::Reference)
            }
            // A boxed value is a value type of one public member `value`
            // (A.1.6, §7.6).
            TypeKind::ValueBox { .. } => JType::same(self.qualified(id)?, Shape::Class),
            // §7.2.4.5: "This language mapping specification does not
            // define any native types." The CORBA module declares three —
            // `AbstractBase` (CORBA 3.3 part 1 §8.2, the base of every
            // abstract interface), and in its pseudo IDL `OpaqueValue`
            // ("Each language mapping specifies what OpaqueValue maps
            // to", §11.2) and `ValueFactory` — which Annex A.1 does not
            // name either. On the CORBA platform they are
            // `java.lang.Object`, the type every Java representation of
            // them is.
            TypeKind::Native if self.in_corba_module(node.module) => {
                JType::same("java.lang.Object".to_string(), Shape::Reference)
            }
            TypeKind::Native => return Err(no_form("a native type")),
            TypeKind::AnnotationType { .. } => return Err(no_form("an annotation type as data")),
        })
    }

    /// The Java type of an IDL primitive, Tables 7.2, 7.3, 8.2.
    pub(super) fn primitive(&self, kind: PrimitiveKind, settings: &Settings) -> JType {
        use PrimitiveKind as P;
        let mut out = match kind {
            P::Boolean => JType::plain("boolean", "Boolean", Shape::Boolean),
            P::Char | P::WChar => JType::plain("char", "Character", Shape::Char),
            P::Float32 => JType::plain("float", "Float", Shape::Float),
            P::Float64 => JType::plain("double", "Double", Shape::Double),
            P::Float128 => JType::plain(
                "java.math.BigDecimal",
                "java.math.BigDecimal",
                Shape::LongDouble,
            ),
            P::UInt64 if settings.promote => JType::plain(
                "java.math.BigInteger",
                "java.math.BigInteger",
                Shape::BigInteger,
            ),
            _ => integer(kind, settings.promote),
        };
        out.primitive = Some(kind);
        out
    }

    fn bound(&self, bound: Bound, at: &str) -> Result<Option<u64>, Idl4JavaError> {
        match bound {
            Bound::Unbounded => Ok(None),
            Bound::Const(expr) => {
                let value =
                    self.ast
                        .eval_integral(expr)
                        .map_err(|_| Idl4JavaError::Unevaluated {
                            what: "a bound".to_string(),
                            at: at.to_string(),
                        })?;
                u64::try_from(value)
                    .map(Some)
                    .map_err(|_| Idl4JavaError::Unevaluated {
                        what: "a bound that is not positive".to_string(),
                        at: at.to_string(),
                    })
            }
        }
    }

    /// `@range`, `@min` and `@max` of one annotation site on top of the
    /// ones already collected: the narrower end wins.
    pub(super) fn merge_range(
        &self,
        outer: Option<Range>,
        range: RangeConstraint,
        at: &str,
    ) -> Result<Range, Idl4JavaError> {
        let eval = |expr| {
            self.ast
                .eval_const(expr)
                .map_err(|_| Idl4JavaError::Unevaluated {
                    what: "a value of @range, @min or @max".to_string(),
                    at: at.to_string(),
                })
        };
        let mut out = outer.unwrap_or_default();
        if let Some(min) = range.min {
            out.min = Some(eval(min)?);
        }
        if let Some(max) = range.max {
            out.max = Some(eval(max)?);
        }
        Ok(out)
    }

    /// The enumerator a member of the enumeration starts at: the one
    /// `@default_literal` marks, the first otherwise (OMG IDL 4.2
    /// §8.3.2.3).
    fn default_enumerator(
        &self,
        id: Id<TypeNode>,
        enumerators: &[Id<zerodds_ast::EnumeratorNode>],
        at: &str,
    ) -> Result<String, Idl4JavaError> {
        let settings = self.settings_of_type(id);
        let mut chosen = None;
        for enumerator in enumerators {
            let node = self
                .ast
                .enumerator(*enumerator)
                .map_err(|_| Idl4JavaError::Unresolved {
                    what: "an enumerator".to_string(),
                    at: at.to_string(),
                })?;
            if chosen.is_none() {
                chosen = Some(node.name.clone());
            }
            if node
                .facets
                .xtypes
                .as_ref()
                .is_some_and(|x| x.is_default_literal)
            {
                chosen = Some(node.name.clone());
                break;
            }
        }
        let name = chosen.ok_or_else(|| Idl4JavaError::NoJavaForm {
            construct: "an enumeration without an enumerator".to_string(),
            at: at.to_string(),
        })?;
        Ok(settings.naming.enumerator(&name))
    }
}
