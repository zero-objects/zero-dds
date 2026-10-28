// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! What the type has to do before the wire can use it.
//!
//! The four properties `docs/float128-column.md` §5 names, in order: the
//! sixteen-byte round trip, exact conversion with the inexact cases
//! reported, ordering including the NaN and signed-zero corners, and
//! decimal that reads back.
//!
//! `f64` is the oracle wherever a value has one. Every binary64 value is
//! a binary128 value, so `Float128::from_f64(x).to_f64()` must give `x`
//! back for every `x` — bit for bit, NaN payloads included — and a
//! decimal string must parse to the same place in both types. Where no
//! `f64` oracle exists the check is against the format itself:
//! `tests/oracle-x86_64.txt` records what GCC's `__float128` printed for
//! the same patterns on the Linux host, and `oracle.rs` compares.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use core::cmp::Ordering;
use zerodds_float128::{Class, Float128};

/// A deterministic bit-pattern generator, so a failure is reproducible
/// from the seed alone. `rand` is not a dependency of a `no_std` crate
/// that has none.
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

    fn next_u128(&mut self) -> u128 {
        (u128::from(self.next_u64()) << 64) | u128::from(self.next_u64())
    }
}

/// The values that break a float implementation, named.
fn corners() -> Vec<(&'static str, Float128)> {
    vec![
        ("zero", Float128::ZERO),
        ("negative zero", Float128::NEG_ZERO),
        ("one", Float128::ONE),
        ("minus one", Float128::NEG_ONE),
        ("infinity", Float128::INFINITY),
        ("negative infinity", Float128::NEG_INFINITY),
        ("quiet nan", Float128::NAN),
        ("nan with payload", Float128::quiet_nan(0x1234_5678)),
        ("negative nan", Float128::neg_quiet_nan(1)),
        ("max", Float128::MAX),
        ("min", Float128::MIN),
        ("min positive normal", Float128::MIN_POSITIVE),
        ("min subnormal", Float128::MIN_SUBNORMAL),
        ("epsilon", Float128::EPSILON),
        (
            "one plus epsilon",
            Float128::from_bits(Float128::ONE.to_bits() + 1),
        ),
        (
            "largest subnormal",
            Float128::from_bits(Float128::MIN_POSITIVE.to_bits() - 1),
        ),
        ("one and a half", Float128::from_f64(1.5)),
        ("minus two and three quarters", Float128::from_f64(-2.75)),
    ]
}

// -- 1. the sixteen bytes ------------------------------------------------

#[test]
fn every_pattern_round_trips_through_sixteen_bytes_in_both_orders() {
    let mut rng = Xorshift(0x5eed_1234_5678_9abc);
    let mut patterns: Vec<u128> = corners().iter().map(|(_, v)| v.to_bits()).collect();
    for _ in 0..20_000 {
        patterns.push(rng.next_u128());
    }
    for bits in patterns {
        let value = Float128::from_bits(bits);
        assert_eq!(Float128::from_be_bytes(value.to_be_bytes()).to_bits(), bits);
        assert_eq!(Float128::from_le_bytes(value.to_le_bytes()).to_bits(), bits);
        // CORBA Figure 9.2: the two orders are the whole sixteen bytes
        // reversed, not two halves swapped in place.
        let mut reversed = value.to_be_bytes();
        reversed.reverse();
        assert_eq!(
            reversed,
            value.to_le_bytes(),
            "byte order is not a whole-value reversal"
        );
    }
}

#[test]
fn one_and_a_half_is_the_pattern_the_vendors_write() {
    // Measured from GCC 14.2 `__float128` on Debian 13 x86-64 and from
    // RTI Connext 7.7's own serialiser, both in
    // `docs/float128-column.md` §2.
    let value = Float128::from_f64(1.5);
    assert_eq!(
        value.to_le_bytes(),
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x80, 0xff, 0x3f]
    );
    assert_eq!(
        value.to_be_bytes(),
        [0x3f, 0xff, 0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
    );
}

#[test]
fn minus_two_and_three_quarters_matches_the_measured_pattern() {
    let value = Float128::from_f64(-2.75);
    assert_eq!(
        value.to_le_bytes(),
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x60, 0x00, 0xc0]
    );
}

// -- 2. conversion, with the inexact cases reported ----------------------

