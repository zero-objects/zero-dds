// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The names the Rust generator gives what IDL declares.
//!
//! Two naming schemes, as IDL4 to Java 1.0 and IDL4 to C# 1.0 §7.1.1 have
//! them for their languages: the IDL names as written ([`RustNaming::Idl`],
//! the default) and the names the Rust API guidelines ask for
//! ([`RustNaming::Rust`], RFC 430): `UpperCamelCase` for types, traits and
//! variants, `snake_case` for fields, modules, functions and parameters,
//! `SCREAMING_SNAKE_CASE` for constants. No IDL4 to Rust mapping is
//! adopted, so the scheme is ZeroDDS's own.
//!
//! The scheme of a build comes from [`RustNamingScope`]
//! (`zerodds_build::Config::rust_naming`, `zerodds-idlc --rust-naming`).
//! `@rust_mapping(apply_naming_convention = ...)` on a module or on a
//! type, constant, exception or interface replaces it for what the
//! annotated declaration encloses, the innermost annotation last. A
//! member, an enumerator, a bit, an operation and a parameter take the
//! scheme of the declaration they belong to.
//!
//! Only the Rust spelling changes. The TypeObject, member ids, the
//! enumerator names of type matching, the field names of a content filter
//! and the keys of `zerodds_build::Config::rust_type` are the IDL names
//! under either scheme.

use zerodds_ast::{AnyId, Ast, Facets, MappingLanguage, MappingValue, ModuleNode, TypeNode};
use zerodds_ast_core::Id;
use zerodds_lang_cst::rust;

/// The naming scheme of the generated Rust.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum RustNaming {
    /// The names as they are in IDL; a Rust keyword takes its raw form.
    #[default]
    Idl,
    /// The names after RFC 430.
    Rust,
}

impl RustNaming {
    /// The name the command line gives the scheme.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Idl => "idl",
            Self::Rust => "rust",
        }
    }

    /// The scheme a command-line name or an `apply_naming_convention`
    /// value selects.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "idl" | "IDL_NAMING_CONVENTION" => Some(Self::Idl),
            "rust" | "RUST_NAMING_CONVENTION" => Some(Self::Rust),
            _ => None,
        }
    }

    fn apply(self, name: &str, rust: fn(&str) -> String) -> String {
        match self {
            Self::Idl => name.to_string(),
            Self::Rust => rust(name),
        }
    }

    /// A module: `snake_case`.
    #[must_use]
    pub fn module(self, name: &str) -> String {
        rust::escape_type_name(&self.apply(name, snake_case))
    }

    /// A structure, union, enumeration, bit set, bit mask, typedef, value
    /// type, exception or interface: `UpperCamelCase`.
    #[must_use]
    pub fn type_name(self, name: &str) -> String {
        rust::escape_type_name(&self.apply(name, upper_camel_case))
    }

    /// A constant: `SCREAMING_SNAKE_CASE`.
    #[must_use]
    pub fn constant(self, name: &str) -> String {
        rust::escape_type_name(&self.apply(name, screaming_snake_case))
    }

    /// The constant of one bit of a bit mask: `SCREAMING_SNAKE_CASE`.
    #[must_use]
    pub fn bit(self, name: &str) -> String {
        rust::escape_keyword(&self.apply(name, screaming_snake_case))
    }

    /// A member of a structure, value type, exception or bit set, without
    /// the keyword escape: `snake_case`.
    #[must_use]
    pub fn field_unescaped(self, name: &str) -> String {
        self.apply(name, snake_case)
    }

    /// A member of a structure, value type, exception or bit set:
    /// `snake_case`.
    #[must_use]
    pub fn field(self, name: &str) -> String {
        rust::escape_keyword(&self.field_unescaped(name))
    }

    /// An operation: `snake_case`.
    #[must_use]
    pub fn function(self, name: &str) -> String {
        rust::escape_keyword(&self.apply(name, snake_case))
    }

    /// A parameter of an operation: `snake_case`.
    #[must_use]
    pub fn parameter(self, name: &str) -> String {
        rust::escape_keyword(&self.apply(name, snake_case))
    }

    /// An enumerator, or the variant an exception takes in an error enum:
    /// `UpperCamelCase`.
    #[must_use]
    pub fn variant(self, name: &str) -> String {
        rust::escape_keyword(&self.apply(name, upper_camel_case))
    }

    /// The variant of a union case, named after the case's member, or of
    /// an operation in a call union. Under the IDL scheme the member's
    /// name with its first letter raised, so the two stay recognisably
    /// one name; `UpperCamelCase` under the Rust scheme.
    #[must_use]
    pub fn case_variant(self, member: &str) -> String {
        match self {
            Self::Idl => {
                let mut chars = member.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                }
            }
            Self::Rust => upper_camel_case(member),
        }
    }

    /// A type name made of an IDL name and a suffix the generator adds
    /// (`Client`, `Dispatcher`, `Error`): the suffix follows the name
    /// directly under either scheme.
    #[must_use]
    pub fn type_with_suffix(self, name: &str, suffix: &str) -> String {
        let mut out = self.apply(name, upper_camel_case);
        out.push_str(suffix);
        out
    }

    /// A type name the generator composes with underscores, as DDS-RPC
    /// 1.0 §7.5.1.1.4-7 composes `Service_operation_In`: unchanged under
    /// the IDL scheme, `UpperCamelCase` under the Rust scheme.
    #[must_use]
    pub fn composed_type(self, composed: &str) -> String {
        self.apply(composed, upper_camel_case)
    }

    /// A field or function name the generator composes itself:
    /// unchanged under the IDL scheme, `snake_case` under the Rust scheme.
    #[must_use]
    pub fn composed_value(self, composed: &str) -> String {
        self.apply(composed, snake_case)
    }
}

