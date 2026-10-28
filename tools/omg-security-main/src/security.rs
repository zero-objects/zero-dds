// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `dds.sec.*` configuration: the flags become a `PropertyQosPolicy`.
//!
//! This module builds the property list and stops there. The suite
//! configures security the way DDS-Security 1.2 §7.3.21 / Tables 50, 63, 71 say to —
//! named properties on the `DomainParticipantQos`'s `PropertyQosPolicy` —
//! and §9.3.1 Table 44 plus §9.4.1.2 Table 46 name the properties
//! themselves. `srcCxx/opendds/ShapeType_configurator_opendds.h` is the
//! committed proof of the shape: it appends exactly
//! `dds.sec.auth.identity_ca`, `dds.sec.auth.identity_certificate`,
//! `dds.sec.auth.private_key`, `dds.sec.access.permissions_ca`,
//! `dds.sec.access.governance` and `dds.sec.access.permissions` and then
//! creates the participant.
//!
//! **Nothing here reads the list back.** `crates/dcps` does, in
//! `DomainParticipant::new_with_runtime` via
//! `zerodds_security_runtime::security_bundle_from_properties`: it
//! resolves the `file:` URIs, builds the one `SecurityProfile`, runs
//! §8.4.2.9.3 `check_create_participant`, applies
//! `dds.sec.auth.key_establishment_algorithm` and arms the handshake
//! driver on the profile the gate holds. An incomplete list fails the
//! create and names the property.
//!
//! This client used to do all of that itself, because the runtime did
//! not — a participant created with a complete, correct property list
//! got no `SecurityBundle` and ran plaintext RTPS without saying so.
//! That hole is closed in the runtime now, so the client sets the
//! properties and goes the regular way, which is also the only way a
//! suite run measures what this project ships rather than what this
//! binary reimplemented.
//!
//! The property-name constants are `zerodds-security-runtime`'s,
//! re-exported rather than re-spelled: a second literal here could drift
//! from the one the runtime reads, and the properties would then be
//! written into a namespace nobody looks at.

use zerodds_qos::PropertyQosPolicy;

pub use zerodds_security_runtime::{
    PROP_GOVERNANCE, PROP_IDENTITY_CA, PROP_IDENTITY_CERT, PROP_IDENTITY_KEY,
    PROP_KEY_ESTABLISHMENT, PROP_PERMISSIONS, PROP_PERMISSIONS_CA,
};

use crate::cli::{
    DEFAULT_GOVERNANCE_FILE, DEFAULT_IDENTITY_CA, DEFAULT_KEY_EST_ALGORITHM,
    DEFAULT_PERMISSIONS_CA, DEFAULT_PERMISSIONS_FILE, Options,
};

/// ZeroDDS vendor extension, **not** a spec property: accept X.509 v1
/// identity certificates, which DDS-Security 1.2 §10.3.1.3 forbids.
pub use zerodds_security_runtime::PROP_ACCEPT_X509V1_IDENTITY;

