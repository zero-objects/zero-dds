// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! One IDL model into one language's source, through the contract the
//! rest of this crate already keeps.
//!
//! The path is the seesaw one and nothing new:
//!
//! ```text
//! Ast → ModelGraph → apply(model, rule::catalogue(language)) → Applied
//!     → the language's materialiser → the language's renderer → text
//! ```
//!
//! `rule::catalogue(Language)` is already the one door into the first
//! half. This is the second half, written once instead of once per
//! caller: every `tools/wireprobe/wireprobe-zerodds-<lang>-gen` spells
//! the same four steps out again, which is why twenty-one of them
//! disagree about what to call them — `lua_items` beside `nim_sections`
//! beside `python_module`. The steps were never different; only the
//! names were.
//!
//! What this does *not* decide is the framing. The caller passes a
//! [`LayoutKey`], and a generated application passes
//! `Dialect::Specification` — a dialect is what a measurement
//! reproduces when it stands in for another vendor, and that belongs to
//! the probes, not to a program someone ships.

use zerodds_ast::Ast;
use zerodds_ast_core::Id;
use zerodds_ast_layout::Endianness;
use zerodds_ast_layout::key::LayoutKey;
use zerodds_lang_cst::Language;

pub use crate::adapter::interface::ServiceMapping;
use crate::emit::CodecTarget;
use crate::emit::encode::CodecOptions;
use crate::encode::EncodeError;
use crate::{ModelGraph, apply, rule};

/// Why a model did not become source.
#[derive(Debug)]
pub enum GenerateError {
    /// The rules did not apply to the model.
    Rules(String),
    /// The language has no form for something in the model.
    NoForm(String),
    /// The language is materialised but not yet routed through here.
    NotWired(Language),
    /// The codec could not be built: the layout could not be planned, or
    /// a construct has no writer or reader yet. [`source`]'s own error,
    /// distinct from [`Self::NoForm`] because a materialiser's refusal
    /// and a codec's are two different walks answering two different
    /// questions about the same type.
    Codec(String),
}

impl core::fmt::Display for GenerateError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Rules(detail) => write!(f, "the rules do not apply: {detail}"),
            Self::NoForm(detail) | Self::Codec(detail) => write!(f, "{detail}"),
            Self::NotWired(language) => write!(
                f,
                "{language:?} has a materialiser but is not routed through `generate` yet"
            ),
        }
    }
}

impl core::error::Error for GenerateError {}

/// The languages whose `interface` binding is complete: every
/// operation, every parameter direction, every raised exception and
/// every attribute, with the request and reply topic types DDS-RPC 1.0
/// §7.5.1.1 gives them where the binding has a codec at all. Every other
/// language `refuse_interface`s an `interface` by name rather than let
/// one reach a partial answer.
///
/// TypeScript, C++ and Java are `emit::interface_{typescript,cpp,
/// java}`; C#, Go and Swift are `adapter::{csharp,go,swift}::
/// interfaces` (all six flattened: OMG IDL 4.2 §7.4.3's inherited
/// operations and attributes written directly into the one declaration,
/// since none of them has a language mechanism this crate composes
/// them through instead); Rust is `emit::interface_rust`, folded into
/// [`source`] through `rust_source`, with a native supertrait. No
/// language belongs here on the strength of an `adapter::<lang>` that
/// merely does not refuse an interface node — `crates/lang-rules/tests/
/// interface_bindings.rs`/`inherited_operations.rs` is where a language
/// earns the list by measuring `raises` and an attribute, not only an
/// operation's name.
pub const COMPLETE_INTERFACE_LANGUAGES: [Language; 15] = [
    Language::Ada,
    Language::Cpp,
    Language::CSharp,
    Language::D,
    Language::Elixir,
    Language::Go,
    Language::Java,
    Language::Julia,
    Language::Lua,
    Language::Nim,
    Language::OCaml,
    Language::Rust,
    Language::Swift,
    Language::TypeScript,
    Language::Zig,
];

/// [`COMPLETE_INTERFACE_LANGUAGES`], less Rust and Ada — what
/// [`declarations`] itself answers `Ok` for.
///
/// `declarations` is the codec-less half of the pair: C++, Java, C#,
/// Go, Nim, Swift, Zig and TypeScript were always declarations-only
/// bindings — DDS-Cxx-PSM, the JVM/JS/.NET/Go/Nim/Swift/Zig runtimes
/// give the octets a body outside this crate, which is `emit::
/// interface_cpp`'s own module doc — so `declarations` was already
/// their whole answer. Rust's and Ada's bindings are not: a `Requester`
/// that cannot encode a request is not a binding, and both crates'
/// codecs live in [`source`] (`rust_source`/`ada_source`) and nowhere
/// in `declarations` — RM 7.2(4) is Ada's own reason, spelled out on
/// [`source`]'s own two-Ada-columns paragraph. A model with an
/// `interface`, asked of `declarations(ast, Language::Rust)` or
/// `declarations(ast, Language::Ada)`, therefore refuses too — the
/// complete answer exists, just not through this function.
pub const COMPLETE_DECLARATIONS_LANGUAGES: [Language; 13] = [
    Language::Cpp,
    Language::CSharp,
    Language::D,
    Language::Elixir,
    Language::Go,
    Language::Java,
    Language::Julia,
    Language::Lua,
    Language::Nim,
    Language::OCaml,
    Language::Swift,
    Language::TypeScript,
    Language::Zig,
];

/// `Err` when `ast` declares an `interface` and `language` is not in
/// `complete` — DDS-TS-E002 and its own three failures aside, this is
/// the fourth way an interface does not reach a language: the language
/// itself (or, for [`declarations`], this particular entry point) has
/// no complete binding for it. Naming every interface the model has and
/// not only the first, so one refusal is not read as "the rest are
/// fine".
fn refuse_interface_unless(
    ast: &Ast,
    language: Language,
    complete: &[Language],
) -> Result<(), GenerateError> {
    if complete.contains(&language) {
        return Ok(());
    }
    let services = crate::adapter::interface::services(ast)
        .map_err(|error| GenerateError::NoForm(error.to_string()))?;
    if services.is_empty() {
        return Ok(());
    }
    let supported = complete
        .iter()
        .copied()
        .map(Language::as_str)
        .collect::<Vec<_>>()
        .join(", ");
    let mut named = Vec::with_capacity(services.len());
    for service in &services {
        let node = ast
            .interface(service.id)
            .map_err(|error| GenerateError::NoForm(format!("{error:?}")))?;
        named.push(format!(
            "`interface {}` at {}",
            service.name,
            interface_position(&node.facets.source)
        ));
    }
    Err(GenerateError::NoForm(format!(
        "{language:?} has no complete DDS-RPC 1.0 binding for an `interface`, and this model \
         declares {}: {}. Generate it with one of: {supported}.",
        named.len(),
        named.join("; "),
    )))
}

/// [`refuse_interface_unless`] against [`COMPLETE_INTERFACE_LANGUAGES`]
/// — what [`source`] and [`csharp_source_with_runtime`] both check.
fn refuse_interface(ast: &Ast, language: Language) -> Result<(), GenerateError> {
    refuse_interface_unless(ast, language, &COMPLETE_INTERFACE_LANGUAGES)
}

/// `path:start-end` for a node's source span, or its origin in words
/// where there is no span — a node the model API built rather than one
/// a file parsed.
fn interface_position(source: &zerodds_ast::facet::SourceFacet) -> String {
    let where_ = match &source.origin {
        zerodds_ast::facet::SourceOrigin::File { path } => path.clone(),
        zerodds_ast::facet::SourceOrigin::Synthetic { reason } => {
            format!("a synthetic node ({reason})")
        }
        zerodds_ast::facet::SourceOrigin::Builtin => "a built-in declaration".to_string(),
    };
    match source.span {
        Some(span) => format!("{where_}:{}-{}", span.start, span.end),
        None => where_,
    }
}

/// The declarations of `ast`, as source in `language`.
///
/// # Errors
/// [`GenerateError`], naming which of the three steps stopped. A model
/// with an `interface` and a `language` outside
/// [`COMPLETE_INTERFACE_LANGUAGES`] refuses before either of the other
/// two runs — see `refuse_interface` — rather than answer with the
/// interface silently missing from the output.
pub fn declarations(ast: &Ast, language: Language) -> Result<String, GenerateError> {
    declarations_with_mapping(ast, language, ServiceMapping::Basic)
}

