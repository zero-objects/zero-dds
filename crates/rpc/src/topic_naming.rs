// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS-RPC topic naming convention — Spec §7.8.2.
//!
//! From a service name `S`, two topic names are derived:
//!
//! * Request topic: `<S>_Request`
//! * Reply topic:   `<S>_Reply`
//!
//! Service names must be non-empty and may only contain ASCII
//! letters (`A-Z`, `a-z`), digits (`0-9`) and `_`. The
//! first character must be a letter or `_` (analogous to the C identifier
//! rule — the spec references IDL identifiers).

extern crate alloc;

use alloc::format;
use alloc::string::String;

use crate::error::{RpcError, RpcResult};

/// Topic suffix for request topics (Spec §7.8.2).
pub const REQUEST_SUFFIX: &str = "_Request";

/// Topic suffix for reply topics (Spec §7.8.2).
pub const REPLY_SUFFIX: &str = "_Reply";

/// Validates a service name.
///
/// # Errors
/// `RpcError::InvalidServiceName` if the name is empty or contains
/// characters outside `[A-Za-z0-9_]`, or begins with a digit.
pub fn validate_service_name(service: &str) -> RpcResult<()> {
    if service.is_empty() {
        return Err(RpcError::InvalidServiceName(String::new()));
    }
    let first = service.as_bytes()[0];
    if !(first.is_ascii_alphabetic() || first == b'_') {
        return Err(RpcError::InvalidServiceName(service.into()));
    }
    for &b in service.as_bytes() {
        if !(b.is_ascii_alphanumeric() || b == b'_') {
            return Err(RpcError::InvalidServiceName(service.into()));
        }
    }
    Ok(())
}

/// Returns the request topic name for a service.
///
/// # Errors
/// See [`validate_service_name`].
pub fn request_topic_name(service: &str) -> RpcResult<String> {
    validate_service_name(service)?;
    Ok(format!("{service}{REQUEST_SUFFIX}"))
}

/// Returns the reply topic name for a service.
///
/// # Errors
/// See [`validate_service_name`].
pub fn reply_topic_name(service: &str) -> RpcResult<String> {
    validate_service_name(service)?;
    Ok(format!("{service}{REPLY_SUFFIX}"))
}

/// Convenient container for the pair of topic names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceTopicNames {
    /// Service name (validated).
    pub service: String,
    /// `<service>_Request`.
    pub request: String,
    /// `<service>_Reply`.
    pub reply: String,
}

impl ServiceTopicNames {
    /// Constructor with validation.
    ///
    /// # Errors
    /// `RpcError::InvalidServiceName` on an empty or invalid
    /// service name.
    pub fn new(service: &str) -> RpcResult<Self> {
        let request = request_topic_name(service)?;
        let reply = reply_topic_name(service)?;
        Ok(Self {
            service: service.into(),
            request,
            reply,
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn happy_path_calculator() {
        assert_eq!(
            request_topic_name("Calculator").unwrap(),
            "Calculator_Request"
        );
        assert_eq!(reply_topic_name("Calculator").unwrap(), "Calculator_Reply");
    }

    #[test]
    fn underscore_prefix_is_valid() {
        // Spec references IDL identifiers; `_foo` is legitimate.
        assert!(validate_service_name("_foo").is_ok());
    }

    #[test]
    fn alphanumeric_with_digits_in_middle() {
        assert!(validate_service_name("Calc2_v3").is_ok());
    }

    #[test]
    fn empty_name_rejected() {
        let err = validate_service_name("").unwrap_err();
        assert_eq!(err, RpcError::InvalidServiceName(String::new()));
    }

    #[test]
    fn whitespace_rejected() {
        let err = validate_service_name("My Service").unwrap_err();
        assert!(matches!(err, RpcError::InvalidServiceName(_)));
    }

    #[test]
    fn dash_rejected() {
        let err = validate_service_name("my-service").unwrap_err();
        assert!(matches!(err, RpcError::InvalidServiceName(_)));
    }

    #[test]
    fn starts_with_digit_rejected() {
        let err = validate_service_name("9calc").unwrap_err();
        assert!(matches!(err, RpcError::InvalidServiceName(_)));
    }

    #[test]
    fn non_ascii_unicode_rejected() {
        let err = validate_service_name("Rechnerü").unwrap_err();
        assert!(matches!(err, RpcError::InvalidServiceName(_)));
    }

    #[test]
    fn service_topic_names_pair() {
        let names = ServiceTopicNames::new("Calc").unwrap();
        assert_eq!(names.service, "Calc");
        assert_eq!(names.request, "Calc_Request");
        assert_eq!(names.reply, "Calc_Reply");
    }

    #[test]
    fn service_topic_names_propagates_error() {
        let err = ServiceTopicNames::new("").unwrap_err();
        assert!(matches!(err, RpcError::InvalidServiceName(_)));
    }
}
