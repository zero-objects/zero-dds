// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-conformance` with `zerodds-mqtt-bridge`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-mqtt-bridge` (`src/oasis_mqtt.rs`, the
//! OASIS MQTT 5.0 vectors): `topic_matches`, `validate_filter`,
//! `validate_topic_name` and `TopicFilterError`, the in-memory `Broker` with
//! `BrokerSubscription`, `QoS` and `Will`, and `ReasonCode`. It relies on the
//! topic-filter grammar, the QoS of a delivery and the reason-code values
//! being the specification's.
//!
//! Oracles: OASIS MQTT 5.0 §4.7.1 (the wildcards `+` and `#`), §4.7.2 (a
//! filter starting with a wildcard does not match a `$` topic), §4.7.1.2
//! (a topic name holds no wildcard), §2.4 (the reason codes, 0x80 and up an
//! error).

use zerodds_mqtt_bridge::{
    ReasonCode, TopicFilterError, topic_matches, validate_filter, validate_topic_name,
};

#[test]
fn mqtt5_4_7_1_the_wildcards_match_one_level_and_the_rest() {
    assert!(topic_matches("a/+/c", "a/b/c"));
    assert!(!topic_matches("a/+/c", "a/b/c/d"));
    assert!(!topic_matches("a/+/c", "a/c"));
    assert!(topic_matches("a/#", "a"));
    assert!(topic_matches("a/#", "a/b/c/d"));
    assert!(!topic_matches("a/#", "b"));
}

#[test]
fn mqtt5_4_7_2_a_wildcard_at_the_start_does_not_match_a_dollar_topic() {
    assert!(!topic_matches("#", "$SYS/uptime"));
    assert!(!topic_matches("+/uptime", "$SYS/uptime"));
    assert!(topic_matches("$SYS/+", "$SYS/uptime"));
}

#[test]
fn mqtt5_4_7_1_2_a_filter_places_its_wildcards_and_a_topic_name_has_none() {
    assert!(validate_filter("a/+/c").is_ok());
    assert!(validate_filter("#").is_ok());
    assert!(matches!(
        validate_filter("a/#/c"),
        Err(TopicFilterError::HashNotAtEnd)
    ));
    assert!(matches!(
        validate_filter("a/foo+/c"),
        Err(TopicFilterError::WildcardMixedWithChars)
    ));
    assert!(validate_topic_name("a/b/c").is_ok());
    assert!(validate_topic_name("a/+/b").is_err());
    assert!(validate_topic_name("a/#").is_err());
}

#[test]
fn mqtt5_2_4_a_reason_code_of_0x80_and_up_is_an_error() {
    assert!(!ReasonCode::Success.is_error());
    assert!(ReasonCode::UnspecifiedError.is_error());
    assert_eq!(ReasonCode::UnspecifiedError as u8, 0x80);
    assert_eq!(ReasonCode::Banned as u8, 0x8a);
    for code in [0x00u8, 0x04, 0x10, 0x80, 0x8a, 0x97, 0xa2] {
        assert_eq!(ReasonCode::from_u8(code).map(|rc| rc as u8), Ok(code));
    }
}
