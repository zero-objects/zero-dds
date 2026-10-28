// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The DDS-PSM-Cxx 1.0 §8.1 mapping of topic types, as a layer over the native one.
//!
//! [`CppMapping::Psm`](super::cpp::CppMapping::Psm) keeps the wire codec, the
//! `TypeObject` and the endpoint gates where they are and gives the application the types
//! the PSM names:
//!
//! | IDL | native (`zd_native::…`) | PSM (the IDL's own namespaces) |
//! |---|---|---|
//! | `struct` | struct with public fields | class: private state, accessors of §7.4.6, all-members constructor, `swap` (§7.12) |
//! | `wchar` | `std::uint16_t` | `wchar_t` |
//! | `wstring` | `std::string` (UTF-8) | `std::wstring` |
//! | `long double` | `zerodds::wire::Float128` | `long double` |
//! | `T a[2][3]` | `std::array<std::array<T, 3>, 2>` | `dds::core::array<dds::core::array<T, 3>, 2>` |
//! | `map<K,V>` | `std::vector<std::pair<K, V>>` | `std::map<K, V>` |
//! | `@optional T m` | `T *`, NULL when absent | `dds::core::optional<T>` |
//! | `@external T m` | `std::shared_ptr<T>` | `dds::core::external<T>` (XTypes 1.3 §7.5.1.2.3.3) |
//! | `union` | `_d` and one field per case | class: `_d()`, one accessor per case |
//! | `enum`, `bitmask`, `bitset` | the native type | a `using` of the native type, which already has the IDL's name and constants (§8.1.3) |
//!
//! The classes carry no codec. `zerodds::psm::Conv<P>` (`zerodds/psm/convert.hpp`) names the
//! native type of each PSM class and converts both ways; the `topic_type_support` of a PSM
//! class calls the native one's `encode` and `decode` through it. So a PSM sample takes the
//! same bytes, the same `TypeObject` and the same type name as the native one, and costs one
//! conversion in each direction.
//!
//! Not remapped, and left to the native types by name: exceptions, value types, constants.
//! `@Optional` and `@Shared` in the spelling of the PSM text are not recognised: IDL 4.2
//! §8.2 makes annotation names case-sensitive, and the wire carries `@optional`.