/// [`declarations`] with the service mapping of DDS-RPC 1.0 §7.5.1 named.
///
/// [`ServiceMapping::Basic`] is the default of every other entry point;
/// [`ServiceMapping::Enhanced`] writes the Enhanced topic types of
/// §7.5.1.2 in the columns that have a binding for it (C++ and Java).
///
/// # Errors
/// As [`declarations`].
pub fn declarations_with_mapping(
    ast: &Ast,
    language: Language,
    mapping: ServiceMapping,
) -> Result<String, GenerateError> {
    refuse_interface_unless(ast, language, &COMPLETE_DECLARATIONS_LANGUAGES)?;
    let model = ModelGraph::of(ast);
    let applied = apply(&model, &rule::catalogue(language))
        .map_err(|error| GenerateError::Rules(format!("{error:?}")))?;

    // One arm per language, and each is the pair the materialiser and
    // the renderer were written as. The spellings differ because they
    // were written apart; the shape does not.
    //
    // Three languages that otherwise have both halves of the pair —
    // Ada83 (`adapter::ada83`/`render::ada83`), F# (`adapter::fsharp`/
    // `render::fsharp`) and Kotlin (`adapter::kotlin`/`render::kotlin`)
    // — have no arm here and cannot get one: `zerodds_lang_cst::Language`
    // itself has no `Ada83`, `FSharp` or `Kotlin` variant, so `language`
    // can never hold one of them and `rule::catalogue(language)` — the
    // step before any of this — has nothing to dispatch to either. Their
    // three `rule::<lang>::catalogue()` functions take no `Language` and
    // are called directly, only from
    // `crates/lang-rules/tests/{ada83,fsharp,kotlin}_end_to_end.rs`; that
    // is a second, narrower contract this function does not go through.
    let text = match language {
        Language::Lua => {
            let items = crate::adapter::lua::lua_items(ast, &applied)
                .map_err(|error| GenerateError::NoForm(format!("{error:?}")))?;
            items
                .iter()
                .map(zerodds_lang_render::lua::stat)
                .collect::<Vec<_>>()
                .join("\n")
        }
        Language::Ada => {
            let items = crate::verbatim::weave_file(
                ast,
                crate::verbatim::aliases::ADA,
                crate::adapter::ada::ada_items(ast, &applied)
                    .map_err(|error| GenerateError::NoForm(format!("{error:?}")))?,
            );
            items
                .iter()
                .map(zerodds_lang_render::ada::decl)
                .collect::<Vec<_>>()
                .join("\n")
        }
        Language::C => {
            let items = crate::adapter::c::c_items(ast, &applied)
                .map_err(|error| GenerateError::NoForm(format!("{error:?}")))?;
            items
                .iter()
                .map(zerodds_lang_render::c::decl)
                .collect::<Vec<_>>()
                .join("\n")
        }
        Language::Cpp => {
            let mut items = crate::adapter::cpp::cpp_items(ast, &applied)
                .map_err(|error| GenerateError::NoForm(format!("{error:?}")))?;
            // A service defined by a pair of types (DDS-RPC 1.0 §7.3.1.4):
            // the common types of §7.5.1.1.1 are the runtime's, which comes
            // first; the traits and the type support follow the structures.
            let pair_items = crate::emit::type_pair_cpp::cpp_type_pair_items(ast)
                .map_err(GenerateError::NoForm)?;
            let has_pair_items = !pair_items.is_empty();
            if has_pair_items {
                crate::emit::type_pair_cpp::drop_common_declarations(&mut items);
                items.insert(
                    0,
                    zerodds_lang_cst::cpp::Decl::Verbatim(
                        crate::emit::type_pair_cpp::RUNTIME_INCLUDE.to_string(),
                    ),
                );
                let aliases = crate::verbatim::aliases::CPP;
                before_end_file(&mut items, ast, aliases, pair_items);
            }
            // Under `CppMapping::Psm` the declarations above are the native
            // types, in their own namespace; the PSM classes follow and hand
            // themselves to the codec through `zerodds::psm::Conv`. The RPC
            // binding below spells the PSM's types where the application
            // sees them (DDS-RPC 1.0 §7.11.1.1.2, DDS-PSM-Cxx 1.0 Table 7.1),
            // so they have to be declared first.
            if crate::adapter::cpp::cpp_mapping() == crate::adapter::cpp::CppMapping::Psm {
                let layer = crate::adapter::cpp_psm::psm_layer(ast)
                    .map_err(|error| GenerateError::NoForm(format!("{error:?}")))?;
                if !layer.is_empty() {
                    before_end_file(
                        &mut items,
                        ast,
                        crate::verbatim::aliases::CPP,
                        vec![zerodds_lang_cst::cpp::Decl::Verbatim(layer)],
                    );
                }
            }
            // Under `CppMapping::Classic` the same, with the types of the classic
            // OMG IDL to C++ mapping. The RPC binding names the PSM's types, so a
            // model with an interface or a pair of types is refused.
            if crate::adapter::cpp::cpp_mapping() == crate::adapter::cpp::CppMapping::Classic {
                if has_pair_items {
                    return Err(GenerateError::NoForm(
                        "the classic C++ mapping has no form for the service types of DDS-RPC 1.0 §7.3.1.4"
                            .to_string(),
                    ));
                }
                let layer = crate::adapter::cpp_classic::classic_layer(ast)
                    .map_err(|error| GenerateError::NoForm(format!("{error:?}")))?;
                before_end_file(
                    &mut items,
                    ast,
                    crate::verbatim::aliases::CPP,
                    vec![zerodds_lang_cst::cpp::Decl::Verbatim(layer)],
                );
            }
            // The RPC binding of DDS-RPC 1.0 §7.11.1, after the data
            // types: its topic structs name them.
            before_end_file(
                &mut items,
                ast,
                crate::verbatim::aliases::CPP,
                crate::emit::interface_cpp::cpp_interface_items_with(ast, mapping)
                    .map_err(|error| GenerateError::NoForm(error.to_string()))?,
            );
            items
                .iter()
                .map(zerodds_lang_render::cpp::decl)
                .collect::<Vec<_>>()
                .join("\n")
        }
        Language::CSharp => {
            // C# has no `csharp_items`: only `csharp_file`, which is
            // already the whole file and not a list to loop over.
            let file = crate::adapter::csharp::csharp_file(ast, &applied)
                .map_err(|error| GenerateError::NoForm(format!("{error:?}")))?;
            zerodds_lang_render::csharp::file(&file)
        }
        Language::D => {
            let items = crate::adapter::d::d_items(ast, &applied)
                .map_err(|error| GenerateError::NoForm(format!("{error:?}")))?;
            items
                .iter()
                .map(zerodds_lang_render::d::decl)
                .collect::<Vec<_>>()
                .join("\n")
        }
        Language::Elixir => {
            // `elixir_modules`, not `elixir_file`, matching `lua_items`
            // over `lua_file`: the modules only, without the chunk a
            // generator writing straight to disk would wrap them in.
            let modules = crate::adapter::elixir::elixir_modules(ast, &applied)
                .map_err(|error| GenerateError::NoForm(format!("{error:?}")))?;
            modules
                .iter()
                .map(zerodds_lang_render::elixir::top_level)
                .collect::<Vec<_>>()
                .join("\n")
        }
        Language::Go => {
            let items = crate::adapter::go::go_items(ast, &applied)
                .map_err(|error| GenerateError::NoForm(format!("{error:?}")))?;
            items
                .iter()
                .map(zerodds_lang_render::go::decl)
                .collect::<Vec<_>>()
                .join("\n")
        }
        Language::Java => {
            let mut items = crate::adapter::java::java_items(ast, &applied)
                .map_err(|error| GenerateError::NoForm(format!("{error:?}")))?;
            // A service defined by a pair of types (DDS-RPC 1.0 §7.3.1.4):
            // the common types of §7.5.1.1.1 are the runtime's.
            if crate::emit::type_pair_java::has_pair_types(ast) {
                crate::emit::type_pair_java::adjust_declarations(&mut items);
            }
            // The RPC binding of DDS-RPC 1.0 §7.11.2, in its own outer
            // class: `ZdTypes` holds the data, `ZdServices` the calls.
            before_end_file(
                &mut items,
                ast,
                crate::verbatim::aliases::JAVA,
                crate::emit::interface_java::java_interface_items_with(ast, mapping)
                    .map_err(|error| GenerateError::NoForm(error.to_string()))?,
            );
            items
                .iter()
                .map(zerodds_lang_render::java::type_decl)
                .collect::<Vec<_>>()
                .join("\n")
        }
        Language::Julia => {
            let items = crate::adapter::julia::julia_items(ast, &applied)
                .map_err(|error| GenerateError::NoForm(format!("{error:?}")))?;
            items
                .iter()
                .map(zerodds_lang_render::julia::decl)
                .collect::<Vec<_>>()
                .join("\n")
        }
        Language::Nim => {
            let sections = crate::adapter::nim::nim_sections(ast, &applied)
                .map_err(|error| GenerateError::NoForm(format!("{error:?}")))?;
            sections
                .iter()
                .map(zerodds_lang_render::nim::section)
                .collect::<Vec<_>>()
                .join("\n")
        }
        Language::Node => {
            // `node_items` takes no `Applied`: `rule::catalogue` answers
            // `Language::Node` with zero rules (see the module doc on
            // `rule::catalogue`), because a struct, a union, a bitset, a
            // valuetype, an exception and an interface are all plain
            // object literals here and declare nothing a rule would
            // compare.
            let items = crate::adapter::node::node_items(ast)
                .map_err(|error| GenerateError::NoForm(format!("{error:?}")))?;
            items
                .iter()
                .map(zerodds_lang_render::node::stat)
                .collect::<Vec<_>>()
                .join("\n")
        }
        Language::OCaml => {
            let items = crate::adapter::ocaml::ocaml_items(ast, &applied)
                .map_err(|error| GenerateError::NoForm(format!("{error:?}")))?;
            items
                .iter()
                .map(zerodds_lang_render::ocaml::decl)
                .collect::<Vec<_>>()
                .join("\n")
        }
        Language::Python => {
            // Python has no `python_items`, only `python_module`.
            let module = crate::adapter::python::python_module(ast, &applied)
                .map_err(|error| GenerateError::NoForm(format!("{error:?}")))?;
            zerodds_lang_render::python::module(&module)
        }
        Language::Rust => {
            // Rust's materialiser lives in `adapter::cst`, not
            // `adapter::rust` — there is no such module — because
            // `adapter::cst` is the R-side of every rule, and Rust's own
            // items are what `rust_items` folds out of it directly.
            let items = crate::adapter::cst::rust_items(ast, &applied)
                .map_err(|error| GenerateError::NoForm(format!("{error:?}")))?;
            items
                .iter()
                .map(zerodds_lang_render::rust::item)
                .collect::<Vec<_>>()
                .join("\n")
        }
        Language::Swift => {
            let items = crate::adapter::swift::swift_items(ast, &applied)
                .map_err(|error| GenerateError::NoForm(format!("{error:?}")))?;
            items
                .iter()
                .map(zerodds_lang_render::swift::decl)
                .collect::<Vec<_>>()
                .join("\n")
        }
        Language::TypeScript => {
            // The two support types a generated file declares itself:
            // the sixteen octets of a `long double` and the box an IDL
            // `any` takes. Both are structural twins of the runtime's,
            // so a value passes between them.
            let mut items = crate::adapter::typescript::prelude();
            // The data types with their descriptor side-tables. §2.1
            // item 3 makes the side-tables part of the IDL-Codegen
            // Profile — "for every IDL type it emits, it additionally
            // emits a Type Descriptor and a corresponding type-guard
            // function" — so they are not optional here. What they cost
            // is the import below: a file carrying descriptors is no
            // longer self-contained, because `registerType` is a
            // function with process-wide state and no generated file can
            // declare it.
            items.extend(
                crate::adapter::typescript::ts_items_with_descriptors(ast, &applied)
                    .map_err(|error| GenerateError::NoForm(format!("{error:?}")))?,
            );
            // The Operations Profile of DDS-TS 1.0 Ch11, after the data
            // types: its descriptors name the exception descriptors the
            // struct walk declares.
            before_end_file(
                &mut items,
                ast,
                crate::verbatim::aliases::TYPESCRIPT,
                crate::emit::interface_typescript::ts_interface_items(ast, &applied)
                    .map_err(|error| GenerateError::NoForm(error.to_string()))?,
            );
            zerodds_lang_render::typescript::file(&zerodds_lang_cst::typescript::File {
                doc: zerodds_lang_cst::common::Doc::none(),
                imports: vec![crate::adapter::typescript::descriptor_import(
                    crate::adapter::typescript::DEFAULT_RUNTIME_PACKAGE,
                )],
                decls: items,
            })
        }
        Language::Zig => {
            let items = crate::adapter::zig::zig_items(ast, &applied)
                .map_err(|error| GenerateError::NoForm(format!("{error:?}")))?;
            items
                .iter()
                .map(zerodds_lang_render::zig::decl)
                .collect::<Vec<_>>()
                .join("\n")
        }
        Language::Ada83 => {
            let items = crate::verbatim::weave_file(
                ast,
                crate::verbatim::aliases::ADA83,
                crate::adapter::ada83::ada83_items(ast, &applied)
                    .map_err(|error| GenerateError::NoForm(format!("{error:?}")))?,
            );
            items
                .iter()
                .map(zerodds_lang_render::ada83::decl)
                .collect::<Vec<_>>()
                .join("\n")
        }
        Language::FSharp => {
            let items = crate::adapter::fsharp::fsharp_items(ast, &applied)
                .map_err(|error| GenerateError::NoForm(format!("{error:?}")))?;
            items
                .iter()
                .map(zerodds_lang_render::fsharp::decl)
                .collect::<Vec<_>>()
                .join("\n")
        }
        Language::Kotlin => {
            let items = crate::adapter::kotlin::kotlin_items(ast, &applied)
                .map_err(|error| GenerateError::NoForm(format!("{error:?}")))?;
            items
                .iter()
                .map(zerodds_lang_render::kotlin::decl)
                .collect::<Vec<_>>()
                .join("\n")
        }
    };
    Ok(text)
}

