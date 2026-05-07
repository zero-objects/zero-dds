// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS-WEB Access Control Decision Engine — Spec §7.3.
//!
//! Implementiert §7.3 von partial auf
//! done.
//!
//! Spec-Quelle: OMG DDS-WEB 1.0 §7.3 (S. 11-13) — `AccessController`
//! mit Permission-Rules-Engine, die fuer jede REST-Resource-Operation
//! eine `Permit` / `Deny`-Entscheidung trifft.
//!
//! Das Datenmodell folgt DDS-Security 1.2 §9.4.1.2 Permissions
//! Document (subject_name + grants[] + rules[allow|deny] mit
//! topic-Filter + Validity-Window).

use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Rules-Engine-Entscheidung.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Decision {
    /// Anfrage erlaubt.
    Permit,
    /// Anfrage abgelehnt — der Caller erhaelt HTTP 403.
    #[default]
    Deny,
}

/// Operations-Klasse (publish/subscribe/admin) auf einem Topic /
/// einer Resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    /// `POST /domain_participant/.../publishers` etc.
    Publish,
    /// `POST /domain_participant/.../subscribers` etc.
    Subscribe,
    /// `POST/PUT/DELETE` auf `qos_profile`/`type`/`application`.
    Admin,
    /// `GET` (read-only).
    Read,
}

/// Eine einzelne Allow-/Deny-Rule (Spec §7.3 + DDS-Security §9.4.1.2.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    /// Allow vs Deny.
    pub effect: Decision,
    /// Topic-Glob (`*` = alle, sonst exact-match oder
    /// `prefix*`/`*suffix`).
    pub topic_glob: String,
    /// Erlaubte Operationen.
    pub operations: Vec<Operation>,
}

/// Subject-spezifischer Permissions-Block (Spec §7.3).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Permissions {
    /// Subject-Name (entspricht DDS-Security Identity-Certificate-DN
    /// oder REST-API-Key-Owner).
    pub subject_name: String,
    /// Default-Decision wenn keine Rule matched.
    pub default: Decision,
    /// Liste der Rules in Reihenfolge.
    pub rules: Vec<Rule>,
}

impl Permissions {
    /// Liefert die Decision fuer eine konkrete Operation auf einem
    /// Topic. Spec §7.3 Decision-Tree:
    ///
    /// 1. Erste matchende Rule mit `effect = Deny` -> `Deny`.
    /// 2. Erste matchende Rule mit `effect = Permit` -> `Permit`.
    /// 3. Sonst `default`.
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

/// Glob-Matcher fuer Topic-Patterns analog zu `fnmatch_simple` in
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
    /// Convenience-Konstruktor.
    #[must_use]
    pub fn allow(topic_glob: &str, operations: Vec<Operation>) -> Self {
        Self {
            effect: Decision::Permit,
            topic_glob: topic_glob.to_string(),
            operations,
        }
    }

    /// Convenience-Konstruktor.
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
