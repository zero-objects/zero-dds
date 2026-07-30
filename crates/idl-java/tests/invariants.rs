//! Toolchain-free structural invariants for the IDL→Java codegen.
//!
//! These run without `javac`: they parse IDL, generate Java, and assert on
//! the emitted source text. Each check guards a specific class of defect
//! from the IDL-construct fix campaign (F2/F6/F7/F25/F30/F38) plus the
//! cross-backend structural rules:
//!
//!   - no duplicate top-level symbol (same package + class emitted twice),
//!   - no unescaped Java reserved word emitted as an identifier,
//!   - no known non-compile pattern in the output:
//!       * IDL `TRUE`/`FALSE` boolean literal emitted verbatim (F6),
//!       * `L"…"` / `L'…'` wide-literal prefix emitted verbatim (F7),
//!       * generic array creation `new List<…>[N]` (F25),
//!       * empty `record`/`class`/`enum` body where content is required.
//!
//! The heavier proof (real `javac` compile + wire-size asserts) lives in the
//! gated `adversarial_corpus.rs`.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::uninlined_format_args,
    missing_docs
)]

use zerodds_idl::config::ParserConfig;
use zerodds_idl_java::{JavaFile, JavaGenError, JavaGenOptions, generate_java_files};

fn parse(src: &str) -> zerodds_idl::ast::Specification {
    zerodds_idl::parse(src, &ParserConfig::default()).expect("parse must succeed")
}

fn gen_java(src: &str) -> Vec<JavaFile> {
    generate_java_files(&parse(src), &JavaGenOptions::default()).expect("gen must succeed")
}

/// Removes Java comments and string/char literals, leaving only code tokens.
/// An IDL name legitimately appears inside generated comments (`// case 1 ->
/// synchronized`) and string literals (the DDS type name
/// `"synchronized::synchronized_"`); neither is an identifier, so both must
/// be stripped before scanning for a bare reserved word used as a name.
fn strip_comments_and_strings(src: &str) -> String {
    let b = src.as_bytes();
    let mut out = String::with_capacity(src.len());
    let mut i = 0usize;
    while i < b.len() {
        let c = b[i];
        if c == b'/' && i + 1 < b.len() && b[i + 1] == b'/' {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
        } else if c == b'/' && i + 1 < b.len() && b[i + 1] == b'*' {
            i += 2;
            while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                i += 1;
            }
            i += 2;
        } else if c == b'"' || c == b'\'' {
            let quote = c;
            i += 1;
            while i < b.len() && b[i] != quote {
                if b[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
            i += 1;
            out.push(' ');
        } else {
            out.push(c as char);
            i += 1;
        }
    }
    out
}

/// `true` if `word` occurs in `hay` as a whole token: not preceded or
/// followed by an identifier char (`[A-Za-z0-9_]`). So `synchronized_` and
/// `getSynchronized_` do NOT count as bare `synchronized`.
fn contains_bare_word(hay: &str, word: &str) -> bool {
    let bytes = hay.as_bytes();
    let wb = word.as_bytes();
    let is_ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let mut i = 0usize;
    while let Some(pos) = hay[i..].find(word) {
        let start = i + pos;
        let end = start + wb.len();
        let before_ok = start == 0 || !is_ident(bytes[start - 1]);
        let after_ok = end >= bytes.len() || !is_ident(bytes[end]);
        if before_ok && after_ok {
            return true;
        }
        i = start + 1;
    }
    false
}

// ---------------------------------------------------------------------------
// (a) No duplicate top-level symbol
// ---------------------------------------------------------------------------

#[test]
fn no_duplicate_top_level_symbol() {
    // A broad corpus exercising every emitter path.
    let src = "
        module m {
            enum Color { RED, GREEN, BLUE };
            struct Base { @key long id; };
            struct Child : Base { double v; };
            union U switch (long) { case 1: long a; default: octet b; };
            typedef sequence<long> LongSeq;
            const long K = 7;
            interface I { struct Nested { long x; }; };
        };
        struct Top { m::Base b; sequence<m::Color> cs; };
    ";
    let files = gen_java(src);
    assert!(!files.is_empty(), "corpus must emit files");
    let mut seen = std::collections::HashSet::new();
    for f in &files {
        let key = format!("{}::{}", f.package_path, f.class_name);
        assert!(
            seen.insert(key.clone()),
            "duplicate top-level symbol emitted: {key}"
        );
    }
}

// ---------------------------------------------------------------------------
// (b) No unescaped reserved word as an identifier
// ---------------------------------------------------------------------------

/// Java reserved words that (a) are valid bare IDL identifiers and (b) are
/// never emitted as Java syntax by the codegen for plain aggregates — so any
/// bare occurrence in the output would be an un-sanitized IDL identifier.
// NB: `native` is omitted — it is also an IDL keyword and cannot appear as a
// bare IDL identifier (the parser rejects it). Java `native` sanitization is
// exercised via the escaped-identifier corpus in `adversarial_corpus.rs`.
// NB: `instanceof` is omitted — the union sealed-interface codegen emits it
// as the Java operator (`sample instanceof U.Foo`), so a bare occurrence is
// legitimate. Its sanitization is covered by `adversarial_corpus.rs`.
const SAFE_RESERVED: &[&str] = &[
    "synchronized",
    "volatile",
    "transient",
    "strictfp",
    "goto",
    "assert",
];

#[test]
fn reserved_word_identifiers_are_escaped_everywhere() {
    for kw in SAFE_RESERVED {
        // Each reserved word at: member, struct, enum-value, module,
        // const, union branch member.
        let src = format!(
            "
            module {kw} {{
                struct {kw} {{ long {kw}; }};
                enum E {{ {kw} }};
                const long {kw}_c = 1;
                union U switch (long) {{ case 1: long {kw}; default: octet other; }};
            }};
            "
        );
        let files = gen_java(&src);
        assert!(!files.is_empty(), "{kw}: corpus must emit files");
        for f in &files {
            let code = strip_comments_and_strings(&f.source);
            assert!(
                !contains_bare_word(&code, kw),
                "reserved word `{kw}` emitted unescaped in {}::{}\n{}",
                f.package_path,
                f.class_name,
                f.source
            );
        }
    }
}

// ---------------------------------------------------------------------------
// (c) No known non-compile patterns
// ---------------------------------------------------------------------------

/// Scans every generated source for a substring, returns the offending file.
fn find_pattern<'a>(files: &'a [JavaFile], needle: &str) -> Option<&'a JavaFile> {
    files.iter().find(|f| f.source.contains(needle))
}

