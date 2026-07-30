// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Multi-input ("project") composition: deduplicating a top-level definition
//! that several IDL inputs pull in through a **shared** `#include`.
//!
//! [`compose`](crate::compose) runs on one file at a time and, because it
//! expands `#include` textually, every input that does `#include "common.idl"`
//! ends up with a full copy of `module common` in its AST — correct for a
//! single file, but when a build system emits one output per input and a user
//! then `include!`s them **flat into one namespace**, each output redeclares
//! `pub mod common` and Rust rejects the second with E0428 ("defined multiple
//! times"); the same duplication shows up as a duplicate class / symbol in the
//! other backends. The existing regression test for GH #25 uses deliberately
//! *disjoint* IDLs (`module common` + `module metrics`) and so never exercises
//! a shared include.
//!
//! [`compose_project`] composes a set of inputs together and works out, per
//! input, which top-level modules are redundant copies of a definition an
//! earlier input in the set already emits, so the caller can strip them and
//! leave exactly one emission of the shared module across the whole project.
//! Cross-module references keep resolving, because the *owning* output still
//! carries the module and the stripped outputs keep their generated
//! `super::common::Shared`-style paths, which resolve to that single sibling
//! once the outputs are `include!`d together.
//!
//! # Provenance, not just the name
//! The deduplication key is provenance (the origin file each definition was
//! read from, [`ComposeOutput::definition_files`]) together with the
//! definition's structural fingerprint — not the bare name. Two `module
//! common` blocks are collapsed only when they are the *same* shared
//! definition (identical body); two different `module common` bodies that
//! merely share a name are a genuine contradiction and raise
//! [`ComposeError::Conflict`] instead of letting one silently win.

use std::path::Path;

use zerodds_idl::ast::{Definition, Specification};
use zerodds_idl::errors::Span;

use crate::compose::{ComposeError, ComposeOptions, ComposeOutput, compose};

/// One input's contribution to a [`compose_project`] result.
pub struct ProjectComposition {
    /// The per-input composition (full AST — cross-module references still
    /// resolve against it — and `type_objects` already filtered so a shared
    /// type's TypeObject is not re-emitted by a non-owning input).
    pub output: ComposeOutput,
    /// Names of top-level modules in `output.ast` that a preceding input in
    /// the project already emits through the same shared `#include`. The
    /// caller strips these from this input's generated source (for Rust, via
    /// [`strip_top_level_modules_rust`]); the owning input keeps them, so the
    /// project as a whole emits each shared module exactly once. Empty for the
    /// owning input and for a single-file project.
    pub shared_modules: Vec<String>,
}

/// Structural, span-independent fingerprint of a single top-level definition.
///
/// The AST carries byte-offset [`Span`]s that differ between two inputs even
/// for a byte-identical shared include (each input expands it at a different
/// offset), so a raw `PartialEq` would report a false difference. Rendering
/// the definition back to canonical IDL text drops every span, giving a stable
/// key that is equal exactly when the two definitions have the same body.
fn fingerprint(def: &Definition) -> String {
    // `Specification`'s `Display` prints each definition span-independently;
    // wrap the single definition to reuse it.
    let spec = Specification {
        definitions: vec![def.clone()],
        span: Span::SYNTHETIC,
    };
    spec.to_string()
}

/// The top-level module name for `def`, or `None` if `def` is not a module.
///
/// Only modules are deduplicated: a shared `#include` in DDS IDL practice wraps
/// its types in a `module`, and a module is emitted as a single, cleanly
/// delimited top-level block in every backend (`pub mod <name> { … }` in Rust),
/// which is what a stripping pass can remove without disturbing the sibling
/// references into it.
fn module_name(def: &Definition) -> Option<&str> {
    match def {
        Definition::Module(m) => Some(m.name.text.as_str()),
        _ => None,
    }
}

/// Canonical form of a path string for provenance comparison; falls back to the
/// input unchanged when the file cannot be canonicalised (e.g. it no longer
/// exists), so a comparison never silently turns two distinct paths equal.
fn canonical(path: &str) -> String {
    std::fs::canonicalize(path)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string())
}

