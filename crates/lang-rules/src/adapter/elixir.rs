// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Elixir CST as the other end of a rule. Spec §3.2.
//!
//! The counterpart of [`crate::adapter::python`], which faces the same
//! problem, and of [`crate::adapter::go`], which does not. Reading this
//! against the Python column is the point: both target a language that
//! states no width, no signedness and no bound in a declaration, and
//! both are arranged around the same question.
//!
//! Values are resolved from the model, never from the overlay — the same
//! rule the sibling materialisers follow and for the same reason
//! (spec §2.3).
//!
//! # The question this column is arranged around
//!
//! Elixir enforces no width, no signedness and no bound. An `integer()`
//! is arbitrary precision, a `binary()` has no length in its type, and
//! an IDL `octet` and an IDL `unsigned long long` are the same Elixir
//! type. A backend that wrote `x: integer()` and stopped would produce a
//! module that Dialyzer accepts and a value the wire cannot hold, with
//! nothing between the two to notice. That is the silent-substitution
//! class this project is arranged against.
//!
//! So: **for each IDL fact Elixir cannot enforce, where is it written
//! down?** There are exactly two places here and no third.
//!
//! 1. **The `@type` spec**, where Dialyzer can act on it. Which module a
//!    member holds, the arity of an array (a tuple's arity is part of
//!    its type), absence (`| nil`), the shape of a map entry
//!    (`list({k, v})`), and the atoms an enumeration admits.
//! 2. **A module attribute**, where Dialyzer cannot. `@zd_members`
//!    carries, per member and in declaration order: the IDL spelling,
//!    the kind, the width in bits, the signedness, the bound, the digit
//!    count of a `fixed`, the dimensions of an array and the module a
//!    named type resolves to. `@zd_idl` carries the declaration's own
//!    IDL name and scoped name. A codec reads them back at run time.
//!
//! Every attribute this column writes has a reader function beside it —
//! `def __zd_members__, do: @zd_members`. Not decoration: a module
//! attribute is a compile-time binding and is gone from the compiled
//! module afterwards, so a bare one is invisible to every other module,
//! and `elixirc` warns "module attribute @zd_members was set but never
//! used" for one with no reader (measured, Elixir 1.20.2).
//!
//! Nothing is left implicit. A member typed only `integer()` with the
//! width nowhere is exactly what this arrangement exists to prevent.
//!
//! ## The primitive mapping
//!
//! | IDL | `@type` | `@zd_members` |
//! |---|---|---|
//! | `boolean` | `boolean()` | `kind: :boolean, bits: 8` |
//! | `octet`, `uint8` | `integer()` | `bits: 8, signed: false` |
//! | `int8` | `integer()` | `bits: 8, signed: true` |
//! | `char` | `integer()` | `kind: :char, bits: 8` |
//! | `wchar` | `integer()` | `kind: :wchar, bits: 16` |
//! | `short` … `unsigned long long` | `integer()` | `bits: 16…64, signed: …` |
//! | `float`, `double` | `float()` | `bits: 32`, `bits: 64` |
//! | `long double` | **`binary()`** | `bits: 128, holder: :decimal_text` |
//! | `string`, `wstring` | `binary()` | `bound: N`, `bits: 8` or `16` |
//! | `fixed<P,S>` | `binary()` | `digits: P, scale: S` |
//!
//! A `char` is an `integer()` and not a one-character `binary()`: it is
//! a code unit of a stated width, and a `binary()` of length one would
//! be a second length constraint Elixir does not enforce either.
//!
//! ### `long double` is a `binary()` and never a `float()`
//!
//! An Elixir `float()` is an IEEE binary64 — 53 bits of significand —
//! and an IDL `long double` is a binary128 with 113. Writing one into a
//! `float()` loses sixty bits at the assignment, before any codec runs,
//! with nothing to say it happened. The member holds the decimal text
//! instead, exactly as `fixed<P,S>` does and as the Go and Python
//! columns do for the same reason.
//!
//! ### `string`, `wstring` and `fixed` are all `binary()`
//!
//! An Elixir binary is a run of octets with no encoding of its own —
//! `String` is a convention over one, not a type — which is what makes
//! it the right holder for a `wstring` carrying UTF-8. The §7.4.1.1.2
//! surrogate rule then lives in the runtime, where it is a second
//! implementation of the clause rather than a copy of the Rust column's
//! answer.
//!
//! ## The shapes
//!
//! | IDL | Elixir |
//! |---|---|
//! | `sequence<T>` | `list(T)` |
//! | `T[N][M]` | `{{T, …M}, …N}` — a tuple, outer dimension outermost |
//! | `map<K,V>` | `list({k, v})`, **never** a `map()` |
//! | `struct` | a module with `defstruct` and `@type t` |
//! | `union` | a module with `:_d` and one nullable field per branch |
//! | `enum` | a module of atoms, with `value/1` and `from_value/1` |
//! | `bitmask` | a module of bit constants, `@type t :: integer()` |
//! | `typedef` | a module with `@type t ::` the target |
//! | `interface` | a module of `@callback`s — a behaviour |
//! | `const` | an attribute and a zero-arity function reading it |
//! | `@optional T m` | `T | nil`, `nil` in the `defstruct` |
//!
//! ### `T[N]` is a tuple and `sequence<T>` is a list
//!
//! A tuple's arity is part of its type and Dialyzer checks it, so
//! `{integer(), integer(), integer()}` is the **one** IDL fact in this
//! whole column that Elixir's own type system enforces — the same
//! argument the Python column makes for `tuple[T, T, T]`. Writing
//! `list(integer())` for an array and putting the length in the
//! attribute would give that up for nothing.
//!
//! The cost is stated rather than hidden: `long a[1000]` produces a
//! `@type` with a thousand `integer()`s in it and a default with a
//! thousand zeros. Nothing truncates it and there is no size at which
//! the spelling changes, because a threshold would mean one IDL
//! construct had two Elixir forms depending on a number nobody chose.
//!
//! ### `map<K,V>` is a list of pairs and never an Elixir `map()`
//!
//! An Elixir map does not keep the order it was built in. The `Map`
//! documentation says so in as many words — a map's keys have no
//! ordering and `Enum` over one gives no guaranteed order — and the
//! representation changes at 32 keys, so an order that held for a small
//! sample does not hold for the next one. A `map<K,V>` goes on the wire
//! as an ordered sequence of key/value pairs (DDS-XTypes 1.3
//! §7.4.1.1.4), so a column holding it in a `map()` would reorder every
//! sample it round-tripped, and the KeyHash of a keyed type would change
//! with it. A list of two-tuples keeps the sample's order where the
//! sample put it. A `map()` loses more than the order, too: two entries
//! with one key silently become one, and a `map<K,V>` may carry both.
//!
//! ### A union is a struct with `:_d` and not a tagged tuple
//!
//! This is the load-bearing decision of this column. `{:idle, 3}` is the
//! natural Elixir shape for a variant, and it cannot hold what an IDL
//! union is: a union's discriminator may take a value **no branch
//! selects**. IDL 4.2 §7.4.1.4.4.1 makes the discriminator a value in
//! its own right, and DDS-XTypes 1.3 §7.4.1.1.5.2 writes the
//! discriminator and then, only if a branch is selected, that branch —
//! so a sample whose discriminator selects nothing is well-formed and
//! carries no branch at all. A tagged tuple has no tag for that state,
//! and a column that invented one (`{:__none__, nil}`) would be putting
//! a value on the wire that the IDL does not describe.
//!
//! So the union is a struct: `:_d` holds the discriminator, every branch
//! is a field of its own defaulting to `nil`, and a sample that selects
//! no branch is the one where every branch field is `nil`. `_d` is a
//! legal `defstruct` key and draws no warning (measured, Elixir 1.20.2 —
//! the unused-variable convention is about variables, not atoms), and it
//! is the spelling every other column uses for the same field.
//!
//! ### An enumeration is a module of atoms and a member of one is an
//! `integer()`
//!
//! Both halves matter, and the split is the Python column's.
//!
//! The **declaration** is a module: Elixir has no enumeration construct,
//! so what an enumeration becomes is a `@type t :: :idle | :active` — a
//! union of literal atoms, which is a type Dialyzer checks exactly — a
//! zero-arity function per enumerator returning its value, and the
//! `value/1` and `from_value/1` pair that carries the mapping in both
//! directions. Every value is written out; `iota`-style consecutive
//! numbering is not available and would be wrong anyway, because
//! `@value` admits gaps.
//!
//! The **member** is nonetheless an `integer()` and not `Mode.t()`: an
//! `@appendable` writer may send an enumerator the reader's IDL does not
//! list, and there is no atom for it. `from_value/1` returns
//! `{:error, {:unknown_value, n}}` for exactly that case rather than
//! raising, and the member holds what arrived. Which enumeration it is
//! stays readable, in `@zd_members`, where a codec can get at it.
//!
//! ### A typedef is a module with a `@type`
//!
//! `defmodule Distance do @type t :: integer() end`, and every use is
//! `Distance.t()`. Elixir's `@type` is per-module and there is no
//! file-scope type alias, so the alias needs a module to live in; the
//! module declares no struct, so `Distance.t()` expands to the target
//! and nothing else — which is what IDL 4.2 §7.4.1.4.3 says an alias is:
//! it "does not define a new type". A comment instead, as the Python
//! column writes, is not needed here: Elixir resolves a remote type
//! lazily, so there is no ordering problem to avoid.
//!
//! ### Modules nest, and this column does not flatten
//!
//! An Elixir module name is an atom with dotted segments and the nesting
//! is real: `Common.Util.Location` is one name in three parts. The C and
//! Go columns have to flatten `::common::util::Location` onto one
//! identifier because a C translation unit has one namespace and a Go
//! package is a directory. This column writes the segments.
//!
//! A segment that names a module of the Elixir distribution is escaped,
//! and both halves of the reason were measured rather than assumed.
//! `defmodule S` and `defmodule Elixir.S` in one file both produce
//! `Elixir.S.beam` and the second **silently replaces** the first, with
//! no diagnostic at all. And a redefinition does not shadow: `defmodule
//! Node do defstruct […] end` followed by `%Node{}` in the same file is
//! `error: Node.__struct__/1 is undefined, cannot expand struct Node`,
//! because the alias answers with the preloaded stdlib module. An IDL
//! `struct Node { sequence<Node> children; };` is an ordinary recursive
//! declaration and this column has to carry it. See
//! [`zerodds_lang_cst::elixir::module_segment`] and its
//! `RESERVED_MODULES`.
//!
//! ### Where the IDL spelling goes
//!
//! An Elixir module segment must start with an upper case letter and an
//! atom must start with a lower case one, so `common` becomes `Common`
//! and a member `X` becomes `:x`. Neither renaming inverts. A member's
//! ID under `@autoid(HASH)` and under `@hashid` is the first four octets
//! of the MD5 of its **IDL** name (DDS-XTypes 1.3 §7.3.1.2.1.1), so a
//! file that only knew the Elixir spelling could not compute the ID for
//! the member it is writing. Every member's IDL name is therefore in
//! `@zd_members`, and every declaration's is in `@zd_idl`.
//!
//! ## What is refused
//!
//! `any`, `ValueBase`, an object reference and an annotation type have
//! no Elixir data form and are named as
//! [`MaterialiseError::NoLanguageForm`] in member position; at
//! declaration level they produce nothing. `any()` is a real Elixir type
//! that Dialyzer accepts, which is precisely why refusing matters rather
//! than emitting it: a member typed `any()` is a field no codec can
//! write, from a generator that reported success.

