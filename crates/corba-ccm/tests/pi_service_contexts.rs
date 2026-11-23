// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Portable interceptors (CORBA 3.3 Part 1 §16.4): the flow-stack rules
//! of the registry, the validity of `RequestInfo` per interception point,
//! the service context rules, and real OTS (`TransactionService` id 0)
//! and CSIv2 (`SecurityAttributeService` id 15) contexts routed through
//! interceptors.

use std::sync::{Arc, Mutex};

use std::collections::{BTreeMap, BTreeSet};

use zerodds_corba_ccm::portable_interceptor::{
    ClientRequestInterceptor, InterceptionPoint, InterceptorError, InterceptorRegistry, IorInfo,
    Outcome, PointResult, Policy, ReplyStatus, RequestException, RequestInfo,
    ServerRequestInterceptor, ServiceContextInjector,
};
use zerodds_corba_cos_transactions::{Otid, PropagationContext, TRANSACTION_SERVICE_CONTEXT_ID};
use zerodds_corba_csiv2::gssup::GssupCredentialToken;
use zerodds_corba_rt::{
    AnyValue, CompletionStatus, CorbaAny, ObjectReference, SystemException, SystemExceptionKind,
};
use zerodds_wire::Endianness;

const SAS_ID: u32 = 15; // SecurityAttributeService

type Log = Arc<Mutex<Vec<String>>>;

/// A client interceptor that logs every point and raises what it is
/// told to at one point.
struct Client {
    name: &'static str,
    log: Log,
    raise_at: Option<(InterceptionPoint, InterceptorError)>,
}

impl Client {
    fn at(&self, point: InterceptionPoint, info: &RequestInfo) -> PointResult {
        let status = info
            .reply_status()
            .map_or_else(|_| "-".to_string(), |s| format!("{s:?}"));
        self.log
            .lock()
            .unwrap()
            .push(format!("{}:{point:?}:{status}", self.name));
        match &self.raise_at {
            Some((p, e)) if *p == point => Err(e.clone()),
            _ => Ok(()),
        }
    }
}

impl ClientRequestInterceptor for Client {
    fn name(&self) -> &str {
        self.name
    }
    fn send_request(&self, info: &mut RequestInfo) -> PointResult {
        self.at(InterceptionPoint::SendRequest, info)
    }
    fn receive_reply(&self, info: &mut RequestInfo) -> PointResult {
        self.at(InterceptionPoint::ReceiveReply, info)
    }
    fn receive_exception(&self, info: &mut RequestInfo) -> PointResult {
        self.at(InterceptionPoint::ReceiveException, info)
    }
    fn receive_other(&self, info: &mut RequestInfo) -> PointResult {
        self.at(InterceptionPoint::ReceiveOther, info)
    }
}

struct Server {
    name: &'static str,
    log: Log,
    raise_at: Option<(InterceptionPoint, InterceptorError)>,
}

impl Server {
    fn at(&self, point: InterceptionPoint, info: &RequestInfo) -> PointResult {
        let status = info
            .reply_status()
            .map_or_else(|_| "-".to_string(), |s| format!("{s:?}"));
        self.log
            .lock()
            .unwrap()
            .push(format!("{}:{point:?}:{status}", self.name));
        match &self.raise_at {
            Some((p, e)) if *p == point => Err(e.clone()),
            _ => Ok(()),
        }
    }
}

impl ServerRequestInterceptor for Server {
    fn name(&self) -> &str {
        self.name
    }
    fn receive_request_service_contexts(&self, info: &mut RequestInfo) -> PointResult {
        self.at(InterceptionPoint::ReceiveRequestServiceContexts, info)
    }
    fn receive_request(&self, info: &mut RequestInfo) -> PointResult {
        self.at(InterceptionPoint::ReceiveRequest, info)
    }
    fn send_reply(&self, info: &mut RequestInfo) -> PointResult {
        self.at(InterceptionPoint::SendReply, info)
    }
    fn send_exception(&self, info: &mut RequestInfo) -> PointResult {
        self.at(InterceptionPoint::SendException, info)
    }
    fn send_other(&self, info: &mut RequestInfo) -> PointResult {
        self.at(InterceptionPoint::SendOther, info)
    }
}

fn no_permission() -> SystemException {
    SystemException::new(SystemExceptionKind::NoPermission, 0, CompletionStatus::No)
}

