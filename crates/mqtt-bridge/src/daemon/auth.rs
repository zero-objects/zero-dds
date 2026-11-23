// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! How the bridge authenticates towards the broker (Spec §7.2).
//!
//! * `none`: an anonymous CONNECT, only to a broker on the loopback
//!   interface.
//! * `password` (`bearer`, `sasl`, `sasl_plain` are its other names): the
//!   CONNECT carries User Name and Password.
//! * `mtls`: the TLS client certificate; the CONNECT carries neither.
//! * `enhanced`: MQTT 5.0 enhanced authentication (§4.12) with
//!   Authentication Method and Authentication Data: SCRAM-SHA-256
//!   (RFC 7677), OAUTHBEARER (RFC 7628), or another method whose
//!   Authentication Data is `auth.bearer_token` (a JWT, for example).
//!
//! Without `auth.mode` the mode follows from what is configured: `enhanced`
//! with `auth.enhanced.method`, else `password` with a user name, password
//! or token, else `mtls` with a client certificate, else `none`.

use std::string::{String, ToString};
use std::vec::Vec;

use zerodds_bridge_security::{
    OAUTHBEARER, OAUTHBEARER_ERROR_ACK, SCRAM_SHA_256, ScramClient, oauthbearer_initial_response,
};

use super::config::DaemonConfig;
use crate::version::ProtocolVersion;

/// `auth.mode` (Spec §7.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthKind {
    /// Anonymous, to a loopback broker only.
    None,
    /// User Name and Password in the CONNECT.
    Password,
    /// The TLS client certificate.
    Mtls,
    /// MQTT 5.0 enhanced authentication.
    Enhanced,
}

impl AuthKind {
    /// The values `auth.mode` takes.
    pub const NAMES: [&'static str; 7] = [
        "none",
        "password",
        "bearer",
        "sasl",
        "sasl_plain",
        "mtls",
        "enhanced",
    ];

    /// The mode of an `auth.mode` value; `bearer`, `sasl` and `sasl_plain`
    /// are `password`.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "none" => Some(Self::None),
            "password" | "bearer" | "sasl" | "sasl_plain" => Some(Self::Password),
            "mtls" => Some(Self::Mtls),
            "enhanced" => Some(Self::Enhanced),
            _ => None,
        }
    }

    /// Its name.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Password => "password",
            Self::Mtls => "mtls",
            Self::Enhanced => "enhanced",
        }
    }
}

/// What a configuration gives for the CONNECT and the enhanced exchange.
#[derive(Debug, Clone, Copy)]
pub struct Credentials<'a> {
    cfg: &'a DaemonConfig,
}

impl<'a> Credentials<'a> {
    /// The credentials of `cfg`.
    #[must_use]
    pub fn of(cfg: &'a DaemonConfig) -> Self {
        Self { cfg }
    }

    /// `auth.mode`, or the mode that follows from what is configured.
    #[must_use]
    pub fn kind(&self) -> AuthKind {
        if let Some(k) = self.cfg.auth_mode {
            return k;
        }
        if self.cfg.auth_enhanced_method.is_some() {
            AuthKind::Enhanced
        } else if self.user().is_some()
            || self.password().is_some()
            || self.cfg.auth_bearer_token.is_some()
        {
            AuthKind::Password
        } else if self.cfg.broker_tls_enabled && !self.cfg.broker_tls_client_cert_file.is_empty() {
            AuthKind::Mtls
        } else {
            AuthKind::None
        }
    }

