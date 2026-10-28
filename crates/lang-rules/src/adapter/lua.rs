// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Lua CST as the other end of a rule. Spec §3.2.
//!
//! The counterpart of [`crate::adapter::python`], which asks the same
//! question, and of [`crate::adapter::go`], which does not have to.
//!
//! Values are resolved from the model, never from the overlay — the same
//! rule the sibling materialisers follow and for the same reason
//! (spec §2.3).
//!
//! # Lua is the hardest of the ten, and it is worth saying why plainly
//!
//! Python at least has a place to write an annotation. Lua has none.
//! There is no type syntax, no declaration, no `struct`, no constant and
//! no annotation the language itself reads. A struct is a table. So is a
//! sequence, a map, an array and a union. `math.type` distinguishes an
//! integer from a float and that is the entire type information a Lua
//! program can obtain about a value it was handed.
//!
//! A column that emitted only constructors would therefore carry **none
//! of the IDL**: not a width, not a signedness, not a bound, not a
//! member id — and not the member **order**, which matters here in a way
//! it does not in Python. A Python class body has an order and
//! `__annotations__` keeps it. A Lua table has none: `next`, and
//! therefore `pairs`, "does not specify any order for the traversal"
//! (Lua 5.4 reference §6.1). DDS-XTypes writes an aggregate's members in
//! declaration order (DDS-XTypes 1.3 §7.4.1.1.5.1), so an order that is
//! not written down is an order that is gone.
//!
//! So this column emits **two things and both are data**:
//!
//! 1. **A constructor per type** — `M.Pose.new()` — returning a table
//!    with every member at the default of its type, so a program can
//!    make a value without reading anything.
//! 2. **A descriptor table per type** — `M.Pose` itself — holding the
//!    member list **in the array part**, where the index is the order,
//!    each member with its IDL name, its Lua key, its kind, its width,
//!    its signedness, its bound, whether it is optional, whether it is a
//!    key and its member id. That table is what a codec reads. Without
//!    it this column carries nothing.
//!
//! Every descriptor is also registered in `M._descriptors`, keyed by the
//! IDL scoped name (`::common::util::Location`), which is the one name
//! that survives every mapping in this file.
//!
//! ## `---@class` and `---@field` are comments
//!
//! Above each type the file carries a LuaCATS block —
//! `---@class Pose`, `---@field x integer` — which
//! `lua-language-server` reads and an editor shows. **Nothing in the Lua
//! runtime reads it.** It cannot refuse a program, it cannot refuse a
//! value, and it is discarded by `luac`. It is written because an editor
//! checking what Lua cannot is worth having, and it is named as a
//! comment here so that nobody mistakes it for a check.
//!
//! ## Modules nest, they do not flatten
//!
//! `::common::util::Location` is `M.common.util.Location`. A Lua table
//! key is a string and any string will do, so the IDL scope survives
//! whole. The Go column flattens to `Common_Util_Location` because a Go
//! package is a directory and cannot nest inside one file; that join is
//! not invertible, since an IDL identifier may itself contain `_`
//! (IDL 4.2 §7.2.3). Nesting is, so this column takes it.
//!
//! ## No identifier is ever renamed
//!
//! This is the one thing this column has that the other nine do not.
//! Lua restricts the **syntax** `t.name` to identifiers that are not
//! keywords, and not the key: `t.end` does not parse and `t["end"]`
//! does, and both name the key `"end"` (Lua 5.4 reference §3.4.9). So a
//! member called `end` keeps the name `end` — only the form it is
//! written in changes, which [`zerodds_lang_cst::lua::table_key`] and
//! [`zerodds_lang_cst::lua::Place::field`] decide. Every descriptor
//! carries `name` and `key` anyway, and in this column they are always
//! the same string; a codec reads `key`, so if that ever stopped being
//! true the codec would not change.
//!
//! ## The mapping
//!
//! | IDL | Lua value | where the rest is written |
//! |---|---|---|
//! | `boolean` | `false` / `true` | `kind = "boolean"` |
//! | `octet`, `uint8`, `int8` … `long long` | integer | `kind`, `bits`, `signed` |
//! | `unsigned long long` | integer, **two's complement** | `signed = false`, `wraps = true` |
//! | `char`, `wchar` | integer | `kind = "char"`, `bits = 8` or `16` |
//! | `float`, `double` | float | `kind = "float"`, `bits` |
//! | `long double` | **string**, the decimal | `kind = "float"`, `bits = 128` |
//! | `string`, `wstring` | string | `kind = "string"`, `bits`, `bound` |
//! | `fixed<P,S>` | string, the decimal | `kind = "decimal"`, `digits`, `scale` |
//! | `sequence<T,N>` | table, **1-based** | `kind = "sequence"`, `base = 1`, `bound`, `element` |
//! | `T[N][M]` | nested tables, 1-based | `kind = "array"`, `dims = { N, M }` |
//! | `map<K,V>` | array of `{ key = …, value = … }` | `kind = "map"`, `key_type`, `value_type` |
//! | `enum` | integer | `values`, `names`, `enumerators`, `bit_bound` |
//! | `bitmask` | integer | `values`, `bits`, `bit_bound` |
//! | `union` | table with `_d` and one key per branch | `discriminator`, `members` |
//! | `@optional T m` | the key, or no key | `optional = true` |
//! | `typedef` | nothing | `kind = "alias"`, `target` |
//! | `native` | opaque | `kind = "native"`, `mapping` |
//! | `const` | `M.LIMIT = 8` | the `---` line above it |
//! | `interface` | table of operation descriptors | `operations` |
//!
//! ## Which Lua this was measured against
//!
//! The target is Lua 5.4 and every clause cited here is from its
//! reference manual. The interpreter the measurements below were taken
//! with is the one on the path, which reports `Lua 5.5.0`: integers and
//! floats are still two subtypes of `number`, an integer is still 64-bit
//! two's complement, a hexadecimal numeral still wraps, `pairs` still
//! has no defined order and an array still starts at 1. Where a
//! measurement is quoted it is that interpreter's output, and it is
//! quoted rather than described so that a later version disagreeing with
//! it is visible.
//!
//! ### `unsigned long long` is an integer holding the bit pattern
//!
//! Lua 5.4 integers are 64-bit two's complement and wrap: measured with
//! the interpreter on the path,
//! `lua -e 'print(math.maxinteger + 1 == math.mininteger)'` prints
//! `true`. So a `unsigned long long` above 2^63 − 1 has no Lua integer
//! that reads as the number it is — `18446744073709551615` reads back as
//! `-1`.
//!
//! A decimal **string** was considered, as `long double` and `fixed` get.
//! It is not taken, for two reasons. The eight octets CDR writes are
//! exactly the eight octets a Lua integer holds, so the integer is
//! lossless on the wire and only the *reading* of it as a number is
//! lost; and a string would make one of the ten integers a different
//! Lua type from the other nine, so a codec would branch on the width it
//! is already reading from the descriptor. What the descriptor says
//! instead is `signed = false, wraps = true`, which is the fact, and a
//! range check is the codec's to make.
//!
//! A **constant** is the one place this bites, because it has to be
//! written as a literal. Lua 5.4 reference §3.1: a decimal numeral whose
//! value does not fit in an integer "denotes a float", and a hexadecimal
//! one "wraps around" to fit. Measured:
//! `lua -e 'print(math.type(18446744073709551615))'` prints `float`, and
//! `lua -e 'print(math.type(0xFFFFFFFFFFFFFFFF), 0xFFFFFFFFFFFFFFFF)'`
//! prints `integer` and `-1`. So a constant above 2^63 − 1 is written in
//! hexadecimal, which is the exact bit pattern; one that does not fit in
//! 64 bits at all is refused by name.
//!
//! ### `long double` is a string and never a Lua float
//!
//! A Lua float is a C `double` — 53 bits of significand — and an IDL
//! `long double` is a binary128 with 113. Writing one into a float loses
//! sixty bits at the assignment, before any codec runs. The member holds
//! the decimal text instead, as it does in the Go and Python columns.
//!
//! ### A sequence is 1-based and an IDL index is not
//!
//! A Lua table used as an array starts at 1: the length operator and
//! `ipairs` both assume it (Lua 5.4 reference §3.4.7). An IDL sequence
//! index starts at 0. So element *i* of a `sequence<T>` is `t[i + 1]`,
//! and this is the single most likely defect in any program written
//! against this column. The descriptor says `base = 1` on every
//! sequence, array and map, so a codec reads the convention rather than
//! assuming it.
//!
//! ### `map<K,V>` is an array of pairs and never a Lua table keyed by K
//!
//! Three reasons, any one of which decides it.
//!
//! `pairs` has no defined order — `next` "does not specify any order for
//! the traversal" (Lua 5.4 reference §6.1) — and a `map<K,V>` goes on
//! the wire as an ordered sequence of key/value pairs
//! (DDS-XTypes 1.3 §7.4.1.1.4). A Lua table would give a different octet
//! stream for one value depending on nothing the program controls.
//!
//! A Lua table cannot hold a `nil` **value** at all: assigning `nil`
//! removes the key. A map whose value type is `@optional`, or whose
//! value is simply absent, has nowhere to live.
//!
//! And a Lua table cannot hold a `nil` **key** either, nor two keys that
//! compare equal by value but not by identity — `map<Point, long>` is
//! ordinary IDL, and two tables holding the same numbers are two
//! different keys.
//!
//! ### `@optional` is the absence of the key, and that is why the
//! descriptor exists
//!
//! DDS-XTypes 1.3 §7.2.2.4.4.4.7 makes **absence** the value. In Lua the
//! absence of a value is the absence of the key: `t.m = nil` removes it,
//! `#t` does not see it and `pairs` does not visit it. So a program
//! holding a value cannot tell "the member is absent" from "this table
//! was never a Pose" — the *value* carries no member list at all.
//!
//! That is exactly why the descriptor carries the member list in order.
//! `M.Pose.members` is where the members are; the value is where the
//! ones that are present are.
//!
//! ### A union has a `_d` and one key per branch, not a tag and a payload
//!
//! An IDL union discriminator may hold a value **no branch selects**
//! (IDL 4.2 §7.4.1.4.4.1; DDS-XTypes 1.3 §7.4.1.1.5.2 writes only the
//! selected branch, and there may be none). A `{ tag, payload }` shape
//! cannot say that without inventing a tag the IDL does not have. So
//! `_d` is a member in its own right and the branches sit beside it,
//! at most one of them present. `_d` cannot collide with a branch: an
//! IDL identifier begins with an ASCII letter (IDL 4.2 §7.2.3), so no
//! IDL member is spelled `_d`.
//!
//! ## What is refused
//!
//! `any`, `ValueBase`, an object reference and an annotation type have
//! no Lua data form and are named as
//! [`MaterialiseError::NoLanguageForm`] where a member would carry one.
//! Nothing is emitted for them at declaration level.
//!
//! **This matters more here than anywhere else.** Every other column has
//! at least a compiler between the generated file and the program: Go
//! would need a type for an `any` member, and there is none. Lua needs
//! nothing — a table entry holding whatever an `any` turned out to be
//! would load, run and report success, with no wire form behind it. That
//! is the silent-substitution class this project is arranged against,
//! and Lua is where it is easiest to commit.
//!
//! ## What the generated file checks at load, and what it does not
//!
//! Two things run while the chunk executes, and both are written into
//! the file rather than left to a test:
//!
//! - `M._resolved` resolves every type name a descriptor mentions
//!   through `assert`. Reading a missing key in Lua answers `nil` and
//!   carries on, so a descriptor naming a type the file does not declare
//!   would otherwise be found by whatever read it next.
//! - a discarded local calls every constructor once. A Lua function body
//!   is not checked against anything until it is called, so a load that
//!   did not call them would say nothing about them.
//!
//! Everything else Lua does not check, and this column does not claim it
//! does: no width, no bound, no member order and no member type is
//! enforced at run time by anything in the generated file.

