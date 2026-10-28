// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The built-in types of DDS-XTypes 1.3 §7.6.5 and Annex E through the C
//! ABI, and `lookup_topicdescription` with an index (§7.6.4.2).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::ffi::{CStr, CString, c_char};
use std::ptr;

use zerodds::builtin_type_ffi::{
    ZERODDS_BUILTIN_BYTES, ZERODDS_BUILTIN_KEYED_BYTES, ZERODDS_BUILTIN_KEYED_STRING,
    ZERODDS_BUILTIN_STRING, zerodds_builtin_bytes_free, zerodds_builtin_string_free,
    zerodds_bytes_writer_write_w_bytes, zerodds_dp_create_builtin_topic,
    zerodds_keyed_bytes_writer_write_bytes_w_key, zerodds_keyed_string_writer_dispose_w_key,
    zerodds_keyed_string_writer_get_key_value_w_key,
    zerodds_keyed_string_writer_lookup_instance_w_key,
    zerodds_keyed_string_writer_register_instance_w_key,
    zerodds_keyed_string_writer_write_string_w_key, zerodds_pub_create_builtin_datawriter,
    zerodds_string_writer_write, zerodds_sub_create_builtin_datareader,
};
use zerodds::factory_ffi::{
    zerodds_dpf_create_participant, zerodds_dpf_delete_participant, zerodds_dpf_get_instance,
};
use zerodds::participant_ffi::{zerodds_dp_create_publisher, zerodds_dp_create_subscriber};

const OK: i32 = 0;

/// The topic of a built-in type is of that type's name, and the writer
/// and the reader of it are refused for a topic that is not.
#[test]
fn a_topic_of_a_built_in_type_has_its_physical_name() {
    let lease = zerodds_test_domains::lease_domain();
    let name = CString::new("BuiltinNames").unwrap();
    // SAFETY: every pointer comes from the create call right above its use
    // and the participant is deleted once.
    unsafe {
        let factory = zerodds_dpf_get_instance();
        let dp = zerodds_dpf_create_participant(factory, lease.id() as u32, ptr::null());
        assert!(!dp.is_null());
        for (kind, expected) in [
            (ZERODDS_BUILTIN_STRING, "DDS::String"),
            (ZERODDS_BUILTIN_KEYED_STRING, "DDS::KeyedString"),
            (ZERODDS_BUILTIN_BYTES, "DDS::Bytes"),
            (ZERODDS_BUILTIN_KEYED_BYTES, "DDS::KeyedBytes"),
        ] {
            let topic_name = CString::new(format!("{}{kind}", name.to_str().unwrap())).unwrap();
            let topic = zerodds_dp_create_builtin_topic(dp, topic_name.as_ptr(), kind, ptr::null());
            assert!(!topic.is_null(), "{expected}");
            assert_eq!((*topic).type_name, expected);
        }
        assert!(zerodds_dp_create_builtin_topic(dp, name.as_ptr(), 9, ptr::null()).is_null());
        let publisher = zerodds_dp_create_publisher(dp, ptr::null());
        let other = zerodds_dp_create_topic_of(dp, "NotBuiltin", "App::Type");
        assert!(zerodds_pub_create_builtin_datawriter(publisher, other, ptr::null()).is_null());
        let _ = zerodds_dpf_delete_participant(factory, dp);
    }
}