/// Add `more` to `items` ahead of the `@verbatim(placement = END_FILE)`
/// text the column put last: OMG IDL 4.2 §8.3.5.1 places that text at the
/// end of the file, and the RPC declarations are part of the file.
fn before_end_file<D>(items: &mut Vec<D>, ast: &Ast, aliases: &[&str], more: Vec<D>) {
    let (_, end_file) = crate::verbatim::file_slots(ast, aliases);
    let at = items.len().saturating_sub(end_file.len());
    let tail = items.split_off(at);
    items.extend(more);
    items.extend(tail);
}

/// One Java compilation unit of a generated Java text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavaUnit {
    /// The public top-level class the unit declares, which is also the
    /// stem of the file it has to be written to (`ZdTypes`,
    /// `ZdServices`, `ZdCodec`).
    pub class: &'static str,
    /// The unit's text, ending in a newline.
    pub text: String,
}

/// The generated Java text of [`source`] or [`declarations`] as the
/// compilation units Java takes.
///
/// The generator writes `ZdTypes` (the data), `ZdServices` (the RPC
/// binding of DDS-RPC 1.0 §7.11.2, when the model has an `interface`) and
/// `ZdCodec` (the codec, for [`source`]) as top-level classes of one text,
/// and JLS §7.6 allows one public top-level class per file, named as the
/// file. This cuts the text where each class begins — a Javadoc block
/// directly above a class goes with it — and returns one unit per class,
/// in the order they stand. Whatever precedes the first class (a header
/// comment, imports) stays with the first unit.
#[must_use]
pub fn java_units(text: &str) -> Vec<JavaUnit> {
    const CLASSES: [(&str, &[&str]); 3] = [
        ("ZdTypes", &["public final class ZdTypes {"]),
        ("ZdServices", &["public final class ZdServices {"]),
        ("ZdCodec", &["public class ZdCodec {"]),
    ];
    let mut starts: Vec<(usize, &'static str)> = Vec::new();
    for (class, openings) in CLASSES {
        let Some(at) = openings.iter().find_map(|opening| {
            text.lines()
                .scan(0usize, |offset, line| {
                    let here = *offset;
                    *offset += line.len() + 1;
                    Some((here, line))
                })
                .find(|(_, line)| line == opening)
                .map(|(here, _)| here)
        }) else {
            continue;
        };
        // A Javadoc block directly above belongs to the class.
        let mut begin = at;
        let mut lines_above: Vec<(usize, &str)> = text[..at]
            .lines()
            .scan(0usize, |offset, line| {
                let here = *offset;
                *offset += line.len() + 1;
                Some((here, line))
            })
            .collect();
        while let Some((here, line)) = lines_above.pop() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("/**") || trimmed.starts_with('*') || trimmed.starts_with("*/") {
                begin = here;
                if trimmed.starts_with("/**") {
                    break;
                }
            } else {
                break;
            }
        }
        starts.push((begin, class));
    }
    starts.sort_unstable();
    let mut units = Vec::new();
    for (index, (begin, class)) in starts.iter().enumerate() {
        let from = if index == 0 { 0 } else { *begin };
        let to = starts.get(index + 1).map_or(text.len(), |(next, _)| *next);
        units.push(JavaUnit {
            class,
            text: format!("{}\n", text[from..to].trim_end()),
        });
    }
    units
}

/// The declarations of `ast` and its codec, as source in `language`: the
/// encoder [`crate::emit::encode::codec`] builds, the Table 9 defaulters
/// its decoder calls, and the decoder [`crate::emit::decode::decoder`]
/// builds — every one of them, rendered through the same materialiser
/// and renderer [`declarations`] uses.
///
/// Every type the model declares, not one root. `codec` and `decoder`
/// already walk and dedupe by name over the whole model — the same two
/// functions `encode_c::c_codec` and `decode_c::c_decoder` bind for the
/// C column, which `crates/security-rtps` builds a full binding through.
/// A single `--type` root, the shape every `wireprobe-zerodds-<lang>-gen`
/// takes, is a probe's question: it exists so a byte comparison has one
/// sample to run, and a generated application has no such reason to stop
/// at the first struct, union or valuetype the model declares.
///
/// # Errors
/// [`GenerateError`]: [`GenerateError::Rules`] and
/// [`GenerateError::NoForm`] as [`declarations`]; [`GenerateError::Codec`]
/// when the layout cannot be planned or a construct has no writer or
/// reader.
pub fn source(ast: &Ast, language: Language, key: LayoutKey) -> Result<String, GenerateError> {
    source_with_options(ast, language, key, CodecOptions::default())
}

