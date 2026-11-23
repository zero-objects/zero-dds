// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CORBA 3.3 Part 1 §16 over IIOP: interceptors an `ORBInitializer`
//! registers run in the request path of `IiopCorbaConnection` and
//! `CorbaServer`. A service context goes from a client interceptor to a
//! server interceptor and a reply context back; a slot the server
//! interceptor sets reaches the servant through `PICurrent`; an exception
//! a server interceptor raises stops the servant and reaches the client;
//! a `ForwardRequest` from either side sends the request to another
//! object. `receive_request` runs in the servant's thread scope: a slot
//! it sets through `PICurrent` reaches the servant, one it sets in the
//! request scope does not (§16.5.4.5 step 5).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use zerodds_corba_interop::string_argument;

use zerodds_corba_ccm::portable_interceptor::{
    ClientRequestInterceptor, InitializerError, InterceptorError, IorInfo, IorInterceptor,
    OrbInitInfo, OrbInitializer, PiCurrent, PointResult, Policy, PolicyError, PolicyFactory,
    RequestInfo, ServerRequestInterceptor, register_orb_initializer,
};
use zerodds_corba_iiop::IiopProfileBody;
use zerodds_corba_interop::runtime::{
    CorbaServer, IiopCorbaConnection, object_reference, object_reference_for, orb_init,
};
use zerodds_corba_interop::{decode_string_body, encode_string_body};
use zerodds_corba_rt::{
    AnyValue, CompletionStatus, CorbaAny, ObjectReference, SystemExceptionKind,
};
use zerodds_corba_rust::{CorbaConnection, CorbaException, SkeletonResult};
use zerodds_wire::Endianness;

/// A service context id outside the OMG range (Part 2 §7.7: ids with
/// the high 20 bits non-zero are vendor ids).
const REQUEST_CTX: u32 = 0x5a44_0001;
const REPLY_CTX: u32 = 0x5a44_0002;
/// A policy type with a registered factory, and one without.
const KNOWN_POLICY: u32 = 0x5a44_0100;
const UNKNOWN_POLICY: u32 = 0x5a44_0101;

#[derive(Default)]
struct Shared {
    /// The slot receive_request_service_contexts fills in the request
    /// scope.
    slot: AtomicU32,
    /// The slot receive_request sets through PICurrent (thread scope).
    tsc_slot: AtomicU32,
    current: OnceLock<Arc<PiCurrent>>,
    server_saw: Mutex<Vec<String>>,
    client_saw: Mutex<Vec<String>>,
    ior_saw: Mutex<Vec<String>>,
    forward_to: Mutex<Option<ObjectReference>>,
}

fn text(any: &CorbaAny) -> String {
    match any {
        CorbaAny(AnyValue::Str(s)) => s.clone(),
        CorbaAny(AnyValue::Null) => "null".to_string(),
        other => format!("{other:?}"),
    }
}

struct ClientSide(Arc<Shared>);

impl ClientRequestInterceptor for ClientSide {
    fn name(&self) -> &str {
        "test-client"
    }
    fn send_request(&self, info: &mut RequestInfo) -> PointResult {
        let to = self.0.forward_to.lock().unwrap().clone().unwrap();
        // The retry after a forward is a new request with the new
        // effective target (§16.4.6.5).
        if info.operation() == "redirect" && *info.effective_target()? != to {
            return Err(InterceptorError::ForwardRequest(to));
        }
        info.add_request_service_context(REQUEST_CTX, b"from-client".to_vec(), false)?;
        Ok(())
    }
    fn receive_reply(&self, info: &mut RequestInfo) -> PointResult {
        let ctx = info.get_reply_service_context(REPLY_CTX)?;
        self.0.client_saw.lock().unwrap().push(format!(
            "{}:{}",
            info.operation(),
            String::from_utf8_lossy(ctx)
        ));
        Ok(())
    }
    fn receive_exception(&self, info: &mut RequestInfo) -> PointResult {
        let id = info.received_exception()?.repository_id().to_string();
        self.0
            .client_saw
            .lock()
            .unwrap()
            .push(format!("{}:exception:{id}", info.operation()));
        Ok(())
    }
}

struct ServerSide(Arc<Shared>);

