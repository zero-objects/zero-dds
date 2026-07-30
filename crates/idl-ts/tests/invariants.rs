//! Toolchain-free structural invariants for the TypeScript codegen.
//!
//! These run without a `tsc`/`node` toolchain: they assert properties of
//! the emitted *source string* that must hold for any valid IDL input.
//! They guard the classes of defect the cross-backend IDL-construct audit
//! surfaced — in particular **F38** (interface-nested types silently
//! dropped), plus the generic non-compile markers (`TRUE`/`FALSE` literal,
//! `L"` wide-string prefix, generic-array `new Array<…>()`, empty
//! `namespace`), duplicate top-level symbols, and reserved-word identifiers
//! emitted unescaped in a binding position.
//!
//! The heavier "generate + actually compile with the real toolchain" corpus
//! lives in `adversarial_corpus.rs` (gated on `tsc`).

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::uninlined_format_args,
    missing_docs
)]

use zerodds_idl::config::ParserConfig;
use zerodds_idl_ts::generate_ts_source;

fn gen_ts(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default())
        .unwrap_or_else(|e| panic!("parse {src:?}: {e:?}"));
    generate_ts_source(&ast).unwrap_or_else(|e| panic!("gen {src:?}: {e:?}"))
}

/// TypeScript/ECMAScript reserved words that must never appear as an
/// emitted binding identifier (declaration name, namespace segment). Kept
/// in sync with `src/keywords.rs::TS_RESERVED`; the ones below are exactly
/// those that are *also legal OMG-IDL identifiers* (so they can be fed in
/// as IDL names). Escaping must turn each into `<kw>_`.
const RESERVED_IDL_LEGAL: &[&str] = &[
    "class",
    "extends",
    "function",
    "return",
    "throw",
    "catch",
    "delete",
    "instanceof",
    "typeof",
    "new",
    "super",
    "this",
    "debugger",
    "continue",
    "break",
    "do",
    "while",
    "with",
    "var",
    "let",
    "implements",
    "package",
    "protected",
    "static",
    "yield",
    "await",
    "null",
];

fn is_reserved_word(name: &str) -> bool {
    // Superset check against the full ECMAScript reserved set that the
    // emitter escapes (mirrors src/keywords.rs).
    const ALL: &[&str] = &[
        "break",
        "case",
        "catch",
        "class",
        "const",
        "continue",
        "debugger",
        "default",
        "delete",
        "do",
        "else",
        "enum",
        "export",
        "extends",
        "false",
        "finally",
        "for",
        "function",
        "if",
        "import",
        "in",
        "instanceof",
        "new",
        "null",
        "return",
        "super",
        "switch",
        "this",
        "throw",
        "true",
        "try",
        "typeof",
        "var",
        "void",
        "while",
        "with",
        "implements",
        "interface",
        "let",
        "package",
        "private",
        "protected",
        "public",
        "static",
        "yield",
        "await",
    ];
    ALL.contains(&name)
}

/// Extracts the identifier a `export <kind> <ident>` declaration binds.
/// Returns `(kind, ident)` for `interface`/`namespace`/`const`/`function`/
/// `type` declarations, ignoring everything else. Works regardless of the
/// indentation the emitter uses inside a namespace body.
fn export_bindings(ts: &str) -> Vec<(&str, String)> {
    let mut out = Vec::new();
    for raw in ts.lines() {
        let line = raw.trim_start();
        let Some(rest) = line.strip_prefix("export ") else {
            continue;
        };
        for kind in ["interface", "namespace", "const", "function", "type"] {
            if let Some(after) = rest.strip_prefix(kind) {
                let after = after.trim_start();
                // ident := leading run of identifier chars.
                let ident: String = after
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
                    .collect();
                if !ident.is_empty() {
                    out.push((kind, ident));
                }
                break;
            }
        }
    }
    out
}

/// No emitted binding identifier may be a bare reserved word.
#[test]
fn no_unescaped_reserved_binding_identifier() {
    // Feed each reserved word in at every binding position the IDL grammar
    // allows: member type name, struct name, enum name, module name, const
    // name, union branch member.
    for kw in RESERVED_IDL_LEGAL {
        let src = format!(
            "module {kw} {{ \
               struct {kw} {{ long {kw}; }}; \
               enum E_{kw} {{ {kw} }}; \
               const long C_{kw} = 1; \
               union U_{kw} switch (long) {{ case 1: long {kw}; default: long other; }}; \
             }};"
        );
        let ts = gen_ts(&src);
        for (kind, ident) in export_bindings(&ts) {
            assert!(
                !is_reserved_word(&ident),
                "reserved word `{ident}` emitted as bare `export {kind}` binding \
                 for input using keyword `{kw}`:\n{ts}"
            );
        }
    }
}

