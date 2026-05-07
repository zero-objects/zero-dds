// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Access-Control-Plugin SPI (OMG DDS-Security 1.1 §8.4).
//!
//! Entscheidet pro Topic / pro Operation, ob ein authentifizierter
//! Participant die Aktion ausfuehren darf. Eingaben:
//! * Governance-XML — Topic-Level-Rules (Discovery, Publishing,
//!   Subscribing, Encrypt-Flag).
//! * Permissions-XML — wer darf was (signiert mit Permissions-CA).
//!
//! Die XML-Dateien werden als Properties uebergeben (Pfade). Das
//! Plugin parst + validiert + cacht intern.
//!
//! zerodds-lint: allow no_dyn_in_safe
//! (Plugin-SPI benötigt `Box<dyn AccessControlPlugin>`.)

extern crate alloc;

use alloc::boxed::Box;

use crate::authentication::IdentityHandle;
use crate::error::SecurityResult;
use crate::properties::PropertyList;

/// Opaker Handle fuer validierte Permissions. Erzeugt durch
/// [`AccessControlPlugin::validate_local_permissions`] bzw.
/// `validate_remote_permissions`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PermissionsHandle(pub u64);

/// Ob eine Aktion durchgefuehrt werden darf. Bewusst sparsam — keine
/// Reason-String, weil der Reason an den Remote nichts verraten soll
/// (Logging via [`crate::LoggingPlugin`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessDecision {
    /// Erlaubt.
    Permit,
    /// Verweigert.
    Deny,
}

impl AccessDecision {
    /// `true` wenn `Permit`.
    #[must_use]
    pub fn is_permitted(self) -> bool {
        matches!(self, Self::Permit)
    }
}

/// Access-Control-Plugin-Trait (Spec §8.4.2.9).
pub trait AccessControlPlugin: Send + Sync {
    /// Validiert lokale Permissions (Governance.xml + Permissions.xml
    /// + Signatur-Check gegen Permissions-CA).
    ///
    /// # Spec §8.4.2.9.1
    fn validate_local_permissions(
        &mut self,
        local: IdentityHandle,
        participant_guid: [u8; 16],
        props: &PropertyList,
    ) -> SecurityResult<PermissionsHandle>;

    /// Validiert Remote-Permissions aus dem SEDP-Handshake.
    ///
    /// # Spec §8.4.2.9.2
    fn validate_remote_permissions(
        &mut self,
        local: IdentityHandle,
        remote: IdentityHandle,
        remote_permissions_token: &[u8],
        remote_credential: &[u8],
    ) -> SecurityResult<PermissionsHandle>;

    /// Darf dieser Participant einen DataWriter auf diesem Topic
    /// erzeugen?
    ///
    /// # Spec §8.4.2.9.4 `check_create_datawriter`.
    fn check_create_datawriter(
        &self,
        perms: PermissionsHandle,
        topic_name: &str,
    ) -> SecurityResult<AccessDecision>;

    /// Darf dieser Participant einen DataReader auf diesem Topic
    /// erzeugen?
    ///
    /// # Spec §8.4.2.9.5 `check_create_datareader`.
    fn check_create_datareader(
        &self,
        perms: PermissionsHandle,
        topic_name: &str,
    ) -> SecurityResult<AccessDecision>;

    /// Darf der lokale Reader die Publication des Remote matchen?
    ///
    /// # Spec §8.4.2.9.17 `check_remote_datawriter_match`.
    fn check_remote_datawriter_match(
        &self,
        local_perms: PermissionsHandle,
        remote_perms: PermissionsHandle,
        topic_name: &str,
    ) -> SecurityResult<AccessDecision>;

    /// Spiegelbildlich: darf Remote-Reader unseren Writer matchen?
    fn check_remote_datareader_match(
        &self,
        local_perms: PermissionsHandle,
        remote_perms: PermissionsHandle,
        topic_name: &str,
    ) -> SecurityResult<AccessDecision>;

    /// Plugin-Class-Id (z.B. "DDS:Access:Permissions:1.2") fuer SPDP-
    /// Annoncierung.
    fn plugin_class_id(&self) -> &str;

    /// Spec §9.4.2.5: `check_create_participant`. Default: permit
    /// (kein Plugin-spezifisches Filtern).
    ///
    /// # Errors
    /// Implementations-spezifisch.
    fn check_create_participant(
        &self,
        _local_perms: PermissionsHandle,
        _domain_id: u32,
    ) -> SecurityResult<AccessDecision> {
        Ok(AccessDecision::Permit)
    }

    /// Spec §9.4.2.6: `check_remote_participant` — darf Remote-
    /// Participant in unsere Domain joinen? Default: permit.
    ///
    /// # Errors
    /// Implementations-spezifisch.
    fn check_remote_participant(
        &self,
        _local_perms: PermissionsHandle,
        _remote_perms: PermissionsHandle,
        _domain_id: u32,
    ) -> SecurityResult<AccessDecision> {
        Ok(AccessDecision::Permit)
    }

    /// Spec §9.4.2.10: `check_create_topic` — darf der lokale
    /// Subject ein Topic mit dem Namen erzeugen? Default: permit.
    ///
    /// # Errors
    /// Implementations-spezifisch.
    fn check_create_topic(
        &self,
        _local_perms: PermissionsHandle,
        _topic_name: &str,
    ) -> SecurityResult<AccessDecision> {
        Ok(AccessDecision::Permit)
    }

    /// Spec §9.4.2.13: `get_permissions_token` — opaker
    /// Permissions-Token fuer SPDP-Annoncierung
    /// (`PID_PERMISSIONS_TOKEN` 0x1002). Default: leer.
    ///
    /// # Errors
    /// Implementations-spezifisch.
    fn get_permissions_token(
        &self,
        _local_perms: PermissionsHandle,
    ) -> SecurityResult<alloc::vec::Vec<u8>> {
        Ok(alloc::vec::Vec::new())
    }

    /// Spec §9.4.2.14: `get_permissions_credential_token` — opake
    /// Credential, die im Authentication-Plugin via
    /// `set_permissions_credential_and_token` weitergereicht wird.
    /// Default: leer.
    ///
    /// # Errors
    /// Implementations-spezifisch.
    fn get_permissions_credential_token(
        &self,
        _local_perms: PermissionsHandle,
    ) -> SecurityResult<alloc::vec::Vec<u8>> {
        Ok(alloc::vec::Vec::new())
    }
}

/// Factory-Alias.
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
