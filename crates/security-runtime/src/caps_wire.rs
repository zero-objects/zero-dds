// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! SPDP mapping for `PeerCapabilities`.
//!
//! Bridge between the policy layer ([`PeerCapabilities`]) and the
//! SPDP wire format ([`WirePropertyList`]). On send, the
//! own caps are written into the SPDP properties; on receive
//! the properties are parsed back to `PeerCapabilities`.
//!
//! # Property keys
//!
//! | Key                               | Semantics                                          |
//! |-----------------------------------|----------------------------------------------------|
//! | `dds.sec.auth.plugin_class`       | OMG standard, authentication plugin class          |
//! | `dds.sec.access.plugin_class`     | OMG standard, access-control plugin class          |
//! | `dds.sec.crypto.plugin_class`     | OMG standard, crypto plugin class                  |
//! | `zerodds.sec.supported_suites`    | CSV: `AES_128_GCM,AES_256_GCM,HMAC_SHA256`         |
//! | `zerodds.sec.offered_protection`  | `NONE` / `SIGN` / `ENCRYPT`                        |
//! | `zerodds.sec.vendor_hint`         | free string, e.g. `"zerodds"`                      |
//!
//! The `zerodds.sec.*` namespace is intentionally ZeroDDS-specific. Other
//! vendors (Cyclone, Fast-DDS) silently ignore unknown properties
//! — this preserves SPDP interop (see architecture
//! doc §8.1).
//!
//! `has_valid_cert` and `validity_window` are **not** transferred via SPDP;
//! these are post-handshake values set by the authentication
//! plugin.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use zerodds_rtps::property_list::{WireProperty, WirePropertyList};
use zerodds_security_pki::DelegationChain;

use crate::caps::PeerCapabilities;
use crate::policy::{ProtectionLevel, SuiteHint};

// ============================================================================
// Property keys (constants for test stability)
// ============================================================================

/// OMG standard: authentication plugin class.
pub const KEY_AUTH_PLUGIN: &str = "dds.sec.auth.plugin_class";
/// OMG standard: access-control plugin class.
pub const KEY_ACCESS_PLUGIN: &str = "dds.sec.access.plugin_class";
/// OMG standard: crypto plugin class.
pub const KEY_CRYPTO_PLUGIN: &str = "dds.sec.crypto.plugin_class";
/// ZeroDDS extension: CSV of accepted suites.
pub const KEY_SUPPORTED_SUITES: &str = "zerodds.sec.supported_suites";
/// ZeroDDS extension: offered protection level.
pub const KEY_OFFERED_PROTECTION: &str = "zerodds.sec.offered_protection";
/// ZeroDDS extension: vendor identification for quirks.
pub const KEY_VENDOR_HINT: &str = "zerodds.sec.vendor_hint";
/// ZeroDDS extension: delegation chain (base64-encoded wire bytes,
/// RC1). Format: base64 over the `DelegationChain::encode()`
/// output. Passes through SPDP via PID_PROPERTY_LIST.
pub const KEY_DELEGATION_CHAIN: &str = "zerodds.sec.delegation_chain";

/// DoS cap for the wire blob (before base64 decode). Architecture §11.
pub const MAX_DELEGATION_CHAIN_BYTES: usize = 8 * 1024;

// ============================================================================
// Suite <-> CSV
// ============================================================================

fn suite_to_str(s: SuiteHint) -> &'static str {
    match s {
        SuiteHint::Aes128Gcm => "AES_128_GCM",
        SuiteHint::Aes256Gcm => "AES_256_GCM",
        SuiteHint::HmacSha256 => "HMAC_SHA256",
    }
}

fn suite_from_str(s: &str) -> Option<SuiteHint> {
    match s.trim() {
        "AES_128_GCM" => Some(SuiteHint::Aes128Gcm),
        "AES_256_GCM" => Some(SuiteHint::Aes256Gcm),
        "HMAC_SHA256" => Some(SuiteHint::HmacSha256),
        _ => None,
    }
}

fn suites_to_csv(suites: &[SuiteHint]) -> String {
    let mut out = String::new();
    for (i, s) in suites.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(suite_to_str(*s));
    }
    out
}

fn suites_from_csv(csv: &str) -> Vec<SuiteHint> {
    csv.split(',').filter_map(suite_from_str).collect()
}

// ============================================================================
// ProtectionLevel <-> String
// ============================================================================

