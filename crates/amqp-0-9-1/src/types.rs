// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! AMQP 0.9.1 wire types (§4.2). All multi-byte integers are **big-endian**.
//! These are the building blocks of method arguments and field tables — an
//! entirely different type system from AMQP 1.0's described types.

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

/// A wire decode/encode error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireError {
    /// Not enough bytes to decode the requested type.
    Truncated,
    /// A `shortstr` exceeded 255 bytes.
    ShortStrTooLong,
    /// A field-table value used an unsupported type tag.
    UnsupportedFieldType(u8),
    /// A string field was not valid UTF-8.
    NotUtf8,
}

/// A reader over a big-endian AMQP 0.9.1 byte slice.
pub struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    /// New reader at offset 0.
    #[must_use]
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }
    /// Bytes consumed so far.
    #[must_use]
    pub fn position(&self) -> usize {
        self.pos
    }
    /// Remaining bytes.
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.buf.len() - self.pos
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], WireError> {
        if self.pos + n > self.buf.len() {
            return Err(WireError::Truncated);
        }
        let s = &self.buf[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }
    /// octet.
    pub fn u8(&mut self) -> Result<u8, WireError> {
        Ok(self.take(1)?[0])
    }
    /// short.
    pub fn u16(&mut self) -> Result<u16, WireError> {
        let b = self.take(2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }
    /// long.
    pub fn u32(&mut self) -> Result<u32, WireError> {
        let b = self.take(4)?;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }
    /// longlong.
    pub fn u64(&mut self) -> Result<u64, WireError> {
        let b = self.take(8)?;
        let mut a = [0u8; 8];
        a.copy_from_slice(b);
        Ok(u64::from_be_bytes(a))
    }
    /// shortstr: 1-byte length + UTF-8 bytes.
    pub fn short_str(&mut self) -> Result<String, WireError> {
        let n = self.u8()? as usize;
        let b = self.take(n)?;
        core::str::from_utf8(b)
            .map(alloc::string::ToString::to_string)
            .map_err(|_| WireError::NotUtf8)
    }
    /// longstr: 4-byte length + raw bytes (returned verbatim).
    pub fn long_str(&mut self) -> Result<Vec<u8>, WireError> {
        let n = self.u32()? as usize;
        Ok(self.take(n)?.to_vec())
    }
    /// Skips a field-table (4-byte length + body) without interpreting it.
    pub fn skip_field_table(&mut self) -> Result<(), WireError> {
        let n = self.u32()? as usize;
        let _ = self.take(n)?;
        Ok(())
    }
    /// signed octet.
    pub fn i8(&mut self) -> Result<i8, WireError> {
        Ok(self.u8()? as i8)
    }
    /// signed short.
    pub fn i16(&mut self) -> Result<i16, WireError> {
        Ok(self.u16()? as i16)
    }
    /// signed long.
    pub fn i32(&mut self) -> Result<i32, WireError> {
        Ok(self.u32()? as i32)
    }
    /// signed longlong.
    pub fn i64(&mut self) -> Result<i64, WireError> {
        Ok(self.u64()? as i64)
    }
    /// 32-bit float.
    pub fn f32(&mut self) -> Result<f32, WireError> {
        Ok(f32::from_bits(self.u32()?))
    }
    /// 64-bit float.
    pub fn f64(&mut self) -> Result<f64, WireError> {
        Ok(f64::from_bits(self.u64()?))
    }
    /// Reads a full field-table (§4.2.5.5): 4-byte byte-length, then
    /// `(shortstr name, typed value)` pairs until the length is consumed.
    ///
    /// # Errors
    /// [`WireError`] on truncation or an unsupported value type tag.
    pub fn field_table(&mut self) -> Result<Vec<(String, FieldValue)>, WireError> {
        let len = self.u32()? as usize;
        let end = self.pos + len;
        if end > self.buf.len() {
            return Err(WireError::Truncated);
        }
        let mut out = Vec::new();
        while self.pos < end {
            let name = self.short_str()?;
            let val = self.field_value()?;
            out.push((name, val));
        }
        Ok(out)
    }
    /// Reads a single typed field value (a 1-byte tag + payload).
    ///
    /// # Errors
    /// [`WireError::UnsupportedFieldType`] for an unknown tag.
    pub fn field_value(&mut self) -> Result<FieldValue, WireError> {
        let tag = self.u8()?;
        Ok(match tag {
            b't' => FieldValue::Bool(self.u8()? != 0),
            b'b' => FieldValue::I8(self.i8()?),
            b'B' => FieldValue::U8(self.u8()?),
            b's' => FieldValue::I16(self.i16()?),
            b'u' => FieldValue::U16(self.u16()?),
            b'I' => FieldValue::I32(self.i32()?),
            b'i' => FieldValue::U32(self.u32()?),
            b'l' => FieldValue::I64(self.i64()?),
            b'f' => FieldValue::F32(self.f32()?),
            b'd' => FieldValue::F64(self.f64()?),
            b'D' => {
                let scale = self.u8()?;
                let value = self.i32()?;
                FieldValue::Decimal { scale, value }
            }
            b'S' => FieldValue::LongStr(self.long_str()?),
            b'T' => FieldValue::Timestamp(self.u64()?),
            b'V' => FieldValue::Void,
            b'x' => FieldValue::Bytes(self.long_str()?),
            b'F' => {
                let len = self.u32()? as usize;
                let end = self.pos + len;
                if end > self.buf.len() {
                    return Err(WireError::Truncated);
                }
                let mut inner = Vec::new();
                while self.pos < end {
                    let name = self.short_str()?;
                    inner.push((name, self.field_value()?));
                }
                FieldValue::Table(inner)
            }
            b'A' => {
                let len = self.u32()? as usize;
                let end = self.pos + len;
                if end > self.buf.len() {
                    return Err(WireError::Truncated);
                }
                let mut items = Vec::new();
                while self.pos < end {
                    items.push(self.field_value()?);
                }
                FieldValue::Array(items)
            }
            other => return Err(WireError::UnsupportedFieldType(other)),
        })
    }
}

/// A typed AMQP 0.9.1 / RabbitMQ field-table value (§4.2.5.5). Covers the
/// full set of standard + RabbitMQ-extension value types.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldValue {
    /// `t` — boolean.
    Bool(bool),
    /// `b` — signed 8-bit.
    I8(i8),
    /// `B` — unsigned 8-bit.
    U8(u8),
    /// `s` — signed 16-bit.
    I16(i16),
    /// `u` — unsigned 16-bit.
    U16(u16),
    /// `I` — signed 32-bit.
    I32(i32),
    /// `i` — unsigned 32-bit.
    U32(u32),
    /// `l` — signed 64-bit.
    I64(i64),
    /// `f` — 32-bit float.
    F32(f32),
    /// `d` — 64-bit float.
    F64(f64),
    /// `D` — decimal (scale + signed 32-bit mantissa).
    Decimal {
        /// Number of decimal places.
        scale: u8,
        /// Signed mantissa.
        value: i32,
    },
    /// `S` — long string (raw bytes).
    LongStr(Vec<u8>),
    /// `T` — POSIX timestamp (unsigned 64-bit seconds).
    Timestamp(u64),
    /// `F` — nested field-table.
    Table(Vec<(String, FieldValue)>),
    /// `A` — array of values.
    Array(Vec<FieldValue>),
    /// `x` — byte array (RabbitMQ extension).
    Bytes(Vec<u8>),
    /// `V` — the void/no value.
    Void,
}

