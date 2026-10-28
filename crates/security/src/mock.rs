// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Mock plugins for tests.
//!
//! The mocks accept **every** peer and simulate a
//! handshake in exactly two steps. Never for production — they
//! provide no real crypto.
//!
//! Purpose:
//! 1. Validate the SPI interface against an actually working flow
//!    (signature checks, handshake state machine).
//! 2. From v1.4 on the DCPS layer can sub-test against the mock before the
//!    production plugin is finished.
//!
//! zerodds-lint: allow no_dyn_in_safe
//! (tests instantiate `Box<dyn AuthenticationPlugin>`.)

extern crate alloc;

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::sync::Arc;
use alloc::vec::Vec;

#[cfg(feature = "std")]
use alloc::borrow::ToOwned;
#[cfg(feature = "std")]
use alloc::string::String;
use core::sync::atomic::{AtomicU64, Ordering};

use crate::access_control::{
    AccessControlListener, AccessControlPlugin, AccessDecision, EndpointRequest,
    EndpointSecurityConfig, ParticipantRequest,
    ParticipantSecurityConfig as AccessParticipantSecurityConfig, PermissionsHandle, RelayRole,
    RemoteReaderDecision, TopicRequest, TopicSecurityConfig,
};
use crate::authentication::{
    AlgorithmMasks, AuthenticationExtensions, AuthenticationListener, AuthenticationPlugin,
    HandshakeHandle, HandshakeStepOutcome, IdentityHandle, IssuedTokens, LocalIdentityValidation,
    ParticipantSecurityConfig, RemoteIdentityValidation, SharedSecretHandle, ValidationResult,
};
use crate::data_tagging::{DataTag, DataTaggingPlugin};
use crate::error::{SecurityError, SecurityErrorKind, SecurityResult};
use crate::logging::{LogLevel, LoggingPlugin};
use crate::properties::PropertyList;
use crate::token::{DataHolder, PermissionsCredentialToken, PermissionsToken};

// ============================================================================
// MockAuthenticationPlugin
// ============================================================================

/// Mock implementation — accepts everything, handshake step count
/// hard-coded.
#[derive(Default)]
pub struct MockAuthenticationPlugin {
    next_handle: AtomicU64,
    identities: BTreeSet<IdentityHandle>,
    handshakes: BTreeMap<HandshakeHandle, SharedSecretHandle>,
    /// Handshakes begun and not given back.
    begun: BTreeSet<HandshakeHandle>,
    /// Tokens handed out, per kind, and not given back.
    tokens_out: IssuedTokens,
    listener: Option<Arc<dyn AuthenticationListener>>,
    /// Names of the operations called, in order.
    calls: Arc<std::sync::Mutex<Vec<String>>>,
}

impl core::fmt::Debug for MockAuthenticationPlugin {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("MockAuthenticationPlugin")
            .field("identities", &self.identities)
            .field("handshakes", &self.handshakes)
            .finish_non_exhaustive()
    }
}

/// The class ids of the handshake tokens of the mock; a token is the CDR of a
/// DataHolder of that class, as the stateless message carries it.
const TOKEN_REQUEST: &str = "DDS:Auth:Mock+Request";
const TOKEN_REPLY: &str = "DDS:Auth:Mock+Reply";
const TOKEN_FINAL: &str = "DDS:Auth:Mock+Final";

fn handshake_token(class_id: &str) -> Vec<u8> {
    DataHolder::new(class_id).to_cdr_le()
}

const TOKEN_IDENTITY: u8 = 0;
const TOKEN_STATUS: u8 = 1;
const TOKEN_PEER_CREDENTIAL: u8 = 2;

impl MockAuthenticationPlugin {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The names of the operations called on the plugin, in order.
    #[must_use]
    pub fn calls(&self) -> Arc<std::sync::Mutex<Vec<String>>> {
        Arc::clone(&self.calls)
    }

    fn record(&self, operation: &str) {
        if let Ok(mut calls) = self.calls.lock() {
            calls.push(operation.to_owned());
        }
    }

    fn next_id(&self) -> u64 {
        // `fetch_add` on AtomicU64 — no `&mut self` needed,
        // so `validate_*` can imply read-only semantics.
        self.next_handle.fetch_add(1, Ordering::Relaxed) + 1
    }

    fn issue(&mut self, kind: u8, token: Vec<u8>) -> Vec<u8> {
        self.tokens_out.issue(kind, token)
    }

    fn take_back(&mut self, kind: u8, token: &[u8]) -> SecurityResult<()> {
        self.tokens_out.take_back(kind, token)
    }

    fn local(&self, handle: IdentityHandle) -> SecurityResult<()> {
        if self.identities.contains(&handle) {
            Ok(())
        } else {
            Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "mock: unknown IdentityHandle",
            ))
        }
    }
}

impl AuthenticationExtensions for MockAuthenticationPlugin {
    fn plugin_class_id(&self) -> &str {
        "DDS:Auth:Mock"
    }
}

