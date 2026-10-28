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
//!   * an XCDR1/XCDR2 encoder and decoder for primitives, strings and bytes;
//!   * the endianness constants and the version string.
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
    #[wasm_bindgen(constructor)]
    pub fn new(endianness: u8) -> Result<CdrEncoder, JsError> {
        Ok(Self {
            inner: Writer::new(endianness_from_u8(endianness).map_err(invalid_endianness)?),
        })
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
    position: usize,
}

#[wasm_bindgen]
impl CdrDecoder {
    #[wasm_bindgen(constructor)]
    pub fn new(bytes: Vec<u8>, endianness: u8) -> Result<CdrDecoder, JsError> {
        Ok(Self {
            bytes,
            endianness: endianness_from_u8(endianness).map_err(invalid_endianness)?,
            position: 0,
        })
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
        let mut reader = Reader::new(&self.bytes, self.endianness);
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
    use super::{CdrDecoder, CdrEncoder, Endianness, endianness_from_u8};
    use alloc::vec;

    /// The same sequence the JS smoke test drives through the generated
    /// bindings, so a break shows up in `cargo test` and not only after a
    /// `wasm-pack build`.
    #[test]
    fn primitive_roundtrip_little_endian() {
        let mut enc = CdrEncoder::new(0).expect("little-endian encoder");
        enc.write_u8(0x42);
        enc.write_u16(0xbeef);
        enc.write_u32(0xcafe_babe);
        enc.write_u64(0x1122_3344_5566_7788);
        enc.write_string("Hallo, ZeroDDS!");
        let bytes = enc.finish();

        let mut dec = CdrDecoder::new(bytes, 0).expect("little-endian decoder");
        assert_eq!(dec.read_u8().expect("u8"), 0x42);
        assert_eq!(dec.read_u16().expect("u16"), 0xbeef);
        assert_eq!(dec.read_u32().expect("u32"), 0xcafe_babe);
        assert_eq!(dec.read_u64().expect("u64"), 0x1122_3344_5566_7788);
        assert_eq!(dec.read_string().expect("string"), "Hallo, ZeroDDS!");
        assert_eq!(dec.remaining(), 0);
    }

    /// The byte order reaches the wire: the same value comes out reversed.
    #[test]
    fn u32_byte_order_is_the_streams() {
        let mut big = CdrEncoder::new(1).expect("big-endian encoder");
        big.write_u32(0x0102_0304);
        assert_eq!(big.finish(), vec![0x01, 0x02, 0x03, 0x04]);

        let mut little = CdrEncoder::new(0).expect("little-endian encoder");
        little.write_u32(0x0102_0304);
        assert_eq!(little.finish(), vec![0x04, 0x03, 0x02, 0x01]);
    }

    /// An alignment call before an already-aligning write changes nothing —
    /// the position after a `write_u8` plus `align(2)` is the position
    /// `write_u16` would have reached on its own.
    #[test]
    fn align_is_idempotent_against_the_typed_writes() {
        let mut with_align = CdrEncoder::new(0).expect("encoder");
        with_align.write_u8(1);
        with_align.align(2);
        with_align.write_u16(2);

        let mut without = CdrEncoder::new(0).expect("encoder");
        without.write_u8(1);
        without.write_u16(2);

        assert_eq!(with_align.position(), 4);
        assert_eq!(with_align.finish(), without.finish());
    }

    /// `write_bytes` puts the octets down unframed, and `read_bytes` takes
    /// exactly as many back.
    #[test]
    fn raw_bytes_pass_through_unframed() {
        let mut enc = CdrEncoder::new(0).expect("encoder");
        enc.write_bytes(&[0xde, 0xad, 0xbe, 0xef]);
        let bytes = enc.finish();
        assert_eq!(bytes, vec![0xde, 0xad, 0xbe, 0xef]);

        let mut dec = CdrDecoder::new(bytes, 0).expect("decoder");
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
        let mut dec = CdrDecoder::new(vec![0x01, 0x02], 0).expect("decoder");
        assert_eq!(dec.read_u8().expect("u8"), 0x01);
        assert!(dec.with_reader(|r| r.read_u32()).is_err());
    }
}