use super::cpp::{NATIVE_NAMESPACE, cpp_type_path, cxx_name, module_names, without_native_prefix};
use crate::adapter::c::ordered_types;
use crate::adapter::cst::MaterialiseError;
use zerodds_ast::{Ast, CaseLabel, CharWidth, MemberNode, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::{Id, Ref};

/// `::a::b::Foo` — the C++ name of the declared type `id` in the PSM namespaces.
pub fn psm_path(ast: &Ast, id: Id<TypeNode>) -> String {
    render_path(&without_native_prefix(|| cpp_type_path(ast, id)))
}

/// `::zd_native::a::b::Foo` — the native type behind it.
pub fn native_path(ast: &Ast, id: Id<TypeNode>) -> String {
    let path = without_native_prefix(|| cpp_type_path(ast, id));
    let rendered = render_path(&path);
    format!("::{NATIVE_NAMESPACE}{rendered}")
}

fn render_path(path: &zerodds_lang_cst::common::Path) -> String {
    let segments: Vec<&str> = path
        .segments()
        .iter()
        .map(zerodds_lang_cst::Ident::as_str)
        .collect();
    format!("::{}", segments.join("::"))
}

fn no_form(construct: &str, at: &str) -> MaterialiseError {
    MaterialiseError::NoLanguageForm {
        construct: construct.to_string(),
        language: "C++ (PSM mapping)",
        at: at.to_string(),
    }
}

/// The PSM spelling of the type `id` (Table 7.1).
///
/// # Errors
/// A construct the PSM mapping has no spelling for, an array length that does not evaluate,
/// or a type nested too deep.
pub fn psm_type(ast: &Ast, id: Id<TypeNode>, at: &str) -> Result<String, MaterialiseError> {
    psm_type_at(ast, id, at, 0)
}

const MAX_DEPTH: usize = 64;

/// zerodds-lint: recursion-depth 64 (element and key/value types; bounded by `MAX_DEPTH`)
fn psm_type_at(
    ast: &Ast,
    id: Id<TypeNode>,
    at: &str,
    depth: usize,
) -> Result<String, MaterialiseError> {
    if depth > MAX_DEPTH {
        return Err(MaterialiseError::Unevaluated {
            construct: format!("type spelling — it nests past {MAX_DEPTH}"),
            at: at.to_string(),
        });
    }
    let Ok(node) = ast.type_(id) else {
        return Err(no_form("a type the model would not hand over", at));
    };
    let element = |r: &Ref<TypeNode>| -> Result<String, MaterialiseError> {
        let Some(target) = r.resolved() else {
            return Err(no_form("an unresolved element type", at));
        };
        psm_type_at(ast, target, at, depth + 1)
    };
    Ok(match &node.kind {
        TypeKind::Primitive(kind) => primitive(*kind).to_string(),
        TypeKind::String {
            width: CharWidth::WChar,
            ..
        } => "std::wstring".to_string(),
        TypeKind::String { .. } | TypeKind::Fixed { .. } => "std::string".to_string(),
        TypeKind::Sequence { element: e, .. } => format!("std::vector<{}>", element(e)?),
        TypeKind::Array { element: e, dims } => {
            let mut out = element(e)?;
            for dim in dims.iter().rev() {
                let length =
                    ast.eval_integral(*dim)
                        .map_err(|_| MaterialiseError::Unevaluated {
                            construct: "array length".to_string(),
                            at: at.to_string(),
                        })?;
                out = format!("::dds::core::array<{out}, {length}>");
            }
            out
        }
        TypeKind::Map { key, value, .. } => {
            format!("std::map<{}, {}>", element(key)?, element(value)?)
        }
        TypeKind::Alias { aliased } if node.name.is_empty() => element(aliased)?,
        TypeKind::Structure { .. }
        | TypeKind::Union { .. }
        | TypeKind::Enumeration { .. }
        | TypeKind::Bitmask { .. }
        | TypeKind::Bitset { .. }
        | TypeKind::Alias { .. }
        | TypeKind::Native
        | TypeKind::ValueBox { .. } => psm_path(ast, id),
        _ => return Err(no_form("this construct", at)),
    })
}

/// Table 7.1, the primitives. The names are the global ones `<stdint.h>` declares.
const fn primitive(kind: PrimitiveKind) -> &'static str {
    match kind {
        PrimitiveKind::Boolean => "bool",
        PrimitiveKind::Char => "char",
        PrimitiveKind::WChar => "wchar_t",
        PrimitiveKind::Octet | PrimitiveKind::UInt8 => "uint8_t",
        PrimitiveKind::Int8 => "int8_t",
        PrimitiveKind::Int16 => "int16_t",
        PrimitiveKind::UInt16 => "uint16_t",
        PrimitiveKind::Int32 => "int32_t",
        PrimitiveKind::UInt32 => "uint32_t",
        PrimitiveKind::Int64 => "int64_t",
        PrimitiveKind::UInt64 => "uint64_t",
        PrimitiveKind::Float32 => "float",
        PrimitiveKind::Float64 => "double",
        PrimitiveKind::Float128 => "long double",
    }
}

/// Whether the attribute is "native" in the sense of §7.4.6: a built-in number, character or
/// boolean, or an enumeration — and not a constructed type, a string or a sequence.
fn is_native_attribute(ast: &Ast, id: Id<TypeNode>) -> bool {
    let mut current = id;
    for _ in 0..MAX_DEPTH {
        let Ok(node) = ast.type_(current) else {
            return false;
        };
        match &node.kind {
            TypeKind::Primitive(_) | TypeKind::Enumeration { .. } | TypeKind::Bitmask { .. } => {
                return true;
            }
            TypeKind::Alias { aliased } => match aliased.resolved() {
                Some(next) => current = next,
                None => return false,
            },
            _ => return false,
        }
    }
    false
}

