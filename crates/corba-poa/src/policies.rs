// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! POA Policies — Spec §11.3.6 Table 11-2.
//!
//! Seven policies, each with its modes:
//!
//! | Policy | Default | Values |
//! |---|---|---|
//! | Lifespan | TRANSIENT | TRANSIENT / PERSISTENT |
//! | IdAssignment | SYSTEM_ID | USER_ID / SYSTEM_ID |
//! | IdUniqueness | UNIQUE_ID | UNIQUE_ID / MULTIPLE_ID |
//! | ImplicitActivation | NO_IMPLICIT_ACTIVATION | IMPLICIT / NO_IMPLICIT |
//! | ServantRetention | RETAIN | RETAIN / NON_RETAIN |
//! | RequestProcessing | USE_ACTIVE_OBJECT_MAP_ONLY | USE_ACTIVE_OBJECT_MAP_ONLY / USE_DEFAULT_SERVANT / USE_SERVANT_MANAGER |
//! | Thread | ORB_CTRL_MODEL | ORB_CTRL / SINGLE_THREAD / MAIN_THREAD |
//!
//! Spec §11.3.6.6 normative: certain combinations are invalid
//! and are rejected by the POA with `InvalidPolicy`.

use crate::error::{PoaError, PoaResult};

/// Spec §11.3.6.1 — Lifespan-Policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LifespanPolicy {
    /// `TRANSIENT` — object references are valid only within the
    /// creating server process (default).
    Transient,
    /// `PERSISTENT` — object references outlive the creating
    /// process and can be served again after a restart.
    Persistent,
}

impl Default for LifespanPolicy {
    fn default() -> Self {
        Self::Transient
    }
}

/// Spec §11.3.6.2 — Id-Assignment-Policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IdAssignmentPolicy {
    /// `SYSTEM_ID` — the POA generates the object IDs (default).
    System,
    /// `USER_ID` — the caller supplies the object IDs explicitly
    /// (`activate_object_with_id`).
    User,
}

impl Default for IdAssignmentPolicy {
    fn default() -> Self {
        Self::System
    }
}

/// Spec §11.3.6.3 — Id-Uniqueness-Policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IdUniquenessPolicy {
    /// `UNIQUE_ID` — a servant may have at most one object ID
    /// (default).
    Unique,
    /// `MULTIPLE_ID` — a servant can be active under multiple
    /// object IDs.
    Multiple,
}

impl Default for IdUniquenessPolicy {
    fn default() -> Self {
        Self::Unique
    }
}

/// Spec §11.3.6.4 — Implicit-Activation-Policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImplicitActivationPolicy {
    /// `NO_IMPLICIT_ACTIVATION` (Default).
    NoImplicit,
    /// `IMPLICIT_ACTIVATION` — `_this()` calls activate the
    /// servant automatically.
    Implicit,
}

impl Default for ImplicitActivationPolicy {
    fn default() -> Self {
        Self::NoImplicit
    }
}

/// Spec §11.3.6.5 — Servant-Retention-Policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ServantRetentionPolicy {
    /// `RETAIN` — the POA maintains an Active Object Map (default).
    Retain,
    /// `NON_RETAIN` — no AOM; every request goes to the
    /// default servant or ServantManager.
    NonRetain,
}

impl Default for ServantRetentionPolicy {
    fn default() -> Self {
        Self::Retain
    }
}

/// Spec §11.3.6.6 — Request-Processing-Policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RequestProcessingPolicy {
    /// `USE_ACTIVE_OBJECT_MAP_ONLY` — AOM only (default).
    UseActiveObjectMapOnly,
    /// `USE_DEFAULT_SERVANT` — on a miss, falls back to the default
    /// servant. Requires MULTIPLE_ID.
    UseDefaultServant,
    /// `USE_SERVANT_MANAGER` — on a miss, the ServantActivator/
    /// Locator is invoked.
    UseServantManager,
}

impl Default for RequestProcessingPolicy {
    fn default() -> Self {
        Self::UseActiveObjectMapOnly
    }
}

/// Spec §11.3.6.7 — Thread-Policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThreadPolicy {
    /// `ORB_CTRL_MODEL` — the ORB chooses the threading (default).
    OrbCtrl,
    /// `SINGLE_THREAD_MODEL` — all requests to this POA are
    /// serialized sequentially onto a single thread.
    SingleThread,
    /// `MAIN_THREAD_MODEL` — requests run on the ORB main thread.
    MainThread,
}

impl Default for ThreadPolicy {
    fn default() -> Self {
        Self::OrbCtrl
    }
}

/// Complete policy set for a POA.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PolicySet {
    /// Lifespan.
    pub lifespan: LifespanPolicy,
    /// IdAssignment.
    pub id_assignment: IdAssignmentPolicy,
    /// IdUniqueness.
    pub id_uniqueness: IdUniquenessPolicy,
    /// ImplicitActivation.
    pub implicit_activation: ImplicitActivationPolicy,
    /// ServantRetention.
    pub servant_retention: ServantRetentionPolicy,
    /// RequestProcessing.
    pub request_processing: RequestProcessingPolicy,
    /// Thread.
    pub thread: ThreadPolicy,
}