#[test]
fn every_f64_widens_and_narrows_back_unchanged() {
    let mut rng = Xorshift(0xdead_beef_cafe_f00d);
    let mut values: Vec<f64> = vec![
        0.0,
        -0.0,
        1.0,
        -1.0,
        f64::MIN,
        f64::MAX,
        f64::MIN_POSITIVE,
        f64::from_bits(1),
        f64::from_bits(0x000f_ffff_ffff_ffff),
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::EPSILON,
    ];
    for _ in 0..50_000 {
        values.push(f64::from_bits(rng.next_u64()));
    }
    for value in values {
        let wide = Float128::from_f64(value);
        let back = wide
            .to_f64()
            .expect("a widened f64 always narrows back exactly");
        if value.is_nan() {
            assert!(back.is_nan(), "{value} lost its NaN-ness");
            assert_eq!(
                back.to_bits(),
                value.to_bits(),
                "{value} lost its NaN payload"
            );
        } else {
            assert_eq!(back.to_bits(), value.to_bits(), "{value} did not survive");
        }
    }
}

#[test]
fn every_f32_widens_and_narrows_back_unchanged() {
    let mut rng = Xorshift(0x0123_4567_89ab_cdef);
    for _ in 0..50_000 {
        let value = f32::from_bits(rng.next_u64() as u32);
        let back = Float128::from_f32(value)
            .to_f32()
            .expect("a widened f32 always narrows back exactly");
        assert_eq!(back.to_bits(), value.to_bits());
    }
}

#[test]
fn a_value_a_double_cannot_hold_is_refused_by_name() {
    // 1 + 2^-60 needs 61 significand bits: no `double` holds it, and the
    // old writer put the `double` 1.0 on the wire for it in silence.
    let value = "1.000000000000000000867361737988403547205962240695953369140625"
        .parse::<Float128>()
        .expect("an exact decimal");
    assert_ne!(value.to_bits(), Float128::ONE.to_bits());
    let error = value
        .to_f64()
        .expect_err("a 61-bit significand does not fit a double");
    assert_eq!(
        error.rounded, 1.0,
        "the rounded value is carried, not hidden"
    );
    assert_eq!(error.source.to_bits(), value.to_bits());
}

#[test]
fn a_value_only_binary128_can_hold_survives_the_wire_and_refuses_a_double() {
    // 1 + 2^-100 needs 101 significand bits: past `double` at 53 and
    // past x87 extended at 64, so only binary128 carries it.
    let mut bits = Float128::ONE.to_bits();
    bits |= 1u128 << 12; // 2^-100 relative to the leading one at bit 112
    let value = Float128::from_bits(bits);
    assert!(value.to_f64().is_err());
    let (_, x87_exact) = value.to_x87_extended();
    assert!(
        !x87_exact,
        "x87 extended cannot hold 101 significand bits either"
    );
    assert_eq!(Float128::from_le_bytes(value.to_le_bytes()).to_bits(), bits);
}

#[test]
fn the_narrowing_rounds_the_way_the_hardware_does() {
    // Every binary128 built from a double and nudged by one ulp of the
    // wider format must round back to a neighbour of the original, and
    // ties must go to even. The `f64` unit is the oracle.
    let mut rng = Xorshift(0xfeed_face_1234_5678);
    for _ in 0..20_000 {
        let base = f64::from_bits(rng.next_u64() & 0x7fef_ffff_ffff_ffff);
        // Normals only. One ulp of a *subnormal* double is not one ulp
        // of the binary128 it widens to — the widened value is a normal
        // with a different exponent — so `1 << 59` would not be half a
        // step there and the test would be asserting the wrong thing.
        if !base.is_normal() || base == f64::MAX {
            continue;
        }
        let wide = Float128::from_f64(base);
        // Exactly halfway to the next double: the tie case.
        let half_up = Float128::from_bits(wide.to_bits() + (1u128 << 59));
        let rounded = half_up.to_f64_rounded();
        let expected = if base.to_bits() & 1 == 0 {
            base
        } else {
            f64::from_bits(base.to_bits() + 1)
        };
        assert_eq!(
            rounded.to_bits(),
            expected.to_bits(),
            "a tie above {base} did not go to even"
        );
        assert!(half_up.to_f64().is_err(), "a tie is not exact");
    }
}

#[test]
fn overflow_and_underflow_are_reported_not_wrapped() {
    let huge = Float128::MAX;
    let error = huge.to_f64().expect_err("binary128 MAX is past f64 MAX");
    assert!(error.rounded.is_infinite());

    let tiny = Float128::MIN_SUBNORMAL;
    let error = tiny
        .to_f64()
        .expect_err("binary128 MIN_SUBNORMAL is below f64");
    assert_eq!(error.rounded, 0.0);
    assert!(error.rounded.is_sign_positive());

    let tiny_negative = Float128::MIN_SUBNORMAL.neg();
    let error = tiny_negative.to_f64().expect_err("below f64, negative");
    assert!(error.rounded.is_sign_negative(), "underflow keeps its sign");
}

