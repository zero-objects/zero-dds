//! Integration test: bitfield/bitmask validation (C4.6 §1.9).

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
use zerodds_idl::semantics::{BitfieldValidationError, validate_bitfields};

fn errs(src: &str) -> Vec<BitfieldValidationError> {
    let ast = parse(src, &ParserConfig::default()).expect("parse");
    validate_bitfields(&ast)
}

#[test]
fn position_within_bound_passes() {
    let e = errs("@bit_bound(8) bitmask Flags { @position(0) F0, @position(1) F1 };");
    assert!(e.is_empty(), "got {e:?}");
}

#[test]
fn position_above_bound_fails() {
    let e = errs("@bit_bound(4) bitmask Flags { @position(0) F0, @position(8) F1 };");
    assert!(
        e.iter()
            .any(|x| matches!(x, BitfieldValidationError::PositionOutOfRange { .. }))
    );
}

#[test]
fn duplicate_position_fails() {
    let e = errs("bitmask Flags { @position(2) F0, @position(2) F1 };");
    assert!(
        e.iter()
            .any(|x| matches!(x, BitfieldValidationError::DuplicatePosition { .. }))
    );
}

#[test]
fn implicit_position_increments_ok() {
    let e = errs("@bit_bound(4) bitmask Flags { F0, F1, F2, F3 };");
    assert!(e.is_empty(), "got {e:?}");
}

#[test]
fn implicit_position_overflow_fails() {
    let e = errs("@bit_bound(2) bitmask Flags { F0, F1, F2 };");
    assert!(
        e.iter()
            .any(|x| matches!(x, BitfieldValidationError::PositionOutOfRange { .. }))
    );
}

#[test]
fn bit_bound_above_max_fails() {
    let e = errs("@bit_bound(128) bitmask Flags { F0 };");
    assert!(
        e.iter()
            .any(|x| matches!(x, BitfieldValidationError::BitBoundTooLarge { .. }))
    );
}
