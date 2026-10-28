// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Julia CST as the other end of a rule. Spec §3.2.
//!
//! The counterpart of [`crate::adapter::go`], which does the same for Go,
//! and of [`crate::adapter::c`], which does it for C. Reading the three
//! side by side is the point: all of them flatten a module onto the name,
//! and each does it for a different reason.
//!
//! Values are resolved from the model, never from the overlay — the same
//! rule the sibling materialisers follow and for the same reason
//! (spec §2.3).
//!
//! ## The four things Julia decides that Go does not
//!
//! 1. **Declaration order is load-bearing.** A `struct` body's field
//!    types are evaluated when the `struct` is defined, so a name
//!    declared later in the file is an `UndefVarError` at definition
//!    time. Go resolves a name declared anywhere in the package block and
//!    walks the model in declaration order; this column walks the type
//!    graph post-order, as [`crate::adapter::c`] has to for the same
//!    reason.
//!
//!    Measured on Julia 1.12.6, because the two cases people expect to
//!    differ do not: `struct A; b::B; end` before `struct B` and
//!    `struct A; b::Vector{B}; end` before `struct B` both stop with
//!    ``UndefVarError: `B` not defined``. A `Vector{…}` position, a
//!    parametric position and a bare one are all evaluated at the same
//!    moment. What *does* work is a struct naming **itself**:
//!    `struct Node; children::Vector{Node}; end` defines, because the
//!    type's own name is bound before its field types are evaluated.
//!
//!    That leaves the cycles that pass through two or more declarations,
//!    which IDL admits (`recursive-mutual` in the corpus is two structs
//!    each holding a `sequence` of the other). For those the column
//!    declares `abstract type ZdFwd_B end` first, spells the member that
//!    cannot yet name `B` as `ZdFwd_B`, and declares `struct B <: ZdFwd_B`
//!    — the one form in the language that can be named before it is
//!    complete. The widening is real and is confined to the members
//!    inside the cycle: `ZdFwd_B` has exactly one subtype in the
//!    generated file.
//!
//! 2. **A module cannot be reopened.** Julia's `module` nests properly
//!    and would be the natural form for `module common { module util { … } }`.
//!    It is not used, and the reason is not that nesting is hard: a
//!    module body is one contiguous block, and evaluating a second
//!    `module Common … end` in the same file **rebinds the name** — the
//!    first module object, with every declaration in it, is dropped.
//!    Measured on Julia 1.12.6: after two `module Common` blocks,
//!    `isdefined(Common, :A)` is `false`, no warning is printed and the
//!    process exits 0. Since declarations here are ordered by dependency
//!    and a dependency order interleaves modules in general, grouping by
//!    module would sometimes require exactly that second block. The scope
//!    goes into the name instead: `::common::util::Location` is
//!    `common_util_Location`.
//!
//! 3. **A name at module scope may shadow the language.** `const String =
//!    Int32` is accepted with no warning, and every later `name::String`
//!    then declares an `Int32`. A `typedef long String;` would do that,
//!    and the file would load. [`julia::escape_name`] moves every
//!    declared name out of the way of the names the file writes; a
//!    **field** name is not escaped that way, because a field lives in
//!    the struct's own namespace and shadows nothing — measured, and the
//!    reason [`julia::escape_field`] is a second function rather than the
//!    same one.
//!
//! 4. **There is nowhere to put the IDL spelling but a comment.** Julia
//!    has no counterpart of Go's `reflect.StructTag`. A member's ID under
//!    `@autoid(HASH)` and under `@hashid` is the first four octets of the
//!    MD5 of its **IDL** name (DDS-XTypes 1.3 §7.3.1.2.1.1), and
//!    [`julia::escape_field`] does not invert. The IDL name is written in
//!    the comment above the member, and a codec is told it rather than
//!    asked to read it back off the type.
//!
//! ## The memory shapes
//!
//! | IDL | Julia |
//! |---|---|
//! | `boolean` | `Bool` |
//! | `octet`, `uint8`, `char` | `UInt8` |
//! | `int8` | `Int8` |
//! | `short` | `Int16` |
//! | `wchar`, `unsigned short` | `UInt16` |
//! | `long` | `Int32` |
//! | `unsigned long` | `UInt32` |
//! | `long long` | `Int64` |
//! | `unsigned long long` | `UInt64` |
//! | `float` | `Float32` |
//! | `double` | `Float64` |
//! | `long double` | `String`, holding the decimal |
//! | `string` | `String` |
//! | `wstring` | `String`, holding UTF-8 |
//! | `fixed<P,S>` | `String`, holding the decimal |
//! | `sequence<T>` | `Vector{T}` |
//! | `T[N][M]` | `NTuple{N, NTuple{M, T}}`, outer dimension outermost |
//! | `map<K,V>` | `Vector{ZdMapEntry_K_V}`, **not** a `Dict` |
//! | `union` | `struct` with `_d` and one `Union{T, Nothing}` per branch |
//! | `@optional T m` | `Union{T, Nothing}`, `nothing` when absent |
//! | `enum` | `@enum Name::Int32`, every value written out |
//! | `bitmask` | `const Name = UInt16` and one shifted constant per bit |
//! | `typedef` | `const Name = T` |
//! | `native` | `const Name = Any`, or the facet's mapping |
//! | `interface` | `abstract type Name end` and one function stub per operation |
//!
//! ### `Int` is never written
//!
//! `Int` is an alias for `Int64` on a 64-bit platform and `Int32` on a
//! 32-bit one (Julia manual, "Integers and Floating-Point Numbers"). A
//! member declared with it would be a different width on an embedded
//! target than on the host that generated the file, for a wire format
//! that fixes every width. The same goes for a bare integer literal in a
//! `const`: `const LIMIT = 8` binds an `Int64` here and an `Int32` there,
//! which is why every numeric constant is written as a conversion.
//!
//! ### `long double` is a `String` and never a `Float64` or a `BigFloat`
//!
//! Julia has `Float64` and `BigFloat` and no binary128. `Float64` has 53
//! bits of significand where an IDL `long double` has 113, so a member
//! declared `Float64` would lose sixty bits at the assignment, before any
//! codec runs. `BigFloat` is not the answer either: its precision is a
//! run-time global set by `setprecision` and is **not** part of the type
//! — `precision(BigFloat)` is 256 by default, and a value's precision is
//! whatever the global was when it was constructed. A declaration cannot
//! say 113 bits. The member holds the decimal text instead, exactly as
//! the Go and Python columns do.
//!
//! ### `map<K,V>` is a vector of entries and never a `Dict`
//!
//! A `Dict` is a hash table and its iteration order is its internal
//! layout's. A `map<K,V>` is written to the wire as a sequence of
//! key/value pairs (DDS-XTypes 1.3 §7.4.1.1.4), so a column holding it in
//! a `Dict` would produce an octet stream whose order came from the hash
//! of the keys, and the KeyHash of a keyed type would follow it.
//!
//! ### `T[N]` is an `NTuple{N, T}` and the length is in the type
//!
//! `NTuple{1000, Int32}` has a thousand parameters and Julia keeps every
//! one of them: nothing truncates it, and a tuple of any other length is
//! a different type that a field of this type cannot hold. That is the
//! one IDL fact Julia's own type system enforces without a codec being
//! asked, and it is the same argument the Python column makes for
//! `tuple[T, T, T]` over `tuple[T, ...]`. An `Array{T, 2}` would be the
//! other choice, and its length would then be in a comment: an
//! `Array{Int32, 2}` accepts 2×3 and 3×2 and 900×1 alike.
//!
//! ### `union` is a struct and never a Julia `Union`
//!
//! This is the load-bearing decision of the column. Julia's `Union{A, B}`
//! is a real type — a method can dispatch on it, and a field declared
//! with it holds an `A` or a `B` — so it looks like the direct mapping.
//! It is not. An IDL union's discriminator may take a value that selects
//! **no** branch: a `switch (long)` with cases 0 and 1 and no `default`
//! is a legal union, and a sample carrying discriminator 7 is a legal
//! sample of it (IDL 4.2 §7.4.1.4.4.1, and DDS-XTypes 1.3 §7.4.1.1.5.2
//! for what is written). A `Union{A, B}` value is an `A` or a `B` and has
//! nowhere to record 7. The column writes a struct with the
//! discriminator in `_d` and one `Union{T, Nothing}` field per branch;
//! all of them are `nothing` when the discriminator selects none, which
//! is a state the value can hold and report.
//!
//! `_d` is a legal Julia field name and is spelled the same here as in
//! every other column.
//!
//! ### `interface` is an abstract type and a set of function stubs
//!
//! Julia has no interface construct: there is no declaration that says
//! "a subtype must provide these methods", and the language's own answer
//! is an abstract type plus a documented method contract (the manual's
//! interfaces chapter states the contracts for `iterate`, `getindex` and
//! the rest in prose, for exactly that reason). The column emits
//! `abstract type Control end` and `function go end` for each operation
//! — a generic function with no methods, which binds the name and adds
//! nothing — with the signature in the comment above it, because a
//! function declared this way has no argument list to put it in.