#[test]
fn boolean_const_is_lowercased_not_verbatim_true_false() {
    // F6: IDL `TRUE`/`FALSE` must map to Java `true`/`false`.
    let files = gen_java("const boolean YES = TRUE; const boolean NO = FALSE;");
    assert!(
        find_pattern(&files, "TRUE").is_none(),
        "F6: bare TRUE leaked"
    );
    assert!(
        find_pattern(&files, "FALSE").is_none(),
        "F6: bare FALSE leaked"
    );
    assert!(
        files.iter().any(|f| f.source.contains("true")),
        "F6: expected `true` literal"
    );
    assert!(
        files.iter().any(|f| f.source.contains("false")),
        "F6: expected `false` literal"
    );
}

#[test]
fn wide_literals_drop_the_l_prefix() {
    // F7: `L"…"` / `L'…'` are not valid Java — the `L` must be stripped.
    let files = gen_java("const wstring WS = L\"hi\"; const wchar WC = L'x';");
    assert!(
        find_pattern(&files, "L\"").is_none(),
        "F7: wide-string `L\"` prefix leaked"
    );
    assert!(
        find_pattern(&files, "L'").is_none(),
        "F7: wide-char `L'` prefix leaked"
    );
    assert!(
        files.iter().any(|f| f.source.contains("\"hi\"")),
        "F7: expected the string body"
    );
}

