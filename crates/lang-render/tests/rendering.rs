// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! What each renderer writes for the shapes a generator produces.
//!
//! Every test asserts the exact text. A test that only checked for a
//! substring would pass on output that no compiler accepts, which is
//! the failure mode a renderer has.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_lang_cst::common::{Doc, Ident, Literal, Path, Visibility};

// -- Rust ----------------------------------------------------------------

mod rust_tree {
    pub use zerodds_lang_cst::rust::*;
}

fn rust_field(name: &str, type_: rust_tree::Type) -> rust_tree::Field {
    rust_tree::Field {
        doc: Doc::none(),
        attrs: Vec::new(),
        visibility: Visibility::Public,
        name: Ident::new(name),
        type_,
    }
}

#[test]
fn a_rust_struct_renders_with_its_derives_on_one_line() {
    let item = rust_tree::Item::Struct(rust_tree::Struct {
        body_text: zerodds_lang_cst::common::BodyText::none(),
        doc: Doc::line("A pose."),
        attrs: Vec::new(),
        derives: vec![
            rust_tree::Derive::of("Debug"),
            rust_tree::Derive::of("Clone"),
        ],
        visibility: Visibility::Public,
        name: Ident::new("Pose"),
        generics: Vec::new(),
        body: rust_tree::StructBody::Named(vec![
            rust_field("x", rust_tree::Type::Primitive(rust_tree::Primitive::I32)),
            rust_field("y", rust_tree::Type::Primitive(rust_tree::Primitive::F64)),
        ]),
    });

    assert_eq!(
        zerodds_lang_render::rust::item(&item),
        "\
/// A pose.
#[derive(Debug, Clone)]
pub struct Pose {
    pub x: i32,
    pub y: f64,
}
"
    );
}

#[test]
fn a_rust_enum_renders_its_variants_and_discriminants() {
    let item = rust_tree::Item::Enum(rust_tree::Enum {
        doc: Doc::none(),
        attrs: vec![rust_tree::Attr {
            path: Path::single("repr"),
            args: vec![rust_tree::AttrArg::Word(Ident::new("u32"))],
        }],
        derives: vec![rust_tree::Derive::of("Copy")],
        visibility: Visibility::Public,
        name: Ident::new("Mode"),
        generics: Vec::new(),
        variants: vec![
            rust_tree::Variant {
                doc: Doc::none(),
                attrs: Vec::new(),
                name: Ident::new("Idle"),
                body: rust_tree::VariantBody::Unit,
                discriminant: Some(rust_tree::Expr::Literal(Literal::int(0))),
            },
            rust_tree::Variant {
                doc: Doc::none(),
                attrs: Vec::new(),
                name: Ident::new("Active"),
                body: rust_tree::VariantBody::Unit,
                discriminant: Some(rust_tree::Expr::Literal(Literal::int(4))),
            },
        ],
    });

    assert_eq!(
        zerodds_lang_render::rust::item(&item),
        "\
#[repr(u32)]
#[derive(Copy)]
pub enum Mode {
    Idle = 0,
    Active = 4,
}
"
    );
}

#[test]
fn a_rust_impl_renders_its_method_body() {
    let item = rust_tree::Item::Impl(rust_tree::Impl {
        doc: Doc::none(),
        attrs: Vec::new(),
        generics: Vec::new(),
        trait_: None,
        self_type: rust_tree::Type::named(Path::single("Pose")),
        items: vec![rust_tree::Item::Function(rust_tree::Function {
            doc: Doc::none(),
            attrs: Vec::new(),
            visibility: Visibility::Public,
            name: Ident::new("x"),
            generics: Vec::new(),
            params: vec![rust_tree::Param {
                name: None,
                type_: rust_tree::Type::unit(),
                receiver: Some(rust_tree::Receiver::Ref),
            }],
            returns: Some(rust_tree::Type::Primitive(rust_tree::Primitive::I32)),
            body: Some(rust_tree::Block {
                statements: vec![rust_tree::Statement::Tail(rust_tree::Expr::Field {
                    receiver: Box::new(rust_tree::Expr::Path(Path::single("self"))),
                    name: Ident::new("x"),
                })],
            }),
        })],
    });

    assert_eq!(
        zerodds_lang_render::rust::item(&item),
        "\
impl Pose {
    pub fn x(&self) -> i32 {
        self.x
    }
}
"
    );
}

