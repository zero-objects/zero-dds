// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-c-api` with `zerodds-security-runtime`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-security-runtime`, all in
//! `security_ffi.rs` (only with its feature `security`, see `Cargo.toml`):
//! - `vendor_form_settings_from_env(env)`: the vendor-form settings of the
//!   six-path configuration and of the application-plugin path; a value it
//!   does not know makes `zerodds_runtime_create_secure` return NULL.
//! - `SecurityProfileConfig`, written field by field with `Material` from a
//!   `PathBuf` (`Material::File(PathBuf::new())` for the identity files the
//!   application-plugin path does not read) and
//!   `PartitionMatching::Subset`.
//! - `governance_from_files(&cfg)`, `builtin_crypto_plugin(&cfg,
//!   &governance, secrets)`, `gate_with_crypto(&cfg, &governance, crypto)`:
//!   the gate of the application-plugin path; an error is printed with
//!   `Display`.
//! - `Governance::{check_create_participant, find_domain_rule}` (without a
//!   domain rule the participant is not created),
//!   `SecurityProfile::from_enclave_dir` and its fields `gate`, `pki`,
//!   `identity_handle`, `adjusted_participant_guid`, `governance`,
//!   `permissions`, `arm_endpoint_access_control`, `SharedSecurityGate`,
//!   `profile::LOCAL_VALIDATION_RETRY_DELAY` (the pause between two
//!   `validate_local_identity` calls that returned PENDING_RETRY).
//!
//! What it relies on: without any `ZERODDS_SEC_*` variable every form is
//! that of the specification; the variables this crate documents set the
//! forms; a value outside the allowed set is an error; a governance that is
//! missing, signed by another CA, or no governance at all is an error with a
//! message; the gate takes the settings of the configuration; an enclave
//! directory without its files is an error that names the file.
//!
//! Oracles: DDS-Security 1.2 §7.3.3.1 (algorithm names with the NUL),
//! §8.2 Table 25 and §8.3 Table 26 (the 1.2 algorithm names),
//! §9.4.1.2.3 (subject names in RFC 4514 form), §10.4.1.5.3.4.1.4 (subset
//! matching of partitions, "legacy" a selectable alternative), §10.4.1.1
//! and §10.4.1.2 (the governance is signed by the Permissions CA and has
//! `<domain_access_rules>`), §9.3.2.11.2 ("called again after a
//! configurable delay"); the variable names in the rustdoc of this crate's
//! `try_to_profile_cfg_with_env`; the enclave layout in the rustdoc of
//! `SecurityProfile::from_enclave_dir`; documents OpenSSL signed
//! (`crates/security-permissions/tests/fixtures/`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use std::sync::Arc;

use zerodds_security::authentication::{SharedSecretHandle, SharedSecretProvider};
use zerodds_security_runtime::{
    AlgoNameNul, AlgorithmNames, Governance, Material, PartitionMatching, SecurityProfile,
    SecurityProfileConfig, SubjectNameForm, builtin_crypto_plugin, gate_with_crypto,
    governance_from_files, vendor_form_settings_from_env,
};

/// The Permissions CA of the OpenSSL fixtures.
const OPENSSL_CA: &[u8] =
    include_bytes!("../../security-permissions/tests/fixtures/permissions_ca_cert.pem");
/// A permissions document the CA signed, not a governance.
const OPENSSL_PERMISSIONS: &[u8] =
    include_bytes!("../../security-permissions/tests/fixtures/valid_permissions.smime.p7s");
/// A document another CA signed.
const OPENSSL_OTHER_CA: &[u8] =
    include_bytes!("../../security-permissions/tests/fixtures/wrong_ca_permissions.smime.p7s");

