// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

// zerodds-lint: allow no_dyn_in_safe

//! Built-in pre-shared-key access-control plugin `DDS:Access:PSK`
//! (DDS-Security 1.2 §10.8).
//!
//! The plugin "grants all permissions" (§10.8): the local application may join
//! any domain, publish and subscribe to any topic, and so may a remote
//! participant, "on the basis that any discovered DDS DomainParticipant must
//! have access to the same pre-shared secret key". Every check of Table 85
//! returns TRUE, `validate_local_permissions` and
//! `validate_remote_permissions` always succeed and need no document, and
//! the PermissionsToken has the class `DDS:Access:PSK:1.2`. The one setting
//! is `dds.sec.access.rtps_psk_protection_kind` (Table 84): NONE, SIGN or
//! ENCRYPT, ENCRYPT if absent, which `get_participant_security_config`
//! turns into `is_rtps_psk_protected` and the plugin mask flag.
//!
//! A permissions document, when given, is read plain (no S/MIME wrapper:
//! the PSK path has no permissions CA) and kept with the handle; no check
//! evaluates it.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

use zerodds_security::access_control::{
    AccessControlListener, AccessControlPlugin, AccessDecision, CryptoAlgorithmRequirements,
    DigitalSignatureAlgorithmInfo, EndpointRequest, EndpointSecurityAlgorithmInfo,
    EndpointSecurityConfig, EndpointSymmetricCipherAlgorithmInfo, KeyEstablishmentAlgorithmInfo,
    ParticipantRequest, ParticipantSecurityAlgorithmInfo, ParticipantSecurityConfig,
    PermissionsHandle, RelayRole, RemoteReaderDecision, SymmetricCipherAlgorithmInfo, TopicRequest,
    TopicSecurityConfig, plugin_participant_attributes_mask,
};
use zerodds_security::algorithm::{CRYPTO_ALGORITHM_SET_ALL, CRYPTO_ALGORITHM_SET_EMPTY};
use zerodds_security::authentication::IdentityHandle;
use zerodds_security::error::{SecurityError, SecurityErrorKind, SecurityResult};
use zerodds_security::token::{DataHolder, PermissionsCredentialToken, PermissionsToken};

use zerodds_security_crypto::{PskDerivedCryptoPlugin, Suite};
use zerodds_security_pki::PskHandshakeAuthenticationPlugin;

use crate::governance::{Governance, parse_governance_xml};
use crate::xml::{Permissions, parse_permissions_xml};

/// The `class_id` of the PermissionsToken of `DDS:Access:PSK`
/// (DDS-Security 1.2 §10.8.2.2).
pub const CLASS_ID_PSK_PERMISSIONS: &str = "DDS:Access:PSK:1.2";

/// `dds.sec.access.rtps_psk_protection_kind` (DDS-Security 1.2 §10.8.1
/// Table 84): NONE, SIGN or ENCRYPT.
pub const PROP_RTPS_PSK_PROTECTION_KIND: &str = "dds.sec.access.rtps_psk_protection_kind";

/// How the RTPS messages that no "Authenticated Participant Exchanged Key"
/// protects are protected (§10.8.1): the value of
/// [`PROP_RTPS_PSK_PROTECTION_KIND`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RtpsPskProtection {
    /// "NONE ... will disable all protection."
    None,
    /// Authentication code only.
    Sign,
    /// Encryption (the default: "If not specified it is treated as if it was
    /// specified to be 'ENCRYPT'").
    Encrypt,
}

impl RtpsPskProtection {
    /// Reads the property value.
    ///
    /// # Errors
    /// `InvalidConfiguration` for a value other than `NONE`, `SIGN`,
    /// `ENCRYPT`: a setting nobody can read must not turn the protection off.
    pub fn parse(value: &str) -> SecurityResult<Self> {
        match value {
            "NONE" => Ok(Self::None),
            "SIGN" => Ok(Self::Sign),
            "ENCRYPT" => Ok(Self::Encrypt),
            other => Err(SecurityError::new(
                SecurityErrorKind::InvalidConfiguration,
                alloc::format!(
                    "psk-access: {PROP_RTPS_PSK_PROTECTION_KIND} = '{other}' is not NONE, SIGN or ENCRYPT"
                ),
            )),
        }
    }
}

/// Property key for the permissions XML (local, not propagated).
pub const PROP_PSK_PERMISSIONS_XML: &str = "org.zerodds.sec.access.psk_permissions";
/// Property key for the governance XML (optional).
pub const PROP_PSK_GOVERNANCE_XML: &str = "org.zerodds.sec.access.psk_governance";
/// Property key for the subject name.
pub const PROP_PSK_SUBJECT_NAME: &str = "org.zerodds.sec.access.subject_name";
/// Property key for the permissions identity (in the wire token).
pub const PROP_PSK_PERMISSIONS_ID: &str = "dds.psk.permissions_id";

/// PSK access-control plugin. The holding container is the same
/// `Permissions` struct as the X.509 path — only the loader differs
/// (no S/MIME decode).
pub struct PskPermissionsAccessControl {
    next_handle: AtomicU64,
    slots: BTreeMap<PermissionsHandle, Slot>,
    listener: Option<Arc<dyn AccessControlListener>>,
    issued_tokens: Vec<PermissionsToken>,
    issued_credential_tokens: Vec<PermissionsCredentialToken>,
    issued_participant_configs: Vec<ParticipantSecurityConfig>,
    issued_topic_configs: Vec<TopicSecurityConfig>,
    issued_writer_configs: Vec<EndpointSecurityConfig>,
    issued_reader_configs: Vec<EndpointSecurityConfig>,
}

