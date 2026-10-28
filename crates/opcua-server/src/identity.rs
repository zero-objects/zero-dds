// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Part 4 §7.40 UserIdentityTokens, the §7.41 `UserTokenPolicy` a Server
//! publishes in its `EndpointDescription`, and the server-side [`UserStore`]
//! that decides whether an ActivateSession token is accepted.
//!
//! All four token types of Table 181 are carried: `AnonymousIdentityToken`,
//! `UserNameIdentityToken`, `X509IdentityToken` and `IssuedIdentityToken`.

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_opcua_gateway::data_value::{ExtensionObject, ExtensionObjectBody};
use zerodds_opcua_gateway::node_id::NodeId;
use zerodds_opcua_pubsub::{DecodeError, EncodeError, UaReader, UaWriter};

use crate::status;
use crate::wire::{read_byte_string, read_string, write_byte_string, write_string};

/// `UserTokenPolicy` (Part 4 §7.41, Table 192) — one identity token a Server
/// declares it accepts. Re-exported from the shared UA datatypes so the
/// policies a [`UserStore`] publishes drop straight into an
/// `EndpointDescription`.
pub use zerodds_opcua_pubsub::uadp::datatypes::{UserTokenPolicy, UserTokenType};

/// A [`UserTokenPolicy`] accepting `token_type` under `policy_id`, with no
/// issued-token type, no issuer endpoint and the SecureChannel's
/// SecurityPolicy (Part 4 §7.41: "The security policy for the SecureChannel is
/// used if this value is null or empty").
#[must_use]
pub fn user_token_policy(policy_id: &str, token_type: UserTokenType) -> UserTokenPolicy {
    UserTokenPolicy {
        policy_id: String::from(policy_id),
        token_type,
        issued_token_type: String::new(),
        issuer_endpoint_url: String::new(),
        security_policy_uri: String::new(),
    }
}

/// The `_Encoding_DefaultBinary` NodeIds of the identity types (ns0, from the
/// normative `Opc.Ua.NodeIds` table).
pub mod token_ids {
    use zerodds_opcua_gateway::node_id::NodeId;
    /// `UserTokenPolicy_Encoding_DefaultBinary`.
    pub const USER_TOKEN_POLICY: NodeId = NodeId::numeric(0, 306);
    /// `AnonymousIdentityToken_Encoding_DefaultBinary`.
    pub const ANONYMOUS: NodeId = NodeId::numeric(0, 321);
    /// `UserNameIdentityToken_Encoding_DefaultBinary`.
    pub const USER_NAME: NodeId = NodeId::numeric(0, 324);
    /// `X509IdentityToken_Encoding_DefaultBinary`.
    pub const X509: NodeId = NodeId::numeric(0, 327);
    /// `IssuedIdentityToken_Encoding_DefaultBinary`.
    pub const ISSUED: NodeId = NodeId::numeric(0, 940);
}

/// A decoded `UserIdentityToken` (Part 4 §7.40, Table 181).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserIdentityToken {
    /// `AnonymousIdentityToken` (§7.40.3). Part 4 §5.7.3.2: "Null or empty
    /// user token shall always be interpreted as anonymous", so the null
    /// ExtensionObject decodes to this with an empty `policy_id`.
    Anonymous {
        /// `policyId`.
        policy_id: String,
    },
    /// `UserNameIdentityToken` (§7.40.4).
    UserName {
        /// `policyId`.
        policy_id: String,
        /// `userName`.
        user_name: String,
        /// `password` — plain UTF-8 under SecurityPolicy `None`, otherwise the
        /// encrypted form of §7.40.2.
        password: Vec<u8>,
        /// `encryptionAlgorithm` — empty if the password is not encrypted.
        encryption_algorithm: String,
    },
    /// `X509IdentityToken` (§7.40.5).
    X509 {
        /// `policyId`.
        policy_id: String,
        /// `certificateData` — the X.509 v3 certificate in DER form.
        certificate_data: Vec<u8>,
    },
    /// `IssuedIdentityToken` (§7.40.6).
    Issued {
        /// `policyId`.
        policy_id: String,
        /// `tokenData`.
        token_data: Vec<u8>,
        /// `encryptionAlgorithm`.
        encryption_algorithm: String,
    },
}

