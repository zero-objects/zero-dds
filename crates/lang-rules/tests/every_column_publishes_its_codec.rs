// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Can a caller outside the generated translation unit call the codec?
//!
//! Twenty-one columns, one question, one table. Three of them answered
//! **no**, and each was found separately, weeks apart, by somebody
//! reading the generated text for another reason:
//!
//! - **Java** wrote every codec `private static` inside a `public
//!   static class`.
//! - **TypeScript** wrote every codec `exported: false` — "a generated
//!   file is one module and its codec functions are its own".
//! - **Go** named every codec `zd_enc_…`, and Go's visibility *is* the
//!   name: an identifier is exported if and only if it begins with an
//!   upper-case letter.
//!
//! The three are one finding, and the finding is about the **harness**,
//! not about any of the three languages. Every probe column writes its
//! entry point into the same translation unit as the generated code —
//! `wireprobe-zerodds-<lang>-gen` appends a `main` — so a private codec
//! and a public one look identical to it. Nothing in the tree asked the
//! question a consumer asks, which is why the answer was wrong in three
//! places at once and stayed wrong.
//!
//! So the question is asked here, of all twenty-one, from one table.
//!
//! # What each column has to show
//!
//! [`publication`] is that table: one arm per language, each naming the
//! rule that language's own specification gives for reaching a name
//! from outside the unit, and the substring that rule appears as in the
//! generated text. The match is exhaustive over
//! `zerodds_lang_cst::Language`, so a twenty-second column cannot be
//! added without answering the question for it.
//!
//! Four languages have **no visibility concept at this level** and say
//! so by name rather than by omission — the answer is trivially yes and
//! the row still exists, because a row that is not there is a row
//! nobody decided:
//!
//! - **Julia** — the generated file is a script; `include` puts every
//!   name into the caller's namespace, and `export` only affects
//!   `using`.
//! - **OCaml** — no `.mli` accompanies the `.ml`, and without an
//!   interface every top-level binding is visible.
//! - **D** and **Kotlin** — the default access of a module-level
//!   declaration is public in both.
//!
//! **C and C++** publish through inclusion: the generated file is a
//! header, and `static` there gives every includer its own copy rather
//! than hiding the function. Measured on 2026-09-06 — a second
//! translation unit that includes the header calls `zd_enc_Max` and
//! links, at the cost of one `-Wunused-function` per includer that does
//! not call every codec in it. That cost is real and is not this test's
//! question; what this row holds is that the definition is in the
//! header a consumer includes.
//!
//! **Ada and Ada 83** publish through the *visible part of a package
//! specification* and nowhere else. A profile in a package **body**'s
//! declarative part is legal Ada and declares nothing outside the unit
//! (RM 7.1, RM83 7.1) — the Ada form of the `private static` the Java
//! column was measured out of. So the substring alone is not the
//! question for these two: the question is *where* it stands. Both
//! columns now emit the frame — `package Zd_Types is … end Zd_Types;`
//! followed by `package body Zd_Types is …`, and `Zd83_Types` for
//! Ada 83 — and the marker is looked for between the specification's
//! own `is` and its own `end`.
//!
//! **Java and C#** carry the identical shape one level out: a `static`
//! method is reachable only through the type it is declared in, so
//! `public static void zd_enc_Max(` inside a package-private `class
//! ZdCodec` publishes nothing. Both columns therefore name their
//! container as well, and the marker is looked for inside it.
//!
//! That is what [`Publication::opens`] and [`Publication::closes`] are:
//! the region of the generated text a consumer can reach. Seventeen
//! columns leave both `None` — for them the whole generated text is
//! that region — and four name a container, exhaustively, in the same
//! table as the marker.
//!
//! # The mutation
//!
//! Any one of the six fixes reverted turns this red and names the
//! column: `Modifier::Public` back to `Private` in
//! `emit/csharp.rs`, `is_pub: true` back to `false` in `emit/zig.rs`,
//! `exported: true` back to `false` in `emit/nim.rs` or
//! `emit/typescript.rs`, `Visibility::Public` back to `Internal` in
//! `emit/swift.rs`, `exported_name` back to the identity in
//! `emit/go.rs`, or `node_exports` returning `None` in `generate.rs`.
//!
//! Four more reach the containers. In `generate::ada_source` and
//! `generate::ada83_source`, sending `Decl::Subprogram` into `bodies`
//! beside `Decl::SubprogramBody` leaves the profiles in the package
//! body — every routine still carries one, the text is still legal
//! Ada, and nothing outside the unit can name a single codec. In
//! `generate::codec`, dropping `Modifier::Public` from the `ZdCodec`
//! class does the same for Java (package access, JLS §6.6.1) and for
//! C# (`internal`, one assembly).

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::Ast;
use zerodds_ast_layout::{Encoding, Endianness, LayoutKey};
use zerodds_lang_cst::Language;

