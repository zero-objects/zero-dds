// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds_writer_dispose_sample` / `_unregister_sample` /
//! `_unregister_with_dispose_sample` (D10, C-FFI part).
//!
//! Background: `zerodds_writer_dispose`/`_unregister`/
//! `_unregister_with_dispose` (`crates/zerodds-c-api/src/lib.rs`) take
//! only a 16-byte key hash and send `write_user_lifecycle(eid, kh, None,
//! …)` — a DATA with `PID_KEY_HASH` alone, no `serializedKey`. Measured
//! (OMG dds-rtps `Test_FinalInstanceState_1`): Eclipse Cyclone 11.0.1,
//! Fast DDS 3.6.1 and OpenDDS 3.35 do not treat that form as a dispose
//! (0/4); with `serializedKey` all three do (4/4). The `_sample`
//! functions here take the encoded sample instead and derive the real
//! KeyHash + canonical key holder from it through
//! `DcpsRuntime::writer_sample_key_material`, so the DATA they send
//! carries the full form.
//!
//! # Why a same-runtime reader, and how the serializedKey claim is checked
//!
//! `zerodds_reader_take`/`_take_endian` (the C-FFI's own byte-oriented
//! reader) deliberately discard lifecycle markers — a caller through
//! that path never sees NOT_ALIVE_DISPOSED at all, C-FFI or not. Seeing
//! it here needs the raw per-sample channel
//! `DcpsRuntime::register_user_reader_kind` hands back
//! (`UserSample::Lifecycle`), which is reached via `zerodds::runtime_arc`
//! — the `Arc<DcpsRuntime>` behind the same `zerodds_ZeroDdsRuntime` the
//! writer under test was created on, so writer and reader are the same
//! participant and delivery does not depend on the network.
//!
//! `UserSample::Lifecycle::key_holder` is `None` on that same-runtime
//! fast path regardless of what the writer sent (`intra_runtime_dispatch_
//! lifecycle`'s own doc comment: threading the writer's key holder
//! through it is an open follow-up), so it cannot be this test's proof
//! that the DATA carried a `serializedKey`. That proof is
//! `DcpsRuntime::writer_sample_key_material(writer_eid, &sample)` itself
//! — the exact call `zerodds_writer_dispose_sample` makes internally
//! before handing its result to `write_user_lifecycle` as `Some(holder)`
//! — returning a non-empty holder: exactly what keeps that call off the
//! hash-only fallback that files `writer.lifecycle.serialized_key_
//! unavailable` (a counter this test has no way to read back through the
//! C-FFI's public surface, since the default sink for a
//! `zerodds_runtime_create` runtime is `null_sink()`).
//!
//! `writer_eid` itself has no C-FFI accessor either; it is recovered from
//! `zerodds_writer_guid`'s last 4 bytes (RTPS Guid = 12-byte prefix + a
//! 4-byte EntityId, DDSI-RTPS 2.5 §8.2.4).

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::uninlined_format_args,
    missing_docs
)]

use std::ffi::CString;
use std::sync::Arc;
use std::time::Duration;

use zerodds::{
    ZeroDdsStatus, runtime_arc, zerodds_runtime_create, zerodds_runtime_destroy,
    zerodds_writer_create, zerodds_writer_create_typed, zerodds_writer_destroy,
    zerodds_writer_dispose_sample, zerodds_writer_guid, zerodds_writer_unregister_sample,
    zerodds_writer_unregister_with_dispose_sample, zerodds_writer_wait_for_matched,
    zerodds_writer_write,
};
use zerodds_dcps::runtime::{DcpsRuntime, UserReaderConfig, UserSample};
use zerodds_rtps::history_cache::ChangeKind;
use zerodds_rtps::wire_types::EntityId;
use zerodds_types::TypeObject;

const OK: i32 = ZeroDdsStatus::Ok as i32;
const PRECONDITION_NOT_MET: i32 = ZeroDdsStatus::PreconditionNotMet as i32;

