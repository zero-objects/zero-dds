// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The daemon's ACL in front of MQTT (Spec §7.3).
//!
//! The rules name MQTT topic filters per subject. A rule grants its
//! subject (`*`: every subject) `allow_publish` — the PUBLISH packets the
//! daemon sends to the broker on a topic name its filters match — and
//! `allow_subscribe` — the subscriptions whose filter lies within one of its
//! filters, and the PUBLISH packets the broker delivers on a topic its
//! filters match. A topic or filter that no rule of any subject names is
//! decided by `default_deny`. The subject is the bridge's own identity
//! towards the broker ([`Subject`]).

use std::string::String;
use std::sync::{Arc, RwLock};
use std::vec::Vec;

use crate::topic_filter::matches;

/// The subject that every rule with this name applies to.
pub const EVERY_SUBJECT: &str = "*";

/// One entry of `acl.rules`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AclRule {
    /// `subject`: a certificate DN, a user name, a JWT `sub`, the SHA-256
    /// fingerprint of the client certificate (`mtls:<hex>`) or `*`.
    pub subject: String,
    /// `allow_publish`: topic filters.
    pub allow_publish: Vec<String>,
    /// `allow_subscribe`: topic filters.
    pub allow_subscribe: Vec<String>,
}

/// The bridge's identity towards the broker (Spec §7.3): the subject DN of
/// its client certificate, else the user name of its CONNECT, else the
/// `sub` of the JWT it presents; `auth.bearer_subject` replaces all three.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Subject {
    /// The name the rules are matched against; `None`: the bridge has no
    /// identity, and only the rules of `*` apply.
    pub name: Option<String>,
    /// The fingerprint `mtls:<SHA-256 hex>` of the client certificate, which
    /// a rule may name instead of the DN.
    pub fingerprint: Option<String>,
}

impl Subject {
    /// A subject of `name`.
    #[must_use]
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
            fingerprint: None,
        }
    }

    /// How the log lines name it.
    #[must_use]
    pub fn display(&self) -> &str {
        self.name.as_deref().unwrap_or("(none)")
    }

    fn is(&self, rule_subject: &str) -> bool {
        rule_subject == EVERY_SUBJECT
            || self.name.as_deref() == Some(rule_subject)
            || self.fingerprint.as_deref() == Some(rule_subject)
    }
}

/// Whether `outer` covers `inner`: every topic name `inner` matches is
/// matched by `outer` (MQTT 5.0 §4.7). A wildcard in the first level of
/// `outer` does not cover a `$` topic ([MQTT-4.7.2-1]).
#[must_use]
pub fn filter_covers(outer: &str, inner: &str) -> bool {
    let o: Vec<&str> = outer.split('/').collect();
    let i: Vec<&str> = inner.split('/').collect();
    if i[0].starts_with('$') && matches!(o[0], "+" | "#") {
        return false;
    }
    for (k, ol) in o.iter().enumerate() {
        if *ol == "#" {
            return true;
        }
        let Some(il) = i.get(k) else {
            return false;
        };
        match (*ol, *il) {
            (_, "#") => return false,
            ("+", _) => {}
            (_, "+") => return false,
            (a, b) if a == b => {}
            _ => return false,
        }
    }
    // `sport/#` also matches `sport`: handled by the `#` level above; here
    // `outer` ended, so `inner` must end too.
    o.len() == i.len()
}

/// Whether some topic name matches both `a` and `b` (MQTT 5.0 §4.7).
#[must_use]
pub fn filters_overlap(a: &str, b: &str) -> bool {
    let x: Vec<&str> = a.split('/').collect();
    let y: Vec<&str> = b.split('/').collect();
    let dollar = |f: &[&str]| f[0].starts_with('$');
    let wild = |f: &[&str]| matches!(f[0], "+" | "#");
    if (dollar(&x) && wild(&y)) || (dollar(&y) && wild(&x)) {
        return false;
    }
    let mut k = 0;
    loop {
        match (x.get(k), y.get(k)) {
            (Some(&"#"), _) | (_, Some(&"#")) => return true,
            (None, None) => return true,
            (Some(&"+"), Some(_)) | (Some(_), Some(&"+")) => {}
            (Some(p), Some(q)) if p == q => {}
            // One ended: `a` and `a/#` share `a`, handled above when the
            // longer one has `#` next.
            _ => return false,
        }
        k += 1;
    }
}

/// The ACL of the top level or of one tenant (Spec §7.3).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DaemonAcl {
    rules: Vec<AclRule>,
    /// `acl.default_deny`; not set: true once a rule exists.
    default_deny: bool,
}

