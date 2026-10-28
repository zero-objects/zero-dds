// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The identifiers a target language will not let a generator paste
//! through, and what each generator renames them to.
//!
//! IDL and its target languages do not share a reserved word list. CORBA
//! 3.3 part 1 §7.2.3 gives IDL twenty-odd keywords and says nothing
//! about `new`, `static` or `class`; all three are legal member names in
//! an IDL file that every front end on the test host accepts. The
//! generated header is another matter — `long new;` is not a C++
//! declaration — so each vendor's IDL compiler renames the identifier on
//! its way into the target language, and the fill and dump emitters here
//! have to spell it the way the vendor spelled it or the probe does not
//! compile.
//!
//! There is no single rule to apply. The prefix, and the list of names
//! it applies to, is the *generator's*, so each of the three below is a
//! measurement of one generator's output over
//! `tests/interop-corpus/identifier-target-keywords/type.idl` and not a
//! rule derived from a specification:
//!
//! | function | generator | measured output |
//! |---|---|---|
//! | [`cxx`] | omniidl 4.3.3, `tao_idl` 2.5.24 | `_cxx_new`, `_cxx_delete`, `_cxx_operator`, `_cxx_static`, `enum _cxx_class { _cxx_template, _cxx_virtual, _cxx_friend }` |
//! | [`c`] | `rtiddsgen -language C`, Connext 7.7 | `DDS_Long _c_static;`, beside `DDS_Long new;` and `DDS_Double delete;` |
//! | [`java`] | JacORB 3.9 | module `package` becomes the Java package `_package`, `enum class` the class `_class` |
//!
//! All three were taken 2026-08-11 on the Linux test host, from each
//! generator's own output for that corpus case.
//!
//! ## The whole class, and which cell of it is still open
//!
//! Three functions here, seven emitter-and-vendor pairs, and the
//! question is the same for each: does the vendor's generator rename,
//! and does the emitter that writes against its header do the same?
//! Measured 2026-08-11 and 2026-08-12 on the Linux test host over the
//! three `identifier-target-keyword-*` cases.
//!
//! | emitter | vendors | the generator renames | the emitter renames | how the position is reached |
//! |---|---|---|---|---|
//! | `fill.rs`, `dump.rs` | Cyclone | no | no | `wireprobe-batch` wraps the case in `module zd_cN`, so the C name is `zd_c1_static` and no position is reached |
//! | `fill.rs`, `dump.rs` | RTI | `_c_` | yes | the type name and the enumerator, both measured |
//! | `fill_cpp.rs`, `dump_cpp.rs` | Fast DDS | no | no | reached, and `fastddsgen`'s own C++ is `const zd_c5::static& data);`, which no compiler takes — recorded as its refusal |
//! | `corba_cpp.rs` | omniORB, TAO | `_cxx_` | yes | the type name, `_cxx_static` |
//! | `corba_java.rs` | JacORB | `_` | yes | the type name, `_static` |
//! | `fill_ros.rs`, `dump_ros.rs`, `ros_idl.rs` | ros-fastrtps, ros-cyclone | — | nothing to apply | rosidl's own convention gives `zdros__msg__Static`, which is not a keyword in any of its target languages |
//!
//! Six of the seven agreed with their generator. The one that did not
//! was the C pair, and it did not agree with *itself*: `fill.rs` put
//! every identifier through [`c`] and `dump.rs` pasted the IDL
//! spelling, so for RTI the two halves of one translation unit named
//! one enumerator two ways. Both halves do it now.
//!
//! The open cell is the bitmask flag. `rtiddsgen` refuses every bitmask
//! in the corpus at the front end and Cyclone's flag is inside the
//! batch's module, so no C column can be asked; Fast DDS refuses the
//! case; the ORBs have no bitmask. It is recorded as unmeasured in
//! `tests/interop-corpus/identifier-target-keyword-bitmask-flag`.
//!
//! ## Why the three lists are not one list
//!
//! The measurement is the reason. `new` and `delete` are C++ keywords
//! and not C ones, and `rtiddsgen -language C` leaves both verbatim
//! while renaming `static`, which is a keyword in both — so a C emitter
//! that shared the C++ list would rename two members the vendor did not
//! and stop agreeing with the vendor's own header. `template`,
//! `virtual` and `friend` run the other way: renamed by both C++ front
//! ends, left alone by JacORB, because Java has none of the three.
//!
//! ## Two generators that rename nothing
//!
//! Measured the same day and worth writing down, because "no escaping"
//! is a finding and not an omission: Cyclone's `idlc` emits
//! `int32_t new;` and `int32_t static;` verbatim, and `fastddsgen`
//! escapes nothing either. Neither has a function here, and the
//! emitters that serve them — `fill.rs`'s [`Vendor::Cyclone`] arm and
//! `fill_cpp.rs` — paste the IDL identifier through unchanged.
//!
//! [`Vendor::Cyclone`]: crate::fill::Vendor::Cyclone
//!
//! ## Where these belong and where they do not
//!
//! An identifier position in generated target-language source, and
//! nowhere else. The IDL the probe hands the vendor's own front end
//! carries the identifier the corpus wrote — that file is the input the
//! renaming is a *consequence* of — and so do the JSON keys the dump
//! emitters print, the paths in a [`ProbeError`] and every string
//! literal. Renaming a JSON key would make the ZeroDDS column disagree
//! with every vendor column over a member name that is not on the wire
//! at all.
//!
//! [`ProbeError`]: crate::encode::ProbeError