/// Builds the participant's `PropertyQosPolicy` from the flags.
///
/// Empty when `-disableSecurity` was passed: the absent property list is
/// what "no security" means on the QoS, it is what a peer sees, and it
/// is what makes the runtime leave the participant in plaintext.
#[must_use]
pub fn property_qos(opts: &Options) -> PropertyQosPolicy {
    if !opts.use_security() {
        return PropertyQosPolicy::new();
    }
    let mut props = PropertyQosPolicy::new();
    // The suite defaults every one of these when the flag is absent and
    // says so on stderr; `main` prints those `Info:` lines. Here the
    // defaults are simply applied, so that the property list is complete
    // whether or not the operator typed the flag.
    props.set(
        PROP_IDENTITY_CA,
        opts.resolved_identity_ca()
            .as_deref()
            .unwrap_or(DEFAULT_IDENTITY_CA),
    );
    if let Some(cert) = opts.resolved_identity_cert() {
        props.set(PROP_IDENTITY_CERT, cert);
    }
    if let Some(key) = opts.resolved_identity_key() {
        props.set(PROP_IDENTITY_KEY, key);
    }
    props.set(
        PROP_PERMISSIONS_CA,
        opts.resolved_permissions_ca()
            .as_deref()
            .unwrap_or(DEFAULT_PERMISSIONS_CA),
    );
    props.set(
        PROP_GOVERNANCE,
        opts.governance
            .as_deref()
            .unwrap_or(DEFAULT_GOVERNANCE_FILE),
    );
    props.set(
        PROP_PERMISSIONS,
        opts.permissions
            .as_deref()
            .unwrap_or(DEFAULT_PERMISSIONS_FILE),
    );
    props.set(
        PROP_KEY_ESTABLISHMENT,
        opts.key_establishment_algorithm
            .as_deref()
            .unwrap_or(DEFAULT_KEY_EST_ALGORITHM),
    );
    // Only when the flag was given. The absent property is what "spec
    // behaviour" looks like on the QoS, and a participant that announces
    // `false` would be announcing that somebody considered the question.
    if opts.accept_x509v1_identity_certs {
        props.set(PROP_ACCEPT_X509V1_IDENTITY, "true");
    }
    props
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use std::path::PathBuf;

    use zerodds_security_runtime::{
        KxSuite, SecurityPropertyError, cert_policy_from_properties, kx_suite_from_properties,
        profile_config_from_properties, resolve_file_uri, strip_file_url,
    };

    use super::*;
    use crate::cli::normalize_args;
    use clap::Parser;

    fn parse(args: &[&str]) -> Options {
        Options::try_parse_from(normalize_args(args.iter().copied())).expect("parse")
    }

    /// The names are the spec's, written out as literals so that
    /// renaming a constant cannot quietly rename what a peer's
    /// access-control plugin looks for.
    #[test]
    fn the_property_names_are_the_spec_strings() {
        assert_eq!(PROP_IDENTITY_CA, "dds.sec.auth.identity_ca");
        assert_eq!(PROP_IDENTITY_CERT, "dds.sec.auth.identity_certificate");
        assert_eq!(PROP_IDENTITY_KEY, "dds.sec.auth.private_key");
        assert_eq!(PROP_PERMISSIONS_CA, "dds.sec.access.permissions_ca");
        assert_eq!(PROP_GOVERNANCE, "dds.sec.access.governance");
        assert_eq!(PROP_PERMISSIONS, "dds.sec.access.permissions");
        for name in [
            PROP_IDENTITY_CA,
            PROP_IDENTITY_CERT,
            PROP_IDENTITY_KEY,
            PROP_PERMISSIONS_CA,
            PROP_GOVERNANCE,
            PROP_PERMISSIONS,
            PROP_KEY_ESTABLISHMENT,
        ] {
            assert!(
                name.starts_with("dds.sec."),
                "{name} is outside the reserved namespace"
            );
        }
    }

    /// `-disableSecurity` empties the property list. A leftover
    /// `dds.sec.*` property on a participant that is meant to run
    /// plaintext would announce an intent the runtime does not carry
    /// out — and, since the runtime now reads the list, would make the
    /// create fail instead.
    #[test]
    fn disable_security_leaves_no_dds_sec_property_behind() {
        let props = property_qos(&parse(&["shapetype_main", "-disableSecurity"]));
        assert!(props.is_empty());
    }

    /// The suite's own defaults appear in the list when the flags are
    /// absent, so that what the participant announces is complete
    /// either way.
    #[test]
    fn the_suite_defaults_reach_the_property_list() {
        let props = property_qos(&parse(&["shapetype_main"]));
        assert_eq!(props.get(PROP_GOVERNANCE), Some(DEFAULT_GOVERNANCE_FILE));
        assert_eq!(props.get(PROP_PERMISSIONS), Some(DEFAULT_PERMISSIONS_FILE));
        assert_eq!(props.get(PROP_IDENTITY_CA), Some(DEFAULT_IDENTITY_CA));
        assert_eq!(props.get(PROP_PERMISSIONS_CA), Some(DEFAULT_PERMISSIONS_CA));
        assert_eq!(
            props.get(PROP_KEY_ESTABLISHMENT),
            Some(DEFAULT_KEY_EST_ALGORITHM)
        );
    }

    /// What this client writes is what the runtime reads. Set a property
    /// and the config the runtime derives follows it — the contract
    /// between the two halves, asserted from the client's side.
    #[test]
    fn the_profile_config_follows_the_property_list() {
        let mut props = property_qos(&parse(&["shapetype_main"]));
        props.set(PROP_IDENTITY_CERT, "file:../certs/id_cert.pem");
        props.set(PROP_IDENTITY_KEY, "file:../certs/private/id_key.pem");
        props.set(PROP_GOVERNANCE, "file:../TESTONLY_governance_signed.p7s");
        let cfg = profile_config_from_properties(&props, 0).expect("complete list");
        assert_eq!(
            cfg.identity_cert_pem.path(),
            Some(PathBuf::from("../certs/id_cert.pem").as_path())
        );
        assert_eq!(
            cfg.identity_key_pem.path(),
            Some(PathBuf::from("../certs/private/id_key.pem").as_path())
        );
        assert_eq!(
            cfg.governance_p7s.path(),
            Some(PathBuf::from("../TESTONLY_governance_signed.p7s").as_path())
        );
    }

    /// A missing identity is named, not defaulted. Without
    /// `-identityCert` (or `ZERODDS_ID_CERTIFICATE`) the list has no
    /// identity in it, and the runtime refuses the participant by name
    /// rather than bringing it up in plaintext.
    #[test]
    fn a_missing_identity_property_is_reported_by_name() {
        // The environment fallback would fill this in on a machine that
        // has it set; the property list is built by hand here so the
        // test measures the absent property and not the operator's
        // shell.
        let mut props = property_qos(&parse(&["shapetype_main"]));
        props.set(PROP_IDENTITY_KEY, "file:../certs/private/id_key.pem");
        let err = profile_config_from_properties(&props, 0)
            .expect_err("no -identityCert was given, so there is no identity");
        match err {
            SecurityPropertyError::MissingProperty(name) => assert_eq!(name, PROP_IDENTITY_CERT),
            other => panic!("expected a named missing property, got {other:?}"),
        }
    }

    /// The suite writes `file:` with no authority; `strip_file_url`
    /// alone leaves that scheme on the path.
    #[test]
    fn the_suites_authority_less_file_uri_resolves_to_a_path() {
        assert_eq!(
            resolve_file_uri(PROP_GOVERNANCE, "file:../TESTONLY_governance_signed.p7s")
                .expect("a file: URI"),
            PathBuf::from("../TESTONLY_governance_signed.p7s")
        );
        assert_eq!(
            strip_file_url("file:../TESTONLY_governance_signed.p7s"),
            "file:../TESTONLY_governance_signed.p7s",
            "the shared helper does not handle this form — that is why \
             resolve_file_uri exists"
        );
        assert_eq!(
            resolve_file_uri(PROP_IDENTITY_CA, "file:///etc/dds/ca.pem").expect("a file: URI"),
            PathBuf::from("/etc/dds/ca.pem")
        );
        assert_eq!(
            resolve_file_uri(PROP_IDENTITY_CA, "../plain/path.pem").expect("a plain path"),
            PathBuf::from("../plain/path.pem")
        );
    }

    /// The X.509 v1 deviation is off unless the flag is given, and it is
    /// a property on the announced QoS when it is — not a value kept
    /// beside the list where the participant's peers cannot see it.
    #[test]
    fn the_x509v1_deviation_is_absent_until_its_flag_is_given() {
        let props = property_qos(&parse(&["shapetype_main"]));
        assert_eq!(
            props.get(PROP_ACCEPT_X509V1_IDENTITY),
            None,
            "DDS-Security 1.2 §10.3.1.3 is the default and an absent \
             property is what that looks like on the QoS"
        );
        assert!(!cert_policy_from_properties(&props).accept_x509v1_identity_certificate);

        let props = property_qos(&parse(&["shapetype_main", "-acceptX509v1IdentityCerts"]));
        assert_eq!(props.get(PROP_ACCEPT_X509V1_IDENTITY), Some("true"));
        assert!(cert_policy_from_properties(&props).accept_x509v1_identity_certificate);
    }

    /// The property name is ZeroDDS's own: under `org.zerodds.`, not under
    /// the `dds.sec.` the specification keeps for its own names
    /// (DDS-Security 1.2 §7.3.2, §7.3.5.1).
    #[test]
    fn the_deviation_property_is_named_as_a_deviation() {
        assert_eq!(
            PROP_ACCEPT_X509V1_IDENTITY,
            "org.zerodds.sec.auth.accept_x509v1_identity_certificate"
        );
        assert!(PROP_ACCEPT_X509V1_IDENTITY.starts_with("org.zerodds."));
        assert!(!PROP_ACCEPT_X509V1_IDENTITY.starts_with("dds.sec."));
    }

    /// `-disableSecurity` wins over the deviation flag: a plaintext
    /// participant announces no `dds.sec.*` property at all, this one
    /// included.
    #[test]
    fn disable_security_removes_the_deviation_property_too() {
        let props = property_qos(&parse(&[
            "shapetype_main",
            "-acceptX509v1IdentityCerts",
            "-disableSecurity",
        ]));
        assert!(props.is_empty());
    }

    /// The suite's single-dash spelling reaches the flag, the way every
    /// other option of this client does.
    #[test]
    fn the_deviation_flag_takes_the_suites_single_dash_spelling() {
        assert!(
            parse(&["shapetype_main", "-acceptX509v1IdentityCerts"]).accept_x509v1_identity_certs
        );
        assert!(
            parse(&["shapetype_main", "--acceptX509v1IdentityCerts"]).accept_x509v1_identity_certs
        );
        assert!(!parse(&["shapetype_main"]).accept_x509v1_identity_certs);
    }

    /// `-kest` reaches the property, the property selects a suite of
    /// Table 26 by its 1.2 or its 1.1 name, and an unknown one is refused
    /// rather than quietly becoming P-256.
    #[test]
    fn kest_maps_the_spec_spelling_and_refuses_the_unknown() {
        let props = property_qos(&parse(&["shapetype_main"]));
        assert_eq!(
            kx_suite_from_properties(&props).expect("the suite default"),
            KxSuite::EcdhP256
        );
        let props = property_qos(&parse(&["shapetype_main", "-kest", "ECDH+prime256v1-CEUM"]));
        assert_eq!(
            kx_suite_from_properties(&props).expect("our wire spelling"),
            KxSuite::EcdhP256
        );
        let props = property_qos(&parse(&["shapetype_main", "-kest", "ECDHE-CEUM+P384"]));
        assert_eq!(
            kx_suite_from_properties(&props).expect("P-384"),
            KxSuite::EcdhP384
        );
        for name in ["DHE+MODP-2048-256", "DH+MODP-2048-256"] {
            let props = property_qos(&parse(&["shapetype_main", "-kest", name]));
            assert_eq!(
                kx_suite_from_properties(&props).expect("the MODP group"),
                KxSuite::DhModp2048_256,
                "{name}"
            );
        }
        let props = property_qos(&parse(&["shapetype_main", "-kest", "ECDH+brainpool"]));
        assert!(
            kx_suite_from_properties(&props).is_err(),
            "a suite Table 26 does not name is refused, not run as P-256"
        );
    }
}
