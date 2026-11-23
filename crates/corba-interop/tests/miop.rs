// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CORBA 3.3 Part 2 §11.2.4, §11.2.5, §11.3 through the ORB runtime: a
//! client sends oneway requests to a group over multicast, the
//! `PortableGroup::GOA` maps the group reference to object ids and
//! dispatches each request to every servant of the group, a gateway
//! forwards the requests of an IIOP client.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use zerodds_corba_rust::{marshaled_body, no_arguments};

use zerodds_corba_giop::{
    Message, Request, ResponseFlags, ServiceContextList, TargetAddress, TargetProfile, Version,
    encode_message,
};
use zerodds_corba_iiop::miop::MiopSender;
use zerodds_corba_iiop::{IiopProfileBody, IiopVersion};
use zerodds_corba_interop::miop::{
    Goa, GoaError, MiopClient, MiopGateway, gateway_object_key, two_way_reference,
};
use zerodds_corba_interop::runtime::IiopCorbaConnection;
use zerodds_corba_ior::miop::{GroupInfo, UipmcProfileBody, group_iiop_component};
use zerodds_corba_ior::{Ior, TaggedProfile};
use zerodds_corba_rust::{CorbaConnection, ObjectReference, SkeletonResult};
use zerodds_wire::Endianness;

/// A multicast port for one test of this process.
fn port(offset: u16) -> u16 {
    30_000 + (std::process::id() % 15_000) as u16 + offset
}

fn group_ref(address: &str, port: u16, domain: &str, id: u64) -> Ior {
    let uipmc = UipmcProfileBody::new(
        address,
        port,
        vec![GroupInfo::new(domain, id, 0).to_component(Endianness::Big)],
    );
    Ior::new(
        "IDL:Shop/Feed:1.0".into(),
        vec![uipmc.to_tagged_profile(Endianness::Big).unwrap()],
    )
}

type Calls = Arc<Mutex<Vec<(String, Vec<u8>)>>>;

fn recorder(goa: &Goa, oid: &[u8]) -> Calls {
    let calls: Calls = Arc::default();
    let c = Arc::clone(&calls);
    goa.activate_object_with_id(oid, move |op, body, _e| {
        c.lock()
            .unwrap()
            .push((op.to_string(), body.bytes().to_vec()));
        SkeletonResult::Reply(Vec::new())
    });
    calls
}

fn wait_for(calls: &Calls, n: usize) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while calls.lock().unwrap().len() < n && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        calls.lock().unwrap().len(),
        n,
        "{:?}",
        calls.lock().unwrap()
    );
}

fn quiet(calls: &Calls, n: usize) {
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(
        calls.lock().unwrap().len(),
        n,
        "{:?}",
        calls.lock().unwrap()
    );
}

#[test]
fn corba_p2_11_3_a_oneway_request_reaches_every_servant_of_the_group() {
    let group = group_ref("239.255.42.1", port(1), "Shop", 1);
    let goa = Goa::new();
    let a = goa.create_id_for_reference(&group).unwrap();
    goa.associate_reference_with_id(&group, b"b").unwrap();
    // A repeated association is ignored.
    goa.associate_reference_with_id(&group, b"b").unwrap();
    let (ca, cb) = (recorder(&goa, &a), recorder(&goa, b"b"));
    let mut ids = goa.reference_to_ids(&group).unwrap();
    ids.sort();
    let mut expected = vec![a.clone(), b"b".to_vec()];
    expected.sort();
    assert_eq!(ids, expected);

    let client = MiopClient::new();
    client
        .send_oneway(
            &group,
            "price",
            Endianness::Big,
            &marshaled_body(b"tick", 0),
        )
        .unwrap();
    // A request of many packets.
    let big: Vec<u8> = (0..20_000u32).map(|i| (i % 251) as u8).collect();
    client
        .send_oneway(&group, "bulk", Endianness::Little, &marshaled_body(&big, 0))
        .unwrap();
    wait_for(&ca, 2);
    wait_for(&cb, 2);
    assert_eq!(
        ca.lock().unwrap()[0],
        ("price".to_string(), b"tick".to_vec())
    );
    assert_eq!(ca.lock().unwrap()[1], ("bulk".to_string(), big.clone()));
    assert_eq!(goa.stats().dispatched, 4);
}

#[test]
fn corba_p2_11_2_4_2_a_disassociated_id_gets_nothing_and_can_be_associated_again() {
    let group = group_ref("239.255.42.2", port(2), "Shop", 2);
    let goa = Goa::new();
    let id = goa.create_id_for_reference(&group).unwrap();
    let calls = recorder(&goa, &id);
    let client = MiopClient::new();
    client
        .send_oneway(&group, "one", Endianness::Big, &marshaled_body(b"1", 0))
        .unwrap();
    wait_for(&calls, 1);
    goa.disassociate_reference_with_id(&group, &id).unwrap();
    // Disassociating twice is ignored.
    goa.disassociate_reference_with_id(&group, &id).unwrap();
    assert!(goa.reference_to_ids(&group).unwrap().is_empty());
    client
        .send_oneway(&group, "two", Endianness::Big, &marshaled_body(b"2", 0))
        .unwrap();
    quiet(&calls, 1);
    assert_eq!(goa.stats().no_member, 1);
    goa.associate_reference_with_id(&group, &id).unwrap();
    client
        .send_oneway(&group, "three", Endianness::Big, &marshaled_body(b"3", 0))
        .unwrap();
    wait_for(&calls, 2);
    assert_eq!(calls.lock().unwrap()[1].0, "three");
}

