// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The C++ CST as the other end of a rule. Spec §3.2.
//!
//! The counterpart of [`crate::adapter::c`], function for function.
//! Reading the two side by side is the point: where they differ, the
//! difference is C++'s, and it is written down here rather than spread
//! through a renderer.
//!
//! Values are resolved from the model, never from the overlay — the same
//! rule the Rust and the C materialiser follow and for the same reason
//! (spec §2.3).
//!
//! ## The four things C++ gives back that C does not
//!
//! 1. **Generics, so most structural typedefs disappear.** C has to
//!    declare a named `zd_seq_int32_t` because it cannot write an
//!    anonymous aggregate twice and have the two be one type;
//!    `std::vector<std::int32_t>` is that type wherever it is written.
//!    The whole of the C column's structural-name machinery — the
//!    `zd_seq_`, `zd_arr_`, `zd_map_` declarations and the collision
//!    hazard that came with them — is gone from the output. It is
//!    **not** gone from the naming: see [`cpp_type_tag`].
//! 2. **Namespaces, so a module is a scope and not a prefix.** C
//!    flattens `a::b::Foo` to `a_b_Foo`; here the IDL scoping survives as
//!    `namespace a { namespace b { struct Foo … } }`, and every
//!    reference to a declared type is written from the root — `::a::b::Foo`
//!    — so a member never depends on which namespace it is read in.
//! 3. **An enumeration wide enough.** A C enumeration constant is an
//!    `int` (C11 §6.7.2.2), which is why the C column emits a
//!    fixed-width typedef and a run of `#define`s. C++11's scoped
//!    enumeration takes an explicit underlying type, so `@bit_bound(32)`
//!    — which admits enumerator values up to `2^32 - 1` — can be held by
//!    the type itself. This is the one place C++ is simpler.
//! 4. **A plain pointer for an absent member.** The C column writes a
//!    `T *` and a NULL for an `@optional` member, and so does this one:
//!    DDS-XTypes 1.3 §7.5.1.2.4.2 asks for "plain pointers rather than
//!    automatic values or smart pointers", NULL for an omitted value. The
//!    class owns what the pointer points at — the destructor deletes it,
//!    a copy is deep (§7.5.1.2.3.2, "the same as specified for C") — and
//!    the member functions that say so are generated beside the members
//!    (`owning_members`).
//!
//! ## The memory shapes
//!
//! | IDL | C++ |
//! |---|---|
//! | `string` | `std::string` |
//! | `wstring` | `std::string`, holding UTF-8 |
//! | `fixed<P,S>` | `std::string`, holding the decimal |
//! | `long double` | `zerodds::wire::Float128` |
//! | `sequence<T>` | `std::vector<T>` |
//! | `T a[2][3]` | `std::array<std::array<T, 3>, 2>` |
//! | `map<K,V>` | `zerodds::Map<K, V>`, a `std::vector<std::pair<K, V>>` with the operations of Table 47 |
//! | `union` | a class with `_d()` and one accessor per case (IDL4 to C++ 1.0) over a discriminator and one storage per case |
//! | `@optional T m` | `T *m`, NULL when absent |
//!
//! A `wstring` holds UTF-8 rather than `char16_t` units for the reason
//! the C column gives: it puts the §7.4.1.1.2 conversion — the surrogate
//! rule the byte count depends on — inside `runtime/cpp`, where it is a
//! second implementation of the clause and not a copy of the Rust one's
//! answer.
//!
//! A `map<K,V>` is `zerodds::Map` — XTypes 1.3
//! §7.5.1.3.3, a class with the operations of Table 47 — over a **vector of
//! pairs** and not a `std::map`. The order a map's entries go on the wire
//! is a policy axis — `zerodds_ast_layout::MapEntryOrder`, declaration
//! order against ascending key — and `std::map` would impose its own
//! ordering on every dialect before the encoder ever saw the value. A
//! container that cannot hold declaration order cannot reproduce the four
//! implementations that write it. The Modern C++ binding of §7.5.1.3.4,
//! `std::map`, is [`CppMapping::Psm`].
//!
//! ## Where the IDL spelling of a member is kept
//!
//! [`cxx_name`] moves a colliding name (§7.11.1.1.2 rule 8) — `long static;`
//! becomes `std::int32_t cxx_static;` — and C++ has more keywords to
//! collide with than C, so it moves more names. XTypes 1.3 §7.3.1.2.1.1
//! computes a member's ID under `@autoid(HASH)` and under `@hashid`
//! from the first four octets of the MD5 of its **IDL** name, so a
//! header that only knows `cxx_static` cannot compute the ID for `static`
//! and the member it writes lands under an ID the peer does not have.
//!
//! Every declaration with named parts carries them beside itself, in
//! IDL declaration order — the order the member IDs are assigned in:
//!
//! ```cpp
//! inline constexpr std::array<std::pair<const char*, const char*>, 2>
//!     Sample_idl_names = {{{"impl", "impl"}, {"cxx_static", "static"}}};
//! ```
//!
//! An **`inline constexpr` variable and not the C column's macro**, and
//! for the reason the constants above are one too: C reaches for the
//! preprocessor because an object in a header is a separate object in
//! every translation unit that includes it, and C++17's inline variable
//! is one object across the program. So the mapping can be a typed name
//! — iterable with `.size()`, usable in a `constexpr` context, and
//! reachable as `::a::b::Sample_idl_names` from anywhere — where the C
//! column has to hand a caller a brace-enclosed initialiser and let it
//! declare the array itself. Measured unused, under
//! `-std=c++20 -Wall -Wextra -Werror`, across all 376 headers the
//! corpus renders: no diagnostic.
//!
//! `std::pair` and not a generated record type: a record would be a
//! declaration whose name could collide with an IDL type, in a header
//! that has no other reason to declare one, and `<utility>` is already
//! in [`includes`].
//!
//! A union is a **class**, not a `union` and not a `std::variant`. Two
//! cases may carry the same type — the corpus has such types — and a
//! `std::variant<T, T>` cannot say which of the two is live without an
//! index the discriminator already is. The class has the accessor
//! functions of IDL4 to C++ 1.0 §7.5.1.4 (`_d()`, and per case a getter, a
//! reference getter and two setters, a setter selecting its case) over
//! one storage member per case and one for the discriminator, which is
//! the C column's `_d` plus `_u` with the anonymous union opened out; the
//! generated codec reads and writes the storage (`zd_d`, `zd_<case>`). A
//! data member is never warned about for being unused, so this compiles
//! under `-Wall -Wextra -Werror`.

use crate::adapter::c::ordered_types;
use crate::adapter::constant;
use crate::adapter::cst::{MaterialiseError, no_spelling_collision};
use crate::apply::Applied;
use crate::verbatim;
use seesaw_tgg::ident::GhostId;
use zerodds_ast::{Ast, CaseLabel, ModuleNode, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::{Id, NodeId};
use zerodds_lang_cst::c::Include;
use zerodds_lang_cst::common::{Doc, Ident, Literal, Path};
use zerodds_lang_cst::cpp;

/// The C++ spelling of `long double`.
///
/// `zerodds::wire::Float128` and **never** the compiler's `long double`:
/// that type is x87 80-bit extended on x86-64, binary128 on aarch64 and
/// a plain `double` on macOS arm64, so a generated class declaring it
/// would have a different wire format on each. `runtime/cpp/include/
/// zerodds/wire.hpp` carries the sixteen-byte type and the conversions
/// that report the values which do not fit; `crate::adapter::c::FLOAT128`
/// is the same decision in the C column.
pub const FLOAT128: &str = "zerodds::wire::Float128";

/// The headers a materialised translation unit needs.
///
/// The C column's counterpart is [`crate::adapter::c::prelude`], which
/// declares support *types* because C has no other way to name a string
/// or a sequence. C++ needs no support types — the shapes are `std::`
/// templates — but it does need the headers those templates come from,
/// and a caller that assembled the `#include` list by hand would drift
/// from what the materialiser emits. `<cstddef>` is not among them:
/// nothing below writes a `size_t`.
#[must_use]
pub fn includes() -> Vec<Include> {
    vec![
        Include::system("array"),
        Include::system("cstdint"),
        Include::system("memory"),
        Include::system("optional"),
        Include::system("string"),
        Include::system("utility"),
        Include::system("vector"),
        Include::local("zerodds/map.hpp"),
        Include::local("zerodds/wire.hpp"),
    ]
}

/// Build the C++ declarations a rule application produced.
///
/// The walk is [`ordered_types`] — a post-order over the type graph, the
/// same one the C column takes. The declaration-order problem is not C's
/// alone: a C++ class whose member names a class not yet defined does not
/// compile either, and `std::optional<T>` and `std::array<T, N>` both
/// require `T` complete.
///
/// # Errors
/// [`MaterialiseError`] when a value the output has to state does not
/// evaluate, or a type has no C++ shape — never a silently dropped
/// declaration.
pub fn cpp_items(ast: &Ast, applied: &Applied) -> Result<Vec<cpp::Decl>, MaterialiseError> {
    let mut out: Vec<(Id<ModuleNode>, cpp::Decl)> = Vec::new();
    let mut emitted: Vec<String> = Vec::new();

    // `@verbatim(placement = BEGIN_FILE)`, wherever it was written,
    // opens the file — OMG IDL 4.2 §8.3.5.1 — so it stands before even
    // the forward declarations. Paired with the root module, so
    // `nest_by_module` leaves it at file scope.
    let (begin_file, end_file) = verbatim::file_slots(ast, verbatim::aliases::CPP);
    for text in begin_file {
        out.push((ast.root_module(), cpp::Decl::Verbatim(text)));
    }

    // A forward declaration for every aggregate, ahead of every
    // definition. A post-order over the type graph has no valid order
    // for two types that name each other — `recursive-mutual` is
    // `struct A { sequence<B> bs; }` beside `struct B { sequence<A> as; }`
    // — and one of the two definitions always names a class the compiler
    // has not seen. `std::vector<T>` admits an incomplete `T` since
    // C++17, which is what makes the declaration enough.
    for id in ordered_types(ast) {
        let Ok(node) = ast.type_(id) else { continue };
        if !matches!(
            node.kind,
            TypeKind::Structure { .. } | TypeKind::Union { .. } | TypeKind::Valuetype { .. }
        ) {
            continue;
        }
        out.push((
            node.module,
            cpp::Decl::Forward {
                is_struct: true,
                name: Ident::new(&cxx_name(&node.name)),
            },
        ));
    }

    no_bits_name_collision(ast)?;

    for id in ordered_types(ast) {
        let start = out.len();
        declare(ast, applied, id, &mut out, &mut emitted)?;
        weave_verbatim(ast, id, start, &mut out);
        maps_alongside(ast, id, &mut out, &emitted);
    }

    // Constants and exceptions are not types, so they are walked
    // separately — the model keeps them in arenas of their own, and both
    // the Rust and the C materialiser do the same. Interfaces are not
    // walked at all, exactly as `c_items` does not walk them: an
    // interface has no data form, its operations belong to the RPC arc,
    // and `crate::coverage` is where that is written down.
    for (id, node) in ast.constants() {
        if ghost_created_for(applied, node.node_id, "CppConst").is_none() {
            continue;
        }
        let _ = id;
        // `eval_const` and not `eval_integral`: `const string MODULE_ID`
        // evaluates perfectly well and is not a number, and asking the
        // integral question about it answered `Unevaluated` and dropped
        // the declaration.
        let value = ast
            .eval_const(node.expr)
            .map_err(|_| MaterialiseError::Unevaluated {
                construct: "constant's value".to_string(),
                at: node.name.clone(),
            })?;
        // `inline constexpr` and not the C column's `#define`. The
        // reason C reached for the preprocessor is C-specific — a
        // `static const` in a header is a separate object in every
        // translation unit that includes it, and every one that does not
        // use it warns — and C++17's inline variable is one object
        // across the program with no such warning, so the constant can
        // be a typed name instead of a token substitution.
        let declared = match node.type_.resolved() {
            Some(target) => cpp_value_type(ast, target, &node.name)?,
            None => {
                return Err(MaterialiseError::Unevaluated {
                    construct: "constant's type".to_string(),
                    at: node.name.clone(),
                });
            }
        };
        let (type_, literal) = cpp_constant(&value, declared, &node.name)?;
        out.push((
            node.module,
            cpp::Decl::Variable(cpp::Variable {
                doc: Doc::none(),
                type_,
                name: Ident::new(&cxx_name(&node.name)),
                value: Some(cpp::Expr::Literal(literal)),
                is_static: false,
                is_constexpr: true,
                is_inline: true,
            }),
        ));
    }

    for (id, node) in ast.exceptions() {
        if ghost_created_for(applied, node.node_id, "CppError").is_none() {
            continue;
        }
        let _ = id;
        let mut fields = Vec::new();
        let mut names = Vec::new();
        for member in &node.members {
            let Ok(member_node) = ast.member(*member) else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "exception member the model listed".to_string(),
                    at: node.name.clone(),
                });
            };
            let Some(target) = member_node.type_.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "exception member type".to_string(),
                    at: member_node.name.clone(),
                });
            };
            names.push((cxx_name(&member_node.name), member_node.name.clone()));
            fields.push(field(
                cpp_value_type(ast, target, &member_node.name)?,
                &member_node.name,
                Doc::none(),
            ));
        }
        let exception_name = cxx_name(&node.name);
        out.push((
            node.module,
            cpp::Decl::Class(class(
                &exception_name,
                fields,
                Doc::line("An exception, as the class its members make."),
            )),
        ));
        out.extend(
            idl_names(&exception_name, &names)?
                .into_iter()
                .map(|decl| (node.module, decl)),
        );
        out.push((node.module, idl_name(&exception_name, &node.name)));
    }

    // `@verbatim(placement = END_FILE)` closes the file, after even the
    // constants and exceptions. OMG IDL 4.2 §8.3.5.1.
    for text in end_file {
        out.push((ast.root_module(), cpp::Decl::Verbatim(text)));
    }

    nest_by_module(ast, out)
}