// The checks of Table 85 read nothing of the slot; the subject and the
// documents stay with the handle for the operations of §10.8 that use them
// (`get_participant_security_config`).
#[allow(dead_code)]
struct Slot {
    subject_name: String,
    permissions: Option<Permissions>,
    /// `dds.sec.access.rtps_psk_protection_kind` of the participant.
    rtps_psk: RtpsPskProtection,
    /// Optional: domain-level governance. Stored here only for
    /// spec compliance; active filters (discovery-protect,
    /// rtps_protection) live in the PSK crypto plugin on the wire path.
    governance: Option<Governance>,
}

impl Default for PskPermissionsAccessControl {
    fn default() -> Self {
        Self::new()
    }
}

impl PskPermissionsAccessControl {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self {
            next_handle: AtomicU64::new(0),
            slots: BTreeMap::new(),
            listener: None,
            issued_tokens: Vec::new(),
            issued_credential_tokens: Vec::new(),
            issued_participant_configs: Vec::new(),
            issued_topic_configs: Vec::new(),
            issued_writer_configs: Vec::new(),
            issued_reader_configs: Vec::new(),
        }
    }

    /// The `EndpointSecurityConfig` of Table 85.
    fn endpoint_config() -> EndpointSecurityConfig {
        EndpointSecurityConfig {
            algorithm_info: EndpointSecurityAlgorithmInfo {
                symmetric_cipher: EndpointSymmetricCipherAlgorithmInfo {
                    required_mask: CRYPTO_ALGORITHM_SET_EMPTY,
                    supported_mask: CRYPTO_ALGORITHM_SET_ALL,
                },
            },
            ..EndpointSecurityConfig::default()
        }
    }

    fn next_id(&self) -> u64 {
        self.next_handle.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Programmatic constructor — registers a pre-parsed
    /// permissions structure for a subject.
    pub fn register(
        &mut self,
        subject_name: String,
        permissions: Permissions,
        governance: Option<Governance>,
    ) -> PermissionsHandle {
        self.insert(
            subject_name,
            Some(permissions),
            governance,
            RtpsPskProtection::Encrypt,
        )
    }

    fn insert(
        &mut self,
        subject_name: String,
        permissions: Option<Permissions>,
        governance: Option<Governance>,
        rtps_psk: RtpsPskProtection,
    ) -> PermissionsHandle {
        let handle = PermissionsHandle(self.next_id());
        self.slots.insert(
            handle,
            Slot {
                subject_name,
                permissions,
                rtps_psk,
                governance,
            },
        );
        handle
    }

    fn slot(&self, handle: PermissionsHandle) -> SecurityResult<&Slot> {
        self.slots.get(&handle).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                "psk-access: unknown PermissionsHandle",
            )
        })
    }

    /// The RTPS PSK protection of the participant behind `handle`
    /// (Table 85, `get_participant_security_config`).
    ///
    /// # Errors
    /// `BadArgument` for an unknown handle.
    pub fn rtps_psk_protection(
        &self,
        handle: PermissionsHandle,
    ) -> SecurityResult<RtpsPskProtection> {
        Ok(self.slot(handle)?.rtps_psk)
    }

    /// `is_rtps_psk_protected` of the ParticipantSecurityConfig: TRUE for
    /// ENCRYPT and SIGN, FALSE for NONE (Table 85).
    ///
    /// # Errors
    /// `BadArgument` for an unknown handle.
    pub fn is_rtps_psk_protected(&self, handle: PermissionsHandle) -> SecurityResult<bool> {
        Ok(self.rtps_psk_protection(handle)? != RtpsPskProtection::None)
    }

    /// `PLUGIN_PARTICIPANT_SECURITY_ATTRIBUTES_FLAG_IS_RTPS_PSK_ENCRYPTED`
    /// of the plugin participant mask: set if and only if the protection is
    /// ENCRYPT (Table 85).
    ///
    /// # Errors
    /// `BadArgument` for an unknown handle.
    pub fn is_rtps_psk_encrypted(&self, handle: PermissionsHandle) -> SecurityResult<bool> {
        Ok(self.rtps_psk_protection(handle)? == RtpsPskProtection::Encrypt)
    }

    /// Creates a wire PermissionsToken in the PSK variant
    /// (`DDS:Access:PSK:1.2`). The token body propagates
    /// only the `permissions_id` — the XML itself is not sent over the
    /// wire (the S/MIME wrap is missing on the PSK path).
    #[must_use]
    pub fn build_permissions_token(permissions_id: &str) -> alloc::vec::Vec<u8> {
        DataHolder::new(CLASS_ID_PSK_PERMISSIONS)
            .with_property(PROP_PSK_PERMISSIONS_ID, permissions_id)
            .to_cdr_le()
    }

    fn known(&self, handle: PermissionsHandle) -> SecurityResult<()> {
        if self.slots.contains_key(&handle) {
            Ok(())
        } else {
            Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "psk-access: unknown PermissionsHandle",
            ))
        }
    }
}

