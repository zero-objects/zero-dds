// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! MessageError-Message — Spec §15.4.8.
//!
//! Body-free. Sent when a message with an unknown magic / version /
//! type is received, or when the header is malformed. The caller then
//! closes the connection (Spec §15.4.8, normative).

/// MessageError body — empty per Spec §15.4.8.
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