fn forward_target() -> ObjectReference {
    ObjectReference {
        type_id: "IDL:demo/Elsewhere:1.0".into(),
        iiop_profile: vec![0, 1, 2, 0],
        policy_overrides: Vec::new(),
        profiles: Vec::new(),
    }
}

fn clients(
    raises: [Option<(InterceptionPoint, InterceptorError)>; 3],
) -> (InterceptorRegistry, Log) {
    let log: Log = Arc::default();
    let mut reg = InterceptorRegistry::new();
    for (name, raise_at) in ["A", "B", "C"].into_iter().zip(raises) {
        reg.add_client(Arc::new(Client {
            name,
            log: log.clone(),
            raise_at,
        }))
        .unwrap();
    }
    (reg, log)
}

fn servers(
    raises: [Option<(InterceptionPoint, InterceptorError)>; 3],
) -> (InterceptorRegistry, Log) {
    let log: Log = Arc::default();
    let mut reg = InterceptorRegistry::new();
    for (name, raise_at) in ["A", "B", "C"].into_iter().zip(raises) {
        reg.add_server(Arc::new(Server {
            name,
            log: log.clone(),
            raise_at,
        }))
        .unwrap();
    }
    (reg, log)
}

fn taken(log: &Log) -> Vec<String> {
    std::mem::take(&mut *log.lock().unwrap())
}

#[test]
fn corba_p1_16_4_7_starting_points_in_order_ending_points_in_reverse() {
    let (reg, log) = clients([None, None, None]);
    let mut info = RequestInfo::client(1, "op", true, ObjectReference::default());
    let depth = reg.client_start(&mut info, false).unwrap();
    assert_eq!(depth, 3);
    let out = reg.client_end(&mut info, depth, Outcome::Proceed);
    assert_eq!(out, Outcome::Proceed);
    assert_eq!(
        taken(&log),
        [
            "A:SendRequest:-",
            "B:SendRequest:-",
            "C:SendRequest:-",
            "C:ReceiveReply:Successful",
            "B:ReceiveReply:Successful",
            "A:ReceiveReply:Successful",
        ]
    );
}

#[test]
fn corba_p1_16_4_6_1_an_exception_in_send_request_pops_only_the_flow_stack() {
    let (reg, log) = clients([
        None,
        Some((
            InterceptionPoint::SendRequest,
            InterceptorError::System(no_permission()),
        )),
        None,
    ]);
    let mut info = RequestInfo::client(1, "op", true, ObjectReference::default());
    let out = reg.client_start(&mut info, false).unwrap_err();
    assert_eq!(
        out,
        Outcome::Exception(RequestException::System(no_permission()))
    );
    // C never starts; only A, which completed its starting point, ends.
    assert_eq!(
        taken(&log),
        [
            "A:SendRequest:-",
            "B:SendRequest:-",
            "A:ReceiveException:SystemException",
        ]
    );
}

#[test]
fn corba_p1_16_4_6_1_forward_request_in_send_request_runs_receive_other() {
    let (reg, log) = clients([
        None,
        None,
        Some((
            InterceptionPoint::SendRequest,
            InterceptorError::ForwardRequest(forward_target()),
        )),
    ]);
    let mut info = RequestInfo::client(1, "op", true, ObjectReference::default());
    let out = reg.client_start(&mut info, false).unwrap_err();
    assert_eq!(out, Outcome::Forward(forward_target()));
    assert_eq!(
        taken(&log),
        [
            "A:SendRequest:-",
            "B:SendRequest:-",
            "C:SendRequest:-",
            "B:ReceiveOther:LocationForward",
            "A:ReceiveOther:LocationForward",
        ]
    );
}

#[test]
fn corba_p1_16_4_6_3_an_exception_in_receive_reply_turns_the_rest_to_receive_exception() {
    let (reg, log) = clients([
        None,
        None,
        Some((
            InterceptionPoint::ReceiveReply,
            InterceptorError::System(no_permission()),
        )),
    ]);
    let mut info = RequestInfo::client(1, "op", true, ObjectReference::default());
    let depth = reg.client_start(&mut info, false).unwrap();
    taken(&log);
    let out = reg.client_end(&mut info, depth, Outcome::Proceed);
    assert_eq!(
        out,
        Outcome::Exception(RequestException::System(no_permission()))
    );
    assert_eq!(
        taken(&log),
        [
            "C:ReceiveReply:Successful",
            "B:ReceiveException:SystemException",
            "A:ReceiveException:SystemException",
        ]
    );
}