#[test]
fn a_rust_generic_type_renders_its_arguments() {
    let type_ = rust_tree::Type::generic(
        Path::new(&["Vec"]),
        vec![rust_tree::Type::generic(
            Path::new(&["Option"]),
            vec![rust_tree::Type::Primitive(rust_tree::Primitive::U8)],
        )],
    );
    assert_eq!(zerodds_lang_render::rust::type_(&type_), "Vec<Option<u8>>");
}

#[test]
fn a_rust_one_element_tuple_keeps_its_comma() {
    // `(T)` is a parenthesised type and `(T,)` is a tuple; dropping the
    // comma changes what the code means.
    let type_ = rust_tree::Type::Tuple(vec![rust_tree::Type::Primitive(rust_tree::Primitive::U8)]);
    assert_eq!(zerodds_lang_render::rust::type_(&type_), "(u8,)");
    assert_eq!(
        zerodds_lang_render::rust::type_(&rust_tree::Type::unit()),
        "()"
    );
}

#[test]
fn a_rust_array_length_renders_as_the_expression_it_is() {
    let type_ = rust_tree::Type::Array {
        element: Box::new(rust_tree::Type::Primitive(rust_tree::Primitive::U8)),
        length: Box::new(rust_tree::Expr::Path(Path::single("MAX"))),
    };
    assert_eq!(zerodds_lang_render::rust::type_(&type_), "[u8; MAX]");
}

#[test]
fn a_rust_file_separates_its_sections() {
    let file = rust_tree::File {
        inner_attrs: Vec::new(),
        doc: Doc::none(),
        uses: vec![rust_tree::Use {
            path: Path::new(&["core", "fmt"]),
            names: vec![Ident::new("Debug"), Ident::new("Display")],
            visibility: Visibility::Private,
        }],
        items: vec![rust_tree::Item::TypeAlias(rust_tree::TypeAlias {
            doc: Doc::none(),
            attrs: Vec::new(),
            visibility: Visibility::Public,
            name: Ident::new("Id"),
            generics: Vec::new(),
            type_: rust_tree::Type::Primitive(rust_tree::Primitive::U32),
        })],
    };
    assert_eq!(
        zerodds_lang_render::rust::file(&file),
        "\
use core::fmt::{Debug, Display};

pub type Id = u32;
"
    );
}

// -- C++ -----------------------------------------------------------------

mod cpp_tree {
    pub use zerodds_lang_cst::cpp::*;
}

#[test]
fn a_cpp_class_renders_its_access_sections_in_order() {
    let decl = cpp_tree::Decl::Class(cpp_tree::Class {
        doc: Doc::none(),
        is_struct: false,
        name: Ident::new("Point"),
        bases: Vec::new(),
        sections: vec![
            cpp_tree::Section {
                access: cpp_tree::Access::Public,
                members: vec![cpp_tree::ClassMember::Constructor(cpp_tree::Constructor {
                    doc: Doc::none(),
                    name: Ident::new("Point"),
                    params: Vec::new(),
                    initialisers: Vec::new(),
                    is_explicit: false,
                    is_constexpr: false,
                    is_noexcept: false,
                    body: cpp_tree::FunctionBody::Defaulted,
                })],
            },
            cpp_tree::Section {
                access: cpp_tree::Access::Private,
                members: vec![cpp_tree::ClassMember::Field(cpp_tree::Field {
                    doc: Doc::none(),
                    type_: cpp_tree::Type::Primitive(cpp_tree::Primitive::Int32),
                    name: Ident::new("x_"),
                    dims: Vec::new(),
                    default: None,
                    is_static: false,
                    is_constexpr: false,
                    bits: None,
                })],
            },
        ],
    });

    assert_eq!(
        zerodds_lang_render::cpp::decl(&decl),
        "\
class Point {
public:
    Point() = default;

private:
    int32_t x_;

};
"
    );
}