/// Weave one type's `@verbatim` text around and into what `declare`
/// just pushed. OMG IDL 4.2 §8.3.5.1; the slot semantics are
/// [`crate::verbatim`]'s.
///
/// `start` is where the type's declarations begin in `out`.
/// `BEGIN_DECLARATION` and `END_DECLARATION` land inside the class body
/// when the type materialised as one; an enum, an alias or a type that
/// declared nothing has no body a C++ tree can hold raw text in, so the
/// text lands adjacent to the declaration instead — the nearest
/// position the tree can express, never a dropped directive.
fn weave_verbatim(
    ast: &Ast,
    id: Id<TypeNode>,
    start: usize,
    out: &mut Vec<(Id<ModuleNode>, cpp::Decl)>,
) {
    let slots = verbatim::declaration_slots(ast, id, verbatim::aliases::CPP);
    if slots.before.is_empty()
        && slots.begin.is_empty()
        && slots.end.is_empty()
        && slots.after.is_empty()
    {
        return;
    }
    let Ok(node) = ast.type_(id) else {
        return;
    };
    let name = cxx_name(&node.name);
    let mut declared = out.split_off(start);
    let mut begin = slots.begin;
    let mut end = slots.end;
    let body = declared.iter_mut().find_map(|(_, decl)| match decl {
        cpp::Decl::Class(class) if class.name.as_str() == name => Some(class),
        _ => None,
    });
    if let Some(class) = body {
        if class.sections.is_empty() {
            class.sections.push(cpp::Section {
                access: cpp::Access::Public,
                members: Vec::new(),
            });
        }
        if let Some(first) = class.sections.first_mut() {
            for text in begin.drain(..).rev() {
                first.members.insert(0, cpp::ClassMember::Verbatim(text));
            }
        }
        if let Some(last) = class.sections.last_mut() {
            for text in end.drain(..) {
                last.members.push(cpp::ClassMember::Verbatim(text));
            }
        }
    }
    for text in slots.before.into_iter().chain(begin) {
        out.push((node.module, cpp::Decl::Verbatim(text)));
    }
    out.append(&mut declared);
    for text in end.into_iter().chain(slots.after) {
        out.push((node.module, cpp::Decl::Verbatim(text)));
    }
}

/// The declarations, each inside the `namespace` its IDL module names.
///
/// Public for the reason the Rust column's `nest_by_module` is: the
/// codecs are a second call, once per type, and they have to land in the
/// same namespace as the declaration they refer to.
///
/// Unlike the Rust column's, this one folds **consecutive runs** rather
/// than gathering every item of a module at the position of its first.
/// The order handed in is a topological one, and gathering would move a
/// declaration back in front of a type it names. A C++ namespace may be
/// reopened, so a run-wise fold costs nothing but a repeated
/// `namespace a {` line and keeps the order the walk computed.
///
/// # Errors
/// [`MaterialiseError::NoModule`] when the model does not answer for a
/// module an item names, or the parent chain runs past
/// `MAX_MODULE_DEPTH`. Returning the item unwrapped instead would put
/// it in the enclosing scope and say nothing — `module a { struct P; };`
/// and `module b { struct P; };` are two types.
pub fn nest_by_module(
    ast: &Ast,
    items: Vec<(Id<ModuleNode>, cpp::Decl)>,
) -> Result<Vec<cpp::Decl>, MaterialiseError> {
    let mut chained: Vec<(Vec<String>, cpp::Decl)> = Vec::with_capacity(items.len());
    for (module, decl) in items {
        // `@verbatim` at file scope is the user's text and stays there, whatever namespace
        // the declarations around it are in.
        let chain = if matches!(decl, cpp::Decl::Verbatim(_)) && module == ast.root_module() {
            Vec::new()
        } else {
            module_chain(ast, module)?
        };
        chained.push((chain, decl));
    }
    Ok(nest(&chained, 0))
}

/// As [`nest_by_module`], for declarations of the two kinds
/// [`CppMapping::Psm`] tells apart: `native` ones stand in
/// [`NATIVE_NAMESPACE`], the others in the namespaces of the IDL.
///
/// # Errors
/// As [`nest_by_module`].
pub fn nest_by_module_placed(
    ast: &Ast,
    items: Vec<(Id<ModuleNode>, bool, cpp::Decl)>,
) -> Result<Vec<cpp::Decl>, MaterialiseError> {
    let mut chained: Vec<(Vec<String>, cpp::Decl)> = Vec::with_capacity(items.len());
    for (module, native, decl) in items {
        let chain = if matches!(decl, cpp::Decl::Verbatim(_)) && module == ast.root_module() {
            Vec::new()
        } else if native {
            module_chain(ast, module)?
        } else {
            without_native_prefix(|| module_chain(ast, module))?
        };
        chained.push((chain, decl));
    }
    Ok(nest(&chained, 0))
}

/// How the C++ column spells topic types.
///
/// [`CppMapping::Psm`] is the default: the mapping DDS-PSM-Cxx 1.0 §7.4 and §8.1 ask for —
/// classes with accessors, `dds::core::optional`, `dds::core::array`, `wchar_t`, `std::wstring`,
/// `long double`, `std::map`. It is a layer over the native types and not a second codec: the
/// native types move into the namespace [`NATIVE_NAMESPACE`], the PSM types take the names of the
/// IDL, and `zerodds::psm::Conv` converts between them on the way into the codec and out of it.
///
/// [`CppMapping::Native`] is the named option that the wire codec, the endpoint gates and every
/// other consumer of the shape of a generated header ask for explicitly: structures with public
/// fields, a plain pointer for `@optional`, `std::array`, a `map` as a vector of pairs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CppMapping {
    /// Public fields, the types the codec reads and writes (`--cpp-mapping native`).
    Native,
    /// DDS-PSM-Cxx 1.0 §7.4 and §8.1, over the native types (the default).
    #[default]
    Psm,
    /// The classic OMG IDL to C++ 1.3 mapping, in C++03, over the native types
    /// (see [`super::cpp_classic`]).
    Classic,
}

/// The namespace the native types stand in under [`CppMapping::Psm`].
pub const NATIVE_NAMESPACE: &str = "zd_native";

thread_local! {
    static MAPPING: std::cell::Cell<CppMapping> = const { std::cell::Cell::new(CppMapping::Psm) };
    static PREFIX_SUSPENDED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Selects the C++ mapping on the current thread until dropped.
///
/// A scope and not a parameter because the mapping decides names in
/// every function that spells a type path, and threading an argument
/// through all of them would change signatures other columns share.
#[derive(Debug)]
pub struct CppMappingScope {
    previous: CppMapping,
}

impl CppMappingScope {
    /// Selects `mapping` on this thread.
    #[must_use]
    pub fn enter(mapping: CppMapping) -> Self {
        let previous = MAPPING.with(|cell| cell.replace(mapping));
        Self { previous }
    }
}

impl Drop for CppMappingScope {
    fn drop(&mut self) {
        MAPPING.with(|cell| cell.set(self.previous));
    }
}

/// The mapping selected on this thread.
#[must_use]
pub fn cpp_mapping() -> CppMapping {
    MAPPING.with(std::cell::Cell::get)
}

/// The namespace the native types are in right now: [`NATIVE_NAMESPACE`] under
/// [`CppMapping::Psm`], none otherwise and while the PSM layer spells its own
/// names.
fn native_prefix() -> Option<&'static str> {
    (cpp_mapping() != CppMapping::Native && !PREFIX_SUSPENDED.with(std::cell::Cell::get))
        .then_some(NATIVE_NAMESPACE)
}

/// The enclosing module names of `module`, outermost first, without the native namespace.
pub(crate) fn module_names(
    ast: &Ast,
    module: Id<ModuleNode>,
) -> Result<Vec<String>, MaterialiseError> {
    without_native_prefix(|| module_chain(ast, module))
}

/// Runs `f` with the native namespace taken off every path it spells, which is how the
/// PSM layer names the types it declares itself.
pub(crate) fn without_native_prefix<R>(f: impl FnOnce() -> R) -> R {
    let previous = PREFIX_SUSPENDED.with(|cell| cell.replace(true));
    let result = f();
    PREFIX_SUSPENDED.with(|cell| cell.set(previous));
    result
}

/// How deep a module chain is followed before the model is taken to be
/// malformed. IDL nests nowhere near this.
const MAX_MODULE_DEPTH: usize = 64;

/// How deep a type spelling is followed before the model is taken to
/// hold a cycle. A `sequence<sequence<…>>` nests nowhere near this.
const MAX_TYPE_DEPTH: usize = 64;

/// The module handle, named without the node the model would not answer
/// for. Slot and generation are what the arena was asked for.
fn module_handle(module: Id<ModuleNode>) -> String {
    format!("#{}@{}", module.slot(), module.generation())
}

/// The enclosing module names, outermost first, keyword-escaped.
///
/// The root is the anonymous global scope and contributes no segment.
fn module_chain(ast: &Ast, module: Id<ModuleNode>) -> Result<Vec<String>, MaterialiseError> {
    let mut out = Vec::new();
    let mut current = Some(module);
    let mut depth = 0usize;
    while let Some(id) = current {
        if depth > MAX_MODULE_DEPTH {
            return Err(MaterialiseError::NoModule {
                at: format!(
                    "{} — the parent chain runs past {MAX_MODULE_DEPTH}",
                    module_handle(module)
                ),
            });
        }
        depth += 1;
        let node = ast.module(id).map_err(|_| MaterialiseError::NoModule {
            at: module_handle(id),
        })?;
        if !node.name.is_empty() {
            out.push(cxx_name(&node.name));
        }
        current = node.parent;
    }
    out.reverse();
    if let Some(prefix) = native_prefix() {
        out.insert(0, prefix.to_string());
    }
    Ok(out)
}

/// One level of the namespace fold.
///
/// zerodds-lint: recursion-depth 64 (namespace nesting; every chain is
/// built by `module_chain`, which is bounded by `MAX_MODULE_DEPTH`)
fn nest(items: &[(Vec<String>, cpp::Decl)], depth: usize) -> Vec<cpp::Decl> {
    let mut out = Vec::new();
    let mut index = 0;
    while index < items.len() {
        let Some((chain, decl)) = items.get(index) else {
            break;
        };
        let Some(name) = chain.get(depth) else {
            out.push(decl.clone());
            index += 1;
            continue;
        };
        let name = name.clone();
        let start = index;
        while items
            .get(index)
            .is_some_and(|(chain, _)| chain.get(depth) == Some(&name))
        {
            index += 1;
        }
        out.push(cpp::Decl::Namespace(cpp::Namespace {
            doc: Doc::none(),
            name: Ident::new(&name),
            decls: nest(&items[start..index], depth + 1),
        }));
    }
    out
}

/// The name component a collection or a codec function takes from a
/// type.
///
/// The IDL kind and **not** the C++ spelling, and this is not cosmetic.
/// C++ has generics, so the type mapping is allowed to collapse things
/// the wire keeps apart, and it does:
///
/// - `octet` is `zerodds::wire::Octet` and `uint8` is `std::uint8_t`, which
///   the tag keeps apart anyway: the two are one wire form but two names;
/// - `wchar` and `unsigned short` are both `std::uint16_t`;
/// - `string`, `wstring` and `fixed<P,S>` are all `std::string`, and the
///   three are framed differently on the wire — a length in bytes, a
///   length in code units after a §7.4.1.1.2 conversion, and a decimal
///   in `P` digits;
/// - `double` and `long double` would both be `double` if the tag were
///   built from a C++ built-in, and they are eight and sixteen bytes.
///
/// The C column met the last of those as a real defect: the structural
/// name gave `sequence<double>` and `sequence<long double>` one
/// declaration and one encoder, and the second went out at the first's
/// width with nothing to say it had. C++ does not collapse the
/// *declarations* — there are none to collapse — but a codec emitter that
/// named its functions from the C++ spelling would collapse the
/// *functions*, which is the same defect one layer along. So the tag is
/// built here, from the model.
#[must_use]
pub fn cpp_type_tag(ast: &Ast, id: Id<TypeNode>) -> String {
    type_tag(ast, id, 0)
}

/// The bound a `string`, a `sequence` or a `map` carries, as the suffix
/// that keeps two of them apart.
///
/// The tag names the routine a hoisted collection's codec is emitted
/// under, and one body is emitted per name. With the bound left out,
/// `sequence<long>` and `sequence<long, 4>` are one name and whichever
/// was declared first decides: measured on
/// `@final struct S { sequence<long> b; sequence<long, 4> a; };`, the
/// generated C++ held no `zd_over` check at all, so `a` accepted a
/// stream of any length; with the members the other way round both got
/// the bounded body, so `b` refused — and truncated — a length the IDL
/// puts no limit on. `string<4>` beside `string` and `wstring<4>`
/// beside `wstring` collapse the same way, onto the trimming call.
/// Either way the reader answered with a value the IDL does not
/// describe and said nothing, which is why the bound is part of the
/// name. The same suffix the Zig column carries.
///
/// Only the *functions* split here. There is no declaration to split:
/// `sequence<long>` and `sequence<long, 4>` are both
/// `std::vector<std::int32_t>`, and [`cpp_value_type`] — not this — is
/// what a member is spelled through. That is the difference from the C
/// column, where the tag also names a declared struct.
///
/// An `array` needs nothing here: its `dims` are already in the tag,
/// under the `Array` arm below.
fn bound_tag(ast: &Ast, bound: &zerodds_ast::Bound) -> String {
    match bound.expr() {
        None => String::new(),
        // `_b` and not a bare `_n`: `seq_long_4` is what the `Array`
        // arm answers for an array of four `long`, and the two are
        // different wire forms.
        Some(expr) => match ast.eval_integral(expr) {
            Ok(limit) => format!("_b{limit}"),
            // As the array's `_x`: a bound that does not evaluate is
            // still not the unbounded form, and the materialiser
            // reports the expression that would not evaluate.
            Err(_) => "_bx".to_string(),
        },
    }
}

