// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The shared IDL composition pipeline.
//!
//! Crate `zerodds-idl-compose`. Safety classification: **STANDARD**.
//! Build-time only, std, `forbid(unsafe_code)`.
//!
//! One call — [`compose`] — takes a path and produces the model plus its
//! registered type objects. Every build-time consumer goes through it, so
//! a `build.rs` and the command-line tool see the same thing from the
//! same file.
//!
//! ## What this crate no longer does
//!
//! It used to do five things, and it existed because only the CLI did all
//! five: the `build.rs` scripts called the parser directly and got none
//! of them. Four have since moved into the front end, where they belong:
//!
//! | step | where it is now |
//! |---|---|
//! | `#include` / `#define` expansion | [`mod@zerodds_idl_parse::preprocess`] |
//! | parse | [`zerodds_idl_parse::parse_preprocessed`] |
//! | vendor key pragmas → `@key` | `zerodds-idl-lower`'s `apply_keylists` |
//! | default extensibility | `Ast::effective_extensibility` |
//! | type objects | [`zerodds_types::from_ast`] |
//!
//! The fourth is the one worth naming. It used to **write** an explicit
//! `@final`/`@appendable` onto every unannotated aggregate so that each
//! backend would agree. The model answers the question instead, in one
//! place, and nothing is written into the tree — which is what stops two
//! backends from disagreeing without either of them being wrong.
//!
//! What is left here is the composition itself: resolving include paths
//! against a search list, and running the four stages in order.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use zerodds_ast::Ast;
use zerodds_idl_parse::{Diagnostic, FileLoader, Severity};
use zerodds_types::resolve::TypeRegistry;

/// A loader over the filesystem, with a search list for `#include`.
///
/// A quoted include resolves against the including file's own directory
/// first, which is what every C preprocessor does and what an IDL author
/// expects when they write `#include "common.idl"` next to their file.
/// Only then does the search list apply. Getting that order wrong makes
/// a file compile against a header from somewhere else, and the two are
/// indistinguishable in the output.
pub struct PathLoader {
    roots: Vec<PathBuf>,
    /// Every file this loader actually opened, in the order it opened
    /// them.
    ///
    /// A build has to know: an `#include` that changed must trigger a
    /// rebuild, and cargo only re-runs a build script for paths it was
    /// told about. Without this the generated code goes stale after an
    /// edit to a header, and nothing reports it — the build is simply
    /// not re-run.
    opened: RefCell<Vec<String>>,
}

impl PathLoader {
    /// A loader over `roots`, searched in the order given.
    #[must_use]
    pub fn new(roots: Vec<PathBuf>) -> Self {
        Self {
            roots,
            opened: RefCell::new(Vec::new()),
        }
    }

    /// A loader with no search list — quoted includes still resolve
    /// beside the including file.
    #[must_use]
    pub fn bare() -> Self {
        Self::new(Vec::new())
    }

    /// Every file opened so far, in order.
    #[must_use]
    pub fn opened(&self) -> Vec<String> {
        self.opened.borrow().clone()
    }
}

impl FileLoader for PathLoader {
    fn load(&self, path: &str, from: Option<&str>) -> Option<(String, String)> {
        let mut candidates: Vec<PathBuf> = Vec::new();
        // Beside the including file first.
        if let Some(from) = from {
            if let Some(parent) = Path::new(from).parent() {
                candidates.push(parent.join(path));
            }
        }
        candidates.extend(self.roots.iter().map(|root| root.join(path)));
        // A path that is already absolute, or that resolves from the
        // working directory, is the last resort rather than the first:
        // an include that only works because of where the build ran is
        // the one that breaks on another machine.
        candidates.push(PathBuf::from(path));

        for candidate in candidates {
            if let Ok(text) = std::fs::read_to_string(&candidate) {
                let resolved = candidate.to_string_lossy().into_owned();
                self.opened.borrow_mut().push(resolved.clone());
                return Some((resolved, text));
            }
        }
        None
    }
}