use crate::adapter::constant;
use crate::adapter::cst::{MaterialiseError, no_spelling_collision};
use crate::adapter::interface;
use crate::apply::Applied;
use crate::verbatim;
use seesaw_tgg::ident::GhostId;
use zerodds_ast::{Ast, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::{Id, NodeId};
use zerodds_lang_cst::common::{Doc, Ident, Literal};
use zerodds_lang_cst::julia;

/// The language this column names in a diagnostic.
const LANGUAGE: &str = "Julia";

/// The Julia field name of the union discriminator.
///
/// `_d`, as in every other column. Julia accepts a leading underscore in
/// a field name, so unlike the Go column there is nothing to move here.
pub const DISCRIMINATOR: &str = "_d";

/// The prefix a name this column invented carries.
const INVENTED_PREFIX: &str = "ZdMapEntry_";

/// The prefix of the abstract type that stands in for a declaration a
/// member has to name before it exists.
///
/// Only a type graph with a cycle through two or more declarations
/// produces one; an acyclic model generates none at all.
const FORWARD_PREFIX: &str = "ZdFwd_";

/// How far the walks in this module follow a chain before they stop.
///
/// The same bound the layout planner uses. A model whose parent chain or
/// type graph does not terminate is a defect, and not a reason to hang.
const MAX_DEPTH: usize = 64;

/// Build the Julia declarations a rule application produced.
///
/// The order is the type graph's, dependencies first, because a field's
/// type expression is evaluated when the `struct` is defined.
///
/// # Errors
/// [`MaterialiseError`] when a value the output has to state does not
/// evaluate, a type has no Julia shape, or two declarations reach one
/// Julia spelling — never a silently dropped declaration.
#[allow(clippy::too_many_lines)]
pub fn julia_items(ast: &Ast, applied: &Applied) -> Result<Vec<julia::Decl>, MaterialiseError> {
    let mut out = Vec::new();
    let mut emitted: Vec<(String, String)> = Vec::new();
    let mut ctx = Ctx::over(ordered_types(ast));

    for index in 0..ctx.order.len() {
        let Some(id) = ctx.order.get(index).copied() else {
            continue;
        };
        ctx.at = Some(index);
        let start = out.len();
        declare(ast, applied, id, &mut out, &mut emitted, &mut ctx)?;
        verbatim::weave(
            verbatim::declaration_slots(ast, id, verbatim::aliases::JULIA),
            &julia_type_name(ast, id),
            start,
            &mut out,
        );
    }
    ctx.at = None;

    // Constants, exceptions and interfaces are not types, so they are
    // walked separately — the model keeps them in arenas of their own,
    // and every sibling materialiser does the same. None of them can be
    // named by a type, so none of them can be forward-referenced: they
    // come after every declaration that could.
    for (_, node) in ast.constants() {
        if ghost_created_for(applied, node.node_id, "JuliaConst").is_none() {
            continue;
        }
        // `eval_const` and not `eval_integral`: `const string NAME` is a
        // constant that evaluates perfectly well and is not a number,
        // and the integral question is simply the wrong one for four of
        // the eight forms IDL 4.2 admits.
        let value = ast
            .eval_const(node.expr)
            .map_err(|_| MaterialiseError::Unevaluated {
                construct: "constant's value".to_string(),
                at: node.name.clone(),
            })?;
        let declared = node.type_.resolved();
        let type_ = match declared {
            Some(target) => value_type(ast, target, &mut ctx)?,
            None => julia::Type::Primitive(julia::Primitive::Int32),
        };
        let name = declared_name(ast, &node.name, node.module);
        remember(&mut emitted, &name, &node.name)?;
        out.push(julia::Decl::Const(julia::ConstDecl {
            doc: idl_note(&name, &node.name),
            name: Ident::new(&name),
            value: julia_constant(&value, &type_, &node.name)?,
        }));
    }

    for (_, node) in ast.exceptions() {
        if ghost_created_for(applied, node.node_id, "JuliaError").is_none() {
            continue;
        }
        let name = declared_name(ast, &node.name, node.module);
        remember(&mut emitted, &name, &node.name)?;
        let mut pairs = Vec::new();
        for member in &node.members {
            if let Ok(member_node) = ast.member(*member) {
                pairs.push((
                    julia::escape_field(&member_node.name),
                    member_node.name.clone(),
                ));
            }
        }
        no_spelling_collision(as_pairs(&pairs), LANGUAGE, &name)?;
        let mut fields = Vec::new();
        for member in &node.members {
            let Ok(member_node) = ast.member(*member) else {
                continue;
            };
            let Some(target) = member_node.type_.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "exception member type".to_string(),
                    at: member_node.name.clone(),
                });
            };
            let spelled = julia::escape_field(&member_node.name);
            fields.push(julia::Field {
                doc: idl_note(&spelled, &member_node.name),
                name: Ident::new(&spelled),
                type_: member_type(ast, target, &member_node.name, &mut ctx)?,
            });
        }
        out.push(julia::Decl::Struct(julia::StructDecl {
            body_text: zerodds_lang_cst::common::BodyText::none(),
            doc: with_idl_note(
                Doc::line("An exception, as the structure its members make."),
                &name,
                &node.name,
            ),
            name: Ident::new(&name),
            supertype: None,
            fields,
        }));
    }

    let mut functions: Vec<String> = Vec::new();
    for (id, node) in ast.interfaces() {
        if ghost_created_for(applied, node.node_id, "JuliaInterface").is_none() {
            continue;
        }
        let name = declared_name(ast, &node.name, node.module);
        remember(&mut emitted, &name, &node.name)?;
        out.push(julia::Decl::Abstract(julia::AbstractDecl {
            doc: with_idl_note(
                Doc::lines(&[
                    "An interface. Julia has no interface construct, so this is an",
                    "abstract type and a set of functions declared before anything",
                    "implements them, one per operation and one (or, `readonly`",
                    "aside, two) per attribute's implied get_/set_ pair; the contract",
                    "is the comment above each one, including a `raises`/",
                    "`getraises`/`setraises` clause where the operation has one. The",
                    "transport is the RPC arc's business; the shape is the IDL's.",
                ]),
                &name,
                &node.name,
            ),
            name: Ident::new(&name),
        }));
        let mut stubs = Vec::new();
        // The whole answer, flattened — but not in the sense the other
        // eleven columns need it in. What an operation or an
        // attribute's implied get_/set_ pair becomes here is
        // `function <name> end` — a generic function at file scope,
        // bound to no type at all — and the loop below skips a
        // spelling already declared. A base's own member is therefore
        // already in the file from the base's own declaration, and
        // `interface::all_operations_flattened` walking it again for
        // the derived interface produces the same file byte for byte
        // (the dedup check, not the flattening, is what makes that
        // true). Nothing is dropped because there is no per-type
        // method list to drop it from: the abstract type carries no
        // operations, so OMG IDL 4.2 §7.4.3's inheritance has nothing
        // here to be lost out of.
        let operations = interface::all_operations_flattened(ast, id)
            .map_err(|error| interface::interface_error_to_materialise(error, &node.name))?;
        for op in &operations {
            let spelled = julia::escape_name(&op.name);
            let mut signature = spelled.clone();
            signature.push('(');
            let mut params = Vec::new();
            for param in &op.params {
                let type_ = member_type(ast, param.type_, &param.name, &mut ctx)?;
                params.push(format!(
                    "{}::{}",
                    julia::escape_name(&param.name),
                    zerodds_lang_render::julia::type_(&type_)
                ));
            }
            signature.push_str(&params.join(", "));
            signature.push(')');
            // `Nothing` and not an omitted result: an operation returning
            // `void` returns Julia's one-valued type, and saying so in
            // the contract is what stops a reader assuming `Any`.
            let returns = match op.returns {
                Some(target) => {
                    let type_ = member_type(ast, target, &op.name, &mut ctx)?;
                    zerodds_lang_render::julia::type_(&type_)
                }
                None => julia::Primitive::Nothing.as_str().to_string(),
            };
            signature.push_str("::");
            signature.push_str(&returns);
            let mut doc_line = signature.clone();
            if let Some(raises) = op.raises_line() {
                doc_line.push_str(" -- ");
                doc_line.push_str(&raises);
            }
            stubs.push((spelled, op.name.clone(), doc_line));
        }
        let signatures: Vec<(String, String)> = stubs
            .iter()
            .map(|(spelled, declared, _)| (spelled.clone(), declared.clone()))
            .collect();
        no_spelling_collision(as_pairs(&signatures), LANGUAGE, &name)?;
        for (spelled, declared, signature) in stubs {
            // A generic function's name is shared: two interfaces that
            // both declare `void start()` declare one function with no
            // methods, and writing `function start end` twice is the same
            // declaration twice rather than a collision. A *type* of that
            // name is a collision, and is reported.
            if let Some((_, first)) = emitted.iter().find(|(other, _)| *other == spelled) {
                return Err(MaterialiseError::SpellingCollision {
                    first: first.clone(),
                    second: declared,
                    spelled,
                    language: LANGUAGE,
                    at: "the generated file".to_string(),
                });
            }
            if functions.contains(&spelled) {
                continue;
            }
            functions.push(spelled.clone());
            let mut doc = Doc::line(&signature);
            if spelled != declared {
                doc.push(&format!("IDL: `{declared}`."));
            }
            out.push(julia::Decl::Function(julia::FunctionDecl {
                doc,
                name: Ident::new(&spelled),
            }));
        }
    }

    resolve_forwards(&mut out, &ctx.forward)?;
    Ok(verbatim::weave_file(ast, verbatim::aliases::JULIA, out))
}

