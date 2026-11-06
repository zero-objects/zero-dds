// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-ts-wasm`. Safety classification: **STANDARD** (FFI boundary
//! via wasm-bindgen; no direct hardware/syscall access).
//!
//! WASM bindings for the ZeroDDS XCDR codec. Unlike the Node variant
//! (`crates/ts-node/`, koffi over `libzerodds`), WASM has no UDP socket and no
//! thread — live DDS in the browser goes through the WebSocket bridge
//! (`crates/websocket-bridge/`, spec `zerodds-ws-bridge-1.0`). What runs in the
//! browser itself is the codec, and that is what this crate exposes.
//!
//! What is exposed here:
//!   * an XCDR1/XCDR2 encoder and decoder for primitives, strings and bytes,
//!     the encoding version a parameter of each: `MAXALIGN` is 8 under
//!     XCDR1 and 4 under XCDR2 (DDS-XTypes 1.3 §7.4.1.1.1, §7.4.3.2.2
//!     Table 37). Without it, version 1, the default data representation
//!     (§7.6.3.1.1);
//!   * the endianness and encoding constants and the version string.
//!
//! The bytes are `zerodds-wire`'s: `Writer` and `Reader` are the same runtime
//! the generated Rust encoders call, so a value encoded in the browser is the
//! value a native peer decodes. A second hand-written codec here would be a
//! second place the wire format lives.
//!
//! ## Layer position
//!
//! Layer 6 — PSMs / bindings. Depends on Layer 0 (`zerodds-wire`) only.

// Needs `std`: a cdylib for wasm32-unknown-unknown, whose allocator and panic handler
// come from std.

// The `#[wasm_bindgen]` macro generates unexported glue wrappers that carry no
// rustdoc of their own; our own items are documented.
#![allow(missing_docs)]

extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use wasm_bindgen::prelude::*;
use zerodds_wire::{Endianness, Reader, WireError, Writer};

#[wasm_bindgen(start)]
pub fn init() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub fn version() -> String {
    "zerodds-wasm 0.0.0".to_string()
}

/// Endianness tag for JS — 0 = little, 1 = big.
#[wasm_bindgen(js_name = endiannessLittle)]
pub fn endianness_little() -> u8 {
    0
}

#[wasm_bindgen(js_name = endiannessBig)]
pub fn endianness_big() -> u8 {
    1
}

/// The two tags the ABI defines, and nothing else.
///
/// Returns the offending tag rather than a `JsError`, so the refusal is
/// reachable from a plain `cargo test`: a `JsError` can only be constructed
/// on a wasm target.
///
/// # Errors
/// The tag itself, for anything other than 0 or 1.
fn endianness_from_u8(value: u8) -> Result<Endianness, u8> {
    match value {
        0 => Ok(Endianness::Little),
        1 => Ok(Endianness::Big),
        other => Err(other),
    }
}

/// Encoding version 1 (XCDR1, `MAXALIGN` 8), the default.
#[wasm_bindgen(js_name = encodingXcdr1)]
pub fn encoding_xcdr1() -> u8 {
    1
}

/// Encoding version 2 (XCDR2, `MAXALIGN` 4).
#[wasm_bindgen(js_name = encodingXcdr2)]
pub fn encoding_xcdr2() -> u8 {
    2
}

/// `MAXALIGN` of an encoding version (DDS-XTypes 1.3 §7.4.3.2.2 Table 37):
/// 8 for version 1, 4 for version 2; absent is version 1, the default data
/// representation (§7.6.3.1.1).
///
/// # Errors
/// The version itself, for anything other than 1 or 2.
fn max_align_of(version: Option<u8>) -> Result<usize, u8> {
    match version.unwrap_or(1) {
        1 => Ok(8),
        2 => Ok(4),
        other => Err(other),
    }
}

fn invalid_encoding(version: u8) -> JsError {
    JsError::new(&format!("invalid encoding version {version}"))
}

fn invalid_endianness(tag: u8) -> JsError {
    JsError::new(&format!("invalid endianness {tag}"))
}

fn decode_error(what: &str, error: &WireError) -> JsError {
    JsError::new(&format!("{what}: {error:?}"))
}

/// XCDR encoder. Buffers bytes until `finish()` is called.
#[wasm_bindgen]
pub struct CdrEncoder {
    inner: Writer,
}

#[wasm_bindgen]
impl CdrEncoder {
    /// `encoding`: 1 (XCDR1, the default when absent) or 2 (XCDR2).
    #[wasm_bindgen(constructor)]
    pub fn new(endianness: u8, encoding: Option<u8>) -> Result<CdrEncoder, JsError> {
        Ok(Self {
            inner: Writer::with_max_align(
                endianness_from_u8(endianness).map_err(invalid_endianness)?,
                max_align_of(encoding).map_err(invalid_encoding)?,
            ),
        })
    }

