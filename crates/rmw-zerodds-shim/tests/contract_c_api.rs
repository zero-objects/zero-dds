// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `rmw-zerodds-shim` with `zerodds-c-api`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-c-api` (as `zerodds`, the crate's
//! `[lib]` name):
//! - `src/lib.rs`, `rmw_zerodds_init_with_security`:
//!   `runtime_create_ros_from_source` over the layers
//!   `ros2_config::process_source` loaded; with the feature `security` first
//!   `security_ffi::runtime_create_secure_ros_from_source` with the enclave
//!   rcl passed (`Ok(None)`: no enclave, the plain path applies);
//!   `zerodds_runtime_participant_guid` for the participant gid; the
//!   `ros_discovery_info` writer and reader through
//!   `zerodds_writer_create_qos` / `zerodds_reader_create_qos` with
//!   `zerodds_endpoint_qos_default` changed to RELIABLE, TRANSIENT_LOCAL,
//!   KEEP_LAST(1), the reader KEEP_ALL; `zerodds_reader_set_data_callback`
//!   (`ZeroDdsDataCallback`), `zerodds_runtime_add_status_callback`,
//!   `zerodds_writer_write` of the participant info behind its 4-octet
//!   encapsulation header, the `zerodds_*_destroy` calls on shutdown.
//! - `src/qos.rs`: `Resolved::endpoint` fills a `ZeroDdsEndpointQos` over
//!   `zerodds_endpoint_qos_default` (a ROS profile has no RESOURCE_LIMITS).
//! - `src/lib.rs`, publishers and subscriptions:
//!   `zerodds_writer_create_qos_user_data` /
//!   `zerodds_reader_create_qos_user_data` (NoKey, USER_DATA
//!   `typehash=…`), `zerodds_writer_set_wire_extensibility(writer, 0)`,
//!   `zerodds_writer_guid`, `zerodds_reader_guid`, `zerodds_writer_write`,
//!   `zerodds_reader_set_data_callback_ex` (`ZeroDdsSampleOrigin`),
//!   `zerodds_reader_set_time_based_filter_ns`,
//!   `zerodds_{writer,reader}_unicast_locators` (`ZeroDdsLocator`, a NULL
//!   `out` asks for the count), `zerodds_{writer,reader}_matched_count`,
//!   `zerodds_{writer,reader}_get_status` (`ZeroDdsWriterStatus`,
//!   `ZeroDdsReaderStatus`, also in `src/events.rs`),
//!   `zerodds_writer_assert_liveliness`,
//!   `zerodds_writer_wait_for_acknowledgments` (`0` acknowledged, `1`
//!   timeout, else error), `zerodds_reader_set_content_filter` (a NULL
//!   expression withdraws the filter), `zerodds_buffer_free`,
//!   `zerodds_writer_{loan_message,commit_loan,discard_loan}`,
//!   `runtime_config` for `ros2.time_based_filter` and `delivery.mode`,
//!   `zerodds_runtime_for_each_{publication,subscription}`
//!   (`ZeroDdsTopicCallback`) and `..._endpoint` (`ZeroDdsEndpointCallback`,
//!   `ZeroDdsEndpointInfo`, which `rmw_c/rmw_zerodds.c` declares again as
//!   `zerodds_endpoint_info_t`).
//! - with `flatdata-loan`: `zerodds_writer_set_delivery_mode` with `1`
//!   (RawSameHost), `zerodds_writer_enable_shm_loan`,
//!   `zerodds_reader_enable_shm`, `zerodds_reader_take_shm`,
//!   `zerodds_reader_release_shm`, `zerodds_reader_raw_wait` (the doorbell
//!   wakes `rmw_wait` on `0` only); with `delivery-iceoryx`: mode `2`,
//!   `zerodds_writer_enable_iceoryx`, `zerodds_reader_enable_iceoryx`.
//! - `src/services.rs`: `runtime_arc`, `user_writer_config` (its USER_DATA
//!   replaced) and `user_reader_config` (with the KEEP_LAST depth of the
//!   inbox), `apply_user_writer_qos` for the client and service endpoints.
//!
//! What it relies on and is checked here:
//! - The endpoint QoS default is the DDS default, and a resolved ROS profile
//!   reaches the writer and reader configuration field by field: the
//!   flags, the times (`0` infinite, nanoseconds otherwise), the liveliness
//!   kind, USER_DATA as given, the KEEP_LAST depth or KEEP_ALL of the
//!   reader, unlimited RESOURCE_LIMITS; a value outside a field's values is
//!   refused by name instead of guessed.
//! - The plain ROS constructor refuses a configuration that names an
//!   enclave (no silent unsecured participant), and refuses a domain that
//!   has no port before it starts anything; with `security`, the secured
//!   constructor leaves a configuration without an enclave to the plain path
//!   and refuses an enclave that does not load.
//! - The runtime helpers answer `None` for a NULL runtime.
//! - Where the rustdoc in `include/zerodds.h` allows a NULL handle, the call
//!   answers it as documented, or with a status other than `ZERODDS_OK`
//!   where the rustdoc names no value; no callback runs.
//! - The status codes, the delivery modes `1` and `2` the shim passes as
//!   literals, the layout of `ZeroDdsEndpointInfo` that the C layer
//!   declares again, the callback types, the status and locator fields the
//!   shim reads, and the signatures of the calls that need a runtime.
//!
//! Oracles: `docs/specs/zerodds-c-api-1.0.md` §8 (status codes); the rustdoc
//! of each call in `include/zerodds.h` (`zerodds_endpoint_qos_default`: the
//! DDS 1.4 defaults; the fields of `ZeroDdsEndpointQos`; "Returns `NULL` for
//! a NULL argument", "`0` on a NULL handle", "0 for NULL", "`-1` for a NULL
//! writer", "`-1` on a NULL/unknown reader"); the rustdoc of
//! `user_writer_config` / `user_reader_config` ("An invalid QoS value,
//! named"; `None` = KEEP_ALL), `runtime_config` / `runtime_arc` (NULL
//! allowed), `runtime_create_secure_ros_from_source` (`Ok(None)` without an
//! enclave; an enclave that does not load is an error) and of
//! `RuntimeConfig::security_enclave_dir` in `zerodds-dcps` ("a build without
//! the `security` feature and the C-API's plain constructors refuse it");
//! DDS 1.4 §2.2.3 (default QoS: KEEP_LAST(1), VOLATILE, AUTOMATIC, infinite
//! DEADLINE, LIFESPAN and lease, LENGTH_UNLIMITED), §2.2.3.14 (a DataWriter
//! is RELIABLE by default, `max_blocking_time` 100 ms), §2.2.3.19 (`depth <=
//! max_samples_per_instance`), §2.2.3.1 (USER_DATA, `sequence<octet>`);
//! DDSI-RTPS 2.5 §9.3.2 (`Duration_t`: fraction in 2^-32 s,
//! `DURATION_INFINITE` = {0x7fffffff, 0xffffffff}), §9.6.2.2 (the discovery
//! port `PB + DG * domainId + d0` with PB 7400, DG 250 is a 16-bit UDP
//! port); `docs/specs/zerodds-delivery-modes-1.0.md` §4 (`0` Portable, `1`
//! RawSameHost, `2` Iceoryx); the declaration `zerodds_endpoint_info_t` in
//! `rmw_c/rmw_zerodds.c` under the LP64 C ABI; the Rust reference on
//! `Option<extern "C" fn>` (the null-pointer optimisation makes it a
//! nullable function pointer).
//!
//! Every call that acts on a writer, a reader or a runtime needs a started
//! runtime, which binds sockets; the integration pillar runs them. Here such
//! a call is checked for its signature, through a function pointer of the
//! type the shim calls it with, and for its answer to a NULL handle where
//! the rustdoc allows one. Not checked here: the encapsulation header the
//! writer stamps over the body, the samples the callbacks receive, the
//! status values, the locators. No socket is opened.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use core::ffi::{c_char, c_int, c_void};
use core::ptr;
use core::sync::atomic::{AtomicUsize, Ordering};
use std::ffi::CString;
use std::path::{Path, PathBuf};