/// The one declaration every column is asked about.
///
/// A `@final struct` with a single `long`: the smallest model that
/// makes a codec, so what a column does with visibility is not mixed
/// with what it does with any construct.
const SOURCE: &str = "@final struct Max { long a; };";

/// How a caller outside the generated unit reaches the codec, and what
/// that looks like in the text.
#[derive(Clone, Copy)]
struct Publication {
    /// The rule, in the language's own terms. Printed on failure, so a
    /// red row says what is missing rather than only that a substring
    /// is absent.
    rule: &'static str,
    /// The opening of the construct that publishes the marker, verbatim,
    /// or `None` where the whole generated text is reachable.
    ///
    /// A container is not decoration: a name inside one is reachable
    /// only if the container itself is, so a column that emits one has
    /// to be asked about it. Ada and Ada 83 name the package
    /// specification, Java and C# the class the `static` methods are
    /// declared in.
    opens: Option<&'static str>,
    /// Where that construct ends, verbatim, or `None` where it runs to
    /// the end of the text. Searched from the end of [`Publication::opens`].
    closes: Option<&'static str>,
    /// What the rule appears as, verbatim, in the generated source —
    /// and, where a container is named, inside that container.
    marker: &'static str,
}

/// The part of `source` a caller outside the generated unit can reach.
///
/// `None` where the column names a container and the container is not
/// in the text at all: a missing `package Zd_Types is` or a `class
/// ZdCodec` that lost its `public` is a failure of its own, distinct
/// from a marker that is merely absent, and the caller says so.
fn reachable<'a>(source: &'a str, publication: &Publication) -> Option<&'a str> {
    let after = match publication.opens {
        None => source,
        Some(opens) => {
            let at = source.find(opens)?;
            &source[at + opens.len()..]
        }
    };
    Some(match publication.closes {
        None => after,
        Some(closes) => match after.find(closes) {
            Some(at) => &after[..at],
            None => after,
        },
    })
}

/// One arm per language, exhaustive by construction.
fn publication(language: Language) -> Publication {
    match language {
        // An inherent method is as visible as its `pub` and its type.
        Language::Rust => Publication {
            rule: "`pub` on the method (Rust reference, Visibility and Privacy)",
            opens: None,
            closes: None,
            marker: "pub fn encode(",
        },
        // Through inclusion; see the module doc.
        Language::Cpp => Publication {
            rule: "defined in the header a consumer includes",
            opens: None,
            closes: None,
            marker: "static void zd_enc_Max(",
        },
        Language::C => Publication {
            rule: "defined in the header a consumer includes",
            opens: None,
            closes: None,
            marker: "static void zd_enc_Max(",
        },
        Language::Java => Publication {
            rule: "`public` on the method and on the class it is declared in (JLS §6.6.1)",
            // The method is `public`, and the class it is declared in has
            // to be too — a package-private `class ZdCodec` publishes
            // nothing outside its package (JLS §6.6.1). `generate::codec`
            // writes the class; this is the row that asks about it.
            opens: Some("public class ZdCodec {"),
            closes: None,
            marker: "public static void zd_enc_Max(",
        },
        Language::CSharp => Publication {
            rule: "`public` on the method and on the class it is declared in (C# specification §7.5.2)",
            // As Java: a `static` method of an `internal` class is
            // reachable from one assembly only (C# specification §7.5.2),
            // and the class is `generate::codec`'s to write. The trailing
            // newline is the renderer's: the brace is on the next line.
            opens: Some("public static class ZdCodec\n"),
            closes: None,
            marker: "public static void zd_enc_Max(",
        },
        // A module-level `def` whose name does not begin with an
        // underscore, and no `__all__` narrowing the module.
        Language::Python => Publication {
            rule: "a module-level `def` with no leading underscore",
            opens: None,
            closes: None,
            marker: "\ndef zd_enc_Max(",
        },
        Language::TypeScript => Publication {
            rule: "`export` on the declaration (ECMAScript modules)",
            opens: None,
            closes: None,
            marker: "export function zd_enc_Max(",
        },
        // Go has no keyword: the name is the visibility.
        Language::Go => Publication {
            rule: "an upper-case first letter (Go specification, Exported identifiers)",
            opens: None,
            closes: None,
            marker: "func Zd_enc_Max(",
        },
        Language::OCaml => Publication {
            rule: "no `.mli`, so every top-level binding is visible",
            opens: None,
            closes: None,
            marker: "zd_enc_max (w :",
        },
        Language::Ada => Publication {
            rule: "the profile stands in the visible part of the package specification (RM 7.1)",
            // The visible part of the specification, and not the
            // package body after it: RM 7.1 admits a profile in either,
            // and only the first one declares anything outside the unit.
            //
            // `closes` cannot collide with the body's own `end Zd_Types;`:
            // `opens` is matched first, and `package body Zd_Types is` does
            // not contain `package Zd_Types is`.
            opens: Some("package Zd_Types is"),
            closes: Some("\nend Zd_Types;"),
            marker: "procedure zd_enc_Max (w : in out Zd_Wire.Writer; v : in Max);",
        },
        Language::D => Publication {
            rule: "module-level declarations are `public` by default",
            opens: None,
            closes: None,
            marker: "void zd_enc_Max(",
        },
        // `def` and not `defp`.
        Language::Elixir => Publication {
            rule: "`def` and not `defp`",
            opens: None,
            closes: None,
            marker: "def zd_enc_Max(",
        },
        Language::Julia => Publication {
            rule: "no visibility concept: `include` binds every name",
            opens: None,
            closes: None,
            marker: "function zd_enc_Max(",
        },
        // The module table `require` answers with.
        Language::Lua => Publication {
            rule: "a field of the table the module returns",
            opens: None,
            closes: None,
            marker: "function ZD.zd_enc_Max(",
        },
        Language::Nim => Publication {
            rule: "the export marker `*` (Nim manual, Export marker)",
            opens: None,
            closes: None,
            marker: "proc zd_enc_Max*(",
        },
        Language::Swift => Publication {
            rule: "`public`; Swift's default is `internal`, one module only",
            opens: None,
            closes: None,
            marker: "public func zd_enc_Max(",
        },
        Language::Zig => Publication {
            rule: "`pub`; without it a declaration is private to its file",
            opens: None,
            closes: None,
            marker: "pub fn zd_enc_Max(",
        },
        Language::Node => Publication {
            rule: "named in `module.exports`; CommonJS publishes nothing by default",
            opens: None,
            closes: None,
            marker: "module.exports = {",
        },
        Language::Ada83 => Publication {
            rule: "the profile stands in the visible part of the package specification (RM83 7.1)",
            // As Ada, RM83 7.1.
            opens: Some("package Zd83_Types is"),
            closes: Some("\nend Zd83_Types;"),
            marker: "procedure zd_enc_Max (w : in Zd_Wire83.Writer_Access; v : in Max);",
        },
        Language::FSharp => Publication {
            rule: "a `let` in a module is public unless marked otherwise",
            opens: None,
            closes: None,
            marker: "let zd_enc_Max (w:",
        },
        Language::Kotlin => Publication {
            rule: "a top-level `fun` is `public` by default",
            opens: None,
            closes: None,
            marker: "fun zd_enc_Max(",
        },
    }
}

