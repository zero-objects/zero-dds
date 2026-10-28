// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Python CST as the other end of a rule. Spec §3.2.
//!
//! The counterpart of [`crate::adapter::java`], [`crate::adapter::cpp`]
//! and [`crate::adapter::c`]. Reading two of them side by side is the
//! point: where they differ, the difference is the language's, and it is
//! written down here rather than spread through a renderer.
//!
//! Values are resolved from the model, never from the overlay — the same
//! rule the sibling materialisers follow and for the same reason
//! (spec §2.3).
//!
//! # Python is the hardest of the five, and this is why
//!
//! The other four target languages state a width, a length and a
//! signedness in the declaration itself. Python states none of them. A
//! `long` is 32 bits and a Python `int` is arbitrary precision; an
//! IDL `octet` and an IDL `unsigned long long` are the same Python type;
//! `long a[3]` and `sequence<long> a` are both "a thing you can index".
//! A backend that wrote `x: int` and stopped would produce a file that
//! type-checks and a value the wire cannot hold, with nothing between
//! the two to notice.
//!
//! So this column is arranged around one question: **for each IDL fact
//! Python cannot enforce, where is it written down?** There are exactly
//! two answers used here and no third.
//!
//! 1. **The Python type**, where Python can enforce it. Mutability
//!    (`list` against `tuple`), arity (`tuple[T, T, T]`), absence
//!    (`typing.Optional[T]`), and which class a member holds. A static
//!    checker rejects a program that gets these wrong.
//! 2. **`typing.Annotated` metadata**, where it cannot. The width, the
//!    signedness, the bound, the digit count, the character width. A
//!    checker passes these through untouched; they are read back at run
//!    time with `typing.get_type_hints(cls, include_extras=True)`, which
//!    is what the codec pass will do.
//!
//! Nothing is left implicit. `x: int` never appears in generated output.
//!
//! ## The integer mapping
//!
//! | IDL | Python | metadata |
//! |---|---|---|
//! | `boolean` | `bool` | — |
//! | `char` | `str` | `_IdlText(bits=8, bound=1)` |
//! | `wchar` | `str` | `_IdlText(bits=16, bound=1)` |
//! | `int8` | `int` | `_IdlInt(bits=8, signed=True)` |
//! | `octet`, `uint8` | `int` | `_IdlInt(bits=8, signed=False)` |
//! | `short` | `int` | `_IdlInt(bits=16, signed=True)` |
//! | `unsigned short` | `int` | `_IdlInt(bits=16, signed=False)` |
//! | `long` | `int` | `_IdlInt(bits=32, signed=True)` |
//! | `unsigned long` | `int` | `_IdlInt(bits=32, signed=False)` |
//! | `long long` | `int` | `_IdlInt(bits=64, signed=True)` |
//! | `unsigned long long` | `int` | `_IdlInt(bits=64, signed=False)` |
//! | `float` | `float` | `_IdlFloat(bits=32)` |
//! | `double` | `float` | `_IdlFloat(bits=64)` |
//! | `long double` | **`str`** | `_IdlFloat(bits=128)` |
//!
//! Each of those is written once as a module-level alias — `_Int32`,
//! `_UInt64` — so a field reads `x: _Int32` and the width is one hop
//! away rather than inline on every line.
//!
//! An alias exists exactly where the type's whole description is fixed.
//! A `string<7>` and a `string<9>` are not the same type, so the string
//! family is written inline instead — including the unbounded one, so
//! that `string` and `string<7>` have one spelling between them rather
//! than a name for the common case and a form for the rest.
//!
//! **Where this departs from every other column.** Java has to decide
//! which of its signed types is wide enough to *hold* an unsigned IDL
//! value, and `unsigned long long` ends up a `long` carrying a bit
//! pattern rather than a number. Python has no such problem and so
//! makes no such trade: every one of the ten integers is a Python `int`
//! holding the number, and only the metadata differs. That is the one
//! place where Python's lack of widths is an advantage, and it is worth
//! saying because everywhere else it is the cost.
//!
//! ### `long double` is a `str` and never a `float`
//!
//! A Python `float` is a C `double` — 53 bits of significand. An IDL
//! `long double` is a binary128 with 113. Writing one into a `float`
//! loses sixty bits at the assignment, before any codec runs, with
//! nothing to say it happened. That is the silent-substitution class
//! this project is arranged against, and it is the same defect the
//! three-column gate found in the C column when `long double` reached
//! `double` there.
//!
//! So the member holds the **decimal text**, exactly as `fixed<P,S>`
//! does and for the same reason: it is the one Python type that holds
//! the value the source wrote without rounding it. The Java column
//! reaches for `org.zerodds.wire.Float128` instead, which this column
//! cannot: a generated Python module here imports nothing but
//! `dataclasses`, `enum` and `typing`, so a runtime class would make
//! every generated file depend on a package that is not part of this
//! layer.
//!
//! ## The shapes
//!
//! | IDL | Python |
//! |---|---|
//! | `string<N>`, `wstring<N>` | `str`, with the width and bound in the metadata |
//! | `fixed<P,S>` | `str`, holding the decimal |
//! | `sequence<T>` | `list[T]` |
//! | `sequence<T,N>` | `list[T]`, with the bound in the metadata |
//! | `T[N]` | `tuple[T, …]` — **N** arguments, not an ellipsis |
//! | `T[N][M]` | `tuple[tuple[T, …M], …N]`, outer dimension outermost |
//! | `map<K,V>` | `list[tuple[K, V]]` |
//! | `union` | a dataclass with `_d` and one optional field per case |
//! | `enum` | `enum.IntEnum`; a member of it is an `int` |
//! | `bitmask` | `enum.IntFlag`; a member of it is an `int` |
//! | `bitset` | a dataclass of one field per bitfield |
//! | `@optional T m` | `typing.Optional[T]`, defaulting to `None` |
//! | `typedef` | nothing; a comment, and the target's type at each use |
//! | `module` | a class the declarations nest in |
//!
//! ### `sequence<T>` is a `list` and `T[N]` is a `tuple`
//!
//! Not a style choice. An IDL array has a length that is part of its
//! type and a sequence does not, and those are the two Python built-ins
//! that say exactly that: a `list` can be appended to and a `tuple`
//! cannot. The array's length goes further — `tuple[int, int, int]`
//! names three elements, and a static checker rejects a two-element
//! literal — which makes the length the **one** IDL fact in this whole
//! column that Python's own type system enforces. Writing
//! `tuple[int, ...]` and putting the length in the metadata would give
//! that up for nothing.
//!
//! The cost is stated rather than hidden: `long a[1000]` produces an
//! annotation with a thousand arguments in it. Nothing truncates it and
//! there is no threshold at which the spelling changes, because a
//! threshold would mean the same IDL construct had two Python forms
//! depending on a number nobody chose.
//!
//! ### `map<K,V>` is a `list` of pairs and not a `dict`
//!
//! Two independent reasons, either of which decides it.
//!
//! A `dict` requires hashable keys. A generated struct is a
//! `@dataclasses.dataclass` with `eq=True`, which sets `__hash__` to
//! `None` — so `dict[Point, int]` is a type that cannot hold anything,
//! and `map<Point, long>` is ordinary IDL.
//!
//! And the entry order is a policy axis. `MapEntryOrder` exists to
//! measure which order a peer puts the entries in, and a container that
//! decided an order of its own would make the axis measure the container.
//! A `list` of pairs keeps the sample's order where the sample put it.
//! The Java column reaches the same answer through an array of a
//! generated entry class; Python needs no generated class because
//! `tuple[K, V]` already is one.
//!
//! ### `@optional` is `typing.Optional[T]` and `None` is the absence
//!
//! One field, not a field beside a flag. XTypes 1.3 §7.4.1.2.3 makes
//! **absence** the value, not a zero, and `None` is Python's. A
//! `bool _has_m` beside an `m` would be two things that can disagree,
//! which is what the C column has to carry because C has no `None`.
//!
//! A non-optional member is emitted with no default at all, so
//! constructing an aggregate without it is an error rather than a
//! silently zero-filled sample.
//!
//! ### An enumeration is an `enum.IntEnum` and a member of it is an `int`
//!
//! Both halves matter.
//!
//! The **declaration** is a real `enum.IntEnum` because a Python enum
//! member *is* an `int` — `Colour.RED == 0` is true and
//! `Colour.RED + 1` is an `int`. Java cannot do this, because a Java
//! `enum` constant's identity is its ordinal and `@value(-1)` is not an
//! ordinal, which is why the Java column emits a class of `int`
//! constants and loses the type.
//!
//! The **member** is nonetheless an `int` and not the enumeration:
//! `Colour(7)` raises `ValueError`, and an `@appendable` writer that
//! added an enumerator sends exactly that. A field typed `Colour` could
//! only refuse the sample. So the member is
//! `typing.Annotated[int, _IdlEnum(declared=Colour, bits=32)]` — the
//! runtime type is what the wire can deliver, and which enumeration it
//! is stays readable, in the metadata, where a codec can get at it.
//!
//! A bitmask is the same arrangement with `enum.IntFlag`.
//!
//! ### A bitset states the bit width and not a holder
//!
//! `bitfield<3> low` is three bits. C and Java have to round that up to
//! the narrowest integer that holds it, because they have no three-bit
//! type; Python has no fixed-width type at all, so nothing is rounded
//! and the metadata states three. The declared holder of
//! `bitfield<1, boolean>` names a range and not a wire form, and the
//! width is what the packing uses — the same reading the Java column
//! gives.
//!
//! ### A `typedef` declares nothing
//!
//! Python has type aliases, and this column does not use one, for a
//! reason that is Python's own: an assignment is evaluated **eagerly**.
//! `Position = a.Point` at module level runs when the module is
//! imported, so it has to come after `class a`, and a typedef in one
//! IDL module naming a type in another has no position in the nesting
//! at which that is true. The alias would be a `NameError` at import,
//! for exactly the cross-module cases and no others. So a typedef is
//! transparent — the use takes the target's type — and a comment
//! carries the name and what it resolved to, which is what the Java
//! column does and for a different reason.
//!
//! ### Modules are nested classes
//!
//! Python's own unit of scope is the module, and one IDL file becomes
//! one Python module, so the file's top level is already the global IDL
//! scope; nothing needs an outer class the way the Java column does. A
//! nested IDL module becomes a class, which is the only Python
//! construct that nests, carries a name and can be referred to through
//! a dotted path.
//!
//! Every reference to a generated type is written from the module root
//! down — `a.b.Point`. Unlike Java there is no capture hazard in doing
//! so: `typing.get_type_hints` resolves an annotation against the
//! module globals, where `a` is bound, and a *member* named `a` is not
//! in those globals. The escape below covers the case that does bite.
//!
//! ### Identifiers
//!
//! Through [`zerodds_lang_cst::python::escape_name`], which appends an
//! underscore — PEP 8's own rule for this case. It moves three kinds of
//! name: a keyword, a soft keyword, and a name the generated module
//! itself binds (`int`, `list`, `typing`). The third is the one that is
//! invisible without it: an `@optional` member named `int` becomes a
//! class attribute set to `None`, and `typing.get_type_hints` resolves
//! against the class namespace before the module's, so every `int` in
//! that class would then resolve to `None`.
//!
//! **The IDL name is not lost.** A renamed field carries it in
//! `dataclasses.field(metadata={"idl": …})`, which is machine-readable;
//! every other renamed declaration carries it in a comment on the line
//! above. And two IDL names that would escape onto one Python name in
//! one scope are refused rather than letting the second take the first's
//! declaration.
//!
//! ## What is refused
//!
//! `any`, `ValueBase`, an object reference and an annotation type have
//! no Python data form here and are named as
//! [`MaterialiseError::NoLanguageForm`] rather than left out of the
//! class. Python would happily take `object` for all four, which is
//! exactly why it is not written: a member typed `object` is a field no
//! codec can write, from a generator that reported success.