/// `@verbatim` in the Julia column: `BEGIN_DECLARATION` and
/// `END_DECLARATION` open and close a `struct … end` body; an `@enum`, a
/// constant and an abstract type take them adjacent.
impl verbatim::Carrier for julia::Decl {
    fn verbatim(text: String) -> Self {
        Self::Verbatim(text)
    }

    fn take_into_body(
        &mut self,
        name: &str,
        begin: &mut Vec<String>,
        end: &mut Vec<String>,
    ) -> bool {
        let Self::Struct(node) = self else {
            return false;
        };
        if node.name.as_str() != name {
            return false;
        }
        node.body_text.begin.append(begin);
        node.body_text.end.append(end);
        true
    }
}

/// The Julia declarations, in a file with its own comment.
///
/// A caller that only wants the declarations uses [`julia_items`]; this
/// is what a generator writes to disk.
///
/// # Errors
/// As [`julia_items`].
pub fn julia_file(ast: &Ast, applied: &Applied) -> Result<julia::File, MaterialiseError> {
    Ok(julia::File {
        doc: Doc::lines(&[
            "Generated by ZeroDDS from an IDL source. Do not edit.",
            "",
            "Declarations are in dependency order, not source order: a struct's",
            "field types are evaluated when the struct is defined, so a name",
            "declared later in the file is an UndefVarError.",
            "",
            "The IDL spelling of every renamed declaration and every renamed",
            "member is in the comment above it. Julia has no field annotation a",
            "program can read back, and a member's ID under @autoid(HASH) is the",
            "MD5 of the IDL name (DDS-XTypes 1.3 7.3.1.2.1.1), so the codec is",
            "told that name rather than asked to recover it from the type.",
        ]),
        decls: julia_items(ast, applied)?,
    })
}

// -- ordering ------------------------------------------------------------

/// Every type in the model, dependencies first.
///
/// Post-order over the type graph. A type already visited is not
/// revisited, which also means a cycle stops rather than recurses and
/// leaves the edges that close it pointing forwards. Those are the ones
/// `resolve_forwards` turns into an `abstract type`; every other edge
/// points at something already declared.
///
/// This walk is the Julia column's own rather than the C column's,
/// although the two produce the same order today: the positions it
/// returns are what decides which members are spelled with a forward
/// name, so a change to the C column's notion of a dependency must not
/// silently change which Julia declarations get a supertype.
#[must_use]
pub fn ordered_types(ast: &Ast) -> Vec<Id<TypeNode>> {
    let mut order = Vec::new();
    let mut seen = Vec::new();
    for (id, _) in ast.types() {
        visit(ast, id, &mut order, &mut seen, 0);
    }
    order
}

/// zerodds-lint: recursion-depth 64 (dependency edges of the type
/// graph; bounded by `MAX_DEPTH` and by `seen`, which admits each
/// type once, so a cycle stops on the second visit)
fn visit(
    ast: &Ast,
    id: Id<TypeNode>,
    order: &mut Vec<Id<TypeNode>>,
    seen: &mut Vec<Id<TypeNode>>,
    depth: usize,
) {
    if depth > MAX_DEPTH || seen.contains(&id) {
        return;
    }
    seen.push(id);
    for dependency in dependencies(ast, id) {
        visit(ast, dependency, order, seen, depth + 1);
    }
    order.push(id);
}

/// The types a type's Julia declaration has to be preceded by.
fn dependencies(ast: &Ast, id: Id<TypeNode>) -> Vec<Id<TypeNode>> {
    let mut out = Vec::new();
    let Ok(node) = ast.type_(id) else {
        return out;
    };
    match &node.kind {
        TypeKind::Sequence { element, .. } | TypeKind::Array { element, .. } => {
            out.extend(element.resolved());
        }
        TypeKind::Map { key, value, .. } => {
            out.extend(key.resolved());
            out.extend(value.resolved());
        }
        TypeKind::Alias { aliased } => out.extend(aliased.resolved()),
        TypeKind::ValueBox { boxed } => out.extend(boxed.resolved()),
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            // The base is held as the member `parent`, so it is a dependency of its own.
            out.extend(super::c::struct_base(ast, id));
            for member in ast.all_members_of(id) {
                if let Ok(member_node) = ast.member(member) {
                    out.extend(member_node.type_.resolved());
                }
            }
        }
        TypeKind::Union { discriminator, .. } => {
            out.extend(discriminator.resolved());
            for case in ast.cases_of(id) {
                if let Ok(case_node) = ast.case(*case) {
                    if let Ok(member) = ast.member(case_node.member) {
                        out.extend(member.type_.resolved());
                    }
                }
            }
        }
        TypeKind::Primitive(_)
        | TypeKind::String { .. }
        | TypeKind::Fixed { .. }
        | TypeKind::Enumeration { .. }
        | TypeKind::Bitmask { .. }
        | TypeKind::Bitset { .. }
        | TypeKind::Native
        | TypeKind::Any
        | TypeKind::ValueBase
        | TypeKind::ObjectRef { .. }
        | TypeKind::AnnotationType { .. } => {}
    }
    out
}

/// Where the walk is, and what it has had to name before it existed.
struct Ctx {
    /// Every type, dependencies first.
    order: Vec<Id<TypeNode>>,
    /// The position of the declaration being written, when one is.
    at: Option<usize>,
    /// The declared names a member had to name before they existed, in
    /// the order they were first needed.
    forward: Vec<String>,
    /// How deep the current type expression is.
    depth: usize,
}

impl Ctx {
    fn over(order: Vec<Id<TypeNode>>) -> Self {
        Self {
            order,
            at: None,
            forward: Vec::new(),
            depth: 0,
        }
    }

    /// A context with no order at all, for a caller that wants the plain
    /// spelling of a type and is not writing a file.
    fn unordered() -> Self {
        Self::over(Vec::new())
    }

    /// Whether `id` is declared after the declaration being written.
    fn is_forward(&self, id: Id<TypeNode>) -> bool {
        let Some(at) = self.at else { return false };
        self.order
            .iter()
            .position(|other| *other == id)
            .is_some_and(|pos| pos > at)
    }

    fn record_forward(&mut self, name: &str) {
        if !self.forward.iter().any(|other| other == name) {
            self.forward.push(name.to_string());
        }
    }
}