#[test]
fn corba_p1_16_4_6_4_receive_exception_may_change_the_exception_or_forward() {
    let changed = SystemException::omg(SystemExceptionKind::Transient, 2, CompletionStatus::No);
    let (reg, log) = clients([
        None,
        Some((
            InterceptionPoint::ReceiveException,
            InterceptorError::ForwardRequest(forward_target()),
        )),
        Some((
            InterceptionPoint::ReceiveException,
            InterceptorError::System(changed),
        )),
    ]);
    let mut info = RequestInfo::client(1, "op", true, ObjectReference::default());
    let depth = reg.client_start(&mut info, false).unwrap();
    taken(&log);
    let out = reg.client_end(
        &mut info,
        depth,
        Outcome::Exception(RequestException::System(no_permission())),
    );
    assert_eq!(out, Outcome::Forward(forward_target()));
    assert_eq!(
        taken(&log),
        [
            "C:ReceiveException:SystemException",
            "B:ReceiveException:SystemException",
            "A:ReceiveOther:LocationForward",
        ]
    );
}

#[test]
fn corba_p1_16_4_7_2_a_oneway_ends_in_receive_other_successful() {
    let (reg, log) = clients([None, None, None]);
    let mut info = RequestInfo::client(1, "notify", false, ObjectReference::default());
    let depth = reg.client_start(&mut info, false).unwrap();
    taken(&log);
    assert_eq!(
        reg.client_end(&mut info, depth, Outcome::Other),
        Outcome::Other
    );
    assert_eq!(
        taken(&log),
        [
            "C:ReceiveOther:Successful",
            "B:ReceiveOther:Successful",
            "A:ReceiveOther:Successful",
        ]
    );
}

#[test]
fn corba_p1_16_4_10_server_flow_with_an_exception_in_receive_request() {
    let (reg, log) = servers([
        None,
        Some((
            InterceptionPoint::ReceiveRequest,
            InterceptorError::System(no_permission()),
        )),
        None,
    ]);
    let mut info = RequestInfo::server(9, "op", true, b"key".to_vec());
    // The starting point runs alone: the transport copies the request
    // scope into the thread scope before receive_request (§16.5.4.5).
    let depth = reg.server_start(&mut info).unwrap();
    assert_eq!(
        taken(&log),
        [
            "A:ReceiveRequestServiceContexts:-",
            "B:ReceiveRequestServiceContexts:-",
            "C:ReceiveRequestServiceContexts:-",
        ]
    );
    let out = reg.server_receive_request(&mut info, depth).unwrap_err();
    assert_eq!(
        out,
        Outcome::Exception(RequestException::System(no_permission()))
    );
    // Every interceptor completed the starting point: all three end.
    assert_eq!(
        taken(&log),
        [
            "A:ReceiveRequest:-",
            "B:ReceiveRequest:-",
            "C:SendException:SystemException",
            "B:SendException:SystemException",
            "A:SendException:SystemException",
        ]
    );
}

#[test]
fn corba_p1_16_4_9_server_reply_and_forward_from_send_exception() {
    let (reg, log) = servers([
        Some((
            InterceptionPoint::SendException,
            InterceptorError::ForwardRequest(forward_target()),
        )),
        None,
        None,
    ]);
    let mut info = RequestInfo::server(9, "op", true, b"key".to_vec());
    let depth = reg.server_start(&mut info).unwrap();
    reg.server_receive_request(&mut info, depth).unwrap();
    taken(&log);
    let user = RequestException::User {
        repository_id: "IDL:demo/Oops:1.0".into(),
        body: zerodds_corba_giop::AlignedBody::new(&[], 0),
        endianness: Endianness::Big,
    };
    let out = reg.server_end(&mut info, depth, Outcome::Exception(user));
    assert_eq!(out, Outcome::Forward(forward_target()));
    assert_eq!(
        taken(&log),
        [
            "C:SendException:UserException",
            "B:SendException:UserException",
            "A:SendException:UserException",
        ]
    );
    assert_eq!(info.reply_status().unwrap(), ReplyStatus::LocationForward);
}

