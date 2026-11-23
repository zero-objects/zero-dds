// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! CosTime over IIOP (OMG Time Service 1.1 §1.2): a GIOP client on a TCP
//! connection to [`TimeServer`] gets UTO and TIO references and calls
//! them, in both byte orders. Feature `serve`.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::net::SocketAddr;

use zerodds_corba_giop::{
    LocateRequest, LocateStatusType, Message, Reply, ReplyStatusType, Request, ResponseFlags,
    ServiceContextList, TargetAddress, Version, encode_message,
};
use zerodds_corba_iiop::{
    Acceptor, AcceptorConfig, Connection, Connector, ConnectorConfig, IiopProfileBody, IiopVersion,
};
use zerodds_corba_ior::{Ior, TaggedProfile};
use zerodds_time_service::proxy::TimeServiceProxy;
use zerodds_time_service::servant::{
    RemoteTimePolicy, TIME_SERVICE_OBJECT_KEY, TIME_UNAVAILABLE_TYPE_ID, TIO_TYPE_ID, UTO_TYPE_ID,
};
use zerodds_time_service::server::TimeServer;
use zerodds_time_service::{AbsoluteTimeError, TimeService, TimeUnavailable, Uto};
use zerodds_wire::{Endianness, Reader, Writer};

/// Repository id of the system exception of a refused current time.
const NO_PERMISSION_ID: &str = "IDL:omg.org/CORBA/NO_PERMISSION:1.0";

fn loopback() -> SocketAddr {
    "127.0.0.1:0".parse().unwrap()
}

fn server(service: TimeService) -> TimeServer {
    TimeServer::start(service, loopback(), String::from("127.0.0.1")).unwrap()
}

/// A server that answers `universal_time` (App. B.2).
fn answering_server(service: TimeService) -> TimeServer {
    TimeServer::start_with_policy(
        service,
        loopback(),
        String::from("127.0.0.1"),
        RemoteTimePolicy::AnswerCorrectingProxies,
    )
    .unwrap()
}

/// One GIOP 1.2 request on `conn`, the reply status and body.
fn call(
    conn: &mut Connection,
    endianness: Endianness,
    request_id: u32,
    key: &[u8],
    operation: &str,
    body: Vec<u8>,
) -> (ReplyStatusType, Vec<u8>) {
    call_v(
        conn,
        Version::V1_2,
        endianness,
        request_id,
        key,
        operation,
        body,
    )
}

fn request(request_id: u32, key: &[u8], operation: &str) -> Request {
    Request::new(
        request_id,
        ResponseFlags::SYNC_WITH_TARGET,
        TargetAddress::Key(key.to_vec()),
        operation.to_string(),
    )
}

/// One request in GIOP `version` on `conn`, the reply status and body.
fn call_v(
    conn: &mut Connection,
    version: Version,
    endianness: Endianness,
    request_id: u32,
    key: &[u8],
    operation: &str,
    body: Vec<u8>,
) -> (ReplyStatusType, Vec<u8>) {
    let mut request = request(request_id, key, operation);
    request.body = body;
    conn.write_message(version, endianness, false, &Message::Request(request))
        .unwrap();
    let (msg, reply_endianness, _) = conn.read_message_full().unwrap();
    assert_eq!(reply_endianness, endianness);
    match msg {
        Message::Reply(r) => {
            assert_eq!(r.request_id, request_id);
            (r.reply_status, r.body)
        }
        other => panic!("expected a Reply, got {other:?}"),
    }
}

fn iiop(ior: &Ior) -> IiopProfileBody {
    ior.profiles
        .iter()
        .find_map(TaggedProfile::as_iiop)
        .unwrap()
        .unwrap()
}

fn decode_ior(body: &[u8], e: Endianness) -> Ior {
    Ior::decode(&mut Reader::new(body, e)).unwrap()
}

fn read_repo_id(body: &[u8], e: Endianness) -> String {
    Reader::new(body, e).read_string().unwrap()
}

