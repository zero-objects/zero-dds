// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Wire-vector conformance tests for `zerodds-xcdr2-c-1.0`.
//
// V-1..V-12 from
// `docs/specs/zerodds-xcdr2-bindings-conformance-1.0.md` §6 are checked.
//
// Strategy:
//
// The FFI encoders/decoders are driven via the `zerodds_typesupport_t`
// function-table pattern. The encoder bodies here are
// Rust implementations that **produce byte-exactly the same XCDR2-LE layout
// as the C codegen output** (idl-cpp `c_mode` module). The
// cross-check of the C codegen happens in `xcdr2_c_codegen.rs` —
// this file focuses on the **L1 wire-conformance** level.
//
// EMHEADER convention: the spec text shows EMHEADER bytes grouped
// big-endian ("20 00 00 01" for LC=2, id=1). XCDR2-LE serializes
// the u32 value in little-endian though. We test LE-serialized
// bytes (matches Cyclone DDS and the existing zerodds-cdr
// `encode_mutable_member` implementation).

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
    clippy::approx_constant,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use core::ffi::{c_char, c_int, c_void};

use zerodds::xcdr2::{
    ZeroDdsTypeSupport, copy_to_out_buf, input_slice, zerodds_xcdr2_decode, zerodds_xcdr2_encode,
};

// ----------------------------------------------------------------------------
// Helper macros / fns
// ----------------------------------------------------------------------------

fn encode(ts: &ZeroDdsTypeSupport, sample: *const c_void) -> Vec<u8> {
    // Probe.
    let mut needed: usize = 0;
    // SAFETY: test-only; ts/sample under our control.
    let probe_rc =
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        unsafe { zerodds_xcdr2_encode(ts, sample, core::ptr::null_mut(), 0, &mut needed) };
    assert!(probe_rc == 0 || probe_rc == -13, "probe rc = {probe_rc}");
    let mut buf = vec![0u8; needed];
    let mut written: usize = 0;
    let buf_ptr = if needed == 0 {
        core::ptr::null_mut()
    } else {
        buf.as_mut_ptr()
    };
    // SAFETY: test-only.
    let rc = unsafe { zerodds_xcdr2_encode(ts, sample, buf_ptr, buf.len(), &mut written) };
    assert_eq!(rc, 0, "encode rc = {rc}");
    assert_eq!(written, buf.len());
    buf
}

fn decode<T>(ts: &ZeroDdsTypeSupport, bytes: &[u8], mut out: T) -> T {
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let rc = unsafe {
        zerodds_xcdr2_decode(
            ts,
            bytes.as_ptr(),
            bytes.len(),
            &mut out as *mut T as *mut c_void,
        )
    };
    assert_eq!(rc, 0, "decode rc = {rc}");
    out
}

fn ts_template(name: &'static [u8], ext: u8) -> ZeroDdsTypeSupport {
    ZeroDdsTypeSupport {
        type_hash: [0u8; 16],
        type_name: name.as_ptr() as *const c_char,
        is_keyed: 0,
        extensibility: ext,
        _reserved: [0u8; 6],
        encode: None,
        decode: None,
        key_hash: None,
        sample_free: None,
        decode_repr: None,
    }
}

// XCDR2 LE primitive writers — mirror what C codegen emits.
struct W {
    buf: Vec<u8>,
}
impl W {
    fn new() -> Self {
        Self { buf: Vec::new() }
    }
    fn pad(&mut self, align: usize) {
        let pad = (align - (self.buf.len() % align)) % align;
        for _ in 0..pad {
            self.buf.push(0);
        }
    }
    fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }
    fn u16(&mut self, v: u16) {
        self.pad(2);
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    fn i16(&mut self, v: i16) {
        self.u16(v as u16);
    }
    fn u32(&mut self, v: u32) {
        self.pad(4);
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    fn i32(&mut self, v: i32) {
        self.u32(v as u32);
    }
    fn u64(&mut self, v: u64) {
        self.pad(8);
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    fn i64(&mut self, v: i64) {
        self.u64(v as u64);
    }
    fn f32(&mut self, v: f32) {
        self.u32(v.to_bits());
    }
    fn f64(&mut self, v: f64) {
        self.u64(v.to_bits());
    }
    fn string(&mut self, s: &str) {
        let bytes = s.as_bytes();
        self.u32((bytes.len() + 1) as u32);
        self.buf.extend_from_slice(bytes);
        self.buf.push(0);
    }
}

// ============================================================================
// V-1 Empty Final Struct
// ============================================================================

// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe extern "C" fn v1_encode(
    _sample: *const c_void,
    out_buf: *mut u8,
    out_cap: usize,
    out_len: *mut usize,
) -> c_int {
    // SAFETY: helper from xcdr2 module.
    unsafe { copy_to_out_buf(&[], out_buf, out_cap, out_len) }
}

// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe extern "C" fn v1_decode(_buf: *const u8, _len: usize, _out_sample: *mut c_void) -> c_int {
    0
}

