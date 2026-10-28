// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `idlc generate` — an IDL model, as one language's source.
//!
//! The materialisers have been in `crates/lang-rules` since the language
//! columns were built, and `zerodds_lang_rules::generate::declarations`
//! is already the one door into all twenty-one of them. What was
//! missing was a way in for a person: the only front end was
//! `tools/wireprobe/wireprobe-zerodds-<lang>-gen`, which takes
//! `--dialect`, `--values`, `--encoding`, `--endian` and writes a probe
//! program whose `main` compares octets against a vendor. An
//! application author wants none of that — an IDL file, a language, and
//! source they can compile.
//!
//! So this is `zerodds_lang_rules::generate::source` with the questions
//! a measurement asks already answered: no dialect, because a dialect is
//! what a measurement reproduces when it stands in for another vendor,
//! and a generated application has no vendor to reproduce. `--encoding`
//! and `--endian` are the two the caller still has to answer, and they
//! reach the codec: they are the `LayoutKey` the encoder and the decoder
//! are planned under, so `--encoding xcdr1` and `--encoding xcdr2` are
//! two different bodies over the same declarations, and so are `--endian
//! le` and `--endian be`.

use std::fmt::Write as _;

use zerodds_ast::facet::Extensibility;
use zerodds_ast::{Ast, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ast_layout::{Encoding, Endianness, LayoutKey};
use zerodds_lang_cst::Language;
use zerodds_lang_rules::adapter::interface::ServiceMapping;
use zerodds_lang_rules::emit::encode::CodecOptions;

/// The `--lang` spelling for `language`, if it names one of
/// [`Language::ALL`].
///
/// The spelling is [`Language::as_str`], the name a coverage table
/// prints and a diagnostic names; `cpp`/`cxx`, `csharp`/`cs`,
/// `fsharp`/`fs` and `ts` are accepted besides, for build systems that
/// pass the name unquoted.
#[must_use]
pub fn parse_language(token: &str) -> Option<Language> {
    // The spellings a build system passes without quoting: `+` and `#` are
    // awkward in CMake lists and shells, and the CMake module documents
    // `cpp` and `csharp`.
    let token = match token {
        "cpp" | "cxx" => "c++",
        "csharp" | "cs" => "c#",
        "fsharp" | "fs" => "f#",
        "ts" => "typescript",
        other => other,
    };
    Language::ALL
        .into_iter()
        .find(|candidate| candidate.as_str() == token)
}

/// Every name `--lang` accepts, comma-separated, for the usage text and
/// the error an unknown one gets.
#[must_use]
pub fn language_names() -> String {
    Language::ALL
        .into_iter()
        .map(Language::as_str)
        .collect::<Vec<_>>()
        .join(", ")
}

/// One file `generate` writes: the name it takes under `--out`, and its
/// text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Generated {
    /// The file name, extension included.
    pub name: String,
    /// What goes in it.
    pub text: String,
}

/// `ast`, read from `path`, as the files a `language` compiler takes.
///
/// Most targets use one file. Ada separates specification and body. Java
/// separates public top-level types, and C#/Java add eligible DDS companions.
/// A Java file holds at most one `public` top-level
/// type, and the materialiser emits two — `ZdTypes` for the
/// declarations and `ZdCodec` for the codec. Written as one file that is
/// `class ZdCodec is public, should be declared in a file named
/// ZdCodec.java`, measured with `javac`, whatever the file is called.
/// The wire-only Java files remain independent of the DDS companion.
///
/// # Errors
/// Why the language's rules or materialiser refused the model: the
/// message from [`zerodds_lang_rules::generate::GenerateError`].
#[cfg(test)]
pub fn files(
    ast: &Ast,
    language: Language,
    path: &str,
    stem: &str,
    key: LayoutKey,
    options: CodecOptions,
    dialect_info: Option<&str>,
) -> Result<Vec<Generated>, String> {
    files_with_mapping(
        ast,
        language,
        path,
        stem,
        key,
        options,
        dialect_info,
        ServiceMapping::Basic,
    )
}

/// `files` with the service mapping of DDS-RPC 1.0 §7.5.1 named
/// (`--rpc-mapping`): the Basic mapping, which `files` writes, or one of
/// the Enhanced readings.
///
/// # Errors
/// As `files`.
#[allow(clippy::too_many_arguments)]
pub fn files_with_mapping(
    ast: &Ast,
    language: Language,
    path: &str,
    stem: &str,
    key: LayoutKey,
    options: CodecOptions,
    dialect_info: Option<&str>,
    mapping: ServiceMapping,
) -> Result<Vec<Generated>, String> {
    let mut text =
        source_with_preamble_mapping(ast, language, path, key, options, dialect_info, mapping)?;
    let encoding = key.encoding;
    if language == Language::Python {
        if let Some(support) = crate::python_support::source(ast, encoding)? {
            text.push_str(&support);
        }
    }
    if language == Language::TypeScript {
        if let Some(support) = crate::typescript_support::source(ast, encoding, key)? {
            text.push_str(&support);
        }
    }
    if language == Language::CSharp {
        let mut output = vec![Generated {
            name: format!("{stem}.cs"),
            text,
        }];
        if let Some(support) = crate::csharp_support::source(ast, encoding, key)? {
            output.push(Generated {
                name: format!("{stem}.TypeSupport.cs"),
                text: support,
            });
        }
        return Ok(output);
    }
    if language == Language::C {
        let mut output = vec![Generated {
            name: format!("{stem}.{}", language.extension()),
            text,
        }];
        if let Some(support) = crate::c_support::source(ast, stem, key, options)? {
            output.push(Generated {
                name: format!("{stem}_typesupport.h"),
                text: support,
            });
        }
        return Ok(output);
    }
    if matches!(language, Language::Ada | Language::Ada83) {
        let (spec_name, body_name) = ada_file_names(language);
        // No body when the model emits no codec: RM 7.2(4) allows a
        // `package_body` only where the specification requires a
        // completion, so an empty one is `spec of this package does not
        // allow a body`. The specification still takes the unit's file
        // name — that is not the `--out` stem's to decide.
        let Some((spec, body)) = ada_halves(language, &text) else {
            return Ok(vec![Generated {
                name: spec_name.to_string(),
                text,
            }]);
        };
        return Ok(vec![
            Generated {
                name: spec_name.to_string(),
                text: spec,
            },
            Generated {
                name: body_name.to_string(),
                text: body,
            },
        ]);
    }
    let Some((types, codec)) = java_halves(language, &text) else {
        return Ok(vec![Generated {
            name: format!("{stem}.{}", language.extension()),
            text,
        }]);
    };
    // The RPC binding of an `interface` is a third public top-level class
    // (`ZdServices`), and a Java file holds one: it gets its own file.
    let mut output: Vec<Generated> = zerodds_lang_rules::generate::java_units(&types)
        .into_iter()
        .map(|unit| Generated {
            name: format!("{}.java", unit.class),
            text: unit.text,
        })
        .collect();
    output.push(Generated {
        name: "ZdCodec.java".to_string(),
        text: codec,
    });
    if let Some(support) = crate::java_support::source(ast, encoding, key)? {
        output.push(Generated {
            name: "ZdTypeSupports.java".to_string(),
            text: support,
        });
    }
    Ok(output)
}

/// The two file names an Ada column's units have to be written under,
/// spec first.
///
/// Not the `--out` stem, and not a choice: GNAT derives the unit name
/// from the file name, so a package `Zd_Types` in anything but
/// `zd_types.ads` fails before a declaration is read — which is what
/// `zerodds_lang_rules::adapter::ada::FILE_NAME` already says and is
/// why the name is taken from there rather than spelled again.
const fn ada_file_names(language: Language) -> (&'static str, &'static str) {
    match language {
        Language::Ada83 => ("zd83_types.ads", "zd83_types.adb"),
        _ => ("zd_types.ads", "zd_types.adb"),
    }
}

/// The Ada text as its two compilation units — the specification and
/// the body — or `None` for any other language and for an Ada text with
/// no body in it.
///
/// The context clause goes with the specification alone. RM 10.1.2
/// makes the scope of a `with_clause` on a library unit declaration
/// extend over the corresponding body, so a body that repeated the
/// three clauses would name the same units a second time; measured with
/// GNAT 14.2.0, the body compiles with no context clause of its own.
/// The header comment goes on both, because each is a file somebody
/// opens.
fn ada_halves(language: Language, text: &str) -> Option<(String, String)> {
    let package = match language {
        Language::Ada => "package body Zd_Types is",
        Language::Ada83 => "package body Zd83_Types is",
        _ => return None,
    };
    let body_at = text.find(package)?;
    let header = text.lines().next()?;
    let spec = format!("{}\n", text[..body_at].trim_end());
    let body = format!("{header}\n\n{}\n", text[body_at..].trim_end());
    // The spec half has to be the spec: a text whose package line moved
    // would otherwise be cut in the middle and written out as two files
    // that do not compile.
    if !spec.contains(" is\n") {
        return None;
    }
    Some((spec, body))
}

/// The Java text as its two compilation units, or `None` for any other
/// language and for a Java text with no codec in it.
///
/// The declarations take the header alone: measured over the maximal
/// model, the only `org.zerodds.wire` name above `public class ZdCodec`
/// is the import the preamble itself wrote, so `ZdTypes.java` needs
/// none. Everything the preamble put at the top — the import and the
/// `ZdEndian` holder, which may be declared once per package — goes with
/// the codec, which is what names them.
fn java_halves(language: Language, text: &str) -> Option<(String, String)> {
    if language != Language::Java {
        return None;
    }
    let codec_at = text.find(CODEC_CLASS)?;
    // Line one is the header comment. The preamble is what [`source`]
    // wrote after it, and it ends at the blank line that function puts
    // between the preamble and the declarations — not at
    // `TYPES_CLASS`, which would take the declarations' own Javadoc
    // along with it and leave `ZdTypes.java` without the doc that
    // describes it.
    let header = text.lines().next()?;
    let after_header = header.len() + 1;
    let preamble_end = text[after_header..]
        .find("\n\n")
        .map_or(after_header, |at| after_header + at + 1);
    let preamble = text.get(after_header..preamble_end)?;
    let types = format!("{header}\n{}\n", text[preamble_end..codec_at].trim_end());
    let codec = format!("{header}\n{preamble}{}\n", text[codec_at..].trim_end());
    // The declarations half has to be the declarations: a text whose
    // preamble boundary moved would otherwise be cut in the middle and
    // written out as two files that do not compile.
    if !types.contains(TYPES_CLASS) {
        return None;
    }
    Some((types, codec))
}

/// The two top-level types the Java materialiser emits, by the line each
/// opens on. Both `wireprobe-zerodds-java-gen` and
/// `crates/endpoint-e2e/tests/java.rs` search for the second one.
const TYPES_CLASS: &str = "public final class ZdTypes {";
const CODEC_CLASS: &str = "public class ZdCodec {";