/// Give every forward-referenced struct the abstract supertype its
/// members named, and declare that supertype first.
///
/// # Errors
/// [`MaterialiseError::NoLanguageForm`] when the cycle closes through a
/// declaration that cannot carry a supertype. Only a `struct` can, so an
/// `@enum` or a `const` in a cycle would be one — neither can occur,
/// because neither has members to close a cycle with, and the check is
/// here rather than assumed.
fn resolve_forwards(
    out: &mut Vec<julia::Decl>,
    forward: &[String],
) -> Result<(), MaterialiseError> {
    if forward.is_empty() {
        return Ok(());
    }
    let mut heads = Vec::new();
    for name in forward {
        let abstract_name = format!("{FORWARD_PREFIX}{name}");
        let mut found = false;
        for decl in out.iter_mut() {
            if let julia::Decl::Struct(structure) = decl {
                if structure.name.as_str() == name {
                    structure.supertype = Some(Ident::new(&abstract_name));
                    found = true;
                    break;
                }
            }
        }
        if !found {
            return Err(MaterialiseError::NoLanguageForm {
                construct: "a type graph cycle closing through a declaration that is not a \
                            struct, which Julia cannot forward-declare"
                    .to_string(),
                language: LANGUAGE,
                at: name.clone(),
            });
        }
        heads.push(julia::Decl::Abstract(julia::AbstractDecl {
            doc: Doc::lines(&[
                "Stands in for the declaration below that a member above has to name.",
                "A struct's field types are evaluated when the struct is defined, so a",
                "cycle through two declarations has no order; an abstract type has no",
                "fields and therefore no forward reference of its own.",
            ]),
            name: Ident::new(&abstract_name),
        }));
    }
    heads.append(out);
    *out = heads;
    Ok(())
}

// -- declarations --------------------------------------------------------

