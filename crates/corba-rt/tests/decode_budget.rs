// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! CORBA 3.3 Part 2 §9.3.5, §9.3.5.1: an `any` and a `TypeCode` from the
//! wire decode within memory proportional to their octets.
//!
//! Every count of a sequence or an array is checked against the octets
//! that remain, but a value of `tk_null`, `tk_void` or an empty struct
//! takes none: nested sequences and arrays of such values multiply their
//! counts level by level. A `TypeCode` indirection to a repeated
//! `TypeCode` is a copy of its target, and a chain of structs that repeat
//! their nested struct doubles the decoded tree at every level. Both are
//! refused before they allocate.
//!
//! A counting global allocator (this test binary only) records the most
//! bytes the measuring thread holds at once while it decodes; the test
//! harness allocates on its own threads, which are not counted.
//!
//! Run: `cargo test -p zerodds-corba-rt --test decode_budget`

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use zerodds_corba_rt::type_code::tckind::{INDIRECTION, TK_LONG, TK_STRUCT};
use zerodds_corba_rt::{
    AnyValue, CorbaAny, MAX_TYPECODE_REREAD_FACTOR, TopLevelTypeCodes, TypeCode,
};
use zerodds_wire::{Endianness, Reader, WireError, Writer};

/// `System`, counting the bytes a thread holds while it is inside
/// [`peak_of`]: the bytes it holds now and the most it held. A realloc is
/// a free of the old block and an allocation of the new one.
struct ThreadCounting;

thread_local! {
    /// `Some((live, peak))` while this thread is counted.
    static COUNT: Cell<Option<(isize, isize)>> = const { Cell::new(None) };
}

fn note(delta: isize) {
    // `try_with`: an allocation during thread teardown, after the slot is
    // gone, is not a measurement.
    let _ = COUNT.try_with(|c| {
        if let Some((live, peak)) = c.get() {
            let live = live + delta;
            c.set(Some((live, peak.max(live))));
        }
    });
}

fn bytes(layout: Layout) -> isize {
    isize::try_from(layout.size()).unwrap_or(isize::MAX)
}

// SAFETY: every call is forwarded to `System` with the caller's arguments;
// `note` only touches a const-initialised thread-local `Cell`, which does not
// allocate.
unsafe impl GlobalAlloc for ThreadCounting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        note(bytes(layout));
        // SAFETY: forwarded unchanged; the caller upholds `alloc`'s contract.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        note(bytes(layout));
        // SAFETY: as in `alloc`.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        note(-bytes(layout));
        // SAFETY: forwarded unchanged; the caller upholds `dealloc`'s contract.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        note(isize::try_from(new_size).unwrap_or(isize::MAX) - bytes(layout));
        // SAFETY: forwarded unchanged; the caller upholds `realloc`'s contract.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: ThreadCounting = ThreadCounting;

/// What `f` returns and the most bytes the calling thread held at once
/// while it ran, beyond what it held before.
fn peak_of<R>(f: impl FnOnce() -> R) -> (R, usize) {
    COUNT.with(|c| c.set(Some((0, 0))));
    let out = f();
    let peak = COUNT.with(|c| c.replace(None)).map_or(0, |(_, peak)| peak);
    (out, usize::try_from(peak).unwrap_or(0))
}

/// What every decode of these tests may hold at most: far below what a
/// decode that materialised the claimed counts would.
const SMALL: usize = 1 << 20;

fn null_array(length: u32) -> TypeCode {
    TypeCode::Array {
        element: Box::new(TypeCode::Null),
        length,
    }
}

fn null_sequence() -> TypeCode {
    TypeCode::Sequence {
        element: Box::new(TypeCode::Null),
        bound: 0,
    }
}

/// `tc` as the `TypeCode` of an `any`, then the octets `value`.
fn any_bytes(tc: &TypeCode, value: &[u8]) -> Vec<u8> {
    let mut w = Writer::new(Endianness::Big);
    tc.encode(&mut w).unwrap();
    w.write_bytes(value);
    w.finish()
}

/// Whether `e` is the refusal of parts without octets beyond the octets
/// read.
fn too_many_empty_parts(e: &WireError) -> bool {
    matches!(e, WireError::OutOfRange { expected, .. }
        if expected.contains("without octets per octet read"))
}

