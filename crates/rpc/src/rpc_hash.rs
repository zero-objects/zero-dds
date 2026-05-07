// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! RPC-spezifischer Hashing-Algorithm (Spec §7.5.1.1.2).
//!
//! "The hashing algorithm uses MD5 of the operation/parameter name;
//! first 4 bytes interpreted as little-endian uint32."
//!
//! Wir delegieren an den existing XTypes-Hash (`crates/types/src/
//! hash.rs`) und liefern eine RPC-spezifische API + Tests, die das
//! Spec-Mapping festschreiben.

extern crate alloc;

/// Spec §7.5.1.1.2: MD5(name)[0..3] LE -> u32.
///
/// Wird verwendet, um Operation-Names und Parameter-Namen auf 4-Byte
/// IDs zu mappen (Wire-Effizienz).
#[must_use]
pub fn rpc_member_hash(name: &str) -> u32 {
    let digest = md5_first4(name.as_bytes());
    u32::from_le_bytes(digest)
}

/// MD5-Erste-4-Bytes-Helper. Wir nutzen den XTypes-Hash-Pfad
/// (`hash_bytes` liefert die ersten 14 Bytes des MD5; wir nehmen
/// die ersten 4 davon — das ist konsistent mit Spec §7.5.1.1.2 +
/// XTypes 1.3 §7.3.1.2.1).
fn md5_first4(bytes: &[u8]) -> [u8; 4] {
    let full = zerodds_types::hash::hash_bytes(bytes);
    [full.0[0], full.0[1], full.0[2], full.0[3]]
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn rpc_hash_is_md5_first_4_bytes_le_u32() {
        // Bekannter MD5("operation") = 5b94f0a4ddc6612e0e7e15c1f2f4c4a4...
        // Wir testen nur die Stabilitaet — der Wert wird vom MD5
        // determiniert und ist Wire-relevant.
        let h1 = rpc_member_hash("getName");
        let h2 = rpc_member_hash("getName");
        assert_eq!(h1, h2, "Hash muss deterministisch sein");
    }

    #[test]
    fn rpc_hash_distinct_for_different_names() {
        let h1 = rpc_member_hash("getName");
        let h2 = rpc_member_hash("setName");
        // Mit ueberwaeltigender Wahrscheinlichkeit unterschiedlich.
        assert_ne!(h1, h2);
    }

    #[test]
    fn rpc_hash_empty_string_is_md5_empty_first_4_bytes() {
        // MD5("") = d41d8cd98f00b204e9800998ecf8427e
        // Erste 4 bytes = d4 1d 8c d9; LE u32 = 0xd98c1dd4
        let h = rpc_member_hash("");
        assert_eq!(h, 0xd98c_1dd4);
    }

    #[test]
    fn rpc_hash_uses_first_4_bytes_not_last() {
        // Spec-Eigenschaft: erste 4 Bytes des Digest, nicht
        // zufaellige andere. Wir verifizieren via MD5("a") =
        // 0cc175b9c0f1b6a831c399e269772661 -> erste 4 byte
        // 0x0c 0xc1 0x75 0xb9 -> LE u32 = 0xb975c10c.
        let h = rpc_member_hash("a");
        assert_eq!(h, 0xb975_c10c);
    }
}