#[test]
fn a_cpp_accessor_with_one_statement_stays_on_one_line() {
    // Which is what the fixture output has for every accessor.
    let method = cpp_tree::ClassMember::Method(cpp_tree::Method {
        doc: Doc::none(),
        returns: cpp_tree::Type::Ref(Box::new(cpp_tree::Type::Primitive(
            cpp_tree::Primitive::Int32,
        ))),
        name: Ident::new("x"),
        params: Vec::new(),
        is_const: false,
        is_static: false,
        is_virtual: false,
        is_pure: false,
        is_constexpr: false,
        is_noexcept: false,
        is_override: false,
        body: Some(cpp_tree::Block {
            statements: vec![cpp_tree::Statement::Return(Some(cpp_tree::Expr::Path(
                Path::single("x_"),
            )))],
        }),
    });
    let decl = cpp_tree::Decl::Class(cpp_tree::Class {
        doc: Doc::none(),
        is_struct: false,
        name: Ident::new("Point"),
        bases: Vec::new(),
        sections: vec![cpp_tree::Section {
            access: cpp_tree::Access::Public,
            members: vec![method],
        }],
    });

    assert_eq!(
        zerodds_lang_render::cpp::decl(&decl),
        "\
class Point {
public:
    int32_t& x() { return x_; }

};
"
    );
}

#[test]
fn a_cpp_constructor_with_initialisers_puts_them_on_their_own_line() {
    let constructor = cpp_tree::Constructor {
        doc: Doc::none(),
        name: Ident::new("Point"),
        params: vec![cpp_tree::Param {
            type_: cpp_tree::Type::Primitive(cpp_tree::Primitive::Int32),
            name: Some(Ident::new("x")),
            default: None,
        }],
        initialisers: vec![(
            Ident::new("x_"),
            cpp_tree::Expr::Call {
                callee: Box::new(cpp_tree::Expr::Path(Path::new(&["std", "move"]))),
                args: vec![cpp_tree::Expr::Path(Path::single("x"))],
            },
        )],
        is_explicit: false,
        is_constexpr: false,
        is_noexcept: false,
        body: cpp_tree::FunctionBody::Block(cpp_tree::Block::default()),
    };
    let decl = cpp_tree::Decl::Class(cpp_tree::Class {
        doc: Doc::none(),
        is_struct: false,
        name: Ident::new("Point"),
        bases: Vec::new(),
        sections: vec![cpp_tree::Section {
            access: cpp_tree::Access::Public,
            members: vec![cpp_tree::ClassMember::Constructor(constructor)],
        }],
    });

    assert_eq!(
        zerodds_lang_render::cpp::decl(&decl),
        "\
class Point {
public:
    Point(int32_t x)
        : x_(std::move(x)) {}

};
"
    );
}

