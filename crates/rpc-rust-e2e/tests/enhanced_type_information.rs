// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Enhanced topics announce the `TypeIdentifier` and register the
//! `TypeObject`s of their types (DDS-RPC 1.0 §7.5.1.2, XTypes 1.3 §7.6.3.3),
//! so two peers match by assignability and not by the type name alone:
//!
//! * §7.4.3: a client of a base interface meets a service of the derived
//!   one, whose request type has another name;
//! * §7.7.2.5: a client of one version of an interface meets a service of
//!   another version when the change is one the clause allows, and does not
//!   when a parameter changed its type (§7.7.2.5.3).
//!
//! Client and service run on two participants of one domain, so the
//! endpoints discover each other through SEDP.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

use zerodds_dcps::dds_type::DdsType;
use zerodds_dcps::factory::DomainParticipantFactory;
use zerodds_dcps::participant::DomainParticipant;
use zerodds_dcps::qos::DomainParticipantQos;
use zerodds_rpc::qos_profile::RpcQos;
use zerodds_rpc::replier::Replier;

fn participant(domain: i32) -> DomainParticipant {
    DomainParticipantFactory::instance()
        .create_participant(domain, DomainParticipantQos::default())
        .expect("a participant on a leased domain")
}

/// Ticks a replier in the background until dropped.
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

mod base_and_derived {
    use super::*;
    use zerodds_rpc_rust_e2e::enhanced_bindings::enh::{
        Base2, Base2Client, Base2Error, Derived2, Derived2Dispatcher, Derived2Error,
    };

    struct Both;
    impl Base2 for Both {
        fn ident(&self) -> Result<i32, Base2Error> {
            Ok(41)
        }
    }
    impl Derived2 for Both {
        fn more(&self) -> Result<i32, Derived2Error> {
            Ok(42)
        }
    }

    /// §7.4.3: the service of `Derived2` is reached by a client of `Base2`
    /// through the topic aliases; the request types are `enh::Base2_Request`
    /// and `enh::Derived2_Request`, which differ in name and match because
    /// the one is assignable to the other.
    #[test]
    fn a_base_client_calls_a_derived_service() {
        let lease = zerodds_test_domains::lease_domain();
        let service_side = participant(lease.id());
        let client_side = participant(lease.id());
        let qos = RpcQos::default_enhanced();
        let service = Derived2Dispatcher::new(Both)
            .serve(&service_side, &qos)
            .expect("the derived service");
        let _service = Ticking::spawn(service);
        let client = Base2Client::new(&client_side, &qos).expect("the base client");
        assert_eq!(client.ident().expect("ident"), 41);
    }
}

mod versions {
    use super::*;
    use zerodds_rpc_rust_e2e::evolution_changed::evo as changed;
    use zerodds_rpc_rust_e2e::evolution_old::evo as old;

    struct ChangedService {
        calls: Arc<AtomicUsize>,
    }

    impl changed::Svc for ChangedService {
        fn add(&self, _a: String, b: i32) -> Result<i32, changed::SvcError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(b)
        }
        fn scale(&self, k: i32) -> Result<i32, changed::SvcError> {
            Ok(k)
        }
        fn mix(&self, x: i32, y: i32) -> Result<i32, changed::SvcError> {
            Ok(x + y)
        }
        fn legacy(&self) -> Result<i32, changed::SvcError> {
            Ok(7)
        }
    }

    /// §7.7.2.5.3: the type of a parameter changed. Under a match by type
    /// name the two versions would be matched and the service would decode
    /// the request of the client with the wrong type; with the objects
    /// announced, the endpoints are not matched and no request arrives.
    #[test]
    fn a_changed_parameter_type_does_not_match() {
        let lease = zerodds_test_domains::lease_domain();
        let service_side = participant(lease.id());
        let client_side = participant(lease.id());
        let mut qos = RpcQos::default_enhanced();
        qos.request_timeout = Duration::from_secs(3);
        let calls = Arc::new(AtomicUsize::new(0));
        let service = changed::SvcDispatcher::new(ChangedService {
            calls: Arc::clone(&calls),
        })
        .serve(&service_side, &qos)
        .expect("the changed service");
        let _service = Ticking::spawn(service);
        let client = old::SvcClient::new(&client_side, &qos).expect("the old client");
        // The requester sees no matched service at all; under a match by
        // type name the request would have been sent and refused by the
        // service instead.
        let result = client.add(1, 2);
        assert!(
            matches!(
                result,
                Err(old::SvcError::Remote(
                    zerodds_rpc::error::RpcError::ServiceNotMatched
                ))
            ),
            "{result:?}"
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0, "the service saw a request");
    }
}

