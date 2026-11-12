// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The classic OMG IDL to C++ 1.3 mapping of topic types, as a layer over the native one.
//!
//! [`CppMapping::Classic`](super::cpp::CppMapping::Classic) keeps the wire codec, the
//! `TypeObject` and the endpoint gates where they are, as `CppMapping::Psm` does, and gives the
//! application the types the classic mapping names, in C++03:
//!
//! | IDL | native (`zd_native::…`) | classic (the IDL's own namespaces) |
//! |---|---|---|
//! | `long`, `unsigned long`, … | `std::int32_t`, … | `CORBA::Long`, `CORBA::ULong`, … |
//! | `octet`, `boolean`, `char`, `wchar` | `zerodds::wire::Octet`, `bool`, `char`, `std::uint16_t` | `CORBA::Octet`, `CORBA::Boolean`, `CORBA::Char`, `CORBA::WChar` |
//! | `string`, `wstring` | `std::string` | `CORBA::String_var`, `CORBA::WString_var` |
//! | `sequence<T>` | `std::vector<T>` | `zerodds::classic::Sequence<T>` |
//! | `T a[2][3]` | `std::array<std::array<T, 3>, 2>` | `T a[2][3]` |
//! | `map<K,V>` | `zerodds::Map<K, V>` | `zerodds::classic::Map<K, V>` (a string key is a `std::string`) |
//! | `@optional T m` | `T *`, NULL when absent | `T *`, NULL when absent (`char *` for a string) |
//! | `@external T m` | `std::shared_ptr<T>` | `zerodds::classic::Var<T>` |
//! | `struct` | struct, the base a public base class | struct with public members, `S_var` |
//! | `union` | `_d()` and one accessor per case | class: `_d()`, one accessor per case |
//! | `enum` | `enum class` | plain `enum`, the values written out |
//! | `bitmask` | an unsigned integer and constants | a struct with the enumeration `_flags` |
//! | `bitset` | a class of integers | a struct of bit fields |
//! | `typedef` | `using` | `typedef` |
//!
//! For the building blocks OMG IDL 4 added to what the classic mapping knew, the rules are those
//! of IDL4 to C++ 1.0 Annex D.2: a map is a class with `maximum()`, `length()`, `insert()` and
//! `erase()` (D.2.1.3.1), a bit set the bit fields of §7.14.3.2 (D.2.1.3.2), a bit mask the
//! struct of Annex C.2.4.1.1 (D.2.1.3.3), an `@optional` member a plain pointer (Table D.3),
//! `@default` the initial value (Table D.4), and `@verbatim` text where its placement says
//! (D.2.3.5).
//!
//! The classic declarations stand between the markers `ZD-CLASSIC-BEGIN` and `ZD-CLASSIC-END` and
//! are C++03: they include `zerodds/classic.hpp` and nothing newer. The native types, the codec and
//! `zerodds::classic::Conv` after them are C++17. `Conv<C>` (`zerodds/classic/convert.hpp`) names
//! the native type of each classic type and converts both ways; the `topic_type_support` of a
//! classic type calls the native one's `encode` and `decode` through it, so a classic sample takes
//! the same bytes, the same `TypeObject` and the same type name as the native one.
//!
//! Not remapped, and left to the native types by name: constants, exceptions. Refused by name,
//! because the classic mapping has no form for them: `fixed`, value types, `any`, and an array as
//! the element of a sequence.

use super::cpp::{cxx_name, module_names};
use super::cpp_psm::{in_namespaces, native_path, psm_path};
use crate::adapter::c::ordered_types;
use crate::adapter::cst::MaterialiseError;
use crate::default_common::resolve_alias;
use zerodds_ast::{Ast, CaseLabel, CharWidth, MemberNode, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;

/// The marker that opens the C++03 part of a classic header.
pub const BEGIN_MARKER: &str = "ZD-CLASSIC-BEGIN";
/// The marker that closes it.
pub const END_MARKER: &str = "ZD-CLASSIC-END";

fn no_form(construct: &str, at: &str) -> MaterialiseError {
    MaterialiseError::NoLanguageForm {
        construct: construct.to_string(),
        language: "C++ (classic mapping)",
        at: at.to_string(),
    }
}

const MAX_DEPTH: usize = 64;

/// A classic type as it stands in a declaration: the type and, for an array, the dimensions
/// that follow the declared name.
struct Spelled {
    type_: String,
    dims: String,
}

impl Spelled {
    fn plain(type_: String) -> Self {
        Self {
            type_,
            dims: String::new(),
        }
    }

    /// `T name[2][3]`.
    fn declare(&self, name: &str) -> String {
        format!("{} {name}{}", self.type_, self.dims)
    }
}

const fn primitive(kind: PrimitiveKind) -> &'static str {
    match kind {
        PrimitiveKind::Boolean => "::CORBA::Boolean",
        PrimitiveKind::Char => "::CORBA::Char",
        PrimitiveKind::WChar => "::CORBA::WChar",
        PrimitiveKind::Octet => "::CORBA::Octet",
        PrimitiveKind::UInt8 => "::CORBA::UInt8",
        PrimitiveKind::Int8 => "::CORBA::Int8",
        PrimitiveKind::Int16 => "::CORBA::Short",
        PrimitiveKind::UInt16 => "::CORBA::UShort",
        PrimitiveKind::Int32 => "::CORBA::Long",
        PrimitiveKind::UInt32 => "::CORBA::ULong",
        PrimitiveKind::Int64 => "::CORBA::LongLong",
        PrimitiveKind::UInt64 => "::CORBA::ULongLong",
        PrimitiveKind::Float32 => "::CORBA::Float",
        PrimitiveKind::Float64 => "::CORBA::Double",
        PrimitiveKind::Float128 => "::CORBA::LongDouble",
    }
}

