// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The topic ACL of the daemon (spec §7.3): which subject may read a topic
//! (DDS → AMQP) and write it (AMQP → DDS). The subjects of the bridge are
//! its SASL identity, or with EXTERNAL the subject DN of its TLS client
//! certificate and that certificate's fingerprint `mtls:<SHA-256 hex>`.

use std::collections::BTreeMap;
use std::string::{String, ToString};
use std::sync::Arc;
use std::vec::Vec;

use super::config::DaemonConfig;
use crate::broker::SaslCredentials;

pub(super) fn allowed(cfg: &DaemonConfig, subjects: &[String], topic: &str, write: bool) -> bool {
    acl_allows(cfg.acl_default_deny, &cfg.acl_rules, subjects, topic, write)
}

pub(super) type AclRules = BTreeMap<String, (Vec<String>, Vec<String>)>;

/// Whether one of `subjects` may read (`write` false) or write topic
/// `topic`: a rule of the topic names it or `*`; a topic no rule names is
/// open unless `default_deny`.
pub(super) fn acl_allows(
    default_deny: bool,
    rules: &AclRules,
    subjects: &[String],
    topic: &str,
    write: bool,
) -> bool {
    match rules.get(topic) {
        Some((read, wr)) => {
            let list = if write { wr } else { read };
            list.iter().any(|s| s == "*" || subjects.contains(s))
        }
        None => !default_deny,
    }
}

/// The subjects the ACL knows the bridge by (§7.3): the SASL user of
/// PLAIN, SCRAM and XOAUTH2; with EXTERNAL the subject DN (RFC 4514) of
/// the first certificate of `amqp.tls.cert_file` and its fingerprint
/// `mtls:<SHA-256 hex>`, the same form as the other bridges of ZeroDDS;
/// none for ANONYMOUS.
///
/// # Errors
/// EXTERNAL with a `cert_file` that does not hold a certificate.
pub(super) fn subjects_of(
    cfg: &DaemonConfig,
    sasl: &SaslCredentials,
) -> Result<Vec<String>, String> {
    if !matches!(sasl, SaslCredentials::External) {
        return Ok(sasl.subject().map(str::to_string).into_iter().collect());
    }
    let pem = std::fs::read(&cfg.tls_cert_file)
        .map_err(|e| format!("amqp.tls.cert_file '{}': {e}", cfg.tls_cert_file))?;
    let mut reader = std::io::BufReader::new(&pem[..]);
    let leaf = zerodds_bridge_security::rustls_pemfile::certs(&mut reader)
        .next()
        .ok_or_else(|| {
            format!(
                "amqp.tls.cert_file '{}' holds no certificate",
                cfg.tls_cert_file
            )
        })?
        .map_err(|e| format!("amqp.tls.cert_file '{}': {e}", cfg.tls_cert_file))?;
    let subject = zerodds_bridge_security::mtls_subject_from_der(leaf.as_ref());
    let mut out = vec![subject.name];
    out.extend(subject.fingerprint);
    Ok(out)
}

/// What a SIGHUP replaces while the daemon runs (§9.2): the ACL, the TLS
/// material of the next connect, and with it the subjects of a client
/// certificate.
pub(super) struct Live {
    pub(super) acl: std::sync::RwLock<(bool, AclRules)>,
    pub(super) tls: std::sync::Mutex<Option<Arc<rustls::ClientConfig>>>,
    /// The bridge's own subjects, which the ACL names ([`subjects_of`]).
    pub(super) subjects: std::sync::RwLock<Vec<String>>,
}

impl Live {
    pub(super) fn new(
        cfg: &DaemonConfig,
        tls: Option<Arc<rustls::ClientConfig>>,
        subjects: Vec<String>,
    ) -> Self {
        Self {
            acl: std::sync::RwLock::new((cfg.acl_default_deny, cfg.acl_rules.clone())),
            tls: std::sync::Mutex::new(tls),
            subjects: std::sync::RwLock::new(subjects),
        }
    }

    pub(super) fn subjects(&self) -> Vec<String> {
        self.subjects.read().map(|s| s.clone()).unwrap_or_default()
    }

