// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-wireprobe-java-gen`. Safety classification: **TOOLING**.
//!
//! `wireprobe-zerodds-java-gen` — the Java compilation unit the Java
//! probe compiles.
//!
//! The Java spelling and nothing else. [`zerodds_wireprobe_gen`] is the
//! stem: the command line, the IDL, the model, the layout key, the
//! sample, the files and the exit codes are there, once, for all five
//! columns.
//!
//! A `ZdTypes.java` of declarations and a `Probe.java` of codec methods,
//! the statements that fill one sample, and a `main` that writes the
//! bytes to standard output. `tools/wireprobe/wireprobe-zerodds-java` is
//! the shell around it.
//!
//! Nothing here writes a byte. The declarations come from the Java
//! materialiser, the codec from the shared walk through
//! [`zerodds_lang_rules::emit::java::Java`], and the bytes come out of
//! `runtime/java` when the compiled program runs. `runtime/java`
//! implements DDS-XTypes 1.3 §7.4 from the clause and calls nothing in
//! Rust, which is what makes this a second reading of the specification
//! rather than a rendering of the first one's answer.
//!
//! ## Both directions
//!
//! `--values` generates the write direction: the sample is filled from
//! the JSON and the octets go to standard output. `--read` generates the
//! read direction: the octets arrive on standard input and the values go
//! to standard output as the canonical JSON of
//! `crates/wire-probe/src/canonical.rs`. The two share the codec walk,
//! the declarations and the runtime; what differs is which half of
//! `emit/` is asked for and what `main` does with it. They are
//! exclusive, and the stem is what refuses both at once — this column
//! used to take both and silently generate the reader.
//!
//! The read direction also carries the Table 9 defaulters —
//! `zerodds_lang_rules::default_java` — which is the one step the shared
//! walk cannot take: which helper a target needs for a member the stream
//! did not carry is the materialiser's answer, and C prepends its own in
//! the same place.
//!
//! # One program for every form and both byte orders
//!
//! [`sources`] bakes the direction, the emit form and the byte order into
//! the program, which is right for a probe run once and wrong for
//! `wireprobe-java-matrix`: a JVM launch costs far more than a C process
//! launch, and `crates/wire-probe/tests/reference_columns.rs` wants this
//! column over the whole matrix — twelve configurations, five encodings,
//! both byte orders, four emit forms, 420 slots per case.
//!
//! So [`combined_sources`] emits one program per case, configuration and
//! encoding, and that program answers for every form and both orders.
//! [`answer_stem`] is how it and the driver agree on where an answer
//! goes. Byte order moves out because the plan does not depend on it —
//! `runtime/java`'s `Writer` and `Reader` both take the order as a
//! constructor argument — and the emit form moves out because one `main`
//! dispatches on it, which `combined_harness` argues.
#![forbid(unsafe_code)]

mod dump;
mod fill;

