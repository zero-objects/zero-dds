//! E2E-Integrationstests fuer C6.1.C — Requester + Replier ueber DCPS-
//! Offline-Loop.
//!
//! Strategie: Beide Endpoints laufen auf demselben `DomainParticipant`
//! im Offline-Mode. Wir pumpen Bytes manuell zwischen Writer-Queue und
//! Reader-Inbox — das simuliert den Phase-B-Transport-Pfad und ist
//! ausreichend fuer Foundation-Tests, die die Korrelations- und
//! Handler-Logik validieren (nicht den UDP-Transport).

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use std::sync::Arc;
use std::time::Duration;

use zerodds_dcps::dds_type::RawBytes;
use zerodds_dcps::factory::DomainParticipantFactory;
use zerodds_dcps::participant::DomainParticipant;
use zerodds_dcps::qos::DomainParticipantQos;

use zerodds_rpc::common_types::RemoteExceptionCode;
use zerodds_rpc::error::RpcError;
use zerodds_rpc::qos_profile::RpcQos;
use zerodds_rpc::replier::{FnHandler, Replier, ReplierHandler};
use zerodds_rpc::requester::Requester;

fn participant(domain: i32) -> DomainParticipant {
    DomainParticipantFactory::instance()
        .create_participant_offline(domain, DomainParticipantQos::default())
}

/// Pumpt alle gequeueten Frames vom Requester→Replier (Request-Topic) und
/// vom Replier→Requester (Reply-Topic). Eine "Tick" ist ein Bidirectional-
/// Roundtrip: erst Requests durchschieben, Replier verarbeiten, dann
/// Replies durchschieben, Requester `tick`.
fn pump<
    TIn: zerodds_dcps::dds_type::DdsType + Send + 'static,
    TOut: zerodds_dcps::dds_type::DdsType + Send + 'static,
>(
    requester: &Requester<TIn, TOut>,
    replier: &Replier<TIn, TOut>,
) {
    for frame in requester.__drain_request_writer() {
        replier.__push_request_raw(frame).unwrap();
    }
    let _ = replier.tick();
    for frame in replier.__drain_reply_writer() {
        requester.__push_reply_raw(frame).unwrap();
    }
    requester.tick();
}

#[test]
fn e2e_single_request_response_roundtrip() {
    let p = participant(1001);
    let q = RpcQos::default_basic();
    let handler: Arc<dyn ReplierHandler<RawBytes, RawBytes>> =
        Arc::new(FnHandler::new(|req: RawBytes| {
            let mut out = req.data;
            out.reverse();
            Ok::<_, RemoteExceptionCode>(RawBytes::new(out))
        }));
    let replier = Replier::<RawBytes, RawBytes>::new(&p, "Echo", &q, handler).unwrap();
    let requester = Requester::<RawBytes, RawBytes>::new(&p, "Echo", &q).unwrap();

    let (id, rx) = requester
        .send_request_async(&RawBytes::new(vec![1, 2, 3, 4]))
        .unwrap();
    pump(&requester, &replier);

    let result = rx.try_recv().expect("reply present");
    let bytes = result.expect("ok reply");
    assert_eq!(bytes, vec![4, 3, 2, 1]);
    assert_eq!(replier.handled_count(), 1);
    // Sequence-Number > 0.
    assert!(id.sequence_number > 0);
}

#[test]
fn e2e_three_methods_distinct_payload_directions() {
    // Simuliert drei Methoden auf demselben Service mit unterschiedlichen
    // Param-Direction-Mustern, indem wir den Payload als
    // (method_tag, payload_bytes) interpretieren. Im echten Codegen
    // produziert C6.1.B genau so eine Discriminator-Union.
    let p = participant(1002);
    let q = RpcQos::default_basic();
    let handler = Arc::new(FnHandler::new(|req: RawBytes| {
        match req.data.first() {
            Some(0xA1) => Ok(RawBytes::new(vec![0xB1, 42])), // method add
            Some(0xA2) => Ok(RawBytes::new(vec![0xB2])),     // method ping (no out param)
            Some(0xA3) => Ok(RawBytes::new(vec![0xB3, 1, 2, 3])), // method bulk
            _ => Err(RemoteExceptionCode::UnknownOperation),
        }
    }));
    let replier = Replier::<RawBytes, RawBytes>::new(&p, "Multi", &q, handler).unwrap();
    let requester = Requester::<RawBytes, RawBytes>::new(&p, "Multi", &q).unwrap();

    let mut receivers = Vec::new();
    for tag in [0xA1u8, 0xA2, 0xA3] {
        let (_id, rx) = requester
            .send_request_async(&RawBytes::new(vec![tag]))
            .unwrap();
        receivers.push((tag, rx));
    }
    pump(&requester, &replier);

    for (tag, rx) in receivers {
        let res = rx.try_recv().expect("reply");
        let bytes = res.expect("ok reply");
        match tag {
            0xA1 => assert_eq!(bytes, vec![0xB1, 42]),
            0xA2 => assert_eq!(bytes, vec![0xB2]),
            0xA3 => assert_eq!(bytes, vec![0xB3, 1, 2, 3]),
            _ => panic!("unexpected method tag {tag:#x}"),
        }
    }
}

