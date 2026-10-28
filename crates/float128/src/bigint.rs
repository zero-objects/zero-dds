// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Just enough arbitrary-precision integer for exact decimal.
//!
//! A binary128 value is `m * 2^e` with `e` down to -16494, so its exact
//! decimal expansion runs to about 11 500 digits. Nothing shorter than a
//! big integer produces those digits, and nothing shorter than those
//! digits produces a *correctly rounded* thirty-six-digit form — a
//! printer that rounds from a `f64` approximation gets the last digit
//! wrong for values it cannot represent, which is every value this crate
//! exists for.
//!
//! Deliberately minimal: multiply by a small factor, add a small term,
//! divide by a small divisor, shift, and compare. Those five are what
//! exact base conversion needs in both directions. No general
//! multiplication, no division by a big integer — `decimal.rs` is
//! written to need neither.

use alloc::vec;
use alloc::vec::Vec;
use core::cmp::Ordering;

/// A non-negative integer, little-endian in 32-bit limbs.
///
/// Thirty-two-bit limbs rather than sixty-four so that a limb-by-small
/// multiply fits a `u64` and a limb-by-small divide fits it too, with no
/// 128-bit intermediate and no overflow case to get wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BigUint {
    /// Least significant limb first. Never holds trailing zero limbs, so
    /// `is_zero` is `limbs.is_empty()` and equality is structural.
    limbs: Vec<u32>,
}

impl BigUint {
    /// Zero.
    pub(crate) fn zero() -> Self {
        Self { limbs: Vec::new() }
    }

    /// From a `u128`.
    pub(crate) fn from_u128(mut value: u128) -> Self {
        let mut limbs = Vec::new();
        while value != 0 {
            limbs.push((value & 0xffff_ffff) as u32);
            value >>= 32;
        }
        Self { limbs }
    }

    /// Whether this is zero.
    pub(crate) fn is_zero(&self) -> bool {
        self.limbs.is_empty()
    }

    /// How many bits the value occupies; zero for zero.
    pub(crate) fn bit_length(&self) -> usize {
        match self.limbs.last() {
            None => 0,
            Some(&top) => (self.limbs.len() - 1) * 32 + (32 - top.leading_zeros() as usize),
        }
    }

    /// The low 128 bits, and whether anything above them was dropped.
    pub(crate) fn to_u128(&self) -> (u128, bool) {
        let mut value: u128 = 0;
        for (index, &limb) in self.limbs.iter().take(4).enumerate() {
            value |= u128::from(limb) << (32 * index);
        }
        (value, self.limbs.len() > 4)
    }

    /// Drop trailing zero limbs so the representation stays canonical.
    fn trim(&mut self) {
        while self.limbs.last() == Some(&0) {
            self.limbs.pop();
        }
    }

    /// `self = self * factor + addend`, both small.
    pub(crate) fn mul_add_small(&mut self, factor: u32, addend: u32) {
        let mut carry = u64::from(addend);
        for limb in &mut self.limbs {
            let product = u64::from(*limb) * u64::from(factor) + carry;
            *limb = (product & 0xffff_ffff) as u32;
            carry = product >> 32;
        }
        while carry != 0 {
            self.limbs.push((carry & 0xffff_ffff) as u32);
            carry >>= 32;
        }
        self.trim();
    }

    /// `self /= divisor`, returning the remainder.
    pub(crate) fn div_rem_small(&mut self, divisor: u32) -> u32 {
        debug_assert!(
            divisor != 0,
            "a zero divisor is the caller's bug, not a value"
        );
        let mut remainder: u64 = 0;
        for limb in self.limbs.iter_mut().rev() {
            let current = (remainder << 32) | u64::from(*limb);
            *limb = (current / u64::from(divisor)) as u32;
            remainder = current % u64::from(divisor);
        }
        self.trim();
        remainder as u32
    }