/// The model, parsed and lowered.
fn model() -> Ast {
    let parsed = zerodds_idl_parse::parse(SOURCE);
    assert!(
        parsed.diagnostics().is_empty(),
        "the source does not parse: {:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "visibility.idl");
    assert!(
        lowered.diagnostics.is_empty(),
        "the source does not lower: {:?}",
        lowered.diagnostics
    );
    let mut ast = lowered.ast;
    ast.resolve_all_by_name().expect("resolve");
    ast
}

#[test]
fn every_column_publishes_its_codec() {
    let ast = model();
    let key = LayoutKey::data(Encoding::PlainCdr2, Endianness::Little);
    let mut refused = Vec::new();
    for language in Language::ALL {
        let source = zerodds_lang_rules::generate::source(&ast, language, key)
            .unwrap_or_else(|error| panic!("{}: {error:?}", language.as_str()));
        let publication = publication(language);
        let Publication {
            rule,
            opens,
            marker,
            ..
        } = publication;
        match reachable(&source, &publication) {
            None => refused.push(format!(
                "{}: {rule}\n    the construct that publishes it is not in the text at all: {}\n{source}",
                language.as_str(),
                opens.unwrap_or_default()
            )),
            Some(reach) if !reach.contains(marker) => refused.push(format!(
                "{}: {rule}\n    expected to find: {marker}\n    in what a caller can reach:\n{reach}\n\nthe whole text:\n{source}",
                language.as_str()
            )),
            Some(_) => {}
        }
    }
    assert!(
        refused.is_empty(),
        "{} of {} columns do not publish their codec:\n\n{}",
        refused.len(),
        Language::ALL.len(),
        refused.join("\n\n")
    );
}

/// The Node column names **every** function it declares, not the first
/// one.
///
/// A `module.exports` built from a hand-written list is a list that goes
/// stale; this one is built from the declarations themselves, and the
/// property that says so is that the decoder and the defaulter are in
/// it beside the encoder.
#[test]
fn the_node_export_list_is_built_from_the_declarations() {
    let ast = model();
    let source = zerodds_lang_rules::generate::source(
        &ast,
        Language::Node,
        LayoutKey::data(Encoding::PlainCdr2, Endianness::Little),
    )
    .expect("the Node column generates");
    let exports = source
        .lines()
        .find(|line| line.starts_with("module.exports"))
        .expect("the file ends with its export list");
    for name in ["zd_enc_Max", "zd_dec_Max", "zd_def_Max"] {
        assert!(
            exports.contains(name),
            "{name} is declared and not exported:\n{exports}"
        );
    }
}

/// The Go rename is a bijection, so two codecs cannot become one name.
///
/// The reason `exported_name` raises the first letter rather than
/// producing the idiomatic `ZdEncMax`: dropping the underscores maps
/// distinct generated names onto one, and two codecs under one name is
/// a collision. Here the two encoders of the model are distinct before
/// and after.
#[test]
fn the_go_column_keeps_two_codecs_two_names() {
    let ast = model();
    let source = zerodds_lang_rules::generate::source(
        &ast,
        Language::Go,
        LayoutKey::data(Encoding::PlainCdr2, Endianness::Little),
    )
    .expect("the Go column generates");
    assert!(source.contains("func Zd_enc_Max("), "{source}");
    assert!(source.contains("func Zd_enc_p_long("), "{source}");
    assert!(
        !source.contains("func zd_enc_"),
        "an unexported codec is left:\n{source}"
    );
}

/// Every Table 9 defaulter the Go column **calls** it also **declares**.
///
/// The property that broke while the rename was being made, and the one
/// no other gate held. `default_go::go_defaulters_called_by` finds the
/// defaulters to emit by matching the names in the emitted decoders
/// against a prefix; the prefix was the literal `"zd_def_"`, the names
/// had become `Zd_def_…`, nothing matched, and the column emitted no
/// defaulter at all. `go_end_to_end.rs` did not see it — it builds the
/// declarations and not the codec — and `go build` on a generated
/// package answered `undefined: Zd_def_Max`.
#[test]
fn the_go_column_declares_every_defaulter_it_calls() {
    let ast = model();
    let source = zerodds_lang_rules::generate::source(
        &ast,
        Language::Go,
        LayoutKey::data(Encoding::PlainCdr2, Endianness::Little),
    )
    .expect("the Go column generates");
    let called: Vec<String> = source
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|word| word.starts_with("Zd_def_"))
        .map(str::to_string)
        .collect();
    assert!(
        !called.is_empty(),
        "the decoders call no defaulter at all:\n{source}"
    );
    for name in &called {
        assert!(
            source.contains(&format!("func {name}(")),
            "{name} is called and not declared:\n{source}"
        );
    }
}