/// Builds a serialized COMPLETE `TypeObject` for a small `@final` struct
/// with two `long` members, the first `@key` iff `keyed`. Mirrors the IDL
/// front end + `zerodds_types::from_ast` path a real `idlc`-generated
/// `type_object()` takes; hand-rolled here since this test has no
/// codegen step of its own.
fn small_struct_type_object(type_name: &str, keyed: bool) -> Vec<u8> {
    let key = if keyed { "@key " } else { "" };
    let idl = format!("@final struct {type_name} {{ {key}long id; long value; }};");
    let parsed = zerodds_idl_parse::parse(&idl);
    assert!(!parsed.has_errors(), "parse: {:?}", parsed.diagnostics());
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "writer_dispose_sample.idl");
    assert!(
        lowered.diagnostics.is_empty(),
        "lower: {:?}",
        lowered.diagnostics
    );
    let mut ast = lowered.ast;
    let report = ast.resolve_all_by_name().expect("resolve");
    assert!(report.is_clean(), "unresolved: {:?}", report.broken);
    let ast = Arc::new(ast);
    let type_ = zerodds_types::dynamic::DynamicType::find(&ast, type_name)
        .unwrap_or_else(|e| panic!("DynamicType::find({type_name}): {e:?}"));
    let complete = zerodds_types::from_ast::complete_type_object(&ast, type_.id())
        .expect("complete_type_object");
    TypeObject::Complete(complete)
        .to_bytes_le()
        .expect("encode TypeObject")
}

/// `@final` PLAIN_CDR body of `{ id, value }` (or `{ id }`-and-`{ id,
/// value }`-compatible either way — two `long`s, no padding, no
/// DHEADER): little-endian, matching the writer's default representation
/// and byte order (`user_writer_offer_first`/`big_endian_override`'s own
/// defaults, neither of which this test's writers change).
fn encode_two_longs(id: i32, value: i32) -> Vec<u8> {
    let mut out = Vec::with_capacity(8);
    out.extend_from_slice(&id.to_le_bytes());
    out.extend_from_slice(&value.to_le_bytes());
    out
}

/// Recovers a writer's `EntityId` from its 16-byte GUID (bytes 12..16,
/// DDSI-RTPS 2.5 §8.2.4) — the C-FFI opaque handle has no accessor of its
/// own, and the direct `DcpsRuntime` calls below need it.
///
/// # Safety
/// `writer` must be a valid, non-destroyed writer handle.
unsafe fn writer_entity_id(writer: *mut zerodds::ZeroDdsWriter) -> EntityId {
    let mut guid = [0u8; 16];
    // SAFETY: `writer` valid per caller pledge; `guid` is 16 writable bytes.
    let rc = unsafe { zerodds_writer_guid(writer, guid.as_mut_ptr()) };
    assert_eq!(rc, OK, "zerodds_writer_guid");
    EntityId::from_bytes(guid[12..16].try_into().expect("4 bytes"))
}

/// Registers a same-runtime native reader (bypassing every C-FFI
/// `zerodds_reader_*`/`zerodds_sub_*` entry point, all of which discard
/// lifecycle markers) so this test can observe `UserSample::Lifecycle`
/// directly. Registers `type_object` again on `rt` — a second
/// registration of the same bytes is an idempotent re-insert — so this
/// reader's own type resolves too, matching the writer's.
fn native_reader(
    rt: &Arc<DcpsRuntime>,
    topic_name: &str,
    type_name: &str,
    type_object: &[u8],
    keyed: bool,
) -> std::sync::mpsc::Receiver<UserSample> {
    let obj = TypeObject::from_bytes_le(type_object).expect("decode TypeObject");
    let hash = rt.register_type_object(obj).expect("register_type_object");
    let cfg = UserReaderConfig {
        topic_name: topic_name.to_string(),
        type_name: type_name.to_string(),
        reliable: true,
        type_identifier: zerodds_types::TypeIdentifier::EquivalenceHashComplete(hash),
        ..Default::default()
    };
    let (_eid, rx) = rt
        .register_user_reader_kind(cfg, keyed)
        .expect("register_user_reader_kind");
    rx
}