/// The generic non-compile markers that the cross-backend audit flagged in
/// other emitters must never appear in TypeScript output.
#[test]
fn no_non_compile_markers() {
    let corpus = [
        "struct Point { long x; long y; };",
        "enum Color { RED, GREEN, BLUE };",
        "struct Flags { boolean a; boolean b; };",
        "union U switch (boolean) { case TRUE: long a; case FALSE: double b; };",
        "struct Arr { long grid[3][4]; sequence<long> ids; };",
        "const boolean YES = TRUE; const boolean NO = FALSE;",
        "struct WS { wstring w; wchar c; };",
        "interface Svc { struct Nested { long v; }; long op(); };",
    ];
    for src in corpus {
        let ts = gen_ts(src);
        // C wide-string prefix.
        assert!(
            !ts.contains("L\""),
            "`L\"` prefix in output for {src:?}:\n{ts}"
        );
        // Uppercase boolean literals — TS booleans are `true`/`false`.
        assert!(
            !contains_word(&ts, "TRUE") && !contains_word(&ts, "FALSE"),
            "uppercase TRUE/FALSE literal in output for {src:?}:\n{ts}"
        );
        // Generic array construction anti-pattern (`new Array<T>()` /
        // `new List<T>[n]`).
        assert!(
            !ts.contains("new Array<") && !ts.contains(">[]"),
            "generic-array-new anti-pattern in output for {src:?}:\n{ts}"
        );
        // No empty `namespace X {}` — the F38 fix must never open a
        // namespace it does not fill.
        assert!(
            !has_empty_namespace(&ts),
            "empty `namespace` emitted for {src:?}:\n{ts}"
        );
    }
}

/// F38 — an interface body that declares nested types/consts/exceptions
/// must emit them (previously the `_ => {}` fall-through dropped them).
#[test]
fn interface_nested_types_are_emitted() {
    let ts = gen_ts(
        "module M { interface Sensor { \
         struct Reading { long value; double ts; }; \
         const long MAX = 100; \
         exception Fault { string reason; }; \
         long read(); \
       }; };",
    );

    // The nested struct, const and exception must all survive.
    assert!(
        ts.contains("namespace Sensor"),
        "interface-scoped namespace missing:\n{ts}"
    );
    assert!(
        ts.contains("interface Reading"),
        "nested struct dropped:\n{ts}"
    );
    assert!(ts.contains("MAX"), "nested const dropped:\n{ts}");
    assert!(
        ts.contains("interface Fault"),
        "nested exception dropped:\n{ts}"
    );
    // Reflection identity carries the interface scope.
    assert!(
        ts.contains("\"M::Sensor::Reading\""),
        "nested struct typeName not interface-scoped:\n{ts}"
    );
    // The RPC surface is still there alongside the nested types.
    assert!(
        ts.contains("SensorService"),
        "interface service descriptor missing:\n{ts}"
    );
}

/// An interface with *no* nested types must not open a namespace at all
/// (guards against the empty-namespace regression the fix's `has_nested`
/// gate prevents).
#[test]
fn interface_without_nested_types_opens_no_namespace() {
    let ts = gen_ts("interface Plain { long op(); };");
    assert!(
        !ts.contains("namespace Plain"),
        "namespace opened for a nested-type-free interface:\n{ts}"
    );
    assert!(!has_empty_namespace(&ts), "empty namespace:\n{ts}");
}

/// No duplicate top-level `export const`/`export function` binding in a
/// module-free (flat) spec — those declaration spaces cannot legally be
/// redeclared in TypeScript, so a collision is always a codegen bug.
#[test]
fn no_duplicate_flat_value_bindings() {
    let flat = [
        "struct Point { long x; long y; }; struct Line { Point a; Point b; };",
        "enum Color { RED, GREEN }; struct Wrap { Color c; long n; };",
        "union U switch (long) { case 1: long a; default: double b; }; \
         struct Holder { long n; };",
        "bitset Bs { bitfield<8> lo; }; bitmask Bm { A, B };",
        "interface Svc { struct Nested { long v; }; long op(); };",
    ];
    for src in flat {
        let ts = gen_ts(src);
        for kind in ["const", "function"] {
            let mut seen = std::collections::BTreeSet::new();
            for (k, ident) in export_bindings(&ts) {
                if k != kind {
                    continue;
                }
                assert!(
                    seen.insert(ident.clone()),
                    "duplicate `export {kind} {ident}` for {src:?}:\n{ts}"
                );
            }
        }
    }
}

// --- helpers ---------------------------------------------------------------

/// Whole-word (ASCII-identifier boundary) substring match.
fn contains_word(hay: &str, needle: &str) -> bool {
    let bytes = hay.as_bytes();
    let nb = needle.as_bytes();
    let is_id = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b == b'$';
    let mut i = 0;
    while let Some(pos) = hay[i..].find(needle) {
        let start = i + pos;
        let end = start + nb.len();
        let before_ok = start == 0 || !is_id(bytes[start - 1]);
        let after_ok = end == bytes.len() || !is_id(bytes[end]);
        if before_ok && after_ok {
            return true;
        }
        i = start + 1;
    }
    false
}

/// True if the source contains an `export namespace X { }` with an empty
/// body (only whitespace between the braces).
fn has_empty_namespace(ts: &str) -> bool {
    let mut rest = ts;
    while let Some(pos) = rest.find("namespace ") {
        let after = &rest[pos + "namespace ".len()..];
        if let Some(brace) = after.find('{') {
            let body = &after[brace + 1..];
            let trimmed = body.trim_start();
            if trimmed.starts_with('}') {
                return true;
            }
        }
        rest = &rest[pos + "namespace ".len()..];
    }
    false
}