/// The words of an identifier: split at each `_`, where a lower-case
/// letter is followed by an upper-case one (`myField`), and inside a run
/// of upper-case letters before the last one that a lower-case letter
/// follows (`HTTPServer` is `HTTP`, `Server`). A digit belongs to the word
/// it stands in.
fn words(name: &str) -> Vec<String> {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Case {
        None,
        Lower,
        Upper,
    }
    let mut out: Vec<String> = Vec::new();
    for piece in name.split('_').filter(|piece| !piece.is_empty()) {
        let chars: Vec<char> = piece.chars().collect();
        let mut word = String::new();
        let mut last = Case::None;
        for (at, ch) in chars.iter().copied().enumerate() {
            let next_is_lower = chars.get(at + 1).is_some_and(|next| next.is_lowercase());
            let boundary = ch.is_uppercase()
                && !word.is_empty()
                && (last == Case::Lower || (last == Case::Upper && next_is_lower));
            if boundary {
                out.push(core::mem::take(&mut word));
            }
            word.push(ch);
            if ch.is_uppercase() {
                last = Case::Upper;
            } else if ch.is_lowercase() {
                last = Case::Lower;
            }
        }
        if !word.is_empty() {
            out.push(word);
        }
    }
    out
}

/// `UpperCamelCase`: each word with its first letter raised and the rest
/// lowered, so an acronym is one word (`HTTP_SERVER` and `HTTPServer` are
/// `HttpServer`). A name without a letter or digit is kept.
#[must_use]
pub fn upper_camel_case(name: &str) -> String {
    let words = words(name);
    if words.is_empty() {
        return name.to_string();
    }
    let mut out = String::with_capacity(name.len());
    for word in words {
        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            out.extend(first.to_uppercase());
            out.extend(chars.flat_map(char::to_lowercase));
        }
    }
    out
}

/// `snake_case`: the words lowered and joined by `_`. A name without a
/// letter or digit is kept.
#[must_use]
pub fn snake_case(name: &str) -> String {
    joined(name, char::to_lowercase)
}

/// `SCREAMING_SNAKE_CASE`: the words raised and joined by `_`. A name
/// without a letter or digit is kept.
#[must_use]
pub fn screaming_snake_case(name: &str) -> String {
    joined(name, char::to_uppercase)
}

fn joined<I: Iterator<Item = char>>(name: &str, case: fn(char) -> I) -> String {
    let words = words(name);
    if words.is_empty() {
        return name.to_string();
    }
    let mut out = String::with_capacity(name.len() + words.len());
    for (at, word) in words.iter().enumerate() {
        if at > 0 {
            out.push('_');
        }
        out.extend(word.chars().flat_map(case));
    }
    out
}

thread_local! {
    static NAMING: core::cell::Cell<RustNaming> = const { core::cell::Cell::new(RustNaming::Idl) };
    static MEMBERS: core::cell::RefCell<Vec<MemberFrame>> =
        const { core::cell::RefCell::new(Vec::new()) };
}

/// The naming scheme of a build on the current thread until dropped.
///
/// A scope and not a parameter for the reason
/// [`crate::adapter::cpp::CppMappingScope`] gives.
#[derive(Debug)]
pub struct RustNamingScope {
    previous: RustNaming,
}

impl RustNamingScope {
    /// Names by `naming` on this thread, where no `@rust_mapping` says
    /// otherwise.
    #[must_use]
    pub fn enter(naming: RustNaming) -> Self {
        let previous = NAMING.with(|cell| cell.replace(naming));
        Self { previous }
    }
}

impl Drop for RustNamingScope {
    fn drop(&mut self) {
        NAMING.with(|cell| cell.set(self.previous));
    }
}

/// The scheme of the build on this thread ([`RustNamingScope`]).
#[must_use]
pub fn build_naming() -> RustNaming {
    NAMING.with(core::cell::Cell::get)
}