    /// `auth.username`, else `mqtt.username`.
    #[must_use]
    pub fn user(&self) -> Option<&'a str> {
        self.cfg
            .outbound_username
            .as_deref()
            .or(self.cfg.username.as_deref())
    }

    /// `auth.password`, else `mqtt.password`.
    #[must_use]
    pub fn password(&self) -> Option<&'a str> {
        self.cfg
            .outbound_password
            .as_deref()
            .or(self.cfg.password.as_deref())
    }

    /// The user name the bridge authenticates with: the CONNECT's under
    /// `password`, the SCRAM user's under `enhanced`.
    #[must_use]
    pub fn user_name(&self) -> Option<&'a str> {
        match self.kind() {
            AuthKind::Password => self.user(),
            AuthKind::Enhanced if self.method() == Some(SCRAM_SHA_256) => self.user(),
            _ => None,
        }
    }

    /// The User Name of the CONNECT (`password` only).
    #[must_use]
    pub fn connect_user_name(&self) -> Option<String> {
        (self.kind() == AuthKind::Password)
            .then(|| self.user().map(ToString::to_string))
            .flatten()
    }

    /// The Password of the CONNECT (`password` only): `auth.password`,
    /// else `auth.bearer_token`, else `mqtt.password`.
    #[must_use]
    pub fn connect_password(&self) -> Option<Vec<u8>> {
        if self.kind() != AuthKind::Password {
            return None;
        }
        self.cfg
            .outbound_password
            .as_deref()
            .or(self.cfg.auth_bearer_token.as_deref())
            .or(self.cfg.password.as_deref())
            .map(|p| p.as_bytes().to_vec())
    }

    /// `auth.enhanced.method` under `enhanced`.
    #[must_use]
    pub fn method(&self) -> Option<&'a str> {
        self.cfg.auth_enhanced_method.as_deref()
    }

    /// The token the bridge presents, `auth.bearer_token`: the CONNECT
    /// password under `password` (unless `auth.password` is set), the
    /// OAUTHBEARER token or the Authentication Data of another method under
    /// `enhanced`.
    #[must_use]
    pub fn token(&self) -> Option<&'a str> {
        match self.kind() {
            AuthKind::Password => self
                .cfg
                .outbound_password
                .is_none()
                .then_some(self.cfg.auth_bearer_token.as_deref())
                .flatten(),
            AuthKind::Enhanced => match self.method() {
                Some(SCRAM_SHA_256) | None => None,
                Some(_) => self.cfg.auth_bearer_token.as_deref(),
            },
            _ => None,
        }
    }
}

/// Whether `host` is the loopback interface: `localhost`, an address of
/// 127.0.0.0/8, `::1`.
#[must_use]
pub fn is_loopback_host(host: &str) -> bool {
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    host.parse::<std::net::IpAddr>()
        .is_ok_and(|ip| ip.is_loopback())
}

/// Checks that the auth settings of `cfg` fit together (Spec §7.2); the
/// message names what is wrong. The start refuses a configuration that
/// fails (exit 1).
///
/// # Errors
/// What does not fit, as a sentence.
pub fn check(cfg: &DaemonConfig) -> Result<(), String> {
    let c = Credentials::of(cfg);
    let kind = c.kind();
    let given = |what: &mut Vec<&'static str>| {
        if cfg.username.is_some() {
            what.push("mqtt.username");
        }
        if cfg.password.is_some() {
            what.push("mqtt.password");
        }
        if cfg.outbound_username.is_some() {
            what.push("auth.username");
        }
        if cfg.outbound_password.is_some() {
            what.push("auth.password");
        }
        if cfg.auth_bearer_token.is_some() {
            what.push("auth.bearer_token");
        }
    };
    if kind != AuthKind::Enhanced && cfg.auth_enhanced_method.is_some() {
        return Err(format!(
            "auth.enhanced is set, and auth.mode is {}; it needs auth.mode: enhanced",
            kind.as_str()
        ));
    }
    match kind {
        AuthKind::None => {
            let mut what = Vec::new();
            given(&mut what);
            if cfg.broker_tls_enabled && !cfg.broker_tls_client_cert_file.is_empty() {
                what.push("mqtt.tls.cert_file");
            }
            if !what.is_empty() {
                return Err(format!(
                    "auth.mode: none connects anonymously, and {} is set",
                    what.join(", ")
                ));
            }
            let url =
                super::config::parse_broker_url(&cfg.broker_url).map_err(|e| format!("{e}"))?;
            if !is_loopback_host(&url.host) {
                return Err(format!(
                    "auth.mode: none (anonymous) is only for a broker on the loopback interface \
                     (localhost, 127.0.0.0/8, ::1); {} is not; set the credentials of the broker",
                    url.host
                ));
            }
        }
        AuthKind::Password => {
            if c.connect_user_name().is_none() && c.connect_password().is_none() {
                return Err(
                    "auth.mode: password needs a user name or a password (mqtt.username, \
                     mqtt.password, auth.username, auth.password, auth.bearer_token)"
                        .to_string(),
                );
            }
            if cfg.outbound_password.is_some() && cfg.auth_bearer_token.is_some() {
                return Err(
                    "auth.password and auth.bearer_token are both the CONNECT password; set one"
                        .to_string(),
                );
            }
            if cfg.protocol_version == ProtocolVersion::V311 && c.connect_user_name().is_none() {
                // MQTT 3.1.1 [MQTT-3.1.2-22]: no Password without a User Name.
                return Err(
                    "mqtt.protocol_version 3.1.1 carries a password only with a user name \
                     (MQTT 3.1.1 §3.1.2.9); set mqtt.username or auth.username"
                        .to_string(),
                );
            }
        }
        AuthKind::Mtls => {
            if !cfg.broker_tls_enabled
                || cfg.broker_tls_client_cert_file.is_empty()
                || cfg.broker_tls_client_key_file.is_empty()
            {
                return Err(
                    "auth.mode: mtls needs mqtt.tls.enabled: true with cert_file and key_file"
                        .to_string(),
                );
            }
            let mut what = Vec::new();
            given(&mut what);
            if !what.is_empty() {
                return Err(format!(
                    "auth.mode: mtls sends no user name and no password, and {} is set",
                    what.join(", ")
                ));
            }
        }
        AuthKind::Enhanced => check_enhanced(cfg, &c)?,
    }
    Ok(())
}

