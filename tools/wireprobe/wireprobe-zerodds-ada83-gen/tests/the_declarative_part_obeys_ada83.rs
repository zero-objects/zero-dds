// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

//! Ada 83 orders a declarative part; the generated program has to keep
//! to it.
//!
//! RM83 3.9 splits a `declarative_part` into basic declarative items
//! followed by later declarative items, and a subprogram body is a later
//! one. An object declared below a body is therefore illegal — GNAT says
//! "decl cannot appear after body". Ada 95 dropped the split, so the Ada
//! 2012 generator may write its objects wherever it likes and this
//! column may not.
//!
//! The rule is checked here rather than by compiling, because the
//! machines that run this test set have no GNAT. A compiler run on
//! The Linux test host is the stronger check and this one is the earlier one: it
//! fails in the second it takes to generate a program, not after 17 760
//! cells have been filled with sources that no compiler accepts.

use std::io::Write;

/// A `long` pair — enough shape for a codec with bodies in it.
const IDL: &str = "struct Sample {\n    long a;\n    long b;\n};\n";

/// The sample `IDL` describes.
const VALUES: &str = r#"{"a": -1234567, "b": 2147483647}"#;

/// An `@optional` member between two `long`s: the shape whose decoder
/// reads a parameter header and then compares the reader's `Status`
/// against `Status_Ok`, which is the comparison
/// `runtime_enums_are_never_compared_infix` is about.
const OPTIONAL_IDL: &str = "struct Sample {\n    long a;\n    @optional long b;\n    long c;\n};\n";

/// The sample `OPTIONAL_IDL` describes.
const OPTIONAL_VALUES: &str = r#"{"a": -1234567, "b": 7, "c": 1}"#;

fn generated(label: &str, extra: &[&str]) -> String {
    generated_from(label, IDL, VALUES, extra)
}

