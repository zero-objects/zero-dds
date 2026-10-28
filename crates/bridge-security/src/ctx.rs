// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Daemon-facing convenience: `SecurityConfig` (CLI/YAML surface) →
//! [`SecurityCtx`] (resolved). Used identically by all six bridge
//! daemons — the only difference is the connection path into which the
//! ctx is hooked.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::acl::{Acl, AclEntry, AclOp};
use crate::auth::{AuthError, AuthInput, AuthMode, AuthSubject};
use crate::tls::{TlsConfigError, load_server_config, load_server_config_with_client_auth};

/// The security context a running daemon checks against, replaced as a
/// whole by a SIGHUP reload (the bridge specs' §9.2: TLS material and ACL
/// hot-update). Every check reads [`LiveSecurity::current`]; a connection
/// accepted after a reload gets the new TLS configuration, one already open
/// keeps its session.
#[derive(Debug)]
pub struct LiveSecurity(std::sync::RwLock<Arc<SecurityCtx>>);

impl LiveSecurity {
    /// Starts with `ctx`.
    #[must_use]
    pub fn new(ctx: SecurityCtx) -> Self {
        Self(std::sync::RwLock::new(Arc::new(ctx)))
    }

    /// The context as it is now.
    #[must_use]
    pub fn current(&self) -> Arc<SecurityCtx> {
        match self.0.read() {
            Ok(g) => Arc::clone(&g),
            Err(p) => Arc::clone(&p.into_inner()),
        }
    }

    /// Replaces the context.
    pub fn replace(&self, ctx: SecurityCtx) {
        match self.0.write() {
            Ok(mut g) => *g = Arc::new(ctx),
            Err(p) => *p.into_inner() = Arc::new(ctx),
        }
    }
}

/// Resolved security config — the output of this layer.
#[derive(Clone)]
pub struct SecurityCtx {
    /// `Some(...)` ⇒ TLS active; rustls ServerConfig (with or without mTLS).
    pub tls: Option<Arc<rustls::ServerConfig>>,
    /// Auth mode (none/bearer/jwt/mtls/sasl).
    pub auth: Arc<AuthMode>,
    /// Topic ACL.
    pub acl: Arc<Acl>,
}

impl core::fmt::Debug for SecurityCtx {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SecurityCtx")
            .field("tls_enabled", &self.tls.is_some())
            .field("auth", &self.auth)
            .finish_non_exhaustive()
    }
}

/// Raw config from YAML/CLI.
#[derive(Debug, Clone, Default)]
pub struct SecurityConfig {
    /// PEM cert path (`--tls-cert`).
    pub tls_cert: Option<PathBuf>,
    /// PEM key path (`--tls-key`).
    pub tls_key: Option<PathBuf>,
    /// PEM CA bundle for mTLS client cert validation (`--client-ca`).
    pub client_ca: Option<PathBuf>,
    /// Auth mode string (`none|bearer|jwt|mtls|sasl`).
    pub auth_mode: String,
    /// Bearer tokens as a map `token → subject name`.
    pub bearer_tokens: HashMap<String, String>,
    /// JWT RSA public key (PKCS#1 DER).
    pub jwt_pubkey_der: Option<Vec<u8>>,
    /// JWT expected `iss` claim.
    pub jwt_expected_iss: Option<String>,
    /// JWT expected `aud` claim.
    pub jwt_expected_aud: Option<String>,
    /// JWT `(claim, value)` the token has to carry.
    pub jwt_required_claim: Option<(String, String)>,
    /// SASL-PLAIN: `user → password` map (for AMQP/MQTT).
    pub sasl_users: HashMap<String, String>,
    /// ACL per topic name.
    pub topic_acl: HashMap<String, AclEntry>,
    /// ACL default entry for unknown topics. `None` = deny-by-default.
    pub topic_acl_default: Option<AclEntry>,
}