impl AuthenticationPlugin for MockAuthenticationPlugin {
    fn validate_local_identity(
        &mut self,
        _domain_id: u32,
        _participant_qos: &PropertyList,
        candidate_participant_guid: [u8; 16],
    ) -> SecurityResult<LocalIdentityValidation> {
        self.record("validate_local_identity");
        let identity = IdentityHandle(self.next_id());
        self.identities.insert(identity);
        Ok(LocalIdentityValidation {
            result: ValidationResult::Ok,
            identity,
            adjusted_participant_guid: candidate_participant_guid,
        })
    }

    fn get_identity_token(&mut self, handle: IdentityHandle) -> SecurityResult<Vec<u8>> {
        self.record("get_identity_token");
        self.local(handle)?;
        Ok(self.issue(TOKEN_IDENTITY, b"mock-identity-token".to_vec()))
    }

    fn get_identity_status_token(&mut self, handle: IdentityHandle) -> SecurityResult<Vec<u8>> {
        self.record("get_identity_status_token");
        self.local(handle)?;
        Ok(self.issue(TOKEN_STATUS, b"mock-identity-status-token".to_vec()))
    }

    fn set_participant_security_config(
        &mut self,
        handle: IdentityHandle,
        participant_security_config: &ParticipantSecurityConfig,
    ) -> SecurityResult<AlgorithmMasks> {
        self.record("set_participant_security_config");
        self.local(handle)?;
        Ok(participant_security_config.algorithm_info)
    }

    fn set_permissions_credential_and_token(
        &mut self,
        handle: IdentityHandle,
        _permissions_credential_token: &[u8],
        _permissions_token: &[u8],
    ) -> SecurityResult<()> {
        self.record("set_permissions_credential_and_token");
        self.local(handle)
    }

    fn validate_remote_identity(
        &mut self,
        local: IdentityHandle,
        remote_participant_guid: [u8; 16],
        _remote_identity_token: &[u8],
        _remote_auth_request_token: Option<&[u8]>,
    ) -> SecurityResult<RemoteIdentityValidation> {
        self.record("validate_remote_identity");
        self.local(local)?;
        let identity = IdentityHandle(self.next_id());
        self.identities.insert(identity);
        // Table 59: the participant with the smaller GUID starts.
        let result = if remote_participant_guid > [0x80; 16] {
            ValidationResult::PendingHandshakeRequest
        } else {
            ValidationResult::PendingHandshakeMessage
        };
        Ok(RemoteIdentityValidation {
            result,
            identity,
            local_auth_request_token: None,
        })
    }

    fn begin_handshake_request(
        &mut self,
        _initiator: IdentityHandle,
        _replier: IdentityHandle,
        _serialized_local_participant_data: &[u8],
    ) -> SecurityResult<(HandshakeHandle, HandshakeStepOutcome)> {
        self.record("begin_handshake_request");
        let h = HandshakeHandle(self.next_id());
        self.begun.insert(h);
        // Mock handshake: the request token is a fixed text.
        Ok((
            h,
            HandshakeStepOutcome::SendMessage {
                token: handshake_token(TOKEN_REQUEST),
            },
        ))
    }

    fn begin_handshake_reply(
        &mut self,
        _replier: IdentityHandle,
        _initiator: IdentityHandle,
        request_token: &[u8],
        _serialized_local_participant_data: &[u8],
    ) -> SecurityResult<(HandshakeHandle, HandshakeStepOutcome)> {
        self.record("begin_handshake_reply");
        if request_token != handshake_token(TOKEN_REQUEST).as_slice() {
            return Err(SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                "mock: unexpected request token",
            ));
        }
        let h = HandshakeHandle(self.next_id());
        self.begun.insert(h);
        // Reply token, on receipt of which the initiator can complete
        // the handshake.
        Ok((
            h,
            HandshakeStepOutcome::SendMessage {
                token: handshake_token(TOKEN_REPLY),
            },
        ))
    }

    fn process_handshake(
        &mut self,
        handshake: HandshakeHandle,
        token: &[u8],
    ) -> SecurityResult<HandshakeStepOutcome> {
        self.record("process_handshake");
        if token == handshake_token(TOKEN_REPLY).as_slice() {
            let secret = SharedSecretHandle(self.next_id());
            self.handshakes.insert(handshake, secret);
            return Ok(HandshakeStepOutcome::SendFinalMessage {
                token: handshake_token(TOKEN_FINAL),
            });
        }
        if token == handshake_token(TOKEN_FINAL).as_slice() {
            // Replier side completed.
            let secret = match self.handshakes.get(&handshake).copied() {
                Some(secret) => secret,
                None => {
                    let secret = SharedSecretHandle(self.next_id());
                    self.handshakes.insert(handshake, secret);
                    secret
                }
            };
            return Ok(HandshakeStepOutcome::Complete { secret });
        }
        Err(SecurityError::new(
            SecurityErrorKind::AuthenticationFailed,
            "mock: unknown handshake token",
        ))
    }

    fn shared_secret(&self, handshake: HandshakeHandle) -> SecurityResult<SharedSecretHandle> {
        self.record("shared_secret");
        self.handshakes.get(&handshake).copied().ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::BadArgument,
                "mock: handshake handle unknown",
            )
        })
    }

    fn get_authenticated_peer_credential_token(
        &mut self,
        handshake: HandshakeHandle,
    ) -> SecurityResult<Vec<u8>> {
        self.record("get_authenticated_peer_credential_token");
        if !self.handshakes.contains_key(&handshake) {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "mock: handshake not completed",
            ));
        }
        Ok(self.issue(
            TOKEN_PEER_CREDENTIAL,
            handshake_token("DDS:Auth:Mock+PeerCredential"),
        ))
    }

    fn set_listener(
        &mut self,
        listener: Option<Arc<dyn AuthenticationListener>>,
    ) -> SecurityResult<()> {
        self.record("set_listener");
        self.listener = listener;
        Ok(())
    }

    fn return_identity_token(&mut self, token: &[u8]) -> SecurityResult<()> {
        self.record("return_identity_token");
        self.take_back(TOKEN_IDENTITY, token)
    }

    fn return_identity_status_token(&mut self, token: &[u8]) -> SecurityResult<()> {
        self.record("return_identity_status_token");
        self.take_back(TOKEN_STATUS, token)
    }

    fn return_authenticated_peer_credential_token(&mut self, token: &[u8]) -> SecurityResult<()> {
        self.record("return_authenticated_peer_credential_token");
        self.take_back(TOKEN_PEER_CREDENTIAL, token)
    }

    fn return_handshake_handle(&mut self, handshake: HandshakeHandle) -> SecurityResult<()> {
        self.record("return_handshake_handle");
        if self.begun.remove(&handshake) {
            self.handshakes.remove(&handshake);
            Ok(())
        } else {
            Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "mock: unknown HandshakeHandle",
            ))
        }
    }

    fn return_identity_handle(&mut self, handle: IdentityHandle) -> SecurityResult<()> {
        self.record("return_identity_handle");
        if self.identities.remove(&handle) {
            Ok(())
        } else {
            Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "mock: unknown IdentityHandle",
            ))
        }
    }

    fn return_sharedsecret_handle(&mut self, handle: SharedSecretHandle) -> SecurityResult<()> {
        self.record("return_sharedsecret_handle");
        if self.handshakes.values().any(|s| *s == handle) {
            self.handshakes.retain(|_, s| *s != handle);
            Ok(())
        } else {
            Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "mock: unknown SharedSecretHandle",
            ))
        }
    }
}