    /// The alignment cap of the encoding: 8 (XCDR1) or 4 (XCDR2).
    #[wasm_bindgen(js_name = maxAlign)]
    pub fn max_align(&self) -> usize {
        self.inner.max_align()
    }

    #[wasm_bindgen(js_name = writeU8)]
    pub fn write_u8(&mut self, value: u8) {
        self.inner.write_u8(value);
    }

    #[wasm_bindgen(js_name = writeU16)]
    pub fn write_u16(&mut self, value: u16) {
        self.inner.write_u16(value);
    }

    #[wasm_bindgen(js_name = writeU32)]
    pub fn write_u32(&mut self, value: u32) {
        self.inner.write_u32(value);
    }

    #[wasm_bindgen(js_name = writeU64)]
    pub fn write_u64(&mut self, value: u64) {
        self.inner.write_u64(value);
    }

    #[wasm_bindgen(js_name = writeString)]
    pub fn write_string(&mut self, value: &str) {
        self.inner.write_string(value);
    }

    #[wasm_bindgen(js_name = writeBytes)]
    pub fn write_bytes(&mut self, data: &[u8]) {
        self.inner.write_bytes(data);
    }

    /// Pads to `alignment`. The typed writes align themselves — this is for a
    /// caller that has to reproduce a padding a hand-written layout expects.
    pub fn align(&mut self, alignment: usize) {
        self.inner.align_to(alignment);
    }

    pub fn position(&self) -> usize {
        self.inner.position()
    }

    /// Finalizes the encoder and returns the accumulated bytes. After
    /// `finish()` the encoder is consumed.
    pub fn finish(self) -> Vec<u8> {
        self.inner.finish()
    }
}

/// XCDR decoder. Reads a byte slice via a position pointer.
#[wasm_bindgen]
pub struct CdrDecoder {
    bytes: Vec<u8>,
    endianness: Endianness,
    max_align: usize,
    position: usize,
}

#[wasm_bindgen]
impl CdrDecoder {
    /// `encoding`: 1 (XCDR1, the default when absent) or 2 (XCDR2).
    #[wasm_bindgen(constructor)]
    pub fn new(
        bytes: Vec<u8>,
        endianness: u8,
        encoding: Option<u8>,
    ) -> Result<CdrDecoder, JsError> {
        Ok(Self {
            bytes,
            endianness: endianness_from_u8(endianness).map_err(invalid_endianness)?,
            max_align: max_align_of(encoding).map_err(invalid_encoding)?,
            position: 0,
        })
    }

    /// The alignment cap of the encoding: 8 (XCDR1) or 4 (XCDR2).
    #[wasm_bindgen(js_name = maxAlign)]
    pub fn max_align(&self) -> usize {
        self.max_align
    }

    /// Runs `read` on a reader parked at the current position, and advances
    /// the position by what it consumed.
    ///
    /// `Reader` borrows the byte slice, so it cannot be a field of a struct
    /// that also owns the bytes; the position is kept here instead and the
    /// reader is re-seated per call.
    fn with_reader<T, F>(&mut self, read: F) -> Result<T, WireError>
    where
        F: FnOnce(&mut Reader<'_>) -> Result<T, WireError>,
    {
        let mut reader = Reader::with_max_align(&self.bytes, self.endianness, self.max_align);
        reader.skip_to(self.position)?;
        let value = read(&mut reader)?;
        self.position = self.bytes.len() - reader.remaining();
        Ok(value)
    }

    #[wasm_bindgen(js_name = readU8)]
    pub fn read_u8(&mut self) -> Result<u8, JsError> {
        self.with_reader(|r| r.read_u8())
            .map_err(|e| decode_error("read_u8", &e))
    }

    #[wasm_bindgen(js_name = readU16)]
    pub fn read_u16(&mut self) -> Result<u16, JsError> {
        self.with_reader(|r| r.read_u16())
            .map_err(|e| decode_error("read_u16", &e))
    }

    #[wasm_bindgen(js_name = readU32)]
    pub fn read_u32(&mut self) -> Result<u32, JsError> {
        self.with_reader(|r| r.read_u32())
            .map_err(|e| decode_error("read_u32", &e))
    }

    #[wasm_bindgen(js_name = readU64)]
    pub fn read_u64(&mut self) -> Result<u64, JsError> {
        self.with_reader(|r| r.read_u64())
            .map_err(|e| decode_error("read_u64", &e))
    }

    #[wasm_bindgen(js_name = readString)]
    pub fn read_string(&mut self) -> Result<String, JsError> {
        self.with_reader(|r| r.read_string())
            .map_err(|e| decode_error("read_string", &e))
    }

    /// Reads `count` raw bytes from the current position.
    #[wasm_bindgen(js_name = readBytes)]
    pub fn read_bytes(&mut self, count: usize) -> Result<Vec<u8>, JsError> {
        self.with_reader(|r| r.read_bytes(count).map(<[u8]>::to_vec))
            .map_err(|e| decode_error("read_bytes", &e))
    }

    pub fn position(&self) -> usize {
        self.position
    }

    pub fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.position)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::{CdrDecoder, CdrEncoder, Endianness, endianness_from_u8, max_align_of};
    use alloc::string::String;
    use alloc::vec;
    use alloc::vec::Vec;

