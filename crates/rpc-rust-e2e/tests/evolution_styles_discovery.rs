// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-RPC 1.0 over a real `DcpsRuntime`: a function-call client against a
//! request-reply service (§7.2.5), the discovery data the Rust runtime
//! announces (§7.6.2.1), and two generated versions of one interface
//! talking to each other (§7.7.2).
//!
//! Requester and replier run on one participant: the samples go through the
//! runtime's own delivery path, as in `enhanced_e2e.rs`.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use zerodds_dcps::dds_type::{DdsType, RawBytes};
use zerodds_dcps::factory::DomainParticipantFactory;
use zerodds_dcps::participant::DomainParticipant;
use zerodds_dcps::qos::DomainParticipantQos;
use zerodds_dcps::runtime::RpcDiscoveryExt;
use zerodds_rpc::common_types::RemoteExceptionCode;
use zerodds_rpc::endpoint_options::{RpcEndpointOptions, RpcMapping};
use zerodds_rpc::error::RpcError;
use zerodds_rpc::qos_profile::RpcQos;
use zerodds_rpc::replier::{FnHandler, Replier, ReplierHandler};
use zerodds_rpc::requester::Requester;
use zerodds_rpc::topic_naming::{AnnotatedTopics, BindingStyle};

fn participant(domain: i32) -> DomainParticipant {
    DomainParticipantFactory::instance()
        .create_participant(domain, DomainParticipantQos::default())
        .expect("a real participant on a leased domain")
}

/// Ticks `replier` in the background until dropped.
struct Ticking {
    stop: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Ticking {
    fn spawn<TIn: DdsType + Send + 'static, TOut: DdsType + Send + 'static>(
        replier: Replier<TIn, TOut>,
    ) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let handle = thread::spawn(move || {
            while !flag.load(Ordering::Acquire) {
                let _ = replier.tick();
                thread::sleep(Duration::from_millis(2));
            }
        });
        Self {
            stop,
            handle: Some(handle),
        }
    }
}

impl Drop for Ticking {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn eventually(cond: impl Fn() -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if cond() {
            return true;
        }
        thread::sleep(Duration::from_millis(20));
    }
    cond()
}

/// The topic names a function-call client of `qualified` computes by
/// itself, written out as the two whole names a request-reply service
/// states in `AnnotatedTopics`: "when <service_name> and user-defined topic
/// names are both provided by the user, the user-defined topic names take
/// precedence" (§7.4.1).
fn explicit_topics(qualified: &str) -> AnnotatedTopics {
    AnnotatedTopics {
        request_topic: Some(format!("{qualified}_Service_Request")),
        reply_topic: Some(format!("{qualified}_Service_Reply")),
        ..AnnotatedTopics::none()
    }
}

// -- §7.2.5: a function-call client against a request-reply service ------

/// §7.2.5: "The Basic and Enhanced Service mappings are independent of the
/// language binding style. For instance, it is possible for clients and
/// service implementations to communicate using different language binding
/// styles as long as their service mappings match." Basic mapping: the
/// generated function-call client calls a service that was built in the
/// request-reply style on the topic and type names the client uses.
#[test]
fn a_basic_function_call_client_calls_a_request_reply_service() {
    use zerodds_rpc_rust_e2e::calculator::demo::{
        Calculator_Call, Calculator_Return, Calculator_add_Out, Calculator_add_Result,
        CalculatorClient,
    };
    let lease = zerodds_test_domains::lease_domain();
    let p = participant(lease.id());
    let qos = RpcQos::default_basic();

    let handler: Arc<dyn ReplierHandler<Calculator_Call, Calculator_Return>> =
        Arc::new(FnHandler::new(|call: Calculator_Call| match call {
            Calculator_Call::Add(input) => Ok(Calculator_Return::Add(Calculator_add_Result::Ok(
                Calculator_add_Out {
                    return_: input.a + input.b,
                },
            ))),
            _ => Err(RemoteExceptionCode::Unsupported),
        }));
    let service = Replier::<Calculator_Call, Calculator_Return>::with_options(
        &p,
        "demo_Calculator",
        "",
        &qos,
        BindingStyle::RequestReply,
        &explicit_topics("demo_Calculator"),
        &RpcEndpointOptions::for_interface("demo", "Calculator"),
        handler,
    )
    .expect("a request-reply service");
    assert_eq!(
        service.request_topic_name(),
        "demo_Calculator_Service_Request"
    );
    assert_eq!(service.reply_topic_name(), "demo_Calculator_Service_Reply");
    let _service = Ticking::spawn(service);

    let client = CalculatorClient::new(&p, &qos).expect("a function-call client");
    assert_eq!(client.add(2, 3).expect("add"), 5);
    assert_eq!(client.add(40, 2).expect("add"), 42);
}

