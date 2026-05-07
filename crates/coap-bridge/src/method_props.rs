// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Method-Properties (Idempotency / Safety) nach RFC 7252 §5.8.
//!
//! GET ist `safe + idempotent`; PUT, DELETE sind `idempotent`;
//! POST ist `nicht safe + nicht idempotent`. Diese Properties sind
//! relevant fuer Caching (§5.6) und Retry-Logik.

use crate::message::CoapCode;

/// Spec §5.8.1 — `is_safe(method)`.
///
/// Eine Method ist "safe" wenn sie keinen Server-State aendert.
/// Nur GET ist safe.
#[must_use]
pub fn is_safe(method: CoapCode) -> bool {
    method == CoapCode::GET
}

/// Spec §5.8.x — `is_idempotent(method)`.
///
/// Eine Method ist idempotent wenn N Aufrufe denselben Effekt
/// haben wie 1 Aufruf. GET, PUT, DELETE sind idempotent;
/// POST nicht.
#[must_use]
pub fn is_idempotent(method: CoapCode) -> bool {
    method == CoapCode::GET || method == CoapCode::PUT || method == CoapCode::DELETE
}

/// Spec §5.8 — `is_method(code)`.
///
/// Liefert `true` wenn der Code eine Request-Method ist (Class 0,
/// Codes 1-31).
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