/// One declared type.
#[allow(clippy::too_many_lines)]
fn declare(
    ast: &Ast,
    applied: &Applied,
    id: Id<TypeNode>,
    out: &mut Vec<julia::Decl>,
    emitted: &mut Vec<(String, String)>,
    ctx: &mut Ctx,
) -> Result<(), MaterialiseError> {
    let Ok(node) = ast.type_(id) else {
        return Ok(());
    };

    match &node.kind {
        // A primitive, a string, a fixed, a sequence and an array are all
        // written where the member is: Julia spells each of them inline,
        // so none of them is a declaration.
        TypeKind::Primitive(_)
        | TypeKind::String { .. }
        | TypeKind::Fixed { .. }
        | TypeKind::Sequence { .. }
        | TypeKind::Array { .. } => {}

        // The one collection that needs a name of its own. A `Dict` is
        // refused for the reason in the module documentation, and a pair
        // written as an `NTuple{2, …}` cannot hold a key and a value of
        // different types with names on them.
        TypeKind::Map { key, value, .. } => {
            let (Some(key), Some(value)) = (key.resolved(), value.resolved()) else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "map key or value".to_string(),
                    at: julia_type_name(ast, id),
                });
            };
            let name = julia_map_entry_name(ast, id);
            if emitted.iter().any(|(spelled, _)| *spelled == name) {
                return Ok(());
            }
            emitted.push((name.clone(), name.clone()));
            out.push(julia::Decl::Struct(julia::StructDecl {
                body_text: zerodds_lang_cst::common::BodyText::none(),
                doc: Doc::line(
                    "One entry of a map. DDS-XTypes 1.3 7.4.1.1.4 defines a map as a \
                     sequence of these, and the rules write the two halves with no \
                     framing between them.",
                ),
                name: Ident::new(&name),
                supertype: None,
                fields: vec![
                    julia::Field {
                        doc: Doc::none(),
                        name: Ident::new("key"),
                        type_: member_type(ast, key, &name, ctx)?,
                    },
                    julia::Field {
                        doc: Doc::none(),
                        name: Ident::new("value"),
                        type_: member_type(ast, value, &name, ctx)?,
                    },
                ],
            }));
        }

        TypeKind::Alias { aliased } => {
            // A typedef with no name is one the frontend made for an
            // inline type; it is transparent and declares nothing.
            if node.name.is_empty() {
                return Ok(());
            }
            if ghost_created_for(applied, node.node_id, "JuliaAlias").is_none() {
                return Ok(());
            }
            let Some(target) = aliased.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "typedef target".to_string(),
                    at: node.name.clone(),
                });
            };
            let name = julia_type_name(ast, id);
            remember(emitted, &name, &node.name)?;
            // `const Name = T` is a second name for one type and not a
            // new one, which is what IDL 4.2 §7.4.1.4.3 says an alias is:
            // it "does not define a new type". Julia has no way to make a
            // new one from an old one short of wrapping it in a struct,
            // so the alias needs no argument against a `struct Distance`
            // — but the binding must be `const`, because a non-`const`
            // global holding a type makes every annotation using it a
            // run-time lookup.
            out.push(julia::Decl::Const(julia::ConstDecl {
                doc: idl_note(&name, &node.name),
                name: Ident::new(&name),
                value: julia::Expr::Type(Box::new(member_type(ast, target, &node.name, ctx)?)),
            }));
        }

        TypeKind::Enumeration { enumerators } => {
            if ghost_created_for(applied, node.node_id, "JuliaEnum").is_none() {
                return Ok(());
            }
            let name = julia_type_name(ast, id);
            remember(emitted, &name, &node.name)?;
            let prefix = module_prefix(ast, id);
            let mut members = Vec::new();
            let mut values: Vec<(i128, String)> = Vec::new();
            for enumerator in enumerators {
                let Ok(enum_node) = ast.enumerator(*enumerator) else {
                    continue;
                };
                if ghost_created_for(applied, enum_node.node_id, "JuliaEnumerator").is_none() {
                    continue;
                }
                let value = ast.effective_enumerator_value(*enumerator).ok_or_else(|| {
                    MaterialiseError::Unevaluated {
                        construct: "enumerator value".to_string(),
                        at: enum_node.name.clone(),
                    }
                })?;
                // `@enum` refuses two members with one value — measured:
                // "both B and A have value 1 in Enum Dup; values must be
                // unique" — and refuses at load time, from a file that
                // parsed. Reported here by both IDL names instead.
                if let Some((_, first)) = values.iter().find(|(other, _)| *other == value) {
                    return Err(MaterialiseError::NoLanguageForm {
                        construct: format!(
                            "two enumerators sharing the value {value}, `{first}` and \
                             `{}`, which @enum refuses at load time",
                            enum_node.name
                        ),
                        language: LANGUAGE,
                        at: node.name.clone(),
                    });
                }
                values.push((value, enum_node.name.clone()));
                let constant = format!("{prefix}{}", julia::escape_name(&enum_node.name));
                remember(emitted, &constant, &enum_node.name)?;
                members.push(julia::Enumerator {
                    doc: idl_note(&constant, &enum_node.name),
                    name: Ident::new(&constant),
                    value: julia::Expr::int(value),
                });
            }
            // `@enum` with no members raises when the file is loaded
            // ("reducing over an empty collection is not allowed"), so an
            // enumeration that reached here with none is refused rather
            // than written.
            if members.is_empty() {
                return Err(MaterialiseError::NoLanguageForm {
                    construct: "an enumeration with no enumerators, which @enum refuses at \
                                load time"
                        .to_string(),
                    language: LANGUAGE,
                    at: node.name.clone(),
                });
            }
            out.push(julia::Decl::Enum(julia::EnumDecl {
                doc: with_idl_note(
                    Doc::lines(&[
                        "An enumeration. The holder is signed because DDS-XTypes 1.3",
                        "7.4.3.5.3 makes it so; the width on the wire is the",
                        "@bit_bound's and is applied by the runtime. Every value is",
                        "written out because @value admits gaps and @enum would",
                        "otherwise number from zero.",
                    ]),
                    &name,
                    &node.name,
                ),
                name: Ident::new(&name),
                base: julia::Primitive::Int32,
                members,
            }));
        }

        TypeKind::Bitmask { bits } => {
            if ghost_created_for(applied, node.node_id, "JuliaBitmask").is_none() {
                return Ok(());
            }
            let name = julia_type_name(ast, id);
            remember(emitted, &name, &node.name)?;
            let bound = ast.effective_bit_bound(id);
            // A `const` alias to an unsigned integer and never an
            // `@enum`: an `@enum` value is one member, and the whole
            // point of a bitmask is holding several at once.
            out.push(julia::Decl::Const(julia::ConstDecl {
                doc: with_idl_note(
                    Doc::lines(&[
                        "A bitmask, as the unsigned integer Table 33 of DDS-XTypes 1.3",
                        "7.4.1.1.3.2 gives for its bound. Not an @enum: an @enum value",
                        "is one member and a bitmask holds several at once.",
                    ]),
                    &name,
                    &node.name,
                ),
                name: Ident::new(&name),
                value: julia::Expr::Type(Box::new(julia::Type::Primitive(julia_holder(bound)))),
            }));
            let prefix = module_prefix(ast, id);
            for bit in bits {
                let Ok(bit_node) = ast.bit_value(*bit) else {
                    continue;
                };
                if ghost_created_for(applied, bit_node.node_id, "JuliaBitConst").is_none() {
                    continue;
                }
                let position = ast.effective_bit_position(*bit).ok_or_else(|| {
                    MaterialiseError::Unevaluated {
                        construct: "bit position".to_string(),
                        at: bit_node.name.clone(),
                    }
                })?;
                let constant = format!("{prefix}{}", julia::escape_name(&bit_node.name));
                remember(emitted, &constant, &bit_node.name)?;
                // The one is converted before it is shifted, so the
                // constant has the bitmask's width. A bare `1 << 40` is
                // an `Int64` here and overflows to zero on a 32-bit
                // platform; `UInt64(1) << 40` is the same value on both.
                out.push(julia::Decl::Const(julia::ConstDecl {
                    doc: idl_note(&constant, &bit_node.name),
                    name: Ident::new(&constant),
                    value: julia::Expr::Binary {
                        op: "<<",
                        lhs: Box::new(julia::Expr::convert(
                            julia::Type::named(&name),
                            julia::Expr::int(1),
                        )),
                        rhs: Box::new(julia::Expr::int(i128::from(position))),
                    },
                }));
            }
        }

        TypeKind::Bitset { .. } => {
            if ghost_created_for(applied, node.node_id, "JuliaBitset").is_none() {
                return Ok(());
            }
            let name = julia_type_name(ast, id);
            remember(emitted, &name, &node.name)?;
            let mut pairs = Vec::new();
            // The base's bitfields are members of the derived struct too:
            // an inherited bitset is one flat run.
            for field in &ast.all_bitfields_of(id) {
                let Ok(field_node) = ast.bitfield(*field) else {
                    continue;
                };
                // An anonymous field is padding: it moves the next field
                // along and carries no value of its own.
                let Some(field_ident) = field_node.name.as_ref() else {
                    continue;
                };
                pairs.push((julia::escape_field(field_ident), field_ident.clone()));
            }
            no_spelling_collision(as_pairs(&pairs), LANGUAGE, &name)?;
            let mut fields = Vec::new();
            let mut index = 0;
            for field in &ast.all_bitfields_of(id) {
                let Ok(field_node) = ast.bitfield(*field) else {
                    continue;
                };
                let Some(field_ident) = field_node.name.as_ref() else {
                    continue;
                };
                fields.push(julia::Field {
                    doc: idl_note(&pairs[index].0, field_ident),
                    name: Ident::new(&pairs[index].0),
                    type_: julia::Type::Primitive(julia_holder(field_node.bits)),
                });
                index += 1;
            }
            out.push(julia::Decl::Struct(julia::StructDecl {
                body_text: zerodds_lang_cst::common::BodyText::none(),
                doc: with_idl_note(
                    Doc::line(
                        "A bitset. The fields are separate integers here; the packing into \
                         the anchor is the codec's, because the order on the wire is the \
                         declaration's.",
                    ),
                    &name,
                    &node.name,
                ),
                name: Ident::new(&name),
                supertype: None,
                fields,
            }));
        }

        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            let created = if matches!(node.kind, TypeKind::Valuetype { .. }) {
                "JuliaValue"
            } else {
                "JuliaStruct"
            };
            if ghost_created_for(applied, node.node_id, created).is_none() {
                return Ok(());
            }
            let name = julia_type_name(ast, id);
            remember(emitted, &name, &node.name)?;
            // XTypes 1.3 §7.5.1.4.1.3: the base is the first member `parent`.
            let members = ast.members_of(id).to_vec();
            let base = super::c::struct_base(ast, id);
            let mut pairs = Vec::new();
            if base.is_some() {
                pairs.push((julia::escape_field("parent"), "parent".to_string()));
            }
            for member in &members {
                if let Ok(member_node) = ast.member(*member) {
                    pairs.push((
                        julia::escape_field(&member_node.name),
                        member_node.name.clone(),
                    ));
                }
            }
            no_spelling_collision(as_pairs(&pairs), LANGUAGE, &name)?;
            let mut fields = Vec::new();
            if let Some(base) = base {
                fields.push(julia::Field {
                    doc: Doc::line(
                        "The base structure, the first member (XTypes 1.3 7.5.1.4.1.3).",
                    ),
                    name: Ident::new(&julia::escape_field("parent")),
                    type_: member_type(ast, base, "parent", ctx)?,
                });
            }
            for member in &members {
                let Ok(member_node) = ast.member(*member) else {
                    continue;
                };
                let Some(target) = member_node.type_.resolved() else {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "member type".to_string(),
                        at: member_node.name.clone(),
                    });
                };
                let spelled = julia::escape_field(&member_node.name);
                let mut type_ = member_type(ast, target, &member_node.name, ctx)?;
                let mut doc = idl_note(&spelled, &member_node.name);
                if member_node.facets.is_optional() {
                    // `Union{T, Nothing}` and not a value beside a flag:
                    // an all-bits-zero `Int32` and a member that did not
                    // arrive would otherwise be the same value.
                    // `Nothing` has exactly one value, `nothing`, and no
                    // `Int32` is one — and the union is a real type a
                    // method can dispatch on, so a decoder that handles
                    // the two cases does it with two methods rather than
                    // a branch. DDS-XTypes 1.3 §7.2.2.4.4.4.7.
                    type_ = julia::Type::optional(type_);
                    doc.push(
                        "Optional. `nothing` when the member is absent — DDS-XTypes 1.3 \
                         7.2.2.4.4.4.7. In a @mutable type an absent member is not written \
                         at all; outside one its absence is written.",
                    );
                }
                fields.push(julia::Field {
                    doc,
                    name: Ident::new(&spelled),
                    type_,
                });
            }
            out.push(julia::Decl::Struct(julia::StructDecl {
                body_text: zerodds_lang_cst::common::BodyText::none(),
                doc: with_idl_note(
                    Doc::line(
                        "Immutable, which is Julia's default `struct`: a decoded sample is \
                         a value and not a handle a second reference can change underneath \
                         the first.",
                    ),
                    &name,
                    &node.name,
                ),
                name: Ident::new(&name),
                supertype: None,
                fields,
            }));
        }

        TypeKind::Union { discriminator, .. } => {
            if ghost_created_for(applied, node.node_id, "JuliaUnion").is_none() {
                return Ok(());
            }
            let name = julia_type_name(ast, id);
            remember(emitted, &name, &node.name)?;
            let Some(disc) = discriminator.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "union discriminator".to_string(),
                    at: name,
                });
            };
            let cases = ast.cases_of(id);
            // The discriminator's field is in the collision check with
            // the branches: IDL 4.2 §7.4.1.4.4.1 gives it no member name
            // of its own, so `_d` is this column's, and a branch called
            // `_d` would take the same Julia spelling and make one of the
            // two unreachable.
            let mut pairs = vec![(DISCRIMINATOR.to_string(), "<discriminator>".to_string())];
            for case in cases {
                if let Ok(case_node) = ast.case(*case) {
                    if let Ok(member) = ast.member(case_node.member) {
                        pairs.push((julia::escape_field(&member.name), member.name.clone()));
                    }
                }
            }
            no_spelling_collision(as_pairs(&pairs), LANGUAGE, &name)?;
            let mut fields = vec![julia::Field {
                doc: Doc::line("Which branch carries the value, if any does."),
                name: Ident::new(DISCRIMINATOR),
                type_: member_type(ast, disc, &name, ctx)?,
            }];
            for case in cases {
                let Ok(case_node) = ast.case(*case) else {
                    continue;
                };
                if ghost_created_for(applied, case_node.node_id, "JuliaCase").is_none() {
                    continue;
                }
                let Ok(member) = ast.member(case_node.member) else {
                    continue;
                };
                let Some(target) = member.type_.resolved() else {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "union case type".to_string(),
                        at: member.name.clone(),
                    });
                };
                let spelled = julia::escape_field(&member.name);
                fields.push(julia::Field {
                    doc: idl_note(&spelled, &member.name),
                    name: Ident::new(&spelled),
                    // Every branch is `Union{T, Nothing}` because at most
                    // one of them carries a value at a time, and because
                    // a discriminator selecting no branch leaves all of
                    // them absent.
                    type_: julia::Type::optional(member_type(ast, target, &member.name, ctx)?),
                });
            }
            out.push(julia::Decl::Struct(julia::StructDecl {
                body_text: zerodds_lang_cst::common::BodyText::none(),
                doc: with_idl_note(
                    Doc::lines(&[
                        "A union. Not a Julia `Union{…}`: an IDL discriminator may take a",
                        "value that selects no branch (IDL 4.2 7.4.1.4.4.1), and a",
                        "`Union{A, B}` value is an A or a B with nowhere to record that.",
                        "`_d` says which branch carries the value; only that branch is",
                        "written to the wire (DDS-XTypes 1.3 7.4.1.1.5.2).",
                    ]),
                    &name,
                    &node.name,
                ),
                name: Ident::new(&name),
                supertype: None,
                fields,
            }));
        }

        TypeKind::Native => {
            if ghost_created_for(applied, node.node_id, "JuliaNative").is_none() {
                return Ok(());
            }
            let name = julia_type_name(ast, id);
            remember(emitted, &name, &node.name)?;
            // Opaque, never a guess at what the name might mean: the
            // concrete type comes from the facet a `#pragma` or an
            // annotation set, and `Any` when there is none.
            let mapping = ast
                .any_of_type(id)
                .ok()
                .and_then(|any| ast.facets(any))
                .and_then(|facets| facets.corba.as_ref())
                .and_then(|corba| corba.native_mapping.clone());
            out.push(julia::Decl::Const(julia::ConstDecl {
                doc: with_idl_note(
                    Doc::line("A native type. Opaque unless a facet named its mapping."),
                    &name,
                    &node.name,
                ),
                name: Ident::new(&name),
                value: julia::Expr::Type(Box::new(match mapping {
                    Some(spelling) => julia::Type::named(&spelling),
                    None => julia::Type::Primitive(julia::Primitive::Any),
                })),
            }));
        }

        TypeKind::ValueBox { boxed } => {
            let Some(target) = boxed.resolved() else {
                return Ok(());
            };
            let name = julia_type_name(ast, id);
            if emitted.iter().any(|(spelled, _)| *spelled == name) {
                return Ok(());
            }
            remember(emitted, &name, &node.name)?;
            out.push(julia::Decl::Const(julia::ConstDecl {
                doc: with_idl_note(
                    Doc::line("A boxed value: the type or `nothing` when absent."),
                    &name,
                    &node.name,
                ),
                name: Ident::new(&name),
                value: julia::Expr::Type(Box::new(julia::Type::optional(member_type(
                    ast, target, &name, ctx,
                )?))),
            }));
        }

        // An object reference belongs to the RPC arc, an annotation type
        // has no runtime form at all, and `any` and `ValueBase` are
        // decided at run time — `zerodds_ast_layout` refuses to plan any
        // of them. Named here rather than passed over, so the coverage
        // table and this file say the same thing. A member that reaches
        // one of them is refused by name in [`member_type`].
        TypeKind::ObjectRef { .. }
        | TypeKind::AnnotationType { .. }
        | TypeKind::Any
        | TypeKind::ValueBase => {}
    }
    Ok(())
}

