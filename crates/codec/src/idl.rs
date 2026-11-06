// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A described type as IDL text: the declaration the Rust type stands for,
//! and every type it reaches, in an order the IDL front end composes.
//!
//! This is the one place a description becomes a model. The text goes
//! through `zerodds-idl-compose` as a `.idl` file would, so the
//! `TypeObject`, the `TypeIdentifier` and the layout plan of a derived type
//! are what the IDL front end, `zerodds-types` and `zerodds-ast-layout`
//! make of the equivalent IDL, and nothing computes them a second way.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use zerodds_wire::codec::{
    AutoId, CaseDesc, EnumeratorDesc, HashId, MemberDesc, Primitive, TypeDesc, TypeKind, ValueDesc,
};

/// Why a description has no IDL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdlError {
    /// The type the error is in.
    pub type_name: String,
    /// What is wrong.
    pub detail: String,
}

impl core::fmt::Display for IdlError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}: {}", self.type_name, self.detail)
    }
}

impl std::error::Error for IdlError {}

/// How deep a description may nest before it is refused.
const MAX_DEPTH: usize = 64;

/// The IDL text of `root` and of every type it reaches, dependencies
/// first; a type that reaches itself is declared ahead (`struct Node;`,
/// `union Choice;`).
///
/// # Errors
/// [`IdlError`] for a shape IDL cannot declare: an `Option` that is not a
/// member's own, an array or an `Option` as a sequence element or a union
/// branch's, two descriptions under one name, a name that is no IDL
/// scoped name.
pub fn idl_of(root: &'static TypeDesc) -> Result<String, IdlError> {
    let mut walk = Walk::default();
    walk.visit(root, 0)?;
    let mut out = String::new();
    for (name, keyword) in &walk.forward {
        let _ = writeln!(
            out,
            "{}",
            in_modules(name, &format!("{keyword} {};", escape(simple(name))))
        );
    }
    for (name, desc) in &walk.order {
        out.push_str(&in_modules(name, &declaration(name, desc)?));
        out.push('\n');
    }
    Ok(out)
}

