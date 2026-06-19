// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! WebSocket payload masking — RFC 6455 §5.3.

use core::sync::atomic::{AtomicU64, Ordering};

/// Spec §5.3 (p. 32): "octet i of the transformed data ('transformed-
/// octet-i') is the XOR of octet i of the original data ('original-
/// octet-i') with octet at index i modulo 4 of the masking key
/// ('masking-key-octet-j')".
///
/// This operation is symmetric — the same call masks **and**
/// unmasks.
pub fn apply_mask(data: &mut [u8], key: [u8; 4]) {
    for (i, b) in data.iter_mut().enumerate() {
        *b ^= key[i & 0x3];
    }
}

/// Generates a masking key. Spec §5.3 (p. 32): "The masking key is
/// a 32-bit value chosen at random by the client. [...] The masking
/// key needs to be unpredictable; thus, the masking key MUST be
/// derived from a strong source of entropy."
///
/// This library provides a default implementation based on a
/// Splitmix64 PRNG with a seed from a monotonically increasing counter +
/// `core::ptr::addr_of!` position hash. Spec conformance **requires**
/// a cryptographically strong source — applications with security
/// requirements MUST use their own key generation scheme
/// (e.g. the `getrandom` crate or an OS RNG). This function is meant
/// only as a codec convenience; it is explicitly `not for security`.
#[must_use]
pub fn generate_masking_key() -> [u8; 4] {
    static COUNTER: AtomicU64 = AtomicU64::new(0xCAFE_F00D_DEAD_BEEF);
    let n = COUNTER.fetch_add(0x9E37_79B9_7F4A_7C15, Ordering::SeqCst);
    let mut x = n;
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    let bytes = x.to_le_bytes();
    [bytes[0], bytes[1], bytes[2], bytes[3]]
}

/// Spec §5.3 — `MaskingKeyProvider` trait for caller-injected
/// secure RNGs.
///
/// Applications with security requirements MUST implement this trait
/// and wire in a cryptographically strong RNG (e.g. `getrandom`,
/// `rand::rngs::OsRng`). The default implementation
/// [`InsecureSplitmixProvider`] is `not for security`.
pub trait MaskingKeyProvider {
    /// Returns a 32-bit masking key.
    fn next_key(&mut self) -> [u8; 4];
}

/// Default provider — explicitly `not for security`.
///
/// Wraps the library-internal `generate_masking_key` function.
#[derive(Debug, Default)]
pub struct InsecureSplitmixProvider;

impl MaskingKeyProvider for InsecureSplitmixProvider {
    fn next_key(&mut self) -> [u8; 4] {
        generate_masking_key()
    }
}

/// Caller-supplied provider — wraps an `FnMut` that returns a key.
/// Lets applications inject their own sources (OsRng, getrandom,
/// hardware RNG) without writing their own trait impl.
pub struct ClosureMaskingKeyProvider<F: FnMut() -> [u8; 4]>(pub F);

impl<F: FnMut() -> [u8; 4]> MaskingKeyProvider for ClosureMaskingKeyProvider<F> {
    fn next_key(&mut self) -> [u8; 4] {
        (self.0)()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_mask_is_symmetric() {
        // Spec §5.3 — XOR is self-inverse.
        let key = [0x01, 0x02, 0x03, 0x04];
        let original: alloc::vec::Vec<u8> = (0..16).collect();
        let mut masked = original.clone();
        apply_mask(&mut masked, key);
        assert_ne!(masked, original);
        apply_mask(&mut masked, key);
        assert_eq!(masked, original);
    }

    #[test]
    fn apply_mask_xors_with_key_modulo_4() {
        // Spec §5.3 — index modulo 4.
        let key = [0xFF, 0x00, 0xFF, 0x00];
        let mut data = [0xAA; 8];
        apply_mask(&mut data, key);
        assert_eq!(data, [0x55, 0xAA, 0x55, 0xAA, 0x55, 0xAA, 0x55, 0xAA]);
    }

    #[test]
    fn apply_mask_handles_partial_key_alignment() {
        // Spec §5.3 — a payload length that is not a multiple of 4 must
        // work correctly.
        let key = [0x01, 0x02, 0x03, 0x04];
        let mut data = [0x10, 0x20, 0x30, 0x40, 0x50];
        apply_mask(&mut data, key);
        // Index 0..3 = key[0..3], Index 4 = key[0].
        assert_eq!(data[0], 0x10 ^ 0x01);
        assert_eq!(data[1], 0x20 ^ 0x02);
        assert_eq!(data[2], 0x30 ^ 0x03);
        assert_eq!(data[3], 0x40 ^ 0x04);
        assert_eq!(data[4], 0x50 ^ 0x01);
    }

    #[test]
    fn empty_payload_is_unchanged() {
        let mut data: alloc::vec::Vec<u8> = alloc::vec::Vec::new();
        apply_mask(&mut data, [1, 2, 3, 4]);
        assert!(data.is_empty());
    }

    #[test]
    fn generate_masking_key_returns_4_bytes() {
        let k = generate_masking_key();
        assert_eq!(k.len(), 4);
    }

    #[test]
    fn generate_masking_key_returns_distinct_values_across_calls() {
        // Spec §5.3 requires unpredictability; minimal sanity: the counter
        // produces distinct values.
        let k1 = generate_masking_key();
        let k2 = generate_masking_key();
        assert_ne!(k1, k2);
    }

    #[test]
    fn insecure_splitmix_provider_implements_trait() {
        let mut p = InsecureSplitmixProvider;
        let k = p.next_key();
        assert_eq!(k.len(), 4);
    }

    #[test]
    fn closure_provider_calls_user_supplied_fn() {
        let mut p = ClosureMaskingKeyProvider(|| [9, 9, 9, 9]);
        assert_eq!(p.next_key(), [9, 9, 9, 9]);
    }
}