/// CORBA 3.3 Part 2 §9.3: `array<array<null, 1000>, 1000>` is a
/// `TypeCode` of a few dozen octets and a value of none; 1000 octets after
/// it let both lengths pass the check against the octets that remain.
/// Decoded in full it would be a million parts (about 200 MB); the
/// decoder refuses it after one part without octets per octet read,
/// holding little memory.
#[test]
fn corba_p2_9_3_nested_arrays_of_null_are_refused_without_a_large_allocation() {
    let tc = TypeCode::Array {
        element: Box::new(null_array(1000)),
        length: 1000,
    };
    let bytes = any_bytes(&tc, &[0; 1000]);
    let (res, peak) = peak_of(|| CorbaAny::decode(&mut Reader::new(&bytes, Endianness::Big)));
    let err = res.expect_err("refused");
    assert!(too_many_empty_parts(&err), "{err:?}");
    assert!(peak < SMALL, "the decode held {peak} bytes");
}

/// CORBA 3.3 Part 2 §9.3: `sequence<sequence<null>>` with 10 sequences
/// of 100000 elements each: every count fits the octets that remain (the
/// stream is padded), the values take none. Decoded in full it would be
/// a million parts; the decoder refuses it, holding little memory.
#[test]
fn corba_p2_9_3_nested_sequences_of_null_are_refused_without_a_large_allocation() {
    let tc = TypeCode::Sequence {
        element: Box::new(null_sequence()),
        bound: 0,
    };
    let mut value = Writer::new(Endianness::Big);
    value.write_u32(10);
    for _ in 0..10 {
        value.write_u32(100_000);
    }
    value.write_bytes(&[0; 100_000]);
    let bytes = any_bytes(&tc, &value.finish());
    let (res, peak) = peak_of(|| CorbaAny::decode(&mut Reader::new(&bytes, Endianness::Big)));
    let err = res.expect_err("refused");
    assert!(too_many_empty_parts(&err), "{err:?}");
    assert!(peak < SMALL, "the decode held {peak} bytes");
}

/// The bound counts parts without octets against the octets read: an
/// `any` of `tk_null`, and a `sequence<null>` of as many elements as the
/// decode has read octets, still decode.
#[test]
fn corba_p2_9_3_parts_without_octets_up_to_the_octets_read_still_decode() {
    let null = any_bytes(&TypeCode::Null, &[]);
    let mut r = Reader::new(&null, Endianness::Big);
    assert_eq!(CorbaAny::decode(&mut r).unwrap().0, AnyValue::Null);

    // The `TypeCode` (20 octets) and the count (4): 24 octets read
    // before the elements, so up to 25 elements are within.
    let tc = null_sequence();
    for (count, fits) in [(24u32, true), (40, false)] {
        let mut value = Writer::new(Endianness::Big);
        value.write_u32(count);
        value.write_bytes(&[0; 64]);
        let bytes = any_bytes(&tc, &value.finish());
        let res = CorbaAny::decode(&mut Reader::new(&bytes, Endianness::Big));
        match (res, fits) {
            (Ok(CorbaAny(AnyValue::Seq { items, .. })), true) => {
                assert_eq!(items.len(), count as usize);
            }
            (Err(e), false) => assert!(too_many_empty_parts(&e), "{e:?}"),
            (other, _) => panic!("{count} elements: {other:?}"),
        }
    }
}

/// Writes the struct `TypeCode` of level `level` of the doubling chain
/// with its `TCKind` at absolute stream offset `at` (a multiple of 4):
/// level 0 is `struct L0 { long x; }`, level `n` is
/// `struct Ln { L(n-1) a; L(n-1) b; }` whose `b` is an indirection to the
/// `TypeCode` of `a`.
fn chain_level(level: u32, at: usize) -> Vec<u8> {
    // The encapsulation's byte 0 follows the TCKind and the length.
    let body_at = at + 8;
    let mut e = Writer::new(Endianness::Big);
    e.write_u8(0);
    e.write_string(&format!("IDL:L{level}:1.0"));
    e.write_string(&format!("L{level}"));
    if level == 0 {
        e.write_u32(1);
        e.write_string("x");
        e.write_u32(TK_LONG);
    } else {
        e.write_u32(2);
        e.write_string("a");
        e.align_to(4);
        let a_at = body_at + e.position();
        e.write_bytes(&chain_level(level - 1, a_at));
        e.write_string("b");
        e.align_to(4);
        e.write_u32(INDIRECTION);
        let field = body_at + e.position();
        let offset = i64::try_from(a_at).unwrap() - i64::try_from(field).unwrap();
        e.write_i32(i32::try_from(offset).unwrap());
    }
    let body = e.finish();
    let mut w = Writer::new(Endianness::Big);
    w.write_u32(TK_STRUCT);
    w.write_u32(u32::try_from(body.len()).unwrap());
    w.write_bytes(&body);
    w.finish()
}

