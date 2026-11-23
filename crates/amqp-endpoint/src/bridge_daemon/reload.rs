// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! SIGHUP (spec §9.2): what a reload replaces while the daemon runs.

use std::string::String;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::vec::Vec;

use super::acl::{Live, acl_allows, allowed, subjects_of};
use super::config::DaemonConfig;
use super::log::{log, set_participant};
use super::tls_config;

/// What `run_with_reload` needs to reload: the flag the signal watcher sets,
/// and how to read the configuration again.
pub struct Reload {
    /// Set on SIGHUP; cleared when the reload is done.
    pub flag: Arc<AtomicBool>,
    /// Reads and validates the configuration again (the same layered sources
    /// the daemon started from).
    pub load: Box<dyn Fn() -> Result<DaemonConfig, String> + Send + Sync>,
}

/// A participant a reload reaches: its tenant (`None` for the daemon's
/// own), what it holds live, the configuration it started with.
pub(super) type Watched = (Option<String>, Arc<Live>, DaemonConfig);

/// One SIGHUP (§9.2, §1.1) over every participant: each gets its part of
/// `new` ([`reloaded_part`]), and its log lines carry its tenant. The
/// outcome per participant, in their order, is that of [`apply_reload`].
pub(super) fn reload_all(
    parts: &[Watched],
    new: &Result<DaemonConfig, String>,
) -> Vec<Result<Vec<String>, String>> {
    let out = parts
        .iter()
        .map(|(tenant, live, started)| {
            set_participant(tenant.as_deref(), Some(&started.container_id));
            apply_reload(live, started, reloaded_part(new, tenant.as_deref()))
        })
        .collect();
    set_participant(None, None);
    out
}

/// What a reload gives the participant of `tenant` (`None`: the daemon's
/// own): the new configuration, or that of the tenant of the same name in
/// it. A tenant the new configuration no longer names gets an error and
/// keeps running as it is until a restart.
pub(super) fn reloaded_part(
    new: &Result<DaemonConfig, String>,
    tenant: Option<&str>,
) -> Result<DaemonConfig, String> {
    let new = new.clone()?;
    match tenant {
        None => Ok(new),
        Some(name) => new
            .tenants
            .iter()
            .find(|t| t.name == name)
            .map(|t| new.of_tenant(t))
            .ok_or_else(|| {
                format!("the tenant {name} is gone from the configuration; a restart removes it")
            }),
    }
}

