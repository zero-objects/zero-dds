// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Function-call-style service API (Spec §7.2.2.1, §7.9.2.1, §7.10.1).
//!
//! The DDS-RPC spec defines two language-binding styles:
//! 1. **Request/reply style** (low-level) — `crates/rpc/src/{requester,
//!    replier}.rs`.
//! 2. **Function-call style** (high-level) — *this module*.
//!
//! Function-call style uses stubs (client-side proxy) and
//! skeletons (service-side dispatch), generated at codegen time from a
//! service definition (IDL `interface Foo { void op(); }`).
//! The stubs look like native function calls, but internally
//! encapsulate the request/reply path.
//!
//! # Architecture
//!
//! Here we provide the **runtime foundation** for generated stubs
//! and skeletons:
//!
//! * The [`FunctionStub`] trait for client-side proxies (each generated
//!   stub class implements it).
//! * The [`FunctionSkeleton`] trait for service-side dispatch — calls the
//!   right operation from the `request_data` union discriminator
//!   and returns the reply.
//! * The [`dispatch_request`] helper for skeleton implementations.
//!
//! Codegen for the function-call binding lives in
//! `crates/lang-rules/src/emit/interface_cpp.rs`,
//! `crates/lang-rules/src/emit/interface_java.rs` and
//! `crates/lang-rules/src/emit/interface_rust.rs`.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use crate::error::{RpcError, RpcResult};

/// Stub trait: every generated client-side proxy implements it.
///
/// The stub encapsulates the request/reply mechanism behind a
/// type-safe method signature (e.g. `fn add(a: i32, b: i32) -> i32`).
pub trait FunctionStub {
    /// Service name from the IDL `interface` name.
    fn service_name(&self) -> &str;
}

/// Skeleton trait: every generated service-side dispatch implements
/// it. The skeleton unpacks the request discriminator, calls the
/// matching operation in the user implementation and packs the reply
/// back as a union.
pub trait FunctionSkeleton {
    /// Service name.
    fn service_name(&self) -> &str;

    /// List of all operations this skeleton accepts.
    /// Each entry is `(operation_name, opcode)`. The opcode is the HASH of
    /// the operation name (§7.5.1.1.6 rule 5a).
    fn operations(&self) -> &[(&'static str, u32)];
}

/// Operation descriptor for codegen.
///
/// For each IDL operation `void op(in t1 x, out t2 y) raises (E)` the
/// codegen produces an [`OperationDescriptor`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationDescriptor {
    /// Method name from IDL.
    pub name: String,
    /// The case label of the operation: the HASH of its unqualified name
    /// (§7.5.1.1.6 rule 5a).
    pub opcode: u32,
    /// `true` for a `oneway` spec — no reply expected.
    pub one_way: bool,
    /// List of `in`/`inout` parameters (order as in IDL).
    pub in_params: Vec<String>,
    /// List of `out`/`inout` parameters; the return value is the last
    /// member the mapped `Out` structure carries (§7.5.1.1.5 rule 3b).
    pub out_params: Vec<String>,
}

/// Service descriptor for codegen — collection of [`OperationDescriptor`]s.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ServiceDescriptor {
    /// IDL interface name.
    pub name: String,
    /// Operations in IDL order.
    pub operations: Vec<OperationDescriptor>,
}

impl ServiceDescriptor {
    /// Constructor.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            operations: Vec::new(),
        }
    }

    /// Registers an operation. The opcode is the HASH of the operation
    /// name (§7.5.1.1.6 rule 5a), like the case label of the call union.
    ///
    /// # Errors
    /// Currently infallible; returns the registered descriptor.
    pub fn add_operation(
        &mut self,
        name: impl Into<String>,
        one_way: bool,
        in_params: Vec<String>,
        out_params: Vec<String>,
    ) -> RpcResult<&OperationDescriptor> {
        let name: String = name.into();
        let opcode = crate::rpc_hash::rpc_member_hash(&name);
        self.operations.push(OperationDescriptor {
            name,
            opcode,
            one_way,
            in_params,
            out_params,
        });
        self.operations
            .last()
            .ok_or_else(|| RpcError::Codec("ServiceDescriptor: push failed".into()))
    }

    /// Lookup by name.
    #[must_use]
    pub fn operation(&self, name: &str) -> Option<&OperationDescriptor> {
        self.operations.iter().find(|o| o.name == name)
    }

    /// Lookup by opcode.
    #[must_use]
    pub fn operation_by_opcode(&self, opcode: u32) -> Option<&OperationDescriptor> {
        self.operations.iter().find(|o| o.opcode == opcode)
    }
}