/// The `TypeCode` the doubling chain of level `level` decodes to.
fn chain_type_code(level: u32) -> TypeCode {
    let members = if level == 0 {
        vec![("x".to_string(), TypeCode::Long)]
    } else {
        let inner = chain_type_code(level - 1);
        vec![("a".to_string(), inner.clone()), ("b".to_string(), inner)]
    };
    TypeCode::Struct {
        repo_id: format!("IDL:L{level}:1.0"),
        name: format!("L{level}"),
        members,
        is_except: false,
    }
}

/// Whether `e` is the refusal of repeated `TypeCode`s beyond the octets
/// of the `TypeCode`.
fn reads_too_much_again(e: &WireError) -> bool {
    matches!(e, WireError::OutOfRange { expected, .. }
        if expected.contains("read at most"))
}

/// CORBA 3.3 Part 2 §9.3.5.1: a repeated `TypeCode` is a copy of its
/// target. A chain of 24 structs, each of which repeats its nested struct,
/// is about 1.4 KB of octets and would decode to 2^24 nodes; the decoder
/// refuses it after the octets the `TypeCode` may read again
/// ([`MAX_TYPECODE_REREAD_FACTOR`] times its own), holding little memory.
/// A short chain decodes to the copies, both members alike.
#[test]
fn corba_p2_9_3_5_1_a_doubling_chain_of_repeated_typecodes_is_refused_quickly() {
    let bytes = chain_level(24, 0);
    assert!(bytes.len() < 2048, "{} octets", bytes.len());
    let (res, peak) = peak_of(|| TypeCode::decode(&mut Reader::new(&bytes, Endianness::Big)));
    let err = res.expect_err("refused");
    assert!(reads_too_much_again(&err), "{err:?}");
    assert!(peak < SMALL, "the decode held {peak} bytes");

    // Through an `any`: the same refusal.
    let mut r = Reader::new(&bytes, Endianness::Big);
    let err = CorbaAny::decode(&mut r).expect_err("refused");
    assert!(reads_too_much_again(&err), "{err:?}");

    // Level 3: 4 copies of L0, each member a full copy.
    let short = chain_level(3, 0);
    let tc = TypeCode::decode(&mut Reader::new(&short, Endianness::Big)).unwrap();
    assert_eq!(tc, chain_type_code(3));
}

/// CORBA 3.3 Part 2 §9.3.5.1.6: a top-level `TypeCode` that is an
/// indirection to an earlier one of the message is a copy of it. The
/// copies together stay within [`MAX_TYPECODE_REREAD_FACTOR`] times the
/// octets of the message read up to them: a struct of 1 KB copied by
/// 8-octet indirections is refused before the copies outgrow that.
#[test]
fn corba_p2_9_3_5_1_6_top_level_copies_stay_within_the_octets_read() {
    let mut w = Writer::new(Endianness::Big);
    let first = TypeCode::Struct {
        repo_id: format!("IDL:{}:1.0", "N".repeat(1000)),
        name: "N".into(),
        members: vec![("x".into(), TypeCode::Long)],
        is_except: false,
    };
    first.encode(&mut w).unwrap();
    let octets = w.position();
    let copies = 1000;
    for _ in 0..copies {
        w.write_u32(INDIRECTION);
        let field = w.position();
        w.write_i32(-i32::try_from(field).unwrap());
    }
    let bytes = w.finish();
    let mut r = Reader::new(&bytes, Endianness::Big);
    let mut tops = TopLevelTypeCodes::new(true);
    assert_eq!(
        TypeCode::decode_top_level(&mut r, &mut tops).unwrap(),
        first
    );
    let mut made = 0usize;
    let err = loop {
        match TypeCode::decode_top_level(&mut r, &mut tops) {
            Ok(tc) => {
                assert_eq!(tc, first);
                made += 1;
            }
            Err(e) => break e,
        }
    };
    assert!(
        matches!(&err, WireError::OutOfRange { expected, .. } if expected.contains("copy at most")),
        "{err:?}"
    );
    // Copies were made, and fewer than the message holds.
    assert!(made > 0 && made < copies, "{made} copies");
    let read = r.position();
    assert!(
        made * octets <= read * MAX_TYPECODE_REREAD_FACTOR,
        "{made} copies of {octets} octets after {read} octets"
    );
}