use crate::adapter::c::ordered_types;
use crate::adapter::constant;
use crate::adapter::cst::{MaterialiseError, no_spelling_collision};
use crate::apply::Applied;
use crate::verbatim;
use seesaw_tgg::ident::GhostId;
use zerodds_ast::{Ast, Bound, CharWidth, ModuleNode, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::{Id, NodeId};
use zerodds_lang_cst::common::{Doc, Ident, Literal, Path};
use zerodds_lang_cst::python::{
    Annotated, Arg, Assign, Class, Decorator, Expr, Import, Keyword, Module, Statement, Type,
    escape_name,
};

/// How deep a module chain is followed before the model is taken to be
/// malformed. IDL nests nowhere near this.
const MAX_MODULE_DEPTH: usize = 64;

/// How deep a type's Python spelling is followed.
///
/// A `sequence<sequence<…>>` nests once per level; a chain longer than
/// this is a cycle the resolver did not catch, and recursing into it
/// would end the process rather than report anything.
const MAX_TYPE_DEPTH: usize = 64;

/// The names the generated preamble binds.
///
/// Every one starts with an underscore, and an IDL identifier cannot:
/// IDL 4.2 §7.2.3 makes the first character an ASCII letter, and the
/// escaped form `_keyword` of §7.2.3.2 denotes the identifier *without*
/// the underscore. So this list should never fire. It is checked anyway,
/// because "should never" about a name collision is the assumption that
/// makes one declaration silently take another's.
const PRELUDE_NAMES: &[&str] = &[
    "_IdlInt",
    "_IdlFloat",
    "_IdlText",
    "_IdlFixed",
    "_IdlSequence",
    "_IdlEnum",
    "_Int8",
    "_UInt8",
    "_Int16",
    "_UInt16",
    "_Int32",
    "_UInt32",
    "_Int64",
    "_UInt64",
    "_Float32",
    "_Float64",
    "_Float128",
    "_Char",
    "_WChar",
];

/// Build the Python module a rule application produced.
///
/// The whole file and not a list of declarations: which imports the
/// preamble needs and which markers the annotations are written in terms
/// of are part of what this column decides, and a caller assembling them
/// separately would be a second place where they are decided.
///
/// # Errors
/// [`MaterialiseError`] when a value the output has to state does not
/// evaluate, a type has no Python shape, or two declarations would take
/// one name in one scope — never a silently dropped declaration.
pub fn python_module(ast: &Ast, applied: &Applied) -> Result<Module, MaterialiseError> {
    let mut items: Vec<Pending> = Vec::new();

    // The same walk the C and Java columns use. Python needs no
    // declaration before use — `from __future__ import annotations`
    // makes every annotation a string, evaluated only when something
    // asks — so the order is not load-bearing for the interpreter. It is
    // load-bearing for the output, which must not move between two runs
    // over one model.
    for id in ordered_types(ast) {
        let start = items.len();
        declare(ast, applied, id, &mut items)?;
        let Ok(node) = ast.type_(id) else {
            continue;
        };
        let module = node.module;
        let name = escape_name(&node.name);
        verbatim::weave_with(
            verbatim::declaration_slots(ast, id, verbatim::aliases::PYTHON),
            start,
            &mut items,
            |text| Pending {
                module,
                statement: Statement::Verbatim(text),
            },
            |declared, begin, end| {
                for pending in declared {
                    if verbatim::Carrier::take_into_body(&mut pending.statement, &name, begin, end)
                    {
                        return;
                    }
                }
            },
        );
    }

    // Constants and exceptions are not types, so they are walked
    // separately — the model keeps them in arenas of their own, and the
    // sibling materialisers do the same.
    for (id, node) in ast.constants() {
        if ghost_created_for(applied, node.node_id, "PythonConst").is_none() {
            continue;
        }
        let _ = id;
        // `eval_const` and not `eval_integral`: `const string MODULE_ID`
        // evaluates perfectly well and is not a number.
        let value = ast
            .eval_const(node.expr)
            .map_err(|_| MaterialiseError::Unevaluated {
                construct: "constant's value".to_string(),
                at: node.name.clone(),
            })?;
        let (type_, kind) = match node.type_.resolved() {
            Some(target) => (
                python_member_type(ast, target, &node.name)?,
                scalar_kind(ast, target),
            ),
            None => (alias(INT32), Some(PrimitiveKind::Int32)),
        };
        let value = constant_expr(kind, &value, &node.name)?;
        items.push(Pending {
            module: node.module,
            statement: Statement::Annotated(Annotated {
                doc: renamed_note(&node.name),
                name: Ident::new(&escape_name(&node.name)),
                // `typing.Final`: a constant that something reassigned
                // would be a constant only in the IDL.
                type_: Type::Subscript {
                    base: Path::new(&["typing", "Final"]),
                    args: vec![type_],
                },
                value: Some(value),
            }),
        });
    }

    for (id, node) in ast.exceptions() {
        if ghost_created_for(applied, node.node_id, "PythonError").is_none() {
            continue;
        }
        let _ = id;
        let mut body = Vec::new();
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
            body.push(field(
                &member_node.name,
                python_member_type(ast, target, &member_node.name)?,
                member_node.facets.is_optional(),
            ));
        }
        items.push(Pending {
            module: node.module,
            statement: dataclass(
                &node.name,
                body,
                Doc::lines(&[
                    "An exception, as the structure its members make.",
                    "",
                    "A dataclass and not a subclass of Exception: raising is a",
                    "run-time decision the RPC arc makes, and a BaseException",
                    "subclass carries __traceback__ and args, which are two more",
                    "attributes than the wire form has.",
                ]),
            ),
        });
    }

    let mut root = Scope::default();
    for item in items {
        insert(ast, &mut root, item)?;
    }
    let body = scope_body(root, 0)?;
    check_scope(&body, 0)?;

    let mut statements = prelude();
    statements.extend(body);
    let statements = verbatim::weave_file(ast, verbatim::aliases::PYTHON, statements);

    Ok(Module {
        doc: Doc::lines(&[
            "The Python shape of one IDL file.",
            "",
            "Generated by zerodds-lang-rules. Do not edit.",
            "",
            "Every IDL fact Python cannot enforce in a type is written into",
            "typing.Annotated metadata: the width of an integer, the bound of a",
            "string or a sequence, the digit count of a fixed. Read it back with",
            "typing.get_type_hints(cls, include_extras=True).",
            "",
            "Requires Python 3.10 or newer, for dataclasses.dataclass(kw_only=True).",
        ]),
        // Every annotation becomes a string, so nothing here has to be
        // declared before it is named and a nested module class may be
        // referred to from anywhere in the file.
        future: vec![Ident::new("annotations")],
        imports: vec![
            Import::Module {
                path: Path::single("dataclasses"),
                alias: None,
            },
            Import::Module {
                path: Path::single("enum"),
                alias: None,
            },
            Import::Module {
                path: Path::single("typing"),
                alias: None,
            },
        ],
        body: statements,
    })
}

/// One declaration, kept with the module that declared it.
struct Pending {
    /// The IDL module it was declared in.
    module: Id<ModuleNode>,
    /// What it materialised as.
    statement: Statement,
}

/// `@verbatim` in the Python column: `BEGIN_DECLARATION` and
/// `END_DECLARATION` open and close a class body — the dataclass of a
/// structure or a union, the `IntEnum` of an enumeration; an alias takes
/// them adjacent.
impl verbatim::Carrier for Statement {
    fn verbatim(text: String) -> Self {
        Self::Verbatim(text)
    }

    fn take_into_body(
        &mut self,
        name: &str,
        begin: &mut Vec<String>,
        end: &mut Vec<String>,
    ) -> bool {
        let Self::Class(class) = self else {
            return false;
        };
        if class.name.as_str() != name {
            return false;
        }
        // A `pass` stays: the text may be nothing but comments, and a
        // class whose suite holds no statement does not parse.
        let mut body: Vec<Statement> = begin.drain(..).map(Statement::Verbatim).collect();
        body.append(&mut class.body);
        body.extend(end.drain(..).map(Statement::Verbatim));
        class.body = body;
        true
    }
}

/// One nesting level of the output: a module's declarations and the
/// modules declared inside it.
///
/// A tree and not a list of chains. Grouping by module id alone would
/// emit `module common { struct Robot; }` and
/// `module common { module util { … } }` as two classes called `common`
/// in one scope, which is the `tests/interop-corpus/module-issue41-shape`
/// arrangement.
#[derive(Default)]
struct Scope {
    /// What this module declares directly, in declaration order.
    body: Vec<Statement>,
    /// The modules declared inside it, in first-seen order.
    children: Vec<(Id<ModuleNode>, String, Scope)>,
}

/// Put one declaration in the scope its module names.
///
/// # Errors
/// [`MaterialiseError::NoModule`] when the model does not answer for a
/// module in the chain, or the chain runs past [`MAX_MODULE_DEPTH`].
fn insert(ast: &Ast, root: &mut Scope, item: Pending) -> Result<(), MaterialiseError> {
    let chain = module_chain(ast, item.module)?;
    let mut scope = root;
    for (id, name) in chain {
        let at = match scope.children.iter().position(|(seen, _, _)| *seen == id) {
            Some(at) => at,
            None => {
                scope.children.push((id, name, Scope::default()));
                scope.children.len() - 1
            }
        };
        scope = &mut scope
            .children
            .get_mut(at)
            .ok_or_else(|| MaterialiseError::NoModule {
                at: module_handle(item.module),
            })?
            .2;
    }
    scope.body.push(item.statement);
    Ok(())
}

/// A scope's statements, with each module inside it as a nested class.
///
/// # Errors
/// [`MaterialiseError::NoModule`] when the nesting runs past
/// [`MAX_MODULE_DEPTH`], which `insert` has already bounded.
///
/// zerodds-lint: recursion-depth 64 (module nesting; bounded by `MAX_MODULE_DEPTH`)
fn scope_body(scope: Scope, depth: usize) -> Result<Vec<Statement>, MaterialiseError> {
    if depth > MAX_MODULE_DEPTH {
        return Err(MaterialiseError::NoModule {
            at: format!("the nesting runs past {MAX_MODULE_DEPTH}"),
        });
    }
    let mut out = scope.body;
    for (_, name, child) in scope.children {
        let body = scope_body(child, depth + 1)?;
        out.push(Statement::Class(Class {
            doc: Doc::line("An IDL module, as the class its declarations nest in."),
            decorators: Vec::new(),
            name: Ident::new(&name),
            bases: Vec::new(),
            // A class body has to hold something. An IDL module that
            // declared nothing this column emits would otherwise render
            // as a header with no block under it, which is a
            // SyntaxError.
            body: if body.is_empty() {
                vec![Statement::Pass]
            } else {
                body
            },
        }));
    }
    Ok(out)
}