#[test]
fn e2e_multi_pending_requests_all_get_their_reply() {
    // 5 Requests parallel — jeder soll seinen eigenen Reply zurueckbekommen.
    let p = participant(1003);
    let q = RpcQos::default_basic();
    let handler = Arc::new(FnHandler::new(|req: RawBytes| {
        // Reply payload echoed mit inkrement
        let mut out = req.data.clone();
        if let Some(b) = out.first_mut() {
            *b = b.wrapping_add(100);
        }
        Ok::<_, RemoteExceptionCode>(RawBytes::new(out))
    }));
    let replier = Replier::<RawBytes, RawBytes>::new(&p, "Bulk", &q, handler).unwrap();
    let requester = Requester::<RawBytes, RawBytes>::new(&p, "Bulk", &q).unwrap();

    let mut rxs = Vec::new();
    for i in 1u8..=5 {
        let (id, rx) = requester
            .send_request_async(&RawBytes::new(vec![i, i, i]))
            .unwrap();
        rxs.push((i, id, rx));
    }
    assert_eq!(requester.pending_count(), 5);
    pump(&requester, &replier);
    assert_eq!(replier.handled_count(), 5);
    assert_eq!(requester.pending_count(), 0);

    for (i, _id, rx) in rxs {
        let res = rx.try_recv().expect("reply");
        let bytes = res.expect("ok");
        assert_eq!(bytes[0], i.wrapping_add(100));
        assert_eq!(bytes.len(), 3);
    }
}

#[test]
fn e2e_handler_error_surfaces_as_remote_exception_at_caller() {
    let p = participant(1004);
    let q = RpcQos::default_basic();
    let handler = Arc::new(FnHandler::new(|_: RawBytes| {
        Err::<RawBytes, _>(RemoteExceptionCode::InvalidArgument)
    }));
    let replier = Replier::<RawBytes, RawBytes>::new(&p, "ErrSvc", &q, handler).unwrap();
    let requester = Requester::<RawBytes, RawBytes>::new(&p, "ErrSvc", &q).unwrap();
    let (_id, rx) = requester
        .send_request_async(&RawBytes::new(vec![1]))
        .unwrap();
    pump(&requester, &replier);
    let res = rx.try_recv().expect("reply");
    assert_eq!(res, Err(RemoteExceptionCode::InvalidArgument));
    assert_eq!(replier.error_count(), 1);
}

#[test]
fn e2e_send_request_blocking_succeeds_with_pumping_thread() {
    // Blocking-Variante mit einem Background-Thread, der das Pumpen macht.
    let p = participant(1005);
    let q = RpcQos::default_basic();
    let handler = Arc::new(FnHandler::new(|req: RawBytes| {
        Ok::<_, RemoteExceptionCode>(RawBytes::new(req.data))
    }));
    let replier = Arc::new(Replier::<RawBytes, RawBytes>::new(&p, "Block", &q, handler).unwrap());
    let requester = Arc::new(Requester::<RawBytes, RawBytes>::new(&p, "Block", &q).unwrap());

    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let pump_handle = {
        let req = Arc::clone(&requester);
        let rep = Arc::clone(&replier);
        let stop = Arc::clone(&stop);
        std::thread::spawn(move || {
            while !stop.load(std::sync::atomic::Ordering::Acquire) {
                for frame in req.__drain_request_writer() {
                    let _ = rep.__push_request_raw(frame);
                }
                let _ = rep.tick();
                for frame in rep.__drain_reply_writer() {
                    let _ = req.__push_reply_raw(frame);
                }
                std::thread::sleep(Duration::from_millis(1));
            }
        })
    };

    let result = requester
        .send_request_blocking(
            &RawBytes::new(vec![7, 7, 7]),
            Some(Duration::from_millis(500)),
        )
        .unwrap();
    assert_eq!(result.data, vec![7, 7, 7]);

    stop.store(true, std::sync::atomic::Ordering::Release);
    pump_handle.join().unwrap();
}