#[test]
fn corba_p2_11_2_4_1_a_reference_without_a_group_profile_is_not_a_group_object() {
    let plain = Ior::new(
        "IDL:X:1.0".into(),
        vec![
            TaggedProfile::iiop(
                &IiopProfileBody::new(IiopVersion::V1_2, "h".into(), 1, b"k".to_vec()),
                Endianness::Big,
            )
            .unwrap(),
        ],
    );
    let goa = Goa::new();
    assert_eq!(
        goa.create_id_for_reference(&plain),
        Err(GoaError::NotAGroupObject)
    );
    assert_eq!(goa.reference_to_ids(&plain), Err(GoaError::NotAGroupObject));
    assert_eq!(
        goa.associate_reference_with_id(&plain, b"x"),
        Err(GoaError::NotAGroupObject)
    );
    assert_eq!(
        goa.disassociate_reference_with_id(&plain, b"x"),
        Err(GoaError::NotAGroupObject)
    );
    // A UIPMC profile without a group component is no group reference either.
    let no_group = Ior::new(
        String::new(),
        vec![
            UipmcProfileBody::new("239.255.42.3", port(3), Vec::new())
                .to_tagged_profile(Endianness::Big)
                .unwrap(),
        ],
    );
    assert_eq!(
        goa.reference_to_ids(&no_group),
        Err(GoaError::NotAGroupObject)
    );
    assert!(
        MiopClient::new()
            .send_oneway(&plain, "x", Endianness::Big, &marshaled_body(b"", 0))
            .is_err()
    );
}

#[test]
fn corba_p2_11_3_1_2_giop_1_0_and_1_1_requests_carry_the_profile_in_the_object_key() {
    let group = group_ref("239.255.42.4", port(4), "Old", 4);
    let goa = Goa::new();
    let id = goa.create_id_for_reference(&group).unwrap();
    let calls = recorder(&goa, &id);
    for (n, version) in [Version::V1_0, Version::V1_1, Version::V1_2]
        .into_iter()
        .enumerate()
    {
        MiopClient::new()
            .with_giop_version(version)
            // One octet, written where the body of each version starts.
            .send_oneway(&group, "legacy", Endianness::Big, &|w| {
                w.write_u8(b'v');
                Ok(())
            })
            .unwrap();
        wait_for(&calls, n + 1);
    }
}

#[test]
fn corba_p2_11_2_6_one_endpoint_carries_two_groups() {
    let p = port(5);
    let first = group_ref("239.255.42.5", p, "Shop", 10);
    let second = group_ref("239.255.42.5", p, "Shop", 11);
    let goa = Goa::new();
    let a = goa.create_id_for_reference(&first).unwrap();
    let b = goa.create_id_for_reference(&second).unwrap();
    let (ca, cb) = (recorder(&goa, &a), recorder(&goa, &b));
    let client = MiopClient::new();
    client
        .send_oneway(&first, "to_first", Endianness::Big, &marshaled_body(b"", 0))
        .unwrap();
    client
        .send_oneway(
            &second,
            "to_second",
            Endianness::Big,
            &marshaled_body(b"", 0),
        )
        .unwrap();
    wait_for(&ca, 1);
    wait_for(&cb, 1);
    assert_eq!(ca.lock().unwrap()[0].0, "to_first");
    assert_eq!(cb.lock().unwrap()[0].0, "to_second");
}

#[test]
fn corba_p2_11_1_3_a_collection_that_is_no_oneway_request_is_counted_not_dispatched() {
    let group = group_ref("239.255.42.6", port(6), "Shop", 6);
    let goa = Goa::new();
    let id = goa.create_id_for_reference(&group).unwrap();
    let calls = recorder(&goa, &id);
    let profile = zerodds_corba_ior::miop::group_profiles(&group)
        .unwrap()
        .uipmc[0]
        .clone();
    let to = std::net::SocketAddr::new("239.255.42.6".parse().unwrap(), port(6));
    let sender = MiopSender::new(to.ip(), 1).unwrap();
    // A request that expects a reply.
    let two_way = encode_message(
        Version::V1_2,
        Endianness::Big,
        false,
        &Message::Request(Request {
            request_id: 1,
            response_flags: ResponseFlags::SYNC_WITH_TARGET,
            target: TargetAddress::Profile(TargetProfile {
                tag: 3,
                profile_data: profile.encode_encapsulation(Endianness::Big).unwrap(),
            }),
            operation: "ask".into(),
            requesting_principal: None,
            service_context: ServiceContextList::default(),
            body: Vec::new(),
        }),
    )
    .unwrap();
    sender.send(to, &two_way).unwrap();
    // A message that is no request, and octets that are no GIOP.
    let close = encode_message(
        Version::V1_2,
        Endianness::Big,
        false,
        &Message::CloseConnection(zerodds_corba_giop::CloseConnection),
    )
    .unwrap();
    sender.send(to, &close).unwrap();
    sender.send(to, b"not a GIOP message").unwrap();
    // A request for a group nobody here is in.
    let stranger = group_ref("239.255.42.6", port(6), "Shop", 99);
    MiopClient::new()
        .send_oneway(
            &stranger,
            "nobody",
            Endianness::Big,
            &marshaled_body(b"", 0),
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let s = goa.stats();
        if s.two_way == 1 && s.not_a_request == 2 && s.no_member == 1 {
            break;
        }
        assert!(Instant::now() < deadline, "{s:?}");
        std::thread::sleep(Duration::from_millis(20));
    }
    quiet(&calls, 0);
}