use rmw_zerodds::qos::Resolved;
use zerodds::{
    ZeroDdsDataCallback, ZeroDdsDataCallbackEx, ZeroDdsEndpointCallback, ZeroDdsEndpointInfo,
    ZeroDdsEndpointQos, ZeroDdsLocator, ZeroDdsReader, ZeroDdsReaderStatus, ZeroDdsRuntime,
    ZeroDdsSampleOrigin, ZeroDdsStatus, ZeroDdsTopicCallback, ZeroDdsWriter, ZeroDdsWriterStatus,
};
use zerodds_config::CommandLine;
use zerodds_dcps::runtime::{ConfigSource, DcpsRuntime};
use zerodds_qos::{DurabilityKind, Duration, LivelinessKind, OwnershipKind};
use zerodds_rtps::wire_types::EntityId;

/// DDSI-RTPS 2.5 §9.3.2: `DURATION_INFINITE`.
const DURATION_INFINITE: Duration = Duration {
    seconds: 0x7fff_ffff,
    fraction: 0xffff_ffff,
};

/// DDS 1.4 §2.3.3: `LENGTH_UNLIMITED`.
const LENGTH_UNLIMITED: i32 = -1;

/// A runtime file of this test alone, so that no system file takes part.
fn runtime_file(name: &str, text: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "zerodds-rmw-contract-c-api-{}-{name}.yaml",
        std::process::id()
    ));
    std::fs::write(&path, text).unwrap();
    path
}

