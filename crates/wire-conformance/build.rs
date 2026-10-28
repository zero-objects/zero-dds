// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Generate the types this crate's tests encode.
//!
//! The generator runs at build time and its output is compiled with the
//! rest of the crate. That is the point: a name comparison cannot see a
//! signature that does not match, and nothing else in the tree compiles
//! what the generator writes.

// A build script may panic: a generator that cannot produce its output
// has nothing to hand the compiler, and failing the build is the whole
// of what it can do about it.
#![allow(clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use zerodds_ast::Ast;
use zerodds_ast_core::ScopedName;
use zerodds_ast_layout::key::{Encoding, Endianness, LayoutKey};
use zerodds_lang_rules::adapter::cst::rust_items;
use zerodds_lang_rules::encode::rust_codec;
use zerodds_lang_rules::rule::rust;
use zerodds_lang_rules::{ModelGraph, apply};

/// The IDL this crate is generated from.
const SOURCE: &str = r"
@final struct Point {
    long x;
    double y;
};

@final struct Sensor {
    @key long id;
    long reading;
};

@appendable struct Header {
    long seq;
};

// Two versions of one `@appendable` type, for the half of evolution a
// deployed system meets first: `Older` writes and `Newer` reads.
// XTypes 1.3 §7.2.4.4.8, Table 19: a non-optional member of the reading
// type that the writing type does not have takes its default value.
// Same member indices and same member ids in the shared prefix, which is
// what the clause requires of an appendable pair.
@appendable struct Older {
    long a;
    double b;
};

@appendable struct Newer {
    long a;
    double b;
    short c;
    string d;
};

@final struct Cell {
    long a;
    long b;
};

// The one member kind whose write can refuse. `char` is eight bits and
// `wchar` sixteen (DDS-XTypes 1.3 §7.2.2.2.1.2.2 and §7.2.2.2.1.2.5), so
// a code point that does not fit has no encoding — and until the runtime
// said so it went out narrowed. The key member puts the same question to
// the generated `key_hash`.
@final struct Label {
    @key char tag;
    wchar wide;
};

// The two XCDR1 parameter-header shapes, and the one member layout that
// tells a restored alignment origin from a stale one. `flag`'s parameter
// value starts at payload offset 4, so a decoder that leaves the origin
// there aligns `after` to 12 where the encoder wrote it at 8.
// XTypes 1.3 §7.4.1.2, rule (19).
@final struct Opt {
    @optional octet flag;
    double after;
};

// Rule (20) and §7.4.3.5.2, the other half of the same question: under
// encoding version 2 an `@optional` member outside a `@mutable` type is
// a BOOLEAN `<is_present>` and then the value if it is there. The member
// set is `opt_final_present`/`opt_final_absent`, the two cases the byte
// gate ran against Cyclone, RTI and Fast DDS on 2026-08-09, so the bytes
// this column produces can be compared with theirs directly.
@final struct OptV2 {
    long a;
    @optional octet b;
    long c;
};

// Rule (23): parameters, then PID_SENTINEL. `c` is two bytes behind an
// eight-byte member, so its declared length is what `finish_pid` checks.
@mutable struct Ext {
    @id(1) double d;
    @id(2) short c;
};

// Rule (21), the same list under encoding version 2. Three members so a
// reordered stream is not just a swap, and a `@must_understand` one so
// §7.2.2.4.4.4.6 has a member to apply to. `@optional` is here because
// inside a parameter list absence is the only marker there is.
@mutable struct ExtV2 {
    @id(1) long a;
    @id(2) short b;
    @id(3) @optional octet c;
};

// The byte gate's own `mut_mustunderstand`, so this column can be handed
// a stream three foreign implementations wrote rather than one it wrote
// itself.
@mutable struct MustU {
    @id(1) @must_understand long a;
    @id(2) long b;
};

// M1: the tc-gate's `mut_reordered` reader, so this column can be
// handed a member list four foreign implementations wrote in an order
// it does not declare. The ids are written out, which is what lets a
// second IDL declare the same three members in another order.
@mutable struct Reordered {
    @id(1) long a;
    @id(2) short b;
    @id(3) long c;
};