use std::collections::BTreeMap;
use std::fmt::Write as _;
use zerodds_ast::{Ast, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ast_layout::{
    Encoding, Endianness, KeyHolderNesting, LayoutKey, PidFlagPolicy, Purpose,
};
use zerodds_lang_rules::decode_java::{decoder_name, java_decoder_from};
use zerodds_lang_rules::emit::encode::{codec_from, codec_name, key_holder};
use zerodds_lang_rules::emit::java::Java;
use zerodds_lang_rules::encode::EncodeError;
use zerodds_lang_rules::rule;
use zerodds_lang_rules::{ModelGraph, apply};
pub use zerodds_wireprobe_gen::{Args, Emit};
use zerodds_wireprobe_gen::{Column, File, Refusal, Session, gap};

/// The Java column.
#[derive(Debug, Clone, Copy)]
pub struct Renderer;

impl Column for Renderer {
    const BINARY: &'static str = "wireprobe-zerodds-java-gen";
    const LANGUAGE: &'static str = "Java";
    const OUTPUT: &'static str = "ZdTypes.java and Probe.java go";
    const FORMS: &'static [Emit] = &[Emit::Payload, Emit::PayloadWithEncapsulation, Emit::KeyHash];

    fn render(session: &Session<'_>) -> Result<Vec<File>, Refusal> {
        let sources = sources(session)?;
        Ok(vec![
            File::new("ZdTypes.java", sources.types),
            File::new("Probe.java", sources.probe),
        ])
    }

    fn combined(session: &Session<'_>, with_read: bool) -> Result<Vec<File>, Refusal> {
        let sources = combined_sources_from(session, with_read)?;
        Ok(vec![
            File::new("ZdTypes.java", sources.types),
            File::new("Probe.java", sources.probe),
        ])
    }
}

/// The two Java files.
#[derive(Debug)]
pub struct Sources {
    /// The declarations.
    pub types: String,
    /// The codec, the fill and `main`.
    pub probe: String,
}

/// This column writes and reads the public fields of the generated types
/// (`fill.rs`, `dump.rs`): the IDL-to-Java mapping, which is asked for by
/// name because the default of the generator is Clause 9.
fn omg<R>(body: impl FnOnce() -> R) -> R {
    zerodds_lang_rules::adapter::java::with_mapping(
        zerodds_lang_rules::adapter::java::JavaMapping::Omg,
        body,
    )
}

/// The declarations, which every form of one generation shares.
///
/// Split out of [`sources`] because the combined unit below builds the
/// same file and nothing else of it: the types do not depend on the
/// direction, the emit form or the byte order, only on the model.
///
/// # Errors
/// [`Refusal::NoForm`] from the materialiser, and [`Refusal::Gap`] from
/// the rule catalogue.
fn declarations(session: &Session<'_>) -> Result<String, Refusal> {
    let ast = &session.ast;
    let graph = ModelGraph::of(ast);
    let applied = apply(&graph, &rule::java::catalogue()).map_err(|error| gap!("{error:?}"))?;
    // The materialiser is the authority on what Java has a form for, so
    // its refusal is exit 3 and every other stop is exit 6.
    let decls = zerodds_lang_rules::adapter::java::java_items(ast, &applied)
        .map_err(|error| Refusal::NoForm(format!("{error:?}")))?;
    Ok(zerodds_lang_render::java::file(
        &zerodds_lang_cst::java::File {
            doc: zerodds_lang_cst::common::Doc::lines(&[
                "Generated by wireprobe-zerodds-java-gen. Do not edit.",
            ]),
            package: None,
            imports: Vec::new(),
            types: decls,
        },
    ))
}

/// Generate both files.
///
/// # Errors
/// [`Refusal`], classified so the command can answer in the verdict's
/// own exit code.
pub fn sources(session: &Session<'_>) -> Result<Sources, Refusal> {
    omg(|| sources_omg(session))
}

fn sources_omg(session: &Session<'_>) -> Result<Sources, Refusal> {
    let ast = &session.ast;
    let root = session.root;
    let types = declarations(session)?;

    let key = session.key;
    // What the chosen direction calls, and nothing else. A method the
    // probe never reaches is dead source in a file `javac` compiles with
    // `-Xlint:all -Werror`, and generating both halves would double the
    // compile of every case in the corpus sweep.
    let members = if session.args.read {
        // The Table 9 defaulters come with the decoders: which helper a
        // member the stream did not carry needs is this column's answer,
        // and `decode_java` prepends them in the one place it belongs.
        java_decoder_from(ast, root, key, session.codec_options)
            .map_err(|error| from_codec(&error))?
    } else if session.args.emit == Emit::KeyHash {
        key_hash_codec(session)?
    } else {
        codec_from::<Java>(ast, root, key, session.codec_options)
            .map_err(|error| from_codec(&error))?
    };
    let codec = codec_class(members);

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

/// One generated codec, rendered as a class the harnesses splice into
/// `Probe`.
///
/// `type_decl` renders `public class ZdCodec { … }` and what a harness
/// wants is the members at one level of indentation: the wrapper's own
/// two lines are the class the renderer needs its signature for and never
/// reach the file.
fn codec_class(members: Vec<zerodds_lang_cst::java::Member>) -> String {
    zerodds_lang_render::java::type_decl(&zerodds_lang_cst::java::TypeDecl::Class(
        zerodds_lang_cst::java::Class {
            doc: zerodds_lang_cst::common::Doc::none(),
            annotations: Vec::new(),
            modifiers: vec![zerodds_lang_cst::java::Modifier::Public],
            name: zerodds_lang_cst::common::Ident::new("ZdCodec"),
            generics: Vec::new(),
            extends: None,
            implements: Vec::new(),
            members,
        },
    ))
}

/// The members of `codec`, pasted into `Probe`.
///
/// [`codec_class`] renders the wrapper's opening line and its closing
/// brace, and neither belongs in the probe: the methods are what a
/// `Probe` with a `main` holds.
fn splice_codec(out: &mut String, codec: &str) {
    for line in codec.lines().skip(1) {
        if line == "}" {
            continue;
        }
        let _ = writeln!(out, "{line}");
    }
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

/// One program per case, configuration and encoding, answering every
/// emit form in both byte orders.
///
/// [`sources`] bakes the direction, the emit form and the byte order into
/// the program, which is right for a probe run once and wrong for
/// `wireprobe-java-matrix`: that driver measures the whole matrix, and
/// for this column the cost is the **number of JVM launches**. A JVM
/// starts in tens of milliseconds where a C process starts in two, the
/// launch does not parallelise the way a signature check does not, and
/// every one of the seven answers a unit covers would otherwise be a
/// program of its own. Not measured for this column.
///
/// So this emits one program per case, configuration and encoding, and
/// the program answers for both orders and all four forms. Byte order
/// can move out because the plan does not depend on it — DDS-XTypes 1.3
/// puts member offsets in the type and the order in the stream, and
/// `LayoutKey`'s own documentation says offsets are invariant under it
/// — and `runtime/java`'s `Writer` and `Reader` both take the order as a
/// constructor argument, so the generated codec has nothing to bake in.
/// The emit form moves out because this unit carries all three codec sets
/// at once: the payload encoder, which answers `payload` and
/// `payload+encapsulation` behind a flag; the key holder, which answers
/// `keyhash`; and the decoder, which answers `read`.
///
/// What is kept from the single-form generator is the payload codec whole
/// — every element encoder, reached or not — because a member of a
/// generated class nothing calls is the one defect detector this column
/// has: `javac` says nothing about it, and the reachability pass that
/// would hide it is not written.
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
/// every form in one translation unit means one broken form takes the
/// other three with it, and a column that reported "no probe" for a
/// payload it writes perfectly well would understate itself and hide the
/// actual defect behind a bigger one. So the batch driver retries
/// without the decoder and files the read slots, and only those, under the
/// compiler's message.
///
/// # Errors
/// [`Refusal`], classified.
pub fn combined_sources_with(args: &Args, with_read: bool) -> Result<Sources, Refusal> {
    let session = Session::open(args)?;
    combined_sources_from(&session, with_read)
}

/// Whether the type `args` names declares a `@key` member, so that it
/// has a KeyHash at all.
///
/// A type with no `@key` member has one instance and no KeyHash whatever
/// its sample holds (XTypes 1.3 §7.6.8), so a unit whose Java cannot be
/// generated for a reason in the sample still answers its KeyHash slot
/// with that, as the Rust column does. A model whose data codec cannot be
/// planned is refused for that instead, again as the Rust column does.
/// The same function as `zerodds_wireprobe_cpp_gen::declares_a_key`,
/// where `reference_columns.rs` measured the difference on the C++
/// column; not measured on this one.
///
/// # Errors
/// [`Refusal`], classified, when the model cannot be read or its data
/// codec cannot be planned.
pub fn declares_a_key(args: &Args) -> Result<bool, Refusal> {
    omg(|| declares_a_key_omg(args))
}

fn declares_a_key_omg(args: &Args) -> Result<bool, Refusal> {
    let session = Session::open(args)?;
    codec_from::<Java>(
        &session.ast,
        session.root,
        session.key,
        session.codec_options,
    )
    .map_err(|error| from_codec(&error))?;
    Ok(
        key_holder::<Java>(&session.ast, session.root, session.holder_key)
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
    omg(|| combined_sources_from_omg(session, with_read))
}

fn combined_sources_from_omg(session: &Session<'_>, with_read: bool) -> Result<Sources, Refusal> {
    let ast = &session.ast;
    let root = session.root;
    let key = session.key;
    let holder_key = session.holder_key;
    let options = session.codec_options;
    let types = declarations(session)?;

    let mut codec =
        codec_from::<Java>(ast, root, key, options).map_err(|error| from_codec(&error))?;
    if with_read {
        codec.extend(
            java_decoder_from(ast, root, key, options).map_err(|error| from_codec(&error))?,
        );
    }

    // The holder, when the type has one. A keyless type is not an error
    // here — three of the four forms still work and the fourth reports
    // that there is no KeyHash to take, which is what every other column
    // answers for it.
    let holder_entry =
        match key_holder::<Java>(ast, root, holder_key).map_err(|error| from_codec(&error))? {
            Some(zerodds_lang_cst::java::Member::Method(method)) => {
                let entry = method.name.as_str().to_string();
                let mut under_holder = codec_from::<Java>(ast, root, holder_key, options)
                    .map_err(|error| from_codec(&error))?;
                // §7.6.8 Step 1 removes a nested aggregate's non-key
                // members only "if there are any key members", and RTI
                // under XCDR1 reads the consequence as "and below a
                // keyless one, stop filtering altogether", so the
                // subtree there is written as the data has it and
                // `keyhash::member_key` names `zd_enc_` at those
                // positions. Both sets have to be there for the file to
                // compile; [`deduplicate`] keeps the overlap from
                // becoming two definitions of one signature.
                if holder_key.key_holder_nesting == KeyHolderNesting::KeylessNestedWhole {
                    let as_data = LayoutKey {
                        purpose: Purpose::Data,
                        ..holder_key
                    };
                    under_holder.extend(
                        codec_from::<Java>(ast, root, as_data, options)
                            .map_err(|error| from_codec(&error))?,
                    );
                }
                under_holder.push(zerodds_lang_cst::java::Member::Method(method));
                codec.extend(under_holder);
                Some(entry)
            }
            Some(_) => return Err(gap!("the key holder is not a method")),
            None => None,
        };
    let codec = codec_class(deduplicate(codec)?);
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
/// definitions of one signature that are not the same text.
///
/// The three codec sets overlap: under
/// [`KeyHolderNesting::KeylessNestedWhole`] the holder's subtree is
/// written with the data-purpose encoders, which are the ones the payload
/// set already holds, and Java has no implicit redeclaration to merge two
/// of them the way a prototype and its definition merge. Keeping both
/// would be two methods of one signature, which does not compile.
///
/// Two definitions of one signature that differ is the case that must not
/// pass quietly — it would mean one of the two plans is being silently
/// dropped, and the probe would write octets from a plan nobody chose.
///
/// # Errors
/// A message naming the method, as [`Refusal::Gap`].
fn deduplicate(
    members: Vec<zerodds_lang_cst::java::Member>,
) -> Result<Vec<zerodds_lang_cst::java::Member>, Refusal> {
    let mut seen: BTreeMap<String, String> = BTreeMap::new();
    let mut out = Vec::with_capacity(members.len());
    for member in members {
        let rendered = codec_class(vec![member.clone()]);
        match seen.get(&signature(&member)) {
            Some(before) if *before == rendered => continue,
            Some(_) => {
                return Err(gap!(
                    "`{}` is defined twice and the two definitions differ; one of the plans \
                     would be silently dropped",
                    signature(&member)
                ));
            }
            None => {
                seen.insert(signature(&member), rendered);
                out.push(member);
            }
        }
    }
    Ok(out)
}

/// What two definitions of one member would collide on.
///
/// The name, the parameter types and the return type, because a Java
/// method is **overloaded**: `zd_enc_int(Writer, int)` and
/// `zd_enc_int(Writer, long)` are two methods and both belong in the
/// file, and a key of the name alone would drop one of them.
///
/// The body is not part of it. A declaration without one is not a
/// signature this generator emits — every method it builds carries its
/// body — and if it ever did, `javac` names the method and the report
/// does not have to.
///
/// Anything that is not a method is keyed by its own text, which is
/// enough: the codec walk emits methods and nothing else.
fn signature(member: &zerodds_lang_cst::java::Member) -> String {
    let zerodds_lang_cst::java::Member::Method(method) = member else {
        return codec_class(vec![member.clone()]);
    };
    let params = method
        .params
        .iter()
        .map(|param| zerodds_lang_render::java::type_(&param.type_))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{}({params}) : {}",
        method.name.as_str(),
        zerodds_lang_render::java::type_(&method.returns)
    )
}

/// The stem of the file one answer of a batch run goes to.
///
/// Where the generator writes an answer and where the batch driver reads
/// it are two programs that never see each other's source, so the rule is
/// spelled once and here: the slot's own name, with `/` made into `-`
/// because `/` is a path separator and the read slots carry one. The
/// answer itself is `<stem>.bin` and a refusal is `<stem>.err`.
///
/// The read direction has no file of its own: its document goes to
/// standard output, because the JSON dialect in `tools/wireprobe/Zdj.java`
/// writes to `FileDescriptor.out` and no Java API redirects it.
#[must_use]
pub fn answer_stem(slot: &str) -> String {
    slot.replace('/', "-")
}

/// `Probe.java` for the combined program: the sample, the fill, the five
/// octet answers, the read direction, and a dispatch on `argv`.
///
/// Written as text for the reason [`harness`] gives — this is the harness
/// *around* the generated code, and holding it as a CST would say it was
/// part of the output a user gets.
///
/// ## What one run answers, and what it costs
///
/// ```text
/// Probe batch <dir>     the payload in both orders, the same behind its
///                       encapsulation header, and the KeyHash: five files
/// Probe read le|be      the bytes on standard input, the document on
///                       standard output
/// ```
///
/// So a unit is three JVM launches and not seven, and the five octet
/// answers travel as files because octets are what a file is for: a
/// length-delimited stream on standard output would be a protocol of our
/// own for the driver to read, and a document is the one answer that
/// cannot go to a file. A JVM launch costs far more than a C process
/// launch, which is why the count is the unit's and not the slot's; how
/// much more is not measured for this column.
///
/// ## The byte order, twice over
///
/// Neither the writer nor the reader has it baked in — `runtime/java`
/// takes it as a constructor argument on both — and the encapsulation
/// identifier does not: it is a pure function of the encoding and the
/// order ([`encapsulation_header`](zerodds_wireprobe_gen::encapsulation_header),
/// the same one every other column computes it with), so both are named
/// here and the program picks the one its argument asks for. The KeyHash
/// has no axis at all — XTypes 1.3 §7.6.8 makes the holder big-endian
/// whatever the sample is — so `zd_keyhash` takes no order.
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
    let sample_type = zerodds_lang_render::java::type_(
        &<Java as zerodds_lang_rules::emit::CodecTarget>::value_type(ast, root),
    );
    let encoder = codec_name::<Java>(ast, root, Purpose::Data);
    let decoder = decoder_name(ast, root);
    let max_align = args.encoding.max_alignment();
    let framing = framing_expression(key);
    let short_codes = key.length_codes.short_code_where_it_fits();

    let mut out = String::new();
    out.push_str(
        "// Generated by wireprobe-zerodds-java-gen. Do not edit.\n\
         //\n\
         // Every emit form and both byte orders, chosen at run time. The bytes\n\
         // come out of runtime/java when this runs; nothing here decides a wire\n\
         // question.\n\n\
         // Every runtime type the generated codec can name. Imported wholesale\n\
         // rather than per case, because which policy a configuration reaches is\n\
         // the layout key's answer and a probe that imported only what one key\n\
         // needed would fail to compile for another.\n\
         import org.zerodds.wire.Encapsulation;\n\
         import org.zerodds.wire.Encoding;\n\
         import org.zerodds.wire.Endianness;\n\
         import org.zerodds.wire.FixedDigitCount;\n\
         import org.zerodds.wire.Float128;\n\
         import org.zerodds.wire.ParameterRole;\n\
         import org.zerodds.wire.PidFlagPolicy;\n\
         import org.zerodds.wire.PlCdr1Framing;\n",
    );
    if with_read {
        out.push_str(
            "import org.zerodds.wire.Reader;\n\
             import org.zerodds.wire.TryConstruct;\n\
             import org.zerodds.wire.WireException;\n",
        );
    }
    out.push_str(
        "import org.zerodds.wire.WcharFraming;\n\
         import org.zerodds.wire.WcharWidth;\n\
         import org.zerodds.wire.Writer;\n\
         import org.zerodds.wire.WstringLengthPolicy;\n\
         import org.zerodds.wire.WstringUnitOrder;\n\n\
         public final class Probe {\n",
    );
    splice_codec(&mut out, codec);

    let _ = write!(
        out,
        "\n    static {sample_type} zd_sample;\n\n\
         \x20   static void zd_fill() {{\n{fill}    }}\n"
    );

    // -- the five octet answers ------------------------------------------
    out.push_str(
        "\n\
         \x20   /** One answer, or the refusal it is. */\n\
         \x20   private interface ZdAnswer {\n\
         \x20       byte[] run();\n\
         \x20   }\n\n\
         \x20   /** Every octet answer in one run: the payload in both byte\n\
         \x20    *  orders, the same behind its encapsulation header, and the\n\
         \x20    *  KeyHash, which has no byte order of its own. */\n\
         \x20   private static void zd_batch(java.nio.file.Path dir)\n\
         \x20           throws java.io.IOException {\n\
         \x20       java.nio.file.Files.createDirectories(dir);\n",
    );
    for (slot, constant, encapsulate) in [
        ("le", "Endianness.LITTLE", "false"),
        ("be", "Endianness.BIG", "false"),
        ("le+enc", "Endianness.LITTLE", "true"),
        ("be+enc", "Endianness.BIG", "true"),
    ] {
        let _ = write!(
            out,
            "        zd_answer(dir, \"{stem}\", new ZdAnswer() {{\n\
             \x20           public byte[] run() {{\n\
             \x20               return zd_payload({constant}, {encapsulate});\n\
             \x20           }}\n\
             \x20       }});\n",
            stem = answer_stem(slot)
        );
    }
    out.push_str(
        "        zd_answer(dir, \"keyhash\", new ZdAnswer() {\n\
         \x20           public byte[] run() {\n\
         \x20               return zd_keyhash();\n\
         \x20           }\n\
         \x20       });\n\
         \x20   }\n\n\
         \x20   /** One answer, as a file: the octets it produced, or the sentence\n\
         \x20    *  it refused with. One answer's failure must not take the other\n\
         \x20    *  four with it — a column that wrote a payload perfectly well and\n\
         \x20    *  only failed on a form it does not carry would understate itself\n\
         \x20    *  and hide the defect behind a bigger one. */\n\
         \x20   private static void zd_answer(\n\
         \x20           java.nio.file.Path dir, String stem, ZdAnswer answer)\n\
         \x20           throws java.io.IOException {\n\
         \x20       byte[] octets;\n\
         \x20       try {\n\
         \x20           octets = answer.run();\n\
         \x20       } catch (Throwable failure) {\n\
         \x20           // Every failure channel this runtime has arrives here:\n\
         \x20           // the WireException it raises for a stream it could not\n\
         \x20           // read, the IllegalStateException a keyless type's KeyHash\n\
         \x20           // raises, and anything a generated method throws on a\n\
         \x20           // value the type cannot hold. The message is the refusal,\n\
         \x20           // so a failure that carries none has to be named rather\n\
         \x20           // than printed as `null`.\n\
         \x20           java.nio.file.Files.write(\n\
         \x20                   dir.resolve(stem + \".err\"),\n\
         \x20                   zd_sentence(failure)\n\
         \x20                           .getBytes(java.nio.charset.StandardCharsets.UTF_8));\n\
         \x20           return;\n\
         \x20       }\n\
         \x20       java.nio.file.Files.write(dir.resolve(stem + \".bin\"), octets);\n\
         \x20   }\n\n\
         \x20   /** The refusal one answer failed with, as a sentence. */\n\
         \x20   private static String zd_sentence(Throwable failure) {\n\
         \x20       String message = failure.getMessage();\n\
         \x20       return message == null ? failure.toString() : message;\n\
         \x20   }\n\n",
    );

    // -- the write direction ---------------------------------------------
    let _ = write!(
        out,
        "    /** The serialised sample in `order`, and the same behind its\n\
         \x20   *  four-byte encapsulation header. XTypes 1.3 §7.4.1.1 pads the\n\
         \x20   *  serialized data to the next four-octet boundary with zeros and\n\
         \x20   *  §7.6.3.1.2 states the count of that padding in the options\n\
         \x20   *  field; `runtime/java` computes both, and the bare payload stays\n\
         \x20   *  unpadded because that is the serialized data itself. */\n\
         \x20   private static byte[] zd_payload(Endianness order, boolean encapsulate) {{\n\
         \x20       zd_fill();\n\
         \x20       Writer w = new Writer(order, {max_align}, {framing}, {short_codes});\n\
         \x20       {encoder}(w, zd_sample);\n\
         \x20       byte[] body = w.finish();\n\
         \x20       if (!encapsulate) {{\n\
         \x20           return body;\n\
         \x20       }}\n\
         \x20       int pad = Encapsulation.padding(body.length);\n\
         \x20       byte[] head = Encapsulation.header(Encoding.{encoding}, order, pad);\n\
         \x20       byte[] out = new byte[4 + body.length + pad];\n\
         \x20       System.arraycopy(head, 0, out, 0, 4);\n\
         \x20       System.arraycopy(body, 0, out, 4, body.length);\n\
         \x20       return out;\n\
         \x20   }}\n\n",
        encoding = match args.encoding {
            Encoding::PlainCdr1 => "PLAIN_CDR1",
            Encoding::PlCdr1 => "PL_CDR1",
            Encoding::PlainCdr2 => "PLAIN_CDR2",
            Encoding::DelimitedCdr => "DELIMITED_CDR",
            Encoding::PlCdr2 => "PL_CDR2",
        }
    );

    // -- the KeyHash ------------------------------------------------------
    match holder_entry {
        Some(entry) => {
            let form = match zerodds_ast_layout::keyhash::key_hash_form_for(
                &session.ast,
                root,
                session.holder_key,
            )
            .map_err(|error| gap!("key-holder maximum size: {error}"))?
            {
                zerodds_ast_layout::keyhash::KeyHashForm::ZeroPadded => "zeroPadded",
                zerodds_ast_layout::keyhash::KeyHashForm::Md5 => "md5",
            };
            let _ = write!(
                out,
                "    private static byte[] zd_keyhash() {{\n\
                 \x20       zd_fill();\n\
                 \x20       // The holder is PLAIN_CDR2 big-endian whatever the sample\n\
                 \x20       // is, so its alignment cap is PLAIN_CDR2's. XTypes 1.3\n\
                 \x20       // §7.6.8.\n\
                 \x20       Writer w = new Writer(Endianness.BIG, 4, null, false);\n\
                 \x20       {entry}(w, zd_sample);\n\
                 \x20       return w.{form}();\n\
                 \x20   }}\n\n"
            );
        }
        None => {
            // Not a compile-time hole: a type with no `@key` member has
            // one instance and no KeyHash, and every column answers that
            // the same way. The refusal is in this method and nowhere
            // else, so the other four answers are untouched.
            let _ = write!(
                out,
                "    private static byte[] zd_keyhash() {{\n\
                 \x20       throw new IllegalStateException(\n\
                 \x20               \"wireprobe-zerodds-java: {} declares no @key member\");\n\
                 \x20   }}\n\n",
                args.type_name.replace('\\', "\\\\").replace('"', "\\\"")
            );
        }
    }

    // -- the read direction -----------------------------------------------
    if !with_read {
        out.push_str(
            "    private static void zd_read(Endianness order) throws java.io.IOException {\n\
             \x20       System.err.println(\n\
             \x20           \"wireprobe-zerodds-java: this probe was built without the read direction\");\n\
             \x20       System.exit(1);\n\
             \x20   }\n\n",
        );
    } else {
        let dump = dump::dump(ast, root, key.union_label)?;
        if !dump.functions.is_empty() {
            out.push('\n');
            for line in dump.functions.lines() {
                let _ = writeln!(out, "    {line}");
            }
        }
        let _ = write!(
            out,
            "\n    /** Everything on standard input. A payload is one message and\n\
             \x20   *  the probe is handed the whole of it. */\n\
             \x20   private static byte[] zd_slurp() throws java.io.IOException {{\n\
             \x20       java.io.ByteArrayOutputStream slurp = new java.io.ByteArrayOutputStream();\n\
             \x20       byte[] chunk = new byte[8192];\n\
             \x20       int read;\n\
             \x20       while ((read = System.in.read(chunk)) > 0) {{\n\
             \x20           slurp.write(chunk, 0, read);\n\
             \x20       }}\n\
             \x20       return slurp.toByteArray();\n\
             \x20   }}\n\n\
             \x20   static void zd_dump() {{\n"
        );
        for line in dump.body.lines() {
            let _ = writeln!(out, "        {line}");
        }
        out.push_str("    }\n\n");
        let _ = write!(
            out,
            "    /** The bytes on standard input read back as the sample, which\n\
             \x20   *  is printed as the canonical JSON of\n\
             \x20   *  `crates/wire-probe/src/canonical.rs`. */\n\
             \x20   private static void zd_read(Endianness order) throws java.io.IOException {{\n\
             \x20       byte[] zd_input = zd_slurp();\n\
             \x20       Reader r = new Reader(zd_input, order, {max_align}, {framing});\n\
             \x20       try {{\n\
             \x20           // The top-level type is not a member of anything, so no\n\
             \x20           // `@try_construct` governs it and §7.2.2.7's default\n\
             \x20           // stands.\n\
             \x20           zd_sample = {decoder}(r, TryConstruct.DISCARD);\n\
             \x20       }} catch (WireException error) {{\n\
             \x20           System.err.println(\"wireprobe-zerodds-java: at \" + error.at() + \": \"\n\
             \x20                   + error.getMessage());\n\
             \x20           System.exit(1);\n\
             \x20           return;\n\
             \x20       }}\n\
             \x20       if (r.readerStatus() == Reader.Status.DISCARDED) {{\n\
             \x20           // Not a failed decode. DDS-XTypes 1.3 §7.2.2.7: the\n\
             \x20           // receiver drops the object and carries on, and a caller\n\
             \x20           // that cannot tell that from a broken peer cannot tell a\n\
             \x20           // narrowed type from a corrupt stream. Exit code 3, the\n\
             \x20           // Rust probe's.\n\
             \x20           System.err.println(\n\
             \x20                   \"wireprobe-zerodds-java: at \" + r.readerPosition() + \": DISCARDED\");\n\
             \x20           System.exit(3);\n\
             \x20           return;\n\
             \x20       }}\n\
             \x20       if (r.readerStatus() != Reader.Status.OK) {{\n\
             \x20           System.err.println(\n\
             \x20                   \"wireprobe-zerodds-java: at \" + r.readerPosition() + \": \"\n\
             \x20                           + r.readerStatus());\n\
             \x20           System.exit(1);\n\
             \x20           return;\n\
             \x20       }}\n\
             \x20       // Nothing may be left over that is not padding. A decoder\n\
             \x20       // that stopped short and said nothing is the silent-drop\n\
             \x20       // shape: the sample comes back looking plausible with its\n\
             \x20       // last members missing. Padding is the one thing a writer\n\
             \x20       // legitimately leaves behind, and it is what alignment asks\n\
             \x20       // for from this offset and never more.\n\
             \x20       int zd_left = r.readerRemaining();\n\
             \x20       int zd_pad = ({max_align} - (r.readerPosition() % {max_align})) % {max_align};\n\
             \x20       if (zd_left > zd_pad) {{\n\
             \x20           System.err.println(\"wireprobe-zerodds-java: \" + zd_left\n\
             \x20                   + \" byte(s) left at offset \" + r.readerPosition()\n\
             \x20                   + \", of which at most \" + zd_pad + \" could be padding\");\n\
             \x20           System.exit(1);\n\
             \x20           return;\n\
             \x20       }}\n\
             \x20       zd_dump();\n\
             \x20       System.out.flush();\n\
             \x20   }}\n\n"
        );
    }

    // -- the dispatch -------------------------------------------------------
    out.push_str(
        "    public static void main(String[] args) throws java.io.IOException {\n\
         \x20       String form = args.length > 0 ? args[0] : \"\";\n\
         \x20       if (form.equals(\"batch\")) {\n\
         \x20           zd_batch(java.nio.file.Paths.get(args.length > 1 ? args[1] : \".\"));\n\
         \x20           return;\n\
         \x20       }\n\
         \x20       if (form.equals(\"read\")) {\n\
         \x20           String name = args.length > 1 ? args[1] : \"\";\n\
         \x20           Endianness order;\n\
         \x20           if (name.equals(\"le\") || name.equals(\"little\")) {\n\
         \x20               order = Endianness.LITTLE;\n\
         \x20           } else if (name.equals(\"be\") || name.equals(\"big\")) {\n\
         \x20               order = Endianness.BIG;\n\
         \x20           } else {\n\
         \x20               // Refused rather than defaulted: a caller who mistyped\n\
         \x20               // the order has to be told, not handed the other one's\n\
         \x20               // octets.\n\
         \x20               System.err.println(\"wireprobe-zerodds-java: `\" + name\n\
         \x20                       + \"` is not a byte order\");\n\
         \x20               System.exit(2);\n\
         \x20               return;\n\
         \x20           }\n\
         \x20           zd_read(order);\n\
         \x20           return;\n\
         \x20       }\n\
         \x20       System.err.println(\"wireprobe-zerodds-java: `\" + form\n\
         \x20               + \"` is not a form this probe answers\");\n\
         \x20       System.exit(2);\n\
         \x20   }\n\
         }\n",
    );
    Ok(out)
}

/// The PL_CDR1 framing one layout key adds up to, as the Java
/// expression that builds it.
///
/// One place, so the two directions cannot part over a flag: a reader
/// configured with a framing the writer did not use finds the next
/// parameter at an offset no writer produced.
fn framing_expression(key: LayoutKey) -> String {
    let headers = key.pl_cdr1_headers;
    format!(
        "new PlCdr1Framing({}, {}, {}, {}, {}, {})",
        match key.pid_flags {
            PidFlagPolicy::SentinelMustUnderstand => "PidFlagPolicy.SENTINEL_MUST_UNDERSTAND",
            PidFlagPolicy::Bare => "PidFlagPolicy.BARE",
            PidFlagPolicy::ImplExtensionOnMembers => "PidFlagPolicy.IMPL_EXTENSION_ON_MEMBERS",
        },
        key.short_pid_range.highest_short_id(),
        headers.pads_declared_length(),
        headers.pads_declared_optional_length(),
        key.member_pid_carries_must_understand(),
        headers.pads_after_value()
    )
}

/// The key holder and the codec it calls.
///
/// The C column's KeyHash run, in Java: the codec planned under the
/// holder's own layout key, and under the dialect that writes a keyless
/// nested aggregate whole a second set planned for data. Java needs no
/// pruning — `javac` has no warning for a static method nothing calls.
/// A type with no `@key` member has one instance and no KeyHash, and
/// every column answers that the same way.
fn key_hash_codec(session: &Session<'_>) -> Result<Vec<zerodds_lang_cst::java::Member>, Refusal> {
    let ast = &session.ast;
    let root = session.root;
    let holder_key = session.holder_key;
    let mut members = codec_from::<Java>(ast, root, holder_key, session.codec_options)
        .map_err(|error| from_codec(&error))?;
    if holder_key.key_holder_nesting == KeyHolderNesting::KeylessNestedWhole {
        let as_data = LayoutKey {
            purpose: Purpose::Data,
            ..holder_key
        };
        members.extend(
            codec_from::<Java>(ast, root, as_data, session.codec_options)
                .map_err(|error| from_codec(&error))?,
        );
    }
    let Some(holder) =
        key_holder::<Java>(ast, root, holder_key).map_err(|error| from_codec(&error))?
    else {
        return Err(gap!("{} declares no @key member", session.args.type_name));
    };
    members.push(holder);
    Ok(members)
}

/// `Probe.java` for a KeyHash run: the sample, its key holder written
/// big-endian under PLAIN_CDR2's alignment cap (XTypes 1.3 §7.6.8), and
/// the sixteen octets DDSI-RTPS 2.5 §9.6.3.8 makes of it.
fn key_hash_harness(session: &Session<'_>, codec: &str, fill: &str) -> Result<String, Refusal> {
    let ast: &Ast = &session.ast;
    let root: Id<TypeNode> = session.root;
    let sample_type = zerodds_lang_render::java::type_(
        &<Java as zerodds_lang_rules::emit::CodecTarget>::value_type(ast, root),
    );
    let holder = format!(
        "zd_keyholder_{}",
        <Java as zerodds_lang_rules::emit::CodecTarget>::type_name(ast, root)
    );
    let form = match zerodds_ast_layout::keyhash::key_hash_form_for(ast, root, session.holder_key)
        .map_err(|error| gap!("key-holder maximum size: {error}"))?
    {
        zerodds_ast_layout::keyhash::KeyHashForm::ZeroPadded => "zeroPadded",
        zerodds_ast_layout::keyhash::KeyHashForm::Md5 => "md5",
    };
    let mut out = String::new();
    out.push_str(
        "// Generated by wireprobe-zerodds-java-gen. Do not edit.\n\
         //\n\
         // The KeyHash comes out of runtime/java when this runs. Nothing here\n\
         // decides a wire question.\n\n\
         import org.zerodds.wire.Encapsulation;\n\
         import org.zerodds.wire.Encoding;\n\
         import org.zerodds.wire.Endianness;\n\
         import org.zerodds.wire.FixedDigitCount;\n\
         import org.zerodds.wire.Float128;\n\
         import org.zerodds.wire.ParameterRole;\n\
         import org.zerodds.wire.PidFlagPolicy;\n\
         import org.zerodds.wire.PlCdr1Framing;\n\
         import org.zerodds.wire.WcharFraming;\n\
         import org.zerodds.wire.WcharWidth;\n\
         import org.zerodds.wire.Writer;\n\
         import org.zerodds.wire.WstringLengthPolicy;\n\
         import org.zerodds.wire.WstringUnitOrder;\n\n\
         public final class Probe {\n",
    );
    splice_codec(&mut out, codec);
    let _ = write!(
        out,
        "\n    static {sample_type} zd_sample;\n\n\
         \x20   static void zd_fill() {{\n{fill}    }}\n\n\
         \x20   public static void main(String[] args) throws java.io.IOException {{\n\
         \x20       zd_fill();\n\
         \x20       // The holder is PLAIN_CDR2 big-endian whatever the sample is,\n\
         \x20       // so its alignment cap is PLAIN_CDR2's. XTypes 1.3 7.6.8.\n\
         \x20       Writer w = new Writer(Endianness.BIG, 4, null, false);\n\
         \x20       {holder}(w, zd_sample);\n\
         \x20       byte[] zdHash = w.{form}();\n\
         \x20       System.out.write(zdHash, 0, zdHash.length);\n\
         \x20       System.out.flush();\n\
         \x20   }}\n\
         }}\n"
    );
    Ok(out)
}

/// `Probe.java` for the read direction: bytes on standard input,
/// canonical JSON on standard output.
///
/// Standard input and not a path baked into the generated Java: the path
/// would be part of the program, so a probe compiled for one payload
/// could not be run against another, and the shell wrapper would have to
/// regenerate for every byte string it wanted read.
fn read_harness(session: &Session<'_>, codec: &str) -> Result<String, Refusal> {
    let args = session.args;
    let ast: &Ast = &session.ast;
    let root: Id<TypeNode> = session.root;
    let key: LayoutKey = session.key;
    let sample_type = zerodds_lang_render::java::type_(
        &<Java as zerodds_lang_rules::emit::CodecTarget>::value_type(ast, root),
    );
    let decoder = decoder_name(ast, root);
    let order = match args.endianness {
        Endianness::Little => "Endianness.LITTLE",
        Endianness::Big => "Endianness.BIG",
    };
    let framing = framing_expression(key);
    let max_align = args.encoding.max_alignment();
    let strip = usize::from(args.encapsulation()) * 4;

    let mut out = String::new();
    out.push_str(
        "// Generated by wireprobe-zerodds-java-gen. Do not edit.\n\
         //\n\
         // The values come out of runtime/java when this runs. Nothing here\n\
         // decides a wire question.\n\n\
         import org.zerodds.wire.Encapsulation;\n\
         import org.zerodds.wire.Encoding;\n\
         import org.zerodds.wire.Endianness;\n\
         import org.zerodds.wire.FixedDigitCount;\n\
         import org.zerodds.wire.Float128;\n\
         import org.zerodds.wire.ParameterRole;\n\
         import org.zerodds.wire.PidFlagPolicy;\n\
         import org.zerodds.wire.PlCdr1Framing;\n\
         import org.zerodds.wire.Reader;\n\
         import org.zerodds.wire.TryConstruct;\n\
         import org.zerodds.wire.WcharFraming;\n\
         import org.zerodds.wire.WcharWidth;\n\
         import org.zerodds.wire.WireException;\n\
         import org.zerodds.wire.Writer;\n\
         import org.zerodds.wire.WstringLengthPolicy;\n\
         import org.zerodds.wire.WstringUnitOrder;\n\n\
         public final class Probe {\n",
    );
    splice_codec(&mut out, codec);

    let dump = dump::dump(ast, root, key.union_label)?;
    if !dump.functions.is_empty() {
        out.push('\n');
        for line in dump.functions.lines() {
            let _ = writeln!(out, "    {line}");
        }
    }
    let _ = write!(
        out,
        "\n    static {sample_type} zd_sample;\n\n\
         \x20   /** Everything on standard input. A payload is one message and the\n\
         \x20    *  probe is handed the whole of it. */\n\
         \x20   static byte[] zd_slurp() throws java.io.IOException {{\n\
         \x20       java.io.ByteArrayOutputStream out = new java.io.ByteArrayOutputStream();\n\
         \x20       byte[] chunk = new byte[8192];\n\
         \x20       int read;\n\
         \x20       while ((read = System.in.read(chunk)) > 0) {{\n\
         \x20           out.write(chunk, 0, read);\n\
         \x20       }}\n\
         \x20       return out.toByteArray();\n\
         \x20   }}\n\n\
         \x20   static void zd_dump() {{\n"
    );
    for line in dump.body.lines() {
        let _ = writeln!(out, "        {line}");
    }
    out.push_str("    }\n\n");

    let _ = write!(
        out,
        "    public static void main(String[] args) throws java.io.IOException {{\n\
         \x20       byte[] zd_input = zd_slurp();\n\
         \x20       int zd_at = 0;\n\
         \x20       int zd_len = zd_input.length;\n"
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
            "        if (zd_len < {strip}) {{\n\
             \x20           System.err.println(\"wireprobe-zerodds-java: fewer than the four-byte encapsulation header\");\n\
             \x20           System.exit(1);\n\
             \x20       }}\n\
             \x20       int zd_declared = zd_input[3] & 3;\n\
             \x20       zd_at += {strip};\n\
             \x20       zd_len -= {strip};\n\
             \x20       if (zd_len < zd_declared) {{\n\
             \x20           System.err.println(\"wireprobe-zerodds-java: the options field declares \"\n\
             \x20                   + zd_declared + \" padding byte(s) and only \" + zd_len + \" follow the header\");\n\
             \x20           System.exit(1);\n\
             \x20       }}\n\
             \x20       zd_len -= zd_declared;\n"
        );
    }
    let _ = write!(
        out,
        "        byte[] zd_body = java.util.Arrays.copyOfRange(zd_input, zd_at, zd_at + zd_len);\n\
         \x20       Reader r = new Reader(zd_body, {order}, {max_align}, {framing});\n\
         \x20       try {{\n\
         \x20           // The top-level type is not a member of anything, so no\n\
         \x20           // `@try_construct` governs it and 7.2.2.7's default stands.\n\
         \x20           zd_sample = {decoder}(r, TryConstruct.DISCARD);\n\
         \x20       }} catch (WireException error) {{\n\
         \x20           System.err.println(\"wireprobe-zerodds-java: at \" + error.at() + \": \" + error.getMessage());\n\
         \x20           System.exit(1);\n\
         \x20           return;\n\
         \x20       }}\n\
         \x20       if (r.readerStatus() == Reader.Status.DISCARDED) {{\n\
         \x20           // Not a failed decode. DDS-XTypes 1.3 7.2.2.7: the receiver\n\
         \x20           // drops the object and carries on, and a caller that cannot\n\
         \x20           // tell that from a broken peer cannot tell a narrowed type\n\
         \x20           // from a corrupt stream. Exit code 3, the Rust probe's.\n\
         \x20           System.err.println(\"wireprobe-zerodds-java: at \" + r.readerPosition() + \": DISCARDED\");\n\
         \x20           System.exit(3);\n\
         \x20           return;\n\
         \x20       }}\n\
         \x20       if (r.readerStatus() != Reader.Status.OK) {{\n\
         \x20           System.err.println(\"wireprobe-zerodds-java: at \" + r.readerPosition() + \": \" + r.readerStatus());\n\
         \x20           System.exit(1);\n\
         \x20           return;\n\
         \x20       }}\n\
         \x20       // Nothing may be left over that is not padding. A decoder that\n\
         \x20       // stopped short and said nothing is the silent-drop shape: the\n\
         \x20       // sample comes back looking plausible with its last members\n\
         \x20       // missing. Padding is the one thing a writer legitimately\n\
         \x20       // leaves behind, and it is what alignment asks for from this\n\
         \x20       // offset and never more.\n\
         \x20       int zd_left = r.readerRemaining();\n\
         \x20       int zd_pad = ({max_align} - (r.readerPosition() % {max_align})) % {max_align};\n\
         \x20       if (zd_left > zd_pad) {{\n\
         \x20           System.err.println(\"wireprobe-zerodds-java: \" + zd_left + \" byte(s) left at offset \"\n\
         \x20                   + r.readerPosition() + \", of which at most \" + zd_pad + \" could be padding\");\n\
         \x20           System.exit(1);\n\
         \x20           return;\n\
         \x20       }}\n\
         \x20       zd_dump();\n\
         \x20       System.out.flush();\n\
         \x20   }}\n\
         }}\n"
    );
    Ok(out)
}

/// `Probe.java`: the codec, the sample, the fill and `main`.
fn harness(session: &Session<'_>, codec: &str, fill: &str) -> Result<String, Refusal> {
    let args = session.args;
    let ast: &Ast = &session.ast;
    let root: Id<TypeNode> = session.root;
    let key: LayoutKey = session.key;
    let sample_type = zerodds_lang_render::java::type_(
        &<Java as zerodds_lang_rules::emit::CodecTarget>::value_type(ast, root),
    );
    let encoder = codec_name::<Java>(ast, root, Purpose::Data);
    let order = match args.endianness {
        Endianness::Little => "Endianness.LITTLE",
        Endianness::Big => "Endianness.BIG",
    };
    let framing = framing_expression(key);
    let short_codes = key.length_codes.short_code_where_it_fits();

    let mut out = String::new();
    out.push_str(
        "// Generated by wireprobe-zerodds-java-gen. Do not edit.\n\
         //\n\
         // The bytes come out of runtime/java when this runs. Nothing here\n\
         // decides a wire question.\n\n\
         // Every runtime type the generated codec can name. Imported
         // wholesale rather than per case, because which policy a
         // configuration reaches is the layout key's answer and a probe
         // that imported only what one key needed would fail to compile
         // for another.
         import org.zerodds.wire.Encapsulation;\n\
         import org.zerodds.wire.Encoding;\n\
         import org.zerodds.wire.Endianness;\n\
         import org.zerodds.wire.FixedDigitCount;\n\
         import org.zerodds.wire.Float128;\n\
         import org.zerodds.wire.ParameterRole;\n\
         import org.zerodds.wire.PidFlagPolicy;\n\
         import org.zerodds.wire.PlCdr1Framing;\n\
         import org.zerodds.wire.WcharFraming;\n\
         import org.zerodds.wire.WcharWidth;\n\
         import org.zerodds.wire.Writer;\n\
         import org.zerodds.wire.WstringLengthPolicy;\n\
         import org.zerodds.wire.WstringUnitOrder;\n\n\
         public final class Probe {\n",
    );

    // The codec, indented into the class.
    splice_codec(&mut out, codec);

    let _ = write!(
        out,
        "\n    static {sample_type} zd_sample;\n\n\
         \x20   static void zd_fill() {{\n{fill}    }}\n\n\
         \x20   public static void main(String[] args) throws java.io.IOException {{\n\
         \x20       zd_fill();\n\
         \x20       Writer w = new Writer({order}, {}, {framing}, {short_codes});\n",
        args.encoding.max_alignment()
    );
    if args.encapsulation() {
        // The header is built at run time and not baked in as four
        // constants, because one of them is not a constant: the options
        // field carries the payload's padding count and the payload's
        // length is not known until it is written.
        //
        // `runtime/java` has published `Encapsulation.header(encoding,
        // endianness, padding)` all along and this generator built the
        // four octets itself with a zero and a comment saying the probe
        // overwrote it. It did not. Measured on `primitive-octet` under
        // plain-cdr2: C and C# write `00070002` and two octets of
        // padding, Java wrote `00070000` and stopped at two.
        let _ = writeln!(
            out,
            "        Encoding zd_encoding = Encoding.{};\n\
             \x20       Endianness zd_order = {order};",
            match args.encoding {
                Encoding::PlainCdr1 => "PLAIN_CDR1",
                Encoding::PlCdr1 => "PL_CDR1",
                Encoding::PlainCdr2 => "PLAIN_CDR2",
                Encoding::DelimitedCdr => "DELIMITED_CDR",
                Encoding::PlCdr2 => "PL_CDR2",
            }
        );
    }
    let _ = write!(
        out,
        "        {encoder}(w, zd_sample);\n\
         \x20       byte[] zd_body = w.finish();\n"
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
        // `--emit payload` stays unpadded: that is the serialized data
        // itself, and it is what the ledger records.
        out.push_str(
            "        int zd_pad = Encapsulation.padding(zd_body.length);\n\
             \x20       System.out.write(\n\
             \x20               Encapsulation.header(zd_encoding, zd_order, zd_pad));\n\
             \x20       System.out.write(zd_body);\n\
             \x20       System.out.write(new byte[zd_pad]);\n",
        );
    } else {
        out.push_str("        System.out.write(zd_body);\n");
    }
    out.push_str(
        "        System.out.flush();\n\
         \x20   }\n\
         }\n",
    );
    Ok(out)
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