#[test]
fn corba_p2_11_2_5_the_gateway_forwards_the_requests_of_an_iiop_client() {
    let p = port(7);
    let uipmc = UipmcProfileBody::new(
        "239.255.42.7",
        p,
        vec![GroupInfo::new("Shop", 7, 0).to_component(Endianness::Big)],
    );
    let gateway = MiopGateway::new(MiopClient::new());
    let acceptor = gateway.serve("127.0.0.1:0".parse().unwrap()).unwrap();
    let gw_port = acceptor.listen_addr().port();
    // The group reference: the UIPMC profile and the gateway's IIOP
    // profile, whose object key names the group (§11.2.3.1).
    let gw_profile = IiopProfileBody::new(
        IiopVersion::V1_2,
        "127.0.0.1".into(),
        gw_port,
        gateway_object_key(&uipmc).unwrap(),
    );
    let group = Ior::new(
        "IDL:Shop/Feed:1.0".into(),
        vec![
            uipmc.to_tagged_profile(Endianness::Big).unwrap(),
            TaggedProfile::iiop(&gw_profile, Endianness::Big).unwrap(),
        ],
    );
    let goa = Goa::new();
    let id = goa.create_id_for_reference(&group).unwrap();
    let calls = recorder(&goa, &id);
    // An MIOP-unaware client uses the IIOP profile (§11.3.3.5).
    let unaware = two_way_reference(&group).unwrap();
    let client = IiopCorbaConnection::new();
    let (reply, _) = client
        .invoke(
            &unaware,
            "restock",
            Endianness::Big,
            &marshaled_body(b"crate", 0),
        )
        .unwrap();
    assert!(reply.is_empty());
    client
        .invoke_oneway(
            &unaware,
            "restock_later",
            Endianness::Big,
            &marshaled_body(b"box", 0),
        )
        .unwrap();
    wait_for(&calls, 2);
    assert_eq!(
        calls.lock().unwrap()[0],
        ("restock".to_string(), b"crate".to_vec())
    );
    // _non_existent of a group reference is true (§11.2.6.10).
    let (body, e) = client
        .invoke(&unaware, "_non_existent", Endianness::Big, &no_arguments)
        .unwrap();
    assert!(zerodds_wire::Reader::new(&body, e).read_bool().unwrap());
    // A key that names no group.
    let lost = ObjectReference {
        type_id: "IDL:X:1.0".into(),
        iiop_profile: IiopProfileBody::new(
            IiopVersion::V1_2,
            "127.0.0.1".into(),
            gw_port,
            b"x".to_vec(),
        )
        .encode_encapsulation(Endianness::Big)
        .unwrap(),
        policy_overrides: Vec::new(),
        profiles: Vec::new(),
    };
    assert!(
        client
            .invoke(&lost, "restock", Endianness::Big, &marshaled_body(b"", 0))
            .is_err()
    );
    quiet(&calls, 2);
}

#[test]
fn corba_p2_11_2_2_4_2_two_way_calls_use_the_group_iiop_profile() {
    let oma = IiopProfileBody::new(
        IiopVersion::V1_2,
        "oma.example".into(),
        1234,
        b"oma".to_vec(),
    );
    let uipmc = UipmcProfileBody::new(
        "239.255.42.8",
        port(8),
        vec![
            GroupInfo::new("Shop", 8, 0).to_component(Endianness::Big),
            group_iiop_component(&oma, Endianness::Big).unwrap(),
        ],
    );
    let group = Ior::new(
        "IDL:Shop/Feed:1.0".into(),
        vec![uipmc.to_tagged_profile(Endianness::Big).unwrap()],
    );
    let r = two_way_reference(&group).unwrap();
    assert_eq!(r.type_id, "IDL:Shop/Feed:1.0");
    let body = IiopProfileBody::decode_encapsulation(&r.iiop_profile).unwrap();
    assert_eq!((body.host.as_str(), body.port), ("oma.example", 1234));
    // Neither a group IIOP profile nor a gateway: no two-way reference.
    assert!(two_way_reference(&group_ref("239.255.42.8", port(8), "S", 1)).is_none());
}

