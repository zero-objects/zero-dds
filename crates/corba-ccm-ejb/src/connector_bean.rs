// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! ConnectorBean — JEE-EJB-Lifecycle-Mapping zu CCM `ComponentExecutor`.
//!
//! JEE-Container ruft auf:
//!
//! * `@PostConstruct` → CCM `ccm_activate`
//! * `@PreDestroy` → CCM `ccm_passivate` + `ccm_remove`
//! * `@Resource` (Injection) → CCM `set_context`
//!
//! Spec-Refs: CCM 4.0 §6.6 (Executor-Lifecycle), JSR 318 EJB 3.1
//! §4.7 (Bean-Lifecycle-Callbacks).

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

/// Lifecycle-Phase. Spec CCM §6.6.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LifecyclePhase {
    /// `@PostConstruct` → `set_context` + `ccm_activate`.
    PostConstruct,
    /// `@PreDestroy` → `ccm_passivate` + `ccm_remove`.
    PreDestroy,
    /// `@PrePassivate` → `ccm_passivate`.
    PrePassivate,
    /// `@PostActivate` → `ccm_activate`.
    PostActivate,
}

/// Hook fuer eine Lifecycle-Phase. Caller-Layer (z.B. EJB-Container)
/// invoket diese.
pub trait LifecycleCallback {
    /// Wird vom Container gerufen, wenn die Phase eintritt.
    ///
    /// # Errors
    /// Static-String wenn die Phase nicht abgeschlossen werden kann.
    fn on_phase(&mut self, phase: LifecyclePhase) -> Result<(), &'static str>;
}

/// ConnectorBean — kapselt einen CCM-ComponentExecutor mit JEE-
/// Lifecycle-Annotations.
pub struct ConnectorBean {
    /// Bean-Name (typisch `<Component>Bean`).
    pub name: String,
    /// Component-Repository-ID (Spec CCM §6.2.6.1).
    pub component_id: String,
    callback: Box<dyn LifecycleCallback + Send>,
    history: Vec<LifecyclePhase>,
}

impl core::fmt::Debug for ConnectorBean {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ConnectorBean")
            .field("name", &self.name)
            .field("component_id", &self.component_id)
            .field("history", &self.history)
            .finish_non_exhaustive()
    }
}

impl ConnectorBean {
    /// Konstruktor.
    pub fn new<C>(name: String, component_id: String, callback: C) -> Self
    where
        C: LifecycleCallback + Send + 'static,
    {
        Self {
            name,
            component_id,
            callback: Box::new(callback),
            history: Vec::new(),
        }
    }

    /// Triggers die Lifecycle-Phase und protokolliert sie.
    ///
    /// # Errors
    /// Wenn der Callback fehlschlaegt.
    pub fn enter_phase(&mut self, phase: LifecyclePhase) -> Result<(), &'static str> {
        self.callback.on_phase(phase)?;
        self.history.push(phase);
        Ok(())
    }

    /// Liste der absolvierten Phasen (in Aufruf-Reihenfolge).
    #[must_use]
    pub fn history(&self) -> &[LifecyclePhase] {
        &self.history
    }
}

// ---------------------------------------------------------------------------
// CCM 4.0 §2 CP6 — PSS-Session-Bind-Helper.
// ---------------------------------------------------------------------------

/// Spec CCM 4.0 §2 CP6 — bindet eine PSS-Session an einen
/// ConnectorBean. Liefert das Tupel `(bean.name, bean.component_id)`
/// als Bind-Identifier zusammen mit dem aktuellen Tx-Status der
/// PSS-Session (CP6 verlangt PSS-Lifecycle gekoppelt an Bean-
/// Lifecycle).
///
/// Cross-Ref `corba-ccm::pss::PssSession::tx_status`.
#[cfg(feature = "std")]
pub fn pss_session_for_bean<'b>(
    bean: &'b ConnectorBean,
    pss: &zerodds_corba_ccm::pss::PssSession,
) -> PssBeanBinding<'b> {
    PssBeanBinding {
        bean_name: &bean.name,
        component_id: &bean.component_id,
        tx_status: pss.tx_status(),
    }
}