impl AccessControlPlugin for PskPermissionsAccessControl {
    /// Table 85: "This operation shall succeed and return an opaque handle".
    /// No document is required; one given in
    /// [`PROP_PSK_PERMISSIONS_XML`] must parse, and is kept.
    fn validate_local_permissions(
        &mut self,
        _identity: IdentityHandle,
        participant: &ParticipantRequest<'_>,
    ) -> SecurityResult<PermissionsHandle> {
        let props = participant.properties;
        let rtps_psk = match props.get(PROP_RTPS_PSK_PROTECTION_KIND) {
            Some(value) => RtpsPskProtection::parse(value)?,
            None => RtpsPskProtection::Encrypt,
        };
        let perms = props
            .get(PROP_PSK_PERMISSIONS_XML)
            .map(|xml| {
                parse_permissions_xml(xml).map_err(|e| {
                    SecurityError::new(
                        SecurityErrorKind::InvalidConfiguration,
                        alloc::format!("psk-access: permissions: {e}"),
                    )
                })
            })
            .transpose()?;
        let governance = props
            .get(PROP_PSK_GOVERNANCE_XML)
            .map(|gov_xml| {
                parse_governance_xml(gov_xml).map_err(|e| {
                    SecurityError::new(
                        SecurityErrorKind::InvalidConfiguration,
                        alloc::format!("psk-access: governance: {e}"),
                    )
                })
            })
            .transpose()?;
        let subject = props
            .get(PROP_PSK_SUBJECT_NAME)
            .unwrap_or_default()
            .to_string();
        Ok(self.insert(subject, perms, governance, rtps_psk))
    }

    /// Table 85: "This operation shall succeed". The remote token and the
    /// credential are not interpreted (§10.8.2.1): nothing a remote presents
    /// is read, and its handle gets the plugin's own setting.
    fn validate_remote_permissions(
        &mut self,
        _local_identity: IdentityHandle,
        _remote_identity: IdentityHandle,
        _remote_permissions_token: &PermissionsToken,
        _remote_credential_token: &DataHolder,
    ) -> SecurityResult<PermissionsHandle> {
        Ok(self.insert(String::new(), None, None, RtpsPskProtection::Encrypt))
    }

    /// Table 85: "This operation shall return TRUE."
    fn check_create_participant(
        &self,
        permissions: PermissionsHandle,
        _participant: &ParticipantRequest<'_>,
    ) -> SecurityResult<AccessDecision> {
        self.known(permissions)?;
        Ok(AccessDecision::Permit)
    }

    /// DDS-Security 1.2 §10.8.3 Table 85: "This operation shall return
    /// TRUE."
    fn check_create_datawriter(
        &self,
        perms: PermissionsHandle,
        _request: &EndpointRequest<'_>,
    ) -> SecurityResult<AccessDecision> {
        self.known(perms)?;
        Ok(AccessDecision::Permit)
    }

    /// Table 85: "This operation shall return TRUE."
    fn check_create_datareader(
        &self,
        perms: PermissionsHandle,
        _request: &EndpointRequest<'_>,
    ) -> SecurityResult<AccessDecision> {
        self.known(perms)?;
        Ok(AccessDecision::Permit)
    }

    /// Table 85: "This operation shall return TRUE."
    fn check_create_topic(
        &self,
        permissions: PermissionsHandle,
        _request: &TopicRequest<'_>,
    ) -> SecurityResult<AccessDecision> {
        self.known(permissions)?;
        Ok(AccessDecision::Permit)
    }

    /// Table 85: "This operation shall return TRUE."
    fn check_local_datawriter_register_instance(
        &self,
        permissions: PermissionsHandle,
        _writer: [u8; 16],
        _key: &[u8],
    ) -> SecurityResult<AccessDecision> {
        self.known(permissions)?;
        Ok(AccessDecision::Permit)
    }

    /// Table 85: "This operation shall return TRUE."
    fn check_local_datawriter_dispose_instance(
        &self,
        permissions: PermissionsHandle,
        _writer: [u8; 16],
        _key: &[u8],
    ) -> SecurityResult<AccessDecision> {
        self.known(permissions)?;
        Ok(AccessDecision::Permit)
    }

    /// Table 85: "This operation shall return TRUE."
    fn check_remote_participant(
        &self,
        permissions: PermissionsHandle,
        _participant: &ParticipantRequest<'_>,
    ) -> SecurityResult<AccessDecision> {
        self.known(permissions)?;
        Ok(AccessDecision::Permit)
    }

    /// Table 85: "This operation shall return TRUE."
    fn check_remote_datawriter(
        &self,
        remote_perms: PermissionsHandle,
        _request: &EndpointRequest<'_>,
    ) -> SecurityResult<AccessDecision> {
        self.known(remote_perms)?;
        Ok(AccessDecision::Permit)
    }

    /// Table 85: "This operation shall return TRUE." (`relay_only` FALSE.)
    fn check_remote_datareader(
        &self,
        remote_perms: PermissionsHandle,
        _request: &EndpointRequest<'_>,
    ) -> SecurityResult<RemoteReaderDecision> {
        self.known(remote_perms)?;
        Ok(RemoteReaderDecision::Permit)
    }

    /// Table 85: "This operation shall return TRUE."
    fn check_remote_topic(
        &self,
        permissions: PermissionsHandle,
        _request: &TopicRequest<'_>,
    ) -> SecurityResult<AccessDecision> {
        self.known(permissions)?;
        Ok(AccessDecision::Permit)
    }

    /// Table 85: "This operation shall return TRUE."
    fn check_local_datawriter_match(
        &self,
        writer_permissions: PermissionsHandle,
        reader_permissions: PermissionsHandle,
        _publication: &EndpointRequest<'_>,
        _subscription: &EndpointRequest<'_>,
    ) -> SecurityResult<AccessDecision> {
        self.known(writer_permissions)?;
        self.known(reader_permissions)?;
        Ok(AccessDecision::Permit)
    }

