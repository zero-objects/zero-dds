// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! KeyHash-Berechnung fuer DDS-Topics (XTypes 1.3 §7.6.8 + DDSI-RTPS
//! 2.5 §9.6.4.8).
//!
//! Der **PID_KEY_HASH** (0x0070, 16 Byte) im Inline-QoS einer DATA/
//! DATA_FRAG-Submessage adressiert die **Instanz** eines Sample.
//! Ein Reader/Persistence-Service kann anhand dieses Hashes
//! Samples derselben Instanz korrelieren, ohne den Payload deserialisieren
//! zu muessen.
//!
//! # Algorithmus (XTypes 1.3 §7.6.8 Steps 1-5)
//!
//! 1. **Step 1** — Konstruktion eines `FooKeyHolder`-Aequivalents: nur
//!    die `@key`-Member von `Foo`, sortiert nach `member_id` aufsteigend.
//! 2. **Step 2** — Werte mit den `@key`-Werten der Instanz fuellen.
//! 3. **Step 3** — Nested aggregate Members rekursiv expandieren (jeder
//!    Sub-`@key`-Member ebenfalls in den KeyHolder).
//! 4. **Step 4** — Serialisierung mit **PLAIN_CDR2 / Big-Endian**,
//!    alignment 4, **ohne** Encapsulation-Header und **ohne** Member-
//!    Header.
//! 5. **Step 5** — Hash:
//!    * Wenn die **maximale** serialisierte Groesse des KeyHolders
//!      <= 16 Byte: KeyHash = Bytes von Step 4, mit Null-Bytes
//!      auf 16 Byte gepadded.
//!    * Sonst: KeyHash = MD5(Bytes von Step 4)[0..16].
//!
//! Die Entscheidung ist **statisch pro Topic-Type** — sie haengt von der
//! Maximal-Groesse aller @key-Member ab, NICHT von der konkreten
//! Instanz-Groesse. Ein Topic mit `@key string<8>` hat max=12 (4 byte
//! length + 8 byte content) → zero-pad. Ein Topic mit `@key string`
//! (ohne max-bound) hat keine fixe max-Groesse → MD5.
//!
//! # WP 1.B Scope
//!
//! Dieses Modul liefert die low-level [`compute_key_hash`]-Funktion.
//! Der DCPS-Sample-Encode-Pfad wiring (PID_KEY_HASH in Inline-QoS)
//! folgt in der gleichen WP, im DcpsRuntime-Encoder.

extern crate alloc;

use alloc::vec::Vec;
use zerodds_foundation::md5;

/// Wire-Laenge eines KeyHash (Spec: 16 Byte).
pub const KEY_HASH_LEN: usize = 16;

/// Berechnet den 16-Byte KeyHash einer Instanz aus dem **PLAIN_CDR2-BE**
/// serialisierten KeyHolder-Byte-Stream.
///
/// `plain_cdr2_be_bytes` MUSS bereits in PLAIN_CDR2-BE-Form vorliegen
/// (Caller stellt das sicher — typisch via `DdsType::encode_key_holder`).
/// `key_holder_max_size` ist die **statisch bekannte** maximale Groesse
/// des KeyHolder-Streams in Bytes; entscheidet zwischen Zero-Pad und
/// MD5-Pfad.
///
/// # Spec-Referenz
/// XTypes 1.3 §7.6.8.4 (Steps 5.1, 5.2).
#[must_use]
pub fn compute_key_hash(
    plain_cdr2_be_bytes: &[u8],
    key_holder_max_size: usize,
) -> [u8; KEY_HASH_LEN] {
    if key_holder_max_size <= KEY_HASH_LEN {
        // Step 5.1: zero-pad auf 16 byte.
        let mut out = [0u8; KEY_HASH_LEN];
        let n = core::cmp::min(plain_cdr2_be_bytes.len(), KEY_HASH_LEN);
        out[..n].copy_from_slice(&plain_cdr2_be_bytes[..n]);
        out
    } else {
        // Step 5.2: MD5 erste 16 byte.
        md5(plain_cdr2_be_bytes)
    }
}

