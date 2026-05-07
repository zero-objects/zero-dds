// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! REP-2007 RMW C-API Skeleton — Rust-Schicht zur Implementierung
//! eines `rmw_zerodds`-Library.
//!
//! Spec REP-2007 + `rmw/rmw.h` (rmw 4.x). Dieses Modul liefert die
//! Rust-Side-Conversions + Result-Codes; den eigentlichen
//! `extern "C"`-Wrapper baut der Caller in einer dedizierten
//! `rmw-zerodds-shim`-Crate, da das C-Build-Setup (cbindgen +
//! ament-cmake-Hooks) ROS-2-Distribution-spezifisch ist.

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
    /// `true` wenn Return-Code OK.
    #[must_use]
    pub const fn is_ok(self) -> bool {
        matches!(self, Self::Ok)
    }

    /// Wire-Wert (i32).
    #[must_use]
    pub const fn to_i32(self) -> i32 {
        self as i32
    }
}

/// RMW-Implementation-Identifier — Spec REP-2007 §3.
/// Value: `"rmw_zerodds_cpp"` per Convention (`rmw_<vendor>_<lang>`).
pub const RMW_IMPLEMENTATION_IDENTIFIER: &str = "rmw_zerodds_cpp";

/// `rmw_node_t` — Logical Handle eines Participants. Spec REP-2007 §5.1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RmwNode {
    /// Implementation-Identifier (must match `RMW_IMPLEMENTATION_IDENTIFIER`).
    pub implementation_identifier: String,
    /// Node-Name (lokal eindeutig pro `rmw_init_options_t::context_handle`).
    pub name: String,
    /// Node-Namespace (typisch `/`).
    pub namespace: String,
    /// Domain-Id (DDS-Domain-ID).
    pub domain_id: u32,
}

/// Validierung der RMW-Implementation-Identifier-Constraint
/// (Spec REP-2007 §4.7: Cross-RMW-Handle-Mixing schlaegt fehl).
///
/// # Errors
/// `RmwRet::IncorrectRmwImplementation` wenn der Identifier nicht
/// `RMW_IMPLEMENTATION_IDENTIFIER` ist.
pub fn check_rmw_identifier(actual: &str) -> Result<(), RmwRet> {
    if actual == RMW_IMPLEMENTATION_IDENTIFIER {
        Ok(())
    } else {
        Err(RmwRet::IncorrectRmwImplementation)
    }
}

/// Mappt eine Rust-Result-Variante auf einen `rmw_ret_t`. Wird von
/// dem `extern "C"`-Wrapper benutzt um interne Fehler an C-Caller
/// zu propagieren.
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