/// `ast`, read from `path`, as one text in `language`, built for
/// `encoding`/`endianness`.
///
/// The whole of it, both Java classes included, before [`files`] cuts
/// that into compilation units — which is what makes this the shape a
/// test that checks the declarations and the codec together, rather
/// than one file at a time, wants. `--dialect` has no default-only text
/// to compare against, which is what `files` needs the dialect-info
/// string and the resolved [`CodecOptions`] for and this does not take
/// them: every call in this crate's own tests is at the default dialect,
/// and `#[cfg(test)]` says so rather than leaving a parameter unused.
#[cfg(test)]
fn source(ast: &Ast, language: Language, path: &str, key: LayoutKey) -> Result<String, String> {
    source_with_preamble(ast, language, path, key, CodecOptions::default(), None)
}

/// The same as [`source`] but with a dialect-info string appended to the
/// preamble comment and the one framing question `key` does not answer
/// (see [`zerodds_lang_rules::emit::encode::CodecOptions`]) named
/// explicitly.
#[cfg(test)]
fn source_with_preamble(
    ast: &Ast,
    language: Language,
    path: &str,
    key: LayoutKey,
    options: CodecOptions,
    dialect_info: Option<&str>,
) -> Result<String, String> {
    source_with_preamble_mapping(
        ast,
        language,
        path,
        key,
        options,
        dialect_info,
        ServiceMapping::Basic,
    )
}

/// `source_with_preamble` with the service mapping named.
fn source_with_preamble_mapping(
    ast: &Ast,
    language: Language,
    path: &str,
    key: LayoutKey,
    options: CodecOptions,
    dialect_info: Option<&str>,
    mapping: ServiceMapping,
) -> Result<String, String> {
    let encoding = key.encoding;
    let endianness = key.endianness;
    let codec_namespace = cpp_codec_namespace(path);
    // The C++ header carries the other XCDR version's codec as well, so its
    // trait reads either; `other_version` is that version where the model
    // has a codec under it.
    let mut other_version = None;
    let body = match language {
        Language::CSharp => {
            zerodds_lang_rules::generate::csharp_source_with_runtime_and_options(ast, key, options)
        }
        Language::Cpp => zerodds_lang_rules::generate::cpp_source_with_mapping(
            ast,
            key,
            options,
            &codec_namespace,
            mapping,
        )
        .map(|both| {
            other_version = both.other;
            both.text
        }),
        _ => {
            zerodds_lang_rules::generate::source_with_mapping(ast, language, key, options, mapping)
        }
    }
    .map_err(|error| error.to_string())?;

    let mut out = String::new();
    let (open, close) = comment_delimiters(language);
    let _ = writeln!(
        out,
        "{open} Generated by `idlc generate` from {path}, {} ({encoding:?}, {endianness:?}).{dialect_info} Do \
         not edit.{close}",
        language.as_str(),
        dialect_info = match dialect_info {
            Some(info) => format!(" {info}."),
            None => String::new(),
        },
    );
    // A header included twice, directly and through another, defines
    // everything in it twice without this.
    if language == Language::Cpp {
        let _ = writeln!(out, "#pragma once");
    }
    if let Some(clause) = module_clause(language) {
        let _ = writeln!(out, "{clause}\n");
    }
    // The trait is decided before the preamble because the preamble has
    // to know whether it was: the two headers `topic_type_support`
    // names are an include nothing else in the file needs, and C++ has
    // no unused-include diagnostic to make an unconditional one
    // harmless — a file that included `dds/topic/TopicTraits.hpp` and
    // declared no topic type would carry a dependency on
    // `crates/cpp/include` that its own contents do not justify.
    let traits = topic_traits(ast, language, encoding, other_version, &codec_namespace);
    let lines = preamble(
        language,
        &body,
        endianness,
        !traits.is_empty(),
        &codec_namespace,
    );
    match preamble_anchor(language) {
        // Spliced into the body, after the line the anchor names.
        Some(anchor) => out.push_str(&splice(&body, anchor, &lines)),
        None => {
            for line in &lines {
                let _ = writeln!(out, "{line}");
            }
            if !lines.is_empty() {
                out.push('\n');
            }
            out.push_str(&body);
        }
    }
    for line in epilogue(language, &body).into_iter().chain(traits) {
        if !out.ends_with('\n') {
            out.push('\n');
        }
        let _ = writeln!(out, "{line}");
    }
    if !out.ends_with('\n') {
        out.push('\n');
    }
    Ok(out)
}

/// Does `haystack` name `word` as a word of its own?
///
/// A substring test would count `Reader` inside `ReaderProxy` and write
/// an import for a name that is not there. Every identifier alphabet in
/// play here is ASCII alphanumeric plus `_`, so the neighbours decide
/// it. The same predicate `zerodds-build`'s `mentions_word` is, for the
/// same reason and over the same kind of question; a copy rather than a
/// shared function because that one is private to a build-script crate
/// this tool does not depend on.
fn names(haystack: &str, word: &str) -> bool {
    let is_ident = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let bytes = haystack.as_bytes();
    let mut from = 0;
    while let Some(found) = haystack[from..].find(word) {
        let at = from + found;
        let before_ok = at == 0 || !is_ident(char::from(bytes[at - 1]));
        let after = at + word.len();
        let after_ok = after >= bytes.len() || !is_ident(char::from(bytes[after]));
        if before_ok && after_ok {
            return true;
        }
        from = at + 1;
    }
    false
}

/// The one runtime name a generated file in `language` cannot resolve on
/// its own, and which its codec always names.
///
/// The guard the preamble is written under. Every backend's codec takes
/// the runtime's writer or reader in the signature of every function it
/// emits — as a bare type (`Writer`, `ZdWriter`), a qualified one
/// (`zerodds::wire::Writer`, `Zd_Wire.Writer`) or the module the call
/// goes through (`zdwire.WriteI32`) — so this name is in the text
/// exactly when there is a codec to import a runtime for. That is what
/// makes the preamble follow the model: a model of nothing but
/// enumerations and constants emits no codec function, names none of
/// these, and gets no import written for it.
///
/// Writing the import unconditionally is not the safe direction. Go
/// refuses to compile a file with an import nothing uses, and Rust's
/// `unused_imports` is an error under the `-D warnings` a consumer's
/// crate hands its dependencies — the same failure `zerodds-build`
/// documents at its own `RUNTIME_IMPORTS`.
///
/// `Zd_Wire` and `Zd_Wire83` are two names and not one prefix: `names`
/// is a whole-word test, so an Ada 83 file's `Zd_Wire83.Writer` does not
/// answer to `Zd_Wire`.
const fn runtime_symbol(language: Language) -> &'static str {
    match language {
        // Bare type names, imported by name or by a whole-module import.
        Language::Rust | Language::Java | Language::Kotlin => "Writer",
        Language::Python | Language::TypeScript => "Writer",
        Language::Julia | Language::Swift => "ZdWriter",
        // A qualified name whose head is the module or namespace.
        Language::C => "zdw_writer",
        Language::Cpp => "zerodds",
        // `Wire` and not `ZeroDds`: the C# materialiser opens every
        // file with `namespace ZeroDds.Idl` for the support types it
        // emits itself, so `ZeroDds` is in the text of a model with no
        // codec at all. The codec's own signatures name
        // `global::ZeroDds.Wire.Writer`, and `Idl` is the only segment
        // the boilerplate carries.
        Language::CSharp => "Wire",
        Language::Elixir => "ZeroddsWire",
        Language::FSharp => "ZdWire",
        Language::OCaml => "Zdwire",
        Language::Ada => "Zd_Wire",
        Language::Ada83 => "Zd_Wire83",
        Language::Lua => "zdw",
        Language::D | Language::Go | Language::Nim | Language::Node | Language::Zig => "zdwire",
    }
}

/// The Rust runtime names a generated codec can reach for.
///
/// The list `zerodds-build`'s `RUNTIME_IMPORTS` is, plus `Endianness`
/// for the constant below, and filtered the same way: Rust is the one
/// language here whose import is a list of names rather than a module,
/// and an unused one is an error under `-D warnings`. A name the emitter
/// starts using and this list does not carry fails loudly as an
/// unresolved name, which is the safe direction.
const RUST_RUNTIME_NAMES: [&str; 7] = [
    "Endianness",
    "Narrowed",
    "ParameterRole",
    "Reader",
    "TryConstruct",
    "WireError",
    "Writer",
];

/// The Python runtime names a generated codec can reach for.
///
/// Python has no unused-import diagnostic, so this list is not filtered
/// the way Rust's is: a name that is here and unused costs nothing, and
/// a name that is missing is a `NameError` at the first sample. It is
/// the list `crates/endpoint-e2e/tests/python.rs` splices in, which is
/// measured against a compiled codec rather than read off the runtime's
/// `__all__`.
const PYTHON_RUNTIME_NAMES: &str = "DHeader, EmHeader, EmHeaderRead, Encoding, Endianness, \
                                    Float128, ParameterRole, Pid, PidRead, PlCdr1Framing, Reader, \
                                    ReaderMark, Status, TryConstruct, WireError, Writer, \
                                    bit_pattern_bool, bit_pattern_f32, bit_pattern_f64, \
                                    bit_pattern_int, no_dheader, utf8_octets, utf16_units";

/// The TypeScript runtime names a generated codec can reach for.
///
/// The list `wireprobe-zerodds-typescript-gen`'s `runtime_import` is,
/// and imported wholesale for the reason its own doc gives: which policy
/// a configuration reaches is the layout key's answer, and a file that
/// imported only what one key needed would not compile under another.
/// `tsc` has no unused-import diagnostic under the settings this tree
/// compiles with, and an unused `type` import is erased entirely.
///
/// Two spellings are not the runtime's own. `Status` is imported under
/// the name the emitter writes — `crate::emit::typescript`'s `Constant`
/// arm spells it `ZdReaderStatus` — because `Status` is a name an IDL
/// `enum` may itself declare, and the codec and the declaration share a
/// file. The seven `type` bindings have no run-time form at all, and an
/// ordinary `import` of one is an error under `verbatimModuleSyntax`.
const TYPESCRIPT_RUNTIME_NAMES: &str = "Endianness, FixedDigitCount, ParameterRole, \
                                        PID_FLAGS_BARE, \
                                        PID_FLAGS_IMPL_EXTENSION_ON_MEMBERS, \
                                        PID_FLAGS_SENTINEL_MUST_UNDERSTAND, PID_SHORT_MAX_ID, \
                                        TryConstruct, WcharFraming, WcharWidth, Writer, \
                                        WstringLengthPolicy, WstringUnitOrder, \
                                        float128FromDecimalString, noDheader, bitPatternBool, \
                                        bitPatternF32, bitPatternF64, bitPatternInt, zdBigInt, \
                                        zdChar, zdNumber, Reader, Status as ZdReaderStatus, \
                                        pidIsUserMember, utf8Octets, utf16Units, type DHeader, \
                                        type EmHeader, type Pid, type PlCdr1Framing, \
                                        type EmHeaderRead, type PidRead, type ReaderMark";

