// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The C++ type of an IDL type: §7.2.4, §7.3, §7.14.3 and §7.14.4 to
//! §7.14.5, and Annex C.2.1.2 for C++03.

use super::{Gen, Idl4CppError, Platform, Standard};
use zerodds_ast::{
    Bound, CharWidth, ConstValue, PrimitiveKind, RangeConstraint, TypeKind, TypeNode,
};
use zerodds_ast_core::{Id, Ref};

/// What a type is in C++, beyond its spelling: what decides how it is
/// passed (§7.4: "by value if the resulting type after resolving any
/// typedef C++ is a built-in type or an enum, and as a reference to const
/// otherwise") and how a member of it starts out (§7.2.4.3.1).
#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    /// `bool`.
    Bool,
    /// `char` or `wchar_t`.
    Char {
        /// Whether it is a `wchar`.
        wide: bool,
    },
    /// `int8_t` to `uint64_t`, Tables 7.2, 7.13 and 7.14.
    Integer {
        /// 8, 16, 32 or 64.
        bits: u8,
        /// Whether the C++ type is signed.
        signed: bool,
    },
    /// `float`, `double` or `long double` (Table 7.3).
    Floating {
        /// 32, 64 or 128 for `long double`.
        bits: u8,
    },
    /// `omg::types::fixed_n<digits, scale>` (§7.2.4.2.4).
    Fixed,
    /// A string type (§7.2.4.2.2, §7.2.4.2.3).
    Str,
    /// A sequence (§7.2.4.2.1).
    Sequence,
    /// An array (§7.2.4.4).
    Array,
    /// A map (§7.14.3.1).
    Map,
    /// A class the mapping declares: a structure, a union, a bit set, a
    /// bit mask.
    Class,
    /// An `enum class` (an `enum` under Annex C); the enumerator a member
    /// of it starts at.
    Enum {
        /// The qualified enumerator.
        default: String,
    },
    /// `omg::types::Any` or `CORBA::Any` (§7.3).
    Any,
    /// An interface or value type, held through `omg::types::ref_type<T>`
    /// (`T*` under Annex C.2.2).
    Reference,
}

/// A C++ type: its spelling and its shape.
#[derive(Debug, Clone, PartialEq)]
pub struct CppType {
    /// The C++ spelling, qualified from `::`.
    pub text: String,
    /// What follows a declarator under Annex C.2.1.2.3: the dimensions of
    /// an anonymous C array (`[3][4]`); empty otherwise.
    pub suffix: String,
    /// What it is, after resolving every typedef.
    pub shape: Shape,
    /// `@range`, `@min`, `@max` collected through the typedefs it was
    /// reached by.
    pub range: Option<Range>,
    /// The IDL kind of a primitive, for its literals.
    pub primitive: Option<PrimitiveKind>,
    /// The type behind the typedefs, when it is a declared type.
    pub declared: Option<Id<TypeNode>>,
}

/// The bounds `@range`, `@min` and `@max` set, as evaluated values.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Range {
    /// The least value admitted.
    pub min: Option<ConstValue>,
    /// The greatest value admitted.
    pub max: Option<ConstValue>,
}

impl CppType {
    fn of(text: &str, shape: Shape) -> Self {
        Self {
            text: text.to_string(),
            suffix: String::new(),
            shape,
            range: None,
            primitive: None,
            declared: None,
        }
    }

    /// Whether it is passed by value: "a built-in type or an enum".
    #[must_use]
    pub fn by_value(&self) -> bool {
        matches!(
            self.shape,
            Shape::Bool
                | Shape::Char { .. }
                | Shape::Integer { .. }
                | Shape::Floating { .. }
                | Shape::Enum { .. }
        )
    }

    /// The parameter type of an `in` argument (§7.4).
    #[must_use]
    pub fn in_param(&self) -> String {
        if self.by_value() {
            self.text.clone()
        } else {
            format!("const {}&", self.text)
        }
    }
}

/// `<cstdint>` names of the integer types; `omg::types::` ones under
/// Annex C (Tables C.1 to C.3).
fn integer_text(kind: PrimitiveKind, standard: Standard) -> (String, u8, bool) {
    use PrimitiveKind as P;
    let (name, bits, signed) = match kind {
        P::Int8 => ("int8_t", 8, true),
        P::Octet | P::UInt8 => ("uint8_t", 8, false),
        P::Int16 => ("int16_t", 16, true),
        P::UInt16 => ("uint16_t", 16, false),
        P::Int32 => ("int32_t", 32, true),
        P::UInt32 => ("uint32_t", 32, false),
        P::Int64 => ("int64_t", 64, true),
        _ => ("uint64_t", 64, false),
    };
    let text = match standard {
        Standard::Cxx11 => format!("std::{name}"),
        Standard::Cxx03 => format!("::omg::types::{name}"),
    };
    (text, bits, signed)
}

