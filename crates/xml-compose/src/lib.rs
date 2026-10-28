// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-XML type definitions into the common AST.
//!
//! Crate `zerodds-xml-compose`. Safety classification: **STANDARD**.
//! Build-time only, std, `forbid(unsafe_code)`.
//!
//! One call — [`compose`] — takes a path to a DDS-XML document and
//! produces the same [`Ast`] that `zerodds-idl-compose` produces from an
//! `.idl` file, plus the type objects the model registers. Everything
//! downstream of the model — `zerodds_types::from_ast`, the layout
//! planner, `zerodds-wire-codec`, every code generator — reads the model
//! and does not know or care which front end filled it.
//!
//! ## Why a crate of its own rather than more of `zerodds-xml`
//!
//! `zerodds-xml` reads DDS-XML already, and stops one layer short: it
//! builds its own descriptive [`zerodds_xml::TypeLibrary`] and bridges
//! that to a `MinimalTypeObject`. That layer is a published API with
//! its own users, and it drops exactly what a model needs — a member
//! carries no `tryConstruct`, no `hashid` and no `external`, a struct
//! carries no `autoid`, an enum carries neither extensibility nor
//! `defaultLiteral`, a union carries no extensibility and no
//! discriminator key, and its primitive table has no `int8`/`uint8` at
//! all. Widening those structs would change signatures every current
//! caller compiles against.
//!
//! So this crate takes the same position against `zerodds-xml` that
//! `zerodds-idl-compose` takes against `zerodds-idl-parse`: it reuses
//! the front end's well-formedness layer — [`zerodds_xml::parse_xml_tree`]
//! and its element tree, with the DDS-XML 1.0 §7.1 depth cap, element
//! budget and namespace handling — and lowers that tree into the model
//! itself. Nothing in `zerodds-xml` changes.
//!
//! ## Spec sources
//!
//! * OMG DDS-XML 1.0 (formal/18-10-01) §7.1 (representation syntax),
//!   §7.3.3 (Types building block).
//! * OMG DDS-XTypes 1.3 Annex A (XML Type Representation) for the type
//!   grammar the `<types>` element carries, and §7.3.1.2.1 for what the
//!   attributes mean.
//! * OMG IDL 4.2 §7.4 where the two representations share a rule.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod expr;
mod lower;

use std::path::{Path, PathBuf};

use zerodds_ast::Ast;
use zerodds_types::resolve::TypeRegistry;

pub use lower::{Diagnostic, Severity};

/// Re-exported because [`ComposeOptions`] carries them in public fields,
/// exactly as `zerodds-idl-compose` re-exports them: a caller that
/// cannot name the type cannot construct the options.
pub use zerodds_idl_lower::{HashedIdCounter, HashedNameSpelling, KeyOnUnionDiscriminator};

/// What one composition produced.
///
/// Field for field what `zerodds_idl_compose::Composed` carries, so a
/// caller that already loads IDL can add XML without a second shape to
/// handle.
pub struct Composed {
    /// The model, built as far as the sources allowed.
    pub ast: Ast,
    /// Every type object the model produced, by hash.
    pub registry: TypeRegistry,
    /// Everything wrong, in the order it was found.
    pub diagnostics: Vec<Diagnostic>,
    /// Types no type object could be built for, with the reason.
    pub without_type_objects: Vec<(String, String)>,
}

impl Composed {
    /// Whether any stage reported an error.
    #[must_use]
    pub fn failed(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    }
}

