// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CIF — Component Implementation Framework (spec §8).
//!
//! Four executor modes (spec §8.1.4):
//!
//! | Mode | Usage | Lifecycle |
//! |---|---|---|
//! | Session | one-instance-per-session | session-scoped |
//! | Service | stateless | per-request |
//! | Process | long-running per-process | process-scoped |
//! | Entity | persistent | persistent |
//!
//! The component executor implements the component method logic;
//! the home executor manages lifecycle calls (`ccm_activate` etc.).

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use crate::context::ComponentContext;

/// Common executor trait (all four modes).
pub trait ComponentExecutor: Send + Sync {
    /// `set_session_context` / `set_service_context` etc. — the
    /// container injects the context before the first method call
    /// (spec §8.1.5).
    fn set_context(&mut self, context: Box<dyn ComponentContext>);

    /// `ccm_activate` — spec §8.1.5.4.
    fn ccm_activate(&mut self) -> Result<(), CifError> {
        Ok(())
    }

    /// `ccm_passivate` — spec §8.1.5.5.
    fn ccm_passivate(&mut self) -> Result<(), CifError> {
        Ok(())
    }

    /// `ccm_remove` — spec §8.1.5.6.
    fn ccm_remove(&mut self) -> Result<(), CifError> {
        Ok(())
    }
}

/// CIF-specific errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CifError {
    /// `CCMException` — generic error from the executor.
    CcmException(String),
}

/// Session executor — spec §8.1.4.1.
pub trait SessionExecutor: ComponentExecutor {
    /// Marker method indicating the executor is in session mode.
    fn session_marker(&self) {}
}

/// Keyed executor — spec §8.1.4.4 (Entity).
pub trait KeyedExecutor: ComponentExecutor {
    /// Returns the primary key of the entity executor.
    fn primary_key(&self) -> Vec<u8>;
}

/// ExecutorLocator — spec §8.1.6.
///
/// Called by the container to obtain the appropriate executor for
/// each method call. For Session/Process: cached; for Service:
/// transient.
pub trait ExecutorLocator: Send + Sync {
    /// Before the method call.
    ///
    /// # Errors
    /// CIF error if the locator cannot provide the executor.
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