/// The layers of `file` alone: no system directory, an empty environment.
fn source_of(file: &Path) -> ConfigSource {
    ConfigSource::from_command_line_with_env(
        &CommandLine::default().with_config(file.to_path_buf()),
        |_| None,
    )
    .unwrap()
}

/// A directory that holds no enclave.
fn missing_enclave() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "zerodds-rmw-contract-c-api-{}-no-enclave",
        std::process::id()
    ));
    assert!(!dir.exists(), "{} must not exist", dir.display());
    dir
}

/// The endpoint QoS the shim builds for a resolved ROS profile.
fn resolved(keep_last: Option<u32>) -> Resolved {
    Resolved {
        reliable: true,
        transient_local: true,
        keep_last,
        deadline_ns: 1_500_000_000,
        lifespan_ns: 2_000_000_000,
        liveliness_kind: 2,
        liveliness_lease_ns: 3_000_000_000,
        avoid_ros_namespace_conventions: false,
    }
}

/// Counts the calls of the callbacks below; `user_data` points at it.
fn counter(calls: &AtomicUsize) -> *mut c_void {
    ptr::from_ref(calls).cast_mut().cast()
}

fn count(user_data: *mut c_void) {
    // SAFETY: every caller passes `counter(&calls)` of a live `AtomicUsize`.
    unsafe { &*user_data.cast::<AtomicUsize>() }.fetch_add(1, Ordering::Relaxed);
}

extern "C" fn on_topic(user_data: *mut c_void, _topic: *const c_char, _type_name: *const c_char) {
    count(user_data);
}

extern "C" fn on_endpoint(user_data: *mut c_void, _info: *const ZeroDdsEndpointInfo) {
    count(user_data);
}

extern "C" fn on_status(user_data: *mut c_void) {
    count(user_data);
}

extern "C" fn on_data(user_data: *mut c_void, _: *const u8, _: usize, _: u8, _: u8) {
    count(user_data);
}

extern "C" fn on_data_ex(
    user_data: *mut c_void,
    _: *const u8,
    _: usize,
    _: u8,
    _: u8,
    _: *const ZeroDdsSampleOrigin,
) {
    count(user_data);
}

#[test]
fn dcps_2_2_3_the_endpoint_qos_default_is_the_dds_default() {
    let q = zerodds::zerodds_endpoint_qos_default();
    assert_eq!(q.reliable, 1, "§2.2.3.14: a DataWriter is RELIABLE");
    assert_eq!(q.transient_local, 0, "VOLATILE");
    assert_eq!(
        (q.history_keep_all, q.history_depth),
        (0, 1),
        "KEEP_LAST(1)"
    );
    assert_eq!(
        (q.deadline_ns, q.lifespan_ns, q.liveliness_lease_ns),
        (0, 0, 0),
        "infinite, which the struct writes as 0"
    );
    assert_eq!(q.liveliness_kind, 0, "AUTOMATIC");
    assert_eq!(
        (q.max_samples, q.max_instances, q.max_samples_per_instance),
        (LENGTH_UNLIMITED, LENGTH_UNLIMITED, LENGTH_UNLIMITED),
        "a ROS profile names no RESOURCE_LIMITS"
    );
    assert_eq!(q.max_blocking_time_ns, 100_000_000, "§2.2.3.14: 100 ms");
}

#[test]
fn dcps_2_2_3_a_resolved_publisher_profile_reaches_the_writer_configuration() {
    let user_data = b"typehash=RIHS01_00;".to_vec();
    let q = resolved(Some(10)).endpoint();
    let cfg = zerodds::user_writer_config(
        "rt/chatter",
        "std_msgs::msg::dds_::String_",
        &q,
        user_data.clone(),
    )
    .unwrap();
    assert_eq!(cfg.topic_name, "rt/chatter");
    assert_eq!(cfg.type_name, "std_msgs::msg::dds_::String_");
    assert!(cfg.reliable);
    assert_eq!(cfg.durability, DurabilityKind::TransientLocal);
    // §9.3.2: 1.5 s is 1 s and 2^31 * 2^-32 s.
    assert_eq!(
        cfg.deadline.period,
        Duration {
            seconds: 1,
            fraction: 1 << 31
        }
    );
    assert_eq!(
        cfg.lifespan.duration,
        Duration {
            seconds: 2,
            fraction: 0
        }
    );
    assert_eq!(cfg.liveliness.kind, LivelinessKind::ManualByTopic);
    assert_eq!(
        cfg.liveliness.lease_duration,
        Duration {
            seconds: 3,
            fraction: 0
        }
    );
    assert_eq!(cfg.user_data, user_data, "§2.2.3.1: the octets as given");
    assert_eq!(cfg.ownership, OwnershipKind::Shared);
    assert!(cfg.partition.is_empty());
    assert!(matches!(
        cfg.type_identifier,
        zerodds_types::TypeIdentifier::None
    ));
    assert_eq!(
        cfg.data_representation_offer, None,
        "the runtime's offer applies"
    );
}

