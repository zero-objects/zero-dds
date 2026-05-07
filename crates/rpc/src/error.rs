// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Fehler-Typen fuer das DDS-RPC-Crate (C6.1.A).
//!
//! Wird von [`crate::common_types`], [`crate::topic_naming`],
//! [`crate::annotations`] und [`crate::service_mapping`] geteilt.

extern crate alloc;

use alloc::string::{String, ToString};
use core::fmt;

/// Sammelfehler fuer die Foundation-Stufe der DDS-RPC-Implementierung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RpcError {
    /// XCDR2-Encoding/Decoding hat das Wire-Format verletzt.
    Codec(String),
    /// DoS-Cap fuer Wire-Payload ueberschritten.
    PayloadTooLarge {
        /// Beobachtete Bytes.
        got: usize,
        /// Erlaubtes Maximum.
        max: usize,
    },
    /// Service-Name ist leer oder enthaelt ungueltige Zeichen.
    InvalidServiceName(String),
    /// Methoden-Name ist leer oder ungueltig.
    InvalidMethodName(String),
    /// Methode ist `oneway`, hat aber ein non-`void` Return.
    OnewayWithReturn(String),
    /// Methode ist `oneway`, hat aber `out`/`inout`-Parameter.
    OnewayWithOutParam {
        /// Methoden-Name.
        method: String,
        /// Parameter-Name.
        param: String,
    },
    /// Doppelte Methoden-Namen in einem `interface`.
    DuplicateMethod(String),
    /// Doppelte Parameter-Namen in einer Methode.
    DuplicateParam {
        /// Methoden-Name.
        method: String,
        /// Parameter-Name.
        param: String,
    },
    /// Unbekannter `RemoteExceptionCode_t`-Diskriminator beim Decode.
    UnknownExceptionCode(u32),
    /// Service ohne Methoden — kein Endpoint kann gebaut werden.
    EmptyService(String),
    /// Request-Reply-Wartezeit ueberschritten (Foundation-Stufe C6.1.C).
    Timeout,
    /// Server-Side hat eine `RemoteExceptionCode` ungleich `Ok` zurueckgegeben.
    /// Der Wert ist der raw Diskriminator — Caller kann
    /// [`crate::common_types::RemoteExceptionCode::from_u32`] nutzen, um in
    /// das Enum zu mappen.
    RemoteException(u32),
    /// Doppelt vergebener `service_instance_name` auf demselben Participant.
    DuplicateInstanceName(String),
    /// Generischer DCPS-Aufruf-Fehler (Topic-Anlegen, Writer-Create etc.).
    Dcps(String),
    /// QoS-Profile nicht in der `DdsXml`-Library gefunden.
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
    /// Convenience: Konstruktor fuer Codec-Fehler aus statischer Message.
    #[must_use]
    pub fn codec(msg: &str) -> Self {
        Self::Codec(msg.to_string())
    }
}

/// Result-Alias.
pub type RpcResult<T> = core::result::Result<T, RpcError>;