/// The modules a declaration nests in, outermost first, with the class
/// name each takes.
///
/// The root module is the anonymous global scope and is the Python
/// module itself, so it is not a class.
///
/// # Errors
/// [`MaterialiseError::NoModule`] when the model does not answer for a
/// module, or the parent chain runs past [`MAX_MODULE_DEPTH`]. Returning
/// the declaration unnested instead would put it in another scope and say
/// nothing, which is the same class of substitution as writing the wrong
/// octet.
fn module_chain(
    ast: &Ast,
    module: Id<ModuleNode>,
) -> Result<Vec<(Id<ModuleNode>, String)>, MaterialiseError> {
    let mut out = Vec::new();
    let mut current = Some(module);
    let mut depth = 0;
    while let Some(id) = current {
        if depth > MAX_MODULE_DEPTH {
            return Err(MaterialiseError::NoModule {
                at: format!(
                    "{} — the parent chain runs past {MAX_MODULE_DEPTH}",
                    module_handle(module)
                ),
            });
        }
        let node = ast.module(id).map_err(|_| MaterialiseError::NoModule {
            at: module_handle(id),
        })?;
        // The root has no name and no parent, and produces no class.
        if node.parent.is_none() {
            break;
        }
        out.push((id, escape_name(&node.name)));
        current = node.parent;
        depth += 1;
    }
    out.reverse();
    Ok(out)
}

/// The module handle, named without the node the model would not answer
/// for. Slot and generation are what the arena was asked for.
fn module_handle(module: Id<ModuleNode>) -> String {
    format!("#{}@{}", module.slot(), module.generation())
}

/// Refuse two declarations that would take one name in one scope.
///
/// Python binds a name; it does not declare one. `class P: ...` twice in
/// one body is not an error, it is a rebinding, and the second silently
/// wins. Two IDL names cannot collide before escaping — IDL forbids it —
/// but `class` and `class_` both escape onto `class_`, and so do a
/// member named `int` and a member named `int_`.
///
/// zerodds-lint: recursion-depth 64 (module nesting; bounded by `MAX_MODULE_DEPTH`)
fn check_scope(body: &[Statement], depth: usize) -> Result<(), MaterialiseError> {
    if depth > MAX_MODULE_DEPTH {
        return Err(MaterialiseError::NoModule {
            at: format!("the nesting runs past {MAX_MODULE_DEPTH}"),
        });
    }
    let mut seen: Vec<&str> = Vec::new();
    for statement in body {
        let name = match statement {
            Statement::Class(class) => class.name.as_str(),
            Statement::Annotated(binding) => binding.name.as_str(),
            Statement::Assign(binding) => binding.name.as_str(),
            Statement::Function(function) => function.name.as_str(),
            // The forms the codec pass added to the tree. None of them
            // binds a name a later declaration could replace — a store
            // writes into a place that already exists, and the rest are
            // control flow — and none of them reaches a *materialised*
            // module at all: this walk sees only what
            // [`python_module`] built. Named rather than swept up with
            // a wildcard so that a form which does start binding names
            // fails here instead of being scoped by omission.
            Statement::Set { .. }
            | Statement::Expr(_)
            | Statement::If { .. }
            | Statement::For { .. }
            | Statement::While { .. }
            | Statement::Break
            | Statement::Return(_)
            | Statement::Comment(_)
            | Statement::Pass
            | Statement::Verbatim(_) => continue,
        };
        if seen.contains(&name) {
            return Err(MaterialiseError::NoLanguageForm {
                construct: format!(
                    "a second declaration binding the name `{name}` in one scope, \
                     which would silently replace the first"
                ),
                language: "Python",
                at: name.to_string(),
            });
        }
        if PRELUDE_NAMES.contains(&name) {
            return Err(MaterialiseError::NoLanguageForm {
                construct: format!(
                    "a declaration named `{name}`, which is one of the names the \
                     generated preamble binds"
                ),
                language: "Python",
                at: name.to_string(),
            });
        }
        seen.push(name);
        if let Statement::Class(class) = statement {
            check_scope(&class.body, depth + 1)?;
        }
    }
    Ok(())
}

/// One type's Python declarations, appended where they belong.
#[allow(clippy::too_many_lines)]
fn declare(
    ast: &Ast,
    applied: &Applied,
    id: Id<TypeNode>,
    items: &mut Vec<Pending>,
) -> Result<(), MaterialiseError> {
    let Ok(node) = ast.type_(id) else {
        return Ok(());
    };
    match &node.kind {
        // A primitive, a string, a `fixed`, a sequence and a map are all
        // spellings Python already has — `list[tuple[K, V]]` needs no
        // entry class, which is the one place this column declares less
        // than the Java one. None of them declares anything.
        TypeKind::Primitive(_)
        | TypeKind::String { .. }
        | TypeKind::Fixed { .. }
        | TypeKind::Sequence { .. }
        | TypeKind::Map { .. } => {}

        // Neither does an array — but every dimension is evaluated
        // anyway, and here it has to be: the length is written into the
        // annotation as that many tuple arguments, so a dimension that
        // does not evaluate has no spelling at all. The C and Java
        // columns fail on the same input, and the columns' failure sets
        // stay comparable.
        TypeKind::Array { dims, .. } => {
            for dim in dims {
                ast.eval_integral(*dim)
                    .map_err(|_| MaterialiseError::Unevaluated {
                        construct: "array length".to_string(),
                        at: named(&node.name, node.node_id),
                    })?;
            }
        }

        // A typedef has no Python declaration; see the module
        // documentation for why an alias would be a `NameError` at
        // import. The comment is what keeps it from being a silent
        // omission: the name, and what it resolved to.
        TypeKind::Alias { aliased } => {
            if node.name.is_empty() {
                return Ok(());
            }
            if ghost_created_for(applied, node.node_id, "PythonAlias").is_none() {
                return Ok(());
            }
            let Some(target) = aliased.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "typedef target".to_string(),
                    at: node.name.clone(),
                });
            };
            let spelling =
                zerodds_lang_render::python::type_(&python_member_type(ast, target, &node.name)?);
            items.push(Pending {
                module: node.module,
                statement: Statement::Comment(Doc::line(&format!(
                    "typedef {spelling} {}; -- not a Python alias, which is evaluated \
                     eagerly and could not be ordered against the classes it names, so \
                     every use takes the spelling above.",
                    node.name
                ))),
            });
        }

        TypeKind::Enumeration { enumerators } => {
            if ghost_created_for(applied, node.node_id, "PythonEnum").is_none() {
                return Ok(());
            }
            let mut body = Vec::new();
            for enumerator in enumerators {
                let Ok(enum_node) = ast.enumerator(*enumerator) else {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "enumerator the model listed".to_string(),
                        at: node.name.clone(),
                    });
                };
                if ghost_created_for(applied, enum_node.node_id, "PythonEnumerator").is_none() {
                    continue;
                }
                let value = ast.effective_enumerator_value(*enumerator).ok_or_else(|| {
                    MaterialiseError::Unevaluated {
                        construct: "enumerator value".to_string(),
                        at: enum_node.name.clone(),
                    }
                })?;
                // Refused outside the 32 bits XTypes 1.3 §7.4.3.5.3
                // gives an enumerated literal, and written as the number
                // the source wrote otherwise. The Java column has to
                // fold a value above `int`'s range onto the pattern it
                // occupies; a Python `int` holds it, so nothing folds.
                let value = in_range(
                    value,
                    i128::from(i32::MIN),
                    i128::from(u32::MAX),
                    "32-bit enumerated literal",
                    &enum_node.name,
                )?;
                body.push(Statement::Assign(Assign {
                    doc: renamed_note(&enum_node.name),
                    name: Ident::new(&escape_name(&enum_node.name)),
                    value: Expr::int(value),
                }));
            }
            items.push(Pending {
                module: node.module,
                statement: enum_class(
                    &node.name,
                    "IntEnum",
                    body,
                    Doc::lines(&[
                        "An enumeration. An enum.IntEnum member is an int, so the names",
                        "and the values are one declaration rather than two.",
                        "",
                        "A member of this type is nonetheless annotated int and not this",
                        "class: an @appendable writer may send a value the reader's IDL",
                        "does not list, and Colour(7) raises ValueError.",
                    ]),
                ),
            });
        }

        TypeKind::Bitmask { bits } => {
            if ghost_created_for(applied, node.node_id, "PythonBitmask").is_none() {
                return Ok(());
            }
            let mut body = Vec::new();
            for bit in bits {
                let Ok(bit_node) = ast.bit_value(*bit) else {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "bitmask bit the model listed".to_string(),
                        at: node.name.clone(),
                    });
                };
                if ghost_created_for(applied, bit_node.node_id, "PythonBitConst").is_none() {
                    continue;
                }
                let position = ast.effective_bit_position(*bit).ok_or_else(|| {
                    MaterialiseError::Unevaluated {
                        construct: "bit position".to_string(),
                        at: bit_node.name.clone(),
                    }
                })?;
                // `1 << 40` is an `int` shift in Python and it is 2^40;
                // the Java column needs `1L` here because `1 << 40` is
                // an int shift there and evaluates to 256.
                body.push(Statement::Assign(Assign {
                    doc: renamed_note(&bit_node.name),
                    name: Ident::new(&escape_name(&bit_node.name)),
                    value: Expr::Binary {
                        op: "<<",
                        lhs: Box::new(Expr::int(1)),
                        rhs: Box::new(Expr::int(i128::from(position))),
                    },
                }));
            }
            items.push(Pending {
                module: node.module,
                statement: enum_class(
                    &node.name,
                    "IntFlag",
                    body,
                    Doc::lines(&[
                        "A bitmask. An enum.IntFlag member is an int and combines with |,",
                        "which is what a bitmask is for.",
                        "",
                        "A member of this type is annotated int: a writer may set a bit",
                        "this declaration does not name.",
                    ]),
                ),
            });
        }

        TypeKind::Bitset { .. } => {
            if ghost_created_for(applied, node.node_id, "PythonBitset").is_none() {
                return Ok(());
            }
            let mut body = Vec::new();
            // The base's bitfields are fields of the derived class too:
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
                body.push(field(field_name, bit_width(field_node.bits), false));
            }
            items.push(Pending {
                module: node.module,
                statement: dataclass(
                    &node.name,
                    body,
                    Doc::lines(&[
                        "A bitset. Each field states its own bit width in the metadata;",
                        "Python has no fixed-width integer, so nothing is rounded up to",
                        "a holder the way the C and Java columns have to.",
                        "",
                        "The packing into the anchor is the codec's.",
                    ]),
                ),
            });
        }

        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            let created = if matches!(node.kind, TypeKind::Valuetype { .. }) {
                "PythonValue"
            } else {
                "PythonStruct"
            };
            if ghost_created_for(applied, node.node_id, created).is_none() {
                return Ok(());
            }
            // XTypes 1.3 §7.5.1.4.1.3: the base is the first member `parent`, an
            // instance of the base's dataclass, and not Python inheritance: a dataclass
            // that inherited would also inherit the base's decorator arguments, which is
            // a second place the shape is decided. The wire still carries the base's
            // members first (§7.4.1.1.5.1).
            //
            // Every member is emitted, with no per-member rule lookup: a
            // member whose lookup missed would vanish from the class and
            // the file would still import. The C and Java columns take
            // the same view.
            let mut body = Vec::new();
            let mut spelled = Vec::new();
            if let Some(base) = super::c::struct_base(ast, id) {
                spelled.push((escape_name("parent"), "parent".to_string()));
                body.push(field(
                    "parent",
                    python_member_type(ast, base, "parent")?,
                    false,
                ));
            }
            for member in ast.members_of(id).iter().copied() {
                let Ok(member_node) = ast.member(member) else {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "member the model listed".to_string(),
                        at: node.name.clone(),
                    });
                };
                let Some(target) = member_node.type_.resolved() else {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "member type".to_string(),
                        at: member_node.name.clone(),
                    });
                };
                spelled.push((escape_name(&member_node.name), member_node.name.clone()));
                body.push(field(
                    &member_node.name,
                    python_member_type(ast, target, &member_node.name)?,
                    member_node.facets.is_optional(),
                ));
            }
            // `check_scope` catches this too, at the end, over the
            // rendered statements — but by then the IDL names are gone
            // and it can only name the spelling the two members share.
            // Here both are still in hand.
            no_spelling_collision(
                spelled
                    .iter()
                    .map(|(field, declared)| (field.as_str(), declared.as_str())),
                "Python",
                &node.name,
            )?;
            items.push(Pending {
                module: node.module,
                statement: dataclass(&node.name, body, Doc::none()),
            });
        }

        TypeKind::Union { discriminator, .. } => {
            if ghost_created_for(applied, node.node_id, "PythonUnion").is_none() {
                return Ok(());
            }
            let Some(disc) = discriminator.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "union discriminator".to_string(),
                    at: node.name.clone(),
                });
            };
            // A field per case and not one per type: two cases may carry
            // the same type, and Python has no untagged union to overlay
            // them with. Which field is live is what `_d` says.
            let mut body = vec![Statement::Annotated(Annotated {
                doc: Doc::line("The discriminator. Which case field is live is what this says."),
                name: Ident::new("_d"),
                type_: python_member_type(ast, disc, &node.name)?,
                value: None,
            })];
            for case in ast.cases_of(id) {
                let Ok(case_node) = ast.case(*case) else {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "union case the model listed".to_string(),
                        at: node.name.clone(),
                    });
                };
                if ghost_created_for(applied, case_node.node_id, "PythonCase").is_none() {
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
                // Optional and defaulting to None, because at most one
                // case is live: a required field per case would make
                // constructing the union mean supplying every branch.
                body.push(field(
                    &member.name,
                    python_member_type(ast, target, &member.name)?,
                    true,
                ));
            }
            items.push(Pending {
                module: node.module,
                statement: dataclass(
                    &node.name,
                    body,
                    Doc::lines(&[
                        "A union: the discriminator, and one field per case.",
                        "",
                        "Every case field is Optional and defaults to None, because at",
                        "most one of them is live. Which one is what _d says; Python has",
                        "no way to state that in a type.",
                    ]),
                ),
            });
        }

        // A native type is opaque and has no Python declaration — a
        // comment records the name and what a use of it takes.
        TypeKind::Native => {
            if ghost_created_for(applied, node.node_id, "PythonNative").is_none() {
                return Ok(());
            }
            items.push(Pending {
                module: node.module,
                statement: Statement::Comment(Doc::line(&format!(
                    "native {}; -- every use takes {}.{}",
                    node.name,
                    zerodds_lang_render::python::type_(&native_type()),
                    native_note(ast, id),
                ))),
            });
        }

        // A value box is a reference in Python already: the type, or
        // None. Nothing to declare.
        TypeKind::ValueBox { .. } => {}

        // An object reference belongs to the RPC arc and an annotation
        // type has no runtime form at all — `zerodds_ast_layout` refuses
        // to plan either — so a Python declaration would be a shape
        // nothing could serialise. `any` and `ValueBase` are in the
        // model because Building Block Any is one of IDL 4.2's fifteen,
        // and what they hold is decided at run time. All four are named
        // here rather than passed over, and `python_member_type` refuses
        // them by name where a member would carry one.
        TypeKind::ObjectRef { .. }
        | TypeKind::AnnotationType { .. }
        | TypeKind::Any
        | TypeKind::ValueBase => {}
    }
    Ok(())
}