impl FieldValue {
    /// A `LongStr` from a UTF-8 string (the common header case).
    #[must_use]
    pub fn str(s: &str) -> Self {
        FieldValue::LongStr(s.as_bytes().to_vec())
    }
}

/// A big-endian writer.
#[derive(Default)]
pub struct Writer {
    buf: Vec<u8>,
}

impl Writer {
    /// New empty writer.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    /// Consumes the writer, returning the bytes.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.buf
    }
    /// Current length.
    #[must_use]
    pub fn len(&self) -> usize {
        self.buf.len()
    }
    /// Whether empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }
    /// octet.
    pub fn u8(&mut self, v: u8) -> &mut Self {
        self.buf.push(v);
        self
    }
    /// short.
    pub fn u16(&mut self, v: u16) -> &mut Self {
        self.buf.extend_from_slice(&v.to_be_bytes());
        self
    }
    /// long.
    pub fn u32(&mut self, v: u32) -> &mut Self {
        self.buf.extend_from_slice(&v.to_be_bytes());
        self
    }
    /// longlong.
    pub fn u64(&mut self, v: u64) -> &mut Self {
        self.buf.extend_from_slice(&v.to_be_bytes());
        self
    }
    /// raw bytes.
    pub fn bytes(&mut self, b: &[u8]) -> &mut Self {
        self.buf.extend_from_slice(b);
        self
    }
    /// shortstr: 1-byte length + bytes (errors if > 255).
    ///
    /// # Errors
    /// [`WireError::ShortStrTooLong`] if `s` exceeds 255 bytes.
    pub fn short_str(&mut self, s: &str) -> Result<&mut Self, WireError> {
        let b = s.as_bytes();
        if b.len() > 255 {
            return Err(WireError::ShortStrTooLong);
        }
        self.u8(b.len() as u8).bytes(b);
        Ok(self)
    }
    /// longstr: 4-byte length + bytes.
    pub fn long_str(&mut self, b: &[u8]) -> &mut Self {
        self.u32(b.len() as u32).bytes(b)
    }
    /// An **empty field-table** (`0x00000000`) — the common case for
    /// arguments/properties we do not populate.
    pub fn empty_field_table(&mut self) -> &mut Self {
        self.u32(0)
    }
    /// A minimal field-table with `S` (longstr) string entries — enough for the
    /// `client-properties` we advertise in connection.start-ok.
    pub fn field_table_strs(&mut self, entries: &[(&str, &str)]) -> Result<&mut Self, WireError> {
        let mut body = Writer::new();
        for (k, v) in entries {
            body.short_str(k)?;
            body.u8(b'S'); // longstr field value
            body.long_str(v.as_bytes());
        }
        let body = body.into_bytes();
        self.long_str(&body);
        Ok(self)
    }
    /// signed octet.
    pub fn i8(&mut self, v: i8) -> &mut Self {
        self.u8(v as u8)
    }
    /// signed short.
    pub fn i16(&mut self, v: i16) -> &mut Self {
        self.u16(v as u16)
    }
    /// signed long.
    pub fn i32(&mut self, v: i32) -> &mut Self {
        self.u32(v as u32)
    }
    /// signed longlong.
    pub fn i64(&mut self, v: i64) -> &mut Self {
        self.u64(v as u64)
    }
    /// 32-bit float.
    pub fn f32(&mut self, v: f32) -> &mut Self {
        self.u32(v.to_bits())
    }
    /// 64-bit float.
    pub fn f64(&mut self, v: f64) -> &mut Self {
        self.u64(v.to_bits())
    }
    /// Writes a single typed field value (tag + payload).
    ///
    /// # Errors
    /// [`WireError`] on a nested `shortstr` overflow.
    pub fn field_value(&mut self, v: &FieldValue) -> Result<&mut Self, WireError> {
        match v {
            FieldValue::Bool(b) => {
                self.u8(b't').u8(u8::from(*b));
            }
            FieldValue::I8(n) => {
                self.u8(b'b').i8(*n);
            }
            FieldValue::U8(n) => {
                self.u8(b'B').u8(*n);
            }
            FieldValue::I16(n) => {
                self.u8(b's').i16(*n);
            }
            FieldValue::U16(n) => {
                self.u8(b'u').u16(*n);
            }
            FieldValue::I32(n) => {
                self.u8(b'I').i32(*n);
            }
            FieldValue::U32(n) => {
                self.u8(b'i').u32(*n);
            }
            FieldValue::I64(n) => {
                self.u8(b'l').i64(*n);
            }
            FieldValue::F32(n) => {
                self.u8(b'f').f32(*n);
            }
            FieldValue::F64(n) => {
                self.u8(b'd').f64(*n);
            }
            FieldValue::Decimal { scale, value } => {
                self.u8(b'D').u8(*scale).i32(*value);
            }
            FieldValue::LongStr(b) => {
                self.u8(b'S').long_str(b);
            }
            FieldValue::Timestamp(t) => {
                self.u8(b'T').u64(*t);
            }
            FieldValue::Bytes(b) => {
                self.u8(b'x').long_str(b);
            }
            FieldValue::Void => {
                self.u8(b'V');
            }
            FieldValue::Table(entries) => {
                let mut body = Writer::new();
                for (k, val) in entries {
                    body.short_str(k)?;
                    body.field_value(val)?;
                }
                self.u8(b'F').long_str(&body.into_bytes());
            }
            FieldValue::Array(items) => {
                let mut body = Writer::new();
                for item in items {
                    body.field_value(item)?;
                }
                self.u8(b'A').long_str(&body.into_bytes());
            }
        }
        Ok(self)
    }
    /// Writes a full field-table (`4-byte length + entries`) from typed values.
    ///
    /// # Errors
    /// [`WireError`] on a nested `shortstr` overflow.
    pub fn field_table(&mut self, entries: &[(&str, FieldValue)]) -> Result<&mut Self, WireError> {
        let mut body = Writer::new();
        for (k, v) in entries {
            body.short_str(k)?;
            body.field_value(v)?;
        }
        self.long_str(&body.into_bytes());
        Ok(self)
    }
}

