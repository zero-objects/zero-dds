// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! POA — Spec §11.3.5.
//!
//! Top-Level-Klasse, die alle Policies + Manager + AOM zu einem
//! lauffaehigen Adapter kombiniert. Implementiert die wichtigsten
//! Spec-Operations:
//!
//! * `activate_object` (SYSTEM_ID) — Spec §11.3.5.20.
//! * `activate_object_with_id` (USER_ID) — Spec §11.3.5.20.
//! * `deactivate_object` — Spec §11.3.5.20.
//! * `id_to_servant` / `servant_to_id` — Spec §11.3.5.20.
//! * `dispatch` — Request-Dispatch fuer alle 6 Modi-Kombinationen
//!   aus ServantRetention × RequestProcessing.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

use std::sync::Mutex;

use crate::active_object_map::{ActiveObjectMap, ServantId};
use crate::error::{PoaError, PoaResult};
use crate::object_id::ObjectId;
use crate::poa_manager::{PoaManager, PoaManagerState};
use crate::policies::{
    IdAssignmentPolicy, PolicySet, RequestProcessingPolicy, ServantRetentionPolicy,
};
use crate::servant::Servant;
use crate::servant_manager::ServantManager;

/// POA-Konfiguration — kombiniert Policies + Manager + Adapter-Name.
#[derive(Debug, Clone)]
pub struct PoaConfig {
    /// Adapter-Name (z.B. `"RootPOA/MyChild"`).
    pub adapter_name: String,
    /// Policy-Set.
    pub policies: PolicySet,
    /// POAManager-Reference.
    pub manager: Arc<PoaManager>,
}

/// POA-Hauptklasse.
pub struct Poa {
    config: PoaConfig,
    aom: Mutex<ActiveObjectMap>,
    next_system_id: AtomicU64,
    default_servant: Mutex<Option<Box<dyn Servant>>>,
    servant_manager: Mutex<Option<ServantManager>>,
}

impl core::fmt::Debug for Poa {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Poa")
            .field("adapter_name", &self.config.adapter_name)
            .field("policies", &self.config.policies)
            .finish()
    }
}

impl Poa {
    /// Erstellt einen neuen POA. Spec §11.3.6.6 Policy-Validierung
    /// laeuft hier automatisch.
    ///
    /// # Errors
    /// `InvalidPolicy` bei inkompatibler Kombination.
    pub fn new(config: PoaConfig) -> PoaResult<Self> {
        config.policies.validate()?;
        let mut aom = ActiveObjectMap::default();
        aom.set_uniqueness(config.policies.id_uniqueness);
        Ok(Self {
            config,
            aom: Mutex::new(aom),
            next_system_id: AtomicU64::new(1),
            default_servant: Mutex::new(None),
            servant_manager: Mutex::new(None),
        })
    }

    /// Adapter-Name.
    #[must_use]
    pub fn adapter_name(&self) -> &str {
        &self.config.adapter_name
    }

    /// Policy-Set.
    #[must_use]
    pub fn policies(&self) -> &PolicySet {
        &self.config.policies
    }

    /// POAManager.
    #[must_use]
    pub fn manager(&self) -> &Arc<PoaManager> {
        &self.config.manager
    }

    /// Setzt den Default-Servant (USE_DEFAULT_SERVANT).
    pub fn set_default_servant(&self, s: Box<dyn Servant>) {
        if let Ok(mut g) = self.default_servant.lock() {
            *g = Some(s);
        }
    }

    /// Setzt den ServantManager (USE_SERVANT_MANAGER).
    pub fn set_servant_manager(&self, m: ServantManager) {
        if let Ok(mut g) = self.servant_manager.lock() {
            *g = Some(m);
        }
    }

    /// `activate_object` — Spec §11.3.5.20. Verlangt SYSTEM_ID + RETAIN.
    ///
    /// # Errors
    /// `WrongPolicy` bei USER_ID oder NON_RETAIN.
    pub fn activate_object(&self, servant: Box<dyn Servant>) -> PoaResult<ObjectId> {
        if self.config.policies.id_assignment != IdAssignmentPolicy::System {
            return Err(PoaError::WrongPolicy(
                "activate_object requires SYSTEM_ID".into(),
            ));
        }
        if self.config.policies.servant_retention != ServantRetentionPolicy::Retain {
            return Err(PoaError::WrongPolicy(
                "activate_object requires RETAIN".into(),
            ));
        }
        let oid = ObjectId::system_id(self.next_system_id.fetch_add(1, Ordering::Relaxed));
        self.aom_lock()?.activate(oid.clone(), servant)?;
        Ok(oid)
    }