/// zerodds-lint: recursion-depth 64 (element types; bounded by
/// `MAX_TYPE_DEPTH`)
fn type_tag(ast: &Ast, id: Id<TypeNode>, depth: usize) -> String {
    if depth > MAX_TYPE_DEPTH {
        return "deep".to_string();
    }
    let Ok(node) = ast.type_(id) else {
        return "void".to_string();
    };
    match &node.kind {
        TypeKind::Primitive(kind) => kind.as_str().replace(' ', "_"),
        TypeKind::String {
            width: zerodds_ast::CharWidth::Char,
            bound,
        } => format!("string{}", bound_tag(ast, bound)),
        TypeKind::String {
            width: zerodds_ast::CharWidth::WChar,
            bound,
        } => format!("wstring{}", bound_tag(ast, bound)),
        TypeKind::Fixed { digits, scale } => format!("fixed_{digits}_{scale}"),
        TypeKind::Sequence { element, bound } => match element.resolved() {
            Some(element) => format!(
                "seq{}_{}",
                bound_tag(ast, bound),
                type_tag(ast, element, depth + 1)
            ),
            None => "void".to_string(),
        },
        TypeKind::Array { element, dims } => match element.resolved() {
            Some(element) => {
                let mut name = format!("arr_{}", type_tag(ast, element, depth + 1));
                for dim in dims {
                    match ast.eval_integral(*dim) {
                        Ok(length) => name.push_str(&format!("_{length}")),
                        // A dimension that does not evaluate gets no
                        // number rather than a zero: the name has to
                        // differ from a `[0]` array's, and the
                        // materialiser reports the failure by name.
                        Err(_) => name.push_str("_x"),
                    }
                }
                name
            }
            None => "void".to_string(),
        },
        TypeKind::Map { key, value, bound } => match (key.resolved(), value.resolved()) {
            (Some(key), Some(value)) => format!(
                "map{}_{}_{}",
                bound_tag(ast, bound),
                type_tag(ast, key, depth + 1),
                type_tag(ast, value, depth + 1)
            ),
            _ => "void".to_string(),
        },
        // A typedef with no name is one the frontend made for an inline
        // type; it is transparent, so the tag is the target's.
        TypeKind::Alias { aliased } if node.name.is_empty() => match aliased.resolved() {
            Some(target) => type_tag(ast, target, depth + 1),
            None => "void".to_string(),
        },
        _ => flattened(ast, id),
    }
}

/// The name a module or a type declared in IDL has in C++: the identifier,
/// or `cxx_` in front of it where it is a C++ reserved word. DDS-RPC 1.0
/// §7.11.1.1.2 rule 8: "Any identifier (e.g., parameter, operation name,
/// exception, interface, module) in an IDL file that is a reserved word in
/// C++ shall be prefixed by "cxx_"."
#[must_use]
pub fn cxx_name(name: &str) -> String {
    if cpp::escape_keyword(name) == name {
        name.to_string()
    } else {
        format!("cxx_{name}")
    }
}

/// A named type's scoped name with the separators flattened onto it.
///
/// Not what a C++ *reference* to the type is written as — that is
/// [`cpp_type_path`], which keeps the scoping — but what a generated
/// function name is built from, where `::` is not an identifier
/// character.
#[must_use]
pub fn flattened(ast: &Ast, id: Id<TypeNode>) -> String {
    let Ok(any) = ast.any_of_type(id) else {
        return "zd_unnamed".to_string();
    };
    match ast.scoped_name_of(any) {
        Some(scoped) => scoped
            .segments()
            .iter()
            .map(|segment| cxx_name(segment))
            .collect::<Vec<_>>()
            .join("_"),
        None => "zd_unnamed".to_string(),
    }
}

/// The rooted path a reference to a declared type is written as.
///
/// `::a::b::Foo` and never `Foo`: a member of a type in another module
/// is written inside whichever namespace its own declaration sits in,
/// and an unqualified name would resolve against that scope first. The
/// segments are keyword-escaped, the same escape `module_chain` and
/// `declare` apply, so the reference and the declaration spell the
/// same name.
#[must_use]
pub fn cpp_type_path(ast: &Ast, id: Id<TypeNode>) -> Path {
    let Ok(any) = ast.any_of_type(id) else {
        return Path::single("zd_unnamed");
    };
    let Some(scoped) = ast.scoped_name_of(any) else {
        return Path::single("zd_unnamed");
    };
    let escaped: Vec<String> = scoped
        .segments()
        .iter()
        .map(|segment| cxx_name(segment))
        .collect();
    let mut borrowed: Vec<&str> = escaped.iter().map(String::as_str).collect();
    if let Some(prefix) = native_prefix() {
        borrowed.insert(0, prefix);
    }
    Path::rooted(&borrowed)
}

/// The C++ type a value of `id` is held in.
///
/// The one answer, for every emitter — a class's field, a codec's
/// parameter, the element type a `std::vector` is instantiated at. The C
/// column learned that the hard way: three private copies of this
/// question in `encode_c`, `decode_c` and `default_c` let `long double`
/// diverge, with a `const double *` pointing at a sixteen-byte member.
///
/// Fallible where the C column's `c_value_type` is total, because C++
/// has nowhere to put a fudge. C writes an array's length into a *name*
/// (`zd_arr_int32_t_x` for a dimension that does not evaluate) and
/// reports the failure from the declaration site; here the length is in
/// the type itself, `std::array<T, N>`, so there is no name to spell
/// approximately and the refusal happens where the type is asked for.
///
/// `at` is where the type sits, as far as the model names it, and is
/// carried only so a refusal can say so.
///
/// # Errors
/// [`MaterialiseError::NoLanguageForm`] for `any`, `ValueBase` and an
/// object reference, at whatever depth they appear — `sequence<any>` is
/// refused as firmly as a bare `any` member.
/// [`MaterialiseError::Unevaluated`] for an array length that does not
/// evaluate, or is negative, and for a type nesting past
/// `MAX_TYPE_DEPTH`.
pub fn cpp_value_type(
    ast: &Ast,
    id: Id<TypeNode>,
    at: &str,
) -> Result<cpp::Type, MaterialiseError> {
    value_type(ast, id, at, 0)
}

/// zerodds-lint: recursion-depth 64 (element types; bounded by
/// `MAX_TYPE_DEPTH`)
fn value_type(
    ast: &Ast,
    id: Id<TypeNode>,
    at: &str,
    depth: usize,
) -> Result<cpp::Type, MaterialiseError> {
    if depth > MAX_TYPE_DEPTH {
        return Err(MaterialiseError::Unevaluated {
            construct: format!("type spelling — it nests past {MAX_TYPE_DEPTH}"),
            at: at.to_string(),
        });
    }
    let Ok(node) = ast.type_(id) else {
        return Ok(cpp::Type::Void);
    };
    let type_ = match &node.kind {
        // Matched ahead of the arm below, which would reach
        // `cpp_primitive`'s `double` — eight bytes where the member is
        // sixteen. See [`FLOAT128`].
        TypeKind::Primitive(PrimitiveKind::Float128) => float128(),
        TypeKind::Primitive(kind) => cpp_primitive(*kind),
        // Both string widths and `fixed<P,S>` are `std::string`. What
        // separates them is the framing, which is the codec's, and
        // [`cpp_type_tag`] is what keeps the codec's names apart.
        TypeKind::String { .. } | TypeKind::Fixed { .. } => std_type("string", Vec::new()),
        TypeKind::Sequence { element, .. } => std_type(
            "vector",
            vec![element_type(ast, element.resolved(), at, depth)?],
        ),
        TypeKind::Array { element, dims } => {
            let mut out = element_type(ast, element.resolved(), at, depth)?;
            // Innermost dimension first, so `long a[2][3]` becomes
            // `std::array<std::array<std::int32_t, 3>, 2>` — the outer
            // dimension outermost, which is the order the subscripts are
            // written in and the order the elements lie in.
            for dim in dims.iter().rev() {
                let length =
                    ast.eval_integral(*dim)
                        .map_err(|_| MaterialiseError::Unevaluated {
                            construct: "array length".to_string(),
                            at: at.to_string(),
                        })?;
                if length < 0 {
                    return Err(MaterialiseError::Unevaluated {
                        construct: format!("array length — it evaluates to {length}"),
                        at: at.to_string(),
                    });
                }
                out = std_type("array", vec![out, length_arg(length)]);
            }
            out
        }
        // XTypes 1.3 §7.5.1.3.3: a class with the operations of Table 47. It is
        // the vector of pairs the codec reads and writes, in the order the entries
        // were inserted, so `zerodds::Map<K, V>` is a `std::vector<std::pair<K, V>>`
        // too (runtime/cpp/include/zerodds/map.hpp). A bound is its third argument.
        TypeKind::Map { key, value, bound } => {
            let mut arguments = vec![
                element_type(ast, key.resolved(), at, depth)?,
                element_type(ast, value.resolved(), at, depth)?,
            ];
            if let Some(expr) = bound.expr() {
                let limit = ast
                    .eval_integral(expr)
                    .map_err(|_| MaterialiseError::Unevaluated {
                        construct: "map bound".to_string(),
                        at: at.to_string(),
                    })?;
                arguments.push(length_arg(limit));
            }
            cpp::Type::template(Path::rooted(&["zerodds", "Map"]), arguments)
        }
        // A typedef with no name is one the frontend made for an inline
        // type; it is transparent, so the C++ type is the target's.
        TypeKind::Alias { aliased } if node.name.is_empty() => {
            element_type(ast, aliased.resolved(), at, depth)?
        }
        // A boxed value is a nullable indirection. `std::shared_ptr` and
        // not a raw pointer: the class stays copyable, which every other
        // materialised type is, and nothing generated has to own or free
        // it by hand.
        TypeKind::ValueBox { boxed } => std_type(
            "shared_ptr",
            vec![element_type(ast, boxed.resolved(), at, depth)?],
        ),
        // `any` is in the model because Building Block Any is one of IDL
        // 4.2's fifteen, and what it holds is decided at run time — there
        // is no C++ type for it and no wire form to generate a codec
        // against. An object reference is a handle into the RPC arc and
        // `coverage.rs` says so. Both used to be reachable as a member
        // type; emitting the IDL spelling into a header produced a file
        // no compiler reads, from a generator that said nothing was
        // wrong.
        TypeKind::Any => return Err(no_form("any", at)),
        TypeKind::ValueBase => return Err(no_form("ValueBase", at)),
        TypeKind::ObjectRef { .. } => return Err(no_form("Object", at)),
        TypeKind::Alias { .. }
        | TypeKind::Structure { .. }
        | TypeKind::Union { .. }
        | TypeKind::Enumeration { .. }
        | TypeKind::Bitmask { .. }
        | TypeKind::Bitset { .. }
        | TypeKind::Valuetype { .. }
        | TypeKind::Native
        | TypeKind::AnnotationType { .. } => cpp::Type::named(cpp_type_path(ast, id)),
    };
    Ok(type_)
}

/// The type of a resolved-or-not element, key, value or alias target.
///
/// zerodds-lint: recursion-depth 64 (collection nesting, via
/// `value_type`; bounded by `MAX_TYPE_DEPTH`, which returns
/// `MaterialiseError::Unevaluated`)
fn element_type(
    ast: &Ast,
    target: Option<Id<TypeNode>>,
    at: &str,
    depth: usize,
) -> Result<cpp::Type, MaterialiseError> {
    match target {
        Some(target) => value_type(ast, target, at, depth + 1),
        // An unresolved reference is a frontend failure the model
        // already reports, and it is reported again here rather than
        // spelled `void`: a `std::vector<void>` would be a file no
        // compiler reads.
        None => Err(MaterialiseError::Unevaluated {
            construct: "element type".to_string(),
            at: at.to_string(),
        }),
    }
}

fn no_form(construct: &str, at: &str) -> MaterialiseError {
    MaterialiseError::NoLanguageForm {
        construct: construct.to_string(),
        language: "C++",
        at: at.to_string(),
    }
}

/// The C++ type an IDL primitive becomes.
///
/// Exhaustive, so a primitive added to the model without a C++ spelling
/// is a compile error and not a member that silently takes another's.
///
/// The fixed-width integers are written **`std::`-qualified**, not as
/// [`cpp::Primitive::Int32`] and its neighbours, which render the bare
/// `int32_t`. `<cstdint>` declares those names in namespace `std`; the
/// standard leaves it unspecified whether it also declares them at
/// global scope, and a generated header that names the unqualified form
/// is relying on an implementation's habit. The `Primitive` variants
/// stay right for a hand-written tree; they are not right for output
/// that has to compile wherever it is sent.
///
/// `char` is one byte and `wchar` two, which is what the wire form is.
/// XTypes 1.3 §7.5.1.1.1 leaves the C++ spelling of `Char16` to the
/// binding, and a fixed-width integer is the only choice that does not
/// depend on the platform's `wchar_t` — which is 16 bits on Windows and
/// 32 on Linux.
///
/// `long double` is **not** routed here: it takes [`FLOAT128`]. The
/// `Float128` arm below exists so the match is total and answers for a
/// caller that wants the nearest built-in; `value_type` matches
/// `Float128` before it reaches this. A caller that does not is the
/// defect the three-column gate found in the C column.
#[must_use]
pub fn cpp_primitive(kind: PrimitiveKind) -> cpp::Type {
    match kind {
        PrimitiveKind::Boolean => cpp::Type::Primitive(cpp::Primitive::Bool),
        PrimitiveKind::Char => cpp::Type::Primitive(cpp::Primitive::Char),
        PrimitiveKind::Float32 => cpp::Type::Primitive(cpp::Primitive::Float),
        PrimitiveKind::Float64 | PrimitiveKind::Float128 => {
            cpp::Type::Primitive(cpp::Primitive::Double)
        }
        // XTypes 1.3 §7.5.1.1.2: only Boolean, Char8 and Byte "may all map
        // to the same underlying C++ type"; every other type of Table 43 is
        // to be told from every other by an overload. `uint8` is
        // `std::uint8_t`, so `octet`, the Byte, is the runtime's one-octet
        // class (`zerodds::wire::Octet`), which converts to and from it.
        PrimitiveKind::Octet => octet(),
        PrimitiveKind::UInt8 => std_type("uint8_t", Vec::new()),
        PrimitiveKind::Int8 => std_type("int8_t", Vec::new()),
        PrimitiveKind::WChar | PrimitiveKind::UInt16 => std_type("uint16_t", Vec::new()),
        PrimitiveKind::Int16 => std_type("int16_t", Vec::new()),
        PrimitiveKind::Int32 => std_type("int32_t", Vec::new()),
        PrimitiveKind::UInt32 => std_type("uint32_t", Vec::new()),
        PrimitiveKind::Int64 => std_type("int64_t", Vec::new()),
        PrimitiveKind::UInt64 => std_type("uint64_t", Vec::new()),
    }
}