// The three shapes `docs/refused-constructs.md` measured under the
// heading `a collection directly inside a collection`. Cyclone and
// Fast DDS are
// byte-identical to this project's reflective probe on all three, and
// the generated column refused to emit any of them.
typedef long Row[3];

@final struct NestedSeq { sequence<sequence<long> > rows; octet tail; };
@final struct SeqArray  { sequence<long> rows[2];        octet tail; };
@final struct ArraySeq  { sequence<Row> rows;            octet tail; };

// `try_construct`, §7.2.2.7 and §7.3.1.2.1.12. One reader type per
// behaviour, each narrower than the writer that produced the payload,
// which is the only shape the question has: the bound is not on the
// wire, both sides serialise the same bytes, and the whole difference is
// what the receiving side decides. The writer is
// `@mutable struct S { long a; string<32> m; long b; }` with
// `a = 7`, `m` twelve characters long and `b = 9`; the payload is in
// the test.
@mutable struct TcDiscard    { long a;                              string<5> m; long b; };
@mutable struct TcUseDefault { long a; @try_construct(USE_DEFAULT)  string<5> m; long b; };
@mutable struct TcTrim       { long a; @try_construct(TRIM)         string<5> m; long b; };
@mutable struct TcBare       { long a; @try_construct               string<5> m; long b; };

// The same four questions of a sequence bound. Table 17's
// `O2.length <= T1.length`, and §7.3.1.2.1.12.3 for the TRIM case.
@mutable struct TcSeqDiscard    { long a;                             sequence<long, 4> m; long b; };
@mutable struct TcSeqUseDefault { long a; @try_construct(USE_DEFAULT) sequence<long, 4> m; long b; };
@mutable struct TcSeqTrim       { long a; @try_construct(TRIM)        sequence<long, 4> m; long b; };

// The element position, §7.3.1.2.1.12.4. The sequence's own length
// overflow and an element's bound overflow are two different failures,
// which is the split OpenDDS's generated element loop implements.
typedef string<5> String5;
@appendable struct TcElemTrim { long a; sequence<@try_construct(TRIM) String5, 10> m; long b; };

// `map<K, V>`, §7.4.1.1.4 and rules (14)/(15) of §7.4.3.5.3.
//
// `MapKeyString` is the corpus case `tests/interop-corpus/map-key-string`
// member for member, so the bytes this column writes can be held
// against the Fast DDS row recorded for it in
// `tests/interop-results/sources/vendor-corpus.tsv` rather than against
// a second sequence this project wrote itself.
@final struct MapKeyString { map<string, long> entries; long tail; };

// The delimiter, both ways round. Rule (14) exempts a map only when
// **both** halves are primitive, which is `MapDelimiter::EitherHalf` —
// the reading `Dialect::Specification` takes — so `plain` carries no
// DHEADER under XCDR2 and `keyed` does.
@final struct MapDelimit { map<long, long> plain; map<string, long> keyed; long tail; };

// A `map<K, V, N>`: the bound is in entries, and the wire carries none
// to compare it with, so an over-long value is refused rather than
// written short.
@final struct MapBounded { map<long, long, 2> pairs; long tail; };

// Table 11 names `map` among the forms a TRIM may be written on, so the
// three behaviours get a map member each, exactly as the string and
// sequence sets above do.
@mutable struct TcMapDiscard    { long a;                             map<long, long, 2> m; long b; };
@mutable struct TcMapUseDefault { long a; @try_construct(USE_DEFAULT) map<long, long, 2> m; long b; };
@mutable struct TcMapTrim       { long a; @try_construct(TRIM)        map<long, long, 2> m; long b; };

