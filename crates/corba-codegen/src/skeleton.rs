// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Server skeletons — the operation-name dispatch.
//!
//! A skeleton receives a GIOP request, reads the operation name out of
//! it and calls the servant method that implements it. The dispatch is
//! the part that is the same in every skeleton and different in every
//! language, and it is what this module builds.
//!
//! ## The three shapes, and why they are not one
//!
//! | column | shape | spec |
//! |---|---|---|
//! | C++ | `strcmp` chain over `const char *` | CORBA 3.3 part 1 §8.3, `ServerRequest` |
//! | C# | `switch` over a `string` | IDL to C# `formal/2003-09-04` §1.6 |
//! | Java | `switch` over a `String` | IDL to Java §1.5, `_invoke` |
//! | Rust | `match` over a `&str` | this project's |
//!
//! C++ has no `switch` over a string, so the chain is not a stylistic
//! choice. Java's `switch` over strings is JLS 7 §14.11 and is what
//! every generated `_invoke` uses. C# has had one since C# 1.0 — the
//! only language of the four where a string `switch` was there from the
//! start. Rust's `match` on a `&str` is the same construct as Java's.
//!
//! ## What the default arm does
//!
//! C++ and Java throw `BAD_OPERATION` — the system exception §8.3 names
//! for an operation the servant does not have, and both languages have
//! the class. Rust has neither a CORBA exception type in this tree nor a
//! throw, so the Rust dispatch returns `Option` and the absent case *is*
//! the answer: the ORB that called it maps `None` onto the reply. That
//! is a smaller claim than inventing an error type whose name would
//! resolve to nothing.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use zerodds_ast::{Ast, InterfaceNode};
use zerodds_ast_core::Id;
use zerodds_lang_cst::{Doc, Ident, Literal, Path, Visibility, cpp, csharp, java, rust};

use crate::special_types::TargetLanguage;
use crate::stub::NoRenderer;

/// One operation a skeleton dispatches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkeletonOp {
    /// The operation name on the wire, which is what the GIOP request
    /// carries and what the dispatch compares against. IDL 4.2 §7.4.3.
    pub name: String,
    /// The servant method to call. Not always the same string: a name
    /// that is a keyword in the target language is escaped, and an
    /// attribute becomes `_get_x` and `_set_x` on the wire while the
    /// servant has `x`.
    pub method_name: String,
}

impl SkeletonOp {
    /// An operation whose wire name and method name are the same.
    #[must_use]
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            method_name: name.to_string(),
        }
    }
}

/// The operations an interface's skeleton dispatches, in declaration
/// order.
///
/// Attributes are included in the form §7.4.4 puts on the wire:
/// `readonly attribute long x` is the operation `_get_x`, and a
/// read-write one is `_get_x` and `_set_x`. A dispatch built from the
/// operation list alone would answer `BAD_OPERATION` for every attribute
/// read, which is a servant that compiles and does not work.
///
/// Base interfaces are **not** walked: a skeleton dispatches its own
/// operations and delegates the rest, and which of the two a generator
/// wants depends on whether it emits one dispatch per interface or one
/// per most-derived class. [`Ast::interface`] gives the bases to a caller
/// that wants them flattened.
#[must_use]
pub fn skeleton_ops_of(ast: &Ast, id: Id<InterfaceNode>) -> Vec<SkeletonOp> {
    let Ok(node) = ast.interface(id) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for operation in &node.operations {
        if let Ok(operation) = ast.operation(*operation) {
            out.push(SkeletonOp::new(&operation.name));
        }
    }
    for attribute in &node.attributes {
        let Ok(attribute) = ast.attribute(*attribute) else {
            continue;
        };
        out.push(SkeletonOp {
            name: format!("_get_{}", attribute.name),
            method_name: attribute.name.clone(),
        });
        if !attribute.readonly {
            out.push(SkeletonOp {
                name: format!("_set_{}", attribute.name),
                method_name: attribute.name.clone(),
            });
        }
    }
    out
}