/// Spec CCM 4.0 §2 CP6 — Ergebnis von [`pss_session_for_bean`].
#[cfg(feature = "std")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PssBeanBinding<'b> {
    /// `ConnectorBean::name`.
    pub bean_name: &'b str,
    /// `ConnectorBean::component_id`.
    pub component_id: &'b str,
    /// Aktueller PSS-Tx-Status zum Bind-Zeitpunkt.
    pub tx_status: zerodds_corba_ccm::pss::PssTxStatus,
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    struct Counter {
        post_construct: u32,
        pre_destroy: u32,
    }

    impl LifecycleCallback for Counter {
        fn on_phase(&mut self, phase: LifecyclePhase) -> Result<(), &'static str> {
            match phase {
                LifecyclePhase::PostConstruct => self.post_construct += 1,
                LifecyclePhase::PreDestroy => self.pre_destroy += 1,
                _ => {}
            }
            Ok(())
        }
    }

    #[test]
    fn post_construct_invokes_callback() {
        let mut bean = ConnectorBean::new(
            "EchoBean".into(),
            "IDL:demo/Echo:1.0".into(),
            Counter {
                post_construct: 0,
                pre_destroy: 0,
            },
        );
        bean.enter_phase(LifecyclePhase::PostConstruct).unwrap();
        assert_eq!(bean.history(), &[LifecyclePhase::PostConstruct]);
    }

    #[test]
    fn full_lifecycle_records_history() {
        let mut bean = ConnectorBean::new(
            "EchoBean".into(),
            "IDL:demo/Echo:1.0".into(),
            Counter {
                post_construct: 0,
                pre_destroy: 0,
            },
        );
        bean.enter_phase(LifecyclePhase::PostConstruct).unwrap();
        bean.enter_phase(LifecyclePhase::PrePassivate).unwrap();
        bean.enter_phase(LifecyclePhase::PostActivate).unwrap();
        bean.enter_phase(LifecyclePhase::PreDestroy).unwrap();
        assert_eq!(bean.history().len(), 4);
        assert_eq!(bean.history()[0], LifecyclePhase::PostConstruct);
        assert_eq!(bean.history()[3], LifecyclePhase::PreDestroy);
    }

    #[cfg(feature = "std")]
    #[test]
    fn connector_bean_with_pss_session_binds_correctly() {
        use alloc::sync::Arc;
        use zerodds_corba_ccm::pss::{InMemoryStorageHome, PssSession, PssTxStatus, StorageHome};

        let bean = ConnectorBean::new(
            "EchoBean".into(),
            "IDL:demo/Echo:1.0".into(),
            Counter {
                post_construct: 0,
                pre_destroy: 0,
            },
        );
        let home = Arc::new(InMemoryStorageHome::new()) as Arc<dyn StorageHome>;
        let pss = PssSession::new(home);
        let binding = pss_session_for_bean(&bean, &pss);
        assert_eq!(binding.bean_name, "EchoBean");
        assert_eq!(binding.component_id, "IDL:demo/Echo:1.0");
        assert_eq!(binding.tx_status, PssTxStatus::NoTransaction);

        // Bei aktiver Tx ist das Binding entsprechend markiert.
        let _tx = pss.begin_transaction().expect("begin");
        let active_binding = pss_session_for_bean(&bean, &pss);
        assert_eq!(active_binding.tx_status, PssTxStatus::Active);
    }

    #[test]
    fn callback_failure_propagates() {
        struct Failing;
        impl LifecycleCallback for Failing {
            fn on_phase(&mut self, _: LifecyclePhase) -> Result<(), &'static str> {
                Err("boom")
            }
        }
        let mut bean = ConnectorBean::new("X".into(), "IDL:x:1.0".into(), Failing);
        assert!(bean.enter_phase(LifecyclePhase::PostConstruct).is_err());
        assert!(bean.history().is_empty());
    }
}