// `wstring`, §7.4.1.1.2. Both types are the corpus cases
// `tests/interop-corpus/wstring-unbounded-utf8` and
// `wstring-bounded-utf8` member for member, so the octets this column
// writes can be held against the rows recorded for them in
// `tests/interop-results/sources/vendor-corpus.tsv` rather than against
// a second sequence this project wrote itself.
//
// The default dialect is `WstringLengthPolicy::SPECIFICATION` — an
// octet count, no terminator, two-octet units — which is the cell
// Cyclone, OpenDDS, RTI's XCDR2 and JacORB write. The seven other cells
// produce different octets from the same IDL and are held against their
// own vendors in `crates/wire/tests/wstring.rs`, at the runtime, because
// reaching them from here would mean a second key per type and the cell
// is not what this crate's build script varies.
@final struct WstrUtf8 { wstring text; long tail; };
@final struct WstrBounded { wstring<16> text; long tail; };

// A `bitmask`, §7.4.1.1.3.2 and Table 33: the holder is the one the
// `@bit_bound` names, which is the whole of the construct's wire form.
// One type per holder width, and `B12` for the bound that is not a
// holder width — twelve bits in two octets, four of which no bit of the
// type can reach.
//
// The bits sit at position 0 and at the top position the bound allows,
// so a holder one width too wide or too narrow moves an octet: `B16`'s
// value is `0x8001` and `B8`'s is `0x81`.
@bit_bound(8)  bitmask B8  { @position(0) B8_LOW,  @position(7)  B8_HIGH };
@bit_bound(16) bitmask B16 { @position(0) B16_LOW, @position(15) B16_HIGH };
@bit_bound(32) bitmask B32 { @position(0) B32_LOW, @position(31) B32_HIGH };
@bit_bound(64) bitmask B64 { @position(0) B64_LOW, @position(63) B64_HIGH };
@bit_bound(12) bitmask B12 { @position(0) B12_LOW, @position(11) B12_HIGH };

@final struct Masks { B8 a; B16 b; B32 c; B64 d; };

// The same construct at the element position, and beside a member whose
// bound is not a holder width, so the alignment the holder brings is
// visible in the octets after it.
@final struct MaskSeq { sequence<B16> flags; B12 narrow; long tail; };

// A `bitset`. XTypes 1.3 has no serialisation rule for one; the reading
// implemented is the only one under which an IDL bitset round-trips
// through the TypeObject of §7.3.4.6 — positions run cumulatively from
// zero in declaration order, **anonymous padding included**, which is
// what the middle field here is for: `pb` sits at position 7 and not at
// position 3.
bitset Packed { bitfield<3> pa; bitfield<4>; bitfield<5> pb; };

@final struct Packet { Packed head; long tail; };

// A nested aggregate and a bounded string, so the two calls the runtime
// never had — `write_nested` and `write_string(v, bound)` — are
// compiled rather than only compared as text.
@final struct Grid {
    sequence<Cell> cells;
    Cell pair[2];
    sequence<long> ids;
    string<8> label;
};

// An enumeration as a member and as a sequence element. Both go through
// the enumeration's own codec, as its holder (XTypes 1.3 §7.4.3.5.3,
// rule (5)); a `write_u32` straight from the enum field would not
// compile, and this type is here so that a compiler and not a text
// comparison says so.
enum Shade { DARK, LIGHT, BRIGHT };
@final struct Painted { Shade s; sequence<Shade> trail; octet tail; };
";