/// Render a skeleton's dispatch as text.
///
/// # Errors
/// [`NoRenderer`] for a column with an OMG mapping and no CST, of which
/// there is none — see [`crate::stub::render_stub_op`].
pub fn render_skeleton_dispatch(
    class_name: &str,
    ops: &[SkeletonOp],
    language: TargetLanguage,
) -> Result<String, NoRenderer> {
    match language {
        TargetLanguage::Cpp => Ok(zerodds_lang_render::cpp::decl(&cpp::Decl::Function(
            cpp_dispatch(class_name, ops),
        ))),
        TargetLanguage::Java => Ok(zerodds_lang_render::java::type_decl(
            &java::TypeDecl::Class(java::Class {
                doc: Doc::none(),
                annotations: Vec::new(),
                modifiers: alloc::vec![java::Modifier::Public, java::Modifier::Abstract],
                name: Ident::new(class_name),
                generics: Vec::new(),
                extends: None,
                implements: Vec::new(),
                members: alloc::vec![java::Member::Method(java_dispatch(ops))],
            }),
        )),
        TargetLanguage::Rust => Ok(zerodds_lang_render::rust::item(&rust::Item::Function(
            rust_dispatch(ops),
        ))),
        TargetLanguage::CSharp => Ok(zerodds_lang_render::csharp::type_decl(
            &csharp::TypeDecl::Class(csharp::Class {
                doc: Doc::none(),
                attributes: Vec::new(),
                modifiers: alloc::vec![csharp::Modifier::Public, csharp::Modifier::Abstract],
                name: Ident::new(class_name),
                generics: Vec::new(),
                bases: Vec::new(),
                constraints: Vec::new(),
                members: alloc::vec![csharp::Member::Method(csharp_dispatch(ops))],
            }),
        )),
    }
}

/// The C++ dispatch: a `strcmp` chain over the request's operation name,
/// ending in `throw CORBA::BAD_OPERATION()`.
///
/// The `ServerRequest` form of §8.3 and not a vendor's: omniORB's
/// `omniCallHandle` and TAO's `TAO_ServerRequest` are internal types
/// with no portable spelling, and a generator emitting one would be
/// emitting for one ORB.
#[must_use]
pub fn cpp_dispatch(class_name: &str, ops: &[SkeletonOp]) -> cpp::Function {
    let request = || cpp::Expr::Path(Path::single("_request"));
    let operation = cpp::Expr::Path(Path::single("_operation"));

    let mut statements = alloc::vec![cpp::Statement::Declare {
        type_: cpp::Type::Pointer(alloc::boxed::Box::new(cpp::Type::Const(
            alloc::boxed::Box::new(cpp::Type::Primitive(cpp::Primitive::Char)),
        ))),
        name: Ident::new("_operation"),
        // `->` and not `.`: a `CORBA::ServerRequest_ptr` is a pointer,
        // and `.` on it does not compile.
        value: Some(cpp::Expr::Call {
            callee: alloc::boxed::Box::new(cpp::Expr::Arrow {
                value: alloc::boxed::Box::new(request()),
                name: Ident::new("operation"),
            }),
            args: Vec::new(),
        }),
    }];

    // The chain is built from the last operation outwards, so the
    // innermost `else` is the throw and every operation sits in one
    // `else if`.
    let mut otherwise = cpp::Block {
        statements: alloc::vec![cpp::Statement::Throw(cpp::Expr::Construct {
            type_: alloc::boxed::Box::new(cpp::Type::named(Path::new(&["CORBA", "BAD_OPERATION"]))),
            args: Vec::new(),
            braced: false,
        })],
    };
    for op in ops.iter().rev() {
        let compare = cpp::Expr::Binary {
            op: "==",
            lhs: alloc::boxed::Box::new(cpp::Expr::Call {
                callee: alloc::boxed::Box::new(cpp::Expr::Path(Path::single("strcmp"))),
                args: alloc::vec![
                    operation.clone(),
                    cpp::Expr::Literal(Literal::string(&op.name)),
                ],
            }),
            rhs: alloc::boxed::Box::new(cpp::Expr::Literal(Literal::int(0))),
        };
        otherwise = cpp::Block {
            statements: alloc::vec![cpp::Statement::If {
                condition: compare,
                then: cpp::Block {
                    statements: alloc::vec![cpp::Statement::Expr(cpp::Expr::Call {
                        callee: alloc::boxed::Box::new(cpp::Expr::Arrow {
                            value: alloc::boxed::Box::new(cpp::Expr::Path(Path::single("this"))),
                            name: Ident::new(&op.method_name),
                        }),
                        args: alloc::vec![request()],
                    })],
                },
                otherwise: Some(otherwise),
            }],
        };
    }
    statements.extend(otherwise.statements);

    cpp::Function {
        doc: Doc::line(&format!(
            "Dispatch a request to {class_name}, by operation name."
        )),
        returns: cpp::Type::Void,
        name: Ident::new("_dispatch"),
        params: alloc::vec![cpp::Param {
            type_: cpp::Type::named(Path::new(&["CORBA", "ServerRequest_ptr"])),
            name: Some(Ident::new("_request")),
            default: None,
        }],
        is_inline: false,
        is_static: false,
        is_constexpr: false,
        is_noexcept: false,
        is_maybe_unused: false,
        body: cpp::FunctionBody::Block(cpp::Block { statements }),
    }
}