/// The Python annotation of a value in a position that has to hold one,
/// refusing the constructs Python cannot hold.
///
/// `any` is the first of those. What it holds is decided at run time —
/// there is no Python type for it and no wire form to generate a codec
/// against. Emitting `a: object` into the class and reporting success
/// produces a field no encoder can write, from a generator that said
/// nothing was wrong. Python makes this refusal harder than the other
/// four columns do, because `object` would type-check.
///
/// # Errors
/// [`MaterialiseError::NoLanguageForm`] for `any`, `ValueBase`, an object
/// reference and an annotation type;
/// [`MaterialiseError::Unevaluated`] for a type or a bound the model will
/// not answer for.
pub fn python_member_type(
    ast: &Ast,
    target: Id<TypeNode>,
    at: &str,
) -> Result<Type, MaterialiseError> {
    type_at(ast, target, at, 0)
}

/// [`python_member_type`], carrying how deep the spelling has nested.
///
/// zerodds-lint: recursion-depth 64 (collection and typedef nesting;
/// bounded by `MAX_TYPE_DEPTH`, which returns
/// `MaterialiseError::Unevaluated` naming the spelling that nested)
#[allow(clippy::too_many_lines)]
fn type_at(ast: &Ast, id: Id<TypeNode>, at: &str, depth: usize) -> Result<Type, MaterialiseError> {
    if depth > MAX_TYPE_DEPTH {
        return Err(MaterialiseError::Unevaluated {
            construct: format!("type, whose Python spelling nests past {MAX_TYPE_DEPTH}"),
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
        // An object reference is a handle into the RPC arc and
        // `coverage.rs` says so; there is no Python data type for it.
        TypeKind::ObjectRef { .. } => Some("Object"),
        // An annotation type has no runtime form, so nothing declares
        // one and a reference to it would name a class that is not
        // there.
        TypeKind::AnnotationType { .. } => Some("annotation type"),
        _ => None,
    };
    if let Some(construct) = refused {
        return Err(MaterialiseError::NoLanguageForm {
            construct: construct.to_string(),
            language: "Python",
            at: at.to_string(),
        });
    }
    Ok(match &node.kind {
        TypeKind::Primitive(kind) => alias(primitive_alias(*kind)),
        TypeKind::String { width, bound } => annotated(
            Type::name("str"),
            call(
                "_IdlText",
                vec![
                    kw("bits", Expr::int(char_bits(*width))),
                    kw("bound", Expr::int(bound_of(ast, *bound, at)?)),
                ],
            ),
        ),
        TypeKind::Fixed { digits, scale } => annotated(
            Type::name("str"),
            call(
                "_IdlFixed",
                vec![
                    kw("digits", Expr::int(i128::from(*digits))),
                    kw("scale", Expr::int(i128::from(*scale))),
                ],
            ),
        ),
        TypeKind::Sequence { element, bound } => {
            let Some(element) = element.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "sequence element".to_string(),
                    at: at.to_string(),
                });
            };
            let list = Type::subscript(
                Path::single("list"),
                vec![type_at(ast, element, at, depth + 1)?],
            );
            match bound_of(ast, *bound, at)? {
                0 => list,
                length => annotated(
                    list,
                    call("_IdlSequence", vec![kw("bound", Expr::int(length))]),
                ),
            }
        }
        TypeKind::Array { element, dims } => {
            let Some(element) = element.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "array element".to_string(),
                    at: at.to_string(),
                });
            };
            let mut out = type_at(ast, element, at, depth + 1)?;
            // Innermost dimension first, so the outer one ends up
            // outermost: `long a[2][3]` is two rows of three.
            for dim in dims.iter().rev() {
                let length =
                    ast.eval_integral(*dim)
                        .map_err(|_| MaterialiseError::Unevaluated {
                            construct: "array length".to_string(),
                            at: at.to_string(),
                        })?;
                let length =
                    usize::try_from(length).map_err(|_| MaterialiseError::NoLanguageForm {
                        construct: format!("an array of {length} elements"),
                        language: "Python",
                        at: at.to_string(),
                    })?;
                // One tuple argument per element, so a static checker
                // rejects a literal of the wrong length. `tuple[T, ...]`
                // would say only that it is a tuple.
                out = Type::subscript(Path::single("tuple"), vec![out; length]);
            }
            out
        }
        TypeKind::Map { key, value, bound } => {
            let (Some(key), Some(value)) = (key.resolved(), value.resolved()) else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "map key or value".to_string(),
                    at: at.to_string(),
                });
            };
            let entry = Type::subscript(
                Path::single("tuple"),
                vec![
                    type_at(ast, key, at, depth + 1)?,
                    type_at(ast, value, at, depth + 1)?,
                ],
            );
            let list = Type::subscript(Path::single("list"), vec![entry]);
            match bound_of(ast, *bound, at)? {
                0 => list,
                length => annotated(
                    list,
                    call("_IdlSequence", vec![kw("bound", Expr::int(length))]),
                ),
            }
        }
        // A typedef is transparent: the use takes the target's type.
        TypeKind::Alias { aliased } => {
            let Some(target) = aliased.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "typedef target".to_string(),
                    at: at.to_string(),
                });
            };
            type_at(ast, target, at, depth + 1)?
        }
        // An `int`, with the enumeration named in the metadata. See the
        // module documentation: `Colour(7)` raises, and an @appendable
        // writer sends exactly that.
        TypeKind::Enumeration { .. } | TypeKind::Bitmask { .. } => annotated(
            Type::name("int"),
            call(
                "_IdlEnum",
                vec![
                    kw("declared", Expr::Path(qualified(ast, id)?)),
                    kw("bits", Expr::int(i128::from(ast.effective_bit_bound(id)))),
                ],
            ),
        ),
        TypeKind::Structure { .. }
        | TypeKind::Union { .. }
        | TypeKind::Bitset { .. }
        | TypeKind::Valuetype { .. } => Type::Name(qualified(ast, id)?),
        TypeKind::Native => native_type(),
        // A boxed value is a reference, absent when None — which is what
        // `typing.Optional` says.
        TypeKind::ValueBox { boxed: inner } => {
            let Some(target) = inner.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "value box target".to_string(),
                    at: at.to_string(),
                });
            };
            optional(type_at(ast, target, at, depth + 1)?)
        }
        TypeKind::Any
        | TypeKind::ValueBase
        | TypeKind::ObjectRef { .. }
        | TypeKind::AnnotationType { .. } => {
            // Refused above; the arm keeps the match total, so a kind
            // added to the model without a Python answer is a compile
            // error and not a field that silently takes another's type.
            return Err(MaterialiseError::NoLanguageForm {
                construct: node.kind.as_str().to_string(),
                language: "Python",
                at: at.to_string(),
            });
        }
    })
}