impl ServerRequestInterceptor for ServerSide {
    fn name(&self) -> &str {
        "test-server"
    }
    fn receive_request_service_contexts(&self, info: &mut RequestInfo) -> PointResult {
        let ctx = info.get_request_service_context(REQUEST_CTX)?.to_vec();
        // This point runs in a thread scope of its own (§16.5.4.6): what
        // receive_request of an earlier request set there is not seen.
        let pic = self.0.current.get().unwrap();
        let tsc = pic
            .get_slot(self.0.tsc_slot.load(Ordering::SeqCst))
            .unwrap();
        self.0.server_saw.lock().unwrap().push(format!(
            "{}:{}:tsc={}",
            info.operation(),
            String::from_utf8_lossy(&ctx),
            text(&tsc)
        ));
        let slot = self.0.slot.load(Ordering::SeqCst);
        info.set_slot(
            slot,
            CorbaAny(AnyValue::Str(String::from_utf8_lossy(&ctx).into_owned())),
        )
        .unwrap();
        Ok(())
    }
    fn receive_request(&self, info: &mut RequestInfo) -> PointResult {
        // object_id is valid from receive_request on (Table 16.2).
        if info.operation() == "moved" && info.object_id()? == b"Echo" {
            let to = self.0.forward_to.lock().unwrap().clone().unwrap();
            return Err(InterceptorError::ForwardRequest(to));
        }
        if info.operation() == "slot" {
            // Step 5: the request scope no longer reaches the servant's
            // thread scope; PICurrent here is that thread scope.
            info.set_slot(
                self.0.slot.load(Ordering::SeqCst),
                CorbaAny(AnyValue::Str("rsc-in-receive-request".into())),
            )
            .unwrap();
            self.0
                .current
                .get()
                .unwrap()
                .set_slot(
                    self.0.tsc_slot.load(Ordering::SeqCst),
                    CorbaAny(AnyValue::Str("tsc-in-receive-request".into())),
                )
                .unwrap();
        }
        if info.operation() == "secret" {
            return Err(InterceptorError::System(
                zerodds_corba_rt::SystemException::new(
                    SystemExceptionKind::NoPermission,
                    0,
                    CompletionStatus::No,
                ),
            ));
        }
        Ok(())
    }
    fn send_reply(&self, info: &mut RequestInfo) -> PointResult {
        info.add_reply_service_context(REPLY_CTX, b"from-server".to_vec(), false)?;
        if info.operation() == "slot" {
            // Step 8: the servant's thread scope overwrote the request
            // scope.
            let rsc = info.get_slot(self.0.slot.load(Ordering::SeqCst)).unwrap();
            let tsc = info
                .get_slot(self.0.tsc_slot.load(Ordering::SeqCst))
                .unwrap();
            self.0.server_saw.lock().unwrap().push(format!(
                "send_reply:{}|{}",
                text(&rsc),
                text(&tsc)
            ));
        }
        Ok(())
    }
}

/// Adds one component to every profile, one to `TAG_INTERNET_IOP` and
/// tries one for profile id 1 (`TAG_MULTIPLE_COMPONENTS`), which the
/// references of this adapter do not carry; asks for two policies.
struct Tagger(Arc<Shared>);

impl IorInterceptor for Tagger {
    fn name(&self) -> &str {
        "test-tagger"
    }
    fn establish_components(&self, info: &mut IorInfo) {
        let mut saw = self.0.ior_saw.lock().unwrap();
        info.add_ior_component(0x5a44_0010, vec![0, 0xab, 0xcd]);
        saw.push(format!(
            "iiop:{:?}",
            info.add_ior_component_to_profile(0, 0x5a44_0012, vec![7])
                .map_err(|e| e.omg_minor())
        ));
        saw.push(format!(
            "multiple:{:?}",
            info.add_ior_component_to_profile(1, 0x5a44_0011, vec![0])
                .map_err(|e| (e.kind, e.omg_minor()))
        ));
        saw.push(format!(
            "known:{:?}",
            info.get_effective_policy(KNOWN_POLICY)
                .map_err(|e| e.omg_minor())
        ));
        saw.push(format!(
            "unknown:{:?}",
            info.get_effective_policy(UNKNOWN_POLICY)
                .map_err(|e| (e.kind, e.omg_minor()))
        ));
    }
}

struct Factory;

impl PolicyFactory for Factory {
    fn create_policy(&self, policy_type: u32, value: &CorbaAny) -> Result<Policy, PolicyError> {
        Ok(Policy {
            policy_type,
            value: value.clone(),
        })
    }
}