/// The unsigned integer a bit width calls for.
///
/// Public because the codec emitter has to cast into exactly this type:
/// a bitfield is unpacked out of a 64-bit anchor and assigned to a member
/// whose width is decided here, and a second copy of the rule would
/// narrow into a type this one had moved on from.
#[must_use]
pub fn cpp_holder(bits: u16) -> cpp::Type {
    match bits {
        0..=8 => std_type("uint8_t", Vec::new()),
        9..=16 => std_type("uint16_t", Vec::new()),
        17..=32 => std_type("uint32_t", Vec::new()),
        _ => std_type("uint64_t", Vec::new()),
    }
}

/// `std::name<args…>`.
/// The documentation a `std::shared_ptr` member carries.
///
/// One paragraph, written once, because the struct member and the union
/// branch are the same declaration and the same reason.
fn external_doc() -> Doc {
    Doc::lines(&[
        "Held behind an indirection — @external, DDS-XTypes 1.3 Table 21. The",
        "annotation changes no octet on the wire; it says where the binding",
        "keeps the value. A `std::shared_ptr` and not the value itself,",
        "because a member that reaches its own enclosing class back has no",
        "size at all held inline — `std::optional<T>` and a plain `T` member",
        "both require T complete, and clang answers `field has incomplete",
        "type` and six `incomplete type ... used in type trait expression`",
        "for `tests/interop-corpus/recursive-optional-external`. A shared_ptr",
        "is complete whatever it points at.",
        "",
        "Where the member is also @optional the pointer is the absence too",
        "(7.2.2.4.4.4.7): a null pointer is the member not being there, and a",
        "std::optional around it would carry a present-but-null state the",
        "wire has no encoding for.",
    ])
}

fn std_type(name: &str, args: Vec<cpp::Type>) -> cpp::Type {
    cpp::Type::template(Path::new(&["std", name]), args)
}

fn float128() -> cpp::Type {
    cpp::Type::named(Path::rooted(&["zerodds", "wire", "Float128"]))
}

/// `zerodds::wire::Octet`, the Byte (see [`cpp_primitive`]).
fn octet() -> cpp::Type {
    cpp::Type::named(Path::rooted(&["zerodds", "wire", "Octet"]))
}

/// A non-type template argument, as this tree can carry one.
///
/// `cpp::Type::Named` holds template *types*; `std::array<T, 3>`'s second
/// argument is a value, and the tree has no variant for one. The length
/// goes through as a path segment spelled with its digits, which is what
/// the renderer writes and what the compiler reads. It is a borrowed slot
/// and it is named as one here rather than left for a reader to discover:
/// adding a `Type::Value(Expr)` variant is a change to
/// `zerodds-lang-cst` and belongs in that crate's own commit.
fn length_arg(length: i128) -> cpp::Type {
    cpp::Type::named(Path::single(&format!("{length}")))
}

/// An integer literal that has a type wherever it is written.
///
/// A decimal literal with no suffix takes the first of `int`, `long` and
/// `long long` that holds it; a value above `i64::MAX` holds in none of
/// them, and clang answers with `-Wimplicitly-unsigned-literal`, which
/// under `-Werror` is a header that does not compile. An enumerator of a
/// `@bit_bound(64)` enumeration reaches that range, so the suffix is
/// written.
///
/// The other end has no suffix that helps it: −2^63 is not a literal in
/// C++ at all, and `zerodds_lang_render::cpp::integer_text` carries
/// both the clause and the form that spells it.
fn int_literal(value: i128) -> Literal {
    if value > i128::from(i64::MAX) {
        return Literal::Int(format!("{value}u"));
    }
    Literal::Int(zerodds_lang_render::cpp::integer_text(value))
}

/// The default member initializer IDL4 to C++ 1.0 §7.2.4.3.1 asks of a
/// struct's default constructor: "initializes all built-in data types to
/// their default value [0, `false`], enumerators to their first value,
/// and the rest of members using their default constructor".
///
/// A built-in gets `{}` (value initialization, which is that zero); an
/// enumeration its first enumerator by name, because an `@value` can put
/// something other than 0 there; an array `{}` as well, so its built-in
/// elements are not left indeterminate either. Everything else — string,
/// sequence, map, struct, union — has a default constructor of its own
/// and gets no initializer. Without one, `Point p;` left every built-in
/// member indeterminate.
fn member_default(
    ast: &Ast,
    target: Id<TypeNode>,
    spelled: &cpp::Type,
    at: &str,
) -> Result<Option<cpp::Expr>, MaterialiseError> {
    // Through the typedefs to what they name.
    let mut id = target;
    for _ in 0..=MAX_TYPE_DEPTH {
        let Ok(node) = ast.type_(id) else {
            return Err(MaterialiseError::Unevaluated {
                construct: "member type".to_string(),
                at: at.to_string(),
            });
        };
        match &node.kind {
            TypeKind::Alias { aliased } => match aliased.resolved() {
                Some(next) => id = next,
                None => return Ok(None),
            },
            TypeKind::Primitive(_) | TypeKind::Array { .. } => {
                return Ok(Some(cpp::Expr::InitList(Vec::new())));
            }
            TypeKind::Enumeration { enumerators } => {
                let first = enumerators
                    .first()
                    .and_then(|first| ast.enumerator(*first).ok())
                    .ok_or_else(|| MaterialiseError::Unevaluated {
                        construct: "the first enumerator of the member's enumeration".to_string(),
                        at: at.to_string(),
                    })?;
                let cpp::Type::Named { path, .. } = spelled else {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "the C++ name of the member's enumeration".to_string(),
                        at: at.to_string(),
                    });
                };
                return Ok(Some(cpp::Expr::Path(path.join(&cxx_name(&first.name)))));
            }
            _ => return Ok(None),
        }
    }
    Err(MaterialiseError::Unevaluated {
        construct: format!("typedef chain — it nests past {MAX_TYPE_DEPTH}"),
        at: at.to_string(),
    })
}

/// A type the model declares under the name `<TypeName>Bits` of a
/// bitmask's flag enumeration, in the namespace of the bitmask.
///
/// DDS-XTypes 1.3 §7.5.1.2.2 names that enumeration as part of the
/// binding. Two declarations of one name in one namespace are a
/// redefinition the C++ compiler reports; the refusal names both.
///
/// # Errors
/// [`MaterialiseError::SpellingCollision`] when a declaration is named
/// like the flag enumeration of a bitmask of the same module.
fn no_bits_name_collision(ast: &Ast) -> Result<(), MaterialiseError> {
    let mut spelled: Vec<(String, String)> = Vec::new();
    for id in ordered_types(ast) {
        let Ok(node) = ast.type_(id) else { continue };
        if node.name.is_empty() || !has_idl_name(&node.kind) {
            continue;
        }
        let scope = format!("{}@{}", node.module.slot(), node.module.generation());
        spelled.push((
            format!("{scope}::{}", cxx_name(&node.name)),
            node.name.clone(),
        ));
        if matches!(node.kind, TypeKind::Bitmask { .. }) {
            spelled.push((
                format!("{scope}::{}Bits", cxx_name(&node.name)),
                format!("{}Bits (the flag type of bitmask {})", node.name, node.name),
            ));
        }
    }
    no_spelling_collision(
        spelled
            .iter()
            .map(|(spelled, declared)| (spelled.as_str(), declared.as_str())),
        "C++",
        "the types of one header",
    )
}

/// How the native C++ mapping holds one member.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Held {
    /// The value itself.
    Value,
    /// A `std::shared_ptr<T>`: an `@external` member, or a member that
    /// closes a cycle and is neither `@optional` nor annotated.
    Shared,
    /// A plain `T *`, NULL when absent: an `@optional` member. DDS-XTypes
    /// 1.3 §7.5.1.2.4.2: "Optional members shall be represented using
    /// plain pointers rather than automatic values or smart pointers."
    Plain,
}

/// [`Held`] for `member`.
///
/// `@external` comes first: "A member that is both external and optional
/// shall be mapped as if it was just external" (§7.5.1.2.3.2.1). Then
/// `@optional`, whether or not it closes a cycle, and then a member that
/// only closes one.
pub fn held(ast: &Ast, member: &zerodds_ast::MemberNode) -> Held {
    if super::annotated_external(member) {
        Held::Shared
    } else if member.facets.is_optional() {
        Held::Plain
    } else if super::held_externally(ast, member) {
        Held::Shared
    } else {
        Held::Value
    }
}

/// The member functions a class needs when it owns what a pointer member
/// points at.
///
/// DDS-XTypes 1.3 §7.5.1.2.3.2: "The behavior of the constructor,
/// destructor, and copy functions shall be the same as specified for C"
/// — NULL after construction, the object deleted by the destructor, a deep
/// copy — and the same holds for the plain pointer of an `@optional`
/// member. `values` are the C++ names of the members that are copied as
/// they are and `pointers` those that are held behind a pointer; an empty
/// `pointers` means the implicit functions are right and nothing is
/// written.
///
/// Every pointer is handled by a function template of the runtime
/// (`clone`, `destroy`, `take`), whose instantiation is at the end of the
/// translation unit: a pointee declared after the class that holds it is
/// complete by then. The members start from their default member
/// initialisers, so the bodies assign.
fn owning_members(
    class: &str,
    base: Option<&str>,
    values: &[String],
    pointers: &[String],
) -> Vec<cpp::ClassMember> {
    if pointers.is_empty() {
        return Vec::new();
    }
    let wire = |function: &str| cpp::Expr::Path(Path::new(&["zerodds", "wire", function]));
    let call = |callee: cpp::Expr, args: Vec<cpp::Expr>| cpp::Expr::Call {
        callee: Box::new(callee),
        args,
    };
    let assign = |place: cpp::Expr, value: cpp::Expr| {
        cpp::Statement::Expr(cpp::Expr::Binary {
            op: "=",
            lhs: Box::new(place),
            rhs: Box::new(value),
        })
    };
    let mine = |name: &str| cpp::Expr::Path(Path::single(name));
    let theirs = |name: &str| cpp::Expr::Member {
        value: Box::new(cpp::Expr::Path(Path::single("zd_other"))),
        name: Ident::new(name),
    };
    let moved = |value: cpp::Expr| call(cpp::Expr::Path(Path::new(&["std", "move"])), vec![value]);
    let own_type = || cpp::Type::named(Path::single(class));
    let param = |type_: cpp::Type| cpp::Param {
        type_,
        name: Some(Ident::new("zd_other")),
        default: None,
    };
    let const_ref = || cpp::Type::Ref(Box::new(cpp::Type::Const(Box::new(own_type()))));
    let rvalue = || cpp::Type::RvalueRef(Box::new(own_type()));
    let not_self = |body: Vec<cpp::Statement>| cpp::Statement::If {
        condition: cpp::Expr::Binary {
            op: "!=",
            lhs: Box::new(cpp::Expr::Path(Path::single("this"))),
            rhs: Box::new(cpp::Expr::AddressOf(Box::new(mine("zd_other")))),
        },
        then: cpp::Block { statements: body },
        otherwise: None,
    };
    let give_self = || {
        cpp::Statement::Return(Some(cpp::Expr::Deref(Box::new(cpp::Expr::Path(
            Path::single("this"),
        )))))
    };
    let constructor = |doc: &str,
                       params: Vec<cpp::Param>,
                       initialisers: Vec<(Ident, cpp::Expr)>,
                       body: cpp::FunctionBody| {
        cpp::ClassMember::Constructor(cpp::Constructor {
            doc: Doc::line(doc),
            name: Ident::new(class),
            params,
            initialisers,
            is_explicit: false,
            is_constexpr: false,
            is_noexcept: false,
            body,
        })
    };
    let operator = |doc: &str, param_type: cpp::Type, body: Vec<cpp::Statement>| {
        cpp::ClassMember::Method(cpp::Method {
            doc: Doc::line(doc),
            returns: cpp::Type::Ref(Box::new(own_type())),
            name: Ident::new("operator="),
            params: vec![param(param_type)],
            is_const: false,
            is_static: false,
            is_virtual: false,
            is_pure: false,
            is_constexpr: false,
            is_noexcept: false,
            is_override: false,
            body: Some(cpp::Block { statements: body }),
        })
    };

    let mut copy = Vec::new();
    let mut copy_assign = Vec::new();
    let mut move_ctor = Vec::new();
    let mut move_assign = Vec::new();
    let mut release = Vec::new();
    // The functions written here replace the implicit ones, which copy and move the base class
    // too (XTypes 1.3 §7.5.1.4.1.1): a derived class passes `zd_other` on to its base.
    let (base_copy, base_move) = base.map_or((Vec::new(), Vec::new()), |base| {
        (
            vec![(Ident::new(base), mine("zd_other"))],
            vec![(Ident::new(base), moved(mine("zd_other")))],
        )
    });
    if let Some(base) = base {
        let assign_base = |value: cpp::Expr| {
            cpp::Statement::Expr(call(
                cpp::Expr::Path(Path::single(&format!("{base}::operator="))),
                vec![value],
            ))
        };
        copy_assign.push(assign_base(mine("zd_other")));
        move_assign.push(assign_base(moved(mine("zd_other"))));
    }
    for name in values {
        copy.push(assign(mine(name), theirs(name)));
        copy_assign.push(assign(mine(name), theirs(name)));
        move_ctor.push(assign(mine(name), moved(theirs(name))));
        move_assign.push(assign(mine(name), moved(theirs(name))));
    }
    for name in pointers {
        let cloned = call(wire("clone"), vec![theirs(name)]);
        let taken = call(wire("take"), vec![theirs(name)]);
        let destroy = || cpp::Statement::Expr(call(wire("destroy"), vec![mine(name)]));
        copy.push(assign(mine(name), cloned.clone()));
        // The copy is made before the old object goes, so a self-contained
        // graph survives its own replacement.
        copy_assign.push(cpp::Statement::Declare {
            type_: cpp::Type::Auto,
            name: Ident::new(&format!("zd_new_{name}")),
            value: Some(cloned),
        });
        copy_assign.push(destroy());
        copy_assign.push(assign(mine(name), mine(&format!("zd_new_{name}"))));
        move_ctor.push(assign(mine(name), taken.clone()));
        move_assign.push(destroy());
        move_assign.push(assign(mine(name), taken));
        release.push(destroy());
    }
    // The copy is made first and every pointer is assigned after the values,
    // so a copy that throws leaves the destination as it was.
    copy_assign.sort_by_key(|statement| !matches!(statement, cpp::Statement::Declare { .. }));
    vec![
        constructor(
            "NULL pointers (XTypes 1.3 7.5.1.2.3.2, as in C).",
            Vec::new(),
            Vec::new(),
            cpp::FunctionBody::Defaulted,
        ),
        constructor(
            "A deep copy of what the pointer members point at.",
            vec![param(const_ref())],
            base_copy,
            cpp::FunctionBody::Block(cpp::Block { statements: copy }),
        ),
        constructor(
            "Takes the pointers over and leaves NULL behind.",
            vec![param(rvalue())],
            base_move,
            cpp::FunctionBody::Block(cpp::Block {
                statements: move_ctor,
            }),
        ),
        operator("A deep copy.", const_ref(), {
            let mut body = vec![not_self(copy_assign)];
            body.push(give_self());
            body
        }),
        operator("Takes the pointers over.", rvalue(), {
            let mut body = vec![not_self(move_assign)];
            body.push(give_self());
            body
        }),
        cpp::ClassMember::Destructor(cpp::Destructor {
            doc: Doc::line("Deletes what the non-NULL pointer members point at."),
            name: Ident::new(class),
            is_virtual: false,
            body: cpp::FunctionBody::Block(cpp::Block {
                statements: release,
            }),
        }),
    ]
}

