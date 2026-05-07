// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Function-Call-Style Service-API (Spec §7.2.2.1, §7.9.2.1, §7.10.1).
//!
//! Das DDS-RPC-Spec definiert zwei Language-Binding-Styles:
//! 1. **Request/Reply-Style** (low-level) — `crates/rpc/src/{requester,
//!    replier}.rs`.
//! 2. **Function-Call-Style** (high-level) — *dieses Modul*.
//!
//! Function-Call-Style verwendet Stubs (Client-Side-Proxy) und
//! Skeletons (Service-Side-Dispatch), die zur Codegen-Zeit aus einer
//! Service-Definition (IDL `interface Foo { void op(); }`) generiert
//! werden. Die Stubs sehen wie native Function-Calls aus, kapseln
//! aber intern den Request/Reply-Pfad.
//!
//! # Architektur
//!
//! Wir liefern hier die **Runtime-Foundation** fuer generierte Stubs
//! und Skeletons:
//!
//! * [`FunctionStub`]-Trait fuer Client-Side-Proxies (jede generierte
//!   Stub-Klasse implementiert es).
//! * [`FunctionSkeleton`]-Trait fuer Service-Side-Dispatch — ruft die
//!   richtige Operation aus dem `request_data`-Union-Discriminator
//!   und liefert die Reply.
//! * [`dispatch_request`]-Helper fuer Skeleton-Implementations.
//!
//! Codegen-Templates leben in `crates/idl-cpp/src/rpc_template.rs`
//! (C++) und `crates/idl-java/src/rpc_template.rs` (Java).

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use crate::error::{RpcError, RpcResult};

/// Stub-Trait: jeder generierte Client-Side-Proxy implementiert das.
///
/// Der Stub kapselt den Request/Reply-Mechanismus hinter einer
/// type-safe Method-Signatur (z.B. `fn add(a: i32, b: i32) -> i32`).
pub trait FunctionStub {
    /// Service-Name aus IDL-`interface`-Namen.
    fn service_name(&self) -> &str;
}

/// Skeleton-Trait: jeder generierte Service-Side-Dispatch implementiert
/// das. Der Skeleton entpackt den Request-Discriminator, ruft die
/// passende Operation in der User-Implementation und packt die Reply
/// als Union zurueck.
pub trait FunctionSkeleton {
    /// Service-Name.
    fn service_name(&self) -> &str;

    /// Liste aller Operations, die dieser Skeleton entgegen nimmt.
    /// Jeder Eintrag ist `(operation_name, opcode)`. Opcodes werden
    /// bei der Codegen automatisch monoton vergeben (Spec §7.2.2.1).
    fn operations(&self) -> &[(&'static str, u32)];
}

/// Operation-Descriptor fuer Codegen.
///
/// Pro IDL-Operation `void op(in t1 x, out t2 y) raises (E)` erzeugt
/// der Codegen einen [`OperationDescriptor`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationDescriptor {
    /// Method-Name aus IDL.
    pub name: String,
    /// Monoton vergebener Opcode.
    pub opcode: u32,
    /// `true` wenn `oneway`-Spec — kein Reply erwartet.
    pub one_way: bool,
    /// Liste der `in`/`inout`-Parameter (Reihenfolge wie in IDL).
    pub in_params: Vec<String>,
    /// Liste der `out`/`inout`-Parameter + Return-Type (Spec
    /// §7.2.4.2 mappt Return zur ersten Member-Position).
    pub out_params: Vec<String>,
}

/// Service-Descriptor fuer Codegen — Sammlung von [`OperationDescriptor`]s.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ServiceDescriptor {
    /// IDL-Interface-Name.
    pub name: String,
    /// Operations in IDL-Reihenfolge.
    pub operations: Vec<OperationDescriptor>,
}

impl ServiceDescriptor {
    /// Konstruktor.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            operations: Vec::new(),
        }
    }

    /// Registriert eine Operation. Opcode wird automatisch vergeben.
    ///
    /// # Errors
    /// `RpcError::Codec` wenn die Operations-Anzahl `u32::MAX`
    /// ueberschreitet.
    pub fn add_operation(
        &mut self,
        name: impl Into<String>,
        one_way: bool,
        in_params: Vec<String>,
        out_params: Vec<String>,
    ) -> RpcResult<&OperationDescriptor> {
        let opcode = u32::try_from(self.operations.len()).map_err(|_| {
            RpcError::Codec("ServiceDescriptor: too many operations (>u32::MAX)".into())
        })?;
        self.operations.push(OperationDescriptor {
            name: name.into(),
            opcode,
            one_way,
            in_params,
            out_params,
        });
        self.operations
            .last()
            .ok_or_else(|| RpcError::Codec("ServiceDescriptor: push failed".into()))
    }

    /// Lookup nach Name.
    #[must_use]
    pub fn operation(&self, name: &str) -> Option<&OperationDescriptor> {
        self.operations.iter().find(|o| o.name == name)
    }

    /// Lookup nach Opcode.
    #[must_use]
    pub fn operation_by_opcode(&self, opcode: u32) -> Option<&OperationDescriptor> {
        self.operations.iter().find(|o| o.opcode == opcode)
    }
}

/// Dispatcher-Helper fuer Skeleton-Implementations.
///
/// Liest den Opcode aus dem Request-Discriminator, ruft den
/// passenden Handler-Closure und liefert die kodierte Reply zurueck.
///
/// # Errors
/// `OperationNotFound` wenn der Opcode nicht im Service vorhanden ist.
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
    fn service_descriptor_assigns_monotonic_opcodes() {
        let s = calculator_service();
        assert_eq!(s.operations[0].opcode, 0);
        assert_eq!(s.operations[1].opcode, 1);
        assert_eq!(s.operations[2].opcode, 2);
    }

    #[test]
    fn service_descriptor_lookup_by_name() {
        let s = calculator_service();
        assert_eq!(s.operation("add").map(|o| o.opcode), Some(0));
        assert_eq!(s.operation("subtract").map(|o| o.opcode), Some(1));
        assert!(s.operation("nonexistent").is_none());
    }

    #[test]
    fn service_descriptor_lookup_by_opcode() {
        let s = calculator_service();
        assert_eq!(
            s.operation_by_opcode(0).map(|o| o.name.as_str()),
            Some("add")
        );
        assert!(s.operation_by_opcode(99).is_none());
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
        let result = dispatch_request(&s, 0, |op| Ok::<String, RpcError>(op.name.clone()))
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
        // Spec §7.2.4.2: Return-Type mapped auf ersten Member von
        // `out_params`.
        let s = calculator_service();
        let add = s.operation("add").expect("add");
        assert_eq!(add.out_params.first().map(String::as_str), Some("result"));
    }

    /// Test-Stub als Beispiel-Codegen-Output.
    struct CalculatorStub {
        service_name: String,
    }
    impl FunctionStub for CalculatorStub {
        fn service_name(&self) -> &str {
            &self.service_name
        }
    }

    /// Test-Skeleton als Beispiel-Codegen-Output.
    struct CalculatorSkeleton;
    impl FunctionSkeleton for CalculatorSkeleton {
        fn service_name(&self) -> &str {
            "Calculator"
        }
        fn operations(&self) -> &[(&'static str, u32)] {
            &[("add", 0), ("subtract", 1), ("ping", 2)]
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
