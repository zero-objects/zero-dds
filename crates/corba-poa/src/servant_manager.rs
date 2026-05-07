// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! ServantManager — Spec §11.3.5.10/11.3.5.11.
//!
//! Zwei Modi:
//! * `ServantActivator` (RETAIN-POAs): Servant wird beim ersten
//!   Request fuer eine Object-Id aktiviert und in der AOM gespeichert,
//!   bis explizit deactiviert.
//! * `ServantLocator` (NON_RETAIN-POAs): Pre-Invoke-Hook liefert den
//!   Servant pro Request, Post-Invoke-Hook gibt ihn frei.

use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::error::PoaResult;
use crate::object_id::ObjectId;
use crate::servant::Servant;

/// Generische ServantManager-Reference fuer einen POA.
#[derive(Clone)]
pub enum ServantManager {
    /// `ServantActivator` (RETAIN).
    Activator(Arc<dyn ServantActivator>),
    /// `ServantLocator` (NON_RETAIN).
    Locator(Arc<dyn ServantLocator>),
}

impl core::fmt::Debug for ServantManager {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Activator(_) => f.write_str("ServantManager::Activator"),
            Self::Locator(_) => f.write_str("ServantManager::Locator"),
        }
    }
}

/// `ServantActivator` — Spec §11.3.5.10.
pub trait ServantActivator: Send + Sync {
    /// `incarnate` — wird vom POA aufgerufen, wenn die Object-Id
    /// nicht in der AOM ist. Liefert den zu aktivierenden Servant.
    ///
    /// # Errors
    /// `NoServant` oder Caller-spezifische Exception (`ForwardRequest`-
    /// equivalent in unserer Repraesentation).
    fn incarnate(&self, oid: &ObjectId, adapter_name: &str) -> PoaResult<Box<dyn Servant>>;

    /// `etherealize` — wird vom POA aufgerufen, wenn der Servant
    /// deactiviert wird. Caller raeumt eventuelle Servant-Ressourcen
    /// auf.
    fn etherealize(&self, oid: &ObjectId, adapter_name: &str, _servant: &dyn Servant) {
        let _ = (oid, adapter_name);
    }
}

/// Cookie — opaque Caller-Daten, die zwischen `preinvoke` und
/// `postinvoke` propagiert werden (Spec §11.3.5.11).
pub type ServantLocatorCookie = Vec<u8>;

/// `ServantLocator` — Spec §11.3.5.11.
pub trait ServantLocator: Send + Sync {
    /// `preinvoke` — liefert den Servant + Cookie fuer einen Request.
    ///
    /// # Errors
    /// Caller-spezifisch.
    fn preinvoke(
        &self,
        oid: &ObjectId,
        adapter_name: &str,
        operation: &str,
    ) -> PoaResult<(Box<dyn Servant>, ServantLocatorCookie)>;

    /// `postinvoke` — wird nach Request-Dispatch aufgerufen, gibt
    /// den Servant frei.
    fn postinvoke(
        &self,
        oid: &ObjectId,
        adapter_name: &str,
        operation: &str,
        cookie: &ServantLocatorCookie,
        _servant: &dyn Servant,
    ) {
        let _ = (oid, adapter_name, operation, cookie);
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::servant::EchoServant;
    use core::sync::atomic::{AtomicUsize, Ordering};

    struct TestActivator {
        incarnate_count: Arc<AtomicUsize>,
    }

    impl ServantActivator for TestActivator {
        fn incarnate(&self, _: &ObjectId, _: &str) -> PoaResult<Box<dyn Servant>> {
            self.incarnate_count.fetch_add(1, Ordering::Relaxed);
            Ok(Box::new(EchoServant {
                repo_id: "IDL:demo/Echo:1.0".into(),
            }))
        }
    }

    #[test]
    fn activator_incarnate_is_called() {
        let counter = Arc::new(AtomicUsize::new(0));
        let act = TestActivator {
            incarnate_count: Arc::clone(&counter),
        };
        let _ = act.incarnate(&ObjectId::system_id(1), "RootPOA").unwrap();
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[allow(dead_code)]
    fn _dummy_consumes_string(_: String) {}

    struct TestLocator;
    impl ServantLocator for TestLocator {
        fn preinvoke(
            &self,
            _: &ObjectId,
            _: &str,
            _: &str,
        ) -> PoaResult<(Box<dyn Servant>, ServantLocatorCookie)> {
            Ok((
                Box::new(EchoServant {
                    repo_id: "IDL:demo/Echo:1.0".into(),
                }),
                alloc::vec![0xab, 0xcd],
            ))
        }
    }

    #[test]
    fn locator_preinvoke_returns_servant_and_cookie() {
        let loc = TestLocator;
        let (s, cookie) = loc
            .preinvoke(&ObjectId::system_id(1), "RootPOA", "ping")
            .unwrap();
        assert_eq!(cookie, alloc::vec![0xab, 0xcd]);
        assert_eq!(s.invoke("ping", &[1]), alloc::vec![1]);
    }
}