/// Dispatcher helper for skeleton implementations.
///
/// Reads the opcode from the request discriminator, calls the
/// matching handler closure and returns the encoded reply.
///
/// # Errors
/// `OperationNotFound` if the opcode is not present in the service.
pub fn dispatch_request<F, T>(service: &ServiceDescriptor, opcode: u32, handler: F) -> RpcResult<T>
where
    F: FnOnce(&OperationDescriptor) -> RpcResult<T>,
{
    let op = service
        .operation_by_opcode(opcode)
        .ok_or_else(|| RpcError::Codec("function-call: unknown opcode".into()))?;
    handler(op)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn calculator_service() -> ServiceDescriptor {
        let mut s = ServiceDescriptor::new("Calculator");
        s.add_operation(
            "add",
            false,
            alloc::vec!["a".into(), "b".into()],
            alloc::vec!["result".into()],
        )
        .expect("add");
        s.add_operation(
            "subtract",
            false,
            alloc::vec!["a".into(), "b".into()],
            alloc::vec!["result".into()],
        )
        .expect("subtract");
        s.add_operation("ping", true, alloc::vec![], alloc::vec![])
            .expect("ping");
        s
    }

    #[test]
    fn service_descriptor_assigns_the_operation_hash() {
        let s = calculator_service();
        assert_eq!(
            s.operations[0].opcode,
            crate::rpc_hash::rpc_member_hash("add")
        );
        assert_eq!(
            s.operations[1].opcode,
            crate::rpc_hash::rpc_member_hash("subtract")
        );
        assert_eq!(
            s.operations[2].opcode,
            crate::rpc_hash::rpc_member_hash("ping")
        );
    }

    #[test]
    fn service_descriptor_lookup_by_name() {
        let s = calculator_service();
        assert_eq!(
            s.operation("add").map(|o| o.opcode),
            Some(crate::rpc_hash::rpc_member_hash("add"))
        );
        assert_eq!(
            s.operation("subtract").map(|o| o.opcode),
            Some(crate::rpc_hash::rpc_member_hash("subtract"))
        );
        assert!(s.operation("nonexistent").is_none());
    }

    #[test]
    fn service_descriptor_lookup_by_opcode() {
        let s = calculator_service();
        let add = crate::rpc_hash::rpc_member_hash("add");
        assert_eq!(
            s.operation_by_opcode(add).map(|o| o.name.as_str()),
            Some("add")
        );
        assert!(s.operation_by_opcode(add.wrapping_add(1)).is_none());
    }

    #[test]
    fn one_way_operation_marked_correctly() {
        let s = calculator_service();
        let ping = s.operation("ping").expect("ping");
        assert!(ping.one_way);
        let add = s.operation("add").expect("add");
        assert!(!add.one_way);
    }

    #[test]
    fn dispatch_request_routes_by_opcode() {
        let s = calculator_service();
        let add = crate::rpc_hash::rpc_member_hash("add");
        let result = dispatch_request(&s, add, |op| Ok::<String, RpcError>(op.name.clone()))
            .expect("dispatch");
        assert_eq!(result, "add");
    }

    #[test]
    fn dispatch_request_unknown_opcode_returns_codec_error() {
        let s = calculator_service();
        let err = dispatch_request(&s, 99, |_op| Ok::<(), RpcError>(())).expect_err("unknown");
        assert!(matches!(err, RpcError::Codec(_)));
    }

    #[test]
    fn out_params_first_member_is_return_value() {
        // §7.5.1.1.5 rule 3b: the return value is the last member the
        // mapped `Out` structure carries; `out_params` here lists the
        // declared out/inout parameters.
        let s = calculator_service();
        let add = s.operation("add").expect("add");
        assert_eq!(add.out_params.first().map(String::as_str), Some("result"));
    }

    /// Test stub as an example codegen output.
    struct CalculatorStub {
        service_name: String,
    }
    impl FunctionStub for CalculatorStub {
        fn service_name(&self) -> &str {
            &self.service_name
        }
    }

    /// Test skeleton as an example codegen output.
    struct CalculatorSkeleton;
    impl FunctionSkeleton for CalculatorSkeleton {
        fn service_name(&self) -> &str {
            "Calculator"
        }
        fn operations(&self) -> &[(&'static str, u32)] {
            // The HASH of each operation name (§7.5.1.1.6 rule 5a).
            &[
                ("add", 0xfc78_ec34),
                ("subtract", 0xc115_f497),
                ("ping", 0x011f_91df),
            ]
        }
    }

    #[test]
    fn stub_and_skeleton_traits_are_object_safe() {
        let stub: alloc::boxed::Box<dyn FunctionStub> = alloc::boxed::Box::new(CalculatorStub {
            service_name: "Calc".into(),
        });
        let skel: alloc::boxed::Box<dyn FunctionSkeleton> =
            alloc::boxed::Box::new(CalculatorSkeleton);
        assert_eq!(stub.service_name(), "Calc");
        assert_eq!(skel.service_name(), "Calculator");
        assert_eq!(skel.operations().len(), 3);
    }
}