impl DaemonAcl {
    /// The ACL of `rules` with `acl.default_deny` (`None`: not set).
    #[must_use]
    pub fn new(rules: Vec<AclRule>, default_deny: Option<bool>) -> Self {
        let default_deny = default_deny.unwrap_or(!rules.is_empty());
        Self {
            rules,
            default_deny,
        }
    }

    /// The ACL of a configuration.
    #[must_use]
    pub fn of(cfg: &super::config::DaemonConfig) -> Self {
        Self::new(cfg.acl_rules.clone(), cfg.acl_default_deny)
    }

    /// Whether `subject` may publish on the topic name `topic` to the
    /// broker.
    #[must_use]
    pub fn may_publish(&self, subject: &Subject, topic: &str) -> bool {
        self.decide(subject, |r| &r.allow_publish, |f| matches(f, topic))
    }

    /// Whether `subject` may receive a PUBLISH of the broker on the topic
    /// name `topic`.
    #[must_use]
    pub fn may_receive(&self, subject: &Subject, topic: &str) -> bool {
        self.decide(subject, |r| &r.allow_subscribe, |f| matches(f, topic))
    }

    /// Whether `subject` may subscribe to `filter`: a filter of one of its
    /// rules covers it; a filter that no rule's filter overlaps is
    /// decided by `default_deny`.
    #[must_use]
    pub fn may_subscribe(&self, subject: &Subject, filter: &str) -> bool {
        let granted = self
            .rules
            .iter()
            .filter(|r| subject.is(&r.subject))
            .any(|r| r.allow_subscribe.iter().any(|f| filter_covers(f, filter)));
        if granted {
            return true;
        }
        let named = self
            .rules
            .iter()
            .any(|r| r.allow_subscribe.iter().any(|f| filters_overlap(f, filter)));
        !named && !self.default_deny
    }

    /// A topic name: granted when a filter of the subject's rules matches
    /// it; refused when only other subjects' rules name it; else
    /// `default_deny` decides.
    fn decide(
        &self,
        subject: &Subject,
        list: impl Fn(&AclRule) -> &Vec<String>,
        hit: impl Fn(&str) -> bool,
    ) -> bool {
        let mut named = false;
        for rule in &self.rules {
            if list(rule).iter().any(|f| hit(f)) {
                if subject.is(&rule.subject) {
                    return true;
                }
                named = true;
            }
        }
        !named && !self.default_deny
    }
}

/// The ACL and the subject of a connection as they apply now; a SIGHUP
/// reload replaces both (Spec §9.2).
#[derive(Debug)]
pub struct LiveAcl(RwLock<Arc<(DaemonAcl, Subject)>>);

impl LiveAcl {
    /// Starts with `acl` for `subject`.
    #[must_use]
    pub fn new(acl: DaemonAcl, subject: Subject) -> Self {
        Self(RwLock::new(Arc::new((acl, subject))))
    }

    /// The ACL and the subject in force.
    #[must_use]
    pub fn current(&self) -> Arc<(DaemonAcl, Subject)> {
        match self.0.read() {
            Ok(g) => Arc::clone(&g),
            Err(p) => Arc::clone(&p.into_inner()),
        }
    }

    /// Replaces both.
    pub fn replace(&self, acl: DaemonAcl, subject: Subject) {
        let next = Arc::new((acl, subject));
        match self.0.write() {
            Ok(mut g) => *g = next,
            Err(p) => *p.into_inner() = next,
        }
    }
}

/// The subject of the client certificate in `cert_file` (PEM, the first
/// certificate): its DN and fingerprint.
///
/// # Errors
/// The file cannot be read or holds no certificate.
pub fn certificate_subject(cert_file: &str) -> Result<Subject, String> {
    use zerodds_bridge_security::rustls_pemfile::{Item, read_one};
    let pem = std::fs::read(cert_file).map_err(|e| format!("{cert_file}: {e}"))?;
    let mut rd = std::io::BufReader::new(pem.as_slice());
    loop {
        match read_one(&mut rd) {
            Ok(Some(Item::X509Certificate(der))) => {
                let s = zerodds_bridge_security::mtls_subject_from_der(der.as_ref());
                return Ok(Subject {
                    name: Some(s.name),
                    fingerprint: s.fingerprint,
                });
            }
            Ok(Some(_)) => {}
            Ok(None) => return Err(format!("{cert_file}: no certificate")),
            Err(e) => return Err(format!("{cert_file}: {e}")),
        }
    }
}