/// The classic spelling of the type `id`.
///
/// # Errors
/// A construct the classic mapping has no form for, or a type nested too deep.
fn spell(ast: &Ast, id: Id<TypeNode>, at: &str, key: bool) -> Result<Spelled, MaterialiseError> {
    spell_at(ast, id, at, key, 0)
}

/// zerodds-lint: recursion-depth 64 (element, key and value types; bounded by `MAX_DEPTH`)
fn spell_at(
    ast: &Ast,
    id: Id<TypeNode>,
    at: &str,
    key: bool,
    depth: usize,
) -> Result<Spelled, MaterialiseError> {
    if depth > MAX_DEPTH {
        return Err(MaterialiseError::Unevaluated {
            construct: format!("type spelling — it nests past {MAX_DEPTH}"),
            at: at.to_string(),
        });
    }
    let Ok(node) = ast.type_(id) else {
        return Err(no_form("a type the model would not hand over", at));
    };
    let inner =
        |r: &zerodds_ast_core::Ref<TypeNode>, key: bool| -> Result<String, MaterialiseError> {
            let Some(target) = r.resolved() else {
                return Err(no_form("an unresolved element type", at));
            };
            let spelled = spell_at(ast, target, at, key, depth + 1)?;
            if spelled.dims.is_empty() {
                Ok(spelled.type_)
            } else {
                Err(no_form(
                    "an array as the element of a sequence or a map, which has no slice type here",
                    at,
                ))
            }
        };
    Ok(match &node.kind {
        TypeKind::Primitive(kind) => Spelled::plain(primitive(*kind).to_string()),
        // A string key is a `std::string`: `String_var` converts to `const char*` and would be
        // compared by address.
        TypeKind::String { .. } if key => Spelled::plain("std::string".to_string()),
        TypeKind::String {
            width: CharWidth::WChar,
            ..
        } => Spelled::plain("::CORBA::WString_var".to_string()),
        TypeKind::String { .. } => Spelled::plain("::CORBA::String_var".to_string()),
        TypeKind::Fixed { .. } => {
            return Err(no_form(
                "a fixed-point type; the classic mapping's CORBA::Fixed is not provided",
                at,
            ));
        }
        TypeKind::Sequence { element, .. } => Spelled::plain(format!(
            "::zerodds::classic::Sequence< {} >",
            inner(element, false)?
        )),
        TypeKind::Array { element, dims } => {
            let element = inner(element, false)?;
            let mut suffix = String::new();
            for dim in dims {
                let length =
                    ast.eval_integral(*dim)
                        .map_err(|_| MaterialiseError::Unevaluated {
                            construct: "array length".to_string(),
                            at: at.to_string(),
                        })?;
                suffix.push_str(&format!("[{length}]"));
            }
            Spelled {
                type_: element,
                dims: suffix,
            }
        }
        TypeKind::Map {
            key: k,
            value,
            bound,
        } => {
            let bound = match bound.expr() {
                Some(expr) => format!(
                    ", {}",
                    ast.eval_integral(expr)
                        .map_err(|_| MaterialiseError::Unevaluated {
                            construct: "map bound".to_string(),
                            at: at.to_string(),
                        })?
                ),
                None => String::new(),
            };
            // IDL4 to C++ 1.0 Annex D.2.1.3.1.
            Spelled::plain(format!(
                "::zerodds::classic::Map< {}, {}{bound} >",
                inner(k, true)?,
                inner(value, false)?
            ))
        }
        TypeKind::Alias { aliased } if node.name.is_empty() => {
            let Some(target) = aliased.resolved() else {
                return Err(no_form("an unresolved typedef", at));
            };
            spell_at(ast, target, at, key, depth + 1)?
        }
        TypeKind::Structure { .. }
        | TypeKind::Union { .. }
        | TypeKind::Enumeration { .. }
        | TypeKind::Bitmask { .. }
        | TypeKind::Bitset { .. }
        | TypeKind::Alias { .. } => Spelled::plain(psm_path(ast, id)),
        _ => return Err(no_form("this construct", at)),
    })
}