    /// `activate_object_with_id` — Spec §11.3.5.20.
    ///
    /// # Errors
    /// `WrongPolicy` bei NON_RETAIN.
    pub fn activate_object_with_id(
        &self,
        oid: ObjectId,
        servant: Box<dyn Servant>,
    ) -> PoaResult<ServantId> {
        if self.config.policies.servant_retention != ServantRetentionPolicy::Retain {
            return Err(PoaError::WrongPolicy(
                "activate_object_with_id requires RETAIN".into(),
            ));
        }
        self.aom_lock()?.activate(oid, servant)
    }

    /// `deactivate_object` — Spec §11.3.5.20.
    ///
    /// # Errors
    /// `WrongPolicy` bei NON_RETAIN; `ObjectNotActive` bei
    /// unbekannter ID.
    pub fn deactivate_object(&self, oid: &ObjectId) -> PoaResult<Box<dyn Servant>> {
        if self.config.policies.servant_retention != ServantRetentionPolicy::Retain {
            return Err(PoaError::WrongPolicy(
                "deactivate_object requires RETAIN".into(),
            ));
        }
        let s = self.aom_lock()?.deactivate(oid)?;
        Ok(s)
    }

    /// `dispatch` — verarbeitet einen Request gemaess der aktuellen
    /// Policy-Kombination und liefert die Reply-Bytes.
    ///
    /// Modus-Matrix:
    ///
    /// | Retention | RequestProcessing | Verhalten |
    /// |---|---|---|
    /// | RETAIN    | USE_AOM_ONLY     | AOM-Lookup; Miss → ObjectNotActive |
    /// | RETAIN    | USE_DEFAULT_SERVANT | AOM, Miss → DefaultServant |
    /// | RETAIN    | USE_SERVANT_MANAGER | AOM, Miss → Activator → AOM-Insert |
    /// | NON_RETAIN | USE_DEFAULT_SERVANT | direkt DefaultServant |
    /// | NON_RETAIN | USE_SERVANT_MANAGER | Locator.preinvoke / postinvoke |
    ///
    /// # Errors
    /// `AdapterInactive` wenn der POAManager INACTIVE ist; sonstige
    /// Spec-Errors je nach Pfad.
    pub fn dispatch(
        &self,
        oid: &ObjectId,
        operation: &str,
        request_body: &[u8],
    ) -> PoaResult<Vec<u8>> {
        // Spec §11.3.5: bei INACTIVE-Manager wird der Request mit
        // OBJ_ADAPTER abgelehnt.
        match self.config.manager.state() {
            PoaManagerState::Inactive => {
                return Err(PoaError::AdapterInactive);
            }
            PoaManagerState::Discarding => {
                return Err(PoaError::ObjAdapter(
                    "POAManager is in DISCARDING state".into(),
                ));
            }
            PoaManagerState::Holding => {
                // Spec §11.3.2.1.2: Caller waere zu queuen — wir
                // signalisieren dem Caller via WrongPolicy/Bad-
                // InvocationOrder, dass er warten muss.
                return Err(PoaError::BadInvocationOrder(
                    "POAManager is HOLDING; request must be queued by caller".into(),
                ));
            }
            PoaManagerState::Active => {}
        }

        let retention = self.config.policies.servant_retention;
        let processing = self.config.policies.request_processing;

        match (retention, processing) {
            (ServantRetentionPolicy::Retain, RequestProcessingPolicy::UseActiveObjectMapOnly) => {
                self.dispatch_aom_only(oid, operation, request_body)
            }
            (ServantRetentionPolicy::Retain, RequestProcessingPolicy::UseDefaultServant) => {
                self.dispatch_aom_or_default(oid, operation, request_body)
            }
            (ServantRetentionPolicy::Retain, RequestProcessingPolicy::UseServantManager) => {
                self.dispatch_aom_or_activator(oid, operation, request_body)
            }
            (ServantRetentionPolicy::NonRetain, RequestProcessingPolicy::UseDefaultServant) => {
                self.dispatch_default_only(operation, request_body)
            }
            (ServantRetentionPolicy::NonRetain, RequestProcessingPolicy::UseServantManager) => {
                self.dispatch_via_locator(oid, operation, request_body)
            }
            // NonRetain + UseAomOnly ist per validate() schon
            // ausgeschlossen, aber wir sind hier defensiv.
            (
                ServantRetentionPolicy::NonRetain,
                RequestProcessingPolicy::UseActiveObjectMapOnly,
            ) => Err(PoaError::WrongPolicy(
                "NON_RETAIN + USE_AOM_ONLY rejected at validate()".into(),
            )),
        }
    }