// ============================================================================
// MockAccessControlPlugin — permits everything unless told otherwise, and
// records what it was asked.
// ============================================================================

/// One operation the [`MockAccessControlPlugin`] was asked, with what the
/// request carried.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MockAccessCall {
    /// The name of the operation of Table 39 (`check_create_datawriter`).
    pub operation: &'static str,
    /// The topic name of the request, if the operation has one.
    pub topic: Option<String>,
    /// The partitions of the request.
    pub partitions: Vec<String>,
    /// The data tags of the request as (name, value).
    pub data_tags: Vec<(String, String)>,
    /// The QoS entries of the request as (name, value).
    pub qos: Vec<(String, String)>,
    /// The serialized key of an instance operation.
    pub key: Option<Vec<u8>>,
    /// The properties of a participant request as (name, value).
    pub properties: Vec<(String, String)>,
    /// The domain id of the request, if it has one.
    pub domain_id: Option<u32>,
}

/// Changes the `TopicSecurityConfig` the mock returns for a topic.
type MockTopicConfig = alloc::sync::Arc<dyn Fn(&str, &mut TopicSecurityConfig) + Send + Sync>;

/// Changes the `EndpointSecurityConfig` the mock returns for an endpoint.
type MockEndpointConfig = alloc::sync::Arc<dyn Fn(&str, &mut EndpointSecurityConfig) + Send + Sync>;

/// What decides a [`MockAccessCall`]: `true` permits.
type MockDecision = alloc::sync::Arc<dyn Fn(&MockAccessCall) -> bool + Send + Sync>;

/// Mock access control: allows everything unless a decision function says
/// otherwise, and records every operation it was asked. Tests only.
pub struct MockAccessControlPlugin {
    next_handle: AtomicU64,
    listener: Option<alloc::sync::Arc<dyn AccessControlListener>>,
    calls: alloc::sync::Arc<std::sync::Mutex<Vec<MockAccessCall>>>,
    decide: Option<MockDecision>,
    relay_only: bool,
    participant_config: AccessParticipantSecurityConfig,
    topic_config: Option<MockTopicConfig>,
    endpoint_config: Option<MockEndpointConfig>,
}

/// A ParticipantSecurityConfig whose algorithm info does not restrict the
/// algorithms of the plugins: every supported mask is the whole set
/// (§9.4.2.9.22), nothing is required, nothing is protected.
fn unrestricted_participant_config() -> AccessParticipantSecurityConfig {
    use crate::access_control::{
        CryptoAlgorithmRequirements, DigitalSignatureAlgorithmInfo, KeyEstablishmentAlgorithmInfo,
        ParticipantSecurityAlgorithmInfo, SymmetricCipherAlgorithmInfo,
    };
    use crate::algorithm::CRYPTO_ALGORITHM_SET_ALL;
    let all = CryptoAlgorithmRequirements {
        supported_mask: CRYPTO_ALGORITHM_SET_ALL,
        required_mask: 0,
    };
    AccessParticipantSecurityConfig {
        algorithm_info: ParticipantSecurityAlgorithmInfo {
            digital_signature: DigitalSignatureAlgorithmInfo {
                trust_chain: all,
                message_auth: all,
            },
            key_establishment: KeyEstablishmentAlgorithmInfo { shared_secret: all },
            symmetric_cipher: SymmetricCipherAlgorithmInfo {
                supported_mask: CRYPTO_ALGORITHM_SET_ALL,
                builtin_endpoints_required_mask: 0,
                builtin_kx_endpoints_required_mask: 0,
                user_endpoints_default_required_mask: 0,
            },
        },
        ..AccessParticipantSecurityConfig::default()
    }
}

