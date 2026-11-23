// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! MIOP between ZeroDDS and JacORB 3.9 (CORBA 3.3 Part 2 §11), both
//! directions: a JacORB GOA member of a group receives the oneway
//! requests ZeroDDS multicasts, and a ZeroDDS GOA member receives the
//! requests JacORB multicasts — a short request in one packet and a long
//! one in a collection of several. JacORB's packet header, packet
//! collections, UIPMC profile and `corbaloc:miop` parser are the oracle
//! of ZeroDDS's, and the other way round.
//!
//! JacORB 3.9 and its Java 8 JDK come from
//! `tests/integration/provision.sh jacorb`; the job
//! `integration-bridges-vendor-corba` runs the tests (`#[ignore]` in the
//! crate tier, which has no JacORB).

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use zerodds_corba_rust::marshaled_body;

use zerodds_corba_interop::miop::{Goa, MiopClient};
use zerodds_corba_ior::miop::{GroupInfo, group_profiles, is_group_reference, parse_miop_corbaloc};
use zerodds_corba_rust::SkeletonResult;
use zerodds_wire::{Endianness, Writer};

#[path = "support/jacorb.rs"]
mod jacorb;

fn sources() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("competitors/jacorb/miop")
}

fn greeting(text: &str, e: Endianness) -> Vec<u8> {
    let mut w = Writer::new(e);
    w.write_string(text);
    w.finish()
}

fn long_text() -> String {
    "ZeroDDS MIOP ".chars().cycle().take(6000).collect()
}

#[test]
#[ignore = "needs JacORB 3.9 (integration-bridges-vendor-corba)"]
fn corba_p2_11_a_jacorb_goa_member_receives_what_zerodds_multicasts() {
    let url = "corbaloc:miop:1.0@1.0-ZeroDDS-21/239.255.44.1:24421";
    let prop = format!("miop.url={url}");
    let spec = jacorb::Spec {
        name: "miop-jacorb-server",
        idl: vec![sources().join("miop.idl")],
        java: vec![sources().join("MiopServer.java")],
        main: "MiopServer",
        props: &[prop.as_str()],
        ior_to_file: false,
        stdout_prefix: "GROUP=",
    };
    let (_server, ior) = jacorb::start(&spec);
    // JacORB's group reference, read by ZeroDDS.
    let group = zerodds_corba_ior::from_stringified(&ior).unwrap();
    assert!(is_group_reference(&group), "{ior}");
    let g = group_profiles(&group).unwrap();
    assert_eq!(g.uipmc[0].the_address, "239.255.44.1");
    assert_eq!(g.uipmc[0].the_port, 24421);
    assert_eq!(g.groups[0].group_domain_id, "ZeroDDS");
    assert_eq!(g.groups[0].object_group_id, 21);
    let output = jacorb::server_output(&spec);
    let client = MiopClient::new();
    // JacORB joins the group after it printed the reference; send until
    // the first request arrives, then the long one.
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        client
            .send_oneway(
                &group,
                "greeting_oneway",
                Endianness::Big,
                &marshaled_body(&greeting("short greeting", Endianness::Big), 0),
            )
            .unwrap();
        std::thread::sleep(Duration::from_millis(200));
        if std::fs::read_to_string(&output)
            .unwrap_or_default()
            .contains("RECEIVED 14 short greeting")
        {
            break;
        }
        assert!(Instant::now() < deadline, "JacORB received nothing");
    }
    client
        .send_oneway(
            &group,
            "greeting_oneway",
            Endianness::Little,
            &marshaled_body(&greeting(&long_text(), Endianness::Little), 0),
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let text = std::fs::read_to_string(&output).unwrap_or_default();
        if text.contains("RECEIVED 6000 ZeroDDS MIOP Zer") {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "JacORB did not receive the long request:\n{text}"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
#[ignore = "needs JacORB 3.9 (integration-bridges-vendor-corba)"]
fn corba_p2_11_a_zerodds_goa_member_receives_what_jacorb_multicasts() {
    let url = "corbaloc:miop:1.0@1.0-ZeroDDS-22/239.255.44.2:24422";
    let group = parse_miop_corbaloc(url).unwrap();
    assert_eq!(
        group_profiles(&group).unwrap().groups,
        vec![GroupInfo::new("ZeroDDS", 22, 0)]
    );
    let goa = Goa::new();
    let id = goa.create_id_for_reference(&group).unwrap();
    let received: Arc<Mutex<Vec<String>>> = Arc::default();
    let r = Arc::clone(&received);
    goa.activate_object_with_id(&id, move |op, body, e| {
        assert_eq!(op, "greeting_oneway");
        r.lock()
            .unwrap()
            .push(body.reader(e).read_string().unwrap());
        SkeletonResult::Reply(Vec::new())
    });
    let prop = format!("miop.url={url}");
    let out = jacorb::run(&jacorb::Spec {
        name: "miop-jacorb-client",
        idl: vec![sources().join("miop.idl")],
        java: vec![sources().join("MiopClient.java")],
        main: "MiopClient",
        props: &[prop.as_str()],
        ior_to_file: false,
        stdout_prefix: "",
    });
    assert!(out.contains("ALL OK"), "{out}");
    let deadline = Instant::now() + Duration::from_secs(10);
    while received.lock().unwrap().len() < 2 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    let got = received.lock().unwrap().clone();
    assert_eq!(got.len(), 2, "{got:?} {:?}", goa.stats());
    assert_eq!(got[0], "short greeting");
    assert_eq!(got[1].len(), 6000);
    assert!(got[1].starts_with("long greeting jacorb miop"));
    assert_eq!(
        goa.stats().reassembly.incomplete + goa.stats().reassembly.malformed,
        0
    );
}