#[test]
fn time_1_2_a_giop_client_gets_a_uto_reference_and_reads_it() {
    let srv = answering_server(TimeService::with_source(|| 138_000_000_000_000_000).with_tdf(60));
    let port = srv.listen_addr().port();
    assert_eq!(
        srv.corbaloc("127.0.0.1"),
        format!("corbaloc::127.0.0.1:{port}/TimeService")
    );
    let service_ior = srv.service_ior().unwrap();
    assert_eq!(iiop(&service_ior).object_key, TIME_SERVICE_OBJECT_KEY);
    let connector = Connector::new(ConnectorConfig::default());
    let mut pooled = connector.connect("127.0.0.1", port).unwrap();
    let conn = pooled.connection().unwrap();
    for (n, e) in [Endianness::Little, Endianness::Big]
        .into_iter()
        .enumerate()
    {
        let id = 10 * n as u32;
        let (status, body) = call(
            conn,
            e,
            id + 1,
            TIME_SERVICE_OBJECT_KEY,
            "universal_time",
            Vec::new(),
        );
        assert_eq!(status, ReplyStatusType::NoException);
        let uto = decode_ior(&body, e);
        assert_eq!(uto.type_id, UTO_TYPE_ID);
        let profile = iiop(&uto);
        assert_eq!((profile.host.as_str(), profile.port), ("127.0.0.1", port));
        // The UTO is an object: its attributes over the same connection.
        let (status, body) = call(
            conn,
            e,
            id + 2,
            &profile.object_key,
            "_get_utc_time",
            Vec::new(),
        );
        assert_eq!(status, ReplyStatusType::NoException);
        let mut r = Reader::new(&body, e);
        assert_eq!(r.read_u64().unwrap(), 138_000_000_000_000_000);
        assert_eq!(r.read_u32().unwrap(), 0);
        assert_eq!(r.read_u16().unwrap(), 0);
        assert_eq!(r.read_i16().unwrap(), 60);
        let mut w = Writer::new(e);
        w.write_string(UTO_TYPE_ID);
        let (status, body) = call(conn, e, id + 3, &profile.object_key, "_is_a", w.finish());
        assert_eq!(status, ReplyStatusType::NoException);
        assert!(Reader::new(&body, e).read_bool().unwrap());
        // interval() returns a TIO reference.
        let (status, body) = call(conn, e, id + 4, &profile.object_key, "interval", Vec::new());
        assert_eq!(status, ReplyStatusType::NoException);
        assert_eq!(decode_ior(&body, e).type_id, TIO_TYPE_ID);
    }
}

#[test]
fn time_1_2_locate_request_finds_the_service_object() {
    let srv = server(TimeService::default());
    let connector = Connector::new(ConnectorConfig::default());
    let mut pooled = connector
        .connect("127.0.0.1", srv.listen_addr().port())
        .unwrap();
    let conn = pooled.connection().unwrap();
    for (id, key, want) in [
        (
            1,
            TIME_SERVICE_OBJECT_KEY.to_vec(),
            LocateStatusType::ObjectHere,
        ),
        (2, b"NameService".to_vec(), LocateStatusType::UnknownObject),
    ] {
        let lr = Message::LocateRequest(LocateRequest {
            request_id: id,
            target: TargetAddress::Key(key),
        });
        conn.write_message(Version::V1_2, Endianness::Little, false, &lr)
            .unwrap();
        match conn.read_message_full().unwrap().0 {
            Message::LocateReply(r) => assert_eq!(r.locate_status, want),
            other => panic!("{other:?}"),
        }
    }
}