impl Default for MockAccessControlPlugin {
    fn default() -> Self {
        Self {
            next_handle: AtomicU64::new(0),
            listener: None,
            calls: alloc::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
            decide: None,
            relay_only: false,
            participant_config: unrestricted_participant_config(),
            topic_config: None,
            endpoint_config: None,
        }
    }
}

impl core::fmt::Debug for MockAccessControlPlugin {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("MockAccessControlPlugin")
            .field("listener", &self.listener.is_some())
            .finish_non_exhaustive()
    }
}

impl MockAccessControlPlugin {
    /// Constructor: permits everything.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A plugin that asks `decide` about every check (`true` permits) and
    /// records every operation in the returned list.
    #[must_use]
    pub fn deciding(
        decide: impl Fn(&MockAccessCall) -> bool + Send + Sync + 'static,
    ) -> (
        Self,
        alloc::sync::Arc<std::sync::Mutex<Vec<MockAccessCall>>>,
    ) {
        let plugin = Self {
            decide: Some(alloc::sync::Arc::new(decide)),
            ..Self::default()
        };
        let calls = alloc::sync::Arc::clone(&plugin.calls);
        (plugin, calls)
    }

    /// Changes the `ParticipantSecurityConfig` that
    /// `get_participant_security_config` returns.
    #[must_use]
    pub fn with_participant_config(
        mut self,
        change: impl FnOnce(&mut AccessParticipantSecurityConfig),
    ) -> Self {
        change(&mut self.participant_config);
        self
    }

    /// Changes the `TopicSecurityConfig` that `get_topic_security_config`
    /// returns for a topic: `change` gets the topic name and the default
    /// config.
    #[must_use]
    pub fn with_topic_config(
        mut self,
        change: impl Fn(&str, &mut TopicSecurityConfig) + Send + Sync + 'static,
    ) -> Self {
        self.topic_config = Some(alloc::sync::Arc::new(change));
        self
    }

    /// Changes the `EndpointSecurityConfig` that
    /// `get_datawriter_security_config` and `get_datareader_security_config`
    /// return: `change` gets the topic name of the request and the default
    /// config.
    #[must_use]
    pub fn with_endpoint_config(
        mut self,
        change: impl Fn(&str, &mut EndpointSecurityConfig) + Send + Sync + 'static,
    ) -> Self {
        self.endpoint_config = Some(alloc::sync::Arc::new(change));
        self
    }

    /// The plugin answers `RelayOnly` where it refuses a remote reader.
    #[must_use]
    pub fn relaying_refused_readers(mut self) -> Self {
        self.relay_only = true;
        self
    }

    /// The list of recorded operations.
    #[must_use]
    pub fn calls(&self) -> alloc::sync::Arc<std::sync::Mutex<Vec<MockAccessCall>>> {
        alloc::sync::Arc::clone(&self.calls)
    }

    fn next_id(&self) -> u64 {
        self.next_handle.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Revokes `handle` and tells the listener.
    pub fn revoke(&self, handle: PermissionsHandle) -> bool {
        self.listener
            .as_ref()
            .is_some_and(|l| l.on_revoke_permissions(self.plugin_class_id(), handle))
    }

    /// The listener the plugin was given, to call it as the plugin would.
    #[must_use]
    pub fn listener(&self) -> Option<alloc::sync::Arc<dyn AccessControlListener>> {
        self.listener.clone()
    }

    /// Records `call` and answers it.
    fn answer(&self, call: MockAccessCall) -> bool {
        let allowed = self.decide.as_ref().is_none_or(|decide| decide(&call));
        if let Ok(mut calls) = self.calls.lock() {
            calls.push(call);
        }
        allowed
    }

    fn call(operation: &'static str) -> MockAccessCall {
        MockAccessCall {
            operation,
            ..MockAccessCall::default()
        }
    }

    fn endpoint_call(operation: &'static str, request: &EndpointRequest<'_>) -> MockAccessCall {
        let pairs = |p: &[(&str, &str)]| {
            p.iter()
                .map(|(n, v)| ((*n).to_owned(), (*v).to_owned()))
                .collect::<Vec<_>>()
        };
        MockAccessCall {
            operation,
            topic: Some(request.topic_name.to_owned()),
            partitions: request.partitions.iter().map(|p| (*p).to_owned()).collect(),
            data_tags: pairs(request.data_tags),
            qos: pairs(request.qos),
            domain_id: Some(request.domain_id),
            ..MockAccessCall::default()
        }
    }

    fn participant_call(
        operation: &'static str,
        participant: &ParticipantRequest<'_>,
    ) -> MockAccessCall {
        MockAccessCall {
            operation,
            domain_id: Some(participant.domain_id),
            properties: participant
                .properties
                .entries()
                .iter()
                .map(|p| (p.name.to_string(), p.value.to_string()))
                .collect(),
            ..MockAccessCall::default()
        }
    }

    fn decision(&self, call: MockAccessCall) -> SecurityResult<AccessDecision> {
        Ok(AccessDecision::from(self.answer(call)))
    }
}

impl AccessControlPlugin for MockAccessControlPlugin {
    fn validate_local_permissions(
        &mut self,
        _identity: IdentityHandle,
        participant: &ParticipantRequest<'_>,
    ) -> SecurityResult<PermissionsHandle> {
        self.answer(Self::participant_call(
            "validate_local_permissions",
            participant,
        ));
        Ok(PermissionsHandle(self.next_id()))
    }