use crate::adapter::constant;
use crate::adapter::cst::{MaterialiseError, no_spelling_collision};
use crate::adapter::interface;
use crate::apply::Applied;
use crate::verbatim;
use seesaw_tgg::ident::GhostId;
use zerodds_ast::{
    Ast, Bound, CaseLabel, CharWidth, ModuleNode, PrimitiveKind, TypeKind, TypeNode,
};
use zerodds_ast_core::{Id, NodeId};
use zerodds_lang_cst::common::{Doc, Ident, Literal, Path};
use zerodds_lang_cst::elixir::{self, Basic, Expr, Type};

/// The language this column names in a diagnostic.
const LANGUAGE: &str = "Elixir";

/// How far up a module chain the walk follows before it stops.
///
/// A parent chain that does not terminate is a defect in the model and
/// not a reason to hang. IDL nests nowhere near this.
const MAX_MODULE_DEPTH: usize = 64;

/// How deep a type's Elixir spelling is followed.
///
/// A `sequence<sequence<…>>` nests once per level; a chain longer than
/// this is a cycle the resolver did not catch, and recursing into it
/// would end the process rather than report anything.
const MAX_TYPE_DEPTH: usize = 64;

/// The module the constants of one IDL scope are gathered into.
///
/// Elixir has no file-scope binding: a `const` becomes an attribute and
/// a function, and both need a module. The name is invented, so it
/// carries the `Zd` marker every invented name in this project carries,
/// and it goes through the same whole-file collision check as a declared
/// one — an IDL `struct ZdConstants` in the same scope is refused by
/// name rather than silently replaced.
pub const CONSTANTS_MODULE: &str = "ZdConstants";

/// The union discriminator's field name.
///
/// `_d`, as in every other column. An IDL identifier never starts with
/// an underscore — IDL 4.2 §7.2.3 makes the first character a letter,
/// and the `_keyword` escape of §7.2.3.2 denotes the identifier
/// *without* it — so this cannot collide with a branch's name. It is
/// checked against them anyway.
pub const DISCRIMINATOR: &str = "_d";

/// Build the Elixir modules a rule application produced.
///
/// The order is the model's own declaration order. Elixir resolves a
/// remote type and a remote call lazily, and this column emits no struct
/// literal, so unlike the C column there is no dependency walk and the
/// output tracks the source.
///
/// # Errors
/// [`MaterialiseError`] when a value the output has to state does not
/// evaluate, a type has no Elixir shape, or two declarations reach one
/// Elixir module name — never a silently dropped declaration.
pub fn elixir_modules(
    ast: &Ast,
    applied: &Applied,
) -> Result<Vec<elixir::TopLevel>, MaterialiseError> {
    let mut out = Vec::new();
    let mut emitted: Vec<(String, String)> = Vec::new();
    // `@verbatim` text, keyed by the module index it goes in front of.
    let mut text: Vec<(usize, Vec<String>)> = Vec::new();

    for (id, _) in ast.types() {
        let start = out.len();
        declare(ast, applied, id, &mut out, &mut emitted)?;
        let slots = verbatim::declaration_slots(ast, id, verbatim::aliases::ELIXIR);
        if slots.is_empty() {
            continue;
        }
        let path = type_module_path(ast, id);
        let mut begin = slots.begin;
        let mut end = slots.end;
        // Inside the type's own module, which is its declaration: the
        // struct, the enumeration's functions and the alias all live in
        // one `defmodule`.
        if let Some(module) = out
            .get_mut(start..)
            .unwrap_or_default()
            .iter_mut()
            .find(|module| module.name == path)
        {
            let mut body: Vec<elixir::Item> = begin.drain(..).map(elixir::Item::Verbatim).collect();
            body.append(&mut module.body);
            body.extend(end.drain(..).map(elixir::Item::Verbatim));
            module.body = body;
        }
        let mut before = slots.before;
        before.append(&mut begin);
        let mut after = end;
        after.extend(slots.after);
        text.push((start, before));
        text.push((out.len(), after));
    }

    declare_constants(ast, applied, &mut out, &mut emitted)?;
    declare_exceptions(ast, applied, &mut out, &mut emitted)?;
    declare_interfaces(ast, applied, &mut out, &mut emitted)?;

    let (begin_file, end_file) = verbatim::file_slots(ast, verbatim::aliases::ELIXIR);
    let mut top: Vec<elixir::TopLevel> = begin_file
        .into_iter()
        .map(elixir::TopLevel::Verbatim)
        .collect();
    let mut text = text.into_iter().peekable();
    for (index, module) in out.into_iter().enumerate() {
        while let Some((_, lines)) = text.next_if(|(at, _)| *at <= index) {
            top.extend(lines.into_iter().map(elixir::TopLevel::Verbatim));
        }
        top.push(elixir::TopLevel::Module(module));
    }
    for (_, lines) in text {
        top.extend(lines.into_iter().map(elixir::TopLevel::Verbatim));
    }
    top.extend(end_file.into_iter().map(elixir::TopLevel::Verbatim));
    Ok(top)
}

/// The Elixir modules, in a file that says where they came from.
///
/// A caller that only wants the modules uses [`elixir_modules`]; this is
/// what a generator writes to disk.
///
/// # Errors
/// As [`elixir_modules`].
pub fn elixir_file(ast: &Ast, applied: &Applied) -> Result<elixir::File, MaterialiseError> {
    Ok(elixir::File {
        doc: Doc::lines(&[
            "Generated by ZeroDDS from an IDL source. Do not edit.",
            "",
            "Elixir enforces no width, no signedness and no bound, so every one",
            "of those is written into a module attribute beside the type: read",
            "them back with Module.__zd_members__/0 and Module.__zd_idl__/0.",
            "The IDL spelling of every declaration and every member is there",
            "too, because a member's ID under @autoid(HASH) is the MD5 of the",
            "IDL name (DDS-XTypes 1.3 7.3.1.2.1.1) and neither the module",
            "segment's capital nor the atom's lower case inverts.",
        ]),
        modules: elixir_modules(ast, applied)?,
    })
}