fn protection_to_str(p: ProtectionLevel) -> &'static str {
    match p {
        ProtectionLevel::None => "NONE",
        ProtectionLevel::Sign => "SIGN",
        ProtectionLevel::Encrypt => "ENCRYPT",
    }
}

fn protection_from_str(s: &str) -> Option<ProtectionLevel> {
    match s.trim() {
        "NONE" => Some(ProtectionLevel::None),
        "SIGN" => Some(ProtectionLevel::Sign),
        "ENCRYPT" => Some(ProtectionLevel::Encrypt),
        _ => None,
    }
}

// ============================================================================
// Public API
// ============================================================================

/// Writes the security caps into the given PropertyList.
///
/// The function merges into the existing list: own keys are
/// replaced, foreign properties stay untouched. This enables
/// integration patterns where other subsystems have added further
/// properties.
pub fn advertise_security_caps(list: &mut WirePropertyList, caps: &PeerCapabilities) {
    set_or_remove(list, KEY_AUTH_PLUGIN, caps.auth_plugin_class.as_deref());
    set_or_remove(list, KEY_ACCESS_PLUGIN, caps.access_plugin_class.as_deref());
    set_or_remove(list, KEY_CRYPTO_PLUGIN, caps.crypto_plugin_class.as_deref());
    if !caps.supported_suites.is_empty() {
        set_value(
            list,
            KEY_SUPPORTED_SUITES,
            &suites_to_csv(&caps.supported_suites),
        );
    } else {
        remove_by_key(list, KEY_SUPPORTED_SUITES);
    }
    set_value(
        list,
        KEY_OFFERED_PROTECTION,
        protection_to_str(caps.offered_protection),
    );
    set_or_remove(list, KEY_VENDOR_HINT, caps.vendor_hint.as_deref());
    // Delegation chain: encode() → base64 → property.
    if let Some(chain) = &caps.delegation_chain {
        let raw = chain.encode();
        if raw.len() <= MAX_DELEGATION_CHAIN_BYTES {
            let b64 = base64_encode(&raw);
            set_value(list, KEY_DELEGATION_CHAIN, &b64);
        } else {
            // Blob over cap → drop, otherwise we risk a chain
            // that nobody accepts anymore.
            remove_by_key(list, KEY_DELEGATION_CHAIN);
        }
    } else {
        remove_by_key(list, KEY_DELEGATION_CHAIN);
    }
}

/// Reads security caps from a PropertyList. Unknown or
/// malformed values are silently treated as "empty" — a
/// peer must not be able to throw us out of the SPDP process via a
/// broken property.
#[must_use]
pub fn parse_peer_caps(list: &WirePropertyList) -> PeerCapabilities {
    let offered_protection = list
        .get(KEY_OFFERED_PROTECTION)
        .and_then(protection_from_str)
        .unwrap_or(ProtectionLevel::None);
    let supported_suites = list
        .get(KEY_SUPPORTED_SUITES)
        .map(suites_from_csv)
        .unwrap_or_default();
    let delegation_chain = list
        .get(KEY_DELEGATION_CHAIN)
        .and_then(|s| {
            // DoS cap: base64 input no larger than 4/3 * raw_max.
            if s.len() > MAX_DELEGATION_CHAIN_BYTES * 4 / 3 + 4 {
                return None;
            }
            base64_decode(s).ok()
        })
        .filter(|raw| raw.len() <= MAX_DELEGATION_CHAIN_BYTES)
        .and_then(|raw| DelegationChain::decode(&raw).ok());
    PeerCapabilities {
        auth_plugin_class: list.get(KEY_AUTH_PLUGIN).map(str::to_string),
        access_plugin_class: list.get(KEY_ACCESS_PLUGIN).map(str::to_string),
        crypto_plugin_class: list.get(KEY_CRYPTO_PLUGIN).map(str::to_string),
        supported_suites,
        offered_protection,
        has_valid_cert: false,
        validity_window: None,
        vendor_hint: list.get(KEY_VENDOR_HINT).map(str::to_string),
        // cert_cn is set by the auth plugin after the handshake, not
        // propagated via SPDP (security decision: the CN should
        // not land unsigned on the wire).
        cert_cn: None,
        delegation_chain,
    }
}

// ============================================================================
// Base64 (RFC 4648 standard alphabet, no padding-strip)
// ============================================================================
//
// We avoid an external base64 crate, because that would be a significant
// supply-chain extension for ~30 lines of code. The implementation
// is conservative and handles malformed input with `Err`.