/// The `auth.jwt` block of a daemon's configuration (bridge specs §3):
/// the RS256 key as a PEM file and the claim checks.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JwtSettings {
    /// `auth.jwt.public_key` — PEM, PKCS#1 or SubjectPublicKeyInfo.
    pub public_key: String,
    /// `auth.jwt.issuer` — the `iss` claim.
    pub issuer: Option<String>,
    /// `auth.jwt.audience` — the `aud` claim.
    pub audience: Option<String>,
    /// `auth.jwt.required_claim` — `claim=value`.
    pub required_claim: Option<String>,
}

impl SecurityConfig {
    /// Reads the key `jwt` names and sets the JWT fields.
    ///
    /// # Errors
    /// [`SecurityError::Config`] naming the `auth.jwt.*` key at fault.
    pub fn apply_jwt(&mut self, jwt: &JwtSettings) -> Result<(), SecurityError> {
        if jwt.public_key.is_empty() {
            return Err(SecurityError::Config(
                "auth.mode: jwt needs auth.jwt.public_key".into(),
            ));
        }
        let pem = std::fs::read_to_string(&jwt.public_key).map_err(|e| {
            SecurityError::Config(format!("auth.jwt.public_key {}: {e}", jwt.public_key))
        })?;
        self.jwt_pubkey_der = Some(
            crate::auth::jwt_public_key_from_pem(&pem)
                .map_err(|e| SecurityError::Config(format!("auth.jwt.public_key: {e}")))?,
        );
        self.jwt_expected_iss = jwt.issuer.clone();
        self.jwt_expected_aud = jwt.audience.clone();
        self.jwt_required_claim = match &jwt.required_claim {
            None => None,
            Some(rc) => {
                let (k, v) = rc.split_once('=').ok_or_else(|| {
                    SecurityError::Config(format!(
                        "auth.jwt.required_claim {rc}: expected claim=value"
                    ))
                })?;
                Some((k.trim().to_string(), v.trim().to_string()))
            }
        };
        Ok(())
    }
}