/// The alias an IDL primitive takes.
///
/// Exhaustive, so a primitive added to the model without a Python
/// spelling is a compile error and not a member that silently takes
/// another's.
#[must_use]
pub const fn primitive_alias(kind: PrimitiveKind) -> &'static str {
    match kind {
        PrimitiveKind::Boolean => "bool",
        PrimitiveKind::Char => CHAR,
        PrimitiveKind::WChar => WCHAR,
        PrimitiveKind::Int8 => "_Int8",
        PrimitiveKind::Octet | PrimitiveKind::UInt8 => "_UInt8",
        PrimitiveKind::Int16 => "_Int16",
        PrimitiveKind::UInt16 => "_UInt16",
        PrimitiveKind::Int32 => INT32,
        PrimitiveKind::UInt32 => "_UInt32",
        PrimitiveKind::Int64 => "_Int64",
        PrimitiveKind::UInt64 => "_UInt64",
        PrimitiveKind::Float32 => "_Float32",
        PrimitiveKind::Float64 => "_Float64",
        // A `str` holding the decimal, and never a Python `float`: see
        // the module documentation.
        PrimitiveKind::Float128 => "_Float128",
    }
}

/// The alias `long` takes, which is also what an unresolved constant
/// falls back to.
const INT32: &str = "_Int32";
/// The alias an IDL `char` takes.
const CHAR: &str = "_Char";
/// The alias an IDL `wchar` takes.
const WCHAR: &str = "_WChar";

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

/// What a `native` declaration maps to.
///
/// `object`, always. A `#pragma` or an annotation may give a mapping
/// spelling, and it is written into the comment rather than into the
/// annotation: a Python annotation naming a class this file does not
/// import would not resolve, and a generated module imports only
/// `dataclasses`, `enum` and `typing`.
fn native_type() -> Type {
    Type::name("object")
}

/// The mapping a `native` declaration named, as a clause of its comment.
fn native_note(ast: &Ast, id: Id<TypeNode>) -> String {
    let mapping = ast
        .any_of_type(id)
        .ok()
        .and_then(|any| ast.facets(any))
        .and_then(|facets| facets.corba.as_ref())
        .and_then(|corba| corba.native_mapping.clone());
    match mapping {
        Some(spelling) => format!(
            " The source named the mapping `{spelling}`, which this module does not import."
        ),
        None => String::new(),
    }
}

/// A generated type's path, written from the module root down.
///
/// `a.b.Point` and not `Point`: a nested module is a class, and a class
/// body is not a scope an annotation is resolved against.
///
/// # Errors
/// [`MaterialiseError::NoModule`] when the model will not answer for a
/// module the type nests in.
fn qualified(ast: &Ast, id: Id<TypeNode>) -> Result<Path, MaterialiseError> {
    let node = ast.type_(id).map_err(|_| MaterialiseError::Unevaluated {
        construct: "type".to_string(),
        at: "the module root".to_string(),
    })?;
    let mut segments = Vec::new();
    for (_, name) in module_chain(ast, node.module)? {
        segments.push(name);
    }
    segments.push(escape_name(&node.name));
    Ok(Path::new(
        &segments.iter().map(String::as_str).collect::<Vec<&str>>(),
    ))
}

/// The path a generated declaration of `id` is reached by.
///
/// `qualified` under a public name, because the codec backend needs it
/// and is in a different module. A codec sits at the file root, where a
/// class nested in `class a: class b:` is `a.b.Point` and `Point` names
/// nothing — the same reason the annotations this file writes are
/// qualified.
///
/// # Errors
/// As `qualified`.
pub fn declared_path(ast: &Ast, id: Id<TypeNode>) -> Result<Path, MaterialiseError> {
    qualified(ast, id)
}

/// The bound a `string`, a `sequence` or a `map` carries, as the suffix
/// that keeps two of them apart.
///
/// The tag names the routine a hoisted collection's codec is emitted
/// under, and one body is emitted per name. With the bound left out,
/// `sequence<long>` and `sequence<long, 4>` are one name and whichever
/// was declared first decides: measured on
/// `@final struct S { sequence<long> b; sequence<long, 4> a; };`, the
/// generated Python held no `zd_over` check at all, so `a` accepted a
/// stream of any length; with the members the other way round both got
/// the bounded body, so `b` refused — and truncated — a length the IDL
/// puts no limit on. `string<4>` beside `string` and `wstring<4>`
/// beside `wstring` collapse the same way, onto the trimming call.
/// Either way the reader answered with a value the IDL does not
/// describe and said nothing, which is why the bound is part of the
/// name. The same suffix the Zig column carries.
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

/// The name a codec function takes for the type it codes.
///
/// The **IDL kind** and not the Python spelling, and in this column that
/// distinction does more work than in any other: `octet`, `unsigned
/// short`, `long` and `unsigned long long` are all `int` in Python, and
/// they are one, two, four and eight octets on the wire. A tag built
/// from the Python type would give every one of them the same codec, and
/// the first type reached would decide the width for the rest with
/// nothing to say it had.
///
/// An alias is followed through: two IDL names for one shape are one
/// wire form, and two codecs for it would be two chances to differ.
///
/// # Errors
/// [`MaterialiseError`] when a module will not answer, or when the name
/// nests past `MAX_TYPE_DEPTH`.
///
/// zerodds-lint: recursion-depth 64 (collection nesting; bounded by `MAX_TYPE_DEPTH`)
pub fn type_tag(ast: &Ast, id: Id<TypeNode>, depth: usize) -> Result<String, MaterialiseError> {
    if depth > MAX_TYPE_DEPTH {
        return Err(MaterialiseError::Unevaluated {
            construct: format!("type, whose codec name nests past {MAX_TYPE_DEPTH}"),
            at: "the module root".to_string(),
        });
    }
    let Ok(node) = ast.type_(id) else {
        return Ok("unknown".to_string());
    };
    Ok(match &node.kind {
        TypeKind::Primitive(kind) => kind.as_str().replace(' ', "_"),
        TypeKind::String {
            width: CharWidth::Char,
            bound,
        } => format!("string{}", bound_tag(ast, bound)),
        TypeKind::String {
            width: CharWidth::WChar,
            bound,
        } => format!("wstring{}", bound_tag(ast, bound)),
        TypeKind::Fixed { digits, scale } => format!("fixed_{digits}_{scale}"),
        TypeKind::Sequence { element, bound } => match element.resolved() {
            Some(element) => format!(
                "seq{}_{}",
                bound_tag(ast, bound),
                type_tag(ast, element, depth + 1)?
            ),
            None => format!("seq_unknown{}", bound_tag(ast, bound)),
        },
        TypeKind::Array { element, dims } => {
            let mut name = match element.resolved() {
                Some(element) => format!("arr_{}", type_tag(ast, element, depth + 1)?),
                None => "arr_unknown".to_string(),
            };
            for dim in dims {
                match ast.eval_integral(*dim) {
                    Ok(length) => name.push_str(&format!("_{length}")),
                    // No number rather than a zero: the name has to
                    // differ from a `[0]` array's, and `declare` reports
                    // the dimension that would not evaluate by name.
                    Err(_) => name.push_str("_x"),
                }
            }
            name
        }
        TypeKind::Map { key, value, bound } => {
            let half = |half: Option<Id<TypeNode>>| match half {
                Some(target) => type_tag(ast, target, depth + 1),
                None => Ok("unknown".to_string()),
            };
            format!(
                "map{}_{}_{}",
                bound_tag(ast, bound),
                half(key.resolved())?,
                half(value.resolved())?
            )
        }
        TypeKind::Alias { aliased } => match aliased.resolved() {
            Some(target) => type_tag(ast, target, depth + 1)?,
            None => "unknown".to_string(),
        },
        TypeKind::ValueBox { boxed } => match boxed.resolved() {
            Some(target) => format!("box_{}", type_tag(ast, target, depth + 1)?),
            None => "box_unknown".to_string(),
        },
        TypeKind::Structure { .. }
        | TypeKind::Union { .. }
        | TypeKind::Enumeration { .. }
        | TypeKind::Bitmask { .. }
        | TypeKind::Bitset { .. }
        | TypeKind::Valuetype { .. }
        | TypeKind::Native => {
            let mut segments = Vec::new();
            for (_, name) in module_chain(ast, node.module)? {
                segments.push(name);
            }
            segments.push(escape_name(&node.name));
            segments.join("_")
        }
        TypeKind::Any => "any".to_string(),
        TypeKind::ValueBase => "valuebase".to_string(),
        TypeKind::ObjectRef { .. } => "objectref".to_string(),
        TypeKind::AnnotationType { .. } => "annotation".to_string(),
    })
}

// -- building the tree ----------------------------------------------------

/// `name = typing.Annotated[…]`, referred to by its alias.
fn alias(name: &'static str) -> Type {
    Type::name(name)
}

/// `typing.Annotated[base, marker]`.
fn annotated(base: Type, marker: Expr) -> Type {
    Type::Annotated {
        base: Box::new(base),
        markers: vec![marker],
    }
}

/// `typing.Optional[inner]`.
fn optional(inner: Type) -> Type {
    Type::Optional(Box::new(inner))
}

/// `name(args)`.
fn call(name: &str, args: Vec<Arg>) -> Expr {
    Expr::Call {
        func: Path::single(name),
        args,
    }
}

/// `name=value`, as a call argument.
fn kw(name: &str, value: Expr) -> Arg {
    Arg::Keyword(Keyword {
        name: Ident::new(name),
        value,
    })
}

/// The metadata of an integer of `bits` bits, unsigned.
///
/// What a bitset's bitfield takes. The declared holder of
/// `bitfield<1, boolean>` names a range and not a wire form, and the
/// width is what the packing uses.
fn bit_width(bits: u16) -> Type {
    annotated(
        Type::name("int"),
        call(
            "_IdlInt",
            vec![
                kw("bits", Expr::int(i128::from(bits))),
                kw("signed", Expr::Literal(Literal::Bool(false))),
            ],
        ),
    )
}

