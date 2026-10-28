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
    /// The QoS profile exists but cannot be resolved: its `base_name`
    /// chain is circular, too long, or names a profile defined later
    /// (DDS-XML 1.0 §7.3.2.4.2).
    QosProfileInvalid {
        /// The `library::profile` path asked for.
        path: String,
        /// Why the profile cannot be resolved.
        reason: String,
    },
    /// A service definition written in Java did not parse.
    ///
    /// DDS-RPC 1.0 §7.3.2 defines a service definition as a modified
    /// subset of the Java grammar; [`crate::java_service_def`] accepts
    /// that subset and refuses the rest by name rather than by ignoring
    /// what it did not recognise. A skipped method is a service with an
    /// operation missing, which nothing downstream can tell from a
    /// service that never declared one.
    JavaSyntax {
        /// What was expected, in the grammar's terms.
        expected: &'static str,
        /// What stood there, as the source spells it.
        found: String,
        /// The 1-based line the token starts on.
        line: usize,
    },
    /// A model handle did not address a live node.
    ///
    /// The service model is lowered from handles into a
    /// [`zerodds_ast::Ast`], and a handle outlives the node it addresses
    /// when the model is edited underneath it. Reporting it is the
    /// alternative to skipping the node, which would produce a service
    /// with a method or a parameter silently missing.
    StaleHandle {
        /// What kind of node the handle addressed.
        what: &'static str,
    },
    /// The IDL uses a construct the service-definition grammar of
    /// DDS-RPC 1.0 §7.3.1.1 removed.
    ///
    /// Productions (1), (2), (7), (9) and (87) modify IDL 3.5 by
    /// removing `import`, `<type_id_dcl>`, `<type_prefix_dcl>`,
    /// `<event>`, `<component>`, `<home_dcl>`, embedded declarations of
    /// types, constants and exceptions in an interface, `oneway` and
    /// `context`. A service definition that carries one is outside the
    /// grammar, and accepting it silently changes the mapping (a
    /// constant in an interface vanishes; a `oneway` operation loses its
    /// reply structure and its Return-union case). Reporting it is the
    /// alternative.
    OutsideServiceGrammar {
        /// What the construct is, in the grammar's terms.
        construct: &'static str,
        /// Where it stands, as the source spells it.
        at: String,
    },
    /// A QoS profile sets something `RpcQos` cannot carry (DDS-RPC 1.0
    /// §7.10): a policy the requester and replier endpoints do not model,
    /// a container of an entity the RPC layer creates with its defaults, or
    /// a policy on which the profile's DataWriter and DataReader
    /// containers disagree.
    UnmodelledQos {
        /// The container the setting is in, as the XML names it
        /// (`datawriter_qos`, `topic_qos`, …).
        container: &'static str,
        /// The policy (`liveliness`, `partition`, …), or the container
        /// itself where the whole container is not modelled.
        policy: &'static str,
        /// Why it cannot be carried.
        reason: &'static str,
    },
    /// A pair of types marked `@RPCRequestType` / `@RPCReplyType` is not a
    /// service definition of DDS-RPC 1.0 §7.3.1.4.
    InvalidTypePair {
        /// The module the pair is declared in, `::`-separated (`""` at
        /// file scope).
        module: String,
        /// What is wrong, in the terms of the clause.
        reason: String,
    },
    /// The Enhanced client sent no request because the service was not
    /// discovered completely (DDS-RPC 1.0 §7.6.2.2.2): its reply reader
    /// had not matched the service's reply writer, or its request writer
    /// not the service's request reader, within the time the call may
    /// wait. The request has not been sent.
    ServiceNotMatched,
    /// Two members of an `@Autoid` structure of the Enhanced Service
    /// Mapping hash to the same member id.
    ///
    /// DDS-RPC 1.0 §7.5.1.2.1.2: "If a structure annotated as @Autoid
    /// has a hash collision due to two or more member names map to the
    /// same hash-code, the collision shall be reported as an error."
    MemberIdCollision {
        /// The synthesized structure.
        structure: String,
        /// The first member.
        first: String,
        /// The second member.
        second: String,
        /// The id both hash to.
        id: u32,
    },
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
            Self::QosProfileInvalid { path, reason } => {
                write!(f, "qos profile {path:?} cannot be resolved: {reason}")
            }
            Self::JavaSyntax {
                expected,
                found,
                line,
            } => write!(f, "line {line}: expected {expected}, found {found:?}"),
            Self::StaleHandle { what } => {
                write!(f, "a handle no longer addresses a live {what}")
            }
            Self::OutsideServiceGrammar { construct, at } => {
                write!(
                    f,
                    "{construct} {at:?} is outside the DDS-RPC service grammar"
                )
            }
            Self::UnmodelledQos {
                container,
                policy,
                reason,
            } => write!(
                f,
                "the QoS profile sets `{policy}` in `{container}`, which the RPC endpoints cannot carry: {reason}"
            ),
            Self::InvalidTypePair { module, reason } => {
                write!(f, "the type pair in module {module:?}: {reason}")
            }
            Self::ServiceNotMatched => write!(
                f,
                "the service is not discovered completely; the request was not sent"
            ),
            Self::MemberIdCollision {
                structure,
                first,
                second,
                id,
            } => write!(
                f,
                "members {first:?} and {second:?} of {structure} hash to the same member id {id:#010x}"
            ),
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