#[test]
fn dcps_2_2_3_a_resolved_subscription_profile_reaches_the_reader_configuration() {
    let q = Resolved {
        reliable: false,
        transient_local: false,
        keep_last: Some(7),
        deadline_ns: 0,
        lifespan_ns: 0,
        liveliness_kind: 1,
        liveliness_lease_ns: 0,
        avoid_ros_namespace_conventions: false,
    }
    .endpoint();
    let (cfg, keep_last) =
        zerodds::user_reader_config("rt/chatter", "T", &q, b"ud".to_vec()).unwrap();
    assert!(!cfg.reliable, "`0` BEST_EFFORT");
    assert_eq!(cfg.durability, DurabilityKind::Volatile);
    assert_eq!(cfg.deadline.period, DURATION_INFINITE, "`0` is infinite");
    assert_eq!(cfg.liveliness.kind, LivelinessKind::ManualByParticipant);
    assert_eq!(cfg.liveliness.lease_duration, DURATION_INFINITE);
    assert_eq!(
        (
            cfg.resource_limits.max_samples,
            cfg.resource_limits.max_instances,
            cfg.resource_limits.max_samples_per_instance
        ),
        (LENGTH_UNLIMITED, LENGTH_UNLIMITED, LENGTH_UNLIMITED)
    );
    assert_eq!(cfg.user_data, b"ud");
    assert_eq!(keep_last, Some(7), "the depth that bounds the inbox");
}

#[test]
fn dcps_2_2_3_18_keep_all_and_the_largest_keep_last_depth_reach_the_reader() {
    // `resolve` caps a ROS depth at u32::MAX; with LENGTH_UNLIMITED
    // max_samples_per_instance no depth exceeds it (§2.2.3.19).
    let (_, keep_last) =
        zerodds::user_reader_config("t", "T", &resolved(Some(u32::MAX)).endpoint(), Vec::new())
            .unwrap();
    assert_eq!(keep_last, Some(u32::MAX as usize));
    let (_, keep_last) =
        zerodds::user_reader_config("t", "T", &resolved(None).endpoint(), Vec::new()).unwrap();
    assert_eq!(keep_last, None, "KEEP_ALL");
    // `ros_discovery_info` as `rmw_zerodds_init_with_security` creates it.
    let writer_qos = ZeroDdsEndpointQos {
        reliable: 1,
        transient_local: 1,
        history_keep_all: 0,
        history_depth: 1,
        ..zerodds::zerodds_endpoint_qos_default()
    };
    let reader_qos = ZeroDdsEndpointQos {
        history_keep_all: 1,
        ..writer_qos
    };
    let writer =
        zerodds::user_writer_config("ros_discovery_info", "T", &writer_qos, Vec::new()).unwrap();
    assert!(writer.reliable);
    assert_eq!(writer.durability, DurabilityKind::TransientLocal);
    let (reader, keep_last) =
        zerodds::user_reader_config("ros_discovery_info", "T", &reader_qos, Vec::new()).unwrap();
    assert!(reader.reliable);
    assert_eq!(reader.durability, DurabilityKind::TransientLocal);
    assert_eq!(keep_last, None, "every announcement reaches the callback");
}

#[test]
fn dcps_2_2_3_19_an_endpoint_qos_outside_its_values_is_refused_by_name() {
    let base = zerodds::zerodds_endpoint_qos_default();
    for (q, field) in [
        (
            ZeroDdsEndpointQos {
                reliable: 2,
                ..base
            },
            "reliable",
        ),
        (
            ZeroDdsEndpointQos {
                transient_local: -1,
                ..base
            },
            "transient_local",
        ),
        (
            ZeroDdsEndpointQos {
                history_keep_all: 2,
                ..base
            },
            "history_keep_all",
        ),
        (
            ZeroDdsEndpointQos {
                liveliness_kind: 3,
                ..base
            },
            "liveliness_kind",
        ),
        (
            ZeroDdsEndpointQos {
                history_depth: 0,
                ..base
            },
            "history_depth",
        ),
        (
            ZeroDdsEndpointQos {
                max_samples: 0,
                ..base
            },
            "max_samples",
        ),
        (
            ZeroDdsEndpointQos {
                history_depth: 3,
                max_samples_per_instance: 2,
                ..base
            },
            "history_depth",
        ),
    ] {
        let writer = zerodds::user_writer_config("t", "T", &q, Vec::new()).err();
        let reader = zerodds::user_reader_config("t", "T", &q, Vec::new()).err();
        for refusal in [writer, reader] {
            let refusal = refusal.unwrap_or_else(|| panic!("{q:?} accepted"));
            assert!(refusal.contains(field), "{refusal} does not name {field}");
        }
    }
}

