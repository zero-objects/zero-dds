// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The `fixed<P,S>` interop goldens, anchored as tests.
//!
//! The ORB octets for `fixed` live in two committed places: the matrix
//! cells (`matrix/{omniorb,jacorb}/cdr-{le,be}/<case>/*.values.bin`) and
//! `tests/interop-results/sources/corba-corpus.tsv`. Until this file,
//! only the big-endian corpus rows were held against the encoder —
//! `fixed_digit_count.rs` — and nothing asserted that the matrix cells
//! and the corpus rows are the same recording. So a matrix re-record
//! could drift from the rows the tests measure and no gate would say so,
//! and the ten omniORB little-endian rows were pinned by no test at all.
//!
//! Three anchors:
//!
//! 1. **The matrix cells are the corpus rows**, octet for octet — thirty
//!    cells: omniORB in both byte orders and JacORB big-endian, over the
//!    ten `fixed` cases. TAO has no cell to anchor (`tao_idl` refuses
//!    the IDL; its runtime's reading is measured in
//!    `crates/ast-layout/tests/fixtures/unspecified_constructs.md` §3),
//!    and JacORB has no little-endian cell
//!    (`org.jacorb.orb.CDROutputStream` has no `setLittleEndian`).
//! 2. **The omniORB little-endian rows are reproduced** under
//!    `Dialect::OmniOrb`, and read back as the case's sample. The BCD
//!    field itself does not swap; what these rows pin is the field *in
//!    situ* — the swapped sequence length before it and the swapped
//!    `long` after it.
//! 3. **The `FixedAll` oracle vectors of
//!    `crates/ast-layout/tests/fixtures/vendor_bytes.md` are
//!    reproduced**, parsed from the fixture rather than quoted, so the
//!    narrative and the encoder cannot drift apart. That run is the only
//!    vendor recording of a `fixed` holding zero (`00000c` from omniORB
//!    against JacORB's `0c`) and of `fixed<2,0>` holding `42` (`042c`
//!    from both), which no corpus case carries.
//!
//! One BCD edge has no vendor octets anywhere in the tree: a value with
//! a **trailing fractional zero** (`1.10` under `fixed<5,2>`). The last
//! test holds it against CORBA 3.3 part 2 §9.3.2.8 alone and says so.
//!
//! On the clause number: in the cached CORBA 3.3 part 2 PDF, the
//! fixed-point CDR clause is **§9.3.2.8** ("Fixed-Point Decimal Type");
//! §9.3.2.7 is "Strings and Wide Strings". The tree cites the fixed
//! clause as §9.3.2.8 throughout, matching that PDF.

#![allow(clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use zerodds_ast_layout::{Dialect, Encoding, Endianness, FixedDigitCount};
use zerodds_wire_probe::canonical::{canonicalise, text};
use zerodds_wire_probe::decode::decode;
use zerodds_wire_probe::encode::{encode, hex, model_from_idl_in_dialect};
use zerodds_wire_probe::value::ProbeRequest;

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

// Every corpus case whose IDL declares a `fixed`, read from the
// directory so a case added later is measured by this file too.
fn fixed_cases() -> Vec<String> {
    let directory = repository_root().join("tests/interop-corpus");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&directory).expect("tests/interop-corpus") {
        let entry = entry.expect("a corpus entry");
        let name = entry.file_name().to_string_lossy().to_string();
        let Ok(text) = std::fs::read_to_string(entry.path().join("type.idl")) else {
            continue;
        };
        if text.contains("fixed<") {
            out.push(name);
        }
    }
    out.sort();
    assert!(
        out.len() >= 10,
        "only {} fixed cases were found — this check must not pass by finding nothing",
        out.len()
    );
    out
}

// The recorded write rows of `corba-corpus.tsv`, keyed by
// (case, system, order). Refusals carry `ERR1:` and are not rows here.
fn recorded() -> BTreeMap<(String, String, String), String> {
    let mut rows = BTreeMap::new();
    let path = repository_root().join("tests/interop-results/sources/corba-corpus.tsv");
    let text = std::fs::read_to_string(path).expect("tests/interop-results/sources");
    for line in text.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 5 || f[3] != "write" {
            continue;
        }
        if f[4].is_empty() || !f[4].bytes().all(|b| b.is_ascii_hexdigit()) {
            continue;
        }
        rows.insert(
            (f[0].to_string(), f[1].to_string(), f[2].to_string()),
            f[4].to_string(),
        );
    }
    rows
}

struct CaseFiles {
    idl: String,
    type_name: String,
    values: serde_json::Value,
}