#[test]
fn corba_p2_11_2_6_10_object_operations_on_a_group_reference() {
    use zerodds_corba_interop::runtime::CorbaServer;
    // The OMA of the group is served by an IIOP object.
    let server = CorbaServer::new();
    server.register(b"oma", |op, body, e| {
        assert_eq!(op, "_is_a");
        let id = body.reader(e).read_string().unwrap();
        let mut w = zerodds_wire::Writer::new(e);
        w.write_bool(id == "IDL:Shop/Base:1.0");
        SkeletonResult::Reply(w.finish())
    });
    let acceptor = server.serve("127.0.0.1:0".parse().unwrap()).unwrap();
    let oma = IiopProfileBody::new(
        IiopVersion::V1_2,
        "127.0.0.1".into(),
        acceptor.listen_addr().port(),
        b"oma".to_vec(),
    );
    let uipmc = UipmcProfileBody::new(
        "239.255.42.9",
        port(9),
        vec![
            GroupInfo::new("Shop", 9, 0).to_component(Endianness::Big),
            group_iiop_component(&oma, Endianness::Big).unwrap(),
        ],
    );
    let group = Ior::new(
        "IDL:Shop/Feed:1.0".into(),
        vec![uipmc.to_tagged_profile(Endianness::Big).unwrap()],
    );
    let client = MiopClient::new();
    let conn = IiopCorbaConnection::new();
    assert!(client.is_a(&group, "IDL:Shop/Feed:1.0", &conn).unwrap());
    assert!(client.is_a(&group, "IDL:Shop/Base:1.0", &conn).unwrap());
    assert!(!client.is_a(&group, "IDL:Other:1.0", &conn).unwrap());
    // Without a TAG_GROUP_IIOP component the client resolves the
    // interface itself: without an interface repository it cannot, and
    // answers false.
    let bare = group_ref("239.255.42.9", port(9), "Shop", 9);
    assert!(!client.is_a(&bare, "IDL:Shop/Base:1.0", &conn).unwrap());
    assert!(
        client
            .is_a(&bare, "IDL:omg.org/CORBA/Object:1.0", &conn)
            .unwrap()
    );
    // With the interface repository it follows the base interfaces of
    // the type of the reference.
    let resolving = MiopClient::new().with_interface_repository(Arc::new(shop_repository()));
    assert!(resolving.is_a(&bare, "IDL:Shop/Base:1.0", &conn).unwrap());
    assert!(!resolving.is_a(&bare, "IDL:Other:1.0", &conn).unwrap());
    // With a TAG_GROUP_IIOP component the object behind it answers, not
    // the repository.
    assert!(!resolving.is_a(&group, "IDL:Shop/Mid:1.0", &conn).unwrap());
    assert!(client.non_existent(&group));
    assert!(client.validate_connection(&group));
    let unicast = Ior::new(
        String::new(),
        vec![
            UipmcProfileBody::new(
                "10.0.0.1",
                1,
                vec![GroupInfo::new("Shop", 9, 0).to_component(Endianness::Big)],
            )
            .to_tagged_profile(Endianness::Big)
            .unwrap(),
        ],
    );
    assert!(!client.validate_connection(&unicast));
}

/// `Shop::Feed : Shop::Mid : Shop::Base` in an interface repository.
fn shop_repository() -> zerodds_corba_ir::Repository {
    use zerodds_corba_ir::{DefDetails, Definition, DefinitionKind, Repository};
    let iface = |id: &str, name: &str, bases: &[&str]| {
        Definition::new(id, name, "1.0", DefinitionKind::Interface).with_details(
            DefDetails::Interface {
                base_interfaces: bases.iter().map(ToString::to_string).collect(),
                is_abstract: false,
            },
        )
    };
    let mut r = Repository::new();
    r.register(
        Definition::new("IDL:Shop:1.0", "Shop", "1.0", DefinitionKind::Module)
            .with_content(iface("IDL:Shop/Base:1.0", "Base", &[]))
            .with_content(iface("IDL:Shop/Mid:1.0", "Mid", &["IDL:Shop/Base:1.0"]))
            .with_content(iface("IDL:Shop/Feed:1.0", "Feed", &["IDL:Shop/Mid:1.0"])),
    )
    .unwrap();
    r
}

/// A gateway on a free port and the reference an MIOP-unaware client
/// uses for the group of `uipmc` (the gateway's IIOP profile).
fn gateway_for(
    uipmc: &UipmcProfileBody,
) -> (MiopGateway, zerodds_corba_iiop::Acceptor, ObjectReference) {
    let gateway = MiopGateway::new(MiopClient::new());
    let acceptor = gateway.serve("127.0.0.1:0".parse().unwrap()).unwrap();
    let gw_profile = IiopProfileBody::new(
        IiopVersion::V1_2,
        "127.0.0.1".into(),
        acceptor.listen_addr().port(),
        gateway_object_key(uipmc).unwrap(),
    );
    let reference = ObjectReference {
        type_id: "IDL:Shop/Feed:1.0".into(),
        iiop_profile: gw_profile.encode_encapsulation(Endianness::Big).unwrap(),
        policy_overrides: Vec::new(),
        profiles: Vec::new(),
    };
    (gateway, acceptor, reference)
}

fn read_frame(s: &mut std::net::TcpStream) -> Vec<u8> {
    use std::io::Read;
    let mut header = [0u8; 12];
    s.read_exact(&mut header).unwrap();
    let size = if header[6] & 1 == 0 {
        u32::from_be_bytes([header[8], header[9], header[10], header[11]])
    } else {
        u32::from_le_bytes([header[8], header[9], header[10], header[11]])
    };
    let mut out = header.to_vec();
    let mut body = vec![0u8; size as usize];
    s.read_exact(&mut body).unwrap();
    out.extend_from_slice(&body);
    out
}

/// Sends `frame` on a new connection to `at`; the gateway answers with
/// `MessageError` and closes the connection.
fn assert_message_error(at: std::net::SocketAddr, frame: &[u8]) {
    use std::io::{Read, Write};
    let mut s = std::net::TcpStream::connect(at).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    s.write_all(frame).unwrap();
    let answer = read_frame(&mut s);
    assert_eq!(&answer[..4], b"GIOP");
    assert_eq!(
        answer[7],
        zerodds_corba_giop::MessageType::MessageError.as_u8()
    );
    let mut rest = Vec::new();
    s.read_to_end(&mut rest).unwrap();
    assert!(rest.is_empty(), "the connection is closed");
}

