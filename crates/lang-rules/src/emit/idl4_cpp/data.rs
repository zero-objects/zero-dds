// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Data types: constants (§7.2.3), structures (§7.2.4.3.1, §7.14.1,
//! §8.1.1), unions (§7.2.4.3.2, §7.14.2), enumerations (§7.2.4.3.3),
//! typedefs (§7.2.4.6), exceptions (§7.4.1), bit sets (§7.14.3.2), bit
//! masks (§7.14.3.3), and the annotations of §7.17 on their members.

use super::text::{self, Writer};
use super::types::{CppType, Range, Shape};
use super::{Gen, Idl4CppError, Item, Platform, Standard, names, unresolved};
use crate::verbatim;
use zerodds_ast::{AnyId, CaseLabel, ConstValue, MemberNode, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::{Id, Ref};

/// One member of a structure, union or exception, mapped.
pub(super) struct Member {
    /// The IDL name.
    pub idl: String,
    /// The C++ name.
    pub name: String,
    /// The type as written in the declaration: `@optional`, `@external`
    /// and the range annotations applied.
    pub decl: CppType,
    /// The type before those.
    pub plain: CppType,
    /// The initializer of a default-constructed member, if it has one.
    pub init: Option<String>,
    /// Whether it is `@external` (Table 7.18).
    pub external: bool,
    /// Whether it is `@optional` (Table 7.15).
    pub optional: bool,
    /// Declarations the member's type needs ahead of the class: the
    /// limits class of a floating-point `@range`.
    pub before: Vec<String>,
}

impl Gen<'_> {
    // -- declarations -----------------------------------------------------------

    /// The C++ of one type declaration.
    pub(super) fn type_declaration(&self, id: Id<TypeNode>) -> Result<String, Idl4CppError> {
        let node = self.ast.type_(id).map_err(|_| unresolved("a type"))?;
        // A forward declaration whose body never arrives is the class
        // declaration every class has ahead of the definitions.
        if node.completeness == zerodds_ast::Completeness::Forward {
            return Ok(String::new());
        }
        let name = names::name(&node.name);
        match &node.kind {
            TypeKind::Structure { .. } => self.structure(id, &name),
            TypeKind::Union { .. } => self.union(id, &name),
            TypeKind::Enumeration { .. } => self.enumeration(id, &name),
            TypeKind::Alias { aliased } => self.typedef(id, &name, aliased),
            TypeKind::Bitset { .. } => self.bitset(id, &name),
            TypeKind::Bitmask { .. } => self.bitmask(id, &name),
            TypeKind::Valuetype { .. } => self.valuetype(id, &name),
            TypeKind::ValueBox { .. } => self.valuebox(id, &name),
            // A sequence, string, map, array or primitive declared on its
            // own is the target of a typedef and has nothing to declare.
            _ => Ok(String::new()),
        }
    }

    // -- constants, §7.2.3 and Annex C.2.1.1 --------------------------------------

    /// One constant; `embedded` for one in an interface body (§7.5: "In
    /// the case of constants, constexpr declarations shall be marked as
    /// static").
    pub(super) fn constant(
        &self,
        id: Id<zerodds_ast::ConstNode>,
        embedded: bool,
    ) -> Result<String, Idl4CppError> {
        let node = self
            .ast
            .constant(id)
            .map_err(|_| unresolved("a constant"))?;
        let at = self.origin(AnyId::Const(id, node.node_id));
        let ctype = self.cpp_type(&node.type_, &at)?;
        let value = self
            .ast
            .eval_const(node.expr)
            .map_err(|_| Idl4CppError::Unevaluated {
                what: "the value of a constant".to_string(),
                at: at.clone(),
            })?;
        let name = names::name(&node.name);
        let literal = self.literal(&ctype, &value, &at)?;
        let static_ = if embedded { "static " } else { "" };
        match self.options.standard {
            Standard::Cxx11 => Ok(match &ctype.shape {
                Shape::Str => {
                    let wide = matches!(value, ConstValue::WString(_));
                    let view = if wide {
                        "::omg::types::wstring_view"
                    } else {
                        "::omg::types::string_view"
                    };
                    format!("{static_}constexpr {view} {name}{{ {literal} }};")
                }
                Shape::Fixed => format!("{static_}constexpr {} {name}{{ {literal} }};", ctype.text),
                _ => format!("{static_}constexpr {} {name} = {literal};", ctype.text),
            }),
            // Annex C.2.1.1: "C++ constant declarations of equivalent type";
            // a string constant is a `std::string`.
            Standard::Cxx03 => Ok(match &ctype.shape {
                Shape::Str | Shape::Fixed => {
                    format!("{static_}const {} {name}({literal});", ctype.text)
                }
                _ => format!("{static_}const {} {name} = {literal};", ctype.text),
            }),
        }
    }

    /// The C++ literal of `value` as a value of `ctype`.
    pub(super) fn literal(
        &self,
        ctype: &CppType,
        value: &ConstValue,
        at: &str,
    ) -> Result<String, Idl4CppError> {
        let unfit = || Idl4CppError::Unevaluated {
            what: format!(
                "the value {} as a {}",
                crate::emit::idl4_java::value_text(value),
                ctype.text
            ),
            at: at.to_string(),
        };
        Ok(match (&ctype.shape, value) {
            (Shape::Bool, ConstValue::Boolean(flag)) => flag.to_string(),
            // An IDL `char` is one octet wide; a value
            // past 0xFF has no narrow literal and falls to the refusal below.
            (Shape::Char { wide: false }, ConstValue::Char(c)) if u32::from(*c) <= 0xFF => {
                text::char_literal(*c)
            }
            (Shape::Char { wide: true }, ConstValue::WChar(point)) => text::wchar_literal(*point),
            (Shape::Char { wide: true }, ConstValue::Char(c)) => text::wchar_literal(u32::from(*c)),
            (Shape::Integer { bits, signed }, _) => {
                let number = match value {
                    ConstValue::Integer(number) => *number,
                    ConstValue::Char(c) => i128::from(u32::from(*c)),
                    ConstValue::WChar(point) => i128::from(*point),
                    _ => return Err(unfit()),
                };
                integer_literal(number, *bits, *signed).ok_or_else(unfit)?
            }
            (Shape::Floating { bits }, _) => {
                let body = match value {
                    ConstValue::Float { mantissa, exponent } => {
                        let mut body = mantissa.clone();
                        if !body.contains('.') && exponent.is_none() {
                            body.push_str(".0");
                        }
                        if let Some(exponent) = exponent {
                            body.push('e');
                            body.push_str(exponent);
                        }
                        body
                    }
                    ConstValue::Integer(number) => format!("{number}.0"),
                    ConstValue::Fixed { .. } => crate::emit::idl4_java::value_text(value),
                    _ => return Err(unfit()),
                };
                match bits {
                    32 => format!("{body}F"),
                    128 => format!("{body}L"),
                    _ => body,
                }
            }
            (Shape::Fixed, ConstValue::Fixed { .. } | ConstValue::Integer(_)) => {
                text::string_literal(&crate::emit::idl4_java::value_text(value))
            }
            (Shape::Str, ConstValue::String(s)) => text::string_literal(s),
            (Shape::Str, ConstValue::WString(points)) => text::wstring_literal(points),
            (Shape::Enum { .. }, ConstValue::Integer(number)) => {
                let Some(enumeration) = ctype.declared else {
                    return Err(unfit());
                };
                self.enumerator_with_value(enumeration, *number)?
                    .ok_or_else(unfit)?
            }
            _ => return Err(unfit()),
        })
    }

    /// The qualified enumerator of `enumeration` whose value is `value`.
    pub(super) fn enumerator_with_value(
        &self,
        enumeration: Id<TypeNode>,
        value: i128,
    ) -> Result<Option<String>, Idl4CppError> {
        for enumerator in self.ast.enumerators_of(enumeration) {
            if self.ast.effective_enumerator_value(*enumerator) == Some(value) {
                return Ok(Some(self.enumerator_text(enumeration, *enumerator)?));
            }
        }
        Ok(None)
    }

    // -- typedefs, §7.2.4.6 and Annex C.2.1.2.4 ----------------------------------------

    fn typedef(
        &self,
        id: Id<TypeNode>,
        name: &str,
        aliased: &Ref<TypeNode>,
    ) -> Result<String, Idl4CppError> {
        let at = self.origin_of_type(id);
        let target = self.cpp_type(aliased, &at)?;
        Ok(match self.options.standard {
            Standard::Cxx11 => format!("using {name} = {};", target.text),
            Standard::Cxx03 => format!("typedef {} {name}{};", target.text, target.suffix),
        })
    }

    pub(super) fn origin_of_type(&self, id: Id<TypeNode>) -> String {
        self.ast.type_(id).map_or_else(
            |_| String::new(),
            |node| self.origin(AnyId::Type(id, node.node_id)),
        )
    }

    // -- members ------------------------------------------------------------------

    /// One member, mapped with its annotations: `@optional` (Table 7.15,
    /// Annex C.2.5.1), `@external` (Table 7.18, Annex C.2.5.2),
    /// `@default`, `@range`, `@min`, `@max` (Table 7.17) and
    /// `@default_literal` (Table 7.16).
    pub(super) fn member(&self, id: Id<MemberNode>, owner: &str) -> Result<Member, Idl4CppError> {
        let node = self.ast.member(id).map_err(|_| unresolved("a member"))?;
        let at = format!("{owner}::{}", node.name);
        let plain = self.cpp_type(&node.type_, &at)?;
        let xtypes = node.facets.xtypes.as_ref();
        let optional = xtypes.is_some_and(|x| x.is_optional);
        let external = xtypes.is_some_and(|x| x.is_external);
        let mut range = plain.range.clone();
        if let Some(constraint) = xtypes.and_then(|x| x.range) {
            range = Some(self.merge_range(range, constraint, &at)?);
        }
        let name = names::name(&node.name);
        let mut decl = plain.clone();
        let mut before = Vec::new();
        if let Some(range) = &range
            && (range.min.is_some() || range.max.is_some())
        {
            decl = self.ranged(
                &plain,
                range,
                &format!("{owner}_{}", node.name),
                &at,
                &mut before,
            )?;
        }
        // The initializer: @default, the first enumerator, zero.
        let default = xtypes
            .and_then(|x| x.default_value)
            .map(|expr| {
                self.ast
                    .eval_const(expr)
                    .map_err(|_| Idl4CppError::Unevaluated {
                        what: "the value of @default".to_string(),
                        at: at.clone(),
                    })
            })
            .transpose()?;
        let mut init = match (&default, &plain.shape) {
            (Some(value), _) => Some(self.literal(&plain, value, &at)?),
            (None, Shape::Enum { default }) => Some(default.clone()),
            (None, _) if plain.by_value() => Some(String::new()),
            // A C array under Annex C: `name()` value-initializes it, the
            // zero of §7.2.4.1 for each element.
            (None, _) if !plain.suffix.is_empty() => Some(String::new()),
            _ => None,
        };
        if optional || external {
            init = None;
            let inner = &decl.text;
            decl = match (self.options.standard, optional) {
                (Standard::Cxx11, true) => CppType {
                    text: format!("::omg::types::optional<{inner}>"),
                    ..decl.clone()
                },
                (Standard::Cxx11, false) => CppType {
                    text: format!("::omg::types::ref_type<{inner}>"),
                    ..decl.clone()
                },
                (Standard::Cxx03, _) => CppType {
                    text: format!("{inner}*"),
                    ..decl.clone()
                },
            };
            if self.options.standard == Standard::Cxx03 {
                init = Some("0".to_string());
            }
        }
        Ok(Member {
            idl: node.name.clone(),
            name,
            decl,
            plain,
            init,
            external,
            optional,
            before,
        })
    }

    /// The type of a member under `@range`, `@min` or `@max`:
    /// `omg::types::ranged<T, min, max>`, or for a floating-point type,
    /// whose bounds are no template argument before C++20,
    /// `omg::types::ranged_by<T, Limits>` with a class of the two bounds.
    fn ranged(
        &self,
        plain: &CppType,
        range: &Range,
        stem: &str,
        at: &str,
        before: &mut Vec<String>,
    ) -> Result<CppType, Idl4CppError> {
        let bound_text = |value: &Option<ConstValue>, low: bool| -> Result<String, Idl4CppError> {
            match value {
                Some(value) => self.literal(plain, value, at),
                None => Ok(format!(
                    "std::numeric_limits<{}>::{}()",
                    plain.text,
                    if low { "lowest" } else { "max" }
                )),
            }
        };
        let min = bound_text(&range.min, true)?;
        let max = bound_text(&range.max, false)?;
        match plain.shape {
            Shape::Integer { .. } | Shape::Char { .. } => Ok(CppType {
                text: format!("::omg::types::ranged<{}, {min}, {max}>", plain.text),
                ..plain.clone()
            }),
            Shape::Floating { .. } => {
                let limits = format!("{}_range", stem.replace("::", "_"));
                let constexpr = if self.options.standard == Standard::Cxx11 {
                    "constexpr "
                } else {
                    ""
                };
                before.push(format!(
                    "struct {limits} {{\n    static {constexpr}{t} min() {{ return {min}; }}\n    \
                     static {constexpr}{t} max() {{ return {max}; }}\n}};",
                    t = plain.text
                ));
                Ok(CppType {
                    text: format!("::omg::types::ranged_by<{}, {limits}>", plain.text),
                    ..plain.clone()
                })
            }
            _ => Err(Idl4CppError::NoCppForm {
                construct: "@range, @min or @max on a type that is not a number".to_string(),
                at: at.to_string(),
            }),
        }
    }

    /// `T name{init};` or `T name;`, the C++03 array suffix after the name.
    fn field(&self, member: &Member, field_name: &str) -> String {
        let suffix = &member.decl.suffix;
        match (&member.init, self.options.standard) {
            (Some(init), Standard::Cxx11) => {
                format!("{} {field_name}{suffix} {{{init}}};", member.decl.text)
            }
            _ => format!("{} {field_name}{suffix};", member.decl.text),
        }
    }

    /// `name(init)` for a C++03 constructor's initializer list.
    fn initializer(member: &Member, field_name: &str) -> Option<String> {
        member
            .init
            .as_ref()
            .map(|init| format!("{field_name}({init})"))
    }

    /// Whether two values of a member compare equal: the pointees of
    /// `@external` members, element by element for a C++03 array.
    fn equal_text(&self, member: &Member, a: &str, b: &str) -> String {
        if member.external && self.options.standard == Standard::Cxx11 {
            return format!("((!{a} && !{b}) || ({a} && {b} && *{a} == *{b}))");
        }
        if (member.external || member.optional) && self.options.standard == Standard::Cxx03 {
            return format!("((!{a} && !{b}) || ({a} && {b} && *{a} == *{b}))");
        }
        if !member.decl.suffix.is_empty() {
            return format!("::omg::types::detail::equal({a}, {b})");
        }
        format!("{a} == {b}")
    }

    fn swap_text(&self, member: &Member, a: &str, b: &str) -> String {
        if !member.decl.suffix.is_empty() {
            return format!("::omg::types::detail::swap_values({a}, {b});");
        }
        format!("swap({a}, {b});")
    }

    // -- structures, §7.2.4.3.1, §7.14.1 and §8.1.1 -------------------------------------

    fn structure(&self, id: Id<TypeNode>, name: &str) -> Result<String, Idl4CppError> {
        let node = self.ast.type_(id).map_err(|_| unresolved("a structure"))?;
        let TypeKind::Structure { base, members } = &node.kind else {
            return Err(unresolved("a structure"));
        };
        let settings = self.settings_of_type(id);
        let base = match base.as_ref().and_then(Ref::resolved) {
            Some(base) => Some(self.qualified(self.through_aliases(base)?)?),
            None => None,
        };
        let mut mapped = Vec::new();
        for member in members {
            mapped.push(self.member(*member, name)?);
        }
        let slots = verbatim::declaration_slots(self.ast, id, verbatim::aliases::CPP);
        let mut w = Writer::new();
        for member in &mapped {
            for text in &member.before {
                w.nested(text);
            }
        }
        for text in &slots.before {
            w.verbatim(text);
        }
        let inherit = base
            .as_ref()
            .map_or_else(String::new, |base| format!(" : public {base}"));
        let cxx11 = self.cxx11();
        if settings.accessors {
            self.accessor_class(&mut w, name, &inherit, &mapped, &slots)?;
        } else {
            w.open(&format!("struct {name}{inherit}"));
            for text in &slots.begin {
                w.verbatim(text);
            }
            for member in &mapped {
                w.line(&self.field(member, &member.name));
            }
            let externals: Vec<&Member> = mapped.iter().filter(|m| m.external).collect();
            if !cxx11 {
                // Annex C.2.1.2.2: no move constructor, no move assignment.
                let inits: Vec<String> = mapped
                    .iter()
                    .filter_map(|m| Self::initializer(m, &m.name))
                    .collect();
                w.line("");
                if inits.is_empty() {
                    w.line(&format!("{name}() {{}}"));
                } else {
                    w.line(&format!("{name}() : {} {{}}", inits.join(", ")));
                }
            } else if !externals.is_empty() {
                // Table 7.18: "Structures containing members annotated
                // with the @external annotation shall make a deep copy of
                // the external member in their copy constructor."
                w.line("");
                w.line(&format!("{name}() = default;"));
                let mut copies = Vec::new();
                if let Some(base) = &base {
                    copies.push(format!("{base}(other)"));
                }
                for member in &mapped {
                    if member.external {
                        copies.push(format!(
                            "{n}(other.{n} ? std::make_shared<{t}>(*other.{n}) : nullptr)",
                            n = member.name,
                            t = member.plain.text
                        ));
                    } else {
                        copies.push(format!("{n}(other.{n})", n = member.name));
                    }
                }
                w.line(&format!(
                    "{name}(const {name}& other) : {} {{}}",
                    copies.join(", ")
                ));
                w.line(&format!("{name}({name}&&) = default;"));
                w.open(&format!("{name}& operator=(const {name}& other)"));
                w.line(&format!("if (this != &other) *this = {name}(other);"));
                w.line("return *this;");
                w.close();
                w.line(&format!("{name}& operator=({name}&&) = default;"));
                w.line(&format!("~{name}() = default;"));
            }
            for text in &slots.end {
                w.verbatim(text);
            }
            w.close_with(";");
        }
        for text in &slots.after {
            w.verbatim(text);
        }
        // "A set of comparison operators, including at least "equal to"
        // and "not equal to."" and "a swap function in the namespace of
        // the C++ struct definition", which "shall ensure that all
        // inherited members are swapped" (§7.14.1).
        let field = |m: &Member| -> String {
            if settings.accessors {
                format!("{}_", m.name)
            } else {
                m.name.clone()
            }
        };
        let mut terms = Vec::new();
        if let Some(base) = &base {
            terms.push(format!(
                "static_cast<const {base}&>(a) == static_cast<const {base}&>(b)"
            ));
        }
        for member in &mapped {
            let f = field(member);
            terms.push(self.equal_text(member, &format!("a.{f}"), &format!("b.{f}")));
        }
        let friend = if settings.accessors { "" } else { "inline " };
        let mut ops = Writer::new();
        let all = if terms.is_empty() {
            "true".to_string()
        } else {
            terms.join("\n        && ")
        };
        if settings.accessors {
            // Declared as friends in the class, where the members are.
        } else {
            ops.open(&format!(
                "{friend}bool operator==(const {name}& a, const {name}& b)"
            ));
            ops.line(&format!("return {all};"));
            ops.close();
            ops.open(&format!(
                "{friend}bool operator!=(const {name}& a, const {name}& b)"
            ));
            ops.line("return !(a == b);");
            ops.close();
            ops.open(&format!("inline void swap({name}& a, {name}& b)"));
            ops.line("using std::swap;");
            if let Some(base) = &base {
                ops.line(&format!(
                    "swap(static_cast<{base}&>(a), static_cast<{base}&>(b));"
                ));
            }
            for member in &mapped {
                ops.line(&self.swap_text(
                    member,
                    &format!("a.{}", member.name),
                    &format!("b.{}", member.name),
                ));
            }
            ops.close();
            w.nested(&ops.finish());
        }
        Ok(w.finish())
    }

    /// §8.1.1: "CLASS_WITH_PUBLIC_ACCESSORS_AND_MODIFIERS changes the
    /// default behavior, mapping the annotated IDL struct to C++ class
    /// where all members are only accessible via public accessor methods,
    /// and may only be modified through public modifier methods."
    fn accessor_class(
        &self,
        w: &mut Writer,
        name: &str,
        inherit: &str,
        mapped: &[Member],
        slots: &verbatim::Slots,
    ) -> Result<(), Idl4CppError> {
        let cxx11 = self.cxx11();
        w.open(&format!("class {name}{inherit}"));
        w.line("public:");
        for text in &slots.begin {
            w.verbatim(text);
        }
        // Constructors: the default one, and "at a minimum ... a
        // constructor that allows the initialization of every element".
        let inits: Vec<String> = mapped
            .iter()
            .filter_map(|m| Self::initializer(m, &format!("{}_", m.name)))
            .collect();
        if cxx11 {
            w.line(&format!("{name}() = default;"));
        } else if inits.is_empty() {
            w.line(&format!("{name}() {{}}"));
        } else {
            w.line(&format!("{name}() : {} {{}}", inits.join(", ")));
        }
        if !mapped.is_empty() {
            let params: Vec<String> = mapped
                .iter()
                .map(|m| {
                    if m.decl.by_value() && m.decl.text == m.plain.text {
                        format!("{} {}", m.decl.text, m.name)
                    } else {
                        format!("const {}& {}", m.decl.text, m.name)
                    }
                })
                .collect();
            let sets: Vec<String> = mapped
                .iter()
                .map(|m| format!("{n}_({n})", n = m.name))
                .collect();
            let explicit = if mapped.len() == 1 { "explicit " } else { "" };
            w.line(&format!(
                "{explicit}{name}({}) : {} {{}}",
                params.join(", "),
                sets.join(", ")
            ));
        }
        for member in mapped {
            let n = &member.name;
            let t = &member.decl.text;
            w.line("");
            if member.decl.by_value() && member.decl.text == member.plain.text {
                w.line(&format!("void {n}({t} value) {{ {n}_ = value; }}"));
            } else {
                w.line(&format!("void {n}(const {t}& value) {{ {n}_ = value; }}"));
                if cxx11 {
                    w.line(&format!(
                        "void {n}({t}&& value) {{ {n}_ = std::move(value); }}"
                    ));
                }
            }
            // "Accessor methods ... shall return the C++ equivalent type
            // ... and shall be declared as const. Additionally, the
            // implementation shall provide non-const accessors of the same
            // name that return a reference to the member."
            w.line(&format!("{t} {n}() const {{ return {n}_; }}"));
            w.line(&format!("{t}& {n}() {{ return {n}_; }}"));
        }
        let terms: Vec<String> = mapped
            .iter()
            .map(|m| self.equal_text(m, &format!("a.{}_", m.name), &format!("b.{}_", m.name)))
            .collect();
        let all = if terms.is_empty() {
            "true".to_string()
        } else {
            terms.join(" && ")
        };
        w.line("");
        w.line(&format!(
            "friend bool operator==(const {name}& a, const {name}& b) {{ return {all}; }}"
        ));
        w.line(&format!(
            "friend bool operator!=(const {name}& a, const {name}& b) {{ return !(a == b); }}"
        ));
        w.open(&format!("friend void swap({name}& a, {name}& b)"));
        w.line("using std::swap;");
        for member in mapped {
            w.line(&self.swap_text(
                member,
                &format!("a.{}_", member.name),
                &format!("b.{}_", member.name),
            ));
        }
        w.close();
        for text in &slots.end {
            w.verbatim(text);
        }
        w.line("");
        w.line("private:");
        for member in mapped {
            w.line(&self.field(member, &format!("{}_", member.name)));
        }
        w.close_with(";");
        Ok(())
    }

    pub(super) fn through_aliases(&self, id: Id<TypeNode>) -> Result<Id<TypeNode>, Idl4CppError> {
        let mut current = id;
        for _ in 0..super::MAX_DEPTH {
            match self.ast.type_(current).map(|node| &node.kind) {
                Ok(TypeKind::Alias { aliased }) => {
                    current = aliased.resolved().ok_or_else(|| unresolved("a typedef"))?;
                }
                Ok(_) => return Ok(current),
                Err(_) => break,
            }
        }
        Err(unresolved("a chain of typedefs"))
    }

    // -- unions, §7.2.4.3.2 and §7.14.2 ----------------------------------------------------

    #[allow(clippy::too_many_lines)]
    fn union(&self, id: Id<TypeNode>, name: &str) -> Result<String, Idl4CppError> {
        let node = self.ast.type_(id).map_err(|_| unresolved("a union"))?;
        let TypeKind::Union {
            discriminator,
            cases,
            default_case,
        } = &node.kind
        else {
            return Err(unresolved("a union"));
        };
        let at = self.origin_of_type(id);
        let disc = self.cpp_type(discriminator, &at)?;
        let disc_id = discriminator
            .resolved()
            .map(|d| self.through_aliases(d))
            .transpose()?
            .ok_or_else(|| unresolved("a discriminator"))?;
        let domain = self.discriminator_domain(disc_id, &at)?;
        struct Case {
            member: Member,
            labels: Vec<i128>,
            default: bool,
        }
        let mut all: Vec<Case> = Vec::new();
        let mut explicit: Vec<i128> = Vec::new();
        for case in cases.iter().chain(default_case.iter()) {
            let case_node = self.ast.case(*case).map_err(|_| unresolved("a case"))?;
            let member = self.member(case_node.member, name)?;
            let mut labels = Vec::new();
            let mut is_default = false;
            for label in &case_node.labels {
                match label {
                    CaseLabel::Default => is_default = true,
                    CaseLabel::Value(expr) => {
                        let value = self.ast.eval_integral(*expr).map_err(|_| {
                            Idl4CppError::Unevaluated {
                                what: "a case label".to_string(),
                                at: format!("{at}::{}", member.idl),
                            }
                        })?;
                        labels.push(value);
                        explicit.push(value);
                    }
                }
            }
            if let Some(existing) = all.iter_mut().find(|c| c.member.name == member.name) {
                existing.labels.extend(labels);
                existing.default |= is_default;
                continue;
            }
            all.push(Case {
                member,
                labels,
                default: is_default,
            });
        }
        let has_default = all.iter().any(|c| c.default);
        let free = domain.first_free(&explicit);
        let literal = |value: i128| self.discriminator_literal(&disc, disc_id, value, &at);
        // "If the union has a default case, the default constructor shall
        // initialize the discriminator, and the selected member field ...
        // If it does not, the default constructor shall initialize the
        // union to the first discriminant value specified in the IDL
        // definition."
        let (initial_disc, initial_index) = if let Some(index) = all.iter().position(|c| c.default)
        {
            let value = free.ok_or_else(|| Idl4CppError::NoCppForm {
                construct: "a default case whose labels leave no discriminator value".to_string(),
                at: at.clone(),
            })?;
            (value, Some(index))
        } else {
            let first = all
                .first()
                .and_then(|c| c.labels.first().copied())
                .ok_or_else(|| Idl4CppError::NoCppForm {
                    construct: "a union without a case".to_string(),
                    at: at.clone(),
                })?;
            (first, Some(0))
        };
        let implicit_default = !has_default && free.is_some();
        let cxx11 = self.cxx11();
        let dt = &disc.text;
        let slots = verbatim::declaration_slots(self.ast, id, verbatim::aliases::CPP);
        let mut w = Writer::new();
        for case in &all {
            for text in &case.member.before {
                w.nested(text);
            }
        }
        for text in &slots.before {
            w.verbatim(text);
        }
        w.open(&format!("class {name}"));
        w.line("public:");
        for text in &slots.begin {
            w.verbatim(text);
        }
        // Constructors, destructor, assignment.
        let init_index = initial_index.map_or_else(|| "-1".to_string(), |i| i.to_string());
        w.line(&format!(
            "{name}() : _disc({}), _member(-1) {{ _construct({init_index}); }}",
            literal(initial_disc)?
        ));
        w.line(&format!(
            "{name}(const {name}& other) : _disc(other._disc), _member(-1) {{ _copy(other); }}"
        ));
        if cxx11 {
            w.line(&format!(
                "{name}({name}&& other) : _disc(other._disc), _member(-1) {{ _move(other); }}"
            ));
        }
        w.line(&format!("~{name}() {{ _destroy(); }}"));
        w.open(&format!("{name}& operator=(const {name}& other)"));
        w.line("if (this != &other) {");
        w.line("    _destroy();");
        w.line("    _disc = other._disc;");
        w.line("    _copy(other);");
        w.line("}");
        w.line("return *this;");
        w.close();
        if cxx11 {
            w.open(&format!("{name}& operator=({name}&& other)"));
            w.line("if (this != &other) {");
            w.line("    _destroy();");
            w.line("    _disc = other._disc;");
            w.line("    _move(other);");
            w.line("}");
            w.line("return *this;");
            w.close();
        }
        // _d().
        w.line("");
        w.line(&format!("{dt} _d() const {{ return _disc; }}"));
        w.open(&format!("void _d({dt} value)"));
        w.line("// Setting the discriminator to a value that selects another member");
        w.line("// \"may result in an error\" (7.2.4.3.2): it is one.");
        w.line("if (_index_of(value) != _member) {");
        w.line(&format!(
            "    throw std::invalid_argument(\"{name}::_d: the value selects another member\");"
        ));
        w.line("}");
        w.line("_disc = value;");
        w.close();
        // Members.
        for (index, case) in all.iter().enumerate() {
            let m = &case.member;
            let n = &m.name;
            let t = &m.decl.text;
            let first_label = if case.default {
                free
            } else {
                case.labels.first().copied()
            };
            let Some(first_label) = first_label else {
                return Err(Idl4CppError::NoCppForm {
                    construct: "a member without a discriminator value".to_string(),
                    at: format!("{at}::{}", m.idl),
                });
            };
            let first = literal(first_label)?;
            let by_value =
                m.decl.by_value() && m.decl.text == m.plain.text && m.decl.suffix.is_empty();
            w.line("");
            if by_value {
                w.line(&format!(
                    "void {n}({t} value) {{ _select({index}, {first}); *_p{index}() = value; }}"
                ));
                // "the accessor shall return by value".
                w.line(&format!(
                    "{t} {n}() {{ _check({index}, \"{n}\"); return *_p{index}(); }}"
                ));
                w.line(&format!(
                    "{t} {n}() const {{ _check({index}, \"{n}\"); return *_p{index}(); }}"
                ));
            } else {
                w.line(&format!(
                    "void {n}(const {t}& value) {{ _select({index}, {first}); *_p{index}() = value; }}"
                ));
                if cxx11 {
                    w.line(&format!(
                        "void {n}({t}&& value) {{ _select({index}, {first}); *_p{index}() = std::move(value); }}"
                    ));
                }
                w.line(&format!(
                    "{t}& {n}() {{ _check({index}, \"{n}\"); return *_p{index}(); }}"
                ));
                w.line(&format!(
                    "const {t}& {n}() const {{ _check({index}, \"{n}\"); return *_p{index}(); }}"
                ));
            }
            // "For each union member that can be selected by more than one
            // case label, the C++ class shall include a modifier method
            // that allows selecting the corresponding discriminator value
            // with an extra parameter".
            if case.labels.len() > 1 || case.default {
                let check = format!("_index_of(discriminator) != {index}");
                let body = |assign: &str| {
                    format!(
                        "{{ if ({check}) throw std::invalid_argument(\"{name}::{n}: the \
                         discriminator selects another member\"); _select({index}, discriminator); \
                         *_p{index}() = {assign}; }}"
                    )
                };
                if by_value {
                    w.line(&format!(
                        "void {n}({t} value, {dt} discriminator) {}",
                        body("value")
                    ));
                } else {
                    w.line(&format!(
                        "void {n}(const {t}& value, {dt} discriminator) {}",
                        body("value")
                    ));
                    if cxx11 {
                        w.line(&format!(
                            "void {n}({t}&& value, {dt} discriminator) {}",
                            body("std::move(value)")
                        ));
                    }
                }
            }
        }
        // _default().
        if implicit_default && let Some(value) = free {
            w.line("");
            w.line(&format!(
                "void _default() {{ _destroy(); _disc = {}; }}",
                literal(value)?
            ));
        }
        // Comparison and swap.
        w.line("");
        w.open(&format!(
            "friend bool operator==(const {name}& a, const {name}& b)"
        ));
        w.line("if (a._disc != b._disc || a._member != b._member) return false;");
        w.open("switch (a._member)");
        for (index, case) in all.iter().enumerate() {
            w.line(&format!(
                "case {index}: return {};",
                self.equal_text(
                    &case.member,
                    &format!("*a._p{index}()"),
                    &format!("*b._p{index}()")
                )
            ));
        }
        w.line("default: return true;");
        w.close();
        w.close();
        w.line(&format!(
            "friend bool operator!=(const {name}& a, const {name}& b) {{ return !(a == b); }}"
        ));
        w.open(&format!("void swap({name}& other)"));
        if cxx11 {
            w.line(&format!("{name} held(std::move(other));"));
            w.line("other = std::move(*this);");
            w.line("*this = std::move(held);");
        } else {
            w.line(&format!("{name} held(other);"));
            w.line("other = *this;");
            w.line("*this = held;");
        }
        w.close();
        for text in &slots.end {
            w.verbatim(text);
        }
        // The storage.
        w.line("");
        w.line("private:");
        w.open(&format!("int _index_of({dt} value) const"));
        for (index, case) in all.iter().enumerate() {
            if case.labels.is_empty() {
                continue;
            }
            let tests: Vec<String> = case
                .labels
                .iter()
                .map(|v| literal(*v).map(|l| format!("value == {l}")))
                .collect::<Result<_, _>>()?;
            w.line(&format!("if ({}) return {index};", tests.join(" || ")));
        }
        match all.iter().position(|c| c.default) {
            Some(index) => w.line(&format!("return {index};")),
            None => w.line("return -1;"),
        }
        w.close();
        w.open("void _construct(int index)");
        w.open("switch (index)");
        for (index, case) in all.iter().enumerate() {
            let m = &case.member;
            let value = match &m.init {
                Some(init) if !init.is_empty() => format!("{}({init})", m.decl.text),
                _ => format!("{}()", m.decl.text),
            };
            w.line(&format!(
                "case {index}: ::new (static_cast<void*>(&_storage)) {}; break;",
                value
            ));
        }
        w.line("default: break;");
        w.close();
        w.line("_member = index;");
        w.close();
        w.open("void _destroy()");
        w.open("switch (_member)");
        for (index, case) in all.iter().enumerate() {
            let alias = format!("_T{index}");
            let _ = case;
            w.line(&format!("case {index}: _p{index}()->~{alias}(); break;"));
        }
        w.line("default: break;");
        w.close();
        w.line("_member = -1;");
        w.close();
        w.open(&format!("void _copy(const {name}& other)"));
        w.open("switch (other._member)");
        for (index, case) in all.iter().enumerate() {
            w.line(&format!(
                "case {index}: ::new (static_cast<void*>(&_storage)) {}(*other._p{index}()); break;",
                case.member.decl.text
            ));
        }
        w.line("default: break;");
        w.close();
        w.line("_member = other._member;");
        w.close();
        if cxx11 {
            w.open(&format!("void _move({name}& other)"));
            w.open("switch (other._member)");
            for (index, case) in all.iter().enumerate() {
                w.line(&format!(
                    "case {index}: ::new (static_cast<void*>(&_storage)) {}(std::move(*other._p{index}())); break;",
                    case.member.decl.text
                ));
            }
            w.line("default: break;");
            w.close();
            w.line("_member = other._member;");
            w.close();
        }
        w.open(&format!("void _select(int index, {dt} value)"));
        w.line("if (_member != index) {");
        w.line("    _destroy();");
        w.line("    _construct(index);");
        w.line("}");
        w.line("_disc = value;");
        w.close();
        w.open("void _check(int index, const char* member) const");
        w.line("if (_member != index) {");
        if self.options.platform == Platform::Corba {
            // A.1.2.3: "The CORBA mapping for an IDL union, shall throw a
            // CORBA::BAD_PARAM system exception when an invalid field is
            // accessed whose discriminant does not match that already
            // set in the union object."
            w.line("    (void)member;");
            w.line("    throw ::CORBA::BAD_PARAM();");
        } else {
            w.line(&format!(
                "    throw std::logic_error(std::string(\"{name}::\") + member + \": the discriminator selects another member\");"
            ));
        }
        w.line("}");
        w.close();
        for (index, case) in all.iter().enumerate() {
            let t = &case.member.decl.text;
            w.line(&format!("typedef {t} _T{index};"));
            w.line(&format!(
                "_T{index}* _p{index}() {{ return reinterpret_cast<_T{index}*>(&_storage); }}"
            ));
            w.line(&format!(
                "const _T{index}* _p{index}() const {{ return reinterpret_cast<const _T{index}*>(&_storage); }}"
            ));
        }
        w.line("");
        w.line(&format!("{dt} _disc;"));
        w.line("int _member;");
        let types: Vec<String> = (0..all.len()).map(|i| format!("_T{i}")).collect();
        if cxx11 {
            w.line(&format!(
                "typename std::aligned_union<1, {}>::type _storage;",
                if types.is_empty() {
                    "char".to_string()
                } else {
                    types.join(", ")
                }
            ));
        } else {
            let sizes = types.iter().fold("1".to_string(), |acc, t| {
                format!("(sizeof({t}) > {acc} ? sizeof({t}) : {acc})")
            });
            w.open("union");
            w.line(&format!("char _bytes[{sizes}];"));
            w.line("long double _align_ld;");
            w.line("::omg::types::int64_t _align_ll;");
            w.line("void* _align_p;");
            w.close_with(" _storage;");
        }
        w.close_with(";");
        for text in &slots.after {
            w.verbatim(text);
        }
        w.line(&format!(
            "inline void swap({name}& a, {name}& b) {{ a.swap(b); }}"
        ));
        Ok(w.finish())
    }

    /// The C++ literal of a discriminator value.
    fn discriminator_literal(
        &self,
        disc: &CppType,
        disc_id: Id<TypeNode>,
        value: i128,
        at: &str,
    ) -> Result<String, Idl4CppError> {
        match &disc.shape {
            Shape::Bool => Ok(if value == 0 { "false" } else { "true" }.to_string()),
            Shape::Enum { .. } => self.enumerator_with_value(disc_id, value)?.ok_or_else(|| {
                Idl4CppError::Unevaluated {
                    what: format!("an enumerator of value {value}"),
                    at: at.to_string(),
                }
            }),
            Shape::Char { .. } | Shape::Integer { .. } => {
                let (bits, signed) = match disc.shape {
                    Shape::Integer { bits, signed } => (bits, signed),
                    _ => (32, true),
                };
                let number = integer_literal(value, bits, signed).ok_or_else(|| {
                    Idl4CppError::Unevaluated {
                        what: format!("the label {value}"),
                        at: at.to_string(),
                    }
                })?;
                Ok(format!("static_cast<{}>({number})", disc.text))
            }
            _ => Err(Idl4CppError::NoCppForm {
                construct: "a discriminator of this type".to_string(),
                at: at.to_string(),
            }),
        }
    }

    fn discriminator_domain(&self, disc: Id<TypeNode>, at: &str) -> Result<Domain, Idl4CppError> {
        let node = self
            .ast
            .type_(disc)
            .map_err(|_| unresolved("a discriminator"))?;
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
                        return Err(Idl4CppError::NoCppForm {
                            construct: "a floating-point discriminator".to_string(),
                            at: at.to_string(),
                        });
                    }
                };
                Domain::Range(low, high)
            }
            _ => {
                return Err(Idl4CppError::NoCppForm {
                    construct: "a discriminator of this type".to_string(),
                    at: at.to_string(),
                });
            }
        })
    }

    // -- enumerations, §7.2.4.3.3, Table 7.18 and Annex C.2.1.2.2.1 ------------------------

    fn enumeration(&self, id: Id<TypeNode>, name: &str) -> Result<String, Idl4CppError> {
        let node = self
            .ast
            .type_(id)
            .map_err(|_| unresolved("an enumeration"))?;
        let slots = verbatim::declaration_slots(self.ast, id, verbatim::aliases::CPP);
        let mut w = Writer::new();
        for text in &slots.before {
            w.verbatim(text);
        }
        let mut entries = Vec::new();
        for enumerator in self.ast.enumerators_of(id) {
            let e = self
                .ast
                .enumerator(*enumerator)
                .map_err(|_| unresolved("an enumerator"))?;
            let value = self
                .ast
                .effective_enumerator_value(*enumerator)
                .ok_or_else(|| Idl4CppError::Unevaluated {
                    what: "the value of an enumerator".to_string(),
                    at: format!("{name}::{}", e.name),
                })?;
            entries.push(format!("{} = {value}", self.enumerator_name(id, &e.name)));
        }
        match self.options.standard {
            Standard::Cxx11 => {
                let underlying = node
                    .facets
                    .xtypes
                    .as_ref()
                    .and_then(|x| x.bit_bound)
                    .map(|bound| format!(" : {}", enum_underlying(bound)));
                w.open(&format!(
                    "enum class {name}{}",
                    underlying.unwrap_or_default()
                ));
            }
            // Annex C.2.1.2.2.1: "An IDL enum shall be mapped to a C++ enum";
            // Table C.5: "@bit_bound ... has no impact".
            Standard::Cxx03 => w.open(&format!("enum {name}")),
        }
        for text in &slots.begin {
            w.verbatim(text);
        }
        let count = entries.len();
        for (index, entry) in entries.into_iter().enumerate() {
            let comma = if index + 1 < count { "," } else { "" };
            w.line(&format!("{entry}{comma}"));
        }
        for text in &slots.end {
            w.verbatim(text);
        }
        w.close_with(";");
        for text in &slots.after {
            w.verbatim(text);
        }
        Ok(w.finish())
    }

    // -- bit sets, §7.14.3.2 -------------------------------------------------------------------

    fn bitset(&self, id: Id<TypeNode>, name: &str) -> Result<String, Idl4CppError> {
        let node = self.ast.type_(id).map_err(|_| unresolved("a bit set"))?;
        let TypeKind::Bitset { base, .. } = &node.kind else {
            return Err(unresolved("a bit set"));
        };
        let at = self.origin_of_type(id);
        let base = match base.as_ref().and_then(Ref::resolved) {
            Some(base) => Some(self.qualified(self.through_aliases(base)?)?),
            None => None,
        };
        let inherit = base
            .as_ref()
            .map_or_else(String::new, |base| format!(" : {base}"));
        let mut w = Writer::new();
        let mut names_in = Vec::new();
        w.open(&format!("struct {name}{inherit}"));
        for field in self.ast.bitfields_of(id) {
            let f = self
                .ast
                .bitfield(*field)
                .map_err(|_| unresolved("a bit field"))?;
            let holder = match &f.holder_type {
                Some(holder) => self.cpp_type(holder, &at)?.text,
                // OMG IDL 4.2 §7.4.13.4.3.2: "the smallest type able to
                // store the bit field with no loss (i.e., boolean if size
                // is 1, octet if it is between 2 and 8, unsigned short ...
                // unsigned long ... unsigned long long".
                None => self.unsigned_holder(f.bits),
            };
            match &f.name {
                Some(field_name) => {
                    let field_name = names::name(field_name);
                    w.line(&format!("{holder} {field_name} : {};", f.bits));
                    names_in.push(field_name);
                }
                None => w.line(&format!("{holder} : {};", f.bits)),
            }
        }
        w.close_with(";");
        let mut terms = Vec::new();
        if let Some(base) = &base {
            terms.push(format!(
                "static_cast<const {base}&>(a) == static_cast<const {base}&>(b)"
            ));
        }
        for field_name in &names_in {
            terms.push(format!("a.{field_name} == b.{field_name}"));
        }
        let all = if terms.is_empty() {
            "true".to_string()
        } else {
            terms.join(" && ")
        };
        w.line(&format!(
            "inline bool operator==(const {name}& a, const {name}& b) {{ return {all}; }}"
        ));
        w.line(&format!(
            "inline bool operator!=(const {name}& a, const {name}& b) {{ return !(a == b); }}"
        ));
        Ok(w.finish())
    }

    fn unsigned_holder(&self, bits: u16) -> String {
        let prefix = match self.options.standard {
            Standard::Cxx11 => "std::",
            Standard::Cxx03 => "::omg::types::",
        };
        match bits {
            1 => "bool".to_string(),
            2..=8 => format!("{prefix}uint8_t"),
            9..=16 => format!("{prefix}uint16_t"),
            17..=32 => format!("{prefix}uint32_t"),
            _ => format!("{prefix}uint64_t"),
        }
    }

    // -- bit masks, §7.14.3.3 and Annex C.2.4.1.1 ------------------------------------------------

    fn bitmask(&self, id: Id<TypeNode>, name: &str) -> Result<String, Idl4CppError> {
        let bound = self.ast.effective_bit_bound(id);
        let underlying = self.unsigned_holder(bound.max(2));
        let cxx11 = self.cxx11();
        let mut w = Writer::new();
        w.open(&format!("struct {name}"));
        // "An unscoped enum named _flags with an explicitly defined
        // underlying type"; the example names it <Name>Bits, and A.1.1.8
        // specializes the CORBA traits on that name: both name it.
        if cxx11 {
            w.open(&format!("enum _flags : {underlying}"));
        } else {
            w.open("enum _flags");
        }
        let bits = self.ast.bits_of(id).to_vec();
        let count = bits.len();
        for (index, bit) in bits.iter().enumerate() {
            let b = self
                .ast
                .bit_value(*bit)
                .map_err(|_| unresolved("a bit value"))?;
            let position =
                self.ast
                    .effective_bit_position(*bit)
                    .ok_or_else(|| Idl4CppError::Unevaluated {
                        what: "the position of a bit value".to_string(),
                        at: format!("{name}::{}", b.name),
                    })?;
            let value = 1u128 << position;
            let suffix = if value > u128::from(u32::MAX) {
                "ULL"
            } else {
                "U"
            };
            let comma = if index + 1 < count { "," } else { "" };
            w.line(&format!(
                "{} = 0x{value:X}{suffix}{comma}",
                names::name(&b.name)
            ));
        }
        w.close_with(";");
        w.line(&format!("typedef _flags {name}Bits;"));
        w.line("");
        w.line(&format!("{name}() : _value(0U) {{}}"));
        w.line(&format!(
            "{name}(const {name}& other) : _value(other._value) {{}}"
        ));
        w.line(&format!("{name}({underlying} v) : _value(v) {{}}"));
        // With the copy constructor above, the implicit copy assignment is
        // deprecated (clang: -Wdeprecated-copy-with-user-provided-copy).
        if cxx11 {
            w.line(&format!("{name}& operator=(const {name}&) = default;"));
        } else {
            w.line(&format!(
                "{name}& operator=(const {name}& other) {{ _value = other._value; return *this; }}"
            ));
        }
        w.line("");
        w.line(&format!(
            "operator {underlying}() const {{ return _value; }}"
        ));
        w.line("// \"An implementation of the function call operator that returns _value.\"");
        w.line(&format!(
            "{underlying} operator()() const {{ return _value; }}"
        ));
        w.line(&format!(
            "{name}& operator|=({underlying} other) {{ _value |= other; return *this; }}"
        ));
        w.line(&format!(
            "{name}& operator&=({underlying} other) {{ _value &= other; return *this; }}"
        ));
        w.line(&format!(
            "{name}& operator^=({underlying} other) {{ _value ^= other; return *this; }}"
        ));
        w.line(&format!(
            "friend bool operator==(const {name}& a, const {name}& b) {{ return a._value == b._value; }}"
        ));
        w.line(&format!(
            "friend bool operator!=(const {name}& a, const {name}& b) {{ return a._value != b._value; }}"
        ));
        w.line("");
        w.line("private:");
        w.line(&format!("{underlying} _value;"));
        w.close_with(";");
        Ok(w.finish())
    }

    // -- exceptions, §7.4.1 and A.1.2 -----------------------------------------------------------------

    /// An exception class: "The mapped class shall extend the
    /// std::exception class", `CORBA::UserException` on the CORBA
    /// platform (A.1.2).
    pub(super) fn exception(
        &self,
        id: Id<zerodds_ast::ExceptionNode>,
    ) -> Result<String, Idl4CppError> {
        let node = self
            .ast
            .exception(id)
            .map_err(|_| unresolved("an exception"))?;
        let name = names::name(&node.name);
        let at = self.origin(AnyId::Exception(id, node.node_id));
        let mut mapped = Vec::new();
        for member in &node.members {
            mapped.push(self.member(*member, &at)?);
        }
        let cxx11 = self.cxx11();
        let corba = self.options.platform == Platform::Corba;
        let base = if corba {
            "::CORBA::UserException"
        } else {
            "std::exception"
        };
        let mut w = Writer::new();
        for member in &mapped {
            for text in &member.before {
                w.nested(text);
            }
        }
        w.open(&format!("class {name} : public {base}"));
        w.line("public:");
        let inits: Vec<String> = mapped
            .iter()
            .filter_map(|m| Self::initializer(m, &format!("{}_", m.name)))
            .collect();
        let mut default_inits = inits.clone();
        default_inits.push(format!("what_(\"{name}\")"));
        if default_inits.is_empty() {
            w.line(&format!("{name}() {{}}"));
        } else {
            w.line(&format!("{name}() : {} {{}}", default_inits.join(", ")));
        }
        if cxx11 {
            w.line(&format!("{name}(const {name}&) = default;"));
            w.line(&format!("{name}({name}&&) = default;"));
        }
        // "an explicit constructor with one parameter for each exception
        // member ... and shall also include a parameter of type const char
        // * to provide explanatory information". A.1.2 keeps it: its
        // example is `AnException(int32_t error_code, const char* what)`.
        let mut params: Vec<String> = mapped
            .iter()
            .map(|m| {
                if m.decl.by_value() && m.decl.text == m.plain.text {
                    format!("{} {}", m.decl.text, m.name)
                } else {
                    format!("const {}& {}", m.decl.text, m.name)
                }
            })
            .collect();
        let mut sets: Vec<String> = mapped
            .iter()
            .map(|m| format!("{n}_({n})", n = m.name))
            .collect();
        params.push("const char* what".to_string());
        sets.push("what_(what ? what : \"\")".to_string());
        if !params.is_empty() {
            w.line(&format!(
                "explicit {name}({}) : {} {{}}",
                params.join(", "),
                sets.join(", ")
            ));
        }
        if cxx11 {
            w.line(&format!("~{name}() noexcept override = default;"));
            w.line(&format!("{name}& operator=(const {name}&) = default;"));
            w.line(&format!("{name}& operator=({name}&&) = default;"));
        } else {
            w.line(&format!("virtual ~{name}() throw() {{}}"));
        }
        for member in &mapped {
            let n = &member.name;
            let t = &member.decl.text;
            w.line("");
            if member.decl.by_value() && member.decl.text == member.plain.text {
                w.line(&format!("void {n}({t} value) {{ {n}_ = value; }}"));
                w.line(&format!("{t} {n}() const {{ return {n}_; }}"));
            } else {
                w.line(&format!("void {n}(const {t}& value) {{ {n}_ = value; }}"));
                w.line(&format!("const {t}& {n}() const {{ return {n}_; }}"));
                w.line(&format!("{t}& {n}() {{ return {n}_; }}"));
            }
        }
        if corba {
            self.corba_exception_members(&mut w, id)?;
        }
        // A.1.2: "Each Exception class has to override what()".
        w.line("");
        if cxx11 {
            w.line("const char* what() const noexcept override { return what_.c_str(); }");
        } else {
            w.line("virtual const char* what() const throw() { return what_.c_str(); }");
        }
        w.line("");
        w.line("private:");
        for member in &mapped {
            w.line(&format!(
                "{} {}_{};",
                member.decl.text, member.name, member.decl.suffix
            ));
        }
        w.line("std::string what_;");
        w.close_with(";");
        Ok(w.finish())
    }

    // -- trait specializations, §7.1.4 -------------------------------------------------------------

    /// The specializations of the traits in `omg::types` for what the
    /// header declares.
    pub(super) fn trait_specializations(&self, items: &[Item]) -> Result<String, Idl4CppError> {
        let mut w = Writer::new();
        for id in self.declared_types(items) {
            let node = self.ast.type_(id).map_err(|_| unresolved("a type"))?;
            let q = self.qualified(id)?;
            match &node.kind {
                TypeKind::Structure { .. } | TypeKind::Union { .. } | TypeKind::Bitset { .. } => {
                    w.line(&format!(
                        "template <> struct is_idl_type<{q}> : std::true_type {{}};"
                    ));
                }
                TypeKind::Bitmask { .. } => {
                    let bound = self.ast.effective_bit_bound(id);
                    w.line(&format!(
                        "template <> struct is_idl_type<{q}> : std::true_type {{}};"
                    ));
                    w.line(&format!(
                        "template <> struct bit_bound<{q}> : std::integral_constant<std::uint32_t, {bound}> {{}};"
                    ));
                    w.line(&format!(
                        "template <> struct underlying_type<{q}> {{ typedef {} type; }};",
                        self.unsigned_holder(bound.max(2))
                    ));
                }
                TypeKind::Enumeration { .. } => {
                    if let Some(bound) = node.facets.xtypes.as_ref().and_then(|x| x.bit_bound) {
                        w.line(&format!(
                            "template <> struct bit_bound<{q}> : std::integral_constant<std::uint32_t, {bound}> {{}};"
                        ));
                        w.line(&format!(
                            "template <> struct underlying_type<{q}> {{ typedef {} type; }};",
                            enum_underlying(bound)
                        ));
                    }
                }
                TypeKind::Valuetype { .. } | TypeKind::ValueBox { .. } => {
                    w.line(&format!(
                        "template <> struct is_idl_type<{q}> : std::true_type {{}};"
                    ));
                    w.line(&format!(
                        "template <> struct is_reference_type<{q}> : std::true_type {{}};"
                    ));
                }
                _ => {}
            }
        }
        for item in items {
            if let Item::Interface(id) = item {
                let q = self.interface_name(*id)?;
                w.line(&format!(
                    "template <> struct is_idl_type<{q}> : std::true_type {{}};"
                ));
                w.line(&format!(
                    "template <> struct is_reference_type<{q}> : std::true_type {{}};"
                ));
            }
        }
        Ok(w.finish())
    }
}