// -- types ---------------------------------------------------------------

/// The Julia spelling of a value's type, refusing what Julia cannot hold.
///
/// `any` is the first of those. It is in the model because Building Block
/// Any is one of IDL 4.2's fifteen, and what it holds is decided at run
/// time. Julia *has* `Any` and a field declared with it is perfectly
/// legal — which is precisely why refusing matters here more than in a
/// language that would have rejected the file: `m::Any` loads, holds
/// anything, and has no wire form behind it. That is the
/// silent-substitution class this project is arranged against.
///
/// # Errors
/// [`MaterialiseError::NoLanguageForm`] for `any`, `ValueBase` and an
/// object reference.
pub fn julia_member_type(
    ast: &Ast,
    target: Id<TypeNode>,
    at: &str,
) -> Result<julia::Type, MaterialiseError> {
    member_type(ast, target, at, &mut Ctx::unordered())
}

fn member_type(
    ast: &Ast,
    target: Id<TypeNode>,
    at: &str,
    ctx: &mut Ctx,
) -> Result<julia::Type, MaterialiseError> {
    let construct = match ast.type_(target).map(|node| &node.kind) {
        Ok(TypeKind::Any) => Some("any"),
        Ok(TypeKind::ValueBase) => Some("ValueBase"),
        Ok(TypeKind::ObjectRef { .. }) => Some("Object"),
        _ => None,
    };
    if let Some(construct) = construct {
        return Err(MaterialiseError::NoLanguageForm {
            construct: construct.to_string(),
            language: LANGUAGE,
            at: at.to_string(),
        });
    }
    value_type(ast, target, ctx)
}

/// The Julia type a value of `id` is held in.
///
/// The one answer, for every emitter. A field of a generated struct, a
/// codec's parameter and a defaulter's destination all have to name the
/// same Julia type or the file does not load.
///
/// # Errors
/// [`MaterialiseError::Unevaluated`] when an array length or a
/// collection's element does not resolve.
pub fn julia_value_type(ast: &Ast, id: Id<TypeNode>) -> Result<julia::Type, MaterialiseError> {
    value_type(ast, id, &mut Ctx::unordered())
}

/// zerodds-lint: recursion-depth 64 (collection and typedef nesting;
/// bounded by `MAX_DEPTH` against `Ctx::depth`, which this function
/// raises before descending and returns
/// `MaterialiseError::Unevaluated` past)
fn value_type(ast: &Ast, id: Id<TypeNode>, ctx: &mut Ctx) -> Result<julia::Type, MaterialiseError> {
    if ctx.depth > MAX_DEPTH {
        return Err(MaterialiseError::Unevaluated {
            construct: "a type expression deeper than the planner follows".to_string(),
            at: ast
                .type_(id)
                .map_or_else(|_| String::new(), |node| node.name.clone()),
        });
    }
    ctx.depth += 1;
    let result = value_type_inner(ast, id, ctx);
    ctx.depth -= 1;
    result
}

/// zerodds-lint: recursion-depth 64 (collection and typedef nesting;
/// bounded by the `MAX_DEPTH` check in [`value_type`], which every
/// descent from here goes through)
fn value_type_inner(
    ast: &Ast,
    id: Id<TypeNode>,
    ctx: &mut Ctx,
) -> Result<julia::Type, MaterialiseError> {
    let Ok(node) = ast.type_(id) else {
        return Ok(julia::Type::Primitive(julia::Primitive::Any));
    };
    Ok(match &node.kind {
        TypeKind::Primitive(kind) => julia_primitive_type(*kind),
        // A `wstring` holds UTF-8 rather than UTF-16 units, which
        // puts the §7.4.1.1.2 surrogate rule inside the Julia runtime
        // where it is a second implementation of the clause and not a
        // copy of the Rust column's answer. A Julia `String` is UTF-8
        // natively (manual, "Strings") and carries its own length, so
        // unlike the C column's `const char *` there is no separate
        // length to keep in step and no terminator convention to
        // agree on.
        TypeKind::String { .. } | TypeKind::Fixed { .. } => {
            julia::Type::Primitive(julia::Primitive::String)
        }
        TypeKind::Sequence { element, .. } => {
            let Some(element) = element.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "sequence element".to_string(),
                    at: node.name.clone(),
                });
            };
            julia::Type::vector(value_type(ast, element, ctx)?)
        }
        TypeKind::Array { element, dims } => {
            let Some(element) = element.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "array element".to_string(),
                    at: node.name.clone(),
                });
            };
            // Outermost dimension outermost: `long grid[2][3]` is two
            // rows of three, which in Julia is
            // `NTuple{2, NTuple{3, Int32}}`. Building it from the
            // inside out is what keeps that true.
            let mut type_ = value_type(ast, element, ctx)?;
            for dim in dims.iter().rev() {
                let length =
                    ast.eval_integral(*dim)
                        .map_err(|_| MaterialiseError::Unevaluated {
                            construct: "array length".to_string(),
                            at: node.name.clone(),
                        })?;
                type_ = julia::Type::ntuple(length, type_);
            }
            type_
        }
        TypeKind::Map { .. } => {
            let entry = julia_map_entry_name(ast, id);
            julia::Type::vector(named_or_forward(ctx, id, &entry))
        }
        // A typedef with no name is one the frontend made for an
        // inline type; it is transparent, so the Julia type is the
        // target's.
        TypeKind::Alias { aliased } if node.name.is_empty() => match aliased.resolved() {
            Some(target) => value_type(ast, target, ctx)?,
            None => julia::Type::Primitive(julia::Primitive::Any),
        },
        // A named alias that is not declared yet is written out
        // rather than named. `const Distance = Int32` binds a second
        // name for one type — IDL 4.2 §7.4.1.4.3: an alias "does not
        // define a new type" — so substituting what it aliases
        // changes nothing about the field's type, and it is the one
        // way past a cycle that closes through a typedef.
        TypeKind::Alias { aliased } if ctx.is_forward(id) => match aliased.resolved() {
            Some(target) => value_type(ast, target, ctx)?,
            None => julia::Type::Primitive(julia::Primitive::Any),
        },
        // The same, for a boxed value: its `const` binding is a
        // `Union{T, Nothing}` and writing that out is the same type.
        TypeKind::ValueBox { boxed } if ctx.is_forward(id) => match boxed.resolved() {
            Some(target) => julia::Type::optional(value_type(ast, target, ctx)?),
            None => julia::Type::Primitive(julia::Primitive::Any),
        },
        _ => {
            let name = julia_type_name(ast, id);
            named_or_forward(ctx, id, &name)
        }
    })
}

