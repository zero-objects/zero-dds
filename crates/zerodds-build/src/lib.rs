// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-build`. Safety classification: **STANDARD**.
//!
//! Build-time tool (a `[build-dependencies]` crate) — `forbid(unsafe_code)`,
//! std-only, no no_std use case (same classification as the
//! `zerodds-idl-*` codegen crates it wraps).
//!
//! `prost-build`-style `build.rs` helper for ZeroDDS IDL.
//!
//! ```no_run
//! fn main() -> Result<(), zerodds_build::Error> {
//!     zerodds_build::Config::new()
//!         .include_dir("idl")
//!         .compile(&["idl/Robot.idl"])
//! }
//! ```
//!
//! `Config::compile` runs the exact same composition pipeline
//! `zerodds-idlc generate --rust` runs — preprocess (`#include`/`#define`/
//! `#ifdef`/`#pragma`), vendor key-pragma (`@key`) application,
//! default-extensibility / default-nested patching, XTypes TypeObject
//! lowering, then the `zerodds-idl-rust` emitter — via the shared
//! [`zerodds_idl_compose`] crate. Before this crate existed, the five
//! in-repo `build.rs` scripts that generated Rust from IDL called
//! `zerodds_idl::parse` + `zerodds_idl_rust::generate_rust_module`
//! directly: no `#include` dependency tracking (so editing an `#include`d
//! file did not trigger a rebuild), no vendor key-pragma support, no
//! default-extensibility, no TypeObject block. Their generated types were
//! not feature-equivalent to what `zerodds-idlc` would have produced for
//! the same IDL. `zerodds-build` closes that gap for Rust `build.rs`
//! consumers.
//!
//! Every `.idl` file passed to [`Config::compile`] and every file it
//! transitively `#include`s is reported to Cargo via
//! `cargo:rerun-if-changed`, so `cargo build` only re-runs codegen when a
//! source actually changed.

#![forbid(unsafe_code)]
#![allow(clippy::print_stdout)] // Cargo's build-script protocol IS stdout (`cargo:rerun-if-changed=...`).

use std::env;
use std::path::{Path, PathBuf};

pub use zerodds_idl_compose::DefaultExt;
use zerodds_idl_compose::{ComposeOptions, compose};
use zerodds_idl_rust::{RustGenOptions, generate_rust_module};

/// Everything that can go wrong compiling one `.idl` file.
#[derive(Debug)]
pub enum Error {
    /// `OUT_DIR` was neither set via [`Config::out_dir`] nor present in
    /// the environment (i.e. not run from a `build.rs`).
    NoOutDir,
    /// The input path has no usable file stem (e.g. it is a directory or
    /// ends in `/..`).
    NoFileStem(PathBuf),
    /// Two input `.idl` files share the same file stem and would both be
    /// written to `<out_dir>/<stem>.rs`, so the second would silently
    /// clobber the first. Rejected loudly — rename one input or split the
    /// batch across separate `out_dir`s.
    DuplicateStem {
        /// The shared file stem (the `<stem>.rs` both inputs map to).
        stem: String,
        /// The input seen first.
        first: PathBuf,
        /// The colliding input seen later.
        second: PathBuf,
    },
    /// Preprocessing/parsing/key-pragma/default-extensibility composition
    /// failed for `path`. See [`zerodds_idl_compose::ComposeError`] for
    /// the wrapped cause.
    Compose {
        /// The `.idl` file that failed to compose.
        path: PathBuf,
        /// The underlying composition error.
        source: zerodds_idl_compose::ComposeError,
    },
    /// The `zerodds-idl-rust` emitter rejected the composed AST for
    /// `path` (an IDL construct outside the DDS-DataType scope — Fixed,
    /// Map, Any, Bitset, Bitmask, Valuetype, Interface, Component, Home).
    Codegen {
        /// The `.idl` file whose AST failed to emit.
        path: PathBuf,
        /// The underlying codegen error.
        source: zerodds_idl_rust::RustGenError,
    },
    /// Writing the generated `<stem>.rs` (or creating `out_dir`) failed.
    Io {
        /// The path that could not be written / created.
        path: PathBuf,
        /// The underlying I/O error.
        source: std::io::Error,
    },
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NoOutDir => f.write_str(
                "zerodds-build: no output directory — call Config::out_dir() \
                 or run from a build.rs (OUT_DIR set by Cargo)",
            ),
            Self::NoFileStem(p) => {
                write!(
                    f,
                    "zerodds-build: cannot derive a file stem from {}",
                    p.display()
                )
            }
            Self::DuplicateStem {
                stem,
                first,
                second,
            } => {
                write!(
                    f,
                    "zerodds-build: inputs {} and {} both map to '{stem}.rs' \
                     — the second would silently overwrite the first; \
                     rename one input or compile them into separate out_dirs",
                    first.display(),
                    second.display()
                )
            }
            Self::Compose { path, source } => {
                write!(f, "zerodds-build: composing {}: {source}", path.display())
            }
            Self::Codegen { path, source } => {
                write!(
                    f,
                    "zerodds-build: generating Rust for {}: {source}",
                    path.display()
                )
            }
            Self::Io { path, source } => {
                write!(f, "zerodds-build: I/O on {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Compose { source, .. } => Some(source),
            Self::Codegen { source, .. } => Some(source),
            Self::Io { source, .. } => Some(source),
            Self::NoOutDir | Self::NoFileStem(_) | Self::DuplicateStem { .. } => None,
        }
    }
}