impl UserIdentityToken {
    /// The anonymous token with an empty policy id.
    #[must_use]
    pub fn anonymous() -> Self {
        Self::Anonymous {
            policy_id: String::new(),
        }
    }

    /// A username/password token with a plaintext UTF-8 password (Part 4
    /// §7.40.4: "If the SecurityPolicy is None then the password only contains
    /// the UTF-8 encoded password").
    #[must_use]
    pub fn user_name(policy_id: &str, user: &str, password: &str) -> Self {
        Self::UserName {
            policy_id: String::from(policy_id),
            user_name: String::from(user),
            password: password.as_bytes().to_vec(),
            encryption_algorithm: String::new(),
        }
    }

    /// An X.509 token carrying a DER certificate.
    #[must_use]
    pub fn x509(policy_id: &str, certificate_data: Vec<u8>) -> Self {
        Self::X509 {
            policy_id: String::from(policy_id),
            certificate_data,
        }
    }

    /// An issued token (e.g. a JWT) carrying its raw token data.
    #[must_use]
    pub fn issued(policy_id: &str, token_data: Vec<u8>) -> Self {
        Self::Issued {
            policy_id: String::from(policy_id),
            token_data,
            encryption_algorithm: String::new(),
        }
    }

    /// The `UserTokenType` this token belongs to.
    #[must_use]
    pub const fn token_type(&self) -> UserTokenType {
        match self {
            Self::Anonymous { .. } => UserTokenType::Anonymous,
            Self::UserName { .. } => UserTokenType::UserName,
            Self::X509 { .. } => UserTokenType::Certificate,
            Self::Issued { .. } => UserTokenType::IssuedToken,
        }
    }

    /// The `policyId` the token claims to conform to.
    #[must_use]
    pub fn policy_id(&self) -> &str {
        match self {
            Self::Anonymous { policy_id }
            | Self::UserName { policy_id, .. }
            | Self::X509 { policy_id, .. }
            | Self::Issued { policy_id, .. } => policy_id,
        }
    }

    /// Wraps the token in the `ExtensionObject` the ActivateSession request
    /// carries.
    ///
    /// # Errors
    /// [`EncodeError`] on an oversized field.
    pub fn to_extension_object(&self) -> Result<ExtensionObject, EncodeError> {
        let mut w = UaWriter::new();
        let type_id = match self {
            Self::Anonymous { policy_id } => {
                write_string(&mut w, policy_id)?;
                token_ids::ANONYMOUS
            }
            Self::UserName {
                policy_id,
                user_name,
                password,
                encryption_algorithm,
            } => {
                write_string(&mut w, policy_id)?;
                write_string(&mut w, user_name)?;
                write_byte_string(&mut w, password)?;
                write_string(&mut w, encryption_algorithm)?;
                token_ids::USER_NAME
            }
            Self::X509 {
                policy_id,
                certificate_data,
            } => {
                write_string(&mut w, policy_id)?;
                write_byte_string(&mut w, certificate_data)?;
                token_ids::X509
            }
            Self::Issued {
                policy_id,
                token_data,
                encryption_algorithm,
            } => {
                write_string(&mut w, policy_id)?;
                write_byte_string(&mut w, token_data)?;
                write_string(&mut w, encryption_algorithm)?;
                token_ids::ISSUED
            }
        };
        Ok(ExtensionObject {
            type_id,
            body: ExtensionObjectBody::ByteString(w.into_vec()),
        })
    }