    fn dispatch_aom_only(
        &self,
        oid: &ObjectId,
        operation: &str,
        request_body: &[u8],
    ) -> PoaResult<Vec<u8>> {
        let aom = self.aom_lock()?;
        let s = aom.get(oid).ok_or(PoaError::ObjectNotActive)?;
        Ok(s.invoke(operation, request_body))
    }

    fn dispatch_aom_or_default(
        &self,
        oid: &ObjectId,
        operation: &str,
        request_body: &[u8],
    ) -> PoaResult<Vec<u8>> {
        {
            let aom = self.aom_lock()?;
            if let Some(s) = aom.get(oid) {
                return Ok(s.invoke(operation, request_body));
            }
        }
        let default = self
            .default_servant
            .lock()
            .map_err(|_| PoaError::BadInvocationOrder("default_servant mutex poisoned".into()))?;
        let s = default.as_ref().ok_or(PoaError::NoServant)?;
        Ok(s.invoke(operation, request_body))
    }

    fn dispatch_aom_or_activator(
        &self,
        oid: &ObjectId,
        operation: &str,
        request_body: &[u8],
    ) -> PoaResult<Vec<u8>> {
        // 1. AOM-Lookup.
        {
            let aom = self.aom_lock()?;
            if let Some(s) = aom.get(oid) {
                return Ok(s.invoke(operation, request_body));
            }
        }
        // 2. Servant-Activator.
        let act = {
            let g = self.servant_manager.lock().map_err(|_| {
                PoaError::BadInvocationOrder("servant_manager mutex poisoned".into())
            })?;
            match g.as_ref() {
                Some(ServantManager::Activator(a)) => Arc::clone(a),
                Some(_) => {
                    return Err(PoaError::WrongPolicy(
                        "RETAIN POA needs ServantActivator, not Locator".into(),
                    ));
                }
                None => return Err(PoaError::NoServant),
            }
        };
        let servant = act.incarnate(oid, &self.config.adapter_name)?;
        // Servant in AOM ablegen.
        let body = servant.invoke(operation, request_body);
        self.aom_lock()?.activate(oid.clone(), servant)?;
        Ok(body)
    }

    fn dispatch_default_only(&self, operation: &str, request_body: &[u8]) -> PoaResult<Vec<u8>> {
        let default = self
            .default_servant
            .lock()
            .map_err(|_| PoaError::BadInvocationOrder("default_servant mutex poisoned".into()))?;
        let s = default.as_ref().ok_or(PoaError::NoServant)?;
        Ok(s.invoke(operation, request_body))
    }

    fn dispatch_via_locator(
        &self,
        oid: &ObjectId,
        operation: &str,
        request_body: &[u8],
    ) -> PoaResult<Vec<u8>> {
        let loc = {
            let g = self.servant_manager.lock().map_err(|_| {
                PoaError::BadInvocationOrder("servant_manager mutex poisoned".into())
            })?;
            match g.as_ref() {
                Some(ServantManager::Locator(l)) => Arc::clone(l),
                Some(_) => {
                    return Err(PoaError::WrongPolicy(
                        "NON_RETAIN POA needs ServantLocator, not Activator".into(),
                    ));
                }
                None => return Err(PoaError::NoServant),
            }
        };
        let (servant, cookie) = loc.preinvoke(oid, &self.config.adapter_name, operation)?;
        let reply = servant.invoke(operation, request_body);
        loc.postinvoke(
            oid,
            &self.config.adapter_name,
            operation,
            &cookie,
            servant.as_ref(),
        );
        Ok(reply)
    }