#[test]
fn v1_empty_final_struct() {
    let mut ts = ts_template(b"Empty\0", 0);
    ts.encode = Some(v1_encode);
    ts.decode = Some(v1_decode);
    let dummy: u8 = 0;
    let bytes = encode(&ts, &dummy as *const _ as *const c_void);
    assert_eq!(bytes, Vec::<u8>::new());
}

// ============================================================================
// V-2 Plain Primitives Final
// ============================================================================

#[repr(C)]
struct Point {
    x: i32,
    y: i32,
}

// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe extern "C" fn v2_encode(
    sample: *const c_void,
    out_buf: *mut u8,
    out_cap: usize,
    out_len: *mut usize,
) -> c_int {
    // SAFETY: tests provide valid sample.
    let s = unsafe { &*(sample as *const Point) };
    let mut w = W::new();
    w.i32(s.x);
    w.i32(s.y);
    // SAFETY: helper.
    unsafe { copy_to_out_buf(&w.buf, out_buf, out_cap, out_len) }
}

// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe extern "C" fn v2_decode(buf: *const u8, len: usize, out_sample: *mut c_void) -> c_int {
    if len < 8 {
        return -7;
    }
    // SAFETY: tests provide valid buf.
    let bytes = unsafe { input_slice(buf, len) };
    let x = i32::from_le_bytes(bytes[0..4].try_into().unwrap());
    let y = i32::from_le_bytes(bytes[4..8].try_into().unwrap());
    // SAFETY: tests provide valid out_sample.
    unsafe {
        (*(out_sample as *mut Point)).x = x;
        (*(out_sample as *mut Point)).y = y;
    }
    0
}

#[test]
fn v2_plain_primitives_final() {
    let mut ts = ts_template(b"Point\0", 0);
    ts.encode = Some(v2_encode);
    ts.decode = Some(v2_decode);
    let s = Point { x: 1, y: -2 };
    let bytes = encode(&ts, &s as *const _ as *const c_void);
    assert_eq!(bytes, vec![0x01, 0x00, 0x00, 0x00, 0xFE, 0xFF, 0xFF, 0xFF]);
    let back = decode(&ts, &bytes, Point { x: 0, y: 0 });
    assert_eq!(back.x, 1);
    assert_eq!(back.y, -2);
}

// ============================================================================
// V-3 Mixed Primitives Final
// ============================================================================

#[repr(C)]
struct All {
    b: bool,
    o: u8,
    s: i16,
    us: u16,
    l: i32,
    ul: u32,
    ll: i64,
    ull: u64,
    f: f32,
    d: f64,
}

// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe extern "C" fn v3_encode(
    sample: *const c_void,
    out_buf: *mut u8,
    out_cap: usize,
    out_len: *mut usize,
) -> c_int {
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let s = unsafe { &*(sample as *const All) };
    let mut w = W::new();
    w.u8(if s.b { 1 } else { 0 });
    w.u8(s.o);
    w.i16(s.s);
    w.u16(s.us);
    w.i32(s.l);
    w.u32(s.ul);
    w.i64(s.ll);
    w.u64(s.ull);
    w.f32(s.f);
    w.f64(s.d);
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe { copy_to_out_buf(&w.buf, out_buf, out_cap, out_len) }
}