/// Convenience-Builder fuer einen PLAIN_CDR2-BE-KeyHolder-Stream.
///
/// PLAIN_CDR2 unterscheidet sich von XCDR1 in Alignment-Regel: maximales
/// Alignment ist **4 Byte** (XTypes 1.3 §7.4.2.5.4) — `i64`/`f64` wird
/// nicht 8-aligned. Dieser Builder kapselt die Alignment-Logik und
/// vermeidet Drift mit dem zerodds-cdr-Buffer-Writer (der XCDR2-LE-default ist).
///
/// Member werden in der **Reihenfolge der `member_id` aufsteigend**
/// hinzugefuegt (Spec-Pflicht §7.6.8.3.1.b).
#[derive(Debug, Default)]
pub struct PlainCdr2BeKeyHolder {
    bytes: Vec<u8>,
}

impl PlainCdr2BeKeyHolder {
    /// Leerer Builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Aktuelle Stream-Laenge (typisch == max_size, wenn alle Member fest).
    #[must_use]
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// `true` wenn keine Bytes geschrieben.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Gibt die fertigen Bytes zurueck.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    /// Zugriff auf die internen Bytes ohne Konsumieren.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Padded auf `align`-Byte-Grenze (1, 2, 4 — max 4 nach §7.4.2.5.4).
    fn pad_to(&mut self, align: usize) {
        let pad = (align - (self.bytes.len() % align)) % align;
        for _ in 0..pad {
            self.bytes.push(0);
        }
    }

    /// Schreibt einen u8.
    pub fn write_u8(&mut self, v: u8) {
        self.bytes.push(v);
    }

    /// Schreibt einen i8.
    pub fn write_i8(&mut self, v: i8) {
        self.bytes.push(v as u8);
    }

    /// Schreibt einen u16 in BE.
    pub fn write_u16(&mut self, v: u16) {
        self.pad_to(2);
        self.bytes.extend_from_slice(&v.to_be_bytes());
    }

    /// Schreibt einen i16 in BE.
    pub fn write_i16(&mut self, v: i16) {
        self.pad_to(2);
        self.bytes.extend_from_slice(&v.to_be_bytes());
    }

    /// Schreibt einen u32 in BE.
    pub fn write_u32(&mut self, v: u32) {
        self.pad_to(4);
        self.bytes.extend_from_slice(&v.to_be_bytes());
    }

    /// Schreibt einen i32 in BE.
    pub fn write_i32(&mut self, v: i32) {
        self.pad_to(4);
        self.bytes.extend_from_slice(&v.to_be_bytes());
    }

    /// Schreibt einen u64 in BE — alignment **4**, NICHT 8 (PLAIN_CDR2).
    pub fn write_u64(&mut self, v: u64) {
        self.pad_to(4);
        self.bytes.extend_from_slice(&v.to_be_bytes());
    }

    /// Schreibt einen i64 in BE — alignment 4.
    pub fn write_i64(&mut self, v: i64) {
        self.pad_to(4);
        self.bytes.extend_from_slice(&v.to_be_bytes());
    }

    /// Schreibt einen f32 in BE.
    pub fn write_f32(&mut self, v: f32) {
        self.pad_to(4);
        self.bytes.extend_from_slice(&v.to_be_bytes());
    }

    /// Schreibt einen f64 in BE — alignment 4.
    pub fn write_f64(&mut self, v: f64) {
        self.pad_to(4);
        self.bytes.extend_from_slice(&v.to_be_bytes());
    }

    /// Schreibt eine bounded String in CDR-Form (4-byte length BE +
    /// utf8-Bytes inkl. terminating NUL). Caller stellt sicher, dass
    /// die Laenge die statische Bound nicht ueberschreitet (sonst
    /// kollidiert das mit `key_holder_max_size`).
    pub fn write_string(&mut self, s: &str) {
        self.pad_to(4);
        let len = u32::try_from(s.len() + 1).unwrap_or(u32::MAX);
        self.bytes.extend_from_slice(&len.to_be_bytes());
        self.bytes.extend_from_slice(s.as_bytes());
        self.bytes.push(0); // NUL-Terminator
    }