#[test]
fn time_1_2_exceptions_reach_the_client() {
    // TimeUnavailable is the CosTime user exception; BAD_PARAM a system
    // exception.
    let srv = answering_server(TimeService::with_source(|| 0));
    let connector = Connector::new(ConnectorConfig::default());
    let mut pooled = connector
        .connect("127.0.0.1", srv.listen_addr().port())
        .unwrap();
    let conn = pooled.connection().unwrap();
    let e = Endianness::Big;
    let (status, body) = call(
        conn,
        e,
        1,
        TIME_SERVICE_OBJECT_KEY,
        "universal_time",
        Vec::new(),
    );
    assert_eq!(status, ReplyStatusType::UserException);
    assert_eq!(read_repo_id(&body, e), TIME_UNAVAILABLE_TYPE_ID);
    let mut w = Writer::new(e);
    w.write_u64(9);
    w.write_u64(1);
    let (status, body) = call(
        conn,
        e,
        2,
        TIME_SERVICE_OBJECT_KEY,
        "new_interval",
        w.finish(),
    );
    assert_eq!(status, ReplyStatusType::SystemException);
    assert_eq!(read_repo_id(&body, e), "IDL:omg.org/CORBA/BAD_PARAM:1.0");
    let (status, body) = call(conn, e, 3, b"Unknown", "universal_time", Vec::new());
    assert_eq!(status, ReplyStatusType::SystemException);
    assert_eq!(
        read_repo_id(&body, e),
        "IDL:omg.org/CORBA/OBJECT_NOT_EXIST:1.0"
    );
}

/// A server of one foreign UTO (object key `foreign-uto`) that answers
/// `_get_utc_time` with `utc`, as another vendor's Time Service would.
fn foreign_uto_server(time: u64, inaccuracy: u32) -> Acceptor {
    Acceptor::start(AcceptorConfig::new(loopback()), move |mut conn| {
        while let Ok((msg, e, v)) = conn.read_message_full() {
            let Message::Request(req) = msg else { continue };
            let ok = matches!(&req.target, TargetAddress::Key(k) if k == b"foreign-uto")
                && req.operation == "_get_utc_time";
            let mut w = Writer::new(e);
            let status = if ok {
                w.write_u64(time);
                w.write_u32(inaccuracy);
                w.write_u16(0);
                w.write_i16(0);
                ReplyStatusType::NoException
            } else {
                w.write_string("IDL:omg.org/CORBA/BAD_OPERATION:1.0");
                w.write_u32(0);
                w.write_u32(1);
                ReplyStatusType::SystemException
            };
            let reply = Message::Reply(Reply {
                request_id: req.request_id,
                reply_status: status,
                service_context: ServiceContextList::default(),
                body: w.finish(),
            });
            if conn.write_message(v, e, false, &reply).is_err() {
                return;
            }
        }
    })
    .unwrap()
}

#[test]
fn time_1_2_a_uto_of_another_server_is_read_over_iiop() {
    let foreign = foreign_uto_server(1_000, 50);
    let foreign_ior = Ior {
        type_id: UTO_TYPE_ID.to_string(),
        profiles: vec![
            TaggedProfile::iiop(
                &IiopProfileBody::new(
                    IiopVersion::V1_2,
                    "127.0.0.1".to_string(),
                    foreign.listen_addr().port(),
                    b"foreign-uto".to_vec(),
                ),
                Endianness::Little,
            )
            .unwrap(),
        ],
    };
    let srv = server(TimeService::default());
    let connector = Connector::new(ConnectorConfig::default());
    let mut pooled = connector
        .connect("127.0.0.1", srv.listen_addr().port())
        .unwrap();
    let conn = pooled.connection().unwrap();
    let e = Endianness::Little;
    // A TIO [900, 1100] of this server spans the foreign UTO 1000 ± 50.
    let mut w = Writer::new(e);
    w.write_u64(900);
    w.write_u64(1_100);
    let (status, body) = call(
        conn,
        e,
        1,
        TIME_SERVICE_OBJECT_KEY,
        "new_interval",
        w.finish(),
    );
    assert_eq!(status, ReplyStatusType::NoException);
    let tio_key = iiop(&decode_ior(&body, e)).object_key;
    let mut w = Writer::new(e);
    foreign_ior.encode(&mut w).unwrap();
    let (status, body) = call(conn, e, 2, &tio_key, "spans", w.finish());
    assert_eq!(status, ReplyStatusType::NoException);
    let mut r = Reader::new(&body, e);
    assert_eq!(r.read_u32().unwrap(), 0); // OTContainer
    let overlap = Ior::decode(&mut r).unwrap();
    let (status, body) = call(
        conn,
        e,
        3,
        &iiop(&overlap).object_key,
        "_get_time_interval",
        Vec::new(),
    );
    assert_eq!(status, ReplyStatusType::NoException);
    let mut r = Reader::new(&body, e);
    assert_eq!((r.read_u64().unwrap(), r.read_u64().unwrap()), (950, 1_050));
    // A foreign reference whose server refuses the read is BAD_PARAM.
    let mut refused = foreign_ior.clone();
    refused.profiles = vec![
        TaggedProfile::iiop(
            &IiopProfileBody::new(
                IiopVersion::V1_2,
                "127.0.0.1".to_string(),
                foreign.listen_addr().port(),
                b"other".to_vec(),
            ),
            Endianness::Little,
        )
        .unwrap(),
    ];
    let mut w = Writer::new(e);
    refused.encode(&mut w).unwrap();
    let (status, body) = call(conn, e, 4, &tio_key, "spans", w.finish());
    assert_eq!(status, ReplyStatusType::SystemException);
    assert_eq!(read_repo_id(&body, e), "IDL:omg.org/CORBA/BAD_PARAM:1.0");
}