#[test]
fn a_configuration_that_names_an_enclave_is_refused_by_the_plain_ros_constructor() {
    let enclave = missing_enclave();
    let file = runtime_file(
        "enclave",
        &format!("security:\n  enclave_dir: {}\n", enclave.display()),
    );
    let source = source_of(&file);
    // The refusal comes before the runtime starts; a leased domain keeps a
    // regression off another process's domain.
    let domain = u32::try_from(zerodds_test_domains::unique_domain()).unwrap();
    let created = zerodds::runtime_create_ros_from_source(domain, &source);
    let _ = std::fs::remove_file(file);
    if let Ok(rt) = created {
        // SAFETY: a runtime this call created, destroyed once.
        unsafe { zerodds::zerodds_runtime_destroy(rt) };
        panic!("an unsecured participant for a configuration naming an enclave");
    }
}

#[test]
fn rtps_9_6_2_2_a_domain_without_a_discovery_port_gives_no_ros_runtime() {
    // 7400 + 250 * domain is no 16-bit port for either domain.
    for domain in [u32::MAX, 1 << 31] {
        if let Ok(rt) = zerodds::runtime_create_ros_from_source(domain, &ConfigSource::default()) {
            // SAFETY: a runtime this call created, destroyed once.
            unsafe { zerodds::zerodds_runtime_destroy(rt) };
            panic!("a runtime on domain {domain}");
        }
    }
}

#[test]
fn the_runtime_helpers_answer_none_for_a_null_runtime() {
    // SAFETY: NULL is allowed by the rustdoc of both helpers.
    unsafe {
        assert!(zerodds::runtime_arc(ptr::null()).is_none());
        assert!(zerodds::runtime_config(ptr::null()).is_none());
    }
}

#[test]
fn the_writer_calls_answer_a_null_writer_as_documented() {
    let topic = CString::new("rt/chatter").unwrap();
    let type_name = CString::new("T").unwrap();
    let qos = zerodds::zerodds_endpoint_qos_default();
    let user_data = b"typehash=RIHS01_00;";
    // SAFETY: NULL runtime, NUL-terminated strings, a live QoS and USER_DATA
    // readable for its length; the rustdoc allows a NULL argument.
    unsafe {
        assert!(
            zerodds::zerodds_writer_create_qos(
                ptr::null_mut(),
                topic.as_ptr(),
                type_name.as_ptr(),
                0,
                &qos
            )
            .is_null()
        );
        assert!(
            zerodds::zerodds_writer_create_qos_user_data(
                ptr::null_mut(),
                topic.as_ptr(),
                type_name.as_ptr(),
                0,
                &qos,
                user_data.as_ptr(),
                user_data.len()
            )
            .is_null()
        );
    }
    let mut guid = [0xAA_u8; 16];
    let mut status = ZeroDdsWriterStatus::default();
    // SAFETY: NULL writer, which each rustdoc allows; the out pointers are
    // locals of the sizes named there.
    unsafe {
        assert_eq!(zerodds::zerodds_writer_matched_count(ptr::null_mut()), 0);
        assert_eq!(
            zerodds::zerodds_writer_unicast_locators(ptr::null_mut(), ptr::null_mut(), 0),
            0
        );
        assert_ne!(
            zerodds::zerodds_writer_get_status(ptr::null_mut(), &mut status),
            ZeroDdsStatus::Ok as c_int
        );
        assert_ne!(
            zerodds::zerodds_writer_guid(ptr::null_mut(), guid.as_mut_ptr()),
            ZeroDdsStatus::Ok as c_int
        );
        assert_eq!(
            zerodds::zerodds_writer_assert_liveliness(ptr::null_mut()),
            -1
        );
        // Neither `0` (acknowledged) nor `1` (timeout): the shim's error.
        assert_eq!(
            zerodds::zerodds_writer_wait_for_acknowledgments(ptr::null_mut(), 0),
            -1
        );
        zerodds::zerodds_writer_destroy(ptr::null_mut());
    }
}

