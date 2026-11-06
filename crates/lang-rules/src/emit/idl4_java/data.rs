// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The data types: structures (§7.2.4.3.1, §7.14.1), unions
//! (§7.2.4.3.2, §7.14.2), enumerations (§7.2.4.3.3), bit sets
//! (§7.14.3.2), bit masks (§7.14.3.3), exceptions (§7.4.1, A.1.1) and
//! constants (§7.2.3, §7.2.3.1).

use super::text::{Writer, char_literal, string_literal, string_literal_units};
use super::types::{JType, Range, Shape};
use super::{Gen, Idl4JavaError, Platform, Settings};
use crate::verbatim;
use zerodds_ast::{
    AnyId, CaseLabel, ConstValue, MemberNode, ModuleNode, PrimitiveKind, TypeKind, TypeNode,
};
use zerodds_ast_core::{Id, Ref};

/// One member as a class holds it.
pub(crate) struct Member {
    /// The IDL name, for messages.
    pub idl: String,
    /// The private field.
    pub field: String,
    /// The parameter of the modifier and of the all values constructor.
    pub param: String,
    /// The accessor.
    pub getter: String,
    /// The modifier.
    pub setter: String,
    /// The Java type, boxed for `@optional` and `@external` (Tables 7.6,
    /// 7.9).
    pub jtype: JType,
    /// The value the default constructor gives it, when Java's own
    /// default is not it.
    pub initial: Option<String>,
    /// What the modifier checks before it stores a value.
    pub checks: Vec<String>,
    /// The user annotations on it (§7.16.2).
    pub annotations: Vec<String>,
}

/// The `java.lang.Throwable` methods an exception's accessor would
/// override with another return type: a mapped name that collides is
/// resolved as §7.1.1 resolves a collision, with a leading underscore.
const THROWABLE_METHODS: [&str; 10] = [
    "getMessage",
    "getLocalizedMessage",
    "getCause",
    "initCause",
    "getStackTrace",
    "setStackTrace",
    "fillInStackTrace",
    "printStackTrace",
    "getSuppressed",
    "addSuppressed",
];