/// The underlying type of an `enum class` under `@bit_bound` (Table 7.18):
/// "int8_t, for bit bound values between 1 and 8; int16_t, for bit bound
/// values between 9 and 16; int32_t, for bit bound values between 17 and
/// 32; and int64_t for bit bound values between 33 and 64".
fn enum_underlying(bound: u16) -> &'static str {
    match bound {
        0..=8 => "std::int8_t",
        9..=16 => "std::int16_t",
        17..=32 => "std::int32_t",
        _ => "std::int64_t",
    }
}

/// A C++ integer literal for `value` as a value of the integer type of
/// `bits` and `signed`, or `None` when it does not fit.
pub(super) fn integer_literal(value: i128, bits: u8, signed: bool) -> Option<String> {
    let (low, high) = match (bits, signed) {
        (8, true) => (-128, 127),
        (8, false) => (0, 255),
        (16, true) => (-32_768, 32_767),
        (16, false) => (0, 65_535),
        (32, true) => (i128::from(i32::MIN), i128::from(i32::MAX)),
        (32, false) => (0, i128::from(u32::MAX)),
        (64, true) => (i128::from(i64::MIN), i128::from(i64::MAX)),
        _ => (0, i128::from(u64::MAX)),
    };
    if value < low || value > high {
        return None;
    }
    Some(match (bits, signed) {
        // The least value of a signed type has no literal of its own: its
        // magnitude is one past the greatest.
        (32, true) if value == low => "(-2147483647 - 1)".to_string(),
        (64, true) if value == low => "(-9223372036854775807LL - 1)".to_string(),
        (64, true) => format!("{value}LL"),
        (64, false) => format!("{value}ULL"),
        (32, false) => format!("{value}U"),
        _ => value.to_string(),
    })
}

/// The values a discriminator type has.
enum Domain {
    /// An enumeration's values or a boolean's two.
    Values(Vec<i128>),
    /// An integer or character range.
    Range(i128, i128),
}

impl Domain {
    /// A value no label takes: the first from zero upward, then below.
    fn first_free(&self, taken: &[i128]) -> Option<i128> {
        match self {
            Self::Values(values) => values.iter().copied().find(|v| !taken.contains(v)),
            Self::Range(low, high) => {
                let start = 0.clamp(*low, *high);
                (start..=*high)
                    .take(taken.len() + 1)
                    .find(|v| !taken.contains(v))
                    .or_else(|| {
                        (*low..start)
                            .rev()
                            .take(taken.len() + 1)
                            .find(|v| !taken.contains(v))
                    })
            }
        }
    }
}