#[test]
fn the_reader_calls_answer_a_null_reader_as_documented() {
    let topic = CString::new("rt/chatter").unwrap();
    let type_name = CString::new("T").unwrap();
    let qos = zerodds::zerodds_endpoint_qos_default();
    // SAFETY: NULL runtime, NUL-terminated strings, a live QoS; the rustdoc
    // allows a NULL argument (as for the writer).
    unsafe {
        assert!(
            zerodds::zerodds_reader_create_qos(
                ptr::null_mut(),
                topic.as_ptr(),
                type_name.as_ptr(),
                0,
                &qos
            )
            .is_null()
        );
        assert!(
            zerodds::zerodds_reader_create_qos_user_data(
                ptr::null_mut(),
                topic.as_ptr(),
                type_name.as_ptr(),
                0,
                &qos,
                ptr::null(),
                0
            )
            .is_null()
        );
    }
    let mut guid = [0xAA_u8; 16];
    let mut status = ZeroDdsReaderStatus::default();
    // SAFETY: NULL reader, which each rustdoc allows; the out pointers are
    // locals of the sizes named there; NULL expression with no parameters.
    unsafe {
        assert_eq!(zerodds::zerodds_reader_matched_count(ptr::null_mut()), 0);
        assert_eq!(
            zerodds::zerodds_reader_unicast_locators(ptr::null_mut(), ptr::null_mut(), 0),
            0
        );
        assert_ne!(
            zerodds::zerodds_reader_get_status(ptr::null_mut(), &mut status),
            ZeroDdsStatus::Ok as c_int
        );
        assert_ne!(
            zerodds::zerodds_reader_guid(ptr::null_mut(), guid.as_mut_ptr()),
            ZeroDdsStatus::Ok as c_int
        );
        assert_eq!(
            zerodds::zerodds_reader_set_time_based_filter_ns(ptr::null_mut(), 1_000_000),
            -1
        );
        assert_ne!(
            zerodds::zerodds_reader_set_content_filter(
                ptr::null_mut(),
                ptr::null(),
                ptr::null(),
                ptr::null(),
                ptr::null(),
                0
            ),
            ZeroDdsStatus::Ok as c_int
        );
        zerodds::zerodds_reader_destroy(ptr::null_mut());
        zerodds::zerodds_buffer_free(ptr::null_mut(), 0);
    }
}

#[test]
fn the_runtime_calls_answer_a_null_runtime_without_a_callback() {
    let calls = AtomicUsize::new(0);
    let mut guid = [0_u8; 16];
    // SAFETY: NULL runtime, which each rustdoc allows; `guid` holds 16
    // octets; the callbacks only count through `counter(&calls)`.
    unsafe {
        // `make_participant_gid` copies the GUID on `0` only.
        assert_ne!(
            zerodds::zerodds_runtime_participant_guid(ptr::null_mut(), guid.as_mut_ptr()),
            ZeroDdsStatus::Ok as c_int
        );
        for rc in [
            zerodds::zerodds_runtime_for_each_publication(
                ptr::null_mut(),
                Some(on_topic),
                counter(&calls),
            ),
            zerodds::zerodds_runtime_for_each_subscription(
                ptr::null_mut(),
                Some(on_topic),
                counter(&calls),
            ),
            zerodds::zerodds_runtime_for_each_publication_endpoint(
                ptr::null_mut(),
                Some(on_endpoint),
                counter(&calls),
            ),
            zerodds::zerodds_runtime_for_each_subscription_endpoint(
                ptr::null_mut(),
                Some(on_endpoint),
                counter(&calls),
            ),
            // The shim takes its `Arc` back on anything but `0`.
            zerodds::zerodds_runtime_add_status_callback(
                ptr::null_mut(),
                Some(on_status),
                counter(&calls),
            ),
        ] {
            assert_ne!(rc, ZeroDdsStatus::Ok as c_int);
        }
        zerodds::zerodds_runtime_destroy(ptr::null_mut());
    }
    assert_eq!(calls.load(Ordering::Relaxed), 0);
}

#[test]
fn c_api_8_the_status_codes_are_those_of_the_specification() {
    for (status, code) in [
        (ZeroDdsStatus::Ok, 0),
        (ZeroDdsStatus::Error, -1),
        (ZeroDdsStatus::BadHandle, -2),
        (ZeroDdsStatus::InvalidUtf8, -3),
        (ZeroDdsStatus::Timeout, -4),
        (ZeroDdsStatus::PreconditionNotMet, -5),
        (ZeroDdsStatus::BadParameter, -6),
        (ZeroDdsStatus::NoData, -7),
        (ZeroDdsStatus::OutOfResources, -8),
        (ZeroDdsStatus::NotEnabled, -9),
        (ZeroDdsStatus::ImmutablePolicy, -10),
        (ZeroDdsStatus::InconsistentPolicy, -11),
        (ZeroDdsStatus::AlreadyDeleted, -12),
        (ZeroDdsStatus::Unsupported, -13),
        (ZeroDdsStatus::IllegalOperation, -14),
    ] {
        assert_eq!(status as c_int, code, "{status:?}");
    }
}