/// One declared type.
#[allow(clippy::too_many_lines)]
fn declare(
    ast: &Ast,
    applied: &Applied,
    id: Id<TypeNode>,
    out: &mut Vec<elixir::Module>,
    emitted: &mut Vec<(String, String)>,
) -> Result<(), MaterialiseError> {
    let Ok(node) = ast.type_(id) else {
        return Ok(());
    };

    match &node.kind {
        // A primitive, a string, a fixed, a sequence, an array and a map
        // are all written where the member is: Elixir spells each of
        // them inline — `list(T)`, `{T, T}`, `list({k, v})` — so none of
        // them is a declaration. The map needs no entry module at all,
        // which is where this column declares less than the Go one.
        TypeKind::Primitive(_)
        | TypeKind::String { .. }
        | TypeKind::Fixed { .. }
        | TypeKind::Sequence { .. }
        | TypeKind::Map { .. } => {}

        // Neither does an array — but every dimension is evaluated here
        // anyway, because the length is written into the `@type` as that
        // many tuple elements and a dimension that does not evaluate has
        // no spelling at all. The C, Go and Python columns fail on the
        // same input, so the columns' failure sets stay comparable.
        TypeKind::Array { dims, .. } => {
            for dim in dims {
                ast.eval_integral(*dim)
                    .map_err(|_| MaterialiseError::Unevaluated {
                        construct: "array length".to_string(),
                        at: named(&node.name, node.node_id),
                    })?;
            }
        }

        TypeKind::Alias { aliased } => {
            // A typedef with no name is one the frontend made for an
            // inline type; it is transparent and declares nothing.
            if node.name.is_empty() {
                return Ok(());
            }
            if ghost_created_for(applied, node.node_id, "ElixirAlias").is_none() {
                return Ok(());
            }
            let Some(target) = aliased.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "typedef target".to_string(),
                    at: node.name.clone(),
                });
            };
            let name = type_module_path(ast, id);
            remember(emitted, &name, &node.name)?;
            let mut body = vec![
                idl_attribute(ast, id, "alias", &node.name),
                reader("__zd_idl__", "zd_idl"),
                elixir::Item::Attribute(elixir::Attribute {
                    doc: Doc::none(),
                    name: Ident::new("zd_target"),
                    value: Expr::Keyword(type_facts(ast, target, &node.name, 0)?),
                }),
                reader("__zd_target__", "zd_target"),
            ];
            body.push(type_alias(
                "t",
                elixir_member_type(ast, target, &node.name)?,
                Doc::none(),
            ));
            out.push(elixir::Module {
                doc: Doc::lines(&[
                    "A typedef, as the type alias it is.",
                    "",
                    "The module declares no struct: IDL 4.2 7.4.1.4.3 says an alias",
                    "\"does not define a new type\", so t() expands to the target and",
                    "a value of the target is a value of this.",
                ]),
                name,
                body,
            });
        }

        TypeKind::Enumeration { enumerators } => {
            if ghost_created_for(applied, node.node_id, "ElixirEnum").is_none() {
                return Ok(());
            }
            let name = type_module_path(ast, id);
            remember(emitted, &name, &node.name)?;
            let bits = ast.effective_bit_bound(id);
            // The atom, the value and the IDL spelling of each
            // enumerator, gathered once and used three times: for the
            // `@type`, for the functions and for the attribute.
            let mut listed: Vec<(String, i128, String)> = Vec::new();
            for enumerator in enumerators {
                let Ok(enum_node) = ast.enumerator(*enumerator) else {
                    continue;
                };
                if ghost_created_for(applied, enum_node.node_id, "ElixirEnumerator").is_none() {
                    continue;
                }
                let value = ast.effective_enumerator_value(*enumerator).ok_or_else(|| {
                    MaterialiseError::Unevaluated {
                        construct: "enumerator value".to_string(),
                        at: enum_node.name.clone(),
                    }
                })?;
                listed.push((
                    elixir::atom_name(&enum_node.name),
                    value,
                    enum_node.name.clone(),
                ));
            }
            no_spelling_collision(
                listed
                    .iter()
                    .map(|(spelled, _, declared)| (spelled.as_str(), declared.as_str())),
                LANGUAGE,
                &node.name,
            )?;
            out.push(elixir::Module {
                doc: Doc::lines(&[
                    "An enumeration, as the atoms it names.",
                    "",
                    "Every value is written out rather than counted: @value admits",
                    "gaps, and a numbering this file inferred would renumber the",
                    "wire (DDS-XTypes 1.3 7.4.3.5.3 gives the holder; the width is",
                    "the @bit_bound's and is in @zd_idl).",
                    "",
                    "A member of this type is annotated integer() and not t(): an",
                    "@appendable writer may send an enumerator this IDL does not",
                    "list, and there is no atom for it. from_value/1 answers",
                    "{:error, {:unknown_value, n}} for that value rather than",
                    "raising.",
                ]),
                name,
                body: enum_body(ast, id, &node.name, bits, &listed),
            });
        }

        TypeKind::Bitmask { bits } => {
            if ghost_created_for(applied, node.node_id, "ElixirBitmask").is_none() {
                return Ok(());
            }
            let name = type_module_path(ast, id);
            remember(emitted, &name, &node.name)?;
            let bound = ast.effective_bit_bound(id);
            let mut listed: Vec<(String, u16, i128, String)> = Vec::new();
            for bit in bits {
                let Ok(bit_node) = ast.bit_value(*bit) else {
                    continue;
                };
                if ghost_created_for(applied, bit_node.node_id, "ElixirBit").is_none() {
                    continue;
                }
                let position = ast.effective_bit_position(*bit).ok_or_else(|| {
                    MaterialiseError::Unevaluated {
                        construct: "bit position".to_string(),
                        at: bit_node.name.clone(),
                    }
                })?;
                let value = bit_value(position).ok_or_else(|| MaterialiseError::Unevaluated {
                    construct: "bit position past the widest holder".to_string(),
                    at: bit_node.name.clone(),
                })?;
                listed.push((
                    elixir::atom_name(&bit_node.name),
                    position,
                    value,
                    bit_node.name.clone(),
                ));
            }
            no_spelling_collision(
                listed
                    .iter()
                    .map(|(spelled, _, _, declared)| (spelled.as_str(), declared.as_str())),
                LANGUAGE,
                &node.name,
            )?;
            out.push(elixir::Module {
                doc: Doc::lines(&[
                    "A bitmask, as one function per bit.",
                    "",
                    "The holder is the unsigned integer Table 33 of DDS-XTypes 1.3",
                    "7.4.1.1.3.2 gives for the @bit_bound, which is in @zd_idl;",
                    "Elixir's integer() has no width of its own, so the width is",
                    "stated there and nowhere else.",
                    "",
                    "Each function returns the bit's value and not its position:",
                    "Bitwise.bsl/2 is a macro and would make every generated module",
                    "require Bitwise for a number this file already knows.",
                ]),
                name,
                body: bitmask_body(ast, id, &node.name, bound, &listed),
            });
        }

        TypeKind::Bitset { .. } => {
            if ghost_created_for(applied, node.node_id, "ElixirBitset").is_none() {
                return Ok(());
            }
            let name = type_module_path(ast, id);
            remember(emitted, &name, &node.name)?;
            // The base's bitfields are members of the derived bitset
            // too: an inherited bitset is one flat run.
            let mut listed: Vec<(String, u16, String)> = Vec::new();
            for field in &ast.all_bitfields_of(id) {
                let Ok(field_node) = ast.bitfield(*field) else {
                    continue;
                };
                // An anonymous bitfield is padding: it moves the next
                // field along and carries no value of its own.
                let Some(field_ident) = field_node.name.as_ref() else {
                    continue;
                };
                listed.push((
                    elixir::atom_name(field_ident),
                    field_node.bits,
                    field_ident.clone(),
                ));
            }
            no_spelling_collision(
                listed
                    .iter()
                    .map(|(spelled, _, declared)| (spelled.as_str(), declared.as_str())),
                LANGUAGE,
                &node.name,
            )?;
            out.push(elixir::Module {
                doc: Doc::lines(&[
                    "A bitset, as a struct of one field per bitfield.",
                    "",
                    "The fields are separate integers here and the packing into the",
                    "anchor is the codec's, because the order on the wire is the",
                    "declaration's. The width of each is in @zd_bitfields: a",
                    "bitfield<3> is three bits, and Elixir has no type that says so.",
                ]),
                name,
                body: bitset_body(ast, id, &node.name, &listed),
            });
        }

        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            let created = if matches!(node.kind, TypeKind::Valuetype { .. }) {
                "ElixirValue"
            } else {
                "ElixirStruct"
            };
            if ghost_created_for(applied, node.node_id, created).is_none() {
                return Ok(());
            }
            let name = type_module_path(ast, id);
            remember(emitted, &name, &node.name)?;
            // XTypes 1.3 §7.5.1.4.1.3: the base is the first member `parent`; the wire
            // still carries the base's members first (§7.4.1.1.5.1).
            let mut fields = Vec::new();
            if let Some(base) = super::c::struct_base(ast, id) {
                fields.push(member_field(ast, base, "parent", false, false, Vec::new())?);
            }
            for member in ast.members_of(id) {
                let Ok(member_node) = ast.member(*member) else {
                    continue;
                };
                let Some(target) = member_node.type_.resolved() else {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "member type".to_string(),
                        at: member_node.name.clone(),
                    });
                };
                let optional = member_node.facets.is_optional();
                fields.push(member_field(
                    ast,
                    target,
                    &member_node.name,
                    optional,
                    optional,
                    Vec::new(),
                )?);
            }
            check_fields(&fields, &node.name)?;
            let kind = if matches!(node.kind, TypeKind::Valuetype { .. }) {
                "valuetype"
            } else {
                "struct"
            };
            out.push(elixir::Module {
                doc: aggregate_doc(),
                name,
                body: aggregate_body(ast, id, &node.name, kind, &fields, Vec::new()),
            });
        }

        TypeKind::Union { discriminator, .. } => {
            if ghost_created_for(applied, node.node_id, "ElixirUnion").is_none() {
                return Ok(());
            }
            let name = type_module_path(ast, id);
            remember(emitted, &name, &node.name)?;
            let Some(disc) = discriminator.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "union discriminator".to_string(),
                    at: node.name.clone(),
                });
            };
            // The discriminator is in the collision check with the
            // branches although an IDL identifier cannot start with an
            // underscore: "cannot" about a name collision is the
            // assumption that makes one declaration take another's.
            let mut fields = vec![Member {
                atom: DISCRIMINATOR.to_string(),
                idl: "<discriminator>".to_string(),
                type_: elixir_member_type(ast, disc, &node.name)?,
                default: default_for(ast, disc, 0)?.unwrap_or_else(Expr::nil),
                enforced: false,
                listed: false,
                facts: Vec::new(),
            }];
            for case in ast.cases_of(id) {
                let Ok(case_node) = ast.case(*case) else {
                    continue;
                };
                if ghost_created_for(applied, case_node.node_id, "ElixirCase").is_none() {
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
                let labels = case_labels(ast, &case_node.labels, &member.name)?;
                // Every branch is nullable and defaults to `nil`,
                // whatever its own type: only the selected branch is on
                // the wire (DDS-XTypes 1.3 §7.4.1.1.5.2), so every other
                // one has no value in that sample, and a discriminator
                // that selects nothing leaves all of them without one.
                fields.push(member_field(
                    ast,
                    target,
                    &member.name,
                    true,
                    member.facets.is_optional(),
                    labels,
                )?);
            }
            check_fields(&fields, &node.name)?;
            let discriminator_facts = type_facts(ast, disc, &node.name, 0)?;
            out.push(elixir::Module {
                doc: Doc::lines(&[
                    "A union, as a struct with a discriminator and one nullable",
                    "field per branch.",
                    "",
                    "Not a tagged tuple. An IDL union's discriminator may take a",
                    "value no branch selects (IDL 4.2 7.4.1.4.4.1), and",
                    "DDS-XTypes 1.3 7.4.1.1.5.2 writes the discriminator and then",
                    "only the selected branch — so a sample that selects nothing is",
                    "well formed and carries no branch. A tagged tuple has no tag",
                    "for that; here it is the value whose branch fields are all nil.",
                    "",
                    "Which values select which branch is in @zd_members, under",
                    "labels:, with :default for the default case.",
                ]),
                name,
                body: aggregate_body(ast, id, &node.name, "union", &fields, discriminator_facts),
            });
        }

        TypeKind::Native => {
            if ghost_created_for(applied, node.node_id, "ElixirNative").is_none() {
                return Ok(());
            }
            let name = type_module_path(ast, id);
            remember(emitted, &name, &node.name)?;
            // Opaque, never a guess at what the name might mean: the
            // concrete type comes from the facet a `#pragma` or an
            // annotation set, and `any()` when there is none. This is
            // the one place `any()` is written, and it is a declaration
            // the IDL made opaque rather than a member the column could
            // not spell.
            let mapping = ast
                .any_of_type(id)
                .ok()
                .and_then(|any| ast.facets(any))
                .and_then(|facets| facets.corba.as_ref())
                .and_then(|corba| corba.native_mapping.clone());
            let type_ = mapping
                .as_deref()
                .map_or_else(|| Type::Basic(Basic::Any), named_type);
            out.push(elixir::Module {
                doc: Doc::lines(&[
                    "A native type. Opaque unless a facet named its mapping.",
                    "",
                    "any() here is the declaration's own opacity and not a member",
                    "this column could not spell: a member that reaches an `any`,",
                    "a ValueBase or an object reference is refused by name.",
                ]),
                name,
                body: vec![
                    idl_attribute(ast, id, "native", &node.name),
                    reader("__zd_idl__", "zd_idl"),
                    type_alias("t", type_, Doc::none()),
                ],
            });
        }

        TypeKind::ValueBox { boxed } => {
            let Some(target) = boxed.resolved() else {
                return Ok(());
            };
            let name = type_module_path(ast, id);
            if emitted.iter().any(|(spelled, _)| *spelled == dotted(&name)) {
                return Ok(());
            }
            remember(emitted, &name, &node.name)?;
            out.push(elixir::Module {
                doc: Doc::line("A boxed value: the boxed type, or nil when absent."),
                name,
                body: vec![
                    idl_attribute(ast, id, "valuebox", &node.name),
                    reader("__zd_idl__", "zd_idl"),
                    type_alias(
                        "t",
                        elixir_member_type(ast, target, &node.name)?.or_nil(),
                        Doc::none(),
                    ),
                ],
            });
        }

        // An object reference belongs to the RPC arc, an annotation type
        // has no runtime form at all, and `any` and `ValueBase` are
        // decided at run time — `zerodds_ast_layout` refuses to plan any
        // of them. Named here rather than passed over, so the coverage
        // table and this file say the same thing. A member that reaches
        // one of them is refused by name in `elixir_member_type`.
        TypeKind::ObjectRef { .. }
        | TypeKind::AnnotationType { .. }
        | TypeKind::Any
        | TypeKind::ValueBase => {}
    }
    Ok(())
}