use crate::adapter::constant;
use crate::adapter::cst::{MaterialiseError, no_spelling_collision};
use crate::adapter::interface;
use crate::apply::Applied;
use crate::fixed_literal::zero_decimal;
use crate::verbatim;
use seesaw_tgg::ident::GhostId;
use zerodds_ast::{
    Ast, Bound, CharWidth, ConstValue, ModuleNode, PrimitiveKind, TypeKind, TypeNode,
};
use zerodds_ast_core::{Id, NodeId};
use zerodds_lang_cst::common::{Doc, Ident, Literal};
use zerodds_lang_cst::lua::{self, Entry, Expr, File, Function, Place, Stat, Table};

/// The language this column names in a diagnostic.
const LANGUAGE: &str = "Lua";

/// The local the generated chunk builds and returns.
///
/// One letter, and the same in every generated file. A Lua module is
/// whatever its chunk returns (Lua 5.4 reference §6.3, `require`), so the
/// name is local to the file and never seen by a consumer — which is why
/// it can be short without being cryptic.
pub const MODULE: &str = "M";

/// The key a union's discriminator takes.
///
/// `_d`, as in every other column. It cannot collide with a branch: an
/// IDL identifier begins with an ASCII letter (IDL 4.2 §7.2.3), so no
/// IDL member is spelled `_d`.
pub const DISCRIMINATOR: &str = "_d";

/// The key holding every descriptor, by IDL scoped name.
const DESCRIPTORS: &str = "_descriptors";

/// The key holding every declared scoped name, in declaration order.
const NAMES: &str = "_names";

/// The key holding the load-time resolution of every named reference.
const RESOLVED: &str = "_resolved";

/// How far up a module chain the walk follows before the model is taken
/// to be malformed. IDL nests nowhere near this.
const MAX_MODULE_DEPTH: usize = 64;

/// How deep a type's descriptor is followed.
///
/// A `sequence<sequence<…>>` nests once per level; a chain longer than
/// this is a cycle the resolver did not catch, and recursing into it
/// would end the process rather than report anything.
const MAX_TYPE_DEPTH: usize = 64;

/// Build the Lua module a rule application produced.
///
/// The whole file and not a list of statements: the local the chunk
/// builds, the registry every descriptor lands in, the load-time
/// resolution and the `return` are all part of what this column decides,
/// and a caller assembling them separately would be a second place where
/// they are decided.
///
/// # Errors
/// [`MaterialiseError`] when a value the output has to state does not
/// evaluate, a type has no Lua form, or two declarations reach one Lua
/// path — never a silently dropped declaration.
pub fn lua_file(ast: &Ast, applied: &Applied) -> Result<File, MaterialiseError> {
    let mut builder = Builder::new(ast);
    builder.walk(applied)?;
    Ok(File {
        end_text: verbatim::file_slots(ast, verbatim::aliases::LUA).1,
        doc: Doc::lines(&[
            "Generated by ZeroDDS from an IDL source. Do not edit.",
            "",
            "Lua declares nothing. A struct, a sequence, an array, a map and a",
            "union are all tables, and `math.type` is the only type question a",
            "program can ask of a value. So every IDL fact is written into a",
            "descriptor table instead: the member list in declaration order, and",
            "for each member its IDL name, its Lua key, its kind, its width, its",
            "signedness, its bound, whether it is optional, whether it is a key",
            "and its member id.",
            "",
            "M._descriptors maps the IDL scoped name of every declared type to its",
            "descriptor. M._names lists those names in declaration order.",
            "",
            "The ---@class and ---@field blocks are comments. lua-language-server",
            "reads them; nothing in the Lua runtime does.",
            "",
            "A sequence, an array and a map are 1-based here and 0-based in IDL:",
            "element i of a sequence is t[i + 1]. Every such descriptor says",
            "base = 1 rather than leaving it to be assumed.",
        ]),
        body: {
            let (begin_file, _) = verbatim::file_slots(ast, verbatim::aliases::LUA);
            let mut body: Vec<Stat> = begin_file.into_iter().map(Stat::Verbatim).collect();
            body.extend(builder.finish());
            body
        },
        returns: Expr::place(Place::name(MODULE)),
    })
}

/// The declarations only, without the chunk around them.
///
/// For a caller that emits its own preamble — a codec pass writing
/// beside the types. The statements name `M`, so the caller has to bind
/// it; [`lua_file`] is what a generator writes to disk. The module
/// tables come first (`M.Robot = {}` before `M.Robot.Pose = …`): without
/// them the first declaration inside a module indexes nil at load.
///
/// # Errors
/// As [`lua_file`].
pub fn lua_items(ast: &Ast, applied: &Applied) -> Result<Vec<Stat>, MaterialiseError> {
    let mut builder = Builder::new(ast);
    builder.walk(applied)?;
    let mut out = builder.module_tables;
    out.append(&mut builder.declarations);
    Ok(verbatim::weave_file(ast, verbatim::aliases::LUA, out))
}

/// `@verbatim` in the Lua column. A declaration is a table constructor,
/// which holds fields and not statements, so `BEGIN_DECLARATION` and
/// `END_DECLARATION` land adjacent to it.
impl verbatim::Carrier for Stat {
    fn verbatim(text: String) -> Self {
        Self::Verbatim(text)
    }

    fn take_into_body(&mut self, _: &str, _: &mut Vec<String>, _: &mut Vec<String>) -> bool {
        false
    }
}

/// The descriptor a value of `target` is described by.
///
/// The one answer, for every emitter: a member of a generated struct, a
/// codec's parameter and a defaulter's destination all read the same
/// table or they disagree about the wire.
///
/// # Errors
/// [`MaterialiseError::NoLanguageForm`] for `any`, `ValueBase`, an
/// object reference and an annotation type;
/// [`MaterialiseError::Unevaluated`] for a bound or a dimension the
/// model will not answer for.
pub fn lua_type_descriptor(
    ast: &Ast,
    target: Id<TypeNode>,
    at: &str,
) -> Result<Table, MaterialiseError> {
    let mut refs = Vec::new();
    Ok(Table::of(type_entries(ast, target, at, &mut refs, 0)?))
}

// -- the walk -------------------------------------------------------------

/// The statements of one generated chunk, as they are built.
struct Builder<'a> {
    ast: &'a Ast,
    /// The module tables, emitted once each, parents first.
    module_tables: Vec<Stat>,
    /// The Lua paths already emitted for a module table.
    module_paths: Vec<String>,
    /// One declaration's statements, in model order.
    declarations: Vec<Stat>,
    /// Every Lua path a declaration took, with the IDL name behind it.
    emitted: Vec<(String, String)>,
    /// Every declared IDL scoped name, in declaration order.
    names: Vec<String>,
    /// Every scoped name a descriptor mentions, with the first mention.
    refs: Vec<(String, String)>,
    /// The constructors, so the chunk can run each once at load.
    constructors: Vec<Place>,
}

impl<'a> Builder<'a> {
    fn new(ast: &'a Ast) -> Self {
        Self {
            ast,
            module_tables: Vec::new(),
            module_paths: Vec::new(),
            declarations: Vec::new(),
            emitted: Vec::new(),
            names: Vec::new(),
            refs: Vec::new(),
            constructors: Vec::new(),
        }
    }

    /// Every declaration the rules created, in the model's own order.
    ///
    /// Nothing in a Lua chunk has to be declared before it is named: a
    /// constructor's body runs when it is called, and a descriptor names
    /// another type by its scoped name and not by its table. So the walk
    /// is the model's declaration order and the output tracks the source,
    /// as the Go column's does and unlike the C column's, which has to
    /// order the type graph.
    fn walk(&mut self, applied: &Applied) -> Result<(), MaterialiseError> {
        let ast = self.ast;
        for (id, _) in ast.types() {
            let start = self.declarations.len();
            self.declare_type(applied, id)?;
            verbatim::weave(
                verbatim::declaration_slots(ast, id, verbatim::aliases::LUA),
                "",
                start,
                &mut self.declarations,
            );
        }
        self.declare_constants(applied)?;
        self.declare_exceptions(applied)?;
        self.declare_interfaces(applied)?;
        Ok(())
    }

    /// The chunk's statements, in written order.
    fn finish(mut self) -> Vec<Stat> {
        let mut out = vec![
            Stat::Local {
                doc: Doc::none(),
                name: Ident::new(MODULE),
                value: Some(Expr::Table(Table::default())),
            },
            Stat::Assign {
                doc: Doc::lines(&[
                    "The descriptor of every type this module declares, by its IDL",
                    "scoped name. That name is the only one that survives every",
                    "mapping in this file, so it is what a codec looks a type up by.",
                ]),
                target: Place::name(MODULE).field(DESCRIPTORS),
                value: Expr::Table(Table::default()),
            },
        ];
        out.append(&mut self.module_tables);
        out.append(&mut self.declarations);
        out.push(Stat::Assign {
            doc: Doc::lines(&[
                "Every declared type, in IDL declaration order.",
                "",
                "The array part of a table, because that is the only part of a Lua",
                "table that has an order: `next` does not specify one for the rest",
                "(Lua 5.4 reference 6.1).",
            ]),
            target: Place::name(MODULE).field(NAMES),
            value: Expr::Table(Table::of(
                self.names
                    .iter()
                    .map(|name| Entry::positional(Expr::string(name)))
                    .collect(),
            )),
        });
        out.push(Stat::Assign {
            doc: Doc::lines(&[
                "Every type a descriptor names, resolved while this chunk runs.",
                "",
                "Reading a missing key in Lua answers nil and carries on, so a",
                "descriptor naming a type this module does not declare would be",
                "found by whatever read it next rather than here. `assert` is what",
                "notices, and it notices at load.",
            ]),
            target: Place::name(MODULE).field(RESOLVED),
            value: Expr::Table(Table::of(self.reference_entries())),
        });
        out.push(Stat::Comment(Doc::lines(&[
            "Every constructor, run once and discarded. A Lua function body is",
            "not checked against anything until it is called, so a load that did",
            "not call these would say nothing about them.",
        ])));
        out.push(Stat::Local {
            doc: Doc::none(),
            name: Ident::new("_"),
            value: Some(Expr::Table(Table::of(
                self.constructors
                    .iter()
                    .map(|target| {
                        Entry::positional(Expr::Call {
                            func: Box::new(target.clone()),
                            args: Vec::new(),
                        })
                    })
                    .collect(),
            ))),
        });
        out
    }

