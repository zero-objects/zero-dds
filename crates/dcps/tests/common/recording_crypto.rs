// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
//! A Cryptographic plugin that records the operations of the three interfaces
//! of DDS-Security 1.2 §9.5.1 that the middleware calls, and delegates every
//! one of them to the builtin `AesGcmCryptoPlugin`.

#![allow(dead_code, clippy::too_many_arguments)]

use std::sync::{Arc, Mutex};

use zerodds_security::access_control::PermissionsHandle;
use zerodds_security::algorithm::CryptoTransformKeyRevisionIntHolder;
use zerodds_security::authentication::{IdentityHandle, SharedSecretHandle};
use zerodds_security::cryptographic::{
    CryptoKeyExchange, CryptoKeyFactory, CryptoTransform, CryptographicPluginInterfaces,
    DatareaderCryptoHandle, DatawriterCryptoHandle, EncodedPayload, EndpointSecurityAlgorithmInfo,
    EndpointSecurityConfig, GuidT, ParticipantCryptoHandle, ParticipantSecurityAlgorithmInfo,
    ParticipantSecurityConfig, PreprocessedSubmessage,
};
use zerodds_security::error::SecurityResult;
use zerodds_security::properties::Property;
use zerodds_security::token::CryptoToken;
use zerodds_security_crypto::AesGcmCryptoPlugin;

/// One recorded call: the operation and the properties it was given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    pub operation: &'static str,
    /// The (name, value) properties of a registration.
    pub properties: Vec<(String, String)>,
    /// `is_rtps_axk_protected` of a participant config, `None` for others.
    pub participant_axk: Option<bool>,
}

pub type Log = Arc<Mutex<Vec<Call>>>;

pub struct RecordingCrypto {
    inner: AesGcmCryptoPlugin,
    log: Log,
}

impl RecordingCrypto {
    pub fn new() -> (Self, Log) {
        let log: Log = Arc::default();
        (
            Self {
                inner: AesGcmCryptoPlugin::new(),
                log: Arc::clone(&log),
            },
            log,
        )
    }

    fn note(&self, operation: &'static str) {
        self.record(operation, &[], None);
    }

    fn record(&self, operation: &'static str, props: &[Property], axk: Option<bool>) {
        self.log.lock().unwrap().push(Call {
            operation,
            properties: props
                .iter()
                .map(|p| (p.name.to_string(), p.value.to_string()))
                .collect(),
            participant_axk: axk,
        });
    }
}

