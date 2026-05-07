//! Integration-Test: Union-Validierung (C4.6 §1.8).

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
use zerodds_idl::semantics::{UnionValidationError, validate_unions};

fn errs(src: &str) -> Vec<UnionValidationError> {
    let ast = parse(src, &ParserConfig::default()).expect("parse");
    validate_unions(&ast)
}

#[test]
fn long_discriminator_with_int_labels_passes() {
    let e = errs("union U switch (long) { case 1: long a; case 2: long b; default: long c; };");
    assert!(e.is_empty(), "got {e:?}");
}

#[test]
fn short_discriminator_with_int_labels_passes() {
    let e = errs("union U switch (short) { case 1: long a; case 2: long b; };");
    assert!(e.is_empty(), "got {e:?}");
}

#[test]
fn duplicate_default_branch_fails() {
    let e = errs("union U switch (long) { default: long a; default: long b; };");
    assert!(
        e.iter()
            .any(|x| matches!(x, UnionValidationError::DuplicateDefault { .. }))
    );
}

#[test]
fn boolean_disc_with_int_label_mismatch() {
    let e = errs("union U switch (boolean) { case 1: long a; default: long b; };");
    assert!(
        e.iter()
            .any(|x| matches!(x, UnionValidationError::LabelTypeMismatch { .. }))
    );
}

#[test]
fn duplicate_case_label_fails() {
    let e = errs("union U switch (long) { case 1: long a; case 1: long b; };");
    assert!(
        e.iter()
            .any(|x| matches!(x, UnionValidationError::DuplicateCaseLabel { .. }))
    );
}

#[test]
fn enum_discriminator_with_scoped_labels_passes() {
    let e = errs(
        "enum Color { RED, GREEN }; union U switch (Color) { case RED: long a; case GREEN: long b; };",
    );
    assert!(e.is_empty(), "got {e:?}");
}

#[test]
fn char_disc_with_char_labels_passes() {
    let e = errs("union U switch (char) { case 'a': long x; case 'b': long y; };");
    assert!(e.is_empty(), "got {e:?}");
}