/// The Java dispatch: the portable `_invoke` of IDL to Java §1.5.
#[must_use]
pub fn java_dispatch(ops: &[SkeletonOp]) -> java::Method {
    let mut cases: Vec<java::SwitchCase> = ops
        .iter()
        .map(|op| java::SwitchCase {
            labels: alloc::vec![java::Expr::Literal(Literal::string(&op.name))],
            body: java::Block {
                statements: alloc::vec![java::Statement::Return(Some(java::Expr::MethodCall {
                    receiver: Some(alloc::boxed::Box::new(java::Expr::This)),
                    name: Ident::new(&java::escape_keyword(&op.method_name)),
                    args: alloc::vec![
                        java::Expr::Path(Path::single("_input")),
                        java::Expr::Path(Path::single("_handler")),
                    ],
                }))],
            },
            // A `return` ends the case; a `break` after it does not
            // compile.
            breaks: false,
        })
        .collect();
    cases.push(java::SwitchCase {
        labels: Vec::new(),
        body: java::Block {
            statements: alloc::vec![java::Statement::Throw(java::Expr::New {
                type_: alloc::boxed::Box::new(java::Type::named(Path::new(&[
                    "org",
                    "omg",
                    "CORBA",
                    "BAD_OPERATION"
                ]))),
                args: Vec::new(),
            })],
        },
        breaks: false,
    });

    java::Method {
        doc: Doc::line("Dispatch a request by operation name. IDL to Java §1.5."),
        annotations: Vec::new(),
        modifiers: alloc::vec![java::Modifier::Public],
        generics: Vec::new(),
        returns: java::Type::named(Path::new(&[
            "org",
            "omg",
            "CORBA",
            "portable",
            "OutputStream",
        ])),
        name: Ident::new("_invoke"),
        params: alloc::vec![
            java_param(
                "_method",
                java::Type::named(Path::new(&["java", "lang", "String"]))
            ),
            java_param(
                "_input",
                java::Type::named(Path::new(&[
                    "org",
                    "omg",
                    "CORBA",
                    "portable",
                    "InputStream"
                ])),
            ),
            java_param(
                "_handler",
                java::Type::named(Path::new(&[
                    "org",
                    "omg",
                    "CORBA",
                    "portable",
                    "ResponseHandler"
                ])),
            ),
        ],
        throws: alloc::vec![java::Type::named(Path::new(&[
            "org",
            "omg",
            "CORBA",
            "SystemException"
        ]))],
        body: Some(java::Block {
            statements: alloc::vec![java::Statement::Switch {
                value: java::Expr::Path(Path::single("_method")),
                cases,
            }],
        }),
    }
}

fn java_param(name: &str, type_: java::Type) -> java::Param {
    java::Param {
        annotations: Vec::new(),
        final_: false,
        type_,
        name: Ident::new(name),
        varargs: false,
    }
}