fn case_files(case: &str) -> CaseFiles {
    let directory = repository_root().join("tests/interop-corpus").join(case);
    let read = |file: &str| {
        std::fs::read_to_string(directory.join(file))
            .unwrap_or_else(|error| panic!("{case}/{file}: {error}"))
    };
    let values = read("values.json");
    CaseFiles {
        idl: read("type.idl"),
        type_name: read("type").trim().to_string(),
        values: serde_json::from_str(&values)
            .unwrap_or_else(|error| panic!("{case}/values.json: {error}")),
    }
}

// Octet-for-octet agreement over everything the peer determined.
//
// `tools/wireprobe/omniorb-template.cpp` writes `0xAA` at the alignment
// fill on purpose — measured over GIOP the ORB puts the previous
// request's octets there, so the fill is not a function of the sample.
// An `aa` where this project wrote `00` is that marker; everything else,
// the lengths included, has to match.
fn agrees_where_the_peer_decided(mine: &str, theirs: &str) -> bool {
    if mine.len() != theirs.len() {
        return false;
    }
    mine.as_bytes()
        .chunks_exact(2)
        .zip(theirs.as_bytes().chunks_exact(2))
        .all(|(a, b)| a == b || (a == b"00" && b == b"aa"))
}

fn root_of(ast: &zerodds_ast::Ast, type_name: &str) -> zerodds_ast_core::Id<zerodds_ast::TypeNode> {
    ast.find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(
        type_name
            .split("::")
            .filter(|segment| !segment.is_empty())
            .map(std::string::ToString::to_string)
            .collect(),
    ))
    .ok()
    .and_then(zerodds_ast::AnyId::as_type)
    .expect("the case's type")
}

fn unhex(text: &str) -> Vec<u8> {
    (0..text.len() / 2)
        .map(|index| {
            u8::from_str_radix(&text[index * 2..index * 2 + 2], 16).expect("a hex payload")
        })
        .collect()
}

// The matrix cells and the corpus rows are the same recording.
//
// Thirty cells: omniORB `cdr-le` and `cdr-be` and JacORB `cdr-be`, over
// the ten `fixed` cases. Each `.values.bin` is required to exist and to
// equal its `corba-corpus.tsv` row octet for octet — the `0xAA` fill
// included, because both places recorded the same serialiser run and a
// difference in the fill would mean they did not. The two refusal
// columns are held to stay refusals: a `.values.bin` appearing under
// `matrix/tao/` or `matrix/jacorb/cdr-le/` would be a measurement this
// file does not compare, and it must fail rather than pass unseen.
#[test]
fn the_matrix_cells_are_the_recorded_rows() {
    let rows = recorded();
    let root = repository_root();
    // The cells live in `zerodds-matrix` since 2026-08-26 and are
    // cloned into `matrix/` beside this repository (see
    // `docs/matrix-repo-split.md`). A checkout without that clone
    // cannot run this comparison — say so and stop, rather than
    // passing with nothing compared: an empty loop below would report
    // green for a file whose whole subject is absent.
    if !root.join("matrix").join("omniorb").is_dir() {
        panic!(
            "matrix/omniorb is not here: this test compares recorded \
             cells and the cells live in the zerodds-matrix repository, \
             cloned into matrix/. See docs/matrix-repo-split.md."
        );
    }
    let mut compared = 0usize;
    for case in fixed_cases() {
        for (orb, order) in [("omniorb", "le"), ("omniorb", "be"), ("jacorb", "be")] {
            let cell = root
                .join("matrix")
                .join(orb)
                .join(format!("cdr-{order}"))
                .join(&case)
                .join(format!("{orb}-cdr-{order}-{case}.values.bin"));
            let bin =
                std::fs::read(&cell).unwrap_or_else(|error| panic!("{}: {error}", cell.display()));
            let key = (case.clone(), orb.to_string(), order.to_string());
            let Some(row) = rows.get(&key) else {
                panic!("{case} {orb} {order}: a matrix cell with no corba-corpus.tsv row");
            };
            assert_eq!(
                hex(&bin),
                *row,
                "{case} {orb} {order}: the matrix cell and the corpus row have drifted apart"
            );
            compared += 1;
        }
        for (orb, order) in [("tao", "le"), ("tao", "be"), ("jacorb", "le")] {
            let cell = root
                .join("matrix")
                .join(orb)
                .join(format!("cdr-{order}"))
                .join(&case)
                .join(format!("{orb}-cdr-{order}-{case}.values.bin"));
            assert!(
                !cell.exists(),
                "{}: octets appeared in a column this file records as a refusal — compare them",
                cell.display()
            );
        }
    }
    assert_eq!(
        compared, 30,
        "ten cases in three recorded columns is thirty cells and {compared} were compared"
    );
}