impl Gen<'_> {
    /// A type declared directly in a module: its file, or none for a
    /// `typedef` (§7.2.4.6) and a forward declaration.
    pub(super) fn top_level_type(&mut self, id: Id<TypeNode>) -> Result<(), Idl4JavaError> {
        let node = self.ast.type_(id).map_err(|_| Idl4JavaError::Unresolved {
            what: "a type".to_string(),
            at: String::new(),
        })?;
        if node.completeness == zerodds_ast::Completeness::Forward {
            return Ok(());
        }
        let module = node.module;
        let origin = self.origin(AnyId::Type(id, node.node_id));
        let declared = self.type_declaration(id)?;
        for (class, body) in declared {
            let slots = verbatim::declaration_slots(self.ast, id, verbatim::aliases::JAVA);
            let mut text = String::new();
            for line in &slots.begin_file {
                text.push_str(line);
                text.push('\n');
            }
            text.push_str(&body);
            for line in &slots.end_file {
                text.push_str(line);
                text.push('\n');
            }
            self.file(module, &class, &origin, &text)?;
        }
        Ok(())
    }

    /// The classes one type declaration maps to, as (name, source); none
    /// for a `typedef`.
    pub(super) fn type_declaration(
        &self,
        id: Id<TypeNode>,
    ) -> Result<Vec<(String, String)>, Idl4JavaError> {
        let node = self.ast.type_(id).map_err(|_| Idl4JavaError::Unresolved {
            what: "a type".to_string(),
            at: String::new(),
        })?;
        let settings = self.settings_of_type(id);
        let name = settings.naming.type_name(&node.name);
        Ok(match &node.kind {
            TypeKind::Structure { .. } => vec![(name.clone(), self.structure(id, &name)?)],
            TypeKind::Union { .. } => vec![(name.clone(), self.union(id, &name)?)],
            TypeKind::Enumeration { .. } => vec![(name.clone(), self.enumeration(id, &name)?)],
            TypeKind::Bitset { .. } => vec![(name.clone(), self.bitset(id, &name)?)],
            TypeKind::Bitmask { .. } => {
                let flags = format!("{name}Flags");
                vec![(flags.clone(), self.bitmask(id, &flags)?)]
            }
            TypeKind::Valuetype { .. } | TypeKind::ValueBox { .. } => self.valuetype(id, &name)?,
            // §7.2.4.6: "the IDL typedef does not result in any Java types".
            TypeKind::Alias { .. } => Vec::new(),
            // §7.2.4.5: "This language mapping specification does not
            // define any native types." A declaration of one has no form;
            // a use of one is refused where it is used.
            TypeKind::Native => Vec::new(),
            TypeKind::Primitive(_)
            | TypeKind::Fixed { .. }
            | TypeKind::String { .. }
            | TypeKind::Sequence { .. }
            | TypeKind::Array { .. }
            | TypeKind::Map { .. }
            | TypeKind::Any
            | TypeKind::ObjectRef { .. }
            | TypeKind::ValueBase
            | TypeKind::AnnotationType { .. } => Vec::new(),
        })
    }

    /// One member, ready to be written.
    pub(super) fn member(
        &self,
        member: Id<MemberNode>,
        settings: &Settings,
        owner: &str,
        throwable: bool,
    ) -> Result<Member, Idl4JavaError> {
        let node = self
            .ast
            .member(member)
            .map_err(|_| Idl4JavaError::Unresolved {
                what: "a member".to_string(),
                at: owner.to_string(),
            })?;
        let at = format!("{owner}.{}", node.name);
        let mut jtype = self.java_type(&node.type_, settings, &at)?;
        let xtypes = node.facets.xtypes.as_ref();
        if let Some(range) = xtypes.and_then(|x| x.range) {
            jtype.range = Some(self.merge_range(jtype.range.take(), range, &at)?);
        }
        let optional = xtypes.is_some_and(|x| x.is_optional);
        let external = xtypes.is_some_and(|x| x.is_external);
        let basic = jtype.is_primitive();
        if (optional || external) && basic {
            jtype = jtype.boxed_type();
        }
        let initial = if optional {
            None
        } else if let Some(value) = xtypes.and_then(|x| x.default_value) {
            let value = self
                .ast
                .eval_const(value)
                .map_err(|_| Idl4JavaError::Unevaluated {
                    what: "the value of @default".to_string(),
                    at: at.clone(),
                })?;
            Some(self.java_value(&value, &jtype, &at)?)
        } else if external && basic {
            Some(boxed_zero(&jtype))
        } else if external && self.reaches(node.type_.resolved(), owner_type(self, member)) {
            // `@external T m` inside `T` itself: a default constructor that
            // built one would build one forever.
            None
        } else {
            self.initial(&jtype, &at)?
        };
        let mut getter = settings.naming.getter(&node.name);
        let mut setter = settings.naming.setter(&node.name);
        if throwable {
            if THROWABLE_METHODS.contains(&getter.as_str()) {
                getter = format!("_{getter}");
            }
            if THROWABLE_METHODS.contains(&setter.as_str()) {
                setter = format!("_{setter}");
            }
        }
        let param = settings.naming.variable(&node.name);
        let checks = self.checks(&jtype, &param, &at, 0)?;
        Ok(Member {
            idl: node.name.clone(),
            field: param.clone(),
            param,
            getter,
            setter,
            jtype,
            initial,
            checks,
            annotations: self.member_annotations(&node.facets, &node.type_)?,
        })
    }

    /// Whether the type `from` names reaches `target` by containment.
    fn reaches(&self, from: Option<Id<TypeNode>>, target: Option<Id<TypeNode>>) -> bool {
        let (Some(from), Some(target)) = (from, target) else {
            return false;
        };
        let mut seen = Vec::new();
        let mut queue = vec![from];
        while let Some(next) = queue.pop() {
            if next == target {
                return true;
            }
            if seen.contains(&next) || seen.len() > super::MAX_DEPTH * 16 {
                continue;
            }
            seen.push(next);
            let Ok(node) = self.ast.type_(next) else {
                continue;
            };
            match &node.kind {
                TypeKind::Alias { aliased } => queue.extend(aliased.resolved()),
                TypeKind::Structure { .. } => {
                    for member in self.ast.all_members_of(next) {
                        if let Ok(m) = self.ast.member(member) {
                            queue.extend(m.type_.resolved());
                        }
                    }
                }
                TypeKind::Array { element, .. } => queue.extend(element.resolved()),
                _ => {}
            }
        }
        false
    }

    /// The value a default constructor gives a member of type `jtype`,
    /// §7.2.4.3.1: primitives as Java initialises them, strings empty,
    /// arrays at their size with each element default-constructed,
    /// sequences empty, everything else through its default constructor.
    pub(super) fn initial(&self, jtype: &JType, at: &str) -> Result<Option<String>, Idl4JavaError> {
        Ok(match &jtype.shape {
            Shape::Boolean
            | Shape::Char
            | Shape::Integer { .. }
            | Shape::Float
            | Shape::Double
            | Shape::Reference => None,
            Shape::BigInteger => Some("java.math.BigInteger.ZERO".to_string()),
            Shape::LongDouble | Shape::Fixed { .. } => {
                Some("java.math.BigDecimal.ZERO".to_string())
            }
            Shape::Str { .. } => Some(if jtype.text == "java.lang.String" {
                "\"\"".to_string()
            } else {
                format!("new {}()", jtype.text)
            }),
            Shape::Sequence {
                specific, element, ..
            } => Some(match specific {
                Some(class) => format!("new org.omg.type.{class}(0)"),
                None => format!("new java.util.ArrayList<{}>()", element.boxed),
            }),
            Shape::Map { .. } => {
                let arguments = jtype
                    .text
                    .strip_prefix("java.util.Map<")
                    .unwrap_or_default();
                Some(format!("new java.util.LinkedHashMap<{arguments}()"))
            }
            Shape::Array { .. } => Some(self.array_initial(jtype, at)?),
            Shape::Class => Some(format!("new {}()", jtype.text)),
            Shape::Enum { default } => Some(default.clone()),
            Shape::BitSet { .. } => Some("new java.util.BitSet()".to_string()),
            Shape::Any => Some(format!("new {}()", jtype.text)),
        })
    }

    /// An array at its declared size, each element default-constructed,
    /// as one expression: a lambda-free immediately built array through a
    /// helper of the runtime would hide the element; a nested array
    /// initialiser names every element.
    fn array_initial(&self, jtype: &JType, at: &str) -> Result<String, Idl4JavaError> {
        let Shape::Array { element, dims } = &jtype.shape else {
            return Ok(String::from("null"));
        };
        let element_initial = self.initial(element, at)?;
        let dims_text: String = dims.iter().map(|d| format!("[{d}]")).collect();
        let Some(element_initial) = element_initial else {
            return Ok(format!("new {}{}", element.text, dims_text));
        };
        // `org.omg.type.Arrays.fill` builds the array and fills every
        // element with a fresh value from the supplier, one per element.
        let total: u64 = dims.iter().product();
        if total > 1 << 16 {
            return Err(Idl4JavaError::NoJavaForm {
                construct: format!(
                    "an array of {total} elements whose elements each need a constructor"
                ),
                at: at.to_string(),
            });
        }
        let mut text = String::new();
        nested_initialiser(&element_initial, dims, &mut text);
        Ok(format!(
            "new {}{}{}",
            element.text,
            "[]".repeat(dims.len()),
            text
        ))
    }

    /// The statements a modifier runs on `var` before it stores it.
    pub(super) fn checks(
        &self,
        jtype: &JType,
        var: &str,
        at: &str,
        depth: usize,
    ) -> Result<Vec<String>, Idl4JavaError> {
        let mut out = Vec::new();
        let what = at;
        match &jtype.shape {
            Shape::Str { bound, wide } => {
                if let Some(bound) = bound {
                    out.push(format!(
                        "if ({var} != null && {var}.length() > {bound}) throw new \
                         IndexOutOfBoundsException(\"{what}: \" + {var}.length() + \" characters, \
                         the bound is {bound}\");"
                    ));
                }
                if !*wide {
                    let index = format!("i{depth}");
                    out.push(format!(
                        "if ({var} != null) for (int {index} = 0; {index} < {var}.length(); \
                         {index}++) if ({var}.charAt({index}) > 0xFF) throw new \
                         IndexOutOfBoundsException(\"{what}: the character at \" + {index} + \" \
                         is no IDL char\");"
                    ));
                }
            }
            Shape::Sequence { element, bound, .. } => {
                if let Some(bound) = bound {
                    out.push(format!(
                        "if ({var} != null && {var}.size() > {bound}) throw new \
                         IndexOutOfBoundsException(\"{what}: \" + {var}.size() + \" elements, the \
                         bound is {bound}\");"
                    ));
                }
                let element_var = format!("e{depth}");
                let inner = self.checks(element, &element_var, at, depth + 1)?;
                if !inner.is_empty() {
                    out.push(format!(
                        "if ({var} != null) for ({} {element_var} : {var}) {{ {} }}",
                        element.boxed,
                        inner.join(" ")
                    ));
                }
            }
            Shape::Array { element, dims } => {
                let inner = self.array_checks(var, dims, element, at, depth)?;
                out.push(format!("if ({var} != null) {{ {inner} }}"));
            }
            Shape::Map { key, value, bound } => {
                if let Some(bound) = bound {
                    out.push(format!(
                        "if ({var} != null && {var}.size() > {bound}) throw new \
                         IndexOutOfBoundsException(\"{what}: \" + {var}.size() + \" entries, the \
                         bound is {bound}\");"
                    ));
                }
                let key_var = format!("k{depth}");
                let value_var = format!("v{depth}");
                let key_checks = self.checks(key, &key_var, at, depth + 1)?;
                let value_checks = self.checks(value, &value_var, at, depth + 1)?;
                if !key_checks.is_empty() || !value_checks.is_empty() {
                    let entry = format!("m{depth}");
                    let arguments = jtype
                        .text
                        .strip_prefix("java.util.Map<")
                        .unwrap_or_default();
                    let arguments = arguments.strip_suffix('>').unwrap_or(arguments);
                    out.push(format!(
                        "if ({var} != null) for (java.util.Map.Entry<{arguments}> {entry} : \
                         {var}.entrySet()) {{ {} {} {} {} }}",
                        if key_checks.is_empty() {
                            String::new()
                        } else {
                            format!(
                                "{} {key_var} = {entry}.getKey();",
                                key_type_of(&jtype.text, true)
                            )
                        },
                        key_checks.join(" "),
                        if value_checks.is_empty() {
                            String::new()
                        } else {
                            format!(
                                "{} {value_var} = {entry}.getValue();",
                                key_type_of(&jtype.text, false)
                            )
                        },
                        value_checks.join(" ")
                    ));
                }
            }
            Shape::Fixed { digits, scale } => {
                let whole = i32::from(*digits) - i32::from(*scale);
                out.push(format!(
                    "if ({var} != null) {{ {var} = {var}.setScale({scale}); if ({var}.precision() \
                     - {var}.scale() > {whole}) throw new ArithmeticException(\"{what}: \" + {var} \
                     + \" has more than {whole} digits before the point\"); }}"
                ));
            }
            Shape::BitSet { bound } => out.push(format!(
                "if ({var} != null && {var}.length() > {bound}) throw new \
                 IndexOutOfBoundsException(\"{what}: bit \" + ({var}.length() - 1) + \" is past \
                 the bit bound {bound}\");"
            )),
            _ => {}
        }
        if let Some(range) = &jtype.range {
            out.extend(self.range_checks(jtype, range, var, at)?);
        }
        Ok(out)
    }

    /// "Bound violations for the array shall raise a
    /// java.lang.IndexOutOfBoundsException" (§7.2.4.4): every dimension's
    /// length, then the elements' own checks.
    fn array_checks(
        &self,
        var: &str,
        dims: &[u64],
        element: &JType,
        at: &str,
        depth: usize,
    ) -> Result<String, Idl4JavaError> {
        let Some((dim, rest)) = dims.split_first() else {
            let checks = self.checks(element, var, at, depth + 1)?;
            return Ok(checks.join(" "));
        };
        let counter = format!("i{depth}_{}", rest.len());
        let inner = self.array_checks(&format!("{var}[{counter}]"), rest, element, at, depth)?;
        let mut out = format!(
            "if ({var} == null || {var}.length != {dim}) throw new IndexOutOfBoundsException(\"{at}: \
             an array of {dim} elements\");"
        );
        if !inner.is_empty() {
            out.push_str(&format!(
                " for (int {counter} = 0; {counter} < {dim}; {counter}++) {{ {inner} }}"
            ));
        }
        Ok(out)
    }

    /// `@range`, `@min`, `@max`: "The provided value is tested in the
    /// member modifier (setter), and a java.lang.IllegalArgumentException
    /// is raised if the parameter does not meet requirements" (Table 7.8).
    fn range_checks(
        &self,
        jtype: &JType,
        range: &Range,
        var: &str,
        at: &str,
    ) -> Result<Vec<String>, Idl4JavaError> {
        let mut out = Vec::new();
        let nullable = !jtype.is_primitive() || jtype.text != jtype_primitive_text(jtype);
        let guard = if nullable {
            format!("{var} != null && ")
        } else {
            String::new()
        };
        for (bound, op, name) in [(&range.min, "<", "min"), (&range.max, ">", "max")] {
            let Some(value) = bound else {
                continue;
            };
            let condition = self.compare(jtype, var, value, op, at)?;
            out.push(format!(
                "if ({guard}{condition}) throw new IllegalArgumentException(\"{at}: \" + {var} + \
                 \" is outside the {name} {}\");",
                value_text(value)
            ));
        }
        Ok(out)
    }

    /// `var op value` in Java, for the shape of `jtype`.
    fn compare(
        &self,
        jtype: &JType,
        var: &str,
        value: &ConstValue,
        op: &str,
        at: &str,
    ) -> Result<String, Idl4JavaError> {
        Ok(match &jtype.shape {
            Shape::Integer { bits, unsigned } => {
                let number = integer_of(value, at)?;
                if *unsigned {
                    match bits {
                        8 => format!("Byte.toUnsignedInt({var}) {op} {number}"),
                        16 => format!("Short.toUnsignedInt({var}) {op} {number}"),
                        32 => format!("Integer.compareUnsigned({var}, (int) {number}L) {op} 0"),
                        _ => format!(
                            "Long.compareUnsigned({var}, new java.math.BigInteger(\"{number}\").longValue()) {op} 0"
                        ),
                    }
                } else if *bits == 64 {
                    format!("{var} {op} {number}L")
                } else {
                    format!("{var} {op} {number}")
                }
            }
            Shape::Char => format!("{var} {op} {}", integer_of(value, at)?),
            Shape::Float | Shape::Double => format!("{var} {op} {}", value_text(value)),
            Shape::BigInteger => format!(
                "{var}.compareTo(new java.math.BigInteger(\"{}\")) {op} 0",
                integer_of(value, at)?
            ),
            Shape::LongDouble | Shape::Fixed { .. } => format!(
                "{var}.compareTo(new java.math.BigDecimal(\"{}\")) {op} 0",
                value_text(value)
            ),
            _ => {
                return Err(Idl4JavaError::NoJavaForm {
                    construct: "@range, @min or @max on a type that is no number".to_string(),
                    at: at.to_string(),
                });
            }
        })
    }

    /// A constant value as a Java expression of the type `jtype`.
    pub(super) fn java_value(
        &self,
        value: &ConstValue,
        jtype: &JType,
        at: &str,
    ) -> Result<String, Idl4JavaError> {
        let unfit = || Idl4JavaError::Unevaluated {
            what: format!(
                "a value of {} for the Java type `{}`",
                value_text(value),
                jtype.text
            ),
            at: at.to_string(),
        };
        let boxed = !jtype.is_primitive() || jtype.text != jtype_primitive_text(jtype);
        let primitive = match (&jtype.shape, value) {
            (Shape::Boolean, ConstValue::Boolean(flag)) => flag.to_string(),
            (Shape::Boolean, ConstValue::Integer(number)) => (*number != 0).to_string(),
            (Shape::Char, ConstValue::Char(ch)) => {
                let unit = u16::try_from(u32::from(*ch)).map_err(|_| unfit())?;
                char_literal(unit)
            }
            (Shape::Char, ConstValue::WChar(unit)) => {
                char_literal(u16::try_from(*unit).map_err(|_| unfit())?)
            }
            (Shape::Char, ConstValue::Integer(number)) => {
                char_literal(u16::try_from(*number).map_err(|_| unfit())?)
            }
            (Shape::Integer { bits, .. }, ConstValue::Integer(number)) => {
                integer_literal(*number, *bits).ok_or_else(unfit)?
            }
            (Shape::Integer { bits, .. }, ConstValue::Char(ch)) => {
                integer_literal(i128::from(u32::from(*ch)), *bits).ok_or_else(unfit)?
            }
            (Shape::Float, ConstValue::Integer(_) | ConstValue::Float { .. }) => {
                format!("{}f", value_text(value))
            }
            (Shape::Double, ConstValue::Integer(_) | ConstValue::Float { .. }) => {
                format!("{}d", value_text(value))
            }
            (Shape::BigInteger, ConstValue::Integer(number)) => {
                format!("new java.math.BigInteger(\"{number}\")")
            }
            (
                Shape::LongDouble | Shape::Fixed { .. },
                ConstValue::Integer(_) | ConstValue::Float { .. } | ConstValue::Fixed { .. },
            ) => format!("new java.math.BigDecimal(\"{}\")", value_text(value)),
            (Shape::Str { .. }, ConstValue::String(text)) => {
                self.string_value(&jtype.text, &string_literal(text))
            }
            (Shape::Str { .. }, ConstValue::WString(units)) => {
                let mut code_units = Vec::new();
                for unit in units {
                    let ch = char::from_u32(*unit);
                    match ch {
                        Some(ch) => {
                            let mut buffer = [0u16; 2];
                            code_units.extend_from_slice(ch.encode_utf16(&mut buffer));
                        }
                        None => code_units.push(u16::try_from(*unit).map_err(|_| unfit())?),
                    }
                }
                self.string_value(&jtype.text, &string_literal_units(&code_units))
            }
            (Shape::Enum { default }, ConstValue::Integer(number)) => {
                let class = default
                    .rsplit_once('.')
                    .map_or(default.as_str(), |(class, _)| class);
                let constant = self
                    .enumerator_named(&jtype.text, *number)
                    .ok_or_else(unfit)?;
                format!("{class}.{constant}")
            }
            _ => return Err(unfit()),
        };
        Ok(if boxed && jtype.is_primitive() {
            format!("{}.valueOf({primitive})", jtype.text)
        } else {
            primitive
        })
    }

    fn string_value(&self, class: &str, literal: &str) -> String {
        if class == "java.lang.String" {
            literal.to_string()
        } else {
            format!("new {class}({literal})")
        }
    }

    /// The Java constant of the enumerator of the enumeration whose Java
    /// name is `class` with the value `value`.
    pub(super) fn enumerator_named(&self, class: &str, value: i128) -> Option<String> {
        for (id, node) in self.ast.types() {
            if !matches!(node.kind, TypeKind::Enumeration { .. }) {
                continue;
            }
            if self.qualified(id).ok().as_deref() != Some(class) {
                continue;
            }
            let settings = self.settings_of_type(id);
            for enumerator in self.ast.enumerators_of(id) {
                if self.ast.effective_enumerator_value(*enumerator) == Some(value) {
                    let name = self.ast.enumerator(*enumerator).ok()?.name.clone();
                    return Some(settings.naming.enumerator(&name));
                }
            }
        }
        None
    }

    // -- structures, §7.2.4.3.1 and §7.14.1 --------------------------------

    fn structure(&self, id: Id<TypeNode>, name: &str) -> Result<String, Idl4JavaError> {
        let node = self.ast.type_(id).map_err(|_| Idl4JavaError::Unresolved {
            what: "a structure".to_string(),
            at: name.to_string(),
        })?;
        let settings = self.settings_of_type(id);
        let TypeKind::Structure { base, members } = &node.kind else {
            return Err(Idl4JavaError::Unresolved {
                what: "a structure".to_string(),
                at: name.to_string(),
            });
        };
        let base = match base.as_ref().and_then(Ref::resolved) {
            Some(base) => Some(self.through_aliases(base, name)?),
            None => None,
        };
        let base_name = match base {
            Some(base) => Some(self.qualified(base)?),
            None => None,
        };
        let mut own = Vec::new();
        for member in members {
            own.push(self.member(*member, &settings, name, false)?);
        }
        check_member_collisions(&own, name)?;
        let slots = verbatim::declaration_slots(self.ast, id, verbatim::aliases::JAVA);
        let mut w = Writer::new();
        for text in &slots.before {
            w.verbatim(text);
        }
        for annotation in self.user_annotations(&node.facets)? {
            w.line(&annotation);
        }
        let extends = base_name
            .as_ref()
            .map_or_else(String::new, |base| format!(" extends {base}"));
        w.open(&format!(
            "public class {name}{extends} implements java.io.Serializable"
        ));
        for text in &slots.begin {
            w.verbatim(text);
        }
        w.line("private static final long serialVersionUID = 1L;");
        write_fields(&mut w, &own);
        // The default constructor.
        w.line("");
        w.line("/** The default constructor (§7.2.4.3.1). */");
        w.open(&format!("public {name}()"));
        if base_name.is_some() {
            w.line("super();");
        }
        write_initialisers(&mut w, &own);
        w.close();
        // The all values constructor.
        let mut params: Vec<String> = Vec::new();
        if let Some(base) = &base_name {
            params.push(format!("{base} parent"));
        }
        params.extend(own.iter().map(|m| format!("{} {}", m.jtype.text, m.param)));
        w.line("");
        w.line("/** The all values constructor (§7.2.4.3.1, §7.14.1). */");
        w.open(&format!("public {name}({})", params.join(", ")));
        if base_name.is_some() {
            w.line("super(java.util.Objects.requireNonNull(parent, \"parent\"));");
        }
        for member in &own {
            w.line(&format!(
                "this.{} = check_{}({});",
                member.field, member.field, member.param
            ));
        }
        w.close();
        // The copy constructor a derived structure's all values
        // constructor calls with its parent.
        w.line("");
        w.line("/** A copy of `other`, for the all values constructor of a derived structure. */");
        w.open(&format!("protected {name}({name} other)"));
        if base_name.is_some() {
            w.line("super(other);");
        }
        for member in &own {
            w.line(&format!("this.{0} = other.{0};", member.field));
        }
        w.close();
        write_accessors(&mut w, &own);
        for text in &slots.end {
            w.verbatim(text);
        }
        w.close();
        for text in &slots.after {
            w.verbatim(text);
        }
        Ok(w.finish())
    }

    /// The type an alias chain ends at.
    pub(super) fn through_aliases(
        &self,
        id: Id<TypeNode>,
        at: &str,
    ) -> Result<Id<TypeNode>, Idl4JavaError> {
        let mut current = id;
        for _ in 0..super::MAX_DEPTH {
            match self.ast.type_(current).map(|node| &node.kind) {
                Ok(TypeKind::Alias { aliased }) => {
                    current = aliased
                        .resolved()
                        .ok_or_else(|| Idl4JavaError::Unresolved {
                            what: "a typedef".to_string(),
                            at: at.to_string(),
                        })?;
                }
                Ok(_) => return Ok(current),
                Err(_) => break,
            }
        }
        Err(Idl4JavaError::Unresolved {
            what: "a chain of typedefs".to_string(),
            at: at.to_string(),
        })
    }

    // -- unions, §7.2.4.3.2 and §7.14.2 -------------------------------------

    #[allow(clippy::too_many_lines)]
    fn union(&self, id: Id<TypeNode>, name: &str) -> Result<String, Idl4JavaError> {
        let node = self.ast.type_(id).map_err(|_| Idl4JavaError::Unresolved {
            what: "a union".to_string(),
            at: name.to_string(),
        })?;
        let settings = self.settings_of_type(id);
        let TypeKind::Union {
            discriminator,
            cases,
            default_case,
        } = &node.kind
        else {
            return Err(Idl4JavaError::Unresolved {
                what: "a union".to_string(),
                at: name.to_string(),
            });
        };
        let disc_type = self.java_type(discriminator, &settings, name)?;
        let disc_id = discriminator
            .resolved()
            .map(|d| self.through_aliases(d, name))
            .transpose()?
            .ok_or_else(|| Idl4JavaError::Unresolved {
                what: "a discriminator".to_string(),
                at: name.to_string(),
            })?;
        let domain = self.discriminator_domain(disc_id, name)?;
        // The cases, with their labels as values.
        struct Case {
            member: super::data::Member,
            labels: Vec<i128>,
            default: bool,
        }
        let mut all_cases = Vec::new();
        let mut explicit: Vec<i128> = Vec::new();
        for case in cases.iter().chain(default_case.iter()) {
            let case_node = self
                .ast
                .case(*case)
                .map_err(|_| Idl4JavaError::Unresolved {
                    what: "a case".to_string(),
                    at: name.to_string(),
                })?;
            let member = self.member(case_node.member, &settings, name, false)?;
            let mut labels = Vec::new();
            let mut is_default = false;
            for label in &case_node.labels {
                match label {
                    CaseLabel::Default => is_default = true,
                    CaseLabel::Value(expr) => {
                        let value = self.ast.eval_integral(*expr).map_err(|_| {
                            Idl4JavaError::Unevaluated {
                                what: "a case label".to_string(),
                                at: format!("{name}.{}", member.idl),
                            }
                        })?;
                        labels.push(value);
                        explicit.push(value);
                    }
                }
            }
            if all_cases
                .iter()
                .any(|c: &Case| c.member.param == member.param)
            {
                continue;
            }
            all_cases.push(Case {
                member,
                labels,
                default: is_default,
            });
        }
        // §7.2.4.3.2: the discriminator takes a leading underscore when
        // its name clashes with the union's or a member's.
        let clash = |candidate: &str| {
            name.eq_ignore_ascii_case(candidate)
                || all_cases
                    .iter()
                    .any(|c| c.member.idl.eq_ignore_ascii_case(candidate))
        };
        let disc_idl = if clash("discriminator") {
            "_discriminator"
        } else {
            "discriminator"
        };
        let disc_getter = match settings.naming {
            super::names::Naming::Idl => format!("get_{disc_idl}"),
            super::names::Naming::Java => {
                if disc_idl.starts_with('_') {
                    "get_Discriminator".to_string()
                } else {
                    "getDiscriminator".to_string()
                }
            }
        };
        let first_free = domain.first_free(&explicit);
        let has_default_label = all_cases.iter().any(|c| c.default);
        let covered = domain.covers(&explicit);
        let disc_text = &disc_type.text;
        let literal = |value: i128| self.discriminator_literal(&disc_type, disc_id, value, name);
        // The value the default constructor starts at: the default value
        // of the discriminator type (§7.2.4.3.2).
        let initial_disc = domain.default_value();
        let selected_initial = all_cases.iter().find(|c| {
            c.labels.contains(&initial_disc) || (c.default && !explicit.contains(&initial_disc))
        });

        let slots = verbatim::declaration_slots(self.ast, id, verbatim::aliases::JAVA);
        let mut w = Writer::new();
        for text in &slots.before {
            w.verbatim(text);
        }
        for annotation in self.user_annotations(&node.facets)? {
            w.line(&annotation);
        }
        w.open(&format!(
            "public final class {name} implements java.io.Serializable"
        ));
        for text in &slots.begin {
            w.verbatim(text);
        }
        w.line("private static final long serialVersionUID = 1L;");
        w.line("");
        w.line(&format!("private {disc_text} __discriminator;"));
        w.line("private java.lang.Object __value;");
        w.line("");
        w.line("/** The discriminator at the default value of its type (§7.2.4.3.2). */");
        w.open(&format!("public {name}()"));
        w.line(&format!(
            "this.__discriminator = {};",
            literal(initial_disc)?
        ));
        if let Some(case) = selected_initial {
            let initial = match &case.member.initial {
                Some(initial) => initial.clone(),
                None => default_primitive_boxed(&case.member.jtype),
            };
            w.line(&format!("this.__value = {initial};"));
        }
        w.close();
        w.line("");
        w.line("/** The discriminator. */");
        w.open(&format!("public {disc_text} {disc_getter}()"));
        w.line("return this.__discriminator;");
        w.close();
        for case in &all_cases {
            let member = &case.member;
            let jt = &member.jtype.text;
            // Whether the discriminator selects this case.
            let selected = if case.default {
                if explicit.is_empty() {
                    "true".to_string()
                } else {
                    let mut conditions = Vec::new();
                    for value in &explicit {
                        conditions.push(format!("this.__discriminator != {}", literal(*value)?));
                    }
                    conditions.join(" && ")
                }
            } else {
                let mut conditions = Vec::new();
                for value in &case.labels {
                    conditions.push(format!("this.__discriminator == {}", literal(*value)?));
                }
                conditions.join(" || ")
            };
            w.line("");
            w.line(&format!(
                "/** `{}`; IllegalStateException when the discriminator selects another member. */",
                member.idl
            ));
            for annotation in &member.annotations {
                w.line(annotation);
            }
            if jt.contains('<') {
                w.line("@SuppressWarnings(\"unchecked\")");
            }
            w.open(&format!("public {jt} {}()", member.getter));
            w.open(&format!("if (!({selected}))"));
            w.line(&format!(
                "throw new IllegalStateException(\"{name}.{}: the discriminator \" + \
                 this.__discriminator + \" selects another member\");",
                member.idl
            ));
            w.close();
            w.line(&format!("return {};", unbox(&member.jtype, "this.__value")));
            w.close();
            // The simple modifier: the first label, or for the default
            // member the first value no label names.
            let first_label = if case.default {
                first_free
            } else {
                case.labels.first().copied()
            };
            let Some(first_label) = first_label else {
                return Err(Idl4JavaError::NoJavaForm {
                    construct: "a default member whose labels leave no discriminator value"
                        .to_string(),
                    at: format!("{name}.{}", member.idl),
                });
            };
            w.line("");
            w.open(&format!(
                "public void {}({jt} {})",
                member.setter, member.param
            ));
            for check in &member.checks {
                w.line(check);
            }
            w.line(&format!(
                "this.__discriminator = {};",
                literal(first_label)?
            ));
            w.line(&format!("this.__value = {};", member.param));
            w.close();
            // The modifier that takes the discriminator: for a member of
            // more than one label, and for the default member.
            if case.labels.len() > 1 || case.default {
                let refused = if case.default {
                    if explicit.is_empty() {
                        "false".to_string()
                    } else {
                        let mut conditions = Vec::new();
                        for value in &explicit {
                            conditions.push(format!("discriminator == {}", literal(*value)?));
                        }
                        conditions.join(" || ")
                    }
                } else {
                    let mut conditions = Vec::new();
                    for value in &case.labels {
                        conditions.push(format!("discriminator != {}", literal(*value)?));
                    }
                    conditions.join(" && ")
                };
                w.line("");
                w.open(&format!(
                    "public void {}({jt} {}, {disc_text} discriminator)",
                    member.setter, member.param
                ));
                w.open(&format!("if ({refused})"));
                w.line(&format!(
                    "throw new IllegalArgumentException(\"{name}.{}: \" + discriminator + \" is \
                     no label of this member\");",
                    member.idl
                ));
                w.close();
                for check in &member.checks {
                    w.line(check);
                }
                w.line("this.__discriminator = discriminator;");
                w.line(&format!("this.__value = {};", member.param));
                w.close();
            }
        }
        // `__default()`: no default label, and the labels do not cover
        // the discriminator type.
        if !has_default_label && !covered {
            let Some(free) = first_free else {
                return Err(Idl4JavaError::NoJavaForm {
                    construct: "a union whose labels leave no default value".to_string(),
                    at: name.to_string(),
                });
            };
            w.line("");
            w.line("/** The discriminator at the first value no label names; no member (§7.2.4.3.2). */");
            w.open("public void __default()");
            w.line(&format!("this.__discriminator = {};", literal(free)?));
            w.line("this.__value = null;");
            w.close();
            let mut conditions = Vec::new();
            for value in &explicit {
                conditions.push(format!("discriminator == {}", literal(*value)?));
            }
            let refused = if conditions.is_empty() {
                "false".to_string()
            } else {
                conditions.join(" || ")
            };
            w.line("");
            w.open(&format!("public void __default({disc_text} discriminator)"));
            w.open(&format!("if ({refused})"));
            w.line(&format!(
                "throw new IllegalArgumentException(\"{name}: \" + discriminator + \" is the label \
                 of a member\");"
            ));
            w.close();
            w.line("this.__discriminator = discriminator;");
            w.line("this.__value = null;");
            w.close();
        }
        for text in &slots.end {
            w.verbatim(text);
        }
        w.close();
        for text in &slots.after {
            w.verbatim(text);
        }
        Ok(w.finish())
    }

    /// The Java text of a discriminator value.
    fn discriminator_literal(
        &self,
        disc: &JType,
        disc_id: Id<TypeNode>,
        value: i128,
        at: &str,
    ) -> Result<String, Idl4JavaError> {
        match &disc.shape {
            Shape::Enum { .. } => {
                let settings = self.settings_of_type(disc_id);
                for enumerator in self.ast.enumerators_of(disc_id) {
                    if self.ast.effective_enumerator_value(*enumerator) == Some(value) {
                        let node = self.ast.enumerator(*enumerator).map_err(|_| {
                            Idl4JavaError::Unresolved {
                                what: "an enumerator".to_string(),
                                at: at.to_string(),
                            }
                        })?;
                        return Ok(format!(
                            "{}.{}",
                            disc.text,
                            settings.naming.enumerator(&node.name)
                        ));
                    }
                }
                Err(Idl4JavaError::Unevaluated {
                    what: format!("an enumerator of value {value}"),
                    at: at.to_string(),
                })
            }
            Shape::Boolean => Ok((value != 0).to_string()),
            Shape::Char => Ok(char_literal(u16::try_from(value).map_err(|_| {
                Idl4JavaError::Unevaluated {
                    what: format!("a char label {value}"),
                    at: at.to_string(),
                }
            })?)),
            Shape::Integer { bits, .. } => {
                integer_literal(value, *bits).ok_or_else(|| Idl4JavaError::Unevaluated {
                    what: format!("a label {value}"),
                    at: at.to_string(),
                })
            }
            _ => Err(Idl4JavaError::NoJavaForm {
                construct: "a discriminator of this type".to_string(),
                at: at.to_string(),
            }),
        }
    }

    /// The values a discriminator type has, in the order "starting from a
    /// 0 index" walks them (§7.2.4.3.2).
    fn discriminator_domain(&self, disc: Id<TypeNode>, at: &str) -> Result<Domain, Idl4JavaError> {
        let node = self
            .ast
            .type_(disc)
            .map_err(|_| Idl4JavaError::Unresolved {
                what: "a discriminator".to_string(),
                at: at.to_string(),
            })?;
        Ok(match &node.kind {
            TypeKind::Enumeration { .. } => Domain::Values(
                self.ast
                    .enumerators_of(disc)
                    .iter()
                    .filter_map(|e| self.ast.effective_enumerator_value(*e))
                    .collect(),
            ),
            TypeKind::Primitive(PrimitiveKind::Boolean) => Domain::Values(vec![0, 1]),
            TypeKind::Primitive(kind) => {
                let (low, high) = match kind {
                    PrimitiveKind::Char | PrimitiveKind::Octet | PrimitiveKind::UInt8 => (0, 255),
                    PrimitiveKind::WChar | PrimitiveKind::UInt16 => (0, 65_535),
                    PrimitiveKind::Int8 => (-128, 127),
                    PrimitiveKind::Int16 => (-32_768, 32_767),
                    PrimitiveKind::Int32 => (i128::from(i32::MIN), i128::from(i32::MAX)),
                    PrimitiveKind::UInt32 => (0, i128::from(u32::MAX)),
                    PrimitiveKind::Int64 => (i128::from(i64::MIN), i128::from(i64::MAX)),
                    PrimitiveKind::UInt64 => (0, i128::from(u64::MAX)),
                    _ => {
                        return Err(Idl4JavaError::NoJavaForm {
                            construct: "a floating-point discriminator".to_string(),
                            at: at.to_string(),
                        });
                    }
                };
                Domain::Range(low, high)
            }
            _ => {
                return Err(Idl4JavaError::NoJavaForm {
                    construct: "a discriminator of this type".to_string(),
                    at: at.to_string(),
                });
            }
        })
    }

    // -- enumerations, §7.2.4.3.3 ---------------------------------------------

    fn enumeration(&self, id: Id<TypeNode>, name: &str) -> Result<String, Idl4JavaError> {
        let node = self.ast.type_(id).map_err(|_| Idl4JavaError::Unresolved {
            what: "an enumeration".to_string(),
            at: name.to_string(),
        })?;
        let settings = self.settings_of_type(id);
        let mut constants = Vec::new();
        for enumerator in self.ast.enumerators_of(id) {
            let e = self
                .ast
                .enumerator(*enumerator)
                .map_err(|_| Idl4JavaError::Unresolved {
                    what: "an enumerator".to_string(),
                    at: name.to_string(),
                })?;
            let value = self
                .ast
                .effective_enumerator_value(*enumerator)
                .ok_or_else(|| Idl4JavaError::Unevaluated {
                    what: "an enumerator's value".to_string(),
                    at: format!("{name}.{}", e.name),
                })?;
            let value = i32::try_from(value).map_err(|_| Idl4JavaError::Unevaluated {
                what: "an enumerator's value beyond int".to_string(),
                at: format!("{name}.{}", e.name),
            })?;
            constants.push((
                settings.naming.enumerator(&e.name),
                value,
                self.user_annotations(&e.facets)?,
            ));
        }
        let mut seen = std::collections::BTreeSet::new();
        for (constant, _, _) in &constants {
            if !seen.insert(constant.clone()) {
                return Err(Idl4JavaError::Collision {
                    name: constant.clone(),
                    first: name.to_string(),
                    second: name.to_string(),
                });
            }
        }
        let slots = verbatim::declaration_slots(self.ast, id, verbatim::aliases::JAVA);
        let mut w = Writer::new();
        for text in &slots.before {
            w.verbatim(text);
        }
        for annotation in self.user_annotations(&node.facets)? {
            w.line(&annotation);
        }
        w.open(&format!("public enum {name}"));
        for text in &slots.begin {
            w.verbatim(text);
        }
        for (index, (constant, value, annotations)) in constants.iter().enumerate() {
            for annotation in annotations {
                w.line(annotation);
            }
            let end = if index + 1 == constants.len() {
                ";"
            } else {
                ","
            };
            w.line(&format!("{constant}({value}){end}"));
        }
        w.line("");
        w.line("private final int value;");
        w.line("");
        w.open(&format!("private {name}(int value)"));
        w.line("this.value = value;");
        w.close();
        w.line("");
        w.line("/** The value of the enumerator. */");
        w.open("public int getValue()");
        w.line("return this.value;");
        w.close();
        w.line("");
        w.line(
            "/** The enumerator of value `v`; IllegalArgumentException for none (§7.2.4.3.3). */",
        );
        w.open(&format!("public static {name} valueOf(int v)"));
        w.open(&format!("for ({name} each : values())"));
        w.open("if (each.value == v)");
        w.line("return each;");
        w.close();
        w.close();
        w.line(&format!(
            "throw new IllegalArgumentException(\"{name}: no enumerator has the value \" + v);"
        ));
        w.close();
        for text in &slots.end {
            w.verbatim(text);
        }
        w.close();
        for text in &slots.after {
            w.verbatim(text);
        }
        Ok(w.finish())
    }

    // -- bit sets, §7.14.3.2 --------------------------------------------------

    fn bitset(&self, id: Id<TypeNode>, name: &str) -> Result<String, Idl4JavaError> {
        let node = self.ast.type_(id).map_err(|_| Idl4JavaError::Unresolved {
            what: "a bit set".to_string(),
            at: name.to_string(),
        })?;
        let settings = self.settings_of_type(id);
        let TypeKind::Bitset { base, .. } = &node.kind else {
            return Err(Idl4JavaError::Unresolved {
                what: "a bit set".to_string(),
                at: name.to_string(),
            });
        };
        let base_name = match base.as_ref().and_then(Ref::resolved) {
            Some(base) => Some(self.qualified(self.through_aliases(base, name)?)?),
            None => None,
        };
        struct Field {
            field: String,
            getter: String,
            setter: String,
            jtype: JType,
            check: String,
            annotations: Vec<String>,
        }
        let mut fields = Vec::new();
        for bitfield in self.ast.bitfields_of(id) {
            let b = self
                .ast
                .bitfield(*bitfield)
                .map_err(|_| Idl4JavaError::Unresolved {
                    what: "a bit field".to_string(),
                    at: name.to_string(),
                })?;
            // An anonymous bit field is padding and has no accessor.
            let Some(field_name) = b.name.clone() else {
                continue;
            };
            let at = format!("{name}.{field_name}");
            // §7.14.3.2: the smallest type that holds the field —
            // boolean, octet, unsigned short, unsigned long, unsigned long
            // long — when the IDL names none.
            let jtype = match b.holder_type.as_ref() {
                Some(holder) => self.java_type(holder, &settings, &at)?,
                None => {
                    let kind = match b.bits {
                        1 => PrimitiveKind::Boolean,
                        2..=8 => PrimitiveKind::Octet,
                        9..=16 => PrimitiveKind::UInt16,
                        17..=32 => PrimitiveKind::UInt32,
                        _ => PrimitiveKind::UInt64,
                    };
                    self.primitive(kind, &settings)
                }
            };
            let check = bitfield_check(&jtype, b.bits, &settings.naming.variable(&field_name), &at);
            fields.push(Field {
                field: settings.naming.variable(&field_name),
                getter: settings.naming.getter(&field_name),
                setter: settings.naming.setter(&field_name),
                jtype,
                check,
                annotations: self.user_annotations(&b.facets)?,
            });
        }
        let slots = verbatim::declaration_slots(self.ast, id, verbatim::aliases::JAVA);
        let mut w = Writer::new();
        for text in &slots.before {
            w.verbatim(text);
        }
        for annotation in self.user_annotations(&node.facets)? {
            w.line(&annotation);
        }
        let extends = base_name
            .as_ref()
            .map_or_else(String::new, |base| format!(" extends {base}"));
        w.open(&format!(
            "public class {name}{extends} implements java.io.Serializable"
        ));
        for text in &slots.begin {
            w.verbatim(text);
        }
        w.line("private static final long serialVersionUID = 1L;");
        for field in &fields {
            w.line("");
            for annotation in &field.annotations {
                w.line(annotation);
            }
            w.line(&format!("private {} {};", field.jtype.text, field.field));
        }
        for field in &fields {
            w.line("");
            w.open(&format!("public {} {}()", field.jtype.text, field.getter));
            w.line(&format!("return this.{};", field.field));
            w.close();
            w.line("");
            w.open(&format!(
                "public void {}({} {})",
                field.setter, field.jtype.text, field.field
            ));
            if !field.check.is_empty() {
                w.line(&field.check);
            }
            w.line(&format!("this.{0} = {0};", field.field));
            w.close();
        }
        for text in &slots.end {
            w.verbatim(text);
        }
        w.close();
        for text in &slots.after {
            w.verbatim(text);
        }
        Ok(w.finish())
    }

    // -- bit masks, §7.14.3.3 -------------------------------------------------

    fn bitmask(&self, id: Id<TypeNode>, flags: &str) -> Result<String, Idl4JavaError> {
        let node = self.ast.type_(id).map_err(|_| Idl4JavaError::Unresolved {
            what: "a bit mask".to_string(),
            at: flags.to_string(),
        })?;
        let settings = self.settings_of_type(id);
        let mut constants = Vec::new();
        for bit in self.ast.bits_of(id) {
            let b = self
                .ast
                .bit_value(*bit)
                .map_err(|_| Idl4JavaError::Unresolved {
                    what: "a bit".to_string(),
                    at: flags.to_string(),
                })?;
            let position = self.ast.effective_bit_position(*bit).ok_or_else(|| {
                Idl4JavaError::Unevaluated {
                    what: "a bit position".to_string(),
                    at: format!("{flags}.{}", b.name),
                }
            })?;
            constants.push((
                settings.naming.enumerator(&b.name),
                position,
                self.user_annotations(&b.facets)?,
            ));
        }
        let bound = self.ast.effective_bit_bound(id);
        let mut w = Writer::new();
        for annotation in self.user_annotations(&node.facets)? {
            w.line(&annotation);
        }
        w.line(&format!(
            "/** The flags of bit mask `{}`, for a java.util.BitSet of at most {bound} bits (§7.14.3.3). */",
            node.name
        ));
        w.open(&format!("public enum {flags}"));
        for (index, (constant, position, annotations)) in constants.iter().enumerate() {
            for annotation in annotations {
                w.line(annotation);
            }
            let end = if index + 1 == constants.len() {
                ";"
            } else {
                ","
            };
            w.line(&format!("{constant}({position}){end}"));
        }
        if constants.is_empty() {
            w.line(";");
        }
        w.line("");
        w.line("/** `@bit_bound`: the bits a value of the mask has. */");
        w.line(&format!("public static final int BIT_BOUND = {bound};"));
        w.line("");
        w.line("private final int position;");
        w.line("");
        w.open(&format!("private {flags}(int position)"));
        w.line("this.position = position;");
        w.close();
        w.line("");
        w.line("/** The bit of the flag: its index in a java.util.BitSet (`@position`). */");
        w.open("public int getPosition()");
        w.line("return this.position;");
        w.close();
        w.line("");
        w.line("/** The flag as the power of two of its position. */");
        w.open("public long getValue()");
        w.line("return 1L << this.position;");
        w.close();
        w.close();
        Ok(w.finish())
    }

    // -- exceptions, §7.4.1 and A.1.1 -----------------------------------------

    /// An exception declared directly in a module: its file.
    pub(super) fn top_level_exception(
        &mut self,
        id: Id<zerodds_ast::ExceptionNode>,
    ) -> Result<(), Idl4JavaError> {
        let node = self
            .ast
            .exception(id)
            .map_err(|_| Idl4JavaError::Unresolved {
                what: "an exception".to_string(),
                at: String::new(),
            })?;
        let module = node.module;
        let settings = self.settings_in(module, None, &node.facets);
        let name = settings.naming.type_name(&node.name);
        let origin = self.origin(AnyId::Exception(id, node.node_id));
        let body = self.exception(id, &name)?;
        self.file(module, &name, &origin, &body)
    }

    /// The class of an exception.
    pub(super) fn exception(
        &self,
        id: Id<zerodds_ast::ExceptionNode>,
        name: &str,
    ) -> Result<String, Idl4JavaError> {
        let node = self
            .ast
            .exception(id)
            .map_err(|_| Idl4JavaError::Unresolved {
                what: "an exception".to_string(),
                at: name.to_string(),
            })?;
        let settings = self.settings_in(node.module, node.scope.as_deref(), &node.facets);
        let mut members = Vec::new();
        for member in &node.members {
            members.push(self.member(*member, &settings, name, true)?);
        }
        check_member_collisions(&members, name)?;
        let base = match self.options.platform {
            Platform::Dds => "java.lang.RuntimeException",
            Platform::Corba => "org.omg.CORBA.UserException",
        };
        let mut w = Writer::new();
        for annotation in self.user_annotations(&node.facets)? {
            w.line(&annotation);
        }
        w.open(&format!("public class {name} extends {base}"));
        w.line("private static final long serialVersionUID = 1L;");
        write_fields(&mut w, &members);
        w.line("");
        w.line("/** The default constructor (§7.4.1, §7.2.4.3.1). */");
        w.open(&format!("public {name}()"));
        w.line("super();");
        write_initialisers(&mut w, &members);
        w.close();
        if !members.is_empty() {
            w.line("");
            w.line("/** The all values constructor (§7.4.1, §7.2.4.3.1). */");
            let params: Vec<String> = members
                .iter()
                .map(|m| format!("{} {}", m.jtype.text, m.param))
                .collect();
            w.open(&format!("public {name}({})", params.join(", ")));
            w.line("super();");
            for member in &members {
                w.line(&format!(
                    "this.{} = check_{}({});",
                    member.field, member.field, member.param
                ));
            }
            w.close();
        }
        write_accessors(&mut w, &members);
        w.close();
        Ok(w.finish())
    }

    // -- constants, §7.2.3 and §7.2.3.1 ---------------------------------------

    /// The constants declared directly in a module.
    pub(super) fn top_level_constants(
        &mut self,
        module: Id<ModuleNode>,
        constants: &[Id<zerodds_ast::ConstNode>],
    ) -> Result<(), Idl4JavaError> {
        let classes = self.constant_classes(module, None, constants)?;
        for (class, origin, body) in classes {
            self.file(module, &class, &origin, &body)?;
        }
        Ok(())
    }

    /// The classes the constants of one scope map to: one per constant
    /// (§7.2.3), or one container per name for the constants the
    /// alternative mapping holds (§7.2.3.1). As (class, origin, source).
    pub(super) fn constant_classes(
        &self,
        module: Id<ModuleNode>,
        scope: Option<&str>,
        constants: &[Id<zerodds_ast::ConstNode>],
    ) -> Result<Vec<(String, String, String)>, Idl4JavaError> {
        let mut out = Vec::new();
        let mut containers: Vec<(String, Vec<String>, String)> = Vec::new();
        for constant in constants {
            let node = self
                .ast
                .constant(*constant)
                .map_err(|_| Idl4JavaError::Unresolved {
                    what: "a constant".to_string(),
                    at: String::new(),
                })?;
            let settings = self.settings_in(module, scope, &node.facets);
            let at = node.name.clone();
            let jtype = self.java_type(&node.type_, &settings, &at)?;
            let value = self
                .ast
                .eval_const(node.expr)
                .map_err(|_| Idl4JavaError::Unevaluated {
                    what: "a constant's value".to_string(),
                    at: at.clone(),
                })?;
            let value = self.java_value(&value, &jtype, &at)?;
            let origin = self.origin(AnyId::Const(*constant, node.node_id));
            let annotations = self.user_annotations(&node.facets)?;
            match &settings.container {
                Some(container) => {
                    let field = settings.naming.constant_field(&node.name);
                    let mut line = String::new();
                    for annotation in &annotations {
                        line.push_str(annotation);
                        line.push('\n');
                    }
                    line.push_str(&format!(
                        "public final static {} {field} = {value};",
                        jtype.text
                    ));
                    match containers.iter_mut().find(|(name, _, _)| name == container) {
                        Some((_, lines, _)) => lines.push(line),
                        None => containers.push((container.clone(), vec![line], origin)),
                    }
                }
                None => {
                    let class = settings.naming.constant_class(&node.name);
                    let mut w = Writer::new();
                    for annotation in &annotations {
                        w.line(annotation);
                    }
                    w.open(&format!("public final class {class}"));
                    w.line(&format!(
                        "public final static {} value = {value};",
                        jtype.text
                    ));
                    w.line("");
                    w.open(&format!("private {class}()"));
                    w.close();
                    w.close();
                    out.push((class, origin, w.finish()));
                }
            }
        }
        for (container, lines, origin) in containers {
            let mut w = Writer::new();
            w.line("/** The constants of this scope (§7.2.3.1). */");
            w.open(&format!("public final class {container}"));
            for line in lines {
                w.nested(&line);
            }
            w.line("");
            w.open(&format!("private {container}()"));
            w.close();
            w.close();
            out.push((container, origin, w.finish()));
        }
        Ok(out)
    }
}