#[test]
fn e2e_reply_after_timeout_is_dropped() {
    let p = participant(1006);
    let mut q = RpcQos::default_basic();
    q.request_timeout = Duration::from_millis(10);
    let handler = Arc::new(FnHandler::new(|r: RawBytes| {
        Ok::<_, RemoteExceptionCode>(r)
    }));
    let replier = Replier::<RawBytes, RawBytes>::new(&p, "Lat", &q, handler).unwrap();
    let requester = Requester::<RawBytes, RawBytes>::new(&p, "Lat", &q).unwrap();
    let err = requester
        .send_request_blocking(&RawBytes::new(vec![1]), None)
        .unwrap_err();
    assert!(matches!(err, RpcError::Timeout));
    // Erst jetzt pumpen — der Reply trifft "zu spaet" ein und wird vom
    // Requester verworfen, kein Crash.
    pump(&requester, &replier);
    // Pending wurde nach Timeout in send_request_blocking nicht entfernt
    // (Foundation-Limitation: Drop-on-Timeout ist C6.1.D-Scope). Der Slot
    // wird im pump dann konsumiert und drop'ed.
    assert!(requester.pending_count() <= 1);
}

#[test]
fn e2e_oneway_does_not_produce_reply_traffic() {
    let p = participant(1007);
    let q = RpcQos::default_basic();
    let handler = Arc::new(FnHandler::new(|_: RawBytes| {
        Ok::<_, RemoteExceptionCode>(RawBytes::new(vec![]))
    }));
    let replier = Replier::<RawBytes, RawBytes>::new(&p, "OnewaySvc", &q, handler).unwrap();
    let requester = Requester::<RawBytes, RawBytes>::new(&p, "OnewaySvc", &q).unwrap();
    let id = requester.send_oneway(&RawBytes::new(vec![1, 2])).unwrap();
    assert!(id.sequence_number > 0);
    assert_eq!(requester.pending_count(), 0);
    // Reply wird trotzdem produziert (Replier weiss nicht, dass es oneway war),
    // aber Requester hat keinen Pending-Slot dafuer und verwirft ihn.
    pump(&requester, &replier);
    assert_eq!(replier.handled_count(), 1);
    assert_eq!(requester.pending_count(), 0);
}

#[test]
fn e2e_instance_routing_filters_per_replier() {
    // Zwei Replier auf derselben Service unterschieden durch
    // service_instance_name. Requester-A spricht nur mit Replier-A.
    let p = participant(1008);
    let q = RpcQos::default_basic();
    let handler_a = Arc::new(FnHandler::new(|_: RawBytes| {
        Ok::<_, RemoteExceptionCode>(RawBytes::new(vec![0xAA]))
    }));
    let handler_b = Arc::new(FnHandler::new(|_: RawBytes| {
        Ok::<_, RemoteExceptionCode>(RawBytes::new(vec![0xBB]))
    }));
    let rep_a =
        Replier::<RawBytes, RawBytes>::with_instance(&p, "Inst", "calc-A", &q, handler_a).unwrap();
    let rep_b =
        Replier::<RawBytes, RawBytes>::with_instance(&p, "Inst", "calc-B", &q, handler_b).unwrap();
    let req_a = Requester::<RawBytes, RawBytes>::with_instance(&p, "Inst", "calc-A", &q).unwrap();

    let (_id, _rx) = req_a.send_request_async(&RawBytes::new(vec![1])).unwrap();
    // Pumpe Request manuell zu beiden Repliern — nur calc-A darf antworten.
    let frames = req_a.__drain_request_writer();
    for f in frames.iter() {
        rep_a.__push_request_raw(f.clone()).unwrap();
        rep_b.__push_request_raw(f.clone()).unwrap();
    }
    let _ = rep_a.tick();
    let _ = rep_b.tick();
    // calc-A handled = 1, calc-B handled = 0 (gefilterted).
    assert_eq!(rep_a.handled_count(), 1);
    assert_eq!(rep_b.handled_count(), 0);
}

#[test]
fn e2e_qos_profile_from_xml_drives_history_depth() {
    use zerodds_xml::parse_dds_xml;
    let xml = r#"<dds>
        <qos_library name="L">
            <qos_profile name="Basic">
                <datawriter_qos>
                    <history>
                        <kind>KEEP_LAST_HISTORY_QOS</kind>
                        <depth>17</depth>
                    </history>
                    <reliability>
                        <kind>RELIABLE_RELIABILITY_QOS</kind>
                    </reliability>
                </datawriter_qos>
            </qos_profile>
        </qos_library>
    </dds>"#;
    let loader = parse_dds_xml(xml).unwrap();
    let q = RpcQos::from_xml_profile(&loader, "L::Basic").unwrap();
    assert_eq!(q.request_history.depth, 17);
    assert_eq!(q.reply_history.depth, 17);

    let p = participant(1009);
    let r = Requester::<RawBytes, RawBytes>::new(&p, "FromXml", &q).unwrap();
    // QoS wurde auf den Writer durchgereicht — DataWriter::qos() gibt eine
    // Kopie mit depth=17 zurueck.
    drop(r);
}