/// Every codec the Ada columns define, they also **declare**.
///
/// The Ada form of `the_node_export_list_is_built_from_the_declarations`,
/// and the property the split in `generate::ada_source` exists for. A
/// subprogram body in a package body needs no declaration in the
/// specification to be legal Ada — it is complete on its own — so the
/// text compiles either way and only a caller outside the unit can tell
/// the difference. When the split was made, the encoders carried a
/// profile and the 28 decoders and Table 9 defaulters did not, and GNAT
/// answered `"zd_dec_Max" not declared in "Zd_Types"`.
///
/// Read off the rendered text rather than the tree, because the text is
/// what a consumer compiles: a top-level body in the package body is a
/// line at the renderer's one indentation level that begins `procedure`
/// or `function` and ends ` is`, and its profile is the same line with
/// the ` is` replaced by `;`.
#[test]
fn the_ada_columns_declare_every_body_they_define() {
    let ast = model();
    let key = LayoutKey::data(Encoding::PlainCdr2, Endianness::Little);
    for (language, package) in [(Language::Ada, "Zd_Types"), (Language::Ada83, "Zd83_Types")] {
        let source = zerodds_lang_rules::generate::source(&ast, language, key)
            .unwrap_or_else(|error| panic!("{}: {error:?}", language.as_str()));
        let body_at = source
            .find(&format!("package body {package} is"))
            .unwrap_or_else(|| panic!("{}: no package body:\n{source}", language.as_str()));
        let (spec, body) = source.split_at(body_at);
        let mut defined = 0_usize;
        for line in body.lines() {
            let Some(profile) = line.strip_prefix("   ").and_then(|l| l.strip_suffix(" is")) else {
                continue;
            };
            if !(profile.starts_with("procedure ") || profile.starts_with("function ")) {
                continue;
            }
            defined += 1;
            assert!(
                spec.contains(&format!("   {profile};")),
                "{}: {profile} is defined in `package body {package}` and declared nowhere in \
                 `package {package}`, so no caller outside the unit can name it:\n{spec}",
                language.as_str()
            );
        }
        assert!(
            defined > 0,
            "{}: the package body defines no subprogram at all:\n{source}",
            language.as_str()
        );
    }
}