/// The subject of the connection `cfg` describes (Spec §7.3): the DN of the
/// client certificate in `mqtt.tls.cert_file` (when TLS is on), else the
/// user name it authenticates with (the CONNECT's under `password`, the
/// SCRAM user's under `enhanced`), else the `sub` of the JWT it presents;
/// set `auth.bearer_subject` names it instead.
///
/// # Errors
/// The client certificate cannot be read.
pub fn subject_of(cfg: &super::config::DaemonConfig) -> Result<Subject, String> {
    if let Some(name) = &cfg.auth_bearer_subject {
        return Ok(Subject::named(name.clone()));
    }
    if cfg.broker_tls_enabled && !cfg.broker_tls_client_cert_file.is_empty() {
        return certificate_subject(&cfg.broker_tls_client_cert_file);
    }
    let creds = super::auth::Credentials::of(cfg);
    if let Some(user) = creds.user_name() {
        return Ok(Subject::named(user));
    }
    if let Some(sub) = creds
        .token()
        .and_then(zerodds_bridge_security::jwt_subject_unverified)
    {
        return Ok(Subject::named(sub));
    }
    Ok(Subject::default())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::string::ToString;

    fn rule(subject: &str, publish: &[&str], subscribe: &[&str]) -> AclRule {
        AclRule {
            subject: subject.to_string(),
            allow_publish: publish.iter().map(ToString::to_string).collect(),
            allow_subscribe: subscribe.iter().map(ToString::to_string).collect(),
        }
    }

    /// The rules of the example of Spec §7.3.
    fn spec_acl() -> DaemonAcl {
        DaemonAcl::new(
            vec![
                rule("*", &["sensors/+/public/#"], &["sensors/+/public/#"]),
                rule("alice", &["chat/+"], &[]),
            ],
            Some(true),
        )
    }

    #[test]
    fn mqttbridge_7_3_a_rule_with_a_filter_covers_its_topics() {
        let acl = spec_acl();
        let alice = Subject::named("alice");
        assert!(acl.may_publish(&alice, "chat/general"));
        assert!(!acl.may_publish(&alice, "chat/general/x"));
        assert!(!acl.may_publish(&alice, "chatter"));
        assert!(acl.may_publish(&alice, "sensors/a/public/t"));
        assert!(acl.may_publish(&alice, "sensors/a/public"));
        assert!(!acl.may_publish(&alice, "sensors/a/private/t"));
        // A DDS topic name is no MQTT topic of the rules.
        assert!(!acl.may_publish(&alice, "Chat::Message"));
    }

    #[test]
    fn mqttbridge_7_3_the_star_subject_is_everyone() {
        let acl = spec_acl();
        let bob = Subject::named("bob");
        assert!(acl.may_publish(&bob, "sensors/x/public/y"));
        assert!(!acl.may_publish(&bob, "chat/general"));
        assert!(acl.may_publish(&Subject::default(), "sensors/x/public/y"));
        assert!(acl.may_subscribe(&bob, "sensors/+/public/#"));
        assert!(acl.may_subscribe(&bob, "sensors/k/public/t"));
        assert!(acl.may_receive(&bob, "sensors/k/public/t"));
    }

    #[test]
    fn mqttbridge_7_3_default_deny_closes_the_rest() {
        let closed = spec_acl();
        let open = DaemonAcl::new(vec![rule("alice", &["chat/+"], &["chat/+"])], Some(false));
        let bob = Subject::named("bob");
        assert!(!closed.may_publish(&bob, "other/topic"));
        assert!(open.may_publish(&bob, "other/topic"));
        // A topic a rule names is the rule's subject's alone, also open.
        assert!(!open.may_publish(&bob, "chat/general"));
        assert!(!open.may_receive(&bob, "chat/general"));
        // A filter that reaches into a rule's topics is refused to others.
        assert!(!open.may_subscribe(&bob, "chat/#"));
        assert!(open.may_subscribe(&bob, "other/#"));
        // Not set: closed once a rule exists, open without rules.
        assert!(!DaemonAcl::new(vec![rule("a", &["x"], &[])], None).may_publish(&bob, "y"));
        assert!(DaemonAcl::new(Vec::new(), None).may_publish(&bob, "y"));
        assert!(!DaemonAcl::new(Vec::new(), Some(true)).may_publish(&bob, "y"));
    }

    #[test]
    fn mqttbridge_7_3_publish_and_subscribe_are_separate_rights() {
        let acl = DaemonAcl::new(vec![rule("alice", &["out/#"], &["in/+"])], Some(true));
        let alice = Subject::named("alice");
        assert!(acl.may_publish(&alice, "out/a"));
        assert!(!acl.may_receive(&alice, "out/a"));
        assert!(!acl.may_subscribe(&alice, "out/a"));
        assert!(acl.may_subscribe(&alice, "in/a"));
        assert!(acl.may_receive(&alice, "in/a"));
        assert!(!acl.may_publish(&alice, "in/a"));
        // A filter wider than the grant is refused, a narrower one taken.
        assert!(!acl.may_subscribe(&alice, "in/#"));
        assert!(acl.may_subscribe(&alice, "in/+"));
    }

    #[test]
    fn mqttbridge_7_3_a_filter_covers_and_overlaps_as_mqtt_matches() {
        assert!(filter_covers("a/#", "a"));
        assert!(filter_covers("a/#", "a/+/c"));
        assert!(filter_covers("+/b", "x/b"));
        assert!(!filter_covers("+/b", "x/#"));
        assert!(!filter_covers("a/b", "a/+"));
        assert!(!filter_covers("#", "$SYS/x"));
        assert!(filter_covers("$SYS/#", "$SYS/x"));
        assert!(filters_overlap("chat/+", "chat/#"));
        assert!(filters_overlap("a", "a/#"));
        assert!(!filters_overlap("a/b", "a/c"));
        assert!(!filters_overlap("+/x", "$SYS/x"));
        assert!(filters_overlap("+/x", "y/+"));
        assert!(!filters_overlap("a/+", "a"));
    }

    /// The subject: the certificate DN first, then the user name (the
    /// CONNECT's under `password`, the SCRAM user's under `enhanced`), then
    /// the JWT `sub`; `auth.bearer_subject` names it instead.
    #[test]
    fn mqttbridge_7_3_the_subject_is_the_certificate_dn_then_the_user_name_then_the_jwt_sub() {
        use super::super::config::DaemonConfig;
        let mut params = rcgen::CertificateParams::new(vec!["bridge".into()]).unwrap();
        params.distinguished_name = rcgen::DistinguishedName::new();
        params
            .distinguished_name
            .push(rcgen::DnType::CommonName, "bridge-01");
        params
            .distinguished_name
            .push(rcgen::DnType::OrganizationName, "Plant");
        let key = rcgen::KeyPair::generate().unwrap();
        let cert = params.self_signed(&key).unwrap();
        let file = std::env::temp_dir().join(format!(
            "zerodds-mqtt-acl-subject-{}.pem",
            std::process::id()
        ));
        std::fs::write(&file, cert.pem()).unwrap();
        // header {"alg":"none"}, payload {"sub":"svc-7"}
        let jwt = "eyJhbGciOiJub25lIn0.eyJzdWIiOiJzdmMtNyJ9.";

        let mut cfg = DaemonConfig::default_for_dev();
        cfg.auth_bearer_token = Some(jwt.into());
        assert_eq!(subject_of(&cfg).unwrap(), Subject::named("svc-7"));
        cfg.username = Some("alice".into());
        assert_eq!(subject_of(&cfg).unwrap(), Subject::named("alice"));
        cfg.broker_tls_enabled = true;
        cfg.broker_tls_client_cert_file = file.display().to_string();
        let s = subject_of(&cfg).unwrap();
        assert_eq!(s.name.as_deref(), Some("O=Plant,CN=bridge-01"));
        assert!(s.fingerprint.as_deref().unwrap().starts_with("mtls:"));
        let by_print = DaemonAcl::new(
            vec![rule(s.fingerprint.as_deref().unwrap(), &["t"], &[])],
            Some(true),
        );
        assert!(by_print.may_publish(&s, "t"));
        cfg.auth_bearer_subject = Some("service-account".into());
        assert_eq!(subject_of(&cfg).unwrap(), Subject::named("service-account"));
        let _ = std::fs::remove_file(file);
        let none = DaemonConfig::default_for_dev();
        assert_eq!(subject_of(&none).unwrap(), Subject::default());
        // `enhanced` with SCRAM-SHA-256: the SCRAM user, also when the
        // CONNECT carries no User Name.
        let mut scram = DaemonConfig::default_for_dev();
        scram.auth_enhanced_method = Some("SCRAM-SHA-256".into());
        scram.outbound_username = Some("scram-user".into());
        scram.outbound_password = Some("pw".into());
        assert_eq!(subject_of(&scram).unwrap(), Subject::named("scram-user"));
        scram.outbound_username = None;
        scram.username = Some("mqtt-user".into());
        assert_eq!(subject_of(&scram).unwrap(), Subject::named("mqtt-user"));
        // `enhanced` with a token method: the JWT `sub`, no user name.
        let mut oauth = DaemonConfig::default_for_dev();
        oauth.auth_enhanced_method = Some("OAUTHBEARER".into());
        oauth.auth_bearer_token = Some(jwt.into());
        assert_eq!(subject_of(&oauth).unwrap(), Subject::named("svc-7"));
    }
}