/// The values a discriminator type has.
enum Domain {
    /// An enumeration's values or a boolean's two.
    Values(Vec<i128>),
    /// An integer or character range.
    Range(i128, i128),
}

impl Domain {
    /// The default value of the type: 0, false, the first enumerator.
    fn default_value(&self) -> i128 {
        match self {
            Self::Values(values) => values.first().copied().unwrap_or(0),
            Self::Range(low, high) => 0.clamp(*low, *high),
        }
    }

    /// "the first available default value starting from a 0 index of the
    /// discriminant type".
    fn first_free(&self, taken: &[i128]) -> Option<i128> {
        match self {
            Self::Values(values) => values.iter().copied().find(|v| !taken.contains(v)),
            Self::Range(low, high) => {
                let start = 0.clamp(*low, *high);
                (start..=*high)
                    .take(taken.len() + 1)
                    .find(|v| !taken.contains(v))
                    .or_else(|| (*low..start).find(|v| !taken.contains(v)))
            }
        }
    }

    /// Whether the labels name every value.
    fn covers(&self, taken: &[i128]) -> bool {
        match self {
            Self::Values(values) => values.iter().all(|v| taken.contains(v)),
            Self::Range(low, high) => {
                let size = high - low + 1;
                let distinct: std::collections::BTreeSet<&i128> = taken.iter().collect();
                i128::try_from(distinct.len()).is_ok_and(|n| n >= size)
            }
        }
    }
}