struct Init(Arc<Shared>);

impl OrbInitializer for Init {
    fn pre_init(&self, info: &mut OrbInitInfo<'_>) -> Result<(), InitializerError> {
        // The initializer is process-wide; it equips only the ORBs of
        // this test.
        match info.orb_id() {
            "pi-16-client" => {
                info.add_client_request_interceptor(Arc::new(ClientSide(self.0.clone())))?;
            }
            "pi-16-server" => {
                self.0
                    .slot
                    .store(info.allocate_slot_id()?, Ordering::SeqCst);
                self.0
                    .tsc_slot
                    .store(info.allocate_slot_id()?, Ordering::SeqCst);
                info.add_server_request_interceptor(Arc::new(ServerSide(self.0.clone())))?;
                info.add_ior_interceptor(Arc::new(Tagger(self.0.clone())))?;
                info.register_policy_factory(KNOWN_POLICY, Arc::new(Factory))?;
            }
            _ => {}
        }
        Ok(())
    }

    fn post_init(&self, info: &mut OrbInitInfo<'_>) -> Result<(), InitializerError> {
        if info.orb_id() == "pi-16-server" {
            let current = info
                .resolve_initial_references("PICurrent")?
                .local::<PiCurrent>()
                .unwrap();
            self.0.current.set(current).unwrap();
        }
        Ok(())
    }
}