/// One field of a generated struct, with everything the three places it
/// appears need.
///
/// The `defstruct`, the `@type` and `@zd_members` state three different
/// things about one member and have to agree; building them from one
/// value is what makes that so rather than nearly so.
struct Member {
    /// The atom the field is stored under.
    atom: String,
    /// The IDL spelling, which the atom does not invert to.
    idl: String,
    /// Its type in the `@type`.
    type_: Type,
    /// Its default in the `defstruct`.
    default: Expr,
    /// Whether it goes in `@enforce_keys`.
    enforced: bool,
    /// Whether `@zd_members` lists it.
    ///
    /// The union discriminator is a field of the struct and is **not** a
    /// member: IDL 4.2 §7.4.1.4.4.1 gives it no member name of its own,
    /// so it has no IDL spelling, no member ID and no place in the
    /// member list a codec walks. Its own type is in `@zd_discriminator`
    /// instead.
    listed: bool,
    /// What `@zd_members` says about it.
    facts: Vec<(Ident, Expr)>,
}

/// One member, as everything the three places need.
///
/// `nullable` and `optional` are two questions and not one. An
/// `@optional` member is both: absence is a value it can take
/// (DDS-XTypes 1.3 §7.2.2.4.4.4.7), and the attribute says so. A union
/// branch is nullable and **not** optional: only the selected branch is
/// on the wire (§7.4.1.1.5.2), so every other branch has no value in
/// that sample — which is not the same statement as `@optional`, and an
/// attribute that conflated them would tell a codec to write an absence
/// flag for a member that has none.
fn member_field(
    ast: &Ast,
    target: Id<TypeNode>,
    name: &str,
    nullable: bool,
    optional: bool,
    extra: Vec<(Ident, Expr)>,
) -> Result<Member, MaterialiseError> {
    let type_ = elixir_member_type(ast, target, name)?;
    let mut facts = vec![kw("idl", Expr::string(name))];
    facts.extend(type_facts(ast, target, name, 0)?);
    facts.extend(extra);
    if optional {
        facts.push(kw("optional", Expr::Literal(Literal::Bool(true))));
    }
    let natural = default_for(ast, target, 0)?;
    Ok(Member {
        atom: elixir::atom_name(name),
        idl: name.to_string(),
        // `nil` is the one state no written value can take.
        type_: if nullable { type_.or_nil() } else { type_ },
        default: if nullable {
            Expr::nil()
        } else {
            natural.clone().unwrap_or_else(Expr::nil)
        },
        // A field whose type has no value that could stand as a default
        // is enforced instead: `%Pose{}` is then a compile error rather
        // than a sample with a hole in it, which is the arrangement the
        // Python column reaches by leaving a dataclass field with no
        // default at all.
        enforced: !nullable && natural.is_none(),
        listed: true,
        facts,
    })
}

/// Refuse two members that would take one atom.
fn check_fields(fields: &[Member], at: &str) -> Result<(), MaterialiseError> {
    no_spelling_collision(
        fields
            .iter()
            .map(|field| (field.atom.as_str(), field.idl.as_str())),
        LANGUAGE,
        at,
    )
}

/// The documentation every aggregate carries.
fn aggregate_doc() -> Doc {
    Doc::lines(&[
        "An aggregate, as a struct and the type of one.",
        "",
        "Elixir states no width, no signedness and no bound, so every one of",
        "those is in @zd_members, in declaration order, with the IDL spelling",
        "of each member beside it. Read it back with __zd_members__/0.",
        "",
        "A field with no default is in @enforce_keys: its type has no value",
        "that could stand as one, so %__MODULE__{} is a compile error rather",
        "than a sample with a hole in it.",
    ])
}

/// The body every aggregate takes: the attributes, the struct, the type
/// and the readers.
fn aggregate_body(
    ast: &Ast,
    id: Id<TypeNode>,
    declared: &str,
    kind: &str,
    fields: &[Member],
    discriminator: Vec<(Ident, Expr)>,
) -> Vec<elixir::Item> {
    let mut body = vec![
        idl_attribute(ast, id, kind, declared),
        reader("__zd_idl__", "zd_idl"),
    ];
    if !discriminator.is_empty() {
        body.push(elixir::Item::Attribute(elixir::Attribute {
            doc: Doc::line("The discriminator's own type, which is not a member."),
            name: Ident::new("zd_discriminator"),
            value: Expr::Keyword(discriminator),
        }));
        body.push(reader("__zd_discriminator__", "zd_discriminator"));
    }
    body.extend(members_body(fields));
    body
}

/// The body of an enumeration's module.
fn enum_body(
    ast: &Ast,
    id: Id<TypeNode>,
    declared: &str,
    bits: u16,
    listed: &[(String, i128, String)],
) -> Vec<elixir::Item> {
    let mut body = vec![
        idl_attribute_with(
            ast,
            id,
            "enum",
            declared,
            vec![kw("bits", Expr::int(i128::from(bits)))],
        ),
        reader("__zd_idl__", "zd_idl"),
        elixir::Item::Attribute(elixir::Attribute {
            doc: Doc::line("Every enumerator: the atom, its value and its IDL spelling."),
            name: Ident::new("zd_enumerators"),
            value: Expr::List(
                listed
                    .iter()
                    .map(|(atom, value, idl)| {
                        Expr::Tuple(vec![
                            Expr::Atom(atom.clone()),
                            Expr::int(*value),
                            Expr::string(idl),
                        ])
                    })
                    .collect(),
            ),
        }),
        reader("__zd_enumerators__", "zd_enumerators"),
        type_alias(
            "t",
            match listed.len() {
                // An enumeration with no enumerator this column emitted
                // has no atom to name. `none()` is the type no value
                // inhabits, which is what that is; `atom()` would say
                // every atom is one of them.
                0 => Type::Basic(Basic::None),
                _ => Type::Union(
                    listed
                        .iter()
                        .map(|(atom, _, _)| Type::Atom(atom.clone()))
                        .collect(),
                ),
            },
            Doc::none(),
        ),
    ];
    for (atom, value, _) in listed {
        body.push(elixir::Item::Function(elixir::Function {
            doc: Doc::none(),
            name: Ident::new(atom),
            params: Vec::new(),
            body: Expr::int(*value),
        }));
    }
    if listed.is_empty() {
        // No `@spec` either: Elixir refuses a spec for a function the
        // module does not define.
        return body;
    }
    body.push(elixir::Item::Spec(elixir::Signature {
        doc: Doc::line("The wire value of an enumerator this IDL names."),
        name: Ident::new("value"),
        params: vec![elixir::Param {
            name: None,
            type_: Type::Local(Ident::new("t")),
        }],
        result: Type::Basic(Basic::Integer),
    }));
    for (atom, value, _) in listed {
        body.push(elixir::Item::Function(elixir::Function {
            doc: Doc::none(),
            name: Ident::new("value"),
            params: vec![Expr::Atom(atom.clone())],
            body: Expr::int(*value),
        }));
    }
    body.push(elixir::Item::Spec(elixir::Signature {
        doc: Doc::lines(&[
            "The enumerator a value names, or the value that names none.",
            "",
            "An @appendable writer may send an enumerator this IDL does not",
            "list, so this answers rather than raising.",
        ]),
        name: Ident::new("from_value"),
        params: vec![elixir::Param {
            name: None,
            type_: Type::Basic(Basic::Integer),
        }],
        result: Type::Union(vec![
            Type::Tuple(vec![
                Type::Atom("ok".to_string()),
                Type::Local(Ident::new("t")),
            ]),
            Type::Tuple(vec![
                Type::Atom("error".to_string()),
                Type::Tuple(vec![
                    Type::Atom("unknown_value".to_string()),
                    Type::Basic(Basic::Integer),
                ]),
            ]),
        ]),
    }));
    // One clause per distinct value. Two enumerators may share a value —
    // `@value` states it and IDL does not forbid the repetition — and a
    // second clause matching the same number could never run, which
    // Elixir reports as a warning about the clause rather than about the
    // IDL. The first is kept, which is the one a reader meets first.
    let mut seen: Vec<i128> = Vec::new();
    for (atom, value, _) in listed {
        if seen.contains(value) {
            continue;
        }
        seen.push(*value);
        body.push(elixir::Item::Function(elixir::Function {
            doc: Doc::none(),
            name: Ident::new("from_value"),
            params: vec![Expr::int(*value)],
            body: Expr::Tuple(vec![Expr::Atom("ok".to_string()), Expr::Atom(atom.clone())]),
        }));
    }
    body.push(elixir::Item::Function(elixir::Function {
        doc: Doc::none(),
        name: Ident::new("from_value"),
        params: vec![Expr::Var(Ident::new("other"))],
        body: Expr::Tuple(vec![
            Expr::Atom("error".to_string()),
            Expr::Tuple(vec![
                Expr::Atom("unknown_value".to_string()),
                Expr::Var(Ident::new("other")),
            ]),
        ]),
    }));
    body
}

/// The body of a bitmask's module.
fn bitmask_body(
    ast: &Ast,
    id: Id<TypeNode>,
    declared: &str,
    bound: u16,
    listed: &[(String, u16, i128, String)],
) -> Vec<elixir::Item> {
    let mut body = vec![
        idl_attribute_with(
            ast,
            id,
            "bitmask",
            declared,
            vec![kw("bits", Expr::int(i128::from(bound)))],
        ),
        reader("__zd_idl__", "zd_idl"),
        elixir::Item::Attribute(elixir::Attribute {
            doc: Doc::line("Every bit: the atom, its position, its value and its IDL spelling."),
            name: Ident::new("zd_bits"),
            value: Expr::List(
                listed
                    .iter()
                    .map(|(atom, position, value, idl)| {
                        Expr::Tuple(vec![
                            Expr::Atom(atom.clone()),
                            Expr::int(i128::from(*position)),
                            Expr::int(*value),
                            Expr::string(idl),
                        ])
                    })
                    .collect(),
            ),
        }),
        reader("__zd_bits__", "zd_bits"),
        type_alias(
            "t",
            Type::Basic(Basic::Integer),
            Doc::line("The holder is unsigned and @bit_bound wide; both are in @zd_idl."),
        ),
    ];
    for (atom, _, value, _) in listed {
        body.push(elixir::Item::Function(elixir::Function {
            doc: Doc::none(),
            name: Ident::new(atom),
            params: Vec::new(),
            body: Expr::int(*value),
        }));
    }
    body
}

/// The value a bit at `position` contributes.
///
/// Computed here rather than written as `1 <<< position`: Elixir's shift
/// is a macro in `Bitwise`, so the operator form would make every
/// generated module `import Bitwise` for a number this file already
/// knows.
///
/// `None` past the width of the holder. A bit bound is at most 64
/// (DDS-XTypes 1.3 §7.4.1.1.3.2), so this is a model the frontend should
/// already have refused — and a shift that overflowed to zero would give
/// two bits the same value with nothing to say so.
fn bit_value(position: u16) -> Option<i128> {
    1_i128.checked_shl(u32::from(position))
}

