// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Arms a [`SecurityProfile`]'s own gate with its own permissions, so that
//! the DDS-Security 1.2 §9.4.2.9.4 / §9.4.2.9.5 endpoint-create checks can
//! run where the endpoints are created.
//!
//! The gate carries the governance and travels into
//! `RuntimeConfig::security`; the permissions live beside it on the profile.
//! Every path that takes `profile.gate` and hands it to a runtime must take
//! the permissions with it, or the runtime holds a gate that knows a topic is
//! access-controlled and has nothing to decide with. There are two such
//! paths — `zerodds_dcps::RuntimeConfig::with_security_bundle` and
//! `zerodds-c-api::security_ffi` — and they both call this one function
//! rather than repeating the pairing.

use crate::gate::SecurityGateError;
use crate::profile::SecurityProfile;
use crate::shared::SignedDocuments;

/// Arms `profile.gate` with the access control of `profile`: the gate builds
/// the builtin plugin `DDS:Access:Permissions` from its governance, the
/// permissions of the profile (the grant of this participant's subject), the
/// signed permissions document, the Permissions CA and the algorithms of the
/// identity certificate (DDS-Security 1.2 §9.4.2.9.1, Table 70).
///
/// Idempotent: calling it twice replaces the plugin with an equal one.
///
/// # Errors
/// [`SecurityGateError::AccessControl`] when the Permissions CA does not parse
/// and when the governance does not allow the algorithms of the identity
/// certificate (`get_participant_security_config` fails, Table 70);
/// [`SecurityGateError::Crypto`] when the gate mutex is poisoned.
pub fn arm_endpoint_access_control(profile: &SecurityProfile) -> Result<(), SecurityGateError> {
    // §9.3.2.11.11 `set_participant_security_config`: the algorithms the
    // plugin uses for the identity, which the permissions plugin checks against
    // `allowed_crypto_algorithms` (Table 70).
    let identity_algorithms = profile.pki.lock().ok().and_then(|mut pki| {
        use zerodds_security::authentication::{AuthenticationPlugin, ParticipantSecurityConfig};
        pki.set_participant_security_config(
            profile.identity_handle,
            &ParticipantSecurityConfig::unrestricted(),
        )
        .ok()
    });
    profile
        .gate
        .set_local_participant(profile.identity_handle, profile.adjusted_participant_guid)?;
    profile.gate.set_builtin_access_control(
        SignedDocuments {
            permissions: profile.permissions.clone(),
            signed_permissions: &profile.permissions_signed,
            signed_governance: &profile.governance_signed,
            identity_certificate_pem: &profile.identity_certificate_pem,
            permissions_ca_pem: &profile.permissions_ca_pem,
        },
        identity_algorithms,
    )
}
