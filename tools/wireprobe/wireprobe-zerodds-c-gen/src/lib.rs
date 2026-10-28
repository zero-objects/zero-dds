// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-wireprobe-c-gen`. Safety classification: **TOOLING**.
//!
//! `wireprobe-zerodds-c-gen` — the C translation unit the C probe
//! compiles.
//!
//! One IDL file, one type, one JSON sample and one layout key in; a
//! `gen.h` of C declarations, a `probe.c` of codec functions and a
//! `main` that writes the bytes out. `tools/wireprobe/wireprobe-zerodds-c`
//! is the shell around it that compiles and runs the result, and takes
//! the same command line as `wireprobe-zerodds`.
//!
//! Nothing here writes a byte. The declarations come from the C
//! materialiser, the codec from the C codec emitter, and the bytes come
//! out of `runtime/c` when the compiled program runs — which is what
//! makes the C column a second implementation rather than a rendering of
//! the Rust one's answer.
//!
//! The C spelling and nothing else. [`zerodds_wireprobe_gen`] is the
//! stem: the command line, the IDL, the model, the layout key, the
//! sample, the files and the exit codes are there, once, for all five
//! columns.
//!
//! # Why this is a library and not only a command
//!
//! [`wireprobe-c-matrix`](../wireprobe_c_matrix/index.html) runs the C
//! column over every case in every configuration, which is a hundred
//! thousand generations. A process launch per generation is several
//! times the generation itself, so the batch driver calls
//! [`combined_sources`] directly and the command is the same call with
//! file IO around it.
#![forbid(unsafe_code)]

mod dump;
mod fill;