/// Composes several IDL inputs as one project and deduplicates any top-level
/// module that more than one input pulls in through a **shared** `#include`.
///
/// Returns one [`ProjectComposition`] per input, in the input order. The first
/// input (in order) that contributes a given shared module *owns* it; every
/// later input that pulls in the same module through the same include reports
/// it in [`ProjectComposition::shared_modules`] so the caller strips it,
/// leaving one emission across the project.
///
/// A module that an input defines itself (its provenance is that input's own
/// file, not an `#include`) is never treated as shared and never stripped.
///
/// # Errors
/// - Any per-input [`compose`] error (I/O, preprocessing, parse, semantic).
/// - [`ComposeError::Conflict`] when two inputs contribute a top-level module
///   with the same name but a different body — a contradiction that cannot be
///   reduced to a single emission.
pub fn compose_project(
    paths: &[impl AsRef<Path>],
    opts: &ComposeOptions,
) -> Result<Vec<ProjectComposition>, ComposeError> {
    // `claimed[name] = (fingerprint, owning_input_path)` for every shared
    // module already emitted by an earlier input.
    let mut claimed: std::collections::HashMap<String, (String, String)> =
        std::collections::HashMap::new();
    let mut result = Vec::with_capacity(paths.len());

    for path in paths {
        let path = path.as_ref();
        let main = canonical(&path.to_string_lossy());
        let mut output = compose(path, opts)?;
        let mut shared_modules = Vec::new();

        for (idx, def) in output.ast.definitions.iter().enumerate() {
            let Some(name) = module_name(def) else {
                continue;
            };
            // Only a module pulled in through an `#include` (provenance differs
            // from this input's own file) can be a shared duplicate.
            let origin = canonical(&output.definition_files[idx]);
            if origin == main {
                continue;
            }
            let fp = fingerprint(def);
            match claimed.get(name) {
                None => {
                    // First input to contribute this shared module owns it.
                    claimed.insert(name.to_string(), (fp, main.clone()));
                }
                Some((prev_fp, owner)) if *prev_fp == fp => {
                    // Identical shared definition → this input's copy is
                    // redundant; the owner already emits it.
                    let _ = owner;
                    shared_modules.push(name.to_string());
                }
                Some((_, owner)) => {
                    return Err(ComposeError::Conflict(format!(
                        "project composition: top-level module `{name}` has conflicting \
                         definitions — `{owner}` and `{}` each define it with a different \
                         body through a shared #include; refusing to let one silently win",
                        path.display()
                    )));
                }
            }
        }

        // Drop the TypeObjects that belong to a module this input no longer
        // emits, so the shared type's TypeObject is serialised once (by the
        // owning input) instead of once per input.
        if !shared_modules.is_empty() {
            output.type_objects.retain(|blob| {
                !shared_modules
                    .iter()
                    .any(|m| blob.fqn == *m || blob.fqn.starts_with(&format!("{m}::")))
            });
        }

        result.push(ProjectComposition {
            output,
            shared_modules,
        });
    }

    Ok(result)
}

/// Removes the named top-level `pub mod <name> { … }` blocks from generated
/// Rust source produced by `zerodds-idl-rust`.
///
/// The emitter writes every top-level module as a `pub mod <name> {` line at
/// column zero, closed by a `}` at column zero; every brace in between is
/// indented. This lets a shared module be removed by dropping the lines from
/// its opening line to that column-zero closing brace, without a full Rust
/// parser. Names not present are ignored. Intended for the non-owning outputs
/// of [`compose_project`], whose `super::`-anchored references then resolve to
/// the single copy the owning output still emits.
#[must_use]
pub fn strip_top_level_modules_rust(code: &str, module_names: &[String]) -> String {
    if module_names.is_empty() {
        return code.to_string();
    }
    let openers: Vec<String> = module_names
        .iter()
        .map(|name| format!("pub mod {name} {{"))
        .collect();

    let mut out = String::with_capacity(code.len());
    let mut skipping = false;
    for line in code.lines() {
        if skipping {
            // The owning module's closing brace is the first column-zero `}`.
            if line == "}" {
                skipping = false;
            }
            continue;
        }
        if openers.iter().any(|opener| line == opener) {
            skipping = true;
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}