/// One reload (§9.2): the ACL applies at once, a withdrawn right stops the
/// topic's samples and transfers; the TLS material applies to the next
/// connect. A right the new ACL grants, changed topics, broker or domain
/// need a restart (the DDS endpoints are made at start) and are logged. A
/// configuration or TLS material that does not load changes nothing.
///
/// Returns what needs a restart, or why the reload was refused.
pub(super) fn apply_reload(
    live: &Live,
    started: &DaemonConfig,
    new: Result<DaemonConfig, String>,
) -> Result<Vec<String>, String> {
    let refused = |e: String| {
        log("reload_refused", &[("err", &e)]);
        Err(e)
    };
    let new = match new {
        Ok(c) => c,
        Err(e) => return refused(e),
    };
    let tls = match tls_config(&new) {
        Ok(t) => t,
        Err(e) => return refused(e),
    };
    // With EXTERNAL a new client certificate is a new subject (§7.3).
    let subjects = match new.sasl().and_then(|sasl| subjects_of(&new, &sasl)) {
        Ok(s) => s,
        Err(e) => return refused(e),
    };
    let mut restart: Vec<String> = Vec::new();
    if new.topics != started.topics {
        restart.push("topics".into());
    }
    if new.broker_url != started.broker_url {
        restart.push("amqp.broker_url".into());
    }
    if new.domain != started.domain {
        restart.push("domain".into());
    }
    let before_subjects = live.subjects();
    for t in &started.topics {
        for write in [false, true] {
            let before = allowed(started, &before_subjects, &t.dds_name, write);
            let now = acl_allows(
                new.acl_default_deny,
                &new.acl_rules,
                &subjects,
                &t.dds_name,
                write,
            );
            if now && !before {
                restart.push(format!(
                    "acl grant {} {}",
                    if write { "write" } else { "read" },
                    t.dds_name
                ));
            }
        }
    }
    if let Ok(mut a) = live.acl.write() {
        *a = (new.acl_default_deny, new.acl_rules.clone());
    }
    if let Ok(mut t) = live.tls.lock() {
        *t = tls;
    }
    if let Ok(mut s) = live.subjects.write() {
        *s = subjects;
    }
    log("reloaded", &[("applied", "acl, tls for the next connect")]);
    if !restart.is_empty() {
        log("reload_needs_restart", &[("changed", &restart.join(", "))]);
    }
    Ok(restart)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::bridge_daemon::test_support::load;

    /// The daemon (`own`) and the tenants `a` (`alice`) and `b` (`bob`),
    /// each with topic `T` on an address of its own; `read` is the subjects
    /// that may read `T`; `b_topics` the topic entries of `b`.
    fn yaml(read: &str, b_topics: &str, with_b: bool) -> String {
        let b = if with_b {
            format!(
                "  - name: b\n    domain: 8\n    amqp:\n      container_id: cb\n      sasl:\n        username: bob\n    topics:\n{b_topics}"
            )
        } else {
            String::new()
        };
        format!(
            "amqp:\n  broker_url: \"amqp://127.0.0.1:5672\"\n  container_id: own\n  sasl:\n    mechanism: PLAIN\n    username: own\n    password: p\nacl:\n  rules:\n    - topic: T\n      read: [{read}]\n      write: [\"*\"]\ntopics:\n  - dds_name: T\n    amqp_address: /queues/own\ntenants:\n  - name: a\n    domain: 7\n    amqp:\n      container_id: ca\n      sasl:\n        username: alice\n    topics:\n      - dds_name: T\n        amqp_address: /queues/a\n{b}"
        )
    }

    const B_TOPICS: &str = "      - dds_name: T\n        amqp_address: /queues/b\n";

    /// The participants of `cfg` as the daemon watches them.
    fn watched(cfg: &DaemonConfig) -> Vec<Watched> {
        let mut parts = vec![(None, cfg.clone())];
        parts.extend(
            cfg.tenants
                .iter()
                .map(|t| (Some(t.name.clone()), cfg.of_tenant(t))),
        );
        parts
            .into_iter()
            .map(|(tenant, c)| {
                let subjects = subjects_of(&c, &c.sasl().unwrap()).unwrap();
                let live = Arc::new(Live::new(&c, None, subjects));
                (tenant, live, c)
            })
            .collect()
    }

    /// §9.2, §1.1: a reload reaches each tenant from the tenant of the same
    /// name. A right withdrawn from `b`'s subject stops `b` alone; a change
    /// of `b`'s topics needs a restart of `b` alone (compared against `a`'s
    /// or the daemon's topics, it would be one for every participant).
    #[test]
    fn amqpbridge_1_l6_a_reload_reaches_each_tenant_from_its_own_entry() {
        let started = load(&yaml(r#""own", "alice", "bob""#, B_TOPICS, true)).unwrap();
        let parts = watched(&started);
        for (_, live, _) in &parts {
            assert!(live.allows("T", false));
        }
        let new = load(&yaml(
            r#""own", "alice""#,
            &format!("{B_TOPICS}      - dds_name: U\n        amqp_address: /queues/u\n"),
            true,
        ));
        let out = reload_all(&parts, &new);
        assert_eq!(out[0], Ok(Vec::new()), "the daemon's own");
        assert_eq!(out[1], Ok(Vec::new()), "tenant a");
        assert_eq!(out[2], Ok(vec!["topics".to_string()]), "tenant b");
        assert!(parts[0].1.allows("T", false));
        assert!(parts[1].1.allows("T", false));
        assert!(!parts[2].1.allows("T", false), "bob's right is withdrawn");
    }

    /// §1.1: a tenant the new configuration no longer names keeps what it
    /// holds and the reload says so; the others are reloaded. A
    /// configuration that does not load changes no participant.
    #[test]
    fn amqpbridge_1_l6_a_tenant_removed_at_reload_is_refused_for_it_alone() {
        let started = load(&yaml(r#""own", "alice", "bob""#, B_TOPICS, true)).unwrap();
        let parts = watched(&started);
        let new = load(&yaml(r#""own""#, B_TOPICS, false));
        let out = reload_all(&parts, &new);
        assert_eq!(out[0], Ok(Vec::new()));
        assert_eq!(out[1], Ok(Vec::new()));
        let gone = out[2].clone().unwrap_err();
        assert!(gone.contains("the tenant b is gone"), "{gone}");
        assert!(!parts[1].1.allows("T", false), "alice's right is withdrawn");
        assert!(parts[2].1.allows("T", false), "b keeps its rules");
        let out = reload_all(&parts, &Err("no file".into()));
        assert!(
            out.iter().all(|o| o == &Err("no file".to_string())),
            "{out:?}"
        );
        assert!(parts[0].1.allows("T", false));
    }
}