/// One attribute of a PSM class.
struct Attribute {
    /// The accessor name, keyword-escaped.
    name: String,
    /// The state member's name.
    state: String,
    /// The PSM type, with `optional`/`shared_ptr` applied.
    type_: String,
    /// §7.4.6: `NT` or `CT`.
    native: bool,
}

fn attribute(ast: &Ast, member: &MemberNode, owner: &str) -> Result<Attribute, MaterialiseError> {
    let Some(target) = member.type_.resolved() else {
        return Err(MaterialiseError::Unevaluated {
            construct: "member type".to_string(),
            at: format!("{owner}::{}", member.name),
        });
    };
    let inner = psm_type(ast, target, &member.name)?;
    let name = cxx_name(&member.name);
    // The same precedence as the native struct: an indirection first, then `@optional`.
    let (type_, native) = match super::cpp::held(ast, member) {
        super::cpp::Held::Shared => (format!("::dds::core::external<{inner}>"), false),
        super::cpp::Held::Plain => (format!("::dds::core::optional<{inner}>"), false),
        super::cpp::Held::Value => (inner, is_native_attribute(ast, target)),
    };
    Ok(Attribute {
        name: name.clone(),
        state: name,
        type_,
        native,
    })
}

/// Gives each state member a name no accessor has.
fn name_state(attributes: &mut [Attribute]) {
    // A state name differs from every accessor and from every state name chosen before it.
    let mut taken: Vec<String> = attributes.iter().map(|a| a.name.clone()).collect();
    for attribute in attributes {
        let mut state = format!("{}_", attribute.name);
        while taken.contains(&state) {
            state.push('_');
        }
        taken.push(state.clone());
        attribute.state = state;
    }
}

fn accessors(attribute: &Attribute) -> String {
    let Attribute {
        name, state, type_, ..
    } = attribute;
    if attribute.native {
        format!(
            "    {type_} {name}() const {{ return {state}; }}\n    {type_}& {name}() {{ return {state}; }}\n    void {name}({type_} value) {{ {state} = value; }}\n"
        )
    } else {
        format!(
            "    const {type_}& {name}() const {{ return {state}; }}\n    {type_}& {name}() {{ return {state}; }}\n    void {name}(const {type_}& value) {{ {state} = value; }}\n"
        )
    }
}

fn parameter(attribute: &Attribute) -> String {
    if attribute.native {
        format!("{} {}", attribute.type_, attribute.name)
    } else {
        format!("const {}& {}", attribute.type_, attribute.name)
    }
}