use std::borrow::Cow;

/// The C++ spelling of an IDL identifier, as omniORB and TAO generate
/// it.
///
/// A C++ keyword comes back with `_cxx_` in front of it and everything
/// else comes back untouched. Measured 2026-08-11 from omniidl's and
/// `tao_idl`'s output for
/// `tests/interop-corpus/identifier-target-keywords`: the members `new`,
/// `delete`, `operator` and `static` are generated as `_cxx_new`,
/// `_cxx_delete`, `_cxx_operator` and `_cxx_static`, the enumeration
/// `class` as `enum _cxx_class` with the enumerators `_cxx_template`,
/// `_cxx_virtual` and `_cxx_friend` — and `package`, `type`, `impl`,
/// `lambda`, `def` and `async`, which are keywords in some other
/// language and none in C++, appear verbatim in the same header. Both
/// front ends produce the same spelling, which is what lets one emitter
/// serve both.
///
/// The list is C++23's, plus C's, because a C keyword that C++ dropped
/// would still be one the C++ front end has to rename and no corpus case
/// has yet asked which. The two overlap almost entirely; `restrict` and
/// the `_Atomic` family are the difference.
#[must_use]
pub fn cxx(name: &str) -> Cow<'_, str> {
    if is_listed(CXX_KEYWORDS, name) || is_listed(C_KEYWORDS, name) {
        Cow::Owned(format!("_cxx_{name}"))
    } else {
        Cow::Borrowed(name)
    }
}

/// The C spelling of an IDL identifier, as `rtiddsgen` generates it.
///
/// A C keyword comes back with `_c_` in front of it. Measured
/// 2026-08-11 from `rtiddsgen -language C` (Connext 7.7) over
/// `tests/interop-corpus/identifier-target-keywords`: the generated
/// structure carries `DDS_Long _c_static;` beside `DDS_Long new;` and
/// `DDS_Double delete;`, which is the whole of why this is a separate
/// function from [`cxx`] — `new` and `delete` are C++ keywords, C has
/// neither, and RTI's C generator renames by the C list alone.
///
/// Two more positions were measured the same day, each from its own
/// corpus case, because a member is not the only place an identifier
/// reaches the generated source:
///
/// | position | case | what rtiddsgen generated |
/// |---|---|---|
/// | the type name | `identifier-target-keyword-typename` | `_c_static`, and every symbol derived from it — the compiler named the spelling itself, *did you mean `_c_staticPlugin_get_serialized_sample_max_size`?* |
/// | an enumerator | `identifier-target-keyword-enumerator` | `_c_volatile`; the probe's write side already spelled it that way and compiled |
///
/// The third, a bitmask flag, has no measurement: `rtiddsgen` refuses
/// every bitmask in the corpus at the front end, so
/// `identifier-target-keyword-bitmask-flag` records the position as
/// unmeasured rather than as agreed.
///
/// The list is applied to the **flattened** name and not to each
/// segment of it. `::package::static` is `package_static`, which C does
/// not reserve; only a declaration at the outermost scope keeps a
/// spelling a compiler reads as a keyword.
///
/// The list is C23's. `bool`, `true`, `false` and `static_assert` became
/// keywords in that revision rather than macros, and a generator built
/// against a newer front end would rename them; no corpus case has one,
/// so the wider list is the safe direction and the narrower one would be
/// the guess.
#[must_use]
pub fn c(name: &str) -> Cow<'_, str> {
    if is_listed(C_KEYWORDS, name) {
        Cow::Owned(format!("_c_{name}"))
    } else {
        Cow::Borrowed(name)
    }
}