/// `naming` with the `@rust_mapping` settings of one node on top.
#[must_use]
pub fn with_facets(mut naming: RustNaming, facets: &Facets) -> RustNaming {
    for setting in &facets.mappings {
        if setting.language != MappingLanguage::Rust {
            continue;
        }
        // The front end admits no other parameter (idl-lower,
        // `mapping_annotation`).
        if let ("apply_naming_convention", MappingValue::Name(name)) =
            (setting.parameter.as_str(), &setting.value)
            && let Some(chosen) = RustNaming::from_name(name)
        {
            naming = chosen;
        }
    }
    naming
}

/// The modules enclosing `module`, innermost first, the root included.
/// Bounded like every walk up the module chain.
fn enclosing(ast: &Ast, module: Id<ModuleNode>) -> Vec<&ModuleNode> {
    let mut chain = Vec::new();
    let mut current = Some(module);
    for _ in 0..=crate::adapter::cst::MAX_MODULE_DEPTH {
        let Some(node) = current.and_then(|id| ast.module(id).ok()) else {
            break;
        };
        chain.push(node);
        current = node.parent;
    }
    chain
}

/// The scheme in force inside `module`: the build's, then every
/// `@rust_mapping` from the outermost module to `module`.
#[must_use]
pub fn in_module(ast: &Ast, module: Id<ModuleNode>) -> RustNaming {
    enclosing(ast, module)
        .into_iter()
        .rev()
        .fold(build_naming(), |naming, node| {
            with_facets(naming, &node.facets)
        })
}

/// The scheme of a declaration in `module` that carries `facets`.
#[must_use]
pub fn of_declaration(ast: &Ast, module: Id<ModuleNode>, facets: &Facets) -> RustNaming {
    with_facets(in_module(ast, module), facets)
}

/// The scheme of a type.
#[must_use]
pub fn of_type(ast: &Ast, node: &TypeNode) -> RustNaming {
    of_declaration(ast, node.module, &node.facets)
}

/// The scheme of a type by its handle; the build's with a stale handle.
#[must_use]
pub fn of_type_id(ast: &Ast, type_: Id<TypeNode>) -> RustNaming {
    ast.type_(type_)
        .map_or_else(|_| build_naming(), |node| of_type(ast, node))
}

/// The scheme of the declaration a member, an operation or an attribute
/// belongs to: a type, an exception or an interface.
#[must_use]
pub fn of_owner(ast: &Ast, owner: AnyId) -> RustNaming {
    match owner {
        AnyId::Type(id, _) => of_type_id(ast, id),
        AnyId::Exception(id, _) => ast.exception(id).map_or_else(
            |_| build_naming(),
            |node| of_declaration(ast, node.module, &node.facets),
        ),
        AnyId::Interface(id, _) => ast.interface(id).map_or_else(
            |_| build_naming(),
            |node| of_declaration(ast, node.module, &node.facets),
        ),
        AnyId::Module(id, _) => in_module(ast, id),
        _ => build_naming(),
    }
}

/// The Rust name of the module `module`, by the scheme in force in it.
#[must_use]
pub fn module_ident(ast: &Ast, module: Id<ModuleNode>) -> String {
    let name = ast.module(module).map_or("", |node| node.name.as_str());
    in_module(ast, module).module(name)
}

/// The Rust name of a declared type.
#[must_use]
pub fn type_ident(ast: &Ast, node: &TypeNode) -> String {
    of_type(ast, node).type_name(&node.name)
}

/// The schemes of the members one generated codec names.
#[derive(Debug)]
struct MemberFrame {
    /// The scheme of the type the codec belongs to.
    naming: RustNaming,
    /// The members a base under another scheme declares, by IDL name.
    others: Vec<(String, RustNaming)>,
}

/// The schemes of a type's members on the current thread until dropped,
/// for the codec functions that carry a member as its IDL name
/// ([`field_ident`]).
///
/// A member a structure inherits is a field of the base, and the base may
/// stand under another scheme than the structure the codec is written
/// for.
#[derive(Debug)]
pub struct MemberNamingScope {
    _not_send: core::marker::PhantomData<*const ()>,
}

impl MemberNamingScope {
    /// The members of `type_` and of its bases, each under the scheme of
    /// the type that declares it.
    #[must_use]
    pub fn of_type(ast: &Ast, type_: Id<TypeNode>) -> Self {
        let naming = of_type_id(ast, type_);
        let mut others = Vec::new();
        for member in ast.all_members_of(type_) {
            let Ok(node) = ast.member(member) else {
                continue;
            };
            let declared = of_owner(ast, node.parent);
            if declared != naming {
                others.push((node.name.clone(), declared));
            }
        }
        Self::push(MemberFrame { naming, others })
    }