/// What one composition produced.
pub struct Composed {
    /// The model, built as far as the sources allowed.
    pub ast: Ast,
    /// Every type object the model produced, by hash.
    pub registry: TypeRegistry,
    /// Everything wrong, from every stage, in the order the stages ran.
    pub diagnostics: Vec<Diagnostic>,
    /// Types no type object could be built for, with the reason.
    ///
    /// Not a diagnostic: a type that has no `TypeObject` is still a type
    /// a backend can generate. It is listed so a caller that needs the
    /// objects — discovery, a type lookup service — finds out which ones
    /// it will not have, instead of discovering it at run time.
    pub without_type_objects: Vec<(String, String)>,
    /// The unit as the parser saw it, for placing a diagnostic raised on
    /// the expanded text afterwards in the file a reader has. The model's
    /// nodes are in file offsets already: their `SourceFacet` names the
    /// file and the range in it. `None` for a bundle of units.
    pub expanded: Option<zerodds_idl_parse::Preprocessed>,
}

impl Composed {
    /// Moves a diagnostic raised on this model's spans into the file it
    /// points at; see `Preprocessed::locate`. Leaves it alone where the
    /// model came from several units.
    pub fn locate(&self, diagnostic: &mut Diagnostic) {
        if let Some(expanded) = &self.expanded {
            expanded.locate(diagnostic);
        }
    }

    /// Whether any stage reported an error.
    #[must_use]
    pub fn failed(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    }
}

/// What the caller decides that the source does not.
#[derive(Debug, Clone)]
pub struct ComposeOptions {
    /// What an aggregate of unspecified extensibility is taken to be.
    ///
    /// `None` leaves the facet absent, so "the source said appendable"
    /// stays distinguishable from "the source said nothing" and
    /// `Ast::effective_extensibility` applies XTypes 1.3 §7.3.1.2.1.8's
    /// default. `Some` writes it into the model, which is what that same
    /// paragraph provides for:
    ///
    /// > Implementations may provide a mechanism to override this
    /// > default behavior; for example, IDL compilers may provide
    /// > configuration options to allow users to specify whether types
    /// > of unspecified extensibility are to be considered final,
    /// > appendable, or mutable.
    ///
    /// What the source *did* say still wins. This fills silence only,
    /// which is why it is an option here and not a rewrite of the tree.
    pub default_extensibility: Option<zerodds_ast::Extensibility>,
    /// The `@default_nested` of the top-level modules that carry none of
    /// their own — the compiler option DDS-XTypes 1.3 §7.3.1.2.1.7
    /// requires ("IDL compilers shall provide the means to change the
    /// default value for non-annotated top-level modules"). `None` is the
    /// specification's FALSE. See
    /// [`zerodds_idl_lower::Options::default_nested`].
    pub default_nested: Option<bool>,
    /// Macros defined before the first line is read — a build system's
    /// `-D`. An empty body is `-DNAME`: defined, expanding to nothing.
    pub defines: Vec<(String, String)>,
    /// Whether a `@hashid` member advances the sequential id counter.
    ///
    /// A dialect axis and not a preference: vendors disagree, and the
    /// answer changes the id of every member *after* a `@hashid` one.
    /// It is here because the wire corpus varies it, and the corpus and
    /// a build have to be able to load a file the same way — two load
    /// paths with the same intent is how a production path came to skip
    /// the resolve pass entirely.
    pub hashed_id_advances_the_counter: zerodds_idl_lower::HashedIdCounter,
    /// Which spelling of an escaped name a member-id hash reads. The
    /// same kind of axis as the counter above — RTI hashes `_long` as
    /// written where the clause hashes `long` — and here for the same
    /// reason: the corpus and a build have to load a file the same way.
    pub hashed_name_spelling: zerodds_idl_lower::HashedNameSpelling,
    /// Whether `@key` on a union discriminator is carried.
    ///
    /// The same axis `zerodds-xml-compose` carries, and here for the
    /// reason the two above are here: a type declared once in IDL and
    /// once in XML has to reach the same model, and the key bit is part
    /// of its `TypeIdentifier`.
    pub key_on_union_discriminator: zerodds_idl_lower::KeyOnUnionDiscriminator,
    /// Whether a declaration may carry the name of the scope it sits in.
    ///
    /// The same kind of axis as the three above, and here for the same
    /// reason: the corpus and a build have to load a file the same way.
    /// The measurement behind it is the corpus case
    /// `resolve-module-named-as-type`, which Cyclone DDS and OpenDDS
    /// refuse and Fast DDS and RTI Connext take.
    pub name_of_the_enclosing_scope: zerodds_idl_lower::NameOfTheEnclosingScope,
    /// Which vendors' extensions this composition admits. Spec §3.5.
    ///
    /// Reaches two stages, because vendor constructs live in two: the
    /// parser sees `keylist` as grammar, the lowering sees `#pragma
    /// keylist` and OpenDDS's `DCPS_*` as directives that change the
    /// model. `Dialect::STRICT` turns both from a warning into an error.
    ///
    /// Not part of a derived `Default`: see the `impl Default` below.
    pub dialect: zerodds_idl_parse::Dialect,
}