/// The class of a structure, and the specialisation of `zerodds::psm::Conv` for it.
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
    for member in ast.members_of(id) {
        let Ok(m) = ast.member(*member) else {
            return Err(MaterialiseError::NoNode {
                kind: "member",
                at: node.name.clone(),
            });
        };
        own.push(attribute(ast, m, &node.name)?);
    }
    name_state(&mut own);
    // The constructor takes the base's attributes first, then its own.
    let mut base_attributes = Vec::new();
    if let Some(b) = base {
        let base_node = ast.type_(b).map_err(|_| MaterialiseError::NoNode {
            kind: "member",
            at: node.name.clone(),
        })?;
        for member in ast.all_members_of(b) {
            let Ok(m) = ast.member(member) else {
                return Err(MaterialiseError::NoNode {
                    kind: "member",
                    at: base_node.name.clone(),
                });
            };
            base_attributes.push(attribute(ast, m, &base_node.name)?);
        }
    }

    let mut text = format!(
        "/// `{}` as DDS-PSM-Cxx 1.0 \u{a7}8.1.1 maps it: a class with accessors (\u{a7}7.4.6).\nclass {name}{} {{\npublic:\n    {name}() = default;\n",
        node.name,
        base.map_or(String::new(), |b| format!(" : public {}", psm_path(ast, b))),
    );
    let parameters: Vec<String> = base_attributes
        .iter()
        .chain(own.iter())
        .map(parameter)
        .collect();
    if !parameters.is_empty() {
        let explicit = if parameters.len() == 1 {
            "explicit "
        } else {
            ""
        };
        let mut initialisers = Vec::new();
        if let Some(b) = base {
            let args: Vec<&str> = base_attributes.iter().map(|a| a.name.as_str()).collect();
            initialisers.push(format!("{}({})", psm_path(ast, b), args.join(", ")));
        }
        for a in &own {
            initialisers.push(format!("{}({})", a.state, a.name));
        }
        let init = if initialisers.is_empty() {
            String::new()
        } else {
            format!("\n        : {}", initialisers.join(", "))
        };
        text.push_str(&format!(
            "    {explicit}{name}({}){init} {{}}\n",
            parameters.join(", ")
        ));
    }
    text.push('\n');
    for a in &own {
        text.push_str(&accessors(a));
    }
    // `swap` as a member and in the namespace, \u{a7}7.12.
    text.push_str(&format!("\n    void swap({name}& other) {{\n"));
    if let Some(b) = base {
        text.push_str(&format!("        {}::swap(other);\n", psm_path(ast, b)));
    }
    if !own.is_empty() {
        text.push_str("        using std::swap;\n");
    }
    for a in &own {
        text.push_str(&format!("        swap({0}, other.{0});\n", a.state));
    }
    text.push_str("    }\n\n");
    // Equality, member by member.
    text.push_str(&format!(
        "    bool operator==(const {name}& other) const {{\n        return "
    ));
    let mut terms: Vec<String> = Vec::new();
    if let Some(b) = base {
        terms.push(format!(
            "static_cast<const {0}&>(*this) == static_cast<const {0}&>(other)",
            psm_path(ast, b)
        ));
    }
    for a in &own {
        terms.push(format!("{0} == other.{0}", a.state));
    }
    if terms.is_empty() {
        text.push_str("true");
    } else {
        text.push_str(&terms.join("\n               && "));
    }
    text.push_str(&format!(
        ";\n    }}\n    bool operator!=(const {name}& other) const {{ return !(*this == other); }}\n\nprivate:\n"
    ));
    for a in &own {
        text.push_str(&format!("    {} {}{{}};\n", a.type_, a.state));
    }
    text.push_str("};\n");
    text.push_str(&format!(
        "\ninline void swap({name}& a, {name}& b) {{ a.swap(b); }}\n"
    ));

    // The specialisation of Conv: every member, the base's first, by name.
    let path = psm_path(ast, id);
    let native = native_path(ast, id);
    let mut all = Vec::new();
    for member in ast.all_members_of(id) {
        let Ok(m) = ast.member(member) else {
            return Err(MaterialiseError::NoNode {
                kind: "member",
                at: node.name.clone(),
            });
        };
        all.push(attribute(ast, m, &node.name)?);
    }
    let mut conv = format!(
        "template <>\nstruct Conv<{path}> {{\n    using Native = {native};\n    static Native to_native(const {path}& v) {{\n        Native n;\n"
    );
    for a in &all {
        conv.push_str(&format!(
            "        n.{0} = Shape<decltype(n.{0}), typename Conv<{1}>::Native>::apply(Conv<{1}>::to_native(v.{0}()));\n",
            a.name, a.type_
        ));
    }
    conv.push_str(&format!(
        "        return n;\n    }}\n    static {path} from_native(const Native& n) {{\n        {path} v;\n"
    ));
    for a in &all {
        conv.push_str(&format!(
            "        v.{0}(Conv<{1}>::from_native(Shape<typename Conv<{1}>::Native, decltype(n.{0})>::apply(n.{0})));\n",
            a.name, a.type_
        ));
    }
    conv.push_str("        return v;\n    }\n};\n");
    Ok((text, conv))
}