#[test]
fn integers_convert_exactly_up_to_the_significand_and_are_refused_past_it() {
    for shift in 0..113u32 {
        let value = (1i128 << shift) | 1;
        if shift == 0 {
            continue;
        }
        let wide = Float128::try_from_i128(value).expect("under 113 bits is exact");
        assert_eq!(wide.to_i128().expect("and comes back"), value);
    }
    let past = (1i128 << 113) | 1;
    let error = Float128::try_from_i128(past).expect_err("114 bits does not fit");
    assert_eq!(
        error
            .rounded
            .to_i128()
            .expect("the rounded value is an integer"),
        1i128 << 113
    );
}

#[test]
fn a_fraction_is_not_an_integer() {
    let half = Float128::from_f64(0.5);
    let error = half.to_i128().expect_err("0.5 is not an integer");
    assert_eq!(error.rounded, 0);
    assert!(Float128::NAN.to_i128().is_err());
    assert!(Float128::INFINITY.to_i128().is_err());
}

// -- 3. ordering ---------------------------------------------------------

#[test]
fn ieee_equality_matches_f64_for_every_double() {
    let pairs: [(f64, f64); 7] = [
        (0.0, -0.0),
        (1.0, 1.0),
        (1.0, -1.0),
        (f64::NAN, f64::NAN),
        (f64::NAN, 1.0),
        (f64::INFINITY, f64::INFINITY),
        (f64::INFINITY, f64::NEG_INFINITY),
    ];
    for (left, right) in pairs {
        let wide_left = Float128::from_f64(left);
        let wide_right = Float128::from_f64(right);
        assert_eq!(
            wide_left == wide_right,
            left == right,
            "equality of {left} and {right} disagrees with f64"
        );
        assert_eq!(
            wide_left.partial_cmp(&wide_right),
            left.partial_cmp(&right),
            "ordering of {left} and {right} disagrees with f64"
        );
    }
}

#[test]
fn total_order_is_total_and_matches_f64() {
    let mut rng = Xorshift(0xabcd_ef01_2345_6789);
    for _ in 0..20_000 {
        let left = f64::from_bits(rng.next_u64());
        let right = f64::from_bits(rng.next_u64());
        assert_eq!(
            Float128::from_f64(left).total_cmp(Float128::from_f64(right)),
            left.total_cmp(&right),
            "total order of {left} and {right} disagrees with f64"
        );
    }
}

#[test]
fn total_order_places_the_corners_where_ieee_says() {
    let ordered = [
        Float128::neg_quiet_nan(0),
        Float128::NEG_INFINITY,
        Float128::MIN,
        Float128::NEG_ONE,
        Float128::MIN_SUBNORMAL.neg(),
        Float128::NEG_ZERO,
        Float128::ZERO,
        Float128::MIN_SUBNORMAL,
        Float128::MIN_POSITIVE,
        Float128::ONE,
        Float128::MAX,
        Float128::INFINITY,
        Float128::NAN,
    ];
    for window in ordered.windows(2) {
        assert_eq!(
            window[0].total_cmp(window[1]),
            Ordering::Less,
            "{:032x} should sort below {:032x}",
            window[0].to_bits(),
            window[1].to_bits()
        );
    }
    // And the signed zeros, which `==` calls equal and `total_cmp` does
    // not — the distinction a key holder depends on.
    assert!(Float128::NEG_ZERO == Float128::ZERO);
    assert_eq!(Float128::NEG_ZERO.total_cmp(Float128::ZERO), Ordering::Less);
}

#[test]
fn classification_names_every_corner() {
    assert_eq!(Float128::ZERO.classify(), Class::Zero);
    assert_eq!(Float128::NEG_ZERO.classify(), Class::Zero);
    assert_eq!(Float128::ONE.classify(), Class::Normal);
    assert_eq!(Float128::MIN_POSITIVE.classify(), Class::Normal);
    assert_eq!(Float128::MIN_SUBNORMAL.classify(), Class::Subnormal);
    assert_eq!(Float128::INFINITY.classify(), Class::Infinite);
    assert_eq!(Float128::NAN.classify(), Class::Nan);
    assert!(Float128::NAN.is_quiet_nan());
    assert!(!Float128::from_bits(Float128::INFINITY.to_bits() | 1).is_quiet_nan());
}

// -- 4. decimal ----------------------------------------------------------