#[test]
fn array_of_sequence_has_no_generic_array_creation() {
    // F25: `new java.util.List<Integer>[N]` is a compile error; the emitter
    // must allocate the raw type and cast, guarded by @SuppressWarnings.
    let files = gen_java("struct S { sequence<long> grid[3]; };");
    // No `new <Type><...>[` — generic array creation.
    for f in &files {
        for line in f.source.lines() {
            if let Some(np) = line.find("new ") {
                let tail = &line[np..];
                if let (Some(lt), Some(br)) = (tail.find('<'), tail.find('[')) {
                    assert!(
                        br < lt,
                        "F25: generic array creation in {}::{}: {line}",
                        f.package_path,
                        f.class_name
                    );
                }
            }
        }
    }
    // The decode path must allocate the array (proof the member is wired).
    let ts = files
        .iter()
        .find(|f| f.class_name.ends_with("TypeSupport"))
        .expect("F25: TypeSupport must exist");
    assert!(
        ts.source.contains("@SuppressWarnings(\"unchecked\")"),
        "F25: raw-array allocation must be @SuppressWarnings-guarded"
    );
}

#[test]
fn cross_module_reference_lowercases_package_segments() {
    // F30: `Alpha::T` -> `alpha.T` (module segment is a Java package, which
    // is emitted lowercase); the type name keeps its case.
    let files = gen_java("module Alpha { struct T { long x; }; }; struct U { Alpha::T t; };");
    let u = files
        .iter()
        .find(|f| f.class_name == "U")
        .expect("F30: struct U must be emitted");
    assert!(
        u.source.contains("alpha.T"),
        "F30: expected lowercase package `alpha.T` in U:\n{}",
        u.source
    );
    assert!(
        !contains_bare_word(&strip_comments_and_strings(&u.source), "Alpha"),
        "F30: un-lowercased `Alpha.T` leaked in U:\n{}",
        u.source
    );
    // The referenced type must actually live in package `alpha`.
    assert!(
        files
            .iter()
            .any(|f| f.package_path == "alpha" && f.class_name == "T"),
        "F30: T must be generated in package `alpha`"
    );
}

#[test]
fn interface_nested_types_are_not_dropped() {
    // F38: a type declared in an interface body must be emitted, not
    // silently dropped.
    let files = gen_java("interface Svc { struct Nested { long x; }; enum Kind { A, B }; };");
    assert!(
        files.iter().any(|f| f.class_name == "Nested"),
        "F38: interface-nested struct `Nested` was dropped"
    );
    assert!(
        files.iter().any(|f| f.class_name == "Kind"),
        "F38: interface-nested enum `Kind` was dropped"
    );
    // Nested types land in a sub-package named after the interface.
    assert!(
        files
            .iter()
            .any(|f| f.class_name == "Nested" && f.package_path == "svc"),
        "F38: nested type must live in the interface sub-package `svc`"
    );
}

#[test]
fn long_double_is_rejected_loudly_not_silently_narrowed() {
    // F2: `long double` is IEEE-754 binary128 (16 bytes). Java has no such
    // primitive and the runtime no 16-byte float accessor — emitting
    // `writeFloat64` produced an 8-byte member under a 16-byte length code.
    // The generator must refuse rather than emit wire-incorrect code.
    for src in [
        "struct S { long double d; };",
        "struct S { sequence<long double> ds; };",
        "const long double LD = 1.0;",
        "struct S { long double grid[2]; };",
    ] {
        let r = generate_java_files(&parse(src), &JavaGenOptions::default());
        assert!(
            matches!(r, Err(JavaGenError::UnsupportedConstruct { .. })),
            "F2: long double must be rejected for `{src}`, got {r:?}"
        );
    }
}

#[test]
fn no_empty_class_or_record_body() {
    // A generated aggregate must never be an empty `{}` body (would mean a
    // dropped member set). Structs with members must show their fields.
    let files = gen_java("struct S { long a; double b; };");
    let pojo = files
        .iter()
        .find(|f| f.class_name == "S")
        .expect("POJO must exist");
    assert!(
        !pojo.source.contains("class S {}") && !pojo.source.contains("record S()"),
        "empty aggregate body emitted:\n{}",
        pojo.source
    );
    assert!(
        pojo.source.contains(" a") && pojo.source.contains(" b"),
        "struct members missing from POJO:\n{}",
        pojo.source
    );
}