    /// Table 85: "This operation shall return TRUE."
    fn check_local_datareader_match(
        &self,
        writer_permissions: PermissionsHandle,
        reader_permissions: PermissionsHandle,
        _subscription: &EndpointRequest<'_>,
        _publication: &EndpointRequest<'_>,
    ) -> SecurityResult<AccessDecision> {
        self.known(writer_permissions)?;
        self.known(reader_permissions)?;
        Ok(AccessDecision::Permit)
    }

    /// Table 85: "This operation shall return TRUE."
    fn check_remote_datawriter_register_instance(
        &self,
        permissions: PermissionsHandle,
        _reader: [u8; 16],
        _publication: [u8; 16],
        _key: &[u8],
    ) -> SecurityResult<AccessDecision> {
        self.known(permissions)?;
        Ok(AccessDecision::Permit)
    }

    /// Table 85: "This operation shall return TRUE."
    fn check_remote_datawriter_dispose_instance(
        &self,
        permissions: PermissionsHandle,
        _reader: [u8; 16],
        _publication: [u8; 16],
        _key: &[u8],
    ) -> SecurityResult<AccessDecision> {
        self.known(permissions)?;
        Ok(AccessDecision::Permit)
    }

    /// Table 85: the PermissionsToken of §10.8.2.2.
    fn get_permissions_token(
        &mut self,
        handle: PermissionsHandle,
    ) -> SecurityResult<PermissionsToken> {
        self.known(handle)?;
        let token = DataHolder::new(CLASS_ID_PSK_PERMISSIONS);
        self.issued_tokens.push(token.clone());
        Ok(token)
    }

    /// Table 85: "the PermissionsToken formatted as described in 10.8.2.1"
    /// (the value is implementation specific; here the same class without
    /// properties).
    fn get_permissions_credential_token(
        &mut self,
        handle: PermissionsHandle,
    ) -> SecurityResult<PermissionsCredentialToken> {
        self.known(handle)?;
        let token = DataHolder::new(CLASS_ID_PSK_PERMISSIONS);
        self.issued_credential_tokens.push(token.clone());
        Ok(token)
    }

    /// Table 85: "This operation shall save a reference to the listener
    /// object". Nothing in this plugin revokes a permission, so the
    /// listener is never called.
    fn set_listener(&mut self, listener: Arc<dyn AccessControlListener>) -> SecurityResult<()> {
        self.listener = Some(listener);
        Ok(())
    }

    /// §9.4.2.9.20.
    fn return_permissions_token(&mut self, token: PermissionsToken) -> SecurityResult<()> {
        crate::issued::give_back(
            &mut self.issued_tokens,
            &token,
            "psk-access",
            "the PermissionsToken",
        )
    }

    /// §9.4.2.9.21.
    fn return_permissions_credential_token(
        &mut self,
        token: PermissionsCredentialToken,
    ) -> SecurityResult<()> {
        crate::issued::give_back(
            &mut self.issued_credential_tokens,
            &token,
            "psk-access",
            "the PermissionsCredentialToken",
        )
    }

    /// Table 85: `allow_unauthenticated_participants` TRUE, `is_access_protected`
    /// FALSE, `is_rtps_axk_protected` FALSE, `is_rtps_psk_protected` from the
    /// property, discovery and liveliness FALSE, the plugin mask
    /// `IS_RTPS_PSK_ENCRYPTED` only for ENCRYPT, every `supported_mask` ALL and
    /// every `required_mask` EMPTY.
    fn get_participant_security_config(
        &mut self,
        permissions: PermissionsHandle,
    ) -> SecurityResult<ParticipantSecurityConfig> {
        let psk = self.rtps_psk_protection(permissions)?;
        let all = CRYPTO_ALGORITHM_SET_ALL;
        let config = ParticipantSecurityConfig {
            allow_unauthenticated_participants: true,
            is_access_protected: false,
            is_rtps_axk_protected: false,
            is_rtps_psk_protected: psk != RtpsPskProtection::None,
            is_discovery_protected: false,
            is_liveliness_protected: false,
            is_key_revision_enabled: false,
            plugin_participant_attributes: if psk == RtpsPskProtection::Encrypt {
                plugin_participant_attributes_mask::IS_RTPS_PSK_ENCRYPTED
            } else {
                0
            },
            ac_participant_properties: Vec::new(),
            algorithm_info: ParticipantSecurityAlgorithmInfo {
                digital_signature: DigitalSignatureAlgorithmInfo {
                    trust_chain: CryptoAlgorithmRequirements {
                        supported_mask: all,
                        required_mask: CRYPTO_ALGORITHM_SET_EMPTY,
                    },
                    message_auth: CryptoAlgorithmRequirements {
                        supported_mask: all,
                        required_mask: CRYPTO_ALGORITHM_SET_EMPTY,
                    },
                },
                key_establishment: KeyEstablishmentAlgorithmInfo {
                    shared_secret: CryptoAlgorithmRequirements {
                        supported_mask: all,
                        required_mask: CRYPTO_ALGORITHM_SET_EMPTY,
                    },
                },
                symmetric_cipher: SymmetricCipherAlgorithmInfo {
                    supported_mask: all,
                    builtin_endpoints_required_mask: CRYPTO_ALGORITHM_SET_EMPTY,
                    builtin_kx_endpoints_required_mask: CRYPTO_ALGORITHM_SET_EMPTY,
                    user_endpoints_default_required_mask: CRYPTO_ALGORITHM_SET_EMPTY,
                },
            },
        };
        self.issued_participant_configs.push(config.clone());
        Ok(config)
    }