/// The type a member is declared in.
fn owner_type(generator: &Gen<'_>, member: Id<MemberNode>) -> Option<Id<TypeNode>> {
    match generator.ast.member(member).ok()?.parent {
        AnyId::Type(id, _) => Some(id),
        _ => None,
    }
}

/// Fields, one per member.
fn write_fields(w: &mut Writer, members: &[Member]) {
    for member in members {
        w.line("");
        for annotation in &member.annotations {
            w.line(annotation);
        }
        w.line(&format!("private {} {};", member.jtype.text, member.field));
    }
}

/// The default constructor's assignments.
fn write_initialisers(w: &mut Writer, members: &[Member]) {
    for member in members {
        if let Some(initial) = &member.initial {
            w.line(&format!("this.{} = {initial};", member.field));
        }
    }
}

/// Accessors, modifiers and the check each modifier runs.
fn write_accessors(w: &mut Writer, members: &[Member]) {
    for member in members {
        let jt = &member.jtype.text;
        w.line("");
        w.open(&format!("public {jt} {}()", member.getter));
        w.line(&format!("return this.{};", member.field));
        w.close();
        w.line("");
        w.open(&format!(
            "public void {}({jt} {})",
            member.setter, member.param
        ));
        w.line(&format!(
            "this.{} = check_{}({});",
            member.field, member.field, member.param
        ));
        w.close();
        w.line("");
        w.open(&format!(
            "private static {jt} check_{}({jt} {})",
            member.field, member.param
        ));
        for check in &member.checks {
            w.line(check);
        }
        w.line(&format!("return {};", member.param));
        w.close();
    }
}