/// As [`source`], with the one framing question [`LayoutKey`] does not
/// answer named explicitly rather than defaulted.
///
/// A caller that reproduces a dialect — `tools/idlc generate --dialect`
/// and `zerodds-build`'s own `Config::dialect` — has an answer for
/// [`CodecOptions::map_length_with_delimiter`] the way it has one for
/// every other axis: the dialect's, before any `--map-length` flag
/// overrides it. [`source`] keeps [`CodecOptions::default`] for every
/// caller that has no dialect to reproduce, which the module doc's
/// "a generated application passes `Dialect::Specification`" is still
/// true of.
///
/// # Errors
/// As [`source`].
pub fn source_with_options(
    ast: &Ast,
    language: Language,
    key: LayoutKey,
    options: CodecOptions,
) -> Result<String, GenerateError> {
    source_with_mapping(ast, language, key, options, ServiceMapping::Basic)
}

/// As [`source_with_options`], with the service mapping of DDS-RPC 1.0
/// §7.5.1 named: the Basic mapping (the default of every other entry
/// point) or the Enhanced one, whose topic types are `@Choice @Autoid`
/// structures (§7.5.1.2).
///
/// # Errors
/// As [`source`].
pub fn source_with_mapping(
    ast: &Ast,
    language: Language,
    key: LayoutKey,
    options: CodecOptions,
    mapping: ServiceMapping,
) -> Result<String, GenerateError> {
    // Ahead of the Rust/Ada/Ada83 special cases below: all three bypass
    // `declarations`, which is where this refusal otherwise lives, and
    // an interface must refuse the same way whichever path a language
    // takes through this function.
    refuse_interface(ast, language)?;
    // Rust is the one language whose two halves cannot be two texts
    // joined. Twenty backends emit free functions, so their codec is a
    // block of declarations that can follow the types; Rust's is
    // `impl Point`, which has to stand in the module `Point` was
    // declared in. Both halves nest, and nesting them separately
    // declares every shared ancestor twice — `nest_by_module`'s own doc
    // is where that failure is written down: Rust allows a name once
    // per scope, so `mod A { mod B { struct Inner } }` beside
    // `mod A { mod B { impl Inner } }` is "the name `A` is defined
    // multiple times". One fold, which is what `zerodds-build`'s
    // `render_with_stem` does for the same reason.
    //
    // Rust's own codec (`crate::encode::rust_codec`) takes no
    // `CodecOptions` at all — it has never grown the map-length branch
    // the twenty `CodecTarget` backends share — so `options` has
    // nothing to reach here and is not threaded through.
    if language == Language::Rust {
        return rust_source(ast, key, mapping);
    }
    // The two Ada columns are the other pair whose halves cannot be two
    // texts joined, and for the opposite reason: Ada keeps a
    // subprogram's profile and its body in two different compilation
    // units. RM 7.1 admits only `basic_declarative_item`s in the
    // visible part of a package specification, and a `subprogram_body`
    // is not one — so a text that put the codec's bodies where its
    // declarations are is not Ada at all. `gnatmake` says
    // `compilation unit expected` at the first type declaration.
    if language == Language::Ada {
        return ada_source(ast, key, options);
    }
    if language == Language::Ada83 {
        return ada83_source(ast, key, options);
    }
    // A C++ model with an interface or a pair of types has codecs in its
    // declarations (the topic types of DDS-RPC 1.0 §7.5.1.1, the pair of
    // §7.3.1.4). The file's own codec at the same scope would define the
    // helpers they define, so it stands in a namespace of its own, as
    // `cpp_source_with_mapping` puts it for `tools/idlc`.
    if language == Language::Cpp
        && (ast.interfaces().next().is_some() || crate::emit::type_pair_cpp::has_pair_types(ast))
    {
        return cpp_source_with_mapping(ast, key, options, "zd_codec", mapping).map(|c| c.text);
    }
    let decls = declarations_with_mapping(ast, language, mapping)?;
    let codec = codec(ast, language, key, options)?;
    // `@verbatim(placement = END_FILE)` closed the declarations, and the
    // codec follows them in the same file; OMG IDL 4.2 §8.3.5.1 puts the
    // text at the end of the file, so it moves past the codec.
    let (decls, end_file) = split_end_file(ast, language, &decls)?;
    Ok(format!("{decls}\n\n{codec}{end_file}"))
}

/// `decls` without the `END_FILE` text its column wrote last, and that
/// text, each part as it will be written.
///
/// # Errors
/// [`GenerateError::NoForm`] when the declarations do not end in the
/// text: a column that wrote it elsewhere would have it moved by nothing,
/// and a file whose `END_FILE` text stands in its middle is the
/// misplacement this exists to prevent.
fn split_end_file(
    ast: &Ast,
    language: Language,
    decls: &str,
) -> Result<(String, String), GenerateError> {
    let (_, end_file) = crate::verbatim::file_slots(ast, crate::verbatim::aliases_of(language));
    if end_file.is_empty() {
        return Ok((decls.to_string(), String::new()));
    }
    let tail: Vec<&str> = end_file.iter().flat_map(|text| text.lines()).collect();
    let tail = tail.join("\n");
    let trimmed = decls.trim_end();
    let Some(head) = trimmed.strip_suffix(tail.as_str()) else {
        return Err(GenerateError::NoForm(format!(
            "the {language:?} declarations do not end in their @verbatim END_FILE text, \
             so it cannot be moved past the codec"
        )));
    };
    Ok((head.trim_end().to_string(), format!("\n\n{tail}\n")))
}

/// The C++ source of `ast`, its codec inside `namespace codec_namespace`.
///
/// The codec is free functions at the scope it is written in, named
/// after what they encode — `zd_enc_p_unsigned_long`, `zd_enc_string`,
/// `zd_def_string` — so every file whose types use an `unsigned long`
/// or a `string` writes the same names. At file scope, two generated
/// headers in one translation unit define each of them twice, and the
/// compiler stops at the second. In a namespace of the file's own, each
/// header keeps its copy. The declarations stay where [`source`] puts
/// them: their namespaces are the IDL modules'.
///
/// # Errors
/// As [`source_with_options`].
pub fn cpp_source_with_codec_namespace(
    ast: &Ast,
    key: LayoutKey,
    options: CodecOptions,
    codec_namespace: &str,
) -> Result<String, GenerateError> {
    cpp_source_with_both_versions(ast, key, options, codec_namespace).map(|both| both.text)
}