/// One data member.
fn field(type_: cpp::Type, name: &str, doc: Doc) -> cpp::ClassMember {
    cpp::ClassMember::Field(cpp::Field {
        doc,
        type_,
        name: Ident::new(&cxx_name(name)),
        dims: Vec::new(),
        default: None,
        is_static: false,
        is_constexpr: false,
        bits: None,
    })
}

/// The storage of a union's discriminator, and the prefix of a case's.
///
/// The accessor functions `_d()` and `x()` of IDL4 to C++ 1.0 take the
/// names an IDL union gives them, so the storage behind them has others;
/// the generated codec reads and writes the storage directly.
pub(crate) const UNION_DISCRIMINATOR: &str = "zd_d";

/// The storage of the case `field` (see [`UNION_DISCRIMINATOR`]).
pub(crate) fn union_storage(field: &str) -> String {
    format!("zd_{field}")
}

/// One case of a union class: its spelled name, its type and the text of
/// the discriminator value its setter selects (`None` for a case that has
/// no value of its own, such as the `default` one).
pub(crate) struct UnionCase {
    /// The spelling of the case member.
    pub(crate) name: String,
    /// The C++ type of the case.
    pub(crate) type_: cpp::Type,
    /// The discriminator value the setter of the case selects, as C++ text.
    pub(crate) label: Option<String>,
    /// What the storage of the case says about itself.
    pub(crate) doc: Doc,
}

/// The members of a union class: the accessor functions of IDL4 to C++ 1.0
/// (`_d()` and `_d(value)`; per case `const T& x() const`, `T& x()`,
/// `void x(const T&)` and `void x(T&&)`, a setter selecting the case), then
/// the storage they stand on. Which storage is live is what the
/// discriminator says.
pub(crate) fn union_members(
    discriminator: &cpp::Type,
    cases: &[UnionCase],
) -> Vec<cpp::ClassMember> {
    let disc = zerodds_lang_render::cpp::type_(discriminator);
    let mut text = format!(
        "    /// The discriminator.\n    {disc} _d() const {{ return {UNION_DISCRIMINATOR}; }}\n    void _d({disc} value) {{ {UNION_DISCRIMINATOR} = value; }}\n"
    );
    for case in cases {
        let ty = zerodds_lang_render::cpp::type_(&case.type_);
        let storage = union_storage(&case.name);
        let select = case.label.as_ref().map_or(String::new(), |label| {
            format!(" {UNION_DISCRIMINATOR} = static_cast<{disc}>({label});")
        });
        text.push_str(&format!(
            "    const {ty}& {name}() const {{ return {storage}; }}\n    {ty}& {name}() {{ return {storage}; }}\n    void {name}(const {ty}& value) {{ {storage} = value;{select} }}\n    void {name}({ty}&& value) {{ {storage} = std::move(value);{select} }}\n",
            name = case.name
        ));
    }
    let mut members = vec![cpp::ClassMember::Verbatim(text)];
    members.push(field(
        discriminator.clone(),
        UNION_DISCRIMINATOR,
        Doc::line("The storage of `_d()`. Which storage below is live is what it says."),
    ));
    for case in cases {
        members.push(cpp::ClassMember::Field(cpp::Field {
            doc: case.doc.clone(),
            type_: case.type_.clone(),
            name: Ident::new(&union_storage(&case.name)),
            dims: Vec::new(),
            default: None,
            is_static: false,
            is_constexpr: false,
            bits: None,
        }));
    }
    members
}

/// The parameter name of the generated `swap`, which no IDL member is
/// likely to be spelled: a member of the same name would be shadowed.
const SWAP_PARAMETER: &str = "zd_other";

/// `void swap(T & zd_other) { std::swap(a, zd_other.a); … }`
fn swap_member(name: &str, names: &[(String, String)]) -> cpp::ClassMember {
    let statements: Vec<_> = names
        .iter()
        .map(|(spelled, _)| {
            cpp::Statement::Expr(cpp::Expr::Call {
                callee: Box::new(cpp::Expr::Path(Path::new(&["std", "swap"]))),
                args: vec![
                    cpp::Expr::Path(Path::single(spelled)),
                    cpp::Expr::Member {
                        value: Box::new(cpp::Expr::Path(Path::single(SWAP_PARAMETER))),
                        name: Ident::new(spelled),
                    },
                ],
            })
        })
        .collect();
    cpp::ClassMember::Method(cpp::Method {
        doc: Doc::line("Exchanges every member with `zd_other` (DDS-PSM-Cxx 1.0 7.12)."),
        returns: cpp::Type::Void,
        name: Ident::new("swap"),
        params: vec![cpp::Param {
            type_: cpp::Type::Ref(Box::new(cpp::Type::named(Path::single(name)))),
            // A struct with no member leaves the parameter unused (-Wunused-parameter).
            name: (!statements.is_empty()).then(|| Ident::new(SWAP_PARAMETER)),
            default: None,
        }],
        is_const: false,
        is_static: false,
        is_virtual: false,
        is_pure: false,
        is_constexpr: false,
        is_noexcept: false,
        is_override: false,
        body: Some(cpp::Block { statements }),
    })
}

/// `inline void swap(T & a, T & b) { a.swap(b); }`, in the type's namespace.
fn swap_function(name: &str) -> cpp::Decl {
    let reference = || cpp::Type::Ref(Box::new(cpp::Type::named(Path::single(name))));
    cpp::Decl::Function(cpp::Function {
        doc: Doc::line("Exchanges two values (DDS-PSM-Cxx 1.0 7.12)."),
        returns: cpp::Type::Void,
        name: Ident::new("swap"),
        params: vec![
            cpp::Param {
                type_: reference(),
                name: Some(Ident::new("zd_a")),
                default: None,
            },
            cpp::Param {
                type_: reference(),
                name: Some(Ident::new("zd_b")),
                default: None,
            },
        ],
        is_inline: true,
        is_static: false,
        is_constexpr: false,
        is_noexcept: false,
        is_maybe_unused: false,
        body: cpp::FunctionBody::Block(cpp::Block {
            statements: vec![cpp::Statement::Expr(cpp::Expr::MethodCall {
                receiver: Box::new(cpp::Expr::Path(Path::single("zd_a"))),
                name: Ident::new("swap"),
                args: vec![cpp::Expr::Path(Path::single("zd_b"))],
            })],
        }),
    })
}

/// The suffix of the variable carrying a declaration's IDL member names.
///
/// Appended to the declaration's own C++ name, inside the declaration's
/// own namespace — so `a::b::Sample` is answered for by
/// `a::b::Sample_idl_names` and two declarations cannot reach one
/// variable unless their names already collided. A name this does
/// collide with is a redefinition the compiler reports, never a mapping
/// one of the two lost.
const IDL_NAMES_SUFFIX: &str = "_idl_names";

/// The suffix of the variable carrying a declaration's own IDL name.
///
/// A reserved word moves a type too — `struct class` becomes `cxx_class`
/// (DDS-RPC 1.0 §7.11.1.1.2 rule 8) — and a type's name reaches the wire
/// twice over: as `TypeObject`'s `type_name` and as the topic type name a
/// peer matches on. The variable is named after the C++ spelling, so
/// `struct class` is `cxx_class_idl_name`. The C++ spelling cannot be
/// inverted back to the IDL name, because an IDL type that is already
/// called `cxx_class` arrives at the same identifier.
const IDL_NAME_SUFFIX: &str = "_idl_name";

/// `const char *`, the spelling both halves of an entry take.
fn text_pointer() -> cpp::Type {
    cpp::Type::Pointer(Box::new(cpp::Type::Const(Box::new(cpp::Type::Primitive(
        cpp::Primitive::Char,
    )))))
}

/// The `<name>_idl_names` table for a declaration's named parts.
///
/// `pairs` is `(the C++ spelling, the IDL spelling)` in IDL declaration
/// order — the order the member IDs are assigned in (XTypes 1.3
/// §7.3.1.2.1.1) — and an entry is written even where the two spellings
/// are the same, because a caller computing a member ID needs every
/// member's IDL name and not only the ones a keyword moved.
///
/// [`None`] for a declaration with no named parts. `std::array<T, 0>` is
/// a legal type and an empty table answers nothing, and the two
/// declarations that reach it — an enumeration with no enumerators, a
/// bitset that is all padding — have no member to be asked about.
/// The table only exists where the escape is invertible.
///
/// Two members whose IDL names differ and whose spellings do not are
/// two entries with the same left half — the table would record the
/// collision rather than refuse it, and a reader recovering the IDL
/// name would get whichever entry it hit first. The check is here
/// because this is the function that holds both spellings for a whole
/// scope; nothing upstream sees more than one member at a time.
///
/// # Errors
/// [`MaterialiseError::SpellingCollision`] when two members reach one
/// spelling.
fn idl_names(
    name: &str,
    pairs: &[(String, String)],
) -> Result<Option<cpp::Decl>, MaterialiseError> {
    no_spelling_collision(
        pairs
            .iter()
            .map(|(spelled, declared)| (spelled.as_str(), declared.as_str())),
        "C++",
        name,
    )?;
    if pairs.is_empty() {
        return Ok(None);
    }
    let entries: Vec<cpp::Expr> = pairs
        .iter()
        .map(|(spelling, idl)| {
            cpp::Expr::InitList(vec![
                cpp::Expr::Literal(Literal::Str(spelling.clone())),
                cpp::Expr::Literal(Literal::Str(idl.clone())),
            ])
        })
        .collect();
    Ok(Some(cpp::Decl::Variable(cpp::Variable {
        doc: Doc::lines(&[
            "The IDL spelling of each member, beside the spelling this header",
            "uses, in IDL declaration order.",
            "",
            "Load-bearing, not documentation. A member's ID under @autoid(HASH)",
            "and under @hashid is the first four octets of the MD5 of its IDL",
            "name (XTypes 1.3 7.3.1.2.1.1), so a caller holding only the C++",
            "spelling — which a keyword collision moved — cannot compute the ID",
            "of the member it is writing.",
        ]),
        type_: std_type(
            "array",
            vec![
                std_type("pair", vec![text_pointer(), text_pointer()]),
                // The count's digits, straight from the entry list.
                // [`length_arg`] is the same borrowed slot for a length
                // the model computed; this one is the length of a `Vec`
                // and reaches the output without a conversion that
                // could answer a different number than it holds.
                cpp::Type::named(Path::single(&pairs.len().to_string())),
            ],
        ),
        name: Ident::new(&format!("{name}{IDL_NAMES_SUFFIX}")),
        // The outer brace is `std::array`'s own aggregate and the inner
        // one is the C array inside it. Written out rather than left to
        // brace elision, which clang reports as `-Wmissing-braces` and
        // `-Werror` then refuses.
        value: Some(cpp::Expr::InitList(vec![cpp::Expr::InitList(entries)])),
        is_static: false,
        is_constexpr: true,
        is_inline: true,
    })))
}

/// The `<name>_idl_name` variable for a declaration's own IDL name.
fn idl_name(name: &str, idl: &str) -> cpp::Decl {
    cpp::Decl::Variable(cpp::Variable {
        doc: Doc::line("The IDL spelling of the declaration above."),
        type_: text_pointer(),
        name: Ident::new(&format!("{name}{IDL_NAME_SUFFIX}")),
        value: Some(cpp::Expr::Literal(Literal::Str(idl.to_string()))),
        is_static: false,
        is_constexpr: true,
        is_inline: true,
    })
}

/// Whether a type's C++ name was made from an IDL identifier.
///
/// The collections are the reason this is asked: `std::vector<T>` is
/// written where it is used and names nothing, and the kinds below all
/// take their C++ name from [`cxx_name`] applied to an
/// identifier the source wrote.
const fn has_idl_name(kind: &TypeKind) -> bool {
    matches!(
        kind,
        TypeKind::Structure { .. }
            | TypeKind::Valuetype { .. }
            | TypeKind::Union { .. }
            | TypeKind::Enumeration { .. }
            | TypeKind::Bitmask { .. }
            | TypeKind::Bitset { .. }
            | TypeKind::Alias { .. }
            | TypeKind::Native
            | TypeKind::ValueBox { .. }
    )
}