#[test]
fn corba_p1_16_4_13_request_info_validity_and_service_context_rules() {
    struct Probe {
        seen: Mutex<Vec<String>>,
    }
    impl ClientRequestInterceptor for Probe {
        fn name(&self) -> &str {
            "probe"
        }
        fn send_request(&self, info: &mut RequestInfo) -> PointResult {
            let mut seen = self.seen.lock().unwrap();
            // Table 16.1: reply_status is not valid in send_request.
            seen.push(format!(
                "{:?}",
                info.reply_status().unwrap_err().omg_minor()
            ));
            info.add_request_service_context(77, vec![1], false)?;
            // A second add without replace: BAD_INV_ORDER minor 15.
            let dup = info
                .add_request_service_context(77, vec![2], false)
                .unwrap_err();
            seen.push(format!("{:?}", dup.omg_minor()));
            info.add_request_service_context(77, vec![3], true)?;
            // Absent id: BAD_PARAM minor 26.
            seen.push(format!(
                "{:?}",
                info.get_request_service_context(78)
                    .unwrap_err()
                    .omg_minor()
            ));
            seen.push(format!("{:?}", info.arguments().unwrap_err().kind));
            for e in [
                info.exceptions().unwrap_err(),
                info.contexts().unwrap_err(),
                info.operation_context().unwrap_err(),
            ] {
                seen.push(format!("{:?}:{:?}", e.kind, e.omg_minor()));
            }
            Ok(())
        }
        fn receive_reply(&self, info: &mut RequestInfo) -> PointResult {
            let mut seen = self.seen.lock().unwrap();
            // add_request_service_context is valid only in send_request.
            let late = info
                .add_request_service_context(79, vec![], true)
                .unwrap_err();
            seen.push(format!("{:?}", late.omg_minor()));
            seen.push(format!(
                "{:?}",
                info.forward_reference().unwrap_err().omg_minor()
            ));
            seen.push(format!("{:?}", info.reply_status()));
            Ok(())
        }
    }
    let probe = Arc::new(Probe {
        seen: Mutex::new(Vec::new()),
    });
    let mut reg = InterceptorRegistry::new();
    reg.add_client(probe.clone()).unwrap();
    let mut info = RequestInfo::client(5, "op", true, ObjectReference::default());
    let depth = reg.client_start(&mut info, false).unwrap();
    assert_eq!(info.request_service_contexts().get(&77), Some(&vec![3]));
    reg.client_end(&mut info, depth, Outcome::Proceed);
    assert_eq!(
        *probe.seen.lock().unwrap(),
        [
            "Some(14)",
            "Some(15)",
            "Some(26)",
            "NoResources",
            "NoResources:Some(1)",
            "NoResources:Some(1)",
            "NoResources:Some(1)",
            "Some(14)",
            "Some(14)",
            "Ok(Successful)",
        ]
    );
}

#[test]
fn corba_p1_16_9_2_a_duplicate_interceptor_name_is_refused() {
    let mut reg = InterceptorRegistry::new();
    reg.add_client(Arc::new(ServiceContextInjector::new("same", 1, vec![])))
        .unwrap();
    let dup = reg
        .add_client(Arc::new(ServiceContextInjector::new("same", 2, vec![])))
        .unwrap_err();
    assert_eq!(dup.name, "same");
    // Anonymous interceptors may repeat.
    reg.add_client(Arc::new(ServiceContextInjector::new("", 3, vec![])))
        .unwrap();
    reg.add_client(Arc::new(ServiceContextInjector::new("", 4, vec![])))
        .unwrap();
    assert_eq!(reg.client_count(), 3);
}