use zerodds_ast::{Ast, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ast_layout::{Endianness, KeyHolderNesting, LayoutKey, PidFlagPolicy, Purpose};
use zerodds_lang_cst::c;
use zerodds_lang_cst::common::{Doc, Ident};
use zerodds_lang_rules::adapter::c::{c_items, c_type_name};
use zerodds_lang_rules::decode_c::{c_decoder_from, decoder_name};
use zerodds_lang_rules::encode::EncodeError;
use zerodds_lang_rules::encode_c::{c_codec_from, c_key_holder, codec_name};
use zerodds_lang_rules::rule;
use zerodds_lang_rules::{ModelGraph, apply};
pub use zerodds_wireprobe_gen::{Args, Emit};
use zerodds_wireprobe_gen::{Column, File, Refusal, Session, encapsulation_header, gap};

/// How large the probe's output buffer is.
///
/// A fixed buffer and not an allocation: the write path is the one an
/// embedded target links, and the point of the runtime's short-buffer
/// status is that it is reachable. A megabyte is far past anything the
/// corpus holds, and a sample that overruns it fails loudly.
const BUFFER_BYTES: usize = 1 << 20;

/// How much storage the read direction has for the values it
/// materialises.
///
/// A string's bytes and a sequence's elements have nowhere to live
/// until they are read, and `runtime/c` allocates nothing of its own —
/// the buffer is the caller's, the same discipline as the writer's
/// output buffer. A payload that overruns it fails loudly with
/// `ZDW_SHORT_BUFFER` rather than coming back short.
const HEAP_BYTES: usize = 1 << 20;

/// The C column.
#[derive(Debug, Clone, Copy)]
pub struct Renderer;

impl Column for Renderer {
    const BINARY: &'static str = "wireprobe-zerodds-c-gen";
    const LANGUAGE: &'static str = "C";
    const OUTPUT: &'static str = "gen.h and probe.c go";
    /// The KeyHash harness the other four columns follow: the key holder
    /// under the holder's own layout key, pruned to what it reaches, and
    /// the holder zero-padded or hashed into sixteen octets.
    const FORMS: &'static [Emit] = &[Emit::Payload, Emit::PayloadWithEncapsulation, Emit::KeyHash];

    fn render(session: &Session<'_>) -> Result<Vec<File>, Refusal> {
        let sources = sources(session)?;
        Ok(vec![
            File::new("gen.h", sources.header),
            File::new("probe.c", sources.probe),
        ])
    }

    fn combined(session: &Session<'_>, with_read: bool) -> Result<Vec<File>, Refusal> {
        let sources = combined_sources_from(session, with_read)?;
        Ok(vec![
            File::new("gen.h", sources.header),
            File::new("probe.c", sources.probe),
        ])
    }
}

/// The two files one generation produces.
#[derive(Debug, Clone)]
pub struct Sources {
    /// The C declarations of the IDL file's types.
    pub header: String,
    /// The codec and the harness that drives it.
    pub probe: String,
}

/// The command, for `main`.
///
/// zerodds-lint: recursion-depth 1 — this does not recurse. The callee
/// is `zerodds_wireprobe_gen::cli_main`, a different function in
/// another crate; `dds_bounded_recursion` compares only the last
/// segment of a call path, so the delegation reads as a self-call.
pub fn cli_main() {
    zerodds_wireprobe_gen::cli_main::<Renderer>();
}

/// Generate the two files and hand them back, touching no disk.
///
/// The batch driver hashes what comes out of here and compiles one
/// binary per distinct pair, so two configurations that produce the same
/// C are compiled once. That is only sound because nothing below reads
/// the clock, the environment or a path into the output.
///
/// # Errors
/// [`Refusal`], classified so the command can answer in the verdict's
/// own exit code.
pub fn sources(session: &Session<'_>) -> Result<Sources, Refusal> {
    let args = session.args;
    let ast = &session.ast;
    let root = session.root;

    // The declarations, through the rule catalogue: a construct with no
    // rule produces nothing here, and the coverage table says which.
    let model = ModelGraph::of(ast);
    let applied =
        apply(&model, &rule::c::catalogue()).map_err(|error| Refusal::Gap(format!("{error}")))?;
    // The materialiser is the authority on what C has a form for, so
    // its refusal is exit 3 and every other stop is exit 6.
    let decls = c_items(ast, &applied).map_err(|error| Refusal::NoForm(format!("{error}")))?;

    let header = c::File {
        doc: Doc::lines(&[
            "Generated by wireprobe-zerodds-c-gen. Do not edit.",
            "",
            "The C shape of one IDL file. The wire rules are not here — they",
            "are in runtime/c, which implements them from DDS-XTypes 1.3.",
        ]),
        guard: Some(Ident::new("ZD_GEN_H")),
        includes: vec![
            c::Include::system("stdbool.h"),
            c::Include::system("stddef.h"),
            c::Include::system("stdint.h"),
        ],
        decls,
    };

    let key = session.key;
    let holder_key = session.holder_key;
    let options = session.codec_options;
    // What the chosen direction and emit form actually calls, and
    // nothing else. A function the probe never reaches is a `static`
    // one nothing uses, `-Wunused-function` names it, and under
    // `-Werror` the file does not compile — which is the point. The
    // warning is what found the `sequence<double>` against
    // `sequence<long double>` type-name collision, so it is kept sharp
    // by emitting the right set rather than by silencing it.
    let mut codec = if args.read {
        c_decoder_from(ast, root, key, options).map_err(|error| from_codec(&error))?
    } else if args.emit == Emit::KeyHash {
        // The holder's plan and not the payload's: the same types with
        // only their `@key` members, which is a different shape and so
        // a different function per type. The payload encoders are not
        // among them — a KeyHash run never writes a payload.
        let mut under_holder =
            c_codec_from(ast, root, holder_key, options).map_err(|error| from_codec(&error))?;
        // And, under one dialect, a second set planned for data.
        // §7.6.8 Step 1 removes a nested aggregate's non-key members
        // only "if there are any key members"; RTI under XCDR1 reads
        // the consequence as "and below a keyless one, stop filtering
        // altogether", so the subtree there is written as the data has
        // it. `keyhash::member_key` is the rule and the call sites ask
        // it, which means those positions name `zd_enc_` where the rest
        // name `zd_kenc_` — and both have to exist for the file to
        // link. Everything the holder does not reach is pruned below.
        if holder_key.key_holder_nesting == KeyHolderNesting::KeylessNestedWhole {
            let as_data = LayoutKey {
                purpose: Purpose::Data,
                ..holder_key
            };
            under_holder
                .extend(c_codec_from(ast, root, as_data, options).map_err(|e| gap!("{e}"))?);
        }
        under_holder
    } else {
        c_codec_from(ast, root, key, options).map_err(|error| from_codec(&error))?
    };

    if args.emit == Emit::KeyHash {
        let holder =
            match c_key_holder(ast, root, holder_key).map_err(|error| from_codec(&error))? {
                Some(holder) => holder,
                // Not a hole in C: a type with no `@key` member has one
                // instance and no KeyHash, and every column answers that the
                // same way. `no-form` would say C cannot express it.
                None => return Err(gap!("{} declares no @key member", args.type_name)),
            };
        let entry = match &holder {
            c::Decl::Function(function) => function.name.as_str().to_string(),
            _ => return Err(gap!("the key holder is not a function")),
        };
        codec.push(holder);
        codec = reachable_from(codec, &entry);
    }

    let root_type = c_type_name(ast, root);
    let mut includes = vec![
        c::Include::system("stdio.h"),
        c::Include::system("stdlib.h"),
        c::Include::system("string.h"),
        c::Include::local("zerodds_wire.h"),
        c::Include::local("gen.h"),
    ];
    if args.read {
        // The same eleven helpers the Cyclone, RTI and Fast DDS probes
        // print through. The JSON dialect is a shared contract; four
        // backends holding four copies of it is how they come to
        // disagree about what a sample is.
        includes.push(c::Include::local("dumpjson.h"));
    }
    let body = c::File {
        doc: Doc::lines(&["Generated by wireprobe-zerodds-c-gen. Do not edit."]),
        guard: None,
        includes,
        decls: codec,
    };

    let mut probe = zerodds_lang_render::c::file(&body);
    probe.push('\n');
    if args.read {
        probe.push_str(&read_harness(args, ast, root, &root_type, key)?);
    } else {
        let filled = fill::fill(ast, root, session.sample()?)?;
        probe.push_str(&harness(
            args, ast, root, &root_type, &filled, key, holder_key,
        )?);
    }

    Ok(Sources {
        header: zerodds_lang_render::c::file(&header),
        probe,
    })
}

/// One translation unit that does all four things, chosen at run time.
///
/// [`sources`] bakes the emit form, the direction and the byte order
/// into the program, so the corpus in every configuration is four
/// binaries per plan and two of each for the two byte orders. The batch
/// driver compiles and runs tens of thousands of them, and on macOS the
/// **first** execution of a freshly linked binary costs about 235 ms of
/// signature validation against 2 ms for every one after — measured, an
/// M1: 20 fresh binaries in 4.5 s, 20 runs of one in 0.04 s. The check
/// is also serialised across processes, so it does not parallelise: 40
/// fresh binaries take 9.1 s at one job and 9.1 s at ten. Against that,
/// one compile is 65 ms. The binary count, not the compiler, is what
/// the run costs.
///
/// So this emits one program per case and plan. It takes
///
/// ```text
/// probe payload|payload+encapsulation|keyhash|read [le|be]
/// ```
///
/// which collapses eight binaries into one.
///
/// Byte order can move out of the program because the plan does not
/// depend on it — DDS-XTypes 1.3 puts member offsets in the type and
/// the order in the stream, and `LayoutKey`'s own documentation says
/// offsets are invariant under it. It is checked rather than assumed:
/// generating both orders under six plans over seven cases gives C that
/// differs in the `ZDW_LITTLE`/`ZDW_BIG` token and nowhere else.
///
/// The emit form can move out because it only ever changed `main`.
///
/// # What is kept from the single-form generator
///
/// The payload codec is **not** pruned to what is reachable, for the
/// reason `reachable_from` gives: an unreferenced element encoder
/// under `-Werror` is what found the `sequence<double>` against
/// `sequence<long double>` type-name collision, and a reachability pass
/// would have thrown the evidence away. Only the key holder's set is
/// pruned, and only because a holder legitimately leaves the encoders
/// for non-key members unused.
///
/// # Errors
/// [`Refusal`], classified so the caller can answer in the verdict's
/// own exit code.
pub fn combined_sources(args: &Args) -> Result<Sources, Refusal> {
    combined_sources_with(args, true)
}

/// [`combined_sources`], with the read direction left out.
///
/// The fallback for a case whose **decoder** does not compile. Putting
/// all four forms in one translation unit means one broken form takes
/// the other three with it, and a C column that reported "no probe" for
/// a payload it can write perfectly well would understate itself and
/// hide the actual defect behind a bigger one. So the batch driver
/// retries without the decoder and files the read slots, and only
/// those, under the compiler's message.
///
/// # Errors
/// [`Refusal`], classified.
pub fn combined_sources_with(args: &Args, with_read: bool) -> Result<Sources, Refusal> {
    let session = Session::open(args)?;
    combined_sources_from(&session, with_read)
}

/// [`combined_sources_with`], on a session the stem has already opened.
///
/// The batch driver opens one per translation unit and the command
/// opens one per run; both end here, so the IDL is read the same way in
/// either.
///
/// # Errors
/// [`Refusal`], classified.
#[allow(clippy::too_many_lines)]
pub fn combined_sources_from(session: &Session<'_>, with_read: bool) -> Result<Sources, Refusal> {
    let args = session.args;
    let ast = &session.ast;
    let root = session.root;
    let values = session.sample()?;

    let model = ModelGraph::of(ast);
    let applied =
        apply(&model, &rule::c::catalogue()).map_err(|error| Refusal::Gap(format!("{error}")))?;
    let decls = c_items(ast, &applied).map_err(|error| Refusal::NoForm(format!("{error}")))?;
    let header = c::File {
        doc: Doc::lines(&[
            "Generated by wireprobe-zerodds-c-gen. Do not edit.",
            "",
            "The C shape of one IDL file. The wire rules are not here — they",
            "are in runtime/c, which implements them from DDS-XTypes 1.3.",
        ]),
        guard: Some(Ident::new("ZD_GEN_H")),
        includes: vec![
            c::Include::system("stdbool.h"),
            c::Include::system("stddef.h"),
            c::Include::system("stdint.h"),
        ],
        decls,
    };

    let key = session.key;
    let holder_key = session.holder_key;
    let options = session.codec_options;

    let mut codec = c_codec_from(ast, root, key, options).map_err(|error| from_codec(&error))?;
    if with_read {
        codec.extend(c_decoder_from(ast, root, key, options).map_err(|error| from_codec(&error))?);
    }

    // The holder, when the type has one. A keyless type is not an error
    // here — three of the four forms still work and the fourth reports
    // that there is no KeyHash to take, which is what every other
    // column answers for it.
    let holder_entry = match c_key_holder(ast, root, holder_key).map_err(|e| gap!("{e}"))? {
        Some(c::Decl::Function(function)) => {
            let entry = function.name.as_str().to_string();
            let mut under_holder =
                c_codec_from(ast, root, holder_key, options).map_err(|error| from_codec(&error))?;
            if holder_key.key_holder_nesting == KeyHolderNesting::KeylessNestedWhole {
                let as_data = LayoutKey {
                    purpose: Purpose::Data,
                    ..holder_key
                };
                under_holder
                    .extend(c_codec_from(ast, root, as_data, options).map_err(|e| gap!("{e}"))?);
            }
            under_holder.push(c::Decl::Function(function));
            codec.extend(reachable_from(under_holder, &entry));
            Some(entry)
        }
        Some(_) => return Err(gap!("the key holder is not a function")),
        None => None,
    };

    let codec = deduplicate(codec)?;

    let mut includes = vec![
        c::Include::system("stdio.h"),
        c::Include::system("stdlib.h"),
        c::Include::system("string.h"),
        c::Include::local("zerodds_wire.h"),
        c::Include::local("gen.h"),
    ];
    if with_read {
        // The same eleven helpers the Cyclone, RTI and Fast DDS probes
        // print through. The JSON dialect is a shared contract; four
        // backends holding four copies of it is how they come to
        // disagree about what a sample is.
        includes.push(c::Include::local("dumpjson.h"));
    }
    includes.dedup();
    let body = c::File {
        doc: Doc::lines(&["Generated by wireprobe-zerodds-c-gen. Do not edit."]),
        guard: None,
        includes,
        decls: codec,
    };

    let mut probe = zerodds_lang_render::c::file(&body);
    probe.push('\n');
    let root_type = c_type_name(ast, root);
    let filled = fill::fill(ast, root, values)?;
    probe.push_str(&combined_harness(
        args,
        ast,
        root,
        &root_type,
        &filled,
        key,
        holder_key,
        holder_entry.as_deref(),
        with_read,
    )?);

    Ok(Sources {
        header: zerodds_lang_render::c::file(&header),
        probe,
    })
}

/// The C name of a PID-flag policy.
///
/// A `match` and not a string built from `as_str`: a policy added to
/// `zerodds_ast_layout::PidFlagPolicy` has to be given a C name here
/// rather than silently emitting an identifier the runtime does not
/// declare.
const fn c_pid_flags(policy: PidFlagPolicy) -> &'static str {
    match policy {
        PidFlagPolicy::SentinelMustUnderstand => "ZDW_PID_FLAGS_SENTINEL_MUST_UNDERSTAND",
        PidFlagPolicy::Bare => "ZDW_PID_FLAGS_BARE",
        PidFlagPolicy::ImplExtensionOnMembers => "ZDW_PID_FLAGS_IMPL_EXTENSION_ON_MEMBERS",
    }
}