const B64_ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(input: &[u8]) -> String {
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    let mut chunks = input.chunks_exact(3);
    for chunk in &mut chunks {
        let n = (u32::from(chunk[0]) << 16) | (u32::from(chunk[1]) << 8) | u32::from(chunk[2]);
        out.push(B64_ALPHABET[((n >> 18) & 0x3F) as usize] as char);
        out.push(B64_ALPHABET[((n >> 12) & 0x3F) as usize] as char);
        out.push(B64_ALPHABET[((n >> 6) & 0x3F) as usize] as char);
        out.push(B64_ALPHABET[(n & 0x3F) as usize] as char);
    }
    let rem = chunks.remainder();
    match rem.len() {
        1 => {
            let n = u32::from(rem[0]) << 16;
            out.push(B64_ALPHABET[((n >> 18) & 0x3F) as usize] as char);
            out.push(B64_ALPHABET[((n >> 12) & 0x3F) as usize] as char);
            out.push('=');
            out.push('=');
        }
        2 => {
            let n = (u32::from(rem[0]) << 16) | (u32::from(rem[1]) << 8);
            out.push(B64_ALPHABET[((n >> 18) & 0x3F) as usize] as char);
            out.push(B64_ALPHABET[((n >> 12) & 0x3F) as usize] as char);
            out.push(B64_ALPHABET[((n >> 6) & 0x3F) as usize] as char);
            out.push('=');
        }
        _ => {}
    }
    out
}

