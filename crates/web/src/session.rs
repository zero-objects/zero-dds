// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `SessionId` Authentifizierte Session — Spec §7.3.1.1.

use alloc::string::String;

/// Spec §7.3.1.1 — Authenticated SessionToken (returned by Root::
/// `create_application` als `authenticatedSessionRepresentation`).
///
/// Spec sagt: "The purpose of the SessionId type is to remember an
/// authenticated client across subsequent invocations of operations on
/// the service."
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionId(pub String);

impl SessionId {
    /// Konstruktor.
    #[must_use]
    pub fn new(token: impl Into<String>) -> Self {
        Self(token.into())
    }

    /// Liefert den Token-String.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// `true` wenn der Token nicht leer ist.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        !self.0.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_id_carries_token_string() {
        let s = SessionId::new("abc123");
        assert_eq!(s.as_str(), "abc123");
        assert!(s.is_valid());
    }

    #[test]
    fn empty_session_id_is_invalid() {
        let s = SessionId::new("");
        assert!(!s.is_valid());
    }
}