/// The body of a bitset's module.
fn bitset_body(
    ast: &Ast,
    id: Id<TypeNode>,
    declared: &str,
    listed: &[(String, u16, String)],
) -> Vec<elixir::Item> {
    vec![
        idl_attribute(ast, id, "bitset", declared),
        reader("__zd_idl__", "zd_idl"),
        elixir::Item::Attribute(elixir::Attribute {
            doc: Doc::line("Every bitfield: the atom, its width in bits and its IDL spelling."),
            name: Ident::new("zd_bitfields"),
            value: Expr::List(
                listed
                    .iter()
                    .map(|(atom, bits, idl)| {
                        Expr::Tuple(vec![
                            Expr::Atom(atom.clone()),
                            Expr::int(i128::from(*bits)),
                            Expr::string(idl),
                        ])
                    })
                    .collect(),
            ),
        }),
        reader("__zd_bitfields__", "zd_bitfields"),
        elixir::Item::Defstruct(
            listed
                .iter()
                .map(|(atom, _, _)| elixir::Field {
                    name: Ident::new(atom),
                    default: Expr::int(0),
                })
                .collect(),
        ),
        type_alias(
            "t",
            Type::Struct(
                listed
                    .iter()
                    .map(|(atom, _, _)| (Ident::new(atom), Type::Basic(Basic::Integer)))
                    .collect(),
            ),
            Doc::none(),
        ),
    ]
}

/// The constants of one IDL scope, while they are being gathered.
struct ConstantGroup {
    /// The scope they were declared in.
    module: Id<ModuleNode>,
    /// The attribute and the function of each, in declaration order.
    items: Vec<elixir::Item>,
    /// What `@zd_constants` says about each.
    listed: Vec<(Ident, Expr)>,
    /// The Elixir and IDL spelling of each, for the collision check.
    names: Vec<(String, String)>,
}

/// The constants of the model, gathered per IDL scope.
///
/// One module per scope and not one per constant: `const long N` becomes
/// an attribute and the function that reads it, and a module for each
/// would make `Common.N.value()` the way to reach a number.
fn declare_constants(
    ast: &Ast,
    applied: &Applied,
    out: &mut Vec<elixir::Module>,
    emitted: &mut Vec<(String, String)>,
) -> Result<(), MaterialiseError> {
    // Grouped in first-seen order, so the output tracks the source.
    let mut groups: Vec<ConstantGroup> = Vec::new();
    for (_, node) in ast.constants() {
        if ghost_created_for(applied, node.node_id, "ElixirConst").is_none() {
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
        // `attribute_name` and not `atom_name`: a constant becomes a
        // module attribute, and Elixir reserves two dozen of those.
        let atom = elixir::attribute_name(&node.name);
        let declared = node.type_.resolved();
        let mut facts = vec![kw("idl", Expr::string(&node.name))];
        match declared {
            Some(target) => facts.extend(type_facts(ast, target, &node.name, 0)?),
            None => facts.extend(primitive_facts(PrimitiveKind::Int32)),
        }
        let literal = elixir_constant(ast, &value, declared, &node.name)?;
        let at = match groups.iter().position(|group| group.module == node.module) {
            Some(at) => at,
            None => {
                groups.push(ConstantGroup {
                    module: node.module,
                    items: Vec::new(),
                    listed: Vec::new(),
                    names: Vec::new(),
                });
                groups.len() - 1
            }
        };
        let Some(group) = groups.get_mut(at) else {
            continue;
        };
        group.items.push(elixir::Item::Attribute(elixir::Attribute {
            doc: idl_note(&atom, &node.name),
            name: Ident::new(&atom),
            value: literal,
        }));
        // The function is what makes the attribute reachable: an
        // attribute alone is a compile-time binding and is gone from the
        // compiled module, so nothing outside could read it.
        group.items.push(elixir::Item::Function(elixir::Function {
            doc: Doc::none(),
            name: Ident::new(&atom),
            params: Vec::new(),
            body: Expr::Attr(Ident::new(&atom)),
        }));
        group.listed.push((Ident::new(&atom), Expr::Keyword(facts)));
        group.names.push((atom, node.name.clone()));
    }

    for group in groups {
        no_spelling_collision(
            group
                .names
                .iter()
                .map(|(spelled, declared)| (spelled.as_str(), declared.as_str())),
            LANGUAGE,
            CONSTANTS_MODULE,
        )?;
        let name = module_path(ast, group.module)?.join(CONSTANTS_MODULE);
        remember(emitted, &name, CONSTANTS_MODULE)?;
        let mut body = vec![
            elixir::Item::Attribute(elixir::Attribute {
                doc: Doc::line("Every constant: the function's name and what it holds."),
                name: Ident::new("zd_constants"),
                value: Expr::Keyword(group.listed),
            }),
            reader("__zd_constants__", "zd_constants"),
        ];
        body.extend(group.items);
        out.push(elixir::Module {
            doc: Doc::lines(&[
                "The constants one IDL scope declares.",
                "",
                "Each is a module attribute and the zero-arity function that reads",
                "it. The attribute alone would not do: it is a compile-time binding",
                "and is gone from the compiled module, so nothing outside this file",
                "could reach it.",
                "",
                "The module's name is this column's own — Elixir has no file-scope",
                "binding, so a constant needs a module to live in — and it goes",
                "through the same collision check as a declared one.",
            ]),
            name,
            body,
        });
    }
    Ok(())
}

/// The exceptions of the model, each as a struct.
fn declare_exceptions(
    ast: &Ast,
    applied: &Applied,
    out: &mut Vec<elixir::Module>,
    emitted: &mut Vec<(String, String)>,
) -> Result<(), MaterialiseError> {
    for (_, node) in ast.exceptions() {
        if ghost_created_for(applied, node.node_id, "ElixirError").is_none() {
            continue;
        }
        let mut name = module_path(ast, node.module)?;
        name = name.join(&elixir::module_segment(&node.name));
        remember(emitted, &name, &node.name)?;
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
            let optional = member_node.facets.is_optional();
            fields.push(member_field(
                ast,
                target,
                &member_node.name,
                optional,
                optional,
                Vec::new(),
            )?);
        }
        check_fields(&fields, &node.name)?;
        let mut body = vec![
            declared_idl_attribute(ast, node.module, "exception", &node.name)?,
            reader("__zd_idl__", "zd_idl"),
        ];
        body.extend(members_body(&fields));
        out.push(elixir::Module {
            doc: Doc::lines(&[
                "An exception, as the structure its members make.",
                "",
                "A struct and not a defexception: Elixir's defexception requires",
                "either a :message field or a message/1 callback, and an IDL",
                "exception has neither — its members are whatever the IDL declared.",
                "Raising is a run-time decision the RPC arc makes; the shape is the",
                "IDL's.",
            ]),
            name,
            body,
        });
    }
    Ok(())
}

/// The interfaces of the model, each as a behaviour.
fn declare_interfaces(
    ast: &Ast,
    applied: &Applied,
    out: &mut Vec<elixir::Module>,
    emitted: &mut Vec<(String, String)>,
) -> Result<(), MaterialiseError> {
    for (id, node) in ast.interfaces() {
        if ghost_created_for(applied, node.node_id, "ElixirBehaviour").is_none() {
            continue;
        }
        let mut name = module_path(ast, node.module)?;
        name = name.join(&elixir::module_segment(&node.name));
        remember(emitted, &name, &node.name)?;
        let mut body = vec![
            declared_idl_attribute(ast, node.module, "interface", &node.name)?,
            reader("__zd_idl__", "zd_idl"),
        ];
        let mut signatures: Vec<(String, String)> = Vec::new();
        // The whole answer and not the declared one. An Elixir behaviour
        // is a flat list of `@callback` declarations — a behaviour does
        // not extend another — so a derived interface's module has to
        // carry OMG IDL 4.2 §7.4.3's inherited operations and
        // attributes itself. `interface::all_operations_flattened`
        // gives both — the get_/set_ pair DDS-RPC 1.0 §7.5.1.1.3
        // implies for each attribute, already spelled the way this
        // column's own snake_case atoms would (`get_memory`), besides
        // the declared operations — which `Ast::all_operations_of`
        // alone never did.
        let operations = interface::all_operations_flattened(ast, id)
            .map_err(|error| interface::interface_error_to_materialise(error, &node.name))?;
        for op in &operations {
            signatures.push((elixir::atom_name(&op.name), op.name.clone()));
            let mut params = Vec::new();
            for param in &op.params {
                params.push(elixir::Param {
                    name: Some(Ident::new(&elixir::atom_name(&param.name))),
                    type_: elixir_member_type(ast, param.type_, &param.name)?,
                });
            }
            let result = match op.returns {
                Some(returns) => elixir_member_type(ast, returns, &op.name)?,
                // `:ok` and not `nil`: an operation returning `void` has
                // completed, and `nil` is the value an absent optional
                // member takes — one atom for two states would make a
                // callback's result indistinguishable from a missing one.
                None => Type::Atom("ok".to_string()),
            };
            let mut doc = idl_note(&elixir::atom_name(&op.name), &op.name);
            if let Some(line) = op.raises_line() {
                doc.push(&line);
            }
            body.push(elixir::Item::Callback(elixir::Signature {
                doc,
                name: Ident::new(&elixir::atom_name(&op.name)),
                params,
                result,
            }));
        }
        no_spelling_collision(
            signatures
                .iter()
                .map(|(spelled, declared)| (spelled.as_str(), declared.as_str())),
            LANGUAGE,
            &node.name,
        )?;
        out.push(elixir::Module {
            doc: Doc::lines(&[
                "An interface, as the behaviour its operations and attributes",
                "declare — an attribute a get_/set_ pair of callbacks, `readonly`",
                "only the first.",
                "",
                "A module of @callback declarations is what an Elixir behaviour is;",
                "an implementing module writes @behaviour and defines them. The",
                "transport is the RPC arc's business; the shape is the IDL's.",
                "`raises`/`getraises`/`setraises` are documented on the callback",
                "they belong to.",
                "",
                "A derived interface carries its bases' operations and attributes",
                "too: a behaviour does not extend another, so IDL 4.2 7.4.3's",
                "inherited members are declared here rather than reached through",
                "one.",
            ]),
            name,
            body,
        });
    }
    Ok(())
}

/// The attributes, struct, type and readers a plain aggregate takes.
fn members_body(fields: &[Member]) -> Vec<elixir::Item> {
    let mut body = vec![
        elixir::Item::Attribute(elixir::Attribute {
            doc: Doc::line("Every member, in declaration order, with what Elixir cannot state."),
            name: Ident::new("zd_members"),
            value: Expr::List(
                fields
                    .iter()
                    .filter(|field| field.listed)
                    .map(|field| {
                        Expr::Tuple(vec![
                            Expr::Atom(field.atom.clone()),
                            Expr::Keyword(field.facts.clone()),
                        ])
                    })
                    .collect(),
            ),
        }),
        reader("__zd_members__", "zd_members"),
    ];
    let enforced: Vec<Expr> = fields
        .iter()
        .filter(|field| field.enforced)
        .map(|field| Expr::Atom(field.atom.clone()))
        .collect();
    if !enforced.is_empty() {
        body.push(elixir::Item::Attribute(elixir::Attribute {
            doc: Doc::none(),
            name: Ident::new("enforce_keys"),
            value: Expr::List(enforced),
        }));
    }
    body.push(elixir::Item::Defstruct(
        fields
            .iter()
            .map(|field| elixir::Field {
                name: Ident::new(&field.atom),
                default: field.default.clone(),
            })
            .collect(),
    ));
    body.push(type_alias(
        "t",
        Type::Struct(
            fields
                .iter()
                .map(|field| (Ident::new(&field.atom), field.type_.clone()))
                .collect(),
        ),
        Doc::none(),
    ));
    body
}