    pub(super) fn allows(&self, topic: &str, write: bool) -> bool {
        let subjects = self.subjects();
        self.acl
            .read()
            .map(|a| acl_allows(a.0, &a.1, &subjects, topic, write))
            .unwrap_or(false)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::bridge_daemon::test_support::load;

    /// The subjects of a bridge with SASL user `user`.
    fn subj(user: Option<&str>) -> Vec<String> {
        user.map(str::to_string).into_iter().collect()
    }

    fn rules() -> AclRules {
        let mut r = AclRules::new();
        r.insert(
            "Orders".into(),
            (vec!["reader".into()], vec!["writer".into()]),
        );
        r.insert("Open".into(), (vec!["*".into()], vec!["*".into()]));
        r.insert("Closed".into(), (Vec::new(), Vec::new()));
        r
    }

    /// §7.3: `read` names the subjects allowed DDS → AMQP, `write` those
    /// allowed AMQP → DDS; one does not grant the other.
    #[test]
    fn amqpbridge_12_1_acl_read_and_write_are_separate_lists() {
        let r = rules();
        assert!(acl_allows(true, &r, &subj(Some("reader")), "Orders", false));
        assert!(!acl_allows(true, &r, &subj(Some("reader")), "Orders", true));
        assert!(acl_allows(true, &r, &subj(Some("writer")), "Orders", true));
        assert!(!acl_allows(
            true,
            &r,
            &subj(Some("writer")),
            "Orders",
            false
        ));
        assert!(!acl_allows(
            false,
            &r,
            &subj(Some("other")),
            "Orders",
            false
        ));
    }

    /// `*` admits every subject, the bridge without one (ANONYMOUS,
    /// EXTERNAL) included; an empty list admits none.
    #[test]
    fn amqpbridge_12_1_acl_the_wildcard_admits_every_subject() {
        let r = rules();
        for subject in [Some("a"), Some("b"), None] {
            assert!(acl_allows(true, &r, &subj(subject), "Open", false));
            assert!(acl_allows(true, &r, &subj(subject), "Open", true));
            assert!(!acl_allows(false, &r, &subj(subject), "Closed", false));
            assert!(!acl_allows(false, &r, &subj(subject), "Closed", true));
        }
        assert!(
            !acl_allows(false, &r, &subj(None), "Orders", false),
            "no subject, no name"
        );
    }

    /// `default_deny`: a topic no rule names is closed with it, open
    /// without it; a named topic follows its rule either way.
    #[test]
    fn amqpbridge_12_1_acl_default_deny_decides_a_topic_no_rule_names() {
        let r = rules();
        assert!(!acl_allows(
            true,
            &r,
            &subj(Some("reader")),
            "Unnamed",
            false
        ));
        assert!(!acl_allows(
            true,
            &r,
            &subj(Some("reader")),
            "Unnamed",
            true
        ));
        assert!(acl_allows(
            false,
            &r,
            &subj(Some("reader")),
            "Unnamed",
            false
        ));
        assert!(acl_allows(
            false,
            &r,
            &subj(Some("reader")),
            "Unnamed",
            true
        ));
        assert!(acl_allows(true, &r, &subj(Some("reader")), "Orders", false));
    }

    /// The configuration's `acl` section reaches [`allowed`], with the
    /// subject the daemon authenticates as.
    #[test]
    fn amqpbridge_12_1_acl_the_configured_rules_decide() {
        let cfg = load(
            "acl:\n  default_deny: true\n  rules:\n    - topic: Orders\n      read: [\"svc\"]\n      write: []\n",
        )
        .unwrap();
        assert!(allowed(&cfg, &subj(Some("svc")), "Orders", false));
        assert!(!allowed(&cfg, &subj(Some("svc")), "Orders", true));
        assert!(!allowed(&cfg, &subj(Some("svc")), "Other", false));
        assert!(!allowed(&cfg, &subj(Some("someone")), "Orders", false));
    }

    /// §9.2: a reload replaces the rules [`Live`] holds; a right withdrawn
    /// is gone at once.
    #[test]
    fn amqpbridge_12_1_acl_live_rules_follow_a_reload() {
        let live = Live {
            acl: std::sync::RwLock::new((false, rules())),
            tls: std::sync::Mutex::new(None),
            subjects: std::sync::RwLock::new(subj(Some("reader"))),
        };
        assert!(live.allows("Orders", false));
        assert!(live.allows("Unnamed", true));
        let mut withdrawn = rules();
        withdrawn.insert("Orders".into(), (Vec::new(), Vec::new()));
        *live.acl.write().unwrap() = (true, withdrawn);
        assert!(!live.allows("Orders", false));
        assert!(!live.allows("Unnamed", true));
        assert!(live.allows("Open", true));
    }

    /// A self-signed certificate with subject `CN=<cn>,O=ZeroDDS Test` and
    /// its key, written to a directory of this test; the paths.
    fn client_cert(cn: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let mut params = rcgen::CertificateParams::new(vec!["localhost".to_string()]).unwrap();
        let mut dn = rcgen::DistinguishedName::new();
        dn.push(rcgen::DnType::OrganizationName, "ZeroDDS Test");
        dn.push(rcgen::DnType::CommonName, cn);
        params.distinguished_name = dn;
        let key = rcgen::KeyPair::generate().unwrap();
        let cert = params.self_signed(&key).unwrap();
        let dir = std::env::temp_dir().join(format!(
            "amqp-acl-{}-{}",
            std::process::id(),
            cn.replace(|c: char| !c.is_ascii_alphanumeric(), "_")
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let (c, k) = (dir.join("client.pem"), dir.join("client.key"));
        std::fs::write(&c, cert.pem()).unwrap();
        std::fs::write(&k, key.serialize_pem()).unwrap();
        (c, k)
    }

    fn external(cert: &std::path::Path, key: &std::path::Path, acl: &str) -> DaemonConfig {
        load(&format!(
            "amqp:\n  broker_url: \"amqps://broker:5671\"\n  sasl:\n    mechanism: EXTERNAL\n  tls:\n    enabled: true\n    ca_file: \"{c}\"\n    cert_file: \"{c}\"\n    key_file: \"{k}\"\n{acl}",
            c = cert.display(),
            k = key.display()
        ))
        .unwrap()
    }

    /// §7.3: with EXTERNAL the subject is the subject DN (RFC 4514) of the
    /// client certificate in `amqp.tls.cert_file`, and its fingerprint
    /// `mtls:<SHA-256 hex>` names the same bridge; a rule naming either
    /// admits it, a rule naming another DN does not.
    #[test]
    fn amqpbridge_7_3_external_takes_the_certificate_dn() {
        let (c, k) = client_cert("bridge-a");
        let cfg = external(&c, &k, "");
        let subjects = subjects_of(&cfg, &cfg.sasl().unwrap()).unwrap();
        assert_eq!(subjects.len(), 2, "{subjects:?}");
        assert_eq!(subjects[0], "CN=bridge-a,O=ZeroDDS Test");
        assert!(subjects[1].starts_with("mtls:"), "{subjects:?}");
        assert_eq!(subjects[1].len(), "mtls:".len() + 64);
        let rules = |who: &str| {
            let mut r = AclRules::new();
            r.insert("T".into(), (vec![who.to_string()], Vec::new()));
            r
        };
        assert!(acl_allows(
            true,
            &rules("CN=bridge-a,O=ZeroDDS Test"),
            &subjects,
            "T",
            false
        ));
        assert!(acl_allows(
            true,
            &rules(&subjects[1]),
            &subjects,
            "T",
            false
        ));
        assert!(!acl_allows(
            true,
            &rules("CN=bridge-b,O=ZeroDDS Test"),
            &subjects,
            "T",
            false
        ));
        assert!(
            !acl_allows(true, &rules("bridge-a"), &subjects, "T", false),
            "the CN alone is not the DN"
        );
        // The configured rules reach it through `allowed`.
        let cfg = external(
            &c,
            &k,
            "acl:\n  default_deny: true\n  rules:\n    - topic: T\n      read: [\"CN=bridge-a,O=ZeroDDS Test\"]\n      write: []\n",
        );
        assert!(allowed(&cfg, &subjects, "T", false));
        assert!(!allowed(&cfg, &subjects, "T", true));
        let _ = std::fs::remove_dir_all(c.parent().unwrap());
    }

    /// §7.3: PLAIN, SCRAM and XOAUTH2 are known by their user, ANONYMOUS by
    /// no subject (only `*` admits it); EXTERNAL with a file that holds no
    /// certificate is a configuration error.
    #[test]
    fn amqpbridge_7_3_the_sasl_user_is_the_subject() {
        for mechanism in ["PLAIN", "SCRAM-SHA-256", "XOAUTH2"] {
            let cfg = load(&format!(
                "amqp:\n  sasl:\n    mechanism: {mechanism}\n    username: svc\n    password: p\n"
            ))
            .unwrap();
            assert_eq!(
                subjects_of(&cfg, &cfg.sasl().unwrap()).unwrap(),
                ["svc"],
                "{mechanism}"
            );
        }
        let cfg = load("amqp:\n  sasl:\n    mechanism: ANONYMOUS\n").unwrap();
        let none = subjects_of(&cfg, &cfg.sasl().unwrap()).unwrap();
        assert!(none.is_empty());
        let r = rules();
        assert!(acl_allows(true, &r, &none, "Open", false));
        assert!(!acl_allows(true, &r, &none, "Orders", false));
        let (c, k) = client_cert("bridge-x");
        std::fs::write(&c, "no certificate\n").unwrap();
        let cfg = external(&c, &k, "");
        let e = subjects_of(&cfg, &cfg.sasl().unwrap()).unwrap_err();
        assert!(e.contains("holds no certificate"), "{e}");
        let _ = std::fs::remove_dir_all(c.parent().unwrap());
    }

    /// §9.2: a reload that brings another client certificate brings its
    /// subject; the rights follow the new DN at once.
    #[test]
    fn amqpbridge_7_3_a_reloaded_certificate_is_the_new_subject() {
        let (a, ka) = client_cert("bridge-r1");
        let (b, kb) = client_cert("bridge-r2");
        let acl = "acl:\n  default_deny: true\n  rules:\n    - topic: T\n      read: [\"CN=bridge-r1,O=ZeroDDS Test\"]\n      write: []\n";
        let started = external(&a, &ka, acl);
        let live = Live::new(
            &started,
            None,
            subjects_of(&started, &started.sasl().unwrap()).unwrap(),
        );
        assert!(live.allows("T", false));
        let restart =
            super::super::reload::apply_reload(&live, &started, Ok(external(&b, &kb, acl)))
                .unwrap();
        assert!(restart.is_empty(), "{restart:?}");
        assert_eq!(live.subjects()[0], "CN=bridge-r2,O=ZeroDDS Test");
        assert!(!live.allows("T", false), "the new DN has no read right");
        let _ = std::fs::remove_dir_all(a.parent().unwrap());
        let _ = std::fs::remove_dir_all(b.parent().unwrap());
    }
}
