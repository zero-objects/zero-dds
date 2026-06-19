// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS-WEB Access Control Decision Engine — Spec §7.3.
//!
//! Implements §7.3 from partial to
//! done.
//!
//! Spec source: OMG DDS-WEB 1.0 §7.3 (pp. 11-13) — `AccessController`
//! with a permission rules engine that makes a `Permit` / `Deny`
//! decision for every REST resource operation.
//!
//! The data model follows DDS-Security 1.2 §9.4.1.2 Permissions
//! Document (subject_name + grants[] + rules[allow|deny] with a
//! topic filter + validity window).

use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Rules-engine decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Decision {
    /// Request allowed.
    Permit,
    /// Request denied — the caller receives HTTP 403.
    #[default]
    Deny,
}

/// Operation class (publish/subscribe/admin) on a topic /
/// resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    /// `POST /domain_participant/.../publishers` etc.
    Publish,
    /// `POST /domain_participant/.../subscribers` etc.
    Subscribe,
    /// `POST/PUT/DELETE` on `qos_profile`/`type`/`application`.
    Admin,
    /// `GET` (read-only).
    Read,
}

/// A single allow/deny rule (Spec §7.3 + DDS-Security §9.4.1.2.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    /// Allow vs Deny.
    pub effect: Decision,
    /// Topic glob (`*` = all, otherwise exact match or
    /// `prefix*`/`*suffix`).
    pub topic_glob: String,
    /// Allowed operations.
    pub operations: Vec<Operation>,
}

/// Subject-specific permissions block (Spec §7.3).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Permissions {
    /// Subject name (corresponds to the DDS-Security identity-certificate DN
    /// or the REST API key owner).
    pub subject_name: String,
    /// Default decision when no rule matches.
    pub default: Decision,
    /// List of rules in order.
    pub rules: Vec<Rule>,
}

impl Permissions {
    /// Returns the decision for a concrete operation on a
    /// topic. Spec §7.3 decision tree:
    ///
    /// 1. First matching rule with `effect = Deny` -> `Deny`.
    /// 2. First matching rule with `effect = Permit` -> `Permit`.
    /// 3. Otherwise `default`.
    #[must_use]
    pub fn evaluate(&self, op: Operation, topic: &str) -> Decision {
        let mut found_permit = false;
        for r in &self.rules {
            if !r.operations.contains(&op) {
                continue;
            }
            if !match_glob(&r.topic_glob, topic) {
                continue;
            }
            match r.effect {
                Decision::Deny => return Decision::Deny,
                Decision::Permit => found_permit = true,
            }
        }
        if found_permit {
            Decision::Permit
        } else {
            self.default
        }
    }
}

/// Glob matcher for topic patterns, analogous to `fnmatch_simple` in
/// `model.rs`.
fn match_glob(pattern: &str, topic: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(rest) = pattern.strip_prefix('*') {
        return topic.ends_with(rest);
    }
    if let Some(rest) = pattern.strip_suffix('*') {
        return topic.starts_with(rest);
    }
    pattern == topic
}

impl Rule {
    /// Convenience constructor.
    #[must_use]
    pub fn allow(topic_glob: &str, operations: Vec<Operation>) -> Self {
        Self {
            effect: Decision::Permit,
            topic_glob: topic_glob.to_string(),
            operations,
        }
    }

    /// Convenience constructor.
    #[must_use]
    pub fn deny(topic_glob: &str, operations: Vec<Operation>) -> Self {
        Self {
            effect: Decision::Deny,
            topic_glob: topic_glob.to_string(),
            operations,
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn ops_all() -> Vec<Operation> {
        alloc::vec![
            Operation::Publish,
            Operation::Subscribe,
            Operation::Admin,
            Operation::Read,
        ]
    }

    #[test]
    fn empty_permissions_returns_default_deny() {
        let p = Permissions {
            subject_name: "Alice".to_string(),
            default: Decision::Deny,
            rules: Vec::new(),
        };
        assert_eq!(p.evaluate(Operation::Publish, "Sensor"), Decision::Deny);
    }

    #[test]
    fn allow_rule_grants_permit() {
        let p = Permissions {
            subject_name: "Alice".to_string(),
            default: Decision::Deny,
            rules: alloc::vec![Rule::allow("Sensor", alloc::vec![Operation::Publish])],
        };
        assert_eq!(p.evaluate(Operation::Publish, "Sensor"), Decision::Permit);
    }

    #[test]
    fn deny_rule_overrides_allow_when_listed_first() {
        let p = Permissions {
            subject_name: "Alice".to_string(),
            default: Decision::Permit,
            rules: alloc::vec![
                Rule::deny("SecretTopic", alloc::vec![Operation::Subscribe]),
                Rule::allow("*", ops_all()),
            ],
        };
        assert_eq!(
            p.evaluate(Operation::Subscribe, "SecretTopic"),
            Decision::Deny
        );
        assert_eq!(
            p.evaluate(Operation::Subscribe, "PublicTopic"),
            Decision::Permit
        );
    }

    #[test]
    fn glob_prefix_pattern_matches() {
        let p = Permissions {
            subject_name: "Alice".to_string(),
            default: Decision::Deny,
            rules: alloc::vec![Rule::allow("Sensor*", alloc::vec![Operation::Subscribe])],
        };
        assert_eq!(
            p.evaluate(Operation::Subscribe, "SensorTemperature"),
            Decision::Permit
        );
        assert_eq!(p.evaluate(Operation::Subscribe, "Other"), Decision::Deny);
    }

    #[test]
    fn glob_suffix_pattern_matches() {
        let p = Permissions {
            subject_name: "Alice".to_string(),
            default: Decision::Deny,
            rules: alloc::vec![Rule::allow("*Result", alloc::vec![Operation::Read])],
        };
        assert_eq!(
            p.evaluate(Operation::Read, "TrackingResult"),
            Decision::Permit
        );
        assert_eq!(p.evaluate(Operation::Read, "Other"), Decision::Deny);
    }

    #[test]
    fn operation_mismatch_falls_through_to_default() {
        let p = Permissions {
            subject_name: "Alice".to_string(),
            default: Decision::Deny,
            rules: alloc::vec![Rule::allow("*", alloc::vec![Operation::Publish])],
        };
        assert_eq!(p.evaluate(Operation::Subscribe, "Anything"), Decision::Deny);
    }

    #[test]
    fn first_matching_deny_wins_over_later_allow() {
        let p = Permissions {
            subject_name: "Alice".to_string(),
            default: Decision::Permit,
            rules: alloc::vec![
                Rule::deny("Restricted", alloc::vec![Operation::Admin]),
                Rule::allow("*", alloc::vec![Operation::Admin]),
            ],
        };
        assert_eq!(p.evaluate(Operation::Admin, "Restricted"), Decision::Deny);
        assert_eq!(p.evaluate(Operation::Admin, "Other"), Decision::Permit);
    }
}
