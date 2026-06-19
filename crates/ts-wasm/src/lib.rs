// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-ts-wasm`. Safety classification: **STANDARD** (FFI boundary
//! via wasm-bindgen; std allowed; no direct hardware/syscall access).
//!
//! WASM bindings for the ZeroDDS XCDR codec. Unlike the Node variant
//! (`zerodds-ts-node`), WASM cannot use UDP sockets or threads —
//! live DDS in the browser needs a WebSocket bridge
//! (`crates/websocket-bridge`).
//!
//! What is exposed here:
//!   * XCDR1/XCDR2 encoder + decoder for primitives + strings + bytes
//!   * KeyHash computation (XTypes 1.3 §7.6.8)
//!   * endianness constants + version string
//!
//! Use cases:
//!   * a browser frontend converts form data into XCDR, sends it via
//!     WebSocket to a DDS gateway
//!   * the browser receives XCDR bytes, decodes them client-side
//!   * schema validation + type checks without a server roundtrip

#![no_std]
// FFI bindings via `#[wasm_bindgen]` macros — the generated wrappers
// automatically get no rustdoc and are documented from the JS layer.
// Allow that here at the crate level; our own fns are
// doc'd anyway.
#![allow(missing_docs)]

extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use wasm_bindgen::prelude::*;
use zerodds_cdr::{BufferReader, BufferWriter, Endianness};

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

fn endianness_from_u8(value: u8) -> Result<Endianness, JsError> {
    match value {
        0 => Ok(Endianness::Little),
        1 => Ok(Endianness::Big),
        other => Err(JsError::new(&format!("invalid endianness {other}"))),
    }
}

/// XCDR encoder. Buffers bytes until `finish()` is called.
#[wasm_bindgen]
pub struct CdrEncoder {
    inner: BufferWriter,
}

#[wasm_bindgen]
impl CdrEncoder {
    #[wasm_bindgen(constructor)]
    pub fn new(endianness: u8) -> Result<CdrEncoder, JsError> {
        let endian = endianness_from_u8(endianness)?;
        Ok(Self {
            inner: BufferWriter::new(endian),
        })
    }

    #[wasm_bindgen(js_name = writeU8)]
    pub fn write_u8(&mut self, value: u8) -> Result<(), JsError> {
        self.inner
            .write_u8(value)
            .map_err(|e| JsError::new(&format!("write_u8: {e:?}")))
    }

    #[wasm_bindgen(js_name = writeU16)]
    pub fn write_u16(&mut self, value: u16) -> Result<(), JsError> {
        self.inner
            .write_u16(value)
            .map_err(|e| JsError::new(&format!("write_u16: {e:?}")))
    }

    #[wasm_bindgen(js_name = writeU32)]
    pub fn write_u32(&mut self, value: u32) -> Result<(), JsError> {
        self.inner
            .write_u32(value)
            .map_err(|e| JsError::new(&format!("write_u32: {e:?}")))
    }

    #[wasm_bindgen(js_name = writeU64)]
    pub fn write_u64(&mut self, value: u64) -> Result<(), JsError> {
        self.inner
            .write_u64(value)
            .map_err(|e| JsError::new(&format!("write_u64: {e:?}")))
    }

    #[wasm_bindgen(js_name = writeString)]
    pub fn write_string(&mut self, value: &str) -> Result<(), JsError> {
        self.inner
            .write_string(value)
            .map_err(|e| JsError::new(&format!("write_string: {e:?}")))
    }

    #[wasm_bindgen(js_name = writeBytes)]
    pub fn write_bytes(&mut self, data: &[u8]) -> Result<(), JsError> {
        self.inner
            .write_bytes(data)
            .map_err(|e| JsError::new(&format!("write_bytes: {e:?}")))
    }

    pub fn align(&mut self, alignment: usize) {
        self.inner.align(alignment);
    }

    pub fn position(&self) -> usize {
        self.inner.position()
    }

    /// Finalizes the encoder and returns the accumulated bytes.
    /// After finish() the encoder is unusable.
    pub fn finish(self) -> Vec<u8> {
        self.inner.into_bytes()
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
        let endian = endianness_from_u8(endianness)?;
        Ok(Self {
            bytes,
            endianness: endian,
            position: 0,
        })
    }

    fn reader(&self) -> BufferReader<'_> {
        let mut r = BufferReader::new(&self.bytes, self.endianness);
        // The reader position is internal to BufferReader — we track it
        // separately and set it via skip() before each read.
        let _ = r.read_bytes(self.position);
        r
    }

    #[wasm_bindgen(js_name = readU8)]
    pub fn read_u8(&mut self) -> Result<u8, JsError> {
        let mut r = self.reader();
        let v = r
            .read_u8()
            .map_err(|e| JsError::new(&format!("read_u8: {e:?}")))?;
        self.position = r.position();
        Ok(v)
    }

    #[wasm_bindgen(js_name = readU16)]
    pub fn read_u16(&mut self) -> Result<u16, JsError> {
        let mut r = self.reader();
        let v = r
            .read_u16()
            .map_err(|e| JsError::new(&format!("read_u16: {e:?}")))?;
        self.position = r.position();
        Ok(v)
    }

    #[wasm_bindgen(js_name = readU32)]
    pub fn read_u32(&mut self) -> Result<u32, JsError> {
        let mut r = self.reader();
        let v = r
            .read_u32()
            .map_err(|e| JsError::new(&format!("read_u32: {e:?}")))?;
        self.position = r.position();
        Ok(v)
    }

    #[wasm_bindgen(js_name = readU64)]
    pub fn read_u64(&mut self) -> Result<u64, JsError> {
        let mut r = self.reader();
        let v = r
            .read_u64()
            .map_err(|e| JsError::new(&format!("read_u64: {e:?}")))?;
        self.position = r.position();
        Ok(v)
    }

    #[wasm_bindgen(js_name = readString)]
    pub fn read_string(&mut self) -> Result<String, JsError> {
        let mut r = self.reader();
        let v = r
            .read_string()
            .map_err(|e| JsError::new(&format!("read_string: {e:?}")))?;
        self.position = r.position();
        Ok(v)
    }

    pub fn position(&self) -> usize {
        self.position
    }

    pub fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.position)
    }
}