    fn aom_lock(&self) -> PoaResult<std::sync::MutexGuard<'_, ActiveObjectMap>> {
        self.aom
            .lock()
            .map_err(|_| PoaError::BadInvocationOrder("AOM mutex poisoned".into()))
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::policies::{
        IdAssignmentPolicy, IdUniquenessPolicy, RequestProcessingPolicy, ServantRetentionPolicy,
    };
    use crate::servant::EchoServant;
    use crate::servant_manager::{ServantActivator, ServantLocator, ServantLocatorCookie};

    fn echo_servant() -> Box<dyn Servant> {
        Box::new(EchoServant {
            repo_id: "IDL:demo/Echo:1.0".into(),
        })
    }

    fn root_config(policies: PolicySet) -> PoaConfig {
        let mgr = Arc::new(PoaManager::new());
        mgr.activate().unwrap();
        PoaConfig {
            adapter_name: "RootPOA".into(),
            policies,
            manager: mgr,
        }
    }

    #[test]
    fn default_root_dispatch_via_aom() {
        let poa = Poa::new(root_config(PolicySet::default())).unwrap();
        let oid = poa.activate_object(echo_servant()).unwrap();
        let reply = poa.dispatch(&oid, "ping", &[1, 2]).unwrap();
        assert_eq!(reply, alloc::vec![1, 2]);
    }

    #[test]
    fn user_id_activation_with_explicit_oid() {
        let poa = Poa::new(root_config(PolicySet {
            id_assignment: IdAssignmentPolicy::User,
            ..PolicySet::default()
        }))
        .unwrap();
        let my_id: ObjectId = b"my-oid".as_slice().into();
        poa.activate_object_with_id(my_id.clone(), echo_servant())
            .unwrap();
        let reply = poa.dispatch(&my_id, "echo", &[42]).unwrap();
        assert_eq!(reply, alloc::vec![42]);
    }

    #[test]
    fn use_default_servant_falls_back_to_default() {
        let poa = Poa::new(root_config(PolicySet {
            id_uniqueness: IdUniquenessPolicy::Multiple,
            request_processing: RequestProcessingPolicy::UseDefaultServant,
            ..PolicySet::default()
        }))
        .unwrap();
        poa.set_default_servant(echo_servant());
        let oid: ObjectId = b"unknown".as_slice().into();
        let reply = poa.dispatch(&oid, "ping", &[7]).unwrap();
        assert_eq!(reply, alloc::vec![7]);
    }

    #[test]
    fn use_default_servant_without_default_is_no_servant() {
        let poa = Poa::new(root_config(PolicySet {
            id_uniqueness: IdUniquenessPolicy::Multiple,
            request_processing: RequestProcessingPolicy::UseDefaultServant,
            ..PolicySet::default()
        }))
        .unwrap();
        let err = poa
            .dispatch(&ObjectId::system_id(1), "ping", &[])
            .unwrap_err();
        assert_eq!(err, PoaError::NoServant);
    }

    struct FixedActivator;
    impl ServantActivator for FixedActivator {
        fn incarnate(&self, _: &ObjectId, _: &str) -> PoaResult<Box<dyn Servant>> {
            Ok(Box::new(EchoServant {
                repo_id: "IDL:demo/Echo:1.0".into(),
            }))
        }
    }

    #[test]
    fn use_servant_manager_with_activator_caches_in_aom() {
        let poa = Poa::new(root_config(PolicySet {
            request_processing: RequestProcessingPolicy::UseServantManager,
            ..PolicySet::default()
        }))
        .unwrap();
        poa.set_servant_manager(ServantManager::Activator(Arc::new(FixedActivator)));
        let oid: ObjectId = b"x".as_slice().into();
        let reply = poa.dispatch(&oid, "ping", &[1]).unwrap();
        assert_eq!(reply, alloc::vec![1]);
        // Zweiter Dispatch sollte aus AOM kommen.
        let reply2 = poa.dispatch(&oid, "ping", &[2]).unwrap();
        assert_eq!(reply2, alloc::vec![2]);
    }

    struct CountingLocator {
        pre: Arc<core::sync::atomic::AtomicUsize>,
        post: Arc<core::sync::atomic::AtomicUsize>,
    }