/// `struct Name { public: … };`
///
/// A `struct` and not a `class`: every member of a generated data type is
/// public, and a `class` would need the access label to say so. The
/// section is written all the same — the tree groups by access and the
/// renderer writes the label once — and is left out entirely when there
/// is nothing in it, so an empty IDL structure produces `struct S {\n};`
/// and not a stray `public:`.
fn class(name: &str, members: Vec<cpp::ClassMember>, doc: Doc) -> cpp::Class {
    let sections = if members.is_empty() {
        Vec::new()
    } else {
        vec![cpp::Section {
            access: cpp::Access::Public,
            members,
        }]
    };
    cpp::Class {
        doc,
        is_struct: true,
        name: Ident::new(name),
        bases: Vec::new(),
        sections,
    }
}

/// The three maps of a type. DDS-XTypes 1.3 §7.5.1.3.2.1: "For any type T,
/// the declaration and implementation of the map types having element type
/// T and key types UInt32, UInt64, and String shall be generated alongside
/// the implementation code for element type T", and §7.5.1.3.3.1 places the
/// class "in the same namespace as the element type declaration":
/// `UInt32MyElementTypeMap`, `UInt64MyElementTypeMap`,
/// `StringMyElementTypeMap`, each a `zerodds::Map`.
///
/// Under [`CppMapping::Psm`] a map is a `std::map` (§7.5.1.3.4) and the
/// classes are not written.
fn maps_alongside(
    ast: &Ast,
    id: Id<TypeNode>,
    out: &mut Vec<(Id<ModuleNode>, cpp::Decl)>,
    emitted: &[String],
) {
    if cpp_mapping() == CppMapping::Psm {
        return;
    }
    let Ok(node) = ast.type_(id) else { return };
    let declared = match &node.kind {
        TypeKind::Structure { .. }
        | TypeKind::Union { .. }
        | TypeKind::Enumeration { .. }
        | TypeKind::Bitmask { .. }
        | TypeKind::Bitset { .. } => true,
        TypeKind::Alias { .. } => !node.name.is_empty(),
        _ => false,
    };
    if !declared {
        return;
    }
    let key = cpp_type_path(ast, id)
        .segments()
        .iter()
        .map(Ident::as_str)
        .collect::<Vec<_>>()
        .join("::");
    if !emitted.contains(&key) {
        return;
    }
    let name = cxx_name(&node.name);
    for (key_name, key_type) in [
        ("UInt32", std_type("uint32_t", Vec::new())),
        ("UInt64", std_type("uint64_t", Vec::new())),
        ("String", std_type("string", Vec::new())),
    ] {
        out.push((
            node.module,
            cpp::Decl::Alias(cpp::Alias {
                doc: Doc::none(),
                name: Ident::new(&format!("{key_name}{name}Map")),
                type_: cpp::Type::template(
                    Path::rooted(&["zerodds", "Map"]),
                    vec![key_type, cpp::Type::named(cpp_type_path(ast, id))],
                ),
            }),
        ));
    }
}

#[allow(clippy::too_many_lines)]
fn declare(
    ast: &Ast,
    applied: &Applied,
    id: Id<TypeNode>,
    out: &mut Vec<(Id<ModuleNode>, cpp::Decl)>,
    emitted: &mut Vec<String>,
) -> Result<(), MaterialiseError> {
    let Ok(node) = ast.type_(id) else {
        return Ok(());
    };

    match &node.kind {
        // C++ writes these where they are used, so none of them declares
        // anything. This is where the C column's `zd_seq_`, `zd_arr_`,
        // `zd_map_`, `zd_string` and `zd_fixed_` declarations were, and
        // with them the collision hazard a structural name carries.
        TypeKind::Primitive(_)
        | TypeKind::String { .. }
        | TypeKind::Fixed { .. }
        | TypeKind::Sequence { .. }
        | TypeKind::Array { .. }
        | TypeKind::Map { .. } => return Ok(()),
        // An object reference belongs to the RPC arc and an annotation
        // type has no runtime form at all — `zerodds_ast_layout` refuses
        // to plan either — so a C++ declaration would be a shape nothing
        // could serialise. `any` and `ValueBase` for the same reason and
        // a different one: their content is decided at run time. Named
        // here rather than passed over, so the coverage table and this
        // file say the same thing. Every one of them is refused by name
        // where it is *used*, in `value_type`.
        TypeKind::ObjectRef { .. }
        | TypeKind::AnnotationType { .. }
        | TypeKind::Any
        | TypeKind::ValueBase => return Ok(()),
        _ => {}
    }

    let name = cxx_name(&node.name);
    let key = cpp_type_path(ast, id)
        .segments()
        .iter()
        .map(Ident::as_str)
        .collect::<Vec<_>>()
        .join("::");
    if emitted.contains(&key) {
        return Ok(());
    }

    match &node.kind {
        TypeKind::Alias { aliased } => {
            if node.name.is_empty() {
                return Ok(());
            }
            if ghost_created_for(applied, node.node_id, "CppAlias").is_none() {
                return Ok(());
            }
            let Some(target) = aliased.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "typedef target".to_string(),
                    at: name,
                });
            };
            emitted.push(key);
            out.push((
                node.module,
                cpp::Decl::Alias(cpp::Alias {
                    doc: Doc::none(),
                    name: Ident::new(&name),
                    type_: cpp_value_type(ast, target, &name)?,
                }),
            ));
        }

        TypeKind::Enumeration { enumerators } => {
            if ghost_created_for(applied, node.node_id, "CppEnum").is_none() {
                return Ok(());
            }
            let mut values = Vec::new();
            let mut names = Vec::new();
            for enumerator in enumerators {
                let Ok(enum_node) = ast.enumerator(*enumerator) else {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "enumerator the model listed".to_string(),
                        at: node.name.clone(),
                    });
                };
                if ghost_created_for(applied, enum_node.node_id, "CppEnumerator").is_none() {
                    continue;
                }
                let value = ast.effective_enumerator_value(*enumerator).ok_or_else(|| {
                    MaterialiseError::Unevaluated {
                        construct: "enumerator value".to_string(),
                        at: enum_node.name.clone(),
                    }
                })?;
                values.push((cxx_name(&enum_node.name), value));
                names.push((cxx_name(&enum_node.name), enum_node.name.clone()));
            }
            emitted.push(key);
            out.push((
                node.module,
                cpp::Decl::Enum(cpp::Enum {
                    doc: Doc::lines(&[
                        "An enumeration. The underlying type is chosen from the",
                        "@bit_bound and from the values themselves — a C enumeration",
                        "constant is an `int` and could hold neither end of",
                        "@bit_bound(32), which is why the C column emits a typedef and",
                        "a run of #defines instead.",
                        "",
                        "Scoped, so the enumerators do not leak into the namespace. IDL",
                        "4.2 7.4.1.4.2 names them from the scope enclosing the",
                        "enumeration — `M::medium`, never `M::Size::medium` — and this",
                        "is the one place the C++ mapping does not reproduce the IDL",
                        "spelling: an unscoped `enum` would put every enumerator of",
                        "every enumeration of a module in one namespace, where two",
                        "IDL-legal names can collide.",
                    ]),
                    is_scoped: true,
                    name: Ident::new(&name),
                    underlying: Some(enum_underlying(ast.effective_bit_bound(id), &values)),
                    enumerators: values
                        .into_iter()
                        .map(|(name, value)| cpp::Enumerator {
                            doc: Doc::none(),
                            name: Ident::new(&name),
                            // Written out, never left to position: a C++
                            // enumeration renumbers silently when an
                            // enumerator is inserted, and the wire format
                            // is the IDL's numbering.
                            value: Some(cpp::Expr::Literal(int_literal(value))),
                        })
                        .collect(),
                }),
            ));
            out.extend(
                idl_names(&name, &names)?
                    .into_iter()
                    .map(|decl| (node.module, decl)),
            );
        }

        TypeKind::Bitmask { bits } => {
            if ghost_created_for(applied, node.node_id, "CppBitmask").is_none() {
                return Ok(());
            }
            emitted.push(key);
            let bound = ast.effective_bit_bound(id);
            let mut names = Vec::new();
            let mut flag_values: Vec<(String, i128)> = Vec::new();
            // An **integer alias with constants**, not a scoped
            // enumeration, and the reason is what a bitmask value is: the
            // OR of two bits. `Flags::A | Flags::B` is not a value of a
            // scoped enumeration — the operator is not even declared for
            // one — and a value outside an enumeration's range is
            // unspecified once its bit-field cannot hold it
            // ([dcl.enum]/8). An enumeration whose values are meant to be
            // combined is an enumeration used for something it does not
            // model. The alias also puts the C++ and the C column on the
            // same shape, so their constants compare directly.
            out.push((
                node.module,
                cpp::Decl::Alias(cpp::Alias {
                    doc: Doc::line(
                        "A bitmask, as the unsigned integer Table 33 of XTypes 1.3 \
                         7.4.1.1.3.2 gives for its bound.",
                    ),
                    name: Ident::new(&name),
                    type_: cpp_holder(bound),
                }),
            ));
            for bit in bits {
                let Ok(bit_node) = ast.bit_value(*bit) else {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "bitmask bit the model listed".to_string(),
                        at: node.name.clone(),
                    });
                };
                if ghost_created_for(applied, bit_node.node_id, "CppBitConst").is_none() {
                    continue;
                }
                let position = ast.effective_bit_position(*bit).ok_or_else(|| {
                    MaterialiseError::Unevaluated {
                        construct: "bit position".to_string(),
                        at: bit_node.name.clone(),
                    }
                })?;
                // `Flags{1} << 63` and not `1 << 63`: the second shifts
                // an `int` past its width, which is undefined behaviour
                // and reachable from a @bit_bound(64) bitmask. The
                // `static_cast` back is what makes the initialiser the
                // alias's own type rather than the `int` the shift
                // promotes to.
                let one = cpp::Expr::Construct {
                    type_: Box::new(cpp::Type::named(cpp_type_path(ast, id))),
                    args: vec![cpp::Expr::Literal(Literal::int(1))],
                    braced: true,
                };
                names.push((cxx_name(&bit_node.name), bit_node.name.clone()));
                out.push((
                    node.module,
                    cpp::Decl::Variable(cpp::Variable {
                        doc: Doc::none(),
                        type_: cpp::Type::named(cpp_type_path(ast, id)),
                        name: Ident::new(&cxx_name(&bit_node.name)),
                        value: Some(cpp::Expr::Cast {
                            kind: cpp::CastKind::Static,
                            type_: Box::new(cpp::Type::named(cpp_type_path(ast, id))),
                            value: Box::new(cpp::Expr::Paren(Box::new(cpp::Expr::Binary {
                                op: "<<",
                                lhs: Box::new(one),
                                rhs: Box::new(cpp::Expr::Literal(Literal::int(i128::from(
                                    position,
                                )))),
                            }))),
                        }),
                        is_static: false,
                        is_constexpr: true,
                        is_inline: true,
                    }),
                ));
                flag_values.push((cxx_name(&bit_node.name), 1i128 << position));
            }
            // XTypes 1.3 §7.5.1.2.2: "the language binding shall be as if
            // there was an enumeration definition like the following:
            // @bit_bound(n) enum <TypeName>Bits { @value(1 << p) FLAG, … }".
            // Scoped, like every enumeration of this column, so its
            // enumerators do not meet the constants above.
            out.push((
                node.module,
                cpp::Decl::Enum(cpp::Enum {
                    doc: Doc::line(
                        "The enumeration of the flags of a bitmask (XTypes 1.3 \
                         7.5.1.2.2): one enumerator per flag, valued 1 << position.",
                    ),
                    is_scoped: true,
                    name: Ident::new(&format!("{name}Bits")),
                    underlying: Some(cpp_holder(bound)),
                    enumerators: flag_values
                        .into_iter()
                        .map(|(flag, value)| cpp::Enumerator {
                            doc: Doc::none(),
                            name: Ident::new(&flag),
                            value: Some(cpp::Expr::Literal(int_literal(value))),
                        })
                        .collect(),
                }),
            ));
            out.extend(
                idl_names(&name, &names)?
                    .into_iter()
                    .map(|decl| (node.module, decl)),
            );
        }

        TypeKind::Bitset { .. } => {
            if ghost_created_for(applied, node.node_id, "CppBitset").is_none() {
                return Ok(());
            }
            emitted.push(key);
            let mut members = Vec::new();
            let mut names = Vec::new();
            // The base's bitfields are members of the derived class too:
            // an inherited bitset is one flat run.
            for bitfield in &ast.all_bitfields_of(id) {
                let Ok(field_node) = ast.bitfield(*bitfield) else {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "bitset field the model listed".to_string(),
                        at: node.name.clone(),
                    });
                };
                // An anonymous field is padding: it moves the next field
                // along and carries no value of its own.
                let Some(field_name) = field_node.name.as_ref() else {
                    continue;
                };
                // Not a C++ bit-field: the allocation order of bit-fields
                // within an allocation unit is implementation-defined
                // ([class.bit]/1), and the order on the wire is the
                // declaration's. Packing is the codec's, where it can be
                // seen.
                names.push((cxx_name(field_name), field_name.clone()));
                members.push(field(cpp_holder(field_node.bits), field_name, Doc::none()));
            }
            out.push((
                node.module,
                cpp::Decl::Class(class(
                    &name,
                    members,
                    Doc::line(
                        "A bitset. The fields are separate integers here; the packing \
                         into the anchor is the codec's, because C++ leaves bit-field \
                         allocation order to the implementation.",
                    ),
                )),
            ));
            out.extend(
                idl_names(&name, &names)?
                    .into_iter()
                    .map(|decl| (node.module, decl)),
            );
        }

        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            let created = if matches!(node.kind, TypeKind::Valuetype { .. }) {
                "CppValue"
            } else {
                "CppStruct"
            };
            if ghost_created_for(applied, node.node_id, created).is_none() {
                return Ok(());
            }
            emitted.push(key);
            // XTypes 1.3 §7.5.1.4.1.1: "The C++ struct corresponding to the
            // subtype shall publicly inherit from the C++ struct
            // corresponding to the supertype." So the class declares its own
            // members and has the base's by inheritance; the plan still puts
            // a base's members on the wire first (§7.4.1.1.5.1), and the
            // codec reaches them by name, as members of the derived class.
            // `names` lists all of them, the base's first: the table of
            // IDL names and `swap` are about everything the class has.
            let base = match &node.kind {
                TypeKind::Structure {
                    base: Some(base), ..
                } => base.resolved(),
                _ => None,
            };
            let own: Vec<_> = ast.members_of(id).to_vec();
            let mut members = Vec::new();
            let mut names = Vec::new();
            let mut values: Vec<String> = Vec::new();
            let mut pointers: Vec<String> = Vec::new();
            for member in ast.all_members_of(id) {
                let Ok(member_node) = ast.member(member) else {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "member the model listed".to_string(),
                        at: node.name.clone(),
                    });
                };
                names.push((cxx_name(&member_node.name), member_node.name.clone()));
                if base.is_some() && !own.contains(&member) {
                    continue;
                }
                let Some(target) = member_node.type_.resolved() else {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "member type".to_string(),
                        at: member_node.name.clone(),
                    });
                };
                let inner = cpp_value_type(ast, target, &member_node.name)?;
                match held(ast, member_node) {
                    Held::Shared => {
                        members.push(field(
                            std_type("shared_ptr", vec![inner]),
                            &member_node.name,
                            external_doc(),
                        ));
                        pointers.push(cxx_name(&member_node.name));
                    }
                    Held::Plain => {
                        let mut member = field(
                            cpp::Type::Pointer(Box::new(inner)),
                            &member_node.name,
                            Doc::lines(&[
                                "An @optional member: a plain pointer, NULL for an omitted",
                                "value. DDS-XTypes 1.3 7.5.1.2.4.2: \"Optional members shall be",
                                "represented using plain pointers rather than automatic values",
                                "or smart pointers [...] A NULL pointer shall indicate an",
                                "omitted value.\" The class owns what it points at: the",
                                "destructor deletes it and a copy is deep, as 7.5.1.2.3.2 asks",
                                "of an external member. In a @mutable type an absent member is",
                                "not written at all; outside one its absence is written — a",
                                "zero <is_present> byte under XCDR2 (rule 20) or a parameter",
                                "header declaring a length of zero under XCDR1 (7.4.1.2).",
                            ]),
                        );
                        if let cpp::ClassMember::Field(f) = &mut member {
                            f.default = Some(cpp::Expr::Path(Path::new(&["nullptr"])));
                        }
                        members.push(member);
                        pointers.push(cxx_name(&member_node.name));
                    }
                    Held::Value => {
                        let default = member_default(ast, target, &inner, &member_node.name)?;
                        let mut member = field(inner, &member_node.name, Doc::none());
                        if let cpp::ClassMember::Field(f) = &mut member {
                            f.default = default;
                        }
                        members.push(member);
                        values.push(cxx_name(&member_node.name));
                    }
                }
            }
            let base_text = base.map(|base| {
                let path = cpp_type_path(ast, base);
                let spelled: Vec<&str> = path.segments().iter().map(|part| part.as_str()).collect();
                format!("::{}", spelled.join("::"))
            });
            members.extend(owning_members(
                &name,
                base_text.as_deref(),
                &values,
                &pointers,
            ));
            // DDS-PSM-Cxx 1.0 §7.12: a topic type has `swap` as a member and
            // in its namespace. Not generated for a member that is itself
            // spelled `swap`, which the member function would collide with.
            let swappable = names.iter().all(|(spelled, _)| spelled != "swap");
            if swappable {
                members.push(swap_member(&name, &names));
            }
            let mut declared = class(&name, members, Doc::none());
            if let Some(base) = base {
                declared.bases.push(cpp::Base {
                    access: cpp::Access::Public,
                    virtual_: false,
                    path: cpp_type_path(ast, base),
                });
            }
            out.push((node.module, cpp::Decl::Class(declared)));
            if swappable {
                out.push((node.module, swap_function(&name)));
            }
            out.extend(
                idl_names(&name, &names)?
                    .into_iter()
                    .map(|decl| (node.module, decl)),
            );
        }

        TypeKind::Union { discriminator, .. } => {
            if ghost_created_for(applied, node.node_id, "CppUnion").is_none() {
                return Ok(());
            }
            emitted.push(key);
            let Some(disc) = discriminator.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "union discriminator".to_string(),
                    at: name,
                });
            };
            let disc_type = cpp_value_type(ast, disc, &name)?;
            // The branches and not `_d`: the discriminator is this
            // binding's accessor name, and IDL 4.2 §7.4.1.4.4.1 gives it
            // no member name of its own to record.
            let mut names = Vec::new();
            let mut cases: Vec<UnionCase> = Vec::new();
            for case in ast.cases_of(id) {
                let Ok(case_node) = ast.case(*case) else {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "union case the model listed".to_string(),
                        at: node.name.clone(),
                    });
                };
                if ghost_created_for(applied, case_node.node_id, "CppCase").is_none() {
                    continue;
                }
                let Ok(member) = ast.member(case_node.member) else {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "member behind a union case the model listed".to_string(),
                        at: node.name.clone(),
                    });
                };
                let Some(target) = member.type_.resolved() else {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "union case type".to_string(),
                        at: member.name.clone(),
                    });
                };
                names.push((cxx_name(&member.name), member.name.clone()));
                // The first label selects the case in its setter; one that
                // cannot be evaluated would leave the setter selecting
                // nothing.
                let mut label = None;
                for candidate in &case_node.labels {
                    if let CaseLabel::Value(expr) = candidate {
                        let value = ast.eval_integral(*expr).map_err(|_| {
                            MaterialiseError::Unevaluated {
                                construct: "union case label".to_string(),
                                at: node.name.clone(),
                            }
                        })?;
                        // A value above `long long` is an unsigned constant: C++ gives
                        // the decimal no type of its own and warns, so it takes `ULL`.
                        label = Some(if value > i128::from(i64::MAX) {
                            format!("{value}ULL")
                        } else {
                            zerodds_lang_render::cpp::integer_text(value)
                        });
                        break;
                    }
                }
                let branch = cpp_value_type(ast, target, &member.name)?;
                if super::held_externally(ast, member) {
                    cases.push(UnionCase {
                        name: cxx_name(&member.name),
                        type_: std_type("shared_ptr", vec![branch]),
                        label,
                        doc: external_doc(),
                    });
                    continue;
                }
                cases.push(UnionCase {
                    name: cxx_name(&member.name),
                    type_: branch,
                    label,
                    doc: Doc::none(),
                });
            }
            out.push((
                node.module,
                cpp::Decl::Class(class(
                    &name,
                    union_members(&disc_type, &cases),
                    Doc::lines(&[
                        "A union: the accessor functions of IDL4 to C++ 1.0 (`_d()` and one",
                        "accessor per case) over a discriminator and one storage per branch.",
                        "Not a C++ `union` — a branch may be a std::string or a std::vector,",
                        "which a union member may be only with the lifetime managed by hand —",
                        "and not a std::variant, because two branches may carry the same",
                        "type and a variant could not then say which is live.",
                        "",
                        "Only the branch `_d()` selects is written. The others hold",
                        "whatever they were left holding and cost the sample their",
                        "storage, which is the price of not allocating.",
                    ]),
                )),
            ));
            out.extend(
                idl_names(&name, &names)?
                    .into_iter()
                    .map(|decl| (node.module, decl)),
            );
        }

        TypeKind::Native => {
            if ghost_created_for(applied, node.node_id, "CppNative").is_none() {
                return Ok(());
            }
            emitted.push(key);
            // Opaque, never a guess at what the name might mean: the
            // concrete type comes from the facet a `#pragma` or an
            // annotation set, and a `void *` when there is none.
            let mapping = ast
                .any_of_type(id)
                .ok()
                .and_then(|any| ast.facets(any))
                .and_then(|facets| facets.corba.as_ref())
                .and_then(|corba| corba.native_mapping.clone());
            out.push((
                node.module,
                cpp::Decl::Alias(cpp::Alias {
                    doc: Doc::line("A native type. Opaque unless a facet named its mapping."),
                    name: Ident::new(&name),
                    type_: match mapping {
                        Some(spelling) => cpp::Type::named(Path::single(&spelling)),
                        None => cpp::Type::Pointer(Box::new(cpp::Type::Void)),
                    },
                }),
            ));
        }

        TypeKind::ValueBox { boxed } => {
            let Some(target) = boxed.resolved() else {
                return Ok(());
            };
            emitted.push(key);
            out.push((
                node.module,
                cpp::Decl::Alias(cpp::Alias {
                    doc: Doc::line("A boxed value: a shared handle, empty when null."),
                    name: Ident::new(&name),
                    type_: std_type("shared_ptr", vec![cpp_value_type(ast, target, &name)?]),
                }),
            ));
        }

        // Answered above, before the name was taken.
        TypeKind::Primitive(_)
        | TypeKind::String { .. }
        | TypeKind::Fixed { .. }
        | TypeKind::Sequence { .. }
        | TypeKind::Array { .. }
        | TypeKind::Map { .. }
        | TypeKind::ObjectRef { .. }
        | TypeKind::AnnotationType { .. }
        | TypeKind::Any
        | TypeKind::ValueBase => return Ok(()),
    }
    // The declaration's own IDL name, in one place rather than in every
    // arm above: an arm that emitted nothing has already returned, so
    // reaching here is what "this declaration was written" means.
    if has_idl_name(&node.kind) && !node.name.is_empty() {
        out.push((node.module, idl_name(&name, &node.name)));
    }
    Ok(())
}

