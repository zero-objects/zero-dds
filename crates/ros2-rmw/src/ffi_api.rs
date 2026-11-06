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

    /// rmw/ret_types.h: RMW_RET_OK 0, RMW_RET_ERROR 1, RMW_RET_TIMEOUT 2,
    /// RMW_RET_UNSUPPORTED 3, RMW_RET_BAD_ALLOC 10,
    /// RMW_RET_INVALID_ARGUMENT 11, RMW_RET_INCORRECT_RMW_IMPLEMENTATION 12.
    #[test]
    fn rmw_ret_types_h_the_return_codes_have_the_header_values() {
        for (ret, value) in [
            (RmwRet::Ok, 0),
            (RmwRet::Error, 1),
            (RmwRet::Timeout, 2),
            (RmwRet::Unsupported, 3),
            (RmwRet::BadAlloc, 10),
            (RmwRet::InvalidArgument, 11),
            (RmwRet::IncorrectRmwImplementation, 12),
        ] {
            assert_eq!(ret.to_i32(), value, "{ret:?}");
            assert_eq!(ret.is_ok(), value == 0, "{ret:?}");
        }
    }

    /// rmw/rmw.h: a function that succeeds returns RMW_RET_OK, one that
    /// fails without a more specific code RMW_RET_ERROR.
    #[test]
    fn rmw_h_success_is_rmw_ret_ok_and_a_plain_failure_rmw_ret_error() {
        assert_eq!(map_to_rmw_ret::<&str>(Ok(())), RmwRet::Ok);
        assert_eq!(map_to_rmw_ret(Err("boom")), RmwRet::Error);
    }

    /// rmw/rmw.h `rmw_get_implementation_identifier` and
    /// rmw/check_type_identifiers_match.h: the identifier is the name
    /// under which ament registers the implementation (`rmw_zerodds_cpp`),
    /// and a handle of another implementation is refused with
    /// RMW_RET_INCORRECT_RMW_IMPLEMENTATION.
    #[test]
    fn rmw_h_the_identifier_is_rmw_zerodds_cpp_and_another_implementation_is_refused() {
        assert_eq!(RMW_IMPLEMENTATION_IDENTIFIER, "rmw_zerodds_cpp");
        check_rmw_identifier("rmw_zerodds_cpp").unwrap();
        for other in ["rmw_cyclonedds_cpp", "rmw_fastrtps_cpp", ""] {
            assert_eq!(
                check_rmw_identifier(other),
                Err(RmwRet::IncorrectRmwImplementation),
                "{other}"
            );
        }
    }
}