/// Builder for an IDL -> Rust `build.rs` compilation, mirroring
/// `zerodds-idlc`'s own flags (`-I`, `-D`, `--default-extensibility`,
/// `--default-nested`, `--no-typeobject`).
#[derive(Debug, Clone)]
pub struct Config {
    include_dirs: Vec<PathBuf>,
    defines: Vec<String>,
    out_dir: Option<PathBuf>,
    default_extensibility: Option<DefaultExt>,
    default_nested: Option<bool>,
    typeobject: bool,
    cdr_only: bool,
    header_comment: Option<String>,
    emit_cargo_directives: bool,
    strip_inner_attrs_for_include: bool,
    flatten_shared_includes: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            include_dirs: Vec::new(),
            defines: Vec::new(),
            out_dir: None,
            default_extensibility: None,
            default_nested: None,
            typeobject: true,
            cdr_only: false,
            header_comment: None,
            emit_cargo_directives: true,
            strip_inner_attrs_for_include: false,
            flatten_shared_includes: false,
        }
    }
}

impl Config {
    /// New builder with the CLI's own defaults: TypeObject emission on,
    /// no forced default-extensibility (falls back to the compile-time
    /// `ext-default-*` feature, same as `zerodds-idlc` without
    /// `--default-extensibility`).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an `#include` search directory (equivalent to `-I`). May be
    /// called multiple times; directories are tried in the order added,
    /// after the requesting file's own directory for quoted includes.
    pub fn include_dir(&mut self, dir: impl Into<PathBuf>) -> &mut Self {
        self.include_dirs.push(dir.into());
        self
    }

    /// Adds a preprocessor define (equivalent to `-D NAME` or
    /// `-D NAME=VALUE`). `value: None` defines the macro as `1` (gcc
    /// convention).
    pub fn define(&mut self, name: impl AsRef<str>, value: Option<&str>) -> &mut Self {
        let name = name.as_ref();
        self.defines.push(match value {
            Some(v) => format!("{name}={v}"),
            None => name.to_string(),
        });
        self
    }

    /// Overrides the output directory. Defaults to `$OUT_DIR` (set by
    /// Cargo when running as a `build.rs`).
    pub fn out_dir(&mut self, dir: impl Into<PathBuf>) -> &mut Self {
        self.out_dir = Some(dir.into());
        self
    }

    /// Default extensibility for un-annotated `struct`/`union`/`enum`
    /// (equivalent to `--default-extensibility`). Unset falls back to the
    /// compile-time `ext-default-*` Cargo feature of this crate.
    pub fn default_extensibility(&mut self, ext: DefaultExt) -> &mut Self {
        self.default_extensibility = Some(ext);
        self
    }

    /// Default `@nested` for un-annotated types without `@topic`
    /// (equivalent to `--default-nested`).
    pub fn default_nested(&mut self, nested: bool) -> &mut Self {
        self.default_nested = Some(nested);
        self
    }

    /// Enables/disables XTypes TypeObject emission (default: on,
    /// equivalent to the absence/presence of `--no-typeobject`).
    pub fn typeobject(&mut self, enabled: bool) -> &mut Self {
        self.typeobject = enabled;
        self
    }

    /// CDR-only mode: omits the `DdsType` impl, emitting only type
    /// declarations + classic `CdrEncode`/`CdrDecode` (the CORBA/GIOP
    /// wire path — see `zerodds_idl_rust::RustGenOptions::cdr_only`).
    pub fn cdr_only(&mut self, enabled: bool) -> &mut Self {
        self.cdr_only = enabled;
        self
    }