/// The underlying type a scoped enumeration is given.
///
/// From the `@bit_bound` **and** from the values, which is not the same
/// question twice. The bound says how wide the holder is on the wire;
/// the values say whether the type can hold them at all, and an
/// enumerator of a fixed-underlying-type enumeration has to be a
/// converted constant expression of that type ([dcl.enum]/5) — a
/// narrowing initialiser is not one, so `= -1` under a `std::uint32_t`
/// underlying type is a header that does not compile.
///
/// So: signed when any enumerator is negative and unsigned otherwise,
/// and wide enough for the larger of the bound and the values. XTypes
/// 1.3 §7.4.3.5.3 makes the wire holder signed, which is why the C
/// column's typedef is an `int32_t`; that is a statement about the
/// stream and the runtime applies it, and it does not oblige a
/// `@bit_bound(32)` enumeration whose largest value is `2^32 - 1` to be
/// declared in a type that cannot hold it.
fn enum_underlying(bound: u16, values: &[(String, i128)]) -> cpp::Type {
    let signed = values.iter().any(|(_, value)| *value < 0);
    let mut bits = bound.max(1);
    for (_, value) in values {
        bits = bits.max(bits_for(*value, signed));
    }
    if signed {
        return match bits {
            0..=8 => std_type("int8_t", Vec::new()),
            9..=16 => std_type("int16_t", Vec::new()),
            17..=32 => std_type("int32_t", Vec::new()),
            _ => std_type("int64_t", Vec::new()),
        };
    }
    cpp_holder(bits)
}

/// How many bits a value needs, in the signedness it will be written in.
fn bits_for(value: i128, signed: bool) -> u16 {
    for bits in [8u16, 16, 32] {
        let fits = if signed {
            let limit = 1i128 << (bits - 1);
            value >= -limit && value < limit
        } else {
            value >= 0 && value < (1i128 << bits)
        };
        if fits {
            return bits;
        }
    }
    64
}

/// Whether the rules created a node of `type_name` for `node`.
///
/// The same two-hop walk the Rust and the C materialiser do: a
/// correspondence is an ordinary node with a link to the anchor and a
/// link to what was created.
fn ghost_created_for(applied: &Applied, node: NodeId, type_name: &str) -> Option<GhostId> {
    let anchor = applied
        .origin
        .iter()
        .find(|(_, model)| **model == node)
        .map(|(ghost, _)| *ghost)?;
    let typ = applied.graph.types.lookup(type_name)?;
    applied
        .graph
        .nodes_of_type(typ)
        .map(|created| created.id)
        .find(|created| {
            applied.graph.connected(&anchor, created)
                || applied
                    .graph
                    .parts(&anchor)
                    .any(|part| applied.graph.connected(&part.other, created))
        })
}