impl CryptoKeyFactory for RecordingCrypto {
    fn register_local_participant(
        &mut self,
        i: IdentityHandle,
        p: PermissionsHandle,
        props: &[Property],
        config: &ParticipantSecurityConfig,
    ) -> SecurityResult<(ParticipantCryptoHandle, ParticipantSecurityAlgorithmInfo)> {
        self.record(
            "register_local_participant",
            props,
            Some(config.is_rtps_axk_protected),
        );
        self.inner.register_local_participant(i, p, props, config)
    }
    fn register_matched_remote_participant(
        &mut self,
        l: ParticipantCryptoHandle,
        i: IdentityHandle,
        p: PermissionsHandle,
        s: SharedSecretHandle,
    ) -> SecurityResult<ParticipantCryptoHandle> {
        self.note("register_matched_remote_participant");
        self.inner.register_matched_remote_participant(l, i, p, s)
    }
    fn register_local_datawriter(
        &mut self,
        p: ParticipantCryptoHandle,
        props: &[Property],
        c: &EndpointSecurityConfig,
        g: GuidT,
    ) -> SecurityResult<(DatawriterCryptoHandle, EndpointSecurityAlgorithmInfo)> {
        self.record("register_local_datawriter", props, None);
        self.inner.register_local_datawriter(p, props, c, g)
    }
    fn register_matched_remote_datareader(
        &mut self,
        l: DatawriterCryptoHandle,
        r: ParticipantCryptoHandle,
        s: SharedSecretHandle,
        relay_only: bool,
    ) -> SecurityResult<DatareaderCryptoHandle> {
        self.note("register_matched_remote_datareader");
        self.inner
            .register_matched_remote_datareader(l, r, s, relay_only)
    }
    fn register_local_datareader(
        &mut self,
        p: ParticipantCryptoHandle,
        props: &[Property],
        c: &EndpointSecurityConfig,
        g: GuidT,
    ) -> SecurityResult<(DatareaderCryptoHandle, EndpointSecurityAlgorithmInfo)> {
        self.record("register_local_datareader", props, None);
        self.inner.register_local_datareader(p, props, c, g)
    }
    fn register_matched_remote_datawriter(
        &mut self,
        l: DatareaderCryptoHandle,
        r: ParticipantCryptoHandle,
        s: SharedSecretHandle,
    ) -> SecurityResult<DatawriterCryptoHandle> {
        self.note("register_matched_remote_datawriter");
        self.inner.register_matched_remote_datawriter(l, r, s)
    }
    fn revise_local_entity_keys(
        &mut self,
        p: ParticipantCryptoHandle,
    ) -> SecurityResult<CryptoTransformKeyRevisionIntHolder> {
        self.note("revise_local_entity_keys");
        self.inner.revise_local_entity_keys(p)
    }
    fn activate_key_revision(
        &mut self,
        p: ParticipantCryptoHandle,
        r: CryptoTransformKeyRevisionIntHolder,
    ) -> SecurityResult<bool> {
        self.note("activate_key_revision");
        self.inner.activate_key_revision(p, r)
    }
    fn unregister_participant(&mut self, p: ParticipantCryptoHandle) -> SecurityResult<bool> {
        self.note("unregister_participant");
        self.inner.unregister_participant(p)
    }
    fn unregister_datawriter(&mut self, h: DatawriterCryptoHandle) -> SecurityResult<bool> {
        self.note("unregister_datawriter");
        self.inner.unregister_datawriter(h)
    }
    fn unregister_datareader(&mut self, h: DatareaderCryptoHandle) -> SecurityResult<bool> {
        self.note("unregister_datareader");
        self.inner.unregister_datareader(h)
    }
}

impl CryptoKeyExchange for RecordingCrypto {
    fn create_local_participant_crypto_tokens(
        &mut self,
        l: ParticipantCryptoHandle,
        r: ParticipantCryptoHandle,
        k: CryptoTransformKeyRevisionIntHolder,
    ) -> SecurityResult<Option<Vec<CryptoToken>>> {
        self.note("create_local_participant_crypto_tokens");
        self.inner.create_local_participant_crypto_tokens(l, r, k)
    }
    fn set_remote_participant_crypto_tokens(
        &mut self,
        l: ParticipantCryptoHandle,
        r: ParticipantCryptoHandle,
        t: &[CryptoToken],
    ) -> SecurityResult<bool> {
        self.note("set_remote_participant_crypto_tokens");
        self.inner.set_remote_participant_crypto_tokens(l, r, t)
    }
    fn create_local_datawriter_crypto_tokens(
        &mut self,
        l: DatawriterCryptoHandle,
        r: DatareaderCryptoHandle,
        k: CryptoTransformKeyRevisionIntHolder,
    ) -> SecurityResult<Option<Vec<CryptoToken>>> {
        self.note("create_local_datawriter_crypto_tokens");
        self.inner.create_local_datawriter_crypto_tokens(l, r, k)
    }
    fn set_remote_datawriter_crypto_tokens(
        &mut self,
        l: DatareaderCryptoHandle,
        r: DatawriterCryptoHandle,
        t: &[CryptoToken],
    ) -> SecurityResult<bool> {
        self.note("set_remote_datawriter_crypto_tokens");
        self.inner.set_remote_datawriter_crypto_tokens(l, r, t)
    }
    fn create_local_datareader_crypto_tokens(
        &mut self,
        l: DatareaderCryptoHandle,
        r: DatawriterCryptoHandle,
        k: CryptoTransformKeyRevisionIntHolder,
    ) -> SecurityResult<Option<Vec<CryptoToken>>> {
        self.note("create_local_datareader_crypto_tokens");
        self.inner.create_local_datareader_crypto_tokens(l, r, k)
    }
    fn set_remote_datareader_crypto_tokens(
        &mut self,
        l: DatawriterCryptoHandle,
        r: DatareaderCryptoHandle,
        t: &[CryptoToken],
    ) -> SecurityResult<bool> {
        self.note("set_remote_datareader_crypto_tokens");
        self.inner.set_remote_datareader_crypto_tokens(l, r, t)
    }
    fn return_crypto_tokens(&mut self, t: Vec<CryptoToken>) -> SecurityResult<bool> {
        self.note("return_crypto_tokens");
        self.inner.return_crypto_tokens(t)
    }
}

