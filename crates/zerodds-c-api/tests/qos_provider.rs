// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `zerodds_qosprovider_*` (DDS-PSM-Cxx 1.0 §7.6.2.1): a provider loads the
//! DDS-XML QoS libraries of a `file:` URI and answers the QoS of each entity
//! kind for a profile, with `base_name` inheritance (DDS-XML 1.0 §7.3.2.4.2);
//! other schemes are UNSUPPORTED, a missing file or profile BAD_PARAMETER.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::ffi::CString;
use std::ptr;

use zerodds::qos_ffi::{
    ZeroDdsDataReaderQos, ZeroDdsDataWriterQos, ZeroDdsDomainParticipantQos, ZeroDdsPublisherQos,
    ZeroDdsTopicQos,
};
use zerodds::qos_provider_ffi::{
    zerodds_qosprovider_create, zerodds_qosprovider_datareader_qos,
    zerodds_qosprovider_datawriter_qos, zerodds_qosprovider_destroy,
    zerodds_qosprovider_participant_qos, zerodds_qosprovider_publisher_qos,
    zerodds_qosprovider_topic_qos,
};

const OK: i32 = 0;
const BAD_PARAMETER: i32 = -6;
const OUT_OF_RESOURCES: i32 = -8;
const UNSUPPORTED: i32 = -13;
const RELIABLE: u32 = 2;
const KEEP_ALL: u32 = 1;
const TRANSIENT_LOCAL: u32 = 1;

// Spellings of DDS-XML 1.0 (`dds-xml_qos_definitions_nonamespace.xsd`): `domain_participant_qos`,
// the DCPS IDL literals of the enumerations, a partition name list as `<element>`s.
const XML: &str = r#"<dds><qos_library name="L">
  <qos_profile name="Base">
    <domain_participant_qos>
      <user_data><value>cmF3X2J5dGVz</value></user_data>
      <entity_factory><autoenable_created_entities>false</autoenable_created_entities></entity_factory>
    </domain_participant_qos>
    <topic_qos>
      <topic_data><value>dG9waWNfZGF0YQ==</value></topic_data>
      <durability><kind>TRANSIENT_LOCAL_DURABILITY_QOS</kind></durability>
    </topic_qos>
    <publisher_qos>
      <partition><name><element>Tracks</element></name></partition>
    </publisher_qos>
    <datawriter_qos>
      <reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability>
      <history><kind>KEEP_LAST_HISTORY_QOS</kind><depth>5</depth></history>
    </datawriter_qos>
    <datareader_qos>
      <durability><kind>TRANSIENT_LOCAL_DURABILITY_QOS</kind></durability>
    </datareader_qos>
  </qos_profile>
  <qos_profile name="Child" base_name="Base">
    <datawriter_qos>
      <history><kind>KEEP_ALL_HISTORY_QOS</kind></history>
    </datawriter_qos>
  </qos_profile>
</qos_library></dds>"#;

fn write_xml(name: &str) -> String {
    let path = std::env::temp_dir().join(format!(
        "zerodds_qos_provider_{}_{name}.xml",
        std::process::id()
    ));
    std::fs::write(&path, XML).unwrap();
    format!("file://{}", path.display())
}