// The omniORB little-endian rows are reproduced, and read back.
//
// `fixed_digit_count.rs` measures the big-endian rows because that is
// where the two ORBs meet; JacORB writes no little-endian stream, so
// these ten rows were pinned by nothing. The BCD field itself does not
// swap — what a little-endian row adds is the field in situ: the
// swapped sequence length before it, the swapped `long` after it, and
// the alignment fill between. Encode under `Dialect::OmniOrb`, whose
// preset is [`FixedDigitCount::Declared`], compare over the octets the
// ORB decided, and require the ORB's own octets to decode as the case's
// sample.
#[test]
fn the_omniorb_little_endian_rows_are_reproduced_and_read_back() {
    let rows = recorded();
    let mut checked = 0usize;
    for case in fixed_cases() {
        let key = (case.clone(), "omniorb".to_string(), "le".to_string());
        let Some(theirs) = rows.get(&key) else {
            panic!("{case} omniorb le is not in corba-corpus.tsv");
        };
        let files = case_files(&case);
        let ast =
            model_from_idl_in_dialect(&files.idl, &format!("{case}/type.idl"), Dialect::OmniOrb)
                .expect("the case lowers");
        let request = ProbeRequest::in_dialect(
            &files.type_name,
            Encoding::PlainCdr1,
            Endianness::Little,
            Dialect::OmniOrb,
        );
        assert_eq!(
            request.fixed_digits,
            FixedDigitCount::Declared,
            "Dialect::OmniOrb stopped selecting the declared count"
        );
        let mine = hex(&encode(&ast, &request, &files.values)
            .unwrap_or_else(|error| panic!("{case}: {error}")));
        assert!(
            agrees_where_the_peer_decided(&mine, theirs),
            "{case} omniorb le:\n  ours   {mine}\n  theirs {theirs}"
        );
        let expected = text(
            &canonicalise(
                &ast,
                root_of(&ast, &files.type_name),
                &files.values,
                "sample",
            )
            .expect("canonical"),
        );
        let read = decode(&ast, &request, &unhex(theirs))
            .unwrap_or_else(|error| panic!("{case}: omniORB's octets do not decode: {error}"));
        assert_eq!(
            text(&read),
            expected,
            "{case}: omniORB's little-endian octets do not read back as the sample"
        );
        checked += 1;
    }
    assert_eq!(
        checked, 10,
        "ten omniORB little-endian rows and {checked} were measured"
    );
}