    /// One `assert` per distinct scoped name a descriptor mentions.
    fn reference_entries(&self) -> Vec<Entry> {
        self.refs
            .iter()
            .map(|(name, at)| Entry {
                doc: Doc::none(),
                key: lua::Key::Expr(Expr::string(name)),
                value: Expr::Call {
                    func: Box::new(Place::name("assert")),
                    args: vec![
                        Expr::place(Place::Index {
                            base: Box::new(Place::name(MODULE).field(DESCRIPTORS)),
                            key: Box::new(Expr::string(name)),
                        }),
                        Expr::string(&format!(
                            "{at} names {name}, which this module does not declare"
                        )),
                    ],
                },
            })
            .collect()
    }

    /// The place a declaration in `module` named `name` is assigned to,
    /// with every module table on the way emitted first.
    ///
    /// # Errors
    /// [`MaterialiseError::NoModule`] when the model does not answer for
    /// a module in the chain, or the chain runs past
    /// [`MAX_MODULE_DEPTH`]. Placing the declaration at the root instead
    /// would put it in another scope and say nothing, which is the same
    /// class of substitution as writing the wrong octet.
    fn place_of(&mut self, module: Id<ModuleNode>, name: &str) -> Result<Place, MaterialiseError> {
        let ast = self.ast;
        let chain = module_chain(ast, module)?;
        let mut place = Place::name(MODULE);
        let mut text = MODULE.to_string();
        for segment in &chain {
            place = place.field(segment);
            text.push('.');
            text.push_str(segment);
            if !self.module_paths.contains(&text) {
                self.module_paths.push(text.clone());
                self.module_tables.push(Stat::Assign {
                    doc: Doc::line(&format!(
                        "The IDL module `{segment}`, as the table its declarations nest in."
                    )),
                    target: place.clone(),
                    value: Expr::Table(Table::default()),
                });
            }
        }
        Ok(place.field(name))
    }

    /// The text of a Lua path, for the whole-file collision check.
    fn path_text(module: &[String], name: &str) -> String {
        let mut text = MODULE.to_string();
        for segment in module {
            text.push('.');
            text.push_str(segment);
        }
        text.push('.');
        text.push_str(name);
        text
    }

    /// Record a declared path, refusing a second declaration reaching it.
    ///
    /// Lua binds a key; it does not declare one. `M.Pose = …` twice is
    /// not an error, it is a rebinding, and the second silently wins — so
    /// one of the two types would be gone from a file that loads. IDL
    /// forbids two declarations of one scoped name, which is why this
    /// should never fire; it is checked anyway, because "should never"
    /// about a name collision is the assumption that makes one
    /// declaration take another's.
    fn remember(&mut self, path: &str, declared: &str) -> Result<(), MaterialiseError> {
        if let Some((_, first)) = self.emitted.iter().find(|(other, _)| other == path) {
            return Err(MaterialiseError::SpellingCollision {
                first: first.clone(),
                second: declared.to_string(),
                spelled: path.to_string(),
                language: LANGUAGE,
                at: "the generated chunk".to_string(),
            });
        }
        self.emitted.push((path.to_string(), declared.to_string()));
        Ok(())
    }

    /// Emit one declaration: its LuaCATS block, its descriptor, its
    /// registration and, where it has one, its constructor.
    fn emit(&mut self, decl: Decl) -> Result<(), MaterialiseError> {
        let ast = self.ast;
        let chain = module_chain(ast, decl.module)?;
        let path = Self::path_text(&chain, &decl.name);
        self.remember(&path, &decl.scoped)?;
        let target = self.place_of(decl.module, &decl.name)?;

        if !decl.annotation.is_empty() {
            self.declarations.push(Stat::Annotation(decl.annotation));
        }
        self.declarations.push(Stat::Assign {
            doc: decl.doc,
            target: target.clone(),
            value: Expr::Table(decl.descriptor),
        });
        self.declarations.push(Stat::Assign {
            doc: Doc::none(),
            target: Place::Index {
                base: Box::new(Place::name(MODULE).field(DESCRIPTORS)),
                key: Box::new(Expr::string(&decl.scoped)),
            },
            value: Expr::place(target.clone()),
        });
        self.names.push(decl.scoped.clone());

        if let Some(value) = decl.constructor {
            let new = target.field("new");
            let mut doc = Doc::line(&format!(
                "A `{}` with every member at the default of its type.",
                decl.scoped
            ));
            doc.push("");
            doc.push("An @optional member is not given a key at all: in Lua the absence");
            doc.push("of a value is the absence of the key (DDS-XTypes 1.3 7.2.2.4.4.4.7).");
            doc.push(&format!("@return {}", decl.class));
            self.declarations.push(Stat::Function(Function {
                doc,
                target: new.clone(),
                params: Vec::new(),
                body: vec![Stat::Return(Some(value))],
            }));
            self.constructors.push(new);
        }
        Ok(())
    }
}

/// One declaration, as this column emits it.
struct Decl {
    /// The IDL module it was declared in.
    module: Id<ModuleNode>,
    /// The IDL simple name, which is also the Lua key.
    name: String,
    /// The IDL scoped name, `::` separated.
    scoped: String,
    /// The LuaCATS class name, dotted.
    class: String,
    /// The `---@class` block, or empty where there is no value shape.
    annotation: Doc,
    /// The `---` block above the descriptor.
    doc: Doc,
    /// The descriptor table.
    descriptor: Table,
    /// The constructor's body, where the type has a value shape.
    constructor: Option<Expr>,
}

// -- types ----------------------------------------------------------------