/// Annex E `StringTypeSupport` and the three others: `get_type_name` names
/// the type, `register_type` registers it under the given name or under its
/// own, and a topic is created from the name.
#[test]
fn the_type_supports_name_and_register_their_types() {
    use zerodds::builtin_type_ffi::{
        zerodds_builtin_type_support_get_type_name, zerodds_dp_register_builtin_type,
    };
    const BAD_PARAMETER: i32 = -6;
    const PRECONDITION_NOT_MET: i32 = -5;
    const BAD_HANDLE: i32 = -2;
    let lease = zerodds_test_domains::lease_domain();
    // SAFETY: every pointer comes from the call right above its use; the
    // names are static or owned by this function; the participant is
    // deleted once.
    unsafe {
        for (kind, expected) in [
            (ZERODDS_BUILTIN_STRING, "DDS::String"),
            (ZERODDS_BUILTIN_KEYED_STRING, "DDS::KeyedString"),
            (ZERODDS_BUILTIN_BYTES, "DDS::Bytes"),
            (ZERODDS_BUILTIN_KEYED_BYTES, "DDS::KeyedBytes"),
        ] {
            let name = zerodds_builtin_type_support_get_type_name(kind);
            assert!(!name.is_null());
            assert_eq!(CStr::from_ptr(name).to_str().unwrap(), expected);
        }
        assert!(zerodds_builtin_type_support_get_type_name(9).is_null());

        let factory = zerodds_dpf_get_instance();
        let dp = zerodds_dpf_create_participant(factory, lease.id() as u32, ptr::null());
        assert!(!dp.is_null());
        let alias = CString::new("Greetings").unwrap();
        let empty = CString::new("").unwrap();
        for kind in [
            ZERODDS_BUILTIN_STRING,
            ZERODDS_BUILTIN_KEYED_STRING,
            ZERODDS_BUILTIN_BYTES,
            ZERODDS_BUILTIN_KEYED_BYTES,
        ] {
            // Under its own name, twice.
            assert_eq!(zerodds_dp_register_builtin_type(dp, kind, ptr::null()), OK);
            assert_eq!(zerodds_dp_register_builtin_type(dp, kind, ptr::null()), OK);
        }
        // Under a name of the caller's, and a topic of that name.
        assert_eq!(
            zerodds_dp_register_builtin_type(dp, ZERODDS_BUILTIN_STRING, alias.as_ptr()),
            OK
        );
        assert_eq!(
            zerodds_dp_register_builtin_type(dp, ZERODDS_BUILTIN_STRING, alias.as_ptr()),
            OK
        );
        let topic = zerodds_dp_create_topic_of(dp, "GreetingsTopic", "Greetings");
        assert!(!topic.is_null());
        // Another type under the taken name, an empty name, a kind that is
        // none of the four, a NULL participant.
        assert_eq!(
            zerodds_dp_register_builtin_type(dp, ZERODDS_BUILTIN_BYTES, alias.as_ptr()),
            PRECONDITION_NOT_MET
        );
        assert_eq!(
            zerodds_dp_register_builtin_type(dp, ZERODDS_BUILTIN_STRING, empty.as_ptr()),
            BAD_PARAMETER
        );
        assert_eq!(
            zerodds_dp_register_builtin_type(dp, 9, ptr::null()),
            BAD_PARAMETER
        );
        assert_eq!(
            zerodds_dp_register_builtin_type(ptr::null_mut(), ZERODDS_BUILTIN_STRING, ptr::null()),
            BAD_HANDLE
        );
        let _ = zerodds_dpf_delete_participant(factory, dp);
    }
}

/// # Safety
///
/// `dp` is a live participant.
unsafe fn zerodds_dp_create_topic_of(
    dp: *mut zerodds::entities::ZeroDdsDomainParticipant,
    name: &str,
    type_name: &str,
) -> *mut zerodds::entities::ZeroDdsTopic {
    let (name, type_name) = (
        CString::new(name).unwrap(),
        CString::new(type_name).unwrap(),
    );
    // SAFETY: the caller's contract.
    unsafe {
        zerodds::participant_ffi::zerodds_dp_create_topic(
            dp,
            name.as_ptr(),
            type_name.as_ptr(),
            ptr::null(),
        )
    }
}

/// `StringDataWriter::write` to a `StringDataReader` of the same
/// participant: the string comes back as it was written.
#[test]
fn a_string_is_written_and_taken() {
    let lease = zerodds_test_domains::lease_domain();
    let topic_name = CString::new("BuiltinString").unwrap();
    let value = CString::new("hello, world").unwrap();
    // SAFETY: as above.
    unsafe {
        let factory = zerodds_dpf_get_instance();
        let dp = zerodds_dpf_create_participant(factory, lease.id() as u32, ptr::null());
        let topic = zerodds_dp_create_builtin_topic(
            dp,
            topic_name.as_ptr(),
            ZERODDS_BUILTIN_STRING,
            ptr::null(),
        );
        let publisher = zerodds_dp_create_publisher(dp, ptr::null());
        let subscriber = zerodds_dp_create_subscriber(dp, ptr::null());
        let reader = zerodds_sub_create_builtin_datareader(subscriber, topic, ptr::null());
        let writer = zerodds_pub_create_builtin_datawriter(publisher, topic, ptr::null());
        assert!(!reader.is_null() && !writer.is_null());
        assert_eq!(zerodds_string_writer_write(writer, value.as_ptr()), OK);
        assert_ne!(zerodds_string_writer_write(writer, ptr::null()), OK);

        let (got, info) = take_string(reader);
        assert!(info.valid_data);
        assert_eq!(got.as_deref(), Some("hello, world"));
        let _ = zerodds_dpf_delete_participant(factory, dp);
    }
}