impl PolicySet {
    /// Validates the policy combination per Spec §11.3.6.6.
    ///
    /// # Errors
    /// `InvalidPolicy` with a diagnostic string when an incompatible
    /// combination was chosen.
    pub fn validate(&self) -> PoaResult<()> {
        // Spec §11.3.6.6: USE_DEFAULT_SERVANT requires MULTIPLE_ID.
        if self.request_processing == RequestProcessingPolicy::UseDefaultServant
            && self.id_uniqueness != IdUniquenessPolicy::Multiple
        {
            return Err(PoaError::InvalidPolicy(
                "USE_DEFAULT_SERVANT requires MULTIPLE_ID".into(),
            ));
        }
        // Spec §11.3.6.6: NON_RETAIN requires USE_DEFAULT_SERVANT
        // or USE_SERVANT_MANAGER (USE_ACTIVE_OBJECT_MAP_ONLY makes
        // no sense without an AOM).
        if self.servant_retention == ServantRetentionPolicy::NonRetain
            && self.request_processing == RequestProcessingPolicy::UseActiveObjectMapOnly
        {
            return Err(PoaError::InvalidPolicy(
                "NON_RETAIN requires USE_DEFAULT_SERVANT or USE_SERVANT_MANAGER".into(),
            ));
        }
        // Spec §11.3.6.4: IMPLICIT_ACTIVATION requires SYSTEM_ID + RETAIN.
        if self.implicit_activation == ImplicitActivationPolicy::Implicit {
            if self.id_assignment != IdAssignmentPolicy::System {
                return Err(PoaError::InvalidPolicy(
                    "IMPLICIT_ACTIVATION requires SYSTEM_ID".into(),
                ));
            }
            if self.servant_retention != ServantRetentionPolicy::Retain {
                return Err(PoaError::InvalidPolicy(
                    "IMPLICIT_ACTIVATION requires RETAIN".into(),
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_set_is_spec_default() {
        // Spec §11.3.6 Table 11-2: TRANSIENT/SYSTEM_ID/UNIQUE_ID/
        // NO_IMPLICIT/RETAIN/USE_AOM_ONLY/ORB_CTRL.
        let p = PolicySet::default();
        assert_eq!(p.lifespan, LifespanPolicy::Transient);
        assert_eq!(p.id_assignment, IdAssignmentPolicy::System);
        assert_eq!(p.id_uniqueness, IdUniquenessPolicy::Unique);
        assert_eq!(p.implicit_activation, ImplicitActivationPolicy::NoImplicit);
        assert_eq!(p.servant_retention, ServantRetentionPolicy::Retain);
        assert_eq!(
            p.request_processing,
            RequestProcessingPolicy::UseActiveObjectMapOnly
        );
        assert_eq!(p.thread, ThreadPolicy::OrbCtrl);
    }

    #[test]
    fn default_set_validates() {
        assert!(PolicySet::default().validate().is_ok());
    }

    #[test]
    fn use_default_servant_without_multiple_id_is_rejected() {
        let p = PolicySet {
            request_processing: RequestProcessingPolicy::UseDefaultServant,
            id_uniqueness: IdUniquenessPolicy::Unique,
            ..PolicySet::default()
        };
        assert!(matches!(p.validate(), Err(PoaError::InvalidPolicy(_))));
    }

    #[test]
    fn use_default_servant_with_multiple_id_is_ok() {
        let p = PolicySet {
            request_processing: RequestProcessingPolicy::UseDefaultServant,
            id_uniqueness: IdUniquenessPolicy::Multiple,
            ..PolicySet::default()
        };
        assert!(p.validate().is_ok());
    }

    #[test]
    fn non_retain_with_aom_only_is_rejected() {
        let p = PolicySet {
            servant_retention: ServantRetentionPolicy::NonRetain,
            request_processing: RequestProcessingPolicy::UseActiveObjectMapOnly,
            ..PolicySet::default()
        };
        assert!(matches!(p.validate(), Err(PoaError::InvalidPolicy(_))));
    }

    #[test]
    fn non_retain_with_servant_manager_is_ok() {
        let p = PolicySet {
            servant_retention: ServantRetentionPolicy::NonRetain,
            request_processing: RequestProcessingPolicy::UseServantManager,
            ..PolicySet::default()
        };
        assert!(p.validate().is_ok());
    }

    #[test]
    fn implicit_activation_requires_system_id_and_retain() {
        let p = PolicySet {
            implicit_activation: ImplicitActivationPolicy::Implicit,
            id_assignment: IdAssignmentPolicy::User,
            ..PolicySet::default()
        };
        assert!(matches!(p.validate(), Err(PoaError::InvalidPolicy(_))));

        let p2 = PolicySet {
            implicit_activation: ImplicitActivationPolicy::Implicit,
            servant_retention: ServantRetentionPolicy::NonRetain,
            request_processing: RequestProcessingPolicy::UseServantManager,
            ..PolicySet::default()
        };
        assert!(matches!(p2.validate(), Err(PoaError::InvalidPolicy(_))));
    }

    #[test]
    fn implicit_activation_with_compatible_policies_is_ok() {
        let p = PolicySet {
            implicit_activation: ImplicitActivationPolicy::Implicit,
            ..PolicySet::default()
        };
        assert!(p.validate().is_ok());
    }
}