/// What has to stand between `language`'s module clause and its
/// declarations for the file to compile: the runtime import, and the
/// byte order the file was generated for as a name a caller can use.
///
/// Empty when `body` names no runtime symbol — see [`runtime_symbol`].
///
/// # The endianness constant
///
/// `--endian` does not change one character of the emitted codec, and
/// cannot: every wire runtime holds the byte order in its writer and its
/// reader, so the same `w.WriteI32(v)` writes either order and the
/// choice is made where the writer is built. Moving it into the
/// generated text so that a flag would have something to change would
/// make the design worse. What the flag can honestly do is name its own
/// answer, once, in the file it was passed for — which is the value the
/// caller then hands the runtime's writer.
///
/// `traits` says whether [`topic_traits`] wrote anything, and reaches
/// exactly one language: the C++ specialisations at the end of the file
/// name `dds::topic::topic_type_support`, which lives in a header under
/// `crates/cpp/include` that nothing else in a generated file touches.
fn preamble(
    language: Language,
    body: &str,
    endianness: Endianness,
    traits: bool,
    codec_namespace: &str,
) -> Vec<String> {
    if !names(body, runtime_symbol(language)) {
        return Vec::new();
    }
    let little = endianness == Endianness::Little;
    let pick = |le: &str, be: &str| if little { le } else { be }.to_string();
    match language {
        Language::Rust => {
            let wanted: Vec<&str> = RUST_RUNTIME_NAMES
                .into_iter()
                .filter(|name| *name == "Endianness" || names(body, name))
                .collect();
            vec![
                format!("use zerodds_wire::{{{}}};", wanted.join(", ")),
                format!(
                    "pub const ZD_ENDIANNESS: Endianness = Endianness::{};",
                    pick("Little", "Big")
                ),
            ]
        }
        Language::C => vec![
            "#include \"zerodds_wire.h\"".to_string(),
            format!("#define ZD_ENDIANNESS {}", pick("ZDW_LITTLE", "ZDW_BIG")),
        ],
        Language::Cpp => {
            let mut lines = vec!["#include \"zerodds/wire.hpp\"".to_string()];
            if traits {
                // `TopicTraits.hpp` declares the primary template the
                // specialisations at the end of the file specialise, and
                // `zerodds::psm::DataRepresentationKind` —
                // `extensibility()`'s return type. The `decode` function
                // `zerodds/dds.hpp` calls takes `zerodds::wire::Encoding`
                // as its third parameter, already reachable through the
                // `zerodds/wire.hpp` include above; no second codec header
                // is needed for it. `<stdexcept>` is the refusal channel
                // those two functions have: `TypedReader<T>::take`
                // catches, frees the FFI buffer and rethrows.
                lines.push("#include \"dds/topic/TopicTraits.hpp\"".to_string());
                lines.push("#include <stdexcept>".to_string());
            }
            if zerodds_lang_rules::adapter::cpp::cpp_mapping()
                == zerodds_lang_rules::adapter::cpp::CppMapping::Classic
            {
                // The classic types of `--cpp-mapping classic` and the conversions that
                // hand them to the codec.
                lines.push("#include \"zerodds/classic/convert.hpp\"".to_string());
                lines.extend(
                    [
                        "<memory>",
                        "<optional>",
                        "<utility>",
                        "<array>",
                        "<type_traits>",
                    ]
                    .map(|header| format!("#include {header}")),
                );
            }
            if zerodds_lang_rules::adapter::cpp::cpp_mapping()
                == zerodds_lang_rules::adapter::cpp::CppMapping::Psm
            {
                // The PSM classes of `--cpp-mapping psm` and the conversions that
                // hand them to the codec.
                lines.push("#include \"zerodds/psm/convert.hpp\"".to_string());
                lines.extend(
                    ["<map>", "<memory>", "<optional>", "<utility>", "<array>"]
                        .map(|header| format!("#include {header}")),
                );
            }
            lines.extend([
                "#include <cstddef>".to_string(),
                "#include <cstdint>".to_string(),
                "#include <string>".to_string(),
                "#include <vector>".to_string(),
                // In the codec's namespace, like the codec: every file
                // declares one, and at file scope the second header in a
                // translation unit redefined the first one's.
                format!(
                    "namespace {codec_namespace} {{ constexpr zerodds::wire::Endianness \
                     zd_endianness = zerodds::wire::Endianness::{}; }}",
                    pick("Little", "Big")
                ),
            ]);
            lines
        }
        // A second top-level type, not `public`: Java allows one public
        // type per file and the declarations already are it.
        Language::Java => vec![
            "import org.zerodds.wire.*;".to_string(),
            format!(
                "final class ZdEndian {{ static final Endianness VALUE = Endianness.{}; }}",
                pick("LITTLE", "BIG")
            ),
        ],
        // No `using`: the declarations name the runtime through
        // `global::ZeroDds.Wire`, which resolves from anywhere. A second
        // block reopens the namespace the declarations are already in.
        // The materializer uses nullable value types but deliberately does not
        // claim nullable-reference annotations. Make that source contract
        // explicit rather than inheriting the consumer project's ambient mode.
        Language::CSharp => vec![
            "#nullable disable".to_string(),
            format!(
                "namespace ZeroDds.Idl {{ public static class ZdEndian {{ public const \
             global::ZeroDds.Wire.Endianness Value = global::ZeroDds.Wire.Endianness.{}; }} }}",
                pick("Little", "Big")
            ),
        ],
        Language::Python => vec![
            format!("from zerodds_wire import {PYTHON_RUNTIME_NAMES}"),
            format!("ZD_ENDIANNESS = Endianness.{}", pick("LITTLE", "BIG")),
        ],
        Language::TypeScript => vec![
            format!("import {{ {TYPESCRIPT_RUNTIME_NAMES} }} from \"@zerodds/wire\";"),
            format!(
                "export const zdEndianness = Endianness.{};",
                pick("Little", "Big")
            ),
        ],
        Language::Go => vec![
            "import \"zdwire\"".to_string(),
            format!(
                "const ZdEndianness = zdwire.{}",
                pick("LittleEndian", "BigEndian")
            ),
        ],
        // `Zdwire` is a compilation unit of the same program, reached by
        // name: OCaml has no import clause to write.
        Language::OCaml => vec![format!(
            "let zd_endianness = Zdwire.{}",
            pick("Little", "Big")
        )],
        // `std.typecons` beside the runtime: an `@optional` member is a
        // `Nullable!T`, and that name is not in scope by itself —
        // `ldc2` answers `template `Nullable` is not defined, did you
        // mean zdNullable(T)(T v)?`, measured. A model with no
        // `@optional` member names neither.
        Language::D => {
            let mut lines = vec!["import zdwire;".to_string()];
            if names(body, "Nullable") {
                lines.push("import std.typecons;".to_string());
            }
            lines.push(format!(
                "enum ZdEndianness = zdwire.Endianness.{};",
                pick("little", "big")
            ));
            lines
        }
        // Fully qualified in the body; the constant is its own module.
        Language::Elixir => vec![format!(
            "defmodule ZdEndian do\n  def value, do: :{}\nend",
            pick("le", "be")
        )],
        Language::Julia => vec![
            "using .ZeroddsWire".to_string(),
            format!("const ZD_ENDIANNESS = {}", pick("ZD_LITTLE", "ZD_BIG")),
        ],
        // Lua's materialiser writes `M.Pose = …` and
        // `function ZD.zd_enc_Pose(…)` and declares neither table: they
        // are the caller's, exactly as `wireprobe-zerodds-lua-gen` and
        // `crates/endpoint-e2e/tests/lua.rs` declare them. `ZD` is what
        // `require` hands back — see [`epilogue`].
        Language::Lua => vec![
            "local zdw = require(\"zerodds_wire\")".to_string(),
            "local M = {}".to_string(),
            "M._descriptors = {}".to_string(),
            "local ZD = {}".to_string(),
            format!("local ZD_ENDIANNESS = \"{}\"", pick("le", "be")),
        ],
        // `std/options` beside the runtime: an `@optional` member is an
        // `Option[T]`, not in scope by itself. `Option[` and not the bare
        // word, which a comment can carry. A `map` is a
        // `seq[ZdMapEntry_…]` and needs no import.
        Language::Nim => {
            let mut lines = vec!["import zdwire".to_string()];
            if body.contains("Option[") {
                lines.push("import std/options".to_string());
            }
            lines.push(format!(
                "const zdEndianness* = zdwire.{}",
                pick("EndianLittle", "EndianBig")
            ));
            lines
        }
        Language::Node => vec![
            "const zdwire = require(\"./zdwire.js\");".to_string(),
            format!(
                "const zdEndianness = zdwire.Endianness.{};",
                pick("Little", "Big")
            ),
        ],
        // `zdEndianness` and not `ZdEndianness`: the runtime's own enum
        // has the second name and a file cannot hold both.
        Language::Swift => vec![
            "import ZdWire".to_string(),
            format!(
                "let zdEndianness: ZdEndianness = .{}",
                pick("little", "big")
            ),
        ],
        // `std` beside the runtime: a Table 9 defaulter takes a
        // `std.mem.Allocator`, so a model with a `string`, a `sequence`
        // or a `map` in it names the standard library and a model of
        // plain scalars does not.
        Language::Zig => {
            let mut lines = vec!["const zdwire = @import(\"zdwire.zig\");".to_string()];
            if names(body, "std") {
                lines.push("const std = @import(\"std\");".to_string());
            }
            lines.push(format!(
                "pub const zd_endianness: zdwire.Endianness = .{};",
                pick("little", "big")
            ));
            lines
        }
        Language::FSharp => vec![format!(
            "let zdEndianness = ZdWire.Endian.{}",
            pick("LE", "BE")
        )],
        Language::Kotlin => vec![
            "import zerodds.wire.*".to_string(),
            format!("val zdEndianness = Endianness.{}", pick("LITTLE", "BIG")),
        ],
        // Ada's context clause has to name every unit the declarations
        // reach, not only the runtime: the materialiser writes
        // `Interfaces.Integer_32` and `Ada.Strings.Unbounded` into the
        // types themselves.
        //
        // No endianness constant, and not because Ada has no way to
        // spell one. An Ada object declaration lives inside a
        // compilation unit, and the declarations these clauses stand in
        // front of are not one: the materialiser writes a bare sequence
        // of types and subprograms with bodies, which is a package
        // *body*'s contents and not a file. Writing
        // `ZD_Endianness : constant …` at file scope would add a second
        // illegal construct to a file that already has one, and would
        // sit outside the package once one is written. `--endian`
        // reaches the Ada columns' `LayoutKey` as it does every other's;
        // what it has no place to be named in yet is the file.
        //
        // The `use type` clauses are the second half of the same
        // context clause (RM 10.1.2 admits a `use_clause` as a
        // `context_item`), and they are not decoration: a predefined
        // operator of a type declared in another package is not
        // directly visible where the type is reached as a qualified
        // name (RM 8.4). Without them the codec's own comparisons are
        // `operator for type "Interfaces.Unsigned_32" is not directly
        // visible`, measured on GNAT 14.2.0 — the same twelve clauses
        // `wireprobe-zerodds-ada-gen` writes above its probe, and for
        // the same reason. `use type` and not `use`: it makes the
        // operators visible and nothing else, so it cannot make a
        // generated declaration a homograph of a predefined name.
        Language::Ada => vec![
            "with Interfaces;".to_string(),
            "with Ada.Strings.Unbounded;".to_string(),
            "with Zd_Wire;".to_string(),
            "use type Interfaces.Unsigned_8;".to_string(),
            "use type Interfaces.Unsigned_16;".to_string(),
            "use type Interfaces.Unsigned_32;".to_string(),
            "use type Interfaces.Unsigned_64;".to_string(),
            "use type Interfaces.Integer_8;".to_string(),
            "use type Interfaces.Integer_16;".to_string(),
            "use type Interfaces.Integer_32;".to_string(),
            "use type Interfaces.Integer_64;".to_string(),
            "use type Interfaces.IEEE_Float_32;".to_string(),
            "use type Interfaces.IEEE_Float_64;".to_string(),
            "use type Zd_Wire.Status;".to_string(),
            "use type Zd_Wire.Try_Construct;".to_string(),
        ],
        Language::Ada83 => vec![
            "with Interfaces;".to_string(),
            "with Zd_Wire83;".to_string(),
        ],
    }
}