#[test]
fn time_1_2_a_giop_1_0_client_gets_its_unsigned_long_long_arguments_aligned() {
    // GIOP 1.0 has no 8-octet boundary before the body: CDR pads an
    // `unsigned long long` relative to the message start. omniORB speaks
    // GIOP 1.0 to a `corbaloc::` reference without a version.
    let srv = server(TimeService::default());
    let connector = Connector::new(ConnectorConfig::default());
    let mut pooled = connector
        .connect("127.0.0.1", srv.listen_addr().port())
        .unwrap();
    let conn = pooled.connection().unwrap();
    let e = Endianness::Little;
    let op = "new_universal_time";
    let header = encode_message(
        Version::V1_0,
        e,
        false,
        &Message::Request(request(1, TIME_SERVICE_OBJECT_KEY, op)),
    )
    .unwrap();
    let pad = header.len() % 8;
    assert_ne!(
        pad, 0,
        "the case this test is for: a body off an 8-octet boundary"
    );
    let mut w = Writer::new(e);
    for _ in 0..pad {
        w.write_u8(0);
    }
    w.write_u64(1_000);
    w.write_u64(50);
    w.write_i16(-60);
    let body = w.finish()[pad..].to_vec();
    let (status, reply) = call_v(conn, Version::V1_0, e, 1, TIME_SERVICE_OBJECT_KEY, op, body);
    assert_eq!(status, ReplyStatusType::NoException);
    let key = iiop(&decode_ior(&reply, e)).object_key;
    let (status, reply) = call_v(conn, Version::V1_0, e, 2, &key, "_get_utc_time", Vec::new());
    assert_eq!(status, ReplyStatusType::NoException);
    // The GIOP 1.0 reply header without service contexts is 24 octets:
    // the body is on an 8-octet boundary.
    let mut r = Reader::new(&reply, e);
    assert_eq!(r.read_u64().unwrap(), 1_000);
    assert_eq!(r.read_u32().unwrap(), 50);
    assert_eq!(r.read_u16().unwrap(), 0);
    assert_eq!(r.read_i16().unwrap(), -60);
}