impl Gen<'_> {
    /// The C++ type of the type `reference` names, under the name it is
    /// referred to by (a typedef's own name, §7.2.4.6).
    pub(super) fn cpp_type(
        &self,
        reference: &Ref<TypeNode>,
        at: &str,
    ) -> Result<CppType, Idl4CppError> {
        let id = reference
            .resolved()
            .ok_or_else(|| Idl4CppError::Unresolved {
                what: "a type".to_string(),
                at: at.to_string(),
            })?;
        self.cpp_type_of(id, at, 0)
    }

    pub(super) fn cpp_type_of(
        &self,
        id: Id<TypeNode>,
        at: &str,
        depth: usize,
    ) -> Result<CppType, Idl4CppError> {
        if depth > super::MAX_DEPTH {
            return Err(Idl4CppError::Unevaluated {
                what: "a chain of typedefs deeper than the mapping follows".to_string(),
                at: at.to_string(),
            });
        }
        let node = self.ast.type_(id).map_err(|_| Idl4CppError::Unresolved {
            what: "a type".to_string(),
            at: at.to_string(),
        })?;
        let no_form = |construct: &str| Idl4CppError::NoCppForm {
            construct: construct.to_string(),
            at: at.to_string(),
        };
        let standard = self.options.standard;
        let mut out = match &node.kind {
            TypeKind::Primitive(kind) => self.primitive(*kind),
            TypeKind::Fixed { digits, scale } => CppType::of(
                &format!("::omg::types::fixed_n<{digits}, {scale}>"),
                Shape::Fixed,
            ),
            TypeKind::String { width, bound } => {
                let wide = *width == CharWidth::WChar;
                let text = match (self.bound(*bound, at)?, wide) {
                    (None, false) => "std::string".to_string(),
                    (None, true) => "std::wstring".to_string(),
                    (Some(n), false) => format!("::omg::types::bounded_string<{n}>"),
                    (Some(n), true) => format!("::omg::types::bounded_wstring<{n}>"),
                };
                CppType::of(&text, Shape::Str)
            }
            TypeKind::Sequence { element, bound } => {
                let element = self.cpp_type(element, at)?;
                let element_text = self.element_text(&element, at)?;
                let text = match self.bound(*bound, at)? {
                    None => match standard {
                        Standard::Cxx11 => format!("::omg::types::sequence<{element_text}>"),
                        Standard::Cxx03 => format!("std::vector<{} >", element_text),
                    },
                    Some(n) => format!("::omg::types::bounded_sequence<{element_text}, {n}>"),
                };
                CppType::of(&text, Shape::Sequence)
            }
            TypeKind::Array { element, dims } => {
                let element = self.cpp_type(element, at)?;
                let element_text = self.element_text(&element, at)?;
                let mut lengths = Vec::new();
                for dim in dims {
                    let length =
                        self.ast
                            .eval_integral(*dim)
                            .map_err(|_| Idl4CppError::Unevaluated {
                                what: "an array dimension".to_string(),
                                at: at.to_string(),
                            })?;
                    lengths.push(
                        u64::try_from(length).map_err(|_| Idl4CppError::Unevaluated {
                            what: "an array dimension that is not positive".to_string(),
                            at: at.to_string(),
                        })?,
                    );
                }
                match standard {
                    // "Multidimensional arrays shall be constructed nesting
                    // std::array or omg::types::array definitions."
                    Standard::Cxx11 => {
                        let mut text = element_text;
                        for length in lengths.iter().rev() {
                            text = format!("::omg::types::array<{text}, {length}>");
                        }
                        CppType::of(&text, Shape::Array)
                    }
                    // Annex C.2.1.2.3: "C-style arrays of the mapped element
                    // type".
                    Standard::Cxx03 => {
                        let mut out = CppType::of(&element_text, Shape::Array);
                        out.suffix = lengths.iter().map(|n| format!("[{n}]")).collect();
                        out
                    }
                }
            }
            TypeKind::Map { key, value, bound } => {
                let key = self.cpp_type(key, at)?;
                let value = self.cpp_type(value, at)?;
                let key_text = self.element_text(&key, at)?;
                let value_text = self.element_text(&value, at)?;
                let text = match (self.bound(*bound, at)?, standard) {
                    (None, Standard::Cxx11) => {
                        format!("::omg::types::map<{key_text}, {value_text}>")
                    }
                    (None, Standard::Cxx03) => format!("std::map<{key_text}, {value_text} >"),
                    (Some(n), _) => {
                        format!("::omg::types::bounded_map<{key_text}, {value_text}, {n}>")
                    }
                };
                CppType::of(&text, Shape::Map)
            }
            TypeKind::Alias { aliased } => {
                let aliased = aliased.resolved().ok_or_else(|| Idl4CppError::Unresolved {
                    what: format!("the type typedef `{}` names", node.name),
                    at: at.to_string(),
                })?;
                let mut inner = self.cpp_type_of(aliased, at, depth + 1)?;
                if let Some(range) = node.facets.xtypes.as_ref().and_then(|x| x.range) {
                    inner.range = Some(self.merge_range(inner.range.take(), range, at)?);
                }
                // §7.2.4.6: the typedef is a type alias, and a use of it
                // names the alias. A C array typedef carries its dimensions
                // in the name.
                inner.text = self.qualified(id)?;
                inner.suffix = String::new();
                inner
            }
            TypeKind::Structure { .. }
            | TypeKind::Union { .. }
            | TypeKind::Bitset { .. }
            | TypeKind::Bitmask { .. } => {
                let mut out = CppType::of(&self.qualified(id)?, Shape::Class);
                out.declared = Some(id);
                out
            }
            TypeKind::Valuetype { .. } | TypeKind::ValueBox { .. } => {
                let mut out = self.reference(&self.qualified(id)?);
                out.declared = Some(id);
                out
            }
            TypeKind::Enumeration { enumerators } => {
                let qualified = self.qualified(id)?;
                let first = self.default_enumerator(id, enumerators, at)?;
                let mut out = CppType::of(&qualified, Shape::Enum { default: first });
                out.declared = Some(id);
                out
            }
            TypeKind::Any => CppType::of(
                match self.options.platform {
                    Platform::Dds => "::omg::types::Any",
                    Platform::Corba => "::CORBA::Any",
                },
                Shape::Any,
            ),
            TypeKind::ObjectRef { interface } => match interface.as_ref().and_then(Ref::resolved) {
                // Table A.13 names `omg::types::ref_type<T>` for the CORBA
                // platform as well.
                Some(target) => self.reference(&self.interface_name(target)?),
                None => match self.options.platform {
                    Platform::Corba => self.reference("::CORBA::Object"),
                    Platform::Dds => return Err(no_form("Object outside the CORBA platform")),
                },
            },
            TypeKind::ValueBase => match self.options.platform {
                Platform::Corba => self.reference("::CORBA::ValueBase"),
                Platform::Dds => return Err(no_form("ValueBase outside the CORBA platform")),
            },
            // §7.2.4.5: "This language mapping specification does not
            // define any native types, but compliant implementations may
            // provide the necessary mechanisms". The natives of the CORBA
            // module are the runtime's classes of that name.
            TypeKind::Native if self.in_corba_module(node.module) => match node.name.as_str() {
                // A.1.8.9.1: "The C++ mapping for the IDL
                // CORBA::ValueFactory native type is an object reference
                // to the ValueFactoryBase class".
                "ValueFactory" => self.reference("::CORBA::ValueFactoryBase"),
                // CORBA 3.3 part 1 §11.2: "Each language mapping specifies
                // what OpaqueValue maps to"; IDL4 to C++ 1.0 does not, and
                // omg/corba.hpp declares it as the C++ mapping of CORBA
                // does, `void*`.
                "OpaqueValue" => CppType::of("::CORBA::OpaqueValue", Shape::Class),
                name => self.reference(&format!("::CORBA::{name}")),
            },
            TypeKind::Native => return Err(no_form("a native type")),
            TypeKind::AnnotationType { .. } => return Err(no_form("an annotation type as data")),
        };
        if out.primitive.is_none()
            && let TypeKind::Primitive(kind) = node.kind
        {
            out.primitive = Some(kind);
        }
        Ok(out)
    }

    /// An interface or value type held as a reference (§7.4), `T*` under
    /// Annex C.2.2.
    pub(super) fn reference(&self, name: &str) -> CppType {
        match self.options.standard {
            Standard::Cxx11 => {
                CppType::of(&format!("::omg::types::ref_type<{name}>"), Shape::Reference)
            }
            Standard::Cxx03 => CppType::of(&format!("{name}*"), Shape::Reference),
        }
    }

    /// The text of a type as a template argument: an anonymous C array has
    /// none under Annex C.
    fn element_text(&self, element: &CppType, at: &str) -> Result<String, Idl4CppError> {
        if element.suffix.is_empty() {
            Ok(element.text.clone())
        } else {
            Err(Idl4CppError::NoCppForm {
                construct: "an anonymous array as an element under Annex C (a C array is no \
                            template argument)"
                    .to_string(),
                at: at.to_string(),
            })
        }
    }

    /// The C++ type of an IDL primitive, Tables 7.2, 7.3, 7.13 and 7.14.
    fn primitive(&self, kind: PrimitiveKind) -> CppType {
        use PrimitiveKind as P;
        let mut out = match kind {
            P::Boolean => CppType::of("bool", Shape::Bool),
            P::Char => CppType::of("char", Shape::Char { wide: false }),
            P::WChar => CppType::of("wchar_t", Shape::Char { wide: true }),
            P::Float32 => CppType::of("float", Shape::Floating { bits: 32 }),
            P::Float64 => CppType::of("double", Shape::Floating { bits: 64 }),
            P::Float128 => CppType::of("long double", Shape::Floating { bits: 128 }),
            _ => {
                let (text, bits, signed) = integer_text(kind, self.options.standard);
                CppType::of(&text, Shape::Integer { bits, signed })
            }
        };
        out.primitive = Some(kind);
        out
    }

    pub(super) fn bound(&self, bound: Bound, at: &str) -> Result<Option<u64>, Idl4CppError> {
        match bound {
            Bound::Unbounded => Ok(None),
            Bound::Const(expr) => {
                let value =
                    self.ast
                        .eval_integral(expr)
                        .map_err(|_| Idl4CppError::Unevaluated {
                            what: "a bound".to_string(),
                            at: at.to_string(),
                        })?;
                u64::try_from(value)
                    .map(Some)
                    .map_err(|_| Idl4CppError::Unevaluated {
                        what: "a bound that is not positive".to_string(),
                        at: at.to_string(),
                    })
            }
        }
    }

    /// The enumerator a member of an enumeration starts at: the one
    /// `@default_literal` names, or the first (§7.2.4.3.1: "enumerators to
    /// their first value"; Table 7.16).
    fn default_enumerator(
        &self,
        id: Id<TypeNode>,
        enumerators: &[Id<zerodds_ast::EnumeratorNode>],
        at: &str,
    ) -> Result<String, Idl4CppError> {
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
            .ok_or_else(|| Idl4CppError::Unresolved {
                what: "an enumeration without enumerators".to_string(),
                at: at.to_string(),
            })?;
        self.enumerator_text(id, *chosen)
    }

    /// The qualified C++ name of an enumerator: `E::name` for an `enum
    /// class`; the enumerator in the enclosing scope, with Annex C.3.1's
    /// prefix and suffix, for a C++03 `enum`.
    pub(super) fn enumerator_text(
        &self,
        enumeration: Id<TypeNode>,
        enumerator: Id<zerodds_ast::EnumeratorNode>,
    ) -> Result<String, Idl4CppError> {
        let node = self
            .ast
            .enumerator(enumerator)
            .map_err(|_| super::unresolved("an enumerator"))?;
        let qualified = self.qualified(enumeration)?;
        match self.options.standard {
            Standard::Cxx11 => Ok(format!("{qualified}::{}", super::names::name(&node.name))),
            Standard::Cxx03 => {
                let scope = qualified
                    .rsplit_once("::")
                    .map_or_else(String::new, |(scope, _)| scope.to_string());
                Ok(format!(
                    "{scope}::{}",
                    self.enumerator_name(enumeration, &node.name)
                ))
            }
        }
    }

    /// The C++ name of an enumerator in its declaration.
    pub(super) fn enumerator_name(&self, enumeration: Id<TypeNode>, idl: &str) -> String {
        match self.options.standard {
            Standard::Cxx11 => super::names::name(idl),
            Standard::Cxx03 => {
                let settings = self.settings_of_type(enumeration);
                super::names::name(&format!(
                    "{}{idl}{}",
                    settings.enum_prefix, settings.enum_suffix
                ))
            }
        }
    }

    /// The bounds a typedef or member annotates over what the typedefs
    /// under it annotated: a bound set here replaces the one below.
    pub(super) fn merge_range(
        &self,
        under: Option<Range>,
        constraint: RangeConstraint,
        at: &str,
    ) -> Result<Range, Idl4CppError> {
        let eval = |expr| {
            self.ast
                .eval_const(expr)
                .map_err(|_| Idl4CppError::Unevaluated {
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