/// The part of [`check`] for `enhanced`.
fn check_enhanced(cfg: &DaemonConfig, c: &Credentials<'_>) -> Result<(), String> {
    if cfg.protocol_version != ProtocolVersion::V5 {
        return Err(
            "auth.mode: enhanced is MQTT 5.0 (§4.12); mqtt.protocol_version is 3.1.1".into(),
        );
    }
    let Some(method) = c.method() else {
        return Err("auth.mode: enhanced needs auth.enhanced.method".into());
    };
    if method.is_empty() {
        return Err("auth.enhanced.method is empty".into());
    }
    let refuse_set = |names: &[(&'static str, bool)]| -> Result<(), String> {
        let set: Vec<&str> = names.iter().filter(|(_, s)| *s).map(|(n, _)| *n).collect();
        if set.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "auth.enhanced.method {method} does not use {}",
                set.join(", ")
            ))
        }
    };
    match method {
        SCRAM_SHA_256 => {
            let (Some(user), Some(pass)) = (c.user(), c.password()) else {
                return Err(format!(
                    "auth.enhanced.method {SCRAM_SHA_256} needs a user name and a password \
                     (auth.username/auth.password or mqtt.username/mqtt.password)"
                ));
            };
            ScramClient::with_nonce(user, pass, "n")
                .map_err(|e| format!("{SCRAM_SHA_256}: {e}"))?;
            refuse_set(&[("auth.bearer_token", cfg.auth_bearer_token.is_some())])
        }
        m if m.starts_with("SCRAM-") => Err(format!(
            "auth.enhanced.method {m} is not supported; the SCRAM method is {SCRAM_SHA_256}"
        )),
        _ => {
            // OAUTHBEARER and every other method: the token.
            if cfg.auth_bearer_token.is_none() {
                return Err(format!(
                    "auth.enhanced.method {method} needs auth.bearer_token"
                ));
            }
            refuse_set(&[
                ("mqtt.username", cfg.username.is_some()),
                ("mqtt.password", cfg.password.is_some()),
                ("auth.username", cfg.outbound_username.is_some()),
                ("auth.password", cfg.outbound_password.is_some()),
            ])
        }
    }
}

/// What fails on the client's side of an enhanced exchange.
pub type ExchangeError = String;

/// The client side of one enhanced authentication exchange (MQTT 5.0
/// §4.12): the data of the CONNECT (or of the AUTH 0x19 of a
/// re-authentication), the answer to each AUTH 0x18 of the broker, the check
/// of the data of the CONNACK (or of the AUTH 0x00).
#[derive(Debug, Clone)]
pub enum Exchange {
    /// SCRAM-SHA-256: client-first, client-final, server-final verified.
    Scram(ScramClient),
    /// OAUTHBEARER: the initial response; an error challenge is answered
    /// once with `%x01` (RFC 7628 §3.2.3).
    OAuthBearer {
        /// The initial client response.
        initial: Vec<u8>,
        /// Whether an error challenge came.
        error: Option<String>,
    },
    /// Another method: the token as one message.
    Single {
        /// The method.
        method: String,
        /// `auth.bearer_token`.
        data: Vec<u8>,
    },
}