#[test]
fn corba_p1_16_4_ots_and_csiv2_service_contexts_route_through_interceptors() {
    let otid = Otid::new(7, vec![1, 2, 3, 4]);
    let ots_bytes = PropagationContext::flat(30, otid.clone())
        .to_service_context_data(Endianness::Big)
        .unwrap();
    let gss = GssupCredentialToken::new("alice".into(), "secret".into(), b"target".to_vec())
        .to_gss_token(Endianness::Big)
        .unwrap();

    let mut client_reg = InterceptorRegistry::new();
    client_reg
        .add_client(Arc::new(ServiceContextInjector::new(
            "OTSInterceptor",
            TRANSACTION_SERVICE_CONTEXT_ID,
            ots_bytes.clone(),
        )))
        .unwrap();
    client_reg
        .add_client(Arc::new(ServiceContextInjector::new(
            "CSIv2Interceptor",
            SAS_ID,
            gss.clone(),
        )))
        .unwrap();
    let mut info = RequestInfo::client(1, "transfer", true, ObjectReference::default());
    client_reg.client_start(&mut info, false).unwrap();
    assert_eq!(
        info.get_request_service_context(0).unwrap(),
        ots_bytes.as_slice()
    );
    assert_eq!(
        info.get_request_service_context(SAS_ID).unwrap(),
        gss.as_slice()
    );

    struct Extractor {
        otid: Mutex<Option<Otid>>,
        user: Mutex<Option<String>>,
    }
    impl ServerRequestInterceptor for Extractor {
        fn name(&self) -> &str {
            "extractor"
        }
        fn receive_request_service_contexts(&self, info: &mut RequestInfo) -> PointResult {
            let pc =
                PropagationContext::from_service_context_data(info.get_request_service_context(0)?)
                    .map_err(|_| {
                        SystemException::new(SystemExceptionKind::Marshal, 0, CompletionStatus::No)
                    })?;
            *self.otid.lock().unwrap() = Some(pc.current.otid);
            let tok =
                GssupCredentialToken::from_gss_token(info.get_request_service_context(SAS_ID)?)
                    .map_err(|_| no_permission())?;
            *self.user.lock().unwrap() = Some(tok.username);
            Ok(())
        }
    }
    let extractor = Arc::new(Extractor {
        otid: Mutex::new(None),
        user: Mutex::new(None),
    });
    let mut server_reg = InterceptorRegistry::new();
    server_reg.add_server(extractor.clone()).unwrap();
    let mut server_info = RequestInfo::server(1, "transfer", true, b"Bank".to_vec());
    for (id, data) in info.request_service_contexts() {
        server_info.set_received_request_context(*id, data.clone());
    }
    server_reg.server_start(&mut server_info).unwrap();
    assert_eq!(*extractor.otid.lock().unwrap(), Some(otid));
    assert_eq!(extractor.user.lock().unwrap().as_deref(), Some("alice"));
}

/// What `exceptions`, `contexts` and `operation_context` raise at one
/// point: `NR` for NO_RESOURCES minor 1, `BIO` for BAD_INV_ORDER minor 14.
fn availability(info: &RequestInfo) -> String {
    let short = |r: Result<(), SystemException>| match r {
        Err(e) if e.kind == SystemExceptionKind::NoResources && e.omg_minor() == Some(1) => "NR",
        Err(e) if e.kind == SystemExceptionKind::BadInvOrder && e.omg_minor() == Some(14) => "BIO",
        other => panic!("unexpected {other:?}"),
    };
    format!(
        "{:?}:{}/{}/{}",
        info.point().unwrap(),
        short(info.exceptions()),
        short(info.contexts()),
        short(info.operation_context())
    )
}

struct Availability(Log);

impl Availability {
    fn note(&self, info: &RequestInfo) -> PointResult {
        self.0.lock().unwrap().push(availability(info));
        Ok(())
    }
}

impl ClientRequestInterceptor for Availability {
    fn name(&self) -> &str {
        "availability"
    }
    fn send_request(&self, info: &mut RequestInfo) -> PointResult {
        self.note(info)
    }
    fn send_poll(&self, info: &mut RequestInfo) -> PointResult {
        self.note(info)
    }
    fn receive_reply(&self, info: &mut RequestInfo) -> PointResult {
        self.note(info)
    }
    fn receive_exception(&self, info: &mut RequestInfo) -> PointResult {
        self.note(info)
    }
    fn receive_other(&self, info: &mut RequestInfo) -> PointResult {
        self.note(info)
    }
}

impl ServerRequestInterceptor for Availability {
    fn name(&self) -> &str {
        "availability"
    }
    fn receive_request_service_contexts(&self, info: &mut RequestInfo) -> PointResult {
        self.note(info)
    }
    fn receive_request(&self, info: &mut RequestInfo) -> PointResult {
        self.note(info)
    }
    fn send_reply(&self, info: &mut RequestInfo) -> PointResult {
        self.note(info)
    }
    fn send_exception(&self, info: &mut RequestInfo) -> PointResult {
        self.note(info)
    }
    fn send_other(&self, info: &mut RequestInfo) -> PointResult {
        self.note(info)
    }
}

