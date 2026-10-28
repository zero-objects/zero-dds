// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-wireprobe-cpp-gen`. Safety classification: **TOOLING**.
//!
//! `wireprobe-zerodds-cpp-gen` — the C++ translation unit the C++
//! probe compiles.
//!
//! The C++ spelling and nothing else. [`zerodds_wireprobe_gen`] is the
//! stem: the command line, the IDL, the model, the layout key, the
//! sample, the files and the exit codes are there, once, for all five
//! columns.
//!
//! A `zd_types.hpp` of declarations and a `probe.cpp` of codec
//! functions, the statements that fill one sample, and a `main` that
//! writes the bytes to standard output.
//! `tools/wireprobe/wireprobe-zerodds-cpp` is the shell around it.
//!
//! Nothing here writes a byte. The declarations come from the C++
//! materialiser, the codec from the shared walk through
//! [`zerodds_lang_rules::emit::cpp::Cpp`], and the bytes come out of
//! `runtime/cpp` when the compiled program runs. `runtime/cpp`
//! implements DDS-XTypes 1.3 §7.4 from the clause and calls nothing in
//! Rust, which is what makes this a second reading of the specification
//! rather than a rendering of the first one's answer.
//!
//! ## Both directions
//!
//! `--values` writes a sample and `--read` reads one: `--read` builds
//! the decoder instead of the encoder and a `main` that takes bytes on
//! standard input and prints the sample as canonical JSON, which is what
//! the C column's `--read` does and what
//! `docs/specs/zerodds-interop-suite-1.0.md` §4 P2 asks for — a byte
//! comparison passes a decoder that is wrong in the same way its own
//! encoder is wrong, and a round trip through two implementations does
//! not.
//!
//! The reading direction was refused for as long as
//! `CodecTarget::default_into` refused for C++, because five shapes need
//! DDS-XTypes 1.3 Table 9 to say what a member the stream did not carry
//! holds. [`zerodds_lang_rules::default_cpp`] is that table.
//!
//! # One translation unit for every form and both byte orders
//!
//! [`sources`] bakes the direction, the emit form and the byte order into
//! the program, so the whole corpus in every configuration is eight
//! binaries per plan. `wireprobe-cpp-matrix` runs this column over the
//! whole matrix in one process, and the cost is not the compiler: on
//! macOS the **first** execution of a freshly linked binary costs about
//! 235 ms of signature validation against 2 ms for every one after — the C
//! column's measurement, and the check does not parallelise either. So
//! [`combined_sources`] emits one program per case, configuration and
//! encoding, taking
//!
//! ```text
//! probe payload|payload+encapsulation|keyhash|read [le|be]
//! ```
//!
//! on its command line. `runtime/cpp`'s `Writer` and `Reader` both take
//! the order as a constructor argument, the encapsulation identifier is a
//! pure function of the encoding and the order, and all three codec sets
//! are in the file at once — so all four forms and both orders fit in one
//! unit. [`combined_sources`] argues the two that had to move, what is
//! kept, and the one place two plans write the same function.
#![forbid(unsafe_code)]

mod dump;
mod fill;