#[derive(Default)]
struct Walk {
    /// Every type by its scoped name: `true` once declared.
    seen: BTreeMap<String, (bool, &'static TypeDesc)>,
    /// The types in declaration order, dependencies first.
    order: Vec<(String, &'static TypeDesc)>,
    /// The types a member reaches before their declaration is complete,
    /// with the keyword that declares them ahead.
    forward: Vec<(String, &'static str)>,
}

impl Walk {
    /// zerodds-lint: recursion-depth 64 (one level per type a member
    /// reaches; bounded by `MAX_DEPTH`)
    fn visit(&mut self, desc: &'static TypeDesc, depth: usize) -> Result<(), IdlError> {
        let name = desc.scoped_name().into_owned();
        if depth > MAX_DEPTH {
            return Err(refused(&name, "a type that nests past 64 levels"));
        }
        check_scoped_name(&name)?;
        if let Some((done, earlier)) = self.seen.get(&name) {
            if !same(&name, desc, earlier) {
                return Err(refused(
                    &name,
                    "two Rust types described under one IDL name",
                ));
            }
            if !done && !self.forward.iter().any(|(forward, _)| *forward == name) {
                let keyword = match desc.kind {
                    TypeKind::Union { .. } => "union",
                    _ => "struct",
                };
                self.forward.push((name.clone(), keyword));
            }
            return Ok(());
        }
        self.seen.insert(name.clone(), (false, desc));
        match desc.kind {
            TypeKind::Struct { values, count, .. } => {
                for index in 0..count {
                    let value = values(index)
                        .ok_or_else(|| refused(&name, "a value description for every member"))?;
                    self.visit_value(&name, value, depth + 1)?;
                }
            }
            TypeKind::Union {
                discriminator,
                values,
                count,
                ..
            } => {
                self.visit_value(&name, discriminator(), depth + 1)?;
                for index in 0..count {
                    let value = values(index)
                        .ok_or_else(|| refused(&name, "a value description for every branch"))?;
                    self.visit_value(&name, value, depth + 1)?;
                }
            }
            TypeKind::Enum { .. } => {}
        }
        if let Some(entry) = self.seen.get_mut(&name) {
            entry.0 = true;
        }
        self.order.push((name, desc));
        Ok(())
    }

    /// zerodds-lint: recursion-depth 64 (one level per collection of a
    /// member's value; bounded by `MAX_DEPTH`)
    fn visit_value(&mut self, owner: &str, value: ValueDesc, depth: usize) -> Result<(), IdlError> {
        if depth > MAX_DEPTH {
            return Err(refused(owner, "a member type that nests past 64 levels"));
        }
        match value {
            ValueDesc::Primitive(_) | ValueDesc::String { .. } => Ok(()),
            ValueDesc::Sequence { element, .. } => self.visit_value(owner, element(), depth + 1),
            ValueDesc::Array { element, .. } => self.visit_value(owner, element(), depth + 1),
            ValueDesc::Optional { inner } => self.visit_value(owner, inner(), depth + 1),
            ValueDesc::Type { desc } => self.visit(desc, depth + 1),
        }
    }
}

/// Whether two descriptions of one name declare the same type: a constant
/// may be duplicated per codegen unit, so the address does not decide it,
/// and the declaration text does.
fn same(name: &str, a: &TypeDesc, b: &TypeDesc) -> bool {
    core::ptr::eq(a, b)
        || matches!((declaration(name, a), declaration(name, b)), (Ok(x), Ok(y)) if x == y)
}

fn refused(type_name: &str, detail: &str) -> IdlError {
    IdlError {
        type_name: type_name.to_string(),
        detail: detail.to_string(),
    }
}

/// A scoped name of IDL identifiers (IDL 4.2 §7.2.3): a generic type's
/// name is made from its arguments at run time, and a template that
/// leaves something else in it is refused here, by name.
fn check_scoped_name(name: &str) -> Result<(), IdlError> {
    let ok = !name.is_empty()
        && name.split("::").all(|segment| {
            let mut chars = segment.chars();
            chars
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        });
    if ok {
        Ok(())
    } else {
        Err(refused(
            name,
            "an IDL scoped name: identifiers separated by `::`",
        ))
    }
}

/// `app::sub::Track` → `Track`.
fn simple(name: &str) -> &str {
    name.rfind("::").map_or(name, |at| &name[at + 2..])
}

/// `declaration` inside `module app { module sub { … }; };`.
fn in_modules(name: &str, declaration: &str) -> String {
    let mut modules: Vec<&str> = name.split("::").collect();
    // The last segment is the type's own name.
    modules.pop();
    let mut out = String::new();
    for module in &modules {
        let _ = write!(out, "module {} {{ ", escape(module));
    }
    out.push_str(declaration);
    for _ in &modules {
        out.push_str(" };");
    }
    out
}

/// `::app::Track` for a reference to a described type.
fn reference(desc: &TypeDesc) -> String {
    format!(
        "::{}",
        desc.scoped_name()
            .split("::")
            .map(escape)
            .collect::<Vec<_>>()
            .join("::")
    )
}

fn autoid_and_nested(out: &mut String, autoid: Option<AutoId>, nested: bool) {
    match autoid {
        Some(AutoId::Sequential) => out.push_str(" @autoid(SEQUENTIAL)"),
        Some(AutoId::Hash) => out.push_str(" @autoid(HASH)"),
        None => {}
    }
    if nested {
        out.push_str(" @nested");
    }
}

/// The declaration of one type.
fn declaration(name: &str, desc: &TypeDesc) -> Result<String, IdlError> {
    match desc.kind {
        TypeKind::Struct {
            extensibility,
            autoid,
            nested,
            members,
            values,
            count,
        } => {
            if members.len() != count {
                return Err(refused(name, "as many member descriptions as member types"));
            }
            let mut out = String::new();
            out.push_str(extensibility.annotation());
            autoid_and_nested(&mut out, autoid, nested);
            let _ = write!(out, " struct {} {{", escape(simple(name)));
            for (index, member) in members.iter().enumerate() {
                let value = values(index)
                    .ok_or_else(|| refused(name, "a value description for every member"))?;
                out.push(' ');
                out.push_str(&member_declaration(name, member, value)?);
            }
            out.push_str(" };");
            Ok(out)
        }
        TypeKind::Union {
            extensibility,
            autoid,
            nested,
            discriminator,
            key,
            cases,
            values,
            count,
        } => {
            if cases.len() != count {
                return Err(refused(name, "as many case descriptions as branch types"));
            }
            let discriminator = discriminator();
            let (switch, _) = spelled(name, discriminator, None, false)?;
            // DDS-XTypes 1.3 §7.2.2.4.4.4.8's own example:
            // `union MyCommand switch (@key CommandKind)`.
            let switch = if key {
                annotated(vec![String::from("@key")], &switch) + &switch
            } else {
                switch
            };
            let mut out = String::new();
            out.push_str(extensibility.annotation());
            autoid_and_nested(&mut out, autoid, nested);
            let _ = write!(out, " union {} switch ({switch}) {{", escape(simple(name)));
            for (index, case) in cases.iter().enumerate() {
                let value = values(index)
                    .ok_or_else(|| refused(name, "a value description for every branch"))?;
                out.push(' ');
                out.push_str(&case_declaration(name, discriminator, case, value)?);
            }
            out.push_str(" };");
            Ok(out)
        }
        TypeKind::Enum {
            enumerators,
            bit_bound,
            extensibility,
        } => {
            let mut out = String::new();
            if let Some(kind) = extensibility {
                out.push_str(kind.annotation());
                out.push(' ');
            }
            if let Some(bits) = bit_bound {
                let _ = write!(out, "@bit_bound({bits}) ");
            }
            let _ = write!(out, "enum {} {{ ", escape(simple(name)));
            let listed: Vec<String> = enumerators
                .iter()
                .map(|enumerator| match enumerator.value {
                    Some(value) => format!("@value({value}) {}", escape(enumerator.name)),
                    None => escape(enumerator.name),
                })
                .collect();
            out.push_str(&listed.join(", "));
            out.push_str(" };");
            Ok(out)
        }
    }
}

/// The annotations of a member or a branch, the last one written with its
/// parameter when the type behind it starts with `::`.
///
/// IDL 4.2 §7.4.15 (annotations): an annotation's name is a scoped name,
/// so a bare `@key` followed by `::app::Inner` reads as the annotation
/// `key::app::Inner`.
fn annotated(mut annotations: Vec<String>, type_: &str) -> String {
    if type_.starts_with("::")
        && let Some(last) = annotations.last_mut()
    {
        match last.as_str() {
            "@key" => *last = String::from("@key(TRUE)"),
            "@optional" => *last = String::from("@optional(TRUE)"),
            "@hashid" => *last = String::from("@hashid(\"\")"),
            _ => {}
        }
    }
    let mut out = String::new();
    for annotation in &annotations {
        out.push_str(annotation);
        out.push(' ');
    }
    out
}

fn id_annotations(annotations: &mut Vec<String>, id: Option<u32>, hash_id: Option<HashId>) {
    if let Some(id) = id {
        annotations.push(format!("@id({id})"));
    }
    match hash_id {
        Some(HashId::Name) => annotations.push(String::from("@hashid")),
        Some(HashId::Text(text)) => {
            annotations.push(format!("@hashid(\"{}\")", text.escape_default()));
        }
        None => {}
    }
}

/// One member: its annotations, its type and its name.
fn member_declaration(
    owner: &str,
    member: &MemberDesc,
    value: ValueDesc,
) -> Result<String, IdlError> {
    let mut annotations: Vec<String> = Vec::new();
    if member.key {
        annotations.push(String::from("@key"));
    }
    id_annotations(&mut annotations, member.id, member.hash_id);
    let value = match value {
        ValueDesc::Optional { inner } => {
            annotations.push(String::from("@optional"));
            inner()
        }
        other => other,
    };
    let (type_, dims) = spelled(owner, value, member.bound, true)?;
    let mut out = annotated(annotations, &type_);
    let _ = write!(out, "{type_} {}", escape(member.name));
    for dim in dims {
        let _ = write!(out, "[{dim}]");
    }
    out.push(';');
    Ok(out)
}

/// One case of a union: its labels, then its branch member.
fn case_declaration(
    owner: &str,
    discriminator: ValueDesc,
    case: &CaseDesc,
    value: ValueDesc,
) -> Result<String, IdlError> {
    let mut out = String::new();
    for label in (case.labels)() {
        let _ = write!(
            out,
            "case {}: ",
            label_spelling(owner, discriminator, label)?
        );
    }
    if case.default {
        out.push_str("default: ");
    }
    if out.is_empty() {
        return Err(refused(owner, "a case with a label or default"));
    }
    if matches!(value, ValueDesc::Optional { .. }) {
        return Err(refused(
            owner,
            "an Option as a union branch: IDL has @optional on a struct member only",
        ));
    }
    let mut annotations = Vec::new();
    id_annotations(&mut annotations, case.id, case.hash_id);
    let (type_, dims) = spelled(owner, value, case.bound, true)?;
    out.push_str(&annotated(annotations, &type_));
    let _ = write!(out, "{type_} {}", escape(case.name));
    for dim in dims {
        let _ = write!(out, "[{dim}]");
    }
    out.push(';');
    Ok(out)
}

/// A case label in IDL: an integer, `TRUE`/`FALSE`, a character literal,
/// or the enumerator (named in the scope that encloses its enumeration) whose
/// wire value it is.
fn label_spelling(owner: &str, discriminator: ValueDesc, label: i128) -> Result<String, IdlError> {
    match discriminator {
        ValueDesc::Primitive(Primitive::Boolean) => match label {
            0 => Ok(String::from("FALSE")),
            1 => Ok(String::from("TRUE")),
            _ => Err(refused(owner, "a boolean label of 0 or 1")),
        },
        ValueDesc::Primitive(Primitive::Char) => {
            let code = u8::try_from(label)
                .map_err(|_| refused(owner, "a char label from U+0000 to U+00FF"))?;
            Ok(
                if code.is_ascii_graphic() && code != b'\'' && code != b'\\' {
                    format!("'{}'", char::from(code))
                } else {
                    format!("'\\x{code:02x}'")
                },
            )
        }
        ValueDesc::Primitive(_) => Ok(format!("{label}")),
        ValueDesc::Type { desc } => match desc.kind {
            TypeKind::Enum { enumerators, .. } => {
                let enumerator = enumerator_of(enumerators, label)
                    .ok_or_else(|| refused(owner, "a label that is the value of an enumerator"))?;
                let scope = desc.scoped_name();
                let module = scope.rfind("::").map_or("", |at| &scope[..at]);
                let mut path = String::from("::");
                for segment in module.split("::").filter(|segment| !segment.is_empty()) {
                    path.push_str(&escape(segment));
                    path.push_str("::");
                }
                path.push_str(&escape(enumerator.name));
                Ok(path)
            }
            _ => Err(refused(
                owner,
                "a discriminator of an integer, char, boolean or enum type",
            )),
        },
        _ => Err(refused(
            owner,
            "a discriminator of an integer, char, boolean or enum type",
        )),
    }
}

/// The enumerator whose value is `value`: an explicit `@value`, or one past
/// the enumerator before it, the first `0`.
fn enumerator_of(enumerators: &[EnumeratorDesc], value: i128) -> Option<&EnumeratorDesc> {
    let mut next = 0_i128;
    for enumerator in enumerators {
        let own = enumerator.value.map_or(next, i128::from);
        if own == value {
            return Some(enumerator);
        }
        next = own + 1;
    }
    None
}

/// The IDL type of a value, and the array dimensions its declarator takes.
///
/// `bound` is the member's own, for its `string` or outermost `sequence`;
/// `declarator` says the value sits where an array declarator may follow.
///
/// zerodds-lint: recursion-depth 64 (one level per collection; the walk
/// above refused deeper descriptions)
fn spelled(
    owner: &str,
    value: ValueDesc,
    bound: Option<u32>,
    declarator: bool,
) -> Result<(String, Vec<u32>), IdlError> {
    Ok(match value {
        ValueDesc::Primitive(primitive) => (primitive.idl().to_string(), Vec::new()),
        ValueDesc::String { bound: own } => (
            match bound.or(own) {
                Some(bound) => format!("string<{bound}>"),
                None => String::from("string"),
            },
            Vec::new(),
        ),
        ValueDesc::Sequence {
            element,
            bound: own,
        } => {
            let element = element();
            if matches!(
                element,
                ValueDesc::Array { .. } | ValueDesc::Optional { .. }
            ) {
                return Err(refused(
                    owner,
                    "a sequence of arrays or of Option values, which IDL declares only through a \
                     typedef this version does not write",
                ));
            }
            let (element, _) = spelled(owner, element, None, false)?;
            (
                match bound.or(own) {
                    Some(bound) => format!("sequence<{element}, {bound}>"),
                    None => format!("sequence<{element}>"),
                },
                Vec::new(),
            )
        }
        ValueDesc::Array { element, len } => {
            if !declarator {
                return Err(refused(
                    owner,
                    "an array inside a sequence, which IDL declares only through a typedef",
                ));
            }
            let (type_, inner) = spelled(owner, element(), bound, true)?;
            let mut dims = vec![len];
            dims.extend(inner);
            (type_, dims)
        }
        ValueDesc::Optional { .. } => {
            return Err(refused(
                owner,
                "an Option that is not a member's own: IDL has @optional on a member only",
            ));
        }
        ValueDesc::Type { desc } => (reference(desc), Vec::new()),
    })
}

/// IDL 4.2 §7.2.3.1: an identifier that collides with a keyword, compared
/// without case, is written with a leading underscore, which the front
/// end removes again.
fn escape(name: &str) -> String {
    if KEYWORDS
        .iter()
        .any(|keyword| keyword.eq_ignore_ascii_case(name))
    {
        format!("_{name}")
    } else {
        name.to_string()
    }
}

/// The keywords of IDL 4.2 §7.2.4, Table 7-6.
const KEYWORDS: &[&str] = &[
    "abstract",
    "any",
    "alias",
    "attribute",
    "bitfield",
    "bitmask",
    "bitset",
    "boolean",
    "case",
    "char",
    "component",
    "connector",
    "const",
    "consumes",
    "context",
    "custom",
    "default",
    "double",
    "exception",
    "emits",
    "enum",
    "eventtype",
    "factory",
    "FALSE",
    "finder",
    "fixed",
    "float",
    "getraises",
    "getter",
    "home",
    "import",
    "in",
    "inout",
    "interface",
    "local",
    "long",
    "manages",
    "map",
    "mirrorport",
    "module",
    "multiple",
    "native",
    "Object",
    "octet",
    "oneway",
    "out",
    "primarykey",
    "private",
    "port",
    "porttype",
    "provides",
    "public",
    "publishes",
    "raises",
    "readonly",
    "setraises",
    "setter",
    "sequence",
    "short",
    "string",
    "struct",
    "supports",
    "switch",
    "TRUE",
    "truncatable",
    "typedef",
    "typeid",
    "typename",
    "typeprefix",
    "unsigned",
    "union",
    "uses",
    "ValueBase",
    "valuetype",
    "void",
    "wchar",
    "wstring",
    "int8",
    "uint8",
    "int16",
    "int32",
    "int64",
    "uint16",
    "uint32",
    "uint64",
];
