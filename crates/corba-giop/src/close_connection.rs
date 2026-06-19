// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CloseConnection message — spec §15.4.7.
//!
//! Body-free. The server sends this message before closing the TCP
//! connection, to signal to the client that pending requests may be
//! retried (spec §15.4.7, normative).

/// CloseConnection body — empty per spec §15.4.7.
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