/// The configuration of the application-plugin path: the identity files are
/// placeholders, the vendor forms are those of the environment `env`.
fn config(
    permissions_ca: Material,
    governance: Material,
    env: &dyn Fn(&str) -> Option<String>,
) -> SecurityProfileConfig {
    let forms = vendor_form_settings_from_env(env).unwrap();
    let unused = || Material::File(PathBuf::new());
    SecurityProfileConfig {
        domain_id: 0,
        identity_ca_pem: unused(),
        identity_cert_pem: unused(),
        identity_key_pem: unused(),
        permissions_ca_pem: permissions_ca,
        governance_p7s: governance,
        permissions_p7s: unused(),
        identity_key_password: None,
        identity_key_password_reading: Default::default(),
        symmetric_cipher_bits: 256,
        max_blocks_per_session: None,
        partition_matching: PartitionMatching::Subset,
        plaintext_loopback: false,
        compatibility_checks: forms.compatibility_checks,
        kx_key_fallback: forms.kx_key_fallback,
        algo_name_nul: forms.algo_name_nul,
        algorithm_names: forms.algorithm_names,
        subject_name_form: forms.subject_name_form,
        origin_authentication_fallback: forms.origin_authentication_fallback,
        identity_crl: None,
    }
}

fn no_environment(_name: &str) -> Option<String> {
    None
}

#[test]
fn security_7_3_3_1_without_variables_every_vendor_form_is_the_spec_form() {
    let forms = vendor_form_settings_from_env(&no_environment).unwrap();
    assert_eq!(forms.algo_name_nul, AlgoNameNul::Spec);
    assert_eq!(forms.algorithm_names, AlgorithmNames::V1_2);
    assert_eq!(forms.subject_name_form, SubjectNameForm::Rfc4514);
    assert!(forms.compatibility_checks);
    assert!(!forms.kx_key_fallback);
    assert!(!forms.origin_authentication_fallback);
}

#[test]
fn the_variables_this_crate_documents_set_the_vendor_forms() {
    let env = |name: &str| -> Option<String> {
        let value = match name {
            "ZERODDS_SEC_KX_KEY_FALLBACK" => "accept",
            "ZERODDS_SEC_COMPATIBILITY_CHECK" => "ignore",
            "ZERODDS_SEC_ALGO_NAME_NUL" => "mirror",
            "ZERODDS_SEC_SUBJECT_NAME_FORM" => "x509_name_oneline",
            "ZERODDS_SEC_ALGORITHM_NAMES" => "1.1",
            "ZERODDS_SEC_ORIGIN_AUTHENTICATION_FALLBACK" => "accept",
            _ => return None,
        };
        Some(value.to_string())
    };
    let forms = vendor_form_settings_from_env(&env).unwrap();
    assert!(forms.kx_key_fallback);
    assert!(!forms.compatibility_checks);
    assert_eq!(forms.algo_name_nul, AlgoNameNul::Mirror);
    assert_eq!(forms.subject_name_form, SubjectNameForm::X509NameOneline);
    assert_eq!(forms.algorithm_names, AlgorithmNames::V1_1);
    assert!(forms.origin_authentication_fallback);
}

#[test]
fn a_vendor_form_value_outside_the_allowed_set_is_an_error() {
    for variable in [
        "ZERODDS_SEC_KX_KEY_FALLBACK",
        "ZERODDS_SEC_COMPATIBILITY_CHECK",
        "ZERODDS_SEC_ALGO_NAME_NUL",
        "ZERODDS_SEC_SUBJECT_NAME_FORM",
        "ZERODDS_SEC_ALGORITHM_NAMES",
    ] {
        let env = |name: &str| (name == variable).then(|| "sometimes".to_string());
        assert!(
            vendor_form_settings_from_env(&env).is_err(),
            "{variable}=sometimes"
        );
    }
}

#[test]
fn security_10_4_1_5_partitions_match_as_a_subset_by_default() {
    assert_eq!(PartitionMatching::default(), PartitionMatching::Subset);
    assert_ne!(PartitionMatching::Subset, PartitionMatching::Legacy);
}

