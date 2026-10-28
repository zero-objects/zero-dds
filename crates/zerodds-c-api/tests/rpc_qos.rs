// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-RPC 1.0 §7.10 through the C API: a `@Qos` profile URL resolves to
//! endpoint QoS, and the mutable policies of a live endpoint change at run
//! time while the immutable ones are refused.

#![allow(clippy::expect_used, clippy::unwrap_used, missing_docs)]

use std::ffi::CString;

#[cfg(feature = "rpc")]
use zerodds::rpc_qos_ffi::zerodds_endpoint_qos_from_profile_url;
use zerodds::rpc_qos_ffi::{zerodds_reader_set_qos, zerodds_writer_set_qos};
use zerodds::{
    ZeroDdsEndpointQos, zerodds_endpoint_qos_default, zerodds_reader_create_qos,
    zerodds_reader_destroy, zerodds_runtime_create, zerodds_runtime_destroy,
    zerodds_writer_create_qos, zerodds_writer_destroy,
};

#[cfg(feature = "rpc")]
const PROFILES: &str = r#"<dds>
  <qos_library name="Other">
    <qos_profile name="Ignored">
      <datawriter_qos><history><kind>KEEP_LAST_HISTORY_QOS</kind><depth>1</depth></history></datawriter_qos>
    </qos_profile>
  </qos_library>
  <qos_library name="RpcLib">
    <qos_profile name="Calculator">
      <datawriter_qos>
        <reliability><kind>BEST_EFFORT_RELIABILITY_QOS</kind></reliability>
        <durability><kind>TRANSIENT_LOCAL_DURABILITY_QOS</kind></durability>
        <history><kind>KEEP_LAST_HISTORY_QOS</kind><depth>32</depth></history>
        <lifespan><duration><sec>3</sec><nanosec>0</nanosec></duration></lifespan>
      </datawriter_qos>
      <datareader_qos>
        <deadline><period><sec>2</sec><nanosec>0</nanosec></period></deadline>
      </datareader_qos>
    </qos_profile>
  </qos_library>
</dds>"#;

#[cfg(feature = "rpc")]
fn load(url: &str) -> (i32, ZeroDdsEndpointQos) {
    let url = CString::new(url).unwrap();
    let mut out = zerodds_endpoint_qos_default();
    // SAFETY: both pointers are valid for the call.
    let rc = unsafe { zerodds_endpoint_qos_from_profile_url(url.as_ptr(), &mut out) };
    (rc, out)
}