/// The C# dispatch: a `switch` over the request's operation name,
/// ending in `throw new omg.org.CORBA.BAD_OPERATION()`.
///
/// The DSI shape of the C++ column and not the portable `_invoke` of the
/// Java one, and the reason is in the mapping: IDL to C#
/// `formal/2003-09-04` puts the static server side on .NET Remoting, so
/// there is no portable stream-and-response-handler pair for a generated
/// skeleton to take. `omg.org.CORBA.ServerRequest` is CORBA 3.3 part 1
/// §8.3's pseudo-object under the namespace the mapping assigns the
/// `CORBA` module, and it is the one server-side surface both documents
/// name.
#[must_use]
pub fn csharp_dispatch(ops: &[SkeletonOp]) -> csharp::Method {
    let request = || csharp::Expr::Path(Path::single("_request"));

    let mut sections: Vec<csharp::SwitchSection> = ops
        .iter()
        .map(|op| csharp::SwitchSection {
            labels: alloc::vec![csharp::Expr::Literal(Literal::string(&op.name))],
            body: csharp::Block {
                statements: alloc::vec![csharp::Statement::Expr(csharp::Expr::Invoke {
                    receiver: Some(alloc::boxed::Box::new(csharp::Expr::This)),
                    name: Ident::new(&csharp::escape_keyword(&op.method_name)),
                    args: alloc::vec![request()],
                })],
            },
            // A C# switch section may not fall through, so each one
            // ends in `break`. Java's arms `return` and need none.
            breaks: true,
        })
        .collect();
    sections.push(csharp::SwitchSection {
        labels: Vec::new(),
        body: csharp::Block {
            statements: alloc::vec![csharp::Statement::Throw(csharp::Expr::New {
                type_: alloc::boxed::Box::new(csharp::Type::named(Path::new(&[
                    "omg",
                    "org",
                    "CORBA",
                    "BAD_OPERATION"
                ]))),
                args: Vec::new(),
                initialiser: Vec::new(),
            })],
        },
        // A `throw` ends the section; a `break` after it is unreachable.
        breaks: false,
    });

    csharp::Method {
        doc: Doc::line("Dispatch a request by operation name. IDL to C# §1.6."),
        attributes: Vec::new(),
        modifiers: alloc::vec![csharp::Modifier::Public],
        returns: csharp::Type::Predefined(csharp::Predefined::Void),
        name: Ident::new("Dispatch"),
        generics: Vec::new(),
        params: alloc::vec![csharp::Param {
            attributes: Vec::new(),
            modifier: None,
            type_: csharp::Type::named(Path::new(&["omg", "org", "CORBA", "ServerRequest"])),
            name: Ident::new("_request"),
            default: None,
        }],
        constraints: Vec::new(),
        body: Some(csharp::Block {
            statements: alloc::vec![csharp::Statement::Switch {
                value: csharp::Expr::Member {
                    value: alloc::boxed::Box::new(request()),
                    name: Ident::new("Operation"),
                },
                sections,
            }],
        }),
        expression: None,
    }
}

