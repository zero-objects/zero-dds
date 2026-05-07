// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! OASIS MQTT-5.0 Conformance-Test-Vektoren — Spec §3 + §4.

use crate::{CaseResult, TestCase};

use zerodds_mqtt_bridge::{
    Broker, BrokerSubscription, QoS, ReasonCode, TopicFilterError, Will, topic_matches,
    validate_filter, validate_topic_name,
};

// ============================================================================
// Section 1 — §4.7 Topic-Filter-Matching mit Wildcards
// ============================================================================

fn case_1_1_exact_match() -> CaseResult {
    if topic_matches("a/b", "a/b") && !topic_matches("a/b", "a/c") {
        CaseResult::Pass
    } else {
        CaseResult::Fail("§4.7.1 exact topic match".into())
    }
}

fn case_1_2_plus_wildcard_single_level() -> CaseResult {
    if topic_matches("a/+/c", "a/b/c")
        && topic_matches("a/+/c", "a/X/c")
        && !topic_matches("a/+/c", "a/b/c/d")
        && !topic_matches("a/+/c", "a/c")
    {
        CaseResult::Pass
    } else {
        CaseResult::Fail("§4.7.1 + wildcard semantics".into())
    }
}

fn case_1_3_hash_wildcard_multi_level() -> CaseResult {
    if topic_matches("a/#", "a/b")
        && topic_matches("a/#", "a/b/c/d")
        && topic_matches("a/#", "a")
        && !topic_matches("a/#", "b")
    {
        CaseResult::Pass
    } else {
        CaseResult::Fail("§4.7.1 # wildcard semantics".into())
    }
}

fn case_1_4_dollar_topic_protected() -> CaseResult {
    // Spec §4.7.2: "#" must NOT match $SYS/...
    if !topic_matches("#", "$SYS/uptime")
        && !topic_matches("+/uptime", "$SYS/uptime")
        && topic_matches("$SYS/uptime", "$SYS/uptime")
        && topic_matches("$SYS/+", "$SYS/uptime")
    {
        CaseResult::Pass
    } else {
        CaseResult::Fail("§4.7.2 $-topic protection".into())
    }
}

// ============================================================================
// Section 2 — §3.8.4 Subscribe-Filter-Validation
// ============================================================================

fn case_2_1_filter_validation() -> CaseResult {
    if validate_filter("a/b/c").is_ok()
        && validate_filter("a/+/c").is_ok()
        && validate_filter("a/b/#").is_ok()
        && validate_filter("#").is_ok()
        && matches!(
            validate_filter("a/#/c"),
            Err(TopicFilterError::HashNotAtEnd)
        )
        && matches!(
            validate_filter("a/foo+/c"),
            Err(TopicFilterError::WildcardMixedWithChars)
        )
    {
        CaseResult::Pass
    } else {
        CaseResult::Fail("§4.7.1.1 filter validation".into())
    }
}

fn case_2_2_topic_name_no_wildcards() -> CaseResult {
    if validate_topic_name("a/b/c").is_ok()
        && validate_topic_name("a/+/b").is_err()
        && validate_topic_name("a/#").is_err()
    {
        CaseResult::Pass
    } else {
        CaseResult::Fail("§4.7.1.2 topic-name no wildcards".into())
    }
}

// ============================================================================
// Section 3 — §3.3 Publish/Subscribe + §3.3.1.3 Retained-Messages
// ============================================================================

fn make_sub(filter: &str, qos: QoS) -> BrokerSubscription {
    BrokerSubscription {
        filter: filter.into(),
        max_qos: qos,
        no_local: false,
        retain_as_published: false,
    }
}

fn case_3_1_subscribe_then_publish() -> CaseResult {
    let mut b = Broker::new();
    b.connect("c1".into(), true, None);
    if b.subscribe("c1", alloc::vec![make_sub("a/+", QoS::AtLeastOnce)])
        .is_err()
    {
        return CaseResult::Fail("subscribe failed".into());
    }
    let envs = b.publish("a/x", alloc::vec![1, 2, 3], QoS::AtLeastOnce, false);
    if envs.len() == 1 && envs[0].client_id == "c1" && envs[0].payload == alloc::vec![1, 2, 3] {
        CaseResult::Pass
    } else {
        CaseResult::Fail("§3.3 publish delivery".into())
    }
}

fn case_3_2_qos_min_of_pub_and_sub() -> CaseResult {
    let mut b = Broker::new();
    b.connect("c1".into(), true, None);
    if b.subscribe("c1", alloc::vec![make_sub("t", QoS::AtLeastOnce)])
        .is_err()
    {
        return CaseResult::Fail("subscribe failed".into());
    }
    let envs = b.publish("t", alloc::vec![1], QoS::ExactlyOnce, false);
    if envs[0].qos == QoS::AtLeastOnce {
        CaseResult::Pass
    } else {
        CaseResult::Fail("§3.8.3 QoS-min".into())
    }
}

fn case_3_3_retained_message() -> CaseResult {
    let mut b = Broker::new();
    b.publish("t", alloc::vec![1, 2], QoS::AtMostOnce, true);
    if b.retained_count() != 1 {
        return CaseResult::Fail("§3.3.1.3 retained should be 1".into());
    }
    // Empty payload clears retained.
    b.publish("t", alloc::vec![], QoS::AtMostOnce, true);
    if b.retained_count() != 0 {
        return CaseResult::Fail("§3.3.1.3 empty payload should clear retained".into());
    }
    CaseResult::Pass
}