impl Exchange {
    /// A new exchange for `cfg`, `None` when its mode is not `enhanced`.
    /// `host` and `port` are the broker's (OAUTHBEARER sends them).
    ///
    /// # Errors
    /// The SCRAM user name or password does not prepare, or the random
    /// generator fails.
    pub fn of(cfg: &DaemonConfig, host: &str, port: u16) -> Result<Option<Self>, ExchangeError> {
        let c = Credentials::of(cfg);
        if c.kind() != AuthKind::Enhanced {
            return Ok(None);
        }
        Ok(match c.method() {
            Some(SCRAM_SHA_256) => {
                let (user, pass) = (c.user().unwrap_or(""), c.password().unwrap_or(""));
                Some(Self::Scram(
                    ScramClient::new(user, pass).map_err(|e| format!("{SCRAM_SHA_256}: {e}"))?,
                ))
            }
            Some(OAUTHBEARER) => Some(Self::OAuthBearer {
                initial: oauthbearer_initial_response(
                    cfg.auth_bearer_token.as_deref().unwrap_or(""),
                    Some(host),
                    Some(port),
                ),
                error: None,
            }),
            Some(m) => Some(Self::Single {
                method: m.to_string(),
                data: cfg
                    .auth_bearer_token
                    .as_deref()
                    .unwrap_or("")
                    .as_bytes()
                    .to_vec(),
            }),
            None => None,
        })
    }

    /// The Authentication Method.
    #[must_use]
    pub fn method(&self) -> &str {
        match self {
            Self::Scram(_) => SCRAM_SHA_256,
            Self::OAuthBearer { .. } => OAUTHBEARER,
            Self::Single { method, .. } => method,
        }
    }

    /// The Authentication Data of the CONNECT or of the AUTH 0x19.
    #[must_use]
    pub fn initial(&self) -> Vec<u8> {
        match self {
            Self::Scram(s) => s.client_first().into_bytes(),
            Self::OAuthBearer { initial, .. } => initial.clone(),
            Self::Single { data, .. } => data.clone(),
        }
    }

    /// The Authentication Data of the AUTH 0x18 that answers the broker's
    /// AUTH 0x18 with `data`.
    ///
    /// # Errors
    /// The broker's data does not continue the method: a malformed SCRAM
    /// server-first-message, a second OAUTHBEARER challenge, any challenge
    /// to a method of one message.
    pub fn answer(&mut self, data: Option<&[u8]>) -> Result<Vec<u8>, ExchangeError> {
        match self {
            Self::Scram(s) => {
                let text = core::str::from_utf8(data.unwrap_or_default())
                    .map_err(|_| "the SCRAM server-first-message is no UTF-8".to_string())?;
                s.client_final(text)
                    .map(String::into_bytes)
                    .map_err(|e| format!("{SCRAM_SHA_256}: {e}"))
            }
            Self::OAuthBearer { error, .. } => {
                if error.is_some() {
                    return Err(format!("{OAUTHBEARER}: a second challenge"));
                }
                // RFC 7628 §3.2.2: the challenge is the server's error status
                // as JSON.
                *error = Some(String::from_utf8_lossy(data.unwrap_or_default()).into_owned());
                Ok(OAUTHBEARER_ERROR_ACK.to_vec())
            }
            Self::Single { method, .. } => Err(format!(
                "{method}: the broker continues the exchange, and the method has one message \
                 (auth.bearer_token)"
            )),
        }
    }

    /// Checks the Authentication Data that ends the exchange (the CONNACK
    /// or the AUTH 0x00).
    ///
    /// # Errors
    /// SCRAM: no server-final-message, or one whose signature does not
    /// verify (the broker does not know the password).
    pub fn finish(&mut self, data: Option<&[u8]>) -> Result<(), ExchangeError> {
        match self {
            Self::Scram(s) => {
                let text = core::str::from_utf8(data.unwrap_or_default())
                    .map_err(|_| "the SCRAM server-final-message is no UTF-8".to_string())?;
                s.verify_server_final(text)
                    .map_err(|e| format!("{SCRAM_SHA_256}: {e}"))
            }
            Self::OAuthBearer { error: Some(e), .. } => Err(format!(
                "{OAUTHBEARER}: the broker reported {e} and then accepted"
            )),
            _ => Ok(()),
        }
    }