#[test]
fn corba_p1_16_4_interceptors_run_in_the_iiop_request_path() {
    let shared = Arc::new(Shared::default());
    register_orb_initializer(Arc::new(Init(shared.clone())));
    let client_orb = orb_init(&mut Vec::new(), "pi-16-client").unwrap();
    let server_orb = orb_init(&mut Vec::new(), "pi-16-server").unwrap();
    assert_eq!(server_orb.pi_current().slot_count(), 2);

    let server = CorbaServer::new();
    server.set_orb(server_orb.clone());
    let servant_calls = Arc::new(AtomicUsize::new(0));
    {
        let current = server_orb.pi_current();
        let slot = shared.slot.load(Ordering::SeqCst);
        let tsc_slot = shared.tsc_slot.load(Ordering::SeqCst);
        let calls = servant_calls.clone();
        server.register(b"Echo", move |op, body, _e| {
            calls.fetch_add(1, Ordering::SeqCst);
            match op {
                // The servant reads, through PICurrent, what
                // receive_request_service_contexts put in the request scope
                // and what receive_request put in the thread scope
                // (§16.5.4.5).
                "slot" => {
                    let rsc = text(&current.get_slot(slot).unwrap());
                    let tsc = text(&current.get_slot(tsc_slot).unwrap());
                    SkeletonResult::Reply(encode_string_body(&format!("{rsc}|{tsc}")))
                }
                _ => SkeletonResult::Reply(body.bytes().to_vec()),
            }
        });
    }
    server.register(b"Echo2", |_op, _body, _e| {
        SkeletonResult::Reply(encode_string_body("from Echo2"))
    });
    let acceptor = server.serve("127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = acceptor.listen_addr();
    let host = addr.ip().to_string();
    let echo = object_reference("IDL:Echo:1.0", &host, addr.port(), b"Echo");
    let echo2 = object_reference("IDL:Echo:1.0", &host, addr.port(), b"Echo2");
    *shared.forward_to.lock().unwrap() = Some(echo2);

    let conn = IiopCorbaConnection::new().with_orb(&client_orb);

    // Request context out, reply context back.
    let (reply, _) = conn
        .invoke(&echo, "ping", Endianness::Big, &string_argument("hi"))
        .unwrap();
    assert_eq!(decode_string_body(&reply).unwrap(), "hi");
    // Request scope slot to the servant; the request scope slot set in
    // receive_request does not reach it, the PICurrent slot does.
    let (reply, _) = conn
        .invoke(&echo, "slot", Endianness::Big, &string_argument(""))
        .unwrap();
    assert_eq!(
        decode_string_body(&reply).unwrap(),
        "from-client|tsc-in-receive-request"
    );
    // A later request starts with an empty thread scope again.
    conn.invoke(&echo, "ping", Endianness::Big, &string_argument("again"))
        .unwrap();
    assert_eq!(
        *shared.server_saw.lock().unwrap(),
        [
            "ping:from-client:tsc=null",
            "slot:from-client:tsc=null",
            "send_reply:from-client|tsc-in-receive-request",
            "ping:from-client:tsc=null",
        ]
    );
    assert_eq!(
        *shared.client_saw.lock().unwrap(),
        ["ping:from-server", "slot:from-server", "ping:from-server"]
    );

    // An exception from receive_request: the servant does not run, the
    // client gets the exception and its receive_exception sees it.
    let calls_before = servant_calls.load(Ordering::SeqCst);
    let err = conn
        .invoke(&echo, "secret", Endianness::Big, &string_argument(""))
        .unwrap_err();
    assert_eq!(
        err.as_system().map(|e| (e.kind, e.completed)),
        Some((SystemExceptionKind::NoPermission, CompletionStatus::No))
    );
    assert_eq!(servant_calls.load(Ordering::SeqCst), calls_before);
    assert_eq!(
        shared.client_saw.lock().unwrap().last().unwrap(),
        "secret:exception:IDL:omg.org/CORBA/NO_PERMISSION:1.0"
    );

    // ForwardRequest in a server interceptor: LOCATION_FORWARD reply, the
    // client follows it to Echo2.
    let (reply, _) = conn
        .invoke(&echo, "moved", Endianness::Big, &string_argument(""))
        .unwrap();
    assert_eq!(decode_string_body(&reply).unwrap(), "from Echo2");
    // ForwardRequest in a client interceptor: the request goes to Echo2
    // without reaching Echo.
    let calls_before = servant_calls.load(Ordering::SeqCst);
    let (reply, _) = conn
        .invoke(&echo, "redirect", Endianness::Big, &string_argument(""))
        .unwrap();
    assert_eq!(decode_string_body(&reply).unwrap(), "from Echo2");
    assert_eq!(servant_calls.load(Ordering::SeqCst), calls_before);

    // A plain connection without the ORB runs no client interceptor.
    let plain = IiopCorbaConnection::new();
    let seen = shared.client_saw.lock().unwrap().len();
    let err = plain
        .invoke(&echo, "ping", Endianness::Big, &string_argument("x"))
        .unwrap_err();
    // The server interceptor requires the request context: BAD_PARAM 26
    // raised in receive_request_service_contexts reaches the client.
    assert!(matches!(err, CorbaException::System(e) if e.omg_minor() == Some(26)));
    assert_eq!(shared.client_saw.lock().unwrap().len(), seen);

    // §16.6: the IOR interceptor's components go into the IIOP profile of
    // a reference the server ORB makes; profile id 1 is BAD_PARAM 29
    // (§16.6.5.3); a known policy type without a policy is nil, an
    // unknown one INV_POLICY 3 (§16.6.5.1).
    let tagged = object_reference_for(&server_orb, "IDL:Echo:1.0", &host, addr.port(), b"Echo");
    let profile = IiopProfileBody::decode_encapsulation(&tagged.iiop_profile).unwrap();
    let tags: Vec<u32> = profile.components.iter().map(|c| c.tag).collect();
    assert!(tags.contains(&0x5a44_0010), "{tags:x?}");
    assert!(tags.contains(&0x5a44_0012), "{tags:x?}");
    assert!(!tags.contains(&0x5a44_0011), "{tags:x?}");
    assert_eq!(
        *shared.ior_saw.lock().unwrap(),
        [
            "iiop:Ok(())".to_string(),
            format!(
                "multiple:{:?}",
                Err::<(), _>((SystemExceptionKind::BadParam, Some(29u32)))
            ),
            "known:Ok(None)".to_string(),
            format!(
                "unknown:{:?}",
                Err::<(), _>((SystemExceptionKind::InvPolicy, Some(3u32)))
            ),
        ]
    );
    let component = profile
        .components
        .iter()
        .find(|c| c.tag == 0x5a44_0010)
        .unwrap();
    assert_eq!(component.component_data, vec![0, 0xab, 0xcd]);
    let (reply, _) = conn
        .invoke(&tagged, "ping", Endianness::Big, &string_argument("tagged"))
        .unwrap();
    assert_eq!(decode_string_body(&reply).unwrap(), "tagged");

    acceptor.shutdown();
    client_orb.destroy().unwrap();
    server_orb.destroy().unwrap();
}