// The `FixedAll` vectors of `vendor_bytes.md` §`fixed<P,S>`.
//
// Parsed out of the fixture, not quoted, so the narrative and this test
// cannot drift apart. The struct exercises the edges no corpus case
// reaches with a vendor octet: a `fixed` holding **zero** — where the
// two digit counts part, `00000c` against `0c` — and an even declared
// count whose value fills it exactly (`fixed<2,0>` holding `42`,
// `042c` from both ORBs). The omniORB line is required under
// [`FixedDigitCount::Declared`] and is also the ZeroDDS line of the
// fixture; the JacORB line is required under
// [`FixedDigitCount::ValuePrecision`]; and each is required in **both**
// byte orders, which is the fixture's "the bytes do not swap" as an
// assertion.
#[test]
fn the_fixed_all_oracle_vectors_are_reproduced() {
    let path = repository_root().join("crates/ast-layout/tests/fixtures/vendor_bytes.md");
    let fixture = std::fs::read_to_string(&path).expect("vendor_bytes.md");
    let section = fixture
        .split("### `fixed<P,S>`, measured against two CORBA ORBs")
        .nth(1)
        .and_then(|rest| rest.split("\n### ").next())
        .expect("the fixed section of vendor_bytes.md");
    let vector = |label: &str| -> String {
        section
            .lines()
            .filter(|line| line.starts_with(label))
            .find_map(|line| {
                let octets: String = line
                    .split_whitespace()
                    .skip(1)
                    .take_while(|token| !token.starts_with('('))
                    .collect();
                (!octets.is_empty() && octets.bytes().all(|b| b.is_ascii_hexdigit()))
                    .then_some(octets)
            })
            .unwrap_or_else(|| panic!("no `{label}` octet line in the fixed section"))
    };
    let omniorb = vector("omniORB");
    let jacorb = vector("JacORB");
    assert_eq!(
        omniorb,
        vector("ZeroDDS"),
        "the fixture's ZeroDDS line no longer matches its omniORB line"
    );
    assert_eq!(omniorb.len(), 70, "the omniORB vector is 35 octets");
    assert_eq!(jacorb.len(), 66, "the JacORB vector is 33 octets");

    let idl = "typedef fixed<5,2> D5S2;\n\
               typedef fixed<4,2> D4S2;\n\
               typedef fixed<3,3> D3S3;\n\
               typedef fixed<1,0> D1S0;\n\
               typedef fixed<2,0> D2S0;\n\
               typedef fixed<31,0> D31S0;\n\
               struct FixedAll {\n\
                   octet head; D5S2 pos; D5S2 neg; D5S2 zero; D4S2 even;\n\
                   D3S3 allfrac; D1S0 one; D2S0 two; D31S0 big; octet tail;\n\
               };\n";
    let values = serde_json::json!({
        "head": 119,
        "pos": "123.45",
        "neg": "-123.45",
        "zero": "0.00",
        "even": "12.34",
        "allfrac": "0.123",
        "one": "7",
        "two": "42",
        "big": "1234567890123456789012345678901",
        "tail": 171,
    });
    let ast = model_from_idl_in_dialect(idl, "fixed-all.idl", Dialect::Specification)
        .expect("the oracle IDL lowers");
    for (count, expected) in [
        (FixedDigitCount::Declared, &omniorb),
        (FixedDigitCount::ValuePrecision, &jacorb),
    ] {
        for endianness in [Endianness::Big, Endianness::Little] {
            let mut request = ProbeRequest::in_dialect(
                "FixedAll",
                Encoding::PlainCdr1,
                endianness,
                Dialect::Specification,
            );
            request.fixed_digits = count;
            let bytes = encode(&ast, &request, &values)
                .unwrap_or_else(|error| panic!("{count:?} {endianness:?}: {error}"));
            assert_eq!(
                &hex(&bytes),
                expected,
                "{count:?} {endianness:?}: the oracle vector is not reproduced"
            );
            let read = decode(&ast, &request, &bytes)
                .unwrap_or_else(|error| panic!("{count:?} {endianness:?}: {error}"));
            assert_eq!(
                text(&read),
                text(
                    &canonicalise(&ast, root_of(&ast, "FixedAll"), &values, "sample")
                        .expect("canonical")
                ),
                "{count:?} {endianness:?}: the oracle octets do not read back as the sample"
            );
        }
    }
}

// A value with a trailing fractional zero, against the clause alone.
//
// **No vendor octets exist for this edge.** Every recorded `fixed`
// value — the ten corpus cases, the `FixedAll` oracle run, the
// `ACE_CDR::Fixed` table of `unspecified_constructs.md` §3 — ends in a
// non-zero digit or is zero outright, so nothing measured says how a
// peer treats `1.10` under `fixed<5,2>`. CORBA 3.3 part 2 §9.3.2.8
// does: the value is scaled by 10^S and the digits go out as they are,
// so the trailing zero is a digit like any other — `00110c` under the
// declared count, `110c` under the value's precision, where the
// significant-digit count keeps the trailing zero
// (`BigDecimal("1.10").precision()` is 3). An ORB run over such a value
// would move this test from the clause to a recording; until then it is
// the formula, and this comment is the record that it is.
#[test]
fn a_trailing_fractional_zero_is_a_digit_under_the_clause() {
    let idl = "@final struct Sample { fixed<5,2> amount; octet tail; };";
    let ast = model_from_idl_in_dialect(idl, "trailing-zero.idl", Dialect::Specification)
        .expect("the IDL lowers");
    let values = serde_json::json!({ "amount": "1.10", "tail": 171 });
    for (count, expected) in [
        (FixedDigitCount::Declared, "00110cab"),
        (FixedDigitCount::ValuePrecision, "110cab"),
    ] {
        let mut request = ProbeRequest::in_dialect(
            "Sample",
            Encoding::PlainCdr1,
            Endianness::Big,
            Dialect::Specification,
        );
        request.fixed_digits = count;
        let bytes =
            encode(&ast, &request, &values).unwrap_or_else(|error| panic!("{count:?}: {error}"));
        assert_eq!(hex(&bytes), expected, "{count:?}");
        let read =
            decode(&ast, &request, &bytes).unwrap_or_else(|error| panic!("{count:?}: {error}"));
        assert_eq!(
            text(&read),
            text(
                &canonicalise(&ast, root_of(&ast, "Sample"), &values, "sample").expect("canonical")
            ),
            "{count:?}: the trailing zero does not survive the round trip"
        );
    }
}