/// What the caller decides that the document does not.
///
/// The three fields are the three `zerodds_idl_compose::ComposeOptions`
/// carries that a type document can be affected by. `defines` has no
/// counterpart: DDS-XML has no preprocessor.
#[derive(Debug, Clone, Default)]
pub struct ComposeOptions {
    /// What an aggregate of unspecified extensibility is taken to be.
    ///
    /// `None` leaves the facet absent, so "the document said appendable"
    /// stays distinguishable from "the document said nothing" and
    /// `Ast::effective_extensibility` applies XTypes 1.3 §7.3.1.2.1.8's
    /// default. What the document *did* say still wins.
    pub default_extensibility: Option<zerodds_ast::Extensibility>,
    /// Whether a hashed member id advances the sequential id counter.
    ///
    /// The same dialect axis `zerodds-idl-compose` carries, and here for
    /// the same reason: a type declared once in IDL and once in XML has
    /// to reach the same member ids, and it cannot if the two load paths
    /// answer this differently.
    pub hashed_id_advances_the_counter: HashedIdCounter,
    /// Which spelling of an escaped name a member-id hash reads.
    ///
    /// Carried for parity with the IDL path. DDS-XML has no escape
    /// underscore — a `<member name="long">` writes the name as it is —
    /// so no document reaches the axis today.
    pub hashed_name_spelling: HashedNameSpelling,
    /// Whether `key="true"` on a `<discriminator>` is carried.
    ///
    /// The same axis the IDL path carries for `@key` in that position,
    /// and here for the reason the two above are here: one declaration
    /// written twice has to reach one model. DDS-XML documents are the
    /// only ones that reach it today — the OMG `dds-xtypes` suite's
    /// `types/xml/unions_key_discriminator.xml` is the case.
    pub key_on_union_discriminator: KeyOnUnionDiscriminator,
    /// Directories searched for the files an `<include>` names, after the
    /// directory of the document that includes them.
    ///
    /// DDS-XTypes 1.3 §7.5.2.2.13: "A collection of URLs to directories to
    /// be searched for additional type description documents that may be
    /// included, directly or indirectly [...] The directory in which the
    /// target of document_url resides shall be considered on the
    /// inclusion search path implicitly".
    pub include_paths: Vec<PathBuf>,
    /// Whether a collection bound of `0` means "unbounded".
    ///
    /// DDS-XTypes 1.3 §7.3.2.4: "The literal expression "-1" shall
    /// indicate an unbounded collection; no other "negative" value is
    /// permitted." Zero is not negative, but it is no bound either:
    /// IDL 4.2 §7.4.1.4.4.3.1 takes a `<positive_int_const>` for the bound
    /// of a sequence, a string and a map, so a bound of 0 is refused. The
    /// OMG `dds-xtypes` suite writes `sequenceMaxLength="0"` and comments
    /// it "unlimited (0 or -1)"; a caller that loads the suite sets this.
    pub zero_bound_is_unbounded: bool,
}

/// Compose one DDS-XML document from a path.
///
/// # Errors
/// Only the read of `path` itself: everything after it is reported
/// through [`Composed::diagnostics`], because a document with one bad
/// declaration still has the rest.
pub fn compose(path: &str) -> Result<Composed, String> {
    compose_with(path, ComposeOptions::default())
}

/// As [`compose`], deciding what the document leaves open.
///
/// # Errors
/// As [`compose`].
pub fn compose_with(path: &str, options: ComposeOptions) -> Result<Composed, String> {
    let source = std::fs::read_to_string(path).map_err(|error| format!("{path}: {error}"))?;
    Ok(compose_source_with(path, &source, options))
}

/// As [`compose`], over text the caller already has.
#[must_use]
pub fn compose_source(name: &str, source: &str) -> Composed {
    compose_source_with(name, source, ComposeOptions::default())
}

/// As [`compose_source`], deciding what the document leaves open.
#[must_use]
pub fn compose_source_with(name: &str, source: &str, options: ComposeOptions) -> Composed {
    compose_all_sources(&[(name.to_owned(), source.to_owned())], options)
}

/// Compose several documents into **one** model.
///
/// The units are lowered into a shared `Ast`, so a module two of them
/// declare is one node — the same guarantee
/// `zerodds_idl_compose::compose_all` gives, and for the same reason:
/// two separate models emit the same module twice and the two collide
/// in one Rust scope.
///
/// # Errors
/// The read of any unit, naming it.
pub fn compose_all(
    paths: &[impl AsRef<Path>],
    options: ComposeOptions,
) -> Result<Composed, String> {
    let mut sources = Vec::with_capacity(paths.len());
    for path in paths {
        let path: &Path = path.as_ref();
        let name = path.to_string_lossy().into_owned();
        let text = std::fs::read_to_string(path).map_err(|error| format!("{name}: {error}"))?;
        sources.push((name, text));
    }
    Ok(compose_all_sources(&sources, options))
}