/// A member's declaration, its classic type, and the default its constructor gives it.
struct Member {
    name: String,
    spelled: Spelled,
    /// The mem-initialiser: value-initialisation, the first enumerator of an enumeration, or the
    /// value of `@default`.
    init: String,
    /// Whether it is the plain pointer of an `@optional` member, which the structure owns.
    pointer: bool,
    /// Whether the member itself is a C array: declared with dimensions, or of a `typedef`
    /// that names an array. No mem-initialiser copies one.
    array: bool,
}

fn enumerator_path(ast: &Ast, enumeration: Id<TypeNode>, enumerator: &str) -> Option<String> {
    let node = ast.type_(enumeration).ok()?;
    let chain = module_names(ast, node.module).ok()?;
    let mut path = String::new();
    for module in chain {
        path.push_str(&format!("::{module}"));
    }
    Some(format!("{path}::{}", cxx_name(enumerator)))
}

fn member(ast: &Ast, member: &MemberNode, owner: &str) -> Result<Member, MaterialiseError> {
    let Some(target) = member.type_.resolved() else {
        return Err(MaterialiseError::Unevaluated {
            construct: "member type".to_string(),
            at: format!("{owner}::{}", member.name),
        });
    };
    let name = cxx_name(&member.name);
    let inner = spell(ast, target, &member.name, false)?;
    // The same precedence as the native struct (`cpp::held`): `@external` first, then
    // `@optional`, then a member that only closes a cycle.
    let held = super::cpp::held(ast, member);
    let array = held == super::cpp::Held::Value
        && (!inner.dims.is_empty()
            || matches!(
                ast.type_(resolve_alias(ast, target)).map(|node| &node.kind),
                Ok(TypeKind::Array { .. })
            ));
    let (spelled, init) = if held == super::cpp::Held::Shared {
        (
            Spelled::plain(format!("::zerodds::classic::Var< {} >", inner.type_)),
            format!("{name}()"),
        )
    } else if held == super::cpp::Held::Plain {
        // IDL4 to C++ 1.0 Table D.3: "Each IDL struct member annotated with @optional shall be
        // represented using plain pointers"; a `_var` type becomes "the corresponding plain
        // pointer type", `char*` for `String_var`. NULL after construction.
        if !inner.dims.is_empty() {
            return Err(no_form("an optional array", &member.name));
        }
        let pointer = match inner.type_.as_str() {
            "::CORBA::String_var" => "char*".to_string(),
            "::CORBA::WString_var" => "::CORBA::WChar*".to_string(),
            other => format!("{other}*"),
        };
        (Spelled::plain(pointer), format!("{name}()"))
    } else {
        // Table 9: an enumeration starts at its first enumerator, not at zero.
        let resolved = resolve_alias(ast, target);
        let first = matches!(
            ast.type_(resolved).map(|node| &node.kind),
            Ok(TypeKind::Enumeration { .. })
        )
        .then(|| crate::emit::default_enumerator_name(ast, resolved))
        .flatten()
        .and_then(|first| enumerator_path(ast, resolved, &first));
        // Table D.4: "C++ elements declared as result of the mappings defined in this
        // specification containing a @default annotation shall be initialized to the value of
        // the annotation."
        let default = default_value(ast, member, resolved)?;
        let init = match (default, first, inner.dims.is_empty()) {
            (Some(value), _, true) => format!("{name}({value})"),
            (None, Some(first), true) => format!("{name}({first})"),
            _ => format!("{name}()"),
        };
        (inner, init)
    };
    Ok(Member {
        name,
        pointer: held == super::cpp::Held::Plain,
        array,
        spelled,
        init,
    })
}