#[test]
fn corba_p1_16_4_12_exceptions_contexts_and_operation_context_follow_tables_16_1_and_16_2() {
    let log: Log = Arc::default();
    let mut reg = InterceptorRegistry::new();
    reg.add_client(Arc::new(Availability(log.clone()))).unwrap();
    reg.add_server(Arc::new(Availability(log.clone()))).unwrap();
    let exception = || Outcome::Exception(RequestException::System(no_permission()));

    // Client: send_request with each ending point, and send_poll.
    for end in [Outcome::Proceed, exception(), Outcome::Other] {
        let mut info = RequestInfo::client(1, "op", true, ObjectReference::default());
        let depth = reg.client_start(&mut info, false).unwrap();
        reg.client_end(&mut info, depth, end);
    }
    let mut info = RequestInfo::client(2, "op", true, ObjectReference::default());
    reg.client_start(&mut info, true).unwrap();
    assert_eq!(
        taken(&log),
        [
            "SendRequest:NR/NR/NR",
            "ReceiveReply:NR/NR/NR",
            "SendRequest:NR/NR/NR",
            "ReceiveException:NR/NR/NR",
            "SendRequest:NR/NR/NR",
            "ReceiveOther:NR/NR/NR",
            "SendPoll:BIO/BIO/BIO",
        ]
    );

    // Server: the starting and intermediate points with each ending point.
    for end in [
        Outcome::Proceed,
        exception(),
        Outcome::Forward(forward_target()),
    ] {
        let mut info = RequestInfo::server(9, "op", true, b"key".to_vec());
        let depth = reg.server_start(&mut info).unwrap();
        reg.server_receive_request(&mut info, depth).unwrap();
        reg.server_end(&mut info, depth, end);
    }
    assert_eq!(
        taken(&log),
        [
            "ReceiveRequestServiceContexts:BIO/BIO/BIO",
            "ReceiveRequest:NR/NR/NR",
            "SendReply:NR/NR/NR",
            "ReceiveRequestServiceContexts:BIO/BIO/BIO",
            "ReceiveRequest:NR/NR/NR",
            "SendException:NR/NR/BIO",
            "ReceiveRequestServiceContexts:BIO/BIO/BIO",
            "ReceiveRequest:NR/NR/NR",
            "SendOther:NR/NR/BIO",
        ]
    );
}

#[test]
fn corba_p1_16_6_5_ior_info_checks_the_profile_id_and_the_policy_type() {
    const TAG_INTERNET_IOP: u32 = 0;
    const TAG_MULTIPLE_COMPONENTS: u32 = 1;
    let known: BTreeSet<u32> = [40, 41].into_iter().collect();
    let effective = Policy {
        policy_type: 40,
        value: CorbaAny(AnyValue::Long(3)),
    };
    let mut policies = BTreeMap::new();
    policies.insert(40, effective.clone());
    let mut info = IorInfo::new(vec![TAG_INTERNET_IOP], known, policies);

    // §16.6.5.1: the effective policy; nil for a known type without one;
    // INV_POLICY minor 3 for a type the ORB does not know.
    assert_eq!(info.get_effective_policy(40).unwrap(), Some(&effective));
    assert_eq!(info.get_effective_policy(41).unwrap(), None);
    let unknown = info.get_effective_policy(99).unwrap_err();
    assert_eq!(
        (unknown.kind, unknown.omg_minor()),
        (SystemExceptionKind::InvPolicy, Some(3))
    );

    // §16.6.5.3: a profile the references carry takes the component; any
    // other profile id is BAD_PARAM minor 29 and adds nothing.
    info.add_ior_component(0x10, vec![1]);
    info.add_ior_component_to_profile(TAG_INTERNET_IOP, 0x11, vec![2])
        .unwrap();
    let refused = info
        .add_ior_component_to_profile(TAG_MULTIPLE_COMPONENTS, 0x12, vec![3])
        .unwrap_err();
    assert_eq!(
        (refused.kind, refused.omg_minor()),
        (SystemExceptionKind::BadParam, Some(29))
    );
    assert_eq!(
        info.components_for_profile(TAG_INTERNET_IOP),
        vec![(0x10, vec![1]), (0x11, vec![2])]
    );
    assert_eq!(
        info.components_for_profile(TAG_MULTIPLE_COMPONENTS),
        vec![(0x10, vec![1])]
    );
}
