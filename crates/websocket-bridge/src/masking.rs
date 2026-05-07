// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! WebSocket Payload Masking — RFC 6455 §5.3.

use core::sync::atomic::{AtomicU64, Ordering};

/// Spec §5.3 (S. 32): "octet i of the transformed data ('transformed-
/// octet-i') is the XOR of octet i of the original data ('original-
/// octet-i') with octet at index i modulo 4 of the masking key
/// ('masking-key-octet-j')".
///
/// Diese Operation ist symmetrisch — derselbe Aufruf masked **und**
/// unmasked.
pub fn apply_mask(data: &mut [u8], key: [u8; 4]) {
    for (i, b) in data.iter_mut().enumerate() {
        *b ^= key[i & 0x3];
    }
}

/// Generiert einen Masking-Key. Spec §5.3 (S. 32): "The masking key is
/// a 32-bit value chosen at random by the client. [...] The masking
/// key needs to be unpredictable; thus, the masking key MUST be
/// derived from a strong source of entropy."
///
/// Diese Library liefert eine Default-Implementation auf Basis eines
/// Splitmix64-PRNG mit Seed aus einer monoton wachsenden Counter +
/// `core::ptr::addr_of!`-Position-Hash. Spec-Komformitaet **fordert**
/// eine kryptographisch starke Quelle — Anwendungen mit Security-
/// Bedarf MUESSEN ein eigenes Key-Generation-Verfahren einsetzen
/// (z.B. `getrandom`-Crate oder OS-RNG). Diese Funktion ist nur
/// als Codec-Convenience gedacht; sie ist explizit `not for security`.
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

/// Spec §5.3 — `MaskingKeyProvider`-Trait fuer caller-injected
/// secure RNGs.
///
/// Anwendungen mit Security-Bedarf MUESSEN diesen Trait implementieren
/// und einen kryptographisch starken RNG (z.B. `getrandom`,
/// `rand::rngs::OsRng`) anbinden. Die Default-Implementation
/// [`InsecureSplitmixProvider`] ist `not for security`.
pub trait MaskingKeyProvider {
    /// Liefert einen 32-bit Masking-Key.
    fn next_key(&mut self) -> [u8; 4];
}

/// Default-Provider — explicit `not for security`.
///
/// Wraps die Library-internal `generate_masking_key`-Funktion.
#[derive(Debug, Default)]
pub struct InsecureSplitmixProvider;

impl MaskingKeyProvider for InsecureSplitmixProvider {
    fn next_key(&mut self) -> [u8; 4] {
        generate_masking_key()
    }
}

/// Caller-supplied Provider — wraps eine `FnMut` die einen Key
/// liefert. Erlaubt Anwendungen, eigene Sources (OsRng, getrandom,
/// Hardware-RNG) ohne eigene Trait-Impl zu injizieren.
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
        // Spec §5.3 — XOR ist self-inverse.
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
        // Spec §5.3 — payload-len kein Vielfaches von 4 muss korrekt
        // funktionieren.
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
        // Spec §5.3 verlangt unpredictable; minimal Sanity: Counter
        // produziert unterschiedliche Werte.
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