/// Setup error.
#[derive(Debug)]
pub enum SecurityError {
    /// Cert/key loader reports an error.
    Tls(TlsConfigError),
    /// Auth mode string unknown.
    UnknownAuthMode(String),
    /// Auth mode requires input that is missing.
    MissingAuthInput(&'static str),
    /// A setting that names the key it comes from, such as
    /// `auth.jwt.public_key` pointing at a file that is not a key.
    Config(String),
}

impl core::fmt::Display for SecurityError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Tls(e) => write!(f, "tls: {e}"),
            Self::UnknownAuthMode(m) => write!(f, "unknown auth-mode: {m}"),
            Self::MissingAuthInput(s) => write!(f, "missing auth input: {s}"),
            Self::Config(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for SecurityError {}

impl From<TlsConfigError> for SecurityError {
    fn from(e: TlsConfigError) -> Self {
        Self::Tls(e)
    }
}

/// Builds [`SecurityCtx`] from [`SecurityConfig`]. Called at daemon
/// start and on SIGHUP reload.
///
/// # Errors
/// [`SecurityError`] on TLS config error or unknown auth mode.
pub fn build_ctx(cfg: &SecurityConfig) -> Result<SecurityCtx, SecurityError> {
    let tls = match (&cfg.tls_cert, &cfg.tls_key) {
        (Some(c), Some(k)) => match &cfg.client_ca {
            Some(ca) => Some(load_server_config_with_client_auth(c, k, ca)?),
            None => Some(load_server_config(c, k)?),
        },
        (None, None) => None,
        _ => {
            return Err(SecurityError::Tls(TlsConfigError::Rustls(
                "tls_cert and tls_key must be set together".to_string(),
            )));
        }
    };

    let auth = match cfg.auth_mode.as_str() {
        "" | "none" => AuthMode::None,
        "bearer" => {
            let mut tokens: HashMap<String, AuthSubject> = HashMap::new();
            for (tok, name) in &cfg.bearer_tokens {
                tokens.insert(tok.clone(), AuthSubject::new(name.clone()));
            }
            AuthMode::Bearer { tokens }
        }
        "jwt" => {
            let der = cfg
                .jwt_pubkey_der
                .clone()
                .ok_or(SecurityError::MissingAuthInput("jwt_pubkey_der"))?;
            AuthMode::Jwt {
                pkcs1_pubkey_der: der,
                expected_issuer: cfg.jwt_expected_iss.clone(),
                expected_audience: cfg.jwt_expected_aud.clone(),
                required_claim: cfg.jwt_required_claim.clone(),
            }
        }
        "mtls" => AuthMode::Mtls,
        "sasl" => {
            if cfg.sasl_users.is_empty() {
                return Err(SecurityError::MissingAuthInput("sasl_users"));
            }
            AuthMode::SaslPlain {
                users: cfg.sasl_users.clone(),
            }
        }
        other => return Err(SecurityError::UnknownAuthMode(other.to_string())),
    };

    let mut acl = if cfg.topic_acl.is_empty() && cfg.topic_acl_default.is_none() {
        if matches!(auth, AuthMode::None) {
            Acl::allow_all()
        } else {
            Acl::deny_all()
        }
    } else {
        Acl::deny_all()
    };
    for (topic, entry) in &cfg.topic_acl {
        acl.set(topic.clone(), entry.clone());
    }
    if let Some(d) = &cfg.topic_acl_default {
        acl.set_default(d.clone());
    }

    Ok(SecurityCtx {
        tls,
        auth: Arc::new(auth),
        acl: Arc::new(acl),
    })
}

/// Authentication wrapper. Each daemon fills the inputs relevant to it:
/// * HTTP/WS/gRPC: `authorization_header`
/// * mTLS (all TCP bridges): `mtls_subject` extracted from `rustls::ServerConnection::peer_certificates()`
/// * MQTT/AMQP SASL-PLAIN: `sasl_plain_blob`
///
/// # Errors
/// [`AuthError`] on any form of reject/missing/malformed.
pub fn authenticate(
    auth: &AuthMode,
    authorization_header: Option<&str>,
    sasl_plain_blob: Option<&[u8]>,
    mtls_subject: Option<AuthSubject>,
) -> Result<AuthSubject, AuthError> {
    let input = AuthInput {
        authorization_header,
        sasl_plain_blob,
        mtls_subject,
    };
    auth.validate(&input)
}

/// Convenience: topic ACL check.
#[must_use]
pub fn authorize(acl: &Acl, subject: &AuthSubject, op: AclOp, topic: &str) -> bool {
    acl.check(subject, op, topic)
}

/// Extracts an `AuthSubject` from a `rustls::ServerConnection` peer cert
/// (for `AuthMode::Mtls`). Returns `None` if no cert was presented.
///
/// Subject name = the certificate's subject DN (RFC 4514), fingerprint
/// `mtls:<SHA-256 hex>` alongside; see [`mtls_subject_from_der`].
#[must_use]
pub fn extract_mtls_subject(conn: &rustls::ServerConnection) -> Option<AuthSubject> {
    let certs = conn.peer_certificates()?;
    Some(mtls_subject_from_der(certs.first()?.as_ref()))
}

/// The mTLS subject of a client certificate in DER (also for DTLS).
///
/// The bridge specs name the subject DN as the identity (§7.2/§7.3): the
/// name is the certificate's subject as an RFC 4514 string
/// (`CN=alice,O=Example`). The fingerprint `mtls:<SHA-256 hex>` goes along,
/// and an ACL entry may name either. A certificate whose subject is empty
/// or does not parse is known by its fingerprint alone.
#[must_use]
pub fn mtls_subject_from_der(leaf_der: &[u8]) -> AuthSubject {
    let fingerprint = format!("mtls:{}", sha256_hex(leaf_der));
    let mut subject = AuthSubject::new(subject_dn(leaf_der).unwrap_or_else(|| fingerprint.clone()));
    subject.fingerprint = Some(fingerprint);
    subject
}

/// RFC 4514 §2: the RDNs in reverse order of the encoding, joined by `,`,
/// multi-valued RDNs by `+`; attribute types by their short name (§3) or
/// dotted OID, values escaped per §2.4.
fn subject_dn(der: &[u8]) -> Option<String> {
    use x509_parser::prelude::{FromDer, X509Certificate};
    let (_, cert) = X509Certificate::from_der(der).ok()?;
    let registry = x509_parser::objects::oid_registry();
    let mut rdns: Vec<String> = Vec::new();
    for rdn in cert.subject().iter() {
        let mut parts: Vec<String> = Vec::new();
        for attr in rdn.iter() {
            let key = x509_parser::objects::oid2abbrev(attr.attr_type(), registry)
                .map_or_else(|_| attr.attr_type().to_id_string(), ToString::to_string);
            let value = attr.as_str().ok()?;
            parts.push(format!("{key}={}", escape_dn_value(value)));
        }
        rdns.push(parts.join("+"));
    }
    rdns.reverse();
    let dn = rdns.join(",");
    (!dn.is_empty()).then_some(dn)
}

/// RFC 4514 §2.4 escaping of an attribute value.
fn escape_dn_value(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    let last = v.chars().count().saturating_sub(1);
    for (i, c) in v.chars().enumerate() {
        let special = matches!(c, '"' | '+' | ',' | ';' | '<' | '>' | '\\')
            || (i == 0 && (c == ' ' || c == '#'))
            || (i == last && c == ' ');
        if c == '\0' {
            out.push_str("\\00");
            continue;
        }
        if special {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

fn sha256_hex(data: &[u8]) -> String {
    use crate::backend::digest::{Context, SHA256};
    let mut ctx = Context::new(&SHA256);
    ctx.update(data);
    let d = ctx.finish();
    let mut s = String::with_capacity(64);
    for b in d.as_ref() {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn build_default_yields_none_auth_allow_all_acl() {
        let cfg = SecurityConfig::default();
        let ctx = build_ctx(&cfg).unwrap();
        assert!(ctx.tls.is_none());
        assert!(matches!(*ctx.auth, AuthMode::None));
        assert!(ctx.acl.check(&AuthSubject::anonymous(), AclOp::Read, "X"));
    }

    #[test]
    fn build_bearer_with_tokens() {
        let mut cfg = SecurityConfig {
            auth_mode: "bearer".into(),
            ..Default::default()
        };
        cfg.bearer_tokens.insert("t".into(), "alice".into());
        let ctx = build_ctx(&cfg).unwrap();
        let s = authenticate(&ctx.auth, Some("Bearer t"), None, None).unwrap();
        assert_eq!(s.name, "alice");
    }

    #[test]
    fn build_sasl_with_users() {
        let mut cfg = SecurityConfig {
            auth_mode: "sasl".into(),
            ..Default::default()
        };
        cfg.sasl_users.insert("u".into(), "p".into());
        let ctx = build_ctx(&cfg).unwrap();
        let s = authenticate(&ctx.auth, None, Some(b"\0u\0p"), None).unwrap();
        assert_eq!(s.name, "u");
    }

    #[test]
    fn build_sasl_without_users_rejected() {
        let cfg = SecurityConfig {
            auth_mode: "sasl".into(),
            ..Default::default()
        };
        let err = build_ctx(&cfg).unwrap_err();
        assert!(matches!(err, SecurityError::MissingAuthInput(_)));
    }

    #[test]
    fn unknown_auth_mode_rejected() {
        let cfg = SecurityConfig {
            auth_mode: "weird".into(),
            ..Default::default()
        };
        let err = build_ctx(&cfg).unwrap_err();
        assert!(matches!(err, SecurityError::UnknownAuthMode(_)));
    }

    #[test]
    fn jwt_without_key_rejected() {
        let cfg = SecurityConfig {
            auth_mode: "jwt".into(),
            ..Default::default()
        };
        let err = build_ctx(&cfg).unwrap_err();
        assert!(matches!(err, SecurityError::MissingAuthInput(_)));
    }

    #[test]
    fn explicit_acl_overrides_open_default() {
        let mut cfg = SecurityConfig::default();
        cfg.topic_acl.insert(
            "T".into(),
            AclEntry {
                read: vec!["alice".into()],
                write: vec!["alice".into()],
            },
        );
        let ctx = build_ctx(&cfg).unwrap();
        let alice = AuthSubject::new("alice");
        let bob = AuthSubject::new("bob");
        assert!(authorize(&ctx.acl, &alice, AclOp::Read, "T"));
        assert!(!authorize(&ctx.acl, &bob, AclOp::Read, "T"));
        assert!(!authorize(&ctx.acl, &alice, AclOp::Read, "Other"));
    }

    #[test]
    fn explicit_acl_default_used_for_unknown() {
        let cfg = SecurityConfig {
            topic_acl_default: Some(AclEntry {
                read: vec!["*".into()],
                write: vec![],
            }),
            ..SecurityConfig::default()
        };
        let ctx = build_ctx(&cfg).unwrap();
        let bob = AuthSubject::new("bob");
        assert!(authorize(&ctx.acl, &bob, AclOp::Read, "Anything"));
        assert!(!authorize(&ctx.acl, &bob, AclOp::Write, "Anything"));
    }

    #[test]
    fn partial_tls_paths_rejected() {
        let cfg = SecurityConfig {
            tls_cert: Some("/x".into()),
            tls_key: None,
            ..Default::default()
        };
        let err = build_ctx(&cfg).unwrap_err();
        assert!(matches!(err, SecurityError::Tls(_)));
    }

    /// §7.2: the mTLS subject is the certificate's subject DN (RFC 4514,
    /// most specific RDN first, `,` escaped), and an ACL entry may name the
    /// DN or the fingerprint.
    #[test]
    fn the_mtls_subject_is_the_dn_and_the_acl_also_takes_the_fingerprint() {
        let mut p = rcgen::CertificateParams::new(Vec::<String>::new()).unwrap();
        p.distinguished_name
            .push(rcgen::DnType::OrganizationName, "Example, Inc");
        p.distinguished_name
            .push(rcgen::DnType::CommonName, "alice");
        let key = rcgen::KeyPair::generate().unwrap();
        let cert = p.self_signed(&key).unwrap();
        let s = mtls_subject_from_der(cert.der());
        // The reference: OpenSSL's RFC 2253 form (the order RFC 4514 took
        // over) of the same certificate.
        let pem = std::env::temp_dir().join(format!("zd-dn-{}.pem", std::process::id()));
        std::fs::write(&pem, cert.pem()).unwrap();
        let out = std::process::Command::new("openssl")
            .args(["x509", "-noout", "-subject", "-nameopt", "RFC2253", "-in"])
            .arg(&pem)
            .output()
            .expect("openssl on PATH: the reference for the DN string");
        let _ = std::fs::remove_file(&pem);
        let reference = String::from_utf8(out.stdout).unwrap();
        let reference = reference.trim().trim_start_matches("subject=");
        assert_eq!(s.name, reference);
        assert!(s.name.contains("Example\\, Inc"), "{}", s.name);
        let fp = s.fingerprint.clone().unwrap();
        assert!(fp.starts_with("mtls:") && fp.len() == 5 + 64, "{fp}");
        for pat in [s.name.clone(), fp] {
            let mut cfg = SecurityConfig::default();
            cfg.topic_acl.insert(
                "T".into(),
                AclEntry {
                    read: vec![pat.clone()],
                    write: Vec::new(),
                },
            );
            let ctx = build_ctx(&cfg).unwrap();
            assert!(authorize(&ctx.acl, &s, AclOp::Read, "T"), "{pat}");
            assert!(!authorize(
                &ctx.acl,
                &AuthSubject::new("bob"),
                AclOp::Read,
                "T"
            ));
        }
    }
}