mod return_type {
    use super::*;
    use zerodds_rpc_rust_e2e::evolution_old::evo as old;
    use zerodds_rpc_rust_e2e::evolution_return::evo as changed;

    struct ChangedService;

    impl changed::Svc for ChangedService {
        fn add(&self, a: i32, b: i32) -> Result<i32, changed::SvcError> {
            Ok(a + b)
        }
        fn scale(&self, k: i32) -> Result<i64, changed::SvcError> {
            Ok(i64::from(k))
        }
        fn mix(&self, x: i32, y: i32) -> Result<i32, changed::SvcError> {
            Ok(x + y)
        }
        fn legacy(&self) -> Result<i32, changed::SvcError> {
            Ok(7)
        }
    }

    /// §7.7.2.5.5: the return type of `scale` changed from `long` to
    /// `long long`, which the old type is not assignable to: the peers are
    /// not matched.
    #[test]
    fn a_return_type_the_old_one_is_not_assignable_to_does_not_match() {
        let lease = zerodds_test_domains::lease_domain();
        let service_side = participant(lease.id());
        let client_side = participant(lease.id());
        let mut qos = RpcQos::default_enhanced();
        qos.request_timeout = Duration::from_secs(3);
        let service = changed::SvcDispatcher::new(ChangedService)
            .serve(&service_side, &qos)
            .expect("the changed service");
        let _service = Ticking::spawn(service);
        let client = old::SvcClient::new(&client_side, &qos).expect("the old client");
        let result = client.add(1, 2);
        assert!(
            matches!(
                result,
                Err(old::SvcError::Remote(
                    zerodds_rpc::error::RpcError::ServiceNotMatched
                ))
            ),
            "{result:?}"
        );
    }
}

mod appended_member {
    use super::*;
    use zerodds_rpc_rust_e2e::evolution_pose_new::shapes as new;
    use zerodds_rpc_rust_e2e::evolution_pose_old::shapes as old;

    struct NewService;

    impl new::Svc for NewService {
        fn length(&self, p: new::Pose) -> Result<i32, new::SvcError> {
            // The member the old client does not send takes its default.
            Ok(p.x + p.y)
        }
        fn origin(&self) -> Result<new::Pose, new::SvcError> {
            Ok(new::Pose { x: 3, y: 4 })
        }
    }

    /// §7.7.2.5.3: a member appended to a structure parameter is a change of
    /// the parameter's type that XTypes allows: the old client is matched
    /// with the new service and the member it does not know takes its
    /// default.
    #[test]
    fn a_member_appended_to_a_parameter_type_is_matched() {
        let lease = zerodds_test_domains::lease_domain();
        let service_side = participant(lease.id());
        let client_side = participant(lease.id());
        let qos = RpcQos::default_enhanced();
        let service = new::SvcDispatcher::new(NewService)
            .serve(&service_side, &qos)
            .expect("the new service");
        let _service = Ticking::spawn(service);
        let client = old::SvcClient::new(&client_side, &qos).expect("the old client");
        assert_eq!(client.length(old::Pose { x: 5 }).expect("length"), 5);
    }

    /// §7.7.2.5.5: the return type gained a member, which the old type is
    /// assignable from: the old client reads the reply of the new service and
    /// the member it does not know is skipped.
    #[test]
    fn a_return_type_with_an_appended_member_is_matched() {
        let lease = zerodds_test_domains::lease_domain();
        let service_side = participant(lease.id());
        let client_side = participant(lease.id());
        let qos = RpcQos::default_enhanced();
        let service = new::SvcDispatcher::new(NewService)
            .serve(&service_side, &qos)
            .expect("the new service");
        let _service = Ticking::spawn(service);
        let client = old::SvcClient::new(&client_side, &qos).expect("the old client");
        assert_eq!(client.origin().expect("origin"), old::Pose { x: 3 });
    }
}