/// The classic literal of a member's `@default`, for a primitive, a string or an enumeration.
fn default_value(
    ast: &Ast,
    member: &MemberNode,
    resolved: Id<TypeNode>,
) -> Result<Option<String>, MaterialiseError> {
    let Some(expr) = member.facets.xtypes.as_ref().and_then(|x| x.default_value) else {
        return Ok(None);
    };
    let unevaluated = || MaterialiseError::Unevaluated {
        construct: "the value of @default".to_string(),
        at: member.name.clone(),
    };
    let value = ast.eval_const(expr).map_err(|_| unevaluated())?;
    let Ok(node) = ast.type_(resolved) else {
        return Err(unevaluated());
    };
    use zerodds_ast::ConstValue as V;
    Ok(Some(match (&node.kind, &value) {
        (TypeKind::Enumeration { .. }, V::Integer(number)) => {
            let mut found = None;
            for enumerator in ast.enumerators_of(resolved) {
                if ast.effective_enumerator_value(*enumerator) == Some(*number) {
                    let e = ast.enumerator(*enumerator).map_err(|_| unevaluated())?;
                    found = enumerator_path(ast, resolved, &e.name);
                }
            }
            found.ok_or_else(unevaluated)?
        }
        (_, V::Boolean(flag)) => flag.to_string(),
        (TypeKind::Primitive(kind), V::Integer(number)) => {
            let suffix = if i32::try_from(*number).is_ok() {
                ""
            } else if *number < 0 {
                "LL"
            } else {
                "ULL"
            };
            format!("static_cast< {} >({number}{suffix})", primitive(*kind))
        }
        (_, V::Float { mantissa, exponent }) => {
            let mut text = mantissa.clone();
            if !text.contains('.') && exponent.is_none() {
                text.push_str(".0");
            }
            if let Some(exponent) = exponent {
                text.push('e');
                text.push_str(exponent);
            }
            text
        }
        (_, V::Char(c)) => format!("static_cast< ::CORBA::Char >({})", u32::from(*c)),
        (_, V::WChar(point)) => format!("static_cast< ::CORBA::WChar >({point})"),
        (_, V::String(text)) => crate::emit::idl4_cpp::text::string_literal(text),
        (_, V::WString(points)) => crate::emit::idl4_cpp::text::wstring_literal(points),
        _ => return Err(no_form("@default of this type", &member.name)),
    }))
}

/// The struct of a structure, its `_var`, and the specialisation of `zerodds::classic::Conv`.
fn structure(
    ast: &Ast,
    id: Id<TypeNode>,
    node: &TypeNode,
) -> Result<(String, String), MaterialiseError> {
    let name = cxx_name(&node.name);
    let base = ast
        .base_of(id)
        .filter(|_| matches!(node.kind, TypeKind::Structure { .. }));
    let mut own = Vec::new();
    for handle in ast.members_of(id) {
        let Ok(m) = ast.member(*handle) else {
            return Err(MaterialiseError::NoNode {
                kind: "member",
                at: node.name.clone(),
            });
        };
        own.push(member(ast, m, &node.name)?);
    }
    let slots = crate::verbatim::declaration_slots(ast, id, crate::verbatim::aliases::CPP);
    let base_path = base.map(|b| psm_path(ast, b));
    let mut text = String::new();
    for verbatim in &slots.before {
        text.push_str(&format!("{verbatim}\n"));
    }
    text.push_str(&format!(
        "/// `{}` as the classic mapping has it: public members.\nstruct {name}{} {{\n",
        node.name,
        base_path
            .as_ref()
            .map_or(String::new(), |b| format!(" : public {b}")),
    ));
    for verbatim in &slots.begin {
        text.push_str(&format!("{verbatim}\n"));
    }
    for m in &own {
        text.push_str(&format!("    {};\n", m.spelled.declare(&m.name)));
    }
    if own.is_empty() {
        text.push_str(&format!("    {name}() {{}}\n"));
    } else {
        let inits: Vec<&str> = own.iter().map(|m| m.init.as_str()).collect();
        text.push_str(&format!("    {name}() : {} {{}}\n", inits.join(", ")));
    }
    // The structure owns the plain pointer of an `@optional` member: copies it deeply and
    // releases it, as Table D.5 has the enclosing object do for an external one.
    if own.iter().any(|m| m.pointer) {
        let mut copies = Vec::new();
        if let Some(b) = &base_path {
            copies.push(format!("{b}(other)"));
        }
        let mut body = Vec::new();
        for m in &own {
            if m.pointer {
                copies.push(format!("{0}(::zerodds::classic::clone(other.{0}))", m.name));
            } else if !m.array {
                copies.push(format!("{0}(other.{0})", m.name));
            } else {
                body.push(format!(
                    "::zerodds::classic::copy_value({0}, other.{0});",
                    m.name
                ));
            }
        }
        text.push_str(&format!(
            "    {name}(const {name}& other) : {} {{ {} }}\n",
            copies.join(", "),
            body.join(" ")
        ));
        let mut assign = Vec::new();
        if let Some(b) = &base_path {
            assign.push(format!("{b}::operator=(other);"));
        }
        for m in &own {
            if m.pointer {
                assign.push(format!(
                    "{{ {1} copy = ::zerodds::classic::clone(other.{0}); ::zerodds::classic::release({0}); {0} = copy; }}",
                    m.name, m.spelled.type_
                ));
            } else {
                assign.push(format!(
                    "::zerodds::classic::copy_value({0}, other.{0});",
                    m.name
                ));
            }
        }
        text.push_str(&format!(
            "    {name}& operator=(const {name}& other) {{\n        if (this != &other) {{ {} }}\n        return *this;\n    }}\n",
            assign.join(" ")
        ));
        let releases: Vec<String> = own
            .iter()
            .filter(|m| m.pointer)
            .map(|m| format!("::zerodds::classic::release({});", m.name))
            .collect();
        text.push_str(&format!("    ~{name}() {{ {} }}\n", releases.join(" ")));
    }
    for verbatim in &slots.end {
        text.push_str(&format!("{verbatim}\n"));
    }
    text.push_str(&format!(
        "}};\ntypedef ::zerodds::classic::Var< {name} > {name}_var;\n"
    ));
    for verbatim in &slots.after {
        text.push_str(&format!("{verbatim}\n"));
    }

    // The conversion: every member, the base's first, by name.
    let path = psm_path(ast, id);
    let native = native_path(ast, id);
    let mut all = Vec::new();
    for handle in ast.all_members_of(id) {
        let Ok(m) = ast.member(handle) else {
            return Err(MaterialiseError::NoNode {
                kind: "member",
                at: node.name.clone(),
            });
        };
        all.push(member(ast, m, &node.name)?);
    }
    let mut conv = format!(
        "template <>\nstruct Conv<{path}> {{\n    typedef {native} Native;\n    static Native to_native(const {path}& v) {{\n        Native n;\n"
    );
    for m in &all {
        conv.push_str(&format!(
            "        to_native_member(n.{0}, v.{0});\n",
            m.name
        ));
    }
    conv.push_str(&format!(
        "        return n;\n    }}\n    static {path} from_native(const Native& n) {{\n        {path} v;\n"
    ));
    for m in &all {
        conv.push_str(&format!(
            "        from_native_member(v.{0}, n.{0});\n",
            m.name
        ));
    }
    conv.push_str("        return v;\n    }\n};\n");
    Ok((text, conv))
}

