// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The descriptors of the AMQP 1.0 specification — Spec `amqp-1.0-types`
//! §1.3.3 and §1.5.
//!
//! Every described type of the specification has two descriptors, a
//! symbolic one (`<domain>:<name>`, domain `amqp`) and a numeric one
//! (`(domain-id << 32) | descriptor-id`, domain-id 0). A peer may send
//! either. This module is the table between the two, read off the
//! `<descriptor name=… code=…/>` lines of Part 2 (`amqp-1.0-transport`
//! §2.7, §2.8.14), Part 3 (`amqp-1.0-messaging` §3.2, §3.4, §3.5), Part 4
//! (`amqp-1.0-transactions` §4.5) and Part 5 (`amqp-1.0-security`
//! §5.3.3).

/// Symbolic and numeric descriptor of each described type of Parts 2, 3,
/// 4 and 5.
pub const AMQP_DESCRIPTORS: &[(&str, u64)] = &[
    // Part 2 §2.7: the performatives.
    ("amqp:open:list", 0x10),
    ("amqp:begin:list", 0x11),
    ("amqp:attach:list", 0x12),
    ("amqp:flow:list", 0x13),
    ("amqp:transfer:list", 0x14),
    ("amqp:disposition:list", 0x15),
    ("amqp:detach:list", 0x16),
    ("amqp:end:list", 0x17),
    ("amqp:close:list", 0x18),
    // Part 2 §2.8.14.
    ("amqp:error:list", 0x1d),
    // Part 3 §3.4: the delivery states.
    ("amqp:received:list", 0x23),
    ("amqp:accepted:list", 0x24),
    ("amqp:rejected:list", 0x25),
    ("amqp:released:list", 0x26),
    ("amqp:modified:list", 0x27),
    // Part 3 §3.5.3, §3.5.4: the termini.
    ("amqp:source:list", 0x28),
    ("amqp:target:list", 0x29),
    // Part 3 §3.5.10 to §3.5.13: the lifetime policies.
    ("amqp:delete-on-close:list", 0x2b),
    ("amqp:delete-on-no-links:list", 0x2c),
    ("amqp:delete-on-no-messages:list", 0x2d),
    ("amqp:delete-on-no-links-or-messages:list", 0x2e),
    // Part 4 §4.5: the coordinator, the two message bodies sent to it and
    // the two delivery states of a transaction.
    ("amqp:coordinator:list", 0x30),
    ("amqp:declare:list", 0x31),
    ("amqp:discharge:list", 0x32),
    ("amqp:declared:list", 0x33),
    ("amqp:transactional-state:list", 0x34),
    // Part 5 §5.3.3: the SASL frame bodies.
    ("amqp:sasl-mechanisms:list", 0x40),
    ("amqp:sasl-init:list", 0x41),
    ("amqp:sasl-challenge:list", 0x42),
    ("amqp:sasl-response:list", 0x43),
    ("amqp:sasl-outcome:list", 0x44),
    // Part 3 §3.2: the message sections.
    ("amqp:header:list", 0x70),
    ("amqp:delivery-annotations:map", 0x71),
    ("amqp:message-annotations:map", 0x72),
    ("amqp:properties:list", 0x73),
    ("amqp:application-properties:map", 0x74),
    ("amqp:data:binary", 0x75),
    ("amqp:amqp-sequence:list", 0x76),
    ("amqp:amqp-value:*", 0x77),
    ("amqp:footer:map", 0x78),
];

/// The numeric descriptor of the type the symbolic descriptor `symbol`
/// names, `None` for a symbol outside the table.
#[must_use]
pub fn code_of(symbol: &str) -> Option<u64> {
    AMQP_DESCRIPTORS
        .iter()
        .find(|(name, _)| *name == symbol)
        .map(|(_, code)| *code)
}

/// The symbolic descriptor of the type the numeric descriptor `code`
/// names, `None` for a code outside the table.
#[must_use]
pub fn symbol_of(code: u64) -> Option<&'static str> {
    AMQP_DESCRIPTORS
        .iter()
        .find(|(_, c)| *c == code)
        .map(|(name, _)| *name)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    /// §1.5: a symbolic descriptor is `<domain>:<name>`, a numeric one
    /// `(domain-id << 32) | descriptor-id` with domain-id 0 for the
    /// specification's own types; the table holds each type once.
    #[test]
    fn amqp10_1_5_every_descriptor_is_in_the_amqp_domain_and_named_once() {
        for (i, (name, code)) in AMQP_DESCRIPTORS.iter().enumerate() {
            assert!(name.starts_with("amqp:"), "{name}");
            assert_eq!(code >> 32, 0, "{name}");
            for (other, other_code) in &AMQP_DESCRIPTORS[i + 1..] {
                assert_ne!(name, other);
                assert_ne!(code, other_code, "{name} and {other}");
            }
        }
        assert_eq!(AMQP_DESCRIPTORS.len(), 40);
    }

    /// §1.3.3: the two descriptors of one type name each other.
    #[test]
    fn amqp10_1_5_a_symbol_and_its_code_name_the_same_type() {
        assert_eq!(code_of("amqp:open:list"), Some(0x10));
        assert_eq!(code_of("amqp:error:list"), Some(0x1d));
        assert_eq!(code_of("amqp:modified:list"), Some(0x27));
        assert_eq!(code_of("amqp:target:list"), Some(0x29));
        assert_eq!(code_of("amqp:sasl-outcome:list"), Some(0x44));
        assert_eq!(code_of("amqp:coordinator:list"), Some(0x30));
        assert_eq!(symbol_of(0x34), Some("amqp:transactional-state:list"));
        assert_eq!(code_of("amqp:amqp-value:*"), Some(0x77));
        assert_eq!(symbol_of(0x75), Some("amqp:data:binary"));
        assert_eq!(
            symbol_of(0x2e),
            Some("amqp:delete-on-no-links-or-messages:list")
        );
        for (name, code) in AMQP_DESCRIPTORS {
            assert_eq!(code_of(name), Some(*code));
            assert_eq!(symbol_of(*code), Some(*name));
        }
        assert_eq!(code_of("amqp:open"), None);
        assert_eq!(code_of("example:book:list"), None);
        assert_eq!(symbol_of(0x19), None);
    }
}
