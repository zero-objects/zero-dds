// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CloseConnection-Message — Spec §15.4.7.
//!
//! Body-frei. Server sendet diese Message, bevor er die TCP-
//! Connection schliesst, um dem Client zu signalisieren, dass
//! Pending-Requests retried werden duerfen (Spec §15.4.7 normativ).

/// CloseConnection-Body — leer per Spec §15.4.7.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct CloseConnection;

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn body_is_unit() {
        let _ = CloseConnection;
    }
}