    /// `self <<= bits`.
    pub(crate) fn shl(&mut self, bits: usize) {
        if self.is_zero() || bits == 0 {
            return;
        }
        let whole = bits / 32;
        let part = (bits % 32) as u32;
        if part != 0 {
            let mut carry = 0u32;
            for limb in &mut self.limbs {
                let shifted = (u64::from(*limb) << part) | u64::from(carry);
                *limb = (shifted & 0xffff_ffff) as u32;
                carry = (shifted >> 32) as u32;
            }
            if carry != 0 {
                self.limbs.push(carry);
            }
        }
        if whole != 0 {
            let mut limbs = vec![0u32; whole];
            limbs.extend_from_slice(&self.limbs);
            self.limbs = limbs;
        }
    }

    /// `self >>= bits`, discarding what falls off and saying whether
    /// anything did.
    pub(crate) fn shr(&mut self, bits: usize) -> bool {
        if self.is_zero() || bits == 0 {
            return false;
        }
        let whole = bits / 32;
        let part = (bits % 32) as u32;
        let mut lost = false;
        if whole >= self.limbs.len() {
            lost = !self.is_zero();
            self.limbs.clear();
            return lost;
        }
        for limb in self.limbs.iter().take(whole) {
            if *limb != 0 {
                lost = true;
            }
        }
        self.limbs.drain(..whole);
        if part != 0 {
            let mask = (1u32 << part) - 1;
            if self.limbs.first().is_some_and(|first| first & mask != 0) {
                lost = true;
            }
            let mut carry = 0u32;
            for limb in self.limbs.iter_mut().rev() {
                let value = *limb;
                *limb = (value >> part) | (carry << (32 - part));
                carry = value & mask;
            }
        }
        self.trim();
        lost
    }

    /// Multiply by `5^power`, which is how a binary fraction becomes an
    /// exact decimal one: `m / 2^k == m * 5^k / 10^k`.
    pub(crate) fn mul_pow5(&mut self, power: usize) {
        // 5^13 is the largest power of five below 2^32.
        const CHUNK: u32 = 1_220_703_125;
        const CHUNK_POWER: usize = 13;
        let mut left = power;
        while left >= CHUNK_POWER {
            self.mul_add_small(CHUNK, 0);
            left -= CHUNK_POWER;
        }
        if left != 0 {
            let mut factor = 1u32;
            for _ in 0..left {
                factor *= 5;
            }
            self.mul_add_small(factor, 0);
        }
    }

    /// Multiply by `10^power`.
    pub(crate) fn mul_pow10(&mut self, power: usize) {
        self.mul_pow5(power);
        self.shl(power);
    }

    /// The decimal digits, most significant first. Empty for zero.
    pub(crate) fn to_decimal_digits(&self) -> Vec<u8> {
        if self.is_zero() {
            return Vec::new();
        }
        // 10^9 is the largest power of ten below 2^32, so each division
        // yields nine digits and the loop runs a ninth as often.
        const CHUNK: u32 = 1_000_000_000;
        let mut work = self.clone();
        let mut groups = Vec::new();
        while !work.is_zero() {
            groups.push(work.div_rem_small(CHUNK));
        }
        let mut digits = Vec::new();
        if let Some(&top) = groups.last() {
            let mut buffer = [0u8; 9];
            let mut length = 0;
            let mut value = top;
            while value != 0 {
                buffer[length] = (value % 10) as u8;
                value /= 10;
                length += 1;
            }
            for index in (0..length).rev() {
                digits.push(buffer[index]);
            }
        }
        for &group in groups.iter().rev().skip(1) {
            let mut value = group;
            let mut buffer = [0u8; 9];
            for slot in buffer.iter_mut().rev() {
                *slot = (value % 10) as u8;
                value /= 10;
            }
            digits.extend_from_slice(&buffer);
        }
        digits
    }
}

impl Ord for BigUint {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.limbs.len().cmp(&other.limbs.len()) {
            Ordering::Equal => {}
            other_order => return other_order,
        }
        for (left, right) in self.limbs.iter().rev().zip(other.limbs.iter().rev()) {
            match left.cmp(right) {
                Ordering::Equal => {}
                other_order => return other_order,
            }
        }
        Ordering::Equal
    }
}

impl PartialOrd for BigUint {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
