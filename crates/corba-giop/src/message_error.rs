// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! MessageError-Message — Spec §15.4.8.
//!
//! Body-frei. Wird gesendet, wenn eine Message mit unbekanntem
//! Magic / Version / Type empfangen wurde, oder wenn der Header
//! malformed ist. Caller schliesst danach die Connection (Spec
//! §15.4.8 normativ).

/// MessageError-Body — leer per Spec §15.4.8.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct MessageError;

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn body_is_unit() {
        let _ = MessageError;
    }
}