    /// The error status an OAUTHBEARER challenge reported, if one came.
    #[must_use]
    pub fn reported_error(&self) -> Option<&str> {
        match self {
            Self::OAuthBearer { error, .. } => error.as_deref(),
            _ => None,
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn cfg(f: impl FnOnce(&mut DaemonConfig)) -> DaemonConfig {
        let mut c = DaemonConfig::default_for_dev();
        c.broker_url = "mqtt://127.0.0.1:1883".into();
        f(&mut c);
        c
    }

    #[test]
    fn mqttbridge_7_2_none_is_refused_for_a_broker_that_is_not_loopback() {
        for host in ["127.0.0.1", "127.8.9.1", "[::1]", "localhost", "LOCALHOST"] {
            let c = cfg(|c| c.broker_url = format!("mqtt://{host}:1883"));
            assert_eq!(Credentials::of(&c).kind(), AuthKind::None);
            check(&c).unwrap_or_else(|e| panic!("{host}: {e}"));
        }
        for host in [
            "10.0.0.1",
            "broker.example",
            "[2001:db8::1]",
            "localhost.example",
        ] {
            let c = cfg(|c| c.broker_url = format!("mqtt://{host}:1883"));
            let e = check(&c).unwrap_err();
            assert!(e.contains("loopback"), "{host}: {e}");
        }
        let explicit = cfg(|c| {
            c.auth_mode = Some(AuthKind::None);
            c.username = Some("u".into());
        });
        assert!(check(&explicit).unwrap_err().contains("mqtt.username"));
    }

    /// Without `auth.mode` the mode follows from what is set; the old names
    /// of `password` are read as it.
    #[test]
    fn mqttbridge_7_2_the_mode_follows_from_what_is_configured() {
        assert_eq!(
            Credentials::of(&cfg(|c| c.username = Some("u".into()))).kind(),
            AuthKind::Password
        );
        assert_eq!(
            Credentials::of(&cfg(|c| c.auth_bearer_token = Some("t".into()))).kind(),
            AuthKind::Password
        );
        assert_eq!(
            Credentials::of(&cfg(|c| {
                c.broker_tls_enabled = true;
                c.broker_tls_client_cert_file = "/c.pem".into();
            }))
            .kind(),
            AuthKind::Mtls
        );
        assert_eq!(
            Credentials::of(&cfg(|c| c.auth_enhanced_method = Some("OAUTHBEARER".into()))).kind(),
            AuthKind::Enhanced
        );
        for name in ["password", "bearer", "sasl", "sasl_plain"] {
            assert_eq!(AuthKind::parse(name), Some(AuthKind::Password), "{name}");
        }
        assert_eq!(AuthKind::parse("token"), None);
    }

    /// `password`: the CONNECT carries the user name and the password, the
    /// token as the password; MQTT 3.1.1 has no password without user name.
    #[test]
    fn mqttbridge_7_2_password_puts_user_name_and_password_into_the_connect() {
        let c = cfg(|c| {
            c.username = Some("top".into());
            c.outbound_username = Some("alice".into());
            c.password = Some("pw0".into());
        });
        let k = Credentials::of(&c);
        assert_eq!(k.connect_user_name().as_deref(), Some("alice"));
        assert_eq!(k.connect_password().as_deref(), Some(b"pw0".as_ref()));
        let bearer = cfg(|c| {
            c.auth_mode = AuthKind::parse("bearer");
            c.auth_bearer_token = Some("tok".into());
        });
        let k = Credentials::of(&bearer);
        assert_eq!(k.connect_user_name(), None);
        assert_eq!(k.connect_password().as_deref(), Some(b"tok".as_ref()));
        assert_eq!(k.token(), Some("tok"));
        check(&bearer).unwrap();
        let mut v311 = bearer.clone();
        v311.protocol_version = ProtocolVersion::V311;
        assert!(check(&v311).unwrap_err().contains("3.1.1"));
        let empty = cfg(|c| c.auth_mode = Some(AuthKind::Password));
        assert!(check(&empty).unwrap_err().contains("needs a user name"));
        let both = cfg(|c| {
            c.outbound_password = Some("p".into());
            c.auth_bearer_token = Some("t".into());
        });
        assert!(check(&both).is_err());
    }

    #[test]
    fn mqttbridge_7_2_mtls_sends_no_user_name_and_no_password() {
        let ok = cfg(|c| {
            c.auth_mode = Some(AuthKind::Mtls);
            c.broker_url = "mqtts://broker.example:8883".into();
            c.broker_tls_enabled = true;
            c.broker_tls_client_cert_file = "/c.pem".into();
            c.broker_tls_client_key_file = "/k.pem".into();
        });
        check(&ok).unwrap();
        let k = Credentials::of(&ok);
        assert_eq!((k.connect_user_name(), k.connect_password()), (None, None));
        let mut with_user = ok.clone();
        with_user.username = Some("u".into());
        assert!(check(&with_user).unwrap_err().contains("mqtt.username"));
        let mut no_key = ok;
        no_key.broker_tls_client_key_file.clear();
        assert!(check(&no_key).unwrap_err().contains("key_file"));
    }

    #[test]
    fn mqttbridge_7_2_enhanced_needs_the_input_of_its_method() {
        let scram = cfg(|c| {
            c.auth_enhanced_method = Some("SCRAM-SHA-256".into());
            c.outbound_username = Some("bridge".into());
            c.outbound_password = Some("pw".into());
        });
        check(&scram).unwrap();
        let k = Credentials::of(&scram);
        assert_eq!((k.connect_user_name(), k.connect_password()), (None, None));
        assert_eq!(k.user_name(), Some("bridge"));
        let mut no_pw = scram.clone();
        no_pw.outbound_password = None;
        assert!(check(&no_pw).is_err());
        let mut v311 = scram;
        v311.protocol_version = ProtocolVersion::V311;
        assert!(check(&v311).unwrap_err().contains("MQTT 5.0"));
        let sha1 = cfg(|c| {
            c.auth_enhanced_method = Some("SCRAM-SHA-1".into());
            c.outbound_username = Some("u".into());
            c.outbound_password = Some("p".into());
        });
        assert!(check(&sha1).unwrap_err().contains("not supported"));
        let oauth = cfg(|c| c.auth_enhanced_method = Some("OAUTHBEARER".into()));
        assert!(check(&oauth).unwrap_err().contains("bearer_token"));
        let other = cfg(|c| {
            c.auth_enhanced_method = Some("JWT".into());
            c.auth_bearer_token = Some("eyJ.eyJ.sig".into());
        });
        check(&other).unwrap();
        assert_eq!(Credentials::of(&other).token(), Some("eyJ.eyJ.sig"));
        let mut with_user = other;
        with_user.username = Some("u".into());
        assert!(check(&with_user).unwrap_err().contains("mqtt.username"));
        let stray = cfg(|c| {
            c.auth_mode = Some(AuthKind::Password);
            c.username = Some("u".into());
            c.auth_enhanced_method = Some("JWT".into());
        });
        assert!(check(&stray).unwrap_err().contains("auth.mode: enhanced"));
    }

    /// The exchange of each method: SCRAM continues with the
    /// client-final-message, a method of one message refuses a challenge,
    /// OAUTHBEARER answers an error challenge with `%x01` and fails after.
    #[test]
    fn mqttbridge_7_2_the_exchange_of_each_method() {
        let scram = cfg(|c| {
            c.auth_enhanced_method = Some("SCRAM-SHA-256".into());
            c.outbound_username = Some("user".into());
            c.outbound_password = Some("pencil".into());
        });
        let mut x = Exchange::of(&scram, "h", 1).unwrap().unwrap();
        assert_eq!(x.method(), "SCRAM-SHA-256");
        let first = String::from_utf8(x.initial()).unwrap();
        assert!(first.starts_with("n,,n=user,r="), "{first}");
        assert!(x.answer(Some(b"r=other,s=AAAA,i=4096")).is_err());
        let oauth = cfg(|c| {
            c.auth_enhanced_method = Some("OAUTHBEARER".into());
            c.auth_bearer_token = Some("tok".into());
        });
        let mut x = Exchange::of(&oauth, "broker", 1883).unwrap().unwrap();
        assert_eq!(
            x.initial(),
            b"n,,\x01host=broker\x01port=1883\x01auth=Bearer tok\x01\x01".to_vec()
        );
        assert_eq!(
            x.answer(Some(br#"{"status":"invalid_token"}"#)).unwrap(),
            b"\x01"
        );
        assert_eq!(x.reported_error(), Some(r#"{"status":"invalid_token"}"#));
        assert!(x.answer(Some(b"{}")).is_err());
        assert!(x.finish(None).is_err());
        let single = cfg(|c| {
            c.auth_enhanced_method = Some("JWT".into());
            c.auth_bearer_token = Some("token".into());
        });
        let mut x = Exchange::of(&single, "h", 1).unwrap().unwrap();
        assert_eq!(x.initial(), b"token".to_vec());
        assert!(x.answer(Some(b"more")).is_err());
        x.finish(None).unwrap();
        assert!(Exchange::of(&cfg(|_| {}), "h", 1).unwrap().is_none());
    }
}