    /// Schreibt einen rohen byte-Slice (z.B. fuer Octet-Arrays).
    pub fn write_bytes(&mut self, b: &[u8]) {
        self.bytes.extend_from_slice(b);
    }
}

/// Berechnet den KeyHash aus einer **unsortierten** Liste von
/// `(member_id, key_value_bytes)`-Paaren. Die Funktion sortiert die
/// Eintraege nach `member_id` aufsteigend (Spec-Pflicht §7.6.8.3.1.b)
/// und konkateniert die `key_value_bytes` in dieser Reihenfolge — der
/// Caller liefert pro Member bereits die PLAIN_CDR2-BE-Bytes des
/// Werts, dieser Helper kapselt nur die Sortier-Pflicht.
///
/// Das ist die Lecke, gegen die §7.4.5 normativ ist: zwei Encoder-
/// Implementierungen, die ihre @key-Member in unterschiedlicher
/// Reihenfolge aufzaehlen, MUESSEN trotzdem denselben KeyHash
/// produzieren. Der Helper erzwingt das, sodass das nicht der
/// Disziplin des Caller-Codes ueberlassen bleibt.
///
/// `key_holder_max_size` wie in [`compute_key_hash`].
#[must_use]
pub fn keyhash_cdr2_be(
    members: &[(u32, alloc::vec::Vec<u8>)],
    key_holder_max_size: usize,
) -> [u8; KEY_HASH_LEN] {
    let mut sorted: alloc::vec::Vec<&(u32, alloc::vec::Vec<u8>)> = members.iter().collect();
    sorted.sort_by_key(|(id, _)| *id);
    let mut concat: alloc::vec::Vec<u8> = alloc::vec::Vec::new();
    for (_, bytes) in &sorted {
        concat.extend_from_slice(bytes);
    }
    compute_key_hash(&concat, key_holder_max_size)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use alloc::vec;

    // ---- compute_key_hash: Step 5.1 zero-pad ----

    #[test]
    fn small_keyholder_zero_padded_to_16() {
        // Single u32 = 4 byte → max 4 ≤ 16 → zero-pad.
        let bytes = 0x1234_5678u32.to_be_bytes();
        let h = compute_key_hash(&bytes, 4);
        assert_eq!(h[0..4], [0x12, 0x34, 0x56, 0x78]);
        assert_eq!(h[4..16], [0u8; 12]);
    }

    #[test]
    fn exactly_16_byte_keyholder_no_md5() {
        let mut bytes = [0u8; 16];
        for (i, b) in bytes.iter_mut().enumerate() {
            *b = i as u8;
        }
        let h = compute_key_hash(&bytes, 16);
        assert_eq!(h, bytes);
    }

    #[test]
    fn input_shorter_than_max_zero_padded() {
        // max=16 statisch, aber nur 5 byte tatsaechlich
        let h = compute_key_hash(&[1, 2, 3, 4, 5], 16);
        assert_eq!(&h[..5], &[1, 2, 3, 4, 5]);
        assert_eq!(&h[5..], &[0u8; 11]);
    }

    // ---- compute_key_hash: Step 5.2 MD5 ----

    #[test]
    fn large_keyholder_md5_hashed() {
        // max=20 → MD5-Pfad
        let bytes = [0u8; 20];
        let h = compute_key_hash(&bytes, 20);
        // MD5(20 zero bytes) bekannt: 0x441018525208457705bf09a8ee3c1093
        assert_eq!(
            h,
            [
                0x44, 0x10, 0x18, 0x52, 0x52, 0x08, 0x45, 0x77, 0x05, 0xbf, 0x09, 0xa8, 0xee, 0x3c,
                0x10, 0x93
            ]
        );
    }

    #[test]
    fn md5_known_vector_empty_string() {
        // MD5("") = d41d8cd98f00b204e9800998ecf8427e
        let h = compute_key_hash(&[], 17);
        assert_eq!(
            h,
            [
                0xd4, 0x1d, 0x8c, 0xd9, 0x8f, 0x00, 0xb2, 0x04, 0xe9, 0x80, 0x09, 0x98, 0xec, 0xf8,
                0x42, 0x7e
            ]
        );
    }

    #[test]
    fn md5_known_vector_abc() {
        let h = compute_key_hash(b"abc", 17);
        // MD5("abc") = 900150983cd24fb0d6963f7d28e17f72
        assert_eq!(
            h,
            [
                0x90, 0x01, 0x50, 0x98, 0x3c, 0xd2, 0x4f, 0xb0, 0xd6, 0x96, 0x3f, 0x7d, 0x28, 0xe1,
                0x7f, 0x72
            ]
        );
    }

    // ---- PlainCdr2BeKeyHolder ----

    #[test]
    fn keyholder_writes_u32_be() {
        let mut h = PlainCdr2BeKeyHolder::new();
        h.write_u32(0x1234_5678);
        assert_eq!(h.into_bytes(), vec![0x12, 0x34, 0x56, 0x78]);
    }

    #[test]
    fn keyholder_pads_to_4_for_u32_after_u8() {
        let mut h = PlainCdr2BeKeyHolder::new();
        h.write_u8(0xAA);
        h.write_u32(0x1234_5678);
        // u8 (1) + 3 pad + u32 (4) = 8 byte
        assert_eq!(h.into_bytes(), vec![0xAA, 0, 0, 0, 0x12, 0x34, 0x56, 0x78]);
    }

    #[test]
    fn keyholder_u64_aligned_to_4_not_8() {
        let mut h = PlainCdr2BeKeyHolder::new();
        h.write_u8(1);
        h.write_u64(0x1122_3344_5566_7788);
        // PLAIN_CDR2: u64 ist 4-aligned, nicht 8 → 1 pad nach u8 = 3
        // bytes pad, dann 8 byte u64
        assert_eq!(h.len(), 1 + 3 + 8);
    }

    #[test]
    fn keyholder_string_length_prefixed_with_nul() {
        let mut h = PlainCdr2BeKeyHolder::new();
        h.write_string("hi");
        // length=3 BE (0x00000003) + "hi\0"
        assert_eq!(h.into_bytes(), vec![0x00, 0x00, 0x00, 0x03, b'h', b'i', 0]);
    }

    #[test]
    fn keyholder_struct_member_order() {
        // Beispiel-KeyHolder mit @key id:u32 + @key topic:string<8>
        let mut h = PlainCdr2BeKeyHolder::new();
        h.write_u32(42); // id (member_id 0)
        h.write_string("foo"); // topic (member_id 1)
        let bytes = h.into_bytes();
        // u32 4 + length-be(4) + "foo\0" (4) = 12 byte
        assert_eq!(bytes.len(), 12);
        assert_eq!(&bytes[0..4], &[0, 0, 0, 42]);
        assert_eq!(&bytes[4..8], &[0, 0, 0, 4]);
        assert_eq!(&bytes[8..12], &[b'f', b'o', b'o', 0]);
    }

    #[test]
    fn keyholder_full_keyhash_pipeline_zero_pad() {
        // Topic mit @key u32 → max 4 byte → zero-pad
        let mut h = PlainCdr2BeKeyHolder::new();
        h.write_u32(0x1122_3344);
        let h_bytes = h.into_bytes();
        let key = compute_key_hash(&h_bytes, 4);
        assert_eq!(&key[..4], &[0x11, 0x22, 0x33, 0x44]);
        assert_eq!(&key[4..], &[0u8; 12]);
    }

    #[test]
    fn keyholder_full_keyhash_pipeline_md5() {
        // Topic mit @key string (unbounded) → MD5
        let mut h = PlainCdr2BeKeyHolder::new();
        h.write_string("hello-world-this-is-a-long-key-name-exceeding-16");
        let h_bytes = h.into_bytes();
        // max_size > 16 → MD5
        let key = compute_key_hash(&h_bytes, usize::MAX);
        // 16 byte deterministischer Hash, ungleich allen-null
        assert_ne!(key, [0u8; 16]);
    }

    // ---- §7.4.5 / §7.6.8.3.1.b — KeyHash CDR2-BE Member-Sort by memberId ----

    #[test]
    fn keyhash_cdr2_be_member_order_independent() {
        // Drei @key Member mit IDs 7, 3, 5. Encoder-Caller A liefert in
        // [3, 5, 7]; Encoder-Caller B liefert in [7, 3, 5]. Spec-Sort
        // erzwingt KeyHash-Identitaet.
        let m_3 = (3u32, vec![0xAAu8, 0xBB, 0xCC, 0xDD]);
        let m_5 = (5u32, vec![0x11u8, 0x22, 0x33, 0x44]);
        let m_7 = (7u32, vec![0xDEu8, 0xAD, 0xBE, 0xEF]);

        let order_a = vec![m_3.clone(), m_5.clone(), m_7.clone()];
        let order_b = vec![m_7.clone(), m_3.clone(), m_5.clone()];
        let order_c = vec![m_5.clone(), m_7.clone(), m_3.clone()];

        let h_a = keyhash_cdr2_be(&order_a, 12);
        let h_b = keyhash_cdr2_be(&order_b, 12);
        let h_c = keyhash_cdr2_be(&order_c, 12);
        assert_eq!(h_a, h_b);
        assert_eq!(h_a, h_c);
        // sortierte Bytes: 3.value || 5.value || 7.value, dann zero-pad bis 16.
        let expected = {
            let mut v = Vec::new();
            v.extend_from_slice(&m_3.1);
            v.extend_from_slice(&m_5.1);
            v.extend_from_slice(&m_7.1);
            v.resize(16, 0);
            v
        };
        assert_eq!(h_a.as_slice(), expected.as_slice());
    }

    #[test]
    fn keyhash_cdr2_be_md5_path_also_order_independent() {
        // 4 Member mit Strings → max_size > 16 → MD5-Pfad.
        let m_1 = (1u32, b"alpha-content-1".to_vec());
        let m_2 = (2u32, b"beta-content-22".to_vec());
        let m_3 = (3u32, b"gamma-content-333".to_vec());
        let m_4 = (4u32, b"delta-content-4444".to_vec());

        let order_a = vec![m_1.clone(), m_2.clone(), m_3.clone(), m_4.clone()];
        let order_b = vec![m_4.clone(), m_3.clone(), m_2.clone(), m_1.clone()];
        let h_a = keyhash_cdr2_be(&order_a, 1024);
        let h_b = keyhash_cdr2_be(&order_b, 1024);
        assert_eq!(h_a, h_b);
        assert_ne!(h_a, [0u8; 16]);
    }

    #[test]
    fn keyhash_cdr2_be_single_member_matches_compute_key_hash() {
        let m = (42u32, vec![0xAB, 0xCD, 0xEF, 0x12]);
        let h = keyhash_cdr2_be(&[m.clone()], 4);
        let expected = compute_key_hash(&m.1, 4);
        assert_eq!(h, expected);
    }

    #[test]
    fn keyhash_cdr2_be_empty_member_set_yields_zero_padding() {
        let h = keyhash_cdr2_be(&[], 4);
        assert_eq!(h, [0u8; 16]);
    }

    #[test]
    fn keyhash_cdr2_be_member_id_zero_sorts_first() {
        // member_id=0 ist gueltig (Spec §7.2.2.4.1.4) und muss vor
        // hoeheren IDs serialisiert werden.
        let m_0 = (0u32, vec![0xFFu8, 0xEE, 0xDD, 0xCC]);
        let m_99 = (99u32, vec![0x00u8, 0x11, 0x22, 0x33]);

        let h_natural = keyhash_cdr2_be(&[m_0.clone(), m_99.clone()], 8);
        let h_swapped = keyhash_cdr2_be(&[m_99.clone(), m_0.clone()], 8);
        assert_eq!(h_natural, h_swapped);
        // Sortiert: m_0 zuerst → Bytes [0xFF, 0xEE, 0xDD, 0xCC, 0x00, 0x11, 0x22, 0x33].
        let mut expected = [0u8; 16];
        expected[..4].copy_from_slice(&m_0.1);
        expected[4..8].copy_from_slice(&m_99.1);
        assert_eq!(h_natural, expected);
    }

    // ---- Mutation-Killer fuer alle PlainCdr2BeKeyHolder-Methoden ----
    // Direkter Roundtrip jeder write_*-Methode + as_bytes/is_empty/len.

    #[test]
    fn keyholder_is_empty_initially_and_after_write_not_empty() {
        let mut h = PlainCdr2BeKeyHolder::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
        h.write_u8(0x01);
        assert!(!h.is_empty());
        assert_eq!(h.len(), 1);
    }

    #[test]
    fn keyholder_as_bytes_returns_written_content() {
        let mut h = PlainCdr2BeKeyHolder::new();
        h.write_u8(0xDE);
        h.write_u8(0xAD);
        h.write_u8(0xBE);
        h.write_u8(0xEF);
        assert_eq!(h.as_bytes(), &[0xDE, 0xAD, 0xBE, 0xEF]);
        // as_bytes ist nicht-konsumierend
        assert_eq!(h.as_bytes(), &[0xDE, 0xAD, 0xBE, 0xEF]);
    }

    #[test]
    fn keyholder_write_i8_negative_two_complement() {
        let mut h = PlainCdr2BeKeyHolder::new();
        h.write_i8(-1);
        h.write_i8(-128);
        h.write_i8(127);
        assert_eq!(h.into_bytes(), vec![0xFF, 0x80, 0x7F]);
    }

    #[test]
    fn keyholder_write_u16_be_with_alignment() {
        let mut h = PlainCdr2BeKeyHolder::new();
        h.write_u8(0xAA); // 1 byte
        h.write_u16(0x1234); // pad to 2: 1 pad-byte, then BE
        assert_eq!(h.into_bytes(), vec![0xAA, 0x00, 0x12, 0x34]);
    }

    #[test]
    fn keyholder_write_i16_be_with_alignment() {
        let mut h = PlainCdr2BeKeyHolder::new();
        h.write_u8(0xAA);
        h.write_i16(-1);
        // i16 -1 BE = 0xFFFF
        assert_eq!(h.into_bytes(), vec![0xAA, 0x00, 0xFF, 0xFF]);
    }

    #[test]
    fn keyholder_write_i32_be_with_alignment() {
        let mut h = PlainCdr2BeKeyHolder::new();
        h.write_u8(0xAA);
        h.write_i32(-1);
        // pad to 4 from 1 = 3 pad-bytes; i32 -1 BE = FFFFFFFF
        assert_eq!(h.into_bytes(), vec![0xAA, 0, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF]);
    }

    #[test]
    fn keyholder_write_i64_be_with_4byte_alignment() {
        // PLAIN_CDR2 alignment fuer 64-bit ist 4, nicht 8 (§7.4.2)
        let mut h = PlainCdr2BeKeyHolder::new();
        h.write_u8(0xAA);
        h.write_i64(-1);
        // pad to 4: 3 pad. i64 -1 BE = FFFFFFFFFFFFFFFF
        let mut expected = vec![0xAA, 0, 0, 0];
        expected.extend_from_slice(&[0xFF; 8]);
        assert_eq!(h.into_bytes(), expected);
    }

    #[test]
    fn keyholder_write_f32_be_known_pattern() {
        let mut h = PlainCdr2BeKeyHolder::new();
        h.write_f32(1.0_f32);
        // 1.0_f32 BE = 0x3F800000
        assert_eq!(h.into_bytes(), vec![0x3F, 0x80, 0x00, 0x00]);
    }

    #[test]
    fn keyholder_write_f64_be_known_pattern() {
        let mut h = PlainCdr2BeKeyHolder::new();
        h.write_f64(1.0_f64);
        // 1.0_f64 BE = 0x3FF0000000000000
        assert_eq!(
            h.into_bytes(),
            vec![0x3F, 0xF0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]
        );
    }

    #[test]
    fn keyholder_write_bytes_appends_raw() {
        let mut h = PlainCdr2BeKeyHolder::new();
        h.write_u8(0x01);
        h.write_bytes(&[0xDE, 0xAD, 0xBE, 0xEF]);
        h.write_bytes(&[]);
        h.write_bytes(&[0xFF]);
        assert_eq!(h.into_bytes(), vec![0x01, 0xDE, 0xAD, 0xBE, 0xEF, 0xFF]);
    }
}