fn generated_from(label: &str, idl_text: &str, values: &str, extra: &[&str]) -> String {
    let dir = std::env::temp_dir().join(format!("zd-ada83-decl-{}-{label}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create the working directory");
    let idl = dir.join("type.idl");
    std::fs::File::create(&idl)
        .expect("create the idl")
        .write_all(idl_text.as_bytes())
        .expect("write the idl");
    // The write branch reads this one; the read branch ignores it.
    std::fs::File::create(dir.join("values.jsn"))
        .expect("create the sample")
        .write_all(values.as_bytes())
        .expect("write the sample");

    // `parse` reads a full command line, argv[0] included.
    let mut argv: Vec<String> = vec![
        "wireprobe-zerodds-ada83-gen".into(),
        "--idl".into(),
        idl.display().to_string(),
        "--type".into(),
        "Sample".into(),
        "--out-dir".into(),
        dir.display().to_string(),
    ];
    argv.extend(extra.iter().map(|s| {
        // The placeholders stand for paths inside this test's own
        // directory; the generator bakes them into the program and never
        // opens them here.
        s.replace("{dir}", &dir.display().to_string())
    }));

    let args = zerodds_wireprobe_ada83_gen::parse(argv).expect("the arguments parse");
    let source = zerodds_wireprobe_ada83_gen::source(&args).expect("the program generates");
    let _ = std::fs::remove_dir_all(&dir);
    source
}

/// The outer declarative part: between `procedure Zd83_Probe is` and the
/// line that is exactly `begin`.
///
/// The procedure's own header opens the part rather than sitting inside
/// it, so it is skipped — otherwise it would count as the first body and
/// every declaration below would look like a violation. Of what remains
/// only the outer level counts: the generator indents that level by
/// exactly three spaces and everything inside a body deeper, so a
/// declaration in a function's own declarative part — legal, and common —
/// is not mistaken for one below a body.
fn outer_declarative_part(source: &str) -> Vec<&str> {
    source
        .lines()
        .skip_while(|line| line.trim() != "procedure Zd83_Probe is")
        .skip(1)
        .take_while(|line| line.trim_end() != "begin")
        .filter(|line| line.starts_with("   ") && !line.starts_with("    "))
        .collect()
}

/// Does this line open a subprogram body?
///
/// A body is `procedure X (...) is` or `function X (...) return T is` —
/// the `is` is what separates it from the declaration, which ends in a
/// semicolon.
fn opens_a_body(line: &str) -> bool {
    let t = line.trim();
    (t.starts_with("procedure ") || t.starts_with("function ")) && t.ends_with(" is")
}

/// Does this line declare an object?
///
/// `name : Type;` or `name : constant Type :=` — a colon outside a
/// parameter list. Subprogram declarations end in `;` too but start with
/// `procedure`/`function`, and those are basic declarative items anyway.
fn declares_an_object(line: &str) -> bool {
    let t = line.trim();
    if t.starts_with("procedure ")
        || t.starts_with("function ")
        || t.starts_with("type ")
        || t.starts_with("subtype ")
        || t.starts_with("--")
        || t.starts_with("package ")
        || t.starts_with("pragma ")
    {
        return false;
    }
    match t.split_once(" : ") {
        // A parameter list carries colons too, but inside parentheses.
        Some((name, _)) => !name.contains('(') && !name.is_empty(),
        None => false,
    }
}

fn assert_no_object_below_a_body(source: &str, what: &str) {
    let mut body_at: Option<(usize, String)> = None;
    for (number, line) in outer_declarative_part(source).iter().enumerate() {
        if body_at.is_none() && opens_a_body(line) {
            body_at = Some((number + 1, (*line).trim().to_string()));
            continue;
        }
        if let Some((body_line, ref body)) = body_at {
            assert!(
                !declares_an_object(line),
                "{what}: line {} declares an object below the body opened at line \
                 {body_line}, which Ada 83 rejects (RM83 3.9)\n  body:   {body}\n  \
                 object: {}",
                number + 1,
                line.trim()
            );
        }
    }
    assert!(
        body_at.is_some(),
        "{what}: no subprogram body in the declarative part — the check above \
         proved nothing, so the shape this test assumes has changed"
    );
}

#[test]
fn writing_declares_no_object_below_a_body() {
    let source = generated(
        "write",
        &[
            "--values",
            "{dir}/values.jsn",
            "--bytes-out",
            "{dir}/out.bin",
        ],
    );
    assert_no_object_below_a_body(&source, "the write branch");
}

#[test]
fn reading_declares_no_object_below_a_body() {
    let source = generated(
        "read",
        &["--in", "{dir}/in.bin", "--values-out", "{dir}/values.txt"],
    );
    assert_no_object_below_a_body(&source, "the read branch");
}

/// Every exception handler carries a statement.
///
/// RM83 11.2: a handler is `when choice => sequence_of_statements`, and
/// the sequence may not be empty. Eleven runtime exceptions written as
/// eleven bare `when X =>` lines look like a fallthrough from some other
/// language and compile in none — GNAT answers "statement expected" for
/// each of them. A `when` whose next non-blank line is another `when` is
/// the shape.
#[test]
fn every_handler_carries_a_statement() {
    for (source, what) in [
        (
            generated(
                "handler-write",
                &[
                    "--values",
                    "{dir}/values.jsn",
                    "--bytes-out",
                    "{dir}/out.bin",
                ],
            ),
            "the write branch",
        ),
        (
            generated(
                "handler-read",
                &["--in", "{dir}/in.bin", "--values-out", "{dir}/values.txt"],
            ),
            "the read branch",
        ),
    ] {
        let handlers: Vec<&str> = source
            .lines()
            .skip_while(|line| line.trim() != "exception")
            .skip(1)
            .collect();
        assert!(
            !handlers.is_empty(),
            "{what}: no exception handler at all — nothing was checked here"
        );
        let mut arms = 0;
        for (index, line) in handlers.iter().enumerate() {
            if !line.trim_start().starts_with("when ") {
                continue;
            }
            arms += 1;
            // Where the choices end: the arrow, on this line or a later
            // one when the choices are `|`-joined across lines.
            let mut arrow = index;
            while arrow < handlers.len() && !handlers[arrow].contains("=>") {
                arrow += 1;
            }
            let next = handlers
                .get(arrow + 1)
                .map(|line| line.trim())
                .unwrap_or_default();
            assert!(
                !next.is_empty() && !next.starts_with("when ") && !next.starts_with("end "),
                "{what}: the handler `{}` has no statement under it, which RM83 11.2 \
                 requires — the next line is `{next}`",
                line.trim()
            );
        }
        assert!(
            arms >= 2,
            "{what}: {arms} handler arm(s), expected at least the named one and `others`"
        );
    }
}

/// No value of a `Zd_Wire83` type is compared with an infix operator.
///
/// RM83 8.3 makes `=` on a type directly visible only where the package
/// declaring the type is used, and Ada 83 has no `use type` to make one
/// operator visible on its own. `Zd_Wire83.Reader_Status (r) =
/// Zd_Wire83.Status_Ok` is therefore illegal — GNAT answers "operator
/// for type Status is not directly visible" — and the comparison has to
/// name the operator: `Zd_Wire83."=" (a, b)`, negated with `not` where
/// the walk asked for `/=`. A plain `use Zd_Wire83` would make the
/// infix form legal and is not an option: it would also put `Text`,
/// `Status` and every other runtime name into a scope where an IDL is
/// free to declare its own.
#[test]
fn runtime_enums_are_never_compared_infix() {
    /// The prefix of every enumeration literal `emit::ada83` names out
    /// of the runtime package.
    const LITERALS: [&str; 7] = [
        "Zd_Wire83.Status_",
        "Zd_Wire83.Tc_",
        "Zd_Wire83.Role_",
        "Zd_Wire83.Wstr_",
        "Zd_Wire83.Wchar_",
        "Zd_Wire83.Unit_",
        "Zd_Wire83.Fixed_",
    ];
    let source = generated_from(
        "operator-read",
        OPTIONAL_IDL,
        OPTIONAL_VALUES,
        &["--in", "{dir}/in.bin", "--values-out", "{dir}/values.txt"],
    );
    for line in source.lines() {
        if !LITERALS.iter().any(|literal| line.contains(literal)) {
            continue;
        }
        // `=>` in an aggregate and `:=` in an assignment both carry no
        // space before the `=`, so neither is mistaken for a comparison.
        assert!(
            !line.contains(" = ") && !line.contains(" /= "),
            "a value of a Zd_Wire83 type is compared with an infix operator, which \
             RM83 8.3 does not make visible here:\n  {}",
            line.trim()
        );
    }
    assert!(
        source.contains("Zd_Wire83.\"=\" ("),
        "no named comparison in the program at all — the @optional decoder no longer \
         checks the reader's status, so this test proved nothing"
    );
}

/// No file name reaches the generated program relative.
///
/// RM83 14.1 leaves the interpretation of an external file's name to the
/// implementation, and a relative name is read against the directory the
/// program runs in. The Ada 83 column has no `Ada.Command_Line`, so the
/// name is a literal in the source, and the driver builds and runs that
/// program in a directory of its own making rather than the caller's.
/// A relative literal therefore names a file that is not there, which
/// `Text_IO.Name_Error` (RM83 14.4) reported out of the read direction
/// of every cell.
#[test]
fn every_baked_path_reaches_the_program_absolute() {
    for (source, what, expected) in [
        (
            generated(
                "absolute-write",
                &["--values", "{dir}/values.jsn", "--bytes-out", "out.bin"],
            ),
            "the write branch",
            // The octets, and nothing else.
            1,
        ),
        (
            generated(
                "absolute-read",
                &["--in", "in.bin", "--values-out", "values.txt"],
            ),
            "the read branch",
            // The octets read, and the JSON written back.
            2,
        ),
    ] {
        let mut literals = 0;
        for line in source.lines() {
            let t = line.trim();
            if !t.contains("Open (") && !t.contains("Create (") {
                continue;
            }
            let literal = t
                .split_once('"')
                .and_then(|(_, rest)| rest.split_once('"'))
                .map(|(name, _)| name)
                .unwrap_or_else(|| panic!("{what}: `{t}` opens a file without a name"));
            literals += 1;
            assert!(
                literal.starts_with('/'),
                "{what}: `{t}` names `{literal}`, which is relative to whatever \
                 directory the probe runs in rather than to the caller's"
            );
        }
        assert_eq!(
            literals, expected,
            "{what}: {literals} file(s) opened, expected {expected} — the shape this \
             test assumes has changed"
        );
    }
}

/// Every block the program opens is closed again.
///
/// A `declare` that is never closed, or one closed twice, is how the
/// read branch used to lose `zd_data`: it was declared in a block that
/// ended before the reader that used it. The count does not prove
/// scoping, but an imbalance is always a defect.
#[test]
fn blocks_balance_in_both_branches() {
    for (source, what) in [
        (
            generated(
                "balance-write",
                &[
                    "--values",
                    "{dir}/values.jsn",
                    "--bytes-out",
                    "{dir}/out.bin",
                ],
            ),
            "the write branch",
        ),
        (
            generated(
                "balance-read",
                &["--in", "{dir}/in.bin", "--values-out", "{dir}/values.txt"],
            ),
            "the read branch",
        ),
    ] {
        // The statement part only: from the outer `begin` to the
        // handler. Everything in between is `declare ... begin ... end;`
        // blocks, so `declare` opens and `end;` closes. A body's own
        // `begin` belongs to its `is` and is not a block, which is why
        // the declarative part above is left out of this count.
        let mut depth: i32 = 0;
        let mut lowest = 0;
        for line in source
            .lines()
            .skip_while(|line| line.trim_end() != "begin")
            .skip(1)
            .take_while(|line| line.trim() != "exception")
        {
            let t = line.trim();
            if t == "declare" {
                depth += 1;
            } else if t == "end;" {
                depth -= 1;
                lowest = lowest.min(depth);
            }
        }
        assert!(
            lowest >= 0,
            "{what}: a block is closed that was never opened (depth went to {lowest})"
        );
        assert_eq!(
            depth, 0,
            "{what}: {depth} block(s) left open at the end of the program"
        );
    }
}

/// A `bitmask` and a `bitset` in one type: the two shapes whose codecs
/// the shared walk writes with `&`, `|`, `~` and `>>`.
const BITS_IDL: &str = "@bit_bound(16)\nbitmask Flags {\n    ALPHA,\n    \
                        @position(15) OMEGA\n};\n\nbitset Packed {\n    \
                        bitfield<3> low;\n    bitfield<5> high;\n};\n\n\
                        @final struct Sample {\n    Flags marks;\n    \
                        Packed bits;\n    octet tail;\n};\n";

/// The sample `BITS_IDL` describes.
const BITS_VALUES: &str = r#"{"marks": ["ALPHA"], "bits": {"low": 5, "high": 17}, "tail": 171}"#;

/// A 64-bit `bitmask` with a bit at position 63 and a union over
/// `unsigned long long` labelled above 2**63: the two places a wire
/// value has no `Long_Integer` of its own.
const WIDE_IDL: &str = "@bit_bound(64)\nbitmask Wide {\n    @position(0) LOWEST,\n    \
                        @position(63) HIGHEST\n};\n\n\
                        union Choice switch (unsigned long long) {\n    \
                        case 16045690984833335998: long whole;\n    \
                        case 7: double fraction;\n};\n\n\
                        @final struct Sample {\n    Wide flags;\n    \
                        Choice picked;\n    long tail;\n};\n";

/// The sample `WIDE_IDL` describes.
const WIDE_VALUES: &str = r#"{"flags": ["HIGHEST"], "picked": {"_d": "16045690984833335998", "whole": -1234567}, "tail": -1}"#;

/// A union over an `enum` whose branches name every enumerator.
const ENUM_UNION_IDL: &str = "enum Colour { RED, GREEN, BLUE };\n\n\
                              union Choice switch (Colour) {\n    \
                              case RED: long whole;\n    case GREEN:\n    \
                              case BLUE: double fraction;\n};\n\n\
                              @final struct Sample {\n    Choice picked;\n    \
                              long tail;\n};\n";

/// The sample `ENUM_UNION_IDL` describes.
const ENUM_UNION_VALUES: &str = r#"{"picked": {"_d": "GREEN", "fraction": 3.5}, "tail": -1}"#;

/// A record with no components at all — alone, as a sequence element
/// and as an array element.
const EMPTY_IDL: &str = "@appendable struct E { };\n\n\
                         @appendable struct Sample {\n    sequence<E> items;\n    \
                         E pair[2];\n    long tail;\n};\n";

/// The sample `EMPTY_IDL` describes.
const EMPTY_VALUES: &str = r#"{"items": [{}, {}, {}], "pair": [{}, {}], "tail": -1}"#;

/// Both directions of one IDL, labelled.
fn both_directions(label: &str, idl: &str, values: &str) -> Vec<(String, String)> {
    vec![
        (
            generated_from(
                &format!("{label}-write"),
                idl,
                values,
                &[
                    "--values",
                    "{dir}/values.jsn",
                    "--bytes-out",
                    "{dir}/out.bin",
                ],
            ),
            "the write branch".to_string(),
        ),
        (
            generated_from(
                &format!("{label}-read"),
                idl,
                values,
                &["--in", "{dir}/in.bin", "--values-out", "{dir}/values.txt"],
            ),
            "the read branch".to_string(),
        ),
    ]
}

/// One line with everything that is not code taken out: a `--` comment
/// and every string literal. A message this generator bakes into a
/// `Put_Line` is prose, and prose has the word "or" in it.
fn code_of(line: &str) -> String {
    let mut out = String::new();
    let mut in_string = false;
    for ch in line.chars() {
        if ch == '"' {
            in_string = !in_string;
            continue;
        }
        if !in_string {
            out.push(ch);
        }
    }
    match out.find("--") {
        Some(at) => out[..at].to_string(),
        None => out,
    }
}

/// No `and`, `or`, `xor` or `not` stands on an integer.
///
/// RM83 4.5.1 declares the four for `Boolean` and for a one-dimensional
/// array of `Boolean`, and for nothing else; Ada 83 has neither a
/// modular type nor `Interfaces` to borrow an integer's from. The shared
/// codec walk still asks for `&`, `|`, `~` and `>>` around a bitset and
/// a bitmask, and writing them as the Ada words is what GNAT reported as
/// "invalid operand types for operator And, left operand has type
/// Standard.Long_Integer, right operand has a modular type" in 126 of
/// this column's cells. `emit::ada83` sends all four to `Zd_Wire83`
/// instead.
///
/// The shape checked is the whole absence of the non-short-circuit
/// forms: the walk's own Boolean joins are `&&` and `||`, which this
/// column writes as `and then` and `or else`, so a bare `and` or `or`
/// left in the output is always the integer one.
#[test]
fn no_boolean_operator_stands_on_an_integer() {
    for (source, what) in both_directions("bitops", BITS_IDL, BITS_VALUES)
        .into_iter()
        .chain(both_directions("bitops-wide", WIDE_IDL, WIDE_VALUES))
    {
        for (index, line) in source.lines().enumerate() {
            let code = code_of(line);
            let code = code.trim_start();
            for (word, short_circuit) in [(" and ", " and then "), (" or ", " or else ")] {
                let mut at = 0;
                while let Some(found) = code[at..].find(word) {
                    let found = at + found;
                    assert!(
                        code[found..].starts_with(short_circuit),
                        "{what}, line {}: `{}` joins with `{}`, which RM83 4.5.1 declares \
                         only for Boolean — an integer's mask goes through Zd_Wire83",
                        index + 1,
                        code,
                        word.trim()
                    );
                    at = found + 1;
                }
            }
            assert!(
                !code.contains(" xor "),
                "{what}, line {}: `{code}` uses `xor` on operands RM83 4.5.1 does not \
                 declare it for",
                index + 1
            );
            // `not` before a digit is the mask complement, never a
            // Boolean one: no Boolean literal begins with a digit.
            for found in code.match_indices("not ").map(|(at, _)| at) {
                let rest = code[found + 4..].trim_start();
                assert!(
                    !rest.starts_with(|ch: char| ch.is_ascii_digit()),
                    "{what}, line {}: `{code}` complements a numeral, which is the \
                     integer `not` RM83 4.5.1 does not declare",
                    index + 1
                );
            }
        }
    }
}

/// Every variant part covers its discriminant's whole subtype.
///
/// RM83 3.7.3 requires the choices of a variant part to cover every
/// value of the discriminant's subtype. This column declares no
/// enumeration type — an enumerator is a named `Long_Integer` constant —
/// so a union over an `enum` has a `Long_Integer` discriminant, and
/// naming RED, GREEN and BLUE leaves GNAT reporting "missing case
/// values: -16#8000_0000_0000_0000# .. -1" and "3 ..
/// 16#7FFF_FFFF_FFFF_FFFF#". Only `Boolean` is exhausted by its own
/// labels here; everything else needs an `others` arm.
#[test]
fn every_variant_part_has_an_others_arm() {
    for (source, what) in both_directions("variant", ENUM_UNION_IDL, ENUM_UNION_VALUES)
        .into_iter()
        .chain(both_directions("variant-wide", WIDE_IDL, WIDE_VALUES))
    {
        let mut cases = 0;
        let mut others = 0;
        let mut inside = false;
        for line in source.lines() {
            let code = line.trim();
            if code.starts_with("case ") && code.ends_with(" is") {
                cases += 1;
                inside = true;
            }
            // The exception handler has an `others` arm of its own and
            // is not a variant part; only what stands between `case D
            // is` and `end case;` counts.
            if code == "end case;" {
                inside = false;
            }
            if inside && code == "when others =>" {
                others += 1;
            }
        }
        assert!(
            cases > 0,
            "{what}: no variant part at all — nothing was checked here"
        );
        assert_eq!(
            cases, others,
            "{what}: {cases} variant part(s) and {others} `others` arm(s)"
        );
    }
}

/// A record with no components is named, never written as an aggregate.
///
/// RM83 4.3.1 gives a record aggregate at least one
/// `component_association`, so a record with no components has no
/// aggregate at all: `()` is not one and `(null record)` is an Ada 2005
/// addition (AI95-287). An object of the type needs no initial value
/// (RM83 3.2.1) and has no component to leave uninitialised, so both
/// directions name one instead — the read branch's defaulter returns it,
/// the write branch's filler declares it in a block (RM83 5.6) around
/// the assignment.
///
/// The test that both directions *render* is the `expect` inside
/// `generated_from`: before this shape was written, `empty-struct` and
/// `empty-struct-in-sequence` refused in every one of this column's
/// eighty cells.
#[test]
fn an_empty_record_is_named_and_never_an_empty_aggregate() {
    for (source, what) in both_directions("empty", EMPTY_IDL, EMPTY_VALUES) {
        for (index, line) in source.lines().enumerate() {
            let code = code_of(line);
            assert!(
                !code.contains("null record)"),
                "{what}, line {}: `{code}` writes the Ada 2005 `(null record)` aggregate",
                index + 1
            );
            assert!(
                !code.contains("=> ()") && !code.contains(":= ()"),
                "{what}, line {}: `{code}` writes an aggregate with no \
                 component_association, which RM83 4.3.1 does not admit",
                index + 1
            );
        }
        assert!(
            source.contains("zd83_empty"),
            "{what}: the empty record reached no named object at all — \
             nothing was checked here"
        );
    }
}

/// Every integer literal the program writes is a `Long_Integer`.
///
/// This column holds every integer in `Long_Integer`, which is signed
/// sixty-four bits, so a wire value at or above 2**63 — an `unsigned
/// long long` union label, a `@bit_bound(64)` bitmask's declared set —
/// has no literal of its own and is written as the two's-complement
/// pattern instead. Written as the plain decimal, GNAT answers "value
/// not in range of type Standard.Long_Integer" and, because RM83 3.7.3
/// and 4.3.1 both want a static value there, "choice given in variant
/// part is not static" with "expression raises exception, cannot be
/// static (RM 4.9(34))".
#[test]
fn every_integer_literal_fits_long_integer() {
    for (source, what) in both_directions("literals", WIDE_IDL, WIDE_VALUES) {
        for (index, line) in source.lines().enumerate() {
            let code = code_of(line);
            let code = code.trim_start();
            let bytes = code.as_bytes();
            let mut at = 0;
            while at < bytes.len() {
                if !bytes[at].is_ascii_digit() {
                    at += 1;
                    continue;
                }
                // A digit that continues an identifier or a based
                // literal is not the start of a decimal one.
                let starts = at == 0
                    || !(bytes[at - 1].is_ascii_alphanumeric()
                        || bytes[at - 1] == b'_'
                        || bytes[at - 1] == b'#'
                        || bytes[at - 1] == b'.');
                let mut end = at;
                while end < bytes.len() && (bytes[end].is_ascii_digit() || bytes[end] == b'_') {
                    end += 1;
                }
                // A decimal point, an exponent or a `#` after it makes
                // it a real or a based literal, neither of which this
                // check is about.
                let plain = end >= bytes.len()
                    || !(bytes[end] == b'.' || bytes[end] == b'#' || bytes[end] == b'E');
                if starts && plain {
                    let digits: String =
                        code[at..end].chars().filter(char::is_ascii_digit).collect();
                    // `-9223372036854775808` is `Long_Integer'First`,
                    // one further down than the ceiling above zero.
                    let negated = code[..at].trim_end().ends_with('-');
                    let ceiling = if negated {
                        u128::from(i64::MAX.unsigned_abs()) + 1
                    } else {
                        u128::from(i64::MAX.unsigned_abs())
                    };
                    if let Ok(value) = digits.parse::<u128>() {
                        assert!(
                            value <= ceiling,
                            "{what}, line {}: `{code}` writes {value}, which is past \
                             Long_Integer'Last — a 64-bit wire value travels as its \
                             two's-complement pattern in this column",
                            index + 1
                        );
                    }
                }
                at = end.max(at + 1);
            }
        }
    }
}