#[test]
fn corba_p2_11_2_5_the_gateway_answers_locate_requests_and_refuses_what_it_cannot_take() {
    use std::io::Write;
    use zerodds_corba_giop::{
        CancelRequest, LocateRequest, LocateStatusType, Reply, ReplyStatusType,
    };
    use zerodds_corba_interop::runtime::LocateOutcome;
    let uipmc = UipmcProfileBody::new(
        "239.255.42.10",
        port(10),
        vec![GroupInfo::new("Shop", 10, 0).to_component(Endianness::Big)],
    );
    let (gateway, acceptor, reference) = gateway_for(&uipmc);
    let at = acceptor.listen_addr();
    let client = IiopCorbaConnection::new();
    // A target whose UIPMC profile names a group is here.
    assert_eq!(
        client.locate(&reference).unwrap(),
        LocateOutcome::Here(reference.clone())
    );
    // A key that names no group is unknown.
    let lost = ObjectReference {
        type_id: "IDL:X:1.0".into(),
        iiop_profile: IiopProfileBody::new(
            IiopVersion::V1_2,
            "127.0.0.1".into(),
            at.port(),
            b"x".to_vec(),
        )
        .encode_encapsulation(Endianness::Big)
        .unwrap(),
        policy_overrides: Vec::new(),
        profiles: Vec::new(),
    };
    assert_eq!(client.locate(&lost).unwrap(), LocateOutcome::Unknown);
    assert_eq!(gateway.stats().locate_requests, 2);
    // A CancelRequest is counted and the connection stays open: the
    // LocateRequest after it is answered.
    let mut s = std::net::TcpStream::connect(at).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let cancel = Message::CancelRequest(CancelRequest { request_id: 7 });
    s.write_all(&encode_message(Version::V1_2, Endianness::Big, false, &cancel).unwrap())
        .unwrap();
    let locate = Message::LocateRequest(LocateRequest {
        request_id: 8,
        target: TargetAddress::Key(gateway_object_key(&uipmc).unwrap()),
    });
    s.write_all(&encode_message(Version::V1_2, Endianness::Big, false, &locate).unwrap())
        .unwrap();
    let (answer, _) = zerodds_corba_giop::decode_message(&read_frame(&mut s)).unwrap();
    let Message::LocateReply(lr) = answer else {
        panic!("{answer:?}")
    };
    assert_eq!(
        (lr.request_id, lr.locate_status),
        (8, LocateStatusType::ObjectHere)
    );
    assert_eq!(gateway.stats().cancel_requests, 1);
    // A header the gateway cannot read, and a Reply sent to it: a
    // MessageError, then the connection ends.
    assert_message_error(at, b"GIOX\x01\x02\x00\x00\x00\x00\x00\x00");
    let reply = Message::Reply(Reply {
        request_id: 1,
        reply_status: ReplyStatusType::NoException,
        service_context: ServiceContextList::default(),
        body: Vec::new(),
    });
    assert_message_error(
        at,
        &encode_message(Version::V1_2, Endianness::Big, false, &reply).unwrap(),
    );
    assert_eq!(gateway.stats().refused, 2);
}

#[test]
fn corba_p2_11_2_6_10_the_gateway_forwards_object_operations_to_the_group_iiop_profile() {
    use zerodds_corba_interop::runtime::CorbaServer;
    // The OMA of the group, served by an IIOP object.
    let server = CorbaServer::new();
    server.register(b"oma", |op, body, e| {
        zerodds_corba_rust::runtime::dispatch_object_operation(
            op,
            body,
            e,
            "IDL:Shop/Feed:1.0",
            &["IDL:Shop/Base:1.0"],
        )
        .unwrap_or(SkeletonResult::BadOperation)
    });
    let oma_acceptor = server.serve("127.0.0.1:0".parse().unwrap()).unwrap();
    let oma = IiopProfileBody::new(
        IiopVersion::V1_2,
        "127.0.0.1".into(),
        oma_acceptor.listen_addr().port(),
        b"oma".to_vec(),
    );
    let with_oma = UipmcProfileBody::new(
        "239.255.42.11",
        port(11),
        vec![
            GroupInfo::new("Shop", 11, 0).to_component(Endianness::Big),
            group_iiop_component(&oma, Endianness::Big).unwrap(),
        ],
    );
    let (gateway, _acceptor, unaware) = gateway_for(&with_oma);
    let client = IiopCorbaConnection::new();
    let is_a = |reference: &ObjectReference, id: &str| {
        let mut w = zerodds_wire::Writer::new(Endianness::Little);
        w.write_string(id);
        client
            .invoke(
                reference,
                "_is_a",
                Endianness::Little,
                &marshaled_body(&w.finish(), 0),
            )
            .map(|(body, e)| zerodds_wire::Reader::new(&body, e).read_bool().unwrap())
    };
    assert!(is_a(&unaware, "IDL:Shop/Base:1.0").unwrap());
    assert!(!is_a(&unaware, "IDL:Other:1.0").unwrap());
    let (body, e) = client
        .invoke(&unaware, "_non_existent", Endianness::Big, &no_arguments)
        .unwrap();
    assert!(!zerodds_wire::Reader::new(&body, e).read_bool().unwrap());
    let (body, e) = client
        .invoke(&unaware, "_repository_id", Endianness::Big, &no_arguments)
        .unwrap();
    assert_eq!(
        zerodds_wire::Reader::new(&body, e).read_string().unwrap(),
        "IDL:Shop/Feed:1.0"
    );
    assert_eq!(gateway.stats().object_operations, 4);
    // Without the component: `_non_existent` is true, the others need the
    // type of the group and are NO_IMPLEMENT.
    let bare = UipmcProfileBody::new(
        "239.255.42.11",
        port(11),
        vec![GroupInfo::new("Shop", 12, 0).to_component(Endianness::Big)],
    );
    let (_gateway, _acceptor, unaware) = gateway_for(&bare);
    let err = is_a(&unaware, "IDL:Shop/Base:1.0").unwrap_err();
    assert!(
        matches!(
            &err,
            zerodds_corba_rust::CorbaException::System(e)
                if e.kind == zerodds_corba_rust::SystemExceptionKind::NoImplement
        ),
        "{err:?}"
    );
    let (body, e) = client
        .invoke(&unaware, "_non_existent", Endianness::Big, &no_arguments)
        .unwrap();
    assert!(zerodds_wire::Reader::new(&body, e).read_bool().unwrap());
}