impl CryptoTransform for RecordingCrypto {
    fn encode_serialized_payload(
        &self,
        plain: &[u8],
        w: DatawriterCryptoHandle,
    ) -> SecurityResult<Option<EncodedPayload>> {
        self.note("encode_serialized_payload");
        self.inner.encode_serialized_payload(plain, w)
    }
    fn encode_datawriter_submessage(
        &self,
        out: &mut Vec<u8>,
        plain: &[u8],
        w: DatawriterCryptoHandle,
        list: &[DatareaderCryptoHandle],
        index: &mut usize,
    ) -> SecurityResult<bool> {
        self.note("encode_datawriter_submessage");
        self.inner
            .encode_datawriter_submessage(out, plain, w, list, index)
    }
    fn encode_datareader_submessage(
        &self,
        plain: &[u8],
        r: DatareaderCryptoHandle,
        list: &[DatawriterCryptoHandle],
    ) -> SecurityResult<Option<Vec<u8>>> {
        self.note("encode_datareader_submessage");
        self.inner.encode_datareader_submessage(plain, r, list)
    }
    fn encode_rtps_message(
        &self,
        out: &mut Vec<u8>,
        plain: &[u8],
        s: ParticipantCryptoHandle,
        list: &[ParticipantCryptoHandle],
        index: &mut usize,
        psk: bool,
    ) -> SecurityResult<bool> {
        self.note("encode_rtps_message");
        self.inner
            .encode_rtps_message(out, plain, s, list, index, psk)
    }
    fn decode_rtps_message(
        &self,
        encoded: &[u8],
        r: ParticipantCryptoHandle,
        s: ParticipantCryptoHandle,
    ) -> SecurityResult<Option<Vec<u8>>> {
        self.note("decode_rtps_message");
        self.inner.decode_rtps_message(encoded, r, s)
    }
    fn preprocess_secure_submsg(
        &self,
        encoded: &[u8],
        r: ParticipantCryptoHandle,
        s: ParticipantCryptoHandle,
    ) -> SecurityResult<Option<PreprocessedSubmessage>> {
        self.note("preprocess_secure_submsg");
        self.inner.preprocess_secure_submsg(encoded, r, s)
    }
    fn decode_datawriter_submessage(
        &self,
        encoded: &[u8],
        r: DatareaderCryptoHandle,
        s: DatawriterCryptoHandle,
    ) -> SecurityResult<Option<Vec<u8>>> {
        self.note("decode_datawriter_submessage");
        self.inner.decode_datawriter_submessage(encoded, r, s)
    }
    fn decode_datareader_submessage(
        &self,
        encoded: &[u8],
        r: DatawriterCryptoHandle,
        s: DatareaderCryptoHandle,
    ) -> SecurityResult<Option<Vec<u8>>> {
        self.note("decode_datareader_submessage");
        self.inner.decode_datareader_submessage(encoded, r, s)
    }
    fn decode_serialized_payload(
        &self,
        encoded: &[u8],
        inline_qos: &[u8],
        r: DatareaderCryptoHandle,
        s: DatawriterCryptoHandle,
    ) -> SecurityResult<Option<Vec<u8>>> {
        self.note("decode_serialized_payload");
        self.inner
            .decode_serialized_payload(encoded, inline_qos, r, s)
    }
}

impl CryptographicPluginInterfaces for RecordingCrypto {
    fn vendor_forms(&self) -> Option<&dyn zerodds_security::cryptographic::VendorFormCrypto> {
        self.inner.vendor_forms()
    }
}