/// The declared name, or the abstract type standing in for it.
fn named_or_forward(ctx: &mut Ctx, id: Id<TypeNode>, name: &str) -> julia::Type {
    if ctx.is_forward(id) {
        ctx.record_forward(name);
        return julia::Type::named(&format!("{FORWARD_PREFIX}{name}"));
    }
    julia::Type::named(name)
}

/// The Julia type an IDL primitive becomes.
///
/// Exhaustive, so a primitive added to the model without a Julia spelling
/// is a compile error and not a member that silently takes another's.
///
/// `char` is one octet and `wchar` two, which is what the wire form is;
/// DDS-XTypes 1.3 §7.5.1.1.1 leaves the binding's spelling of `Char16`
/// open, and a fixed-width integer is the only choice that does not
/// depend on a platform type. Julia's `Char` is a 32-bit code point, so
/// it is neither of them.
///
/// `long double` is a `String` and never a `Float64` or a `BigFloat`; the
/// module documentation carries why.
#[must_use]
pub fn julia_primitive_type(kind: PrimitiveKind) -> julia::Type {
    let primitive = match kind {
        PrimitiveKind::Boolean => julia::Primitive::Bool,
        PrimitiveKind::Octet | PrimitiveKind::UInt8 | PrimitiveKind::Char => {
            julia::Primitive::UInt8
        }
        PrimitiveKind::Int8 => julia::Primitive::Int8,
        PrimitiveKind::WChar | PrimitiveKind::UInt16 => julia::Primitive::UInt16,
        PrimitiveKind::Int16 => julia::Primitive::Int16,
        PrimitiveKind::Int32 => julia::Primitive::Int32,
        PrimitiveKind::UInt32 => julia::Primitive::UInt32,
        PrimitiveKind::Int64 => julia::Primitive::Int64,
        PrimitiveKind::UInt64 => julia::Primitive::UInt64,
        PrimitiveKind::Float32 => julia::Primitive::Float32,
        PrimitiveKind::Float64 => julia::Primitive::Float64,
        PrimitiveKind::Float128 => julia::Primitive::String,
    };
    julia::Type::Primitive(primitive)
}

/// The unsigned integer a bit width calls for.
#[must_use]
pub const fn julia_holder(bits: u16) -> julia::Primitive {
    match bits {
        0..=8 => julia::Primitive::UInt8,
        9..=16 => julia::Primitive::UInt16,
        17..=32 => julia::Primitive::UInt32,
        _ => julia::Primitive::UInt64,
    }
}

// -- names ---------------------------------------------------------------

/// A declared type's Julia identifier.
///
/// The scoped name with the module separators flattened onto it:
/// `::common::util::Location` is `common_util_Location`.
///
/// The case is the IDL's. Julia's convention is `UpperCamelCase` for a
/// type and lower case for a field, and the column does not impose it:
/// the language enforces nothing, and a rename that cannot be inverted
/// would put the generated file and the IDL out of step for every reader
/// who has both open. The Go column has to move the case of one letter,
/// because in Go that letter is what `public` means.
#[must_use]
pub fn julia_type_name(ast: &Ast, id: Id<TypeNode>) -> String {
    let Ok(any) = ast.any_of_type(id) else {
        return "ZdUnnamed".to_string();
    };
    match ast.scoped_name_of(any) {
        Some(scoped) => flatten(scoped.segments()),
        None => "ZdUnnamed".to_string(),
    }
}

/// The entry struct a `map<K,V>` needs a name for.
///
/// Structural, so two identical anonymous maps collapse onto one
/// declaration instead of two that differ only in a counter. The tag is
/// built from the IDL kind and not the Julia spelling: `octet` and
/// `uint8` are both `UInt8` here, and a name built from the Julia type
/// would give `map<octet,V>` and `map<uint8,V>` one declaration —
/// harmless today and the same collapse that would be wrong if the two
/// ever differed on the wire.
#[must_use]
pub fn julia_map_entry_name(ast: &Ast, id: Id<TypeNode>) -> String {
    map_entry_name_at(ast, id, 0)
}

/// [`julia_map_entry_name`], carrying the depth of the walk that reached
/// it.
///
/// The depth is threaded rather than started over: `type_tag` reaches a
/// map's entry name here and this function reaches `type_tag` again, so
/// a counter reset at this door would leave the pair unbounded — the
/// `MAX_DEPTH` check in `type_tag` would see 0 on every turn of the
/// cycle.
///
/// zerodds-lint: recursion-depth 64 (key and value tags, via `type_tag`;
/// bounded by `MAX_DEPTH`, which names the type by its arena slot rather
/// than descending)
fn map_entry_name_at(ast: &Ast, id: Id<TypeNode>, depth: usize) -> String {
    if depth > MAX_DEPTH {
        return format!("{INVENTED_PREFIX}deep_{}", id.slot());
    }
    let Ok(node) = ast.type_(id) else {
        return format!("{INVENTED_PREFIX}unknown");
    };
    let TypeKind::Map { key, value, .. } = &node.kind else {
        return format!("{INVENTED_PREFIX}unknown");
    };
    let key = key
        .resolved()
        .map_or_else(|| "unknown".to_string(), |k| type_tag(ast, k, depth + 1));
    let value = value
        .resolved()
        .map_or_else(|| "unknown".to_string(), |v| type_tag(ast, v, depth + 1));
    format!("{INVENTED_PREFIX}{key}_{value}")
}

/// The name component a map entry takes from its key or value.
///
/// zerodds-lint: recursion-depth 64 (sequence, array and map nesting,
/// via `map_entry_name_at`; bounded by `MAX_DEPTH`, which names the
/// type by its arena slot rather than descending)
fn type_tag(ast: &Ast, id: Id<TypeNode>, depth: usize) -> String {
    if depth > MAX_DEPTH {
        // The slot and not a bare `deep`: two types that both nest past
        // the bound would otherwise take one tag, and the two structs
        // they name would merge onto one declaration with nothing said.
        return format!("deep_{}", id.slot());
    }
    match ast.type_(id).map(|node| node.kind.clone()) {
        Ok(TypeKind::Primitive(kind)) => kind.as_str().replace(' ', "_"),
        Ok(TypeKind::String { width, .. }) => match width {
            zerodds_ast::CharWidth::Char => "string".to_string(),
            zerodds_ast::CharWidth::WChar => "wstring".to_string(),
        },
        Ok(TypeKind::Sequence { element, .. }) => match element.resolved() {
            Some(element) => format!("seq_of_{}", type_tag(ast, element, depth + 1)),
            None => "seq_of_unknown".to_string(),
        },
        Ok(TypeKind::Array { element, dims }) => match element.resolved() {
            Some(element) => {
                let mut name = format!("arr_of_{}", type_tag(ast, element, depth + 1));
                for dim in &dims {
                    match ast.eval_integral(*dim) {
                        Ok(length) => name.push_str(&format!("_{length}")),
                        // A dimension that does not evaluate gets no
                        // number rather than a zero: the name has to
                        // differ from a `[0]` array's, and the caller
                        // reports the failure by name.
                        Err(_) => name.push_str("_x"),
                    }
                }
                name
            }
            None => "arr_of_unknown".to_string(),
        },
        Ok(TypeKind::Map { .. }) => map_entry_name_at(ast, id, depth + 1),
        _ => julia_type_name(ast, id),
    }
}