/// The class of a union and the specialisation of `zerodds::classic::Conv`.
fn union_class(
    ast: &Ast,
    id: Id<TypeNode>,
    node: &TypeNode,
    discriminator: &zerodds_ast_core::Ref<TypeNode>,
) -> Result<(String, String), MaterialiseError> {
    let name = cxx_name(&node.name);
    let Some(disc) = discriminator.resolved() else {
        return Err(MaterialiseError::Unevaluated {
            construct: "union discriminator".to_string(),
            at: node.name.clone(),
        });
    };
    let disc_type = spell(ast, disc, &node.name, false)?.type_;
    struct Case {
        name: String,
        type_: String,
        label: Option<i128>,
    }
    let mut cases: Vec<Case> = Vec::new();
    for case in ast.cases_of(id) {
        let Ok(case_node) = ast.case(*case) else {
            return Err(MaterialiseError::NoNode {
                kind: "case",
                at: node.name.clone(),
            });
        };
        let Ok(m) = ast.member(case_node.member) else {
            return Err(MaterialiseError::NoNode {
                kind: "member",
                at: node.name.clone(),
            });
        };
        let mut label = None;
        for candidate in &case_node.labels {
            if let CaseLabel::Value(expr) = candidate {
                label =
                    Some(
                        ast.eval_integral(*expr)
                            .map_err(|_| MaterialiseError::Unevaluated {
                                construct: "union case label".to_string(),
                                at: node.name.clone(),
                            })?,
                    );
                break;
            }
        }
        let built = member(ast, m, &node.name)?;
        if !built.spelled.dims.is_empty() {
            return Err(no_form("an array as the case of a union", &m.name));
        }
        cases.push(Case {
            name: built.name,
            type_: built.spelled.type_,
            label,
        });
    }
    let mut inits = vec!["d_()".to_string()];
    inits.extend(cases.iter().map(|c| format!("{}_()", c.name)));
    let mut text = format!(
        "/// `{}` as the classic mapping has it: `_d()` and an accessor and a modifier per case.\nclass {name} {{\npublic:\n    {name}() : {} {{}}\n\n    {disc_type} _d() const {{ return d_; }}\n    void _d({disc_type} value) {{ d_ = value; }}\n\n",
        node.name,
        inits.join(", ")
    );
    for case in &cases {
        let select = case.label.map_or(String::new(), |value| {
            format!(" d_ = static_cast< {disc_type} >({value});")
        });
        text.push_str(&format!(
            "    const {0}& {1}() const {{ return {1}_; }}\n    {0}& {1}() {{ return {1}_; }}\n    void {1}(const {0}& value) {{ {1}_ = value;{select} }}\n",
            case.type_, case.name
        ));
    }
    let slots = crate::verbatim::declaration_slots(ast, id, crate::verbatim::aliases::CPP);
    for verbatim in &slots.end {
        text.push_str(&format!("{verbatim}\n"));
    }
    text.push_str(&format!("\nprivate:\n    {disc_type} d_;\n"));
    for case in &cases {
        text.push_str(&format!("    {} {}_;\n", case.type_, case.name));
    }
    text.push_str(&format!(
        "}};\ntypedef ::zerodds::classic::Var< {name} > {name}_var;\n"
    ));
    if let Some(at) = text.find("public:\n") {
        let begin: String = slots.begin.iter().map(|v| format!("{v}\n")).collect();
        text.insert_str(at + "public:\n".len(), &begin);
    }
    let before: String = slots.before.iter().map(|v| format!("{v}\n")).collect();
    text.insert_str(0, &before);
    for verbatim in &slots.after {
        text.push_str(&format!("{verbatim}\n"));
    }

    let path = psm_path(ast, id);
    let native = native_path(ast, id);
    let mut conv = format!(
        "template <>\nstruct Conv<{path}> {{\n    typedef {native} Native;\n    static Native to_native(const {path}& v) {{\n        Native n;\n"
    );
    // The cases first: a setter selects its own discriminator value; the discriminator last.
    for case in &cases {
        conv.push_str(&format!(
            "        {{\n            typedef std::remove_cv<std::remove_reference<decltype(n.{0}())>::type>::type Member;\n            Member member{{}};\n            to_native_member(member, v.{0}());\n            n.{0}(member);\n        }}\n",
            case.name
        ));
    }
    conv.push_str(&format!(
        "        n._d(Conv<{disc_type}>::to_native(v._d()));\n        return n;\n    }}\n    static {path} from_native(const Native& n) {{\n        {path} v;\n"
    ));
    for case in &cases {
        conv.push_str(&format!(
            "        {{\n            {1} member;\n            from_native_member(member, n.{0}());\n            v.{0}(member);\n        }}\n",
            case.name, case.type_
        ));
    }
    conv.push_str(&format!(
        "        v._d(Conv<{disc_type}>::from_native(n._d()));\n        return v;\n    }}\n}};\n"
    ));
    Ok((text, conv))
}