/// What has to stand *after* `language`'s declarations.
///
/// One language answers with anything. A Lua file is a chunk, and a
/// chunk hands `require` whatever it returns; the codec the declarations
/// fill `ZD` with reaches a caller only if the chunk returns it. Every
/// other language here names its declarations through a module, a
/// namespace or the file itself and has nothing to return.
fn epilogue(language: Language, body: &str) -> Vec<String> {
    match language {
        Language::Lua if names(body, runtime_symbol(language)) => {
            vec![String::new(), "return ZD".to_string()]
        }
        _ => Vec::new(),
    }
}

/// The `dds::topic::topic_type_support<T>` specialisations `language`'s
/// declarations call for, as the lines that follow them.
///
/// One language answers with anything, and for the reason the preamble
/// exists at all: a generated file is source somebody compiles, and the
/// C++ file this tool writes is compiled against
/// `crates/cpp/include/zerodds/dds.hpp`, whose `zerodds::TypedWriter<T>`
/// and `zerodds::TypedReader<T>` instantiate
/// `dds::topic::topic_type_support<T>` and name six members of it —
/// `type_name`, `is_keyed`, `type_object`, `type_object_len`, `encode`
/// and a four-argument `decode`. The declarations above and the codec
/// beside them answer every wire question those six ask; what was
/// missing is the joint, and this is it. `the_cpp_trait_carries_every_
/// member_dds_hpp_asks_for` reads the six names out of that header, so
/// the list is not spelled twice.
///
/// **Model-driven, not a fixed list.** Every named aggregate the model
/// declares that is not `@nested` gets one. `@nested` is left out
/// deliberately: XTypes 1.3 §7.4.6.3.5 makes such a declaration a member
/// type and not a topic type, and a specialisation would let
/// `TypedWriter<T>` publish a topic of a type the model says cannot be
/// one — the same refusal the Rust column states as
/// `DdsType::IS_NESTED`, which `create_topic` answers with
/// `PreconditionNotMet`. A comment stands where the specialisation would
/// have been, so a reader who looks for it finds the reason.
fn topic_traits(
    ast: &Ast,
    language: Language,
    encoding: Encoding,
    other_version: Option<(&str, Encoding)>,
    codec_namespace: &str,
) -> Vec<String> {
    if language != Language::Cpp {
        return Vec::new();
    }
    let mut bodies = Vec::new();
    // The types a service defined by a pair of types leaves to its RPC
    // binding (DDS-RPC 1.0 §7.3.1.4): the runtime's common types, and the
    // request and reply types, which `dds::rpc::dds_type_traits` serves.
    let by_rpc_binding = zerodds_lang_rules::emit::type_pair_cpp::excluded_types(ast);
    for (id, node) in ast.types() {
        if by_rpc_binding.contains(&id) {
            continue;
        }
        if node.name.is_empty()
            || !matches!(
                node.kind,
                TypeKind::Structure { .. } | TypeKind::Union { .. } | TypeKind::Valuetype { .. }
            )
        {
            continue;
        }
        if is_nested(node) {
            bodies.push(format!(
                "// `{}`: `@nested`, XTypes 1.3 \u{a7}7.4.6.3.5 \u{2014} a member type and \
                 not a\n// topic type, so it gets no `topic_type_support` and \
                 `zerodds::TypedWriter`\n// cannot be instantiated at it.",
                node.name
            ));
            continue;
        }
        bodies.push(cpp_specialisation(
            ast,
            id,
            node,
            encoding,
            other_version,
            codec_namespace,
        ));
        if zerodds_lang_rules::adapter::cpp::cpp_mapping()
            == zerodds_lang_rules::adapter::cpp::CppMapping::Psm
            && matches!(
                node.kind,
                TypeKind::Structure { .. } | TypeKind::Union { .. }
            )
        {
            bodies.push(cpp_psm_specialisation(ast, id));
        }
        if zerodds_lang_rules::adapter::cpp::cpp_mapping()
            == zerodds_lang_rules::adapter::cpp::CppMapping::Classic
            && matches!(
                node.kind,
                TypeKind::Structure { .. } | TypeKind::Union { .. }
            )
        {
            bodies.push(cpp_classic_specialisation(ast, id));
        }
    }
    if bodies.is_empty() {
        return Vec::new();
    }
    let mut lines = vec![
        String::new(),
        "// `dds::topic::topic_type_support<T>` \u{2014} the trait \
         `zerodds/dds.hpp` asks"
            .to_string(),
        "// every topic type for, so `zerodds::TypedWriter<T>` and \
         `TypedReader<T>` take"
            .to_string(),
        "// a type this file declares. Nothing here answers a wire question a \
         second"
            .to_string(),
        "// time: `encode` and `decode` build the runtime's writer and reader \
         and call"
            .to_string(),
        "// the codec above, and the `TypeObject` is the octets \
         `zerodds_types::from_ast`"
            .to_string(),
        "// lowers from this model.".to_string(),
        "namespace dds {".to_string(),
        "namespace topic {".to_string(),
    ];
    for body in bodies {
        lines.push(String::new());
        lines.push(body);
    }
    lines.push(String::new());
    lines.push("} // namespace topic".to_string());
    lines.push("} // namespace dds".to_string());
    lines
}

/// Whether the declaration carries `@nested`.
///
/// The same predicate `crates/zerodds-build`'s `dds_type::is_nested` is,
/// over the same facet; a copy rather than a shared function because
/// that one is private to a build-script crate this tool does not depend
/// on.
pub(crate) fn is_nested(node: &TypeNode) -> bool {
    node.facets
        .xtypes
        .as_ref()
        .is_some_and(|xtypes| xtypes.is_nested)
}

/// The namespace a generated C++ header keeps its codec in: `zd_` and the
/// IDL file's stem, anything but an ASCII letter, digit or `_` spelled
/// `_` and runs of `_` folded into one (a double underscore is reserved
/// in C++, [lex.name]). Two headers generated from files of different
/// stems therefore never share a helper, and the names a caller reaches
/// for (`zd_nebula_ctl_v1::zd_endianness`) follow from the file name.
fn cpp_codec_namespace(path: &str) -> String {
    let stem = std::path::Path::new(path)
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut out = String::from("zd_");
    for c in stem.chars() {
        let c = if c.is_ascii_alphanumeric() { c } else { '_' };
        if !(c == '_' && out.ends_with('_')) {
            out.push(c);
        }
    }
    out.trim_end_matches('_').to_string()
}

