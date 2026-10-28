// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A dispatch chain longer than any stack the renderer could recurse on.
//!
//! The codec walk builds a union's cases, an enumeration's enumerators
//! and a `@mutable` aggregate's member dispatch as one `If` per arm, each
//! in the previous one's `else` (`lang-rules/src/emit/encode.rs`). The
//! model bounds none of the three counts, so a renderer that took the
//! next arm by calling itself made its own stack the limit on how many
//! cases a union may have — the same shape as the CORBA C++ emitter that
//! followed `A → sequence<B> → B → sequence<A>` until the stack ran out.
//!
//! The ten renderers that write the chain flat — C, D, F#, Go, Kotlin,
//! Lua, Node, Python, Swift and Zig — take the next arm in a loop. Each
//! test below renders [`ARMS`] of them inside a thread with [`STACK`] of
//! stack: enough for a walk that recurses only into a *nested block*,
//! nowhere near enough for one frame per arm.
//!
//! The tree is built and dropped on a thread of its own with [`ROOM`] to
//! spare. Dropping a chain this deep is itself recursive — that is
//! `Vec`'s drop glue and not this crate's walk — and running it on the
//! small stack would measure the wrong thing.
//!
//! The ten renderers that nest an `else if` in their own output — Ada,
//! Ada 83, C++, C#, Elixir, Java, Julia, Nim, OCaml and TypeScript — have
//! no such test: for them the arm chain is still one call per arm, and the
//! marker on each of those functions says so rather than claiming a depth.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_lang_cst::common::{Doc, Ident, Literal, Path};

/// How many arms the chain carries.
///
/// Chosen against the stack below, not against any corpus: the largest
/// recorded case declares twenty members. A recursive walk needs one
/// frame per arm, and no frame this small crate builds is under sixteen
/// bytes, so ten thousand arms cannot fit in a hundred and twenty-eight
/// kilobytes however tight the frame.
const ARMS: usize = 10_000;

/// The stack the rendering runs on.
const STACK: usize = 128 * 1024;

/// The stack the tree is built and dropped on. `Vec`'s drop glue
/// recurses once per level, which is not what is being measured.
const ROOM: usize = 64 * 1024 * 1024;

/// Build the tree on a thread with [`ROOM`], render it on a nested
/// thread with only [`STACK`]: a walk that recursed once per arm would
/// end the process there rather than pass quietly.
fn rendered<Tree, Out>(
    build: impl FnOnce() -> Tree + Send,
    render: impl FnOnce(&Tree) -> Out + Send,
) -> Out
where
    Tree: Send + Sync,
    Out: Send,
{
    std::thread::scope(|outer| {
        std::thread::Builder::new()
            .stack_size(ROOM)
            .spawn_scoped(outer, move || {
                let tree = build();
                std::thread::scope(|inner| {
                    std::thread::Builder::new()
                        .stack_size(STACK)
                        .spawn_scoped(inner, || render(&tree))
                        .expect("the rendering thread starts")
                        .join()
                        .expect("the rendering thread finishes")
                })
            })
            .expect("the building thread starts")
            .join()
            .expect("the building thread finishes")
    })
}

/// The chain is flat: one `if` head, `ARMS - 1` continuations, and no
/// line indented deeper than the body of a single arm.
fn assert_flat(text: &str, continuation: &str, deepest: usize) {
    assert_eq!(
        text.matches(continuation).count(),
        ARMS - 1,
        "every arm after the first continues the chain"
    );
    let widest = text
        .lines()
        .map(|line| line.len() - line.trim_start().len())
        .max()
        .expect("the output has lines");
    assert!(
        widest <= deepest,
        "no line is indented past the body of one arm: {widest} > {deepest}"
    );
}

// -- C -------------------------------------------------------------------