/// The flattened module path a declaration's file-scope constants carry.
///
/// The materialiser's own answer, published for the codec walk: an
/// enumerator of `::a::b::Colour` is bound at module scope as `a_b_RED`,
/// and a codec that rebuilt the prefix for itself would part from this
/// file on the first renamed module.
#[must_use]
pub fn julia_module_prefix(ast: &Ast, id: Id<TypeNode>) -> String {
    module_prefix(ast, id)
}

/// The flattened module path a file-scope constant carries.
fn module_prefix(ast: &Ast, id: Id<TypeNode>) -> String {
    let Ok(any) = ast.any_of_type(id) else {
        return String::new();
    };
    let Some(scoped) = ast.scoped_name_of(any) else {
        return String::new();
    };
    let segments = scoped.segments();
    if segments.len() <= 1 {
        return String::new();
    }
    let mut prefix = flatten(&segments[..segments.len() - 1]);
    prefix.push('_');
    prefix
}

/// The Julia name a declaration outside the type arena takes.
///
/// Constants, exceptions and interfaces are not types, so their scope
/// comes from the module they were declared in rather than from
/// [`Ast::scoped_name_of`].
fn declared_name(ast: &Ast, name: &str, module: Id<zerodds_ast::ModuleNode>) -> String {
    let mut segments: Vec<String> = Vec::new();
    let mut current = Some(module);
    let mut depth = 0;
    // Outermost first, so the walk up is reversed at the end. The bound
    // matches the layout planner's own: a parent chain that does not
    // terminate is a defect in the model and not a reason to hang.
    while let Some(id) = current {
        if depth > MAX_DEPTH {
            break;
        }
        depth += 1;
        let Ok(node) = ast.module(id) else { break };
        if node.name.is_empty() {
            break;
        }
        segments.push(node.name.clone());
        current = node.parent;
    }
    segments.reverse();
    segments.push(name.to_string());
    let borrowed: Vec<&str> = segments.iter().map(String::as_str).collect();
    flatten_str(&borrowed)
}

/// Join scoped segments with `_`, escaping each.
///
/// `_` and not concatenation: `commonutilLocation` cannot be read back to
/// its segments, and two different scoped names would reach one Julia
/// name with nothing to say so. The join is still not injective — an IDL
/// identifier may itself contain `_` — which is why [`remember`] checks
/// every declared name against every other one in the file.
fn flatten(segments: &[String]) -> String {
    let borrowed: Vec<&str> = segments.iter().map(String::as_str).collect();
    flatten_str(&borrowed)
}

fn flatten_str(segments: &[&str]) -> String {
    segments
        .iter()
        .map(|segment| julia::escape_name(segment))
        .collect::<Vec<String>>()
        .join("_")
}

/// The declaration's own IDL name, where the Julia spelling moved it.
fn idl_note(spelled: &str, declared: &str) -> Doc {
    if spelled == declared {
        return Doc::none();
    }
    Doc::line(&format!("IDL: `{declared}`."))
}

/// The same, appended to a documentation comment the caller wrote.
fn with_idl_note(mut doc: Doc, spelled: &str, declared: &str) -> Doc {
    if spelled != declared {
        doc.push(&format!("IDL: `{declared}`."));
    }
    doc
}

/// Record a declared name, refusing a second declaration that reaches the
/// same Julia spelling.
///
/// One file is one namespace here, so this is a whole-file check and not
/// a per-scope one — which is the difference from
/// [`no_spelling_collision`], used for the members of one aggregate. Two
/// IDL modules can flatten onto one Julia name: `a_b::C` and `a::b_C`
/// both give `a_b_C`.
///
/// Julia says nothing at all about that, which is why the check is here
/// and not left to the interpreter. Measured on Julia 1.12.6: a file
/// declaring `struct S; x::Int32; end` and then `struct S; y::Float64; end`
/// loads, prints no warning and exits 0, and `fieldnames(S)` is `(:y,)`
/// — the first declaration is gone. A second `const C = …` behaves the
/// same way. So a column that emitted both and let the interpreter find
/// it would emit a file that loads and is missing a type.
fn remember(
    emitted: &mut Vec<(String, String)>,
    spelled: &str,
    declared: &str,
) -> Result<(), MaterialiseError> {
    if let Some((_, first)) = emitted.iter().find(|(other, _)| other == spelled) {
        return Err(MaterialiseError::SpellingCollision {
            first: first.clone(),
            second: declared.to_string(),
            spelled: spelled.to_string(),
            language: LANGUAGE,
            at: "the generated file".to_string(),
        });
    }
    emitted.push((spelled.to_string(), declared.to_string()));
    Ok(())
}

/// Borrow a list of owned name pairs for [`no_spelling_collision`].
fn as_pairs(pairs: &[(String, String)]) -> Vec<(&str, &str)> {
    pairs
        .iter()
        .map(|(spelled, declared)| (spelled.as_str(), declared.as_str()))
        .collect()
}

/// Whether the rules created a node of `type_name` for `node`.
///
/// The same two-hop walk every sibling materialiser does: a
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

/// The Julia expression a constant's value takes.
///
/// Written as a conversion rather than a bare literal wherever the value
/// is a number: `const LIMIT = 8` binds an `Int64` on a 64-bit host and
/// an `Int32` on a 32-bit one, because a Julia integer literal takes the
/// platform's `Int`. `UInt8(8)` binds what the IDL declared, everywhere.
/// A `String` and a `Bool` need no conversion — a string literal is a
/// `String` and `true` is a `Bool` on every platform — and wrapping them
/// would say nothing.
///
/// `long double` is refused by name. Its Julia holder is a `String`
/// carrying the decimal, and CORBA gives a `long double` literal no
/// spelling that survives a `Float64`: emitting one would put a different
/// number in the file than the IDL wrote.
fn julia_constant(
    value: &zerodds_ast::ConstValue,
    type_: &julia::Type,
    at: &str,
) -> Result<julia::Expr, MaterialiseError> {
    use zerodds_ast::ConstValue;
    let refuse = |construct: &str| MaterialiseError::NoLanguageForm {
        construct: construct.to_string(),
        language: LANGUAGE,
        at: at.to_string(),
    };
    // A string literal is a `String` and `true` is a `Bool` on every
    // platform, so a conversion around either would say nothing. Every
    // other constant is written as one.
    let already_exact = matches!(
        type_,
        julia::Type::Primitive(julia::Primitive::String | julia::Primitive::Bool)
    );
    let is_string = matches!(type_, julia::Type::Primitive(julia::Primitive::String));
    let convert = |expr: julia::Expr| {
        if already_exact {
            expr
        } else {
            julia::Expr::convert(type_.clone(), expr)
        }
    };
    Ok(match value {
        ConstValue::Boolean(flag) => julia::Expr::Literal(Literal::Bool(*flag)),
        ConstValue::Integer(number) => convert(julia::Expr::Literal(Literal::int(*number))),
        // The code point as a number: an IDL `char` is one octet here and
        // an IDL `wchar` two, and a Julia `Char` is a 32-bit code point.
        ConstValue::Char(character) => convert(julia::Expr::Literal(Literal::int(i128::from(
            u32::from(*character),
        )))),
        ConstValue::WChar(point) => convert(julia::Expr::Literal(Literal::int(i128::from(*point)))),
        ConstValue::Float { mantissa, exponent } => {
            if is_string {
                return Err(refuse(
                    "a `long double` constant, which has no Julia literal",
                ));
            }
            convert(julia::Expr::Literal(Literal::Float(constant::float_text(
                mantissa,
                exponent.as_deref(),
            ))))
        }
        ConstValue::Fixed { digits, scale } => {
            julia::Expr::Literal(Literal::Str(constant::fixed_decimal(digits, *scale)))
        }
        ConstValue::String(text) => julia::Expr::Literal(Literal::Str(text.clone())),
        ConstValue::WString(points) => julia::Expr::Literal(Literal::Str(
            constant::wstring_utf8(points)
                .ok_or_else(|| refuse("a `wstring` constant holding a lone surrogate"))?,
        )),
        // `ConstValue` is `#[non_exhaustive]`, so a form added to the
        // model reaches here. Refused by name rather than given a
        // plausible default: a constant emitted as the wrong literal is a
        // file that disagrees with the IDL and loads.
        _ => return Err(refuse("a constant form this binding has no literal for")),
    })
}