/// Two members whose Java names are one (§7.1.1.2).
fn check_member_collisions(members: &[Member], owner: &str) -> Result<(), Idl4JavaError> {
    for (index, member) in members.iter().enumerate() {
        for other in &members[..index] {
            if other.field == member.field || other.getter == member.getter {
                return Err(Idl4JavaError::Collision {
                    name: member.getter.clone(),
                    first: format!("{owner}.{}", other.idl),
                    second: format!("{owner}.{}", member.idl),
                });
            }
        }
    }
    Ok(())
}

/// The text of a constant value, for messages and for a decimal literal.
/// A constant value as the text a message shows.
#[must_use]
pub fn value_text(value: &ConstValue) -> String {
    match value {
        ConstValue::Boolean(flag) => flag.to_string(),
        ConstValue::Integer(number) => number.to_string(),
        ConstValue::Float { mantissa, exponent } => match exponent {
            Some(exponent) => format!("{mantissa}e{exponent}"),
            None => mantissa.clone(),
        },
        ConstValue::Fixed { digits, scale } => {
            let scale = usize::from(*scale);
            let (sign, digits) = digits
                .strip_prefix('-')
                .map_or(("", digits.as_str()), |rest| ("-", rest));
            if scale == 0 {
                format!("{sign}{digits}")
            } else if digits.len() > scale {
                let (whole, fraction) = digits.split_at(digits.len() - scale);
                format!("{sign}{whole}.{fraction}")
            } else {
                format!("{sign}0.{}{digits}", "0".repeat(scale - digits.len()))
            }
        }
        ConstValue::Char(ch) => format!("'{ch}'"),
        ConstValue::WChar(unit) => format!("U+{unit:04X}"),
        ConstValue::String(text) => format!("\"{text}\""),
        ConstValue::WString(_) => "a wide string".to_string(),
        _ => "a value".to_string(),
    }
}