/// One dataclass field.
///
/// An `@optional` member defaults to `None` and a required one has no
/// default at all, so constructing an aggregate without a required
/// member is an error rather than a silently zero-filled sample.
///
/// A member whose name was escaped carries the IDL name in
/// `dataclasses.field(metadata=…)`, which is machine-readable — a
/// comment would not be.
fn field(name: &str, type_: Type, is_optional: bool) -> Statement {
    let escaped = escape_name(name);
    let renamed = escaped != name;
    let type_ = if is_optional { optional(type_) } else { type_ };
    let value = match (is_optional, renamed) {
        (false, false) => None,
        (true, false) => Some(Expr::Literal(Literal::Null)),
        (_, true) => {
            let mut args = Vec::new();
            if is_optional {
                args.push(kw("default", Expr::Literal(Literal::Null)));
            }
            args.push(kw(
                "metadata",
                Expr::Dict(vec![(
                    Expr::Literal(Literal::string("idl")),
                    Expr::Literal(Literal::string(name)),
                )]),
            ));
            Some(Expr::Call {
                func: Path::new(&["dataclasses", "field"]),
                args,
            })
        }
    };
    Statement::Annotated(Annotated {
        doc: Doc::none(),
        name: Ident::new(&escaped),
        type_,
        value,
    })
}

/// `@dataclasses.dataclass(kw_only=True) class <name>: …`
///
/// `kw_only=True` because the field order is the IDL's and the defaults
/// are not: an `@optional` member defaults to `None` and the member
/// after it does not, and a positional dataclass refuses a field with no
/// default after one with a default. Reordering the fields to satisfy
/// that would make the class's field order differ from the IDL's, which
/// is the order the wire uses.
///
/// Mutable and comparing by value: the defaults. A frozen dataclass
/// could not be filled in as a sample is decoded, and `eq=True` is what
/// makes two decodes of one sample compare equal, which is what a
/// round-trip test asserts.
fn dataclass(name: &str, body: Vec<Statement>, doc: Doc) -> Statement {
    let escaped = escape_name(name);
    let mut doc = doc;
    if escaped != name && doc.is_empty() {
        doc = Doc::line(&format!("IDL name: {name}"));
    } else if escaped != name {
        doc.push("");
        doc.push(&format!("IDL name: {name}"));
    }
    Statement::Class(Class {
        doc,
        decorators: vec![Decorator {
            path: Path::new(&["dataclasses", "dataclass"]),
            args: vec![kw("kw_only", Expr::Literal(Literal::Bool(true)))],
        }],
        name: Ident::new(&escaped),
        bases: Vec::new(),
        body: if body.is_empty() {
            vec![Statement::Pass]
        } else {
            body
        },
    })
}

/// `class <name>(enum.<base>): …`
fn enum_class(name: &str, base: &str, body: Vec<Statement>, doc: Doc) -> Statement {
    let escaped = escape_name(name);
    let mut doc = doc;
    if escaped != name {
        doc.push("");
        doc.push(&format!("IDL name: {name}"));
    }
    Statement::Class(Class {
        doc,
        decorators: Vec::new(),
        name: Ident::new(&escaped),
        bases: vec![Type::Name(Path::new(&["enum", base]))],
        // An IDL enumeration with no enumerators is legal in the model
        // and a class with an empty body is not legal Python.
        body: if body.is_empty() {
            vec![Statement::Pass]
        } else {
            body
        },
    })
}

/// The comment a renamed declaration carries, and nothing when it kept
/// its name.
fn renamed_note(name: &str) -> Doc {
    if escape_name(name) == name {
        return Doc::none();
    }
    Doc::line(&format!("IDL name: {name}"))
}

// -- the preamble ---------------------------------------------------------

/// The marker classes and the width aliases every generated module
/// carries.
///
/// The same in every file, whether or not a given alias is used. Two
/// reasons, and the second is the one that decides it:
///
/// - the preamble is the vocabulary the rest of the file is written in,
///   so a reader learns it once and a diff between two generated modules
///   is only the declarations;
/// - it is the contract with the codec pass. A module that dropped
///   `_IdlFixed` because no member happened to be a `fixed` would make
///   `getattr(module, "_IdlFixed")` succeed or fail depending on the
///   IDL, which is a dependency on the input that nothing declares.
///
/// The markers are per-module classes, so `_IdlInt` in two generated
/// files are two classes. That is deliberate: a shared one would make
/// every generated module import a runtime package, and this layer emits
/// types and nothing else.
fn prelude() -> Vec<Statement> {
    let mut out = vec![
        marker(
            "_IdlInt",
            &[("bits", "int"), ("signed", "bool")],
            Doc::lines(&[
                "The IDL integer an int annotation stands for.",
                "",
                "A Python int is arbitrary precision and an IDL long is 32 bits.",
                "Nothing in Python enforces the difference, so it is stated here",
                "instead: read it back with typing.get_type_hints(cls,",
                "include_extras=True).",
            ]),
        ),
        marker(
            "_IdlFloat",
            &[("bits", "int")],
            Doc::lines(&[
                "The IDL floating-point type a float or str annotation stands for.",
                "",
                "bits=128 goes with a str and not a float: a Python float is a",
                "binary64 and would drop sixty bits of the significand at the",
                "assignment.",
            ]),
        ),
        marker(
            "_IdlText",
            &[("bits", "int"), ("bound", "int")],
            Doc::lines(&[
                "An IDL string, wstring, char or wchar, as a Python str.",
                "",
                "bits is the width of one character on the wire, 8 or 16. bound is",
                "the declared maximum length, or 0 for an unbounded one.",
            ]),
        ),
        marker(
            "_IdlFixed",
            &[("digits", "int"), ("scale", "int")],
            Doc::lines(&[
                "An IDL fixed<digits, scale>, as the decimal text.",
                "",
                "Not a decimal.Decimal: Decimal('0.00') keeps one digit and a",
                "scale, so the digit count the FixedDigitCount axis measures would",
                "be the container's answer rather than the source's.",
            ]),
        ),
        marker(
            "_IdlSequence",
            &[("bound", "int")],
            Doc::line("The declared maximum length of a bounded sequence or map."),
        ),
        marker(
            "_IdlEnum",
            &[("declared", "type[enum.Enum]"), ("bits", "int")],
            Doc::lines(&[
                "The enumeration or bitmask an int annotation stands for.",
                "",
                "The member is an int because a writer may send a value this",
                "declaration does not list, which enum.IntEnum would refuse.",
            ]),
        ),
    ];
    for (name, bits, signed) in [
        ("_Int8", 8, true),
        ("_UInt8", 8, false),
        ("_Int16", 16, true),
        ("_UInt16", 16, false),
        ("_Int32", 32, true),
        ("_UInt32", 32, false),
        ("_Int64", 64, true),
        ("_UInt64", 64, false),
    ] {
        out.push(Statement::Assign(Assign {
            doc: Doc::none(),
            name: Ident::new(name),
            value: type_value(annotated(
                Type::name("int"),
                call(
                    "_IdlInt",
                    vec![
                        kw("bits", Expr::int(bits)),
                        kw("signed", Expr::Literal(Literal::Bool(signed))),
                    ],
                ),
            )),
        }));
    }
    for (name, base, bits) in [
        ("_Float32", "float", 32),
        ("_Float64", "float", 64),
        // A binary128 in a Python float would lose sixty bits of the
        // significand at the assignment; the decimal text loses nothing.
        ("_Float128", "str", 128),
    ] {
        out.push(Statement::Assign(Assign {
            doc: Doc::none(),
            name: Ident::new(name),
            value: type_value(annotated(
                Type::name(base),
                call("_IdlFloat", vec![kw("bits", Expr::int(bits))]),
            )),
        }));
    }
    for (name, bits) in [(CHAR, 8), (WCHAR, 16)] {
        out.push(Statement::Assign(Assign {
            doc: Doc::none(),
            name: Ident::new(name),
            value: type_value(annotated(
                Type::name("str"),
                call(
                    "_IdlText",
                    vec![kw("bits", Expr::int(bits)), kw("bound", Expr::int(1))],
                ),
            )),
        }));
    }
    out
}

/// One frozen marker dataclass.
fn marker(name: &str, fields: &[(&str, &str)], doc: Doc) -> Statement {
    Statement::Class(Class {
        doc,
        decorators: vec![Decorator {
            path: Path::new(&["dataclasses", "dataclass"]),
            args: vec![kw("frozen", Expr::Literal(Literal::Bool(true)))],
        }],
        name: Ident::new(name),
        bases: Vec::new(),
        body: fields
            .iter()
            .map(|(field_name, type_name)| {
                Statement::Annotated(Annotated {
                    doc: Doc::none(),
                    name: Ident::new(field_name),
                    type_: Type::Name(Path::new(&type_name.split('.').collect::<Vec<&str>>())),
                    value: None,
                })
            })
            .collect(),
    })
}

/// A type written where a value belongs, which is what a type alias is.
fn type_value(type_: Type) -> Expr {
    Expr::Annotation(Box::new(type_))
}

// -- constants ------------------------------------------------------------

/// The IDL primitive a type resolves to, when it resolves to one.
///
/// What a constant's range check needs. A `const long` has to fit 32
/// bits whatever Python's `int` could hold, because the wire is 32 bits
/// wide.
fn scalar_kind(ast: &Ast, id: Id<TypeNode>) -> Option<PrimitiveKind> {
    let mut current = id;
    for _ in 0..MAX_TYPE_DEPTH {
        let node = ast.type_(current).ok()?;
        match &node.kind {
            TypeKind::Primitive(kind) => return Some(*kind),
            TypeKind::Alias { aliased } => current = aliased.resolved()?,
            _ => return None,
        }
    }
    None
}

/// The Python expression a constant's value takes.
///
/// The declared type already carries the string-shaped forms: a
/// `string`, a `wstring`, a `fixed` and a `long double` are all a Python
/// `str` here. What this has to do is pick the right literal for each,
/// and refuse a value the IDL type does not hold.
///
/// **A range check even though Python needs none.** A Python `int` holds
/// 2^70 perfectly well, and `const octet X = 300;` would compile and
/// then be written as one byte. So the range is checked against the
/// **IDL** type and not against Python's, which is what keeps this
/// column's failure set comparable with the other four's.
///
/// # Errors
/// [`MaterialiseError`] naming the constant and what will not hold it.
fn constant_expr(
    kind: Option<PrimitiveKind>,
    value: &zerodds_ast::ConstValue,
    at: &str,
) -> Result<Expr, MaterialiseError> {
    use zerodds_ast::ConstValue;
    let refuse = |construct: String| MaterialiseError::NoLanguageForm {
        construct,
        language: "Python",
        at: at.to_string(),
    };
    match value {
        ConstValue::Integer(number) => {
            let (low, high, name) = integer_range(kind);
            Ok(Expr::int(in_range(*number, low, high, name, at)?))
        }
        ConstValue::Boolean(flag) => Ok(Expr::Literal(Literal::Bool(*flag))),
        // Python has no character type, so both are a string of one.
        // Unlike Java there is no code point a Python `str` cannot hold,
        // so a `wchar` above the Basic Multilingual Plane is written
        // rather than refused.
        ConstValue::Char(character) => Ok(Expr::Literal(Literal::Char(*character))),
        ConstValue::WChar(point) => match constant::wchar_char(*point) {
            Some(character) => Ok(Expr::Literal(Literal::Char(character))),
            None => Err(refuse(format!(
                "the code point U+{point:04X}, which is not a scalar value"
            ))),
        },
        ConstValue::Float { mantissa, exponent } => {
            let text = constant::float_text(mantissa, exponent.as_deref());
            // A `long double` constant is the decimal text, because the
            // member that holds one is a `str`. Parsing it into a Python
            // `float` here would round it at the point of generation.
            if matches!(kind, Some(PrimitiveKind::Float128)) {
                return Ok(Expr::Literal(Literal::Str(text)));
            }
            Ok(Expr::Literal(Literal::Float(text)))
        }
        ConstValue::Fixed { digits, scale } => Ok(Expr::Literal(Literal::Str(
            constant::fixed_decimal(digits, *scale),
        ))),
        ConstValue::String(text) => Ok(Expr::Literal(Literal::Str(text.clone()))),
        ConstValue::WString(points) => match constant::wstring_utf8(points) {
            Some(text) => Ok(Expr::Literal(Literal::Str(text))),
            None => Err(refuse(String::from(
                "a `wstring` constant holding a lone surrogate",
            ))),
        },
        // `ConstValue` is `#[non_exhaustive]`, so a form added to the
        // model reaches here. Refused by name rather than given a
        // plausible default: a constant emitted as the wrong literal is
        // a value that disagrees with the IDL and imports cleanly.
        _ => Err(refuse(String::from(
            "a constant form this binding has no literal for",
        ))),
    }
}