    /// The octets the vendors write for `primitive-mixed` of
    /// tests/interop-corpus (every primitive in one `@final` struct), from
    /// tests/interop-corpus/expected-bytes.tsv: the row of `encoding`,
    /// `endian` whose dialects name `dialect`.
    fn expected(encoding: &str, endian: &str, dialect: &str) -> Vec<u8> {
        let table = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/interop-corpus/expected-bytes.tsv"
        ))
        .expect("tests/interop-corpus/expected-bytes.tsv");
        let row = table
            .lines()
            .filter(|line| !line.starts_with('#'))
            .map(|line| line.split('\t').collect::<Vec<_>>())
            .find(|f| {
                f.len() == 6
                    && f[0] == "primitive-mixed"
                    && f[1] == "values"
                    && f[2] == encoding
                    && f[3] == endian
                    && f[4].split(',').any(|d| d == dialect)
            })
            .unwrap_or_else(|| panic!("no row primitive-mixed {encoding} {endian} {dialect}"));
        (0..row[5].len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&row[5][i..i + 2], 16).expect("hex"))
            .collect()
    }

    /// `primitive-mixed`'s values (tests/interop-corpus/primitive-mixed/values.json)
    /// through the encoder: a `char` and `octet`s as one octet each, the `wchar`
    /// as its UTF-16 unit, the signed values and the floats as their bit patterns.
    fn encode_primitive_mixed(endianness: u8, encoding: u8) -> Vec<u8> {
        let mut enc = CdrEncoder::new(endianness, Some(encoding)).expect("encoder");
        enc.write_u8(1); // flag
        enc.write_u8(171); // raw_octet
        enc.write_u8(254); // unsigned_byte
        enc.write_u8((-13i8) as u8); // signed_byte
        enc.write_u8(b'Z'); // letter
        enc.write_u16(0x00e4); // wide_letter 'ä'
        enc.write_u16((-12345i16) as u16); // short_signed
        enc.write_u16(48879); // short_unsigned
        enc.write_u32((-1_234_567i32) as u32); // long_signed
        enc.write_u32(3_735_928_559); // long_unsigned
        enc.write_u64((-987_654_321_987i64) as u64); // llong_signed
        enc.write_u64(16_045_690_984_833_335_998); // llong_unsigned
        enc.write_u32((-3.75f32).to_bits()); // single_float
        enc.write_u64(core::f64::consts::PI.to_bits()); // double_float
        enc.finish()
    }

    /// DDS-XTypes 1.3 §7.4.1.1.1 and §7.4.3.2.2 Table 37: under XCDR2 `MAXALIGN`
    /// is 4, so the `long long` after two `long`s at offset 20 and the `double`
    /// at 44 take no padding. The octets are the specification row of the
    /// table, which Cyclone DDS, RTI Connext, Fast DDS and OpenDDS agree on.
    #[test]
    fn xtypes_7_4_3_2_2_xcdr2_caps_the_alignment_at_four_as_the_vendors_write_it() {
        let octets = encode_primitive_mixed(1, 2);
        assert_eq!(octets, expected("xcdr2", "be", "specification"));

        let mut dec = CdrDecoder::new(octets, 1, Some(2)).expect("decoder");
        assert_eq!(dec.max_align(), 4);
        for _ in 0..5 {
            dec.read_u8().expect("octet");
        }
        assert_eq!(dec.read_u16().expect("wchar"), 0x00e4);
        dec.read_u16().expect("short");
        dec.read_u16().expect("ushort");
        dec.read_u32().expect("long");
        assert_eq!(dec.read_u32().expect("ulong"), 3_735_928_559);
        assert_eq!(dec.position(), 20);
        assert_eq!(dec.read_u64().expect("llong"), (-987_654_321_987i64) as u64);
        assert_eq!(dec.read_u64().expect("ullong"), 16_045_690_984_833_335_998);
        assert_eq!(dec.read_u32().expect("float"), (-3.75f32).to_bits());
        assert_eq!(dec.position(), 40);
        assert_eq!(
            dec.read_u64().expect("double"),
            core::f64::consts::PI.to_bits()
        );
        assert_eq!(dec.remaining(), 0);
    }

    /// The same values under XCDR1 (`MAXALIGN` 8, and the default when no
    /// version is given): four octets of padding before the `long long` and
    /// before the `double`, as Cyclone DDS, Fast DDS and OpenDDS write it in
    /// both byte orders.
    #[test]
    fn xtypes_7_4_3_2_2_xcdr1_aligns_eight_octet_values_to_eight_as_the_vendors_write_it() {
        assert_eq!(
            encode_primitive_mixed(1, 1),
            expected("xcdr1", "be", "cyclone")
        );
        assert_eq!(
            encode_primitive_mixed(0, 1),
            expected("xcdr1", "le", "cyclone")
        );
        let default = CdrEncoder::new(0, None).expect("encoder without a version");
        assert_eq!(default.max_align(), 8);

        let octets = expected("xcdr1", "le", "cyclone");
        let mut dec = CdrDecoder::new(octets, 0, None).expect("decoder");
        for _ in 0..5 {
            dec.read_u8().expect("octet");
        }
        for _ in 0..3 {
            dec.read_u16().expect("16-bit");
        }
        dec.read_u32().expect("long");
        dec.read_u32().expect("ulong");
        assert_eq!(dec.read_u64().expect("llong"), (-987_654_321_987i64) as u64);
        assert_eq!(dec.position(), 32);
    }

    /// An encoding version other than 1 and 2 is refused, not guessed.
    #[test]
    fn xtypes_7_4_3_2_2_an_unknown_encoding_version_is_refused() {
        assert_eq!(max_align_of(None), Ok(8));
        assert_eq!(max_align_of(Some(1)), Ok(8));
        assert_eq!(max_align_of(Some(2)), Ok(4));
        assert_eq!(max_align_of(Some(0)), Err(0));
        assert_eq!(max_align_of(Some(3)), Err(3));
    }

    /// DDS-XTypes 1.3 §7.4.1.1.1: the byte order of the stream decides the
    /// order of a value's octets.
    #[test]
    fn xtypes_7_4_1_1_1_the_byte_order_reaches_the_wire() {
        let mut big = CdrEncoder::new(1, None).expect("big-endian encoder");
        big.write_u32(0x0102_0304);
        assert_eq!(big.finish(), vec![0x01, 0x02, 0x03, 0x04]);

        let mut little = CdrEncoder::new(0, None).expect("little-endian encoder");
        little.write_u32(0x0102_0304);
        assert_eq!(little.finish(), vec![0x04, 0x03, 0x02, 0x01]);
    }

    /// DDS-XTypes 1.3 §7.4.1.1.2: a string is its length with the NUL, the
    /// characters and the NUL.
    #[test]
    fn xtypes_7_4_1_1_2_a_string_is_its_length_its_characters_and_a_nul() {
        let mut enc = CdrEncoder::new(0, Some(2)).expect("encoder");
        enc.write_string("AB");
        let octets = enc.finish();
        assert_eq!(octets, vec![3, 0, 0, 0, b'A', b'B', 0]);
        let mut dec = CdrDecoder::new(octets, 0, Some(2)).expect("decoder");
        assert_eq!(dec.read_string().expect("string"), String::from("AB"));
    }

    /// `write_bytes` puts the octets down unframed, and `read_bytes` takes
    /// exactly as many back.
    #[test]
    fn raw_bytes_pass_through_unframed() {
        let mut enc = CdrEncoder::new(0, None).expect("encoder");
        enc.write_bytes(&[0xde, 0xad, 0xbe, 0xef]);
        let bytes = enc.finish();
        assert_eq!(bytes, vec![0xde, 0xad, 0xbe, 0xef]);

        let mut dec = CdrDecoder::new(bytes, 0, None).expect("decoder");
        assert_eq!(
            dec.read_bytes(4).expect("bytes"),
            vec![0xde, 0xad, 0xbe, 0xef]
        );
        assert_eq!(dec.position(), 4);
    }

    /// An endianness tag the ABI does not define is refused, not guessed.
    /// Asserted on `endianness_from_u8` rather than on the constructors,
    /// because the constructors' error type is a `JsError` and that one can
    /// only be built on a wasm target.
    #[test]
    fn unknown_endianness_tag_is_refused() {
        assert_eq!(endianness_from_u8(0), Ok(Endianness::Little));
        assert_eq!(endianness_from_u8(1), Ok(Endianness::Big));
        assert_eq!(endianness_from_u8(2), Err(2));
        assert_eq!(endianness_from_u8(7), Err(7));
    }

    /// A truncated stream reports the error instead of returning a value the
    /// bytes do not carry. Same reason as above for going through
    /// `with_reader`: the public method's error type is a `JsError`.
    #[test]
    fn truncated_stream_reports_rather_than_invents() {
        let mut dec = CdrDecoder::new(vec![0x01, 0x02], 0, None).expect("decoder");
        assert_eq!(dec.read_u8().expect("u8"), 0x01);
        assert!(dec.with_reader(|r| r.read_u32()).is_err());
    }
}