    /// Decodes the token an ActivateSession request carried.
    ///
    /// # Errors
    /// [`DecodeError`] on truncation or an ExtensionObject whose `type_id` is
    /// not one of the four Table 181 tokens.
    pub fn from_extension_object(eo: &ExtensionObject) -> Result<Self, DecodeError> {
        // Part 4 §5.7.3.2: "Null or empty user token shall always be
        // interpreted as anonymous."
        let ExtensionObjectBody::ByteString(bytes) = &eo.body else {
            return Ok(Self::anonymous());
        };
        if bytes.is_empty() && eo.type_id == NodeId::numeric(0, 0) {
            return Ok(Self::anonymous());
        }
        let mut r = UaReader::new(bytes);
        if eo.type_id == token_ids::ANONYMOUS {
            Ok(Self::Anonymous {
                policy_id: read_string(&mut r)?,
            })
        } else if eo.type_id == token_ids::USER_NAME {
            Ok(Self::UserName {
                policy_id: read_string(&mut r)?,
                user_name: read_string(&mut r)?,
                password: read_byte_string(&mut r)?,
                encryption_algorithm: read_string(&mut r)?,
            })
        } else if eo.type_id == token_ids::X509 {
            Ok(Self::X509 {
                policy_id: read_string(&mut r)?,
                certificate_data: read_byte_string(&mut r)?,
            })
        } else if eo.type_id == token_ids::ISSUED {
            Ok(Self::Issued {
                policy_id: read_string(&mut r)?,
                token_data: read_byte_string(&mut r)?,
                encryption_algorithm: read_string(&mut r)?,
            })
        } else {
            Err(DecodeError::MalformedMessage {
                message: "unsupported UserIdentityToken type",
            })
        }
    }
}

/// One credential the [`UserStore`] accepts.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Credential {
    UserName { user: String, password: Vec<u8> },
    Certificate(Vec<u8>),
    IssuedToken(Vec<u8>),
}

/// The Server's set of accepted user identities: the [`UserTokenPolicy`]s it
/// publishes in its `EndpointDescription` and the credentials each policy
/// accepts.
///
/// A store with no policies configured accepts the anonymous token only —
/// Part 4 §7.41: "A tokenType of ANONYMOUS indicates that the Server does not
/// require any user identification."
#[derive(Debug, Clone, Default)]
pub struct UserStore {
    policies: Vec<UserTokenPolicy>,
    credentials: Vec<Credential>,
    allow_anonymous: bool,
}

/// The identity a validated token established, as reported to the application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientUserId {
    /// The anonymous user (Part 5 `ClientUserId` = "Anonymous").
    Anonymous,
    /// A named user from a `UserNameIdentityToken`.
    UserName(String),
    /// The subject of an `X509IdentityToken`, identified by its DER bytes.
    Certificate(Vec<u8>),
    /// An `IssuedIdentityToken`'s raw token data.
    IssuedToken(Vec<u8>),
}

impl UserStore {
    /// A store that accepts the anonymous token under an empty `policyId`.
    #[must_use]
    pub fn anonymous_only() -> Self {
        Self {
            policies: alloc::vec![user_token_policy("anonymous", UserTokenType::Anonymous)],
            credentials: Vec::new(),
            allow_anonymous: true,
        }
    }

    /// Adds a username/password credential and, if not already present, the
    /// `USERNAME` policy it conforms to.
    pub fn add_user(&mut self, policy_id: &str, user: &str, password: &str) -> &mut Self {
        self.ensure_policy(policy_id, UserTokenType::UserName);
        self.credentials.push(Credential::UserName {
            user: String::from(user),
            password: password.as_bytes().to_vec(),
        });
        self
    }

    /// Adds a trusted user certificate (DER) and the `CERTIFICATE` policy.
    pub fn add_certificate(&mut self, policy_id: &str, der: Vec<u8>) -> &mut Self {
        self.ensure_policy(policy_id, UserTokenType::Certificate);
        self.credentials.push(Credential::Certificate(der));
        self
    }

    /// Adds an accepted issued token and the `ISSUEDTOKEN` policy.
    pub fn add_issued_token(&mut self, policy_id: &str, token_data: Vec<u8>) -> &mut Self {
        self.ensure_policy(policy_id, UserTokenType::IssuedToken);
        self.credentials.push(Credential::IssuedToken(token_data));
        self
    }

    /// Whether the anonymous token is accepted.
    pub fn set_allow_anonymous(&mut self, allow: bool) -> &mut Self {
        if allow {
            self.ensure_policy("anonymous", UserTokenType::Anonymous);
        }
        self.allow_anonymous = allow;
        self
    }

    /// The policies to publish in the `EndpointDescription` (Part 4 §7.41).
    #[must_use]
    pub fn policies(&self) -> &[UserTokenPolicy] {
        &self.policies
    }