/// Which encoding each type is generated for.
const TYPES: &[(&str, Encoding)] = &[
    ("Point", Encoding::PlainCdr2),
    ("Sensor", Encoding::PlainCdr2),
    ("Header", Encoding::DelimitedCdr),
    ("Older", Encoding::DelimitedCdr),
    ("Newer", Encoding::DelimitedCdr),
    ("Cell", Encoding::PlainCdr2),
    ("Label", Encoding::PlainCdr2),
    ("Opt", Encoding::PlainCdr1),
    ("OptV2", Encoding::PlainCdr2),
    ("Ext", Encoding::PlCdr1),
    ("ExtV2", Encoding::PlCdr2),
    ("MustU", Encoding::PlCdr2),
    ("Reordered", Encoding::PlCdr2),
    ("NestedSeq", Encoding::PlainCdr2),
    ("SeqArray", Encoding::PlainCdr2),
    ("ArraySeq", Encoding::PlainCdr2),
    ("Grid", Encoding::PlainCdr2),
    // Version 1, because the recorded rows for the two `wstring` cases
    // are `xcdr1`; a `@final` struct is framed identically under both,
    // and the `xcdr2` rows in the corpus confirm it byte for byte.
    ("WstrUtf8", Encoding::PlainCdr1),
    ("WstrBounded", Encoding::PlainCdr1),
    // Under XCDR1 first: the recorded Fast DDS row for `map-key-string`
    // is `xcdr1 be`, and version 1 delimits nothing, so the comparison
    // is against the entry count and the pairs with no framing in
    // between. `MapDelimit` takes version 2, where the delimiter is the
    // question.
    ("MapKeyString", Encoding::PlainCdr1),
    ("MapDelimit", Encoding::PlainCdr2),
    ("MapBounded", Encoding::PlainCdr2),
    ("TcMapDiscard", Encoding::PlCdr2),
    ("TcMapUseDefault", Encoding::PlCdr2),
    ("TcMapTrim", Encoding::PlCdr2),
    ("TcDiscard", Encoding::PlCdr2),
    ("TcUseDefault", Encoding::PlCdr2),
    ("TcTrim", Encoding::PlCdr2),
    ("TcBare", Encoding::PlCdr2),
    ("TcSeqDiscard", Encoding::PlCdr2),
    ("TcSeqUseDefault", Encoding::PlCdr2),
    ("TcSeqTrim", Encoding::PlCdr2),
    ("TcElemTrim", Encoding::DelimitedCdr),
    ("Masks", Encoding::PlainCdr2),
    ("MaskSeq", Encoding::PlainCdr2),
    ("Packed", Encoding::PlainCdr2),
    ("Packet", Encoding::PlainCdr2),
    ("Shade", Encoding::PlainCdr2),
    ("Painted", Encoding::PlainCdr2),
];

fn main() {
    println!("cargo::rerun-if-changed=build.rs");

    let parsed = zerodds_idl_parse::parse(SOURCE);
    assert!(
        parsed.diagnostics().is_empty(),
        "the source does not parse: {:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "conformance.idl");
    assert!(
        lowered.diagnostics.is_empty(),
        "the source does not lower: {:?}",
        lowered.diagnostics
    );
    let ast: Ast = lowered.ast;

    // The declarations, through the rule catalogue.
    let model = ModelGraph::of(&ast);
    let applied = apply(&model, &rust::catalogue()).expect("apply the catalogue");
    let mut items = rust_items(&ast, &applied).expect("materialise");

    // The codecs, from the layout plan. `LayoutKey::data` is
    // `Dialect::default` — the clause-by-clause reading — which is what
    // a crate named for conformance should compile: the bytes the
    // generator puts on the wire when nobody names a peer. A dialect's
    // own bytes are `crates/wire-probe/tests/vendor_bytes.rs`'s
    // business, and `tests/bytes.rs` names the framing this plan gives
    // rather than assuming it.
    for (name, encoding) in TYPES {
        let id = ast
            .find_by_scoped_name(&ScopedName::absolute(vec![(*name).to_string()]))
            .ok()
            .and_then(zerodds_ast::AnyId::as_type)
            .unwrap_or_else(|| panic!("{name} is not in the model"));
        items.extend(
            rust_codec(&ast, id, LayoutKey::data(*encoding, Endianness::Little))
                .unwrap_or_else(|error| panic!("{name}: {error}")),
        );
    }

    let file = zerodds_lang_cst::rust::File {
        inner_attrs: Vec::new(),
        doc: zerodds_lang_cst::common::Doc::lines(&[
            "Generated by zerodds-lang-rules from conformance.idl.",
            "",
            "Do not edit: the point of this file is that nobody did.",
        ]),
        uses: Vec::new(),
        items,
    };

    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR")).join("generated.rs");
    std::fs::write(&out, zerodds_lang_render::rust::file(&file)).expect("write the generated file");
}