#[test]
fn the_shortest_decimal_reads_back_as_the_same_pattern() {
    let mut rng = Xorshift(0x1111_2222_3333_4444);
    let mut values: Vec<Float128> = corners().iter().map(|(_, v)| *v).collect();
    for _ in 0..400 {
        // Finite patterns only: an infinity and a NaN have no digits,
        // and their round trip is checked separately.
        let bits = rng.next_u128() & !(0x7fffu128 << 112);
        let exponent = u128::from(rng.next_u64() as u16 % 0x7fff);
        values.push(Float128::from_bits(bits | (exponent << 112)));
    }
    for value in values {
        let text = value.to_shortest_decimal();
        if !value.is_finite() {
            continue;
        }
        let back: Float128 = text.parse().unwrap_or_else(|error| {
            panic!("`{text}` came out of the formatter and back as {error}")
        });
        assert_eq!(
            back.to_bits(),
            value.to_bits(),
            "`{text}` does not read back as 0x{:032x}",
            value.to_bits()
        );
        // Significant digits only: the exponent's own digits are not
        // part of the count binary128's 36 is a bound on.
        let mantissa = text.split(['e', 'E']).next().unwrap_or(&text);
        let digits = mantissa.chars().filter(char::is_ascii_digit).count();
        assert!(
            digits <= Float128::DIGITS_ROUND_TRIP,
            "`{text}` needed more than the thirty-six digits binary128 is supposed to take"
        );
    }
}

#[test]
fn the_exact_decimal_reads_back_as_the_same_pattern() {
    let values = [
        Float128::MIN_SUBNORMAL,
        Float128::MIN_POSITIVE,
        Float128::MAX,
        Float128::from_f64(0.1),
        Float128::from_f64(-2.75),
        Float128::EPSILON,
    ];
    for value in values {
        let text = value.to_exact_decimal();
        let back: Float128 = text.parse().expect("an exact expansion parses");
        assert_eq!(
            back.to_bits(),
            value.to_bits(),
            "the exact form of 0x{:032x} did not return",
            value.to_bits()
        );
    }
}

#[test]
fn the_exact_decimal_of_a_half_is_a_half() {
    assert_eq!(Float128::from_f64(0.5).to_exact_decimal(), "0.5");
    assert_eq!(Float128::from_f64(1.5).to_exact_decimal(), "1.5");
    assert_eq!(Float128::from_f64(-2.75).to_exact_decimal(), "-2.75");
    assert_eq!(Float128::ZERO.to_exact_decimal(), "0.0");
    assert_eq!(Float128::NEG_ZERO.to_exact_decimal(), "-0.0");
    assert_eq!(Float128::ONE.to_exact_decimal(), "1.0");
}

#[test]
fn parsing_agrees_with_f64_wherever_f64_has_an_opinion() {
    // Correct rounding is the property; `f64`'s own parser is the
    // oracle. A decimal that rounds to a given double must round to the
    // binary128 that widens from it, whenever the decimal has few enough
    // digits to be exactly representable in neither but rounded the same
    // way — which is what `to_f64_rounded` checks.
    let texts = [
        "0.1",
        "0.2",
        "0.3",
        "1e308",
        "1e-308",
        "2.2250738585072014e-308",
        "4.9e-324",
        "-0.0",
        "1.7976931348623157e308",
        "123456789012345678901234567890",
        "1e-400",
        "1e400",
    ];
    for text in texts {
        let wide: Float128 = text.parse().expect("a decimal");
        let narrow: f64 = text.parse().expect("the same decimal");
        assert_eq!(
            wide.to_f64_rounded().to_bits(),
            narrow.to_bits(),
            "`{text}` rounds differently in the two widths"
        );
    }
}

#[test]
fn parsing_a_random_short_decimal_agrees_with_f64() {
    let mut rng = Xorshift(0x9999_8888_7777_6666);
    for _ in 0..5_000 {
        let digits = rng.next_u64() % 1_000_000_000_000_000_000;
        let exponent = (rng.next_u64() % 60) as i32 - 30;
        let text = format!("{digits}e{exponent}");
        let wide: Float128 = text.parse().expect("a decimal");
        let narrow: f64 = text.parse().expect("the same decimal");
        assert_eq!(
            wide.to_f64_rounded().to_bits(),
            narrow.to_bits(),
            "`{text}` rounds differently in the two widths"
        );
    }
}