/// The C tree gained `Expr::Paren` because `#define MOVING (Flags)1 << 4`
/// expands inside `x & MOVING` as `(x & (Flags)1) << 4`. Neither tree
/// carries operator precedence, so the C++ one had the same gap: a
/// shifted sum renders as `a + b << c` and nothing says it was meant the
/// other way round.
#[test]
fn a_cpp_expression_brackets_only_where_the_tree_asked_for_it() {
    let sum = cpp_tree::Expr::Binary {
        op: "+",
        lhs: Box::new(cpp_tree::Expr::Path(Path::single("a"))),
        rhs: Box::new(cpp_tree::Expr::Path(Path::single("b"))),
    };
    let shifted = |lhs: cpp_tree::Expr| {
        cpp_tree::Decl::Variable(cpp_tree::Variable {
            doc: Doc::none(),
            type_: cpp_tree::Type::Primitive(cpp_tree::Primitive::Int32),
            name: Ident::new("shifted"),
            value: Some(cpp_tree::Expr::Binary {
                op: "<<",
                lhs: Box::new(lhs),
                rhs: Box::new(cpp_tree::Expr::Literal(Literal::int(4))),
            }),
            is_static: false,
            is_constexpr: true,
            is_inline: false,
        })
    };

    assert_eq!(
        zerodds_lang_render::cpp::decl(&shifted(sum.clone())),
        "constexpr int32_t shifted = a + b << 4;\n",
        "the tree holds no precedence, so an unbracketed sum stays unbracketed"
    );
    assert_eq!(
        zerodds_lang_render::cpp::decl(&shifted(cpp_tree::Expr::Paren(Box::new(sum)))),
        "constexpr int32_t shifted = (a + b) << 4;\n"
    );
}

#[test]
fn a_cpp_namespace_body_is_not_indented() {
    let decl = cpp_tree::Decl::Namespace(cpp_tree::Namespace {
        doc: Doc::none(),
        name: Ident::new("dds"),
        decls: vec![cpp_tree::Decl::Forward {
            is_struct: true,
            name: Ident::new("Pose"),
        }],
    });
    assert_eq!(
        zerodds_lang_render::cpp::decl(&decl),
        "\
namespace dds {
struct Pose;
} // namespace dds
"
    );
}

#[test]
fn a_cpp_template_specialisation_renders_its_header_above_the_declaration() {
    let decl = cpp_tree::Decl::Template(cpp_tree::Template {
        doc: Doc::none(),
        params: Vec::new(),
        decl: Box::new(cpp_tree::Decl::Forward {
            is_struct: true,
            name: Ident::new("topic_type_support"),
        }),
        specialisation: Vec::new(),
    });
    assert_eq!(
        zerodds_lang_render::cpp::decl(&decl),
        "\
template <>
struct topic_type_support;
"
    );
}

#[test]
fn a_cpp_rooted_path_keeps_its_leading_colons() {
    // `::Point` and `Point` name different things inside a namespace.
    let type_ = cpp_tree::Type::named(Path::rooted(&["Point"]));
    assert_eq!(zerodds_lang_render::cpp::type_(&type_), "::Point");
}

// -- C -------------------------------------------------------------------

mod c_tree {
    pub use zerodds_lang_cst::c::*;
}

/// The tree has no `else if` node: an else branch is a block, and a
/// chain is a block holding one `if`. Rendered literally that nests one
/// pair of braces per arm, and a `@mutable` decoder has one arm per
/// member — C99 §5.2.4.1 requires a compiler to support only 127 levels
/// of nested compound statement, so a large enough type produced a file
/// no conforming compiler must accept.
#[test]
fn a_c_else_holding_one_if_renders_as_else_if() {
    let refuse = |name: &str| c_tree::Block {
        statements: vec![c_tree::Statement::Expr(c_tree::Expr::Call {
            callee: Box::new(c_tree::Expr::Name(Ident::new(name))),
            args: Vec::new(),
        })],
    };
    let test = |value: i128| c_tree::Expr::Binary {
        op: "==",
        lhs: Box::new(c_tree::Expr::Name(Ident::new("id"))),
        rhs: Box::new(c_tree::Expr::Literal(Literal::int(value))),
    };
    let decl = c_tree::Decl::Function(c_tree::Function {
        doc: Doc::none(),
        is_static: true,
        is_inline: false,
        is_maybe_unused: false,
        returns: c_tree::Type::Void,
        name: Ident::new("dispatch"),
        params: Vec::new(),
        body: Some(c_tree::Block {
            statements: vec![c_tree::Statement::If {
                condition: test(0),
                then: refuse("first"),
                otherwise: Some(c_tree::Block {
                    statements: vec![c_tree::Statement::If {
                        condition: test(1),
                        then: refuse("second"),
                        otherwise: Some(refuse("otherwise")),
                    }],
                }),
            }],
        }),
    });

    assert_eq!(
        zerodds_lang_render::c::decl(&decl),
        "\
static void dispatch(void) {
    if (id == 0) {
        first();
    } else if (id == 1) {
        second();
    } else {
        otherwise();
    }
}
"
    );
}