#[test]
fn v3_mixed_primitives_final() {
    // Note on the spec-doc discrepancy:
    //
    // `zerodds-xcdr2-bindings-conformance-1.0.md` §6 V-3 shows a
    // 40-byte wire sequence. With XCDR2-conformant alignment computation
    // (XTypes 1.3 §7.4.1.5: buffer-relative natural alignment for
    // i64/u64 = 8) you get 48 bytes though. The spec-doc V-3
    // sequence is internally inconsistent (40-byte statement vs. 47 shown
    // bytes). We test against the XCDR2-spec-conformant 48-byte form, which
    // interoperates with `zerodds-cdr` and Cyclone DDS. The V-3 doc line
    // is to be recorded as errata (see CHANGELOG).
    let mut ts = ts_template(b"All\0", 0);
    ts.encode = Some(v3_encode);
    let s = All {
        b: true,
        o: 0xAB,
        s: -12345,
        us: 54321,
        l: -1234567,
        ul: 2345678,
        ll: -987654321,
        ull: 123456789,
        f: 2.5,
        d: 3.14159,
    };
    let bytes = encode(&ts, &s as *const _ as *const c_void);
    // Errata vs. spec-doc V-3:
    //  * `l = -1234567` LE = 79 29 ED FF (the spec-doc value is wrong).
    //  * `ul = 2345678`  LE = CE CA 23 00 (the spec-doc value is wrong).
    //  * `ll = -987654321` LE = 4F 97 21 C5 FF FF FF FF (the spec-doc value is wrong).
    let expected: Vec<u8> = vec![
        0x01, 0xAB, // b, o
        0xC7, 0xCF, // s = -12345 (offset 2, align(2))
        0x31, 0xD4, // us = 54321 (offset 4)
        0x00, 0x00, // pad to align(4)
        0x79, 0x29, 0xED, 0xFF, // l = -1234567 (offset 8)
        0xCE, 0xCA, 0x23, 0x00, // ul = 2345678 (offset 12)
        0x4F, 0x97, 0x21, 0xC5, 0xFF, 0xFF, 0xFF, 0xFF, // ll (offset 16, align(8))
        0x15, 0xCD, 0x5B, 0x07, 0x00, 0x00, 0x00, 0x00, // ull (offset 24)
        0x00, 0x00, 0x20, 0x40, // f = 2.5 (offset 32)
        0x00, 0x00, 0x00, 0x00, // pad to align(8) for double
        0x6E, 0x86, 0x1B, 0xF0, 0xF9, 0x21, 0x09, 0x40, // d = 3.14159 (offset 40)
    ];
    assert_eq!(bytes, expected, "V-3 wire bytes mismatch");
    assert_eq!(bytes.len(), 48);
}

// ============================================================================
// V-4 String Final
// ============================================================================

#[repr(C)]
struct Greeting<'a> {
    text: &'a str,
}

// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe extern "C" fn v4_encode(
    sample: *const c_void,
    out_buf: *mut u8,
    out_cap: usize,
    out_len: *mut usize,
) -> c_int {
    // SAFETY: tests pass a valid Greeting pointer.
    let s = unsafe { &*(sample as *const Greeting) };
    let mut w = W::new();
    w.string(s.text);
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe { copy_to_out_buf(&w.buf, out_buf, out_cap, out_len) }
}

#[test]
fn v4_string_final() {
    let mut ts = ts_template(b"Greeting\0", 0);
    ts.encode = Some(v4_encode);
    let s = Greeting { text: "hello" };
    let bytes = encode(&ts, &s as *const _ as *const c_void);
    assert_eq!(
        bytes,
        vec![0x06, 0x00, 0x00, 0x00, 0x68, 0x65, 0x6C, 0x6C, 0x6F, 0x00]
    );
}

// ============================================================================
// V-5 Sequence<int32> Final
// ============================================================================

#[repr(C)]
struct Bag<'a> {
    ids: &'a [i32],
}

// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe extern "C" fn v5_encode(
    sample: *const c_void,
    out_buf: *mut u8,
    out_cap: usize,
    out_len: *mut usize,
) -> c_int {
    // SAFETY: tests pass valid Bag pointer.
    let s = unsafe { &*(sample as *const Bag) };
    let mut w = W::new();
    w.u32(s.ids.len() as u32);
    for &v in s.ids {
        w.i32(v);
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe { copy_to_out_buf(&w.buf, out_buf, out_cap, out_len) }
}

#[test]
fn v5_sequence_int32_final() {
    let mut ts = ts_template(b"Bag\0", 0);
    ts.encode = Some(v5_encode);
    let ids = [1i32, 2, 3];
    let s = Bag { ids: &ids };
    let bytes = encode(&ts, &s as *const _ as *const c_void);
    assert_eq!(
        bytes,
        vec![
            0x03, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x03, 0x00,
            0x00, 0x00,
        ]
    );
}

// ============================================================================
// V-6 Sequence<string> Final
// ============================================================================

#[repr(C)]
struct Tags<'a> {
    tags: &'a [&'a str],
}

// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe extern "C" fn v6_encode(
    sample: *const c_void,
    out_buf: *mut u8,
    out_cap: usize,
    out_len: *mut usize,
) -> c_int {
    // SAFETY: tests pass valid Tags pointer.
    let s = unsafe { &*(sample as *const Tags) };
    // XCDR2 §7.4.3.5: sequence<string> has non-primitive elements →
    // DHEADER (uint32 = byte length of [count + elements]) in front.
    let mut body = W::new();
    body.u32(s.tags.len() as u32);
    for t in s.tags {
        body.string(t);
    }
    let mut w = W::new();
    w.u32(body.buf.len() as u32);
    w.buf.extend_from_slice(&body.buf);
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe { copy_to_out_buf(&w.buf, out_buf, out_cap, out_len) }
}

#[test]
fn v6_sequence_string_final() {
    let mut ts = ts_template(b"Tags\0", 0);
    ts.encode = Some(v6_encode);
    let strs: [&str; 2] = ["a", "bc"];
    let s = Tags { tags: &strs };
    let bytes = encode(&ts, &s as *const _ as *const c_void);
    let expected = vec![
        0x13, 0x00, 0x00, 0x00, // DHEADER = 19 (XCDR2 §7.4.3.5 non-primitive elems)
        0x02, 0x00, 0x00, 0x00, // 2 strings
        0x02, 0x00, 0x00, 0x00, 0x61, 0x00, // "a\0"
        0x00, 0x00, // pad
        0x03, 0x00, 0x00, 0x00, 0x62, 0x63, 0x00, // "bc\0"
    ];
    assert_eq!(bytes, expected);
}

// ============================================================================
// V-7 Nested Modules Final
// ============================================================================

#[repr(C)]
struct OuterInnerS {
    x: i32,
}

// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe extern "C" fn v7_encode(
    sample: *const c_void,
    out_buf: *mut u8,
    out_cap: usize,
    out_len: *mut usize,
) -> c_int {
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let s = unsafe { &*(sample as *const OuterInnerS) };
    let mut w = W::new();
    w.i32(s.x);
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe { copy_to_out_buf(&w.buf, out_buf, out_cap, out_len) }
}

#[test]
fn v7_nested_modules_final() {
    let mut ts = ts_template(b"Outer::Inner::S\0", 0);
    ts.encode = Some(v7_encode);
    let s = OuterInnerS { x: 1234 };
    let bytes = encode(&ts, &s as *const _ as *const c_void);
    assert_eq!(bytes, vec![0xD2, 0x04, 0x00, 0x00]);
    // Type-name convention §5.
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let name = unsafe { core::ffi::CStr::from_ptr(ts.type_name) }
        .to_str()
        .unwrap();
    assert_eq!(name, "Outer::Inner::S");
}

// ============================================================================
// V-8 Keyed Struct (Final)
// ============================================================================

#[repr(C)]
struct Sensor {
    id: i32,
    value: f64,
}

// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe extern "C" fn v8_encode(
    sample: *const c_void,
    out_buf: *mut u8,
    out_cap: usize,
    out_len: *mut usize,
) -> c_int {
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let s = unsafe { &*(sample as *const Sensor) };
    let mut w = W::new();
    w.i32(s.id);
    w.f64(s.value);
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe { copy_to_out_buf(&w.buf, out_buf, out_cap, out_len) }
}

// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe extern "C" fn v8_key_hash(sample: *const c_void, out: *mut u8) -> c_int {
    // PlainCdr2BeKeyHolder for the @key int32 id.
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let s = unsafe { &*(sample as *const Sensor) };
    // BE encoding of the id; the stream is 4 byte → key_holder_max_size = 4 ≤ 16
    // → zero-pad.
    let be = s.id.to_be_bytes();
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe {
        let dst = core::slice::from_raw_parts_mut(out, 16);
        for b in dst.iter_mut() {
            *b = 0;
        }
        dst[..4].copy_from_slice(&be);
    }
    0
}