/// Part 2 §7.7.2: received service contexts "must be passed on through
/// a bridge". An object operation the gateway forwards over the group's
/// IIOP profile carries the contexts of the client's request, a vendor
/// context and one of GIOP 1.4 among them, and the reply contexts of the
/// object come back to the client.
#[test]
fn corba_p2_7_7_2_the_gateway_passes_service_contexts_on_and_back() {
    use zerodds_corba_ccm::portable_interceptor::{
        InitializerError, OrbInitInfo, OrbInitializer, PointResult, RequestInfo,
        ServerRequestInterceptor, register_orb_initializer,
    };
    use zerodds_corba_giop::{Reply, ServiceContext};
    use zerodds_corba_iiop::{Connector, ConnectorConfig};
    use zerodds_corba_interop::runtime::{CorbaServer, orb_init};

    const VENDOR_CTX: u32 = 0x4a43_0001;
    const RMI_CUSTOM: u32 = 17;
    const REPLY_CTX: u32 = 0x4a43_0002;

    struct ReplyTagger;
    impl ServerRequestInterceptor for ReplyTagger {
        fn name(&self) -> &str {
            "oma-reply-tagger"
        }
        fn send_reply(&self, info: &mut RequestInfo) -> PointResult {
            let seen = info.get_request_service_context(VENDOR_CTX)?.to_vec();
            info.add_reply_service_context(REPLY_CTX, seen, false)?;
            Ok(())
        }
    }
    struct Init;
    impl OrbInitializer for Init {
        fn pre_init(&self, info: &mut OrbInitInfo<'_>) -> Result<(), InitializerError> {
            if info.orb_id() == "miop-oma" {
                info.add_server_request_interceptor(Arc::new(ReplyTagger))?;
            }
            Ok(())
        }
    }
    register_orb_initializer(Arc::new(Init));
    let server = CorbaServer::new();
    server.set_orb(orb_init(&mut Vec::new(), "miop-oma").unwrap());
    server.register(b"oma", |op, body, e| {
        zerodds_corba_rust::runtime::dispatch_object_operation(
            op,
            body,
            e,
            "IDL:Shop/Feed:1.0",
            &["IDL:Shop/Base:1.0"],
        )
        .unwrap_or(SkeletonResult::BadOperation)
    });
    let received = Arc::new(Mutex::new(Vec::new()));
    {
        let received = received.clone();
        server.on_request_contexts(move |list| received.lock().unwrap().push(list.0.clone()));
    }
    let oma_acceptor = server.serve("127.0.0.1:0".parse().unwrap()).unwrap();
    let oma = IiopProfileBody::new(
        IiopVersion::V1_2,
        "127.0.0.1".into(),
        oma_acceptor.listen_addr().port(),
        b"oma".to_vec(),
    );
    let uipmc = UipmcProfileBody::new(
        "239.255.42.11",
        port(13),
        vec![
            GroupInfo::new("Shop", 13, 0).to_component(Endianness::Big),
            group_iiop_component(&oma, Endianness::Big).unwrap(),
        ],
    );
    let (_gateway, acceptor, _reference) = gateway_for(&uipmc);
    let addr = acceptor.listen_addr();
    let connector = Connector::new(ConnectorConfig::default());
    let mut pooled = connector
        .connect(&addr.ip().to_string(), addr.port())
        .unwrap();
    let conn = pooled.connection().unwrap();
    let mut w = zerodds_wire::Writer::new(Endianness::Big);
    w.write_string("IDL:Shop/Base:1.0");
    let request = Message::Request(Request {
        request_id: 5,
        response_flags: ResponseFlags::SYNC_WITH_TARGET,
        target: TargetAddress::Key(gateway_object_key(&uipmc).unwrap()),
        operation: "_is_a".into(),
        requesting_principal: None,
        service_context: ServiceContextList(vec![
            ServiceContext::new(VENDOR_CTX, b"from-client".to_vec()),
            ServiceContext::new(RMI_CUSTOM, vec![0, 2]),
        ]),
        body: w.finish(),
    });
    conn.write_message(Version::V1_2, Endianness::Big, false, &request)
        .unwrap();
    let reply: Reply = match conn.read_message_full().unwrap().0 {
        Message::Reply(r) => r,
        other => panic!("{other:?}"),
    };
    assert_eq!(
        reply.reply_status,
        zerodds_corba_giop::ReplyStatusType::NoException
    );
    assert!(
        zerodds_wire::Reader::new(&reply.body, Endianness::Big)
            .read_bool()
            .unwrap()
    );
    let back = reply
        .service_context
        .0
        .iter()
        .find(|c| c.context_id == REPLY_CTX)
        .expect("the reply context of the object");
    assert_eq!(back.context_data, b"from-client");
    let received = received.lock().unwrap();
    assert_eq!(received.len(), 1);
    let at_oma: Vec<(u32, Vec<u8>)> = received[0]
        .iter()
        .map(|c| (c.context_id, c.context_data.clone()))
        .collect();
    assert!(at_oma.contains(&(VENDOR_CTX, b"from-client".to_vec())));
    assert!(at_oma.contains(&(RMI_CUSTOM, vec![0, 2])));
}