/// The labels a union case selects, as the attribute states them.
///
/// A `default:` label is the atom `:default`: it selects every value the
/// other cases do not claim, and a number could not say that.
fn case_labels(
    ast: &Ast,
    labels: &[CaseLabel],
    at: &str,
) -> Result<Vec<(Ident, Expr)>, MaterialiseError> {
    let mut out = Vec::new();
    for label in labels {
        match label {
            CaseLabel::Default => out.push(Expr::Atom("default".to_string())),
            CaseLabel::Value(expr) => {
                let value =
                    ast.eval_integral(*expr)
                        .map_err(|_| MaterialiseError::Unevaluated {
                            construct: "union case label".to_string(),
                            at: at.to_string(),
                        })?;
                out.push(Expr::int(value));
            }
        }
    }
    Ok(vec![kw("labels", Expr::List(out))])
}

// -- types ----------------------------------------------------------------

/// The Elixir spelling of a value's type, refusing what Elixir cannot
/// hold.
///
/// `any` is the first of those. It is in the model because Building
/// Block Any is one of IDL 4.2's fifteen, and what it holds is decided at
/// run time. Elixir's own `any()` would satisfy Dialyzer and there would
/// be no wire form to generate a codec against, which is the
/// silent-substitution class this project is arranged against — and it
/// is a harder refusal here than in the C family, because `any()` is a
/// type a checker accepts everywhere.
///
/// # Errors
/// [`MaterialiseError::NoLanguageForm`] for `any`, `ValueBase`, an object
/// reference and an annotation type; [`MaterialiseError::Unevaluated`]
/// for a length or an element the model will not answer for.
pub fn elixir_member_type(
    ast: &Ast,
    target: Id<TypeNode>,
    at: &str,
) -> Result<Type, MaterialiseError> {
    let construct = match ast.type_(target).map(|node| &node.kind) {
        Ok(TypeKind::Any) => Some("any"),
        Ok(TypeKind::ValueBase) => Some("ValueBase"),
        Ok(TypeKind::ObjectRef { .. }) => Some("Object"),
        Ok(TypeKind::AnnotationType { .. }) => Some("an annotation type"),
        _ => None,
    };
    if let Some(construct) = construct {
        return Err(MaterialiseError::NoLanguageForm {
            construct: construct.to_string(),
            language: LANGUAGE,
            at: at.to_string(),
        });
    }
    elixir_value_type(ast, target, 0)
}

/// The Elixir type a value of `id` is held in.
///
/// The one answer, for every emitter. A field of a generated struct, a
/// codec's parameter and a defaulter's destination all have to name the
/// same type or the module does not compile.
///
/// # Errors
/// [`MaterialiseError::Unevaluated`] when an array length or a
/// collection's element does not resolve.
///
/// zerodds-lint: recursion-depth 64 (nested collections; bounded by `MAX_TYPE_DEPTH`)
pub fn elixir_value_type(
    ast: &Ast,
    id: Id<TypeNode>,
    depth: usize,
) -> Result<Type, MaterialiseError> {
    if depth > MAX_TYPE_DEPTH {
        return Err(MaterialiseError::Unevaluated {
            construct: format!("a type nested past {MAX_TYPE_DEPTH} levels"),
            at: "the member's type".to_string(),
        });
    }
    let Ok(node) = ast.type_(id) else {
        return Ok(Type::Basic(Basic::Any));
    };
    Ok(match &node.kind {
        TypeKind::Primitive(kind) => elixir_primitive_type(*kind),
        // A `wstring` holds UTF-8 rather than UTF-16 units, which puts
        // the §7.4.1.1.2 surrogate rule inside the runtime where it is a
        // second implementation of the clause and not a copy of the Rust
        // column's answer. An Elixir binary is a run of octets with no
        // encoding of its own, which is what makes that possible.
        TypeKind::String { .. } | TypeKind::Fixed { .. } => Type::Basic(Basic::Binary),
        TypeKind::Sequence { element, .. } => {
            let Some(element) = element.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "sequence element".to_string(),
                    at: named(&node.name, node.node_id),
                });
            };
            Type::list(elixir_value_type(ast, element, depth + 1)?)
        }
        TypeKind::Array { element, dims } => {
            let Some(element) = element.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "array element".to_string(),
                    at: named(&node.name, node.node_id),
                });
            };
            // Outermost dimension outermost: `long grid[2][3]` is two
            // rows of three, so the outer tuple has two elements and
            // each has three. Building it from the inside out is what
            // keeps that true.
            let mut type_ = elixir_value_type(ast, element, depth + 1)?;
            for dim in dims.iter().rev() {
                let length =
                    ast.eval_integral(*dim)
                        .map_err(|_| MaterialiseError::Unevaluated {
                            construct: "array length".to_string(),
                            at: named(&node.name, node.node_id),
                        })?;
                let count = usize::try_from(length).map_err(|_| MaterialiseError::Unevaluated {
                    construct: "array length as a tuple arity".to_string(),
                    at: named(&node.name, node.node_id),
                })?;
                type_ = Type::Tuple(alloc_repeat(&type_, count));
            }
            type_
        }
        TypeKind::Map { key, value, .. } => {
            let (Some(key), Some(value)) = (key.resolved(), value.resolved()) else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "map key or value".to_string(),
                    at: named(&node.name, node.node_id),
                });
            };
            Type::list(Type::Tuple(vec![
                elixir_value_type(ast, key, depth + 1)?,
                elixir_value_type(ast, value, depth + 1)?,
            ]))
        }
        // A typedef with no name is one the frontend made for an inline
        // type; it is transparent, so the Elixir type is the target's.
        TypeKind::Alias { aliased } if node.name.is_empty() => match aliased.resolved() {
            Some(target) => elixir_value_type(ast, target, depth + 1)?,
            None => Type::Basic(Basic::Any),
        },
        // An enumeration and a bitmask are integers in member position;
        // the module documentation carries why.
        TypeKind::Enumeration { .. } | TypeKind::Bitmask { .. } => Type::Basic(Basic::Integer),
        _ => Type::Remote {
            module: type_module_path(ast, id),
            name: Ident::new("t"),
        },
    })
}

/// The Elixir type an IDL primitive becomes.
///
/// Exhaustive, so a primitive added to the model without an Elixir
/// spelling is a compile error and not a member that silently takes
/// another's.
///
/// Every one of the ten integers is `integer()`, and only the attribute
/// differs. That is where Elixir's lack of widths is an advantage rather
/// than the cost it is everywhere else: the Java column has to decide
/// which of its signed types is wide enough to hold an unsigned IDL
/// value, and an `unsigned long long` ends up a `long` carrying a bit
/// pattern rather than a number. Elixir's integers are arbitrary
/// precision, so nothing is folded.
#[must_use]
pub fn elixir_primitive_type(kind: PrimitiveKind) -> Type {
    Type::Basic(match kind {
        PrimitiveKind::Boolean => Basic::Boolean,
        PrimitiveKind::Octet
        | PrimitiveKind::UInt8
        | PrimitiveKind::Int8
        | PrimitiveKind::Char
        | PrimitiveKind::WChar
        | PrimitiveKind::Int16
        | PrimitiveKind::UInt16
        | PrimitiveKind::Int32
        | PrimitiveKind::UInt32
        | PrimitiveKind::Int64
        | PrimitiveKind::UInt64 => Basic::Integer,
        PrimitiveKind::Float32 | PrimitiveKind::Float64 => Basic::Float,
        // The decimal text. An Elixir float() is an IEEE binary64 with
        // 53 bits of significand and a `long double` is a binary128 with
        // 113; the assignment would lose sixty bits before any codec
        // ran.
        PrimitiveKind::Float128 => Basic::Binary,
    })
}

/// A type spelled by a name a facet gave.
///
/// `MyMod.t` becomes a remote type and `pid` a local one, because those
/// are the two forms an Elixir type name takes. The spelling is the
/// facet's own: this column does not know what a `#pragma` meant, and
/// inventing a type for it would be worse than writing what it said.
fn named_type(spelling: &str) -> Type {
    match spelling.rsplit_once('.') {
        Some((module, name)) => Type::Remote {
            module: Path::new(&module.split('.').collect::<Vec<&str>>()),
            name: Ident::new(name),
        },
        None => Type::Local(Ident::new(spelling)),
    }
}

/// `count` copies of a type, for a tuple's arity.
fn alloc_repeat(type_: &Type, count: usize) -> Vec<Type> {
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        out.push(type_.clone());
    }
    out
}

// -- the attribute --------------------------------------------------------