#[test]
fn the_non_finite_words_survive_a_round_trip() {
    for (name, value) in [
        ("inf", Float128::INFINITY),
        ("-inf", Float128::NEG_INFINITY),
        ("nan", Float128::NAN),
    ] {
        assert_eq!(value.to_shortest_decimal(), name);
        let back: Float128 = name.parse().expect("a word");
        assert_eq!(back.is_nan(), value.is_nan());
        if !value.is_nan() {
            assert_eq!(back.to_bits(), value.to_bits());
        }
    }
    assert!(
        "Infinity"
            .parse::<Float128>()
            .expect("a word")
            .is_infinite()
    );
    assert!(
        "-INF"
            .parse::<Float128>()
            .expect("a word")
            .is_sign_negative()
    );
}

#[test]
fn a_string_that_is_not_a_number_is_refused() {
    assert!("".parse::<Float128>().is_err());
    assert!("-".parse::<Float128>().is_err());
    assert!("1_5".parse::<Float128>().is_err());
    assert!("1.2.3".parse::<Float128>().is_err());
    assert!("0x1p3".parse::<Float128>().is_err());
    assert!("1e".parse::<Float128>().is_err());
    assert!("1e+".parse::<Float128>().is_err());
}

// -- the x87 dialect the field sends -------------------------------------

#[test]
fn x87_one_and_a_half_is_what_gcc_wrote() {
    // Measured: `gcc 14.2 -m64` on Debian 13, `1.5L` occupies
    // `00 00 00 00 00 00 00 c0 ff 3f` with six bytes of padding after.
    let (bytes, exact) = Float128::from_f64(1.5).to_x87_padded_bytes();
    assert!(exact);
    assert_eq!(
        bytes,
        [0, 0, 0, 0, 0, 0, 0, 0xc0, 0xff, 0x3f, 0, 0, 0, 0, 0, 0]
    );
    // And back, which is what reading a Fast DDS payload needs.
    assert_eq!(
        Float128::from_x87_padded_bytes(bytes).to_bits(),
        Float128::from_f64(1.5).to_bits()
    );
}

#[test]
fn the_padding_never_reaches_the_value() {
    // Fast DDS was measured writing leftover stack in the six bytes the
    // x87 type does not use. Two payloads that differ only there must
    // decode to the same number, or a reader disagrees with itself.
    let mut clean = Float128::from_f64(-2.75).to_x87_padded_bytes().0;
    let quiet = Float128::from_x87_padded_bytes(clean);
    clean[10..].copy_from_slice(&[0xde, 0xcc, 0x5d, 0x7c, 0x00, 0x00]);
    let noisy = Float128::from_x87_padded_bytes(clean);
    assert_eq!(quiet.to_bits(), noisy.to_bits());
}

#[test]
fn x87_carries_sixty_four_significand_bits_and_says_when_it_cannot() {
    // 1 + 2^-60 fits x87's 64-bit significand and not a double's 53.
    let value = Float128::from_bits(Float128::ONE.to_bits() | (1u128 << 52));
    let (extended, exact) = value.to_x87_extended();
    assert!(exact, "61 significand bits fit inside 64");
    assert_eq!(
        Float128::from_x87_extended(extended).to_bits(),
        value.to_bits()
    );
    assert!(value.to_f64().is_err(), "and do not fit inside 53");

    // 1 + 2^-70 does not fit 64.
    let wider = Float128::from_bits(Float128::ONE.to_bits() | (1u128 << 42));
    let (_, exact) = wider.to_x87_extended();
    assert!(!exact);
}

#[test]
fn every_x87_pattern_widens_and_narrows_back() {
    let mut rng = Xorshift(0x2468_ace0_1357_9bdf);
    for _ in 0..20_000 {
        let significand = rng.next_u64() | 0x8000_0000_0000_0000;
        let sign_exponent = (rng.next_u64() as u16) & 0xfffe;
        if sign_exponent & 0x7fff == 0x7fff || sign_exponent & 0x7fff == 0 {
            continue;
        }
        let extended = zerodds_float128::x87::Extended80 {
            sign_exponent,
            significand,
        };
        let wide = Float128::from_x87_extended(extended);
        let (back, exact) = wide.to_x87_extended();
        assert!(
            exact,
            "widening then narrowing lost something it should not have"
        );
        assert_eq!(back, extended);
    }
}

#[test]
fn the_heuristic_does_not_claim_more_than_it_knows() {
    let x87 = Float128::from_f64(1.5).to_x87_padded_bytes().0;
    assert!(zerodds_float128::x87::looks_like_x87(x87));
    // A binary128 1.5 has zeros where x87 puts its integer bit.
    assert!(!zerodds_float128::x87::looks_like_x87(
        Float128::from_f64(1.5).to_le_bytes()
    ));
    // And a payload with anything in the padding is not x87 at all.
    let mut noisy = x87;
    noisy[15] = 1;
    assert!(!zerodds_float128::x87::looks_like_x87(noisy));
}