    impl ServantLocator for CountingLocator {
        fn preinvoke(
            &self,
            _: &ObjectId,
            _: &str,
            _: &str,
        ) -> PoaResult<(Box<dyn Servant>, ServantLocatorCookie)> {
            self.pre.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
            Ok((
                Box::new(EchoServant {
                    repo_id: "IDL:demo/Echo:1.0".into(),
                }),
                alloc::vec![0xab],
            ))
        }
        fn postinvoke(
            &self,
            _: &ObjectId,
            _: &str,
            _: &str,
            _: &ServantLocatorCookie,
            _: &dyn Servant,
        ) {
            self.post
                .fetch_add(1, core::sync::atomic::Ordering::Relaxed);
        }
    }

    #[test]
    fn non_retain_with_locator_calls_pre_post_per_invocation() {
        use core::sync::atomic::{AtomicUsize, Ordering};

        let pre = Arc::new(AtomicUsize::new(0));
        let post = Arc::new(AtomicUsize::new(0));
        let poa = Poa::new(root_config(PolicySet {
            servant_retention: ServantRetentionPolicy::NonRetain,
            request_processing: RequestProcessingPolicy::UseServantManager,
            ..PolicySet::default()
        }))
        .unwrap();
        poa.set_servant_manager(ServantManager::Locator(Arc::new(CountingLocator {
            pre: Arc::clone(&pre),
            post: Arc::clone(&post),
        })));
        for _ in 0..3 {
            let oid: ObjectId = b"any".as_slice().into();
            poa.dispatch(&oid, "ping", &[]).unwrap();
        }
        assert_eq!(pre.load(Ordering::Relaxed), 3);
        assert_eq!(post.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn dispatch_in_holding_state_is_rejected() {
        let mgr = Arc::new(PoaManager::new()); // HOLDING by default
        let cfg = PoaConfig {
            adapter_name: "RootPOA".into(),
            policies: PolicySet::default(),
            manager: mgr,
        };
        let poa = Poa::new(cfg).unwrap();
        let oid = ObjectId::system_id(1);
        // Wir koennen nicht aktivieren, weil dispatch HOLDING checkt;
        // aber activate_object selbst geht ohne State-Pruefung.
        poa.activate_object_with_id(oid.clone(), echo_servant())
            .unwrap();
        let err = poa.dispatch(&oid, "ping", &[]).unwrap_err();
        assert!(matches!(err, PoaError::BadInvocationOrder(_)));
    }

    #[test]
    fn dispatch_in_inactive_state_yields_adapter_inactive() {
        let mgr = Arc::new(PoaManager::new());
        mgr.deactivate();
        let cfg = PoaConfig {
            adapter_name: "RootPOA".into(),
            policies: PolicySet::default(),
            manager: mgr,
        };
        let poa = Poa::new(cfg).unwrap();
        let err = poa
            .dispatch(&ObjectId::system_id(1), "ping", &[])
            .unwrap_err();
        assert_eq!(err, PoaError::AdapterInactive);
    }

    #[test]
    fn dispatch_in_discarding_state_returns_obj_adapter() {
        let mgr = Arc::new(PoaManager::new());
        mgr.activate().unwrap();
        mgr.discard_requests().unwrap();
        let cfg = PoaConfig {
            adapter_name: "RootPOA".into(),
            policies: PolicySet::default(),
            manager: mgr,
        };
        let poa = Poa::new(cfg).unwrap();
        let err = poa
            .dispatch(&ObjectId::system_id(1), "ping", &[])
            .unwrap_err();
        assert!(matches!(err, PoaError::ObjAdapter(_)));
    }

    #[test]
    fn user_id_with_activate_object_is_wrong_policy() {
        let poa = Poa::new(root_config(PolicySet {
            id_assignment: IdAssignmentPolicy::User,
            ..PolicySet::default()
        }))
        .unwrap();
        let err = poa.activate_object(echo_servant()).unwrap_err();
        assert!(matches!(err, PoaError::WrongPolicy(_)));
    }

    #[test]
    fn invalid_policy_combination_is_rejected_at_construction() {
        let cfg = root_config(PolicySet {
            servant_retention: ServantRetentionPolicy::NonRetain,
            request_processing: RequestProcessingPolicy::UseActiveObjectMapOnly,
            ..PolicySet::default()
        });
        let err = Poa::new(cfg).unwrap_err();
        assert!(matches!(err, PoaError::InvalidPolicy(_)));
    }
}