/// Drop the second definition of anything defined twice, and refuse two
/// definitions of one name that are not the same text.
///
/// The three codec sets overlap: under `KeylessNestedWhole` the holder's
/// subtree is written with the data-purpose encoders, which are the ones
/// the payload set already holds. Keeping both would not compile.
///
/// Two definitions of a name that differ is the case that must not pass
/// quietly — it would mean one of the two plans is being silently
/// dropped, and the probe would write bytes from a plan nobody chose.
fn deduplicate(decls: Vec<c::Decl>) -> Result<Vec<c::Decl>, String> {
    let mut seen: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
    let mut out = Vec::with_capacity(decls.len());
    for decl in decls {
        let rendered = zerodds_lang_render::c::decl(&decl);
        // A forward declaration and the definition it announces are two
        // decls with one name, and both belong in the file. The body is
        // part of the key so that the pair is not read as a conflict.
        let name = match &decl {
            c::Decl::Function(function) => format!(
                "{}{}",
                function.name.as_str(),
                if function.body.is_some() { "()" } else { ";" }
            ),
            _ => rendered.clone(),
        };
        match seen.get(&name) {
            Some(before) if *before == rendered => continue,
            Some(_) => {
                return Err(format!(
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

/// The `main` of the combined probe: the sample, the fill, one function
/// per form, and a dispatch on `argv`.
///
/// Written as text for the reason [`harness`] gives — this is the
/// harness *around* the generated code, and holding it as a CST would
/// say it was part of the output a user gets.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn combined_harness(
    args: &Args,
    ast: &Ast,
    root: Id<TypeNode>,
    root_type: &str,
    filled: &fill::Fill,
    key: LayoutKey,
    holder_key: LayoutKey,
    holder_entry: Option<&str>,
    with_read: bool,
) -> Result<String, String> {
    let mut out = String::new();
    out.push_str(&format!(
        "/* The sample. Static, so the sequences below can point into file-scope\n\
         \x20  storage, nothing in the write path allocates, and the pointers the\n\
         \x20  reader hands it point into storage that outlives the read. */\n\
         static {root_type} zd_sample;\n\n"
    ));
    out.push_str(filled.declarations());
    out.push_str("\nstatic void zd_fill(void) {\n");
    out.push_str(filled.statements());
    out.push_str("}\n\n");

    let headers = key.pl_cdr1_headers;
    let max_align = args.encoding.max_alignment();
    let short_codes = key.length_codes.short_code_where_it_fits();
    let little = encapsulation_header(args.encoding, Endianness::Little);
    let big = encapsulation_header(args.encoding, Endianness::Big);

    out.push_str(&format!(
        "/* the flags a parameter id carries at each position, the highest short id,\n\
         \x20  padded length,\n\
         \x20  padded length for an @optional member outside a @mutable type,\n\
         \x20  the member's own must-understand bit */\n\
         static const zdw_pl_cdr1_framing zd_framing = {{ {}, {}, {}, {}, {}, {} }};\n\
         static uint8_t zd_buffer[{BUFFER_BYTES}];\n\n",
        c_pid_flags(key.pid_flags),
        key.short_pid_range.highest_short_id(),
        headers.pads_declared_length(),
        headers.pads_declared_optional_length(),
        key.member_pid_carries_must_understand(),
        headers.pads_after_value()
    ));

    // -- the write direction ------------------------------------------
    let encoder = codec_name(ast, root, Purpose::Data);
    out.push_str(&format!(
        "static int zd_write(zdw_endianness order, int encapsulate) {{\n\
         \x20   zdw_writer w;\n\
         \x20   /* The encapsulation identifier is big-endian on the wire whatever\n\
         \x20      the payload behind it is, and it is outside the CDR stream —\n\
         \x20      alignment inside the payload still counts from zero. */\n\
         \x20   uint8_t zd_encapsulation[4];\n\
         \x20   static const uint8_t zd_padding[3] = {{ 0, 0, 0 }};\n\
         \x20   size_t zd_pad;\n\
         \x20   zd_encapsulation[0] = order == ZDW_BIG ? 0x{:02x} : 0x{:02x};\n\
         \x20   zd_encapsulation[1] = order == ZDW_BIG ? 0x{:02x} : 0x{:02x};\n\
         \x20   zd_encapsulation[2] = 0;\n\
         \x20   zd_encapsulation[3] = 0;\n\
         \x20   zdw_writer_init(&w, zd_buffer, sizeof zd_buffer, order, {max_align});\n\
         \x20   zdw_writer_set_pl_cdr1(&w, zd_framing);\n\
         \x20   /* The half of the length-code policy a plan cannot answer: a\n\
         \x20      sequence whose count is in the value. `LengthCodePolicy`. */\n\
         \x20   zdw_writer_set_short_length_codes(&w, {short_codes});\n\
         \x20   {encoder}(&w, &zd_sample);\n\
         \x20   if (zdw_writer_status(&w) != ZDW_OK) {{\n\
         \x20       fprintf(stderr, \"wireprobe-zerodds-c: %s\\n\", zdw_status_name(zdw_writer_status(&w)));\n\
         \x20       return 1;\n\
         \x20   }}\n\
         \x20   /* XTypes 1.3 7.4.1.1 pads the serialized data to the next four-byte\n\
         \x20      boundary with zeros, and 7.6.3.1.2 states the count in the options\n\
         \x20      field so a receiver can find where the data ended. */\n\
         \x20   zd_pad = (4u - (zdw_writer_length(&w) & 3u)) & 3u;\n\
         \x20   zd_encapsulation[3] = (uint8_t)zd_pad;\n\
         \x20   if (encapsulate && fwrite(zd_encapsulation, 1, 4, stdout) != 4) {{\n\
         \x20       fprintf(stderr, \"wireprobe-zerodds-c: the bytes could not be written\\n\");\n\
         \x20       return 1;\n\
         \x20   }}\n\
         \x20   if (fwrite(zd_buffer, 1, zdw_writer_length(&w), stdout) != zdw_writer_length(&w)) {{\n\
         \x20       fprintf(stderr, \"wireprobe-zerodds-c: the bytes could not be written\\n\");\n\
         \x20       return 1;\n\
         \x20   }}\n\
         \x20   if (encapsulate && fwrite(zd_padding, 1, zd_pad, stdout) != zd_pad) {{\n\
         \x20       fprintf(stderr, \"wireprobe-zerodds-c: the bytes could not be written\\n\");\n\
         \x20       return 1;\n\
         \x20   }}\n\
         \x20   return 0;\n\
         }}\n\n",
        big[0], little[0], big[1], little[1]
    ));

    // -- the KeyHash ---------------------------------------------------
    match holder_entry {
        Some(entry) => {
            let form = key_hash_form(ast, root, holder_key)?;
            out.push_str(&format!(
                "static int zd_keyhash(void) {{\n\
                 \x20   zdw_writer w;\n\
                 \x20   uint8_t zd_hash[16];\n\
                 \x20   /* The holder is PLAIN_CDR2 big-endian whatever the sample is,\n\
                 \x20      so its alignment cap is PLAIN_CDR2's. XTypes 1.3 7.6.8. */\n\
                 \x20   zdw_writer_init(&w, zd_buffer, sizeof zd_buffer, ZDW_BIG, 4);\n\
                 \x20   {entry}(&w, &zd_sample);\n\
                 \x20   if (zdw_writer_status(&w) != ZDW_OK) {{\n\
                 \x20       fprintf(stderr, \"wireprobe-zerodds-c: %s\\n\", zdw_status_name(zdw_writer_status(&w)));\n\
                 \x20       return 1;\n\
                 \x20   }}\n\
                 \x20   {form}(&w, zd_hash);\n\
                 \x20   if (fwrite(zd_hash, 1, sizeof zd_hash, stdout) != sizeof zd_hash) {{\n\
                 \x20       fprintf(stderr, \"wireprobe-zerodds-c: the bytes could not be written\\n\");\n\
                 \x20       return 1;\n\
                 \x20   }}\n\
                 \x20   return 0;\n\
                 }}\n\n"
            ));
        }
        None => {
            // Not a compile-time hole: a type with no `@key` member has
            // one instance and no KeyHash, and every column answers
            // that the same way. Naming the type keeps the message the
            // one a caller can act on.
            out.push_str(&format!(
                "static int zd_keyhash(void) {{\n\
                 \x20   fprintf(stderr, \"wireprobe-zerodds-c: {} declares no @key member\\n\");\n\
                 \x20   return 1;\n\
                 }}\n\n",
                args.type_name.replace('\\', "\\\\").replace('"', "\\\"")
            ));
        }
    }

    // -- the read direction --------------------------------------------
    let decoder = decoder_name(ast, root);
    if !with_read {
        out.push_str(
            "static int zd_read(zdw_endianness order) {\n\
             \x20   (void)order;\n\
             \x20   fprintf(stderr, \"wireprobe-zerodds-c: this probe was built without the read direction\\n\");\n\
             \x20   return 1;\n\
             }\n\n",
        );
    }
    if with_read {
        let parts = dump::dump(ast, root, key.union_label)?;
        if !parts.functions.is_empty() {
            out.push_str(&parts.functions);
            out.push('\n');
        }
        out.push_str(&format!(
        "static int zd_read(zdw_endianness order) {{\n\
         \x20   static uint8_t zd_input[{BUFFER_BYTES}];\n\
         \x20   static uint8_t zd_heap[{HEAP_BYTES}];\n\
         \x20   zdw_reader r;\n\
         \x20   size_t zd_len;\n\
         \x20   size_t zd_left;\n\
         \x20   size_t zd_pad;\n\
         \x20   zd_len = fread(zd_input, 1, sizeof zd_input, stdin);\n\
         \x20   if (!feof(stdin)) {{\n\
         \x20       fprintf(stderr, \"wireprobe-zerodds-c: the payload is larger than the probe's buffer\\n\");\n\
         \x20       return 1;\n\
         \x20   }}\n\
         \x20   zdw_reader_init(&r, zd_input, zd_len, order, {max_align});\n\
         \x20   zdw_reader_set_pl_cdr1(&r, zd_framing);\n\
         \x20   zdw_reader_set_heap(&r, zd_heap, sizeof zd_heap);\n\
         \x20   /* The top-level type is not a member of anything, so no\n\
         \x20      `@try_construct` governs it and 7.2.2.7's default stands. */\n\
         \x20   {decoder}(&r, &zd_sample, ZDW_TC_DISCARD);\n\
         \x20   if (zdw_reader_status(&r) == ZDW_DISCARDED) {{\n\
         \x20       /* Not a failed decode. DDS-XTypes 1.3 7.2.2.7: the receiver\n\
         \x20          drops the object and carries on, and a caller that cannot\n\
         \x20          tell that from a broken peer cannot tell a narrowed type\n\
         \x20          from a corrupt stream. Exit code 3, the Rust probe's. */\n\
         \x20       fprintf(stderr, \"wireprobe-zerodds-c: at %lu: %s\\n\",\n\
         \x20               (unsigned long)zdw_reader_error_offset(&r),\n\
         \x20               zdw_status_name(zdw_reader_status(&r)));\n\
         \x20       return 3;\n\
         \x20   }}\n\
         \x20   if (zdw_reader_status(&r) != ZDW_OK) {{\n\
         \x20       const char *zd_fits = zdw_reader_fits(&r);\n\
         \x20       fprintf(stderr, \"wireprobe-zerodds-c: at %lu: %s%s%s\\n\",\n\
         \x20               (unsigned long)zdw_reader_error_offset(&r),\n\
         \x20               zdw_status_name(zdw_reader_status(&r)),\n\
         \x20               zd_fits == NULL ? \"\" : \", which is what \",\n\
         \x20               zd_fits == NULL ? \"\" : zd_fits);\n\
         \x20       return 1;\n\
         \x20   }}\n\
         \x20   /* Nothing may be left over that is not padding. A decoder that\n\
         \x20      stopped short and said nothing is the silent-drop shape: the\n\
         \x20      sample comes back looking plausible with its last members\n\
         \x20      missing. Padding is the one thing a writer legitimately\n\
         \x20      leaves behind, and it is what alignment asks for from this\n\
         \x20      offset and never more. */\n\
         \x20   zd_left = zdw_reader_remaining(&r);\n\
         \x20   zd_pad = ({max_align}u - (zdw_reader_position(&r) % {max_align}u)) % {max_align}u;\n\
         \x20   if (zd_left > zd_pad) {{\n\
         \x20       fprintf(stderr, \"wireprobe-zerodds-c: %lu byte(s) left at offset %lu, of which at most %lu could be padding\\n\",\n\
         \x20               (unsigned long)zd_left, (unsigned long)zdw_reader_position(&r),\n\
         \x20               (unsigned long)zd_pad);\n\
         \x20       return 1;\n\
         \x20   }}\n"
    ));
        for line in parts.body.lines() {
            out.push_str("    ");
            out.push_str(line);
            out.push('\n');
        }
        out.push_str("    return fflush(stdout) == 0 ? 0 : 1;\n}\n\n");
    }

    // -- the dispatch ---------------------------------------------------
    out.push_str(
        "int main(int argc, char **argv) {\n\
         \x20   const char *form = argc > 1 ? argv[1] : \"payload\";\n\
         \x20   /* Anything that is not `be` is little endian, and an unknown\n\
         \x20      form is refused rather than defaulted: a caller who mistyped\n\
         \x20      one has to be told, not handed the other form's bytes. */\n\
         \x20   zdw_endianness order = (argc > 2 && argv[2][0] == 'b') ? ZDW_BIG : ZDW_LITTLE;\n\
         \x20   if (strcmp(form, \"read\") == 0) {\n\
         \x20       return zd_read(order);\n\
         \x20   }\n\
         \x20   zd_fill();\n\
         \x20   if (strcmp(form, \"payload\") == 0) {\n\
         \x20       return zd_write(order, 0);\n\
         \x20   }\n\
         \x20   if (strcmp(form, \"payload+encapsulation\") == 0) {\n\
         \x20       return zd_write(order, 1);\n\
         \x20   }\n\
         \x20   if (strcmp(form, \"keyhash\") == 0) {\n\
         \x20       return zd_keyhash();\n\
         \x20   }\n\
         \x20   fprintf(stderr, \"wireprobe-zerodds-c: `%s` is not an emit form\\n\", form);\n\
         \x20   return 2;\n\
         }\n",
    );
    Ok(out)
}

/// The sample, the fill and `main`.
///
/// Written as text rather than as a tree: this is the harness around the
/// generated code, the counterpart of `tools/wireprobe/cyclone-template.c`,
/// and holding it as a CST would say it was part of the output a user
/// gets. What a user gets is `gen.h` and the codec above it.
#[allow(clippy::too_many_arguments)]
fn harness(
    args: &Args,
    ast: &Ast,
    root: Id<TypeNode>,
    root_type: &str,
    filled: &fill::Fill,
    key: LayoutKey,
    holder_key: LayoutKey,
) -> Result<String, String> {
    let mut out = String::new();
    out.push_str(&format!(
        "/* The sample. Static, so the sequences below can point into file-scope\n\
         \x20  storage and nothing in the write path allocates. */\n\
         static {root_type} zd_sample;\n\n"
    ));
    out.push_str(filled.declarations());
    out.push_str("\nstatic void zd_fill(void) {\n");
    out.push_str(filled.statements());
    out.push_str("}\n\n");

    let endianness = match args.endianness {
        Endianness::Little => "ZDW_LITTLE",
        Endianness::Big => "ZDW_BIG",
    };
    let encoder = codec_name(ast, root, Purpose::Data);
    // The four byte-level parts of the PL_CDR1 policy. The fifth —
    // which header form each member takes — changes the member's extent
    // and is therefore the layout plan's, not the writer's.
    let headers = key.pl_cdr1_headers;
    let framing = format!(
        "    /* the flags a parameter id carries at each position, the highest short id,\n\
         \x20  padded length,\n\
         \x20      padded length for an @optional member outside a @mutable type,\n\
         \x20      the member's own must-understand bit */\n\
         \x20   static const zdw_pl_cdr1_framing zd_framing = {{ {}, {}, {}, {}, {}, {} }};\n",
        c_pid_flags(key.pid_flags),
        key.short_pid_range.highest_short_id(),
        headers.pads_declared_length(),
        headers.pads_declared_optional_length(),
        key.member_pid_carries_must_understand(),
        headers.pads_after_value()
    );
    // The half of `LengthCodePolicy::ShortWhereItFits` a plan cannot
    // answer: a sequence or a map whose length is in the value. The
    // plan answers for every member whose type fixes the size.
    let short_codes = key.length_codes.short_code_where_it_fits();

    out.push_str(&format!(
        "int main(void) {{\n\
         \x20   static uint8_t zd_buffer[{BUFFER_BYTES}];\n\
         \x20   zdw_writer w;\n\
         \x20   size_t written;\n\
         \x20   zd_fill();\n"
    ));

    match args.emit {
        Emit::KeyHash => {
            let form = key_hash_form(ast, root, holder_key)?;
            // The holder takes the writer's default PL_CDR1 framing and
            // not the payload's. `wireprobe-zerodds` does the same, and
            // the framing is unreachable here anyway: the holder is
            // PLAIN_CDR2, which has no parameter list.
            out.push_str(&format!(
                "    /* The holder is PLAIN_CDR2 big-endian whatever the sample is,\n\
                 \x20      so its alignment cap is PLAIN_CDR2's. XTypes 1.3 7.6.8. */\n\
                 \x20   uint8_t zd_hash[16];\n\
                 \x20   zdw_writer_init(&w, zd_buffer, sizeof zd_buffer, ZDW_BIG, 4);\n\
                 \x20   zd_keyholder_{root_type}(&w, &zd_sample);\n\
                 \x20   if (zdw_writer_status(&w) != ZDW_OK) {{\n\
                 \x20       fprintf(stderr, \"wireprobe-zerodds-c: %s\\n\", zdw_status_name(zdw_writer_status(&w)));\n\
                 \x20       return 1;\n\
                 \x20   }}\n\
                 \x20   {form}(&w, zd_hash);\n\
                 \x20   written = fwrite(zd_hash, 1, sizeof zd_hash, stdout);\n\
                 \x20   if (written != sizeof zd_hash) {{\n\
                 \x20       fprintf(stderr, \"wireprobe-zerodds-c: the bytes could not be written\\n\");\n\
                 \x20       return 1;\n\
                 \x20   }}\n\
                 \x20   return 0;\n\
                 }}\n"
            ));
            return Ok(out);
        }
        // The header cannot go out before the payload any more: two of
        // its four bytes depend on the payload's length. §7.6.3.1.2 puts
        // the count of trailing padding bytes in the low two bits of the
        // second options byte, so the declaration is emitted here and
        // the write happens below, once `zdw_writer_length` knows it.
        Emit::PayloadWithEncapsulation => {
            let header = encapsulation_header(args.encoding, args.endianness);
            out.push_str(&format!(
                "    /* The encapsulation identifier is big-endian on the wire whatever\n\
                 \x20      the payload behind it is, and it is outside the CDR stream —\n\
                 \x20      alignment inside the payload still counts from zero. */\n\
                 \x20   uint8_t zd_encapsulation[4] = {{ 0x{:02x}, 0x{:02x}, 0x{:02x}, 0x{:02x} }};\n\
                 \x20   static const uint8_t zd_padding[3] = {{ 0, 0, 0 }};\n\
                 \x20   size_t zd_pad;\n",
                header[0], header[1], header[2], header[3]
            ));
        }
        Emit::Payload => {}
    }

    out.push_str(&format!(
        "    zdw_writer_init(&w, zd_buffer, sizeof zd_buffer, {endianness}, {});\n\
         {framing}\
         \x20   zdw_writer_set_pl_cdr1(&w, zd_framing);\n\
         \x20   /* The half of the length-code policy a plan cannot answer: a\n\
         \x20      sequence whose count is in the value. `LengthCodePolicy`. */\n\
         \x20   zdw_writer_set_short_length_codes(&w, {short_codes});\n\
         \x20   {encoder}(&w, &zd_sample);\n\
         \x20   if (zdw_writer_status(&w) != ZDW_OK) {{\n\
         \x20       fprintf(stderr, \"wireprobe-zerodds-c: %s\\n\", zdw_status_name(zdw_writer_status(&w)));\n\
         \x20       return 1;\n\
         \x20   }}\n",
        args.encoding.max_alignment()
    ));

    if args.emit == Emit::PayloadWithEncapsulation {
        out.push_str(
            "    /* XTypes 1.3 7.4.1.1 pads the serialized data to the next four-byte\n\
             \x20      boundary with zeros, and 7.6.3.1.2 states the count in the options\n\
             \x20      field so a receiver can find where the data ended. Cyclone, RTI and\n\
             \x20      Fast DDS all write both. */\n\
             \x20   zd_pad = (4u - (zdw_writer_length(&w) & 3u)) & 3u;\n\
             \x20   zd_encapsulation[3] = (uint8_t)(zd_encapsulation[3] | (uint8_t)zd_pad);\n\
             \x20   if (fwrite(zd_encapsulation, 1, 4, stdout) != 4) {\n\
             \x20       fprintf(stderr, \"wireprobe-zerodds-c: the bytes could not be written\\n\");\n\
             \x20       return 1;\n\
             \x20   }\n",
        );
    }

    out.push_str(
        "    written = fwrite(zd_buffer, 1, zdw_writer_length(&w), stdout);\n\
         \x20   if (written != zdw_writer_length(&w)) {\n\
         \x20       fprintf(stderr, \"wireprobe-zerodds-c: the bytes could not be written\\n\");\n\
         \x20       return 1;\n\
         \x20   }\n",
    );

    if args.emit == Emit::PayloadWithEncapsulation {
        out.push_str(
            "    if (fwrite(zd_padding, 1, zd_pad, stdout) != zd_pad) {\n\
             \x20       fprintf(stderr, \"wireprobe-zerodds-c: the bytes could not be written\\n\");\n\
             \x20       return 1;\n\
             \x20   }\n",
        );
    }

    out.push_str("    return 0;\n}\n");
    Ok(out)
}

/// The functions `entry` reaches, and nothing else.
///
/// # Why only the KeyHash run gets this
///
/// A key holder writes the `@key` members and no others, so the
/// encoders for every type only a *non-key* member reaches are defined
/// and never called. `combined-keyed-topic` has three of them —
/// `Position`, `Mode` and `sequence<double>` — and `-Wunused-function`
/// names all three.
///
/// The payload run is deliberately **not** pruned. There the same
/// warning is a defect detector: `sequence<double>` and
/// `sequence<long double>` merging onto one generated type left one of
/// the two element encoders unreferenced, and that is how the collision
/// was found. A reachability pass would have dropped the evidence and
/// the second sequence would have gone out at the first's width with
/// nothing to say it had. The corpus runs the payload path for every
/// case, so the detector stays where it earns its keep.
fn reachable_from(decls: Vec<c::Decl>, entry: &str) -> Vec<c::Decl> {
    let mut wanted = vec![entry.to_string()];
    let mut index = 0;
    while index < wanted.len() {
        let name = wanted[index].clone();
        index += 1;
        for decl in &decls {
            let c::Decl::Function(function) = decl else {
                continue;
            };
            if function.name.as_str() != name {
                continue;
            }
            let Some(body) = &function.body else { continue };
            for called in calls_in_block(body) {
                if !wanted.contains(&called) {
                    wanted.push(called);
                }
            }
        }
    }
    decls
        .into_iter()
        .filter(|decl| match decl {
            c::Decl::Function(function) => wanted.iter().any(|kept| kept == function.name.as_str()),
            _ => true,
        })
        .collect()
}

/// Every name called by direct reference inside a block.
///
/// Direct calls only — the generated C has no function pointers, so a
/// call through anything else would be a shape this generator does not
/// produce and pruning it away would be a defect rather than a tidy-up.
///
/// zerodds-lint: recursion-depth = the nesting of the C this generator
/// emits. `c::Block`, `c::Statement` and `c::Expr` are an owned tree —
/// no variant holds a shared or back reference — so the walk cannot
/// cycle, and the tree it walks is the one the emitters beside it
/// built: one level per array dimension and per `for` the printer
/// opens.
fn calls_in_block(block: &c::Block) -> Vec<String> {
    let mut out = Vec::new();
    for statement in &block.statements {
        calls_in_statement(statement, &mut out);
    }
    out
}

/// zerodds-lint: recursion-depth = the nesting of the C this generator
/// emits. `c::Block`, `c::Statement` and `c::Expr` are an owned tree —
/// no variant holds a shared or back reference — so the walk cannot
/// cycle, and the tree it walks is the one the emitters beside it
/// built: one level per array dimension and per `for` the printer
/// opens.
fn calls_in_statement(statement: &c::Statement, out: &mut Vec<String>) {
    match statement {
        c::Statement::Declare { value, .. } => {
            if let Some(value) = value {
                calls_in_expr(value, out);
            }
        }
        c::Statement::Expr(value) => calls_in_expr(value, out),
        c::Statement::Return(value) => {
            if let Some(value) = value {
                calls_in_expr(value, out);
            }
        }
        c::Statement::If {
            condition,
            then,
            otherwise,
        } => {
            calls_in_expr(condition, out);
            out.extend(calls_in_block(then));
            if let Some(block) = otherwise {
                out.extend(calls_in_block(block));
            }
        }
        c::Statement::For {
            init,
            condition,
            step,
            body,
        } => {
            calls_in_statement(init, out);
            calls_in_expr(condition, out);
            calls_in_expr(step, out);
            out.extend(calls_in_block(body));
        }
        c::Statement::While { condition, body } => {
            calls_in_expr(condition, out);
            out.extend(calls_in_block(body));
        }
        c::Statement::Break | c::Statement::Comment(_) => {}
    }
}

/// zerodds-lint: recursion-depth = the nesting of the C this generator
/// emits. `c::Block`, `c::Statement` and `c::Expr` are an owned tree —
/// no variant holds a shared or back reference — so the walk cannot
/// cycle, and the tree it walks is the one the emitters beside it
/// built: one level per array dimension and per `for` the printer
/// opens.
fn calls_in_expr(expr: &c::Expr, out: &mut Vec<String>) {
    match expr {
        c::Expr::Call { callee, args } => {
            if let c::Expr::Name(name) = callee.as_ref() {
                out.push(name.as_str().to_string());
            } else {
                calls_in_expr(callee, out);
            }
            for arg in args {
                calls_in_expr(arg, out);
            }
        }
        c::Expr::Member { value, .. }
        | c::Expr::Arrow { value, .. }
        | c::Expr::Unary { value, .. }
        | c::Expr::AddressOf(value)
        | c::Expr::Deref(value)
        | c::Expr::Paren(value) => calls_in_expr(value, out),
        c::Expr::Index { value, index } => {
            calls_in_expr(value, out);
            calls_in_expr(index, out);
        }
        c::Expr::Binary { lhs, rhs, .. } => {
            calls_in_expr(lhs, out);
            calls_in_expr(rhs, out);
        }
        c::Expr::Cast { value, .. } => calls_in_expr(value, out),
        c::Expr::Initializer(items) => {
            for item in items {
                calls_in_expr(item, out);
            }
        }
        c::Expr::Literal(_) | c::Expr::Name(_) | c::Expr::SizeOf(_) => {}
    }
}

/// The read direction's `main`: bytes on standard input, canonical JSON
/// on standard output.
///
/// Standard input and not a path baked into the generated C: the path
/// would be part of the program, so a probe compiled for one payload
/// could not be run against another, and the shell wrapper would have
/// to regenerate for every byte string it wanted read.
fn read_harness(
    args: &Args,
    ast: &Ast,
    root: Id<TypeNode>,
    root_type: &str,
    key: LayoutKey,
) -> Result<String, String> {
    let mut out = String::new();
    out.push_str(&format!(
        "/* The sample the decoder fills. Static, so the pointers the reader\n\
         \x20  hands it point into storage that outlives the read. */\n\
         static {root_type} zd_sample;\n\n"
    ));

    let endianness = match args.endianness {
        Endianness::Little => "ZDW_LITTLE",
        Endianness::Big => "ZDW_BIG",
    };
    let parts = dump::dump(ast, root, key.union_label)?;
    if !parts.functions.is_empty() {
        out.push_str(&parts.functions);
        out.push('\n');
    }
    let decoder = decoder_name(ast, root);
    let headers = key.pl_cdr1_headers;
    let max_align = args.encoding.max_alignment();
    let strip = usize::from(args.emit == Emit::PayloadWithEncapsulation) * 4;
    // Declared only where it is read: the C probe is compiled with
    // `-Wall -Wextra`, and a generator that produced warnings would
    // train its users to ignore them.
    let declared = if strip > 0 {
        "    size_t zd_declared;\n"
    } else {
        ""
    };

    out.push_str(&format!(
        "int main(void) {{\n\
         \x20   static uint8_t zd_input[{BUFFER_BYTES}];\n\
         \x20   static uint8_t zd_heap[{HEAP_BYTES}];\n\
         \x20   const uint8_t *zd_at = zd_input;\n\
         \x20   size_t zd_len;\n\
         \x20   size_t zd_left;\n\
         \x20   size_t zd_pad;\n\
         {declared}\
         \x20   zdw_reader r;\n\
         \x20   /* the flags a parameter id carries at each position, the highest short id,\n\
         \x20  padded length,\n\
         \x20      padded length for an @optional member outside a @mutable type,\n\
         \x20      the member's own must-understand bit */\n\
         \x20   static const zdw_pl_cdr1_framing zd_framing = {{ {}, {}, {}, {}, {}, {} }};\n\
         \x20   zd_len = fread(zd_input, 1, sizeof zd_input, stdin);\n\
         \x20   if (!feof(stdin)) {{\n\
         \x20       fprintf(stderr, \"wireprobe-zerodds-c: the payload is larger than the probe's buffer\\n\");\n\
         \x20       return 1;\n\
         \x20   }}\n",
        c_pid_flags(key.pid_flags),
        key.short_pid_range.highest_short_id(),
        headers.pads_declared_length(),
        headers.pads_declared_optional_length(),
        key.member_pid_carries_must_understand(),
        headers.pads_after_value()
    ));

    if strip > 0 {
        // The encapsulation identifier is not re-read for the encoding:
        // the caller named one, and a probe that took the stream's word
        // for it could not be pointed at a stream to test what it does
        // with a header it disagrees with.
        //
        // The **options** word is read, because 7.6.3.1.2 says a
        // receiver shall use its low two bits to find where the
        // serialized data ended. Without them the trailing padding is
        // handed to the decoder as data.
        out.push_str(&format!(
            "    if (zd_len < {strip}u) {{\n\
             \x20       fprintf(stderr, \"wireprobe-zerodds-c: fewer than the four-byte encapsulation header\\n\");\n\
             \x20       return 1;\n\
             \x20   }}\n\
             \x20   zd_declared = (size_t)(zd_at[3] & 3u);\n\
             \x20   zd_at += {strip}u;\n\
             \x20   zd_len -= {strip}u;\n\
             \x20   if (zd_len < zd_declared) {{\n\
             \x20       fprintf(stderr, \"wireprobe-zerodds-c: the options field declares %lu padding byte(s) \"\n\
             \x20                       \"and only %lu follow the header\\n\",\n\
             \x20               (unsigned long)zd_declared, (unsigned long)zd_len);\n\
             \x20       return 1;\n\
             \x20   }}\n\
             \x20   zd_len -= zd_declared;\n"
        ));
    }

    out.push_str(&format!(
        "    zdw_reader_init(&r, zd_at, zd_len, {endianness}, {max_align});\n\
         \x20   zdw_reader_set_pl_cdr1(&r, zd_framing);\n\
         \x20   zdw_reader_set_heap(&r, zd_heap, sizeof zd_heap);\n\
         \x20   /* The top-level type is not a member of anything, so no\n\
         \x20      `@try_construct` governs it and 7.2.2.7's default stands. */\n\
         \x20   {decoder}(&r, &zd_sample, ZDW_TC_DISCARD);\n\
         \x20   if (zdw_reader_status(&r) == ZDW_DISCARDED) {{\n\
         \x20       /* Not a failed decode. DDS-XTypes 1.3 7.2.2.7: the\n\
         \x20          receiver drops the object and carries on, and a\n\
         \x20          caller that cannot tell that from a broken peer\n\
         \x20          cannot tell a narrowed type from a corrupt stream.\n\
         \x20          Exit code 3, the same one the Rust probe uses. */\n\
         \x20       fprintf(stderr, \"wireprobe-zerodds-c: at %lu: %s\\n\",\n\
         \x20               (unsigned long)zdw_reader_error_offset(&r),\n\
         \x20               zdw_status_name(zdw_reader_status(&r)));\n\
         \x20       return 3;\n\
         \x20   }}\n\
         \x20   if (zdw_reader_status(&r) != ZDW_OK) {{\n\
         \x20       const char *zd_fits = zdw_reader_fits(&r);\n\
         \x20       fprintf(stderr, \"wireprobe-zerodds-c: at %lu: %s%s%s\\n\",\n\
         \x20               (unsigned long)zdw_reader_error_offset(&r),\n\
         \x20               zdw_status_name(zdw_reader_status(&r)),\n\
         \x20               zd_fits == NULL ? \"\" : \", which is what \",\n\
         \x20               zd_fits == NULL ? \"\" : zd_fits);\n\
         \x20       return 1;\n\
         \x20   }}\n"
    ));

    // Nothing may be left over that is not padding. A decoder that
    // stopped short and said nothing is the silent-drop shape this
    // suite exists to find: the sample would come back looking
    // plausible with its last members missing. Padding is the one thing
    // a writer legitimately leaves behind, and it is what alignment
    // asks for from this offset and never more.
    out.push_str(&format!(
        "    zd_left = zdw_reader_remaining(&r);\n\
         \x20   zd_pad = ({max_align}u - (zdw_reader_position(&r) % {max_align}u)) % {max_align}u;\n\
         \x20   if (zd_left > zd_pad) {{\n\
         \x20       fprintf(stderr, \"wireprobe-zerodds-c: %lu byte(s) left at offset %lu, of which at most %lu could be padding\\n\",\n\
         \x20               (unsigned long)zd_left, (unsigned long)zdw_reader_position(&r),\n\
         \x20               (unsigned long)zd_pad);\n\
         \x20       return 1;\n\
         \x20   }}\n"
    ));

    for line in parts.body.lines() {
        out.push_str("    ");
        out.push_str(line);
        out.push('\n');
    }
    out.push_str("    return fflush(stdout) == 0 ? 0 : 1;\n}\n");
    Ok(out)
}

/// Which of the two KeyHash forms the holder takes.
///
/// The maximum holder size, including bounded variable lengths: sixteen
/// bytes or fewer and the holder *is* the hash, zero-padded; longer or unbounded and
/// it is the MD5 of it. DDSI-RTPS 2.5 §9.6.3.8.
fn key_hash_form(
    ast: &Ast,
    root: Id<TypeNode>,
    holder_key: LayoutKey,
) -> Result<&'static str, String> {
    let form = zerodds_ast_layout::keyhash::key_hash_form_for(ast, root, holder_key)
        .map_err(|error| format!("key-holder maximum size: {error}"))?;
    Ok(match form {
        zerodds_ast_layout::keyhash::KeyHashForm::ZeroPadded => "zdw_zero_padded",
        zerodds_ast_layout::keyhash::KeyHashForm::Md5 => "zdw_md5_of",
    })
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