/// One case of a union, for the class: its attribute, its labels and whether it is the default
/// branch.
struct UnionCase {
    attribute: Attribute,
    labels: Vec<i128>,
    is_default: bool,
}

/// The values of the discriminator type `disc`, as numbers, when they can be listed: a
/// `boolean`, an enumeration; `None` for an integer or character type, which has more values
/// than any union lists.
fn listed_values(ast: &Ast, disc: Id<TypeNode>) -> Result<Option<Vec<i128>>, MaterialiseError> {
    let mut current = disc;
    for _ in 0..MAX_DEPTH {
        let Ok(node) = ast.type_(current) else {
            return Err(MaterialiseError::NoNode {
                kind: "discriminator type",
                at: format!("{disc:?}"),
            });
        };
        match &node.kind {
            TypeKind::Alias { aliased } => match aliased.resolved() {
                Some(next) => current = next,
                None => {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "typedef of the discriminator".to_string(),
                        at: node.name.clone(),
                    });
                }
            },
            TypeKind::Primitive(PrimitiveKind::Boolean) => return Ok(Some(vec![0, 1])),
            TypeKind::Enumeration { enumerators } => {
                let mut values = Vec::new();
                let mut next: i128 = 0;
                for e in enumerators {
                    let Ok(e) = ast.enumerator(*e) else {
                        return Err(MaterialiseError::NoNode {
                            kind: "enumerator",
                            at: node.name.clone(),
                        });
                    };
                    let value = match e.value {
                        Some(expr) => {
                            ast.eval_integral(expr)
                                .map_err(|_| MaterialiseError::Unevaluated {
                                    construct: "enumerator value".to_string(),
                                    at: node.name.clone(),
                                })?
                        }
                        None => next,
                    };
                    values.push(value);
                    next = value + 1;
                }
                return Ok(Some(values));
            }
            _ => return Ok(None),
        }
    }
    Ok(None)
}

/// A legal value of the discriminator that no case label names, for the default branch and for
/// `_default()` (IDL4 to C++ 1.0 §7.2.4.3.2); `None` when the labels cover every value.
fn unused_discriminator(
    ast: &Ast,
    disc: Id<TypeNode>,
    used: &[i128],
) -> Result<Option<i128>, MaterialiseError> {
    if let Some(values) = listed_values(ast, disc)? {
        return Ok(values.into_iter().find(|v| !used.contains(v)));
    }
    // An integer or character type: the smallest non-negative number nobody lists.
    let mut candidate: i128 = 0;
    while used.contains(&candidate) {
        candidate += 1;
    }
    Ok(Some(candidate))
}

