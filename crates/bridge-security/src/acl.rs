// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! §7.3 topic ACL — read/write permissions per topic with wildcard
//! and group matching.
//!
//! Match rules:
//!
//! * `*` — all subjects (wildcard).
//! * `<name>` — exact match on [`AuthSubject::name`].
//! * `*group:<g>*` — `subject.groups.contains(<g>)`.
//!
//! On failure: 403 (HTTP bridges) or subscribe reject (TCP).

use std::collections::HashMap;

use crate::auth::AuthSubject;

/// Which operation the caller wants to perform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AclOp {
    /// Read permission (subscribe / read sample).
    Read,
    /// Write permission (publish / write sample).
    Write,
}

/// ACL entry for a topic.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AclEntry {
    /// Subjects with read permission.
    pub read: Vec<String>,
    /// Subjects with write permission.
    pub write: Vec<String>,
}

impl AclEntry {
    /// Convenience: everyone may read + write.
    #[must_use]
    pub fn allow_all() -> Self {
        Self {
            read: vec!["*".to_string()],
            write: vec!["*".to_string()],
        }
    }
}

/// Complete ACL across all topics.
#[derive(Debug, Clone, Default)]
pub struct Acl {
    entries: HashMap<String, AclEntry>,
    /// Default for topics that are not in the map.
    /// `None` = deny-by-default (Spec §7.3 default).
    default: Option<AclEntry>,
}

impl Acl {
    /// Empty ACL — deny everything.
    #[must_use]
    pub fn deny_all() -> Self {
        Self {
            entries: HashMap::new(),
            default: None,
        }
    }

    /// Open ACL — allow everything (for `--auth-mode none` without topic limit).
    #[must_use]
    pub fn allow_all() -> Self {
        Self {
            entries: HashMap::new(),
            default: Some(AclEntry::allow_all()),
        }
    }

    /// Set a topic entry.
    pub fn set(&mut self, topic: impl Into<String>, entry: AclEntry) {
        self.entries.insert(topic.into(), entry);
    }

    /// Set the default entry for unknown topics.
    pub fn set_default(&mut self, entry: AclEntry) {
        self.default = Some(entry);
    }

    /// Check permission. Returns `true` = allow, `false` = deny.
    #[must_use]
    pub fn check(&self, subject: &AuthSubject, op: AclOp, topic: &str) -> bool {
        let entry = self.entries.get(topic).or(self.default.as_ref());
        let Some(entry) = entry else {
            return false;
        };
        let list = match op {
            AclOp::Read => &entry.read,
            AclOp::Write => &entry.write,
        };
        list.iter().any(|pat| match_subject(pat, subject))
    }
}

fn match_subject(pat: &str, subject: &AuthSubject) -> bool {
    if pat == "*" {
        return true;
    }
    if let Some(group) = pat
        .strip_prefix("*group:")
        .and_then(|s| s.strip_suffix('*'))
    {
        return subject.groups.iter().any(|g| g == group);
    }
    pat == subject.name
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn alice() -> AuthSubject {
        AuthSubject::new("alice").with_group("engineers")
    }
    fn bob() -> AuthSubject {
        AuthSubject::new("bob")
    }

    #[test]
    fn deny_by_default() {
        let acl = Acl::deny_all();
        assert!(!acl.check(&alice(), AclOp::Read, "Trade"));
    }

    #[test]
    fn explicit_allow_for_user() {
        let mut acl = Acl::deny_all();
        acl.set(
            "Trade",
            AclEntry {
                read: vec!["alice".into()],
                write: vec!["alice".into()],
            },
        );
        assert!(acl.check(&alice(), AclOp::Read, "Trade"));
        assert!(acl.check(&alice(), AclOp::Write, "Trade"));
        assert!(!acl.check(&bob(), AclOp::Read, "Trade"));
    }

    #[test]
    fn star_wildcard_allows_anyone() {
        let mut acl = Acl::deny_all();
        acl.set(
            "Public",
            AclEntry {
                read: vec!["*".into()],
                write: vec!["alice".into()],
            },
        );
        assert!(acl.check(&bob(), AclOp::Read, "Public"));
        assert!(!acl.check(&bob(), AclOp::Write, "Public"));
    }

    #[test]
    fn group_wildcard_allows_members() {
        let mut acl = Acl::deny_all();
        acl.set(
            "EngOnly",
            AclEntry {
                read: vec!["*group:engineers*".into()],
                write: vec!["*group:engineers*".into()],
            },
        );
        assert!(acl.check(&alice(), AclOp::Read, "EngOnly"));
        assert!(!acl.check(&bob(), AclOp::Read, "EngOnly"));
    }

    #[test]
    fn default_entry_used_for_unknown_topic() {
        let mut acl = Acl::deny_all();
        acl.set_default(AclEntry {
            read: vec!["*".into()],
            write: vec![],
        });
        assert!(acl.check(&bob(), AclOp::Read, "AnythingNew"));
        assert!(!acl.check(&bob(), AclOp::Write, "AnythingNew"));
    }

    #[test]
    fn write_uses_write_list_only() {
        let mut acl = Acl::deny_all();
        acl.set(
            "T",
            AclEntry {
                read: vec!["*".into()],
                write: vec!["alice".into()],
            },
        );
        assert!(acl.check(&alice(), AclOp::Write, "T"));
        assert!(!acl.check(&bob(), AclOp::Write, "T"));
    }

    #[test]
    fn allow_all_constructor_lets_everyone_through() {
        let acl = Acl::allow_all();
        assert!(acl.check(&bob(), AclOp::Read, "X"));
        assert!(acl.check(&bob(), AclOp::Write, "Y"));
    }
}
