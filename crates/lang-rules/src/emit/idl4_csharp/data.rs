// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The data types: structures (§7.2.4.3.1, §7.14.1, §8.1.3), unions
//! (§7.2.4.3.2, §7.14.2), enumerations (§7.2.4.3.3), bit sets
//! (§7.14.3.2), bit masks (§7.14.3.3), exceptions (§7.4.1, A.1.1) and
//! constants (§7.2.3).

use super::text::{Writer, char_literal, string_literal, string_literal_units};
use super::types::{CType, Range, Shape};
use super::{Gen, Idl4CsharpError, Platform, Settings};
use crate::verbatim;
use zerodds_ast::{
    AnyId, CaseLabel, ConstValue, MemberNode, ModuleNode, PrimitiveKind, TypeKind, TypeNode,
};
use zerodds_ast_core::{Id, Ref};

/// One member as a class holds it.
pub(crate) struct Member {
    /// The IDL name, for messages.
    pub idl: String,
    /// The property.
    pub property: String,
    /// The private field behind it.
    pub field: String,
    /// The parameter of the all values constructor.
    pub param: String,
    /// The C# type, `System.Nullable<T>` for an `@optional` or `@external`
    /// value type, `IEnumerable<T>` for an `@external` sequence.
    pub ctype: CType,
    /// Whether the property has no setter: a sequence or a map without
    /// `@external` (§7.2.4.3.1).
    pub read_only: bool,
    /// Whether copies of it are shallow (`@external`, Table 7.8).
    pub shallow: bool,
    /// The value the default constructor gives it, when C#'s own default
    /// is not it.
    pub initial: Option<String>,
    /// Where it is, for messages.
    pub at: String,
    /// The attributes on it: `[Omg.Types.Key]`, `[Omg.Types.Optional]`,
    /// `[Omg.Types.Unit]` and the user annotations (§7.16.2).
    pub attributes: Vec<String>,
    /// Whether it is `@optional`.
    pub optional: bool,
}

/// The members of `System.Exception` a property of an exception would
/// hide (§7.1.1: a collision with a name the mapping introduces takes a
/// leading underscore).
const EXCEPTION_MEMBERS: [&str; 10] = [
    "Message",
    "Data",
    "Source",
    "HelpLink",
    "StackTrace",
    "InnerException",
    "TargetSite",
    "HResult",
    "GetType",
    "GetBaseException",
];