#[test]
fn time_b_2_the_server_gives_no_current_time_by_default() {
    // App. B.2, first way: no current time of the service leaves its
    // address space, neither through universal_time nor through
    // absolute_time of a UTO of time 0 (§1.3.4.5: current time + time in
    // the object). The clock gives a time, so the refusal is the system
    // exception NO_PERMISSION, not TimeUnavailable (§2.1.1).
    let srv = server(TimeService::with_source(|| 138_000_000_000_000_000));
    let connector = Connector::new(ConnectorConfig::default());
    let mut pooled = connector
        .connect("127.0.0.1", srv.listen_addr().port())
        .unwrap();
    let conn = pooled.connection().unwrap();
    for e in [Endianness::Little, Endianness::Big] {
        let (status, body) = call(
            conn,
            e,
            1,
            TIME_SERVICE_OBJECT_KEY,
            "universal_time",
            Vec::new(),
        );
        assert_eq!(status, ReplyStatusType::SystemException);
        assert_eq!(read_repo_id(&body, e), NO_PERMISSION_ID);
        let (status, body) = call(
            conn,
            e,
            2,
            TIME_SERVICE_OBJECT_KEY,
            "secure_universal_time",
            Vec::new(),
        );
        assert_eq!(status, ReplyStatusType::UserException);
        assert_eq!(read_repo_id(&body, e), TIME_UNAVAILABLE_TYPE_ID);
        // new_universal_time reads no clock: served.
        let mut w = Writer::new(e);
        w.write_u64(0);
        w.write_u64(0);
        w.write_i16(0);
        let (status, body) = call(
            conn,
            e,
            3,
            TIME_SERVICE_OBJECT_KEY,
            "new_universal_time",
            w.finish(),
        );
        assert_eq!(status, ReplyStatusType::NoException);
        let zero = iiop(&decode_ior(&body, e)).object_key;
        let (status, body) = call(conn, e, 4, &zero, "absolute_time", Vec::new());
        assert_eq!(status, ReplyStatusType::SystemException);
        assert_eq!(read_repo_id(&body, e), NO_PERMISSION_ID);
    }
    // The proxy reports both as unavailable.
    let proxy = TimeServiceProxy::new("127.0.0.1", srv.listen_addr().port());
    assert_eq!(proxy.universal_time(), Err(TimeUnavailable));
    assert_eq!(
        proxy.absolute_time(Uto::new(0, 0, 0).unwrap()),
        Err(AbsoluteTimeError::TimeUnavailable)
    );
}

#[test]
fn time_b_2_secure_time_stays_in_the_address_space_of_the_service() {
    use std::sync::Arc;
    use zerodds_time_service::admin::{AdminOp, AdminSubject, AuditError, AuditRecord, AuditSink};
    use zerodds_time_service::secure::{
        SecureTimeSource, SourceDesignation, SourceReading, SyncProtection,
    };
    use zerodds_time_service::time_base::current_time;
    struct Keep;
    impl AuditSink for Keep {
        fn record(&self, _r: &AuditRecord) -> Result<(), AuditError> {
            Ok(())
        }
    }
    struct Synced;
    impl SecureTimeSource for Synced {
        fn read(&self) -> Result<SourceReading, TimeUnavailable> {
            Ok(SourceReading {
                time: current_time(),
                inaccuracy: 10_000,
                synchronized: true,
            })
        }
    }
    let (service, admin) = TimeService::default().administered(
        Arc::new(|_: &AdminSubject, _: AdminOp| true),
        Arc::new(Keep),
    );
    admin
        .designate_source(
            &AdminSubject::new("ops"),
            SourceDesignation::new(
                "reference",
                SyncProtection::LocalReference,
                Arc::new(Synced),
            ),
        )
        .unwrap();
    for policy in [
        RemoteTimePolicy::Refuse,
        RemoteTimePolicy::AnswerCorrectingProxies,
    ] {
        let srv = TimeServer::start_with_policy(
            service.clone(),
            loopback(),
            String::from("127.0.0.1"),
            policy,
        )
        .unwrap();
        // In its own address space the served service gives secure time ...
        assert!(service.secure_universal_time().is_ok(), "{policy:?}");
        // ... over IIOP neither the server nor the proxy does.
        let connector = Connector::new(ConnectorConfig::default());
        let mut pooled = connector
            .connect("127.0.0.1", srv.listen_addr().port())
            .unwrap();
        let conn = pooled.connection().unwrap();
        let e = Endianness::Big;
        let (status, body) = call(
            conn,
            e,
            1,
            TIME_SERVICE_OBJECT_KEY,
            "secure_universal_time",
            Vec::new(),
        );
        assert_eq!(status, ReplyStatusType::UserException, "{policy:?}");
        assert_eq!(read_repo_id(&body, e), TIME_UNAVAILABLE_TYPE_ID);
        let proxy = TimeServiceProxy::from_ior(&srv.service_ior().unwrap()).unwrap();
        assert_eq!(proxy.secure_universal_time(), Err(TimeUnavailable));
    }
}