/// The same for the Enhanced mapping: no header, the reply found by its
/// related sample identity, and the topic names the two styles compute
/// differently stated whole on the service side.
#[test]
fn an_enhanced_function_call_client_calls_a_request_reply_service() {
    use zerodds_rpc_rust_e2e::enhanced_bindings::enh::{
        Calc_Reply, Calc_Request, Calc_add_Out, Calc_add_Result, CalcClient,
    };
    let lease = zerodds_test_domains::lease_domain();
    let p = participant(lease.id());
    let qos = RpcQos::default_enhanced();

    let handler: Arc<dyn ReplierHandler<Calc_Request, Calc_Reply>> =
        Arc::new(FnHandler::new(|request: Calc_Request| match request {
            Calc_Request::Add(input) => Ok(Calc_Reply::Add(Calc_add_Result::Ok(Calc_add_Out {
                return_: input.a + input.b,
            }))),
            _ => Err(RemoteExceptionCode::UnknownOperation),
        }));
    let service = Replier::<Calc_Request, Calc_Reply>::with_options(
        &p,
        "enh_Calc",
        "",
        &qos,
        BindingStyle::RequestReply,
        &explicit_topics("enh_Calc"),
        &CalcClient::endpoint_options(),
        handler,
    )
    .expect("a request-reply service");
    assert_eq!(service.request_topic_name(), "enh_Calc_Service_Request");
    assert_eq!(service.reply_topic_name(), "enh_Calc_Service_Reply");
    let _service = Ticking::spawn(service);

    let client = CalcClient::new(&p, &qos).expect("a function-call client");
    assert_eq!(client.add(2, 3).expect("add"), 5);
    // An operation the request-reply service does not serve is answered
    // the way a service without it answers.
    match client.ping() {
        Err(zerodds_rpc_rust_e2e::enhanced_bindings::enh::CalcError::Remote(
            RpcError::RemoteException(code),
        )) => assert_eq!(code, RemoteExceptionCode::UnknownOperation.as_u32()),
        other => panic!("expected REMOTE_EX_UNKNOWN_OPERATION, got {other:?}"),
    }
}

// -- §7.6.2.1: what the Rust runtime announces ----------------------------