/// `ComposeOptions::default()` admits every vendor extension.
///
/// Written out rather than derived, because `Dialect::default()` is
/// `STRICT` and the two defaults are deliberately opposite. A tool
/// pointed at a file somebody else wrote should read it before it judges
/// it; `--strict` is the caller who asked the other question.
impl Default for ComposeOptions {
    fn default() -> Self {
        Self {
            default_extensibility: None,
            default_nested: None,
            defines: Vec::new(),
            hashed_id_advances_the_counter: zerodds_idl_lower::HashedIdCounter::default(),
            hashed_name_spelling: zerodds_idl_lower::HashedNameSpelling::default(),
            key_on_union_discriminator: zerodds_idl_lower::KeyOnUnionDiscriminator::default(),
            name_of_the_enclosing_scope: zerodds_idl_lower::NameOfTheEnclosingScope::default(),
            dialect: zerodds_idl_parse::Dialect::ALL_VENDORS,
        }
    }
}

/// Re-exported because [`ComposeOptions`] carries it in a public field:
/// a caller that cannot name the type cannot construct the options, and
/// `zerodds-build` could not until this was here.
pub use zerodds_idl_lower::HashedIdCounter;
pub use zerodds_idl_lower::HashedNameSpelling;
pub use zerodds_idl_lower::KeyOnUnionDiscriminator;
pub use zerodds_idl_lower::NameOfTheEnclosingScope;

/// Preprocess, parse, lower and register, from a path.
///
/// # Errors
/// Only the read of `path` itself: everything after it is reported
/// through [`Composed::diagnostics`], because a file with one bad
/// declaration still has the rest, and a build that stops at the first
/// one costs a round trip per defect.
pub fn compose(path: &str, loader: &dyn FileLoader) -> Result<Composed, String> {
    compose_with(path, loader, ComposeOptions::default())
}

/// As [`compose`], deciding what the source leaves open.
///
/// # Errors
/// As [`compose`].
pub fn compose_with(
    path: &str,
    loader: &dyn FileLoader,
    options: ComposeOptions,
) -> Result<Composed, String> {
    let source = std::fs::read_to_string(path).map_err(|error| format!("{path}: {error}"))?;
    Ok(compose_source_with(path, &source, loader, options))
}

/// As [`compose`], over text the caller already has.
#[must_use]
pub fn compose_source(path: &str, source: &str, loader: &dyn FileLoader) -> Composed {
    compose_source_with(path, source, loader, ComposeOptions::default())
}

