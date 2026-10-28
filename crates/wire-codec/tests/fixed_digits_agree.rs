//! The two halves of the `fixed` digit-count axis give the same answer.
//!
//! `zerodds_wire::FixedDigits` and `zerodds_ast_layout::key::FixedDigitCount`
//! are the same axis held twice: a plan chooses between the two readings of
//! CORBA 3.3 part 2 §9.3.2.8, and a `Writer` applies one of them without
//! being able to see the planner — `zerodds-wire` does not depend on
//! `zerodds-ast-layout`, and must not, because a byte layer that needs the
//! layout planner is not a byte layer.
//!
//! That leaves a copy, and a copy nothing compares is a copy that drifts:
//! a `ValuePrecision` that stopped clamping zero to one digit on one side
//! only would write a `fixed` with no digit nibbles and a sign, and every
//! test that runs both halves together would still pass. This crate sees
//! both, so this is where they are held against each other.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast_layout::key::FixedDigitCount;
use zerodds_wire::FixedDigits;

/// The two enums map onto each other, value for value.
fn crossed(count: FixedDigitCount) -> FixedDigits {
    match count {
        FixedDigitCount::Declared => FixedDigits::Declared,
        FixedDigitCount::ValuePrecision => FixedDigits::ValuePrecision,
        FixedDigitCount::ValueStringDigits => FixedDigits::ValueStringDigits,
    }
}

#[test]
fn both_halves_answer_the_same_over_every_argument() {
    // IDL 4.2 rule (42) caps `P` at 31 and a value cannot have more
    // significant digits than that, but the arithmetic is checked over the
    // whole of `u8` on both axes: the clamp lives in the caller, and a half
    // that disagreed outside the declared range would be a half that
    // disagrees once the caller's clamp moves.
    // For ValueStringDigits, string_digits is the digit count of the input
    // string. We test with string_digits = significant as a representative
    // value; Declared and ValuePrecision ignore it.
    for count in FixedDigitCount::ALL {
        let digits = crossed(count);
        for declared in 0..=u8::MAX {
            for significant in 0..=u8::MAX {
                let string_digits = significant;
                assert_eq!(
                    count.wire_digits(declared, significant, string_digits),
                    digits.wire_digits(declared, significant, string_digits),
                    "{count:?}: the two halves disagree at declared={declared}, \
                     significant={significant}, string_digits={string_digits}"
                );
            }
        }
    }
}

/// Neither half ever answers zero.
///
/// A `fixed` of no digit nibbles is a field that carries a sign and nothing
/// to sign: `BigDecimal("0.00").precision()` is 1, and the packing pairs the
/// sign nibble with a digit.
#[test]
fn neither_half_ever_writes_a_fixed_without_digits() {
    for count in FixedDigitCount::ALL {
        for significant in 0..=u8::MAX {
            let string_digits = significant;
            assert_ne!(
                count.wire_digits(1, significant, string_digits),
                0,
                "{count:?} answers zero digits at significant={significant}"
            );
            assert_ne!(crossed(count).wire_digits(1, significant, string_digits), 0);
        }
    }
}
