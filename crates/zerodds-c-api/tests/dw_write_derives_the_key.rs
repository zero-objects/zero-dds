// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `zerodds_dw_write` of a keyed writer whose type is registered writes each
//! sample under the key its type yields from the body, through the writer's
//! instance registry — so `zerodds_dw_lookup_instance` finds the instance the
//! write created, under the key `zerodds_dw_sample_key_hash` names for the
//! same bytes.
//!
//! The byte-oriented write took no key: every keyed sample went out under
//! the all-zero key and registered no instance, so a later
//! `lookup_instance`/`unregister_instance` of it found nothing.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::ffi::CString;
use std::ptr;
use std::sync::Arc;

use zerodds::extra_ffi::zerodds_dw_lookup_instance;
use zerodds::factory_ffi::{
    zerodds_dpf_create_participant, zerodds_dpf_delete_participant, zerodds_dpf_get_instance,
};
use zerodds::participant_ffi::{zerodds_dp_create_publisher, zerodds_dp_create_topic};
use zerodds::publisher_ffi::{
    zerodds_dw_sample_key_hash, zerodds_dw_write, zerodds_pub_create_datawriter_typed_wire,
};
use zerodds_types::TypeObject;

const OK: i32 = 0;

/// A serialized COMPLETE `TypeObject` of `@final struct <name> { @key long
/// id; long value; };`, as `writer_dispose_sample.rs` builds it.
fn keyed_type_object(type_name: &str) -> Vec<u8> {
    let idl = format!("@final struct {type_name} {{ @key long id; long value; }};");
    let parsed = zerodds_idl_parse::parse(&idl);
    assert!(!parsed.has_errors(), "parse: {:?}", parsed.diagnostics());
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "dw_write_derives_the_key.idl");
    assert!(
        lowered.diagnostics.is_empty(),
        "lower: {:?}",
        lowered.diagnostics
    );
    let mut ast = lowered.ast;
    assert!(ast.resolve_all_by_name().expect("resolve").is_clean());
    let ast = Arc::new(ast);
    let type_ = zerodds_types::dynamic::DynamicType::find(&ast, type_name).expect("find");
    let complete =
        zerodds_types::from_ast::complete_type_object(&ast, type_.id()).expect("complete");
    TypeObject::Complete(complete)
        .to_bytes_le()
        .expect("encode TypeObject")
}

/// The `@final` body of `{ id, value }`: two little-endian `long`s.
fn body(id: i32, value: i32) -> Vec<u8> {
    let mut out = id.to_le_bytes().to_vec();
    out.extend_from_slice(&value.to_le_bytes());
    out
}

#[test]
fn a_write_registers_the_instance_of_the_key_its_body_carries() {
    let lease = zerodds_test_domains::lease_domain();
    let type_name = "KeyedPointD";
    let type_object = keyed_type_object(type_name);
    let topic_name = CString::new("CFfiDwWriteKeyTopic").unwrap();
    let type_name_c = CString::new(type_name).unwrap();

    // SAFETY: every pointer comes from the create call right above its use,
    // is checked for NULL, and the participant is deleted exactly once.
    unsafe {
        let factory = zerodds_dpf_get_instance();
        let dp = zerodds_dpf_create_participant(factory, lease.id() as u32, ptr::null());
        assert!(!dp.is_null(), "participant create failed");
        let topic =
            zerodds_dp_create_topic(dp, topic_name.as_ptr(), type_name_c.as_ptr(), ptr::null());
        let publisher = zerodds_dp_create_publisher(dp, ptr::null());
        assert!(!topic.is_null() && !publisher.is_null());
        let writer = zerodds_pub_create_datawriter_typed_wire(
            publisher,
            topic,
            ptr::null(),
            type_object.as_ptr(),
            type_object.len(),
            1,
            0,
            2,
        );
        assert!(!writer.is_null(), "typed keyed writer create failed");

        let mut handles = Vec::new();
        for (id, value) in [(7, 100), (7, 200), (8, 300)] {
            let sample = body(id, value);
            assert_eq!(
                zerodds_dw_write(writer, sample.as_ptr(), sample.len(), 0),
                OK
            );
            let mut key = [0u8; 16];
            assert_eq!(
                zerodds_dw_sample_key_hash(writer, sample.as_ptr(), sample.len(), key.as_mut_ptr()),
                OK
            );
            let mut handle = 0u64;
            assert_eq!(
                zerodds_dw_lookup_instance(writer, key.as_ptr(), 16, &mut handle),
                OK
            );
            assert_ne!(
                handle, 0,
                "the write of ({id}, {value}) registered no instance under its key"
            );
            handles.push(handle);
        }
        assert_eq!(
            handles[0], handles[1],
            "two samples of key 7 are two instances"
        );
        assert_ne!(handles[0], handles[2], "keys 7 and 8 are one instance");

        let _ = zerodds_dpf_delete_participant(factory, dp);
    }
}