// ============================================================================
// Section 4 — §3.1.2.5 Will-Message
// ============================================================================

fn case_4_1_will_delivered_on_abnormal_disconnect() -> CaseResult {
    let mut b = Broker::new();
    b.connect(
        "c1".into(),
        true,
        Some(Will {
            topic: "lwt".into(),
            payload: alloc::vec![9, 9],
            qos: QoS::AtLeastOnce,
            retain: false,
        }),
    );
    b.connect("c2".into(), true, None);
    let _ = b.subscribe("c2", alloc::vec![make_sub("lwt", QoS::AtLeastOnce)]);
    let envs = b.disconnect("c1", true);
    if envs.len() == 1 && envs[0].client_id == "c2" {
        CaseResult::Pass
    } else {
        CaseResult::Fail("§3.1.2.5 will delivery".into())
    }
}

fn case_4_2_clean_disconnect_drops_will() -> CaseResult {
    let mut b = Broker::new();
    b.connect(
        "c1".into(),
        true,
        Some(Will {
            topic: "lwt".into(),
            payload: alloc::vec![],
            qos: QoS::AtMostOnce,
            retain: false,
        }),
    );
    let envs = b.disconnect("c1", false);
    if envs.is_empty() {
        CaseResult::Pass
    } else {
        CaseResult::Fail("§3.14.4 clean disconnect drops will".into())
    }
}

// ============================================================================
// Section 5 — §2.4 Reason-Codes
// ============================================================================

fn case_5_1_reason_codes_error_bit() -> CaseResult {
    if !ReasonCode::Success.is_error()
        && ReasonCode::UnspecifiedError.is_error()
        && ReasonCode::TopicNameInvalid.is_error()
        && (ReasonCode::Banned as u8) == 0x8a
    {
        CaseResult::Pass
    } else {
        CaseResult::Fail("§2.4 reason-code semantics".into())
    }
}

fn case_5_2_all_43_codes_round_trip() -> CaseResult {
    let codes = [
        0x00u8, 0x01, 0x02, 0x04, 0x10, 0x11, 0x18, 0x19, 0x80, 0x81, 0x82, 0x83, 0x84, 0x85, 0x86,
        0x87, 0x88, 0x89, 0x8a, 0x8b, 0x8c, 0x8d, 0x8e, 0x8f, 0x90, 0x91, 0x92, 0x93, 0x94, 0x95,
        0x96, 0x97, 0x98, 0x99, 0x9a, 0x9b, 0x9c, 0x9d, 0x9e, 0x9f, 0xa0, 0xa1, 0xa2,
    ];
    for c in codes {
        match ReasonCode::from_u8(c) {
            Ok(rc) if rc as u8 == c => {}
            _ => {
                return CaseResult::Fail(alloc::format!("§2.4: code {c:#x} round-trip"));
            }
        }
    }
    CaseResult::Pass
}

/// Komplette OASIS-MQTT-5.0 Test-Suite.
pub const SUITE: &[TestCase] = &[
    TestCase {
        name: "mqtt5-4.7.1-exact-match",
        run: case_1_1_exact_match,
    },
    TestCase {
        name: "mqtt5-4.7.1-plus-wildcard",
        run: case_1_2_plus_wildcard_single_level,
    },
    TestCase {
        name: "mqtt5-4.7.1-hash-wildcard",
        run: case_1_3_hash_wildcard_multi_level,
    },
    TestCase {
        name: "mqtt5-4.7.2-dollar-topic-protected",
        run: case_1_4_dollar_topic_protected,
    },
    TestCase {
        name: "mqtt5-4.7.1.1-filter-validation",
        run: case_2_1_filter_validation,
    },
    TestCase {
        name: "mqtt5-4.7.1.2-topic-name-no-wildcards",
        run: case_2_2_topic_name_no_wildcards,
    },
    TestCase {
        name: "mqtt5-3.3-subscribe-then-publish",
        run: case_3_1_subscribe_then_publish,
    },
    TestCase {
        name: "mqtt5-3.8.3-qos-min",
        run: case_3_2_qos_min_of_pub_and_sub,
    },
    TestCase {
        name: "mqtt5-3.3.1.3-retained-message",
        run: case_3_3_retained_message,
    },
    TestCase {
        name: "mqtt5-3.1.2.5-will-delivered",
        run: case_4_1_will_delivered_on_abnormal_disconnect,
    },
    TestCase {
        name: "mqtt5-3.14.4-clean-disconnect-drops-will",
        run: case_4_2_clean_disconnect_drops_will,
    },
    TestCase {
        name: "mqtt5-2.4-reason-codes-error-bit",
        run: case_5_1_reason_codes_error_bit,
    },
    TestCase {
        name: "mqtt5-2.4-all-43-codes-roundtrip",
        run: case_5_2_all_43_codes_round_trip,
    },
];

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn full_suite_passes() {
        let (p, s, f) = crate::run_suite(SUITE);
        assert_eq!(f, 0, "no MQTT cases must fail");
        assert_eq!(p + s, SUITE.len());
        assert!(p >= 12);
    }
}