/// One `topic_type_support` specialisation, with the `TypeObject` blob
/// it points at.
fn cpp_specialisation(
    ast: &Ast,
    id: Id<TypeNode>,
    node: &TypeNode,
    encoding: Encoding,
    other_version: Option<(&str, Encoding)>,
    codec_namespace: &str,
) -> String {
    let path = qualified_cpp_path(ast, id);
    let tag = zerodds_lang_rules::adapter::cpp::cpp_type_tag(ast, id);
    let encoder = zerodds_lang_rules::emit::encode::codec_name::<zerodds_lang_rules::emit::cpp::Cpp>(
        ast,
        id,
        zerodds_ast_layout::key::Purpose::Data,
    );
    let decoder = zerodds_lang_rules::emit::decode::decoder_name::<
        zerodds_lang_rules::emit::cpp::Cpp,
    >(ast, id);
    let version = xcdr_version(encoding);
    let max_align = encoding.max_alignment();

    let mut out = String::new();
    let (object, object_len) = match type_object_octets(ast, id) {
        Some(bytes) => {
            let _ = write!(
                out,
                "/// The `CompleteTypeObject` of `{path}`, XCDR2-LE, XTypes 1.3 \
                 \u{a7}7.3.4.\n///\n/// `zerodds_writer_create_typed` registers it and derives \
                 the\n/// `TypeIdentifier` from it before the endpoint is published, so a \
                 peer\n/// that asks the TypeLookup service for that identifier gets an \
                 answer.\ninline constexpr std::uint8_t zd_type_object_{tag}[] = {{\n{}}};\n\n",
                octet_rows(&bytes)
            );
            (
                format!("zd_type_object_{tag}"),
                format!("sizeof(zd_type_object_{tag})"),
            )
        }
        // Not a hole: `zerodds/dds.hpp` documents the pair of zeroes as
        // the byte-oriented `create_kind` path, which matches by type
        // name per DDS 1.4 §2.2.3. Stating an identity the runtime
        // cannot resolve would be the worse answer, and the comment
        // says which types take this road.
        None => {
            let _ = write!(
                out,
                "// `{}`: XTypes has no `TypeObject` for this declaration, so the \
                 pair\n// below is the `nullptr`/`0` `zerodds/dds.hpp` reads as \"send me down\
                 \n// the `create_kind` path\" \u{2014} matching by type name, DDS 1.4 \
                 \u{a7}2.2.3.\n",
                node.name
            );
            ("nullptr".to_string(), "0".to_string())
        }
    };

    let dependencies = if object == "nullptr" {
        Vec::new()
    } else {
        type_object_dependency_octets(ast, id)
    };
    for (i, bytes) in dependencies.iter().enumerate() {
        let _ = write!(
            out,
            "/// Dependency {i} of `zd_type_object_{tag}`: a complete TypeObject it \
             refers to.\ninline constexpr std::uint8_t zd_type_object_{tag}_dep{i}[] = {{\n{}}};\n\n",
            octet_rows(bytes)
        );
    }
    let dependency_accessors = if dependencies.is_empty() {
        String::from(
            "    static std::size_t type_object_dependency_count() { return 0; }\n    \
             static const std::uint8_t *type_object_dependency(std::size_t) { return nullptr; }\n    \
             static std::size_t type_object_dependency_len(std::size_t) { return 0; }\n\n",
        )
    } else {
        let names: Vec<String> = (0..dependencies.len())
            .map(|i| format!("zd_type_object_{tag}_dep{i}"))
            .collect();
        let lens: Vec<String> = names.iter().map(|n| format!("sizeof({n})")).collect();
        format!(
            "    /// The complete TypeObjects `type_object()` refers to, transitively.\n    \
             static std::size_t type_object_dependency_count() {{ return {n}; }}\n    \
             static const std::uint8_t *type_object_dependency(std::size_t i) {{\n        \
             static const std::uint8_t *const all[] = {{{names}}};\n        \
             return i < {n} ? all[i] : nullptr;\n    }}\n    \
             static std::size_t type_object_dependency_len(std::size_t i) {{\n        \
             static const std::size_t lens[] = {{{lens}}};\n        \
             return i < {n} ? lens[i] : 0;\n    }}\n\n",
            n = dependencies.len(),
            names = names.join(", "),
            lens = lens.join(", "),
        )
    };

    let _ = write!(
        out,
        "template <>\nstruct topic_type_support<{path}> {{\n    \
         /// The peer's topic type name, which XTypes 1.3 \u{a7}7.6.3 matches by\n    \
         /// strict equality \u{2014} the IDL scoped name, without the leading `::`\n    \
         /// that Cyclone, Fast DDS and RTI all omit.\n    \
         static const char *type_name() {{ return \"{}\"; }}\n\n    \
         /// Whether an instance of this type is keyed, XTypes 1.3 \u{a7}7.6.8: \
         `true`\n    /// exactly when the key holder has a member in it.\n    \
         static constexpr bool is_keyed() {{ return {}; }}\n\n    \
         /// The extensibility the sample is framed with, and which\n    \
         /// `zerodds::TypedWriter<T>` declares to the C ABI so the\n    \
         /// encapsulation identifier names the body honestly \u{2014} XTypes 1.3\n    \
         /// \u{a7}7.6.3.1.2 Table 59, DDSI-RTPS 2.5 \u{a7}10.5. An `@appendable` body\n    \
         /// carries a DHEADER and a `@mutable` one an EMHEADER per member;\n    \
         /// under a `PLAIN_CDR2` identifier a reader that believes the header\n    \
         /// reads the delimiter as the first member.\n    \
         static constexpr ::zerodds::psm::DataRepresentationKind \
         extensibility() {{\n        \
         return ::zerodds::psm::DataRepresentationKind::{};\n    }}\n\n    \
         static const std::uint8_t *type_object() {{ return {object}; }}\n    \
         static std::size_t type_object_len() {{ return {object_len}; }}\n\n\
         {dependency_accessors}",
        qualified_idl_name(ast, id, node),
        is_keyed(ast, id, encoding),
        extensibility_kind(ast.effective_extensibility(id)),
    );

    // Little-endian whatever `--endian` said, and stated rather than
    // silently overridden. `zerodds::TypedWriter<T>::write` hands these
    // octets to `zerodds_writer_write`, and that FFI stamps the
    // encapsulation header from its runtime's representation offer,
    // which is little-endian and not the caller's to pick. Big-endian
    // octets under a `*_LE` identifier are read at the wrong offsets by
    // every peer that believes the header, this implementation
    // included. `<codec namespace>::zd_endianness` above is still the
    // flag's answer, for a caller who drives `zerodds::wire::Writer` itself.
    let _ = write!(
        out,
        "    /// One sample's octets, for `zerodds::TypedWriter<T>::write`.\n    ///\n    \
         /// Little-endian whatever `--endian` was passed: `zerodds_writer_write`\n    \
         /// stamps the encapsulation identifier from the runtime's own\n    \
         /// representation offer, which is little-endian, and big-endian octets\n    \
         /// under a `*_LE` identifier are read at the wrong offsets by every peer\n    \
         /// that believes the header. `{codec_namespace}::zd_endianness` above is\n    \
         /// the flag's answer, for a caller who builds the writer itself.\n    \
         static std::vector<std::uint8_t> encode(const {path} &v) {{\n        \
         zerodds::wire::Writer w(zerodds::wire::Endianness::Little, {max_align});\n        \
         ::{codec_namespace}::{encoder}(w, v);\n        \
         return w.finish();\n    }}\n\n"
    );

    let other = match version {
        "PlainCdr2" => "PlainCdr1",
        _ => "PlainCdr2",
    };
    let flag = match version {
        "PlainCdr2" => "xcdr1",
        _ => "xcdr2",
    };
    // By version, not by one enumerator: PL_CDR2 and D_CDR2 are XCDR2 as
    // well, and a caller that passes them is still asking for XCDR2.
    let other_version_test = match version {
        "PlainCdr2" => "is_v1",
        _ => "is_v2",
    };
    // Each version through its own codec: the one `--encoding` named in
    // `codec_namespace`, the other in the namespace beside it. Where the
    // model has no codec under the other version, that version is refused
    // by name — running its octets through this codec would read wrong
    // values out of a well-formed sample and report nothing.
    let other_branch = match other_version {
        Some((nested, other_encoding)) => format!(
            "{{\n            \
             zerodds::wire::Reader r(raw, len,\n                \
             be ? zerodds::wire::Endianness::Big : zerodds::wire::Endianness::Little,\n                \
             {});\n            \
             {path} v;\n            \
             ::{codec_namespace}::{nested}::{decoder}(r, v, zerodds::wire::TryConstruct::Discard);\n            \
             if (r.reader_status() != zerodds::wire::Status::Ok) {{\n                \
             throw std::runtime_error(std::string(\"{path}: \") +\n                    \
             zerodds::wire::status_name(r.reader_status()));\n            }}\n            \
             return v;\n        }}",
            other_encoding.max_alignment()
        ),
        None => format!(
            "{{\n            \
             throw std::runtime_error(\n                \
             \"{path}: this codec was generated for {version}, and the \"\n                \
             \"sample announces {other}, which this model has no {other} codec for. \"\n                \
             \"Generate a second file with `--encoding {flag}`.\");\n        }}"
        ),
    };
    let _ = write!(
        out,
        "    /// One sample, from the octets and the encapsulation header's own\n    \
         /// answers: `version` is the representation identifier's, `be` its byte\n    \
         /// order, DDSI-RTPS 2.5 \u{a7}10.5. The writer chose the version\n    \
         /// (DDS-XTypes 1.3 \u{a7}7.6.3.1.2), and each one is read through its own\n    \
         /// codec.\n    static {path} decode(const std::uint8_t *raw, \
         std::size_t len,\n                        ::zerodds::wire::Encoding version, \
         bool be) {{\n        \
         if (::zerodds::wire::{other_version_test}(version)) {other_branch}\n        \
         zerodds::wire::Reader r(raw, len,\n            \
         be ? zerodds::wire::Endianness::Big : zerodds::wire::Endianness::Little,\n            \
         {max_align});\n        \
         {path} v;\n        \
         ::{codec_namespace}::{decoder}(r, v, zerodds::wire::TryConstruct::Discard);\n        \
         // The reader accumulates its first refusal rather than throwing, so a\n        \
         // decoder that returned here would hand back a half-filled value with\n        \
         // nothing said. `TypedReader<T>::take` catches, frees the FFI buffer\n        \
         // and rethrows.\n        \
         if (r.reader_status() != zerodds::wire::Status::Ok) {{\n            \
         throw std::runtime_error(std::string(\"{path}: \") +\n                \
         zerodds::wire::status_name(r.reader_status()));\n        }}\n        \
         return v;\n    }}\n}};"
    );
    out
}

/// The `topic_type_support` of the PSM class of `id` (`--cpp-mapping psm`): everything is the
/// native specialisation's — the type name, the key, the extensibility, the `TypeObject` — and
/// `encode` and `decode` pass the sample through `zerodds::psm::Conv`, so the octets are the
/// codec's.
fn cpp_psm_specialisation(ast: &Ast, id: Id<TypeNode>) -> String {
    let native = qualified_cpp_path(ast, id);
    let psm = zerodds_lang_rules::adapter::cpp_psm::psm_path(ast, id);
    format!(
        "template <>\nstruct topic_type_support<{psm}> {{\n    \
         using native = topic_type_support<{native}>;\n    \
         static const char *type_name() {{ return native::type_name(); }}\n    \
         static constexpr bool is_keyed() {{ return native::is_keyed(); }}\n    \
         static constexpr ::zerodds::psm::DataRepresentationKind extensibility() {{\n        \
         return native::extensibility();\n    }}\n    \
         static const std::uint8_t *type_object() {{ return native::type_object(); }}\n    \
         static std::size_t type_object_len() {{ return native::type_object_len(); }}\n    \
         static std::size_t type_object_dependency_count() {{\n        \
         return native::type_object_dependency_count();\n    }}\n    \
         static const std::uint8_t *type_object_dependency(std::size_t i) {{\n        \
         return native::type_object_dependency(i);\n    }}\n    \
         static std::size_t type_object_dependency_len(std::size_t i) {{\n        \
         return native::type_object_dependency_len(i);\n    }}\n\n    \
         /// The PSM sample, as the native one the codec writes.\n    \
         static std::vector<std::uint8_t> encode(const {psm} &v) {{\n        \
         return native::encode(::zerodds::psm::Conv<{psm}>::to_native(v));\n    }}\n\n    \
         /// The PSM sample, from the octets the native decoder reads.\n    \
         static {psm} decode(const std::uint8_t *raw, std::size_t len,\n                        \
         ::zerodds::wire::Encoding version, bool be) {{\n        \
         return ::zerodds::psm::Conv<{psm}>::from_native(native::decode(raw, len, version, be));\n    }}\n}};"
    )
}

/// The `topic_type_support` of the classic type of `id` (`--cpp-mapping classic`): everything is
/// the native specialisation's, and `encode` and `decode` pass the sample through
/// `zerodds::classic::Conv`, so the octets are the codec's.
fn cpp_classic_specialisation(ast: &Ast, id: Id<TypeNode>) -> String {
    let native = qualified_cpp_path(ast, id);
    let classic = zerodds_lang_rules::adapter::cpp_psm::psm_path(ast, id);
    format!(
        "template <>\nstruct topic_type_support<{classic}> {{\n    \
         using native = topic_type_support<{native}>;\n    \
         static const char *type_name() {{ return native::type_name(); }}\n    \
         static constexpr bool is_keyed() {{ return native::is_keyed(); }}\n    \
         static constexpr ::zerodds::psm::DataRepresentationKind extensibility() {{\n        \
         return native::extensibility();\n    }}\n    \
         static const std::uint8_t *type_object() {{ return native::type_object(); }}\n    \
         static std::size_t type_object_len() {{ return native::type_object_len(); }}\n    \
         static std::size_t type_object_dependency_count() {{\n        \
         return native::type_object_dependency_count();\n    }}\n    \
         static const std::uint8_t *type_object_dependency(std::size_t i) {{\n        \
         return native::type_object_dependency(i);\n    }}\n    \
         static std::size_t type_object_dependency_len(std::size_t i) {{\n        \
         return native::type_object_dependency_len(i);\n    }}\n\n    \
         /// The classic sample, as the native one the codec writes.\n    \
         static std::vector<std::uint8_t> encode(const {classic} &v) {{\n        \
         return native::encode(::zerodds::classic::Conv<{classic}>::to_native(v));\n    }}\n\n    \
         /// The classic sample, from the octets the native decoder reads.\n    \
         static {classic} decode(const std::uint8_t *raw, std::size_t len,\n                        \
         ::zerodds::wire::Encoding version, bool be) {{\n        \
         return ::zerodds::classic::Conv<{classic}>::from_native(native::decode(raw, len, version, be));\n    }}\n}};"
    )
}

