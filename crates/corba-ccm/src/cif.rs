// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CIF — Component Implementation Framework (Spec §8).
//!
//! Vier Executor-Modi (Spec §8.1.4):
//!
//! | Mode | Verwendung | Lifecycle |
//! |---|---|---|
//! | Session | one-instance-per-session | session-scoped |
//! | Service | stateless | per-request |
//! | Process | long-running per-process | process-scoped |
//! | Entity | persistent | persistent |
//!
//! Component-Executor implementiert die Component-Method-Logik;
//! Home-Executor verwaltet Lifecycle-Calls (`ccm_activate` etc.).

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use crate::context::ComponentContext;

/// Allgemeines Executor-Trait (alle vier Modi).
pub trait ComponentExecutor: Send + Sync {
    /// `set_session_context` / `set_service_context` etc. — der
    /// Container injiziert den Context vor dem ersten Method-Call
    /// (Spec §8.1.5).
    fn set_context(&mut self, context: Box<dyn ComponentContext>);

    /// `ccm_activate` — Spec §8.1.5.4.
    fn ccm_activate(&mut self) -> Result<(), CifError> {
        Ok(())
    }

    /// `ccm_passivate` — Spec §8.1.5.5.
    fn ccm_passivate(&mut self) -> Result<(), CifError> {
        Ok(())
    }

    /// `ccm_remove` — Spec §8.1.5.6.
    fn ccm_remove(&mut self) -> Result<(), CifError> {
        Ok(())
    }
}

/// CIF-spezifische Fehler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CifError {
    /// `CCMException` — generischer Fehler aus dem Executor.
    CcmException(String),
}

/// Session-Executor — Spec §8.1.4.1.
pub trait SessionExecutor: ComponentExecutor {
    /// Marker-Method, dass der Executor Session-Mode hat.
    fn session_marker(&self) {}
}

/// Keyed-Executor — Spec §8.1.4.4 (Entity).
pub trait KeyedExecutor: ComponentExecutor {
    /// Liefert den Primary-Key des Entity-Executors.
    fn primary_key(&self) -> Vec<u8>;
}

/// ExecutorLocator — Spec §8.1.6.
///
/// Vom Container aufgerufen, um pro Method-Call den passenden
/// Executor zu bekommen. Bei Session/Process: cached; bei Service:
/// transient.
pub trait ExecutorLocator: Send + Sync {
    /// Vor dem Method-Call.
    ///
    /// # Errors
    /// CIF-Fehler, falls der Locator den Executor nicht liefern kann.
    fn obtain_executor(&self, oid: &[u8]) -> Result<Box<dyn ComponentExecutor>, CifError>;
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use core::sync::atomic::{AtomicUsize, Ordering};

    struct LoggingContext {
        log: alloc::sync::Arc<AtomicUsize>,
    }

    impl ComponentContext for LoggingContext {
        fn get_caller_principal(&self) -> Option<alloc::vec::Vec<u8>> {
            self.log.fetch_add(1, Ordering::Relaxed);
            None
        }
    }

    struct DemoExecutor {
        ctx: Option<Box<dyn ComponentContext>>,
        activations: AtomicUsize,
        passivations: AtomicUsize,
    }

    impl ComponentExecutor for DemoExecutor {
        fn set_context(&mut self, c: Box<dyn ComponentContext>) {
            self.ctx = Some(c);
        }
        fn ccm_activate(&mut self) -> Result<(), CifError> {
            self.activations.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
        fn ccm_passivate(&mut self) -> Result<(), CifError> {
            self.passivations.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
    }

    impl SessionExecutor for DemoExecutor {}

    #[test]
    fn lifecycle_calls_increment_counters() {
        let mut e = DemoExecutor {
            ctx: None,
            activations: AtomicUsize::new(0),
            passivations: AtomicUsize::new(0),
        };
        e.set_context(Box::new(LoggingContext {
            log: alloc::sync::Arc::new(AtomicUsize::new(0)),
        }));
        assert!(e.ctx.is_some());
        e.ccm_activate().unwrap();
        e.ccm_activate().unwrap();
        e.ccm_passivate().unwrap();
        assert_eq!(e.activations.load(Ordering::Relaxed), 2);
        assert_eq!(e.passivations.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn cif_error_carries_diagnostic() {
        let e = CifError::CcmException("permission denied".into());
        match e {
            CifError::CcmException(s) => assert_eq!(s, "permission denied"),
        }
    }
}
