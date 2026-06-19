// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Error types for the DDS-RPC crate (C6.1.A).
//!
//! Shared by [`crate::common_types`], [`crate::topic_naming`],
//! [`crate::annotations`] and [`crate::service_mapping`].

extern crate alloc;

use alloc::string::{String, ToString};
use core::fmt;

/// Collective error for the foundation stage of the DDS-RPC implementation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RpcError {
    /// XCDR2 encoding/decoding violated the wire format.
    Codec(String),
    /// DoS cap for the wire payload exceeded.
    PayloadTooLarge {
        /// Observed bytes.
        got: usize,
        /// Allowed maximum.
        max: usize,
    },
    /// The service name is empty or contains invalid characters.
    InvalidServiceName(String),
    /// The method name is empty or invalid.
    InvalidMethodName(String),
    /// The method is `oneway` but has a non-`void` return.
    OnewayWithReturn(String),
    /// The method is `oneway` but has `out`/`inout` parameters.
    OnewayWithOutParam {
        /// Method name.
        method: String,
        /// Parameter name.
        param: String,
    },
    /// Duplicate method names in an `interface`.
    DuplicateMethod(String),
    /// Duplicate parameter names in a method.
    DuplicateParam {
        /// Method name.
        method: String,
        /// Parameter name.
        param: String,
    },
    /// Unknown `RemoteExceptionCode_t` discriminator on decode.
    UnknownExceptionCode(u32),
    /// Service without methods — no endpoint can be built.
    EmptyService(String),
    /// Request-reply wait time exceeded (foundation stage C6.1.C).
    Timeout,
    /// The server side returned a `RemoteExceptionCode` other than `Ok`.
    /// The value is the raw discriminator — the caller can use
    /// [`crate::common_types::RemoteExceptionCode::from_u32`] to map it into
    /// the enum.
    RemoteException(u32),
    /// A `service_instance_name` assigned twice on the same participant.
    DuplicateInstanceName(String),
    /// Generic DCPS call error (topic creation, writer create, etc.).
    Dcps(String),
    /// QoS profile not found in the `DdsXml` library.
    QosProfileNotFound(String),
}

impl fmt::Display for RpcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Codec(s) => write!(f, "rpc codec error: {s}"),
            Self::PayloadTooLarge { got, max } => {
                write!(f, "rpc payload too large: {got} > {max}")
            }
            Self::InvalidServiceName(s) => write!(f, "invalid service name: {s:?}"),
            Self::InvalidMethodName(s) => write!(f, "invalid method name: {s:?}"),
            Self::OnewayWithReturn(m) => {
                write!(f, "oneway method {m:?} must return void")
            }
            Self::OnewayWithOutParam { method, param } => write!(
                f,
                "oneway method {method:?} must not have out/inout param {param:?}"
            ),
            Self::DuplicateMethod(m) => write!(f, "duplicate method {m:?}"),
            Self::DuplicateParam { method, param } => {
                write!(f, "duplicate parameter {param:?} in method {method:?}")
            }
            Self::UnknownExceptionCode(v) => {
                write!(f, "unknown RemoteExceptionCode_t discriminator {v}")
            }
            Self::EmptyService(n) => write!(f, "service {n:?} has no methods"),
            Self::Timeout => write!(f, "rpc request timed out"),
            Self::RemoteException(code) => {
                write!(f, "remote exception code {code}")
            }
            Self::DuplicateInstanceName(n) => {
                write!(f, "duplicate service instance name {n:?}")
            }
            Self::Dcps(s) => write!(f, "dcps error: {s}"),
            Self::QosProfileNotFound(n) => {
                write!(f, "qos profile {n:?} not found")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RpcError {}

impl RpcError {
    /// Convenience: constructor for a codec error from a static message.
    #[must_use]
    pub fn codec(msg: &str) -> Self {
        Self::Codec(msg.to_string())
    }
}

/// Result-Alias.
pub type RpcResult<T> = core::result::Result<T, RpcError>;
