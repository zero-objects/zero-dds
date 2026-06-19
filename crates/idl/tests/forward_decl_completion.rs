//! Integration test: forward-decl completion (C4.6 §1.5).
//!
//! Spec §7.5.4: every forward-decl must be defined in the same translation
//! unit, otherwise an error.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use zerodds_idl::config::ParserConfig;
use zerodds_idl::parse;
use zerodds_idl::semantics::{Resolver, ResolverError};

fn build(src: &str) -> Resolver {
    let ast = parse(src, &ParserConfig::default()).expect("parse");
    let mut r = Resolver::new();
    r.build(&ast);
    r
}

#[test]
fn forward_then_definition_is_complete() {
    let r = build("struct Foo; struct Foo { long x; };");
    let errs = r.forward_decl_errors();
    assert!(errs.is_empty(), "got {errs:?}");
}

#[test]
fn forward_without_definition_is_error() {
    let r = build("struct DangleMe;");
    let errs = r.forward_decl_errors();
    assert_eq!(errs.len(), 1);
    assert!(matches!(
        &errs[0],
        ResolverError::ForwardDeclNotCompleted { name, .. } if name == "DangleMe"
    ));
}

#[test]
fn union_forward_without_definition_is_error() {
    let r = build("union UDangle;");
    let errs = r.forward_decl_errors();
    assert_eq!(errs.len(), 1);
}

#[test]
fn nested_module_forward_completion_works() {
    let r = build("module M { struct Inner; struct Inner { long x; }; };");
    let errs = r.forward_decl_errors();
    assert!(errs.is_empty(), "got {errs:?}");
}

#[test]
fn forward_in_outer_completed_in_inner_is_not_completion() {
    // Spec §7.5.4: forward decl + full form must be in the *same* scope.
    // `struct Foo;` outer, `struct Foo {...};` in M counts as different.
    let r = build("struct Foo; module M { struct Foo { long x; }; };");
    let errs = r.forward_decl_errors();
    assert_eq!(
        errs.len(),
        1,
        "expected outer Foo to remain forward; got {errs:?}"
    );
}
