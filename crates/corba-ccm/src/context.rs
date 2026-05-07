// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Component-Context — Spec §8.1.7.
//!
//! Wird vom Container in den Executor injiziert; gibt dem
//! Executor Zugriff auf Container-Services (PrincipalAuth,
//! Receptacle-Connections, Transactions).

use alloc::vec::Vec;

/// Component-Context — Spec §8.1.7.1.
pub trait ComponentContext: Send + Sync {
    /// `get_caller_principal` — Caller-Identitaet aus CSIv2-SAS.
    /// `None` wenn anonyme Verbindung.
    fn get_caller_principal(&self) -> Option<Vec<u8>>;
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    struct AnonContext;
    impl ComponentContext for AnonContext {
        fn get_caller_principal(&self) -> Option<Vec<u8>> {
            None
        }
    }

    struct AlicePrincipal;
    impl ComponentContext for AlicePrincipal {
        fn get_caller_principal(&self) -> Option<Vec<u8>> {
            Some(b"alice".to_vec())
        }
    }

    #[test]
    fn anonymous_context_returns_none() {
        assert!(AnonContext.get_caller_principal().is_none());
    }

    #[test]
    fn principal_context_returns_bytes() {
        assert_eq!(
            AlicePrincipal.get_caller_principal(),
            Some(b"alice".to_vec())
        );
    }
}