    fn validate_remote_permissions(
        &mut self,
        _local_identity: IdentityHandle,
        _remote_identity: IdentityHandle,
        _remote_permissions_token: &PermissionsToken,
        _remote_credential_token: &DataHolder,
    ) -> SecurityResult<PermissionsHandle> {
        self.answer(Self::call("validate_remote_permissions"));
        Ok(PermissionsHandle(self.next_id()))
    }

    fn check_create_participant(
        &self,
        _permissions: PermissionsHandle,
        participant: &ParticipantRequest<'_>,
    ) -> SecurityResult<AccessDecision> {
        self.decision(Self::participant_call(
            "check_create_participant",
            participant,
        ))
    }

    fn check_create_datawriter(
        &self,
        _p: PermissionsHandle,
        request: &EndpointRequest<'_>,
    ) -> SecurityResult<AccessDecision> {
        self.decision(Self::endpoint_call("check_create_datawriter", request))
    }

    fn check_create_datareader(
        &self,
        _p: PermissionsHandle,
        request: &EndpointRequest<'_>,
    ) -> SecurityResult<AccessDecision> {
        self.decision(Self::endpoint_call("check_create_datareader", request))
    }

    fn check_create_topic(
        &self,
        _p: PermissionsHandle,
        request: &TopicRequest<'_>,
    ) -> SecurityResult<AccessDecision> {
        self.decision(MockAccessCall {
            operation: "check_create_topic",
            topic: Some(request.topic_name.to_owned()),
            qos: request
                .qos
                .iter()
                .map(|(n, v)| ((*n).to_owned(), (*v).to_owned()))
                .collect(),
            domain_id: Some(request.domain_id),
            ..MockAccessCall::default()
        })
    }

    fn check_local_datawriter_register_instance(
        &self,
        _p: PermissionsHandle,
        _writer: [u8; 16],
        key: &[u8],
    ) -> SecurityResult<AccessDecision> {
        self.decision(MockAccessCall {
            key: Some(key.to_vec()),
            ..Self::call("check_local_datawriter_register_instance")
        })
    }

    fn check_local_datawriter_dispose_instance(
        &self,
        _p: PermissionsHandle,
        _writer: [u8; 16],
        key: &[u8],
    ) -> SecurityResult<AccessDecision> {
        self.decision(MockAccessCall {
            key: Some(key.to_vec()),
            ..Self::call("check_local_datawriter_dispose_instance")
        })
    }

    fn check_remote_participant(
        &self,
        _p: PermissionsHandle,
        participant: &ParticipantRequest<'_>,
    ) -> SecurityResult<AccessDecision> {
        self.decision(Self::participant_call(
            "check_remote_participant",
            participant,
        ))
    }

    fn check_remote_datawriter(
        &self,
        _r: PermissionsHandle,
        request: &EndpointRequest<'_>,
    ) -> SecurityResult<AccessDecision> {
        self.decision(Self::endpoint_call("check_remote_datawriter", request))
    }

    fn check_remote_datareader(
        &self,
        _r: PermissionsHandle,
        request: &EndpointRequest<'_>,
    ) -> SecurityResult<RemoteReaderDecision> {
        let allowed = self.answer(Self::endpoint_call("check_remote_datareader", request));
        Ok(if allowed {
            RemoteReaderDecision::Permit
        } else if self.relay_only {
            RemoteReaderDecision::RelayOnly
        } else {
            RemoteReaderDecision::Deny
        })
    }

    fn check_remote_topic(
        &self,
        _p: PermissionsHandle,
        request: &TopicRequest<'_>,
    ) -> SecurityResult<AccessDecision> {
        self.decision(MockAccessCall {
            operation: "check_remote_topic",
            topic: Some(request.topic_name.to_owned()),
            domain_id: Some(request.domain_id),
            ..MockAccessCall::default()
        })
    }

    fn check_local_datawriter_match(
        &self,
        _writer_permissions: PermissionsHandle,
        _reader_permissions: PermissionsHandle,
        publication: &EndpointRequest<'_>,
        _subscription: &EndpointRequest<'_>,
    ) -> SecurityResult<AccessDecision> {
        self.decision(Self::endpoint_call(
            "check_local_datawriter_match",
            publication,
        ))
    }