impl Builder<'_> {
    #[allow(clippy::too_many_lines)]
    fn declare_type(
        &mut self,
        applied: &Applied,
        id: Id<TypeNode>,
    ) -> Result<(), MaterialiseError> {
        let ast = self.ast;
        let Ok(node) = ast.type_(id) else {
            return Ok(());
        };
        let scoped = scoped_name(ast, id);
        let class = class_name(ast, id);

        match &node.kind {
            // A primitive, a string, a `fixed`, a sequence, an array and
            // a map are all written where the member is: each is a
            // descriptor entry and none of them declares anything. Lua
            // needs no entry class for a map either, which the Go column
            // does — a pair is a table with two keys and needs no name.
            TypeKind::Primitive(_)
            | TypeKind::String { .. }
            | TypeKind::Fixed { .. }
            | TypeKind::Sequence { .. }
            | TypeKind::Map { .. }
            | TypeKind::Array { .. } => {}

            TypeKind::Alias { aliased } => {
                // A typedef with no name is one the frontend made for an
                // inline type; it is transparent and declares nothing.
                if node.name.is_empty() {
                    return Ok(());
                }
                if ghost_created_for(applied, node.node_id, "LuaAlias").is_none() {
                    return Ok(());
                }
                let Some(target) = aliased.resolved() else {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "typedef target".to_string(),
                        at: node.name.clone(),
                    });
                };
                let inner = Table::of(type_entries(ast, target, &scoped, &mut self.refs, 0)?);
                self.emit(Decl {
                    module: node.module,
                    name: node.name.clone(),
                    scoped: scoped.clone(),
                    class: cats_type(ast, target, 0),
                    annotation: Doc::none(),
                    doc: Doc::lines(&[
                        "A typedef. Lua has no alias to declare, so what is left of it",
                        "is this entry: the name, and the descriptor of what it names.",
                        "Every use of the alias takes the target's descriptor.",
                    ]),
                    descriptor: Table::of(vec![
                        Entry::named("idl", Expr::string(&scoped)),
                        Entry::named("name", Expr::string(&node.name)),
                        Entry::named("kind", Expr::string("alias")),
                        Entry::named("target", Expr::Table(inner)),
                    ]),
                    constructor: Some(default_expr(ast, target, &scoped, 0)?),
                })?;
            }

            TypeKind::Enumeration { enumerators } => {
                if ghost_created_for(applied, node.node_id, "LuaEnum").is_none() {
                    return Ok(());
                }
                let mut ordered = Vec::new();
                let mut values = Vec::new();
                let mut names = Vec::new();
                let mut first = None;
                for enumerator in enumerators {
                    let Ok(enum_node) = ast.enumerator(*enumerator) else {
                        continue;
                    };
                    if ghost_created_for(applied, enum_node.node_id, "LuaEnumerator").is_none() {
                        continue;
                    }
                    let value = self
                        .ast
                        .effective_enumerator_value(*enumerator)
                        .ok_or_else(|| MaterialiseError::Unevaluated {
                            construct: "enumerator value".to_string(),
                            at: enum_node.name.clone(),
                        })?;
                    let literal = integer_literal(value, &enum_node.name)?;
                    if first.is_none() {
                        first = Some(literal.clone());
                    }
                    ordered.push(Entry::positional(Expr::Table(Table::of(vec![
                        Entry::named("name", Expr::string(&enum_node.name)),
                        Entry::named("value", literal.clone()),
                    ]))));
                    values.push(Entry::named(&enum_node.name, literal.clone()));
                    names.push(Entry {
                        doc: Doc::none(),
                        key: lua::Key::Expr(literal),
                        value: Expr::string(&enum_node.name),
                    });
                }
                self.emit(Decl {
                    module: node.module,
                    name: node.name.clone(),
                    scoped: scoped.clone(),
                    class: class.clone(),
                    annotation: Doc::none(),
                    doc: Doc::lines(&[
                        "An enumeration, as the two maps and the ordered list an",
                        "enumerator carries. The values are written out because @value",
                        "admits gaps, so a position is not a value.",
                        "",
                        "`names` is the inverse of `values`; a Lua table holds both",
                        "directions at once because its keys may be integers as well as",
                        "strings.",
                    ]),
                    descriptor: Table::of(vec![
                        Entry::named("idl", Expr::string(&scoped)),
                        Entry::named("name", Expr::string(&node.name)),
                        Entry::named("kind", Expr::string("enum")),
                        Entry::named(
                            "bit_bound",
                            Expr::int(i128::from(ast.effective_bit_bound(id))),
                        ),
                        Entry::named("enumerators", Expr::Table(Table::of(ordered))),
                        Entry::named("values", Expr::Table(Table::of(values))),
                        Entry::named("names", Expr::Table(Table::of(names))),
                    ]),
                    // The first enumerator's value, and not zero: `@value`
                    // admits gaps, so an enumeration need not have a
                    // literal with the value 0 at all, and a zero default
                    // would be a value the type does not have.
                    constructor: first,
                })?;
            }

            TypeKind::Bitmask { bits } => {
                if ghost_created_for(applied, node.node_id, "LuaBitmask").is_none() {
                    return Ok(());
                }
                let bound = ast.effective_bit_bound(id);
                let mut ordered = Vec::new();
                let mut values = Vec::new();
                for bit in bits {
                    let Ok(bit_node) = ast.bit_value(*bit) else {
                        continue;
                    };
                    if ghost_created_for(applied, bit_node.node_id, "LuaBitEntry").is_none() {
                        continue;
                    }
                    let position = ast.effective_bit_position(*bit).ok_or_else(|| {
                        MaterialiseError::Unevaluated {
                            construct: "bit position".to_string(),
                            at: bit_node.name.clone(),
                        }
                    })?;
                    // `1 << position` and not the number it evaluates to:
                    // the position is what the IDL declared and the value
                    // is derived from it, so the file says which is which.
                    let shift = Expr::Binary {
                        op: "<<",
                        lhs: Box::new(Expr::int(1)),
                        rhs: Box::new(Expr::int(i128::from(position))),
                    };
                    ordered.push(Entry::positional(Expr::Table(Table::of(vec![
                        Entry::named("name", Expr::string(&bit_node.name)),
                        Entry::named("position", Expr::int(i128::from(position))),
                        Entry::named("value", shift.clone()),
                    ]))));
                    values.push(Entry::named(&bit_node.name, shift));
                }
                self.emit(Decl {
                    module: node.module,
                    name: node.name.clone(),
                    scoped: scoped.clone(),
                    class: class.clone(),
                    annotation: Doc::none(),
                    doc: Doc::lines(&[
                        "A bitmask: one value per named bit, and the holder width the",
                        "@bit_bound gives (Table 33 of DDS-XTypes 1.3 7.4.1.1.3.2).",
                        "",
                        "A Lua integer is 64-bit two's complement, so a bit_bound of 64",
                        "puts bit 63 in the sign; the width is here so a codec masks",
                        "rather than assumes.",
                    ]),
                    descriptor: Table::of(vec![
                        Entry::named("idl", Expr::string(&scoped)),
                        Entry::named("name", Expr::string(&node.name)),
                        Entry::named("kind", Expr::string("bitmask")),
                        Entry::named("bit_bound", Expr::int(i128::from(bound))),
                        Entry::named("bits", Expr::Table(Table::of(ordered))),
                        Entry::named("values", Expr::Table(Table::of(values))),
                    ]),
                    // Zero: no bit set, which is a value of every bitmask.
                    constructor: Some(Expr::int(0)),
                })?;
            }

            TypeKind::Bitset { .. } => {
                if ghost_created_for(applied, node.node_id, "LuaBitset").is_none() {
                    return Ok(());
                }
                let mut members = Vec::new();
                let mut defaults = Vec::new();
                let mut fields = Vec::new();
                let mut pairs = Vec::new();
                // The base's bitfields are members of the derived bitset
                // too: an inherited bitset is one flat run.
                for field in &ast.all_bitfields_of(id) {
                    let Ok(field_node) = ast.bitfield(*field) else {
                        continue;
                    };
                    // An anonymous bitfield is padding: it moves the next
                    // field along and carries no value of its own.
                    let Some(name) = field_node.name.as_ref() else {
                        continue;
                    };
                    pairs.push((name.clone(), name.clone()));
                    members.push(Entry::positional(Expr::Table(Table::of(vec![
                        Entry::named("name", Expr::string(name)),
                        Entry::named("key", Expr::string(name)),
                        Entry::named("lua", Expr::string("integer")),
                        Entry::named("kind", Expr::string("int")),
                        Entry::named("bits", Expr::int(i128::from(field_node.bits))),
                        Entry::named("signed", Expr::bool(false)),
                        Entry::named("wraps", Expr::bool(false)),
                        Entry::named("optional", Expr::bool(false)),
                        Entry::named("is_key", Expr::bool(false)),
                        Entry::named("id", Expr::bool(false)),
                    ]))));
                    defaults.push(Entry::named(name, Expr::int(0)));
                    fields.push(cats_field(name, "integer", false));
                }
                no_spelling_collision(as_pairs(&pairs), LANGUAGE, &node.name)?;
                self.emit(Decl {
                    module: node.module,
                    name: node.name.clone(),
                    scoped: scoped.clone(),
                    class: class.clone(),
                    annotation: cats_class(&class, &fields),
                    doc: Doc::lines(&[
                        "A bitset. Each field states its own bit width; Lua has no",
                        "fixed-width integer, so nothing is rounded up to a holder the",
                        "way the C and Go columns have to.",
                        "",
                        "The packing into the anchor is the codec's, because the order",
                        "on the wire is the declaration's.",
                    ]),
                    descriptor: Table::of(vec![
                        Entry::named("idl", Expr::string(&scoped)),
                        Entry::named("name", Expr::string(&node.name)),
                        Entry::named("kind", Expr::string("bitset")),
                        Entry::named("members", Expr::Table(Table::of(members))),
                    ]),
                    constructor: Some(Expr::Table(Table::of(defaults))),
                })?;
            }

            TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
                let is_value = matches!(node.kind, TypeKind::Valuetype { .. });
                let created = if is_value { "LuaValue" } else { "LuaStruct" };
                if ghost_created_for(applied, node.node_id, created).is_none() {
                    return Ok(());
                }
                // XTypes 1.3 §7.5.1.4.1.3: the base is the first member `parent`; the
                // wire still carries the base's members first (§7.4.1.1.5.1).
                let members = ast.members_of(id).to_vec();
                let built =
                    self.aggregate(crate::adapter::c::struct_base(ast, id), &members, &scoped)?;
                self.emit(Decl {
                    module: node.module,
                    name: node.name.clone(),
                    scoped: scoped.clone(),
                    class: class.clone(),
                    annotation: cats_class(&class, &built.fields),
                    doc: Doc::lines(&[
                        "The member list is the array part of `members`, so the index is",
                        "the declaration order — which is the order the wire uses",
                        "(DDS-XTypes 1.3 7.4.1.1.5.1). A Lua table has no order of its",
                        "own: `next` does not specify one (Lua 5.4 reference 6.1).",
                    ]),
                    descriptor: Table::of(vec![
                        Entry::named("idl", Expr::string(&scoped)),
                        Entry::named("name", Expr::string(&node.name)),
                        Entry::named(
                            "kind",
                            Expr::string(if is_value { "valuetype" } else { "struct" }),
                        ),
                        Entry::named(
                            "extensibility",
                            Expr::string(ast.effective_extensibility(id).as_str()),
                        ),
                        Entry::named("members", Expr::Table(Table::of(built.members))),
                    ]),
                    constructor: Some(Expr::Table(Table::of(built.defaults))),
                })?;
            }

            TypeKind::Union { discriminator, .. } => {
                if ghost_created_for(applied, node.node_id, "LuaUnion").is_none() {
                    return Ok(());
                }
                let Some(disc) = discriminator.resolved() else {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "union discriminator".to_string(),
                        at: scoped,
                    });
                };
                let cases = ast.cases_of(id).to_vec();
                let mut branches = Vec::new();
                let mut pairs = vec![(DISCRIMINATOR.to_string(), "<discriminator>".to_string())];
                for case in &cases {
                    let Ok(case_node) = ast.case(*case) else {
                        continue;
                    };
                    if ghost_created_for(applied, case_node.node_id, "LuaCase").is_none() {
                        continue;
                    }
                    let Ok(member) = ast.member(case_node.member) else {
                        continue;
                    };
                    pairs.push((member.name.clone(), member.name.clone()));
                    branches.push(case_node.member);
                }
                no_spelling_collision(as_pairs(&pairs), LANGUAGE, &node.name)?;

                let disc_entries = {
                    let mut entries = vec![
                        Entry::named("name", Expr::string(DISCRIMINATOR)),
                        Entry::named("key", Expr::string(DISCRIMINATOR)),
                    ];
                    entries.append(&mut type_entries(
                        ast,
                        disc,
                        &format!("{scoped}._d"),
                        &mut self.refs,
                        0,
                    )?);
                    entries
                };
                let built = self.aggregate(None, &branches, &scoped)?;
                let mut fields = vec![cats_field(DISCRIMINATOR, &cats_type(ast, disc, 0), false)];
                // Every branch is optional in the value: at most one of
                // them is present at a time, and the discriminator may
                // select none of them at all.
                for field in &built.fields {
                    fields.push(optional_field(field));
                }
                self.emit(Decl {
                    module: node.module,
                    name: node.name.clone(),
                    scoped: scoped.clone(),
                    class: class.clone(),
                    annotation: cats_class(&class, &fields),
                    doc: Doc::lines(&[
                        "A union: the discriminator under `_d`, and one key per branch.",
                        "",
                        "Not a tag and a payload. An IDL discriminator may hold a value",
                        "no branch selects (IDL 4.2 7.4.1.4.4.1), and only the selected",
                        "branch is written (DDS-XTypes 1.3 7.4.1.1.5.2) — so `_d` is a",
                        "member in its own right and the branches sit beside it, at most",
                        "one present.",
                    ]),
                    descriptor: Table::of(vec![
                        Entry::named("idl", Expr::string(&scoped)),
                        Entry::named("name", Expr::string(&node.name)),
                        Entry::named("kind", Expr::string("union")),
                        Entry::named(
                            "extensibility",
                            Expr::string(ast.effective_extensibility(id).as_str()),
                        ),
                        Entry::named("discriminator", Expr::Table(Table::of(disc_entries))),
                        Entry::named("members", Expr::Table(Table::of(built.members))),
                    ]),
                    // The discriminator and nothing else. Which branch its
                    // value selects is the codec's reading of the labels,
                    // and it may select none.
                    constructor: Some(Expr::Table(Table::of(vec![Entry::named(
                        DISCRIMINATOR,
                        default_expr(ast, disc, &scoped, 0)?,
                    )]))),
                })?;
            }

            TypeKind::Native => {
                if ghost_created_for(applied, node.node_id, "LuaNative").is_none() {
                    return Ok(());
                }
                // Opaque, never a guess at what the name might mean: the
                // concrete mapping comes from the facet a `#pragma` or an
                // annotation set, and `false` where there is none.
                let mapping = self
                    .ast
                    .any_of_type(id)
                    .ok()
                    .and_then(|any| ast.facets(any))
                    .and_then(|facets| facets.corba.as_ref())
                    .and_then(|corba| corba.native_mapping.clone());
                self.emit(Decl {
                    module: node.module,
                    name: node.name.clone(),
                    scoped: scoped.clone(),
                    class: class.clone(),
                    annotation: Doc::none(),
                    doc: Doc::lines(&[
                        "A native type: opaque unless a facet named its mapping.",
                        "",
                        "`mapping = false` and not nil. A table entry set to nil is an",
                        "entry that does not exist, so nil could not be told from a key",
                        "this file never wrote.",
                    ]),
                    descriptor: Table::of(vec![
                        Entry::named("idl", Expr::string(&scoped)),
                        Entry::named("name", Expr::string(&node.name)),
                        Entry::named("kind", Expr::string("native")),
                        Entry::named(
                            "mapping",
                            mapping.as_deref().map_or(Expr::bool(false), Expr::string),
                        ),
                    ]),
                    constructor: None,
                })?;
            }

            // A value box is transparent here: the descriptor of what it
            // boxes, with `boxed = true` on it. Lua has no reference type
            // to declare and no pointer to write.
            TypeKind::ValueBox { .. } => {}

            // An object reference belongs to the RPC arc, an annotation
            // type has no runtime form at all, and `any` and `ValueBase`
            // are decided at run time — `zerodds_ast_layout` refuses to
            // plan any of them. Named here rather than passed over, so
            // the coverage table and this file say the same thing. A
            // member that reaches one is refused by name in
            // `type_entries`.
            TypeKind::ObjectRef { .. }
            | TypeKind::AnnotationType { .. }
            | TypeKind::Any
            | TypeKind::ValueBase => {}
        }
        Ok(())
    }

    /// The member list, the constructor's defaults and the `---@field`
    /// lines of one aggregate.
    fn aggregate(
        &mut self,
        base: Option<Id<TypeNode>>,
        members: &[Id<zerodds_ast::MemberNode>],
        at: &str,
    ) -> Result<Aggregate, MaterialiseError> {
        let ast = self.ast;
        let mut pairs = Vec::new();
        if base.is_some() {
            pairs.push(("parent".to_string(), "parent".to_string()));
        }
        for member in members {
            if let Ok(node) = ast.member(*member) {
                pairs.push((node.name.clone(), node.name.clone()));
            }
        }
        no_spelling_collision(as_pairs(&pairs), LANGUAGE, at)?;

        let mut built = Aggregate::default();
        // The base is the first member `parent` (XTypes 1.3 §7.5.1.4.1.3).
        if let Some(base) = base {
            let site = format!("{at}.parent");
            let mut entries = vec![
                Entry::named("name", Expr::string("parent")),
                Entry::named("key", Expr::string("parent")),
            ];
            entries.append(&mut type_entries(ast, base, &site, &mut self.refs, 0)?);
            entries.push(Entry::named("optional", Expr::bool(false)));
            entries.push(Entry::named("is_key", Expr::bool(false)));
            entries.push(Entry::named("id", Expr::bool(false)));
            built
                .members
                .push(Entry::positional(Expr::Table(Table::of(entries))));
            built
                .fields
                .push(cats_field("parent", &cats_type(ast, base, 0), false));
            built.defaults.push(Entry {
                doc: Doc::none(),
                key: lua::table_key("parent"),
                value: default_expr(ast, base, &site, 0)?,
            });
        }
        for member in members {
            let Ok(node) = ast.member(*member) else {
                continue;
            };
            // The aggregate and the member together, because this is
            // what the `assert` in `M._resolved` names when a descriptor
            // mentions a type the file does not declare — and a bare
            // member name would leave the reader to find which aggregate.
            let site = format!("{at}.{}", node.name);
            let Some(target) = node.type_.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "member type".to_string(),
                    at: site,
                });
            };
            let optional = node.facets.is_optional();
            let mut entries = vec![
                Entry::named("name", Expr::string(&node.name)),
                Entry::named("key", Expr::string(&node.name)),
            ];
            entries.append(&mut type_entries(ast, target, &site, &mut self.refs, 0)?);
            entries.push(Entry::named("optional", Expr::bool(optional)));
            entries.push(Entry::named("is_key", Expr::bool(node.facets.is_key())));
            // `false` and not nil where the model has assigned no id: a
            // table entry set to nil does not exist, so nil could not be
            // told from a key this file never wrote — which is the whole
            // reason this descriptor is here.
            entries.push(Entry::named(
                "id",
                node.facets
                    .effective_member_id()
                    .map_or(Expr::bool(false), |id| Expr::int(i128::from(id))),
            ));
            built
                .members
                .push(Entry::positional(Expr::Table(Table::of(entries))));
            built
                .fields
                .push(cats_field(&node.name, &cats_type(ast, target, 0), optional));
            // An @optional member gets no key at all. In Lua the absence
            // of a value is the absence of the key, so writing one with a
            // default would make an absent member indistinguishable from
            // one that arrived with that value.
            if !optional {
                built.defaults.push(Entry {
                    doc: Doc::none(),
                    key: lua::table_key(&node.name),
                    value: default_expr(ast, target, &site, 0)?,
                });
            }
        }
        Ok(built)
    }
}