/// An else branch that is not a lone `if` keeps its braces: flattening
/// it would move the statements beside it into the wrong scope.
#[test]
fn a_c_else_holding_anything_else_keeps_its_block() {
    let call = |name: &str| {
        c_tree::Statement::Expr(c_tree::Expr::Call {
            callee: Box::new(c_tree::Expr::Name(Ident::new(name))),
            args: Vec::new(),
        })
    };
    let decl = c_tree::Decl::Function(c_tree::Function {
        doc: Doc::none(),
        is_static: true,
        is_inline: false,
        is_maybe_unused: false,
        returns: c_tree::Type::Void,
        name: Ident::new("dispatch"),
        params: Vec::new(),
        body: Some(c_tree::Block {
            statements: vec![c_tree::Statement::If {
                condition: c_tree::Expr::Name(Ident::new("flag")),
                then: c_tree::Block {
                    statements: vec![call("first")],
                },
                otherwise: Some(c_tree::Block {
                    statements: vec![
                        c_tree::Statement::If {
                            condition: c_tree::Expr::Name(Ident::new("other")),
                            then: c_tree::Block {
                                statements: vec![call("second")],
                            },
                            otherwise: None,
                        },
                        call("after"),
                    ],
                }),
            }],
        }),
    });

    assert_eq!(
        zerodds_lang_render::c::decl(&decl),
        "\
static void dispatch(void) {
    if (flag) {
        first();
    } else {
        if (other) {
            second();
        }
        after();
    }
}
"
    );
}

#[test]
fn a_c_struct_renders_with_its_members() {
    let decl = c_tree::Decl::Struct(c_tree::Struct {
        body_text: zerodds_lang_cst::common::BodyText::none(),
        doc: Doc::none(),
        name: Some(Ident::new("Point")),
        members: vec![
            c_tree::Member {
                doc: Doc::none(),
                type_: c_tree::Type::Primitive(c_tree::Primitive::Int32),
                name: Ident::new("x"),
                dims: Vec::new(),
                bits: None,
            },
            c_tree::Member {
                doc: Doc::none(),
                type_: c_tree::Type::Primitive(c_tree::Primitive::Char),
                name: Ident::new("label"),
                dims: vec![c_tree::Expr::Literal(Literal::int(32))],
                bits: None,
            },
        ],
    });

    assert_eq!(
        zerodds_lang_render::c::decl(&decl),
        "\
struct Point {
    int32_t x;
    char label[32];
};
"
    );
}

#[test]
fn a_c_prototype_with_no_parameters_writes_void() {
    // An empty `()` in C means "unspecified", which is not what a
    // generated prototype states.
    let decl = c_tree::Decl::Function(c_tree::Function {
        doc: Doc::none(),
        is_static: false,
        is_inline: false,
        is_maybe_unused: false,
        returns: c_tree::Type::Void,
        name: Ident::new("reset"),
        params: Vec::new(),
        body: None,
    });
    assert_eq!(zerodds_lang_render::c::decl(&decl), "void reset(void);\n");
}