/// The range an integer constant of `kind` has to fit, and the name to
/// report when it does not.
///
/// A type the model would not resolve falls back to the widest IDL
/// integer rather than to Python's unbounded one: something has to bound
/// it, and 64 bits is the widest thing the wire carries.
const fn integer_range(kind: Option<PrimitiveKind>) -> (i128, i128, &'static str) {
    match kind {
        Some(PrimitiveKind::Int8) => (-128, 127, "int8"),
        Some(PrimitiveKind::Octet | PrimitiveKind::UInt8) => (0, 255, "octet"),
        Some(PrimitiveKind::Int16) => (i16::MIN as i128, i16::MAX as i128, "short"),
        Some(PrimitiveKind::UInt16) => (0, u16::MAX as i128, "unsigned short"),
        Some(PrimitiveKind::Int32) => (i32::MIN as i128, i32::MAX as i128, "long"),
        Some(PrimitiveKind::UInt32) => (0, u32::MAX as i128, "unsigned long"),
        Some(PrimitiveKind::UInt64) => (0, u64::MAX as i128, "unsigned long long"),
        Some(PrimitiveKind::Char) => (0, 255, "char"),
        Some(PrimitiveKind::WChar) => (0, u16::MAX as i128, "wchar"),
        Some(PrimitiveKind::Boolean) => (0, 1, "boolean"),
        _ => (i64::MIN as i128, i64::MAX as i128, "long long"),
    }
}

/// `value`, or the failure that says it does not fit.
///
/// # Errors
/// [`MaterialiseError::NoLanguageForm`], naming the value and the IDL
/// type that will not hold it. Truncating instead would put a number
/// nobody wrote into the output — and Python would not even notice,
/// which is why this check is here and not left to the language.
fn in_range(
    value: i128,
    low: i128,
    high: i128,
    type_: &str,
    at: &str,
) -> Result<i128, MaterialiseError> {
    if value < low || value > high {
        return Err(MaterialiseError::NoLanguageForm {
            construct: format!("the value {value}, which an IDL `{type_}` does not hold"),
            language: "Python",
            at: at.to_string(),
        });
    }
    Ok(value)
}

/// A model node's name, or its identity when the node is anonymous.
///
/// An array type declared inline has no name of its own, and an error
/// that said only "the array length does not evaluate" would leave the
/// reader to find which array.
fn named(name: &str, node: NodeId) -> String {
    if name.is_empty() {
        format!("{node:?}")
    } else {
        name.to_string()
    }
}