/// A C++ header that reads both XCDR versions.
#[derive(Debug, Clone)]
pub struct CppSource {
    /// The header.
    pub text: String,
    /// The nested namespace the other version's codec stands in, and that
    /// version — `None` when the model has no codec under it, in which
    /// case the header reads the one version `key` names.
    pub other: Option<(&'static str, zerodds_ast_layout::Encoding)>,
}

/// The C++ header with the codec `key` plans in `codec_namespace` and the
/// other XCDR version's decoders beside it, in a nested namespace named for
/// that version (`xcdr1` or `xcdr2`).
///
/// A peer chooses its representation (DDS-XTypes 1.3 §7.6.3.1.2) and a
/// reader takes what arrives; one codec per file left a C++ reader
/// refusing every sample a peer wrote in the other version. C++ keeps the
/// two sets apart by namespace, so neither set is renamed: the routines of
/// each call their own by unqualified name, and the
/// `topic_type_support<T>::decode` the front end emits chooses by the
/// version the encapsulation announces. The other version is planned with
/// `key`'s every other axis — the dialect is one choice for the file.
///
/// # Errors
/// As [`source`], for the version `key` names. A model the other version
/// cannot carry is not an error: the header then holds one codec, and
/// [`CppSource::other`] says so.
pub fn cpp_source_with_both_versions(
    ast: &Ast,
    key: LayoutKey,
    options: CodecOptions,
    codec_namespace: &str,
) -> Result<CppSource, GenerateError> {
    cpp_source_with_mapping(ast, key, options, codec_namespace, ServiceMapping::Basic)
}

/// As [`cpp_source_with_both_versions`], with the service mapping of
/// DDS-RPC 1.0 §7.5.1 named.
///
/// # Errors
/// As [`cpp_source_with_both_versions`].
pub fn cpp_source_with_mapping(
    ast: &Ast,
    key: LayoutKey,
    options: CodecOptions,
    codec_namespace: &str,
    mapping: ServiceMapping,
) -> Result<CppSource, GenerateError> {
    use zerodds_ast_layout::Encoding;
    refuse_interface(ast, Language::Cpp)?;
    let decls = declarations_with_mapping(ast, Language::Cpp, mapping)?;
    let primary = codec(ast, Language::Cpp, key, options)?;
    let (decls, end_file) = split_end_file(ast, Language::Cpp, &decls)?;
    let (nested, other_encoding) = if key.encoding.is_v2() {
        ("xcdr1", Encoding::PlainCdr1)
    } else {
        ("xcdr2", Encoding::PlainCdr2)
    };
    let other_key = LayoutKey {
        encoding: other_encoding,
        ..key
    };
    // The decoders and the Table 9 defaulters they call, and no encoder:
    // the trait writes in the version `key` names, so an encoder here
    // would be a `static` routine nothing calls.
    let other_decls = crate::emit::decode::decoder_excluding::<crate::emit::cpp::Cpp>(
        ast,
        other_key,
        options,
        &crate::emit::type_pair_cpp::excluded_types(ast),
        &[],
    )
    .and_then(|decoders| {
        let mut decls = crate::default_cpp::cpp_defaulters_called_by_excluding(
            ast,
            &decoders,
            &crate::emit::type_pair_cpp::excluded_types(ast),
        )?;
        decls.extend(decoders);
        Ok(decls)
    })
    .map(|decls| {
        decls
            .iter()
            .map(zerodds_lang_render::cpp::decl)
            .collect::<Vec<_>>()
            .join("\n")
    });
    let (other_text, other) = match other_decls {
        Ok(text) => (
            format!(
                "\n\n// The same types under the other XCDR version, which a peer may\n\
                 // choose to write in. DDS-XTypes 1.3 \u{a7}7.6.3.1.2.\n\
                 namespace {nested} {{\n\n{text}\n\n}} // namespace {nested}"
            ),
            Some((nested, other_encoding)),
        ),
        Err(_) => (String::new(), None),
    };
    // The PSM classes of `CppMapping::Psm` are among `decls`: the RPC binding
    // that follows them names them.
    Ok(CppSource {
        text: format!(
            "{decls}\n\nnamespace {codec_namespace} {{\n\n{primary}{other_text}\n\n}} // namespace {codec_namespace}{end_file}"
        ),
        other,
    })
}

/// C# declarations and codecs linked against the shared wire runtime.
///
/// Unlike standalone declarations, these must use the runtime's IdlName and
/// Float128 types, not redeclare same-named but assembly-incompatible copies.
///
/// # Errors
/// The same rule, materialization and codec errors as [`source`].
pub fn csharp_source_with_runtime(ast: &Ast, key: LayoutKey) -> Result<String, GenerateError> {
    csharp_source_with_runtime_and_options(ast, key, CodecOptions::default())
}

/// As [`csharp_source_with_runtime`], with [`CodecOptions`] named
/// explicitly. See [`source_with_options`].
///
/// # Errors
/// As [`csharp_source_with_runtime`].
pub fn csharp_source_with_runtime_and_options(
    ast: &Ast,
    key: LayoutKey,
    options: CodecOptions,
) -> Result<String, GenerateError> {
    refuse_interface(ast, Language::CSharp)?;
    let model = ModelGraph::of(ast);
    let applied = apply(&model, &rule::catalogue(Language::CSharp))
        .map_err(|error| GenerateError::Rules(format!("{error:?}")))?;
    let mut file = crate::adapter::csharp::csharp_file(ast, &applied)
        .map_err(|error| GenerateError::NoForm(format!("{error:?}")))?;
    crate::adapter::csharp::drop_support_namespace(&mut file);
    let decls = zerodds_lang_render::csharp::file(&file);
    let codec = codec(ast, Language::CSharp, key, options)?;
    let (decls, end_file) = split_end_file(ast, Language::CSharp, &decls)?;
    Ok(format!("{decls}\n\n{codec}{end_file}"))
}

/// The Ada spelling of the byte order `key` was planned under, as the
/// constant a caller hands `Zd_Wire.New_Writer`.
///
/// Every other column takes this constant from `tools/idlc`'s preamble,
/// which writes it above the declarations. Ada has no file scope to
/// write it at: RM 3.1 makes an object declaration a declarative item,
/// and a declarative item lives inside a compilation unit. So the
/// constant is a declaration of the package like any other, and it is
/// emitted here — where the package is — rather than spliced in from
/// outside it.
fn ada_endianness(endianness: Endianness) -> &'static str {
    match endianness {
        Endianness::Little => "Little",
        Endianness::Big => "Big",
    }
}

/// Ada's declarations and codec, as the two compilation units the
/// language asks for: `package Zd_Types is …` and
/// `package body Zd_Types is …`, in one text, in that order.
///
/// The split is by declaration kind and not by position: every
/// `ada::Decl::SubprogramBody` the codec emits goes into the body, and
/// everything else — the types, and the profile the emitter already
/// writes for every one of those bodies — into the spec. That is what
/// makes the codec reachable from outside: a subprogram whose body
/// stands in a package body and whose profile stands nowhere is not
/// declared by the package at all, which is the Ada form of the
/// `private static` the Java column was written as.
///
/// `tools/idlc` is what cuts the text into `zd_types.ads` and
/// `zd_types.adb` — the two file names GNAT derives the unit name from.
///
/// # Errors
/// As [`source`].
fn ada_source(ast: &Ast, key: LayoutKey, options: CodecOptions) -> Result<String, GenerateError> {
    use zerodds_lang_cst::ada;
    use zerodds_lang_cst::common::{Doc, Ident, Path};

    let model = ModelGraph::of(ast);
    let applied = apply(&model, &rule::catalogue(Language::Ada))
        .map_err(|error| GenerateError::Rules(format!("{error:?}")))?;
    let mut spec = crate::adapter::ada::ada_items(ast, &applied)
        .map_err(|error| GenerateError::NoForm(format!("{error:?}")))?;
    let codec = codec_of::<crate::emit::ada::Ada>(
        ast,
        key,
        options,
        crate::default_ada::ada_defaulters_called_by,
    )
    .map_err(codec_error)?;
    let mut bodies = Vec::new();
    for decl in codec {
        if matches!(decl, ada::Decl::SubprogramBody(_)) {
            bodies.push(decl);
        } else {
            spec.push(decl);
        }
    }
    // A model that emits no codec — an empty module, a file of nothing
    // but constants — gets neither the constant nor the body. RM 7.2(4)
    // allows a `package_body` only where the specification requires a
    // completion, and GNAT says `spec of this package does not allow a
    // body`; and the constant is what would name `Zd_Wire` in a unit
    // that otherwise never mentions the runtime, which is the same rule
    // `tools/idlc`'s preamble follows for every other column.
    if !bodies.is_empty() {
        spec.insert(
            0,
            ada::Decl::Object(ada::ObjectDecl {
                doc: Doc::lines(&[
                    "The byte order this unit was generated for. The codec below writes",
                    "either order -- Zd_Wire.Writer carries it -- so this is what a caller",
                    "hands Zd_Wire.New_Writer to get the one that was asked for.",
                ]),
                name: Ident::new("Zd_Endianness"),
                type_: Path::new(&["Zd_Wire", "Byte_Order"]),
                value: ada::Expr::Name(Path::new(&["Zd_Wire", ada_endianness(key.endianness)])),
            }),
        );
    }
    let package = Ident::new(crate::adapter::ada::PACKAGE);
    let spec_text = zerodds_lang_render::ada::file(&ada::File {
        doc: Doc::none(),
        withs: Vec::new(),
        use_types: Vec::new(),
        package: package.clone(),
        decls: spec,
    });
    // `@verbatim(placement = BEGIN_FILE)` before the specification —
    // where an Ada file's context clauses are — and `END_FILE` after the
    // last unit.
    let (begin_file, end_file) = crate::verbatim::file_slots(ast, crate::verbatim::aliases::ADA);
    let head = text_block(&begin_file, "\n");
    let tail = text_block(&end_file, "\n\n");
    if bodies.is_empty() {
        return Ok(format!("{head}{}\n{tail}", spec_text.trim_end()));
    }
    let body_text = zerodds_lang_render::ada::package_body(&package, &bodies);
    Ok(format!(
        "{head}{}\n\n{}\n{tail}",
        spec_text.trim_end(),
        body_text.trim_end()
    ))
}

/// `@verbatim` text joined as the lines it is, followed by `separator`
/// — or nothing at all when there is none.
fn text_block(texts: &[String], separator: &str) -> String {
    if texts.is_empty() {
        return String::new();
    }
    let lines: Vec<&str> = texts.iter().flat_map(|text| text.lines()).collect();
    if separator == "\n" {
        format!("{}\n", lines.join("\n"))
    } else {
        format!("\n{}\n", lines.join("\n"))
    }
}