/// §7.6.2.1.2: the service-side endpoints carry the instance name the user
/// gave, the aliases of §7.4.3 and each other's GUID; the client-side
/// endpoints carry no instance name ("the client-side DataReader shall not
/// include service_instance_name"), the aliases and each other's GUID. The
/// data is read from the matched peers through `matched_rpc_readers` and
/// `matched_rpc_writers`, the way a remote participant reads it from SEDP.
#[test]
fn the_runtime_announces_aliases_the_instance_name_and_the_related_keys() {
    let lease = zerodds_test_domains::lease_domain();
    let p = participant(lease.id());
    let qos = RpcQos::default_enhanced();
    let options = RpcEndpointOptions::for_interface("robot", "Calc")
        .with_parent("robot_Base")
        .with_mapping(RpcMapping::Enhanced);
    let handler: Arc<dyn ReplierHandler<RawBytes, RawBytes>> =
        Arc::new(FnHandler::new(|request: RawBytes| {
            Ok::<_, RemoteExceptionCode>(request)
        }));
    let service = Replier::<RawBytes, RawBytes>::with_options(
        &p,
        "robot_Calc",
        "calc-A",
        &qos,
        BindingStyle::FunctionCall,
        &AnnotatedTopics::none(),
        &options,
        handler,
    )
    .expect("a replier");
    let client = Requester::<RawBytes, RawBytes>::with_options(
        &p,
        "robot_Calc",
        "calc-A",
        &qos,
        BindingStyle::FunctionCall,
        &AnnotatedTopics::none(),
        &options,
    )
    .expect("a requester");

    let request_reader = service.__request_reader().rtps_guid().expect("live");
    let reply_writer = service.__reply_writer().rtps_guid().expect("live");
    let request_writer = client.__request_writer().rtps_guid().expect("live");
    let reply_reader = client.__reply_reader().rtps_guid().expect("live");

    // The client sees the service's endpoints.
    let service_request_reader = RpcDiscoveryExt {
        service_instance_name: Some("calc-A".to_string()),
        related_entity_guid: Some(reply_writer),
        topic_aliases: Some(vec!["robot_Base_Service_Request".to_string()]),
    };
    assert!(
        eventually(|| {
            client
                .__request_writer()
                .matched_rpc_readers()
                .iter()
                .any(|peer| peer.key == request_reader && peer.ext == service_request_reader)
        }),
        "the request writer's peer: {:?}",
        client.__request_writer().matched_rpc_readers()
    );
    let service_reply_writer = RpcDiscoveryExt {
        service_instance_name: Some("calc-A".to_string()),
        related_entity_guid: Some(request_reader),
        topic_aliases: Some(vec!["robot_Base_Service_Reply".to_string()]),
    };
    assert!(
        eventually(|| {
            client
                .__reply_reader()
                .matched_rpc_writers()
                .iter()
                .any(|peer| peer.key == reply_writer && peer.ext == service_reply_writer)
        }),
        "the reply reader's peer: {:?}",
        client.__reply_reader().matched_rpc_writers()
    );

    // The service sees the client's endpoints: no instance name.
    let client_request_writer = RpcDiscoveryExt {
        service_instance_name: None,
        related_entity_guid: Some(reply_reader),
        topic_aliases: Some(vec!["robot_Base_Service_Request".to_string()]),
    };
    assert!(
        eventually(|| {
            service
                .__request_reader()
                .matched_rpc_writers()
                .iter()
                .any(|peer| peer.key == request_writer && peer.ext == client_request_writer)
        }),
        "the request reader's peer: {:?}",
        service.__request_reader().matched_rpc_writers()
    );
    let client_reply_reader = RpcDiscoveryExt {
        service_instance_name: None,
        related_entity_guid: Some(request_writer),
        topic_aliases: Some(vec!["robot_Base_Service_Reply".to_string()]),
    };
    assert!(
        eventually(|| {
            service
                .__reply_writer()
                .matched_rpc_readers()
                .iter()
                .any(|peer| peer.key == reply_reader && peer.ext == client_reply_reader)
        }),
        "the reply writer's peer: {:?}",
        service.__reply_writer().matched_rpc_readers()
    );
}

/// "If none is specified, there is no default value" (§7.6.2.1.2): a
/// service without an instance name and an interface without parents
/// announce neither, and still name each other.
#[test]
fn a_service_without_an_instance_name_or_parents_announces_neither() {
    let lease = zerodds_test_domains::lease_domain();
    let p = participant(lease.id());
    let qos = RpcQos::default_enhanced();
    let options =
        RpcEndpointOptions::for_interface("robot", "Plain").with_mapping(RpcMapping::Enhanced);
    let handler: Arc<dyn ReplierHandler<RawBytes, RawBytes>> =
        Arc::new(FnHandler::new(|request: RawBytes| {
            Ok::<_, RemoteExceptionCode>(request)
        }));
    let service = Replier::<RawBytes, RawBytes>::with_options(
        &p,
        "robot_Plain",
        "",
        &qos,
        BindingStyle::FunctionCall,
        &AnnotatedTopics::none(),
        &options,
        handler,
    )
    .expect("a replier");
    let client = Requester::<RawBytes, RawBytes>::with_options(
        &p,
        "robot_Plain",
        "",
        &qos,
        BindingStyle::FunctionCall,
        &AnnotatedTopics::none(),
        &options,
    )
    .expect("a requester");
    let request_reader = service.__request_reader().rtps_guid().expect("live");
    let reply_writer = service.__reply_writer().rtps_guid().expect("live");
    let expected = RpcDiscoveryExt {
        service_instance_name: None,
        related_entity_guid: Some(reply_writer),
        topic_aliases: None,
    };
    assert!(
        eventually(|| {
            client
                .__request_writer()
                .matched_rpc_readers()
                .iter()
                .any(|peer| peer.key == request_reader && peer.ext == expected)
        }),
        "{:?}",
        client.__request_writer().matched_rpc_readers()
    );
}

// -- §7.7.2: two generated versions of one interface ---------------------

mod versions {
    use super::*;
    use zerodds_rpc_rust_e2e::evolution_new::evo as new;
    use zerodds_rpc_rust_e2e::evolution_old::evo as old;

    struct OldService;