    fn check_local_datareader_match(
        &self,
        _writer_permissions: PermissionsHandle,
        _reader_permissions: PermissionsHandle,
        subscription: &EndpointRequest<'_>,
        _publication: &EndpointRequest<'_>,
    ) -> SecurityResult<AccessDecision> {
        self.decision(Self::endpoint_call(
            "check_local_datareader_match",
            subscription,
        ))
    }

    fn check_remote_datawriter_register_instance(
        &self,
        _p: PermissionsHandle,
        _reader: [u8; 16],
        _publication: [u8; 16],
        key: &[u8],
    ) -> SecurityResult<AccessDecision> {
        self.decision(MockAccessCall {
            key: Some(key.to_vec()),
            ..Self::call("check_remote_datawriter_register_instance")
        })
    }

    fn check_remote_datawriter_dispose_instance(
        &self,
        _p: PermissionsHandle,
        _reader: [u8; 16],
        _publication: [u8; 16],
        key: &[u8],
    ) -> SecurityResult<AccessDecision> {
        self.decision(MockAccessCall {
            key: Some(key.to_vec()),
            ..Self::call("check_remote_datawriter_dispose_instance")
        })
    }

    fn check_relay(
        &self,
        _permissions: PermissionsHandle,
        _role: RelayRole,
        request: &EndpointRequest<'_>,
    ) -> SecurityResult<AccessDecision> {
        self.decision(Self::endpoint_call("check_relay", request))
    }

    fn get_permissions_token(
        &mut self,
        _handle: PermissionsHandle,
    ) -> SecurityResult<PermissionsToken> {
        self.answer(Self::call("get_permissions_token"));
        Ok(DataHolder::new(self.plugin_class_id()))
    }

    fn get_permissions_credential_token(
        &mut self,
        _handle: PermissionsHandle,
    ) -> SecurityResult<PermissionsCredentialToken> {
        self.answer(Self::call("get_permissions_credential_token"));
        Ok(DataHolder::new("DDS:Access:MockCredential"))
    }

    fn set_listener(
        &mut self,
        listener: alloc::sync::Arc<dyn AccessControlListener>,
    ) -> SecurityResult<()> {
        self.listener = Some(listener);
        Ok(())
    }

    fn return_permissions_token(&mut self, _token: PermissionsToken) -> SecurityResult<()> {
        self.answer(Self::call("return_permissions_token"));
        Ok(())
    }

    fn return_permissions_credential_token(
        &mut self,
        _token: PermissionsCredentialToken,
    ) -> SecurityResult<()> {
        self.answer(Self::call("return_permissions_credential_token"));
        Ok(())
    }

    fn get_participant_security_config(
        &mut self,
        _permissions: PermissionsHandle,
    ) -> SecurityResult<AccessParticipantSecurityConfig> {
        self.answer(Self::call("get_participant_security_config"));
        Ok(self.participant_config.clone())
    }

    fn get_topic_security_config(
        &mut self,
        _permissions: PermissionsHandle,
        topic_name: &str,
    ) -> SecurityResult<TopicSecurityConfig> {
        self.answer(MockAccessCall {
            topic: Some(topic_name.to_owned()),
            ..Self::call("get_topic_security_config")
        });
        let mut config = TopicSecurityConfig::default();
        if let Some(change) = &self.topic_config {
            change(topic_name, &mut config);
        }
        Ok(config)
    }

    fn get_datawriter_security_config(
        &mut self,
        _permissions: PermissionsHandle,
        request: &EndpointRequest<'_>,
    ) -> SecurityResult<EndpointSecurityConfig> {
        self.answer(Self::endpoint_call(
            "get_datawriter_security_config",
            request,
        ));
        let mut config = EndpointSecurityConfig::default();
        if let Some(change) = &self.endpoint_config {
            change(request.topic_name, &mut config);
        }
        Ok(config)
    }

    fn get_datareader_security_config(
        &mut self,
        _permissions: PermissionsHandle,
        request: &EndpointRequest<'_>,
    ) -> SecurityResult<EndpointSecurityConfig> {
        self.answer(Self::endpoint_call(
            "get_datareader_security_config",
            request,
        ));
        let mut config = EndpointSecurityConfig::default();
        if let Some(change) = &self.endpoint_config {
            change(request.topic_name, &mut config);
        }
        Ok(config)
    }

    fn return_participant_security_config(
        &mut self,
        _config: AccessParticipantSecurityConfig,
    ) -> SecurityResult<()> {
        self.answer(Self::call("return_participant_security_config"));
        Ok(())
    }

    fn return_topic_security_config(&mut self, _config: TopicSecurityConfig) -> SecurityResult<()> {
        self.answer(Self::call("return_topic_security_config"));
        Ok(())
    }

    fn return_datawriter_security_config(
        &mut self,
        _config: EndpointSecurityConfig,
    ) -> SecurityResult<()> {
        self.answer(Self::call("return_datawriter_security_config"));
        Ok(())
    }

    fn return_datareader_security_config(
        &mut self,
        _config: EndpointSecurityConfig,
    ) -> SecurityResult<()> {
        self.answer(Self::call("return_datareader_security_config"));
        Ok(())
    }