    /// Table 85: "The fields of the TopicSecurityConfig attributes shall all
    /// be set to FALSE."
    fn get_topic_security_config(
        &mut self,
        permissions: PermissionsHandle,
        _topic_name: &str,
    ) -> SecurityResult<TopicSecurityConfig> {
        self.known(permissions)?;
        let config = TopicSecurityConfig::default();
        self.issued_topic_configs.push(config);
        Ok(config)
    }

    /// Table 85: the booleans FALSE, the attribute mask empty, `supported_mask`
    /// ALL and `required_mask` EMPTY; the `ac_endpoint_properties` are empty.
    fn get_datawriter_security_config(
        &mut self,
        permissions: PermissionsHandle,
        _request: &EndpointRequest<'_>,
    ) -> SecurityResult<EndpointSecurityConfig> {
        self.known(permissions)?;
        let config = Self::endpoint_config();
        self.issued_writer_configs.push(config.clone());
        Ok(config)
    }

    /// Table 85, as [`Self::get_datawriter_security_config`].
    fn get_datareader_security_config(
        &mut self,
        permissions: PermissionsHandle,
        _request: &EndpointRequest<'_>,
    ) -> SecurityResult<EndpointSecurityConfig> {
        self.known(permissions)?;
        let config = Self::endpoint_config();
        self.issued_reader_configs.push(config.clone());
        Ok(config)
    }

    // §9.4.2.9.26 to §9.4.2.9.29.
    return_security_configs!("psk-access");

    /// The plugin grants all permissions (§10.8), relaying included.
    fn check_relay(
        &self,
        permissions: PermissionsHandle,
        _role: RelayRole,
        _request: &EndpointRequest<'_>,
    ) -> SecurityResult<AccessDecision> {
        self.known(permissions)?;
        Ok(AccessDecision::Permit)
    }

    fn plugin_class_id(&self) -> &str {
        CLASS_ID_PSK_PERMISSIONS
    }
}

// ---------------------------------------------------------------------
// PSK-Profile-Bundle (Spec §10.7-§10.9)
// ---------------------------------------------------------------------

/// Bundle of the three PSK plugins for a simple caller path.
/// Spec ref: §10.7 auth, §10.8 access, §10.9 crypto.
pub struct PskProfile {
    /// Authentication plugin: the PSK handshake of ZeroDDS
    /// (`org.zerodds.auth.psk-handshake:1.0`), not the NOOP plugin
    /// `DDS:Auth:PSK` of §10.7.
    pub auth: PskHandshakeAuthenticationPlugin,
    /// Access-control plugin (`DDS:Access:PSK:1.2`).
    pub access_control: PskPermissionsAccessControl,
    /// Crypto plugin: the pre-shared-key plugin of ZeroDDS
    /// (`ZeroDDS:Crypto:PSK-Derived:1.0`), not `DDS:Crypto:PSK` of §10.9.
    pub crypto: PskDerivedCryptoPlugin,
}

impl PskProfile {
    /// Creates a PSK profile bundle. Configures:
    ///
    /// 1. Authentication with `(identity_id, pre_shared_key)`.
    /// 2. Access control with `governance_xml` (optional) +
    ///    `permissions_xml` (plain, without an S/MIME wrap).
    /// 3. Crypto with `Aes128Gcm` (default suite) and `psk_id=1`.
    ///
    /// # Errors
    /// Configuration/XML errors are passed through as
    /// [`zerodds_security::SecurityError`].
    pub fn with_pre_shared_key(
        identity_id: &str,
        key: alloc::vec::Vec<u8>,
        governance_xml: Option<&str>,
        permissions_xml: &str,
    ) -> SecurityResult<Self> {
        let mut auth = PskHandshakeAuthenticationPlugin::new();
        auth.register_psk(identity_id.to_string(), key.clone())?;
        auth.validate_local_psk_identity(identity_id)?;

        let mut access = PskPermissionsAccessControl::new();
        let perms = parse_permissions_xml(permissions_xml).map_err(|e| {
            SecurityError::new(
                SecurityErrorKind::InvalidConfiguration,
                alloc::format!("psk-profile: permissions: {e}"),
            )
        })?;
        let governance = governance_xml
            .map(|x| {
                parse_governance_xml(x).map_err(|e| {
                    SecurityError::new(
                        SecurityErrorKind::InvalidConfiguration,
                        alloc::format!("psk-profile: governance: {e}"),
                    )
                })
            })
            .transpose()?;
        access.register(identity_id.to_string(), perms, governance);

        let mut crypto = PskDerivedCryptoPlugin::with_suite(Suite::Aes128Gcm);
        crypto.register_psk(1, key)?;

        Ok(Self {
            auth,
            access_control: access,
            crypto,
        })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use zerodds_security::authentication::IdentityHandle;
    use zerodds_security::properties::{Property, PropertyList};

    const ALICE_XML: &str = r#"
<dds><permissions>
  <grant><subject_name>CN=alice</subject_name><validity><not_before>1970-01-01T00:00:00</not_before><not_after>2100-01-01T00:00:00</not_after></validity>
    <allow_rule><domains><id_range><min>0</min></id_range></domains>
      <publish><topics><topic>Chatter</topic><topic>sensor_*</topic></topics></publish>
      <subscribe><topics><topic>Echo</topic></topics></subscribe>
    </allow_rule>
    <default>DENY</default>
  </grant>
</permissions></dds>
"#;

    fn build_alice() -> (PskPermissionsAccessControl, PermissionsHandle) {
        let mut ac = PskPermissionsAccessControl::new();
        let perms = parse_permissions_xml(ALICE_XML).unwrap();
        let h = ac.register("CN=alice".into(), perms, None);
        (ac, h)
    }

    /// A participant in domain 0 with `props`.
    fn pr(props: &PropertyList) -> ParticipantRequest<'_> {
        ParticipantRequest::new(0, [0xAA; 16], props)
    }