#[cfg(target_pointer_width = "64")]
#[test]
fn the_endpoint_info_has_the_layout_the_c_layer_declares() {
    use core::mem::{align_of, offset_of, size_of};
    // Every member, so that a new one fails here before the C declaration
    // in `rmw_c/rmw_zerodds.c` reads the wrong offsets.
    let info = ZeroDdsEndpointInfo {
        topic_name: ptr::null(),
        type_name: ptr::null(),
        endpoint_guid: ptr::null(),
        reliable: 1_u8,
        transient_local: 0_u8,
        deadline_ns: 0_u64,
        lifespan_ns: 0_u64,
        liveliness_lease_ns: 0_u64,
        user_data: ptr::null(),
        user_data_len: 0_usize,
    };
    assert_eq!(info.reliable, 1);
    // `zerodds_endpoint_info_t` under LP64: three pointers, two uint8_t, six
    // octets of padding, three uint64_t, a pointer and a size_t.
    assert_eq!(offset_of!(ZeroDdsEndpointInfo, topic_name), 0);
    assert_eq!(offset_of!(ZeroDdsEndpointInfo, type_name), 8);
    assert_eq!(offset_of!(ZeroDdsEndpointInfo, endpoint_guid), 16);
    assert_eq!(offset_of!(ZeroDdsEndpointInfo, reliable), 24);
    assert_eq!(offset_of!(ZeroDdsEndpointInfo, transient_local), 25);
    assert_eq!(offset_of!(ZeroDdsEndpointInfo, deadline_ns), 32);
    assert_eq!(offset_of!(ZeroDdsEndpointInfo, lifespan_ns), 40);
    assert_eq!(offset_of!(ZeroDdsEndpointInfo, liveliness_lease_ns), 48);
    assert_eq!(offset_of!(ZeroDdsEndpointInfo, user_data), 56);
    assert_eq!(offset_of!(ZeroDdsEndpointInfo, user_data_len), 64);
    assert_eq!(size_of::<ZeroDdsEndpointInfo>(), 72);
    assert_eq!(align_of::<ZeroDdsEndpointInfo>(), 8);
    // The C layer passes `zerodds_endpoint_cb_t` and `zerodds_topic_cb_t`,
    // plain nullable function pointers, where Rust takes an `Option`.
    assert_eq!(
        size_of::<Option<ZeroDdsEndpointCallback>>(),
        size_of::<*const c_void>()
    );
    assert_eq!(
        size_of::<Option<ZeroDdsTopicCallback>>(),
        size_of::<*const c_void>()
    );
}

#[test]
fn the_calls_that_need_a_runtime_keep_the_signatures_the_shim_calls() {
    // The callback types the shim's `extern "C"` functions coerce to.
    let _: ZeroDdsTopicCallback = on_topic;
    let _: ZeroDdsEndpointCallback = on_endpoint;
    let _: ZeroDdsDataCallback = on_data;
    let _: ZeroDdsDataCallbackEx = on_data_ex;
    let _: extern "C" fn(*mut c_void) = on_status;
    // Rust helpers.
    let _: fn(&DcpsRuntime, EntityId, &ZeroDdsEndpointQos) -> Result<(), String> =
        zerodds::apply_user_writer_qos;
    let _: fn(u32, &ConfigSource) -> Result<*mut ZeroDdsRuntime, String> =
        zerodds::runtime_create_ros_from_source;
    // C calls on a live writer or reader.
    let _: unsafe extern "C" fn(*mut ZeroDdsWriter, *const u8, usize) -> c_int =
        zerodds::zerodds_writer_write;
    let _: unsafe extern "C" fn(*mut ZeroDdsWriter, c_int) -> c_int =
        zerodds::zerodds_writer_set_wire_extensibility;
    let _: unsafe extern "C" fn(*mut ZeroDdsWriter, usize, *mut *mut u8, *mut usize) -> c_int =
        zerodds::zerodds_writer_loan_message;
    let _: unsafe extern "C" fn(*mut ZeroDdsWriter, *mut u8, usize) -> c_int =
        zerodds::zerodds_writer_commit_loan;
    let _: unsafe extern "C" fn(*mut ZeroDdsWriter, *mut u8, usize) -> c_int =
        zerodds::zerodds_writer_discard_loan;
    let _: unsafe extern "C" fn(*mut ZeroDdsWriter, *mut ZeroDdsLocator, usize) -> usize =
        zerodds::zerodds_writer_unicast_locators;
    let _: unsafe extern "C" fn(*mut ZeroDdsReader, *mut ZeroDdsLocator, usize) -> usize =
        zerodds::zerodds_reader_unicast_locators;
    let _: unsafe extern "C" fn(
        *mut ZeroDdsReader,
        Option<ZeroDdsDataCallback>,
        *mut c_void,
    ) -> c_int = zerodds::zerodds_reader_set_data_callback;
    let _: unsafe extern "C" fn(
        *mut ZeroDdsReader,
        Option<ZeroDdsDataCallbackEx>,
        *mut c_void,
    ) -> c_int = zerodds::zerodds_reader_set_data_callback_ex;
    let _: unsafe extern "C" fn(
        *mut ZeroDdsReader,
        *const c_char,
        *const c_char,
        *const c_char,
        *const *const c_char,
        usize,
    ) -> c_int = zerodds::zerodds_reader_set_content_filter;
    let _: unsafe extern "C" fn(
        *mut ZeroDdsRuntime,
        Option<extern "C" fn(*mut c_void)>,
        *mut c_void,
    ) -> c_int = zerodds::zerodds_runtime_add_status_callback;
    // The members the shim reads, with the types it copies them into.
    let w = ZeroDdsWriterStatus::default();
    let _: (u64, u64, u64, u32, u64, u64) = (
        w.offered_deadline_missed_total,
        w.liveliness_lost_total,
        w.offered_incompatible_qos_total,
        w.offered_incompatible_qos_last_policy_id,
        w.matched_current,
        w.incompatible_type_total,
    );
    let r = ZeroDdsReaderStatus::default();
    let _: (u64, u64, u64, u64, u32, u64, u64, u64) = (
        r.requested_deadline_missed_total,
        r.liveliness_alive,
        r.liveliness_not_alive,
        r.requested_incompatible_qos_total,
        r.requested_incompatible_qos_last_policy_id,
        r.sample_lost_total,
        r.matched_current,
        r.incompatible_type_total,
    );
    let l = ZeroDdsLocator::default();
    let _: (i32, u32, [u8; 16]) = (l.kind, l.port, l.address);
    let o = ZeroDdsSampleOrigin::default();
    let _: ([u8; 16], i64, i64, [u8; 2]) = (
        o.writer_guid,
        o.source_timestamp_ns,
        o.sequence_number,
        o.representation_identifier,
    );
}