fn base64_char_to_val(c: u8) -> Option<u8> {
    match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

fn base64_decode(input: &str) -> Result<Vec<u8>, ()> {
    let bytes = input.as_bytes();
    if bytes.len() % 4 != 0 {
        return Err(());
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for chunk in bytes.chunks_exact(4) {
        let mut vals = [0u8; 4];
        let mut pad = 0usize;
        for (i, &c) in chunk.iter().enumerate() {
            if c == b'=' {
                pad += 1;
                vals[i] = 0;
            } else if pad > 0 {
                return Err(());
            } else {
                vals[i] = base64_char_to_val(c).ok_or(())?;
            }
        }
        let n = (u32::from(vals[0]) << 18)
            | (u32::from(vals[1]) << 12)
            | (u32::from(vals[2]) << 6)
            | u32::from(vals[3]);
        out.push(((n >> 16) & 0xFF) as u8);
        if pad < 2 {
            out.push(((n >> 8) & 0xFF) as u8);
        }
        if pad < 1 {
            out.push((n & 0xFF) as u8);
        }
    }
    Ok(out)
}

// ============================================================================
// internals
// ============================================================================

/// Sets the key to `value` (overwrites an existing entry).
fn set_value(list: &mut WirePropertyList, key: &str, value: &str) {
    remove_by_key(list, key);
    list.push(WireProperty::new(key.to_string(), value.to_string()));
}

/// Sets the key to `value` if `Some`, otherwise the key is removed.
fn set_or_remove(list: &mut WirePropertyList, key: &str, value: Option<&str>) {
    match value {
        Some(v) => set_value(list, key, v),
        None => remove_by_key(list, key),
    }
}

fn remove_by_key(list: &mut WirePropertyList, key: &str) {
    list.entries.retain(|e| e.name != key);
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::caps::Validity;

    fn secure_caps() -> PeerCapabilities {
        PeerCapabilities {
            auth_plugin_class: Some("DDS:Auth:PKI-DH:1.2".to_string()),
            access_plugin_class: Some("DDS:Access:Permissions:1.2".to_string()),
            crypto_plugin_class: Some("DDS:Crypto:AES-GCM-GMAC:1.2".to_string()),
            supported_suites: alloc::vec![SuiteHint::Aes128Gcm, SuiteHint::Aes256Gcm],
            offered_protection: ProtectionLevel::Encrypt,
            has_valid_cert: true, // NOT propagated
            validity_window: Some(Validity {
                not_before: 0,
                not_after: 100,
            }), // NOT propagated
            vendor_hint: Some("zerodds".to_string()),
            cert_cn: None, // NOT propagated
            delegation_chain: None,
        }
    }

    // ---- Suite-CSV ----

    #[test]
    fn suite_csv_roundtrip() {
        let suites = alloc::vec![
            SuiteHint::Aes128Gcm,
            SuiteHint::Aes256Gcm,
            SuiteHint::HmacSha256,
        ];
        let csv = suites_to_csv(&suites);
        assert_eq!(csv, "AES_128_GCM,AES_256_GCM,HMAC_SHA256");
        assert_eq!(suites_from_csv(&csv), suites);
    }

    #[test]
    fn suite_csv_empty() {
        assert_eq!(suites_to_csv(&[]), "");
        assert_eq!(suites_from_csv(""), Vec::<SuiteHint>::new());
    }

    #[test]
    fn suite_csv_ignores_unknown_tokens() {
        let parsed = suites_from_csv("AES_128_GCM,FUTURE_SUITE,HMAC_SHA256");
        assert_eq!(
            parsed,
            alloc::vec![SuiteHint::Aes128Gcm, SuiteHint::HmacSha256]
        );
    }

    #[test]
    fn suite_csv_trims_whitespace() {
        let parsed = suites_from_csv(" AES_128_GCM , AES_256_GCM ");
        assert_eq!(
            parsed,
            alloc::vec![SuiteHint::Aes128Gcm, SuiteHint::Aes256Gcm]
        );
    }

    // ---- ProtectionLevel ----

    #[test]
    fn protection_string_roundtrip_all_levels() {
        for lvl in [
            ProtectionLevel::None,
            ProtectionLevel::Sign,
            ProtectionLevel::Encrypt,
        ] {
            assert_eq!(protection_from_str(protection_to_str(lvl)), Some(lvl));
        }
    }

    #[test]
    fn protection_from_str_unknown_is_none() {
        assert!(protection_from_str("WEIRD").is_none());
    }

    // ---- advertise + parse (Roundtrip) ----

    #[test]
    fn roundtrip_preserves_wire_fields() {
        let caps = secure_caps();
        let mut list = WirePropertyList::new();
        advertise_security_caps(&mut list, &caps);
        let parsed = parse_peer_caps(&list);

        assert_eq!(parsed.auth_plugin_class, caps.auth_plugin_class);
        assert_eq!(parsed.access_plugin_class, caps.access_plugin_class);
        assert_eq!(parsed.crypto_plugin_class, caps.crypto_plugin_class);
        assert_eq!(parsed.supported_suites, caps.supported_suites);
        assert_eq!(parsed.offered_protection, caps.offered_protection);
        assert_eq!(parsed.vendor_hint, caps.vendor_hint);
    }

    #[test]
    fn roundtrip_drops_non_wire_fields() {
        // has_valid_cert + validity_window do NOT land in SPDP —
        // these are post-handshake values.
        let caps = secure_caps();
        let mut list = WirePropertyList::new();
        advertise_security_caps(&mut list, &caps);
        let parsed = parse_peer_caps(&list);

        assert!(!parsed.has_valid_cert);
        assert!(parsed.validity_window.is_none());
    }

    #[test]
    fn legacy_peer_without_security_properties_parses_as_empty() {
        let list = WirePropertyList::new();
        let parsed = parse_peer_caps(&list);

        assert!(parsed.auth_plugin_class.is_none());
        assert!(parsed.crypto_plugin_class.is_none());
        assert!(parsed.access_plugin_class.is_none());
        assert!(parsed.supported_suites.is_empty());
        assert_eq!(parsed.offered_protection, ProtectionLevel::None);
        assert!(parsed.vendor_hint.is_none());
    }

    #[test]
    fn advertise_overwrites_existing_keys() {
        let mut list = WirePropertyList::new();
        list.push(WireProperty::new(KEY_OFFERED_PROTECTION, "SIGN"));
        list.push(WireProperty::new(KEY_AUTH_PLUGIN, "stale-value"));

        advertise_security_caps(
            &mut list,
            &PeerCapabilities {
                auth_plugin_class: Some("DDS:Auth:PKI-DH:1.2".to_string()),
                offered_protection: ProtectionLevel::Encrypt,
                ..Default::default()
            },
        );
        assert_eq!(list.get(KEY_OFFERED_PROTECTION), Some("ENCRYPT"));
        assert_eq!(list.get(KEY_AUTH_PLUGIN), Some("DDS:Auth:PKI-DH:1.2"));
    }

    #[test]
    fn advertise_keeps_foreign_properties_intact() {
        let mut list = WirePropertyList::new();
        list.push(WireProperty::new("foreign.key", "keep-me"));
        advertise_security_caps(&mut list, &secure_caps());
        assert_eq!(list.get("foreign.key"), Some("keep-me"));
    }

    #[test]
    fn advertise_removes_keys_when_caps_field_is_none() {
        // If a cap field was set to None (e.g. the peer removed
        // its auth plugin), old properties must disappear.
        let mut list = WirePropertyList::new();
        list.push(WireProperty::new(KEY_AUTH_PLUGIN, "DDS:Auth:PKI-DH:1.2"));
        advertise_security_caps(
            &mut list,
            &PeerCapabilities {
                auth_plugin_class: None,
                ..Default::default()
            },
        );
        assert!(list.get(KEY_AUTH_PLUGIN).is_none());
    }

    #[test]
    fn advertise_is_idempotent() {
        let caps = secure_caps();
        let mut list1 = WirePropertyList::new();
        let mut list2 = WirePropertyList::new();
        advertise_security_caps(&mut list1, &caps);
        advertise_security_caps(&mut list2, &caps);
        advertise_security_caps(&mut list2, &caps);
        assert_eq!(list1, list2);
    }

    #[test]
    fn parse_malformed_protection_falls_back_to_none() {
        let list =
            WirePropertyList::new().with(WireProperty::new(KEY_OFFERED_PROTECTION, "MAXIMAL"));
        let parsed = parse_peer_caps(&list);
        assert_eq!(parsed.offered_protection, ProtectionLevel::None);
    }

    #[test]
    fn parse_malformed_suite_csv_drops_invalid_tokens() {
        let list = WirePropertyList::new()
            .with(WireProperty::new(KEY_SUPPORTED_SUITES, "AES_128_GCM,BOGUS"));
        let parsed = parse_peer_caps(&list);
        assert_eq!(parsed.supported_suites, alloc::vec![SuiteHint::Aes128Gcm]);
    }

    #[test]
    fn advertise_with_no_suites_omits_suites_key() {
        let caps = PeerCapabilities {
            offered_protection: ProtectionLevel::Sign,
            ..Default::default()
        };
        let mut list = WirePropertyList::new();
        advertise_security_caps(&mut list, &caps);
        assert!(list.get(KEY_SUPPORTED_SUITES).is_none());
    }

    // ---- vendor interop smoke ----

    #[test]
    fn unknown_foreign_properties_dont_affect_parse() {
        let list = WirePropertyList::new()
            .with(WireProperty::new("com.rti.dds.Priority", "9"))
            .with(WireProperty::new("org.eprosima.fastdds.type", "X"))
            .with(WireProperty::new(KEY_OFFERED_PROTECTION, "SIGN"));
        let parsed = parse_peer_caps(&list);
        assert_eq!(parsed.offered_protection, ProtectionLevel::Sign);
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod base64_and_delegation_tests {
    use super::*;

    #[test]
    fn base64_encode_known_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn base64_decode_known_vectors() {
        assert_eq!(base64_decode("").unwrap(), b"");
        assert_eq!(base64_decode("Zg==").unwrap(), b"f");
        assert_eq!(base64_decode("Zm8=").unwrap(), b"fo");
        assert_eq!(base64_decode("Zm9v").unwrap(), b"foo");
        assert_eq!(base64_decode("Zm9vYg==").unwrap(), b"foob");
        assert_eq!(base64_decode("Zm9vYmE=").unwrap(), b"fooba");
        assert_eq!(base64_decode("Zm9vYmFy").unwrap(), b"foobar");
    }

    #[test]
    fn base64_decode_rejects_bad_length() {
        assert!(base64_decode("ABC").is_err()); // not %4
        assert!(base64_decode("A").is_err());
    }

    #[test]
    fn base64_decode_rejects_bad_chars() {
        assert!(base64_decode("AB!?").is_err());
        assert!(base64_decode("@@@@").is_err());
    }

    #[test]
    fn base64_roundtrip_random_bytes() {
        let blob: alloc::vec::Vec<u8> = (0..255u8).collect();
        let encoded = base64_encode(&blob);
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(decoded, blob);
    }

    #[test]
    fn parse_skips_oversize_base64_property() {
        let mut list = WirePropertyList::new();
        // Base64 input larger than (8KiB * 4/3) + 4 → must be parsed as None.
        let huge = "A".repeat(MAX_DELEGATION_CHAIN_BYTES * 4 / 3 + 100);
        list.push(WireProperty::new(KEY_DELEGATION_CHAIN, huge.as_str()));
        let parsed = parse_peer_caps(&list);
        assert!(parsed.delegation_chain.is_none());
    }
}