    /// Same shape as the X.509 fixture in
    /// `crate::plugin::tests`: one allow pattern, one deny rule carved
    /// out of it.
    const CARL_XML: &str = r#"
<dds><permissions>
  <grant><subject_name>CN=carl</subject_name><validity><not_before>1970-01-01T00:00:00</not_before><not_after>2100-01-01T00:00:00</not_after></validity>
    <allow_rule><domains><id_range><min>0</min></id_range></domains>
      <publish><topics><topic>sensor/*</topic></topics></publish>
      <subscribe><topics><topic>sensor/*</topic></topics></subscribe>
    </allow_rule>
    <deny_rule><domains><id_range><min>0</min></id_range></domains>
      <publish><topics><topic>sensor/secret</topic></topics></publish>
      <subscribe><topics><topic>sensor/secret</topic></topics></subscribe>
    </deny_rule>
    <default>DENY</default>
  </grant>
</permissions></dds>
"#;

    #[test]
    fn plugin_class_id_matches_spec() {
        let ac = PskPermissionsAccessControl::new();
        assert_eq!(ac.plugin_class_id(), "DDS:Access:PSK:1.2");
    }

    /// DDS-Security 1.2 §10.8.3 Table 85: every check of DDS:Access:PSK
    /// returns TRUE, for topics a grant would deny too. The grants of the
    /// document were evaluated; an unknown handle is still an error.
    #[test]
    fn every_check_returns_true() {
        let mut ac = PskPermissionsAccessControl::new();
        let perms = parse_permissions_xml(CARL_XML).expect("CARL_XML parses");
        let h = ac.register("CN=carl".into(), perms, None);
        for topic in ["sensor/temp", "sensor/secret", "actuator"] {
            let req = EndpointRequest::new(0, topic);
            assert_eq!(
                ac.check_create_datawriter(h, &req).unwrap(),
                AccessDecision::Permit
            );
            assert_eq!(
                ac.check_create_datareader(h, &req).unwrap(),
                AccessDecision::Permit
            );
            assert_eq!(
                ac.check_remote_datawriter(h, &req).unwrap(),
                AccessDecision::Permit
            );
            assert_eq!(
                ac.check_remote_datareader(h, &req).unwrap(),
                RemoteReaderDecision::Permit
            );
        }
        let (alice, _) = build_alice();
        assert!(
            alice
                .check_create_datawriter(PermissionsHandle(999), &EndpointRequest::new(0, "T"))
                .is_err()
        );
    }

    /// Table 85: "check_create_participant, check_remote_participant,
    /// check_create_topic" and the checks of instances, remote topics and
    /// matches return TRUE, whatever the document of the handle says.
    #[test]
    fn the_participant_topic_instance_and_match_checks_return_true() {
        let mut ac = PskPermissionsAccessControl::new();
        let perms = parse_permissions_xml(CARL_XML).unwrap();
        let h = ac.register("CN=carl".into(), perms, None);
        let props = PropertyList::new();
        let yes = AccessDecision::Permit;
        assert_eq!(ac.check_create_participant(h, &pr(&props)).unwrap(), yes);
        assert_eq!(ac.check_remote_participant(h, &pr(&props)).unwrap(), yes);
        for topic in ["sensor/secret", "other"] {
            let t = TopicRequest::new(0, topic);
            assert_eq!(ac.check_create_topic(h, &t).unwrap(), yes);
            assert_eq!(ac.check_remote_topic(h, &t).unwrap(), yes);
        }
        let ep = EndpointRequest::new(0, "sensor/secret");
        let (guid, key) = ([3u8; 16], [9u8, 9]);
        assert_eq!(
            ac.check_local_datawriter_register_instance(h, guid, &key)
                .unwrap(),
            yes
        );
        assert_eq!(
            ac.check_local_datawriter_dispose_instance(h, guid, &key)
                .unwrap(),
            yes
        );
        assert_eq!(
            ac.check_local_datawriter_match(h, h, &ep, &ep).unwrap(),
            yes
        );
        assert_eq!(
            ac.check_local_datareader_match(h, h, &ep, &ep).unwrap(),
            yes
        );
        assert_eq!(
            ac.check_remote_datawriter_register_instance(h, guid, [4; 16], &key)
                .unwrap(),
            yes
        );
        assert_eq!(
            ac.check_remote_datawriter_dispose_instance(h, guid, [4; 16], &key)
                .unwrap(),
            yes
        );
        let bogus = PermissionsHandle(999);
        assert!(ac.check_create_participant(bogus, &pr(&props)).is_err());
        assert!(
            ac.check_create_topic(bogus, &TopicRequest::new(0, "T"))
                .is_err()
        );
        assert!(ac.check_local_datawriter_match(bogus, h, &ep, &ep).is_err());
    }

    #[test]
    fn property_list_driver_loads_permissions_without_smime() {
        let mut ac = PskPermissionsAccessControl::new();
        let props = PropertyList::new()
            .with(Property::local(
                PROP_PSK_PERMISSIONS_XML,
                ALICE_XML.to_string(),
            ))
            .with(Property::local(PROP_PSK_SUBJECT_NAME, "CN=alice"));
        let h = ac
            .validate_local_permissions(IdentityHandle(1), &pr(&props))
            .expect("psk-access: validate via props");
        assert_eq!(
            ac.check_create_datawriter(h, &EndpointRequest::new(0, "Chatter"))
                .unwrap(),
            AccessDecision::Permit,
        );
    }

    /// Table 85: validate_local_permissions "shall succeed"; no document and
    /// no subject name are needed. The protection of the RTPS messages that
    /// no exchanged key covers is `dds.sec.access.rtps_psk_protection_kind`
    /// (Table 84): ENCRYPT when absent, a value outside NONE, SIGN, ENCRYPT
    /// is refused. The attributes follow Table 85: `is_rtps_psk_protected`
    /// for SIGN and ENCRYPT, the encrypted flag for ENCRYPT only.
    #[test]
    fn nothing_is_needed_and_the_rtps_psk_protection_is_read() {
        let mut ac = PskPermissionsAccessControl::new();
        let local = IdentityHandle(1);
        let none = PropertyList::new();
        let h = ac
            .validate_local_permissions(local, &pr(&none))
            .expect("succeeds without any property");
        assert_eq!(
            ac.rtps_psk_protection(h).unwrap(),
            RtpsPskProtection::Encrypt
        );
        assert!(ac.is_rtps_psk_protected(h).unwrap());
        assert!(ac.is_rtps_psk_encrypted(h).unwrap());
        for (value, protected, encrypted) in [
            ("NONE", false, false),
            ("SIGN", true, false),
            ("ENCRYPT", true, true),
        ] {
            let props =
                PropertyList::new().with(Property::local(PROP_RTPS_PSK_PROTECTION_KIND, value));
            let h = ac.validate_local_permissions(local, &pr(&props)).unwrap();
            assert_eq!(ac.is_rtps_psk_protected(h).unwrap(), protected, "{value}");
            assert_eq!(ac.is_rtps_psk_encrypted(h).unwrap(), encrypted, "{value}");
        }
        let bad = PropertyList::new().with(Property::local(PROP_RTPS_PSK_PROTECTION_KIND, "none"));
        assert_eq!(
            ac.validate_local_permissions(local, &pr(&bad))
                .unwrap_err()
                .kind,
            SecurityErrorKind::InvalidConfiguration
        );
    }

    /// §10.8.2.2: the PermissionsToken has the class `DDS:Access:PSK:1.2`
    /// and no other attribute is needed; the credential token is the same
    /// kind of token (Table 85).
    #[test]
    fn permissions_token_class_id_matches_spec() {
        let mut ac = PskPermissionsAccessControl::new();
        let none = PropertyList::new();
        let h = ac
            .validate_local_permissions(IdentityHandle(1), &pr(&none))
            .unwrap();
        for token in [
            ac.get_permissions_token(h).unwrap(),
            ac.get_permissions_credential_token(h).unwrap(),
        ] {
            assert_eq!(token.class_id, "DDS:Access:PSK:1.2");
            assert!(token.properties.is_empty() && token.binary_properties.is_empty());
        }
        assert!(ac.get_permissions_token(PermissionsHandle(999)).is_err());
        assert!(
            ac.get_permissions_credential_token(PermissionsHandle(999))
                .is_err()
        );
    }

    /// Table 85: validate_remote_permissions "shall succeed", whatever
    /// token and credential the remote presents; every check then permits.
    #[test]
    fn a_remote_is_accepted_whatever_it_presents() {
        let mut ac = PskPermissionsAccessControl::new();
        let credentials = [
            DataHolder::new(""),
            DataHolder::new("DDS:Auth:PKI-DH:1.0")
                .with_binary_property("c.id", b"not a certificate".to_vec())
                .with_binary_property("c.perm", b"not xml".to_vec()),
            DataHolder::new("anything").with_property("a", "b"),
        ];
        let tokens = [
            DataHolder::new("DDS:Access:PSK:1.2"),
            DataHolder::new("DDS:Access:Permissions:1.2"),
            DataHolder::new(""),
        ];
        for (token, credential) in tokens.iter().zip(&credentials) {
            let h = ac
                .validate_remote_permissions(
                    IdentityHandle(1),
                    IdentityHandle(2),
                    token,
                    credential,
                )
                .expect("succeeds");
            assert_eq!(
                ac.check_remote_datawriter(h, &EndpointRequest::new(0, "Any"))
                    .unwrap(),
                AccessDecision::Permit
            );
        }
    }

    /// Table 85, `get_participant_security_config`: every field of the list.
    #[test]
    fn the_participant_config_is_the_one_of_table_85() {
        let mut ac = PskPermissionsAccessControl::new();
        for (value, protected, mask) in [
            (
                None,
                true,
                plugin_participant_attributes_mask::IS_RTPS_PSK_ENCRYPTED,
            ),
            (
                Some("ENCRYPT"),
                true,
                plugin_participant_attributes_mask::IS_RTPS_PSK_ENCRYPTED,
            ),
            (Some("SIGN"), true, 0),
            (Some("NONE"), false, 0),
        ] {
            let mut props = PropertyList::new();
            if let Some(value) = value {
                props.set(Property::local(PROP_RTPS_PSK_PROTECTION_KIND, value));
            }
            let h = ac
                .validate_local_permissions(IdentityHandle(1), &pr(&props))
                .unwrap();
            let config = ac.get_participant_security_config(h).unwrap();
            assert!(config.allow_unauthenticated_participants);
            assert!(!config.is_access_protected);
            assert!(!config.is_rtps_axk_protected);
            assert_eq!(config.is_rtps_psk_protected, protected, "{value:?}");
            assert!(!config.is_discovery_protected);
            assert!(!config.is_liveliness_protected);
            assert!(!config.is_key_revision_enabled);
            assert_eq!(config.plugin_participant_attributes, mask, "{value:?}");
            assert!(config.ac_participant_properties.is_empty());
            let all = CRYPTO_ALGORITHM_SET_ALL;
            let info = config.algorithm_info;
            assert_eq!(info.digital_signature.trust_chain.supported_mask, all);
            assert_eq!(info.digital_signature.message_auth.supported_mask, all);
            assert_eq!(info.key_establishment.shared_secret.supported_mask, all);
            assert_eq!(info.symmetric_cipher.supported_mask, all);
            assert_eq!(info.digital_signature.trust_chain.required_mask, 0);
            assert_eq!(info.digital_signature.message_auth.required_mask, 0);
            assert_eq!(info.key_establishment.shared_secret.required_mask, 0);
            assert_eq!(info.symmetric_cipher.builtin_endpoints_required_mask, 0);
            assert_eq!(info.symmetric_cipher.builtin_kx_endpoints_required_mask, 0);
            assert_eq!(
                info.symmetric_cipher.user_endpoints_default_required_mask,
                0
            );
            // Bit 0 of the attributes mask is IS_VALID alone, plus the PSK bit.
            let expected = 0x8000_0000 | if protected { 0x10 } else { 0 };
            assert_eq!(config.attributes_mask(), expected);
            ac.return_participant_security_config(config).unwrap();
        }
    }

    /// Table 85: the topic and endpoint configs are all FALSE and empty, the
    /// algorithm masks ALL and EMPTY.
    #[test]
    fn the_topic_and_endpoint_configs_are_all_false() {
        let (mut ac, h) = build_alice();
        let topic = ac.get_topic_security_config(h, "Chatter").unwrap();
        assert_eq!(topic, TopicSecurityConfig::default());
        for config in [
            ac.get_datawriter_security_config(h, &EndpointRequest::new(0, "Chatter"))
                .unwrap(),
            ac.get_datareader_security_config(h, &EndpointRequest::new(0, "Chatter"))
                .unwrap(),
        ] {
            assert_eq!(config.topic, TopicSecurityConfig::default());
            assert!(!config.is_submessage_protected);
            assert!(!config.is_payload_protected);
            assert!(!config.is_key_protected);
            assert_eq!(config.plugin_endpoint_attributes, 0);
            assert_eq!(config.attributes_mask(), 0x8000_0000);
            assert_eq!(
                config.algorithm_info.symmetric_cipher.supported_mask,
                CRYPTO_ALGORITHM_SET_ALL
            );
            assert_eq!(config.algorithm_info.symmetric_cipher.required_mask, 0);
        }
        let bogus = PermissionsHandle(999);
        assert!(ac.get_topic_security_config(bogus, "T").is_err());
        assert!(
            ac.get_datawriter_security_config(bogus, &EndpointRequest::new(0, "T"))
                .is_err()
        );
        assert!(
            ac.get_datareader_security_config(bogus, &EndpointRequest::new(0, "T"))
                .is_err()
        );
        assert!(ac.get_participant_security_config(bogus).is_err());
    }

    struct Quiet;
    impl AccessControlListener for Quiet {
        fn on_revoke_permissions(&self, _plugin: &str, _handle: PermissionsHandle) -> bool {
            true
        }
    }

    /// §9.4.2.9.19 to §9.4.2.9.29: the listener is taken; only what was
    /// issued is returned, and once.
    #[test]
    fn listener_and_return_operations() {
        let (mut ac, h) = build_alice();
        ac.set_listener(Arc::new(Quiet)).unwrap();
        let token = ac.get_permissions_token(h).unwrap();
        let credential = ac.get_permissions_credential_token(h).unwrap();
        ac.return_permissions_token(token.clone()).unwrap();
        assert!(ac.return_permissions_token(token).is_err());
        ac.return_permissions_credential_token(credential.clone())
            .unwrap();
        assert!(ac.return_permissions_credential_token(credential).is_err());

        let participant = ac.get_participant_security_config(h).unwrap();
        ac.return_participant_security_config(participant.clone())
            .unwrap();
        assert!(ac.return_participant_security_config(participant).is_err());
        let topic = ac.get_topic_security_config(h, "T").unwrap();
        ac.return_topic_security_config(topic).unwrap();
        assert!(ac.return_topic_security_config(topic).is_err());
        let writer = ac
            .get_datawriter_security_config(h, &EndpointRequest::new(0, "T"))
            .unwrap();
        ac.return_datawriter_security_config(writer.clone())
            .unwrap();
        assert!(ac.return_datawriter_security_config(writer).is_err());
        let reader = ac
            .get_datareader_security_config(h, &EndpointRequest::new(0, "T"))
            .unwrap();
        ac.return_datareader_security_config(reader.clone())
            .unwrap();
        assert!(ac.return_datareader_security_config(reader).is_err());
    }
}