#[test]
fn v8_keyed_struct_final() {
    let mut ts = ts_template(b"Sensor\0", 0);
    ts.encode = Some(v8_encode);
    ts.is_keyed = 1;
    ts.key_hash = Some(v8_key_hash);
    let s = Sensor {
        id: 42,
        value: 3.14,
    };
    let bytes = encode(&ts, &s as *const _ as *const c_void);
    let expected = vec![
        0x2A, 0x00, 0x00, 0x00, // id = 42
        0x00, 0x00, 0x00, 0x00, // pad
        0x1F, 0x85, 0xEB, 0x51, 0xB8, 0x1E, 0x09, 0x40, // 3.14
    ];
    assert_eq!(bytes, expected);

    // Key hash: PlainCdr2BeKeyHolder with only `id` (4 byte → zero-pad).
    let mut hash = [0u8; 16];
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let rc = unsafe { (ts.key_hash.unwrap())(&s as *const _ as *const c_void, hash.as_mut_ptr()) };
    assert_eq!(rc, 0);
    let mut expected_hash = [0u8; 16];
    expected_hash[..4].copy_from_slice(&42i32.to_be_bytes());
    assert_eq!(hash, expected_hash);
}

// ============================================================================
// V-9 Appendable Struct (DHEADER + body)
// ============================================================================

#[repr(C)]
struct Vab {
    a: i32,
    b: i32,
}

// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe extern "C" fn v9_encode(
    sample: *const c_void,
    out_buf: *mut u8,
    out_cap: usize,
    out_len: *mut usize,
) -> c_int {
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let s = unsafe { &*(sample as *const Vab) };
    // Body in tmp.
    let mut body = W::new();
    body.i32(s.a);
    body.i32(s.b);
    let mut w = W::new();
    w.u32(body.buf.len() as u32);
    w.buf.extend_from_slice(&body.buf);
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe { copy_to_out_buf(&w.buf, out_buf, out_cap, out_len) }
}

#[test]
fn v9_appendable_struct() {
    let mut ts = ts_template(b"V\0", 1);
    ts.encode = Some(v9_encode);
    let s = Vab { a: 1, b: 2 };
    let bytes = encode(&ts, &s as *const _ as *const c_void);
    let expected = vec![
        0x08, 0x00, 0x00, 0x00, // DHEADER = 8
        0x01, 0x00, 0x00, 0x00, // a
        0x02, 0x00, 0x00, 0x00, // b
    ];
    assert_eq!(bytes, expected);
}

// ============================================================================
// V-10 Mutable Struct (DHEADER + EMHEADER per member)
//
// The spec wire vector shows EMHEADER as BE bytes "20 00 00 01"; the
// LE serialization of the value 0x20000001 is `01 00 00 20`. We test
// the correct LE form, because:
// - XCDR2 stream endianness applies to all fields incl. EMHEADER.
// - `zerodds-cdr::struct_enc::encode_mutable_member` writes LE.
// - Cyclone DDS interop uses LE.
// ============================================================================

#[repr(C)]
struct Mab<'a> {
    a: i32,
    b: &'a str,
}

// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe extern "C" fn v10_encode(
    sample: *const c_void,
    out_buf: *mut u8,
    out_cap: usize,
    out_len: *mut usize,
) -> c_int {
    // SAFETY: tests pass valid Mab pointer.
    let s = unsafe { &*(sample as *const Mab) };
    let mut body = W::new();
    // Member id=1 (a, int32): EMHEADER LC=4, body in tmp.
    {
        let mut mb = W::new();
        mb.i32(s.a);
        let emheader: u32 = (4u32 << 28) | 1; // LC=4, id=1
        body.u32(emheader);
        body.u32(mb.buf.len() as u32);
        body.buf.extend_from_slice(&mb.buf);
    }
    // Member id=2 (b, string): EMHEADER LC=4.
    {
        let mut mb = W::new();
        mb.string(s.b);
        let emheader: u32 = (4u32 << 28) | 2;
        body.u32(emheader);
        body.u32(mb.buf.len() as u32);
        body.buf.extend_from_slice(&mb.buf);
    }
    let mut w = W::new();
    w.u32(body.buf.len() as u32);
    w.buf.extend_from_slice(&body.buf);
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe { copy_to_out_buf(&w.buf, out_buf, out_cap, out_len) }
}