/// Ada 83's declarations and codec, as the two compilation units the
/// language asks for. [`ada_source`] for Ada 2012, and the same split
/// for the same reason (RM83 7.1).
///
/// No endianness constant of `Zd_Wire83.Byte_Order` is missing here: it
/// is the first declaration of the package, exactly as the Ada 2012
/// column's is.
///
/// # Errors
/// As [`source`].
fn ada83_source(ast: &Ast, key: LayoutKey, options: CodecOptions) -> Result<String, GenerateError> {
    use zerodds_lang_cst::ada83;
    use zerodds_lang_cst::common::{Doc, Ident, Path};

    let model = ModelGraph::of(ast);
    let applied = apply(&model, &rule::catalogue(Language::Ada83))
        .map_err(|error| GenerateError::Rules(format!("{error:?}")))?;
    let mut spec = crate::adapter::ada83::ada83_items(ast, &applied)
        .map_err(|error| GenerateError::NoForm(format!("{error:?}")))?;
    let codec = codec_of::<crate::emit::ada83::Ada83>(
        ast,
        key,
        options,
        crate::default_ada83::ada83_defaulters_called_by,
    )
    .map_err(codec_error)?;
    let mut bodies = Vec::new();
    for decl in codec {
        if matches!(decl, ada83::Decl::SubprogramBody(_)) {
            bodies.push(decl);
        } else {
            spec.push(decl);
        }
    }
    // As [`ada_source`]: no codec, no body and no constant. RM83 7.1
    // and GNAT's `-gnat83` answer the same way.
    if !bodies.is_empty() {
        spec.insert(
            0,
            ada83::Decl::Object(ada83::ObjectDecl {
                doc: Doc::lines(&[
                    "The byte order this unit was generated for. The codec below writes",
                    "either order -- Zd_Wire83.Writer carries it -- so this is what a caller",
                    "hands Zd_Wire83.New_Writer to get the one that was asked for.",
                ]),
                name: Ident::new("Zd_Endianness"),
                type_: Path::new(&["Zd_Wire83", "Byte_Order"]),
                value: ada83::Expr::Name(Path::new(&["Zd_Wire83", ada_endianness(key.endianness)])),
            }),
        );
    }
    let package = Ident::new(crate::adapter::ada83::PACKAGE);
    let spec_text = zerodds_lang_render::ada83::file(&ada83::File {
        doc: Doc::none(),
        withs: Vec::new(),
        package: package.clone(),
        decls: spec,
    });
    // The file slots as in `ada_source`.
    let (begin_file, end_file) = crate::verbatim::file_slots(ast, crate::verbatim::aliases::ADA83);
    let head = text_block(&begin_file, "\n");
    let tail = text_block(&end_file, "\n\n");
    if bodies.is_empty() {
        return Ok(format!("{head}{}\n{tail}", spec_text.trim_end()));
    }
    let body_text = zerodds_lang_render::ada83::package_body(&package, &bodies);
    Ok(format!(
        "{head}{}\n\n{}\n{tail}",
        spec_text.trim_end(),
        body_text.trim_end()
    ))
}

/// Rust's declarations and codec, folded into the module tree once.
///
/// Not `declarations` followed by `codec`: see [`source`]'s own comment.
/// What this fixes is not cosmetic. `impl Inner` was emitted at the top
/// level for an `Inner` declared in `module A { module B { … } }`, which
/// is `cannot find type `Inner` in this scope`, and every member path
/// inside the body was written relative to the module the item belongs
/// in — `super::A::B::Inner` read from the crate root, where `super`
/// leaves the crate. Placed in `A::B` and in `C` respectively, both
/// resolve.
///
/// # Errors
/// As [`source`].
fn rust_source(
    ast: &Ast,
    key: LayoutKey,
    mapping: ServiceMapping,
) -> Result<String, GenerateError> {
    let model = ModelGraph::of(ast);
    let applied = apply(&model, &rule::catalogue(Language::Rust))
        .map_err(|error| GenerateError::Rules(format!("{error:?}")))?;
    let mut pairs = crate::adapter::cst::rust_item_pairs(ast, &applied)
        .map_err(|error| GenerateError::NoForm(format!("{error:?}")))?;
    pairs.extend(rust_codec_pairs(ast, key)?);

    // `rust_item_pairs` still writes the flattened-trait answer the
    // codec-less `declarations()` entry point keeps — see its own
    // module doc and `crates/lang-rules/tests/inherited_operations.rs`
    // — which for a *remote* interface is now `emit::interface_rust`'s
    // job instead: two `pub trait I { .. }` in one module is
    // `error[E0428]`. Only a remote interface's trait is superseded —
    // `emit::interface_rust` writes none for a `local interface`
    // (`Service::is_remote`), so that one trait is the only answer and
    // stays exactly as `rust_item_pairs` wrote it.
    let remote_traits: std::collections::BTreeSet<String> =
        crate::adapter::interface::services(ast)
            .map_err(|error| GenerateError::NoForm(error.to_string()))?
            .into_iter()
            .filter(crate::adapter::interface::Service::is_remote)
            .map(|service| zerodds_lang_cst::rust::escape_keyword(&service.name))
            .collect();
    pairs.retain(|(_, item)| {
        !matches!(
            item,
            zerodds_lang_cst::rust::Item::Trait(trait_)
                if remote_traits.contains(trait_.name.as_str())
        )
    });

    // The DDS-RPC 1.0 binding of every `interface`: the request/reply
    // topic types with a real XCDR2 codec, a client, a service trait
    // and a dispatcher over `zerodds_rpc`. Folded into the same pairs
    // and the same `nest_by_module` pass as everything else — see that
    // function's own doc on why a second fold is not an option.
    pairs.extend(
        crate::emit::interface_rust::rust_interface_pairs_with(ast, mapping)
            .map_err(|error| GenerateError::NoForm(error.to_string()))?,
    );
    // A service defined by a pair of types (DDS-RPC 1.0 §7.3.1.4): the
    // topic-type impls and the endpoints over them.
    pairs.extend(
        crate::emit::type_pair_rust::rust_type_pair_items(ast).map_err(GenerateError::NoForm)?,
    );
    let items = crate::verbatim::weave_file(
        ast,
        crate::verbatim::aliases::RUST,
        crate::adapter::cst::nest_by_module(ast, pairs)
            .map_err(|error| GenerateError::NoForm(format!("{error:?}")))?,
    );
    Ok(render_rust(&items))
}

/// Rust's codec items, each kept with the module its type was declared
/// in so a caller can fold them with items of its own.
///
/// `crate::encode::rust_codec` is called once per type the model
/// declares that gets one: a structure, a union, a valuetype, an
/// enumeration, a bitmask or a bitset. The list is `zerodds-build`'s
/// `has_codec` plus `Valuetype`, and the three it adds are not optional.
/// A `struct` with an `enum` member emits `self.col.encode(w)?` — the
/// member's own method — so a model that declares an enumeration and
/// never gets a codec for it is `no method named `encode` found for enum
/// `Colour``, measured, not a smaller output.
///
/// An anonymous type — `node.name.is_empty()` — is never one of these:
/// only a `sequence`, an `array` and a `map` synthesise one, and none of
/// the three is ever a top-level declaration.
///
/// # Errors
/// [`GenerateError::Codec`] when the layout cannot be planned or a
/// construct has no Rust writer.
fn rust_codec_pairs(
    ast: &Ast,
    key: LayoutKey,
) -> Result<Vec<(Id<zerodds_ast::ModuleNode>, zerodds_lang_cst::rust::Item)>, GenerateError> {
    let mut pairs = Vec::new();
    // A model with interfaces gets the XCDR1 pair of every type that has
    // one: DDS-RPC 1.0 §7.5.1.1, the Basic Service Mapping, is classic CDR,
    // and the topic types call the XCDR1 pair of the types they reach
    // (`emit::interface_rust`).
    let rpc = ast.interfaces().next().is_some() || crate::emit::type_pair_rust::has_pair_types(ast);
    for (id, node) in ast.types() {
        if node.name.is_empty() || !crate::encode::has_codec(&node.kind) {
            continue;
        }
        for item in crate::encode::rust_codec(ast, id, key).map_err(codec_error)? {
            pairs.push((node.module, item));
        }
        if rpc {
            if let Ok(items) = crate::encode::rust_codec_pair(
                ast,
                id,
                crate::emit::interface_rust::rpc_layout_key_xcdr1(),
            ) {
                for item in items {
                    pairs.push((node.module, item));
                }
            }
        }
    }
    Ok(pairs)
}

/// `module.exports = { … };` over every function the Node column
/// declares.
///
/// CommonJS gives a module no other way to publish a name: a
/// `function` in a `.js` file is local to it, and
/// `require("./gen.js").zd_enc_Max` answered `undefined`. Nothing said
/// so, because the probe harness writes its own entry point into the
/// generated file rather than requiring it — the same blind spot the Go
/// and Zig columns had.
///
/// `None` where the run declared no function at all, so an empty model
/// does not gain a bare `module.exports = {}`.
fn node_exports(decls: &[zerodds_lang_cst::node::Stat]) -> Option<zerodds_lang_cst::node::Stat> {
    use zerodds_lang_cst::node::{Expr, Stat};
    let names: Vec<(zerodds_lang_cst::common::Ident, Expr)> = decls
        .iter()
        .filter_map(|decl| match decl {
            Stat::Function(function) => {
                Some((function.name.clone(), Expr::Name(function.name.clone())))
            }
            _ => None,
        })
        .collect();
    if names.is_empty() {
        return None;
    }
    Some(Stat::Assign {
        doc: zerodds_lang_cst::common::Doc::line(
            "What a caller outside this file can reach. CommonJS publishes nothing by default.",
        ),
        place: Expr::Member {
            object: Box::new(Expr::name("module")),
            name: zerodds_lang_cst::common::Ident::new("exports"),
        },
        value: Expr::Object(names),
    })
}