impl Gen<'_> {
    /// A type declared directly in a module: its file, or none for a
    /// `typedef` (§7.2.4.6) and a forward declaration.
    pub(super) fn top_level_type(&mut self, id: Id<TypeNode>) -> Result<(), Idl4CsharpError> {
        let node = self.ast.type_(id).map_err(|_| unresolved("a type", ""))?;
        if node.completeness == zerodds_ast::Completeness::Forward {
            return Ok(());
        }
        let module = node.module;
        let origin = self.origin(AnyId::Type(id, node.node_id));
        for (name, body) in self.type_declaration(id)? {
            let slots = verbatim::declaration_slots(self.ast, id, verbatim::aliases::CSHARP);
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
            self.file(module, &name, &origin, &text)?;
        }
        Ok(())
    }

    /// The declarations one type maps to, as (name, source); none for a
    /// `typedef`.
    pub(super) fn type_declaration(
        &self,
        id: Id<TypeNode>,
    ) -> Result<Vec<(String, String)>, Idl4CsharpError> {
        let node = self.ast.type_(id).map_err(|_| unresolved("a type", ""))?;
        let settings = self.settings_of_type(id);
        let name = settings.naming.type_name(&node.name);
        Ok(match &node.kind {
            TypeKind::Structure { .. } => vec![(name.clone(), self.structure(id, &name)?)],
            TypeKind::Union { .. } => vec![(name.clone(), self.union(id, &name)?)],
            TypeKind::Enumeration { .. } => vec![(name.clone(), self.enumeration(id, &name)?)],
            TypeKind::Bitset { .. } => vec![(name.clone(), self.bitset(id, &name)?)],
            TypeKind::Bitmask { .. } => {
                let flags = settings.naming.bitmask_name(&node.name);
                vec![(flags.clone(), self.bitmask(id, &flags)?)]
            }
            TypeKind::Valuetype { .. } | TypeKind::ValueBox { .. } => {
                vec![(name.clone(), self.valuetype(id, &name)?)]
            }
            // §7.2.4.6: "the declaration of types using typedef in IDL
            // shall not result in the creation of any C# type".
            TypeKind::Alias { .. } | TypeKind::Native => Vec::new(),
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
        exception: bool,
    ) -> Result<Member, Idl4CsharpError> {
        let node = self
            .ast
            .member(member)
            .map_err(|_| unresolved("a member", owner))?;
        let at = format!("{owner}.{}", node.name);
        let mut ctype = self.cs_type(&node.type_, settings, &at)?;
        let xtypes = node.facets.xtypes.as_ref();
        if let Some(range) = xtypes.and_then(|x| x.range) {
            ctype.range = Some(self.merge_range(ctype.range.take(), range, &at)?);
        }
        let optional = xtypes.is_some_and(|x| x.is_optional);
        let external = xtypes.is_some_and(|x| x.is_external);
        let struct_owner = self.owner_is_struct(member);
        let collection = matches!(ctype.shape, Shape::Sequence { .. } | Shape::Map { .. });
        let element_text = match &ctype.shape {
            Shape::Sequence { element, .. } => Some(element.text.clone()),
            _ => None,
        };
        if optional || external {
            ctype = ctype.nullable();
        }
        // Table 7.8: "Properties created as a result of mapping IDL
        // sequences annotated with @external shall map to the C#
        // System.Collections.Generic.IEnumerable<T> interface".
        if external && let Some(element) = &element_text {
            ctype.text = format!("global::System.Collections.Generic.IEnumerable<{element}>");
        }
        let initial = if optional {
            None
        } else if let Some(value) = xtypes.and_then(|x| x.default_value) {
            let value = self
                .ast
                .eval_const(value)
                .map_err(|_| Idl4CsharpError::Unevaluated {
                    what: "the value of @default".to_string(),
                    at: at.clone(),
                })?;
            Some(self.cs_value(&value, &ctype, &at)?)
        } else if external && self.reaches(node.type_.resolved(), owner_type(self, member)) {
            None
        } else {
            self.initial(&ctype, &at)?
        };
        let mut property = settings.naming.member(&node.name);
        if exception && EXCEPTION_MEMBERS.contains(&property.as_str()) {
            property = format!("_{property}");
        }
        let mut attributes = Vec::new();
        if xtypes.is_some_and(|x| x.is_key) {
            attributes.push("[global::Omg.Types.Key]".to_string());
        }
        if optional {
            attributes.push("[global::Omg.Types.Optional]".to_string());
        }
        if let Some(unit) = xtypes.and_then(|x| x.unit.as_ref()) {
            attributes.push(format!(
                "[global::Omg.Types.Unit({})]",
                string_literal(unit)
            ));
        }
        attributes.extend(self.member_annotations(&node.facets, &node.type_)?);
        Ok(Member {
            idl: node.name.clone(),
            field: format!("__{}", node.name.trim_start_matches('_')),
            param: settings.naming.parameter(&node.name),
            property,
            ctype,
            // §8.1.3: a C# struct has every member with a setter, and
            // every copy deep.
            read_only: collection && !external && !struct_owner,
            shallow: external && !struct_owner,
            initial,
            at,
            attributes,
            optional,
        })
    }

    fn owner_is_struct(&self, member: Id<MemberNode>) -> bool {
        owner_type(self, member).is_some_and(|owner| {
            self.ast
                .type_(owner)
                .is_ok_and(|node| matches!(node.kind, TypeKind::Structure { .. }))
                && self.struct_type_of(owner)
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

    /// The value a default constructor gives a member, §7.2.4.3.1:
    /// primitives as C# initialises them, strings `string.Empty`, arrays at
    /// their size with each element default-constructed, sequences empty,
    /// everything else through its default constructor.
    pub(super) fn initial(
        &self,
        ctype: &CType,
        at: &str,
    ) -> Result<Option<String>, Idl4CsharpError> {
        if ctype.text.starts_with("global::System.Nullable<") {
            return Ok(None);
        }
        Ok(match &ctype.shape {
            Shape::Bool
            | Shape::Char { .. }
            | Shape::Integer { .. }
            | Shape::Float
            | Shape::Double
            | Shape::LongDouble
            | Shape::Fixed { .. }
            | Shape::Reference
            | Shape::Struct => None,
            Shape::Str { .. } => Some("string.Empty".to_string()),
            Shape::Sequence { .. } => Some(new_sequence(ctype)),
            Shape::Map { .. } => Some(new_map(ctype)),
            Shape::Array { element, dims } => Some(self.array_initial(element, dims, at)?),
            // A value type is an abstract class (§7.6): nothing to construct.
            Shape::Class if self.is_value_class(&ctype.text) => None,
            Shape::Class => Some(format!("new {}()", ctype.text)),
            Shape::Enum { default } => Some(default.clone()),
            Shape::BitArray { bound } => {
                Some(format!("new global::System.Collections.BitArray({bound})"))
            }
            Shape::Any => Some(format!("new {}()", ctype.text)),
        })
    }

    /// An array at its declared size, each element default-constructed.
    fn array_initial(
        &self,
        element: &CType,
        dims: &[u64],
        at: &str,
    ) -> Result<String, Idl4CsharpError> {
        let sizes: Vec<String> = dims.iter().map(u64::to_string).collect();
        let created = format!("new {}[{}]", element.text, sizes.join(", "));
        match self.initial(element, at)? {
            None => Ok(created),
            Some(initial) => Ok(format!(
                "global::Omg.Types.Arrays.Fill({created}, () => {initial})"
            )),
        }
    }

    /// The statements a setter runs on `var` before it stores it.
    pub(super) fn checks(
        &self,
        ctype: &CType,
        var: &str,
        at: &str,
        depth: usize,
    ) -> Result<Vec<String>, Idl4CsharpError> {
        let mut out = Vec::new();
        match &ctype.shape {
            Shape::Str { bound, wide } => {
                if let Some(bound) = bound {
                    out.push(format!(
                        "if ({var} != null && {var}.Length > {bound}) throw new \
                         global::System.ArgumentOutOfRangeException(\"{at}\", {var}.Length, \"the \
                         bound is {bound}\");"
                    ));
                }
                if !*wide {
                    out.push(format!(
                        "if ({var} != null) foreach (char c{depth} in {var}) if (c{depth} > '\\u00ff') \
                         throw new global::System.ArgumentOutOfRangeException(\"{at}\", c{depth}, \
                         \"no IDL char\");"
                    ));
                }
            }
            Shape::Char { wide: false } => out.push(format!(
                "if ({var} > '\\u00ff') throw new global::System.ArgumentOutOfRangeException(\"{at}\", \
                 {var}, \"no IDL char\");"
            )),
            Shape::Sequence { element, bound } => {
                if let Some(bound) = bound {
                    out.push(format!(
                        "if ({var} != null && global::System.Linq.Enumerable.Count({var}) > {bound}) \
                         throw new global::System.ArgumentOutOfRangeException(\"{at}\", \"the bound \
                         is {bound}\");"
                    ));
                }
                let element_var = format!("e{depth}");
                let inner = self.checks(element, &element_var, at, depth + 1)?;
                if !inner.is_empty() {
                    out.push(format!(
                        "if ({var} != null) foreach ({} {element_var} in {var}) {{ {} }}",
                        element.text,
                        inner.join(" ")
                    ));
                }
            }
            Shape::Array { element, dims } => {
                // "The bounds for the array shall be checked by the setter
                // of the corresponding property and a
                // System.ArgumentOutOfRangeException shall be raised"
                // (§7.2.4.4).
                for (rank, dim) in dims.iter().enumerate() {
                    out.push(format!(
                        "if ({var} == null || {var}.GetLength({rank}) != {dim}) throw new \
                         global::System.ArgumentOutOfRangeException(\"{at}\", \"an array of {dim} \
                         elements in dimension {rank}\");"
                    ));
                }
                let element_var = format!("e{depth}");
                let inner = self.checks(element, &element_var, at, depth + 1)?;
                if !inner.is_empty() {
                    out.push(format!(
                        "foreach ({} {element_var} in {var}) {{ {} }}",
                        element.text,
                        inner.join(" ")
                    ));
                }
            }
            Shape::Map { key, value, bound } => {
                if let Some(bound) = bound {
                    out.push(format!(
                        "if ({var} != null && {var}.Count > {bound}) throw new \
                         global::System.ArgumentOutOfRangeException(\"{at}\", {var}.Count, \"the \
                         bound is {bound}\");"
                    ));
                }
                let key_checks = self.checks(key, &format!("m{depth}.Key"), at, depth + 1)?;
                let value_checks = self.checks(value, &format!("m{depth}.Value"), at, depth + 1)?;
                if !key_checks.is_empty() || !value_checks.is_empty() {
                    out.push(format!(
                        "if ({var} != null) foreach (var m{depth} in {var}) {{ {} {} }}",
                        key_checks.join(" "),
                        value_checks.join(" ")
                    ));
                }
            }
            Shape::Fixed { digits, scale } => {
                // §7.2.4.2.4: "Range checking shall raise a
                // System.ArithmeticException exception, or a derived
                // exception, if necessary."
                let nullable = ctype.text.starts_with("global::System.Nullable<");
                let value = if nullable {
                    format!("{var}.Value")
                } else {
                    var.to_string()
                };
                let check = format!(
                    "global::Omg.Types.Fixed.Check({value}, {digits}, {scale}, \"{at}\");"
                );
                out.push(if nullable {
                    format!("if ({var}.HasValue) {check}")
                } else {
                    check
                });
            }
            Shape::BitArray { bound } => out.push(format!(
                "if ({var} != null && {var}.Length > {bound}) throw new \
                 global::System.ArgumentOutOfRangeException(\"{at}\", {var}.Length, \"the bit \
                 bound is {bound}\");"
            )),
            _ => {}
        }
        if let Some(range) = &ctype.range {
            out.extend(self.range_checks(ctype, range, var, at)?);
        }
        Ok(out)
    }

    /// `@range`, `@min`, `@max`: "shall throw a
    /// System.ArgumentOutOfRangeException if they are set to a value out
    /// of the corresponding range" (Table 7.7).
    fn range_checks(
        &self,
        ctype: &CType,
        range: &Range,
        var: &str,
        at: &str,
    ) -> Result<Vec<String>, Idl4CsharpError> {
        let mut out = Vec::new();
        let nullable = ctype.text.starts_with("global::System.Nullable<");
        for (bound, op, name) in [(&range.min, "<", "min"), (&range.max, ">", "max")] {
            let Some(value) = bound else {
                continue;
            };
            let literal = self.number_literal(ctype, value, at)?;
            let guard = if nullable {
                format!("{var}.HasValue && {var}.Value")
            } else {
                var.to_string()
            };
            out.push(format!(
                "if ({guard} {op} {literal}) throw new global::System.ArgumentOutOfRangeException(\"{at}\", \
                 {var}, \"outside the {name} {}\");",
                value_text(value)
            ));
        }
        Ok(out)
    }

    /// A number of the type `ctype` as a C# literal, for a comparison.
    fn number_literal(
        &self,
        ctype: &CType,
        value: &ConstValue,
        at: &str,
    ) -> Result<String, Idl4CsharpError> {
        let unfit = || Idl4CsharpError::NoCsharpForm {
            construct: "@range, @min or @max on a type that is no number".to_string(),
            at: at.to_string(),
        };
        Ok(match &ctype.shape {
            Shape::Integer { .. } => integer_of(value, at)?.to_string(),
            Shape::Char { .. } => format!("(char){}", integer_of(value, at)?),
            Shape::Float => format!("{}f", value_text(value)),
            Shape::Double => format!("{}d", value_text(value)),
            Shape::LongDouble | Shape::Fixed { .. } => format!("{}m", value_text(value)),
            _ => return Err(unfit()),
        })
    }

    /// A constant value as a C# expression of the type `ctype`.
    pub(super) fn cs_value(
        &self,
        value: &ConstValue,
        ctype: &CType,
        at: &str,
    ) -> Result<String, Idl4CsharpError> {
        let unfit = || Idl4CsharpError::Unevaluated {
            what: format!(
                "a value of {} for the C# type `{}`",
                value_text(value),
                ctype.text
            ),
            at: at.to_string(),
        };
        Ok(match (&ctype.shape, value) {
            (Shape::Bool, ConstValue::Boolean(flag)) => flag.to_string(),
            (Shape::Bool, ConstValue::Integer(number)) => (*number != 0).to_string(),
            (Shape::Char { .. }, ConstValue::Char(ch)) => {
                char_literal(u16::try_from(u32::from(*ch)).map_err(|_| unfit())?)
            }
            (Shape::Char { .. }, ConstValue::WChar(unit)) => {
                char_literal(u16::try_from(*unit).map_err(|_| unfit())?)
            }
            (Shape::Char { .. }, ConstValue::Integer(number)) => {
                char_literal(u16::try_from(*number).map_err(|_| unfit())?)
            }
            (Shape::Integer { bits, signed }, ConstValue::Integer(number)) => {
                integer_literal(*number, *bits, *signed).ok_or_else(unfit)?
            }
            (Shape::Integer { bits, signed }, ConstValue::Char(ch)) => {
                integer_literal(i128::from(u32::from(*ch)), *bits, *signed).ok_or_else(unfit)?
            }
            (Shape::Float, ConstValue::Integer(_) | ConstValue::Float { .. }) => {
                format!("{}f", value_text(value))
            }
            (Shape::Double, ConstValue::Integer(_) | ConstValue::Float { .. }) => {
                format!("{}d", value_text(value))
            }
            (
                Shape::LongDouble | Shape::Fixed { .. },
                ConstValue::Integer(_) | ConstValue::Float { .. } | ConstValue::Fixed { .. },
            ) => {
                let text = value_text(value);
                // `decimal` holds 28 significant digits; a value with more
                // has no C# literal.
                let digits = text.chars().filter(char::is_ascii_digit).count();
                if digits > 28 && !text.contains('e') {
                    return Err(Idl4CsharpError::NoCsharpForm {
                        construct: format!("the value {text}, which `decimal` does not hold"),
                        at: at.to_string(),
                    });
                }
                format!("{text}m")
            }
            (Shape::Str { .. }, ConstValue::String(text)) => string_literal(text),
            (Shape::Str { .. }, ConstValue::WString(units)) => {
                let mut code_units = Vec::new();
                for unit in units {
                    match char::from_u32(*unit) {
                        Some(ch) => {
                            let mut buffer = [0u16; 2];
                            code_units.extend_from_slice(ch.encode_utf16(&mut buffer));
                        }
                        None => code_units.push(u16::try_from(*unit).map_err(|_| unfit())?),
                    }
                }
                string_literal_units(&code_units)
            }
            (Shape::Enum { .. }, ConstValue::Integer(number)) => {
                let constant = self
                    .enumerator_named(&ctype.text, *number)
                    .ok_or_else(unfit)?;
                format!("{}.{constant}", ctype.text)
            }
            _ => return Err(unfit()),
        })
    }

    /// The C# enumerator of the enumeration whose C# name is `qualified`
    /// with the value `value`.
    pub(super) fn enumerator_named(&self, qualified: &str, value: i128) -> Option<String> {
        for (id, node) in self.ast.types() {
            if !matches!(node.kind, TypeKind::Enumeration { .. }) {
                continue;
            }
            if self.qualified(id).ok().as_deref() != Some(qualified) {
                continue;
            }
            let settings = self.settings_of_type(id);
            for enumerator in self.ast.enumerators_of(id) {
                if self.ast.effective_enumerator_value(*enumerator) == Some(value) {
                    let name = self.ast.enumerator(*enumerator).ok()?.name.clone();
                    return Some(settings.naming.member(&name));
                }
            }
        }
        None
    }

    // -- structures, §7.2.4.3.1, §7.14.1 and §8.1.3 ---------------------------

    fn structure(&self, id: Id<TypeNode>, name: &str) -> Result<String, Idl4CsharpError> {
        let node = self
            .ast
            .type_(id)
            .map_err(|_| unresolved("a structure", name))?;
        let settings = self.settings_of_type(id);
        let TypeKind::Structure { base, members } = &node.kind else {
            return Err(unresolved("a structure", name));
        };
        let base = match base.as_ref().and_then(Ref::resolved) {
            Some(base) => Some(self.through_aliases(base, name)?),
            None => None,
        };
        let is_struct = settings.struct_type;
        if is_struct && base.is_some() {
            return Err(Idl4CsharpError::NoCsharpForm {
                construct: "a structure with a base under struct_type = \"struct\" (a C# struct \
                            has no base type)"
                    .to_string(),
                at: name.to_string(),
            });
        }
        let base_name = match base {
            Some(base) => Some(self.qualified(base)?),
            None => None,
        };
        let mut own = Vec::new();
        for member in members {
            own.push(self.member(*member, &settings, name, false)?);
        }
        check_member_collisions(&own, name)?;
        let slots = verbatim::declaration_slots(self.ast, id, verbatim::aliases::CSHARP);
        let mut w = Writer::new();
        for text in &slots.before {
            w.verbatim(text);
        }
        for annotation in self.user_annotations(&node.facets)? {
            w.line(&annotation);
        }
        let kind = if is_struct { "struct" } else { "class" };
        let bases = match &base_name {
            Some(base) => format!("{base}, global::System.IEquatable<{name}>"),
            None => format!("global::System.IEquatable<{name}>"),
        };
        w.open(&format!("public {kind} {name} : {bases}"));
        for text in &slots.begin {
            w.verbatim(text);
        }
        write_fields(&mut w, &own);
        // The default constructor.
        w.line("");
        w.line("/// <summary>The default constructor (§7.2.4.3.1).</summary>");
        w.open(&format!("public {name}()"));
        write_initialisers(&mut w, &own, is_struct);
        w.close();
        // The copy constructor.
        w.line("");
        w.line("/// <summary>A deep copy of <paramref name=\"other\"/> (§7.2.4.3.1).</summary>");
        let chain = if base_name.is_some() {
            " : base(other)"
        } else {
            ""
        };
        w.open(&format!("public {name}({name} other){chain}"));
        if !is_struct {
            w.line(
                "if (other is null) throw new global::System.ArgumentNullException(nameof(other));",
            );
        }
        for member in &own {
            let copy = if member.shallow {
                format!("other.{}", member.field)
            } else {
                self.copy(&member.ctype, &format!("other.{}", member.field), 0)
            };
            w.line(&format!("this.{} = {copy};", member.field));
        }
        w.close();
        // The all values constructor.
        let mut params: Vec<String> = Vec::new();
        if let Some(base) = &base_name {
            params.push(format!("{base} parentObject"));
        }
        for member in &own {
            let attribute = if member.optional {
                "[global::Omg.Types.Optional] "
            } else {
                ""
            };
            params.push(format!("{attribute}{} {}", member.ctype.text, member.param));
        }
        let chain = if base_name.is_some() {
            " : base(parentObject)"
        } else {
            ""
        };
        w.line("");
        w.line("/// <summary>The all values constructor (§7.2.4.3.1, §7.14.1).</summary>");
        w.open(&format!("public {name}({}){chain}", params.join(", ")));
        for member in &own {
            for check in self.checks(&member.ctype, &member.param, &member.at, 0)? {
                w.line(&check);
            }
            // "The constructor shall perform shallow assignments of
            // reference types and deep copies of value types."
            w.line(&format!("this.{} = {};", member.field, member.param));
        }
        w.close();
        self.write_properties(&mut w, &own)?;
        write_equality(&mut w, name, &own, base_name.is_some(), is_struct);
        for text in &slots.end {
            w.verbatim(text);
        }
        w.close();
        for text in &slots.after {
            w.verbatim(text);
        }
        Ok(w.finish())
    }

    /// One property per member: a getter, and a setter that checks
    /// what it is given, except for a read-only one.
    pub(super) fn write_properties(
        &self,
        w: &mut Writer,
        members: &[Member],
    ) -> Result<(), Idl4CsharpError> {
        for member in members {
            w.line("");
            for attribute in &member.attributes {
                w.line(attribute);
            }
            w.open(&format!("public {} {}", member.ctype.text, member.property));
            w.line(&format!("get {{ return this.{}; }}", member.field));
            if !member.read_only {
                w.open("set");
                for check in self.checks(&member.ctype, "value", &member.at, 0)? {
                    w.line(&check);
                }
                w.line(&format!("this.{} = value;", member.field));
                w.close();
            }
            w.close();
        }
        Ok(())
    }

    /// A deep copy of `expr`, of the type `ctype`.
    pub(super) fn copy(&self, ctype: &CType, expr: &str, depth: usize) -> String {
        let element = format!("e{depth}");
        match &ctype.shape {
            Shape::Class if !self.is_value_class(&ctype.text) => {
                format!("({expr} is null ? null : new {}({expr}))", ctype.text)
            }
            Shape::Sequence { element: of, .. } => format!(
                "global::Omg.Types.Sequences.Copy({expr}, {element} => {})",
                self.copy(of, &element, depth + 1)
            ),
            Shape::Array { element: of, .. } => format!(
                "global::Omg.Types.Arrays.Copy({expr}, ({} {element}) => {})",
                of.text,
                self.copy(of, &element, depth + 1)
            ),
            Shape::Map { key, value, .. } => {
                let k = format!("k{depth}");
                let v = format!("v{depth}");
                format!(
                    "global::Omg.Types.Maps.Copy({expr}, {k} => {}, {v} => {})",
                    self.copy(key, &k, depth + 1),
                    self.copy(value, &v, depth + 1)
                )
            }
            Shape::BitArray { .. } => {
                format!("({expr} is null ? null : new global::System.Collections.BitArray({expr}))")
            }
            Shape::Any => format!("({expr} is null ? null : new {}({expr}))", ctype.text),
            _ => expr.to_string(),
        }
    }

    /// Whether a class type is a value type's abstract class, which has no
    /// copy constructor to call: a value type is copied by reference.
    fn is_value_class(&self, text: &str) -> bool {
        self.ast.types().any(|(id, node)| {
            matches!(
                node.kind,
                TypeKind::Valuetype { .. } | TypeKind::ValueBox { .. }
            ) && self.qualified(id).ok().as_deref() == Some(text)
        })
    }

    /// The type an alias chain ends at.
    pub(super) fn through_aliases(
        &self,
        id: Id<TypeNode>,
        at: &str,
    ) -> Result<Id<TypeNode>, Idl4CsharpError> {
        let mut current = id;
        for _ in 0..super::MAX_DEPTH {
            match self.ast.type_(current).map(|node| &node.kind) {
                Ok(TypeKind::Alias { aliased }) => {
                    current = aliased
                        .resolved()
                        .ok_or_else(|| unresolved("a typedef", at))?;
                }
                Ok(_) => return Ok(current),
                Err(_) => break,
            }
        }
        Err(unresolved("a chain of typedefs", at))
    }

    // -- unions, §7.2.4.3.2 and §7.14.2 -------------------------------------

    #[allow(clippy::too_many_lines)]
    fn union(&self, id: Id<TypeNode>, name: &str) -> Result<String, Idl4CsharpError> {
        let node = self
            .ast
            .type_(id)
            .map_err(|_| unresolved("a union", name))?;
        let settings = self.settings_of_type(id);
        let TypeKind::Union {
            discriminator,
            cases,
            default_case,
        } = &node.kind
        else {
            return Err(unresolved("a union", name));
        };
        let disc_type = self.cs_type(discriminator, &settings, name)?;
        let disc_id = discriminator
            .resolved()
            .map(|d| self.through_aliases(d, name))
            .transpose()?
            .ok_or_else(|| unresolved("a discriminator", name))?;
        let domain = self.discriminator_domain(disc_id, name)?;
        struct Case {
            member: Member,
            labels: Vec<i128>,
            default: bool,
        }
        let mut all_cases: Vec<Case> = Vec::new();
        let mut explicit: Vec<i128> = Vec::new();
        for case in cases.iter().chain(default_case.iter()) {
            let case_node = self
                .ast
                .case(*case)
                .map_err(|_| unresolved("a case", name))?;
            let member = self.member(case_node.member, &settings, name, false)?;
            let mut labels = Vec::new();
            let mut is_default = false;
            for label in &case_node.labels {
                match label {
                    CaseLabel::Default => is_default = true,
                    CaseLabel::Value(expr) => {
                        let value = self.ast.eval_integral(*expr).map_err(|_| {
                            Idl4CsharpError::Unevaluated {
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
                .any(|c| c.member.property == member.property)
            {
                continue;
            }
            all_cases.push(Case {
                member,
                labels,
                default: is_default,
            });
        }
        // §7.2.4.3.2: "prepend an "_" to the discriminator property name
        // if there is a name clash with the mapped union type name or any
        // of the field names".
        let disc = if name == "Discriminator"
            || all_cases
                .iter()
                .any(|c| c.member.property == "Discriminator")
        {
            "_Discriminator"
        } else {
            "Discriminator"
        };
        let first_free = domain.first_free(&explicit);
        let disc_text = &disc_type.text;
        let literal = |value: i128| self.discriminator_literal(&disc_type, disc_id, value, name);
        let initial_disc = domain.default_value();
        let selected_initial = all_cases.iter().find(|c| {
            c.labels.contains(&initial_disc) || (c.default && !explicit.contains(&initial_disc))
        });

        let slots = verbatim::declaration_slots(self.ast, id, verbatim::aliases::CSHARP);
        let mut w = Writer::new();
        for text in &slots.before {
            w.verbatim(text);
        }
        for annotation in self.user_annotations(&node.facets)? {
            w.line(&annotation);
        }
        w.open(&format!(
            "public class {name} : global::System.IEquatable<{name}>"
        ));
        for text in &slots.begin {
            w.verbatim(text);
        }
        w.line("private object __value;");
        w.line("");
        w.line("/// <summary>The discriminator (§7.2.4.3.2).</summary>");
        w.line(&format!(
            "public {disc_text} {disc} {{ get; private set; }}"
        ));
        w.line("");
        w.line(
            "/// <summary>The discriminator at the default value of its type, its member \
             default-constructed.</summary>",
        );
        w.open(&format!("public {name}()"));
        w.line(&format!("this.{disc} = {};", literal(initial_disc)?));
        if let Some(case) = selected_initial
            && let Some(initial) = &case.member.initial
        {
            w.line(&format!("this.__value = {initial};"));
        } else if let Some(case) = selected_initial {
            w.line(&format!(
                "this.__value = default({});",
                case.member.ctype.text
            ));
        }
        w.close();
        // The copy constructor: "a deep copy of the member selected by the
        // discriminator, if any".
        w.line("");
        w.line("/// <summary>A deep copy of <paramref name=\"other\"/> (§7.2.4.3.2).</summary>");
        w.open(&format!("public {name}({name} other)"));
        w.line("if (other is null) throw new global::System.ArgumentNullException(nameof(other));");
        w.line(&format!("this.{disc} = other.{disc};"));
        w.line("this.__value = other.__value;");
        for case in &all_cases {
            let copy = self.copy(
                &case.member.ctype,
                &format!("(({}) other.__value)", case.member.ctype.text),
                0,
            );
            if case.member.shallow
                || copy.starts_with("((") && !copy.contains("new ") && !copy.contains("Copy(")
            {
                continue;
            }
            let selected = self.union_selected(
                &case.labels,
                case.default,
                &explicit,
                &format!("other.{disc}"),
                &literal,
            )?;
            w.line(&format!("if ({selected}) this.__value = {copy};"));
        }
        w.close();
        for case in &all_cases {
            let member = &case.member;
            let ct = &member.ctype.text;
            let selected = self.union_selected(
                &case.labels,
                case.default,
                &explicit,
                &format!("this.{disc}"),
                &literal,
            )?;
            let first_label = if case.default {
                first_free
            } else {
                case.labels.first().copied()
            };
            let Some(first_label) = first_label else {
                return Err(Idl4CsharpError::NoCsharpForm {
                    construct: "a default member whose labels leave no discriminator value"
                        .to_string(),
                    at: format!("{name}.{}", member.idl),
                });
            };
            let set_name = format!("Set{}", member.property.trim_start_matches('@'));
            w.line("");
            for attribute in &member.attributes {
                w.line(attribute);
            }
            w.open(&format!("public {ct} {}", member.property));
            w.open("get");
            w.line(&format!(
                "if (!({selected})) throw new global::System.InvalidOperationException(\"{name}.{}: \
                 the discriminator selects another member\");",
                member.idl
            ));
            w.line(&format!("return ({ct}) this.__value;"));
            w.close();
            if !member.read_only {
                w.open("set");
                for check in self.checks(&member.ctype, "value", &member.at, 0)? {
                    w.line(&check);
                }
                w.line(&format!("this.{disc} = {};", literal(first_label)?));
                w.line("this.__value = value;");
                w.close();
            }
            w.close();
            if member.read_only {
                // "void Set<SequenceMemberName>()" and
                // "void Set<SequenceMemberName>(IEnumerable<T> elements)";
                // for a map the elements are key-value pairs.
                let (fresh, elements, fill) = match &member.ctype.shape {
                    Shape::Sequence { element, .. } => (
                        new_sequence(&member.ctype),
                        format!("global::System.Collections.Generic.IEnumerable<{}>", element.text),
                        "foreach (var element in elements) created.Add(element);".to_string(),
                    ),
                    Shape::Map { key, value, .. } => (
                        new_map(&member.ctype),
                        format!(
                            "global::System.Collections.Generic.IEnumerable<global::System.Collections.Generic.KeyValuePair<{}, {}>>",
                            key.text, value.text
                        ),
                        "foreach (var element in elements) created.Add(element.Key, element.Value);"
                            .to_string(),
                    ),
                    _ => (String::new(), String::new(), String::new()),
                };
                w.line("");
                w.open(&format!("public void {set_name}()"));
                w.line(&format!("this.__value = {fresh};"));
                w.line(&format!("this.{disc} = {};", literal(first_label)?));
                w.close();
                w.line("");
                w.open(&format!("public void {set_name}({elements} elements)"));
                w.line("if (elements is null) throw new global::System.ArgumentNullException(nameof(elements));");
                w.line(&format!("var created = {fresh};"));
                w.line(&fill);
                for check in self.checks(&member.ctype, "created", &member.at, 0)? {
                    w.line(&check);
                }
                w.line("this.__value = created;");
                w.line(&format!("this.{disc} = {};", literal(first_label)?));
                w.close();
            }
            // "For all such members, the union shall provide a modifier
            // method void Set<MemberName>(<MemberType> value,
            // <DiscriminatorType> discriminator) ... The modifier method
            // shall throw a System.ArgumentException exception when a
            // value is passed for the discriminator that is not among the
            // case labels for the member."
            if !member.read_only && (case.labels.len() > 1 || case.default) {
                let refused = if case.default {
                    let mut conditions = Vec::new();
                    for value in &explicit {
                        conditions.push(format!("discriminator == {}", literal(*value)?));
                    }
                    if conditions.is_empty() {
                        "false".to_string()
                    } else {
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
                    "public void {set_name}({ct} value, {disc_text} discriminator)"
                ));
                w.line(&format!(
                    "if ({refused}) throw new global::System.ArgumentException(\"{name}.{}: no label \
                     of this member\", nameof(discriminator));",
                    member.idl
                ));
                for check in self.checks(&member.ctype, "value", &member.at, 0)? {
                    w.line(&check);
                }
                w.line(&format!("this.{disc} = discriminator;"));
                w.line("this.__value = value;");
                w.close();
            }
        }
        // IEquatable<T>.
        w.line("");
        w.open(&format!("public bool Equals({name} other)"));
        w.line("if (other is null) return false;");
        w.line("if (ReferenceEquals(this, other)) return true;");
        w.line(&format!(
            "return global::System.Collections.Generic.EqualityComparer<{disc_text}>.Default.Equals(this.{disc}, other.{disc}) && global::Omg.Types.Structural.Equal(this.__value, other.__value);"
        ));
        w.close();
        w.line("");
        w.line(&format!(
            "public override bool Equals(object obj) {{ return Equals(obj as {name}); }}"
        ));
        w.line("");
        w.line(&format!(
            "public override int GetHashCode() {{ return global::System.HashCode.Combine(this.{disc}, global::Omg.Types.Structural.Hash(this.__value)); }}"
        ));
        for text in &slots.end {
            w.verbatim(text);
        }
        w.close();
        for text in &slots.after {
            w.verbatim(text);
        }
        Ok(w.finish())
    }

    /// The condition under which `disc` selects a case.
    fn union_selected(
        &self,
        labels: &[i128],
        default: bool,
        explicit: &[i128],
        disc: &str,
        literal: &impl Fn(i128) -> Result<String, Idl4CsharpError>,
    ) -> Result<String, Idl4CsharpError> {
        Ok(if default {
            if explicit.is_empty() {
                "true".to_string()
            } else {
                let mut conditions = Vec::new();
                for value in explicit {
                    conditions.push(format!("{disc} != {}", literal(*value)?));
                }
                conditions.join(" && ")
            }
        } else {
            let mut conditions = Vec::new();
            for value in labels {
                conditions.push(format!("{disc} == {}", literal(*value)?));
            }
            conditions.join(" || ")
        })
    }

    /// The C# text of a discriminator value.
    fn discriminator_literal(
        &self,
        disc: &CType,
        disc_id: Id<TypeNode>,
        value: i128,
        at: &str,
    ) -> Result<String, Idl4CsharpError> {
        match &disc.shape {
            Shape::Enum { .. } => {
                let settings = self.settings_of_type(disc_id);
                for enumerator in self.ast.enumerators_of(disc_id) {
                    if self.ast.effective_enumerator_value(*enumerator) == Some(value) {
                        let node = self
                            .ast
                            .enumerator(*enumerator)
                            .map_err(|_| unresolved("an enumerator", at))?;
                        return Ok(format!(
                            "{}.{}",
                            disc.text,
                            settings.naming.member(&node.name)
                        ));
                    }
                }
                Err(Idl4CsharpError::Unevaluated {
                    what: format!("an enumerator of value {value}"),
                    at: at.to_string(),
                })
            }
            Shape::Bool => Ok((value != 0).to_string()),
            Shape::Char { .. } => Ok(char_literal(u16::try_from(value).map_err(|_| {
                Idl4CsharpError::Unevaluated {
                    what: format!("a char label {value}"),
                    at: at.to_string(),
                }
            })?)),
            Shape::Integer { bits, signed } => {
                integer_literal(value, *bits, *signed).ok_or_else(|| Idl4CsharpError::Unevaluated {
                    what: format!("a label {value}"),
                    at: at.to_string(),
                })
            }
            _ => Err(Idl4CsharpError::NoCsharpForm {
                construct: "a discriminator of this type".to_string(),
                at: at.to_string(),
            }),
        }
    }

    /// The values a discriminator type has, in the order "starting from
    /// the zero-index of the discriminant type" walks them.
    fn discriminator_domain(
        &self,
        disc: Id<TypeNode>,
        at: &str,
    ) -> Result<Domain, Idl4CsharpError> {
        let node = self
            .ast
            .type_(disc)
            .map_err(|_| unresolved("a discriminator", at))?;
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
                        return Err(Idl4CsharpError::NoCsharpForm {
                            construct: "a floating-point discriminator".to_string(),
                            at: at.to_string(),
                        });
                    }
                };
                Domain::Range(low, high)
            }
            _ => {
                return Err(Idl4CsharpError::NoCsharpForm {
                    construct: "a discriminator of this type".to_string(),
                    at: at.to_string(),
                });
            }
        })
    }

    // -- enumerations, §7.2.4.3.3 ---------------------------------------------

    fn enumeration(&self, id: Id<TypeNode>, name: &str) -> Result<String, Idl4CsharpError> {
        let node = self
            .ast
            .type_(id)
            .map_err(|_| unresolved("an enumeration", name))?;
        let settings = self.settings_of_type(id);
        // "If the IDL enumeration declaration is preceded by a @bit_bound
        // annotation; the corresponding C# enum type shall be sbyte for bit
        // bound values between 1 and 8; short, for bit bound values between
        // 9 and 16; int, for bit bound values between 17 and 32; and long,
        // for bit bound values between 33 and 64."
        let explicit_bound = node.facets.xtypes.as_ref().and_then(|x| x.bit_bound);
        let underlying = match explicit_bound {
            Some(1..=8) => " : sbyte",
            Some(9..=16) => " : short",
            Some(17..=32) => " : int",
            Some(_) => " : long",
            None => "",
        };
        let slots = verbatim::declaration_slots(self.ast, id, verbatim::aliases::CSHARP);
        let mut w = Writer::new();
        for text in &slots.before {
            w.verbatim(text);
        }
        for annotation in self.user_annotations(&node.facets)? {
            w.line(&annotation);
        }
        w.open(&format!("public enum {name}{underlying}"));
        for text in &slots.begin {
            w.verbatim(text);
        }
        for enumerator in self.ast.enumerators_of(id) {
            let e = self
                .ast
                .enumerator(*enumerator)
                .map_err(|_| unresolved("an enumerator", name))?;
            let value = self
                .ast
                .effective_enumerator_value(*enumerator)
                .ok_or_else(|| Idl4CsharpError::Unevaluated {
                    what: "an enumerator's value".to_string(),
                    at: format!("{name}.{}", e.name),
                })?;
            for annotation in self.user_annotations(&e.facets)? {
                w.line(&annotation);
            }
            w.line(&format!("{} = {value},", settings.naming.member(&e.name)));
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

    // -- bit sets, §7.14.3.2 --------------------------------------------------

    fn bitset(&self, id: Id<TypeNode>, name: &str) -> Result<String, Idl4CsharpError> {
        let node = self
            .ast
            .type_(id)
            .map_err(|_| unresolved("a bit set", name))?;
        let settings = self.settings_of_type(id);
        // A bit set's fields are its base's and then its own: a C# struct
        // has no base type, and "public properties for each named bitfield
        // in the set" are all of them.
        let mut chain = vec![id];
        let mut current = id;
        while let Ok(TypeKind::Bitset {
            base: Some(base), ..
        }) = self.ast.type_(current).map(|n| &n.kind)
        {
            let Some(base) = base.resolved() else {
                return Err(unresolved("the base of a bit set", name));
            };
            current = self.through_aliases(base, name)?;
            if chain.contains(&current) || chain.len() > super::MAX_DEPTH {
                return Err(unresolved("a chain of bit set bases", name));
            }
            chain.push(current);
        }
        chain.reverse();
        let mut fields: Vec<(String, String, String, u16, Vec<String>)> = Vec::new();
        for set in chain {
            for bitfield in self.ast.bitfields_of(set) {
                let b = self
                    .ast
                    .bitfield(*bitfield)
                    .map_err(|_| unresolved("a bit field", name))?;
                // An anonymous bit field is padding and has no property.
                let Some(field_name) = b.name.clone() else {
                    continue;
                };
                let text = match b.holder_type.as_ref() {
                    Some(holder) => self.cs_type(holder, &settings, name)?.text,
                    // "the smallest unsigned integer type able to store the
                    // bit field with no loss; that is, byte if it is between
                    // 1 and 8, ushort if it is between 9 and 16, uint if it is
                    // between 17 and 32 and ulong if it is between 33 and 64".
                    None => match b.bits {
                        1..=8 => "byte".to_string(),
                        9..=16 => "ushort".to_string(),
                        17..=32 => "uint".to_string(),
                        _ => "ulong".to_string(),
                    },
                };
                fields.push((
                    settings.naming.member(&field_name),
                    format!("__{}", field_name.trim_start_matches('_')),
                    text,
                    b.bits,
                    self.user_annotations(&b.facets)?,
                ));
            }
        }
        let mut w = Writer::new();
        for annotation in self.user_annotations(&node.facets)? {
            w.line(&annotation);
        }
        w.open(&format!(
            "public struct {name} : global::System.IEquatable<{name}>"
        ));
        for (_, field, text, _, _) in &fields {
            w.line(&format!("private {text} {field};"));
        }
        for (property, field, text, bits, attributes) in &fields {
            w.line("");
            for attribute in attributes {
                w.line(attribute);
            }
            w.open(&format!("public {text} {property}"));
            w.line(&format!("get {{ return this.{field}; }}"));
            w.open("set");
            w.line(&bitfield_check(text, *bits, &format!("{name}.{property}")));
            w.line(&format!("this.{field} = value;"));
            w.close();
            w.close();
        }
        w.line("");
        w.open(&format!("public bool Equals({name} other)"));
        if fields.is_empty() {
            w.line("return true;");
        } else {
            let all: Vec<String> = fields
                .iter()
                .map(|(_, field, _, _, _)| format!("this.{field} == other.{field}"))
                .collect();
            w.line(&format!("return {};", all.join(" && ")));
        }
        w.close();
        w.line("");
        w.line(&format!(
            "public override bool Equals(object obj) {{ return obj is {name} other && Equals(other); }}"
        ));
        w.line("");
        w.open("public override int GetHashCode()");
        w.line("var hash = new global::System.HashCode();");
        for (_, field, _, _, _) in &fields {
            w.line(&format!("hash.Add(this.{field});"));
        }
        w.line("return hash.ToHashCode();");
        w.close();
        w.close();
        Ok(w.finish())
    }

    // -- bit masks, §7.14.3.3 ---------------------------------------------------

    fn bitmask(&self, id: Id<TypeNode>, flags: &str) -> Result<String, Idl4CsharpError> {
        let node = self
            .ast
            .type_(id)
            .map_err(|_| unresolved("a bit mask", flags))?;
        let settings = self.settings_of_type(id);
        let bound = self.ast.effective_bit_bound(id);
        // "the enum type shall be byte, for bit bound values between 1 and
        // 8; ushort, for bit bound values between 9 and 16; uint, for
        // values between 17 and 32; and ulong for bit bound values between
        // 33 and 64."
        let (underlying, one) = match bound {
            1..=8 => ("byte", "1"),
            9..=16 => ("ushort", "1"),
            17..=32 => ("uint", "1U"),
            _ => ("ulong", "1UL"),
        };
        let mut w = Writer::new();
        for annotation in self.user_annotations(&node.facets)? {
            w.line(&annotation);
        }
        w.line("[global::System.Flags]");
        w.open(&format!("public enum {flags} : {underlying}"));
        for bit in self.ast.bits_of(id) {
            let b = self
                .ast
                .bit_value(*bit)
                .map_err(|_| unresolved("a bit", flags))?;
            // "The value of each C# enum literal is dictated by the
            // @position annotation of the corresponding IDL bitmask member.
            // If no position is specified, the C# enum literals shall be set
            // to the value of the next power of two."
            let position = self.ast.effective_bit_position(*bit).ok_or_else(|| {
                Idl4CsharpError::Unevaluated {
                    what: "a bit position".to_string(),
                    at: format!("{flags}.{}", b.name),
                }
            })?;
            for attribute in self.user_annotations(&b.facets)? {
                w.line(&attribute);
            }
            w.line(&format!(
                "{} = {one} << {position},",
                settings.naming.member(&b.name)
            ));
        }
        w.close();
        Ok(w.finish())
    }

    // -- exceptions, §7.4.1 and A.1.1 -------------------------------------------

    /// An exception declared directly in a module.
    pub(super) fn top_level_exception(
        &mut self,
        id: Id<zerodds_ast::ExceptionNode>,
    ) -> Result<(), Idl4CsharpError> {
        let node = self
            .ast
            .exception(id)
            .map_err(|_| unresolved("an exception", ""))?;
        let settings = self.settings_in(node.module, None, &node.facets);
        let name = settings.naming.type_name(&node.name);
        let origin = self.origin(AnyId::Exception(id, node.node_id));
        let module = node.module;
        let body = self.exception(id, &name)?;
        self.file(module, &name, &origin, &body)
    }

    /// The class of an exception: "a C# class with the same name as the
    /// IDL exception. The mapped class shall extend the System.Exception
    /// class" — `Corba.UserException` on the CORBA platform (A.1.1).
    pub(super) fn exception(
        &self,
        id: Id<zerodds_ast::ExceptionNode>,
        name: &str,
    ) -> Result<String, Idl4CsharpError> {
        let node = self
            .ast
            .exception(id)
            .map_err(|_| unresolved("an exception", name))?;
        let settings = self.settings_in(node.module, node.scope.as_deref(), &node.facets);
        let mut members = Vec::new();
        for member in &node.members {
            members.push(self.member(*member, &settings, name, true)?);
        }
        check_member_collisions(&members, name)?;
        let base = match self.options.platform {
            Platform::Dds => "global::System.Exception",
            Platform::Corba => "global::Corba.UserException",
        };
        let mut w = Writer::new();
        for annotation in self.user_annotations(&node.facets)? {
            w.line(&annotation);
        }
        w.open(&format!("public class {name} : {base}"));
        write_fields(&mut w, &members);
        w.line("");
        w.open(&format!("public {name}()"));
        write_initialisers(&mut w, &members, false);
        w.close();
        w.line("");
        w.open(&format!("public {name}({name} other)"));
        w.line("if (other is null) throw new global::System.ArgumentNullException(nameof(other));");
        for member in &members {
            let copy = if member.shallow {
                format!("other.{}", member.field)
            } else {
                self.copy(&member.ctype, &format!("other.{}", member.field), 0)
            };
            w.line(&format!("this.{} = {copy};", member.field));
        }
        w.close();
        if !members.is_empty() {
            let params: Vec<String> = members
                .iter()
                .map(|m| format!("{} {}", m.ctype.text, m.param))
                .collect();
            w.line("");
            w.open(&format!("public {name}({})", params.join(", ")));
            for member in &members {
                for check in self.checks(&member.ctype, &member.param, &member.at, 0)? {
                    w.line(&check);
                }
                w.line(&format!("this.{} = {};", member.field, member.param));
            }
            w.close();
        }
        self.write_properties(&mut w, &members)?;
        w.close();
        Ok(w.finish())
    }

    // -- constants, §7.2.3 --------------------------------------------------------

    /// The constants declared directly in a module.
    pub(super) fn top_level_constants(
        &mut self,
        module: Id<ModuleNode>,
        constants: &[Id<zerodds_ast::ConstNode>],
    ) -> Result<(), Idl4CsharpError> {
        for (class, origin, body) in self.constant_classes(module, None, constants)? {
            self.file(module, &class, &origin, &body)?;
        }
        Ok(())
    }

    /// The classes the constants of one scope map to: one per constant
    /// (§7.2.3.1), or one `public static partial class` per container
    /// name (§7.2.3.2). As (class, origin, source).
    pub(super) fn constant_classes(
        &self,
        module: Id<ModuleNode>,
        scope: Option<&str>,
        constants: &[Id<zerodds_ast::ConstNode>],
    ) -> Result<Vec<(String, String, String)>, Idl4CsharpError> {
        let mut out = Vec::new();
        let mut containers: Vec<(String, Vec<String>, String)> = Vec::new();
        for constant in constants {
            let node = self
                .ast
                .constant(*constant)
                .map_err(|_| unresolved("a constant", ""))?;
            let settings = self.settings_in(module, scope, &node.facets);
            let at = node.name.clone();
            let ctype = self.cs_type(&node.type_, &settings, &at)?;
            let value =
                self.ast
                    .eval_const(node.expr)
                    .map_err(|_| Idl4CsharpError::Unevaluated {
                        what: "a constant's value".to_string(),
                        at: at.clone(),
                    })?;
            let value = self.cs_value(&value, &ctype, &at)?;
            let origin = self.origin(AnyId::Const(*constant, node.node_id));
            let annotations = self.user_annotations(&node.facets)?;
            let name = settings.naming.constant(&node.name);
            match &settings.container {
                Some(container) => {
                    let mut line = String::new();
                    for annotation in &annotations {
                        line.push_str(annotation);
                        line.push('\n');
                    }
                    line.push_str(&format!("public const {} {name} = {value};", ctype.text));
                    match containers.iter_mut().find(|(n, _, _)| n == container) {
                        Some((_, lines, _)) => lines.push(line),
                        None => containers.push((container.clone(), vec![line], origin)),
                    }
                }
                None => {
                    let mut w = Writer::new();
                    for annotation in &annotations {
                        w.line(annotation);
                    }
                    w.open(&format!("public static class {name}"));
                    w.line(&format!("public const {} Value = {value};", ctype.text));
                    w.close();
                    out.push((name, origin, w.finish()));
                }
            }
        }
        for (container, lines, origin) in containers {
            let mut w = Writer::new();
            w.line("/// <summary>The constants of this scope (§7.2.3.2).</summary>");
            w.open(&format!("public static partial class {container}"));
            for line in lines {
                w.nested(&line);
            }
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
    fn default_value(&self) -> i128 {
        match self {
            Self::Values(values) => values.first().copied().unwrap_or(0),
            Self::Range(low, high) => 0.clamp(*low, *high),
        }
    }

    /// "the first available default value starting from the zero-index of
    /// the discriminant type".
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
}

/// The type a member is declared in.
fn owner_type(generator: &Gen<'_>, member: Id<MemberNode>) -> Option<Id<TypeNode>> {
    match generator.ast.member(member).ok()?.parent {
        AnyId::Type(id, _) => Some(id),
        _ => None,
    }
}

/// A new empty sequence of the type `ctype`: `Omg.Types.SequenceList<T>`,
/// or `Omg.Types.BoundedList<T>` for a bounded one.
fn new_sequence(ctype: &CType) -> String {
    match &ctype.shape {
        Shape::Sequence {
            element,
            bound: Some(bound),
        } => format!(
            "new global::Omg.Types.BoundedList<{}>({bound})",
            element.text
        ),
        Shape::Sequence { element, .. } => {
            format!("new global::Omg.Types.SequenceList<{}>()", element.text)
        }
        _ => "null".to_string(),
    }
}

/// A new empty map of the type `ctype`.
fn new_map(ctype: &CType) -> String {
    match &ctype.shape {
        Shape::Map {
            key,
            value,
            bound: Some(bound),
        } => format!(
            "new global::Omg.Types.BoundedDictionary<{}, {}>({bound})",
            key.text, value.text
        ),
        Shape::Map { key, value, .. } => format!(
            "new global::System.Collections.Generic.Dictionary<{}, {}>()",
            key.text, value.text
        ),
        _ => "null".to_string(),
    }
}

/// The private fields, one per member.
fn write_fields(w: &mut Writer, members: &[Member]) {
    for member in members {
        w.line(&format!("private {} {};", member.ctype.text, member.field));
    }
}

/// The default constructor's assignments. A C# struct assigns every
/// field.
fn write_initialisers(w: &mut Writer, members: &[Member], every: bool) {
    for member in members {
        match &member.initial {
            Some(initial) => w.line(&format!("this.{} = {initial};", member.field)),
            None if every => w.line(&format!("this.{} = default;", member.field)),
            None => {}
        }
    }
}

/// `Equals(T)`, `Equals(object)` and `GetHashCode()` over the members:
/// "The class shall implement the IEquatable<T> interface".
fn write_equality(w: &mut Writer, name: &str, members: &[Member], derived: bool, is_struct: bool) {
    w.line("");
    w.open(&format!("public bool Equals({name} other)"));
    if !is_struct {
        w.line("if (other is null) return false;");
        w.line("if (ReferenceEquals(this, other)) return true;");
    }
    let mut terms: Vec<String> = Vec::new();
    if derived {
        terms.push("base.Equals(other)".to_string());
    }
    for member in members {
        terms.push(format!(
            "global::Omg.Types.Structural.Equal(this.{0}, other.{0})",
            member.field
        ));
    }
    if terms.is_empty() {
        w.line("return true;");
    } else {
        w.line(&format!("return {};", terms.join(" && ")));
    }
    w.close();
    w.line("");
    if is_struct {
        w.line(&format!(
            "public override bool Equals(object obj) {{ return obj is {name} other && Equals(other); }}"
        ));
    } else {
        w.line(&format!(
            "public override bool Equals(object obj) {{ return Equals(obj as {name}); }}"
        ));
    }
    w.line("");
    w.open("public override int GetHashCode()");
    w.line("var hash = new global::System.HashCode();");
    if derived {
        w.line("hash.Add(base.GetHashCode());");
    }
    for member in members {
        w.line(&format!(
            "hash.Add(global::Omg.Types.Structural.Hash(this.{}));",
            member.field
        ));
    }
    w.line("return hash.ToHashCode();");
    w.close();
}

/// Two members of one class that map to one C# name.
fn check_member_collisions(members: &[Member], owner: &str) -> Result<(), Idl4CsharpError> {
    for (index, member) in members.iter().enumerate() {
        if let Some(other) = members[..index]
            .iter()
            .find(|m| m.property == member.property)
        {
            return Err(Idl4CsharpError::Collision {
                name: member.property.clone(),
                first: format!("{owner}.{}", other.idl),
                second: format!("{owner}.{}", member.idl),
            });
        }
    }
    Ok(())
}

/// A constant value for messages.
pub(super) fn value_text(value: &ConstValue) -> String {
    crate::emit::idl4_java::value_text(value)
}

/// An integer of a constant value.
fn integer_of(value: &ConstValue, at: &str) -> Result<i128, Idl4CsharpError> {
    match value {
        ConstValue::Integer(number) => Ok(*number),
        ConstValue::Char(ch) => Ok(i128::from(u32::from(*ch))),
        ConstValue::WChar(unit) => Ok(i128::from(*unit)),
        _ => Err(Idl4CsharpError::Unevaluated {
            what: format!("an integer, not {}", value_text(value)),
            at: at.to_string(),
        }),
    }
}

/// A C# integer literal of the type `bits`/`signed` for `value`, `None`
/// when the type does not hold it.
pub(super) fn integer_literal(value: i128, bits: u8, signed: bool) -> Option<String> {
    let (low, high): (i128, i128) = match (bits, signed) {
        (8, true) => (-128, 127),
        (8, false) => (0, 255),
        (16, true) => (-32_768, 32_767),
        (16, false) => (0, 65_535),
        (32, true) => (i128::from(i32::MIN), i128::from(i32::MAX)),
        (32, false) => (0, i128::from(u32::MAX)),
        (_, true) => (i128::from(i64::MIN), i128::from(i64::MAX)),
        (_, false) => (0, i128::from(u64::MAX)),
    };
    if !(low..=high).contains(&value) {
        return None;
    }
    Some(match (bits, signed) {
        (8, true) => format!("(sbyte)({value})"),
        (8, false) => format!("(byte){value}"),
        (16, true) => format!("(short)({value})"),
        (16, false) => format!("(ushort){value}"),
        (32, false) => format!("{value}U"),
        (64, true) if value == i128::from(i64::MIN) => "long.MinValue".to_string(),
        (64, true) => format!("{value}L"),
        (64, false) => format!("{value}UL"),
        _ => value.to_string(),
    })
}

/// The check a bit field's setter runs: the value has to fit the field's
/// bits.
fn bitfield_check(text: &str, bits: u16, at: &str) -> String {
    let unsigned = matches!(text, "byte" | "ushort" | "uint" | "ulong");
    if text == "bool" || bits >= 64 {
        return String::new();
    }
    if unsigned {
        format!(
            "if ((ulong)value >> {bits} != 0) throw new global::System.ArgumentOutOfRangeException(\"{at}\", value, \"{bits} bits\");"
        )
    } else {
        let high = (1i128 << (bits - 1)) - 1;
        let low = -(1i128 << (bits - 1));
        format!(
            "if (value < {low} || value > {high}) throw new global::System.ArgumentOutOfRangeException(\"{at}\", value, \"{bits} bits\");"
        )
    }
}

fn unresolved(what: &str, at: &str) -> Idl4CsharpError {
    Idl4CsharpError::Unresolved {
        what: what.to_string(),
        at: at.to_string(),
    }
}