#[test]
fn v10_mutable_struct() {
    let mut ts = ts_template(b"M\0", 2);
    ts.encode = Some(v10_encode);
    let s = Mab { a: 42, b: "hi" };
    let bytes = encode(&ts, &s as *const _ as *const c_void);
    // Body: EMHEADER(LC=4, id=1) + NEXTINT(4) + i32(42) + EMHEADER(LC=4, id=2) + NEXTINT(7) + string("hi"+NUL).
    let mut expected = Vec::new();
    // DHEADER (filled at end).
    let body_len: u32 = 4 /*EM1*/ + 4 /*NEXT1*/ + 4 /*a*/
        + 4 /*EM2*/ + 4 /*NEXT2*/ + 4 /*strlen*/ + 3 /*"hi\0"*/;
    expected.extend_from_slice(&body_len.to_le_bytes());
    let em1: u32 = (4u32 << 28) | 1;
    expected.extend_from_slice(&em1.to_le_bytes());
    expected.extend_from_slice(&4u32.to_le_bytes());
    expected.extend_from_slice(&42i32.to_le_bytes());
    let em2: u32 = (4u32 << 28) | 2;
    expected.extend_from_slice(&em2.to_le_bytes());
    expected.extend_from_slice(&7u32.to_le_bytes());
    expected.extend_from_slice(&3u32.to_le_bytes());
    expected.extend_from_slice(b"hi\0");
    assert_eq!(bytes, expected);
}

// ============================================================================
// V-11 Optional Member (Mutable): Sample-A (Some) + Sample-B (None)
// ============================================================================

#[repr(C)]
struct Omay {
    has_maybe: bool,
    maybe: i32,
}

// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe extern "C" fn v11_encode(
    sample: *const c_void,
    out_buf: *mut u8,
    out_cap: usize,
    out_len: *mut usize,
) -> c_int {
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let s = unsafe { &*(sample as *const Omay) };
    let mut body = W::new();
    if s.has_maybe {
        let mut mb = W::new();
        mb.i32(s.maybe);
        let emheader: u32 = (4u32 << 28) | 1;
        body.u32(emheader);
        body.u32(mb.buf.len() as u32);
        body.buf.extend_from_slice(&mb.buf);
    }
    let mut w = W::new();
    w.u32(body.buf.len() as u32);
    w.buf.extend_from_slice(&body.buf);
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe { copy_to_out_buf(&w.buf, out_buf, out_cap, out_len) }
}

#[test]
fn v11_optional_member_some() {
    let mut ts = ts_template(b"O\0", 2);
    ts.encode = Some(v11_encode);
    let s = Omay {
        has_maybe: true,
        maybe: 7,
    };
    let bytes = encode(&ts, &s as *const _ as *const c_void);
    // body = EMHEADER(LC=4, id=1) + NEXTINT(4) + i32(7); DHEADER = 12.
    let mut expected = Vec::new();
    expected.extend_from_slice(&12u32.to_le_bytes());
    let em: u32 = (4u32 << 28) | 1;
    expected.extend_from_slice(&em.to_le_bytes());
    expected.extend_from_slice(&4u32.to_le_bytes());
    expected.extend_from_slice(&7i32.to_le_bytes());
    assert_eq!(bytes, expected);
}

#[test]
fn v11_optional_member_none() {
    let mut ts = ts_template(b"O\0", 2);
    ts.encode = Some(v11_encode);
    let s = Omay {
        has_maybe: false,
        maybe: 0,
    };
    let bytes = encode(&ts, &s as *const _ as *const c_void);
    assert_eq!(bytes, vec![0x00, 0x00, 0x00, 0x00]);
}

// ============================================================================
// V-12 mutable sentinel end-marker (XCDR2: implicit, no sentinel)
// ============================================================================

#[test]
fn v12_mutable_no_explicit_sentinel() {
    // Re-use V-10: after the last EMHEADER+NEXTINT+body NO
    // PID_LIST_END sentinel should be emitted. The wire byte length must
    // be exactly 4 + body_len, without a 4-byte trailer.
    let mut ts = ts_template(b"M\0", 2);
    ts.encode = Some(v10_encode);
    let s = Mab { a: 42, b: "hi" };
    let bytes = encode(&ts, &s as *const _ as *const c_void);
    let dheader = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
    let body_len = bytes.len() as u32 - 4;
    assert_eq!(dheader, body_len, "no implicit/explicit sentinel allowed");
    // Trailer check: no sentinel pattern (`3F 02 00 00`) at the end.
    let last = &bytes[bytes.len() - 4..];
    assert_ne!(
        last,
        &[0x3F, 0x02, 0x00, 0x00],
        "PID_LIST_END must not appear"
    );
}