#[test]
fn a_c_dispatch_chain_of_ten_thousand_arms_renders_flat() {
    use zerodds_lang_cst::c::{Block, Decl, Expr, Function, Statement, Type};

    let text = rendered(
        || {
            let arm = |value: i128| Expr::Binary {
                op: "==",
                lhs: Box::new(Expr::Name(Ident::new("id"))),
                rhs: Box::new(Expr::Literal(Literal::int(value))),
            };
            let mut otherwise: Option<Block> = None;
            for value in (0..ARMS).rev() {
                otherwise = Some(Block {
                    statements: vec![Statement::If {
                        condition: arm(value as i128),
                        then: Block {
                            statements: vec![Statement::Break],
                        },
                        otherwise,
                    }],
                });
            }
            Decl::Function(Function {
                doc: Doc::none(),
                is_static: true,
                is_inline: false,
                is_maybe_unused: false,
                returns: Type::Void,
                name: Ident::new("dispatch"),
                params: Vec::new(),
                body: otherwise,
            })
        },
        zerodds_lang_render::c::decl,
    );
    assert_flat(&text, "} else if (", 8);
}

// -- D -------------------------------------------------------------------

#[test]
fn a_d_dispatch_chain_of_ten_thousand_arms_renders_flat() {
    use zerodds_lang_cst::d::{Decl, Expr, FuncDecl, Stmt};

    let text = rendered(
        || {
            let mut otherwise: Option<Vec<Stmt>> = None;
            for _ in 0..ARMS {
                otherwise = Some(vec![Stmt::If {
                    condition: Expr::Name(Ident::new("ok")),
                    then: vec![Stmt::Break],
                    otherwise,
                }]);
            }
            Decl::Func(FuncDecl {
                doc: Doc::none(),
                name: Ident::new("dispatch"),
                params: Vec::new(),
                returns: None,
                body: otherwise.expect("the chain has arms"),
            })
        },
        zerodds_lang_render::d::decl,
    );
    assert_flat(&text, "else if (", 8);
}

// -- F# ------------------------------------------------------------------

#[test]
fn an_fsharp_dispatch_chain_of_ten_thousand_arms_renders_flat() {
    use zerodds_lang_cst::fsharp::{Decl, Expr, LetDecl, Stmt};

    let text = rendered(
        || {
            let mut otherwise: Option<Vec<Stmt>> = None;
            for _ in 0..ARMS {
                otherwise = Some(vec![Stmt::If {
                    condition: Expr::Name(Ident::new("ok")),
                    then: vec![Stmt::Leave],
                    otherwise,
                }]);
            }
            Decl::Let(LetDecl {
                doc: Doc::none(),
                name: Ident::new("dispatch"),
                params: Vec::new(),
                type_: None,
                body: otherwise.expect("the chain has arms"),
            })
        },
        zerodds_lang_render::fsharp::decl,
    );
    assert_flat(&text, "elif ", 8);
}

// -- Go ------------------------------------------------------------------

#[test]
fn a_go_dispatch_chain_of_ten_thousand_arms_renders_flat() {
    use zerodds_lang_cst::go::{Decl, Expr, FuncDecl, Stmt};

    let text = rendered(
        || {
            let mut otherwise: Option<Vec<Stmt>> = None;
            for _ in 0..ARMS {
                otherwise = Some(vec![Stmt::If {
                    condition: Expr::name("ok"),
                    then: vec![Stmt::Break],
                    otherwise,
                }]);
            }
            Decl::Func(FuncDecl {
                doc: Doc::none(),
                name: Ident::new("dispatch"),
                params: Vec::new(),
                results: Vec::new(),
                body: otherwise.expect("the chain has arms"),
            })
        },
        zerodds_lang_render::go::decl,
    );
    assert_flat(&text, "} else if ", 8);
}

// -- Kotlin --------------------------------------------------------------

#[test]
fn a_kotlin_dispatch_chain_of_ten_thousand_arms_renders_flat() {
    use zerodds_lang_cst::kotlin::{Block, Decl, Expr, FuncDecl, Stmt, Type};

    let text = rendered(
        || {
            let mut otherwise: Option<Block> = None;
            for _ in 0..ARMS {
                otherwise = Some(Block {
                    statements: vec![Stmt::If {
                        condition: Expr::name("ok"),
                        then: Block {
                            statements: vec![Stmt::Break],
                        },
                        otherwise,
                    }],
                });
            }
            Decl::Func(FuncDecl {
                doc: Doc::none(),
                name: Ident::new("dispatch"),
                params: Vec::new(),
                returns: Type::Named {
                    path: Path::new(&["Unit"]),
                    args: Vec::new(),
                },
                body: otherwise.expect("the chain has arms").statements,
            })
        },
        zerodds_lang_render::kotlin::decl,
    );
    assert_flat(&text, "} else if (", 8);
}