/// As [`compose_all`], over text the caller already has.
///
/// Each entry is `(name, source)`; the name is what a diagnostic points
/// at.
#[must_use]
pub fn compose_all_sources(sources: &[(String, String)], options: ComposeOptions) -> Composed {
    let mut ast = Ast::new();
    let diagnostics = lower::lower_all(&mut ast, sources, &options);

    let mut registry = TypeRegistry::new();
    let mut diagnostics = diagnostics;
    // A failure of the registration pass itself is reported, not turned
    // into an empty list. An empty `without_type_objects` reads as
    // "every type has an object", which is the opposite of what an error
    // here means, and a caller that trusted it would announce a type it
    // cannot serve.
    let without_type_objects = match zerodds_types::from_ast::register_all(&ast, &mut registry) {
        Ok(skipped) => skipped
            .into_iter()
            .map(|(name, error)| (name, format!("{error:?}")))
            .collect(),
        Err(error) => {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                message: format!("no type object could be built for this model: {error:?}"),
                at: sources
                    .first()
                    .map_or_else(|| String::from("<bundle>"), |(name, _)| name.clone()),
                spec_reference: "DDS-XTypes 1.3 §7.3.4",
            });
            Vec::new()
        }
    };

    Composed {
        ast,
        registry,
        diagnostics,
        without_type_objects,
    }
}

/// DDS-XTypes 1.3 §7.5.2.2.14 `create_type_w_document`: the type named
/// `type_name` of the XML Type Representation in `document`.
///
/// `include_paths` are the directories searched for the files the
/// document includes; a file is looked for beside the document that
/// names it first (here the current directory, a document in memory has
/// none of its own), then in each of them.
///
/// # Errors
/// "If an error occurs, this method shall return a nil value": the
/// diagnostics of the document if it has an error, or the name of the
/// type if the document has no such type — "which will trivially be the
/// case if the name is nil or the empty string".
pub fn create_type_w_document(
    document: &str,
    type_name: &str,
    include_paths: &[&str],
) -> Result<zerodds_types::dynamic::DynamicType, String> {
    let options = ComposeOptions {
        include_paths: include_paths.iter().map(PathBuf::from).collect(),
        ..ComposeOptions::default()
    };
    let composed = compose_source_with("<document>", document, options);
    type_of_composed(composed, type_name)
}

/// DDS-XTypes 1.3 §7.5.2.2.13 `create_type_w_uri`: the type named
/// `type_name` of the XML Type Representation at `document_url`.
///
/// "Implementations shall minimally support the file:// URL scheme": a
/// `file://` URL, and a plain path, are read; another scheme is refused.
/// The directory of the document is on the include search path
/// implicitly.
///
/// # Errors
/// As [`create_type_w_document`], and a URL of another scheme or a file
/// that cannot be read.
pub fn create_type_w_uri(
    document_url: &str,
    type_name: &str,
    include_paths: &[&str],
) -> Result<zerodds_types::dynamic::DynamicType, String> {
    let path = match document_url.split_once("://") {
        Some(("file", rest)) => rest,
        Some((scheme, _)) => {
            return Err(format!(
                "`{scheme}://` is not supported; only file:// URLs are read (§7.5.2.2.13)"
            ));
        }
        None => document_url,
    };
    let options = ComposeOptions {
        include_paths: include_paths.iter().map(PathBuf::from).collect(),
        ..ComposeOptions::default()
    };
    let composed = compose_with(path, options)?;
    type_of_composed(composed, type_name)
}

fn type_of_composed(
    composed: Composed,
    type_name: &str,
) -> Result<zerodds_types::dynamic::DynamicType, String> {
    if composed.failed() {
        return Err(composed
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("; "));
    }
    let ast = std::sync::Arc::new(composed.ast);
    zerodds_types::dynamic::DynamicType::find(&ast, type_name).map_err(|e| e.to_string())
}

/// The `<type-folder>/xml/<type-file>.xml` a caller names by its two
/// halves, which is the layout the OMG `dds-xtypes` suite uses.
///
/// Here rather than in the tool because the same layout is what every
/// other consumer of a DDS-XML type folder sees.
#[must_use]
pub fn xml_path(folder: &str, file: &str) -> PathBuf {
    Path::new(folder).join("xml").join(format!("{file}.xml"))
}