/// What one aggregate contributes to its descriptor, its constructor and
/// its LuaCATS block.
#[derive(Default)]
struct Aggregate {
    /// The member descriptors, in declaration order.
    members: Vec<Entry>,
    /// The constructor's entries, the optional members left out.
    defaults: Vec<Entry>,
    /// The `---@field` lines.
    fields: Vec<String>,
}

// -- constants, exceptions and interfaces ---------------------------------

impl Builder<'_> {
    fn declare_constants(&mut self, applied: &Applied) -> Result<(), MaterialiseError> {
        let ast = self.ast;
        for (_, node) in ast.constants() {
            if ghost_created_for(applied, node.node_id, "LuaConst").is_none() {
                continue;
            }
            // `eval_const` and not `eval_integral`: `const string NAME` is
            // a constant that evaluates perfectly well and is not a
            // number, and the integral question is the wrong one for four
            // of the eight forms IDL 4.2 admits.
            let value = ast
                .eval_const(node.expr)
                .map_err(|_| MaterialiseError::Unevaluated {
                    construct: "constant's value".to_string(),
                    at: node.name.clone(),
                })?;
            let declared = node.type_.resolved();
            let spelling =
                declared.map_or_else(|| "long".to_string(), |target| idl_spelling(ast, target, 0));
            let chain = module_chain(ast, node.module)?;
            let path = Self::path_text(&chain, &node.name);
            let scoped = declared_scoped(&chain, &node.name);
            self.remember(&path, &scoped)?;
            let target = self.place_of(node.module, &node.name)?;
            let expr = constant_expr(
                &value,
                declared.is_some_and(|id| is_decimal_holder(ast, id)),
                &node.name,
            )?;
            self.declarations.push(Stat::Assign {
                doc: Doc::line(&format!("`const {spelling} {}` — IDL {scoped}.", node.name)),
                target,
                value: expr,
            });
        }
        Ok(())
    }

    fn declare_exceptions(&mut self, applied: &Applied) -> Result<(), MaterialiseError> {
        let ast = self.ast;
        for (_, node) in ast.exceptions() {
            if ghost_created_for(applied, node.node_id, "LuaError").is_none() {
                continue;
            }
            let chain = module_chain(ast, node.module)?;
            let scoped = declared_scoped(&chain, &node.name);
            let class = dotted(&chain, &node.name);
            let built = self.aggregate(None, &node.members, &scoped)?;
            self.emit(Decl {
                module: node.module,
                name: node.name.clone(),
                scoped: scoped.clone(),
                class: class.clone(),
                annotation: cats_class(&class, &built.fields),
                doc: Doc::lines(&[
                    "An exception, as the structure its members make. Raising is the",
                    "RPC arc's decision; the shape is the IDL's.",
                ]),
                descriptor: Table::of(vec![
                    Entry::named("idl", Expr::string(&scoped)),
                    Entry::named("name", Expr::string(&node.name)),
                    Entry::named("kind", Expr::string("exception")),
                    Entry::named("members", Expr::Table(Table::of(built.members))),
                ]),
                constructor: Some(Expr::Table(Table::of(built.defaults))),
            })?;
        }
        Ok(())
    }

    fn declare_interfaces(&mut self, applied: &Applied) -> Result<(), MaterialiseError> {
        let ast = self.ast;
        for (id, node) in ast.interfaces() {
            if ghost_created_for(applied, node.node_id, "LuaInterface").is_none() {
                continue;
            }
            let chain = module_chain(ast, node.module)?;
            let scoped = declared_scoped(&chain, &node.name);
            let class = dotted(&chain, &node.name);
            let mut operations = Vec::new();
            let mut signatures = Vec::new();
            // The whole answer and not the declared one. The descriptor
            // table has no link to a base's table, so a reader holding
            // this one sees exactly the operations listed here and OMG
            // IDL 4.2 §7.4.3's inherited ones have to be among them.
            // `interface::all_operations_flattened` walks the bases the
            // same `Ast::all_operations_of` did and gives the declared
            // operations only (`Origin::Declared`) — attributes get
            // their own descriptor array below, a shape of their own
            // and not a pair of synthesised operations, for the same
            // reason this column's own doc gives params/results a
            // table each: "an operation's parameters and result are a
            // shape, and a shape is exactly what this column carries".
            let all = interface::all_operations_flattened(ast, id)
                .map_err(|error| interface::interface_error_to_materialise(error, &node.name))?;
            for op in all
                .iter()
                .filter(|op| op.origin == interface::Origin::Declared)
            {
                signatures.push((op.name.clone(), op.name.clone()));
                let mut params = Vec::new();
                for param in &op.params {
                    let mut entries = vec![
                        Entry::named("name", Expr::string(&param.name)),
                        Entry::named("key", Expr::string(&param.name)),
                    ];
                    entries.append(&mut type_entries(
                        ast,
                        param.type_,
                        &format!("{scoped}.{}({})", op.name, param.name),
                        &mut self.refs,
                        0,
                    )?);
                    params.push(Entry::positional(Expr::Table(Table::of(entries))));
                }
                // An array of nought or one, and not a key that is
                // sometimes absent: a `void` operation and one whose
                // result key was never written would be the same table.
                let mut results = Vec::new();
                if let Some(returns) = op.returns {
                    results.push(Entry::positional(Expr::Table(Table::of(type_entries(
                        ast,
                        returns,
                        &format!("{scoped}.{}", op.name),
                        &mut self.refs,
                        0,
                    )?))));
                }
                operations.push(Entry::positional(Expr::Table(Table::of(vec![
                    Entry::named("name", Expr::string(&op.name)),
                    Entry::named("key", Expr::string(&op.name)),
                    Entry::named("params", Expr::Table(Table::of(params))),
                    Entry::named("results", Expr::Table(Table::of(results))),
                    Entry::named("raises", raise_names(&op.raises)),
                ]))));
            }
            let mut attributes = Vec::new();
            let mut attribute_signatures = Vec::new();
            let all_attrs = interface::all_attributes_flattened(ast, id)
                .map_err(|error| interface::interface_error_to_materialise(error, &node.name))?;
            for attribute in &all_attrs {
                attribute_signatures.push((attribute.name.clone(), attribute.name.clone()));
                let type_entries_for_attr = type_entries(
                    ast,
                    attribute.type_,
                    &format!("{scoped}.{}", attribute.name),
                    &mut self.refs,
                    0,
                )?;
                attributes.push(Entry::positional(Expr::Table(Table::of(vec![
                    Entry::named("name", Expr::string(&attribute.name)),
                    Entry::named("key", Expr::string(&attribute.name)),
                    Entry::named("type", Expr::Table(Table::of(type_entries_for_attr))),
                    Entry::named("readonly", Expr::bool(attribute.readonly)),
                    Entry::named("get_raises", raise_names(&attribute.get_raises)),
                    Entry::named("set_raises", raise_names(&attribute.set_raises)),
                ]))));
            }
            no_spelling_collision(as_pairs(&signatures), LANGUAGE, &node.name)?;
            no_spelling_collision(as_pairs(&attribute_signatures), LANGUAGE, &node.name)?;
            self.emit(Decl {
                module: node.module,
                name: node.name.clone(),
                scoped: scoped.clone(),
                class,
                annotation: Doc::none(),
                doc: Doc::lines(&[
                    "An interface, as the operations and attributes it has — its",
                    "bases' before its own, because the table has no link to a base's.",
                    "Emitted rather than refused: an operation's parameters and result",
                    "are a shape, and a shape is exactly what this column carries — an",
                    "attribute's type and `readonly` flag are the same kind of shape.",
                    "What none of it is is a call — the transport is the RPC arc's",
                    "business.",
                    "",
                    "`results` is an array of nought or one. A `void` operation and one",
                    "whose result key was never written would otherwise be the same",
                    "table, because a Lua key set to nil does not exist. `raises`/",
                    "`get_raises`/`set_raises` are arrays of exception names, empty",
                    "where the IDL declares none.",
                ]),
                descriptor: Table::of(vec![
                    Entry::named("idl", Expr::string(&scoped)),
                    Entry::named("name", Expr::string(&node.name)),
                    Entry::named("kind", Expr::string("interface")),
                    Entry::named("operations", Expr::Table(Table::of(operations))),
                    Entry::named("attributes", Expr::Table(Table::of(attributes))),
                ]),
                constructor: None,
            })?;
        }
        Ok(())
    }
}