#[test]
fn time_b_2_the_proxy_widens_the_interval_by_the_measured_latency() {
    use std::time::{Duration, Instant};
    use zerodds_time_service::time_base::current_time;
    // The service generates its timestamp and then takes 60 ms until the
    // reply leaves: the caller receives a time 60 ms old. Its interval
    // must still contain the caller's time when the proxy returns.
    const DELAY: Duration = Duration::from_millis(60);
    let srv = answering_server(
        TimeService::with_source(|| {
            let t = current_time();
            std::thread::sleep(DELAY);
            t
        })
        // 1 ms for what passes between the proxy's last clock reading
        // and the test's (and for a slewed system clock).
        .with_inaccuracy(10_000)
        .unwrap()
        .with_tdf(0),
    );
    let proxy = TimeServiceProxy::from_ior(&srv.service_ior().unwrap()).unwrap();
    let before = Instant::now();
    let uto = proxy.universal_time().unwrap();
    let now = current_time();
    let outer = before.elapsed();
    // The caller's clock at return lies within the corrected interval.
    assert!(
        uto.time() - uto.inaccuracy() <= now && now <= uto.time() + uto.inaccuracy(),
        "now {now} outside {} ± {}",
        uto.time(),
        uto.inaccuracy()
    );
    // The widening is half the measured round trip: at least half the
    // delay, at most half of what the test measured around the call.
    let half = uto.inaccuracy() - 10_000;
    let delay_ticks = DELAY.as_nanos() as u64 / 100;
    let outer_ticks = (outer.as_nanos() as u64).div_ceil(100);
    assert!(half >= delay_ticks / 2, "half {half}");
    assert!(
        half <= outer_ticks.div_ceil(2) + 1,
        "half {half}, outer {outer_ticks}"
    );
    // Uncorrected, the stamp would end before the caller's time.
    let stamp = uto.time() - half;
    assert!(stamp + 10_000 < now, "the stale stamp the proxy corrects");
}

#[test]
fn time_b_2_the_proxy_widens_absolute_time_by_the_measured_latency() {
    use std::time::{Duration, Instant};
    use zerodds_time_service::time_base::current_time;
    // As for universal_time: the service reads its clock for
    // absolute_time and takes 60 ms until the reply leaves.
    const DELAY: Duration = Duration::from_millis(60);
    let srv = answering_server(
        TimeService::with_source(|| {
            let t = current_time();
            std::thread::sleep(DELAY);
            t
        })
        .with_tdf(0),
    );
    let proxy = TimeServiceProxy::from_ior(&srv.service_ior().unwrap()).unwrap();
    // One second ahead, with an inaccuracy of 1 ms (also for what passes
    // between the proxy's last clock reading and the test's).
    const SECOND: u64 = 10_000_000;
    let relative = Uto::new(SECOND, 10_000, 0).unwrap();
    let before = Instant::now();
    let uto = proxy.absolute_time(relative).unwrap();
    let target = current_time() + SECOND;
    let outer = before.elapsed();
    assert!(
        uto.time() - uto.inaccuracy() <= target && target <= uto.time() + uto.inaccuracy(),
        "target {target} outside {} ± {}",
        uto.time(),
        uto.inaccuracy()
    );
    let half = uto.inaccuracy() - 10_000;
    let delay_ticks = DELAY.as_nanos() as u64 / 100;
    let outer_ticks = (outer.as_nanos() as u64).div_ceil(100);
    assert!(half >= delay_ticks / 2, "half {half}");
    assert!(
        half <= outer_ticks.div_ceil(2) + 1,
        "half {half}, outer {outer_ticks}"
    );
    let stamp = uto.time() - half;
    assert!(
        stamp + 10_000 < target,
        "the stale stamp the proxy corrects"
    );
    // An overflow on the service is DATA_CONVERSION (§1.3.4.5).
    assert_eq!(
        proxy.absolute_time(Uto::new(u64::MAX, 0, 0).unwrap()),
        Err(AbsoluteTimeError::DataConversion)
    );
    // A service whose clock gives no time: TRANSIENT, TimeUnavailable here.
    let stopped = answering_server(TimeService::with_source(|| 0));
    let proxy = TimeServiceProxy::from_ior(&stopped.service_ior().unwrap()).unwrap();
    assert_eq!(
        proxy.absolute_time(relative),
        Err(AbsoluteTimeError::TimeUnavailable)
    );
}
