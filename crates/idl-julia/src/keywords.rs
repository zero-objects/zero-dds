// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Julia reserved words and escaping logic.
//!
//! Julia has no raw-identifier/backtick escape syntax for keywords.
//! The established community convention on collision is a trailing
//! underscore, e.g. `struct` -> `struct_`. Safe for the wire format:
//! XCDR encodes members positionally, so renaming an identifier for
//! keyword-collision has no effect on wire bytes.
//!
//! Two distinct classes of names must be escaped:
//!
//!  1. **Keywords** ([`JULIA_RESERVED`]) — the parser rejects them outright.
//!  2. **Auto-imported constant bindings** ([`JULIA_BUILTINS`]) — Julia
//!     implicitly `using Core, Base` into every module, so names such as `Base`,
//!     `Type`, `Int`, `Vector` or `String` are already bound as constants in
//!     `Main`. Emitting `const Base = …` / `struct Base …` for an IDL type named
//!     `Base` fails with `ERROR: invalid redefinition of constant Main.Base`.
//!
//! The same trailing-`_` convention covers both classes. It is applied at the
//! single [`escape_julia_ident`] choke point, which every backend name path
//! (definition *and* reference) routes through, so a mangled definition and its
//! references stay consistent. Escaping runs on the already-flattened name
//! ([`crate::emitter`] flattens `Outer::Base` to `Outer_Base` first), so only
//! an unqualified top-level name can ever match a builtin — a nested `Outer_Base`
//! is not a `Main` binding and is left untouched.
//!
//! Injectivity: the scheme is the pre-existing trailing-`_` convention. A raw
//! IDL identifier that literally ends in `_` and whose stem is a reserved name
//! (e.g. `Base_` vs escaped `Base`) is the one theoretical pre-image clash, the
//! same tradeoff the keyword handling already carries; such names are not
//! observed in the corpus.

/// Julia's reserved keywords (fixed, case-sensitive).
pub(crate) const JULIA_RESERVED: &[&str] = &[
    "baremodule",
    "begin",
    "break",
    "catch",
    "const",
    "continue",
    "do",
    "else",
    "elseif",
    "end",
    "export",
    "false",
    "finally",
    "for",
    "function",
    "global",
    "if",
    "import",
    "let",
    "local",
    "macro",
    "module",
    "quote",
    "return",
    "struct",
    "true",
    "try",
    "using",
    "while",
];

/// Names that Julia auto-imports into every module as constant bindings via the
/// implicit `using Core, Base`. Redefining any of them with `const`/`struct`
/// raises `ERROR: invalid redefinition of constant Main.<name>`, so an IDL
/// identifier flattening to exactly one of these must be mangled like a keyword.
///
/// The set covers the three implicit module objects (`Base`/`Core`/`Main`), the
/// abstract/concrete types exported from `Core` and `Base`, and the constant
/// singletons. Source: Julia manual, "Base" reference and "Modules" (implicit
/// `using Core, Base`); each entry satisfies `isdefined(Main, Symbol(name))` in
/// a fresh Julia 1.x session. Only names that are *constant* bindings are listed
/// (a mutable/global `Main` name would not trigger the redefinition error).
pub(crate) const JULIA_BUILTINS: &[&str] = &[
    // implicit module objects
    "Base",
    "Core",
    "Main",
    // top-level abstract types / meta
    "Any",
    "Type",
    "DataType",
    "Union",
    "UnionAll",
    "Nothing",
    "Missing",
    "Function",
    "Module",
    "Method",
    "Task",
    "Enum",
    "Exception",
    "IO",
    "Ref",
    "Ptr",
    "Number",
    "Real",
    "AbstractFloat",
    "Integer",
    "Signed",
    "Unsigned",
    "AbstractString",
    "AbstractChar",
    "AbstractArray",
    "AbstractVector",
    "AbstractMatrix",
    "AbstractDict",
    "AbstractSet",
    // concrete numeric types
    "Bool",
    "Int",
    "UInt",
    "Int8",
    "Int16",
    "Int32",
    "Int64",
    "Int128",
    "UInt8",
    "UInt16",
    "UInt32",
    "UInt64",
    "UInt128",
    "Float16",
    "Float32",
    "Float64",
    "Complex",
    "Rational",
    "BigInt",
    "BigFloat",
    // character / string types
    "Char",
    "String",
    "Symbol",
    "SubString",
    // container / misc concrete types
    "Array",
    "Vector",
    "Matrix",
    "Dict",
    "IdDict",
    "Set",
    "Tuple",
    "NamedTuple",
    "Pair",
    "Cmd",
    "Regex",
    "Channel",
    "BitArray",
    "BitVector",
    "BitSet",
    // constant singletons / globals
    "nothing",
    "missing",
    "pi",
    "im",
    "Inf",
    "NaN",
    "ARGS",
    "ENV",
    "VERSION",
    "C_NULL",
    "stdin",
    "stdout",
    "stderr",
    "undef",
];

/// Checks whether an identifier is a Julia reserved keyword.
#[must_use]
pub fn is_reserved(name: &str) -> bool {
    JULIA_RESERVED.contains(&name)
}

/// Checks whether an identifier collides with an auto-imported `Main` constant
/// binding (see [`JULIA_BUILTINS`]).
#[must_use]
pub fn is_builtin(name: &str) -> bool {
    JULIA_BUILTINS.contains(&name)
}

/// Returns an identifier that is guaranteed usable in Julia source:
/// unchanged if collision-free, otherwise suffixed with `_`. Covers both Julia
/// keywords and auto-imported builtin constant bindings.
#[must_use]
pub fn escape_julia_ident(name: &str) -> String {
    if is_reserved(name) || is_builtin(name) {
        format!("{name}_")
    } else {
        name.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn struct_is_reserved() {
        assert!(is_reserved("struct"));
        assert!(is_reserved("function"));
        assert!(is_reserved("end"));
    }

    #[test]
    fn foo_is_not_reserved() {
        assert!(!is_reserved("Foo"));
        assert!(!is_reserved("my_field"));
    }

    #[test]
    fn escape_struct_yields_trailing_underscore() {
        assert_eq!(escape_julia_ident("struct"), "struct_");
    }

    #[test]
    fn escape_non_keyword_unchanged() {
        assert_eq!(escape_julia_ident("Foo"), "Foo");
    }

    #[test]
    fn all_reserved_escape_with_trailing_underscore() {
        for kw in JULIA_RESERVED {
            assert_eq!(escape_julia_ident(kw), format!("{kw}_"));
        }
    }

    #[test]
    fn base_is_builtin_and_mangled() {
        assert!(is_builtin("Base"));
        assert!(!is_reserved("Base"));
        assert_eq!(escape_julia_ident("Base"), "Base_");
    }

    #[test]
    fn all_builtins_escape_with_trailing_underscore() {
        for b in JULIA_BUILTINS {
            assert_eq!(escape_julia_ident(b), format!("{b}_"));
        }
    }

    #[test]
    fn non_builtin_type_name_unchanged() {
        assert!(!is_builtin("Pose"));
        assert_eq!(escape_julia_ident("Pose"), "Pose");
        // flattened nested name never matches a bare builtin
        assert_eq!(escape_julia_ident("Outer_Base"), "Outer_Base");
    }
}