/// As [`compose_source`], deciding what the source leaves open.
#[must_use]
pub fn compose_source_with(
    path: &str,
    source: &str,
    loader: &dyn FileLoader,
    options: ComposeOptions,
) -> Composed {
    let preprocessed = zerodds_idl_parse::preprocess_with(path, source, loader, &options.defines);
    let parsed = zerodds_idl_parse::parse_preprocessed_with(&preprocessed, options.dialect);
    let mut diagnostics = parsed.diagnostics().to_vec();
    // The preprocessor's come first and are in file offsets already; the
    // parser's are on the expanded text; everything raised on the model
    // after them is on the tree (`Parse::source_span`).
    let expanded_from = preprocessed.diagnostics.len();
    let tree_from = diagnostics.len();

    let lowered = zerodds_idl_lower::lower_with(&parsed.syntax(), path, options.lowering());
    diagnostics.extend(lowered.diagnostics);
    let mut ast = lowered.ast;
    diagnostics.extend(resolve(&mut ast, path));
    // After resolve, because a cycle is a property of the resolved type
    // graph: an unresolved reference has no edge to close one.
    diagnostics.extend(inadmissible_cycles(&ast));
    for diagnostic in diagnostics.iter_mut().skip(tree_from) {
        on_the_expanded_text(&parsed, diagnostic);
    }
    for diagnostic in diagnostics.iter_mut().skip(expanded_from) {
        preprocessed.locate(diagnostic);
    }
    // The diagnostics above are in file offsets now; the nodes are next.
    // Lowering gave each the range of the tree it was lowered from,
    // under the unit's path, and an empty macro chain. A reader of a
    // node's source — a lint, a generator naming a declaration, a
    // snapshot — wants the file and the range a person can open, and
    // the macros that wrote the text.
    place_nodes_in_their_files(&mut ast, &parsed, &preprocessed, |_| true);

    // Type objects are built from whatever the model holds. A model with
    // errors in it still registers the types that came out whole, which
    // is what lets a caller see how far a broken file got.
    let mut registry = TypeRegistry::new();
    let without_type_objects = zerodds_types::from_ast::register_all(&ast, &mut registry)
        .map(|skipped| {
            skipped
                .into_iter()
                .map(|(name, error)| (name, format!("{error:?}")))
                .collect()
        })
        .unwrap_or_default();

    Composed {
        ast,
        registry,
        diagnostics,
        without_type_objects,
        expanded: Some(preprocessed),
    }
}

/// The source facet `any` holds now.
fn ast_source(ast: &Ast, any: zerodds_ast::AnyId) -> Option<&zerodds_ast::SourceFacet> {
    ast.facets(any).map(|facets| &facets.source)
}

/// A diagnostic raised on the tree, moved to the expanded text the
/// preprocessor's map reads ([`zerodds_idl_parse::Parse::source_span`]).
fn on_the_expanded_text(parsed: &zerodds_idl_parse::Parse, diagnostic: &mut Diagnostic) {
    diagnostic.primary = parsed.source_span(diagnostic.primary);
    for (span, _) in &mut diagnostic.labels {
        *span = parsed.source_span(*span);
    }
}

/// The `SourceFacet` of every node `which` selects moved from the tree
/// to the file it was written in: the tree's range as a range of the
/// expanded text ([`zerodds_idl_parse::Parse::source_span`]), and that
/// as the file, the range in it and the macros that wrote it
/// ([`zerodds_idl_parse::Preprocessed::provenance`]). A node with no
/// source range, or one before the first token, keeps what it has.
///
/// Every placement is computed before the first is written, so a node
/// two handles reach is placed once from the range lowering gave it and
/// not again from the range the first placement left.
fn place_nodes_in_their_files(
    ast: &mut Ast,
    parsed: &zerodds_idl_parse::Parse,
    preprocessed: &zerodds_idl_parse::Preprocessed,
    which: impl Fn(zerodds_ast::AnyId) -> bool,
) {
    let mut placements = Vec::new();
    for any in ast.all_nodes().filter(|any| which(*any)) {
        let Some(facets) = ast.facets(any) else {
            continue;
        };
        let Some(span) = facets.source.span else {
            continue;
        };
        if !matches!(facets.source.origin, zerodds_ast::SourceOrigin::File { .. }) {
            continue;
        }
        let expanded = parsed.source_span(zerodds_idl_cst::Span::new(span.start, span.end));
        if let Some(placed) = preprocessed.provenance(expanded) {
            placements.push((any, placed));
        }
    }
    for (any, placed) in placements {
        let Some(facets) = ast.facets_mut(any) else {
            continue;
        };
        facets.source.origin = zerodds_ast::SourceOrigin::File { path: placed.path };
        facets.source.span = Some(zerodds_ast::ByteSpan {
            start: placed.span.start,
            end: placed.span.end,
        });
        facets.source.macro_chain = placed
            .macro_chain
            .into_iter()
            .map(|used| zerodds_ast::MacroExpansion {
                name: used.name,
                invoked_at: Some(zerodds_ast::ByteSpan {
                    start: used.at.start,
                    end: used.at.end,
                }),
                path: used.path,
            })
            .collect();
    }
}