/// `extensibility` as `zerodds::psm::DataRepresentationKind`
/// spells it.
///
/// That enumeration is declared in
/// `crates/cpp/include/dds/topic/TopicTraits.hpp` with the comment that it
/// is the compile-time tag `topic_type_support<T>::extensibility()`
/// returns — the published interface named this member before anything
/// emitted it.
const fn extensibility_kind(extensibility: Extensibility) -> &'static str {
    match extensibility {
        Extensibility::Final => "FINAL",
        Extensibility::Appendable => "APPENDABLE",
        Extensibility::Mutable => "MUTABLE",
    }
}

/// The XTypes 1.3 §7.4 version `encoding` belongs to, spelled as
/// `zerodds::wire::Encoding` spells its plain (top-level-sample) form —
/// the form `topic_type_support<T>::decode`'s third parameter takes.
const fn xcdr_version(encoding: Encoding) -> &'static str {
    match encoding {
        Encoding::PlainCdr1 | Encoding::PlCdr1 => "PlainCdr1",
        Encoding::PlainCdr2 | Encoding::DelimitedCdr | Encoding::PlCdr2 => "PlainCdr2",
    }
}

/// The `CompleteTypeObject` of `id` as XCDR2-LE octets, or `None` where
/// XTypes has none for the declaration.
///
/// The same three steps `crates/zerodds-build`'s `dds_type::
/// identity_items` takes for the Rust column, and deliberately the same
/// function: one lowering of one model, rendered into two languages.
/// The blob is emitted only if it survives its own round trip — one that
/// does not parse would reach the runtime's `register_type_object` and
/// be refused there, hours later and with no model in hand.
fn type_object_octets(ast: &Ast, id: Id<TypeNode>) -> Option<Vec<u8>> {
    zerodds_types::from_ast::complete_type_object(ast, id)
        .ok()
        .map(zerodds_types::type_object::TypeObject::Complete)
        .and_then(|object| object.to_bytes_le().ok())
        .filter(|bytes| zerodds_types::type_object::TypeObject::from_bytes_le(bytes).is_ok())
}

/// The complete TypeObjects `id`'s own object refers to, transitively, in
/// the `to_bytes_le` form: what a peer — or a content filter that decodes
/// the sample — needs besides the root to rebuild the type. Empty for a
/// type without dependencies, and for one the closure cannot be built for.
fn type_object_dependency_octets(ast: &Ast, id: Id<TypeNode>) -> Vec<Vec<u8>> {
    let Ok((_, objects)) = crate::type_metadata::standard_metadata(ast, id) else {
        return Vec::new();
    };
    objects
        .into_iter()
        .filter(|object| matches!(object, zerodds_types::type_object::TypeObject::Complete(_)))
        .skip(1)
        .filter_map(|object| object.to_bytes_le().ok())
        .collect()
}

/// Whether `id`'s key holder has a member in it.
///
/// Asked of `keyhash::key_members_with` and not of the declaration's own
/// members: XTypes 1.3 §7.6.8 step 1 pulls the key members of a nested
/// aggregate up into the holder, so a structure whose only `@key` is on
/// a member type is keyed and a walk of its own facets says it is not.
fn is_keyed(ast: &Ast, id: Id<TypeNode>, encoding: Encoding) -> bool {
    let key = LayoutKey::data(encoding, Endianness::Big);
    zerodds_ast_layout::keyhash::key_members_with(
        ast,
        id,
        key.key_holder_nesting,
        key.key_holder_order,
    )
    .is_ok_and(|members| !members.is_empty())
}

/// `::a::b::C` — the C++ name of `id`, from the file root.
fn qualified_cpp_path(ast: &Ast, id: Id<TypeNode>) -> String {
    let path = zerodds_lang_rules::adapter::cpp::cpp_type_path(ast, id);
    let segments: Vec<&str> = path
        .segments()
        .iter()
        .map(zerodds_lang_cst::Ident::as_str)
        .collect();
    format!("::{}", segments.join("::"))
}

/// `a::b::C` — the IDL scoped name, which is what a peer matches a topic
/// type by.
///
/// Without the leading `::`: XTypes 1.3 §7.6.3 matches by strict string
/// equality, and Cyclone, Fast DDS and RTI all publish the name without
/// it. The same spelling `crates/zerodds-build` puts in the Rust
/// column's `DdsType::TYPE_NAME`.
pub(crate) fn qualified_idl_name(ast: &Ast, id: Id<TypeNode>, node: &TypeNode) -> String {
    ast.scoped_name_of(zerodds_ast::AnyId::Type(id, node.node_id))
        .map_or_else(
            || node.name.clone(),
            |scoped| {
                let text = scoped.to_string();
                text.strip_prefix("::").unwrap_or(&text).to_string()
            },
        )
}

/// Octets as the rows of a C++ array initialiser, twelve to a line.
fn octet_rows(bytes: &[u8]) -> String {
    let mut out = String::new();
    for row in bytes.chunks(12) {
        let octets: Vec<String> = row.iter().map(|byte| format!("0x{byte:02x}")).collect();
        let _ = writeln!(out, "    {},", octets.join(", "));
    }
    out
}

/// The line `language`'s preamble has to stand *after*, if it cannot
/// stand before the declarations.
///
/// One language answers `Some`. Python's materialiser opens the file
/// with the module docstring and `from __future__ import annotations`,
/// and §7 of the language reference puts a future statement ahead of
/// every statement but the docstring: an import written above it is
/// `SyntaxError: from __future__ imports must occur at the beginning of
/// the file`, which is not a warning and not recoverable. So the
/// preamble goes in after the materialiser's own import block, which
/// ends on `import typing` — the same anchor
/// `crates/endpoint-e2e/tests/python.rs` splices its runtime imports at,
/// for the same reason.
const fn preamble_anchor(language: Language) -> Option<&'static str> {
    match language {
        Language::Python => Some("\nimport typing\n"),
        _ => None,
    }
}

/// `body` with `lines` written in after the first `anchor`.
///
/// The whole body unchanged when `lines` is empty or `anchor` is not in
/// it: a preamble that cannot be placed is not silently dropped
/// somewhere else, and the compiler in that language names the missing
/// import.
fn splice(body: &str, anchor: &str, lines: &[String]) -> String {
    if lines.is_empty() {
        return body.to_string();
    }
    let Some(found) = body.find(anchor) else {
        return body.to_string();
    };
    let at = found + anchor.len();
    let mut out = String::with_capacity(body.len());
    out.push_str(&body[..at]);
    for line in lines {
        out.push_str(line);
        out.push('\n');
    }
    out.push_str(&body[at..]);
    out
}

/// What opens, and what closes, a one-line comment in `language`.
///
/// The header above is the first line of every generated file, so it has
/// to be a comment in the language it is written into. `//` is not one in
/// Python, Elixir, Julia, Nim, Ada, Ada 83, Lua or OCaml, and a file whose
/// first line does not parse is not source at all — the point of a
/// generator is that its output compiles. OCaml has no line comment form
/// at all, so its header opens and closes on the one line.
#[must_use]
const fn comment_delimiters(language: Language) -> (&'static str, &'static str) {
    match language {
        Language::Python | Language::Elixir | Language::Julia | Language::Nim => ("#", ""),
        Language::Ada | Language::Ada83 | Language::Lua => ("--", ""),
        Language::OCaml => ("(*", " *)"),
        _ => ("//", ""),
    }
}

