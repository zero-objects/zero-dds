// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The decimal text of a `fixed<P,S>` value, shared by the default emitters
//! of every language column.

/// The one spelling of zero a `fixed<P,S>` has: `0`, or `0.` followed by
/// `scale` zeros.
pub(crate) fn zero_decimal(scale: u8) -> String {
    if scale == 0 {
        return String::from("0");
    }
    format!("0.{}", "0".repeat(usize::from(scale)))
}

/// A fixed-point literal's digits and scale back into a decimal.
pub(crate) fn fixed_text(digits: &str, scale: u8) -> String {
    let (negative, magnitude) = match digits.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, digits),
    };
    let scale = usize::from(scale);
    let padded = if magnitude.len() <= scale {
        format!("{}{magnitude}", "0".repeat(scale + 1 - magnitude.len()))
    } else {
        magnitude.to_string()
    };
    let split = padded.len() - scale;
    let mut out = String::new();
    if negative {
        out.push('-');
    }
    out.push_str(&padded[..split]);
    if scale > 0 {
        out.push('.');
        out.push_str(&padded[split..]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_digits_take_their_point_and_their_sign() {
        assert_eq!(zero_decimal(0), "0");
        assert_eq!(zero_decimal(3), "0.000");
        assert_eq!(fixed_text("12345", 2), "123.45");
        assert_eq!(fixed_text("-5", 3), "-0.005");
        assert_eq!(fixed_text("7", 0), "7");
    }
}