/// Rust items as text, one blank line between them.
fn render_rust(items: &[zerodds_lang_cst::rust::Item]) -> String {
    items
        .iter()
        .map(zerodds_lang_render::rust::item)
        .collect::<Vec<_>>()
        .join("\n")
}

/// [`EncodeError`] into [`GenerateError`], the one conversion every arm
/// below shares.
fn codec_error(error: EncodeError) -> GenerateError {
    GenerateError::Codec(format!("{error:?}"))
}

/// The encoder, the Table 9 defaulters and the decoder for one
/// [`CodecTarget`], over the whole model and under one plan.
///
/// `defaulters` is the language's own `<lang>_defaulters_called_by`,
/// which reads the decoders it is handed to find which Table 9
/// defaulters they call. A parameter and not a method on `T`: the twenty
/// backends do not share one name for it, and `decode_c::c_decoder`
/// binds the same function this way for the same reason.
///
/// zerodds-lint: recursion-depth 1 — this does not recurse. The callee
/// is `crate::emit::encode::codec`; `dds_bounded_recursion` compares only the last
/// segment of a call path, so it reads as a self-call. The two
/// share a last segment and nothing else.
fn codec_of<T: CodecTarget>(
    ast: &Ast,
    key: LayoutKey,
    options: CodecOptions,
    defaulters: impl FnOnce(&Ast, &[T::Decl]) -> Result<Vec<T::Decl>, EncodeError>,
) -> Result<Vec<T::Decl>, EncodeError> {
    let mut decls = crate::emit::encode::codec::<T>(ast, key, options)?;
    let decoders = crate::emit::decode::decoder::<T>(ast, key, options)?;
    decls.extend(defaulters(ast, &decoders)?);
    decls.extend(decoders);
    Ok(decls)
}

/// [`codec_of`] for C++, without the types the RPC binding of a pair of
/// types (DDS-RPC 1.0 §7.3.1.4) provides itself.
fn cpp_codec_of(
    ast: &Ast,
    key: LayoutKey,
    options: CodecOptions,
) -> Result<Vec<zerodds_lang_cst::cpp::Decl>, EncodeError> {
    use crate::emit::cpp::Cpp;
    let excluded = crate::emit::type_pair_cpp::excluded_types(ast);
    let mut decls = crate::emit::encode::codec_excluding::<Cpp>(ast, key, options, &excluded, &[])?;
    let decoders =
        crate::emit::decode::decoder_excluding::<Cpp>(ast, key, options, &excluded, &[])?;
    decls.extend(crate::default_cpp::cpp_defaulters_called_by_excluding(
        ast, &decoders, &excluded,
    )?);
    decls.extend(decoders);
    Ok(decls)
}

/// [`codec_of`] for Java, without the types the RPC binding of a pair of
/// types (DDS-RPC 1.0 §7.3.1.4) provides itself and with the codecs of the
/// members of the pair types, which the binding calls.
fn java_codec_members(
    ast: &Ast,
    key: LayoutKey,
    options: CodecOptions,
) -> Result<Vec<zerodds_lang_cst::java::Member>, EncodeError> {
    use crate::emit::java::Java;
    let excluded = crate::emit::type_pair_java::excluded_types(ast);
    let mut reaching = crate::emit::type_pair_java::body_roots(ast);
    // The binding of an interface calls the codecs of the types its
    // operations carry.
    for id in crate::emit::interface_cpp::remote_member_types(ast).map_err(|error| {
        EncodeError::NoLayout {
            detail: error.to_string(),
        }
    })? {
        if !reaching.contains(&id) {
            reaching.push(id);
        }
    }
    let mut decls =
        crate::emit::encode::codec_excluding::<Java>(ast, key, options, &excluded, &reaching)?;
    let decoders =
        crate::emit::decode::decoder_excluding::<Java>(ast, key, options, &excluded, &reaching)?;
    decls.extend(crate::default_java::java_defaulters_called_by(
        ast, &decoders,
    )?);
    decls.extend(decoders);
    Ok(decls)
}

/// [`java_codec_members`] and, when the model has a service, the same
/// codecs under classic CDR in the nested class `Xcdr1`: the Basic mapping
/// of DDS-RPC 1.0 §7.5.1.1 writes XCDR1 unless the params say XCDR2, and a
/// nested `@appendable` member has no DHEADER there while a `@mutable`
/// one is a parameter list (XTypes 1.3 §7.4.1.1, §7.4.1.2). The binding
/// calls `ZdCodec.Xcdr1.<name>` for the former and `ZdCodec.<name>` for
/// the latter.
fn java_codec_of(
    ast: &Ast,
    key: LayoutKey,
    options: CodecOptions,
) -> Result<Vec<zerodds_lang_cst::java::Member>, EncodeError> {
    use zerodds_lang_cst::java;
    let mut members = java_codec_members(ast, key, options)?;
    members.extend(crate::emit::type_pair_java::header_codecs(ast));
    if ast.interfaces().next().is_some() || crate::emit::type_pair_java::has_pair_types(ast) {
        let classic = LayoutKey {
            encoding: zerodds_ast_layout::Encoding::PlainCdr1,
            ..key
        };
        members.push(java::Member::Nested(Box::new(java::TypeDecl::Class(
            java::Class {
                doc: zerodds_lang_cst::common::Doc::line(
                    "The codecs of `ZdCodec` under classic CDR (XCDR1). DDS-RPC 1.0 \u{a7}7.5.1.1.",
                ),
                annotations: Vec::new(),
                modifiers: vec![java::Modifier::Public, java::Modifier::Static],
                name: zerodds_lang_cst::common::Ident::new("Xcdr1"),
                generics: Vec::new(),
                extends: None,
                implements: Vec::new(),
                members: java_codec_members(ast, classic, options)?,
            },
        ))));
    }
    Ok(members)
}