// -- describing a type ----------------------------------------------------

/// The entries describing a value of `id`.
///
/// Every IDL fact Lua cannot enforce lands here: the width, the
/// signedness, the bound, the digit count, the dimensions and the name of
/// A `raises`/`getraises`/`setraises` clause, as the array of exception
/// names its descriptor table carries — empty where the IDL declared
/// none, which is a table entry a reader indexes rather than a doc
/// comment it has to parse.
fn raise_names(raises: &[interface::Raise]) -> Expr {
    Expr::Table(Table::of(
        raises
            .iter()
            .map(|raise| Entry::positional(Expr::string(&raise.name)))
            .collect(),
    ))
}

/// whatever named type is on the other end.
///
/// zerodds-lint: recursion-depth 64 (type nesting; bounded by `MAX_TYPE_DEPTH`)
#[allow(clippy::too_many_lines)]
fn type_entries(
    ast: &Ast,
    id: Id<TypeNode>,
    at: &str,
    refs: &mut Vec<(String, String)>,
    depth: usize,
) -> Result<Vec<Entry>, MaterialiseError> {
    if depth > MAX_TYPE_DEPTH {
        return Err(MaterialiseError::Unevaluated {
            construct: format!("type, whose descriptor nests past {MAX_TYPE_DEPTH}"),
            at: at.to_string(),
        });
    }
    let node = ast.type_(id).map_err(|_| MaterialiseError::Unevaluated {
        construct: "type".to_string(),
        at: at.to_string(),
    })?;
    let refused = match &node.kind {
        TypeKind::Any => Some("any"),
        TypeKind::ValueBase => Some("ValueBase"),
        TypeKind::ObjectRef { .. } => Some("Object"),
        TypeKind::AnnotationType { .. } => Some("annotation type"),
        _ => None,
    };
    if let Some(construct) = refused {
        return Err(MaterialiseError::NoLanguageForm {
            construct: construct.to_string(),
            language: LANGUAGE,
            at: at.to_string(),
        });
    }
    Ok(match &node.kind {
        TypeKind::Primitive(kind) => primitive_entries(*kind),
        TypeKind::String { width, bound } => vec![
            Entry::named("lua", Expr::string("string")),
            Entry::named("kind", Expr::string("string")),
            Entry::named("bits", Expr::int(char_bits(*width))),
            Entry::named("bound", Expr::int(bound_of(ast, *bound, at)?)),
        ],
        TypeKind::Fixed { digits, scale } => vec![
            Entry::named("lua", Expr::string("string")),
            Entry::named("kind", Expr::string("decimal")),
            Entry::named("digits", Expr::int(i128::from(*digits))),
            Entry::named("scale", Expr::int(i128::from(*scale))),
        ],
        TypeKind::Sequence { element, bound } => {
            let Some(element) = element.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "sequence element".to_string(),
                    at: at.to_string(),
                });
            };
            vec![
                Entry::named("lua", Expr::string("table")),
                Entry::named("kind", Expr::string("sequence")),
                // A Lua array starts at 1 and an IDL index at 0. Written
                // out rather than assumed: element i is t[i + 1].
                Entry::named("base", Expr::int(1)),
                Entry::named("bound", Expr::int(bound_of(ast, *bound, at)?)),
                Entry::named(
                    "element",
                    Expr::Table(Table::of(type_entries(ast, element, at, refs, depth + 1)?)),
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
                let length =
                    ast.eval_integral(*dim)
                        .map_err(|_| MaterialiseError::Unevaluated {
                            construct: "array length".to_string(),
                            at: at.to_string(),
                        })?;
                lengths.push(Entry::positional(Expr::int(length)));
            }
            vec![
                Entry::named("lua", Expr::string("table")),
                Entry::named("kind", Expr::string("array")),
                Entry::named("base", Expr::int(1)),
                // Outermost dimension first, as the model holds them:
                // `long grid[2][3]` is two rows of three, and reversing
                // them silently transposes the data.
                Entry::named("dims", Expr::Table(Table::of(lengths))),
                Entry::named(
                    "element",
                    Expr::Table(Table::of(type_entries(ast, element, at, refs, depth + 1)?)),
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
                Entry::named("lua", Expr::string("table")),
                Entry::named("kind", Expr::string("map")),
                Entry::named("base", Expr::int(1)),
                Entry::named("bound", Expr::int(bound_of(ast, *bound, at)?)),
                Entry::named(
                    "key_type",
                    Expr::Table(Table::of(type_entries(ast, key, at, refs, depth + 1)?)),
                ),
                Entry::named(
                    "value_type",
                    Expr::Table(Table::of(type_entries(ast, value, at, refs, depth + 1)?)),
                ),
            ]
        }
        // A typedef is transparent: the use takes the target's
        // descriptor, with the alias's own name kept beside it so the
        // declaration is not lost.
        TypeKind::Alias { aliased } => {
            let Some(target) = aliased.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "typedef target".to_string(),
                    at: at.to_string(),
                });
            };
            let mut entries = type_entries(ast, target, at, refs, depth + 1)?;
            if !node.name.is_empty() {
                entries.push(Entry::named("alias", Expr::string(&scoped_name(ast, id))));
            }
            entries
        }
        // A boxed value is the value, or its absence. Lua has no
        // reference type to name, so the descriptor is the target's with
        // `boxed = true` on it.
        TypeKind::ValueBox { boxed } => {
            let Some(target) = boxed.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "value box target".to_string(),
                    at: at.to_string(),
                });
            };
            let mut entries = type_entries(ast, target, at, refs, depth + 1)?;
            entries.push(Entry::named("boxed", Expr::bool(true)));
            entries
        }
        TypeKind::Enumeration { .. } | TypeKind::Bitmask { .. } => {
            let scoped = scoped_name(ast, id);
            note(refs, &scoped, at);
            vec![
                Entry::named("lua", Expr::string("integer")),
                Entry::named(
                    "kind",
                    Expr::string(if matches!(node.kind, TypeKind::Bitmask { .. }) {
                        "bitmask"
                    } else {
                        "enum"
                    }),
                ),
                Entry::named("bits", Expr::int(i128::from(ast.effective_bit_bound(id)))),
                Entry::named("ref", Expr::string(&scoped)),
            ]
        }
        TypeKind::Structure { .. }
        | TypeKind::Union { .. }
        | TypeKind::Bitset { .. }
        | TypeKind::Valuetype { .. } => {
            let scoped = scoped_name(ast, id);
            note(refs, &scoped, at);
            vec![
                Entry::named("lua", Expr::string("table")),
                Entry::named("kind", Expr::string(aggregate_kind(&node.kind))),
                Entry::named("ref", Expr::string(&scoped)),
            ]
        }
        TypeKind::Native => {
            let scoped = scoped_name(ast, id);
            note(refs, &scoped, at);
            vec![
                Entry::named("lua", Expr::string("any")),
                Entry::named("kind", Expr::string("native")),
                Entry::named("ref", Expr::string(&scoped)),
            ]
        }
        // Refused above; the arm keeps the match total, so a kind added
        // to the model without a Lua answer is a compile error and not a
        // member that silently takes another's descriptor.
        TypeKind::Any
        | TypeKind::ValueBase
        | TypeKind::ObjectRef { .. }
        | TypeKind::AnnotationType { .. } => {
            return Err(MaterialiseError::NoLanguageForm {
                construct: node.kind.as_str().to_string(),
                language: LANGUAGE,
                at: at.to_string(),
            });
        }
    })
}

/// Record a scoped name a descriptor mentioned, once.
fn note(refs: &mut Vec<(String, String)>, name: &str, at: &str) {
    if refs.iter().any(|(seen, _)| seen == name) {
        return;
    }
    refs.push((name.to_string(), at.to_string()));
}

