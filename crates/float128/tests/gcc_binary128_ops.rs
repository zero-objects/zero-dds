// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Addition, subtraction, multiplication and division against GCC's
//! `__float128`.
//!
//! IEEE 754-2008 clause 5.4.1: each of the four computes the exact result
//! and rounds it once, to nearest with ties to even by default (clause
//! 4.3.3). `tests/oracle/x86_64-gcc-14.2-ops.txt` holds what libgcc's
//! soft-fp answered for 6072 operand pairs (`tests/oracle/quad_ops.c`,
//! measured on the Linux test host with `gcc (Debian 14.2.0-19) 14.2.0`):
//! every pattern of `tests/oracle/patterns.txt` with four neighbours —
//! zeros, subnormals, the extremes, values around one — and 4000
//! pseudo-random pairs with close exponents (cancellation, the halfway
//! cases of the alignment) and with exponents from the whole range
//! (overflow, subnormal results). Every result bit is compared; a NaN
//! result is compared as a NaN, because the quiet NaN this crate returns
//! need not carry GCC's payload or sign.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_float128::Float128;

const ORACLE: &str = include_str!("oracle/x86_64-gcc-14.2-ops.txt");

fn pattern(text: &str) -> u128 {
    u128::from_str_radix(text, 16).expect("a 32-digit hex pattern")
}

fn agrees(ours: Float128, theirs: u128) -> bool {
    let theirs = Float128::from_bits(theirs);
    if theirs.is_nan() {
        return ours.is_nan();
    }
    ours.to_bits() == theirs.to_bits()
}

#[test]
fn ieee754_5_4_1_the_four_operations_round_as_gcc_float128_does() {
    let mut rows = 0;
    let mut wrong = Vec::new();
    for line in ORACLE.lines() {
        let fields: Vec<u128> = line.split_whitespace().map(pattern).collect();
        let [a, b, sum, difference, product, quotient] = fields[..] else {
            panic!("a row has {} fields: {line}", fields.len());
        };
        rows += 1;
        let (x, y) = (Float128::from_bits(a), Float128::from_bits(b));
        for (name, ours, theirs) in [
            ("+", x + y, sum),
            ("-", x - y, difference),
            ("*", x * y, product),
            ("/", x / y, quotient),
        ] {
            if !agrees(ours, theirs) {
                wrong.push(format!(
                    "{a:032x} {name} {b:032x}: ours {:032x}, gcc {theirs:032x}",
                    ours.to_bits()
                ));
            }
        }
    }
    assert!(rows > 6000, "{rows} rows");
    assert!(
        wrong.is_empty(),
        "{} of {} results differ:\n{}",
        wrong.len(),
        rows * 4,
        wrong
            .iter()
            .take(40)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}