use std::collections::BTreeMap;
use std::fmt::Write as _;
use zerodds_ast::{Ast, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ast_layout::{Endianness, KeyHolderNesting, LayoutKey, PidFlagPolicy, Purpose};
use zerodds_lang_cst::cpp;
use zerodds_lang_rules::decode_cpp::{cpp_decoder_from, decoder_name};
use zerodds_lang_rules::emit::cpp::Cpp;
use zerodds_lang_rules::emit::encode::{codec_from, codec_name, key_holder};
use zerodds_lang_rules::encode::EncodeError;
use zerodds_lang_rules::rule;
use zerodds_lang_rules::{ModelGraph, apply};
pub use zerodds_wireprobe_gen::{Args, Emit};
use zerodds_wireprobe_gen::{Column, File, Refusal, Session, encapsulation_header, gap};

/// The probe's input buffer, and its cap on a payload.
///
/// The same megabyte the C column takes. A payload that does not fit is
/// reported rather than truncated: a probe that read the first megabyte
/// of a longer message would report a sample nobody sent.
const BUFFER_BYTES: usize = 1 << 20;

/// The C++ column.
#[derive(Debug, Clone, Copy)]
pub struct Renderer;

impl Column for Renderer {
    const BINARY: &'static str = "wireprobe-zerodds-cpp-gen";
    const LANGUAGE: &'static str = "C++";
    const OUTPUT: &'static str = "zd_types.hpp and probe.cpp go";
    const FORMS: &'static [Emit] = &[Emit::Payload, Emit::PayloadWithEncapsulation, Emit::KeyHash];

    fn render(session: &Session<'_>) -> Result<Vec<File>, Refusal> {
        let sources = sources(session)?;
        Ok(vec![
            File::new("zd_types.hpp", sources.types),
            File::new("probe.cpp", sources.probe),
        ])
    }

    fn combined(session: &Session<'_>, with_read: bool) -> Result<Vec<File>, Refusal> {
        let sources = combined_sources_from(session, with_read)?;
        Ok(vec![
            File::new("zd_types.hpp", sources.types),
            File::new("probe.cpp", sources.probe),
        ])
    }
}

/// The two C++ files.
#[derive(Debug)]
pub struct Sources {
    /// The declarations.
    pub types: String,
    /// The codec, the fill and `main`.
    pub probe: String,
}

/// The declarations, which every form of one generation shares.
///
/// Split out of [`sources`] because the combined unit below builds the
/// same header and nothing else of it: the types do not depend on the
/// direction, the emit form or the byte order, only on the model.
///
/// # Errors
/// [`Refusal::NoForm`] from the materialiser, and [`Refusal::Gap`] from
/// the rule catalogue.
fn declarations(session: &Session<'_>) -> Result<String, Refusal> {
    let ast = &session.ast;
    let graph = ModelGraph::of(ast);
    let applied = apply(&graph, &rule::cpp::catalogue()).map_err(|error| gap!("{error:?}"))?;
    // The materialiser is the authority on what C++ has a form for, so
    // its refusal is exit 3 and every other stop is exit 6.
    let decls = zerodds_lang_rules::adapter::cpp::cpp_items(ast, &applied)
        .map_err(|error| Refusal::NoForm(format!("{error:?}")))?;
    Ok(zerodds_lang_render::cpp::file(
        &zerodds_lang_cst::cpp::File {
            doc: zerodds_lang_cst::common::Doc::lines(&[
                "Generated by wireprobe-zerodds-cpp-gen. Do not edit.",
            ]),
            pragma_once: true,
            includes: zerodds_lang_rules::adapter::cpp::includes(),
            decls,
        },
    ))
}

/// Generate both files.
///
/// # Errors
/// [`Refusal`], classified so the command can answer in the verdict's
/// own exit code.
pub fn sources(session: &Session<'_>) -> Result<Sources, Refusal> {
    // The probe reads the fields of the native structures: the wire column asks for `native`
    // (the default of the library is the DDS-PSM-Cxx mapping).
    let _native = zerodds_lang_rules::adapter::cpp::CppMappingScope::enter(
        zerodds_lang_rules::adapter::cpp::CppMapping::Native,
    );
    let ast = &session.ast;
    let root = session.root;
    let types = declarations(session)?;

    let key = session.key;
    // One direction's codec only. A translation unit holding both would
    // carry a `static` function nothing calls, which is an error under
    // the `-Werror` the probe is compiled with — and it is worth
    // keeping: normally an uncalled codec means the generator emitted
    // the wrong thing.
    let members = if session.args.read {
        cpp_decoder_from(ast, root, key, session.codec_options)
            .map_err(|error| from_codec(&error))?
    } else if session.args.emit == Emit::KeyHash {
        key_hash_codec(session)?
    } else {
        codec_from::<Cpp>(ast, root, key, session.codec_options)
            .map_err(|error| from_codec(&error))?
    };
    // Free functions in the probe's translation unit, which is what C++
    // has and Java does not.
    let codec = zerodds_lang_render::cpp::file(&zerodds_lang_cst::cpp::File {
        doc: zerodds_lang_cst::common::Doc::none(),
        pragma_once: false,
        includes: Vec::new(),
        decls: members,
    });

    let probe = if session.args.read {
        read_harness(session, &codec)?
    } else if session.args.emit == Emit::KeyHash {
        let filled = fill::fill(ast, root, session.sample()?)?;
        key_hash_harness(session, &codec, filled.statements())?
    } else {
        let filled = fill::fill(ast, root, session.sample()?)?;
        harness(session, &codec, filled.statements())?
    };
    Ok(Sources { types, probe })
}

/// One translation unit that does all four things, chosen at run time.
///
/// [`sources`] bakes the direction, the emit form and the byte order into
/// the program, so the whole corpus in every configuration is eight
/// binaries per plan. The batch driver behind `wireprobe-cpp-matrix`
/// compiles and runs tens of thousands of them, and the cost is not the
/// compiler: on macOS the **first** execution of a freshly linked binary
/// costs about 235 ms of signature validation against 2 ms for every one
/// after — the C column's measurement, and the check does not parallelise
/// either. The binary count is what a matrix sweep costs.
///
/// So this emits one program per case, configuration and encoding. It
/// takes
///
/// ```text
/// probe payload|payload+encapsulation|keyhash|read [le|be]
/// ```
///
/// which collapses eight binaries into one.
///
/// Byte order can move out of the program because the plan does not
/// depend on it — DDS-XTypes 1.3 puts member offsets in the type and the
/// order in the stream, and `LayoutKey`'s own documentation says offsets
/// are invariant under it. `runtime/cpp` takes the order as a constructor
/// argument on both `Writer` and `Reader`, so the generated codec has
/// nothing to bake in. It is checked rather than assumed:
/// `tests/combined_unit.rs::the_byte_order_is_a_runtime_argument`
/// generates both orders under one plan and requires the two programs to
/// be identical.
///
/// The emit form moves out because this unit carries all three codec sets
/// at once and the form is a word on `main`'s command line. Not only
/// `main` differs from the single-form generator — `sources` builds a
/// *different* codec set for `keyhash` than for the payload — and here all
/// three are in the file: the payload encoder, which answers `payload`
/// and `payload+encapsulation` behind a flag; the key holder, which
/// answers `keyhash`; and the decoder, which answers `read`.
///
/// ## What is kept from the single-form generator
///
/// The payload codec is **not** pruned to what is reachable, for the
/// reason [`sources`] gives: an unreferenced element encoder under
/// `-Werror` is a defect detector, and a reachability pass would have
/// thrown the evidence away. Only the key holder's set is pruned, and only
/// because a holder legitimately leaves the encoders for non-key members
/// unused.
///
/// # Errors
/// [`Refusal`], classified so the caller can answer in the verdict's own
/// exit code.
pub fn combined_sources(args: &Args) -> Result<Sources, Refusal> {
    combined_sources_with(args, true)
}

/// [`combined_sources`], with the read direction left out.
///
/// The fallback for a case whose **decoder** does not compile. Putting
/// all four forms in one translation unit means one broken form takes the
/// other three with it, and a column that reported "no probe" for a
/// payload it can write perfectly well would understate itself and hide
/// the actual defect behind a bigger one. So the batch driver retries
/// without the decoder and files the read slots, and only those, under the
/// compiler's message.
///
/// # Errors
/// [`Refusal`], classified.
pub fn combined_sources_with(args: &Args, with_read: bool) -> Result<Sources, Refusal> {
    // The probe reads the fields of the native structures: the wire column asks for `native`
    // (the default of the library is the DDS-PSM-Cxx mapping).
    let _native = zerodds_lang_rules::adapter::cpp::CppMappingScope::enter(
        zerodds_lang_rules::adapter::cpp::CppMapping::Native,
    );
    let session = Session::open(args)?;
    combined_sources_from(&session, with_read)
}

/// Whether the type `args` names declares a `@key` member, so that it
/// has a KeyHash at all.
///
/// A type with no `@key` member has one instance and no KeyHash whatever
/// its sample holds (XTypes 1.3 §7.6.8). A unit whose C++ cannot be
/// generated for a reason in the sample — a code point the C++ carrier
/// cannot hold — still has that answer for its KeyHash slot, and the
/// Rust column gives it there: measured by `reference_columns.rs` as 120
/// KeyHash slots of `primitive-char-above-latin1` and
/// `primitive-wchar-above-bmp` that answered with the sample's refusal.
///
/// A model whose data codec cannot be planned — a type cycle nothing on
/// it terminates — is refused for that and not answered as keyless: the
/// Rust column refuses such a model before it asks for a key, measured
/// on `recursive-array`'s KeyHash slots.
///
/// # Errors
/// [`Refusal`], classified, when the model cannot be read or its data
/// codec cannot be planned.
pub fn declares_a_key(args: &Args) -> Result<bool, Refusal> {
    // The probe reads the fields of the native structures: the wire column asks for `native`
    // (the default of the library is the DDS-PSM-Cxx mapping).
    let _native = zerodds_lang_rules::adapter::cpp::CppMappingScope::enter(
        zerodds_lang_rules::adapter::cpp::CppMapping::Native,
    );
    let session = Session::open(args)?;
    codec_from::<Cpp>(
        &session.ast,
        session.root,
        session.key,
        session.codec_options,
    )
    .map_err(|error| from_codec(&error))?;
    Ok(
        key_holder::<Cpp>(&session.ast, session.root, session.holder_key)
            .map_err(|error| from_codec(&error))?
            .is_some(),
    )
}

/// [`combined_sources_with`], on a session the stem has already opened.
///
/// The batch driver opens one per translation unit and the command opens
/// one per run; both end here, so the IDL is read the same way in either.
///
/// # Errors
/// [`Refusal`], classified.
pub fn combined_sources_from(session: &Session<'_>, with_read: bool) -> Result<Sources, Refusal> {
    // The probe reads the fields of the native structures: the wire column asks for `native`
    // (the default of the library is the DDS-PSM-Cxx mapping).
    let _native = zerodds_lang_rules::adapter::cpp::CppMappingScope::enter(
        zerodds_lang_rules::adapter::cpp::CppMapping::Native,
    );
    let ast = &session.ast;
    let root = session.root;
    let types = declarations(session)?;

    let key = session.key;
    let holder_key = session.holder_key;
    let options = session.codec_options;

    let mut codec =
        codec_from::<Cpp>(ast, root, key, options).map_err(|error| from_codec(&error))?;
    if with_read {
        codec
            .extend(cpp_decoder_from(ast, root, key, options).map_err(|error| from_codec(&error))?);
    }

    // The holder, when the type has one. A keyless type is not an error
    // here — three of the four forms still work and the fourth reports
    // that there is no KeyHash to take, which is what every other column
    // answers for it.
    let holder_entry =
        match key_holder::<Cpp>(ast, root, holder_key).map_err(|error| from_codec(&error))? {
            Some(cpp::Decl::Function(function)) => {
                let entry = function.name.as_str().to_string();
                let mut under_holder = codec_from::<Cpp>(ast, root, holder_key, options)
                    .map_err(|error| from_codec(&error))?;
                // §7.6.8 Step 1 removes a nested aggregate's non-key members
                // only "if there are any key members"; RTI under XCDR1 reads
                // the consequence as "and below a keyless one, stop filtering
                // altogether", so the subtree there is written as the data has
                // it, and `keyhash::member_key` names `zd_enc_` at those
                // positions. Both sets have to be there for the file to
                // compile; [`deduplicate`] is what keeps the overlap from
                // becoming two definitions of one name.
                if holder_key.key_holder_nesting == KeyHolderNesting::KeylessNestedWhole {
                    let as_data = LayoutKey {
                        purpose: Purpose::Data,
                        ..holder_key
                    };
                    under_holder.extend(
                        codec_from::<Cpp>(ast, root, as_data, options)
                            .map_err(|error| from_codec(&error))?,
                    );
                }
                under_holder.push(cpp::Decl::Function(function));
                codec.extend(zerodds_lang_rules::default_cpp::reachable_from(
                    under_holder,
                    &entry,
                ));
                Some(entry)
            }
            Some(_) => return Err(gap!("the key holder is not a function")),
            None => None,
        };
    let codec = deduplicate(codec)?;

    // Free functions in the probe's translation unit, placed by the harness
    // after the includes that declare `zerodds::wire`, as `harness` does.
    let codec = zerodds_lang_render::cpp::file(&zerodds_lang_cst::cpp::File {
        doc: zerodds_lang_cst::common::Doc::none(),
        pragma_once: false,
        includes: Vec::new(),
        decls: codec,
    });
    let filled = fill::fill(ast, root, session.sample()?)?;
    let probe = combined_harness(
        session,
        &codec,
        filled.statements(),
        holder_entry.as_deref(),
        with_read,
    )?;
    Ok(Sources { types, probe })
}

/// Drop the second definition of anything defined twice, and refuse two
/// definitions of one name that are not the same text.
///
/// The three codec sets overlap: under
/// [`KeyHolderNesting::KeylessNestedWhole`] the holder's subtree is
/// written with the data-purpose encoders, which are the ones the payload
/// set already holds, and C++ has no implicit redeclaration to merge them
/// the way a prototype and its definition merge. Keeping both would be two
/// definitions of one name, which does not compile.
///
/// Two definitions of a name that differ is the case that must not pass
/// quietly — it would mean one of the two plans is being silently
/// dropped, and the probe would write bytes from a plan nobody chose.
///
/// # Errors
/// A message naming the function, as [`Refusal::Gap`].
fn deduplicate(decls: Vec<cpp::Decl>) -> Result<Vec<cpp::Decl>, Refusal> {
    let mut seen: BTreeMap<String, String> = BTreeMap::new();
    let mut out = Vec::with_capacity(decls.len());
    for decl in decls {
        let rendered = zerodds_lang_render::cpp::decl(&decl);
        // A prototype and the definition it announces are two decls with
        // one name, and both belong in the file — C++ resolves a call to a
        // free function by what is declared above it, which is what a
        // mutually recursive type needs. The body is part of the key so
        // that the pair is not read as a conflict.
        let name = match &decl {
            cpp::Decl::Function(function) => format!(
                "{}{}",
                function.name.as_str(),
                if matches!(function.body, cpp::FunctionBody::None) {
                    ";"
                } else {
                    "()"
                }
            ),
            _ => rendered.clone(),
        };
        match seen.get(&name) {
            Some(before) if *before == rendered => continue,
            Some(_) => {
                return Err(gap!(
                    "`{name}` is defined twice and the two definitions differ; one of the \
                     plans would be silently dropped"
                ));
            }
            None => {
                seen.insert(name, rendered);
                out.push(decl);
            }
        }
    }
    Ok(out)
}

/// `probe.cpp` for the combined program: the sample, the fill, one
/// function per form, and a dispatch on `argv`.
///
/// Written as text for the reason [`harness`] gives — this is the harness
/// *around* the generated code, and holding it as a CST would say it was
/// part of the output a user gets.
///
/// # The byte order, twice over
///
/// `runtime/cpp` takes the order as a constructor argument, so neither
/// the writer nor the reader has it baked in. The encapsulation
/// identifier does not: it is a pure function of the encoding and the
/// order ([`encapsulation_header`], the same one every other column
/// computes it with), so both are written out here and the program picks
/// the one its argument names. The KeyHash has no axis at all —
/// XTypes 1.3 §7.6.8 makes the holder big-endian whatever the sample is
/// — so `zd_keyhash` takes no order and is dispatched with the bare word.
#[allow(clippy::too_many_lines)]
fn combined_harness(
    session: &Session<'_>,
    codec: &str,
    fill: &str,
    holder_entry: Option<&str>,
    with_read: bool,
) -> Result<String, Refusal> {
    let args = session.args;
    let ast: &Ast = &session.ast;
    let root: Id<TypeNode> = session.root;
    let key: LayoutKey = session.key;
    let sample_type = zerodds_lang_render::cpp::type_(
        &<Cpp as zerodds_lang_rules::emit::CodecTarget>::value_type(ast, root),
    );
    let encoder = codec_name::<Cpp>(ast, root, Purpose::Data);
    let max_align = args.encoding.max_alignment();
    let framing = pl_cdr1_framing(key);
    let short_codes = key.length_codes.short_code_where_it_fits();
    let little = encapsulation_header(args.encoding, Endianness::Little);
    let big = encapsulation_header(args.encoding, Endianness::Big);
    // The identifier is one 16-bit word written big-endian whatever the
    // payload behind it is (DDSI-RTPS 2.5 §10.5), so its low byte is the
    // one the two orders differ in and the program picks between the two.
    // The **high** byte is one constant for both orders, and not an
    // assumption about this encoding: every identifier
    // `encapsulation_header` spells is below 0x0100, so it is always
    // zero. Bytes two and three are the options field, zero until the
    // padding count goes into its low two bits (§7.6.3.1.2).
    let high = big[0];
    let low_big = big[1];
    let low_little = little[1];

    let mut out = String::new();
    out.push_str(
        "// Generated by wireprobe-zerodds-cpp-gen. Do not edit.\n\
         //\n\
         // Every emit form and both byte orders, chosen on this program's\n\
         // command line. The bytes come out of runtime/cpp when this runs;\n\
         // nothing here decides a wire question.\n\n\
         #include \"zd_types.hpp\"\n\
         #include \"zerodds/wire.hpp\"\n\n\
         #include <cstdio>\n\
         #include <cstring>\n\
         #include <vector>\n\n",
    );
    if with_read {
        // The JSON dialect, shared with every other probe. `dumpjson.hpp`
        // adds the one helper a C++ binding needs on top of the C header:
        // a `long double` printed to every digit it has.
        out.push_str(
            "// The JSON dialect, shared with every other probe. `dumpjson.hpp`\n\
             // adds the one helper a C++ binding needs on top of the C header:\n\
             // a `long double` printed to every digit it has.\n\
             #include \"dumpjson.hpp\"\n\n",
        );
    }
    out.push_str(codec);
    out.push('\n');

    let _ = write!(
        out,
        "static {sample_type} zd_sample;\n\n\
         static void zd_fill() {{\n{fill}}}\n\n"
    );

    // -- the write direction -------------------------------------------
    out.push_str(&unwritable());
    let _ = write!(
        out,
        "static int zd_write(zerodds::wire::Endianness order, bool encapsulate) {{\n\
         \x20   zerodds::wire::Writer w(order, {max_align}, {framing}, {short_codes});\n\
         \x20   /* The encapsulation identifier is big-endian on the wire whatever\n\
         \x20      the payload behind it is, and it is outside the CDR stream —\n\
         \x20      alignment inside the payload still counts from zero. The options\n\
         \x20      field is zero here and gets the padding count below. The `static_cast` is\n\
         \x20      the narrowing an aggregate initialiser of `unsigned char` refuses. */\n\
         \x20   unsigned char zd_head[4] = {{ 0x{high:02x}, static_cast<unsigned char>(order == zerodds::wire::Endianness::Big ? 0x{low_big:02x} : 0x{low_little:02x}), 0x00, 0x00 }};\n\
         \x20   static const unsigned char zd_padding[3] = {{ 0, 0, 0 }};\n\
         \x20   std::size_t zd_pad;\n\
         \x20   try {{\n\
         \x20       {encoder}(w, zd_sample);\n\
         \x20   }} catch (const zerodds::wire::WireError &error) {{\n{}\
         \x20       return 1;\n\
         \x20   }}\n\
         \x20   const std::vector<std::uint8_t> zd_body = w.finish();\n\
         \x20   /* XTypes 1.3 §7.4.1.1 pads the serialized data to the next four-octet\n\
         \x20      boundary with zeros, and §7.6.3.1.2 states the count in the options\n\
         \x20      field so a receiver can find where the data ended. Both halves or\n\
         \x20      neither, and the header cannot go out before the encode because two\n\
         \x20      of its four bytes are the payload's length. `payload` alone stays\n\
         \x20      unpadded: that is the serialized data itself. */\n\
         \x20   zd_pad = (4u - (zd_body.size() & 3u)) & 3u;\n\
         \x20   zd_head[3] = static_cast<unsigned char>(zd_head[3] | zd_pad);\n\
         \x20   if (encapsulate && std::fwrite(zd_head, 1, 4, stdout) != 4) {{\n\
         \x20       return zd_unwritable();\n\
         \x20   }}\n\
         \x20   if (std::fwrite(zd_body.data(), 1, zd_body.size(), stdout) != zd_body.size()) {{\n\
         \x20       return zd_unwritable();\n\
         \x20   }}\n\
         \x20   if (encapsulate && std::fwrite(zd_padding, 1, zd_pad, stdout) != zd_pad) {{\n\
         \x20       return zd_unwritable();\n\
         \x20   }}\n\
         \x20   return 0;\n\
         }}\n\n",
        wire_error_catch()
    );

    // -- the KeyHash ----------------------------------------------------
    match holder_entry {
        Some(entry) => {
            let form = key_hash_form(session)?;
            let _ = write!(
                out,
                "static int zd_keyhash() {{\n\
                 \x20   /* The holder is PLAIN_CDR2 big-endian whatever the sample is,\n\
                 \x20      so its alignment cap is PLAIN_CDR2's. XTypes 1.3 §7.6.8. */\n\
                 \x20   zerodds::wire::Writer w(zerodds::wire::Endianness::Big, 4);\n\
                 \x20   try {{\n\
                 \x20       {entry}(w, zd_sample);\n\
                 \x20   }} catch (const zerodds::wire::WireError &error) {{\n{}\
                 \x20       return 1;\n\
                 \x20   }}\n\
                 \x20   const auto zd_hash = w.{form}();\n\
                 \x20   if (std::fwrite(zd_hash.data(), 1, zd_hash.size(), stdout) != zd_hash.size()) {{\n\
                 \x20       return zd_unwritable();\n\
                 \x20   }}\n\
                 \x20   return 0;\n\
                 }}\n\n",
                wire_error_catch()
            );
        }
        None => {
            // Not a compile-time hole: a type with no `@key` member has
            // one instance and no KeyHash, and every column answers that
            // the same way. Naming the type keeps the message the one a
            // caller can act on.
            let _ = write!(
                out,
                "static int zd_keyhash() {{\n\
                 \x20   std::fprintf(stderr, \"wireprobe-zerodds-cpp: {} declares no @key member\\n\");\n\
                 \x20   return 1;\n\
                 }}\n\n",
                args.type_name.replace('\\', "\\\\").replace('"', "\\\"")
            );
        }
    }

    // -- the read direction ---------------------------------------------
    if !with_read {
        out.push_str(
            "static int zd_read(zerodds::wire::Endianness order) {\n\
             \x20   static_cast<void>(order);\n\
             \x20   std::fprintf(stderr,\n\
             \x20       \"wireprobe-zerodds-cpp: this probe was built without the read direction\\n\");\n\
             \x20   return 1;\n\
             }\n\n",
        );
    } else {
        let decoder = decoder_name(ast, root);
        let parts = dump::dump(ast, root, key.union_label)?;
        if !parts.functions.is_empty() {
            out.push_str(&parts.functions);
            out.push('\n');
        }
        let _ = write!(
            out,
            "static int zd_read(zerodds::wire::Endianness order) {{\n\
             \x20   static unsigned char zd_input[{BUFFER_BYTES}];\n\
             \x20   const unsigned char *zd_at = zd_input;\n\
             \x20   std::size_t zd_len = std::fread(zd_input, 1, sizeof zd_input, stdin);\n\
             \x20   if (std::feof(stdin) == 0) {{\n\
             \x20       std::fprintf(stderr,\n\
             \x20           \"wireprobe-zerodds-cpp: the payload is larger than the probe's buffer\\n\");\n\
             \x20       return 1;\n\
             \x20   }}\n\
             \x20   zerodds::wire::Reader r(zd_at, zd_len, order, {max_align}, {framing});\n\
             \x20   try {{\n\
             \x20       /* The top-level type is not a member of anything, so no\n\
             \x20          `@try_construct` governs it and §7.2.2.7's default stands. */\n\
             \x20       {decoder}(r, zd_sample, zerodds::wire::TryConstruct::Discard);\n\
             \x20   }} catch (const zerodds::wire::WireError &error) {{\n{}\
             \x20       return 1;\n\
             \x20   }}\n\
             \x20   if (r.reader_status() == zerodds::wire::Status::Discarded) {{\n\
             \x20       /* Not a failed decode. DDS-XTypes 1.3 §7.2.2.7: the receiver\n\
             \x20          drops the object and carries on, and a caller that cannot\n\
             \x20          tell that from a broken peer cannot tell a narrowed type\n\
             \x20          from a corrupt stream. Exit code 3, the same one the C and\n\
             \x20          the Rust probe use. */\n\
             \x20       std::fprintf(stderr, \"wireprobe-zerodds-cpp: at %lu: %s\\n\",\n\
             \x20           static_cast<unsigned long>(r.reader_error_offset()),\n\
             \x20           zerodds::wire::status_name(r.reader_status()));\n\
             \x20       return 3;\n\
             \x20   }}\n\
             \x20   if (r.reader_status() != zerodds::wire::Status::Ok) {{\n\
             \x20       std::fprintf(stderr, \"wireprobe-zerodds-cpp: at %lu: %s\\n\",\n\
             \x20           static_cast<unsigned long>(r.reader_error_offset()),\n\
             \x20           zerodds::wire::status_name(r.reader_status()));\n\
             \x20       return 1;\n\
             \x20   }}\n\
             \x20   /* Nothing may be left over that is not padding. A decoder that\n\
             \x20      stopped short and said nothing is the silent-drop shape this\n\
             \x20      suite exists to find. */\n\
             \x20   const std::size_t zd_left = r.reader_remaining();\n\
             \x20   const std::size_t zd_pad =\n\
             \x20       ({max_align}u - (r.reader_position() % {max_align}u)) % {max_align}u;\n\
             \x20   if (zd_left > zd_pad) {{\n\
             \x20       std::fprintf(stderr,\n\
             \x20           \"wireprobe-zerodds-cpp: %lu byte(s) left at offset %lu, \"\n\
             \x20           \"of which at most %lu could be padding\\n\",\n\
             \x20           static_cast<unsigned long>(zd_left),\n\
             \x20           static_cast<unsigned long>(r.reader_position()),\n\
             \x20           static_cast<unsigned long>(zd_pad));\n\
             \x20       return 1;\n\
             \x20   }}\n",
            wire_error_catch()
        );
        for line in parts.body.lines() {
            out.push_str("    ");
            out.push_str(line);
            out.push('\n');
        }
        out.push_str("    return std::fflush(stdout) == 0 ? 0 : 1;\n}\n\n");
    }

    // -- the dispatch ---------------------------------------------------
    out.push_str(
        "int main(int argc, char **argv) {\n\
         \x20   const char *form = argc > 1 ? argv[1] : \"payload\";\n\
         \x20   /* Anything that is not `be` is little endian, and an unknown form is\n\
         \x20      refused rather than defaulted: a caller who mistyped one has to be\n\
         \x20      told, not handed the other form's bytes. */\n\
         \x20   const zerodds::wire::Endianness order =\n\
         \x20       (argc > 2 && argv[2][0] == 'b') ? zerodds::wire::Endianness::Big\n\
         \x20                                        : zerodds::wire::Endianness::Little;\n\
         \x20   /* The read direction is dispatched before the sample is filled: the\n\
         \x20      decoder fills it, and a value the caller did not send must not be\n\
         \x20      standing in one it did. */\n\
         \x20   if (std::strcmp(form, \"read\") == 0) {\n\
         \x20       return zd_read(order);\n\
         \x20   }\n\
         \x20   zd_fill();\n\
         \x20   if (std::strcmp(form, \"payload\") == 0) {\n\
         \x20       return zd_write(order, false);\n\
         \x20   }\n\
         \x20   if (std::strcmp(form, \"payload+encapsulation\") == 0) {\n\
         \x20       return zd_write(order, true);\n\
         \x20   }\n\
         \x20   if (std::strcmp(form, \"keyhash\") == 0) {\n\
         \x20       return zd_keyhash();\n\
         \x20   }\n\
         \x20   std::fprintf(stderr, \"wireprobe-zerodds-cpp: `%s` is not an emit form\\n\", form);\n\
         \x20   return 2;\n\
         }\n",
    );
    Ok(out)
}

/// The one line a failed `std::fwrite` answers in.
///
/// A function and not a repeated literal because it is reported four
/// times in one program, and the message is what the batch driver files
/// as this slot's refusal — the same one the single-form harness prints.
fn unwritable() -> String {
    String::from(
        "static int zd_unwritable() {\n\
         \x20   std::fprintf(stderr, \"wireprobe-zerodds-cpp: the bytes could not be written\\n\");\n\
         \x20   return 1;\n\
         }\n\n",
    )
}

/// The `catch` that turns this runtime's own channel into one sentence.
///
/// `runtime/cpp` throws `WireError` for the failures it sees itself — a
/// truncated stream, a length no buffer that long can carry, a value the
/// type cannot hold — and the batch driver files the **first line** of
/// standard error as this slot's refusal. An uncaught exception's first
/// line is `terminate called after throwing an instance of
/// 'zerodds::wire::Truncated'`, which names the class and not the clause,
/// so each channel is caught where it is raised and printed with `what()`
/// and the offset, the same sentence the read harness prints.
fn wire_error_catch() -> String {
    String::from(
        "\
         \x20       /* A value the type cannot hold, or a stream that ran out. The C\n\
         \x20          column reports the same outcomes through its status; this\n\
         \x20          runtime throws instead, so this is where the two meet. */\n\
         \x20       std::fprintf(stderr, \"wireprobe-zerodds-cpp: at %lu: %s\\n\",\n\
         \x20           static_cast<unsigned long>(error.at()), error.what());\n",
    )
}

/// Which of the two KeyHash forms the holder takes.
///
/// The maximum holder size, including bounded variable lengths: sixteen
/// octets or fewer and the holder *is* the hash, zero-padded; longer or
/// unbounded and it is the MD5 of it. DDSI-RTPS 2.5 §9.6.3.8.
///
/// # Errors
/// [`Refusal::Gap`] where the holder's maximum size does not evaluate.
fn key_hash_form(session: &Session<'_>) -> Result<&'static str, Refusal> {
    match zerodds_ast_layout::keyhash::key_hash_form_for(
        &session.ast,
        session.root,
        session.holder_key,
    )
    .map_err(|error| gap!("key-holder maximum size: {error}"))?
    {
        zerodds_ast_layout::keyhash::KeyHashForm::ZeroPadded => Ok("zero_padded"),
        zerodds_ast_layout::keyhash::KeyHashForm::Md5 => Ok("md5"),
    }
}

/// The command, for `main`.
///
/// zerodds-lint: recursion-depth 1 — this does not recurse. The callee
/// is `zerodds_wireprobe_gen::cli_main`, a different function in
/// another crate; `dds_bounded_recursion` compares only the last
/// segment of a call path, so the delegation reads as a self-call.
pub fn cli_main() {
    // The probe reads the fields of the native structures: the wire column asks for `native`
    // (the default of the library is the DDS-PSM-Cxx mapping).
    let _native = zerodds_lang_rules::adapter::cpp::CppMappingScope::enter(
        zerodds_lang_rules::adapter::cpp::CppMapping::Native,
    );
    zerodds_wireprobe_gen::cli_main::<Renderer>();
}

/// `probe.cpp`: the codec, the sample, the fill and `main`.
fn harness(session: &Session<'_>, codec: &str, fill: &str) -> Result<String, Refusal> {
    let args = session.args;
    let ast: &Ast = &session.ast;
    let root: Id<TypeNode> = session.root;
    let key: LayoutKey = session.key;
    let sample_type = zerodds_lang_render::cpp::type_(
        &<Cpp as zerodds_lang_rules::emit::CodecTarget>::value_type(ast, root),
    );
    let encoder = codec_name::<Cpp>(ast, root, Purpose::Data);
    let order = match args.endianness {
        Endianness::Little => "zerodds::wire::Endianness::Little",
        Endianness::Big => "zerodds::wire::Endianness::Big",
    };
    let framing = pl_cdr1_framing(key);
    let short_codes = key.length_codes.short_code_where_it_fits();

    let mut out = String::new();
    out.push_str(
        "// Generated by wireprobe-zerodds-cpp-gen. Do not edit.\n\
         //\n\
         // The bytes come out of runtime/cpp when this runs. Nothing here\n\
         // decides a wire question.\n\n\
         #include \"zd_types.hpp\"\n\
         #include \"zerodds/wire.hpp\"\n\n\
         #include <cstdio>\n\
         #include <cstring>\n\
         #include <vector>\n\n",
    );
    out.push_str(codec);
    let _ = write!(
        out,
        "\nstatic {sample_type} zd_sample;\n\n\
         static void zd_fill() {{\n{fill}}}\n\n\
         int main() {{\n\
         \x20   zd_fill();\n\
         \x20   zerodds::wire::Writer w({order}, {}, {framing}, {short_codes});\n",
        args.encoding.max_alignment()
    );
    if args.encapsulation() {
        let header = session.encapsulation_header();
        let _ = writeln!(
            out,
            "    unsigned char zd_head[4] = {{ 0x{:02x}, 0x{:02x}, 0x{:02x}, 0x{:02x} }};\n\
             \x20   static const unsigned char zd_padding[3] = {{ 0, 0, 0 }};",
            header[0], header[1], header[2], header[3]
        );
    }
    let _ = write!(
        out,
        "    {encoder}(w, zd_sample);\n\
         \x20   const std::vector<std::uint8_t> zd_body = w.finish();\n"
    );
    if args.encapsulation() {
        // §7.4.1.1 pads the serialized data to the next four-octet
        // boundary with zeros, and §7.6.3.1.2 puts the count of those
        // octets in the encapsulation options so a receiver can find
        // where the data ended. Both halves or neither: padding without
        // the count hands a conforming receiver up to three octets of
        // zero as data, and the count without the padding tells it to
        // cut octets that are data.
        //
        // This column wrote neither, and the header went out *before*
        // the encode, so the count could not have been filled in even
        // if something had tried. Measured on `primitive-octet` under
        // plain-cdr2, little-endian: C, C# and Java write
        // `00070002 ab01 0000` and this wrote `00070000 ab01`. The Rust
        // column — `wire-codec/src/encode.rs`,
        // `Emit::PayloadWithEncapsulation` — and the Python column
        // agree with the three.
        //
        // `--emit payload` stays unpadded: that is the serialized data
        // itself, and it is what the ledger records.
        out.push_str(
            "    const std::size_t zd_pad = (4u - (zd_body.size() & 3u)) & 3u;\n\
             \x20   zd_head[3] = static_cast<unsigned char>(zd_head[3] | zd_pad);\n\
             \x20   std::fwrite(zd_head, 1, 4, stdout);\n",
        );
    }
    out.push_str("    std::fwrite(zd_body.data(), 1, zd_body.size(), stdout);\n");
    if args.encapsulation() {
        out.push_str("    std::fwrite(zd_padding, 1, zd_pad, stdout);\n");
    }
    out.push_str("    return 0;\n}\n");
    Ok(out)
}

/// The key holder and what it calls, and nothing else.
///
/// The C column's KeyHash run, in C++: the codec planned under the
/// holder's own layout key — the same types with only their `@key`
/// members — and, under the dialect that writes a keyless nested
/// aggregate whole, a second set planned for data, because
/// `keyhash::member_key` names `zd_enc_` at those positions. Everything
/// the holder does not reach is pruned
/// ([`zerodds_lang_rules::default_cpp::reachable_from`]), so `-Werror`
/// keeps meaning that an unused routine is a generator defect.
///
/// A type with no `@key` member has one instance and no KeyHash, and
/// every column answers that the same way.
fn key_hash_codec(session: &Session<'_>) -> Result<Vec<zerodds_lang_cst::cpp::Decl>, Refusal> {
    let ast = &session.ast;
    let root = session.root;
    let holder_key = session.holder_key;
    let mut decls = codec_from::<Cpp>(ast, root, holder_key, session.codec_options)
        .map_err(|error| from_codec(&error))?;
    if holder_key.key_holder_nesting == KeyHolderNesting::KeylessNestedWhole {
        let as_data = LayoutKey {
            purpose: Purpose::Data,
            ..holder_key
        };
        decls.extend(
            codec_from::<Cpp>(ast, root, as_data, session.codec_options)
                .map_err(|error| from_codec(&error))?,
        );
    }
    let Some(holder) =
        key_holder::<Cpp>(ast, root, holder_key).map_err(|error| from_codec(&error))?
    else {
        return Err(gap!("{} declares no @key member", session.args.type_name));
    };
    let zerodds_lang_cst::cpp::Decl::Function(function) = &holder else {
        return Err(gap!("the key holder is not a function"));
    };
    let entry = function.name.as_str().to_string();
    decls.push(holder);
    Ok(zerodds_lang_rules::default_cpp::reachable_from(
        decls, &entry,
    ))
}

/// `probe.cpp` for a KeyHash run: the sample, its key holder written
/// big-endian under PLAIN_CDR2's alignment cap (XTypes 1.3 §7.6.8), and
/// the sixteen octets DDSI-RTPS 2.5 §9.6.3.8 makes of it — the holder
/// zero-padded when its maximum size is sixteen octets or fewer, its MD5
/// otherwise. The same two steps as the C column's `zd_keyhash`.
fn key_hash_harness(session: &Session<'_>, codec: &str, fill: &str) -> Result<String, Refusal> {
    let ast: &Ast = &session.ast;
    let root: Id<TypeNode> = session.root;
    let sample_type = zerodds_lang_render::cpp::type_(
        &<Cpp as zerodds_lang_rules::emit::CodecTarget>::value_type(ast, root),
    );
    let holder = format!(
        "zd_keyholder_{}",
        <Cpp as zerodds_lang_rules::emit::CodecTarget>::type_name(ast, root)
    );
    let form = key_hash_form(session)?;
    let mut out = String::new();
    out.push_str(
        "// Generated by wireprobe-zerodds-cpp-gen. Do not edit.\n\
         //\n\
         // The KeyHash comes out of runtime/cpp when this runs. Nothing here\n\
         // decides a wire question.\n\n\
         #include \"zd_types.hpp\"\n\
         #include \"zerodds/wire.hpp\"\n\n\
         #include <cstdio>\n\
         #include <cstring>\n\
         #include <vector>\n\n",
    );
    out.push_str(codec);
    let _ = write!(
        out,
        "\nstatic {sample_type} zd_sample;\n\n\
         static void zd_fill() {{\n{fill}}}\n\n\
         int main() {{\n\
         \x20   zd_fill();\n\
         \x20   // The holder is PLAIN_CDR2 big-endian whatever the sample is,\n\
         \x20   // so its alignment cap is PLAIN_CDR2's. XTypes 1.3 7.6.8.\n\
         \x20   zerodds::wire::Writer w(zerodds::wire::Endianness::Big, 4);\n\
         \x20   {holder}(w, zd_sample);\n\
         \x20   const auto zd_hash = w.{form}();\n\
         \x20   std::fwrite(zd_hash.data(), 1, zd_hash.size(), stdout);\n\
         \x20   return 0;\n\
         }}\n"
    );
    Ok(out)
}

/// `probe.cpp` for the read direction: bytes on standard input,
/// canonical JSON on standard output.
///
/// Standard input and not a path baked into the generated C++: the path
/// would be part of the program, so a probe compiled for one payload
/// could not be run against another, and the shell wrapper would have to
/// regenerate for every byte string it wanted read.
///
/// ## Where the two failure channels meet
///
/// `runtime/cpp` throws for its own failures — a truncated stream, a
/// length no buffer that long can carry — and carries a sticky
/// `zerodds::wire::Status` for the refusals the *type* makes, which the
/// runtime cannot see: an integer that names no enumerator, a sequence
/// past its bound, §7.2.2.7's DISCARD. Both end here. The C column has
/// one channel because its runtime has no exceptions, and the outcome it
/// reports is the same one: exit 3 for a discarded sample, non-zero for
/// a stream that could not be read.
fn read_harness(session: &Session<'_>, codec: &str) -> Result<String, Refusal> {
    let args = session.args;
    let ast: &Ast = &session.ast;
    let root: Id<TypeNode> = session.root;
    let key: LayoutKey = session.key;
    let sample_type = zerodds_lang_render::cpp::type_(
        &<Cpp as zerodds_lang_rules::emit::CodecTarget>::value_type(ast, root),
    );
    let decoder = decoder_name(ast, root);
    let order = match args.endianness {
        Endianness::Little => "zerodds::wire::Endianness::Little",
        Endianness::Big => "zerodds::wire::Endianness::Big",
    };
    let framing = pl_cdr1_framing(key);
    let max_align = args.encoding.max_alignment();
    let strip = usize::from(args.encapsulation()) * 4;

    let mut out = String::new();
    out.push_str(
        "// Generated by wireprobe-zerodds-cpp-gen. Do not edit.\n\
         //\n\
         // The values come out of runtime/cpp when this runs. Nothing here\n\
         // decides a wire question.\n\n\
         #include \"zd_types.hpp\"\n\
         #include \"zerodds/wire.hpp\"\n\n\
         #include <cstdio>\n\
         #include <vector>\n\n\
         // The JSON dialect, shared with every other probe. `dumpjson.hpp`\n\
         // adds the one helper a C++ binding needs on top of the C header:\n\
         // a `long double` printed to every digit it has.\n\
         #include \"dumpjson.hpp\"\n\n",
    );
    out.push_str(codec);
    let dump_parts = dump::dump(ast, root, key.union_label)?;
    let _ = write!(
        out,
        "\n/* The sample the decoder fills. */\nstatic {sample_type} zd_sample;\n\n"
    );
    if !dump_parts.functions.is_empty() {
        out.push_str(&dump_parts.functions);
        out.push('\n');
    }
    let _ = write!(
        out,
        "int main() {{\n\
         \x20   static unsigned char zd_input[{BUFFER_BYTES}];\n\
         \x20   const unsigned char *zd_at = zd_input;\n\
         \x20   std::size_t zd_len = std::fread(zd_input, 1, sizeof zd_input, stdin);\n\
         \x20   if (std::feof(stdin) == 0) {{\n\
         \x20       std::fprintf(stderr,\n\
         \x20           \"wireprobe-zerodds-cpp: the payload is larger than the probe's buffer\\n\");\n\
         \x20       return 1;\n\
         \x20   }}\n"
    );

    if strip > 0 {
        // The encapsulation identifier is not re-read for the encoding:
        // the caller named one, and a probe that took the stream's word
        // for it could not be pointed at a stream to test what it does
        // with a header it disagrees with.
        //
        // The **options** word is read, because §7.6.3.1.2 says a
        // receiver shall use its low two bits to find where the
        // serialized data ended. Without them the trailing padding is
        // handed to the decoder as data.
        let _ = write!(
            out,
            "    if (zd_len < {strip}u) {{\n\
             \x20       std::fprintf(stderr,\n\
             \x20           \"wireprobe-zerodds-cpp: fewer than the four-byte encapsulation header\\n\");\n\
             \x20       return 1;\n\
             \x20   }}\n\
             \x20   const std::size_t zd_declared = static_cast<std::size_t>(zd_at[3] & 3u);\n\
             \x20   zd_at += {strip}u;\n\
             \x20   zd_len -= {strip}u;\n\
             \x20   if (zd_len < zd_declared) {{\n\
             \x20       std::fprintf(stderr,\n\
             \x20           \"wireprobe-zerodds-cpp: the options field declares %lu padding byte(s) \"\n\
             \x20           \"and only %lu follow the header\\n\",\n\
             \x20           static_cast<unsigned long>(zd_declared),\n\
             \x20           static_cast<unsigned long>(zd_len));\n\
             \x20       return 1;\n\
             \x20   }}\n\
             \x20   zd_len -= zd_declared;\n"
        );
    }

    let _ = write!(
        out,
        "    zerodds::wire::Reader r(zd_at, zd_len, {order}, {max_align}, {framing});\n\
         \x20   try {{\n\
         \x20       // The top-level type is not a member of anything, so no\n\
         \x20       // `@try_construct` governs it and 7.2.2.7's default stands.\n\
         \x20       {decoder}(r, zd_sample, zerodds::wire::TryConstruct::Discard);\n\
         \x20   }} catch (const zerodds::wire::WireError &error) {{\n\
         \x20       // This runtime's own channel. A stream that ran out or a\n\
         \x20       // length no buffer that long can carry: the sample is gone,\n\
         \x20       // and the C column reports the same outcome through its\n\
         \x20       // status.\n\
         \x20       std::fprintf(stderr, \"wireprobe-zerodds-cpp: at %lu: %s\\n\",\n\
         \x20           static_cast<unsigned long>(error.at()), error.what());\n\
         \x20       return 1;\n\
         \x20   }}\n\
         \x20   if (r.reader_status() == zerodds::wire::Status::Discarded) {{\n\
         \x20       // Not a failed decode. DDS-XTypes 1.3 7.2.2.7: the receiver\n\
         \x20       // drops the object and carries on, and a caller that cannot\n\
         \x20       // tell that from a broken peer cannot tell a narrowed type\n\
         \x20       // from a corrupt stream. Exit code 3, the same one the C and\n\
         \x20       // the Rust probe use.\n\
         \x20       std::fprintf(stderr, \"wireprobe-zerodds-cpp: at %lu: %s\\n\",\n\
         \x20           static_cast<unsigned long>(r.reader_error_offset()),\n\
         \x20           zerodds::wire::status_name(r.reader_status()));\n\
         \x20       return 3;\n\
         \x20   }}\n\
         \x20   if (r.reader_status() != zerodds::wire::Status::Ok) {{\n\
         \x20       std::fprintf(stderr, \"wireprobe-zerodds-cpp: at %lu: %s\\n\",\n\
         \x20           static_cast<unsigned long>(r.reader_error_offset()),\n\
         \x20           zerodds::wire::status_name(r.reader_status()));\n\
         \x20       return 1;\n\
         \x20   }}\n"
    );

    // Nothing may be left over that is not padding. A decoder that
    // stopped short and said nothing is the silent-drop shape this suite
    // exists to find: the sample would come back looking plausible with
    // its last members missing. Padding is the one thing a writer
    // legitimately leaves behind, and it is what alignment asks for from
    // this offset and never more.
    let _ = write!(
        out,
        "    const std::size_t zd_left = r.reader_remaining();\n\
         \x20   const std::size_t zd_pad =\n\
         \x20       ({max_align}u - (r.reader_position() % {max_align}u)) % {max_align}u;\n\
         \x20   if (zd_left > zd_pad) {{\n\
         \x20       std::fprintf(stderr,\n\
         \x20           \"wireprobe-zerodds-cpp: %lu byte(s) left at offset %lu, \"\n\
         \x20           \"of which at most %lu could be padding\\n\",\n\
         \x20           static_cast<unsigned long>(zd_left),\n\
         \x20           static_cast<unsigned long>(r.reader_position()),\n\
         \x20           static_cast<unsigned long>(zd_pad));\n\
         \x20       return 1;\n\
         \x20   }}\n"
    );

    for line in dump_parts.body.lines() {
        out.push_str("    ");
        out.push_str(line);
        out.push('\n');
    }
    out.push_str("    return std::fflush(stdout) == 0 ? 0 : 1;\n}\n");
    Ok(out)
}

/// The `PlCdr1Framing` aggregate, as both harnesses write it.
fn pl_cdr1_framing(key: LayoutKey) -> String {
    let headers = key.pl_cdr1_headers;
    format!(
        "zerodds::wire::PlCdr1Framing{{ {}, {}, {}, {}, {}, {} }}",
        match key.pid_flags {
            PidFlagPolicy::SentinelMustUnderstand =>
                "zerodds::wire::PidFlagPolicy::SentinelMustUnderstand",
            PidFlagPolicy::Bare => "zerodds::wire::PidFlagPolicy::Bare",
            PidFlagPolicy::ImplExtensionOnMembers =>
                "zerodds::wire::PidFlagPolicy::ImplExtensionMembers",
        },
        key.short_pid_range.highest_short_id(),
        headers.pads_declared_length(),
        headers.pads_declared_optional_length(),
        key.member_pid_carries_must_understand(),
        headers.pads_after_value()
    )
}

/// The verdict a codec error answers in.
///
/// One place per generator, reading `EncodeError::is_spec_refusal`: a
/// clause of the specification is `refused` (exit 1) and everything else
/// is this tree's own gap (exit 6).
fn from_codec(error: &EncodeError) -> Refusal {
    if error.is_spec_refusal() {
        Refusal::Refused(format!("{error}"))
    } else {
        Refusal::Gap(format!("{error}"))
    }
}