/// An integer of a constant value.
fn integer_of(value: &ConstValue, at: &str) -> Result<i128, Idl4JavaError> {
    match value {
        ConstValue::Integer(number) => Ok(*number),
        ConstValue::Char(ch) => Ok(i128::from(u32::from(*ch))),
        ConstValue::WChar(unit) => Ok(i128::from(*unit)),
        _ => Err(Idl4JavaError::Unevaluated {
            what: format!("an integer bound, not {}", value_text(value)),
            at: at.to_string(),
        }),
    }
}

/// A Java integer literal of `bits` width for `value`, which may be the
/// value of an unsigned type: then the literal is its bit pattern
/// (Table 7.2).
pub(super) fn integer_literal(value: i128, bits: u8) -> Option<String> {
    match bits {
        8 => {
            if (-128..=127).contains(&value) {
                Some(format!("(byte) {value}"))
            } else if (128..=255).contains(&value) {
                Some(format!("(byte) 0x{value:02X}"))
            } else {
                None
            }
        }
        16 => {
            if (-32_768..=32_767).contains(&value) {
                Some(format!("(short) {value}"))
            } else if (32_768..=65_535).contains(&value) {
                Some(format!("(short) 0x{value:04X}"))
            } else {
                None
            }
        }
        32 => {
            if (i128::from(i32::MIN)..=i128::from(i32::MAX)).contains(&value) {
                Some(format!("{value}"))
            } else if (i128::from(i32::MAX) + 1..=i128::from(u32::MAX)).contains(&value) {
                Some(format!("0x{value:08X}"))
            } else {
                None
            }
        }
        _ => {
            if (i128::from(i64::MIN)..=i128::from(i64::MAX)).contains(&value) {
                Some(format!("{value}L"))
            } else if (i128::from(i64::MAX) + 1..=i128::from(u64::MAX)).contains(&value) {
                Some(format!("0x{value:016X}L"))
            } else {
                None
            }
        }
    }
}