    /// Header comment placed at the top of every generated file.
    pub fn header_comment(&mut self, comment: impl Into<String>) -> &mut Self {
        self.header_comment = Some(comment.into());
        self
    }

    /// Disables emitting `cargo:rerun-if-changed` lines (for callers that
    /// invoke `compile` outside a `build.rs`, e.g. tests / codegen tools).
    /// Enabled by default.
    pub fn emit_cargo_directives(&mut self, enabled: bool) -> &mut Self {
        self.emit_cargo_directives = enabled;
        self
    }

    /// Strips the generated file's `#![allow(...)]` inner attributes
    /// before writing it out (default: off, matching `zerodds-idlc`'s raw
    /// output). Set this when the caller's `src/lib.rs` pulls the file in
    /// via `include!(concat!(env!("OUT_DIR"), "/<Stem>.rs"))`: an inner
    /// attribute is only legal as the literal first token of the file it
    /// textually ends up in, which an `include!`d file is not — the
    /// caller re-applies the same allows as *outer* attributes at the
    /// `include!` call site instead (see the crate docs / the
    /// `zerodds-build-sample` example). Leave this off when writing the
    /// file to be used as a real Rust module file (`#[path = "..."] mod
    /// foo;`), where the inner attributes are valid as-is.
    pub fn strip_inner_attrs_for_include(&mut self, enabled: bool) -> &mut Self {
        self.strip_inner_attrs_for_include = enabled;
        self
    }

    /// Composes the whole `idls` set as one **project** and emits each shared
    /// `#include`d top-level module only once across the set (default: off).
    ///
    /// Off (the default), every input is composed on its own, so two inputs
    /// that both `#include "common.idl"` each carry a full copy of `module
    /// common` — correct when each output is wrapped in its own module at the
    /// `include!` site (`mod a { include!("a.rs") }`), where the copies are
    /// isolated. Enable this when the outputs are `include!`d **flat into one
    /// namespace**: the copies would then collide (Rust E0428, "`common`
    /// defined multiple times"), so the shared module is emitted by the first
    /// input that pulls it in and stripped from the rest, their
    /// `super::common::…` references resolving to that single copy. Two inputs
    /// that pull in a *contradictory* same-named module (different body) are
    /// rejected (`Error::Compose` wrapping `ComposeError::Conflict`) rather
    /// than silently resolved.
    pub fn flatten_shared_includes(&mut self, enabled: bool) -> &mut Self {
        self.flatten_shared_includes = enabled;
        self
    }

    /// Composes + generates Rust for every `.idl` file in `idls`, writing
    /// `<out_dir>/<stem>.rs` per input. Each output module is
    /// self-contained (its own `#![allow(...)]` header) — a caller
    /// typically `include!`s it, stripping inner attributes first if the
    /// include site is not the module's own file (see
    /// `zerodds-idl-compose`'s crate docs for the pattern the in-repo
    /// `build.rs` scripts use).
    ///
    /// # Errors
    /// See [`Error`]. The first failing `.idl` file aborts the batch —
    /// `build.rs` failures should be immediate and unambiguous.
    pub fn compile(&self, idls: &[impl AsRef<Path>]) -> Result<(), Error> {
        let out_dir = match &self.out_dir {
            Some(d) => d.clone(),
            None => PathBuf::from(env::var("OUT_DIR").map_err(|_| Error::NoOutDir)?),
        };
        std::fs::create_dir_all(&out_dir).map_err(|source| Error::Io {
            path: out_dir.clone(),
            source,
        })?;

        // Preflight: two inputs with the same file stem both write
        // `<out_dir>/<stem>.rs`, so the second would silently overwrite the
        // first. Reject the whole batch before writing anything.
        let mut seen: std::collections::HashMap<String, PathBuf> = std::collections::HashMap::new();
        for idl in idls {
            let path = idl.as_ref();
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or_else(|| Error::NoFileStem(path.to_path_buf()))?;
            if let Some(first) = seen.insert(stem.to_string(), path.to_path_buf()) {
                return Err(Error::DuplicateStem {
                    stem: stem.to_string(),
                    first,
                    second: path.to_path_buf(),
                });
            }
        }

        if self.flatten_shared_includes {
            return self.compile_project(idls, &out_dir);
        }

        for idl in idls {
            self.compile_one(idl.as_ref(), &out_dir)?;
        }
        Ok(())
    }

