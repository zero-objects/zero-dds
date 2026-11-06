// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-corba-codegen` with `zerodds-lang-render`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-lang-render` (`src/special_types.rs`,
//! `src/stub.rs`, `src/skeleton.rs`): `cpp::type_`, `java::type_`,
//! `csharp::type_` and `rust::type_` for the special types of its mapping
//! table, and `cpp::decl`, `java::type_decl`, `csharp::type_decl` and
//! `rust::item` for the stubs and skeletons. It relies on a pointer, a
//! generic and an array arriving in each language's own spelling, and on a
//! qualified name keeping its separators.
//!
//! Oracle: ISO C++17 [dcl.ptr] (`void*`) and [temp.names]
//! (`std::vector<int32_t>`), the Java Language Specification §4.5 (a
//! parameterized type `java.util.List<java.lang.Object>`), ECMA-334 §17.1
//! (a single-dimensional array type `T[]`), the Rust reference ("Paths":
//! `zerodds_float128::Float128`; generic arguments in angle brackets).
#![allow(clippy::unwrap_used)]

use zerodds_lang_cst::common::Path;
use zerodds_lang_cst::{cpp, csharp, java, rust};

#[test]
fn a_cpp_pointer_and_a_template_are_written_as_cpp_writes_them() {
    let pointer = zerodds_lang_render::cpp::type_(&cpp::Type::Pointer(Box::new(cpp::Type::Void)));
    assert_eq!(pointer.replace(' ', ""), "void*");
    let vector = zerodds_lang_render::cpp::type_(&cpp::Type::template(
        Path::new(&["std", "vector"]),
        vec![cpp::Type::named(Path::single("int32_t"))],
    ));
    assert_eq!(vector, "std::vector<int32_t>");
}

#[test]
fn a_java_parameterized_type_keeps_its_dotted_names() {
    let text = zerodds_lang_render::java::type_(&java::Type::generic(
        Path::new(&["java", "util", "List"]),
        vec![java::Type::named(Path::new(&["java", "lang", "Object"]))],
    ));
    assert_eq!(text, "java.util.List<java.lang.Object>");
}

#[test]
fn a_csharp_array_is_its_element_and_brackets() {
    let text = zerodds_lang_render::csharp::type_(&csharp::Type::array(csharp::Type::Predefined(
        csharp::Predefined::Object,
    )));
    assert_eq!(text, "object[]");
}

#[test]
fn a_rust_path_and_a_generic_are_written_with_double_colons_and_angle_brackets() {
    let float128 = zerodds_lang_render::rust::type_(&rust::Type::named(Path::new(&[
        "zerodds_float128",
        "Float128",
    ])));
    assert_eq!(float128, "zerodds_float128::Float128");
    let vec = zerodds_lang_render::rust::type_(&rust::Type::generic(
        Path::single("Vec"),
        vec![rust::Type::Primitive(rust::Primitive::U64)],
    ));
    assert_eq!(vec, "Vec<u64>");
}
