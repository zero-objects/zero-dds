// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Access control plugin SPI (OMG DDS-Security 1.1 §8.4).
//!
//! Decides per topic / per operation whether an authenticated
//! participant may perform the action. Inputs:
//! * Governance XML — topic-level rules (discovery, publishing,
//!   subscribing, encrypt flag).
//! * Permissions XML — who may do what (signed with the permissions CA).
//!
//! The XML files are passed as properties (paths). The
//! plugin parses + validates + caches internally.
//!
//! zerodds-lint: allow no_dyn_in_safe
//! (The plugin SPI needs `Box<dyn AccessControlPlugin>`.)

extern crate alloc;

use alloc::boxed::Box;

use crate::authentication::IdentityHandle;
use crate::error::SecurityResult;
use crate::properties::PropertyList;

/// Opaque handle for validated permissions. Created by
/// [`AccessControlPlugin::validate_local_permissions`] or
/// `validate_remote_permissions`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PermissionsHandle(pub u64);

/// Whether an action may be performed. Deliberately sparse — no
/// reason string, because the reason must not reveal anything to the
/// remote (logging via [`crate::LoggingPlugin`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessDecision {
    /// Allowed.
    Permit,
    /// Denied.
    Deny,
}

impl AccessDecision {
    /// `true` if `Permit`.
    #[must_use]
    pub fn is_permitted(self) -> bool {
        matches!(self, Self::Permit)
    }
}

/// Access control plugin trait (spec §8.4.2.9).
pub trait AccessControlPlugin: Send + Sync {
    /// Validates local permissions (Governance.xml + Permissions.xml
    /// + signature check against the permissions CA).
    ///
    /// # Spec §8.4.2.9.1
    fn validate_local_permissions(
        &mut self,
        local: IdentityHandle,
        participant_guid: [u8; 16],
        props: &PropertyList,
    ) -> SecurityResult<PermissionsHandle>;

    /// Validates remote permissions from the SEDP handshake.
    ///
    /// # Spec §8.4.2.9.2
    fn validate_remote_permissions(
        &mut self,
        local: IdentityHandle,
        remote: IdentityHandle,
        remote_permissions_token: &[u8],
        remote_credential: &[u8],
    ) -> SecurityResult<PermissionsHandle>;

    /// May this participant create a DataWriter on this topic?
    ///
    /// # Spec §8.4.2.9.4 `check_create_datawriter`.
    fn check_create_datawriter(
        &self,
        perms: PermissionsHandle,
        topic_name: &str,
    ) -> SecurityResult<AccessDecision>;

    /// May this participant create a DataReader on this topic?
    ///
    /// # Spec §8.4.2.9.5 `check_create_datareader`.
    fn check_create_datareader(
        &self,
        perms: PermissionsHandle,
        topic_name: &str,
    ) -> SecurityResult<AccessDecision>;

    /// May the local reader match the remote's publication?
    ///
    /// # Spec §8.4.2.9.17 `check_remote_datawriter_match`.
    fn check_remote_datawriter_match(
        &self,
        local_perms: PermissionsHandle,
        remote_perms: PermissionsHandle,
        topic_name: &str,
    ) -> SecurityResult<AccessDecision>;

    /// Mirror image: may a remote reader match our writer?
    fn check_remote_datareader_match(
        &self,
        local_perms: PermissionsHandle,
        remote_perms: PermissionsHandle,
        topic_name: &str,
    ) -> SecurityResult<AccessDecision>;

    /// Plugin class id (e.g. "DDS:Access:Permissions:1.2") for SPDP
    /// announcing.
    fn plugin_class_id(&self) -> &str;

    /// Spec §9.4.2.5: `check_create_participant`. Default: permit
    /// (no plugin-specific filtering).
    ///
    /// # Errors
    /// Implementation-specific.
    fn check_create_participant(
        &self,
        _local_perms: PermissionsHandle,
        _domain_id: u32,
    ) -> SecurityResult<AccessDecision> {
        Ok(AccessDecision::Permit)
    }

    /// Spec §9.4.2.6: `check_remote_participant` — may the remote
    /// participant join our domain? Default: permit.
    ///
    /// # Errors
    /// Implementation-specific.
    fn check_remote_participant(
        &self,
        _local_perms: PermissionsHandle,
        _remote_perms: PermissionsHandle,
        _domain_id: u32,
    ) -> SecurityResult<AccessDecision> {
        Ok(AccessDecision::Permit)
    }

    /// Spec §9.4.2.10: `check_create_topic` — may the local
    /// subject create a topic with that name? Default: permit.
    ///
    /// # Errors
    /// Implementation-specific.
    fn check_create_topic(
        &self,
        _local_perms: PermissionsHandle,
        _topic_name: &str,
    ) -> SecurityResult<AccessDecision> {
        Ok(AccessDecision::Permit)
    }

    /// Spec §9.4.2.13: `get_permissions_token` — opaque
    /// permissions token for SPDP announcing
    /// (`PID_PERMISSIONS_TOKEN` 0x1002). Default: empty.
    ///
    /// # Errors
    /// Implementation-specific.
    fn get_permissions_token(
        &self,
        _local_perms: PermissionsHandle,
    ) -> SecurityResult<alloc::vec::Vec<u8>> {
        Ok(alloc::vec::Vec::new())
    }

    /// Spec §9.4.2.14: `get_permissions_credential_token` — opaque
    /// credential passed on in the authentication plugin via
    /// `set_permissions_credential_and_token`.
    /// Default: empty.
    ///
    /// # Errors
    /// Implementation-specific.
    fn get_permissions_credential_token(
        &self,
        _local_perms: PermissionsHandle,
    ) -> SecurityResult<alloc::vec::Vec<u8>> {
        Ok(alloc::vec::Vec::new())
    }
}

/// Factory alias.
pub type AccessControlBox = Box<dyn AccessControlPlugin>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn access_decision_helper() {
        assert!(AccessDecision::Permit.is_permitted());
        assert!(!AccessDecision::Deny.is_permitted());
    }
}