/// What `@zd_members` says about one type.
///
/// Every fact Elixir's own type system cannot carry: the kind, the width
/// in bits, the signedness, the bound, the digit count and the module a
/// named type resolves to. Nested, for a collection, so that
/// `sequence<string<7>>` states the bound of the element and not only of
/// the sequence.
///
/// # Errors
/// [`MaterialiseError::Unevaluated`] for a bound or a length that does
/// not evaluate; [`MaterialiseError::NoLanguageForm`] for a construct
/// with no Elixir form.
///
/// zerodds-lint: recursion-depth 64 (nested collections; bounded by `MAX_TYPE_DEPTH`)
#[allow(clippy::too_many_lines)]
pub fn type_facts(
    ast: &Ast,
    id: Id<TypeNode>,
    at: &str,
    depth: usize,
) -> Result<Vec<(Ident, Expr)>, MaterialiseError> {
    if depth > MAX_TYPE_DEPTH {
        return Err(MaterialiseError::Unevaluated {
            construct: format!("a type nested past {MAX_TYPE_DEPTH} levels"),
            at: at.to_string(),
        });
    }
    let Ok(node) = ast.type_(id) else {
        return Err(MaterialiseError::Unevaluated {
            construct: "a member's type the model does not answer for".to_string(),
            at: at.to_string(),
        });
    };
    Ok(match &node.kind {
        TypeKind::Primitive(kind) => primitive_facts(*kind),
        TypeKind::String { width, bound } => vec![
            kw(
                "kind",
                Expr::Atom(
                    match width {
                        CharWidth::Char => "string",
                        CharWidth::WChar => "wstring",
                    }
                    .to_string(),
                ),
            ),
            kw("bits", Expr::int(char_bits(*width))),
            kw("bound", Expr::int(bound_of(ast, *bound, at)?)),
        ],
        TypeKind::Fixed { digits, scale } => vec![
            kw("kind", Expr::Atom("fixed".to_string())),
            kw("digits", Expr::int(i128::from(*digits))),
            kw("scale", Expr::int(i128::from(*scale))),
            kw("holder", Expr::Atom("decimal_text".to_string())),
        ],
        TypeKind::Sequence { element, bound } => {
            let Some(element) = element.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "sequence element".to_string(),
                    at: at.to_string(),
                });
            };
            vec![
                kw("kind", Expr::Atom("sequence".to_string())),
                kw("bound", Expr::int(bound_of(ast, *bound, at)?)),
                kw(
                    "element",
                    Expr::Keyword(type_facts(ast, element, at, depth + 1)?),
                ),
            ]
        }
        TypeKind::Array { element, dims } => {
            let Some(element) = element.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "array element".to_string(),
                    at: at.to_string(),
                });
            };
            let mut lengths = Vec::new();
            for dim in dims {
                lengths.push(Expr::int(ast.eval_integral(*dim).map_err(|_| {
                    MaterialiseError::Unevaluated {
                        construct: "array length".to_string(),
                        at: at.to_string(),
                    }
                })?));
            }
            vec![
                kw("kind", Expr::Atom("array".to_string())),
                kw("dims", Expr::List(lengths)),
                kw(
                    "element",
                    Expr::Keyword(type_facts(ast, element, at, depth + 1)?),
                ),
            ]
        }
        TypeKind::Map { key, value, bound } => {
            let (Some(key), Some(value)) = (key.resolved(), value.resolved()) else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "map key or value".to_string(),
                    at: at.to_string(),
                });
            };
            vec![
                kw("kind", Expr::Atom("map".to_string())),
                kw("bound", Expr::int(bound_of(ast, *bound, at)?)),
                kw("key", Expr::Keyword(type_facts(ast, key, at, depth + 1)?)),
                kw(
                    "value",
                    Expr::Keyword(type_facts(ast, value, at, depth + 1)?),
                ),
            ]
        }
        TypeKind::Enumeration { .. } => vec![
            kw("kind", Expr::Atom("enum".to_string())),
            kw("module", Expr::Module(type_module_path(ast, id))),
            kw("bits", Expr::int(i128::from(ast.effective_bit_bound(id)))),
        ],
        TypeKind::Bitmask { .. } => vec![
            kw("kind", Expr::Atom("bitmask".to_string())),
            kw("module", Expr::Module(type_module_path(ast, id))),
            kw("bits", Expr::int(i128::from(ast.effective_bit_bound(id)))),
            kw("signed", Expr::Literal(Literal::Bool(false))),
        ],
        // A named typedef states both: which alias the member was
        // written with, and what it resolves to. A codec needs the
        // second and a reader wants the first.
        TypeKind::Alias { aliased } => {
            let Some(target) = aliased.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "typedef target".to_string(),
                    at: at.to_string(),
                });
            };
            if node.name.is_empty() {
                return type_facts(ast, target, at, depth + 1);
            }
            vec![
                kw("kind", Expr::Atom("alias".to_string())),
                kw("module", Expr::Module(type_module_path(ast, id))),
                kw(
                    "target",
                    Expr::Keyword(type_facts(ast, target, at, depth + 1)?),
                ),
            ]
        }
        TypeKind::Structure { .. } => named_facts(ast, id, "struct"),
        TypeKind::Union { .. } => named_facts(ast, id, "union"),
        TypeKind::Valuetype { .. } => named_facts(ast, id, "valuetype"),
        TypeKind::Bitset { .. } => named_facts(ast, id, "bitset"),
        TypeKind::Native => named_facts(ast, id, "native"),
        TypeKind::ValueBox { .. } => named_facts(ast, id, "valuebox"),
        TypeKind::Any
        | TypeKind::ValueBase
        | TypeKind::ObjectRef { .. }
        | TypeKind::AnnotationType { .. } => {
            return Err(MaterialiseError::NoLanguageForm {
                construct: "a construct with no Elixir data form".to_string(),
                language: LANGUAGE,
                at: at.to_string(),
            });
        }
    })
}

/// The facts a member holding a declared type carries.
fn named_facts(ast: &Ast, id: Id<TypeNode>, kind: &str) -> Vec<(Ident, Expr)> {
    vec![
        kw("kind", Expr::Atom(kind.to_string())),
        kw("module", Expr::Module(type_module_path(ast, id))),
    ]
}

/// What `@zd_members` says about one primitive.
fn primitive_facts(kind: PrimitiveKind) -> Vec<(Ident, Expr)> {
    let signed = |name: &str, bits: i128, signed: bool| {
        vec![
            kw("kind", Expr::Atom(name.to_string())),
            kw("bits", Expr::int(bits)),
            kw("signed", Expr::Literal(Literal::Bool(signed))),
        ]
    };
    match kind {
        // A boolean is one octet on the wire and has no signedness.
        PrimitiveKind::Boolean => vec![
            kw("kind", Expr::Atom("boolean".to_string())),
            kw("bits", Expr::int(8)),
        ],
        PrimitiveKind::Octet => signed("octet", 8, false),
        PrimitiveKind::UInt8 => signed("uint8", 8, false),
        PrimitiveKind::Int8 => signed("int8", 8, true),
        // A character is a code unit of a stated width. DDS-XTypes 1.3
        // §7.5.1.1.1 leaves a binding's spelling of Char16 open, and the
        // width is the part that decides what goes on the wire.
        PrimitiveKind::Char => vec![
            kw("kind", Expr::Atom("char".to_string())),
            kw("bits", Expr::int(8)),
        ],
        PrimitiveKind::WChar => vec![
            kw("kind", Expr::Atom("wchar".to_string())),
            kw("bits", Expr::int(16)),
        ],
        PrimitiveKind::Int16 => signed("int16", 16, true),
        PrimitiveKind::UInt16 => signed("uint16", 16, false),
        PrimitiveKind::Int32 => signed("int32", 32, true),
        PrimitiveKind::UInt32 => signed("uint32", 32, false),
        PrimitiveKind::Int64 => signed("int64", 64, true),
        PrimitiveKind::UInt64 => signed("uint64", 64, false),
        PrimitiveKind::Float32 => vec![
            kw("kind", Expr::Atom("float32".to_string())),
            kw("bits", Expr::int(32)),
        ],
        PrimitiveKind::Float64 => vec![
            kw("kind", Expr::Atom("float64".to_string())),
            kw("bits", Expr::int(64)),
        ],
        PrimitiveKind::Float128 => vec![
            kw("kind", Expr::Atom("float128".to_string())),
            kw("bits", Expr::int(128)),
            kw("holder", Expr::Atom("decimal_text".to_string())),
        ],
    }
}

/// How wide one character of `width` is on the wire.
const fn char_bits(width: CharWidth) -> i128 {
    match width {
        CharWidth::Char => 8,
        CharWidth::WChar => 16,
    }
}

/// A bound as a number, with `0` meaning unbounded.
///
/// # Errors
/// [`MaterialiseError::Unevaluated`] when the bounding expression does
/// not evaluate. Zero is what unbounded is written as, so a bound that
/// failed to evaluate must not fall back to it: the two would be
/// indistinguishable in the output.
fn bound_of(ast: &Ast, bound: Bound, at: &str) -> Result<i128, MaterialiseError> {
    match bound.expr() {
        None => Ok(0),
        Some(expr) => ast
            .eval_integral(expr)
            .map_err(|_| MaterialiseError::Unevaluated {
                construct: "bound".to_string(),
                at: at.to_string(),
            }),
    }
}

// -- defaults -------------------------------------------------------------

/// The value a field of this type takes in the `defstruct`.
///
/// `None` where the type has no value that could stand as one — a nested
/// struct's type is `%Other{}` and `nil` is not one of its values. The
/// field is then named in `@enforce_keys`, which makes `%Struct{}` a
/// compile error: a caller has to supply it, exactly as the Python
/// column's dataclass field with no default does.
///
/// Every value returned here is a value of the field's declared type, so
/// the `defstruct` and the `@type` agree — which matters because it is
/// the pair Dialyzer reads together.
///
/// # Errors
/// [`MaterialiseError::Unevaluated`] when an array length does not
/// evaluate.
///
/// zerodds-lint: recursion-depth 64 (nested collections; bounded by `MAX_TYPE_DEPTH`)
fn default_for(
    ast: &Ast,
    id: Id<TypeNode>,
    depth: usize,
) -> Result<Option<Expr>, MaterialiseError> {
    if depth > MAX_TYPE_DEPTH {
        return Ok(None);
    }
    let Ok(node) = ast.type_(id) else {
        return Ok(None);
    };
    Ok(match &node.kind {
        TypeKind::Primitive(kind) => Some(match kind {
            PrimitiveKind::Boolean => Expr::Literal(Literal::Bool(false)),
            PrimitiveKind::Float32 | PrimitiveKind::Float64 => {
                Expr::Literal(Literal::Float("0.0".to_string()))
            }
            // The decimal text of zero, because the holder is a binary.
            PrimitiveKind::Float128 => Expr::string("0"),
            _ => Expr::int(0),
        }),
        // An empty binary, which is the empty string and the zero-length
        // `fixed` alike.
        TypeKind::String { .. } | TypeKind::Fixed { .. } => Some(Expr::string("")),
        // An empty list, for both. A `map<K,V>` is a list of pairs here.
        TypeKind::Sequence { .. } | TypeKind::Map { .. } => Some(Expr::List(Vec::new())),
        TypeKind::Array { element, dims } => {
            let Some(element) = element.resolved() else {
                return Ok(None);
            };
            let Some(inner) = default_for(ast, element, depth + 1)? else {
                return Ok(None);
            };
            let mut value = inner;
            for dim in dims.iter().rev() {
                let length =
                    ast.eval_integral(*dim)
                        .map_err(|_| MaterialiseError::Unevaluated {
                            construct: "array length".to_string(),
                            at: named(&node.name, node.node_id),
                        })?;
                let count = usize::try_from(length).map_err(|_| MaterialiseError::Unevaluated {
                    construct: "array length as a tuple arity".to_string(),
                    at: named(&node.name, node.node_id),
                })?;
                let mut items = Vec::with_capacity(count);
                for _ in 0..count {
                    items.push(value.clone());
                }
                value = Expr::Tuple(items);
            }
            Some(value)
        }
        // Both are integers in member position, and zero is an integer
        // whether or not it is an enumerator this IDL names.
        TypeKind::Enumeration { .. } | TypeKind::Bitmask { .. } => Some(Expr::int(0)),
        TypeKind::Alias { aliased } => match aliased.resolved() {
            Some(target) => default_for(ast, target, depth + 1)?,
            None => None,
        },
        // A struct, a union, a bitset, a valuetype, a boxed value, a
        // native type and everything refused: no value of the declared
        // type can be written here without constructing one, and
        // constructing one at compile time would need the module it
        // names to be compiled first.
        _ => None,
    })
}

