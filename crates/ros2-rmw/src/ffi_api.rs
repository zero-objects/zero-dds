// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! REP-2007 RMW C-API skeleton — Rust layer for implementing
//! an `rmw_zerodds` library.
//!
//! Spec REP-2007 + `rmw/rmw.h` (rmw 4.x). This module provides the
//! Rust-side conversions + result codes; the actual
//! `extern "C"` wrapper is built by the caller in a dedicated
//! `rmw-zerodds-shim` crate, since the C build setup (cbindgen +
//! ament-cmake hooks) is ROS-2-distribution-specific.

use alloc::string::String;

/// `rmw_ret_t` — REP-2007 §4 Return-Codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum RmwRet {
    /// `RMW_RET_OK`.
    Ok = 0,
    /// `RMW_RET_ERROR`.
    Error = 1,
    /// `RMW_RET_TIMEOUT`.
    Timeout = 2,
    /// `RMW_RET_UNSUPPORTED`.
    Unsupported = 3,
    /// `RMW_RET_BAD_ALLOC`.
    BadAlloc = 10,
    /// `RMW_RET_INVALID_ARGUMENT`.
    InvalidArgument = 11,
    /// `RMW_RET_INCORRECT_RMW_IMPLEMENTATION`.
    IncorrectRmwImplementation = 12,
}

impl RmwRet {
    /// `true` if the return code is OK.
    #[must_use]
    pub const fn is_ok(self) -> bool {
        matches!(self, Self::Ok)
    }

    /// Wire value (i32).
    #[must_use]
    pub const fn to_i32(self) -> i32 {
        self as i32
    }
}

/// RMW-Implementation-Identifier — Spec REP-2007 §3.
/// Value: `"rmw_zerodds_cpp"` per Convention (`rmw_<vendor>_<lang>`).
pub const RMW_IMPLEMENTATION_IDENTIFIER: &str = "rmw_zerodds_cpp";

/// `rmw_node_t` — logical handle of a participant. Spec REP-2007 §5.1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RmwNode {
    /// Implementation identifier (must match `RMW_IMPLEMENTATION_IDENTIFIER`).
    pub implementation_identifier: String,
    /// Node name (locally unique per `rmw_init_options_t::context_handle`).
    pub name: String,
    /// Node namespace (typically `/`).
    pub namespace: String,
    /// Domain ID (DDS domain ID).
    pub domain_id: u32,
}

/// Validation of the RMW implementation-identifier constraint
/// (Spec REP-2007 §4.7: cross-RMW handle mixing fails).
///
/// # Errors
/// `RmwRet::IncorrectRmwImplementation` if the identifier is not
/// `RMW_IMPLEMENTATION_IDENTIFIER`.
pub fn check_rmw_identifier(actual: &str) -> Result<(), RmwRet> {
    if actual == RMW_IMPLEMENTATION_IDENTIFIER {
        Ok(())
    } else {
        Err(RmwRet::IncorrectRmwImplementation)
    }
}

/// Maps a Rust result variant to an `rmw_ret_t`. Used by
/// the `extern "C"` wrapper to propagate internal errors to C callers.
#[must_use]
pub fn map_to_rmw_ret<E>(r: Result<(), E>) -> RmwRet {
    match r {
        Ok(()) => RmwRet::Ok,
        Err(_) => RmwRet::Error,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn ok_is_zero() {
        assert_eq!(RmwRet::Ok.to_i32(), 0);
        assert!(RmwRet::Ok.is_ok());
    }

    #[test]
    fn error_codes_match_rmw_h() {
        assert_eq!(RmwRet::Error.to_i32(), 1);
        assert_eq!(RmwRet::Timeout.to_i32(), 2);
        assert_eq!(RmwRet::Unsupported.to_i32(), 3);
        assert_eq!(RmwRet::BadAlloc.to_i32(), 10);
        assert_eq!(RmwRet::InvalidArgument.to_i32(), 11);
        assert_eq!(RmwRet::IncorrectRmwImplementation.to_i32(), 12);
    }

    #[test]
    fn implementation_identifier_matches_convention() {
        assert!(RMW_IMPLEMENTATION_IDENTIFIER.starts_with("rmw_"));
        assert!(RMW_IMPLEMENTATION_IDENTIFIER.ends_with("_cpp"));
    }

    #[test]
    fn check_rmw_identifier_accepts_correct() {
        check_rmw_identifier(RMW_IMPLEMENTATION_IDENTIFIER).unwrap();
    }

    #[test]
    fn check_rmw_identifier_rejects_other_vendor() {
        assert_eq!(
            check_rmw_identifier("rmw_cyclonedds_cpp"),
            Err(RmwRet::IncorrectRmwImplementation)
        );
    }

    #[test]
    fn map_to_rmw_ret_ok() {
        let r: Result<(), &str> = Ok(());
        assert_eq!(map_to_rmw_ret(r), RmwRet::Ok);
    }

    #[test]
    fn map_to_rmw_ret_err() {
        let r: Result<(), &str> = Err("boom");
        assert_eq!(map_to_rmw_ret(r), RmwRet::Error);
    }

    #[test]
    fn rmw_node_construction() {
        let n = RmwNode {
            implementation_identifier: RMW_IMPLEMENTATION_IDENTIFIER.into(),
            name: "talker".into(),
            namespace: "/".into(),
            domain_id: 42,
        };
        assert_eq!(n.domain_id, 42);
    }
}