/// CORBA 3.3 Part 2 §7.7.2: a `ServiceContextList` is a sequence, and a
/// bridge passes on what it received. Two contexts with one vendor id
/// in the client's request both reach the object behind the gateway, in
/// their order and beside the other contexts.
#[test]
fn corba_p2_7_7_2_the_gateway_passes_two_contexts_of_one_id() {
    use zerodds_corba_giop::{Reply, ServiceContext};
    use zerodds_corba_iiop::{Connector, ConnectorConfig};
    use zerodds_corba_interop::runtime::CorbaServer;

    const VENDOR_CTX: u32 = 0x4a43_0011;
    const OTHER_CTX: u32 = 0x4a43_0012;

    let server = CorbaServer::new();
    server.register(b"oma-twice", |op, body, e| {
        zerodds_corba_rust::runtime::dispatch_object_operation(
            op,
            body,
            e,
            "IDL:Shop/Feed:1.0",
            &["IDL:Shop/Base:1.0"],
        )
        .unwrap_or(SkeletonResult::BadOperation)
    });
    let received = Arc::new(Mutex::new(Vec::new()));
    {
        let received = received.clone();
        server.on_request_contexts(move |list| received.lock().unwrap().push(list.0.clone()));
    }
    let oma_acceptor = server.serve("127.0.0.1:0".parse().unwrap()).unwrap();
    let oma = IiopProfileBody::new(
        IiopVersion::V1_2,
        "127.0.0.1".into(),
        oma_acceptor.listen_addr().port(),
        b"oma-twice".to_vec(),
    );
    let uipmc = UipmcProfileBody::new(
        "239.255.42.12",
        port(14),
        vec![
            GroupInfo::new("Shop", 14, 0).to_component(Endianness::Big),
            group_iiop_component(&oma, Endianness::Big).unwrap(),
        ],
    );
    let (_gateway, acceptor, _reference) = gateway_for(&uipmc);
    let addr = acceptor.listen_addr();
    let connector = Connector::new(ConnectorConfig::default());
    let mut pooled = connector
        .connect(&addr.ip().to_string(), addr.port())
        .unwrap();
    let conn = pooled.connection().unwrap();
    let mut w = zerodds_wire::Writer::new(Endianness::Big);
    w.write_string("IDL:Shop/Base:1.0");
    let sent = vec![
        ServiceContext::new(VENDOR_CTX, b"first".to_vec()),
        ServiceContext::new(OTHER_CTX, b"between".to_vec()),
        ServiceContext::new(VENDOR_CTX, b"second".to_vec()),
    ];
    let request = Message::Request(Request {
        request_id: 6,
        response_flags: ResponseFlags::SYNC_WITH_TARGET,
        target: TargetAddress::Key(gateway_object_key(&uipmc).unwrap()),
        operation: "_is_a".into(),
        requesting_principal: None,
        service_context: ServiceContextList(sent.clone()),
        body: w.finish(),
    });
    conn.write_message(Version::V1_2, Endianness::Big, false, &request)
        .unwrap();
    let reply: Reply = match conn.read_message_full().unwrap().0 {
        Message::Reply(r) => r,
        other => panic!("{other:?}"),
    };
    assert_eq!(
        reply.reply_status,
        zerodds_corba_giop::ReplyStatusType::NoException
    );
    let received = received.lock().unwrap();
    assert_eq!(received.len(), 1);
    let at_oma: Vec<ServiceContext> = received[0]
        .iter()
        .filter(|c| c.context_id == VENDOR_CTX || c.context_id == OTHER_CTX)
        .cloned()
        .collect();
    assert_eq!(at_oma, sent);
}