#[test]
fn a_c_pointer_type_binds_to_the_declarator() {
    let decl = c_tree::Decl::Function(c_tree::Function {
        doc: Doc::none(),
        is_static: true,
        is_inline: true,
        is_maybe_unused: false,
        returns: c_tree::Type::Pointer(Box::new(c_tree::Type::Void)),
        name: Ident::new("buffer"),
        params: vec![c_tree::Param {
            type_: c_tree::Type::Const(Box::new(c_tree::Type::Pointer(Box::new(
                c_tree::Type::Primitive(c_tree::Primitive::Char),
            )))),
            name: Some(Ident::new("name")),
        }],
        body: None,
    });
    assert_eq!(
        zerodds_lang_render::c::decl(&decl),
        "static inline void *buffer(const char *name);\n"
    );
}

#[test]
fn a_c_file_writes_its_include_guard_at_both_ends() {
    let file = c_tree::File {
        doc: Doc::none(),
        guard: Some(Ident::new("POINT_H")),
        includes: vec![c_tree::Include::system("stdint.h")],
        decls: vec![c_tree::Decl::Typedef(c_tree::Typedef {
            doc: Doc::none(),
            type_: c_tree::Type::Primitive(c_tree::Primitive::Int32),
            name: Ident::new("PointId"),
            dims: Vec::new(),
        })],
    };
    assert_eq!(
        zerodds_lang_render::c::file(&file),
        "\
#ifndef POINT_H
#define POINT_H

#include <stdint.h>

typedef int32_t PointId;

#endif /* POINT_H */
"
    );
}

#[test]
fn a_c_enum_has_no_comma_after_its_last_enumerator() {
    // A trailing comma is C99 and later; the emitters this replaces did
    // not write one, and the fixture output records that.
    let decl = c_tree::Decl::Enum(c_tree::Enum {
        doc: Doc::none(),
        name: Some(Ident::new("Mode")),
        enumerators: vec![
            c_tree::Enumerator {
                doc: Doc::none(),
                name: Ident::new("IDLE"),
                value: Some(c_tree::Expr::Literal(Literal::int(0))),
            },
            c_tree::Enumerator {
                doc: Doc::none(),
                name: Ident::new("ACTIVE"),
                value: Some(c_tree::Expr::Literal(Literal::int(1))),
            },
        ],
    });
    assert_eq!(
        zerodds_lang_render::c::decl(&decl),
        "\
enum Mode {
    IDLE = 0,
    ACTIVE = 1
};
"
    );
}

// -- Java ----------------------------------------------------------------

mod java_tree {
    pub use zerodds_lang_cst::java::*;
}

#[test]
fn a_java_class_renders_its_modifiers_annotations_and_members() {
    let decl = java_tree::TypeDecl::Class(java_tree::Class {
        doc: Doc::none(),
        annotations: vec![java_tree::Annotation {
            path: Path::single("Generated"),
            args: vec![java_tree::AnnotationArg::Value(java_tree::Expr::Literal(
                Literal::string("zerodds"),
            ))],
        }],
        modifiers: vec![java_tree::Modifier::Public, java_tree::Modifier::Final],
        name: Ident::new("Pose"),
        generics: Vec::new(),
        extends: None,
        implements: Vec::new(),
        members: vec![
            java_tree::Member::Field(java_tree::Field {
                doc: Doc::none(),
                annotations: Vec::new(),
                modifiers: vec![java_tree::Modifier::Private],
                type_: java_tree::Type::Primitive(java_tree::Primitive::Int),
                name: Ident::new("x"),
                value: None,
            }),
            java_tree::Member::Method(java_tree::Method {
                doc: Doc::none(),
                annotations: Vec::new(),
                modifiers: vec![java_tree::Modifier::Public],
                generics: Vec::new(),
                returns: java_tree::Type::Primitive(java_tree::Primitive::Int),
                name: Ident::new("getX"),
                params: Vec::new(),
                throws: Vec::new(),
                body: Some(java_tree::Block {
                    statements: vec![java_tree::Statement::Return(Some(java_tree::Expr::Field {
                        value: Box::new(java_tree::Expr::This),
                        name: Ident::new("x"),
                    }))],
                }),
            }),
        ],
    });

    assert_eq!(
        zerodds_lang_render::java::type_decl(&decl),
        "\
@Generated(\"zerodds\")
public final class Pose {
    private int x;

    public int getX() {
        return this.x;
    }
}
"
    );
}

