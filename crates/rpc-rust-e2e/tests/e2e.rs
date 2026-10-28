// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The generated Rust DDS-RPC 1.0 binding, run over a real
//! `DcpsRuntime` on loopback — client and service, real UDP, real
//! discovery.
//!
//! macOS loopback UDP `connect` is known to time out on this host
//! family (see the project's own notes on the subject); a failure here
//! that is only a discovery timeout on macOS is not a defect in the
//! generated binding. Run on Linux for the measurement that counts.
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stdout
)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use zerodds_dcps::factory::DomainParticipantFactory;
use zerodds_dcps::participant::DomainParticipant;
use zerodds_dcps::qos::DomainParticipantQos;
use zerodds_rpc::qos_profile::RpcQos;
use zerodds_rpc::replier::Replier;

use zerodds_rpc_rust_e2e::calculator::demo::{
    Calculator, Calculator_Call, Calculator_Return, Calculator_add_In, CalculatorClient,
    CalculatorDispatcher, CalculatorError, DivByZero,
};
use zerodds_rpc_rust_e2e::hierarchy_bindings::hierarchy::{
    Base, BaseDispatcher, Derived, DerivedClient, DerivedDispatcher,
};

fn participant(domain: i32) -> DomainParticipant {
    DomainParticipantFactory::instance()
        .create_participant(domain, DomainParticipantQos::default())
        .expect("a real participant on a leased domain")
}

/// Ticks `replier` in the background until dropped.
struct TickingReplier<
    TIn: zerodds_dcps::dds_type::DdsType + Send + 'static,
    TOut: zerodds_dcps::dds_type::DdsType + Send + 'static,
> {
    stop: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
    _phantom: core::marker::PhantomData<(TIn, TOut)>,
}

impl<
    TIn: zerodds_dcps::dds_type::DdsType + Send + 'static,
    TOut: zerodds_dcps::dds_type::DdsType + Send + 'static,
> TickingReplier<TIn, TOut>
{
    fn spawn(replier: Replier<TIn, TOut>) -> Self {
        let replier = Arc::new(replier);
        let stop = Arc::new(AtomicBool::new(false));
        let replier_for_thread = Arc::clone(&replier);
        let stop_for_thread = Arc::clone(&stop);
        let handle = thread::spawn(move || {
            while !stop_for_thread.load(Ordering::Acquire) {
                let _ = replier_for_thread.tick();
                thread::sleep(Duration::from_millis(2));
            }
        });
        Self {
            stop,
            handle: Some(handle),
            _phantom: core::marker::PhantomData,
        }
    }
}

impl<
    TIn: zerodds_dcps::dds_type::DdsType + Send + 'static,
    TOut: zerodds_dcps::dds_type::DdsType + Send + 'static,