/// A peer that answers every request as another ORB's object would: in
/// the request's version, with a reply context of four octets and a
/// `double` body aligned from its own message start; the offsets of the
/// bodies it sent go to `offsets`.
fn offset_peer(offsets: Arc<Mutex<Vec<usize>>>) -> std::net::SocketAddr {
    use std::io::{Read, Write};
    use zerodds_corba_giop::{Reply, ReplyStatusType, ServiceContext, decode_message_ctx};
    // One message, or `None` once the gateway closes the connection.
    fn next_frame(s: &mut std::net::TcpStream) -> Option<Vec<u8>> {
        let mut header = [0u8; 12];
        s.read_exact(&mut header).ok()?;
        let size = if header[6] & 1 == 0 {
            u32::from_be_bytes([header[8], header[9], header[10], header[11]])
        } else {
            u32::from_le_bytes([header[8], header[9], header[10], header[11]])
        };
        let mut out = header.to_vec();
        out.resize(12 + size as usize, 0);
        s.read_exact(&mut out[12..]).ok()?;
        Some(out)
    }
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let at = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else {
                break;
            };
            let offsets = Arc::clone(&offsets);
            std::thread::spawn(move || {
                while let Some(frame) = next_frame(&mut s) {
                    let version = Version::new(frame[4], frame[5]);
                    let Ok((Message::Request(request), e, _)) = decode_message_ctx(&frame) else {
                        continue;
                    };
                    let reply = Message::Reply(Reply {
                        request_id: request.request_id,
                        reply_status: ReplyStatusType::NoException,
                        service_context: ServiceContextList(vec![ServiceContext::new(
                            0x4f4d_0001,
                            vec![1, 2, 3, 4],
                        )]),
                        body: Vec::new(),
                    });
                    let mut out = encode_message(version, e, false, &reply).unwrap();
                    let offset = out
                        .len()
                        .next_multiple_of(if version.minor >= 2 { 8 } else { 1 });
                    out.resize(offset, 0);
                    offsets.lock().unwrap().push(offset);
                    let mut w = zerodds_corba_giop::body_writer(offset, e);
                    w.write_f64(1.000_000_000_000_2);
                    out.extend_from_slice(&zerodds_corba_giop::finish_body(w, offset));
                    let size = u32::try_from(out.len() - 12).unwrap();
                    let size = match e {
                        Endianness::Big => size.to_be_bytes(),
                        Endianness::Little => size.to_le_bytes(),
                    };
                    out[8..12].copy_from_slice(&size);
                    if s.write_all(&out).is_err() {
                        break;
                    }
                }
            });
        }
    });
    at
}

/// CORBA 3.3 Part 2 §9.4.3 through the gateway: the reply body of the
/// group's object goes back to the client on the offset modulo eight it
/// stood at, where its `double` stays aligned from the message start. A
/// GIOP 1.2 object's body (on eight) is moved onto eight of the client's
/// GIOP 1.1 reply, whose passed-back context would leave it on four; a
/// GIOP 1.1 object's body on four stays on four in a GIOP 1.0 reply; a
/// GIOP 1.2 reply cannot carry a body on four and is `MARSHAL`,
/// `COMPLETED_YES`.
#[test]
fn corba_p2_9_4_3_the_gateway_relays_a_reply_body_on_its_offset_modulo_eight() {
    use zerodds_corba_giop::{ReplyStatusType, decode_message_ctx};
    let double = 1.000_000_000_000_2f64.to_be_bytes();
    let offsets = Arc::new(Mutex::new(Vec::new()));
    let peer = offset_peer(Arc::clone(&offsets));
    for (object, client, expect) in [
        (IiopVersion::V1_2, Version::V1_1, Some(0)),
        (IiopVersion::V1_1, Version::V1_0, Some(4)),
        (IiopVersion::V1_1, Version::V1_2, None),
    ] {
        let oma = IiopProfileBody::new(object, "127.0.0.1".into(), peer.port(), b"oma".to_vec());
        let uipmc = UipmcProfileBody::new(
            "239.255.42.15",
            port(15),
            vec![
                GroupInfo::new("Shop", 15, 0).to_component(Endianness::Big),
                group_iiop_component(&oma, Endianness::Big).unwrap(),
            ],
        );
        let (_gateway, acceptor, _) = gateway_for(&uipmc);
        let request = Message::Request(Request {
            request_id: 7,
            response_flags: ResponseFlags::SYNC_WITH_TARGET,
            target: TargetAddress::Key(gateway_object_key(&uipmc).unwrap()),
            operation: "_non_existent".into(),
            requesting_principal: Some(Vec::new()),
            service_context: ServiceContextList::default(),
            body: Vec::new(),
        });
        let mut s = std::net::TcpStream::connect(acceptor.listen_addr()).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        std::io::Write::write_all(
            &mut s,
            &encode_message(client, Endianness::Big, false, &request).unwrap(),
        )
        .unwrap();
        let frame = read_frame(&mut s);
        let (Message::Reply(reply), _, _) = decode_message_ctx(&frame).unwrap() else {
            panic!("{object:?} -> {client:?}: no reply");
        };
        let case = format!("object {object:?}, client {client:?}");
        match expect {
            Some(residue) => {
                assert_eq!(reply.reply_status, ReplyStatusType::NoException, "{case}");
                let at = reply.body_offset(client, Endianness::Big).unwrap();
                assert_eq!(at % 8, residue, "{case}: body at {at}");
                let value = frame
                    .windows(8)
                    .position(|w| w == double)
                    .unwrap_or_else(|| panic!("{case}: no double"));
                assert_eq!(value % 8, 0, "{case}: double at {value}");
            }
            None => {
                assert_eq!(
                    reply.reply_status,
                    ReplyStatusType::SystemException,
                    "{case}"
                );
                let e = zerodds_corba_rt::SystemException::decode(&mut zerodds_wire::Reader::new(
                    &reply.body,
                    Endianness::Big,
                ))
                .unwrap();
                assert_eq!(
                    e.kind,
                    zerodds_corba_rt::SystemExceptionKind::Marshal,
                    "{case}"
                );
                assert_eq!(
                    e.completed,
                    zerodds_corba_rt::CompletionStatus::Yes,
                    "{case}"
                );
            }
        }
    }
    // The peer's own bodies stood on eight (GIOP 1.2) and on four.
    let offsets = offsets.lock().unwrap().clone();
    assert_eq!(
        offsets.iter().map(|o| o % 8).collect::<Vec<_>>(),
        [0, 4, 4],
        "{offsets:?}"
    );
}