impl ComposeOptions {
    /// The lowering options these map to.
    #[must_use]
    pub fn lowering(&self) -> zerodds_idl_lower::Options {
        zerodds_idl_lower::Options {
            default_extensibility: self.default_extensibility,
            default_nested: self.default_nested,
            hashed_id_advances_the_counter: self.hashed_id_advances_the_counter,
            hashed_name_spelling: self.hashed_name_spelling,
            key_on_union_discriminator: self.key_on_union_discriminator,
            name_of_the_enclosing_scope: self.name_of_the_enclosing_scope,
            dialect: self.dialect,
        }
    }
}

/// The recursion IDL 4.2 §7.4.1.4.4.4.4 does not admit.
///
/// > A type may only contain itself through a sequence, a map, or an
/// > `@optional` member — something on the cycle at which a *value* can
/// > stop.
///
/// The rule already existed and already cited the clause: the layout
/// stage raises `LayoutError::RecursionOutsideCollection`. It sat one
/// stage past the front end, so `zerodds-idlc check` reported nothing
/// for `struct Sa { Sa m; };` and only `generate` refused it — somebody
/// using `check` to ask whether a file is valid IDL did not find out.
///
/// It runs here rather than in the CLI so that every caller of the front
/// end gets the same answer: `check`, a `build.rs` through
/// `zerodds-build`, and a wire probe all compose the same way, and a
/// rule that only one of them applies is a rule that surprises the other
/// two.
///
/// **Not a second implementation.**
/// [`zerodds_ast_layout::types_that_close_an_inadmissible_cycle`] runs
/// the planner's own walk and keeps only that one error. The barrier
/// bookkeeping — which of sequence, map or `@optional` lies on the cycle
/// — lives in the descent, and a rule written twice drifts.
fn inadmissible_cycles(ast: &Ast) -> Vec<Diagnostic> {
    // The plain XCDR2 data key. The answer does not depend on it: every
    // arm that raises this error is about the shape of the type graph
    // and not about an encoding. A key is required only because `plan`
    // takes one.
    let key = zerodds_ast_layout::LayoutKey::data(
        zerodds_ast_layout::Encoding::PlainCdr2,
        zerodds_ast_layout::Endianness::Little,
    );
    let closing = zerodds_ast_layout::types_that_close_an_inadmissible_cycle(ast, key);
    // An anonymous type closes the same cycle as the declaration that
    // contains it, and the walk returns both: `Node pair[2]` is an
    // `Array { element: Node }` node with no name and no source position,
    // so `recursive-array` produced two errors, the second reading
    // `` `::` closes a type cycle `` at offset 0. One cycle is one error,
    // and the one worth reporting is the one a reader can find in the file.
    //
    // Filtered on the source span rather than the name: a scoped name for
    // an anonymous type is not absent, it is empty, and a rule that reads
    // an empty string as "anonymous" also swallows a declaration whose
    // name failed to resolve.
    //
    // Kept whole when *no* entry carries a span. A cycle reported without
    // a position is still a cycle, and reporting none of them would be
    // exactly the silence this rule exists to break.
    let declared: Vec<_> = closing
        .iter()
        .copied()
        .filter(|(id, _)| {
            ast.type_(*id)
                .is_ok_and(|node| node.facets.source.span.is_some())
        })
        .collect();
    let reported = if declared.is_empty() {
        closing
    } else {
        declared
    };
    reported
        .into_iter()
        .map(|(id, node_id)| {
            let name = ast
                .scoped_name_of(zerodds_ast::AnyId::Type(id, node_id))
                .map_or_else(|| String::from("a type"), |scoped| scoped.to_string());
            let span = ast
                .type_(id)
                .ok()
                .and_then(|node| node.facets.source.span)
                .map_or_else(
                    || zerodds_idl_cst::Span::new(0, 0),
                    |span| zerodds_idl_cst::Span::new(span.start, span.end),
                );
            Diagnostic::error(
                zerodds_idl_parse::DiagnosticCode::InvalidBound,
                span,
                format!(
                    "`{name}` closes a type cycle with nothing on it that terminates a value \
                     — no collection, @optional member or productive @external indirection — \
                     which OMG IDL 4.2 §7.4.1.4.4.4.4 and XTypes recursion rules do not admit"
                ),
                "OMG IDL 4.2 §7.4.1.4.4.4.4",
            )
        })
        .collect()
}