/// Happy path: a keyed typed writer's `dispose_sample` derives the real
/// KeyHash + a non-empty canonical key holder from the sample, the
/// same-runtime reader sees NOT_ALIVE_DISPOSED for that exact instance,
/// and the alive sample and the dispose marker name the same instance.
#[test]
fn dispose_sample_derives_the_real_key_and_the_reader_sees_not_alive_disposed() {
    let lease = zerodds_test_domains::lease_domain();
    let domain = lease.id() as u32;
    let topic = "CFfiDisposeSampleTopic";
    let type_name = "KeyedPointA";
    let type_object = small_struct_type_object(type_name, true);
    let topic_c = CString::new(topic).unwrap();
    let type_name_c = CString::new(type_name).unwrap();

    // SAFETY: every pointer below comes from the `zerodds_runtime_create*`/
    // `zerodds_writer_create*` calls right above its use and is torn down
    // exactly once at the end of this test.
    unsafe {
        let runtime = zerodds_runtime_create(domain);
        assert!(!runtime.is_null(), "runtime create failed");
        let rt = runtime_arc(runtime).expect("runtime_arc");

        let writer = zerodds_writer_create_typed(
            runtime,
            topic_c.as_ptr(),
            type_name_c.as_ptr(),
            1,
            1,
            type_object.as_ptr(),
            type_object.len(),
        );
        assert!(!writer.is_null(), "typed keyed writer create failed");
        let writer_eid = writer_entity_id(writer);

        let rx = native_reader(&rt, topic, type_name, &type_object, true);

        assert_eq!(
            zerodds_writer_wait_for_matched(writer, 1, 5_000),
            OK,
            "writer did not match the same-runtime reader"
        );

        let sample = encode_two_longs(7, 100);
        assert_eq!(
            zerodds_writer_write(writer, sample.as_ptr(), sample.len()),
            OK
        );

        let alive_key_hash = match rx.recv_timeout(Duration::from_secs(5)) {
            Ok(UserSample::Alive { key_hash, .. }) => key_hash,
            other => panic!("expected an Alive sample, got {other:?}"),
        };

        // The derivation `zerodds_writer_dispose_sample` performs
        // internally, called directly here first so the assertion is
        // about the mechanism (a non-empty holder), not just the FFI
        // call's return status.
        let (derived_hash, holder) = rt
            .writer_sample_key_material(writer_eid, &sample)
            .expect("writer_sample_key_material");
        assert!(!holder.is_empty(), "canonical key holder must not be empty");
        assert_eq!(
            alive_key_hash,
            Some(derived_hash),
            "the alive sample and the dispose marker must name the same instance"
        );

        assert_eq!(
            zerodds_writer_dispose_sample(writer, sample.as_ptr(), sample.len()),
            OK
        );

        match rx.recv_timeout(Duration::from_secs(5)) {
            Ok(UserSample::Lifecycle { key_hash, kind, .. }) => {
                assert_eq!(
                    kind,
                    ChangeKind::NotAliveDisposed,
                    "the reader must see NOT_ALIVE_DISPOSED for this instance"
                );
                assert_eq!(
                    key_hash, derived_hash,
                    "must dispose the same instance that was written"
                );
            }
            other => panic!("expected a Lifecycle marker, got {other:?}"),
        }

        zerodds_writer_destroy(writer);
        zerodds_runtime_destroy(runtime);
    }
}