/// Whether the rules created a node of `type_name` for `node`.
///
/// The same two-hop walk the sibling materialisers do: a correspondence
/// is an ordinary node with a link to the anchor and a link to what was
/// created.
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

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;
    use crate::rule;
    use crate::{ModelGraph, apply};
    use std::path::{Path as FsPath, PathBuf};

    /// IDL in, one Python module out.
    fn render(idl: &str, path: &str) -> Result<String, String> {
        let parsed = zerodds_idl_parse::parse(idl);
        if parsed.has_errors() {
            return Err(format!("parse: {:?}", parsed.diagnostics()));
        }
        let lowered = zerodds_idl_lower::lower(&parsed.syntax(), path);
        if !lowered.diagnostics.is_empty() {
            return Err(format!("lower: {:?}", lowered.diagnostics));
        }
        let mut ast = lowered.ast;
        ast.resolve_all_by_name()
            .map_err(|error| format!("resolve: {error}"))?;
        let applied = apply(&ModelGraph::of(&ast), &rule::python::catalogue())
            .map_err(|error| format!("apply: {error}"))?;
        let module =
            python_module(&ast, &applied).map_err(|error| format!("materialise: {error}"))?;
        Ok(zerodds_lang_render::python::module(&module))
    }

    /// The same, for a source with no file behind it.
    fn declarations(idl: &str) -> String {
        match render(idl, "test.idl") {
            Ok(text) => text,
            Err(why) => panic!("{why}"),
        }
    }

    /// Every corpus case, by name.
    fn corpus() -> Vec<(String, PathBuf)> {
        let root = FsPath::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/interop-corpus");
        let mut out = Vec::new();
        for entry in std::fs::read_dir(&root).expect("the corpus directory") {
            let entry = entry.expect("a corpus entry");
            let idl = entry.path().join("type.idl");
            if idl.is_file() {
                out.push((entry.file_name().to_string_lossy().into_owned(), idl));
            }
        }
        out.sort();
        out
    }

    /// The whole corpus, through every layer, with nothing skipped.
    ///
    /// A rendered file is not a compiled one. `ZD_PYTHON_OUT` names a
    /// directory to write the modules into, and `python3 -m py_compile`
    /// plus `mypy --strict` over that directory is the second half of
    /// the check — a module that does not compile or does not
    /// type-check is a materialiser defect, not a corpus defect.
    #[test]
    fn every_corpus_case_renders_or_names_its_failure() {
        let cases = corpus();
        assert!(
            cases.len() > 300,
            "the corpus is not where it was: {} cases",
            cases.len()
        );
        let out = std::env::var("ZD_PYTHON_OUT").ok();
        let mut failures = Vec::new();
        for (name, path) in cases {
            let source = std::fs::read_to_string(&path).expect("a corpus source");
            match render(&source, &path.display().to_string()) {
                Ok(text) => {
                    if let Some(dir) = &out {
                        let case = FsPath::new(dir).join(&name);
                        std::fs::create_dir_all(&case).expect("the output directory");
                        std::fs::write(case.join("zd_types.py"), text).expect("the output file");
                    }
                }
                Err(why) => failures.push(format!("{name}: {why}")),
            }
        }
        let failures = crate::adapter::without_the_front_end_refusals(failures);
        assert!(failures.is_empty(), "{failures:#?}");
    }

    #[test]
    fn every_idl_primitive_reaches_an_annotation_that_states_its_width() {
        // The decision this column turns on: a Python `int` is
        // arbitrary precision, so the width has to be somewhere, and
        // `x: int` is never written.
        let generated = declarations(
            "@final struct P {
               boolean a; octet b; char c; wchar d;
               int8 e; uint8 f; short g; unsigned short h;
               long i; unsigned long j; long long k; unsigned long long l;
               float m; double n; long double o;
             };",
        );
        for expected in [
            "a: bool",
            "b: _UInt8",
            "c: _Char",
            "d: _WChar",
            "e: _Int8",
            "f: _UInt8",
            "g: _Int16",
            "h: _UInt16",
            "i: _Int32",
            "j: _UInt32",
            "k: _Int64",
            "l: _UInt64",
            "m: _Float32",
            "n: _Float64",
            "o: _Float128",
        ] {
            assert!(generated.contains(expected), "{expected}\n{generated}");
        }
        // Scoped to the aggregate: the marker dataclasses in the
        // preamble are the one place a bare `int` belongs.
        let body = generated
            .split_once("class P:")
            .map(|(_, rest)| rest.to_string())
            .expect("the aggregate");
        assert!(
            !body.contains(": int\n"),
            "a bare int reached a field\n{body}"
        );
        assert!(
            !body.contains(": float\n"),
            "a bare float reached a field\n{body}"
        );
        assert!(
            !body.contains(": str\n"),
            "a bare str reached a field\n{body}"
        );
    }

    #[test]
    fn every_width_alias_states_its_bits_and_its_signedness() {
        let generated = declarations("@final struct P { long x; };");
        assert!(
            generated.contains("_Int32 = typing.Annotated[int, _IdlInt(bits=32, signed=True)]"),
            "{generated}"
        );
        assert!(
            generated.contains("_UInt64 = typing.Annotated[int, _IdlInt(bits=64, signed=False)]"),
            "{generated}"
        );
    }

    #[test]
    fn a_long_double_is_the_decimal_text_and_never_a_python_float() {
        // A Python float is a binary64. Writing a binary128 into one
        // drops sixty bits of the significand at the assignment, before
        // any codec runs.
        let generated = declarations("@final struct P { long double x; };");
        assert!(
            generated.contains("_Float128 = typing.Annotated[str, _IdlFloat(bits=128)]"),
            "{generated}"
        );
        assert!(generated.contains("x: _Float128"), "{generated}");
    }

    #[test]
    fn a_sequence_is_a_list_and_an_array_is_a_tuple_of_its_length() {
        let generated = declarations(
            "@final struct P { sequence<long> s; long a[3]; long g[2][3]; sequence<string> t; };",
        );
        assert!(generated.contains("s: list[_Int32]"), "{generated}");
        assert!(
            generated.contains("a: tuple[_Int32, _Int32, _Int32]"),
            "{generated}"
        );
        // Outer dimension outermost: two rows of three.
        assert!(
            generated
                .contains("g: tuple[tuple[_Int32, _Int32, _Int32], tuple[_Int32, _Int32, _Int32]]"),
            "{generated}"
        );
        assert!(
            generated.contains("t: list[typing.Annotated[str,"),
            "{generated}"
        );
    }

    #[test]
    fn a_bound_reaches_the_annotation_and_an_unbounded_one_leaves_it_alone() {
        let generated = declarations(
            "@final struct P { sequence<long, 5> s; sequence<long> u; string<7> t; };",
        );
        assert!(
            generated.contains("s: typing.Annotated[list[_Int32], _IdlSequence(bound=5)]"),
            "{generated}"
        );
        assert!(generated.contains("u: list[_Int32]"), "{generated}");
        assert!(
            generated.contains("t: typing.Annotated[str, _IdlText(bits=8, bound=7)]"),
            "{generated}"
        );
    }

    #[test]
    fn a_map_is_a_list_of_pairs_and_never_a_dict() {
        // A generated struct is an `eq=True` dataclass, so its
        // `__hash__` is None and `dict[Point, int]` is a type that
        // cannot hold anything. And the entry order is what the
        // MapEntryOrder axis measures.
        let generated = declarations("@final struct P { map<long, string> m; };");
        assert!(generated.contains("m: list[tuple[_Int32,"), "{generated}");
        assert!(!generated.contains("dict["), "{generated}");
    }

    #[test]
    fn an_optional_member_is_optional_and_defaults_to_none() {
        // XTypes 1.3 7.4.1.2.3 makes absence the value, not a zero.
        let generated =
            declarations("@mutable struct P { @optional long a; @optional string b; long c; };");
        assert!(
            generated.contains("a: typing.Optional[_Int32] = None"),
            "{generated}"
        );
        assert!(
            generated.contains("b: typing.Optional[typing."),
            "{generated}"
        );
        // A required member has no default at all, so constructing the
        // aggregate without it is an error rather than a silently
        // zero-filled sample.
        assert!(generated.contains("c: _Int32\n"), "{generated}");
    }

    #[test]
    fn a_dataclass_is_keyword_only_because_the_defaults_are_not_in_idl_order() {
        // An @optional member defaults to None and the member after it
        // does not; a positional dataclass refuses that, and reordering
        // the fields would make the class disagree with the wire.
        let generated = declarations("@mutable struct P { @optional long a; long b; };");
        assert!(
            generated.contains("@dataclasses.dataclass(kw_only=True)"),
            "{generated}"
        );
    }

    #[test]
    fn an_enumeration_is_an_intenum_and_a_member_of_it_is_an_int() {
        let generated = declarations(
            "enum Colour { RED, GREEN, BLUE };
             @final struct P { Colour c; };",
        );
        assert!(
            generated.contains("class Colour(enum.IntEnum):"),
            "{generated}"
        );
        assert!(generated.contains("RED = 0"), "{generated}");
        assert!(generated.contains("BLUE = 2"), "{generated}");
        // An @appendable writer may send a value this IDL does not
        // list, and `Colour(7)` raises ValueError.
        assert!(
            generated.contains("c: typing.Annotated[int, _IdlEnum(declared=Colour, bits=32)]"),
            "{generated}"
        );
    }

    #[test]
    fn an_explicit_enumerator_value_is_written_as_the_source_wrote_it() {
        // The Java column has to fold 70000 onto the pattern an `int`
        // occupies; a Python `int` holds it, so nothing folds.
        let generated = declarations("enum E { @value(-1) LOW, @value(70000) HIGH };");
        assert!(generated.contains("LOW = -1"), "{generated}");
        assert!(generated.contains("HIGH = 70000"), "{generated}");
    }

    #[test]
    fn a_bitmask_is_an_intflag_and_a_wide_shift_needs_no_suffix() {
        let generated =
            declarations("@bit_bound(64) bitmask Big { @position(0) A, @position(40) B };");
        assert!(
            generated.contains("class Big(enum.IntFlag):"),
            "{generated}"
        );
        // `1 << 40` is 2^40 in Python. The Java column needs `1L` here,
        // because there `1 << 40` is an int shift and evaluates to 256.
        assert!(generated.contains("B = (1 << 40)"), "{generated}");
    }

    #[test]
    fn a_bitset_states_the_declared_bit_width_and_rounds_nothing_up() {
        let generated = declarations(
            "bitset Packed { bitfield<3> low; bitfield<5> high; };
             @final struct P { Packed value; };",
        );
        assert!(
            generated.contains("low: typing.Annotated[int, _IdlInt(bits=3, signed=False)]"),
            "{generated}"
        );
        assert!(
            generated.contains("high: typing.Annotated[int, _IdlInt(bits=5, signed=False)]"),
            "{generated}"
        );
        assert!(generated.contains("value: Packed"), "{generated}");
    }

    #[test]
    fn a_fixed_is_a_str_and_never_a_decimal() {
        // `Decimal("0.00").as_tuple().digits` is `(0,)` — one digit —
        // so a Decimal would answer the FixedDigitCount axis itself.
        let generated = declarations(
            "typedef fixed<9,3> Money;
             @final struct P { Money amount; };",
        );
        assert!(
            generated.contains("amount: typing.Annotated[str, _IdlFixed(digits=9, scale=3)]"),
            "{generated}"
        );
        // The preamble says in prose why a Decimal is not used; what
        // must not appear is an import of it or a member typed as one.
        assert!(!generated.contains("import decimal"), "{generated}");
        assert!(!generated.contains("amount: decimal"), "{generated}");
    }

    #[test]
    fn a_union_is_a_dataclass_with_a_discriminator_and_one_optional_field_per_case() {
        let generated = declarations(
            "union Choice switch (long) {
               case 1: long whole;
               case 2: long other;
               default: double fraction;
             };
             @final struct P { Choice c; };",
        );
        assert!(generated.contains("_d: _Int32"), "{generated}");
        assert!(
            generated.contains("whole: typing.Optional[_Int32] = None"),
            "{generated}"
        );
        assert!(
            generated.contains("other: typing.Optional[_Int32] = None"),
            "{generated}"
        );
        assert!(
            generated.contains("fraction: typing.Optional[_Float64] = None"),
            "{generated}"
        );
        assert!(generated.contains("c: Choice"), "{generated}");
    }

    #[test]
    fn a_typedef_declares_nothing_and_says_why() {
        // An eagerly evaluated Python alias could not be ordered
        // against the classes it names.
        let generated = declarations(
            "typedef sequence<long> Row;
             @final struct P { Row r; };",
        );
        assert!(generated.contains("r: list[_Int32]"), "{generated}");
        assert!(
            generated.contains("typedef list[_Int32] Row;"),
            "{generated}"
        );
    }

    #[test]
    fn a_module_is_a_nested_class_and_a_reference_is_written_from_the_root() {
        let generated = declarations(
            "module a { @final struct P { long x; }; };
             module b { @final struct P { double y; }; };
             @final struct Q { a::P one; b::P two; };",
        );
        assert!(generated.contains("class a:"), "{generated}");
        assert!(generated.contains("class b:"), "{generated}");
        assert!(generated.contains("one: a.P"), "{generated}");
        assert!(generated.contains("two: b.P"), "{generated}");
    }

    #[test]
    fn a_module_reopened_beside_a_nested_one_is_one_class() {
        // `module common { struct Robot; module util { … } }` — the
        // arrangement that produced two classes of one name when the
        // grouping was by module id rather than by path. Python binds a
        // name rather than declaring one, so the second would have
        // silently replaced the first.
        let generated = declarations(
            "module common {
               module util { @final struct Location { double x; }; };
               @final struct Robot { ::common::util::Location where_; };
             };",
        );
        assert_eq!(generated.matches("class common:").count(), 1, "{generated}");
        assert!(
            generated.contains("where_: common.util.Location"),
            "{generated}"
        );
    }

    #[test]
    fn an_identifier_that_is_a_python_keyword_is_escaped_and_the_idl_name_is_kept() {
        let generated = declarations(
            "module class {
               enum type { match, lambda, virtual };
               @final struct P { long global; };
             };",
        );
        assert!(generated.contains("class class_:"), "{generated}");
        assert!(
            generated.contains("class type_(enum.IntEnum):"),
            "{generated}"
        );
        assert!(generated.contains("match_ = 0"), "{generated}");
        assert!(generated.contains("lambda_ = 1"), "{generated}");
        // Not a Python keyword, so it keeps its spelling.
        assert!(generated.contains("virtual = 2"), "{generated}");
        // The IDL name survives the escape, machine-readably for a
        // field and in a comment for everything else.
        assert!(
            generated
                .contains("global_: _Int32 = dataclasses.field(metadata={\"idl\": \"global\"})"),
            "{generated}"
        );
        assert!(generated.contains("IDL name: match"), "{generated}");
    }

    #[test]
    fn a_member_named_after_a_builtin_the_annotations_use_is_escaped_too() {
        // The case the parser does not catch. `int: typing.Optional[…] =
        // None` sets a class attribute called `int`, and
        // `typing.get_type_hints` resolves the class namespace before
        // the module's — so every `int` in that class would resolve to
        // `None`.
        let generated = declarations("@mutable struct P { @optional long int; long list; };");
        assert!(generated.contains("int_:"), "{generated}");
        assert!(generated.contains("list_:"), "{generated}");
    }

    #[test]
    fn two_names_that_would_escape_onto_one_are_refused() {
        // Python binds a name; it does not declare one. The second
        // `class_` would silently replace the first.
        let failure = render("@final struct P { long class; long class_; };", "test.idl")
            .expect_err("two members escaping onto one name are refused");
        assert!(failure.contains("class_"), "{failure}");
    }

    #[test]
    fn a_construct_with_no_python_data_form_is_named_and_not_left_out() {
        // `object` would type-check for all of these, which is exactly
        // why it is not written: a member typed `object` is a field no
        // codec can write, from a generator that reported success.
        for (idl, construct) in [
            ("@final struct P { any a; long tail; };", "any"),
            ("@final struct P { Object o; long tail; };", "Object"),
        ] {
            let failure = render(idl, "test.idl").expect_err(construct);
            assert!(failure.contains(construct), "{failure}");
            assert!(failure.contains("Python"), "{failure}");
        }
    }

    #[test]
    fn a_constant_outside_its_idl_range_is_refused_although_python_would_hold_it() {
        // A Python `int` holds 300 perfectly well and the wire holds one
        // byte. Nothing in the language would notice.
        let failure = render("const octet X = 300;", "test.idl")
            .expect_err("an octet constant of 300 is refused");
        assert!(failure.contains("300"), "{failure}");
        assert!(failure.contains("octet"), "{failure}");
    }

    #[test]
    fn a_constant_is_final_and_carries_its_idl_type() {
        let generated = declarations("const long MAX = 5; const string NAME = \"z\";");
        assert!(
            generated.contains("MAX: typing.Final[_Int32] = 5"),
            "{generated}"
        );
        assert!(
            generated.contains("NAME: typing.Final[typing.Annotated[str,"),
            "{generated}"
        );
    }

    #[test]
    fn an_unevaluated_array_length_is_named_and_not_substituted() {
        // The C and Java columns fail on the same input; the columns'
        // failure sets stay comparable.
        let failure = render("@final struct P { long a[UNKNOWN]; };", "test.idl")
            .expect_err("an array whose length does not evaluate is refused");
        assert!(!failure.is_empty(), "{failure}");
    }

    #[test]
    fn an_interface_declares_nothing_and_the_rest_of_the_file_still_renders() {
        // The one rule in the catalogue whose node nothing reads. An
        // interface has no data form in a types-only column, so the
        // check is that it is passed over rather than that it breaks
        // the file — coverage.rs is where the cell says so.
        let generated = declarations(
            "interface Sensor { long read(); };
             @final struct P { long x; };",
        );
        assert!(generated.contains("class P:"), "{generated}");
        assert!(!generated.contains("Sensor"), "{generated}");
    }

    #[test]
    fn an_empty_aggregate_still_has_a_body() {
        // A class header with no block under it is a SyntaxError, not
        // untidy output.
        let generated = declarations("@final struct P { };");
        assert!(generated.contains("class P:"), "{generated}");
        assert!(generated.contains("pass"), "{generated}");
    }
}
