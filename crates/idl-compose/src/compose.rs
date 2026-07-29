// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The shared composition pipeline: everything `zerodds-idlc generate` does
//! to an IDL file *before* handing it to a `zerodds-idl-*` language
//! emitter — preprocess (`#include`/`#define`/`#ifdef`/`#pragma`) -> parse
//! -> apply vendor key-pragmas (`@key`) -> apply default-extensibility /
//! default-nested -> lower XTypes TypeObjects.
//!
//! Every in-repo `build.rs` that called `zerodds_idl::parse` +
//! `zerodds_idl_rust::generate_rust_module` directly skipped all of this —
//! no `#include` tracking, no vendor key-pragma support, no
//! default-extensibility, no TypeObject block — so its output was not
//! feature-equivalent to the CLI's. This module is the single place that
//! composition logic lives; both `zerodds-idlc` and `zerodds-build` call it.

use std::path::{Path, PathBuf};

use zerodds_idl::ast::Specification;
use zerodds_idl::config::ParserConfig;
use zerodds_idl::parser::parse;
use zerodds_idl::preprocessor::{FileId, Preprocessor};

use crate::default_ext::{self, DefaultExt};
use crate::key_pragmas;
use crate::resolver::FsResolver;
use crate::typeobject::{self, TypeObjectBlob};

/// Options for [`compose`]. `Default` matches the CLI's own defaults
/// (compile-time `ext-default-*` feature, TypeObject emission on).
#[derive(Debug, Clone)]
pub struct ComposeOptions {
    /// `-I` search path for `#include` (checked in order, after the
    /// requesting file's own directory for quoted includes).
    pub include_dirs: Vec<PathBuf>,
    /// `-D NAME[=VALUE]` preprocessor defines, prepended as a synthetic
    /// `#define` prelude (gcc convention: `-D NAME` without a value means
    /// `1`).
    pub defines: Vec<String>,
    /// Default extensibility for un-annotated struct/union/enum
    /// (XTypes 1.3 SS7.3.3.1). `None` uses [`DefaultExt::cfg_default`] —
    /// the compile-time `ext-default-*` feature, same as the CLI without
    /// `--default-extensibility`.
    pub default_extensibility: Option<DefaultExt>,
    /// Default `@nested` for un-annotated types without `@topic`. `None`/
    /// `Some(false)` is a no-op (the OMG default).
    pub default_nested: Option<bool>,
    /// Lower + serialise XTypes TypeObjects (default-on, matching the CLI;
    /// `--no-typeobject` there maps to `false` here).
    pub emit_typeobject: bool,
}

impl Default for ComposeOptions {
    fn default() -> Self {
        Self {
            include_dirs: Vec::new(),
            defines: Vec::new(),
            default_extensibility: None,
            default_nested: None,
            emit_typeobject: true,
        }
    }
}

/// Result of [`compose`].
pub struct ComposeOutput {
    /// The parsed + patched AST, ready for a `zerodds-idl-*` emitter.
    pub ast: Specification,
    /// XTypes TypeObjects, one per named type, topologically ordered.
    /// Empty when `emit_typeobject` was `false` or lowering failed for a
    /// construct the mapper does not yet support (a warning-worthy but
    /// non-fatal condition, matching the CLI).
    pub type_objects: Vec<TypeObjectBlob>,
    /// Every source file that participated: the main IDL file first, then
    /// every transitively `#include`d file, in the order the preprocessor
    /// first touched them. Feed these to `cargo:rerun-if-changed` (a
    /// build.rs) or a Makefile dependency line (`--dump-deps`-equivalent).
    pub dependency_files: Vec<String>,
    /// Non-fatal warning produced while lowering TypeObjects (e.g. a
    /// recursive type the mapper cannot yet handle). `None` on full
    /// success or when `emit_typeobject` was `false`.
    pub type_object_warning: Option<String>,
}

/// Composition error. Mirrors the CLI's own error surface so a caller can
/// render the same message shape.
#[derive(Debug)]
pub enum ComposeError {
    /// The main IDL file could not be read.
    Io(String),
    /// `#include`/`#define`/`#ifdef`/`#pragma` expansion failed.
    Preprocess(String),
    /// The expanded source failed to parse as OMG IDL 4.2.
    Parse(String),
}

impl core::fmt::Display for ComposeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Io(m) | Self::Preprocess(m) | Self::Parse(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for ComposeError {}

/// Runs the full CLI composition pipeline on a single IDL file: read ->
/// preprocess -> parse -> vendor key-pragmas -> default-extensibility /
/// default-nested -> TypeObject lowering.
///
/// # Errors
/// See [`ComposeError`].
pub fn compose(path: &Path, opts: &ComposeOptions) -> Result<ComposeOutput, ComposeError> {
    let path_str = path.to_string_lossy();
    let raw = std::fs::read_to_string(path)
        .map_err(|e| ComposeError::Io(format!("cannot read {path_str}: {e}")))?;

    let mut prelude = String::new();
    for def in &opts.defines {
        match def.split_once('=') {
            Some((name, value)) => prelude.push_str(&format!("#define {name} {value}\n")),
            None => prelude.push_str(&format!("#define {def} 1\n")),
        }
    }
    let pp_input = format!("{prelude}{raw}");
    let resolver = FsResolver::new(opts.include_dirs.clone());
    let processed = Preprocessor::new(resolver)
        .process(&path_str, &pp_input)
        .map_err(|e| ComposeError::Preprocess(format!("preprocessing failed: {e:?}")))?;

    let dependency_files: Vec<String> = {
        let map = &processed.source_map;
        (0..map.file_count())
            .filter_map(|i| {
                map.file_name(FileId(u32::try_from(i).unwrap_or(0)))
                    .map(str::to_string)
            })
            .collect()
    };

    let cfg = ParserConfig::default();
    let mut ast =
        parse(&processed.expanded, &cfg).map_err(|e| ComposeError::Parse(format!("{e}")))?;

    let key_map =
        key_pragmas::collect_key_pragmas(&processed.pragma_keylists, &processed.opensplice_pragmas);
    key_pragmas::apply_key_pragmas(&mut ast, &key_map);

    let ext = opts
        .default_extensibility
        .unwrap_or_else(DefaultExt::cfg_default);
    default_ext::apply_default_extensibility(&mut ast, ext);
    if let Some(nested) = opts.default_nested {
        default_ext::apply_default_nested(&mut ast, nested);
    }

    let (type_objects, type_object_warning) = if opts.emit_typeobject {
        match typeobject::type_object_blobs(&ast) {
            Ok(blobs) => (blobs, None),
            Err(e) => (
                Vec::new(),
                Some(format!("TypeObject emission skipped — {e}")),
            ),
        }
    } else {
        (Vec::new(), None)
    };

    Ok(ComposeOutput {
        ast,
        type_objects,
        dependency_files,
        type_object_warning,
    })
}