#[test]
fn a_material_from_a_path_is_that_file() {
    let path = PathBuf::from("/etc/dds/governance.p7s");
    let material: Material = path.clone().into();
    assert_eq!(material, Material::File(path.clone()));
    assert_eq!(material.path(), Some(path.as_path()));
    // The placeholder of the application-plugin path is never read; read,
    // it is an error rather than empty content.
    assert!(Material::File(PathBuf::new()).read().is_err());
    assert_eq!(
        Material::Inline(b"pem".to_vec()).read().unwrap(),
        b"pem".to_vec()
    );
}

#[test]
fn security_10_4_1_2_a_signed_document_without_domain_access_rules_is_no_governance() {
    let cfg = config(
        Material::Inline(OPENSSL_CA.to_vec()),
        Material::Inline(OPENSSL_PERMISSIONS.to_vec()),
        &no_environment,
    );
    let err = governance_from_files(&cfg).expect_err("a permissions document");
    assert!(!err.to_string().is_empty(), "printed by the FFI");
}

#[test]
fn security_10_4_1_1_a_governance_another_ca_signed_is_refused() {
    let cfg = config(
        Material::Inline(OPENSSL_CA.to_vec()),
        Material::Inline(OPENSSL_OTHER_CA.to_vec()),
        &no_environment,
    );
    assert!(governance_from_files(&cfg).is_err());
}

#[test]
fn a_governance_file_that_is_not_there_is_an_error_naming_it() {
    let missing = std::env::temp_dir().join(format!(
        "zerodds-contract-{}-no-governance.p7s",
        std::process::id()
    ));
    let cfg = config(
        Material::Inline(OPENSSL_CA.to_vec()),
        missing.into(),
        &no_environment,
    );
    let err = governance_from_files(&cfg).expect_err("no such file");
    assert!(
        err.to_string().contains("no-governance.p7s"),
        "the message names the file: {err}"
    );
}

/// The secrets of a plugin of the application, none here.
struct NoSecrets;

impl SharedSecretProvider for NoSecrets {
    fn get_shared_secret(&self, _handle: SharedSecretHandle) -> Option<Vec<u8>> {
        None
    }
}

#[test]
fn the_gate_takes_the_settings_of_the_configuration() {
    let env = |name: &str| (name == "ZERODDS_SEC_ALGO_NAME_NUL").then(|| "omit".to_string());
    let cfg = config(
        Material::Inline(OPENSSL_CA.to_vec()),
        Material::Inline(Vec::new()),
        &env,
    );
    let governance = Governance::default();
    let crypto = builtin_crypto_plugin(&cfg, &governance, Arc::new(NoSecrets));
    let gate = gate_with_crypto(&cfg, &governance, Box::new(crypto)).unwrap();
    assert_eq!(gate.symmetric_cipher_bits().unwrap(), 256);
    assert_eq!(gate.algo_name_nul().unwrap(), AlgoNameNul::Omit);
    assert_eq!(
        gate.compatibility_checks().unwrap(),
        cfg.compatibility_checks
    );
    assert_eq!(gate.kx_key_fallback().unwrap(), cfg.kx_key_fallback);
    // A governance without a rule for the domain: the application-plugin
    // path refuses the participant.
    assert!(governance.find_domain_rule(cfg.domain_id).is_none());
}

#[test]
fn an_enclave_directory_without_its_files_is_an_error_naming_the_file() {
    let dir = std::env::temp_dir().join(format!(
        "zerodds-contract-{}-no-enclave",
        std::process::id()
    ));
    let err = SecurityProfile::from_enclave_dir(dir, 0, [0x01; 16])
        .expect_err("the directory does not exist");
    let message = err.to_string();
    assert!(message.contains("no-enclave"), "{message}");
    assert!(
        message.contains(".pem") || message.contains(".p7s"),
        "{message}"
    );
}

#[test]
fn security_9_3_2_11_2_the_retry_delay_is_a_pause() {
    let delay = zerodds_security_runtime::profile::LOCAL_VALIDATION_RETRY_DELAY;
    assert!(!delay.is_zero(), "a zero delay would spin");
    assert!(
        delay.as_secs() < 25,
        "a retry fits the time budget of a call"
    );
}
