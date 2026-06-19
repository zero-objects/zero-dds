// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Component-Context — Spec §8.1.7.
//!
//! Injected by the container into the executor; gives the
//! Executor access to container services (PrincipalAuth,
//! Receptacle-Connections, Transactions).

use alloc::vec::Vec;

/// Component context — spec §8.1.7.1.
pub trait ComponentContext: Send + Sync {
    /// `get_caller_principal` — caller identity from CSIv2-SAS.
    /// `None` for an anonymous connection.
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