// -- Lua -----------------------------------------------------------------

#[test]
fn a_lua_dispatch_chain_of_ten_thousand_arms_renders_flat() {
    use zerodds_lang_cst::lua::{Expr, Stat};

    let text = rendered(
        || {
            let mut otherwise: Option<Vec<Stat>> = None;
            for _ in 0..ARMS {
                otherwise = Some(vec![Stat::If {
                    condition: Expr::bool(true),
                    then: vec![Stat::Break],
                    otherwise,
                }]);
            }
            let mut chain = otherwise.expect("the chain has arms");
            chain.remove(0)
        },
        zerodds_lang_render::lua::stat,
    );
    assert_flat(&text, "elseif ", 4);
}

// -- Node ----------------------------------------------------------------

#[test]
fn a_node_dispatch_chain_of_ten_thousand_arms_renders_flat() {
    use zerodds_lang_cst::node::{Expr, Stat};

    let text = rendered(
        || {
            let mut otherwise: Option<Vec<Stat>> = None;
            for _ in 0..ARMS {
                otherwise = Some(vec![Stat::If {
                    condition: Expr::name("ok"),
                    then: vec![Stat::Break],
                    otherwise,
                }]);
            }
            let mut chain = otherwise.expect("the chain has arms");
            chain.remove(0)
        },
        zerodds_lang_render::node::stat,
    );
    assert_flat(&text, "} else if (", 4);
}

// -- Python --------------------------------------------------------------

#[test]
fn a_python_dispatch_chain_of_ten_thousand_arms_renders_flat() {
    use zerodds_lang_cst::python::{Expr, Statement};

    let text = rendered(
        || {
            let mut otherwise: Option<Vec<Statement>> = None;
            for _ in 0..ARMS {
                otherwise = Some(vec![Statement::If {
                    condition: Expr::name("ok"),
                    then: vec![Statement::Break],
                    otherwise,
                }]);
            }
            let mut chain = otherwise.expect("the chain has arms");
            chain.remove(0)
        },
        zerodds_lang_render::python::statement,
    );
    assert_flat(&text, "elif ", 4);
}

// -- Swift ---------------------------------------------------------------

#[test]
fn a_swift_dispatch_chain_of_ten_thousand_arms_renders_flat() {
    use zerodds_lang_cst::swift::{Decl, Expr, FuncDecl, Stmt};

    let text = rendered(
        || {
            let mut otherwise: Option<Vec<Stmt>> = None;
            for _ in 0..ARMS {
                otherwise = Some(vec![Stmt::If {
                    condition: Expr::Name(Ident::new("ok")),
                    then: vec![Stmt::Break],
                    otherwise,
                }]);
            }
            Decl::Func(FuncDecl {
                doc: Doc::none(),
                visibility: zerodds_lang_cst::common::Visibility::Internal,
                name: Ident::new("dispatch"),
                params: Vec::new(),
                throws: false,
                returns: None,
                body: otherwise.expect("the chain has arms"),
            })
        },
        zerodds_lang_render::swift::decl,
    );
    assert_flat(&text, "} else if ", 8);
}

// -- Zig -----------------------------------------------------------------

#[test]
fn a_zig_dispatch_chain_of_ten_thousand_arms_renders_flat() {
    use zerodds_lang_cst::zig::{Decl, Expr, FuncDecl, Stmt, Type};

    let text = rendered(
        || {
            let mut otherwise: Option<Vec<Stmt>> = None;
            for _ in 0..ARMS {
                otherwise = Some(vec![Stmt::If {
                    condition: Expr::Name(Ident::new("ok")),
                    then: vec![Stmt::Break],
                    otherwise,
                }]);
            }
            Decl::Func(FuncDecl {
                doc: Doc::none(),
                is_pub: false,
                name: Ident::new("dispatch"),
                params: Vec::new(),
                returns: Type::Named(Ident::new("void")),
                body: otherwise.expect("the chain has arms"),
            })
        },
        zerodds_lang_render::zig::decl,
    );
    assert_flat(&text, "} else if (", 8);
}