/// `zerodds_writer_write` of a keyed writer whose type is registered carries
/// the key the type yields from the body: two samples of key 7 are one
/// instance, key 8 another. The byte-oriented write took no key at all
/// and sent every sample under the all-zero key, so every keyed sample of
/// the C and C++ bindings was one instance.
#[test]
fn write_of_a_keyed_typed_writer_carries_the_key_of_the_body() {
    let lease = zerodds_test_domains::lease_domain();
    let domain = lease.id() as u32;
    let topic = "CFfiWriteKeyTopic";
    let type_name = "KeyedPointW";
    let type_object = small_struct_type_object(type_name, true);
    let topic_c = CString::new(topic).unwrap();
    let type_name_c = CString::new(type_name).unwrap();

    // SAFETY: as in the dispose test above — every pointer comes from the
    // create call right above its use and is torn down exactly once.
    unsafe {
        let runtime = zerodds_runtime_create(domain);
        assert!(!runtime.is_null(), "runtime create failed");
        let rt = runtime_arc(runtime).expect("runtime_arc");
        let writer = zerodds_writer_create_typed(
            runtime,
            topic_c.as_ptr(),
            type_name_c.as_ptr(),
            1,
            1,
            type_object.as_ptr(),
            type_object.len(),
        );
        assert!(!writer.is_null(), "typed keyed writer create failed");
        let writer_eid = writer_entity_id(writer);
        let rx = native_reader(&rt, topic, type_name, &type_object, true);
        assert_eq!(zerodds_writer_wait_for_matched(writer, 1, 5_000), OK);

        let mut keys = Vec::new();
        for (id, value) in [(7, 100), (7, 200), (8, 300)] {
            let sample = encode_two_longs(id, value);
            assert_eq!(
                zerodds_writer_write(writer, sample.as_ptr(), sample.len()),
                OK
            );
            let key_hash = match rx.recv_timeout(Duration::from_secs(5)) {
                Ok(UserSample::Alive { key_hash, .. }) => key_hash,
                other => panic!("expected an Alive sample, got {other:?}"),
            };
            let (derived, _) = rt
                .writer_sample_key_material(writer_eid, &sample)
                .expect("writer_sample_key_material");
            assert_eq!(
                key_hash,
                Some(derived),
                "sample ({id}, {value}) carries no key or another key"
            );
            keys.push(derived);
        }
        assert_eq!(keys[0], keys[1], "two samples of key 7 are two instances");
        assert_ne!(keys[0], keys[2], "keys 7 and 8 are one instance");

        zerodds_writer_destroy(writer);
        zerodds_runtime_destroy(runtime);
    }
}

/// `unregister_sample` and `unregister_with_dispose_sample` go through
/// the same derivation and reach the reader as the matching `ChangeKind`.
#[test]
fn unregister_sample_variants_reach_the_reader_with_the_right_change_kind() {
    let lease = zerodds_test_domains::lease_domain();
    let domain = lease.id() as u32;
    let topic = "CFfiUnregisterSampleTopic";
    let type_name = "KeyedPointB";
    let type_object = small_struct_type_object(type_name, true);
    let topic_c = CString::new(topic).unwrap();
    let type_name_c = CString::new(type_name).unwrap();

    // SAFETY: see the dispose_sample test above — same lifetime discipline.
    unsafe {
        let runtime = zerodds_runtime_create(domain);
        assert!(!runtime.is_null(), "runtime create failed");
        let rt = runtime_arc(runtime).expect("runtime_arc");

        let writer = zerodds_writer_create_typed(
            runtime,
            topic_c.as_ptr(),
            type_name_c.as_ptr(),
            1,
            1,
            type_object.as_ptr(),
            type_object.len(),
        );
        assert!(!writer.is_null(), "typed keyed writer create failed");

        let rx = native_reader(&rt, topic, type_name, &type_object, true);
        assert_eq!(
            zerodds_writer_wait_for_matched(writer, 1, 5_000),
            OK,
            "writer did not match the same-runtime reader"
        );

        // Instance 1: unregister_sample -> NotAliveUnregistered.
        let sample1 = encode_two_longs(1, 10);
        assert_eq!(
            zerodds_writer_write(writer, sample1.as_ptr(), sample1.len()),
            OK
        );
        assert!(matches!(
            rx.recv_timeout(Duration::from_secs(5)),
            Ok(UserSample::Alive { .. })
        ));
        assert_eq!(
            zerodds_writer_unregister_sample(writer, sample1.as_ptr(), sample1.len()),
            OK
        );
        match rx.recv_timeout(Duration::from_secs(5)) {
            Ok(UserSample::Lifecycle { kind, .. }) => {
                assert_eq!(kind, ChangeKind::NotAliveUnregistered);
            }
            other => panic!("expected a Lifecycle marker, got {other:?}"),
        }

        // Instance 2: unregister_with_dispose_sample -> combined marker.
        let sample2 = encode_two_longs(2, 20);
        assert_eq!(
            zerodds_writer_write(writer, sample2.as_ptr(), sample2.len()),
            OK
        );
        assert!(matches!(
            rx.recv_timeout(Duration::from_secs(5)),
            Ok(UserSample::Alive { .. })
        ));
        assert_eq!(
            zerodds_writer_unregister_with_dispose_sample(writer, sample2.as_ptr(), sample2.len()),
            OK
        );
        match rx.recv_timeout(Duration::from_secs(5)) {
            Ok(UserSample::Lifecycle { kind, .. }) => {
                assert_eq!(kind, ChangeKind::NotAliveDisposedUnregistered);
            }
            other => panic!("expected a Lifecycle marker, got {other:?}"),
        }

        zerodds_writer_destroy(writer);
        zerodds_runtime_destroy(runtime);
    }
}