/// Which of the four aggregate kinds a type is.
const fn aggregate_kind(kind: &TypeKind) -> &'static str {
    match kind {
        TypeKind::Union { .. } => "union",
        TypeKind::Bitset { .. } => "bitset",
        TypeKind::Valuetype { .. } => "valuetype",
        _ => "struct",
    }
}

/// The entries describing one IDL primitive.
///
/// Exhaustive, so a primitive added to the model without a Lua answer is
/// a compile error and not a member that silently takes another's.
///
/// `char` and `wchar` are integers and not one-character strings: a
/// `wchar` is not one octet, so a string holding one would have to choose
/// an encoding, and choosing one here would put the DDS-XTypes 1.3
/// §7.4.1.1.2 question in two places. `bits` is what tells the two apart.
///
/// `long double` is a string holding the decimal; the module
/// documentation carries why.
fn primitive_entries(kind: PrimitiveKind) -> Vec<Entry> {
    let integer = |bits: i128, signed: bool, wraps: bool| {
        vec![
            Entry::named("lua", Expr::string("integer")),
            Entry::named("kind", Expr::string("int")),
            Entry::named("bits", Expr::int(bits)),
            Entry::named("signed", Expr::bool(signed)),
            Entry::named("wraps", Expr::bool(wraps)),
        ]
    };
    let character = |bits: i128| {
        vec![
            Entry::named("lua", Expr::string("integer")),
            Entry::named("kind", Expr::string("char")),
            Entry::named("bits", Expr::int(bits)),
        ]
    };
    match kind {
        PrimitiveKind::Boolean => vec![
            Entry::named("lua", Expr::string("boolean")),
            Entry::named("kind", Expr::string("boolean")),
            Entry::named("bits", Expr::int(8)),
        ],
        PrimitiveKind::Char => character(8),
        PrimitiveKind::WChar => character(16),
        PrimitiveKind::Octet | PrimitiveKind::UInt8 => integer(8, false, false),
        PrimitiveKind::Int8 => integer(8, true, false),
        PrimitiveKind::Int16 => integer(16, true, false),
        PrimitiveKind::UInt16 => integer(16, false, false),
        PrimitiveKind::Int32 => integer(32, true, false),
        PrimitiveKind::UInt32 => integer(32, false, false),
        PrimitiveKind::Int64 => integer(64, true, false),
        // The one integer that does not fit. A Lua integer is 64-bit
        // two's complement, so a value at or above 2^63 reads back
        // negative — `wraps` is where that is written down, and a range
        // check is the codec's to make.
        PrimitiveKind::UInt64 => integer(64, false, true),
        PrimitiveKind::Float32 => vec![
            Entry::named("lua", Expr::string("number")),
            Entry::named("kind", Expr::string("float")),
            Entry::named("bits", Expr::int(32)),
        ],
        PrimitiveKind::Float64 => vec![
            Entry::named("lua", Expr::string("number")),
            Entry::named("kind", Expr::string("float")),
            Entry::named("bits", Expr::int(64)),
        ],
        // A Lua float is a C double: 53 bits of significand against
        // binary128's 113. The member holds the decimal text instead.
        PrimitiveKind::Float128 => vec![
            Entry::named("lua", Expr::string("string")),
            Entry::named("kind", Expr::string("float")),
            Entry::named("bits", Expr::int(128)),
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

// -- default values -------------------------------------------------------

/// The value a member of type `id` takes in a constructor.
///
/// zerodds-lint: recursion-depth 64 (type nesting; bounded by `MAX_TYPE_DEPTH`)
#[allow(clippy::too_many_lines)]
fn default_expr(
    ast: &Ast,
    id: Id<TypeNode>,
    at: &str,
    depth: usize,
) -> Result<Expr, MaterialiseError> {
    if depth > MAX_TYPE_DEPTH {
        return Err(MaterialiseError::Unevaluated {
            construct: format!("type, whose default nests past {MAX_TYPE_DEPTH}"),
            at: at.to_string(),
        });
    }
    let node = ast.type_(id).map_err(|_| MaterialiseError::Unevaluated {
        construct: "type".to_string(),
        at: at.to_string(),
    })?;
    Ok(match &node.kind {
        TypeKind::Primitive(PrimitiveKind::Boolean) => Expr::bool(false),
        TypeKind::Primitive(PrimitiveKind::Float32 | PrimitiveKind::Float64) => {
            Expr::Literal(Literal::Float(lua::float_text("0")))
        }
        // The decimal text, because the holder is a string.
        TypeKind::Primitive(PrimitiveKind::Float128) => Expr::string("0"),
        TypeKind::Primitive(_) => Expr::int(0),
        TypeKind::String { .. } => Expr::string(""),
        TypeKind::Fixed { scale, .. } => Expr::string(&zero_decimal(*scale)),
        // An empty table: a sequence and a map both start with nothing in
        // them, and a Lua table used as an array starts at index 1.
        TypeKind::Sequence { .. } | TypeKind::Map { .. } => Expr::Table(Table::default()),
        TypeKind::Array { element, dims } => {
            let Some(element) = element.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "array element".to_string(),
                    at: at.to_string(),
                });
            };
            let mut lengths = Vec::new();
            for dim in dims {
                lengths.push(ast.eval_integral(*dim).map_err(|_| {
                    MaterialiseError::Unevaluated {
                        construct: "array length".to_string(),
                        at: at.to_string(),
                    }
                })?);
            }
            // Built from the innermost dimension out, so `long grid[2][3]`
            // is two tables of three and not three of two. The whole
            // array is written out: an IDL array has its length in its
            // type, and a constructor returning an empty table would
            // return a value of a different shape from the one declared.
            let mut value = default_expr(ast, element, at, depth + 1)?;
            for length in lengths.iter().rev() {
                let count =
                    usize::try_from(*length).map_err(|_| MaterialiseError::NoLanguageForm {
                        construct: format!("an array of {length} elements"),
                        language: LANGUAGE,
                        at: at.to_string(),
                    })?;
                value = Expr::Table(Table::of(
                    vec![value; count]
                        .into_iter()
                        .map(Entry::positional)
                        .collect(),
                ));
            }
            value
        }
        TypeKind::Alias { aliased } => {
            let Some(target) = aliased.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "typedef target".to_string(),
                    at: at.to_string(),
                });
            };
            default_expr(ast, target, at, depth + 1)?
        }
        TypeKind::ValueBox { boxed } => {
            let Some(target) = boxed.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "value box target".to_string(),
                    at: at.to_string(),
                });
            };
            default_expr(ast, target, at, depth + 1)?
        }
        // The first enumerator's value. `@value` admits gaps, so an
        // enumeration need not have a literal with the value 0 at all,
        // and a zero default would be a value the type does not have.
        TypeKind::Enumeration { enumerators } => {
            let first = enumerators
                .first()
                .and_then(|first| ast.effective_enumerator_value(*first));
            match first {
                Some(value) => integer_literal(value, at)?,
                None => Expr::int(0),
            }
        }
        // No bit set, which is a value of every bitmask.
        TypeKind::Bitmask { .. } => Expr::int(0),
        TypeKind::Structure { .. }
        | TypeKind::Union { .. }
        | TypeKind::Bitset { .. }
        | TypeKind::Valuetype { .. } => Expr::Call {
            func: Box::new(constructor_place(ast, id)?),
            args: Vec::new(),
        },
        // A native type is opaque and this column cannot build a value of
        // it. `false` and not nil: a table entry set to nil is an entry
        // that does not exist, so the member would vanish from the value
        // while the descriptor still listed it.
        TypeKind::Native => Expr::bool(false),
        TypeKind::Any
        | TypeKind::ValueBase
        | TypeKind::ObjectRef { .. }
        | TypeKind::AnnotationType { .. } => {
            return Err(MaterialiseError::NoLanguageForm {
                construct: node.kind.as_str().to_string(),
                language: LANGUAGE,
                at: at.to_string(),
            });
        }
    })
}

/// `M.a.b.T.new`, the constructor of a declared aggregate.
///
/// # Errors
/// [`MaterialiseError::NoModule`] when the model does not answer for a
/// module the type nests in.
fn constructor_place(ast: &Ast, id: Id<TypeNode>) -> Result<Place, MaterialiseError> {
    Ok(declared_place(ast, id)?.field("new"))
}

/// `M.a.b.T` — the place a declared type's descriptor is bound to.
///
/// Public because the codec backend and the Table 9 defaulters reach a
/// declared enumeration's `values` table and a bitmask's through it:
/// one derivation of the path, so a codec cannot part from the
/// materialiser over a spelling.
///
/// # Errors
/// [`MaterialiseError::NoModule`] when the model does not answer for a
/// module the type nests in.
pub fn declared_place(ast: &Ast, id: Id<TypeNode>) -> Result<Place, MaterialiseError> {
    let node = ast.type_(id).map_err(|_| MaterialiseError::Unevaluated {
        construct: "type".to_string(),
        at: "a member's declared type".to_string(),
    })?;
    let mut place = Place::name(MODULE);
    for segment in module_chain(ast, node.module)? {
        place = place.field(&segment);
    }
    Ok(place.field(&node.name))
}

// -- constants ------------------------------------------------------------

/// Whether a constant's declared type is held as decimal text.
///
/// A `long double` and a `fixed` both are, and neither has a Lua literal
/// that carries the value: see [`constant_expr`].
fn is_decimal_holder(ast: &Ast, id: Id<TypeNode>) -> bool {
    // A counted loop and not a call to itself on the alias target: a
    // `typedef` cycle is a finding the model's invariant pass reports
    // rather than a shape it rejects, and the recursive form read such
    // a chain with nothing counting it.
    let mut current = id;
    for _ in 0..MAX_TYPE_DEPTH {
        match ast.type_(current).map(|node| node.kind.clone()) {
            Ok(TypeKind::Primitive(PrimitiveKind::Float128) | TypeKind::Fixed { .. }) => {
                return true;
            }
            Ok(TypeKind::Alias { aliased }) => match aliased.resolved() {
                Some(target) => current = target,
                None => return false,
            },
            _ => return false,
        }
    }
    false
}

