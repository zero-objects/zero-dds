// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Method properties (idempotency / safety) per RFC 7252 §5.8.
//!
//! GET is `safe + idempotent`; PUT, DELETE are `idempotent`;
//! POST is `not safe + not idempotent`. These properties are
//! relevant for caching (§5.6) and retry logic.

use crate::message::CoapCode;

/// Spec §5.8.1 — `is_safe(method)`.
///
/// A method is "safe" if it does not change server state.
/// Only GET is safe.
#[must_use]
pub fn is_safe(method: CoapCode) -> bool {
    method == CoapCode::GET
}

/// Spec §5.8.x — `is_idempotent(method)`.
///
/// A method is idempotent if N invocations have the same effect
/// as 1 invocation. GET, PUT, DELETE are idempotent;
/// POST is not.
#[must_use]
pub fn is_idempotent(method: CoapCode) -> bool {
    method == CoapCode::GET || method == CoapCode::PUT || method == CoapCode::DELETE
}

/// Spec §5.8 — `is_method(code)`.
///
/// Returns `true` if the code is a request method (class 0,
/// codes 1-31).
#[must_use]
pub fn is_method(code: CoapCode) -> bool {
    code == CoapCode::GET
        || code == CoapCode::POST
        || code == CoapCode::PUT
        || code == CoapCode::DELETE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_is_safe() {
        assert!(is_safe(CoapCode::GET));
    }

    #[test]
    fn post_put_delete_not_safe() {
        assert!(!is_safe(CoapCode::POST));
        assert!(!is_safe(CoapCode::PUT));
        assert!(!is_safe(CoapCode::DELETE));
    }

    #[test]
    fn get_put_delete_idempotent() {
        assert!(is_idempotent(CoapCode::GET));
        assert!(is_idempotent(CoapCode::PUT));
        assert!(is_idempotent(CoapCode::DELETE));
    }

    #[test]
    fn post_not_idempotent() {
        assert!(!is_idempotent(CoapCode::POST));
    }

    #[test]
    fn method_predicate_recognizes_all_four() {
        assert!(is_method(CoapCode::GET));
        assert!(is_method(CoapCode::POST));
        assert!(is_method(CoapCode::PUT));
        assert!(is_method(CoapCode::DELETE));
    }
}
