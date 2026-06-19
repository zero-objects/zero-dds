// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! RPC-specific hashing algorithm (Spec §7.5.1.1.2).
//!
//! "The hashing algorithm uses MD5 of the operation/parameter name;
//! first 4 bytes interpreted as little-endian uint32."
//!
//! We delegate to the existing XTypes hash (`crates/types/src/
//! hash.rs`) and provide an RPC-specific API + tests that pin down the
//! spec mapping.

extern crate alloc;

/// Spec §7.5.1.1.2: MD5(name)[0..3] LE -> u32.
///
/// Used to map operation names and parameter names to 4-byte
/// IDs (wire efficiency).
#[must_use]
pub fn rpc_member_hash(name: &str) -> u32 {
    let digest = md5_first4(name.as_bytes());
    u32::from_le_bytes(digest)
}

/// MD5-first-4-bytes helper. We use the XTypes hash path
/// (`hash_bytes` returns the first 14 bytes of the MD5; we take
/// the first 4 of them — this is consistent with Spec §7.5.1.1.2 +
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
        // Known MD5("operation") = 5b94f0a4ddc6612e0e7e15c1f2f4c4a4...
        // We test only the stability — the value is determined by MD5
        // and is wire-relevant.
        let h1 = rpc_member_hash("getName");
        let h2 = rpc_member_hash("getName");
        assert_eq!(h1, h2, "hash must be deterministic");
    }

    #[test]
    fn rpc_hash_distinct_for_different_names() {
        let h1 = rpc_member_hash("getName");
        let h2 = rpc_member_hash("setName");
        // Different with overwhelming probability.
        assert_ne!(h1, h2);
    }

    #[test]
    fn rpc_hash_empty_string_is_md5_empty_first_4_bytes() {
        // MD5("") = d41d8cd98f00b204e9800998ecf8427e
        // First 4 bytes = d4 1d 8c d9; LE u32 = 0xd98c1dd4
        let h = rpc_member_hash("");
        assert_eq!(h, 0xd98c_1dd4);
    }

    #[test]
    fn rpc_hash_uses_first_4_bytes_not_last() {
        // Spec property: the first 4 bytes of the digest, not
        // random other ones. We verify via MD5("a") =
        // 0cc175b9c0f1b6a831c399e269772661 -> first 4 bytes
        // 0x0c 0xc1 0x75 0xb9 -> LE u32 = 0xb975c10c.
        let h = rpc_member_hash("a");
        assert_eq!(h, 0xb975_c10c);
    }
}