/// The Java spelling of an IDL identifier, as JacORB generates it.
///
/// A Java keyword comes back with a single `_` in front of it. Measured
/// 2026-08-11 from JacORB 3.9's output for
/// `tests/interop-corpus/identifier-target-keywords`: the IDL module
/// `package` becomes the Java package `_package` and the enumeration
/// `class` the class `_class`, while the enumerators `template`,
/// `virtual` and `friend` keep their spelling, because Java has no such
/// keywords. One underscore, and one is the whole prefix — the same
/// output carries `_package` and not `_java_package`, which is why this
/// is a third function and not [`c`] with another string in front.
///
/// The list is Java SE's fifty reserved words plus the three literals
/// `true`, `false` and `null`, which §3.9 of the Java Language
/// Specification keeps out of the keyword list but which are just as
/// unusable as an identifier. The contextual words — `var`, `record`,
/// `sealed`, `yield` — are legal identifiers and are deliberately absent.
#[must_use]
pub fn java(name: &str) -> Cow<'_, str> {
    if is_listed(JAVA_KEYWORDS, name) {
        Cow::Owned(format!("_{name}"))
    } else {
        Cow::Borrowed(name)
    }
}

/// The Rust spelling of an IDL identifier, as `hddsgen` generates it.
///
/// A Rust keyword comes back as a raw identifier. Measured 2026-08-16
/// from `hddsgen gen rust` over `tests/interop-corpus/recursive-mutual`,
/// whose IDL declares `sequence<A> as`: the generated field is
/// `pub r#as: Vec<A>`. An emitter that wrote the bare name produced
/// `expected identifier, found keyword` and no octets — which reads on
/// the matrix as HDDS refusing a case it in fact generated.
///
/// The list is Rust 2021's, strict and reserved together: a reserved
/// word is not usable as an identifier either, and `r#` is correct for
/// both. `self`, `super`, `crate` and `Self` are the exception — they
/// cannot be raw identifiers at all — and are left alone, because a
/// field so named is a construct hddsgen would have to rename itself
/// and no corpus case has asked which way.
#[must_use]
pub fn rust(name: &str) -> Cow<'_, str> {
    const NOT_RAW_ABLE: &[&str] = &["Self", "crate", "self", "super"];
    if is_listed(RUST_KEYWORDS, name) && !is_listed(NOT_RAW_ABLE, name) {
        Cow::Owned(format!("r#{name}"))
    } else {
        Cow::Borrowed(name)
    }
}

/// Whether `name` is in a sorted keyword list.
///
/// A binary search, which is why each list below carries a test that it
/// is sorted: an unsorted list would not fail loudly, it would miss one
/// keyword and emit source the vendor's compiler refuses, which reads on
/// the matrix as a vendor difference.
fn is_listed(list: &[&str], name: &str) -> bool {
    list.binary_search(&name).is_ok()
}

/// The keywords of C23, ISO/IEC 9899:2024 §6.4.1, sorted.
///
/// The `_`-prefixed spellings sort first, which is byte order and not an
/// accident of typing.
const C_KEYWORDS: &[&str] = &[
    "_Alignas",
    "_Alignof",
    "_Atomic",
    "_BitInt",
    "_Bool",
    "_Complex",
    "_Decimal128",
    "_Decimal32",
    "_Decimal64",
    "_Generic",
    "_Imaginary",
    "_Noreturn",
    "_Static_assert",
    "_Thread_local",
    "alignas",
    "alignof",
    "auto",
    "bool",
    "break",
    "case",
    "char",
    "const",
    "constexpr",
    "continue",
    "default",
    "do",
    "double",
    "else",
    "enum",
    "extern",
    "false",
    "float",
    "for",
    "goto",
    "if",
    "inline",
    "int",
    "long",
    "nullptr",
    "register",
    "restrict",
    "return",
    "short",
    "signed",
    "sizeof",
    "static",
    "static_assert",
    "struct",
    "switch",
    "thread_local",
    "true",
    "typedef",
    "typeof",
    "typeof_unqual",
    "union",
    "unsigned",
    "void",
    "volatile",
    "while",
];