    impl old::Svc for OldService {
        fn add(&self, a: i32, b: i32) -> Result<i32, old::SvcError> {
            Ok(a + b)
        }
        fn scale(&self, k: i32) -> Result<i32, old::SvcError> {
            if k > 100 {
                return Err(old::SvcError::Overflow(old::Overflow { limit: 100 }));
            }
            Ok(k * 2)
        }
        fn mix(&self, x: i32, y: i32) -> Result<i32, old::SvcError> {
            Ok(x * 10 + y)
        }
        fn legacy(&self) -> Result<i32, old::SvcError> {
            Ok(7)
        }
    }

    struct NewService;

    impl new::Svc for NewService {
        fn add(&self, a: i32, b: i32) -> Result<i32, new::SvcError> {
            Ok(a + b)
        }
        fn scale(&self, k: i32) -> Result<i32, new::SvcError> {
            if k > 100 {
                return Err(new::SvcError::Overflow(new::Overflow { limit: 100 }));
            }
            if k < 0 {
                return Err(new::SvcError::Underflow(new::Underflow { limit: 0 }));
            }
            Ok(k * 2)
        }
        // The parameters are declared in the other order than in the old
        // version; the members are found by their names.
        fn mix(&self, y: i32, x: i32) -> Result<i32, new::SvcError> {
            Ok(x * 10 + y)
        }
        fn extra(&self) -> Result<i32, new::SvcError> {
            Ok(9)
        }
    }

    fn unknown_operation(error: &RpcError) -> bool {
        *error == RpcError::RemoteException(RemoteExceptionCode::UnknownOperation.as_u32())
    }

    /// §7.7.2.1 and §7.7.2.2: a client of the old version calls the service
    /// of the new version. The operations both have are served; the one
    /// the new version removed is answered `REMOTE_EX_UNKNOWN_OPERATION`.
    /// §7.7.2.5.2: the parameters of the `@Mutable` operation `mix` were
    /// reordered and still arrive under their names.
    #[test]
    fn an_old_client_calls_a_new_service() {
        let lease = zerodds_test_domains::lease_domain();
        // Two participants: a topic of one participant has one set of type
        // metadata, and the two versions describe their types differently.
        let p = participant(lease.id());
        let client_side = participant(lease.id());
        let qos = RpcQos::default_enhanced();
        let service = new::SvcDispatcher::new(NewService)
            .serve(&p, &qos)
            .expect("the new service");
        let _service = Ticking::spawn(service);

        let client = old::SvcClient::new(&client_side, &qos).expect("the old client");
        assert_eq!(client.add(2, 3).expect("add"), 5);
        assert_eq!(client.scale(21).expect("scale"), 42);
        // `mix(x = 2, y = 3)` of the old version is `mix(y = 3, x = 2)` of
        // the new one.
        assert_eq!(client.mix(2, 3).expect("mix"), 23);
        match client.legacy() {
            Err(old::SvcError::Remote(error)) => assert!(unknown_operation(&error), "{error:?}"),
            other => panic!("expected REMOTE_EX_UNKNOWN_OPERATION, got {other:?}"),
        }
        // An exception both versions declare stays an exception.
        match client.scale(1000) {
            Err(old::SvcError::Overflow(exception)) => assert_eq!(exception.limit, 100),
            other => panic!("expected Overflow, got {other:?}"),
        }
        // §7.7.2.5.6: the new service raises an exception the old client
        // does not declare; the client reports REMOTE_EX_UNKNOWN_EXCEPTION.
        match client.scale(-1) {
            Err(old::SvcError::Remote(RpcError::RemoteException(code))) => {
                assert_eq!(code, RemoteExceptionCode::UnknownException.as_u32());
            }
            other => panic!("expected REMOTE_EX_UNKNOWN_EXCEPTION, got {other:?}"),
        }
    }

    /// The other direction: a client of the new version calls the service
    /// of the old version. The operation the new version added is
    /// answered `REMOTE_EX_UNKNOWN_OPERATION` (§7.7.2.1), the reordered
    /// parameters arrive under their names, and the exception the old
    /// version lacks is no problem for a client that knows more.
    #[test]
    fn a_new_client_calls_an_old_service() {
        let lease = zerodds_test_domains::lease_domain();
        let p = participant(lease.id());
        let client_side = participant(lease.id());
        let qos = RpcQos::default_enhanced();
        let service = old::SvcDispatcher::new(OldService)
            .serve(&p, &qos)
            .expect("the old service");
        let _service = Ticking::spawn(service);

        let client = new::SvcClient::new(&client_side, &qos).expect("the new client");
        assert_eq!(client.add(2, 3).expect("add"), 5);
        assert_eq!(client.scale(21).expect("scale"), 42);
        // `mix(y = 3, x = 2)` of the new version is `mix(x = 2, y = 3)` of
        // the old one.
        assert_eq!(client.mix(3, 2).expect("mix"), 23);
        match client.extra() {
            Err(new::SvcError::Remote(error)) => assert!(unknown_operation(&error), "{error:?}"),
            other => panic!("expected REMOTE_EX_UNKNOWN_OPERATION, got {other:?}"),
        }
        match client.scale(1000) {
            Err(new::SvcError::Overflow(exception)) => assert_eq!(exception.limit, 100),
            other => panic!("expected Overflow, got {other:?}"),
        }
    }
}