    fn plugin_class_id(&self) -> &str {
        "DDS:Access:Mock:1.0"
    }
}

// ============================================================================
// MockLoggingPlugin — collects events in a Vec for test assertions
// ============================================================================

/// A log entry — collected for test assertions.
#[cfg(feature = "std")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MockLogEntry {
    /// Severity.
    pub level: LogLevel,
    /// Participant-GUID (16 octets).
    pub participant: [u8; 16],
    /// Category-String.
    pub category: String,
    /// Message.
    pub message: String,
}

/// Shared sink type — `Arc<Mutex<Vec<MockLogEntry>>>`.
#[cfg(feature = "std")]
pub type MockLogSink = std::sync::Arc<std::sync::Mutex<Vec<MockLogEntry>>>;

/// Mock logger: collects all events in a `MockLogSink` so tests
/// can inspect the events afterwards.
#[cfg(feature = "std")]
pub struct MockLoggingPlugin {
    sink: MockLogSink,
    control: std::sync::Mutex<MockLogControl>,
}

/// The state the Table 46 operations of [`MockLoggingPlugin`] keep.
#[cfg(feature = "std")]
#[derive(Default)]
struct MockLogControl {
    options: Option<crate::logging::LogOptions>,
    enabled: bool,
    listener: Option<std::sync::Arc<dyn crate::logging::LoggerListener>>,
}

#[cfg(feature = "std")]
impl MockLoggingPlugin {
    /// Constructor.
    #[must_use]
    pub fn new(sink: MockLogSink) -> Self {
        Self {
            sink,
            control: std::sync::Mutex::new(MockLogControl::default()),
        }
    }

    /// The options saved by `set_log_options`, if any.
    #[must_use]
    pub fn log_options(&self) -> Option<crate::logging::LogOptions> {
        self.control.lock().ok().and_then(|c| c.options.clone())
    }

    /// `true` once `enable_logging` was called.
    #[must_use]
    pub fn is_logging_enabled(&self) -> bool {
        self.control.lock().map(|c| c.enabled).unwrap_or(false)
    }
}

#[cfg(feature = "std")]
impl LoggingPlugin for MockLoggingPlugin {
    fn set_log_options(
        &self,
        options: crate::logging::LogOptions,
    ) -> Result<(), crate::error::SecurityError> {
        let mut c = self.control.lock().map_err(|_| {
            crate::error::SecurityError::new(
                crate::error::SecurityErrorKind::Internal,
                "the mock logger state is poisoned",
            )
        })?;
        if c.enabled {
            return Err(crate::error::SecurityError::bad_argument(
                "log options cannot be set after logging was enabled",
            ));
        }
        c.options = Some(options);
        Ok(())
    }

    fn enable_logging(&self) -> Result<(), crate::error::SecurityError> {
        let mut c = self.control.lock().map_err(|_| {
            crate::error::SecurityError::new(
                crate::error::SecurityErrorKind::Internal,
                "the mock logger state is poisoned",
            )
        })?;
        if c.options.is_none() {
            return Err(crate::error::SecurityError::bad_argument(
                "logging cannot be enabled before log options are set",
            ));
        }
        c.enabled = true;
        Ok(())
    }

    fn set_listener(
        &self,
        listener: Option<std::sync::Arc<dyn crate::logging::LoggerListener>>,
    ) -> Result<(), crate::error::SecurityError> {
        let mut c = self.control.lock().map_err(|_| {
            crate::error::SecurityError::new(
                crate::error::SecurityErrorKind::Internal,
                "the mock logger state is poisoned",
            )
        })?;
        c.listener = listener;
        Ok(())
    }

    fn log(&self, level: LogLevel, participant: [u8; 16], category: &str, message: &str) {
        let listener = self.control.lock().ok().and_then(|c| c.listener.clone());
        if let Some(l) = listener {
            l.log_message(level, participant, category, message);
        }
        if let Ok(mut v) = self.sink.lock() {
            v.push(MockLogEntry {
                level,
                participant,
                category: category.to_owned(),
                message: message.to_owned(),
            });
        }
    }

    fn plugin_class_id(&self) -> &str {
        "DDS:Logging:Mock"
    }
}

// ============================================================================
// MockDataTaggingPlugin — minimal tag store for tests
// ============================================================================

/// Mock DataTagging plugin: stores tag lists per endpoint GUID in
/// an in-memory map. Returns an empty Vec for an unknown GUID
/// (spec-compliant default).
#[derive(Debug, Default)]
pub struct MockDataTaggingPlugin {
    tags: BTreeMap<[u8; 16], Vec<DataTag>>,
}

impl MockDataTaggingPlugin {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl DataTaggingPlugin for MockDataTaggingPlugin {
    fn set_tags(&mut self, endpoint_guid: [u8; 16], tags: Vec<DataTag>) {
        self.tags.insert(endpoint_guid, tags);
    }

    fn get_tags(&self, endpoint_guid: [u8; 16]) -> Vec<DataTag> {
        self.tags.get(&endpoint_guid).cloned().unwrap_or_default()
    }