/// A writer with no registered `TypeObject` at all
/// (`zerodds_writer_create`) has nothing to derive a key from — every
/// `_sample` entry point must refuse with `PreconditionNotMet`, never
/// fall back to the hash-only marker silently.
#[test]
fn sample_variants_refuse_a_writer_without_a_registered_type() {
    let lease = zerodds_test_domains::lease_domain();
    let domain = lease.id() as u32;
    let topic_c = CString::new("CFfiDisposeSampleUntypedTopic").unwrap();
    let type_name_c = CString::new("RawBytes").unwrap();

    // SAFETY: `runtime`/`writer` come from the calls right above their use
    // and are destroyed exactly once at the end of this test.
    unsafe {
        let runtime = zerodds_runtime_create(domain);
        assert!(!runtime.is_null(), "runtime create failed");
        let writer = zerodds_writer_create(runtime, topic_c.as_ptr(), type_name_c.as_ptr(), 1);
        assert!(!writer.is_null(), "untyped writer create failed");

        let sample = [1u8, 2, 3, 4];
        assert_eq!(
            zerodds_writer_write(writer, sample.as_ptr(), sample.len()),
            OK
        );

        assert_eq!(
            zerodds_writer_dispose_sample(writer, sample.as_ptr(), sample.len()),
            PRECONDITION_NOT_MET,
            "an untyped writer has no TypeObject to derive a key from"
        );
        assert_eq!(
            zerodds_writer_unregister_sample(writer, sample.as_ptr(), sample.len()),
            PRECONDITION_NOT_MET
        );
        assert_eq!(
            zerodds_writer_unregister_with_dispose_sample(writer, sample.as_ptr(), sample.len()),
            PRECONDITION_NOT_MET
        );

        zerodds_writer_destroy(writer);
        zerodds_runtime_destroy(runtime);
    }
}

/// A writer whose type is registered but has no `@key` member writes a topic
/// without a key. DDS 1.4 §2.2.1.2.2: "If no key is provided, the data set
/// associated with the Topic is restricted to a single instance", so there is
/// an instance to dispose and no key to derive: the marker names the one
/// instance. (The writer of an untyped `zerodds_writer_create`, whose key
/// cannot be told, is refused in the test above.)
#[test]
fn dispose_sample_disposes_the_one_instance_of_a_typed_unkeyed_writer() {
    let lease = zerodds_test_domains::lease_domain();
    let domain = lease.id() as u32;
    let type_name = "UnkeyedPoint";
    let type_object = small_struct_type_object(type_name, false);
    let topic_c = CString::new("CFfiDisposeSampleUnkeyedTopic").unwrap();
    let type_name_c = CString::new(type_name).unwrap();

    // SAFETY: see the untyped-writer test above — same lifetime discipline.
    unsafe {
        let runtime = zerodds_runtime_create(domain);
        assert!(!runtime.is_null(), "runtime create failed");
        let writer = zerodds_writer_create_typed(
            runtime,
            topic_c.as_ptr(),
            type_name_c.as_ptr(),
            1,
            0,
            type_object.as_ptr(),
            type_object.len(),
        );
        assert!(!writer.is_null(), "typed unkeyed writer create failed");

        let sample = encode_two_longs(1, 2);
        assert_eq!(
            zerodds_writer_write(writer, sample.as_ptr(), sample.len()),
            OK
        );
        assert_eq!(
            zerodds_writer_dispose_sample(writer, sample.as_ptr(), sample.len()),
            OK,
            "the one instance of a topic without a key can be disposed"
        );

        zerodds_writer_destroy(writer);
        zerodds_runtime_destroy(runtime);
    }
}