// -- names ----------------------------------------------------------------

/// A declared type's Elixir module name.
///
/// The scoped name, each segment raised to an upper-case initial:
/// `::common::util::Location` is `Common.Util.Location`. Nothing is
/// flattened, because Elixir module names nest.
#[must_use]
pub fn type_module_path(ast: &Ast, id: Id<TypeNode>) -> Path {
    let Ok(any) = ast.any_of_type(id) else {
        return Path::single("ZdUnnamed");
    };
    let Some(scoped) = ast.scoped_name_of(any) else {
        return Path::single("ZdUnnamed");
    };
    let segments: Vec<String> = scoped
        .segments()
        .iter()
        .map(|segment| elixir::module_segment(segment))
        .collect();
    if segments.is_empty() {
        return Path::single("ZdUnnamed");
    }
    Path::new(&segments.iter().map(String::as_str).collect::<Vec<&str>>())
}

/// The module path an IDL module names, outermost segment first.
///
/// # Errors
/// [`MaterialiseError::NoModule`] when the model does not answer for a
/// module in the chain, or the chain runs past [`MAX_MODULE_DEPTH`].
/// Returning the declaration unscoped instead would put it in another
/// scope and say nothing, which is the same class of substitution as
/// writing the wrong octet.
fn module_path(ast: &Ast, module: Id<ModuleNode>) -> Result<Path, MaterialiseError> {
    let mut segments: Vec<String> = Vec::new();
    let mut current = Some(module);
    let mut depth = 0;
    while let Some(id) = current {
        if depth > MAX_MODULE_DEPTH {
            return Err(MaterialiseError::NoModule {
                at: module_handle(module),
            });
        }
        depth += 1;
        let node = ast.module(id).map_err(|_| MaterialiseError::NoModule {
            at: module_handle(id),
        })?;
        // The root is the anonymous global scope and names no segment.
        if node.parent.is_none() || node.name.is_empty() {
            break;
        }
        segments.push(elixir::module_segment(&node.name));
        current = node.parent;
    }
    segments.reverse();
    Ok(Path::new(
        &segments.iter().map(String::as_str).collect::<Vec<&str>>(),
    ))
}

/// The module handle, named without the node the model would not answer
/// for. Slot and generation are what the arena was asked for.
fn module_handle(module: Id<ModuleNode>) -> String {
    format!("#{}@{}", module.slot(), module.generation())
}

/// A module name as one string, for the collision check and a
/// diagnostic.
fn dotted(path: &Path) -> String {
    path.segments()
        .iter()
        .map(Ident::as_str)
        .collect::<Vec<&str>>()
        .join(".")
}

/// Record a declared module name, refusing a second declaration that
/// reaches the same one.
///
/// A whole-file check, and it has to be: Elixir does not declare a
/// module, it defines one, and a second `defmodule` of the same name
/// replaces the first. With `--ignore-module-conflict` — which a
/// generated file is compiled with, because one file may legitimately be
/// compiled twice — that happens with no diagnostic at all.
///
/// Two IDL scopes can reach one Elixir name: `module elixir { struct S; }`
/// and a top-level `struct S` both name `Elixir.S`, and `a_b::C` and
/// `a::b_C` do not (the segments stay apart), which is one hazard fewer
/// than the Go column carries.
fn remember(
    emitted: &mut Vec<(String, String)>,
    name: &Path,
    declared: &str,
) -> Result<(), MaterialiseError> {
    let spelled = dotted(name);
    if let Some((_, first)) = emitted.iter().find(|(other, _)| *other == spelled) {
        return Err(MaterialiseError::SpellingCollision {
            first: first.clone(),
            second: declared.to_string(),
            spelled,
            language: LANGUAGE,
            at: "the generated file".to_string(),
        });
    }
    emitted.push((spelled, declared.to_string()));
    Ok(())
}

/// A model node's name, or its identity when the node is anonymous.
fn named(name: &str, node: NodeId) -> String {
    if name.is_empty() {
        return format!("an unnamed type (#{node:?})");
    }
    name.to_string()
}

// -- small builders -------------------------------------------------------

/// `key: value`, as a keyword-list entry.
fn kw(key: &str, value: Expr) -> (Ident, Expr) {
    (Ident::new(key), value)
}

/// `@zd_idl [name: …, scoped: …, kind: …]`.
fn idl_attribute(ast: &Ast, id: Id<TypeNode>, kind: &str, declared: &str) -> elixir::Item {
    idl_attribute_with(ast, id, kind, declared, Vec::new())
}

/// The same, with more facts about the declaration itself.
fn idl_attribute_with(
    ast: &Ast,
    id: Id<TypeNode>,
    kind: &str,
    declared: &str,
    extra: Vec<(Ident, Expr)>,
) -> elixir::Item {
    let mut facts = vec![
        kw("name", Expr::string(declared)),
        kw("scoped", Expr::string(&scoped_idl_name(ast, id, declared))),
        kw("kind", Expr::Atom(kind.to_string())),
    ];
    facts.extend(extra);
    elixir::Item::Attribute(elixir::Attribute {
        doc: Doc::line(
            "The declaration's own IDL spelling, which the module name does not invert.",
        ),
        name: Ident::new("zd_idl"),
        value: Expr::Keyword(facts),
    })
}

/// `@zd_idl` for a declaration outside the type arena.
///
/// An exception and an interface are not types, so their scope comes
/// from the module they were declared in rather than from
/// [`Ast::scoped_name_of`].
///
/// # Errors
/// [`MaterialiseError::NoModule`] when the model does not answer for a
/// module in the chain.
fn declared_idl_attribute(
    ast: &Ast,
    module: Id<ModuleNode>,
    kind: &str,
    declared: &str,
) -> Result<elixir::Item, MaterialiseError> {
    let mut scoped = String::from("::");
    for segment in idl_module_chain(ast, module)? {
        scoped.push_str(&segment);
        scoped.push_str("::");
    }
    scoped.push_str(declared);
    Ok(elixir::Item::Attribute(elixir::Attribute {
        doc: Doc::line(
            "The declaration's own IDL spelling, which the module name does not invert.",
        ),
        name: Ident::new("zd_idl"),
        value: Expr::Keyword(vec![
            kw("name", Expr::string(declared)),
            kw("scoped", Expr::string(&scoped)),
            kw("kind", Expr::Atom(kind.to_string())),
        ]),
    }))
}

/// The IDL names of the modules a declaration nests in, outermost first.
///
/// The IDL spellings and not the Elixir ones: this is what the source
/// wrote, and it is the half that does not invert.
///
/// # Errors
/// [`MaterialiseError::NoModule`] as [`module_path`].
fn idl_module_chain(ast: &Ast, module: Id<ModuleNode>) -> Result<Vec<String>, MaterialiseError> {
    let mut segments: Vec<String> = Vec::new();
    let mut current = Some(module);
    let mut depth = 0;
    while let Some(id) = current {
        if depth > MAX_MODULE_DEPTH {
            return Err(MaterialiseError::NoModule {
                at: module_handle(module),
            });
        }
        depth += 1;
        let node = ast.module(id).map_err(|_| MaterialiseError::NoModule {
            at: module_handle(id),
        })?;
        if node.parent.is_none() || node.name.is_empty() {
            break;
        }
        segments.push(node.name.clone());
        current = node.parent;
    }
    segments.reverse();
    Ok(segments)
}

/// The IDL scoped name of a declared type, as the source wrote it.
fn scoped_idl_name(ast: &Ast, id: Id<TypeNode>, declared: &str) -> String {
    let Ok(any) = ast.any_of_type(id) else {
        return declared.to_string();
    };
    let Some(scoped) = ast.scoped_name_of(any) else {
        return declared.to_string();
    };
    let mut out = String::from("::");
    out.push_str(&scoped.segments().join("::"));
    out
}

/// `def __zd_x__, do: @zd_x`.
fn reader(function: &str, attribute: &str) -> elixir::Item {
    elixir::Item::Function(elixir::Function {
        doc: Doc::none(),
        name: Ident::new(function),
        params: Vec::new(),
        body: Expr::Attr(Ident::new(attribute)),
    })
}

/// `@type name :: type`.
fn type_alias(name: &str, type_: Type, doc: Doc) -> elixir::Item {
    elixir::Item::TypeAlias(elixir::TypeAlias {
        doc,
        name: Ident::new(name),
        type_,
    })
}

/// The declaration's own IDL name, where the Elixir spelling moved it.
fn idl_note(spelled: &str, declared: &str) -> Doc {
    if spelled == declared {
        return Doc::none();
    }
    Doc::line(&format!("IDL: `{declared}`."))
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

/// The Elixir expression a constant's value takes.
///
/// `long double` is refused by name. Its Elixir holder is a `binary()`
/// carrying the decimal, and a float literal would put a different
/// number in the file than the IDL wrote — the same refusal the Go
/// column makes.
fn elixir_constant(
    ast: &Ast,
    value: &zerodds_ast::ConstValue,
    declared: Option<Id<TypeNode>>,
    at: &str,
) -> Result<Expr, MaterialiseError> {
    use zerodds_ast::ConstValue;
    let refuse = |construct: &str| MaterialiseError::NoLanguageForm {
        construct: construct.to_string(),
        language: LANGUAGE,
        at: at.to_string(),
    };
    let is_binary = declared.is_some_and(|id| {
        matches!(
            elixir_value_type(ast, id, 0),
            Ok(Type::Basic(Basic::Binary))
        )
    });
    Ok(match value {
        ConstValue::Boolean(flag) => Expr::Literal(Literal::Bool(*flag)),
        ConstValue::Integer(number) => Expr::int(*number),
        // The code point as a number: an IDL `char` is one octet here
        // and an IDL `wchar` two, and Elixir's `?a` is neither.
        ConstValue::Char(character) => Expr::int(i128::from(u32::from(*character))),
        ConstValue::WChar(point) => Expr::int(i128::from(*point)),
        ConstValue::Float { mantissa, exponent } => {
            if is_binary {
                return Err(refuse(
                    "a `long double` constant, which has no Elixir literal",
                ));
            }
            Expr::Literal(Literal::Float(constant::float_text(
                mantissa,
                exponent.as_deref(),
            )))
        }
        ConstValue::Fixed { digits, scale } => {
            Expr::string(&constant::fixed_decimal(digits, *scale))
        }
        ConstValue::String(text) => Expr::string(text),
        ConstValue::WString(points) => Expr::string(
            &constant::wstring_utf8(points)
                .ok_or_else(|| refuse("a `wstring` constant holding a lone surrogate"))?,
        ),
        // `ConstValue` is `#[non_exhaustive]`, so a form added to the
        // model reaches here. Refused by name rather than given a
        // plausible default: a constant emitted as the wrong literal is
        // a file that disagrees with the IDL and compiles.
        _ => return Err(refuse("a constant form this binding has no literal for")),
    })
}