    fn plugin_class_id(&self) -> &str {
        "DDS:Tagging:Mock"
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn mock_authentication_end_to_end_handshake() {
        // Two plugins simulate two participants.
        let mut alice = MockAuthenticationPlugin::new();
        let mut bob = MockAuthenticationPlugin::new();

        let alice_id = alice
            .validate_local_identity(0, &PropertyList::new(), [0x01; 16])
            .expect("alice identity")
            .identity;
        let bob_id = bob
            .validate_local_identity(0, &PropertyList::new(), [0xFF; 16])
            .expect("bob identity")
            .identity;

        // Alice sees Bob via SPDP.
        let bob_remote_at_alice = alice
            .validate_remote_identity(alice_id, [0xFF; 16], b"mock-bob-token", None)
            .expect("bob-remote");
        assert_eq!(
            bob_remote_at_alice.result,
            ValidationResult::PendingHandshakeRequest
        );
        let alice_remote_at_bob = bob
            .validate_remote_identity(bob_id, [0x01; 16], b"mock-alice-token", None)
            .expect("alice-remote");
        assert_eq!(
            alice_remote_at_bob.result,
            ValidationResult::PendingHandshakeMessage
        );

        // Alice startet Handshake.
        let (alice_h, outcome1) = alice
            .begin_handshake_request(alice_id, bob_remote_at_alice.identity, &[])
            .expect("request");
        let request_token = match outcome1 {
            HandshakeStepOutcome::SendMessage { token } => token,
            other => panic!("expected SendMessage, got {other:?}"),
        };

        // Bob answers.
        let (bob_h, outcome2) = bob
            .begin_handshake_reply(bob_id, alice_remote_at_bob.identity, &request_token, &[])
            .expect("reply");
        let reply_token = match outcome2 {
            HandshakeStepOutcome::SendMessage { token } => token,
            other => panic!("expected SendMessage, got {other:?}"),
        };

        // Alice processes the reply: VALIDATION_OK_FINAL_MESSAGE.
        let outcome3 = alice
            .process_handshake(alice_h, &reply_token)
            .expect("proc");
        let final_token = match outcome3 {
            HandshakeStepOutcome::SendFinalMessage { token } => token,
            other => panic!("expected SendFinalMessage, got {other:?}"),
        };

        // Bob side: completes with the final message.
        let outcome4 = bob
            .process_handshake(bob_h, &final_token)
            .expect("proc bob");
        assert!(matches!(outcome4, HandshakeStepOutcome::Complete { .. }));

        // The secret handle on Alice's side is queryable.
        let fetched = alice.shared_secret(alice_h).expect("fetch");
        assert_ne!(fetched, SharedSecretHandle::NIL);
    }

    #[test]
    fn mock_authentication_returns_what_it_issued() {
        let mut p = MockAuthenticationPlugin::new();
        let id = p
            .validate_local_identity(0, &PropertyList::new(), [1; 16])
            .expect("identity")
            .identity;
        let token = p.get_identity_token(id).expect("token");
        p.return_identity_token(&token).expect("issued");
        assert!(p.return_identity_token(&token).is_err(), "twice");
        assert!(p.return_identity_status_token(&token).is_err(), "kind");
        p.return_identity_handle(id).expect("issued handle");
        assert!(p.return_identity_handle(id).is_err(), "twice");
        assert!(p.get_identity_token(id).is_err(), "after return");
    }

    #[test]
    fn mock_access_control_permits_everything() {
        let mut ac = MockAccessControlPlugin::new();
        let local = IdentityHandle(1);
        let props = PropertyList::new();
        let perms = ac
            .validate_local_permissions(
                local,
                &crate::access_control::ParticipantRequest::new(0, [0xAA; 16], &props),
            )
            .expect("perms");
        assert!(
            ac.check_create_datawriter(
                perms,
                &crate::access_control::EndpointRequest::new(0, "Chatter")
            )
            .unwrap()
            .is_permitted()
        );
        assert!(
            ac.check_create_datareader(
                perms,
                &crate::access_control::EndpointRequest::new(0, "Chatter")
            )
            .unwrap()
            .is_permitted()
        );
    }

    #[test]
    fn auth_plugin_can_be_boxed_as_trait_object() {
        let plugin: Box<dyn AuthenticationPlugin> = Box::new(MockAuthenticationPlugin::new());
        assert_eq!(plugin.plugin_class_id(), "DDS:Auth:Mock");
    }

    #[test]
    fn mock_data_tagging_set_get_roundtrip() {
        let mut tagger = MockDataTaggingPlugin::new();
        let g = [0xAB; 16];
        let tags = alloc::vec![DataTag {
            name: "classification".into(),
            value: "secret".into(),
        }];
        tagger.set_tags(g, tags.clone());
        assert_eq!(tagger.get_tags(g), tags);
        assert!(tagger.get_tags([0xCD; 16]).is_empty());
        assert_eq!(tagger.plugin_class_id(), "DDS:Tagging:Mock");
    }

    #[cfg(feature = "std")]
    #[test]
    fn mock_logging_captures_events() {
        use std::sync::{Arc, Mutex};
        let sink = Arc::new(Mutex::new(Vec::new()));
        let logger = MockLoggingPlugin::new(Arc::clone(&sink));
        logger.log(LogLevel::Critical, [0u8; 16], "auth.failed", "bad cert");
        let captured = sink.lock().unwrap();
        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0].level, LogLevel::Critical);
        assert_eq!(captured[0].category, "auth.failed");
        assert_eq!(captured[0].message, "bad cert");
    }
}