/// Waits for one sample, up to two seconds.
///
/// # Safety
///
/// `reader` is a live reader of `DDS::String`.
unsafe fn take_string(
    reader: *mut zerodds::entities::ZeroDdsDataReader,
) -> (Option<String>, zerodds::subscriber_ffi::ZeroDdsSampleInfo) {
    use zerodds::builtin_type_ffi::zerodds_string_reader_take_next;
    // SAFETY: the caller's contract.
    unsafe {
        let mut info: zerodds::subscriber_ffi::ZeroDdsSampleInfo = std::mem::zeroed();
        for _ in 0..200 {
            let mut value: *mut c_char = ptr::null_mut();
            if zerodds_string_reader_take_next(reader, &raw mut value, &raw mut info) == OK {
                let text =
                    (!value.is_null()).then(|| CStr::from_ptr(value).to_str().unwrap().to_string());
                zerodds_builtin_string_free(value);
                return (text, info);
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("no sample arrived");
    }
}

/// `KeyedStringDataWriter`: the instance operations name the instance by
/// the key, a write under another key's handle is refused, and the key of
/// the instance is read back from its handle.
#[test]
fn a_keyed_string_has_its_instance_operations() {
    let lease = zerodds_test_domains::lease_domain();
    let topic_name = CString::new("BuiltinKeyedString").unwrap();
    let (a, b) = (CString::new("a").unwrap(), CString::new("b").unwrap());
    let value = CString::new("v").unwrap();
    // SAFETY: as above.
    unsafe {
        let factory = zerodds_dpf_get_instance();
        let dp = zerodds_dpf_create_participant(factory, lease.id() as u32, ptr::null());
        let topic = zerodds_dp_create_builtin_topic(
            dp,
            topic_name.as_ptr(),
            ZERODDS_BUILTIN_KEYED_STRING,
            ptr::null(),
        );
        let publisher = zerodds_dp_create_publisher(dp, ptr::null());
        let writer = zerodds_pub_create_builtin_datawriter(publisher, topic, ptr::null());
        assert!(!writer.is_null());

        let mut handle_a = 0_u64;
        assert_eq!(
            zerodds_keyed_string_writer_register_instance_w_key(
                writer,
                a.as_ptr(),
                &raw mut handle_a
            ),
            OK
        );
        assert_ne!(handle_a, 0);
        let mut found = 0_u64;
        assert_eq!(
            zerodds_keyed_string_writer_lookup_instance_w_key(writer, a.as_ptr(), &raw mut found),
            OK
        );
        assert_eq!(found, handle_a);
        assert_eq!(
            zerodds_keyed_string_writer_lookup_instance_w_key(writer, b.as_ptr(), &raw mut found),
            OK
        );
        assert_eq!(found, 0, "b is not registered");

        // The handle of a write is nil or the key's.
        assert_eq!(
            zerodds_keyed_string_writer_write_string_w_key(
                writer,
                a.as_ptr(),
                value.as_ptr(),
                handle_a
            ),
            OK
        );
        assert_ne!(
            zerodds_keyed_string_writer_write_string_w_key(
                writer,
                b.as_ptr(),
                value.as_ptr(),
                handle_a
            ),
            OK,
            "the handle is a's, the key is b"
        );
        assert_eq!(
            zerodds_keyed_string_writer_write_string_w_key(writer, b.as_ptr(), value.as_ptr(), 0),
            OK
        );

        let mut key: *mut c_char = ptr::null_mut();
        assert_eq!(
            zerodds_keyed_string_writer_get_key_value_w_key(writer, handle_a, &raw mut key),
            OK
        );
        assert_eq!(CStr::from_ptr(key).to_str().unwrap(), "a");
        zerodds_builtin_string_free(key);

        assert_eq!(
            zerodds_keyed_string_writer_dispose_w_key(writer, a.as_ptr()),
            OK
        );
        let _ = zerodds_dpf_delete_participant(factory, dp);
    }
}

/// `BytesDataWriter::write_w_bytes` and `KeyedBytesDataWriter::
/// write_bytes_w_key` take the octets `offset .. offset + length`; a range
/// outside the array is refused.
#[test]
fn bytes_are_written_from_an_offset_and_a_length() {
    let lease = zerodds_test_domains::lease_domain();
    let bytes_topic = CString::new("BuiltinBytes").unwrap();
    let keyed_topic = CString::new("BuiltinKeyedBytes").unwrap();
    let key = CString::new("k").unwrap();
    let data = [1_u8, 2, 3, 4, 5];
    // SAFETY: as above.
    unsafe {
        let factory = zerodds_dpf_get_instance();
        let dp = zerodds_dpf_create_participant(factory, lease.id() as u32, ptr::null());
        let publisher = zerodds_dp_create_publisher(dp, ptr::null());
        let subscriber = zerodds_dp_create_subscriber(dp, ptr::null());
        let topic = zerodds_dp_create_builtin_topic(
            dp,
            bytes_topic.as_ptr(),
            ZERODDS_BUILTIN_BYTES,
            ptr::null(),
        );
        let reader = zerodds_sub_create_builtin_datareader(subscriber, topic, ptr::null());
        let writer = zerodds_pub_create_builtin_datawriter(publisher, topic, ptr::null());
        assert_eq!(
            zerodds_bytes_writer_write_w_bytes(writer, data.as_ptr(), data.len(), 1, 3, 0),
            OK
        );
        assert_ne!(
            zerodds_bytes_writer_write_w_bytes(writer, data.as_ptr(), data.len(), 3, 3, 0),
            OK
        );
        let mut info: zerodds::subscriber_ffi::ZeroDdsSampleInfo = std::mem::zeroed();
        let mut got = None;
        for _ in 0..200 {
            let (mut out, mut len) = (ptr::null_mut(), 0_usize);
            if zerodds::builtin_type_ffi::zerodds_bytes_reader_take_next(
                reader,
                &raw mut out,
                &raw mut len,
                &raw mut info,
            ) == OK
            {
                got = Some(std::slice::from_raw_parts(out, len).to_vec());
                zerodds_builtin_bytes_free(out, len);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(got.as_deref(), Some(&[2_u8, 3, 4][..]));

        let keyed = zerodds_dp_create_builtin_topic(
            dp,
            keyed_topic.as_ptr(),
            ZERODDS_BUILTIN_KEYED_BYTES,
            ptr::null(),
        );
        let keyed_writer = zerodds_pub_create_builtin_datawriter(publisher, keyed, ptr::null());
        assert_eq!(
            zerodds_keyed_bytes_writer_write_bytes_w_key(
                keyed_writer,
                key.as_ptr(),
                data.as_ptr(),
                data.len(),
                0,
                2,
                0
            ),
            OK
        );
        assert_ne!(
            zerodds_keyed_bytes_writer_write_bytes_w_key(
                keyed_writer,
                key.as_ptr(),
                data.as_ptr(),
                data.len(),
                0,
                9,
                0
            ),
            OK
        );
        let _ = zerodds_dpf_delete_participant(factory, dp);
    }
}

/// DDS-XTypes 1.3 §7.6.4.1 and §7.6.4.2: a second type under a topic's name
/// is another topic, and `lookup_topicdescription` iterates over the
/// topics of the name in creation order.
#[test]
fn lookup_iterates_over_the_topics_of_a_name() {
    use zerodds::extra_ffi::{
        zerodds_dp_lookup_topicdescription, zerodds_dp_lookup_topicdescription_at,
    };
    let lease = zerodds_test_domains::lease_domain();
    let name = CString::new("OneNameTwoTypes").unwrap();
    // SAFETY: as above.
    unsafe {
        let factory = zerodds_dpf_get_instance();
        let dp = zerodds_dpf_create_participant(factory, lease.id() as u32, ptr::null());
        let first = zerodds_dp_create_topic_of(dp, "OneNameTwoTypes", "A::First");
        let second = zerodds_dp_create_topic_of(dp, "OneNameTwoTypes", "B::Second");
        assert!(
            !first.is_null() && !second.is_null(),
            "a second type is another topic"
        );
        assert_ne!(first, second);
        assert_eq!(
            zerodds_dp_lookup_topicdescription_at(dp, name.as_ptr(), 0),
            first
        );
        assert_eq!(
            zerodds_dp_lookup_topicdescription_at(dp, name.as_ptr(), 1),
            second
        );
        assert!(zerodds_dp_lookup_topicdescription_at(dp, name.as_ptr(), 2).is_null());
        assert_eq!(zerodds_dp_lookup_topicdescription(dp, name.as_ptr()), first);
        let other = CString::new("Unknown").unwrap();
        assert!(zerodds_dp_lookup_topicdescription_at(dp, other.as_ptr(), 0).is_null());
        let _ = zerodds_dpf_delete_participant(factory, dp);
    }
}