// -- §7.7.1: three generated Basic versions of one interface ---------------

mod basic_versions {
    use super::*;
    use zerodds_rpc_rust_e2e::basic_fewer::evo as fewer;
    use zerodds_rpc_rust_e2e::basic_new::evo as new;
    use zerodds_rpc_rust_e2e::basic_old::evo as old;

    struct OldService;

    impl old::Svc for OldService {
        fn add(&self, a: i32, b: i32) -> Result<i32, old::SvcError> {
            Ok(a + b)
        }
        fn legacy(&self) -> Result<i32, old::SvcError> {
            Ok(7)
        }
    }

    struct NewService;

    impl new::Svc for NewService {
        fn extra(&self) -> Result<i32, new::SvcError> {
            Ok(9)
        }
        fn add(&self, a: i32, b: i32) -> Result<i32, new::SvcError> {
            Ok(a + b)
        }
    }

    fn unsupported(error: &RpcError) -> bool {
        *error == RpcError::RemoteException(RemoteExceptionCode::Unsupported.as_u32())
    }

    /// §7.7.1.1: "If a client uses an interface that has fewer methods than
    /// that of the service, it is simply oblivious of the fact that the
    /// service has more capabilities than the client can use." The client
    /// with one operation calls the service with two.
    #[test]
    fn a_client_with_fewer_operations_calls_a_service_with_more() {
        let lease = zerodds_test_domains::lease_domain();
        let p = participant(lease.id());
        let qos = RpcQos::default_basic();
        let service = new::SvcDispatcher::new(NewService)
            .serve(&p, &qos)
            .expect("the new service");
        let _service = Ticking::spawn(service);

        let client = fewer::SvcClient::new(&p, &qos).expect("the client with fewer operations");
        assert_eq!(client.add(2, 3).expect("add"), 5);
        assert_eq!(client.add(40, 2).expect("add"), 42);
    }

    /// §7.7.1.1 and §7.7.1.2: a client of the old version calls the service
    /// of the new version, whose operations are listed in another order.
    /// The operation both have is served; the one the new version removed
    /// is answered `REMOTE_EX_UNSUPPORTED`.
    #[test]
    fn an_old_basic_client_calls_a_new_service() {
        let lease = zerodds_test_domains::lease_domain();
        let p = participant(lease.id());
        let qos = RpcQos::default_basic();
        let service = new::SvcDispatcher::new(NewService)
            .serve(&p, &qos)
            .expect("the new service");
        let _service = Ticking::spawn(service);

        let client = old::SvcClient::new(&p, &qos).expect("the old client");
        assert_eq!(client.add(2, 3).expect("add"), 5);
        match client.legacy() {
            Err(old::SvcError::Remote(error)) => assert!(unsupported(&error), "{error:?}"),
            other => panic!("expected REMOTE_EX_UNSUPPORTED, got {other:?}"),
        }
    }

    /// §7.7.1.1: a client of the new version calls the service of the old
    /// version. The operation the new version added is answered
    /// `REMOTE_EX_UNSUPPORTED`.
    #[test]
    fn a_new_basic_client_calls_an_old_service() {
        let lease = zerodds_test_domains::lease_domain();
        let p = participant(lease.id());
        let qos = RpcQos::default_basic();
        let service = old::SvcDispatcher::new(OldService)
            .serve(&p, &qos)
            .expect("the old service");
        let _service = Ticking::spawn(service);

        let client = new::SvcClient::new(&p, &qos).expect("the new client");
        assert_eq!(client.add(2, 3).expect("add"), 5);
        match client.extra() {
            Err(new::SvcError::Remote(error)) => assert!(unsupported(&error), "{error:?}"),
            other => panic!("expected REMOTE_EX_UNSUPPORTED, got {other:?}"),
        }
    }
}
