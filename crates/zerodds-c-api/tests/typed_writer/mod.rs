// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The setup the tests of the typed C API share: the type an IDL text
//! declares, a participant, a topic of that type and a writer that
//! announces the type's TypeObject.
#![allow(dead_code)]

use std::ffi::CString;
use std::ptr;

use zerodds::dyntype_ffi::{ZeroDdsDynType, zerodds_dyntype_from_idl, zerodds_dyntype_type_object};
use zerodds::entities::{
    ZeroDdsDataWriter, ZeroDdsDomainParticipant, ZeroDdsDomainParticipantFactory, ZeroDdsTopic,
};
use zerodds::factory_ffi::{zerodds_dpf_create_participant, zerodds_dpf_get_instance};
use zerodds::participant_ffi::{zerodds_dp_create_publisher, zerodds_dp_create_topic};
use zerodds::publisher_ffi::zerodds_pub_create_datawriter_typed;

/// What [`typed_writer`] created.
pub struct TypedWriter {
    /// The type, to be destroyed with `zerodds_dyntype_destroy`.
    pub t: *mut ZeroDdsDynType,
    /// Its TypeObject, owned by `t`.
    pub object: *const u8,
    /// The TypeObject's length.
    pub object_len: usize,
    /// The factory the participant came from.
    pub f: *const ZeroDdsDomainParticipantFactory,
    /// The participant, to be deleted with `zerodds_dpf_delete_participant`.
    pub dp: *mut ZeroDdsDomainParticipant,
    /// The topic of the type.
    pub topic: *mut ZeroDdsTopic,
    /// The writer, created with default QoS; checked not to be null.
    pub writer: *mut ZeroDdsDataWriter,
}

/// The type `type_name` of `idl`, a participant on `domain`, the topic
/// `topic_name` of that type and a writer of it.
///
/// # Safety
/// The caller destroys `t` and deletes `dp` when the test is done.
pub unsafe fn typed_writer(
    idl: &str,
    type_name: &str,
    topic_name: &str,
    domain: u32,
) -> TypedWriter {
    let idl = CString::new(idl).unwrap();
    let name = CString::new(type_name).unwrap();
    let topic_name = CString::new(topic_name).unwrap();
    // SAFETY: every pointer is either a CString above or a handle the
    // previous call returned and checked.
    unsafe {
        let mut status = -1;
        let t = zerodds_dyntype_from_idl(idl.as_ptr(), name.as_ptr(), &mut status);
        assert!(!t.is_null(), "status {status}");
        let mut object_len = 0usize;
        let object = zerodds_dyntype_type_object(t, &mut object_len);
        let f = zerodds_dpf_get_instance();
        let dp = zerodds_dpf_create_participant(f, domain, ptr::null());
        let topic = zerodds_dp_create_topic(dp, topic_name.as_ptr(), name.as_ptr(), ptr::null());
        let publisher = zerodds_dp_create_publisher(dp, ptr::null());
        let writer =
            zerodds_pub_create_datawriter_typed(publisher, topic, ptr::null(), object, object_len);
        assert!(!writer.is_null());
        TypedWriter {
            t,
            object,
            object_len,
            f,
            dp,
            topic,
            writer,
        }
    }
}