/// Resolve every pending name, and report each one that does not.
///
/// Lowering leaves a name pending rather than failing on it — a forward
/// declaration depends on that, and so does a name declared later in the
/// same unit. Something has to run the pass afterwards, and until now
/// nothing in a production path did: `Ast::resolve_all_by_name` was
/// called only from tests and from one build script that had found this
/// out for itself.
///
/// What that cost is not subtle. A member whose type lives in another
/// module stayed unresolved, the generator emitted the type without it,
/// and the sample went onto the wire missing a field — reported as a
/// deserialisation error by the peer, which is where it first became
/// visible. `module common { module util { struct Location …; }; struct
/// Robot { ::common::util::Location location; }; };` is the shape, and
/// it is ordinary IDL.
///
/// A name that cannot be resolved is an **error** here. It used to be
/// nothing at all.
fn resolve(ast: &mut Ast, path: &str) -> Vec<Diagnostic> {
    let Ok(report) = ast.resolve_all_by_name() else {
        return vec![Diagnostic {
            severity: Severity::Error,
            code: zerodds_idl_parse::DiagnosticCode::UnresolvedName,
            message: format!("{path}: the model could not run a resolve pass"),
            primary: zerodds_idl_cst::Span::new(0, 0),
            labels: Vec::new(),
            file: None,
            spec_reference: "OMG IDL 4.2 §7.5",
        }];
    };
    report
        .broken
        .iter()
        .map(|(node, cause)| {
            // Where the node that names it was written: the range its
            // `SourceFacet` holds, and its file. In a unit that is the
            // expanded text still, which the caller then places; in a
            // bundle every unit's nodes are in their files already.
            let facets = ast.resolve_node_id(*node).and_then(|any| ast.facets(any));
            let primary = facets
                .and_then(|f| f.source.span)
                .map_or(zerodds_idl_cst::Span::new(0, 0), |span| {
                    zerodds_idl_cst::Span::new(span.start, span.end)
                });
            let file = facets.and_then(|f| match &f.source.origin {
                zerodds_ast::SourceOrigin::File { path } => Some(path.clone()),
                _ => None,
            });
            Diagnostic {
                severity: Severity::Error,
                code: zerodds_idl_parse::DiagnosticCode::UnresolvedName,
                message: format!("a name does not resolve: {cause:?} (node {node:?})"),
                primary,
                labels: Vec::new(),
                file,
                spec_reference: "OMG IDL 4.2 §7.5",
            }
        })
        .collect()
}

/// Compose several units into **one** model.
///
/// The units are lowered into a shared `Ast`, so a module two of them
/// declare is **one node** — and everything downstream sees one. Two
/// separate compositions produce two models that know nothing of each
/// other, and a generator run over each of them emits `pub mod common`
/// twice; two `include!` calls in one Rust scope then collide, which is
/// what issue #39 reported.
///
/// This is the model-level answer and not a textual one. Writing a
/// synthetic unit full of `#include` lines reaches the same output for
/// IDL and buys nothing anywhere else: the `Ast` is the common model for
/// IDL, DDS-XML, ROS and a `TypeObject` off the wire, and only a source
/// that has an include directive could take part. Node identity is what
/// merges them here, so a module declared by three sources is one node
/// whatever those sources were.
///
/// Each unit keeps its own diagnostics, in the order given.
///
/// # Errors
/// The read of any unit, naming it.
pub fn compose_all(
    paths: &[impl AsRef<std::path::Path>],
    loader: &dyn FileLoader,
    options: ComposeOptions,
) -> Result<Composed, String> {
    let mut sources = Vec::with_capacity(paths.len());
    for path in paths {
        let path = path.as_ref();
        let name = path.to_string_lossy().into_owned();
        let text = std::fs::read_to_string(path).map_err(|error| format!("{name}: {error}"))?;
        sources.push((name, text));
    }
    Ok(compose_all_sources(&sources, loader, options))
}