/// The clause that has to stand before the declarations for the file to
/// be a translation unit in `language`, if the language needs one.
///
/// Most languages take a bare sequence of declarations: Rust, C, C++,
/// Java (the materialiser emits the outer `ZdTypes` class itself),
/// TypeScript, Python, Swift, Kotlin, D, Zig, Julia, Lua, Nim, OCaml and
/// the two JavaScript targets all compile a file that opens on a
/// declaration. Go and F# do not — `go build` rejects a file with no
/// `package` clause, and `fsc` rejects one whose first construct is not
/// `module` or `namespace`.
///
/// # `rec`, and why F#'s clause is three words
///
/// F# resolves a name in the order the file writes it, and the F#
/// materialiser writes declarations in the model's own order: a `map`'s
/// entry class is declared where the `map` is reached, which is after
/// the aggregate whose member named it. As a plain `module` that is
/// `maponly.fs(7,20): error FS0039: the type ZdMapEntry_long_string is
/// not defined`, measured with `dotnet fsi` (SDK 9.0.106) — and the
/// same refusal is waiting for every recursive type and for the
/// mutually recursive codec functions beside them.
///
/// `module rec` is the language's own answer, and it is the one this
/// tree already relies on: `zerodds_lang_cst::fsharp`'s module
/// documentation states that `File::module_` is *always* rendered
/// `module rec <name>` for exactly this reason, and
/// `zerodds_lang_render::fsharp::file` writes it. Only this tool
/// assembled the clause itself and dropped the word, which is why the
/// F# column emitted a file no dependency order could have saved. The
/// fix is the keyword and not a reordering of the declarations: a walk
/// that sorted the entry classes to the front would still leave
/// `struct Node { sequence<Node> kids; }` and its self-recursive codec
/// unresolvable.
#[must_use]
const fn module_clause(language: Language) -> Option<&'static str> {
    match language {
        Language::Go => Some("package zdgen"),
        Language::FSharp => Some("module rec ZdGen"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
    use super::*;

    /// Compose one IDL source into a model.
    fn model(source: &str) -> Ast {
        let parsed = zerodds_idl_parse::parse(source);
        assert!(!parsed.has_errors(), "the fixture does not parse: {source}");
        zerodds_idl_lower::lower(&parsed.syntax(), "test.idl").ast
    }

    /// One model as C++, XCDR2 little-endian — what `--lang c++` with no
    /// other flag writes.
    fn cpp(source: &str) -> String {
        super::source(
            &model(source),
            Language::Cpp,
            "test.idl",
            LayoutKey::data(Encoding::PlainCdr2, Endianness::Little),
        )
        .expect("the C++ column takes this model")
    }

    /// `--rpc-mapping` reaches the emitter: the same interface is written as
    /// the Basic topic types or the Enhanced ones.
    #[test]
    fn the_rpc_mapping_selects_what_an_interface_is_written_as() {
        let ast = model("module m { interface Pinger { long count(); }; };");
        let key = LayoutKey::data(Encoding::PlainCdr2, Endianness::Little);
        let text = |mapping| {
            super::source_with_preamble_mapping(
                &ast,
                Language::Rust,
                "test.idl",
                key,
                CodecOptions::default(),
                None,
                mapping,
            )
            .expect("the Rust column takes this model")
        };
        let basic = text(ServiceMapping::Basic);
        let enhanced = text(ServiceMapping::Enhanced);
        assert_ne!(basic, enhanced);
        let unknown = "REMOTE_EX_UNKNOWN_OPERATION";
        assert!(!basic.contains(unknown), "Basic has no Enhanced dispatcher");
        assert!(
            enhanced.contains(unknown),
            "Enhanced answers an unknown operation"
        );
    }

    const TEMPERATURE: &str = "module sensor_msgs { module msg { struct Temperature { long \
                               celsius; string sensor_id; }; }; };";

    /// `--cpp-mapping psm` (the default) leaves the native types in `zd_native`, declares the PSM
    /// classes under the IDL's own names, and gives them a `topic_type_support` that calls the
    /// native codec through `zerodds::psm::Conv`. `--cpp-mapping native` has none of it.
    #[test]
    fn the_psm_mapping_is_a_layer_over_the_native_types() {
        let default = cpp(TEMPERATURE);
        assert!(default.contains("namespace zd_native"), "{default}");
        assert!(default.contains("zerodds::psm::Conv"), "{default}");

        let native = {
            let _scope = zerodds_lang_rules::adapter::cpp::CppMappingScope::enter(
                zerodds_lang_rules::adapter::cpp::CppMapping::Native,
            );
            cpp(TEMPERATURE)
        };
        assert!(!native.contains("zd_native"), "{native}");
        assert!(!native.contains("zerodds::psm::Conv"), "{native}");

        let _scope = zerodds_lang_rules::adapter::cpp::CppMappingScope::enter(
            zerodds_lang_rules::adapter::cpp::CppMapping::Psm,
        );
        let psm = cpp(TEMPERATURE);
        assert!(psm.contains("namespace zd_native"), "{psm}");
        assert!(psm.contains("class Temperature {"), "{psm}");
        assert!(psm.contains("int32_t celsius() const"), "{psm}");
        assert!(
            psm.contains("const std::string& sensor_id() const"),
            "{psm}"
        );
        assert!(psm.contains("void swap(Temperature& other)"), "{psm}");
        assert!(
            psm.contains("struct Conv<::sensor_msgs::msg::Temperature>"),
            "{psm}"
        );
        assert!(
            psm.contains("struct topic_type_support<::sensor_msgs::msg::Temperature>"),
            "{psm}"
        );
        assert!(
            psm.contains("struct topic_type_support<::zd_native::sensor_msgs::msg::Temperature>"),
            "{psm}"
        );
        assert!(
            psm.contains("#include \"zerodds/psm/convert.hpp\""),
            "{psm}"
        );
    }

    /// The types Table 7.1 and §8.1.4 name.
    #[test]
    fn the_psm_mapping_spells_the_types_of_table_7_1() {
        let _scope = zerodds_lang_rules::adapter::cpp::CppMappingScope::enter(
            zerodds_lang_rules::adapter::cpp::CppMapping::Psm,
        );
        let psm = cpp(
            "struct S { wchar c; wstring w; long double d; long a[2][3]; map<string, long> m; \
             @optional long o; @Shared sequence<octet> p; };",
        );
        for wanted in [
            "wchar_t c() const",
            "const std::wstring& w() const",
            "long double d() const",
            "::dds::core::array<::dds::core::array<int32_t, 3>, 2>",
            // §7.12: an array parameter is a const reference.
            "const ::dds::core::array<::dds::core::array<int32_t, 3>, 2>& a",
            "std::map<std::string, int32_t>",
            "::dds::core::optional<int32_t>",
            // §8.1.5: a `@Shared` attribute is a pointer. `@Shared` is the name DDS-XTypes 1.0
            // Beta 1 gave what XTypes 1.3 calls `@external`, whose C++ binding for the PSM is
            // `dds::core::external<T>` (§7.5.1.2.3.3).
            "::dds::core::external<std::vector<uint8_t>>",
        ] {
            assert!(psm.contains(wanted), "`{wanted}` is missing from:\n{psm}");
        }
    }

    /// Every member `crates/cpp/include/zerodds/dds.hpp` asks the trait
    /// for is one the generated C++ declares.
    ///
    /// The list is **read out of that header**, not spelled here: it is
    /// the published interface, and a generator held to a copy of it
    /// diverges the moment the header gains a member. The failure that
    /// makes this test exist is `zerodds-examples/cpp-typed` — five
    /// clang errors, all of them `no member named … in
    /// 'dds::topic::topic_type_support<…>'`, over a header that had
    /// `type_name`, `is_keyed` and a three-argument `decode` and nothing
    /// else.
    #[test]
    fn the_cpp_trait_carries_every_member_dds_hpp_asks_for() {
        let header = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../crates/cpp/include/zerodds/dds.hpp");
        let text = std::fs::read_to_string(&header)
            .unwrap_or_else(|error| panic!("{}: {error}", header.display()));

        // `traits::` is the alias both `TypedWriter<T>` and
        // `TypedReader<T>` name the specialisation through. The name is
        // half of it; the other half is how many arguments the call
        // passes, because that is the half `zerodds-examples/cpp-typed`
        // fell over on — `decode` was there and took three.
        let mut wanted: Vec<(String, usize)> = Vec::new();
        let mut from = 0;
        while let Some(found) = text[from..].find("traits::") {
            let at = from + found + "traits::".len();
            let end = at
                + text[at..]
                    .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                    .unwrap_or(text.len() - at);
            let name = text[at..end].to_string();
            from = at;
            if name.is_empty() || !text[end..].starts_with('(') {
                continue;
            }
            let call = (name, arguments(&text, end));
            if !wanted.contains(&call) {
                wanted.push(call);
            }
        }
        assert!(
            wanted.len() >= 6,
            "dds.hpp named only {wanted:?}; the header is the source of this list and it \
             should hold at least the six members TypedWriter and TypedReader call"
        );

        let generated = cpp(TEMPERATURE);
        for (name, arity) in &wanted {
            // A `static` member of the specialisation and not a mention
            // anywhere: the trait is what `dds.hpp` instantiates, and a
            // free function of the same name would not answer for it.
            let declared = generated
                .match_indices(&format!("{name}("))
                .find(|(at, _)| {
                    let line = generated[..*at].rsplit('\n').next().unwrap_or("");
                    line.trim_start().starts_with("static ")
                })
                .map(|(at, token)| arguments(&generated, at + token.len() - 1));
            assert_eq!(
                declared,
                Some(*arity),
                "`dds.hpp` calls `traits::{name}` with {arity} argument(s), and the \
                 generated `topic_type_support` answers with {declared:?}:\n{generated}"
            );
        }
    }

    /// How many arguments or parameters the parenthesis at `open` holds.
    ///
    /// Depth-one commas, so `decode(raw, len, version, be != 0)` is four
    /// and an empty list is zero. Every list on both sides of this test
    /// is a flat one — no template argument and no nested call carries a
    /// comma through here — and a list that grew one would be counted
    /// too high and fail loudly rather than pass by accident.
    fn arguments(text: &str, open: usize) -> usize {
        assert_eq!(text.as_bytes().get(open), Some(&b'('), "not a parenthesis");
        let mut depth = 0_usize;
        let mut commas = 0_usize;
        let mut seen = false;
        for (at, byte) in text.bytes().enumerate().skip(open) {
            match byte {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        return usize::from(seen) + commas;
                    }
                }
                b',' if depth == 1 => commas += 1,
                byte if depth == 1 && !byte.is_ascii_whitespace() => seen = true,
                _ => {}
            }
            let _ = at;
        }
        panic!("unbalanced parenthesis at {open}");
    }

    /// The specialisation names the type the way a peer matches it and
    /// the way C++ resolves it — two different spellings of one
    /// declaration.
    #[test]
    fn the_cpp_trait_names_the_idl_scoped_name_and_the_cpp_path() {
        let generated = cpp(TEMPERATURE);
        assert!(
            generated.contains("struct topic_type_support<::sensor_msgs::msg::Temperature> {"),
            "no specialisation at the C++ path:\n{generated}"
        );
        // Without the leading `::`, XTypes 1.3 §7.6.3.
        assert!(
            generated.contains(
                "static const char *type_name() { return \"sensor_msgs::msg::Temperature\"; }"
            ),
            "no IDL scoped name:\n{generated}"
        );
    }

    /// `extensibility()` answers what the model says, and the
    /// three answers are three different ones.
    ///
    /// `zerodds::TypedWriter<T>` hands this to
    /// `zerodds_writer_set_wire_extensibility`, which is what picks the
    /// encapsulation identifier — `PLAIN_CDR2_LE` 0x0007, `D_CDR2_LE`
    /// 0x0009 or `PL_CDR2_LE` 0x000b, XTypes 1.3 §7.6.3.1.2 Table 59.
    /// A generator that answered FINAL for everything would put an
    /// `@appendable` body, DHEADER and all, under the identifier for a
    /// body without one; measured on the wire, that is what happened
    /// before this member existed.
    #[test]
    fn the_cpp_trait_reports_the_models_extensibility() {
        for (annotation, kind) in [
            ("@final", "FINAL"),
            ("@appendable", "APPENDABLE"),
            ("@mutable", "MUTABLE"),
        ] {
            let generated = cpp(&format!("{annotation} struct S {{ @id(1) long a; }};"));
            assert!(
                generated.contains(&format!(
                    "return ::zerodds::psm::DataRepresentationKind::{kind};"
                )),
                "{annotation} did not reach `extensibility()`:\n{generated}"
            );
        }
        // The default with no annotation is the one a topic type carries
        // unless it says otherwise, and it is not FINAL: XTypes 1.3
        // §7.3.1.2.1.1 makes `@appendable` the default extensibility, so
        // the untagged case is exactly the one the missing call got
        // wrong.
        let bare = cpp("struct S { long a; };");
        assert!(
            bare.contains("return ::zerodds::psm::DataRepresentationKind::APPENDABLE;"),
            "the default extensibility is not what the model answers:\n{bare}"
        );
    }

    /// `encode` and `decode` call the codec that stands above them in
    /// the same file, under the names the emitter gave it.
    ///
    /// Asked of `codec_name` and `decoder_name` rather than of a string
    /// spelled here: the two would drift, and the failure would be a
    /// trait calling a function the file does not hold.
    #[test]
    fn the_cpp_trait_calls_the_codec_the_emitter_named() {
        let ast = model(TEMPERATURE);
        let id = ast
            .types()
            .find(|(_, node)| node.name == "Temperature")
            .expect("the model declares Temperature")
            .0;
        let encoder = zerodds_lang_rules::emit::encode::codec_name::<
            zerodds_lang_rules::emit::cpp::Cpp,
        >(&ast, id, zerodds_ast_layout::key::Purpose::Data);
        let decoder = zerodds_lang_rules::emit::decode::decoder_name::<
            zerodds_lang_rules::emit::cpp::Cpp,
        >(&ast, id);
        let generated = cpp(TEMPERATURE);
        assert!(
            generated.contains(&format!("{encoder}(w, v);")),
            "the trait's `encode` does not call `{encoder}`:\n{generated}"
        );
        assert!(
            generated.contains(&format!(
                "{decoder}(r, v, zerodds::wire::TryConstruct::Discard);"
            )),
            "the trait's `decode` does not call `{decoder}`:\n{generated}"
        );
    }

    /// `is_keyed()` follows the model's `@key`, and follows it through a
    /// nested aggregate — XTypes 1.3 §7.6.8 step 1 pulls a member type's
    /// key members up into the holder.
    #[test]
    fn is_keyed_follows_the_key_holder_and_not_the_declarations_own_members() {
        let keyless = cpp("struct Plain { long a; };");
        assert!(
            keyless.contains("static constexpr bool is_keyed() { return false; }"),
            "a model with no `@key` is keyed:\n{keyless}"
        );
        let keyed = cpp("struct Keyed { @key long id; string payload; };");
        assert!(
            keyed.contains("static constexpr bool is_keyed() { return true; }"),
            "a `@key` member did not reach `is_keyed`:\n{keyed}"
        );
        // §7.6.8 step 1 walks *into* a `@key` member of aggregate type
        // and takes its key members. A walk of `Outer`'s own facets
        // finds a `@key` and stops; the holder is what says which
        // octets it is made of, and it is what this asks.
        let nested = cpp(
            "@nested struct Inner { @key long id; long slack; }; struct Outer { @key Inner \
             inner; long payload; };",
        );
        assert!(
            nested.contains("static constexpr bool is_keyed() { return true; }"),
            "a nested aggregate's key did not reach the holder:\n{nested}"
        );
    }

    /// The `TypeObject` octets are the ones `zerodds_types::from_ast`
    /// lowers, byte for byte.
    ///
    /// The whole point of the C++ column reaching for that function is
    /// that there is one lowering of one model: an array assembled some
    /// other way would be a second answer to §7.3.4 that no byte
    /// comparison in this tree looks at.
    #[test]
    fn the_cpp_type_object_is_the_blob_from_ast_lowers() {
        let ast = model(TEMPERATURE);
        let id = ast
            .types()
            .find(|(_, node)| node.name == "Temperature")
            .expect("the model declares Temperature")
            .0;
        let bytes = type_object_octets(&ast, id).expect("Temperature has a CompleteTypeObject");
        assert!(!bytes.is_empty());
        let generated = cpp(TEMPERATURE);
        assert!(
            generated.contains(
                "inline constexpr std::uint8_t zd_type_object_sensor_msgs_msg_Temperature[] = {"
            ),
            "no TypeObject array:\n{generated}"
        );
        for row in octet_rows(&bytes).lines() {
            assert!(
                generated.contains(row),
                "the emitted array does not carry `{row}`:\n{generated}"
            );
        }
        assert!(
            generated.contains(
                "static const std::uint8_t *type_object() { return \
                 zd_type_object_sensor_msgs_msg_Temperature; }"
            ),
            "`type_object()` does not point at the array:\n{generated}"
        );
        assert!(
            generated.contains(
                "static std::size_t type_object_len() { return \
                 sizeof(zd_type_object_sensor_msgs_msg_Temperature); }"
            ),
            "`type_object_len()` is not the array's own size:\n{generated}"
        );
    }

    /// A `@nested` declaration is a member type and not a topic type, so
    /// it gets no specialisation — and the file says so where one would
    /// have stood.
    #[test]
    fn a_nested_declaration_gets_no_specialisation() {
        let generated = cpp("@nested struct Inner { long a; }; struct Outer { Inner inner; };");
        assert!(
            generated.contains("struct topic_type_support<::Outer> {"),
            "the topic type has no specialisation:\n{generated}"
        );
        assert!(
            !generated.contains("struct topic_type_support<::Inner> {"),
            "a `@nested` type got a specialisation:\n{generated}"
        );
        assert!(
            generated.contains("`Inner`: `@nested`"),
            "nothing says why `Inner` has none:\n{generated}"
        );
    }

    /// The trait's `decode` reads the version the file was not generated
    /// for through that version's own codec, never through this one's.
    #[test]
    fn the_cpp_trait_reads_each_version_through_its_own_codec() {
        let two = cpp(TEMPERATURE);
        assert!(
            two.contains("if (::zerodds::wire::is_v1(version)) {"),
            "an XCDR2 file does not set XCDR1 apart:\n{two}"
        );
        assert!(
            two.contains("namespace xcdr1 {") && two.contains("::xcdr1::zd_dec_"),
            "an XCDR2 file reads XCDR1 through no codec of its own:\n{two}"
        );
        let one = super::source(
            &model(TEMPERATURE),
            Language::Cpp,
            "test.idl",
            LayoutKey::data(Encoding::PlainCdr1, Endianness::Little),
        )
        .expect("the C++ column takes this model under XCDR1");
        assert!(
            one.contains("if (::zerodds::wire::is_v2(version)) {"),
            "an XCDR1 file does not set XCDR2 apart:\n{one}"
        );
        assert!(
            one.contains("namespace xcdr2 {") && one.contains("::xcdr2::zd_dec_"),
            "an XCDR1 file reads XCDR2 through no codec of its own:\n{one}"
        );
        // MAXALIGN is the version's, XTypes 1.3 §7.4.1: four under
        // XCDR2, eight under XCDR1. A reader built at the other one
        // reads every member after the first from the wrong offset.
        assert!(two.contains("zerodds::wire::Writer w(zerodds::wire::Endianness::Little, 4);"));
        assert!(one.contains("zerodds::wire::Writer w(zerodds::wire::Endianness::Little, 8);"));
    }

    /// `encode` is little-endian under `--endian be` as well, because
    /// `zerodds_writer_write` stamps a little-endian encapsulation
    /// identifier the caller does not choose. The preamble constant
    /// still carries the flag's own answer.
    #[test]
    fn the_cpp_traits_encode_stays_little_endian_under_endian_be() {
        let big = super::source(
            &model(TEMPERATURE),
            Language::Cpp,
            "test.idl",
            LayoutKey::data(Encoding::PlainCdr2, Endianness::Big),
        )
        .expect("the C++ column takes this model big-endian");
        assert!(
            big.contains(
                "constexpr zerodds::wire::Endianness zd_endianness = \
                          zerodds::wire::Endianness::Big;"
            ),
            "the flag did not reach the preamble:\n{big}"
        );
        assert!(
            big.contains("zerodds::wire::Writer w(zerodds::wire::Endianness::Little, 4);"),
            "the trait's `encode` follows `--endian`, which the FFI's own \
             encapsulation identifier contradicts:\n{big}"
        );
    }

    /// The header the specialisations need is included exactly when
    /// there are specialisations, and no second codec header is: the
    /// specialisations' `decode` takes `zerodds::wire::Encoding`, already
    /// reachable through the unconditional `zerodds/wire.hpp` include. C++
    /// has no unused-include diagnostic, so the wrong direction here is
    /// silent: a file with no topic type in it would carry a dependency on
    /// `crates/cpp/include` that nothing in it justifies.
    #[test]
    fn the_topic_trait_headers_are_included_only_where_a_trait_is_emitted() {
        let with = cpp(TEMPERATURE);
        assert!(with.contains("#include \"dds/topic/TopicTraits.hpp\""));
        assert!(!with.contains("#include \"dds/topic/xcdr2.hpp\""));
        // An enumeration is not an aggregate and is not a topic type.
        let without = cpp("enum Colour { red, green };");
        assert!(
            !without.contains("dds/topic/TopicTraits.hpp"),
            "a model with no topic type included the trait header:\n{without}"
        );
        assert!(
            !without.contains("topic_type_support"),
            "a model with no topic type got a specialisation:\n{without}"
        );
    }

    /// No other language grows a `topic_type_support`. The trait is
    /// `zerodds/dds.hpp`'s, and that header is C++.
    #[test]
    fn only_the_cpp_column_emits_topic_type_support() {
        let ast = model(TEMPERATURE);
        for language in Language::ALL {
            let traits = topic_traits(&ast, language, Encoding::PlainCdr2, None, "zd_temperature");
            if language == Language::Cpp {
                assert!(!traits.is_empty(), "the C++ column emitted none");
            } else {
                assert!(
                    traits.is_empty(),
                    "{} emitted a topic_type_support",
                    language.as_str()
                );
            }
        }
    }

    /// Every language `Language::ALL` carries parses back from its own
    /// `as_str`, and nothing outside that list does.
    #[test]
    fn every_language_round_trips_through_its_own_name() {
        for language in Language::ALL {
            assert_eq!(parse_language(language.as_str()), Some(language));
        }
        assert_eq!(parse_language("cobol"), None);
        assert_eq!(parse_language(""), None);
    }

    /// Every language's header line opens with a comment that language
    /// accepts, and closes it where the language has no line form.
    ///
    /// The header is line 1 of every generated file. `//` is not a
    /// comment in Python, Elixir, Julia, Nim, Ada, Ada 83, Lua or OCaml,
    /// and for a while every one of those files was emitted with it —
    /// `generate` reported "0 errors" over source whose first line does
    /// not parse.
    #[test]
    fn the_header_is_a_comment_in_the_language_it_is_written_into() {
        for language in Language::ALL {
            let (open, close) = comment_delimiters(language);
            let expected = match language {
                Language::Python | Language::Elixir | Language::Julia | Language::Nim => ("#", ""),
                Language::Ada | Language::Ada83 | Language::Lua => ("--", ""),
                Language::OCaml => ("(*", " *)"),
                _ => ("//", ""),
            };
            assert_eq!(
                (open, close),
                expected,
                "{} takes {expected:?}",
                language.as_str()
            );
        }
    }

    /// Go and F# are the two languages whose file is not a translation
    /// unit without a leading clause: `go build` refuses a file with no
    /// `package`, `fsc` a file whose first construct is not `module` or
    /// `namespace`. No other language on the list needs one.
    ///
    /// F#'s carries `rec`, and the word is the whole point: without it
    /// a declaration cannot name one written below it, which is what
    /// the materialiser's own order and every recursive type need. See
    /// [`module_clause`].
    #[test]
    fn only_go_and_fsharp_carry_a_module_clause() {
        for language in Language::ALL {
            let clause = module_clause(language);
            match language {
                Language::Go => assert_eq!(clause, Some("package zdgen")),
                Language::FSharp => assert_eq!(clause, Some("module rec ZdGen")),
                other => assert_eq!(clause, None, "{} wants no clause", other.as_str()),
            }
        }
    }

    /// `language_names` is what an unknown `--lang` is told to pick
    /// from, so every language has to be findable in it — spelled
    /// exactly as `parse_language` reads it back.
    #[test]
    fn language_names_lists_every_language_once() {
        let names = language_names();
        for language in Language::ALL {
            assert!(
                names.split(", ").any(|name| name == language.as_str()),
                "{} missing from {names}",
                language.as_str()
            );
        }
        assert_eq!(names.split(", ").count(), Language::ALL.len());
    }

    /// Nim imports `std/options` for an `@optional` member and nothing
    /// for the word "Table" in the defaulters' comment ("Table 9 of
    /// DDS-XTypes 1.3"), which every struct carries: the `std/tables` it
    /// used to import was unused in every generated file, a compiler
    /// warning each time. A `map` is a `seq` of entries.
    #[test]
    fn nim_imports_only_the_std_modules_it_names() {
        let nim = |source: &str| {
            super::source(
                &model(source),
                Language::Nim,
                "test.idl",
                LayoutKey::data(Encoding::PlainCdr2, Endianness::Little),
            )
            .expect("the Nim column takes this model")
        };
        let plain = nim("struct Pose { long x; };");
        assert!(plain.contains("Table 9"), "{plain}");
        assert!(!plain.contains("import std/"), "{plain}");
        let mapped = nim("struct Index { map<string, long> ids; };");
        assert!(!mapped.contains("import std/"), "{mapped}");
        let optional = nim("struct Reading { @optional long value; };");
        assert!(optional.contains("import std/options"), "{optional}");
    }
}