    fn ensure_policy(&mut self, policy_id: &str, token_type: UserTokenType) {
        if !self
            .policies
            .iter()
            .any(|p| p.policy_id == policy_id && p.token_type == token_type)
        {
            self.policies.push(user_token_policy(policy_id, token_type));
        }
    }

    /// Validates an ActivateSession token (Part 4 §5.7.3).
    ///
    /// `user_token_signature_ok` reports whether the request's
    /// `userTokenSignature` verified against the token's certificate — the
    /// proof of possession §5.7.3.1 demands for an `X509IdentityToken`. It is
    /// `None` when the SecureChannel has SecurityPolicy `None`, where §7.40.5
    /// requires no signature.
    ///
    /// # Errors
    /// `Bad_IdentityTokenInvalid` when no published policy matches the token,
    /// `Bad_IdentityTokenRejected` when the anonymous token is not accepted,
    /// `Bad_UserAccessDenied` for unknown credentials, and
    /// `Bad_UserSignatureInvalid` for an X.509 token whose signature failed.
    pub fn validate(
        &self,
        token: &UserIdentityToken,
        user_token_signature_ok: Option<bool>,
    ) -> Result<ClientUserId, u32> {
        // Part 4 §7.40.3: "Servers that provide a null or empty PolicyId shall
        // accept null or empty and treat them as equal."
        let claimed = token.policy_id();
        let matching = self.policies.iter().any(|p| {
            p.token_type == token.token_type()
                && (p.policy_id == claimed || (claimed.is_empty() && p.policy_id.is_empty()))
        });
        let anonymous_default =
            claimed.is_empty() && token.token_type() == UserTokenType::Anonymous;
        if !matching && !anonymous_default {
            return Err(status::BAD_IDENTITY_TOKEN_INVALID);
        }
        match token {
            UserIdentityToken::Anonymous { .. } => {
                if self.allow_anonymous {
                    Ok(ClientUserId::Anonymous)
                } else {
                    Err(status::BAD_IDENTITY_TOKEN_REJECTED)
                }
            }
            UserIdentityToken::UserName {
                user_name,
                password,
                encryption_algorithm,
                ..
            } => {
                if !encryption_algorithm.is_empty() {
                    // §7.40.2 defines the encrypted secret formats; this stack
                    // publishes no UserTokenPolicy that asks for one, so a
                    // token that claims encryption cannot be checked and is
                    // rejected rather than silently accepted.
                    return Err(status::BAD_IDENTITY_TOKEN_INVALID);
                }
                let ok = self.credentials.iter().any(|c| {
                    matches!(c, Credential::UserName { user, password: p }
                        if user == user_name && p == password)
                });
                if ok {
                    Ok(ClientUserId::UserName(user_name.clone()))
                } else {
                    Err(status::BAD_USER_ACCESS_DENIED)
                }
            }
            UserIdentityToken::X509 {
                certificate_data, ..
            } => {
                if user_token_signature_ok == Some(false) {
                    return Err(status::BAD_USER_SIGNATURE_INVALID);
                }
                let ok = self
                    .credentials
                    .iter()
                    .any(|c| matches!(c, Credential::Certificate(der) if der == certificate_data));
                if ok {
                    Ok(ClientUserId::Certificate(certificate_data.clone()))
                } else {
                    Err(status::BAD_USER_ACCESS_DENIED)
                }
            }
            UserIdentityToken::Issued {
                token_data,
                encryption_algorithm,
                ..
            } => {
                if !encryption_algorithm.is_empty() {
                    // §7.40.6: "The Client shall set this field to null or
                    // empty and Servers shall ignore any value specified."
                    return Err(status::BAD_IDENTITY_TOKEN_INVALID);
                }
                let ok = self
                    .credentials
                    .iter()
                    .any(|c| matches!(c, Credential::IssuedToken(d) if d == token_data));
                if ok {
                    Ok(ClientUserId::IssuedToken(token_data.clone()))
                } else {
                    Err(status::BAD_IDENTITY_TOKEN_REJECTED)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_four_table_181_tokens_round_trip_through_the_extension_object() {
        let tokens = alloc::vec![
            UserIdentityToken::anonymous(),
            UserIdentityToken::user_name("user", "alice", "s3cret"),
            UserIdentityToken::x509("cert", alloc::vec![0x30, 0x82, 0x01]),
            UserIdentityToken::issued("jwt", b"eyJhbGciOiJIUzI1NiJ9".to_vec()),
        ];
        for t in tokens {
            let eo = t.to_extension_object().expect("encode");
            assert_eq!(
                UserIdentityToken::from_extension_object(&eo).expect("decode"),
                t
            );
        }
    }

    #[test]
    fn null_token_is_anonymous() {
        // Part 4 §5.7.3.2 — "Null or empty user token shall always be
        // interpreted as anonymous."
        let null = ExtensionObject {
            type_id: NodeId::numeric(0, 0),
            body: ExtensionObjectBody::None,
        };
        assert_eq!(
            UserIdentityToken::from_extension_object(&null).expect("decode"),
            UserIdentityToken::anonymous()
        );
    }

    #[test]
    fn user_token_policy_round_trips() {
        let p = UserTokenPolicy {
            policy_id: String::from("user"),
            token_type: UserTokenType::UserName,
            issued_token_type: String::new(),
            issuer_endpoint_url: String::from("https://auth.example/"),
            security_policy_uri: String::from("http://opcfoundation.org/UA/SecurityPolicy#None"),
        };
        use zerodds_opcua_pubsub::{UaDecode, UaEncode};
        let mut w = UaWriter::new();
        p.encode(&mut w).expect("encode");
        let mut r = UaReader::new(w.as_slice());
        assert_eq!(UserTokenPolicy::decode(&mut r).expect("decode"), p);
    }

    #[test]
    fn store_accepts_each_token_type_and_rejects_bad_credentials() {
        let mut store = UserStore::anonymous_only();
        store
            .add_user("user", "alice", "s3cret")
            .add_certificate("cert", alloc::vec![1, 2, 3])
            .add_issued_token("jwt", b"token".to_vec());
        assert_eq!(store.policies().len(), 4);

        assert_eq!(
            store.validate(&UserIdentityToken::anonymous(), None),
            Ok(ClientUserId::Anonymous)
        );
        assert_eq!(
            store.validate(
                &UserIdentityToken::user_name("user", "alice", "s3cret"),
                None
            ),
            Ok(ClientUserId::UserName(String::from("alice")))
        );
        assert_eq!(
            store.validate(
                &UserIdentityToken::user_name("user", "alice", "wrong"),
                None
            ),
            Err(status::BAD_USER_ACCESS_DENIED)
        );
        assert_eq!(
            store.validate(
                &UserIdentityToken::x509("cert", alloc::vec![1, 2, 3]),
                Some(true)
            ),
            Ok(ClientUserId::Certificate(alloc::vec![1, 2, 3]))
        );
        // Part 4 §5.7.3.1 — the X.509 proof of possession is the signature.
        assert_eq!(
            store.validate(
                &UserIdentityToken::x509("cert", alloc::vec![1, 2, 3]),
                Some(false)
            ),
            Err(status::BAD_USER_SIGNATURE_INVALID)
        );
        assert_eq!(
            store.validate(&UserIdentityToken::issued("jwt", b"token".to_vec()), None),
            Ok(ClientUserId::IssuedToken(b"token".to_vec()))
        );
        assert_eq!(
            store.validate(&UserIdentityToken::issued("jwt", b"forged".to_vec()), None),
            Err(status::BAD_IDENTITY_TOKEN_REJECTED)
        );
        // A policyId the Server never published (Part 4 §7.41).
        assert_eq!(
            store.validate(
                &UserIdentityToken::user_name("nope", "alice", "s3cret"),
                None
            ),
            Err(status::BAD_IDENTITY_TOKEN_INVALID)
        );
    }

    #[test]
    fn anonymous_is_rejected_when_the_server_does_not_allow_it() {
        let mut store = UserStore::default();
        store.add_user("user", "alice", "s3cret");
        assert_eq!(
            store.validate(&UserIdentityToken::anonymous(), None),
            Err(status::BAD_IDENTITY_TOKEN_REJECTED)
        );
    }
}