/// The class of a union: IDL4 to C++ 1.0 §7.2.4.3.2 (DDS-PSM-Cxx 1.0 §7.4.4 maps unions as
/// IDL2C++11 §6.13.2 does). `_d()` and the accessors, a modifier per member that selects its
/// case, a modifier with a `discriminator` for a member that more than one label selects, and
/// `_default()` for an implicit default.
fn union_class(
    ast: &Ast,
    id: Id<TypeNode>,
    node: &TypeNode,
    discriminator: &Ref<TypeNode>,
) -> Result<(String, String), MaterialiseError> {
    let name = cxx_name(&node.name);
    let Some(disc) = discriminator.resolved() else {
        return Err(MaterialiseError::Unevaluated {
            construct: "union discriminator".to_string(),
            at: node.name.clone(),
        });
    };
    let disc_type = psm_type(ast, disc, &node.name)?;
    let mut cases: Vec<UnionCase> = Vec::new();
    for case in ast.cases_of(id) {
        let Ok(case_node) = ast.case(*case) else {
            return Err(MaterialiseError::NoNode {
                kind: "case",
                at: node.name.clone(),
            });
        };
        let Ok(member) = ast.member(case_node.member) else {
            return Err(MaterialiseError::NoNode {
                kind: "member",
                at: node.name.clone(),
            });
        };
        let mut labels = Vec::new();
        let mut is_default = false;
        for label in &case_node.labels {
            match label {
                CaseLabel::Value(expr) => labels.push(ast.eval_integral(*expr).map_err(|_| {
                    MaterialiseError::Unevaluated {
                        construct: "union case label".to_string(),
                        at: node.name.clone(),
                    }
                })?),
                CaseLabel::Default => is_default = true,
            }
        }
        cases.push(UnionCase {
            attribute: attribute(ast, member, &node.name)?,
            labels,
            is_default,
        });
    }
    let mut attributes: Vec<Attribute> = cases
        .iter()
        .map(|c| Attribute {
            name: c.attribute.name.clone(),
            state: c.attribute.state.clone(),
            type_: c.attribute.type_.clone(),
            native: c.attribute.native,
        })
        .collect();
    name_state(&mut attributes);

    let used: Vec<i128> = cases.iter().flat_map(|c| c.labels.clone()).collect();
    let unused = unused_discriminator(ast, disc, &used)?;
    let has_default_case = cases.iter().any(|c| c.is_default);
    // The default constructor: the default branch's discriminator when there is one, else the
    // first label of the first case (§7.2.4.3.2).
    let initial = if has_default_case {
        unused
    } else {
        cases.iter().find_map(|c| c.labels.first().copied())
    };
    let initial_text = initial.map_or(String::new(), |v| {
        format!("d_ = static_cast<{disc_type}>({v});")
    });

    let mut text = format!(
        "/// `{}`: IDL4 to C++ 1.0 \u{a7}7.2.4.3.2. A modifier selects the case of its member.\nclass {name} {{\npublic:\n    {name}() {{ {initial_text} }}\n    {name}(const {name}&) = default;\n    {name}({name}&&) = default;\n    ~{name}() = default;\n    {name}& operator=(const {name}&) = default;\n    {name}& operator=({name}&&) = default;\n\n    {disc_type} _d() const {{ return d_; }}\n    void _d({disc_type} value) {{ d_ = value; }}\n\n",
        node.name
    );
    for (attribute, case) in attributes.iter().zip(cases.iter()) {
        let (n, st, ty) = (&attribute.name, &attribute.state, &attribute.type_);
        // The value a modifier without a discriminator selects: the first label, or the
        // default branch's value.
        let select_value =
            case.labels
                .first()
                .copied()
                .or(if case.is_default { unused } else { None });
        let select = select_value.map_or(String::new(), |value| {
            format!(" d_ = static_cast<{disc_type}>({value});")
        });
        let multiple = case.labels.len() > 1;
        if attribute.native {
            text.push_str(&format!(
                "    {ty} {n}() const {{ return {st}; }}\n    {ty}& {n}() {{ return {st}; }}\n    void {n}({ty} value) {{ {st} = value;{select} }}\n"
            ));
            if multiple {
                text.push_str(&format!(
                    "    void {n}({ty} value, {disc_type} discriminator) {{ {st} = value; d_ = discriminator; }}\n"
                ));
            }
        } else {
            text.push_str(&format!(
                "    const {ty}& {n}() const {{ return {st}; }}\n    {ty}& {n}() {{ return {st}; }}\n    void {n}(const {ty}& value) {{ {st} = value;{select} }}\n    void {n}({ty}&& value) {{ {st} = std::move(value);{select} }}\n"
            ));
            if multiple {
                text.push_str(&format!(
                    "    void {n}(const {ty}& value, {disc_type} discriminator) {{ {st} = value; d_ = discriminator; }}\n    void {n}({ty}&& value, {disc_type} discriminator) {{ {st} = std::move(value); d_ = discriminator; }}\n"
                ));
            }
        }
    }
    // An implicit default member: no default case and labels that leave a value out.
    if !has_default_case {
        if let Some(value) = unused {
            text.push_str(&format!(
                "\n    void _default() {{ d_ = static_cast<{disc_type}>({value}); }}\n"
            ));
        }
    }
    text.push_str(&format!(
        "\n    void swap({name}& other) {{\n        using std::swap;\n        swap(d_, other.d_);\n"
    ));
    for attribute in &attributes {
        text.push_str(&format!("        swap({0}, other.{0});\n", attribute.state));
    }
    text.push_str("    }\n");
    text.push_str(&format!(
        "\n    bool operator==(const {name}& other) const {{\n        return d_ == other.d_"
    ));
    for attribute in &attributes {
        text.push_str(&format!(" && {0} == other.{0}", attribute.state));
    }
    text.push_str(&format!(
        ";\n    }}\n    bool operator!=(const {name}& other) const {{ return !(*this == other); }}\n\nprivate:\n    {disc_type} d_{{}};\n"
    ));
    for attribute in &attributes {
        text.push_str(&format!(
            "    {} {}{{}};\n",
            attribute.type_, attribute.state
        ));
    }
    text.push_str(&format!(
        "}};\n\ninline void swap({name}& a, {name}& b) {{ a.swap(b); }}\n"
    ));

    let path = psm_path(ast, id);
    let native = native_path(ast, id);
    // The cases first, whose setters select their own discriminator value, and the
    // discriminator last, which is the one the value had.
    let mut conv = format!(
        "template <>\nstruct Conv<{path}> {{\n    using Native = {native};\n    static Native to_native(const {path}& v) {{\n        Native n;\n"
    );
    for case in &cases {
        conv.push_str(&format!(
            "        n.{0}(Shape<std::remove_cv_t<std::remove_reference_t<decltype(n.{0}())>>, typename Conv<{1}>::Native>::apply(Conv<{1}>::to_native(v.{0}())));\n",
            case.attribute.name, case.attribute.type_
        ));
    }
    // Only the arm the discriminator selects is read: the native codec fills that one, and the
    // others of a native union are not initialised. A modifier selects its case, so the
    // discriminator is set last.
    conv.push_str(&format!(
        "        n._d(Conv<{disc_type}>::to_native(v._d()));\n        return n;\n    }}\n    static {path} from_native(const Native& n) {{\n        {path} v;\n        switch (static_cast<long long>(n._d())) {{\n"
    ));
    for case in &cases {
        let line = format!(
            "            v.{0}(Conv<{1}>::from_native(Shape<typename Conv<{1}>::Native, std::remove_cv_t<std::remove_reference_t<decltype(n.{0}())>>>::apply(n.{0}())));\n            break;\n",
            case.attribute.name, case.attribute.type_
        );
        if case.is_default {
            conv.push_str("            default:\n");
        }
        for label in &case.labels {
            conv.push_str(&format!("            case {label}LL:\n"));
        }
        if !case.labels.is_empty() || case.is_default {
            conv.push_str(&line);
        }
    }
    if !has_default_case {
        conv.push_str("            default:\n                break;\n");
    }
    conv.push_str(&format!(
        "        }}\n        v._d(Conv<{disc_type}>::from_native(n._d()));\n        return v;\n    }}\n}};\n"
    ));
    Ok((text, conv))
}