/// The Lua expression a constant's value takes.
///
/// `long double` is refused by name. Its Lua holder is a string carrying
/// the decimal, and a Lua float is a C `double`: writing the literal
/// would put a different number in the file than the IDL wrote.
fn constant_expr(
    value: &ConstValue,
    decimal_holder: bool,
    at: &str,
) -> Result<Expr, MaterialiseError> {
    let refuse = |construct: &str| MaterialiseError::NoLanguageForm {
        construct: construct.to_string(),
        language: LANGUAGE,
        at: at.to_string(),
    };
    Ok(match value {
        ConstValue::Boolean(flag) => Expr::bool(*flag),
        ConstValue::Integer(number) => integer_literal(*number, at)?,
        // The code point as a number: a `char` is one octet here and a
        // `wchar` two, and both are held as integers.
        ConstValue::Char(character) => Expr::int(i128::from(u32::from(*character))),
        ConstValue::WChar(point) => Expr::int(i128::from(*point)),
        ConstValue::Float { mantissa, exponent } => {
            if decimal_holder {
                return Err(refuse("a `long double` constant, which has no Lua literal"));
            }
            // Written so that `math.type` answers `float`: Lua 5.4
            // reference §3.1 makes a numeral without a radix point or an
            // exponent an integer.
            Expr::Literal(Literal::Float(lua::float_text(&constant::float_text(
                mantissa,
                exponent.as_deref(),
            ))))
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
        // a file that disagrees with the IDL and loads.
        _ => return Err(refuse("a constant form this binding has no literal for")),
    })
}

/// The literal a whole number takes.
///
/// Decimal where it fits in a Lua integer, and **hexadecimal** where it
/// does not. Lua 5.4 reference §3.1: a decimal numeral whose value does
/// not fit in an integer denotes a *float*, and a hexadecimal one "wraps
/// around" to fit. Measured with the interpreter on the path:
/// `math.type(18446744073709551615)` is `float`, and
/// `math.type(0xFFFFFFFFFFFFFFFF)` is `integer`, with the value `-1`.
///
/// So `const unsigned long long BIG = 18446744073709551615;` is written
/// `0xFFFFFFFFFFFFFFFF`, which is the exact eight octets CDR writes.
///
/// # Errors
/// [`MaterialiseError::NoLanguageForm`] for a value outside 64 bits in
/// either direction. A Lua integer cannot hold it and a float would put
/// a different number in the file.
fn integer_literal(value: i128, at: &str) -> Result<Expr, MaterialiseError> {
    // −2^63 itself is hexadecimal too: `-9223372036854775808` is unary
    // minus over a decimal numeral, and that numeral is already a
    // *float* by the time the minus applies (reference §3.1). Measured:
    // `math.type(-9223372036854775808)` prints `float`, and
    // `0x8000000000000000` is the integer.
    if value == i128::from(i64::MIN) {
        return Ok(Expr::Literal(Literal::Int(
            "0x8000000000000000".to_string(),
        )));
    }
    if i64::try_from(value).is_ok() {
        return Ok(Expr::int(value));
    }
    if let Ok(unsigned) = u64::try_from(value) {
        return Ok(Expr::Literal(Literal::Int(format!("0x{unsigned:016X}"))));
    }
    Err(MaterialiseError::NoLanguageForm {
        construct: format!("the integer {value}, which is outside a 64-bit Lua integer"),
        language: LANGUAGE,
        at: at.to_string(),
    })
}

// -- names ----------------------------------------------------------------

/// A declared type's IDL scoped name: `::common::util::Location`.
///
/// The one name that survives every mapping in this file, which is why
/// it is the key of `M._descriptors` and the value of every `ref`.
#[must_use]
pub fn scoped_name(ast: &Ast, id: Id<TypeNode>) -> String {
    let Ok(any) = ast.any_of_type(id) else {
        return "::<unnamed>".to_string();
    };
    match ast.scoped_name_of(any) {
        Some(scoped) => format!("{scoped}"),
        None => "::<unnamed>".to_string(),
    }
}

/// The scoped name of a declaration outside the type arena.
fn declared_scoped(chain: &[String], name: &str) -> String {
    let mut out = String::from("::");
    for segment in chain {
        out.push_str(segment);
        out.push_str("::");
    }
    out.push_str(name);
    out
}

/// A dotted name, for a LuaCATS class.
fn dotted(chain: &[String], name: &str) -> String {
    let mut segments: Vec<&str> = chain.iter().map(String::as_str).collect();
    segments.push(name);
    segments.join(".")
}

/// The LuaCATS class name of a declared type.
fn class_name(ast: &Ast, id: Id<TypeNode>) -> String {
    let Ok(any) = ast.any_of_type(id) else {
        return "unknown".to_string();
    };
    match ast.scoped_name_of(any) {
        Some(scoped) => scoped.segments().join("."),
        None => "unknown".to_string(),
    }
}

/// The IDL spelling of a type, for the comment above a constant.
///
/// zerodds-lint: recursion-depth 64 (type nesting; bounded by `MAX_TYPE_DEPTH`)
fn idl_spelling(ast: &Ast, id: Id<TypeNode>, depth: usize) -> String {
    if depth > MAX_TYPE_DEPTH {
        return "…".to_string();
    }
    match ast
        .type_(id)
        .map(|node| (node.name.clone(), node.kind.clone()))
    {
        Ok((_, TypeKind::Primitive(kind))) => kind.as_str().to_string(),
        Ok((
            _,
            TypeKind::String {
                width: CharWidth::Char,
                ..
            },
        )) => "string".to_string(),
        Ok((
            _,
            TypeKind::String {
                width: CharWidth::WChar,
                ..
            },
        )) => "wstring".to_string(),
        Ok((_, TypeKind::Fixed { digits, scale })) => format!("fixed<{digits},{scale}>"),
        Ok((_, TypeKind::Sequence { element, .. })) => match element.resolved() {
            Some(element) => format!("sequence<{}>", idl_spelling(ast, element, depth + 1)),
            None => "sequence<?>".to_string(),
        },
        Ok((name, _)) if !name.is_empty() => scoped_name(ast, id),
        _ => "?".to_string(),
    }
}

/// The modules a declaration nests in, outermost first.
///
/// The root module is the anonymous global scope and is the chunk
/// itself, so it produces no table.
///
/// # Errors
/// [`MaterialiseError::NoModule`] when the model does not answer for a
/// module, or the parent chain runs past [`MAX_MODULE_DEPTH`].
fn module_chain(ast: &Ast, module: Id<ModuleNode>) -> Result<Vec<String>, MaterialiseError> {
    let mut out = Vec::new();
    let mut current = Some(module);
    let mut depth = 0;
    while let Some(id) = current {
        if depth > MAX_MODULE_DEPTH {
            return Err(MaterialiseError::NoModule {
                at: format!(
                    "#{}@{} — the parent chain runs past {MAX_MODULE_DEPTH}",
                    module.slot(),
                    module.generation()
                ),
            });
        }
        let node = ast.module(id).map_err(|_| MaterialiseError::NoModule {
            at: format!("#{}@{}", id.slot(), id.generation()),
        })?;
        // The root has no name and no parent, and produces no table.
        if node.parent.is_none() {
            break;
        }
        out.push(node.name.clone());
        current = node.parent;
        depth += 1;
    }
    out.reverse();
    Ok(out)
}

// -- the LuaCATS block ----------------------------------------------------

/// One `---@field` line, or a prose line where the name cannot be one.
///
/// LuaCATS writes a field as `---@field name type`, and a name that is
/// not a Lua identifier — a keyword, in practice — has no place in that
/// grammar. It is written as prose instead, naming the key and the type,
/// rather than guessed at: the key is unchanged either way, and this
/// block is read by an editor and by nothing else.
fn cats_field(name: &str, type_: &str, optional: bool) -> String {
    if lua::is_identifier(name) {
        let mark = if optional { "?" } else { "" };
        return format!("@field {name}{mark} {type_}");
    }
    let absence = if optional {
        ", absent when not set"
    } else {
        ""
    };
    format!("the member `{name}` is at [\"{name}\"] and holds {type_}{absence}")
}

/// The `---@class` block of one type.
fn cats_class(class: &str, fields: &[String]) -> Doc {
    let mut doc = Doc::line(&format!("@class {class}"));
    for field in fields {
        doc.push(field);
    }
    doc
}

/// The same `---@field` line, with the member marked optional.
fn optional_field(field: &str) -> String {
    match field.strip_prefix("@field ") {
        Some(rest) => match rest.split_once(' ') {
            Some((name, type_)) if !name.ends_with('?') => {
                format!("@field {name}? {type_}")
            }
            _ => field.to_string(),
        },
        None => field.to_string(),
    }
}

/// The LuaCATS type a value of `id` is annotated with.
///
/// A comment and nothing else: `lua-language-server` reads it and the
/// Lua runtime does not.
///
/// zerodds-lint: recursion-depth 64 (type nesting; bounded by `MAX_TYPE_DEPTH`)
fn cats_type(ast: &Ast, id: Id<TypeNode>, depth: usize) -> String {
    if depth > MAX_TYPE_DEPTH {
        return "any".to_string();
    }
    let Ok(node) = ast.type_(id) else {
        return "any".to_string();
    };
    match &node.kind {
        TypeKind::Primitive(PrimitiveKind::Boolean) => "boolean".to_string(),
        TypeKind::Primitive(PrimitiveKind::Float32 | PrimitiveKind::Float64) => {
            "number".to_string()
        }
        TypeKind::Primitive(PrimitiveKind::Float128) | TypeKind::String { .. } => {
            "string".to_string()
        }
        TypeKind::Primitive(_) => "integer".to_string(),
        TypeKind::Fixed { .. } => "string".to_string(),
        TypeKind::Sequence { element, .. } => match element.resolved() {
            Some(element) => format!("{}[]", cats_type(ast, element, depth + 1)),
            None => "any[]".to_string(),
        },
        TypeKind::Array { element, dims } => {
            let mut out = match element.resolved() {
                Some(element) => cats_type(ast, element, depth + 1),
                None => "any".to_string(),
            };
            for _ in dims {
                out.push_str("[]");
            }
            out
        }
        // A map is an array of pair tables. LuaCATS has no shape for
        // that which is worth asserting here, so `table[]` says what it
        // is and the descriptor says what is in it.
        TypeKind::Map { .. } => "table[]".to_string(),
        TypeKind::Alias { aliased } => match aliased.resolved() {
            Some(target) => cats_type(ast, target, depth + 1),
            None => "any".to_string(),
        },
        TypeKind::ValueBox { boxed } => match boxed.resolved() {
            Some(target) => format!("{}?", cats_type(ast, target, depth + 1)),
            None => "any?".to_string(),
        },
        TypeKind::Enumeration { .. } | TypeKind::Bitmask { .. } => "integer".to_string(),
        TypeKind::Structure { .. }
        | TypeKind::Union { .. }
        | TypeKind::Bitset { .. }
        | TypeKind::Valuetype { .. } => class_name(ast, id),
        TypeKind::Native
        | TypeKind::Any
        | TypeKind::ValueBase
        | TypeKind::ObjectRef { .. }
        | TypeKind::AnnotationType { .. } => "any".to_string(),
    }
}

// -- the graph ------------------------------------------------------------

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
