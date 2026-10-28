// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Writes the oracle's input: bit patterns, and what this crate prints
//! for them.
//!
//! `crates/float128/tests/oracle/README.md` is the procedure. The output
//! goes to `tests/oracle/patterns.txt` and is checked in, so the oracle
//! run on the Linux host and the test that compares against it are
//! looking at the same values a year from now.
//!
//! ```sh
//! cargo run -p zerodds-float128 --example emit_oracle_input \
//!     > crates/float128/tests/oracle/patterns.txt
//! ```
#![allow(clippy::print_stdout)]

use zerodds_float128::Float128;

/// The same generator the tests use, so the corner cases and the random
/// ones come from one place.
struct Xorshift(u64);

impl Xorshift {
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

fn main() {
    let named = [
        Float128::ZERO,
        Float128::NEG_ZERO,
        Float128::ONE,
        Float128::NEG_ONE,
        Float128::MAX,
        Float128::MIN,
        Float128::MIN_POSITIVE,
        Float128::MIN_SUBNORMAL,
        Float128::EPSILON,
        Float128::from_bits(Float128::ONE.to_bits() + 1),
        Float128::from_bits(Float128::MIN_POSITIVE.to_bits() - 1),
        Float128::from_f64(1.5),
        Float128::from_f64(-2.75),
        Float128::from_f64(0.1),
        Float128::from_f64(f64::MAX),
        Float128::from_f64(f64::MIN_POSITIVE),
        Float128::from_bits(Float128::ONE.to_bits() | (1u128 << 52)),
        Float128::from_bits(Float128::ONE.to_bits() | (1u128 << 12)),
    ];
    let mut rng = Xorshift(0x0aac_1e50_dead_beef);
    let mut patterns: Vec<Float128> = named.to_vec();
    for _ in 0..500 {
        let bits = (u128::from(rng.next_u64()) << 64) | u128::from(rng.next_u64());
        // Finite only: quadmath prints `inf` and `nan` without digits,
        // and those cases are pinned by name in the unit tests instead.
        let exponent = u128::from(rng.next_u64() as u16 % 0x7fff) << 112;
        patterns.push(Float128::from_bits(
            (bits & !(0x7fffu128 << 112)) | exponent,
        ));
    }
    for value in patterns {
        println!("{:032x} {}", value.to_bits(), value.to_shortest_decimal());
    }
}