    /// Project path: compose every input together so a top-level module pulled
    /// in through a shared `#include` is emitted once across the set (see
    /// [`Config::flatten_shared_includes`]).
    fn compile_project(&self, idls: &[impl AsRef<Path>], out_dir: &Path) -> Result<(), Error> {
        let opts = self.compose_options();
        let paths: Vec<&Path> = idls.iter().map(AsRef::as_ref).collect();
        let comps = zerodds_idl_compose::compose_project(&paths, &opts).map_err(|source| {
            // Attribute the failure to the first input; the message names the
            // offending definition and both inputs.
            Error::Compose {
                path: paths.first().map_or_else(PathBuf::new, |p| p.to_path_buf()),
                source,
            }
        })?;

        for (idl, comp) in paths.iter().zip(comps) {
            if self.emit_cargo_directives {
                println!("cargo:rerun-if-changed={}", idl.display());
                for dep in comp.output.dependency_files.iter().skip(1) {
                    println!("cargo:rerun-if-changed={dep}");
                }
                if let Some(warning) = &comp.output.type_object_warning {
                    println!("cargo:warning=zerodds-build: {} — {warning}", idl.display());
                }
            }
            let mut code = self.render(idl, &comp.output)?;
            // Strip the shared modules an earlier input already emits, leaving
            // one copy across the flat-included project.
            code = zerodds_idl_compose::strip_top_level_modules_rust(&code, &comp.shared_modules);
            self.write_module(idl, out_dir, code)?;
        }
        Ok(())
    }

    /// The [`ComposeOptions`] this config maps to.
    fn compose_options(&self) -> ComposeOptions {
        ComposeOptions {
            include_dirs: self.include_dirs.clone(),
            defines: self.defines.clone(),
            default_extensibility: self.default_extensibility,
            default_nested: self.default_nested,
            emit_typeobject: self.typeobject,
        }
    }

    /// Renders the Rust module (types + TypeObject holder) for one composed
    /// input, applying `strip_inner_attrs_for_include`. Does not yet strip
    /// shared modules — the caller does that where it applies.
    fn render(
        &self,
        idl: &Path,
        composed: &zerodds_idl_compose::ComposeOutput,
    ) -> Result<String, Error> {
        let rust_opts = RustGenOptions {
            header_comment: self.header_comment.clone(),
            cdr_only: self.cdr_only,
        };
        let mut code =
            generate_rust_module(&composed.ast, &rust_opts).map_err(|source| Error::Codegen {
                path: idl.to_path_buf(),
                source,
            })?;
        let type_objects_base = idl.file_stem().and_then(|s| s.to_str()).unwrap_or("types");
        code.push_str(&zerodds_idl_compose::typeobject::render_rust(
            &composed.type_objects,
            &composed.ast,
            type_objects_base,
        ));
        if self.strip_inner_attrs_for_include {
            code = code
                .lines()
                .filter(|l| !l.trim_start().starts_with("#!["))
                .collect::<Vec<_>>()
                .join("\n");
        }
        Ok(code)
    }

    /// Writes `code` to `<out_dir>/<stem>.rs`.
    fn write_module(&self, idl: &Path, out_dir: &Path, code: String) -> Result<(), Error> {
        let stem = idl
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| Error::NoFileStem(idl.to_path_buf()))?;
        let out_path = out_dir.join(format!("{stem}.rs"));
        std::fs::write(&out_path, code).map_err(|source| Error::Io {
            path: out_path,
            source,
        })
    }

    fn compile_one(&self, idl: &Path, out_dir: &Path) -> Result<(), Error> {
        if self.emit_cargo_directives {
            println!("cargo:rerun-if-changed={}", idl.display());
        }

        let composed = compose(idl, &self.compose_options()).map_err(|source| Error::Compose {
            path: idl.to_path_buf(),
            source,
        })?;

        if self.emit_cargo_directives {
            // `dependency_files[0]` is `idl` itself (already emitted above);
            // every further entry is a transitively #include-d file — an
            // edit there must also trigger a rebuild.
            for dep in composed.dependency_files.iter().skip(1) {
                println!("cargo:rerun-if-changed={dep}");
            }
            if let Some(warning) = &composed.type_object_warning {
                println!("cargo:warning=zerodds-build: {} — {warning}", idl.display());
            }
        }

        let code = self.render(idl, &composed)?;
        self.write_module(idl, out_dir, code)
    }
}

/// Shorthand for `Config::new().compile(idls)`.
///
/// # Errors
/// See [`Config::compile`].
pub fn compile(idls: &[impl AsRef<Path>]) -> Result<(), Error> {
    Config::new().compile(idls)
}