/// As [`compose_all`], over text the caller already has.
///
/// Each entry is `(name, source)`; the name is what a diagnostic points
/// at.
#[must_use]
pub fn compose_all_sources(
    sources: &[(String, String)],
    loader: &dyn FileLoader,
    options: ComposeOptions,
) -> Composed {
    let mut ast = Ast::new();
    let mut diagnostics = Vec::new();
    for (name, source) in sources {
        let preprocessed =
            zerodds_idl_parse::preprocess_with(name, source, loader, &options.defines);
        let parsed = zerodds_idl_parse::parse_preprocessed(&preprocessed);
        let mut unit = parsed.diagnostics().to_vec();
        let expanded_from = preprocessed.diagnostics.len();
        let tree_from = unit.len();
        // What every node held before this unit: a node the unit adds is
        // new, and one it completes — a forward declaration an earlier
        // unit made — holds a range of this unit's tree afterwards.
        let before: std::collections::HashMap<zerodds_ast::AnyId, zerodds_ast::SourceFacet> = ast
            .all_nodes()
            .filter_map(|any| ast.facets(any).map(|facets| (any, facets.source.clone())))
            .collect();
        unit.extend(zerodds_idl_lower::lower_into_with(
            &mut ast,
            &parsed.syntax(),
            name,
            options.lowering(),
        ));
        // This unit's nodes into its files, while its map is in hand: the
        // nodes of the next unit index another tree.
        let lowered_here = |any: zerodds_ast::AnyId| match before.get(&any) {
            None => true,
            Some(source) => ast_source(&ast, any).is_some_and(|now| now != source),
        };
        let here: std::collections::HashSet<zerodds_ast::AnyId> =
            ast.all_nodes().filter(|any| lowered_here(*any)).collect();
        place_nodes_in_their_files(&mut ast, &parsed, &preprocessed, |any| here.contains(&any));
        // Each unit's own diagnostics are placed in its own files and
        // name the unit, since a bundle has no one file to be "the" unit.
        for (index, diagnostic) in unit.iter_mut().enumerate() {
            if index >= tree_from {
                on_the_expanded_text(&parsed, diagnostic);
            }
            if index >= expanded_from {
                preprocessed.locate(diagnostic);
            }
            if diagnostic.file.is_none() {
                diagnostic.file = Some(name.clone());
            }
        }
        diagnostics.extend(unit);
    }
    // One pass over the merged model, after every unit is in it. A name
    // that reaches across units resolves only here.
    let path = sources
        .first()
        .map_or_else(|| String::from("<bundle>"), |(name, _)| name.clone());
    diagnostics.extend(resolve(&mut ast, &path));

    let mut registry = TypeRegistry::new();
    let without_type_objects = zerodds_types::from_ast::register_all(&ast, &mut registry)
        .map(|skipped| {
            skipped
                .into_iter()
                .map(|(name, error)| (name, format!("{error:?}")))
                .collect()
        })
        .unwrap_or_default();

    Composed {
        ast,
        registry,
        diagnostics,
        without_type_objects,
        expanded: None,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
    use super::*;
    use zerodds_idl_parse::MemoryLoader;

    #[test]
    fn a_clean_unit_composes_and_registers_its_types() {
        let composed = compose_source(
            "test.idl",
            "module m { @appendable struct S { @key long id; }; };",
            &MemoryLoader::new(),
        );
        assert!(!composed.failed(), "{:?}", composed.diagnostics);
        assert!(
            composed.ast.types().any(|(_, node)| node.name == "S"),
            "the type is in the model"
        );
        assert!(
            composed.without_type_objects.is_empty(),
            "{:?}",
            composed.without_type_objects
        );
    }

    /// A file with one bad declaration keeps the rest. A pipeline that
    /// stopped at the first error would cost one build per defect.
    #[test]
    fn a_broken_declaration_does_not_discard_the_good_ones() {
        let composed = compose_source(
            "test.idl",
            "module m { struct Good { long a; }; struct Bad { long ; }; };",
            &MemoryLoader::new(),
        );
        assert!(composed.failed(), "the bad one is reported");
        assert!(
            composed.ast.types().any(|(_, node)| node.name == "Good"),
            "the good one survived"
        );
    }

    /// A quoted include resolves beside the including file before the
    /// search list. Two files of the same name, one next to the source
    /// and one on the search path, must not be interchangeable.
    #[test]
    fn a_quoted_include_prefers_the_including_files_directory() {
        let dir = std::env::temp_dir().join(format!("idl-compose-{}", std::process::id()));
        let beside = dir.join("beside");
        let elsewhere = dir.join("elsewhere");
        std::fs::create_dir_all(&beside).expect("beside");
        std::fs::create_dir_all(&elsewhere).expect("elsewhere");
        std::fs::write(beside.join("common.idl"), "struct Near { long a; };").expect("near");
        std::fs::write(elsewhere.join("common.idl"), "struct Far { long a; };").expect("far");
        let main = beside.join("main.idl");
        std::fs::write(&main, "#include \"common.idl\"\nstruct Main { long b; };").expect("main");

        let loader = PathLoader::new(vec![elsewhere.clone()]);
        let composed = compose(main.to_str().expect("utf8"), &loader).expect("read");
        assert!(
            composed.ast.types().any(|(_, node)| node.name == "Near"),
            "the include beside the file wins"
        );
        assert!(
            !composed.ast.types().any(|(_, node)| node.name == "Far"),
            "the one on the search path must not"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The option fills silence and does not overrule the source. This
    /// is the distinction the whole option rests on: a file that says
    /// `@final` still says it.
    #[test]
    fn the_default_only_reaches_types_the_source_left_open() {
        use zerodds_ast::Extensibility;
        let source = "module m { @final struct Said { long a; }; struct Silent { long a; }; };";
        let composed = compose_source_with(
            "test.idl",
            source,
            &MemoryLoader::new(),
            ComposeOptions {
                default_extensibility: Some(Extensibility::Mutable),
                ..ComposeOptions::default()
            },
        );
        let of = |name: &str| {
            let (id, _) = composed
                .ast
                .types()
                .find(|(_, node)| node.name == name)
                .expect("the type");
            composed.ast.effective_extensibility(id)
        };
        assert_eq!(of("Said"), Extensibility::Final, "the source said final");
        assert_eq!(of("Silent"), Extensibility::Mutable, "the option filled it");
    }

    /// Without the option, silence stays silence and the specification's
    /// own default applies.
    #[test]
    fn without_the_option_the_specification_default_applies() {
        use zerodds_ast::Extensibility;
        let composed = compose_source(
            "test.idl",
            "module m { struct Silent { long a; }; };",
            &MemoryLoader::new(),
        );
        let (id, _) = composed
            .ast
            .types()
            .find(|(_, node)| node.name == "Silent")
            .expect("the type");
        assert_eq!(
            composed.ast.effective_extensibility(id),
            Extensibility::Appendable,
            "XTypes 1.3 §7.3.1.2.1.8"
        );
    }

    /// A define reaches the unit before its first line, so a guard at
    /// the top sees it.
    #[test]
    fn a_define_is_in_place_before_the_first_line() {
        let composed = compose_source_with(
            "test.idl",
            "#ifdef WITH_EXTRA\nstruct Extra { long a; };\n#endif\nstruct Always { long b; };",
            &MemoryLoader::new(),
            ComposeOptions {
                defines: vec![(String::from("WITH_EXTRA"), String::new())],
                ..ComposeOptions::default()
            },
        );
        assert!(
            composed.ast.types().any(|(_, node)| node.name == "Extra"),
            "the guarded declaration is in"
        );

        let without = compose_source(
            "test.idl",
            "#ifdef WITH_EXTRA\nstruct Extra { long a; };\n#endif\nstruct Always { long b; };",
            &MemoryLoader::new(),
        );
        assert!(
            !without.ast.types().any(|(_, node)| node.name == "Extra"),
            "and out without the define"
        );
    }

    /// The loader records what it opened. A build that is not told about
    /// an include does not re-run when that include changes, and nothing
    /// reports the stale output.
    #[test]
    fn the_loader_records_every_file_it_opened() {
        let dir = std::env::temp_dir().join(format!("idl-compose-deps-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(dir.join("common.idl"), "struct Shared { long a; };").expect("common");
        let main = dir.join("main.idl");
        std::fs::write(&main, "#include \"common.idl\"\nstruct Main { long b; };").expect("main");

        let loader = PathLoader::bare();
        let _ = compose(main.to_str().expect("utf8"), &loader).expect("read");
        let opened = loader.opened();
        assert!(
            opened.iter().any(|p| p.ends_with("common.idl")),
            "the include is in the list: {opened:?}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_file_is_an_error_and_names_itself() {
        let Err(error) = compose("/nonexistent/nope.idl", &PathLoader::bare()) else {
            panic!("a path that does not exist must not compose")
        };
        assert!(error.contains("nope.idl"), "{error}");
    }
}