/// The type and the literal a C++ constant takes.
///
/// The type is not always the member type. `cpp_value_type` answers
/// `std::string` for a `string`, a `wstring` and a `fixed`, and C++17
/// has no `constexpr std::string` — the class gained constexpr
/// constructors in C++20, and this project builds at 17. The three
/// therefore become `const char *`, which is `constexpr`, converts to
/// `std::string` at every use, and is the same UTF-8 the member type
/// holds. The alternative was `inline const std::string`, which is a
/// dynamic initialisation at program start for a value the compiler
/// already has.
///
/// `long double` is refused by name: `zerodds::wire::Float128` is a
/// class with no literal, and the compiler's own `long double` is three
/// different formats across the platforms this project builds for.
fn cpp_constant(
    value: &zerodds_ast::ConstValue,
    declared: cpp::Type,
    at: &str,
) -> Result<(cpp::Type, Literal), MaterialiseError> {
    use zerodds_ast::ConstValue;
    let refuse = |construct: &str| MaterialiseError::NoLanguageForm {
        construct: construct.to_string(),
        language: "C++",
        at: at.to_string(),
    };
    /// `const char *`, the type the three string-shaped forms take.
    fn text_type() -> cpp::Type {
        cpp::Type::Pointer(Box::new(cpp::Type::Const(Box::new(cpp::Type::Primitive(
            cpp::Primitive::Char,
        )))))
    }
    Ok(match value {
        ConstValue::Boolean(flag) => (declared, Literal::Bool(*flag)),
        ConstValue::Integer(number) => (declared, int_literal(*number)),
        ConstValue::Char(character) => (declared, Literal::Char(*character)),
        // The code point as a number. `cpp_primitive` gives `wchar` a
        // fixed-width integer for the reason the C column gives — the
        // platform's `wchar_t` is not the wire's code unit — and a
        // character literal would be the execution character set.
        ConstValue::WChar(point) => (declared, int_literal(i128::from(*point))),
        ConstValue::Float { mantissa, exponent } => {
            if matches!(&declared, cpp::Type::Named { path, .. } if path
                .segments()
                .last()
                .is_some_and(|last| last.as_str() == "Float128"))
            {
                return Err(refuse("a `long double` constant, which has no C++ literal"));
            }
            let mut text = constant::float_text(mantissa, exponent.as_deref());
            // Every generated header is compiled under `-Wconversion`,
            // and a `double` initialiser narrowing into a `float` is
            // what it reports.
            if matches!(&declared, cpp::Type::Primitive(cpp::Primitive::Float)) {
                text.push('f');
            }
            (declared, Literal::Float(text))
        }
        ConstValue::Fixed { digits, scale } => (
            text_type(),
            Literal::Str(constant::fixed_decimal(digits, *scale)),
        ),
        ConstValue::String(text) => (text_type(), Literal::Str(text.clone())),
        ConstValue::WString(points) => (
            text_type(),
            Literal::Str(
                constant::wstring_utf8(points)
                    .ok_or_else(|| refuse("a `wstring` constant holding a lone surrogate"))?,
            ),
        ),
        // `ConstValue` is `#[non_exhaustive]`, so a form added to the
        // model reaches here. Refused by name rather than given a
        // plausible default.
        _ => return Err(refuse("a constant form this binding has no literal for")),
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;
    use crate::rule;
    use crate::{ModelGraph, apply};
    use std::path::{Path as FsPath, PathBuf};

    /// Where the interop corpus sits, from this crate's manifest.
    fn corpus() -> PathBuf {
        FsPath::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("tests")
            .join("interop-corpus")
    }

    /// Parse, lower and resolve one IDL file.
    fn model(source: &str, path: &str) -> Result<Ast, String> {
        let parsed = zerodds_idl_parse::parse(source);
        if parsed.has_errors() {
            return Err(format!("parse: {:?}", parsed.diagnostics()));
        }
        let lowered = zerodds_idl_lower::lower(&parsed.syntax(), path);
        if !lowered.diagnostics.is_empty() {
            return Err(format!("lower: {:?}", lowered.diagnostics));
        }
        let mut ast = lowered.ast;
        let report = ast
            .resolve_all_by_name()
            .map_err(|error| error.to_string())?;
        if !report.is_clean() {
            return Err(format!("resolve: {:?}", report.broken));
        }
        Ok(ast)
    }

    /// One IDL file, all the way to rendered C++.
    fn header(source: &str, path: &str) -> Result<String, String> {
        let ast = model(source, path)?;
        let graph = ModelGraph::of(&ast);
        let applied = apply(&graph, &rule::cpp::catalogue()).map_err(|error| error.to_string())?;
        let decls = cpp_items(&ast, &applied).map_err(|error| error.to_string())?;
        Ok(zerodds_lang_render::cpp::file(&cpp::File {
            doc: Doc::line("Generated by zerodds-lang-rules. Do not edit."),
            pragma_once: true,
            includes: includes(),
            decls,
        }))
    }

    /// Every case of the interop corpus, parsed, lowered, applied,
    /// materialised and rendered.
    ///
    /// A case that fails has to fail with a [`MaterialiseError`] this
    /// asserts by name; a case that renders has to render something. The
    /// second half of the gate is not here and cannot be: a header that
    /// renders and does not compile is a materialiser defect, and only a
    /// C++ compiler knows which. Set `ZERODDS_CPP_OUT` to a directory and
    /// this writes every rendered header into it, one `<case>.hpp` per
    /// case, for `c++ -std=c++17 -fsyntax-only -Wall -Wextra -Wpedantic
    /// -Werror -I runtime/cpp/include` to read.
    #[test]
    fn every_corpus_case_renders_or_names_its_refusal() {
        let dump = std::env::var("ZERODDS_CPP_OUT").ok();
        if let Some(directory) = &dump {
            std::fs::create_dir_all(directory).expect("the dump directory");
        }
        let mut cases: Vec<PathBuf> = std::fs::read_dir(corpus())
            .expect("the interop corpus")
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| path.join("type.idl").is_file())
            .collect();
        cases.sort();
        assert!(
            cases.len() > 300,
            "the corpus has {} cases, which is not it",
            cases.len()
        );

        let mut rendered = 0usize;
        let mut refused: Vec<String> = Vec::new();
        for case in &cases {
            let name = case
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("?")
                .to_string();
            let idl = case.join("type.idl");
            let source = std::fs::read_to_string(&idl).expect("the case's IDL");
            match header(&source, &idl.display().to_string()) {
                Ok(text) => {
                    assert!(!text.is_empty(), "{name} rendered nothing");
                    if let Some(directory) = &dump {
                        std::fs::write(FsPath::new(directory).join(format!("{name}.hpp")), &text)
                            .expect("the dumped header");
                    }
                    rendered += 1;
                }
                Err(message) => refused.push(format!("{name}: {message}")),
            }
        }

        assert_eq!(
            rendered + refused.len(),
            cases.len(),
            "every case is either rendered or refused"
        );
        let refused = crate::adapter::without_the_front_end_refusals(refused);
        assert!(
            refused.is_empty(),
            "{} of {} cases were refused:\n{}",
            refused.len(),
            cases.len(),
            refused.join("\n")
        );
    }

    #[test]
    fn an_optional_member_is_a_plain_pointer_and_not_a_second_field() {
        let text = header(
            "@appendable struct S { @optional long a; long b; };",
            "t.idl",
        )
        .expect("renders");
        assert!(text.contains("std::int32_t* a = nullptr;"), "{text}");
        assert!(!text.contains("std::optional"), "{text}");
        assert!(!text.contains("_has_a"), "{text}");
        // The class owns the object: destructor, deep copy, move.
        assert!(text.contains("~S()"), "{text}");
        assert!(text.contains("S(const S& zd_other)"), "{text}");
        assert!(text.contains("zerodds::wire::clone(zd_other.a)"), "{text}");
        assert!(text.contains("zerodds::wire::destroy(a);"), "{text}");
    }

    #[test]
    fn a_module_is_a_namespace_and_not_a_prefix() {
        let text =
            header("module a { module b { struct P { long x; }; }; };", "t.idl").expect("renders");
        assert!(text.contains("namespace a {"), "{text}");
        assert!(text.contains("namespace b {"), "{text}");
        assert!(!text.contains("a_b_P"), "{text}");
    }

    #[test]
    fn a_reference_across_modules_is_written_from_the_root() {
        let text = header(
            "module a { struct P { long x; }; }; module b { struct Q { a::P p; }; };",
            "t.idl",
        )
        .expect("renders");
        assert!(text.contains("::a::P p;"), "{text}");
    }

    #[test]
    fn a_sequence_is_a_vector_and_a_map_is_a_class_over_a_vector_of_pairs() {
        let text = header(
            "struct S { sequence<long> items; map<long, double> m; };",
            "t.idl",
        )
        .expect("renders");
        assert!(text.contains("std::vector<std::int32_t> items;"), "{text}");
        // Not a std::map: the entry order is a policy axis and a
        // std::map would decide it before the encoder ran. The class of
        // XTypes 1.3 §7.5.1.3.3 is the vector of pairs with the operations
        // of Table 47 (runtime/cpp/include/zerodds/map.hpp).
        assert!(
            text.contains("::zerodds::Map<std::int32_t, double> m;"),
            "{text}"
        );
        assert!(!text.contains("std::map<"), "{text}");
    }

    #[test]
    fn a_structure_has_swap_as_a_member_and_in_its_namespace() {
        let text = header("struct S { long a; long b; };", "t.idl").expect("renders");
        // DDS-PSM-Cxx 1.0 7.12: `swap` as a member and next to the type.
        assert!(text.contains("zd_other.a"), "{text}");
        assert!(text.contains("zd_other.b"), "{text}");
        assert!(text.contains("zd_a.swap(zd_b)"), "{text}");
        // A member spelled `swap` would collide with the member function.
        let colliding = header("struct T { long swap; };", "t.idl").expect("renders");
        assert!(!colliding.contains("zd_other"), "{colliding}");
    }

    #[test]
    fn an_array_nests_with_the_outer_dimension_outermost() {
        let text = header("struct S { long a[2][3]; };", "t.idl").expect("renders");
        assert!(
            text.contains("std::array<std::array<std::int32_t, 3>, 2> a = {};"),
            "{text}"
        );
    }

    #[test]
    fn a_union_is_a_class_with_a_discriminator_and_one_field_per_case() {
        let text = header(
            "union U switch (long) { case 1: long a; case 2: long b; };",
            "t.idl",
        )
        .expect("renders");
        assert!(text.contains("struct U {"), "{text}");
        assert!(text.contains("std::int32_t _d() const"), "{text}");
        assert!(text.contains("void _d(std::int32_t value)"), "{text}");
        assert!(text.contains("std::int32_t zd_d;"), "{text}");
        assert!(text.contains("const std::int32_t& a() const"), "{text}");
        assert!(text.contains("std::int32_t& a()"), "{text}");
        assert!(text.contains("void a(const std::int32_t& value)"), "{text}");
        assert!(text.contains("void a(std::int32_t&& value)"), "{text}");
        // The setter of a case selects it.
        assert!(
            text.contains("zd_a = value; zd_d = static_cast<std::int32_t>(1);"),
            "{text}"
        );
        assert!(
            text.contains("zd_b = value; zd_d = static_cast<std::int32_t>(2);"),
            "{text}"
        );
        // Two cases of the same type, which is what rules out a variant.
        assert!(text.contains("std::int32_t zd_a;"), "{text}");
        assert!(text.contains("std::int32_t zd_b;"), "{text}");
        // The prose above the class names both of the shapes that were
        // not chosen, so these look for the declarations they would have
        // produced and not for the words.
        assert!(!text.contains("union U"), "{text}");
        assert!(!text.contains("std::variant<"), "{text}");
    }

    #[test]
    fn an_enumeration_takes_an_underlying_type_from_its_bit_bound() {
        let narrow = header("@bit_bound(8) enum E { A, B };", "t.idl").expect("renders");
        assert!(narrow.contains("enum class E : std::uint8_t {"), "{narrow}");
        let wide = header("enum W { A, B };", "t.idl").expect("renders");
        assert!(wide.contains("enum class W : std::uint32_t {"), "{wide}");
    }

    #[test]
    fn a_long_double_is_the_runtime_type_and_never_the_compiler_s() {
        let text = header("struct S { long double d; double e; };", "t.idl").expect("renders");
        // IDL4 to C++ 1.0 §7.2.4.3.1: built-in members start at their
        // default value, which `= {}` value-initializes.
        assert!(text.contains("::zerodds::wire::Float128 d = {};"), "{text}");
        assert!(text.contains("double e = {};"), "{text}");
        assert!(!text.contains("long double"), "{text}");
    }

    #[test]
    fn the_tag_keeps_apart_what_the_cpp_spelling_collapses() {
        let ast = model(
            "struct S { octet a; uint8 b; double c; long double d; string e; wstring f; };",
            "t.idl",
        )
        .expect("model");
        let mut tags = Vec::new();
        let mut spellings = Vec::new();
        let members = ast
            .types()
            .find_map(|(id, node)| {
                matches!(node.kind, TypeKind::Structure { .. }).then(|| ast.all_members_of(id))
            })
            .expect("the structure");
        for member in members {
            let node = ast.member(member).expect("member");
            let target = node.type_.resolved().expect("resolved");
            tags.push(cpp_type_tag(&ast, target));
            spellings.push(zerodds_lang_render::cpp::type_(
                &cpp_value_type(&ast, target, "t").expect("type"),
            ));
        }
        assert_eq!(tags.len(), 6);
        let mut unique = tags.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), 6, "the tags collapse: {tags:?}");
        // And the C++ spellings do collapse, which is the whole point of
        // the tag existing.
        let mut collapsed = spellings.clone();
        collapsed.sort();
        collapsed.dedup();
        assert!(
            collapsed.len() < 6,
            "the spellings do not collapse, so the tag would be redundant: {spellings:?}"
        );
    }

    /// §7.5.1.1.2, Table 43: "On platforms compliant with the C99
    /// specification, the implementation may generate equivalent
    /// C99-compatible types" — the generated class spells the integers
    /// with the fixed-width types of `<cstdint>`, which are the C99 column
    /// of the table, and no `int` or `long`.
    #[test]
    fn the_integers_are_spelled_with_the_c99_types_of_table_43() {
        let text = header(
            "struct S { int8 a; uint8 b; short c; unsigned short d; long e; unsigned long f; \
             long long g; unsigned long long h; };",
            "t.idl",
        )
        .expect("renders");
        for expected in [
            "std::int8_t a ",
            "std::uint8_t b ",
            "std::int16_t c ",
            "std::uint16_t d ",
            "std::int32_t e ",
            "std::uint32_t f ",
            "std::int64_t g ",
            "std::uint64_t h ",
        ] {
            assert!(text.contains(expected), "{expected}:\n{text}");
        }
    }

    /// §7.5.1.1.2: "All other mappings for basic types shall be
    /// distinguishable for the purposes of overloading" — only Boolean,
    /// Char8 and Byte may coincide. `octet` is the runtime's one-octet class,
    /// `uint8` is `std::uint8_t`, `int8` is `std::int8_t` and `char` is `char`.
    #[test]
    fn the_octet_is_not_the_type_of_uint8_or_int8_or_char() {
        let text = header(
            "struct S { octet a; uint8 b; int8 c; char d; boolean e; };",
            "t.idl",
        )
        .expect("renders");
        assert!(text.contains("::zerodds::wire::Octet a "), "{text}");
        assert!(text.contains("std::uint8_t b "), "{text}");
        assert!(text.contains("std::int8_t c "), "{text}");
        assert!(text.contains("char d "), "{text}");
        assert!(text.contains("bool e "), "{text}");
    }

    #[test]
    fn an_any_member_is_refused_by_name_at_any_depth() {
        let bare = header("struct S { any a; };", "t.idl").expect_err("refused");
        assert!(bare.contains("C++ has no form for `any`"), "{bare}");
        let nested = header("struct S { sequence<any> a; };", "t.idl").expect_err("refused");
        assert!(nested.contains("C++ has no form for `any`"), "{nested}");
    }
}
