// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A dynamic type from IDL text: its identity (name, key, extensibility, TypeObject) and a value
//! as JSON through the codec (`dds::core::xtypes`, DDS-PSM-Cxx 1.0 §7.5).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::ffi::{CStr, CString};
use std::ptr;

use zerodds::dyntype_ffi::{
    zerodds_dyntype_bytes_free, zerodds_dyntype_decode_json, zerodds_dyntype_default_json,
    zerodds_dyntype_dependency_count, zerodds_dyntype_destroy, zerodds_dyntype_encode_json,
    zerodds_dyntype_extensibility, zerodds_dyntype_from_idl, zerodds_dyntype_is_keyed,
    zerodds_dyntype_name, zerodds_dyntype_type_object,
};
use zerodds::topic_ffi::zerodds_string_free;

const IDL: &str = "module m { @appendable struct Track { @key long id; string label; \
                   sequence<double> samples; @optional long extra; }; };";
const OK: i32 = 0;
const BAD_PARAMETER: i32 = -6;

#[test]
fn a_type_from_idl_has_the_identity_of_a_compiled_one_and_a_value_round_trips() {
    let idl = CString::new(IDL).unwrap();
    let name = CString::new("m::Track").unwrap();
    // SAFETY: strings are NUL-terminated; every handle and buffer is used once and released.
    unsafe {
        let mut status = -1;
        let t = zerodds_dyntype_from_idl(idl.as_ptr(), name.as_ptr(), &mut status);
        assert!(!t.is_null(), "status {status}");
        assert_eq!(status, OK);

        let mut text = ptr::null_mut();
        assert_eq!(zerodds_dyntype_name(t, &mut text), OK);
        assert_eq!(CStr::from_ptr(text).to_str().unwrap(), "m::Track");
        zerodds_string_free(text);
        assert_eq!(zerodds_dyntype_is_keyed(t), 1);
        assert_eq!(zerodds_dyntype_extensibility(t), 1, "APPENDABLE");
        let mut len = 0usize;
        let object = zerodds_dyntype_type_object(t, &mut len);
        assert!(!object.is_null() && len > 0);
        let _ = zerodds_dyntype_dependency_count(t);

        // The default value is a complete value, which encodes.
        let mut default = ptr::null_mut();
        assert_eq!(zerodds_dyntype_default_json(t, &mut default), OK);
        let default_text = CStr::from_ptr(default).to_str().unwrap().to_string();
        assert!(default_text.contains("\"label\""), "{default_text}");
        zerodds_string_free(default);

        let value = CString::new(r#"{"id":7,"label":"abc","samples":[1.5,2.5]}"#).unwrap();
        let mut bytes = ptr::null_mut();
        let mut size = 0usize;
        assert_eq!(
            zerodds_dyntype_encode_json(t, value.as_ptr(), 1, 0, &mut bytes, &mut size),
            OK
        );
        assert!(size > 0);
        let mut json = ptr::null_mut();
        assert_eq!(
            zerodds_dyntype_decode_json(t, bytes, size, 1, 0, &mut json),
            OK
        );
        let decoded: serde_json::Value =
            serde_json::from_str(CStr::from_ptr(json).to_str().unwrap()).unwrap();
        assert_eq!(decoded["id"], 7);
        assert_eq!(decoded["label"], "abc");
        assert_eq!(decoded["samples"], serde_json::json!([1.5, 2.5]));
        assert!(
            decoded.get("extra").is_none(),
            "an absent @optional member is a left-out key"
        );
        zerodds_string_free(json);
        zerodds_dyntype_bytes_free(bytes, size);

        // A value with a member missing is refused, not defaulted.
        let missing = CString::new(r#"{"id":7}"#).unwrap();
        let mut none = ptr::null_mut();
        let mut none_len = 0usize;
        assert_eq!(
            zerodds_dyntype_encode_json(t, missing.as_ptr(), 1, 0, &mut none, &mut none_len),
            BAD_PARAMETER
        );
        zerodds_dyntype_destroy(t);
    }
}

#[test]
fn idl_that_does_not_parse_or_a_name_it_does_not_declare_is_bad_parameter() {
    let bad = CString::new("struct {").unwrap();
    let good = CString::new(IDL).unwrap();
    let name = CString::new("m::Track").unwrap();
    let other = CString::new("m::Other").unwrap();
    // SAFETY: strings are NUL-terminated.
    unsafe {
        let mut status = 0;
        assert!(zerodds_dyntype_from_idl(bad.as_ptr(), name.as_ptr(), &mut status).is_null());
        assert_eq!(status, BAD_PARAMETER);
        assert!(zerodds_dyntype_from_idl(good.as_ptr(), other.as_ptr(), &mut status).is_null());
        assert_eq!(status, BAD_PARAMETER);
    }
}