/// The Rust dispatch: a `match` on the operation name whose absent case
/// is `None`.
#[must_use]
pub fn rust_dispatch(ops: &[SkeletonOp]) -> rust::Function {
    let mut arms: Vec<rust::MatchArm> = ops
        .iter()
        .map(|op| rust::MatchArm {
            pattern: rust::Pattern::Literal(Literal::string(&op.name)),
            guard: None,
            body: rust::Expr::Call {
                callee: alloc::boxed::Box::new(rust::Expr::Path(Path::single("Some"))),
                args: alloc::vec![rust::Expr::MethodCall {
                    receiver: alloc::boxed::Box::new(rust::Expr::Path(Path::single("self"))),
                    method: Ident::new(&rust::escape_keyword(&op.method_name)),
                    args: alloc::vec![rust::Expr::Path(Path::single("request"))],
                }],
            },
        })
        .collect();
    arms.push(rust::MatchArm {
        pattern: rust::Pattern::Wildcard,
        guard: None,
        body: rust::Expr::Path(Path::single("None")),
    });

    rust::Function {
        doc: Doc::lines(&[
            "Dispatch a request by operation name.",
            "`None` is an operation this servant does not have; the ORB turns it into `BAD_OPERATION`.",
        ]),
        attrs: Vec::new(),
        visibility: Visibility::Public,
        name: Ident::new("dispatch"),
        generics: Vec::new(),
        params: alloc::vec![
            rust::Param {
                name: None,
                type_: rust::Type::unit(),
                receiver: Some(rust::Receiver::Ref),
            },
            rust::Param {
                name: Some(Ident::new("operation")),
                type_: rust::Type::Ref {
                    target: alloc::boxed::Box::new(rust::Type::Primitive(rust::Primitive::Str)),
                    mutable: false,
                    lifetime: None,
                },
                receiver: None,
            },
            rust::Param {
                name: Some(Ident::new("request")),
                type_: rust::Type::named(Path::single("Request")),
                receiver: None,
            },
        ],
        returns: Some(rust::Type::generic(
            Path::single("Option"),
            alloc::vec![rust::Type::named(Path::single("Reply"))],
        )),
        body: Some(rust::Block {
            statements: alloc::vec![rust::Statement::Tail(rust::Expr::Match {
                scrutinee: alloc::boxed::Box::new(rust::Expr::Path(Path::single("operation"))),
                arms,
            })],
        }),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn ops() -> Vec<SkeletonOp> {
        alloc::vec![
            SkeletonOp::new("echo"),
            SkeletonOp {
                name: "ping".to_string(),
                method_name: "Ping".to_string(),
            },
        ]
    }

    #[test]
    fn cpp_compares_with_strcmp_and_ends_in_bad_operation() {
        let text =
            render_skeleton_dispatch("EchoImpl", &ops(), TargetLanguage::Cpp).expect("renderable");
        assert!(text.contains("strcmp(_operation, \"echo\")"), "{text}");
        assert!(text.contains("this->Ping(_request)"), "{text}");
        assert!(text.contains("throw CORBA::BAD_OPERATION();"), "{text}");
    }

    #[test]
    fn java_switches_on_the_method_string_and_throws_the_omg_class() {
        let text =
            render_skeleton_dispatch("EchoPOA", &ops(), TargetLanguage::Java).expect("renderable");
        assert!(text.contains("switch (_method)"), "{text}");
        assert!(text.contains("case \"echo\":"), "{text}");
        assert!(
            text.contains("throw new org.omg.CORBA.BAD_OPERATION();"),
            "{text}"
        );
        // The portable signature JacORB and every other Java ORB
        // generates.
        assert!(text.contains("_invoke("), "{text}");
    }

    #[test]
    fn rust_matches_and_answers_none_rather_than_an_invented_error() {
        let text = render_skeleton_dispatch("EchoServant", &ops(), TargetLanguage::Rust)
            .expect("renderable");
        assert!(text.contains("\"echo\" =>"), "{text}");
        assert!(text.contains("_ => None"), "{text}");
        assert!(text.contains("Option<Reply>"), "{text}");
    }

    #[test]
    fn every_operation_reaches_the_dispatch_in_every_column() {
        let many: Vec<SkeletonOp> = (0..8).map(|i| SkeletonOp::new(&format!("op{i}"))).collect();
        for language in TargetLanguage::RENDERABLE {
            let text = render_skeleton_dispatch("Many", &many, language).expect("renderable");
            for op in &many {
                assert!(text.contains(&op.name), "{language:?} dropped {}", op.name);
            }
        }
    }

    #[test]
    fn an_empty_interface_still_dispatches_to_the_default_arm() {
        for language in TargetLanguage::RENDERABLE {
            let text = render_skeleton_dispatch("Empty", &[], language).expect("renderable");
            assert!(!text.is_empty(), "{language:?}");
        }
    }

    #[test]
    fn csharp_switches_on_the_requests_operation_and_throws_bad_operation() {
        let text = render_skeleton_dispatch("EchoServant", &ops(), TargetLanguage::CSharp)
            .expect("csharp");
        assert!(text.contains("switch (_request.Operation)"), "{text}");
        assert!(text.contains("case \"echo\":"), "{text}");
        assert!(
            text.contains("throw new omg.org.CORBA.BAD_OPERATION()"),
            "{text}"
        );
        // Every arm ends in `break`: C# forbids fall-through.
        assert!(text.contains("break;"), "{text}");
    }

    #[test]
    fn no_column_is_refused_any_more() {
        for language in TargetLanguage::ALL {
            assert!(
                render_skeleton_dispatch("X", &ops(), language).is_ok(),
                "{language:?}"
            );
        }
        assert_eq!(TargetLanguage::RENDERABLE.len(), TargetLanguage::ALL.len());
    }
}