#[test]
fn a_provider_answers_the_qos_of_every_entity_kind_with_inheritance() {
    let uri = CString::new(write_xml("all")).unwrap();
    let profile = CString::new("L::Child").unwrap();
    // SAFETY: strings are NUL-terminated; the provider is destroyed once.
    unsafe {
        let mut status = 1;
        let p = zerodds_qosprovider_create(uri.as_ptr(), profile.as_ptr(), &mut status);
        assert!(!p.is_null(), "status {status}");

        // The DataWriter: Reliability from Base, History overridden by Child.
        let mut dw: ZeroDdsDataWriterQos = std::mem::zeroed();
        assert_eq!(
            zerodds_qosprovider_datawriter_qos(p, ptr::null(), &mut dw),
            OK
        );
        assert_eq!(dw.reliability.kind, RELIABLE);
        assert_eq!(dw.history.kind, KEEP_ALL);

        // The same through an explicit id.
        let base = CString::new("L::Base").unwrap();
        let mut dw_base: ZeroDdsDataWriterQos = std::mem::zeroed();
        assert_eq!(
            zerodds_qosprovider_datawriter_qos(p, base.as_ptr(), &mut dw_base),
            OK
        );
        assert_eq!(dw_base.history.kind, 0, "Base keeps KEEP_LAST");
        assert_eq!(dw_base.history.depth, 5);

        let mut dr: ZeroDdsDataReaderQos = std::mem::zeroed();
        assert_eq!(
            zerodds_qosprovider_datareader_qos(p, ptr::null(), &mut dr),
            OK
        );
        assert_eq!(dr.durability.kind, TRANSIENT_LOCAL);

        // TOPIC_DATA needs a buffer: a zero one is OUT_OF_RESOURCES with the
        // length needed.
        let mut topic: ZeroDdsTopicQos = std::mem::zeroed();
        assert_eq!(
            zerodds_qosprovider_topic_qos(p, ptr::null(), &mut topic),
            OUT_OF_RESOURCES
        );
        assert_eq!(topic.topic_data.value_len, b"topic_data".len());
        let mut buf = vec![0u8; topic.topic_data.value_len];
        topic.topic_data.value = buf.as_mut_ptr();
        assert_eq!(
            zerodds_qosprovider_topic_qos(p, ptr::null(), &mut topic),
            OK
        );
        assert_eq!(&buf[..], b"topic_data");
        assert_eq!(topic.durability.kind, TRANSIENT_LOCAL);

        let mut dp: ZeroDdsDomainParticipantQos = std::mem::zeroed();
        let mut ud = vec![0u8; 16];
        dp.user_data.value = ud.as_mut_ptr();
        dp.user_data.value_len = ud.len();
        assert_eq!(
            zerodds_qosprovider_participant_qos(p, ptr::null(), &mut dp),
            OK
        );
        assert_eq!(&ud[..dp.user_data.value_len], b"raw_bytes");
        assert!(!dp.entity_factory.autoenable_created_entities);

        let mut publisher: ZeroDdsPublisherQos = std::mem::zeroed();
        assert_eq!(
            zerodds_qosprovider_publisher_qos(p, ptr::null(), &mut publisher),
            OK
        );
        assert_eq!(publisher.partition.names_len, 1);
        assert_eq!(
            std::ffi::CStr::from_ptr(*publisher.partition.names)
                .to_str()
                .unwrap(),
            "Tracks"
        );

        // A profile the document does not have.
        let none = CString::new("L::Nope").unwrap();
        assert_eq!(
            zerodds_qosprovider_datawriter_qos(p, none.as_ptr(), &mut dw),
            BAD_PARAMETER
        );
        zerodds_qosprovider_destroy(p);
    }
}

#[test]
fn a_data_uri_and_an_http_uri_name_the_document_too() {
    use std::io::{Read, Write};
    // The document in a data: URI, percent-encoded (RFC 2397).
    let encoded: String = XML.bytes().map(|b| format!("%{b:02X}")).collect();
    let data = CString::new(format!("data:text/xml,{encoded}")).unwrap();
    // The same document over http:, from a server that answers one request.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        let (mut conn, _) = listener.accept().unwrap();
        let mut request = [0u8; 512];
        let _ = conn.read(&mut request).unwrap();
        let head = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/xml\r\nContent-Length: {}\r\n\r\n",
            XML.len()
        );
        conn.write_all(head.as_bytes()).unwrap();
        conn.write_all(XML.as_bytes()).unwrap();
    });
    let http = CString::new(format!("http://127.0.0.1:{port}/qos.xml")).unwrap();
    let profile = CString::new("L::Child").unwrap();
    // SAFETY: strings are NUL-terminated; each provider is destroyed once.
    unsafe {
        for uri in [&data, &http] {
            let mut status = 0;
            let p = zerodds_qosprovider_create(uri.as_ptr(), profile.as_ptr(), &mut status);
            assert!(!p.is_null(), "{uri:?}: status {status}");
            zerodds_qosprovider_destroy(p);
        }
    }
    server.join().unwrap();
}

#[test]
fn an_unknown_scheme_is_unsupported_and_a_missing_file_or_a_closed_https_port_is_bad_parameter() {
    let http = CString::new("gopher://example.org/qos.xml").unwrap();
    let https = CString::new("https://127.0.0.1:1/qos.xml").unwrap();
    let missing = CString::new("file:///nonexistent/zerodds_qos.xml").unwrap();
    // SAFETY: strings are NUL-terminated.
    unsafe {
        let mut status = 0;
        assert!(zerodds_qosprovider_create(http.as_ptr(), ptr::null(), &mut status).is_null());
        assert_eq!(status, UNSUPPORTED);
        assert!(zerodds_qosprovider_create(https.as_ptr(), ptr::null(), &mut status).is_null());
        assert_eq!(status, BAD_PARAMETER);
        assert!(zerodds_qosprovider_create(missing.as_ptr(), ptr::null(), &mut status).is_null());
        assert_eq!(status, BAD_PARAMETER);
        // A default profile the document lacks is refused at creation.
        let uri = CString::new(write_xml("lacks")).unwrap();
        let profile = CString::new("L::Nope").unwrap();
        assert!(zerodds_qosprovider_create(uri.as_ptr(), profile.as_ptr(), &mut status).is_null());
        assert_eq!(status, BAD_PARAMETER);
    }
}