#[cfg(feature = "flatdata-loan")]
#[test]
fn delivery_modes_4_raw_same_host_is_1_iceoryx_2_and_a_null_doorbell_does_not_ring() {
    use zerodds::ZeroDdsDeliveryMode;
    assert_eq!(ZeroDdsDeliveryMode::Portable as c_int, 0);
    assert_eq!(ZeroDdsDeliveryMode::RawSameHost as c_int, 1);
    assert_eq!(ZeroDdsDeliveryMode::Iceoryx as c_int, 2);
    // SAFETY: NULL reader, which the rustdoc of `zerodds_reader_raw_wait`
    // allows; a timeout of 0 does not wait.
    let rc = unsafe { zerodds::zerodds_reader_raw_wait(ptr::null_mut(), 0) };
    assert_ne!(rc, ZeroDdsStatus::Ok as c_int, "the doorbell wakes on 0");
    let _: unsafe extern "C" fn(*mut ZeroDdsWriter, c_int) -> c_int =
        zerodds::zerodds_writer_set_delivery_mode;
    let _: unsafe extern "C" fn(*mut ZeroDdsWriter, *const c_char, usize, usize) -> c_int =
        zerodds::zerodds_writer_enable_shm_loan;
    let _: unsafe extern "C" fn(*mut ZeroDdsReader, *const c_char, u8) -> c_int =
        zerodds::zerodds_reader_enable_shm;
    let _: unsafe extern "C" fn(*mut ZeroDdsReader, *mut *const u8, *mut usize, *mut u32) -> c_int =
        zerodds::zerodds_reader_take_shm;
    let _: unsafe extern "C" fn(*mut ZeroDdsReader, u32) -> c_int =
        zerodds::zerodds_reader_release_shm;
}

#[cfg(feature = "delivery-iceoryx")]
#[test]
fn delivery_modes_3_3_the_iceoryx_calls_keep_their_signatures() {
    let _: unsafe extern "C" fn(*mut ZeroDdsWriter, *const c_char, usize) -> c_int =
        zerodds::zerodds_writer_enable_iceoryx;
    let _: unsafe extern "C" fn(*mut ZeroDdsReader, *const c_char) -> c_int =
        zerodds::zerodds_reader_enable_iceoryx;
}

#[cfg(feature = "security")]
#[test]
fn without_an_enclave_the_secured_constructor_leaves_the_plain_path() {
    let file = runtime_file("secure-none", "\n");
    let source = source_of(&file);
    let created = zerodds::security_ffi::runtime_create_secure_ros_from_source(0, &source, None);
    let _ = std::fs::remove_file(file);
    match created {
        Ok(None) => {}
        Ok(Some(rt)) => {
            // SAFETY: a runtime this call created, destroyed once.
            unsafe { zerodds::zerodds_runtime_destroy(rt) };
            panic!("a secured runtime without an enclave");
        }
        Err(e) => panic!("{e}"),
    }
}

#[cfg(feature = "security")]
#[test]
fn an_enclave_that_does_not_load_is_an_error_not_a_plain_participant() {
    let enclave = missing_enclave();
    let file = runtime_file("secure-missing", "\n");
    let source = source_of(&file);
    // The enclave rcl passed; its profile does not load, so nothing starts.
    let created = zerodds::security_ffi::runtime_create_secure_ros_from_source(
        0,
        &source,
        Some(enclave.as_path()),
    );
    let _ = std::fs::remove_file(file);
    match created {
        Err(_) => {}
        Ok(Some(rt)) => {
            // SAFETY: a runtime this call created, destroyed once.
            unsafe { zerodds::zerodds_runtime_destroy(rt) };
            panic!("a runtime from an enclave that does not exist");
        }
        Ok(None) => panic!("the enclave rcl named was dropped"),
    }
}