/// The text of the classic layer for `ast`: the C++03 declarations between the markers, and the
/// `zerodds::classic::Conv` specialisations after them.
///
/// # Errors
/// A construct the classic mapping has no form for, or a model that contradicts itself.
pub fn classic_layer(ast: &Ast) -> Result<String, MaterialiseError> {
    let order = ordered_types(ast);
    let pair = crate::emit::type_pair_cpp::has_pair_types(ast);
    let runtime_defined = |node: &TypeNode| -> Result<bool, MaterialiseError> {
        Ok(pair
            && crate::emit::type_pair_cpp::is_runtime_defined(
                &module_names(ast, node.module)?,
                &node.name,
            ))
    };
    let mut forward = String::new();
    let mut types = String::new();
    let mut conversions = String::new();

    for id in &order {
        let Ok(node) = ast.type_(*id) else {
            return Err(no_form(
                "a type the model would not hand over",
                "forward declarations",
            ));
        };
        if node.name.is_empty() || runtime_defined(node)? {
            continue;
        }
        if matches!(
            node.kind,
            TypeKind::Structure { .. } | TypeKind::Union { .. }
        ) {
            let chain = module_names(ast, node.module)?;
            let keyword = if matches!(node.kind, TypeKind::Union { .. }) {
                "class"
            } else {
                "struct"
            };
            forward.push_str(&in_namespaces(
                &chain,
                &format!("{keyword} {};", cxx_name(&node.name)),
            ));
        }
    }
    for id in order {
        let Ok(node) = ast.type_(id) else {
            return Err(no_form(
                "a type the model would not hand over",
                "type declarations",
            ));
        };
        if node.name.is_empty() || runtime_defined(node)? {
            continue;
        }
        let chain = module_names(ast, node.module)?;
        let name = cxx_name(&node.name);
        let (text, conv) = match &node.kind {
            TypeKind::Structure { .. } => {
                let (class, conv) = structure(ast, id, node)?;
                (class, Some(conv))
            }
            TypeKind::Union { discriminator, .. } => {
                let (class, conv) = union_class(ast, id, node, discriminator)?;
                (class, Some(conv))
            }
            TypeKind::Enumeration { .. } => {
                let mut values = Vec::new();
                for enumerator in ast.enumerators_of(id) {
                    let Ok(e) = ast.enumerator(*enumerator) else {
                        return Err(MaterialiseError::NoNode {
                            kind: "enumerator",
                            at: node.name.clone(),
                        });
                    };
                    let value = ast.effective_enumerator_value(*enumerator).ok_or_else(|| {
                        MaterialiseError::Unevaluated {
                            construct: "enumerator value".to_string(),
                            at: e.name.clone(),
                        }
                    })?;
                    if i32::try_from(value).is_err() {
                        return Err(no_form(
                            "an enumerator beyond 32 signed bits, which a C++03 enumeration does not hold",
                            &e.name,
                        ));
                    }
                    values.push(format!("    {} = {value}", cxx_name(&e.name)));
                }
                let native = native_path(ast, id);
                let path = psm_path(ast, id);
                let conv = format!(
                    "template <>\nstruct Conv<{path}> {{\n    typedef {native} Native;\n    typedef std::underlying_type<Native>::type Underlying;\n    static Native to_native({path} v) {{ return static_cast<Native>(static_cast<Underlying>(v)); }}\n    static {path} from_native(Native n) {{ return static_cast<{path}>(static_cast<Underlying>(n)); }}\n}};\n"
                );
                let slots =
                    crate::verbatim::declaration_slots(ast, id, crate::verbatim::aliases::CPP);
                let lines = |texts: &[String]| -> String {
                    texts.iter().map(|v| format!("{v}\n")).collect()
                };
                (
                    format!(
                        "{}/// An enumeration, with the numbering of the IDL.\nenum {name} {{\n{}{}\n{}}};\n{}",
                        lines(&slots.before),
                        lines(&slots.begin),
                        values.join(",\n"),
                        lines(&slots.end),
                        lines(&slots.after)
                    ),
                    Some(conv),
                )
            }
            // IDL4 to C++ 1.0 Annex D.2.1.3.3: "IDL bitmask declarations shall be mapped as
            // defined in Clause C.2.4.1.1": a struct with the enumeration `_flags`, the private
            // `_value` of the unsigned type of the bound, the default and copy constructors,
            // `|=`, `&=`, `^=` and the function call operator.
            TypeKind::Bitmask { bits } => {
                let holder = match ast.effective_bit_bound(id) {
                    0..=8 => "::CORBA::Octet",
                    9..=16 => "::CORBA::UShort",
                    17..=32 => "::CORBA::ULong",
                    _ => "::CORBA::ULongLong",
                };
                let mut flags = Vec::new();
                for bit in bits {
                    let Ok(b) = ast.bit_value(*bit) else {
                        return Err(MaterialiseError::NoNode {
                            kind: "bit",
                            at: node.name.clone(),
                        });
                    };
                    let position = ast.effective_bit_position(*bit).ok_or_else(|| {
                        MaterialiseError::Unevaluated {
                            construct: "bit position".to_string(),
                            at: b.name.clone(),
                        }
                    })?;
                    let value = 1u128 << position;
                    let suffix = if value > u128::from(u32::MAX) {
                        "ULL"
                    } else {
                        "U"
                    };
                    flags.push(format!(
                        "        {} = 0x{value:X}{suffix}",
                        cxx_name(&b.name)
                    ));
                }
                let text = format!(
                    "/// A bitmask as Annex C.2.4.1.1 of IDL4 to C++ 1.0 has it.\n\
                     struct {name} {{\n    enum _flags {{\n{}\n    }};\n    typedef _flags {name}Bits;\n\n    \
                     {name}() : _value(0U) {{}}\n    \
                     {name}(const {name}& other) : _value(other._value) {{}}\n    \
                     {name}({holder} v) : _value(v) {{}}\n    \
                     {name}& operator=(const {name}& other) {{ _value = other._value; return *this; }}\n\n    \
                     operator {holder}() const {{ return _value; }}\n    \
                     {holder} operator()() const {{ return _value; }}\n    \
                     {name}& operator|=({holder} other) {{ _value |= other; return *this; }}\n    \
                     {name}& operator&=({holder} other) {{ _value &= other; return *this; }}\n    \
                     {name}& operator^=({holder} other) {{ _value ^= other; return *this; }}\n    \
                     bool operator==(const {name}& other) const {{ return _value == other._value; }}\n    \
                     bool operator!=(const {name}& other) const {{ return _value != other._value; }}\n\n\
                     private:\n    {holder} _value;\n}};\n",
                    flags.join(",\n")
                );
                let path = psm_path(ast, id);
                let native = native_path(ast, id);
                let conv = format!(
                    "template <>\nstruct Conv<{path}> {{\n    typedef {native} Native;\n    \
                     static Native to_native(const {path}& v) {{ return static_cast<Native>(v()); }}\n    \
                     static {path} from_native(Native n) {{ return {path}(static_cast<{holder}>(n)); }}\n}};\n"
                );
                (text, Some(conv))
            }
            TypeKind::Alias { aliased } => {
                let Some(target) = aliased.resolved() else {
                    return Err(no_form("an unresolved typedef", &node.name));
                };
                let spelled = spell(ast, target, &node.name, false)?;
                (format!("typedef {};\n", spelled.declare(&name)), None)
            }
            // Annex D.2.1.3.2: "IDL bitset declarations shall be mapped as defined in Clause
            // 7.14.3.2": a struct of bit fields, the base a public base class.
            TypeKind::Bitset { base, .. } => {
                let base_path = base
                    .as_ref()
                    .and_then(zerodds_ast_core::Ref::resolved)
                    .map(|b| psm_path(ast, b));
                let mut fields = String::new();
                for field in ast.bitfields_of(id) {
                    let Ok(f) = ast.bitfield(*field) else {
                        return Err(MaterialiseError::NoNode {
                            kind: "bit field",
                            at: node.name.clone(),
                        });
                    };
                    let holder = match &f.holder_type {
                        Some(holder) => {
                            let Some(target) = holder.resolved() else {
                                return Err(no_form("an unresolved bit field type", &node.name));
                            };
                            spell(ast, target, &node.name, false)?.type_
                        }
                        // OMG IDL 4.2 §7.4.13.4.3.2: the smallest type that holds the field.
                        None => match f.bits {
                            1 => "::CORBA::Boolean",
                            2..=8 => "::CORBA::Octet",
                            9..=16 => "::CORBA::UShort",
                            17..=32 => "::CORBA::ULong",
                            _ => "::CORBA::ULongLong",
                        }
                        .to_string(),
                    };
                    match &f.name {
                        Some(field_name) => fields.push_str(&format!(
                            "    {holder} {} : {};\n",
                            cxx_name(field_name),
                            f.bits
                        )),
                        None => fields.push_str(&format!("    {holder} : {};\n", f.bits)),
                    }
                }
                let text = format!(
                    "/// A bit set as §7.14.3.2 of IDL4 to C++ 1.0 has it: bit fields.\nstruct {name}{} {{\n{fields}}};\n",
                    base_path
                        .as_ref()
                        .map_or(String::new(), |b| format!(" : public {b}"))
                );
                // The native class has every field, the base's included, as an integer.
                let path = psm_path(ast, id);
                let native = native_path(ast, id);
                let mut to = String::new();
                let mut from = String::new();
                for field in ast.all_bitfields_of(id) {
                    let Ok(f) = ast.bitfield(field) else {
                        return Err(MaterialiseError::NoNode {
                            kind: "bit field",
                            at: node.name.clone(),
                        });
                    };
                    let Some(field_name) = &f.name else {
                        continue;
                    };
                    let field_name = cxx_name(field_name);
                    to.push_str(&format!(
                        "        n.{field_name} = static_cast<decltype(n.{field_name})>(v.{field_name});\n"
                    ));
                    from.push_str(&format!("        v.{field_name} = n.{field_name};\n"));
                }
                let conv = format!(
                    "template <>\nstruct Conv<{path}> {{\n    typedef {native} Native;\n    \
                     static Native to_native(const {path}& v) {{\n        Native n;\n{to}        return n;\n    }}\n    \
                     static {path} from_native(const Native& n) {{\n        {path} v = {path}();\n{from}        return v;\n    }}\n}};\n"
                );
                (text, Some(conv))
            }
            TypeKind::Native
            | TypeKind::ValueBox { .. }
            | TypeKind::Valuetype { .. }
            | TypeKind::ValueBase
            | TypeKind::ObjectRef { .. } => {
                return Err(no_form(
                    "a native type, value box, value type or interface has no classic mapping here",
                    &node.name,
                ));
            }
            // Spellings of anonymous constructs (a typedef names them), `any`, and annotation
            // declarations, which carry no data: nothing to declare.
            TypeKind::Primitive(_)
            | TypeKind::Fixed { .. }
            | TypeKind::String { .. }
            | TypeKind::Sequence { .. }
            | TypeKind::Array { .. }
            | TypeKind::Map { .. }
            | TypeKind::Any
            | TypeKind::AnnotationType { .. } => continue,
        };
        // DDS-XTypes 1.3 §7.5.1.3.2.1 and §7.5.1.3.3: beside every declared type the three
        // map classes of the clause.
        let mut text = text;
        if matches!(
            node.kind,
            TypeKind::Structure { .. }
                | TypeKind::Union { .. }
                | TypeKind::Enumeration { .. }
                | TypeKind::Bitmask { .. }
                | TypeKind::Bitset { .. }
                | TypeKind::Alias { .. }
        ) {
            for (label, key) in [
                ("UInt32", "::CORBA::ULong"),
                ("UInt64", "::CORBA::ULongLong"),
                ("String", "std::string"),
            ] {
                text.push_str(&format!(
                    "typedef ::zerodds::Map< {key}, {name} > {label}{name}Map;\n"
                ));
            }
        }
        types.push_str(&in_namespaces(&chain, &text));
        types.push('\n');
        if let Some(conv) = conv {
            conversions.push_str(&conv);
            conversions.push('\n');
        }
    }
    Ok(format!(
        "// ---- The classic layer: IDL to C++ 1.3 over the native types above. ----\n\
         // The native types are in `zd_native`; these are what an application writes.\n\
         // `zerodds::classic::Conv` hands them to the codec.\n\n\
         // {BEGIN_MARKER} (C++03 from here)\n\
         #include <string>\n\
         #include \"zerodds/classic.hpp\"\n\
         #include \"zerodds/map.hpp\"\n\
         {forward}\n{types}\
         // {END_MARKER}\n\n\
         #include \"zerodds/classic/convert.hpp\"\n\
         namespace zerodds {{\nnamespace classic {{\n\n{conversions}}} // namespace classic\n}} // namespace zerodds\n"
    ))
}