/// `namespace a { namespace b { … } }` around `text`.
pub(crate) fn in_namespaces(chain: &[String], text: &str) -> String {
    let mut out = String::new();
    for name in chain {
        out.push_str(&format!("namespace {name} {{\n"));
    }
    out.push_str(text);
    if !text.ends_with('\n') {
        out.push('\n');
    }
    for _ in chain {
        out.push_str("}\n");
    }
    out
}

/// The text of the PSM layer for `ast`: forward declarations, the types in dependency order,
/// and the `zerodds::psm::Conv` specialisations.
///
/// # Errors
/// A construct the PSM spelling has no form for, or a model that contradicts itself.
pub fn psm_layer(ast: &Ast) -> Result<String, MaterialiseError> {
    let order = ordered_types(ast);
    // A model with a pair of types takes the common types of DDS-RPC 1.0 \u{a7}7.5.1.1.1 from the
    // runtime (`dds::SampleIdentity`, `dds::rpc::RequestHeader`, ...): no PSM class, no
    // `swap` and no conversion for them, as the native declarations are dropped too.
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
            forward.push_str(&in_namespaces(
                &chain,
                &format!("class {};", cxx_name(&node.name)),
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
            TypeKind::Enumeration { .. }
            | TypeKind::Bitmask { .. }
            | TypeKind::Bitset { .. }
            | TypeKind::Native => (
                format!(
                    "/// Declared once, with the IDL's name and constants (DDS-PSM-Cxx 1.0 \u{a7}8.1.3).\nusing {name} = {};\n",
                    native_path(ast, id)
                ),
                None,
            ),
            TypeKind::Alias { aliased } => {
                let Some(target) = aliased.resolved() else {
                    return Err(no_form("an unresolved typedef", &node.name));
                };
                (
                    format!("using {name} = {};\n", psm_type(ast, target, &node.name)?),
                    None,
                )
            }
            TypeKind::ValueBox { boxed } => {
                let Some(target) = boxed.resolved() else {
                    return Err(no_form("an unresolved boxed value", &node.name));
                };
                (
                    format!(
                        "using {name} = std::shared_ptr<{}>;\n",
                        psm_type(ast, target, &node.name)?
                    ),
                    None,
                )
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
            // DDS-PSM-Cxx 1.0 \u{a7}8.1 has no mapping for these; saying so beats leaving the
            // name out of the PSM layer.
            TypeKind::Valuetype { .. } | TypeKind::ValueBase | TypeKind::ObjectRef { .. } => {
                return Err(no_form(
                    "a valuetype, interface or ValueBase has no DDS-PSM-Cxx 1.0 mapping",
                    &node.name,
                ));
            }
        };
        types.push_str(&in_namespaces(&chain, &text));
        types.push('\n');
        if let Some(conv) = conv {
            conversions.push_str(&conv);
            conversions.push('\n');
        }
    }
    // An exception is not remapped (DDS-PSM-Cxx 1.0 \u{a7}8.1 has none): the application throws and
    // catches the native class, under the IDL's own name.
    for (_, exception) in ast.exceptions() {
        let chain = module_names(ast, exception.module)?;
        let name = cxx_name(&exception.name);
        let mut native = format!("::{NATIVE_NAMESPACE}");
        for segment in &chain {
            native.push_str("::");
            native.push_str(segment);
        }
        native.push_str("::");
        native.push_str(&name);
        types.push_str(&in_namespaces(
            &chain,
            &format!("using {name} = {native};\n"),
        ));
        types.push('\n');
    }
    // A model that declares no type has nothing to layer: no text, so that no preamble is written
    // for it (the model, not the flag, decides whether the file names the runtime).
    if types.is_empty() {
        return Ok(String::new());
    }
    Ok(format!(
        "// ---- The PSM layer: DDS-PSM-Cxx 1.0 \u{a7}8.1 over the native types above. ----\n\
         // The native types are in `{NATIVE_NAMESPACE}`; these classes are what an application writes.\n\
         // `zerodds::psm::Conv` hands them to the codec.\n\n{forward}\n{types}\
         namespace zerodds {{\nnamespace psm {{\n\n{conversions}}} // namespace psm\n}} // namespace zerodds\n"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_that_is_an_accessor_is_not_a_state_member() {
        let mut attributes = vec![
            Attribute {
                name: "a".into(),
                state: String::new(),
                type_: "int32_t".into(),
                native: true,
            },
            Attribute {
                name: "a_".into(),
                state: String::new(),
                type_: "int32_t".into(),
                native: true,
            },
        ];
        name_state(&mut attributes);
        assert_eq!(attributes[0].state, "a__");
        assert_eq!(attributes[1].state, "a___");
    }

    #[test]
    fn the_primitives_are_the_table_7_1_names() {
        assert_eq!(primitive(PrimitiveKind::WChar), "wchar_t");
        assert_eq!(primitive(PrimitiveKind::Float128), "long double");
        assert_eq!(primitive(PrimitiveKind::Octet), "uint8_t");
    }
}