    /// Every member under `naming`: the fields of a structure the
    /// generator synthesizes, which has no declaration in the model.
    #[must_use]
    pub fn all(naming: RustNaming) -> Self {
        Self::push(MemberFrame {
            naming,
            others: Vec::new(),
        })
    }

    fn push(frame: MemberFrame) -> Self {
        MEMBERS.with(|stack| stack.borrow_mut().push(frame));
        Self {
            _not_send: core::marker::PhantomData,
        }
    }
}

impl Drop for MemberNamingScope {
    fn drop(&mut self) {
        MEMBERS.with(|stack| {
            stack.borrow_mut().pop();
        });
    }
}

/// The scheme of the member `name` in the innermost [`MemberNamingScope`],
/// the build's outside one.
#[must_use]
pub fn member_naming(name: &str) -> RustNaming {
    MEMBERS.with(|stack| {
        stack.borrow().last().map_or_else(build_naming, |frame| {
            frame
                .others
                .iter()
                .find(|(other, _)| other == name)
                .map_or(frame.naming, |(_, naming)| *naming)
        })
    })
}

/// The Rust field of the member `name` in the innermost
/// [`MemberNamingScope`].
#[must_use]
pub fn field_ident(name: &str) -> String {
    member_naming(name).field(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_three_cases_split_words_alike() {
        for (idl, camel, snake, screaming) in [
            ("my_field", "MyField", "my_field", "MY_FIELD"),
            ("myField", "MyField", "my_field", "MY_FIELD"),
            ("MyField", "MyField", "my_field", "MY_FIELD"),
            ("MY_FIELD", "MyField", "my_field", "MY_FIELD"),
            ("HTTPServer", "HttpServer", "http_server", "HTTP_SERVER"),
            ("ShapeType", "ShapeType", "shape_type", "SHAPE_TYPE"),
            ("shapesize", "Shapesize", "shapesize", "SHAPESIZE"),
            ("x", "X", "x", "X"),
            ("Vector3D", "Vector3D", "vector3_d", "VECTOR3_D"),
            ("point_2d", "Point2d", "point_2d", "POINT_2D"),
            ("_leading", "Leading", "leading", "LEADING"),
            ("trailing_", "Trailing", "trailing", "TRAILING"),
            ("double__gap", "DoubleGap", "double_gap", "DOUBLE_GAP"),
            ("ABC", "Abc", "abc", "ABC"),
            ("aBC", "ABc", "a_bc", "A_BC"),
        ] {
            assert_eq!(upper_camel_case(idl), camel, "{idl}");
            assert_eq!(snake_case(idl), snake, "{idl}");
            assert_eq!(screaming_snake_case(idl), screaming, "{idl}");
        }
    }

    #[test]
    fn the_idl_scheme_keeps_the_name_and_both_escape() {
        let idl = RustNaming::Idl;
        let rust = RustNaming::Rust;
        assert_eq!(idl.field("myField"), "myField");
        assert_eq!(rust.field("myField"), "my_field");
        assert_eq!(idl.field("type"), "r#type");
        assert_eq!(rust.field("Type"), "r#type");
        assert_eq!(rust.field("Self"), "self_");
        assert_eq!(idl.type_name("shape_type"), "shape_type");
        assert_eq!(rust.type_name("shape_type"), "ShapeType");
        assert_eq!(rust.type_name("option"), "Option_");
        assert_eq!(rust.variant("self"), "Self_");
        assert_eq!(idl.constant("maxSize"), "maxSize");
        assert_eq!(rust.constant("maxSize"), "MAX_SIZE");
        assert_eq!(idl.module("MyModule"), "MyModule");
        assert_eq!(rust.module("MyModule"), "my_module");
        assert_eq!(idl.case_variant("my_branch"), "My_branch");
        assert_eq!(rust.case_variant("my_branch"), "MyBranch");
        assert_eq!(idl.composed_type("Calc_add_In"), "Calc_add_In");
        assert_eq!(rust.composed_type("Calc_add_In"), "CalcAddIn");
        assert_eq!(
            rust.type_with_suffix("robot_arm", "Client"),
            "RobotArmClient"
        );
        assert_eq!(
            idl.type_with_suffix("robot_arm", "Client"),
            "robot_armClient"
        );
    }

    #[test]
    fn the_scopes_nest_and_end() {
        assert_eq!(build_naming(), RustNaming::Idl);
        {
            let _build = RustNamingScope::enter(RustNaming::Rust);
            assert_eq!(build_naming(), RustNaming::Rust);
            assert_eq!(field_ident("myField"), "my_field");
            {
                let _members = MemberNamingScope::all(RustNaming::Idl);
                assert_eq!(field_ident("myField"), "myField");
            }
            assert_eq!(field_ident("myField"), "my_field");
        }
        assert_eq!(build_naming(), RustNaming::Idl);
    }
}