#[cfg(feature = "rpc")]
#[test]
fn a_profile_url_resolves_the_named_library_over_the_rpc_defaults() {
    let dir = std::env::temp_dir().join(format!("zerodds-rpc-qos-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("qos.xml");
    std::fs::write(&file, PROFILES).unwrap();
    let path = file.display().to_string();

    // §7.10.1: `file://path#library`, and the protocol is optional.
    for url in [format!("file://{path}#RpcLib"), format!("{path}#RpcLib")] {
        let (rc, qos) = load(&url);
        assert_eq!(rc, 0, "{url}");
        assert_eq!(qos.reliable, 0, "reliability from the writer container");
        assert_eq!(
            qos.transient_local, 1,
            "durability from the writer container"
        );
        assert_eq!(qos.history_keep_all, 0);
        assert_eq!(qos.history_depth, 32);
        assert_eq!(
            qos.lifespan_ns, 3_000_000_000,
            "lifespan from the writer container"
        );
        assert_eq!(
            qos.deadline_ns, 2_000_000_000,
            "deadline from the reader container"
        );
    }

    // Without a library the first one is used.
    let (rc, qos) = load(&path);
    assert_eq!(rc, 0);
    assert_eq!(qos.history_depth, 1);

    // §7.10.2: what the profile does not name is the RPC default, KEEP_ALL.
    let only_deadline = dir.join("deadline.xml");
    std::fs::write(
        &only_deadline,
        r#"<dds><qos_library name="L"><qos_profile name="P"><datareader_qos>
        <deadline><period><sec>1</sec><nanosec>0</nanosec></period></deadline>
        </datareader_qos></qos_profile></qos_library></dds>"#,
    )
    .unwrap();
    let (rc, qos) = load(&only_deadline.display().to_string());
    assert_eq!(rc, 0);
    assert_eq!(
        (qos.reliable, qos.transient_local, qos.history_keep_all),
        (1, 0, 1)
    );

    // §7.10.1: the files are DDS4CCM QoS profiles, which write the profile
    // directly under `<dds>` (DDS4CCM 1.1 §7.4.2.3).
    let ccm = dir.join("ccm.xml");
    std::fs::write(
        &ccm,
        r#"<dds><qos_profile name="P"><datawriter_qos>
        <reliability><kind>BEST_EFFORT_RELIABILITY_QOS</kind></reliability>
        <history><kind>KEEP_LAST_HISTORY_QOS</kind><depth>9</depth></history>
        </datawriter_qos></qos_profile></dds>"#,
    )
    .unwrap();
    let (rc, qos) = load(&ccm.display().to_string());
    assert_eq!(rc, 0, "a DDS4CCM profile file");
    assert_eq!((qos.reliable, qos.history_depth), (0, 9));

    // A protocol other than file, a missing file and a missing library are
    // refused.
    assert_eq!(load("http://host/qos.xml").0, -6);
    assert_eq!(load(&format!("{path}#NoSuchLibrary")).0, -1);
    assert_eq!(load("/nonexistent/qos.xml").0, -1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_mutable_policies_of_a_live_endpoint_change_and_the_immutable_ones_are_refused() {
    let lease = zerodds_test_domains::lease_domain();
    let topic = CString::new("CFfiRpcQosSetQos").unwrap();
    let typ = CString::new("RawBytes").unwrap();
    let qos = ZeroDdsEndpointQos {
        reliable: 1,
        history_keep_all: 1,
        ..zerodds_endpoint_qos_default()
    };
    // SAFETY: every pointer comes from the create call above its use and is
    // checked for NULL.
    unsafe {
        let rt = zerodds_runtime_create(lease.id() as u32);
        assert!(!rt.is_null());
        let writer = zerodds_writer_create_qos(rt, topic.as_ptr(), typ.as_ptr(), 0, &qos);
        let reader = zerodds_reader_create_qos(rt, topic.as_ptr(), typ.as_ptr(), 0, &qos);
        assert!(!writer.is_null() && !reader.is_null());

        let tuned = ZeroDdsEndpointQos {
            deadline_ns: 500_000_000,
            lifespan_ns: 2_000_000_000,
            ..qos
        };
        assert_eq!(zerodds_writer_set_qos(writer, &tuned), 0);
        assert_eq!(zerodds_reader_set_qos(reader, &tuned), 0);

        // RELIABILITY and DURABILITY are immutable once the endpoint exists.
        let best_effort = ZeroDdsEndpointQos {
            reliable: 0,
            ..tuned
        };
        assert_eq!(zerodds_writer_set_qos(writer, &best_effort), -10);
        assert_eq!(zerodds_reader_set_qos(reader, &best_effort), -10);
        let durable = ZeroDdsEndpointQos {
            transient_local: 1,
            ..tuned
        };
        assert_eq!(zerodds_writer_set_qos(writer, &durable), -10);

        // DDS 1.4 §2.2.3: HISTORY, RESOURCE_LIMITS and the `max_blocking_time`
        // of RELIABILITY are "Changeable: No" as well.
        let keep_last = ZeroDdsEndpointQos {
            history_keep_all: 0,
            history_depth: 4,
            ..tuned
        };
        assert_eq!(zerodds_writer_set_qos(writer, &keep_last), -10);
        assert_eq!(zerodds_reader_set_qos(reader, &keep_last), -10);
        let limited = ZeroDdsEndpointQos {
            max_samples: 100,
            ..tuned
        };
        assert_eq!(zerodds_writer_set_qos(writer, &limited), -10);
        assert_eq!(zerodds_reader_set_qos(reader, &limited), -10);
        let patient = ZeroDdsEndpointQos {
            max_blocking_time_ns: 7_000_000_000,
            ..tuned
        };
        assert_eq!(zerodds_writer_set_qos(writer, &patient), -10);
        // Nothing was changed by the refusals: the tuned values still apply.
        assert_eq!(zerodds_writer_set_qos(writer, &tuned), 0);
        assert_eq!(zerodds_reader_set_qos(reader, &tuned), 0);

        zerodds_reader_destroy(reader);
        zerodds_writer_destroy(writer);
        zerodds_runtime_destroy(rt);
    }
}