/// The primitive spelling of a primitive type, for telling a boxed
/// member from a plain one.
fn jtype_primitive_text(jtype: &JType) -> &'static str {
    match &jtype.shape {
        Shape::Boolean => "boolean",
        Shape::Char => "char",
        Shape::Integer { bits: 8, .. } => "byte",
        Shape::Integer { bits: 16, .. } => "short",
        Shape::Integer { bits: 32, .. } => "int",
        Shape::Integer { .. } => "long",
        Shape::Float => "float",
        Shape::Double => "double",
        _ => "",
    }
}

/// Zero, boxed: the value an `@external` member of a basic type starts at.
fn boxed_zero(jtype: &JType) -> String {
    match &jtype.shape {
        Shape::Boolean => "Boolean.FALSE".to_string(),
        Shape::Char => "Character.valueOf((char) 0)".to_string(),
        Shape::Integer { bits: 8, .. } => "Byte.valueOf((byte) 0)".to_string(),
        Shape::Integer { bits: 16, .. } => "Short.valueOf((short) 0)".to_string(),
        Shape::Integer { bits: 32, .. } => "Integer.valueOf(0)".to_string(),
        Shape::Integer { .. } => "Long.valueOf(0L)".to_string(),
        Shape::Float => "Float.valueOf(0f)".to_string(),
        Shape::Double => "Double.valueOf(0d)".to_string(),
        _ => "null".to_string(),
    }
}