/// Two spaces in front of every non-empty line.
///
/// `wireprobe-zerodds-elixir-gen`'s own helper: Elixir's codec functions
/// render at the top level and this is what nests them inside
/// `defmodule ZdCodec`.
fn indent(text: &str) -> String {
    let mut out = String::new();
    for line in text.lines() {
        if line.is_empty() {
            out.push('\n');
        } else {
            out.push_str("  ");
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// The codec of `ast`, as source in `language`.
///
/// One arm per language, the same twenty-one [`declarations`] answers.
/// Fifteen share [`codec_of`] and a plain per-declaration render; four
/// more — C#, Java, Nim, Elixir — share [`codec_of`] but not the render,
/// for a reason each arm's comment gives. OCaml and Rust share neither:
/// OCaml's Table 9 defaulters are a distinct CST type from its codec
/// (`ocaml::ValueFn` beside `ocaml::CodecFn`), and Rust has no
/// [`CodecTarget`] at all — its codec is `crate::encode::rust_codec`,
/// walked directly against the Rust CST, which the module doc on
/// `crate::emit::encode` already gives the reason for: it "has not been
/// brought over". Every one of the twenty-one produces a full codec;
/// none is declarations-only.
///
/// zerodds-lint: recursion-depth 1 — this does not recurse. It calls
/// `codec_of`, and what `codec_of` calls back is
/// `crate::emit::encode::codec`, a different function that shares a
/// last segment with this one.
#[allow(clippy::too_many_lines)]
fn codec(
    ast: &Ast,
    language: Language,
    key: LayoutKey,
    options: CodecOptions,
) -> Result<String, GenerateError> {
    let text = match language {
        Language::Lua => {
            let decls = codec_of::<crate::emit::lua::Lua>(
                ast,
                key,
                options,
                crate::default_lua::lua_defaulters_called_by,
            )
            .map_err(codec_error)?;
            decls
                .iter()
                .map(zerodds_lang_render::lua::stat)
                .collect::<Vec<_>>()
                .join("\n")
        }
        Language::Ada => {
            let decls = codec_of::<crate::emit::ada::Ada>(
                ast,
                key,
                options,
                crate::default_ada::ada_defaulters_called_by,
            )
            .map_err(codec_error)?;
            decls
                .iter()
                .map(zerodds_lang_render::ada::decl)
                .collect::<Vec<_>>()
                .join("\n")
        }
        Language::C => {
            let decls = codec_of::<crate::emit::c::C>(
                ast,
                key,
                options,
                crate::default_c::c_defaulters_called_by,
            )
            .map_err(codec_error)?;
            decls
                .iter()
                .map(zerodds_lang_render::c::decl)
                .collect::<Vec<_>>()
                .join("\n")
        }
        Language::Cpp => {
            let decls = cpp_codec_of(ast, key, options).map_err(codec_error)?;
            decls
                .iter()
                .map(zerodds_lang_render::cpp::decl)
                .collect::<Vec<_>>()
                .join("\n")
        }
        // C# has no free functions: a codec is a `static` method, and a
        // method needs a class to live on. `ZdCodec` is
        // `wireprobe-zerodds-csharp-gen`'s own name for it.
        Language::CSharp => {
            let members = codec_of::<crate::emit::csharp::CSharp>(
                ast,
                key,
                options,
                crate::default_csharp::csharp_defaulters_called_by,
            )
            .map_err(codec_error)?;
            zerodds_lang_render::csharp::type_decl(&zerodds_lang_cst::csharp::TypeDecl::Class(
                zerodds_lang_cst::csharp::Class {
                    doc: zerodds_lang_cst::common::Doc::none(),
                    attributes: Vec::new(),
                    modifiers: vec![
                        zerodds_lang_cst::csharp::Modifier::Public,
                        zerodds_lang_cst::csharp::Modifier::Static,
                    ],
                    name: zerodds_lang_cst::common::Ident::new("ZdCodec"),
                    generics: Vec::new(),
                    bases: Vec::new(),
                    constraints: Vec::new(),
                    members,
                },
            ))
        }
        Language::D => {
            let decls = codec_of::<crate::emit::d::D>(
                ast,
                key,
                options,
                crate::default_d::d_defaulters_called_by,
            )
            .map_err(codec_error)?;
            decls
                .iter()
                .map(zerodds_lang_render::d::decl)
                .collect::<Vec<_>>()
                .join("\n")
        }
        // Elixir's codec functions render at the top level and Elixir
        // has no top level: every `def` needs a `defmodule` around it.
        // `wireprobe-zerodds-elixir-gen` writes exactly this module,
        // `import Bitwise` included, and this is that pattern with the
        // probe's harness and dump left out.
        Language::Elixir => {
            let decls = codec_of::<crate::emit::elixir::Elixir>(
                ast,
                key,
                options,
                crate::default_elixir::elixir_defaulters_called_by,
            )
            .map_err(codec_error)?;
            let mut functions = String::new();
            for function in &decls {
                functions.push('\n');
                functions.push_str(&indent(&zerodds_lang_render::elixir::codec_function(
                    function,
                )));
            }
            let mut out = String::from("defmodule ZdCodec do\n");
            if ["&&&", "|||", ">>>", "<<<"]
                .iter()
                .any(|operator| functions.contains(operator))
            {
                out.push_str("  import Bitwise\n");
            }
            out.push_str(&functions);
            out.push_str("end\n");
            out
        }
        Language::Go => {
            let decls = codec_of::<crate::emit::go::Go>(
                ast,
                key,
                options,
                crate::default_go::go_defaulters_called_by,
            )
            .map_err(codec_error)?;
            decls
                .iter()
                .map(zerodds_lang_render::go::decl)
                .collect::<Vec<_>>()
                .join("\n")
        }
        // Java, the same reason as C#: a codec is an instance-free
        // method and a method needs a class.
        Language::Java => {
            let members = java_codec_of(ast, key, options).map_err(codec_error)?;
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
        Language::Julia => {
            let decls = codec_of::<crate::emit::julia::Julia>(
                ast,
                key,
                options,
                crate::default_julia::julia_defaulters_called_by,
            )
            .map_err(codec_error)?;
            decls
                .iter()
                .map(zerodds_lang_render::julia::decl)
                .collect::<Vec<_>>()
                .join("\n")
        }
        // Nim's renderer joins a `seq` of procedures itself
        // (`proc_decls`, plural) rather than taking one at a time; every
        // other backend's renderer does not, and this is the one place
        // that shows.
        Language::Nim => {
            let decls = codec_of::<crate::emit::nim::Nim>(
                ast,
                key,
                options,
                crate::default_nim::nim_defaulters_called_by,
            )
            .map_err(codec_error)?;
            zerodds_lang_render::nim::proc_decls(&decls)
        }
        Language::Node => {
            let mut decls = codec_of::<crate::emit::node::Node>(
                ast,
                key,
                options,
                crate::default_node::node_defaulters_called_by,
            )
            .map_err(codec_error)?;
            if let Some(exports) = node_exports(&decls) {
                decls.push(exports);
            }
            decls
                .iter()
                .map(zerodds_lang_render::node::stat)
                .collect::<Vec<_>>()
                .join("\n")
        }
        // OCaml alone: its Table 9 defaulters are `ocaml::ValueFn`, a
        // type distinct from the codec's own `ocaml::CodecFn` — the one
        // backend whose `<lang>_defaulters_called_by` does not return
        // `Vec<T::Decl>`, so [`codec_of`] cannot bind it. `codec_chain`
        // and `value_chain` are `ocaml.rs`'s own renderers for the two;
        // the defaulters render ahead of the decoders that call them,
        // the same order `codec_of` gives every other backend.
        Language::OCaml => {
            let encoders =
                crate::emit::encode::codec::<crate::emit::ocaml::OCaml>(ast, key, options)
                    .map_err(codec_error)?;
            let decoders =
                crate::emit::decode::decoder::<crate::emit::ocaml::OCaml>(ast, key, options)
                    .map_err(codec_error)?;
            let defaulters = crate::default_ocaml::ocaml_defaulters_called_by(ast, &decoders)
                .map_err(codec_error)?;
            let mut out = zerodds_lang_render::ocaml::codec_chain(&encoders);
            out.push('\n');
            out.push_str(&zerodds_lang_render::ocaml::value_chain(&defaulters));
            out.push('\n');
            out.push_str(&zerodds_lang_render::ocaml::codec_chain(&decoders));
            out
        }
        Language::Python => {
            let decls = codec_of::<crate::emit::python::Python>(
                ast,
                key,
                options,
                crate::default_python::python_defaulters_called_by,
            )
            .map_err(codec_error)?;
            decls
                .iter()
                .map(zerodds_lang_render::python::statement)
                .collect::<Vec<_>>()
                .join("\n")
        }
        // Rust has no `CodecTarget` backend — see this function's own
        // doc — so its codec is `crate::encode::rust_codec`, gathered by
        // [`rust_codec_pairs`] and folded into the module tree here:
        // `impl Inner` belongs in the module `Inner` was declared in,
        // and the member paths in its body are written relative to that
        // module.
        //
        // [`source`] does not come through this arm. It folds these
        // pairs together with the declarations' in one pass, because two
        // separate folds declare every shared ancestor twice. This arm
        // is the codec on its own — correct in isolation, which is what
        // a caller asking for only the codec gets.
        Language::Rust => {
            let items = crate::adapter::cst::nest_by_module(ast, rust_codec_pairs(ast, key)?)
                .map_err(|error| GenerateError::NoForm(format!("{error:?}")))?;
            render_rust(&items)
        }
        Language::Swift => {
            let decls = codec_of::<crate::emit::swift::Swift>(
                ast,
                key,
                options,
                crate::default_swift::swift_defaulters_called_by,
            )
            .map_err(codec_error)?;
            decls
                .iter()
                .map(zerodds_lang_render::swift::decl)
                .collect::<Vec<_>>()
                .join("\n")
        }
        Language::TypeScript => {
            let decls = codec_of::<crate::emit::typescript::TypeScript>(
                ast,
                key,
                options,
                crate::default_typescript::typescript_defaulters_called_by,
            )
            .map_err(codec_error)?;
            decls
                .iter()
                .map(zerodds_lang_render::typescript::decl)
                .collect::<Vec<_>>()
                .join("\n")
        }
        Language::Zig => {
            let decls = codec_of::<crate::emit::zig::Zig>(
                ast,
                key,
                options,
                crate::default_zig::zig_defaulters_called_by,
            )
            .map_err(codec_error)?;
            decls
                .iter()
                .map(zerodds_lang_render::zig::decl)
                .collect::<Vec<_>>()
                .join("\n")
        }
        Language::Ada83 => {
            let decls = codec_of::<crate::emit::ada83::Ada83>(
                ast,
                key,
                options,
                crate::default_ada83::ada83_defaulters_called_by,
            )
            .map_err(codec_error)?;
            decls
                .iter()
                .map(zerodds_lang_render::ada83::decl)
                .collect::<Vec<_>>()
                .join("\n")
        }
        Language::FSharp => {
            let decls = codec_of::<crate::emit::fsharp::Fsharp>(
                ast,
                key,
                options,
                crate::default_fsharp::fsharp_defaulters_called_by,
            )
            .map_err(codec_error)?;
            decls
                .iter()
                .map(zerodds_lang_render::fsharp::decl)
                .collect::<Vec<_>>()
                .join("\n")
        }
        Language::Kotlin => {
            let decls = codec_of::<crate::emit::kotlin::Kotlin>(
                ast,
                key,
                options,
                crate::default_kotlin::kotlin_defaulters_called_by,
            )
            .map_err(codec_error)?;
            decls
                .iter()
                .map(zerodds_lang_render::kotlin::decl)
                .collect::<Vec<_>>()
                .join("\n")
        }
    };
    Ok(text)
}