#[test]
fn a_java_enum_ends_its_constants_with_a_semicolon_only_when_members_follow() {
    let bare = java_tree::TypeDecl::Enum(java_tree::Enum {
        doc: Doc::none(),
        annotations: Vec::new(),
        modifiers: vec![java_tree::Modifier::Public],
        name: Ident::new("Mode"),
        implements: Vec::new(),
        constants: vec![
            java_tree::EnumConstant {
                doc: Doc::none(),
                annotations: Vec::new(),
                name: Ident::new("IDLE"),
                args: Vec::new(),
                body: Vec::new(),
            },
            java_tree::EnumConstant {
                doc: Doc::none(),
                annotations: Vec::new(),
                name: Ident::new("ACTIVE"),
                args: Vec::new(),
                body: Vec::new(),
            },
        ],
        members: Vec::new(),
    });
    assert_eq!(
        zerodds_lang_render::java::type_decl(&bare),
        "\
public enum Mode {
    IDLE,
    ACTIVE
}
"
    );
}

#[test]
fn a_java_generic_type_renders_its_arguments_with_dots_in_the_path() {
    let type_ = java_tree::Type::generic(
        Path::new(&["java", "util", "List"]),
        vec![java_tree::Type::named(Path::single("Pose"))],
    );
    assert_eq!(
        zerodds_lang_render::java::type_(&type_),
        "java.util.List<Pose>"
    );
}

#[test]
fn a_java_file_writes_its_package_and_imports() {
    let file = java_tree::File {
        doc: Doc::none(),
        package: Some(Path::new(&["org", "zerodds", "generated"])),
        imports: vec![java_tree::Import {
            path: Path::new(&["java", "util", "List"]),
            static_: false,
            wildcard: false,
        }],
        types: vec![java_tree::TypeDecl::Interface(java_tree::Interface {
            doc: Doc::none(),
            annotations: Vec::new(),
            modifiers: vec![java_tree::Modifier::Public],
            name: Ident::new("Marker"),
            generics: Vec::new(),
            extends: Vec::new(),
            members: Vec::new(),
        })],
    };
    assert_eq!(
        zerodds_lang_render::java::file(&file),
        "\
package org.zerodds.generated;

import java.util.List;

public interface Marker {
}
"
    );
}

// -- what every renderer must not do -------------------------------------

#[test]
fn every_renderer_ends_its_output_with_exactly_one_newline() {
    // A file that ends without one, or with several, differs from what
    // every formatter produces and shows up as a byte difference.
    let rust = zerodds_lang_render::rust::file(&rust_tree::File::default());
    let cpp = zerodds_lang_render::cpp::file(&cpp_tree::File::default());
    let c = zerodds_lang_render::c::file(&c_tree::File::default());
    let java = zerodds_lang_render::java::file(&java_tree::File::default());
    for (name, text) in [("rust", rust), ("cpp", cpp), ("c", c), ("java", java)] {
        assert!(text.is_empty(), "{name} wrote something for an empty file");
    }
}

#[test]
fn no_rendered_line_carries_trailing_whitespace() {
    let item = rust_tree::Item::Struct(rust_tree::Struct {
        body_text: zerodds_lang_cst::common::BodyText::none(),
        doc: Doc::lines(&["first", "", "third"]),
        attrs: Vec::new(),
        derives: Vec::new(),
        visibility: Visibility::Private,
        name: Ident::new("S"),
        generics: Vec::new(),
        body: rust_tree::StructBody::Named(vec![rust_field(
            "a",
            rust_tree::Type::Primitive(rust_tree::Primitive::U8),
        )]),
    });
    for line in zerodds_lang_render::rust::item(&item).lines() {
        assert_eq!(line, line.trim_end(), "trailing whitespace on {line:?}");
    }
}