/// Packs up to 8 booleans into a single AMQP `bit` octet (LSB = first flag).
#[must_use]
pub fn pack_bits(flags: &[bool]) -> u8 {
    let mut b = 0u8;
    for (i, &f) in flags.iter().take(8).enumerate() {
        if f {
            b |= 1 << i;
        }
    }
    b
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn integer_roundtrip_big_endian() {
        let mut w = Writer::new();
        w.u8(0x12)
            .u16(0x3456)
            .u32(0x789a_bcde)
            .u64(0x0102_0304_0506_0708);
        let bytes = w.into_bytes();
        assert_eq!(bytes[0], 0x12);
        assert_eq!(&bytes[1..3], &[0x34, 0x56]); // big-endian
        let mut r = Reader::new(&bytes);
        assert_eq!(r.u8().unwrap(), 0x12);
        assert_eq!(r.u16().unwrap(), 0x3456);
        assert_eq!(r.u32().unwrap(), 0x789a_bcde);
        assert_eq!(r.u64().unwrap(), 0x0102_0304_0506_0708);
    }

    #[test]
    fn shortstr_longstr_roundtrip() {
        let mut w = Writer::new();
        w.short_str("guest").unwrap().long_str(b"\x00user\x00pass");
        let bytes = w.into_bytes();
        let mut r = Reader::new(&bytes);
        assert_eq!(r.short_str().unwrap(), "guest");
        assert_eq!(r.long_str().unwrap(), b"\x00user\x00pass");
    }

    #[test]
    fn shortstr_rejects_over_255() {
        let big = "x".repeat(256);
        let mut w = Writer::new();
        assert_eq!(
            w.short_str(&big).map(|_| ()).unwrap_err(),
            WireError::ShortStrTooLong
        );
    }

    #[test]
    fn field_table_strs_then_skip() {
        let mut w = Writer::new();
        w.field_table_strs(&[("product", "ZeroDDS"), ("version", "1.0")])
            .unwrap();
        let bytes = w.into_bytes();
        let mut r = Reader::new(&bytes);
        r.skip_field_table().unwrap();
        assert_eq!(r.remaining(), 0, "the whole table must be consumed");
    }

    #[test]
    fn bit_packing() {
        assert_eq!(pack_bits(&[true, false, true]), 0b101);
        assert_eq!(pack_bits(&[false, true]), 0b010);
    }

    #[test]
    fn field_table_typed_roundtrip() {
        let nested = FieldValue::Table(vec![("x".into(), FieldValue::I32(7))]);
        let entries: &[(&str, FieldValue)] = &[
            ("ok", FieldValue::Bool(true)),
            ("n", FieldValue::I64(-42)),
            ("u", FieldValue::U16(513)),
            ("ratio", FieldValue::F64(1.5)),
            (
                "dec",
                FieldValue::Decimal {
                    scale: 2,
                    value: 12345,
                },
            ),
            ("name", FieldValue::str("ZeroDDS")),
            ("ts", FieldValue::Timestamp(1_700_000_000)),
            (
                "arr",
                FieldValue::Array(vec![FieldValue::U8(1), FieldValue::U8(2)]),
            ),
            ("sub", nested),
            ("nothing", FieldValue::Void),
        ];
        let mut w = Writer::new();
        w.field_table(entries).unwrap();
        let bytes = w.into_bytes();
        let mut r = Reader::new(&bytes);
        let decoded = r.field_table().unwrap();
        assert_eq!(r.remaining(), 0, "table fully consumed");
        assert_eq!(decoded.len(), entries.len());
        for ((k, v), (dk, dv)) in entries.iter().zip(decoded.iter()) {
            assert_eq!(k, dk);
            assert_eq!(v, dv);
        }
    }

    #[test]
    fn unknown_field_type_rejected() {
        // tag 'Z' is not a valid AMQP field type.
        let mut w = Writer::new();
        w.u32(3); // table byte-length: shortstr "k" (2) + tag (1)
        w.short_str("k").unwrap();
        w.u8(b'Z');
        let bytes = w.into_bytes();
        let mut r = Reader::new(&bytes);
        assert_eq!(
            r.field_table().unwrap_err(),
            WireError::UnsupportedFieldType(b'Z')
        );
    }
}