> Drop for TickingReplier<TIn, TOut>
{
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// The `Calculator` implementation under test: `memory` is real,
/// mutable state, so `get_attribute_memory`/`set_attribute_memory`/`reset`/`swap` have
/// something to observe.
struct CalculatorImpl {
    memory: Mutex<i32>,
}

impl Calculator for CalculatorImpl {
    fn add(&self, a: i32, b: i32) -> Result<i32, CalculatorError> {
        Ok(a + b)
    }

    fn div(&self, a: f64, b: f64) -> Result<f64, CalculatorError> {
        if b == 0.0 {
            return Err(CalculatorError::DivByZero(DivByZero {
                msg: "division by zero".to_string(),
            }));
        }
        Ok(a / b)
    }

    fn reset(&self) -> Result<(), CalculatorError> {
        *self.memory.lock().expect("memory lock") = 0;
        Ok(())
    }

    fn swap(&self, v: &mut i32) -> Result<(), CalculatorError> {
        let mut memory = self.memory.lock().expect("memory lock");
        std::mem::swap(&mut *memory, v);
        Ok(())
    }

    fn get_attribute_memory(&self) -> Result<i32, CalculatorError> {
        Ok(*self.memory.lock().expect("memory lock"))
    }

    fn set_attribute_memory(&self, memory: i32) -> Result<(), CalculatorError> {
        *self.memory.lock().expect("memory lock") = memory;
        Ok(())
    }
}

/// Every construct DDS-RPC 1.0 §7.5.1.1 names, checked against the
/// generated Rust binding: return values, `raises`, a `void` operation, an
/// attribute's getter/setter, an `inout` parameter, and an unrecognised
/// discriminator.
#[test]
fn the_generated_calculator_binding_runs_every_call() {
    let lease = zerodds_test_domains::lease_domain();
    let participant = participant(lease.id());
    let qos = RpcQos::default_basic();

    let dispatcher = CalculatorDispatcher::new(CalculatorImpl {
        memory: Mutex::new(0),
    });
    let replier = dispatcher.serve(&participant, &qos).expect("a replier");
    let _ticking = TickingReplier::spawn(replier);

    let client = CalculatorClient::new(&participant, &qos).expect("a client");

    // -- return value -------------------------------------------------
    assert_eq!(client.add(2, 3).expect("add succeeds"), 5);

    // -- raises: the success path first, so the failure path is not
    // measured against a service that never worked -------------------
    assert!((client.div(9.0, 3.0).expect("div succeeds") - 3.0).abs() < f64::EPSILON);

    // -- raises: the declared exception, typed, not a system code -----
    match client.div(1.0, 0.0) {
        Err(CalculatorError::DivByZero(exc)) => {
            assert_eq!(exc.msg, "division by zero");
        }
        other => panic!("expected DivByZero, got {other:?}"),
    }

    // -- attribute get/set — the implied operations of §7.5.1.1.3 -----
    client
        .set_attribute_memory(42)
        .expect("set_attribute_memory succeeds");
    assert_eq!(
        client
            .get_attribute_memory()
            .expect("get_attribute_memory succeeds"),
        42
    );

    // -- inout: the initial value travels in, the service's own value
    // comes back, and the caller's local is what the service wrote ---
    let mut v = 7;
    client.swap(&mut v).expect("swap succeeds");
    assert_eq!(v, 42, "swap hands back the service's own memory");
    assert_eq!(
        client
            .get_attribute_memory()
            .expect("get_attribute_memory succeeds"),
        7,
        "swap wrote the caller's value into the service's memory"
    );

    // -- a void operation: the reply carries the empty Out structure ---
    client
        .set_attribute_memory(9)
        .expect("set_attribute_memory succeeds");
    client.reset().expect("reset() is answered");
    // The reply came after the service ran `reset`; the loop only
    // tolerates a slow scheduler.
    let mut cleared = false;
    for _ in 0..50 {
        if client
            .get_attribute_memory()
            .expect("get_attribute_memory succeeds")
            == 0
        {
            cleared = true;
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }
    assert!(
        cleared,
        "reset() did not clear memory within the wait budget"
    );

    // -- an unrecognised discriminator: DDS-RPC 1.0 §7.5.1.1.6 rule 3,
    // decoded as `UnknownOp` and answered `REMOTE_EX_UNSUPPORTED` —
    // §7.7.1.1: "If the client uses an interface with more operations
    // than that of the service, invocation of an unsupported operation
    // shall result in REMOTE_EX_UNSUPPORTED remote exception code."
    // Built directly against the raw `Requester`, bypassing the typed
    // client — no typed method can construct this case, by design.
    // It declares the same topic types as the generated client
    // (`demo::Calculator_Request`/`_Reply`, §7.5.1.1.6/7): a topic of one
    // name has one type.
    let raw =
        zerodds_rpc::requester::Requester::<Calculator_Call, Calculator_Return>::with_options(
            &participant,
            "demo_Calculator",
            "",
            &qos,
            zerodds_rpc::topic_naming::BindingStyle::FunctionCall,
            &zerodds_rpc::topic_naming::AnnotatedTopics::none(),
            &zerodds_rpc::endpoint_options::RpcEndpointOptions::for_interface("demo", "Calculator"),
        )
        .expect("a second, anonymous requester on the same topic");
    let outcome = raw.send_request_blocking(&Calculator_Call::UnknownOp(0), None);
    match outcome {
        Err(zerodds_rpc::error::RpcError::RemoteException(code)) => {
            assert_eq!(
                code,
                zerodds_rpc::common_types::RemoteExceptionCode::Unsupported.as_u32()
            );
        }
        other => panic!("expected RemoteException(Unsupported), got {other:?}"),
    }
}

/// A live requester names the identity of its own request sample: the RTPS
/// GUID of the request writer and the sequence number the writer is about to
/// stamp (DDS-RPC 1.0 §7.2.1, §7.8.1). Without that a peer that answers with
/// the sample identity of the request would correlate against nothing.
#[test]
fn a_live_request_identity_is_the_rtps_identity_of_the_request_writer() {
    let lease = zerodds_test_domains::lease_domain();
    let participant = participant(lease.id());
    let qos = RpcQos::default_basic();

    let raw = zerodds_rpc::requester::Requester::<Calculator_Call, Calculator_Return>::new(
        &participant,
        "demo_Calculator",
        &qos,
    )
    .expect("a requester on the demo topic");

    let (id, _rx) = raw
        .send_request_async(&Calculator_Call::UnknownOp(0))
        .expect("the request is written");

    assert_eq!(
        raw.__writer_rtps_guid(),
        Some(id.writer_guid),
        "the request id carries the GUID of the request writer, not a synthesized one"
    );
    assert_eq!(
        id.sequence_number, 1,
        "the request id carries the writer's first RTPS sequence number"
    );
}

/// A `Derived : Base` hierarchy: the derived client reaches the base's
/// own operation through the composed base client field, and each
/// level is served by its own dispatcher over its own topic — DDS-RPC
/// 1.0 §7.5.1.1.8.
struct HierarchyImpl;

impl Base for HierarchyImpl {
    fn ping(
        &self,
        x: i32,
    ) -> Result<i32, zerodds_rpc_rust_e2e::hierarchy_bindings::hierarchy::BaseError> {
        Ok(x)
    }
}

impl Derived for HierarchyImpl {
    fn twice(
        &self,
        x: i32,
    ) -> Result<i32, zerodds_rpc_rust_e2e::hierarchy_bindings::hierarchy::DerivedError> {
        Ok(x * 2)
    }
}

#[test]
fn a_derived_client_reaches_an_inherited_operation_through_its_base_field() {
    let lease = zerodds_test_domains::lease_domain();
    let participant = participant(lease.id());
    let qos = RpcQos::default_basic();

    // Both levels are independently servable — DDS-RPC 1.0 keeps a
    // base's topic separate from a derived interface's own
    // (§7.5.1.1.8) — and one implementation answers both, since
    // `Derived: Base` is a supertrait bound.
    let base_replier = BaseDispatcher::new(HierarchyImpl)
        .serve(&participant, &qos)
        .expect("a base replier");
    let _ticking_base = TickingReplier::spawn(base_replier);
    let derived_replier = DerivedDispatcher::new(HierarchyImpl)
        .serve(&participant, &qos)
        .expect("a derived replier");
    let _ticking_derived = TickingReplier::spawn(derived_replier);

    let client = DerivedClient::new(&participant, &qos).expect("a derived client");

    assert_eq!(
        client.twice(21).expect("twice succeeds"),
        42,
        "the derived interface's own operation"
    );
    assert_eq!(
        client.base_base.ping(5).expect("ping succeeds"),
        5,
        "the inherited operation, through the composed base client"
    );
}

/// A service that answers every operation with `DivByZero`, which only
/// `div` declares.
struct Misbehaving;

impl Calculator for Misbehaving {
    fn add(&self, _a: i32, _b: i32) -> Result<i32, CalculatorError> {
        Err(CalculatorError::DivByZero(DivByZero {
            msg: "not declared by add".to_string(),
        }))
    }

    fn div(&self, _a: f64, _b: f64) -> Result<f64, CalculatorError> {
        Ok(0.0)
    }

    fn reset(&self) -> Result<(), CalculatorError> {
        Ok(())
    }

    fn swap(&self, _v: &mut i32) -> Result<(), CalculatorError> {
        Ok(())
    }

    fn get_attribute_memory(&self) -> Result<i32, CalculatorError> {
        Ok(0)
    }

    fn set_attribute_memory(&self, _memory: i32) -> Result<(), CalculatorError> {
        Ok(())
    }
}

/// §7.5.2: an exception the operation does not declare is
/// `REMOTE_EX_UNKNOWN_EXCEPTION`. `DivByZero` is a member of the service's
/// error type but `add` does not raise it, so the dispatcher answers the
/// code instead of a `Result` case the client cannot read.
#[test]
fn an_exception_the_operation_does_not_declare_is_unknown_exception() {
    use zerodds_rpc::common_types::RemoteExceptionCode;
    use zerodds_rpc::replier::ReplierHandler;
    let dispatcher = CalculatorDispatcher::new(Misbehaving);
    assert_eq!(
        dispatcher.handle(Calculator_Call::Add(Calculator_add_In { a: 1, b: 2 })),
        Err(RemoteExceptionCode::UnknownException)
    );
}