/// The keywords of C++23, ISO/IEC 14882:2024 Table 5, sorted.
///
/// The alternative tokens of §5.5 — `and`, `bitand`, `not_eq` and the
/// rest — are in the table and therefore in the list. They are spelled
/// like ordinary identifiers, which is exactly why an IDL member called
/// `xor` would slip past a list that left them out.
///
/// `export`, `register` and the other words the revisions have retired
/// stay in: a front end built against an older standard renames them,
/// and a name this list left out is a probe that does not compile
/// against that front end.
const CXX_KEYWORDS: &[&str] = &[
    "alignas",
    "alignof",
    "and",
    "and_eq",
    "asm",
    "auto",
    "bitand",
    "bitor",
    "bool",
    "break",
    "case",
    "catch",
    "char",
    "char16_t",
    "char32_t",
    "char8_t",
    "class",
    "co_await",
    "co_return",
    "co_yield",
    "compl",
    "concept",
    "const",
    "const_cast",
    "consteval",
    "constexpr",
    "constinit",
    "continue",
    "decltype",
    "default",
    "delete",
    "do",
    "double",
    "dynamic_cast",
    "else",
    "enum",
    "explicit",
    "export",
    "extern",
    "false",
    "float",
    "for",
    "friend",
    "goto",
    "if",
    "inline",
    "int",
    "long",
    "mutable",
    "namespace",
    "new",
    "noexcept",
    "not",
    "not_eq",
    "nullptr",
    "operator",
    "or",
    "or_eq",
    "private",
    "protected",
    "public",
    "register",
    "reinterpret_cast",
    "requires",
    "return",
    "short",
    "signed",
    "sizeof",
    "static",
    "static_assert",
    "static_cast",
    "struct",
    "switch",
    "template",
    "this",
    "thread_local",
    "throw",
    "true",
    "try",
    "typedef",
    "typeid",
    "typename",
    "union",
    "unsigned",
    "using",
    "virtual",
    "void",
    "volatile",
    "wchar_t",
    "while",
    "xor",
    "xor_eq",
];

/// The reserved words of Java SE, JLS §3.9, plus the three literals of
/// §3.10.3 and §3.10.8, sorted.
///
/// `const` and `goto` are reserved and unused, and are in the list for
/// the reason the retired C++ words are: `javac` refuses them as
/// identifiers whatever the compiler does with them afterwards.
const JAVA_KEYWORDS: &[&str] = &[
    "abstract",
    "assert",
    "boolean",
    "break",
    "byte",
    "case",
    "catch",
    "char",
    "class",
    "const",
    "continue",
    "default",
    "do",
    "double",
    "else",
    "enum",
    "extends",
    "false",
    "final",
    "finally",
    "float",
    "for",
    "goto",
    "if",
    "implements",
    "import",
    "instanceof",
    "int",
    "interface",
    "long",
    "native",
    "new",
    "null",
    "package",
    "private",
    "protected",
    "public",
    "return",
    "short",
    "static",
    "strictfp",
    "super",
    "switch",
    "synchronized",
    "this",
    "throw",
    "throws",
    "transient",
    "true",
    "try",
    "void",
    "volatile",
    "while",
];

/// The keywords of Rust 2021, strict and reserved together, sorted.
///
/// Reserved words are in because they are equally unusable as a bare
/// identifier, and `r#` is the right answer for both.
const RUST_KEYWORDS: &[&str] = &[
    "Self", "abstract", "as", "async", "await", "become", "box", "break", "const", "continue",
    "crate", "do", "dyn", "else", "enum", "extern", "false", "final", "fn", "for", "if", "impl",
    "in", "let", "loop", "macro", "match", "mod", "move", "mut", "override", "priv", "pub", "ref",
    "return", "self", "static", "struct", "super", "trait", "true", "try", "type", "typeof",
    "unsafe", "unsized", "use", "virtual", "where", "while", "yield",
];

#[cfg(test)]
mod tests {
    use super::{C_KEYWORDS, CXX_KEYWORDS, JAVA_KEYWORDS, c, cxx, java};

    /// `is_listed` is a binary search, so an unsorted list would answer
    /// "not a keyword" for a keyword and the emitter would paste it
    /// through.
    #[test]
    fn every_list_is_sorted() {
        for list in [C_KEYWORDS, CXX_KEYWORDS, JAVA_KEYWORDS] {
            let mut sorted = list.to_vec();
            sorted.sort_unstable();
            assert_eq!(list, sorted.as_slice());
            sorted.dedup();
            assert_eq!(list.len(), sorted.len(), "a keyword is listed twice");
        }
    }

    /// The three prefixes are the generators', and each is a different
    /// generator's.
    #[test]
    fn each_target_keeps_its_own_prefix() {
        assert_eq!(cxx("class"), "_cxx_class");
        assert_eq!(c("static"), "_c_static");
        assert_eq!(java("package"), "_package");
    }

    /// A name no target reserves is borrowed rather than rebuilt, which
    /// is the case every corpus member but a handful takes.
    #[test]
    fn an_ordinary_name_is_passed_through_unallocated() {
        assert!(matches!(cxx("impl"), std::borrow::Cow::Borrowed("impl")));
        assert!(matches!(c("impl"), std::borrow::Cow::Borrowed("impl")));
        assert!(matches!(java("impl"), std::borrow::Cow::Borrowed("impl")));
    }
}