/// The value a union starts its selected member at when the member has no
/// default constructor: the primitive's zero, boxed.
fn default_primitive_boxed(jtype: &JType) -> String {
    match &jtype.shape {
        Shape::Reference => "null".to_string(),
        _ => boxed_zero(jtype),
    }
}

/// `__value` as the member's type.
fn unbox(jtype: &JType, value: &str) -> String {
    format!("({}) {value}", jtype.boxed)
}

/// The check a bit field's modifier runs: the value has to fit the
/// field's bits.
fn bitfield_check(jtype: &JType, bits: u16, var: &str, at: &str) -> String {
    match &jtype.shape {
        Shape::Integer {
            bits: width,
            unsigned,
        } if u16::from(*width) > bits => {
            let mask = (1u128 << bits) - 1;
            let (cast, suffix) = match width {
                64 => ("", "L"),
                _ => ("(long) ", "L"),
            };
            let value = if *unsigned {
                match width {
                    8 => format!("Byte.toUnsignedLong({var})"),
                    16 => format!("Short.toUnsignedLong({var})"),
                    32 => format!("Integer.toUnsignedLong({var})"),
                    _ => var.to_string(),
                }
            } else {
                format!("{cast}{var}")
            };
            if *unsigned || *width == 64 {
                format!(
                    "if (({value} & ~0x{mask:X}{suffix}) != 0) throw new IllegalArgumentException(\"{at}: \" + {var} + \" does not fit {bits} bits\");"
                )
            } else {
                let low = -(1i128 << (bits - 1));
                let high = (1i128 << (bits - 1)) - 1;
                format!(
                    "if ({var} < {low} || {var} > {high}) throw new IllegalArgumentException(\"{at}: \" + {var} + \" does not fit {bits} bits\");"
                )
            }
        }
        _ => String::new(),
    }
}

/// The key or value type argument of a `java.util.Map<K, V>` spelling.
fn key_type_of(map: &str, key: bool) -> String {
    let inner = map
        .strip_prefix("java.util.Map<")
        .and_then(|rest| rest.strip_suffix('>'))
        .unwrap_or_default();
    // The arguments are split at the top-level comma.
    let mut depth = 0usize;
    for (index, ch) in inner.char_indices() {
        match ch {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                return if key {
                    inner[..index].trim().to_string()
                } else {
                    inner[index + 1..].trim().to_string()
                };
            }
            _ => {}
        }
    }
    inner.to_string()
}

/// `{ x, x, … }` nested to the array's dimensions.
///
/// zerodds-lint: recursion-depth 8 — one frame per array dimension, as
/// `read_array_dimension` in `zerodds-wire-codec`; `dims` loses its head
/// at each frame, so the depth is the declarator's dimension count.
fn nested_initialiser(element: &str, dims: &[u64], out: &mut String) {
    let Some((first, rest)) = dims.split_first() else {
        out.push_str(element);
        return;
    };
    out.push_str("{ ");
    for index in 0..*first {
        if index > 0 {
            out.push_str(", ");
        }
        nested_initialiser(element, rest, out);
    }
    out.push_str(" }");
}
