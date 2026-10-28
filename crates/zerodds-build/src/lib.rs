// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-build`. Safety classification: **TOOLING**.
//!
//! A `build.rs` helper for ZeroDDS IDL.
//!
//! Build-time only (`[build-dependencies]`), std, `forbid(unsafe_code)`.
//!
//! ```no_run
//! fn main() -> Result<(), zerodds_build::Error> {
//!     zerodds_build::Config::new()
//!         .include_dir("idl")
//!         .compile(&["idl/Robot.idl"])
//! }
//! ```
//!
//! Each `.idl` becomes one `.rs` in `OUT_DIR`, named after the file, to
//! be pulled in with `include!`:
//!
//! ```ignore
//! include!(concat!(env!("OUT_DIR"), "/Robot.rs"));
//! ```
//!
//! ## Topic types
//!
//! What that writes is the Rust type and its codec — a pair of inherent
//! functions over `zerodds_wire`. `DomainParticipant::create_topic`
//! takes a `T: zerodds_dcps::DdsType`, which is a different surface, and
//! [`Config::dds_type_impls`] writes it:
//!
//! ```no_run
//! fn main() -> Result<(), zerodds_build::Error> {
//!     zerodds_build::Config::new()
//!         .dds_type_impls(true)
//!         .compile(&["idl/Robot.idl"])
//! }
//! ```
//!
//! ```ignore
//! participant.create_topic::<Robot>("entity.robot")?;
//! ```
//!
//! It is off by default because it puts `zerodds-dcps`, `zerodds-types`
//! and `zerodds-wire` into the consumer's `[dependencies]`; the option's
//! own documentation says why that is not a default worth taking.
//!
//! ## Where it goes
//!
//! **At the crate root.** The generated modules name types across module
//! boundaries with paths relative to where they sit, and each `pub mod`
//! carries `use super::*` so the file's own imports reach into it. Both
//! hold wherever the file is included; what does not hold is splitting
//! one `.idl`'s output across two places.
//!
//! ## Rebuilds
//!
//! Every `.idl` passed here **and every file it `#include`s** is reported
//! to cargo as `rerun-if-changed`. Without the second half, an edit to an
//! included file leaves the generated code stale and nothing says so:
//! cargo does not re-run a build script for a path it was not told about.

#![forbid(unsafe_code)]
#![allow(clippy::print_stdout)] // cargo's build-script protocol *is* stdout.
#![warn(missing_docs)]

use std::path::{Path, PathBuf};

mod dds_type;

use zerodds_ast::facet::Extensibility;
use zerodds_ast::{Ast, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ast_layout::key::{Encoding, Endianness, LayoutKey, Purpose};
use zerodds_ast_layout::{AxisFlag, Dialect, DialectLowering, DialectSettings, PlCdr1Reading};
use zerodds_idl_compose::{ComposeOptions, PathLoader, compose_with};
use zerodds_lang_rules::adapter::cst::{RustMapFormScope, nest_by_module, rust_item_pairs};

/// How the generated Rust holds an IDL `map` ([`Config::rust_map`]).
pub use zerodds_lang_rules::adapter::cst::RustMapForm;
use zerodds_lang_rules::encode::{rust_codec, rust_codec_pair};
use zerodds_lang_rules::rule::rust;
use zerodds_lang_rules::{ModelGraph, apply};

/// What went wrong.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// A unit could not be read.
    Read {
        /// The path as given.
        path: PathBuf,
        /// What the filesystem said.
        detail: String,
    },
    /// A unit did not compose: it holds an error the front end reported.
    ///
    /// Every diagnostic is carried, not just the first — a build that
    /// reports one defect per run costs a round trip per defect.
    Compose {
        /// The path as given.
        path: PathBuf,
        /// Every error-severity diagnostic, rendered.
        diagnostics: Vec<String>,
    },
    /// The model composed and the emitter refused a type in it.
    Codegen {
        /// The path as given.
        path: PathBuf,
        /// What the emitter said, naming the construct.
        detail: String,
    },
    /// The generated module could not be written.
    Write {
        /// Where it was going.
        path: PathBuf,
        /// What the filesystem said.
        detail: String,
    },
    /// `OUT_DIR` was not set and no output directory was chosen.
    ///
    /// Cargo sets `OUT_DIR` for a build script. Reaching this means the
    /// caller is not one, and has to say where the output goes.
    NoOutDir,
    /// A value given to [`Config::axis`] is not one its axis recognises.
    ///
    /// `axis` is infallible — it only appends to a list — so this is
    /// where a typo in the value string surfaces, the same way an
    /// unknown axis value surfaces at `zerodds-idlc generate`'s command
    /// line, just later: there is no command line here to refuse it at.
    Axis {
        /// `AxisFlag::flag_name`, so the offending call is easy to find
        /// among several `.axis(...)` calls on the same `Config`.
        flag: &'static str,
        /// The value given.
        value: String,
        /// Why it was refused, from the axis's own policy.
        detail: String,
    },
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Read { path, detail } => write!(f, "{}: {detail}", path.display()),
            Self::Compose { path, diagnostics } => {
                write!(f, "{}: {}", path.display(), diagnostics.join("; "))
            }
            Self::Codegen { path, detail } => write!(f, "{}: {detail}", path.display()),
            Self::Write { path, detail } => write!(f, "{}: {detail}", path.display()),
            Self::NoOutDir => write!(f, "OUT_DIR is not set and no out_dir() was given"),
            Self::Axis {
                flag,
                value: _,
                detail,
            } => write!(f, "{flag}: {detail}"),
        }
    }
}

impl std::error::Error for Error {}

/// What an aggregate of unspecified extensibility is taken to be.
///
/// XTypes 1.3 §7.3.1.2.1.8 makes it appendable and then says in as many
/// words that a compiler may let the user choose. Nothing here overrules
/// a source that *did* say.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefaultExt {
    /// `@final`.
    Final,
    /// `@appendable` — the specification's own default.
    Appendable,
    /// `@mutable`.
    Mutable,
}

impl From<DefaultExt> for Extensibility {
    fn from(value: DefaultExt) -> Self {
        match value {
            DefaultExt::Final => Self::Final,
            DefaultExt::Appendable => Self::Appendable,
            DefaultExt::Mutable => Self::Mutable,
        }
    }
}

impl From<Extensibility> for DefaultExt {
    fn from(value: Extensibility) -> Self {
        match value {
            Extensibility::Final => Self::Final,
            Extensibility::Appendable => Self::Appendable,
            Extensibility::Mutable => Self::Mutable,
        }
    }
}

/// An IDL-to-Rust compilation, configured the way `zerodds-idlc`'s own
/// flags configure it.
pub struct Config {
    include_dirs: Vec<PathBuf>,
    defines: Vec<(String, String)>,
    out_dir: Option<PathBuf>,
    default_extensibility: Option<DefaultExt>,
    header_comment: Option<String>,
    emit_cargo_directives: bool,
    typeobject: bool,
    type_identifier_constants: bool,
    dds_type_impls: bool,
    dialect: Dialect,
    axis_overrides: Vec<(AxisFlag, String)>,
    pl_cdr1_reading: PlCdr1Reading,
    rust_map: RustMapForm,
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}

impl Config {
    /// A new configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            include_dirs: Vec::new(),
            defines: Vec::new(),
            out_dir: None,
            default_extensibility: None,
            header_comment: None,
            emit_cargo_directives: true,
            typeobject: true,
            type_identifier_constants: false,
            dds_type_impls: false,
            dialect: Dialect::default(),
            axis_overrides: Vec::new(),
            pl_cdr1_reading: PlCdr1Reading::default(),
            rust_map: RustMapForm::default(),
        }
    }

    /// Add a directory to the `#include` search list — the CLI's `-I`.
    pub fn include_dir(&mut self, dir: impl Into<PathBuf>) -> &mut Self {
        self.include_dirs.push(dir.into());
        self
    }

    /// Define a macro — the CLI's `-D`. `None` is `-DNAME`.
    pub fn define(&mut self, name: impl AsRef<str>, value: Option<&str>) -> &mut Self {
        self.defines.push((
            name.as_ref().to_string(),
            value.unwrap_or_default().to_string(),
        ));
        self
    }

    /// Where the generated modules go. Defaults to `OUT_DIR`.
    pub fn out_dir(&mut self, dir: impl Into<PathBuf>) -> &mut Self {
        self.out_dir = Some(dir.into());
        self
    }

    /// What an aggregate of unspecified extensibility is taken to be.
    pub fn default_extensibility(&mut self, ext: DefaultExt) -> &mut Self {
        self.default_extensibility = Some(ext);
        self
    }

    /// Reproduce one peer's answers to every wire axis at once — the
    /// same preset `zerodds-idlc generate --dialect` takes, over the
    /// same [`Dialect`] `zerodds_ast_layout` defines.
    ///
    /// The default, [`Dialect::default`] (`specification`), is what
    /// every axis-affecting field already defaulted to before this
    /// existed — a generated module built with no `dialect` call reads
    /// and writes exactly what it did before. What changes when this is
    /// called is both halves the dialect settles: the front end's three
    /// lowering options (`@hashid`'s counter, an escaped name's
    /// spelling, a declaration named after its own scope), which used
    /// to be the specification's unconditionally, and the `LayoutKey`
    /// [`Self::compile`] plans the codec under. [`Self::axis`] overrides
    /// one answer of whichever dialect is in effect.
    pub fn dialect(&mut self, dialect: Dialect) -> &mut Self {
        self.dialect = dialect;
        self
    }

    /// Override one wire axis's answer, after the dialect's.
    ///
    /// `axis` is one of [`zerodds_ast_layout::AXES`] — the same table
    /// `zerodds-idlc generate`'s `--<axis>` flags and
    /// `wireprobe-zerodds`'s read — and `value` is one of the spellings
    /// [`AxisFlag::help_values`] lists for it. Several calls apply in
    /// the order given, each overriding one answer of the dialect
    /// [`Self::dialect`] named (or the default dialect, if it was not
    /// called).
    ///
    /// Infallible, and applied lazily: a value the axis does not
    /// recognise is refused by [`Self::compile`] and
    /// [`Self::compile_bundle`], as [`Error::Axis`], because there is no
    /// command line to refuse it at here — a `build.rs` that supplied a
    /// typo learns about it the way it learns about a `.idl` that does
    /// not parse, from the build failing with a message that names it.
    ///
    /// [`AxisFlag::DefaultExtensibility`] also sets what
    /// [`Self::default_extensibility`] sets, because that one axis is
    /// two settings and not one: `LayoutKey::default_extensibility`,
    /// which `Self::layout_key` answers for, decides the framing of a
    /// member whose *own* type has no resolved facet, but a top-level
    /// type's own facet is resolved once, during composition
    /// (`ComposeOptions::default_extensibility`), before any
    /// `LayoutKey` exists — and `zerodds_ast::Ast::effective_extensibility`,
    /// which the generated `DdsType::EXTENSIBILITY` and the evolution-
    /// tolerant decode pattern both read, only ever sees the composed
    /// answer. An `axis` call that touched the `LayoutKey` alone would
    /// leave those two reading different extensibilities for the same
    /// type — a member framed as `@final` decoded as if it were
    /// `@appendable`, silently, because nothing before this call site
    /// caught it. `wireprobe-zerodds` and `zerodds-idlc generate`'s own
    /// `--default-extensibility` do not have this split: the first
    /// never resolves a facet at compose time at all, so its `LayoutKey`
    /// fallback is the only answer to agree with; the second's
    /// `--default-extensibility` is composition's own flag and was
    /// never routed through the axis table.
    pub fn axis(&mut self, axis: AxisFlag, value: impl Into<String>) -> &mut Self {
        let value = value.into();
        if axis == AxisFlag::DefaultExtensibility
            && let Some(extensibility) = Extensibility::parse(&value)
        {
            self.default_extensibility = Some(extensibility.into());
        }
        self.axis_overrides.push((axis, value));
        self
    }

    /// Whether the generated PL_CDR1 reader takes a stream another
    /// implementation framed, or reports it.
    ///
    /// [`PlCdr1Reading::Strict`] is the default. Under
    /// [`PlCdr1Reading::Tolerant`] the reader accepts any flags on
    /// `PID_EXTENDED` and `PID_LIST_END` (XTypes 1.3 §7.4.1.2.1), the short
    /// and the extended header of a parameter (§7.4.1.2.2) and a declared
    /// length that is the value's own or that length rounded up to four
    /// (§7.4.1.2). It changes the generated Rust reader only; the writer
    /// and the generators for other languages are not affected, and there
    /// is no `--<axis>` flag for it.
    pub fn pl_cdr1_reading(&mut self, reading: PlCdr1Reading) -> &mut Self {
        self.pl_cdr1_reading = reading;
        self
    }

    /// The `LayoutKey` this configuration's dialect and axis overrides
    /// plan `encoding`/`endianness` under.
    ///
    /// One fold and not [`zerodds_ast_layout::dialect_settings_from_overrides`]
    /// directly: that function's error names the value alone, and this
    /// keeps [`AxisFlag::flag_name`] beside it so [`Error::Axis`] can
    /// name which `.axis(...)` call on this `Config` was wrong, the way
    /// `zerodds-idlc`'s own command-line errors name the flag.
    ///
    /// # Errors
    /// [`Error::Axis`] for the first override whose value its axis does
    /// not recognise.
    fn layout_key(&self, encoding: Encoding, endianness: Endianness) -> Result<LayoutKey, Error> {
        let mut settings =
            DialectSettings::in_dialect(self.dialect, encoding, endianness, Purpose::Data);
        for (axis, value) in &self.axis_overrides {
            settings = axis
                .parse_and_apply(settings, value)
                .map_err(|detail| Error::Axis {
                    flag: axis.flag_name(),
                    value: value.clone(),
                    detail,
                })?;
        }
        Ok(settings.key.with_pl_cdr1_reading(self.pl_cdr1_reading))
    }

    /// The front-end lowering options this configuration's dialect
    /// settles — before any `LayoutKey` exists, so not something
    /// [`Self::layout_key`] answers for.
    fn lowering(&self) -> DialectLowering {
        DialectLowering::for_dialect(self.dialect)
    }

    /// What composition should resolve an unannotated aggregate to —
    /// `ComposeOptions::default_extensibility`.
    ///
    /// [`Self::default_extensibility`] wins when given (explicitly, or
    /// through [`Self::axis`]'s own write of the same field — see that
    /// method's doc). Absent that, and only when [`Self::dialect`] named
    /// something other than the default: that dialect's own answer, not
    /// `None`. `None` is composition's own signal to leave a type's
    /// facet unresolved rather than writing the rule's default in — see
    /// `zerodds_idl_lower::complete`'s own comment on why — and
    /// `Ast::effective_extensibility`'s fallback for an unresolved facet
    /// is a fixed constant, not [`Self::layout_key`]'s dialect. Left as
    /// `None` here whenever the dialect did not move, the two answer the
    /// same way by construction and a run with neither `dialect()` nor
    /// `axis()` called reads exactly as it always did. Moved by the
    /// dialect and left `None` here, they would not: the codec's
    /// `DHEADER` follows the `LayoutKey` (correctly, the dialect's), and
    /// the generated `DdsType::EXTENSIBILITY` constant and the
    /// evolution-tolerant decode pattern — both `effective_extensibility`
    /// readers — would still answer the specification's, so a decoder
    /// built from the file would default a truncated member of a type
    /// planned `@final` instead of refusing the stream, silently.
    fn compose_default_extensibility(&self) -> Option<Extensibility> {
        self.default_extensibility.map(Into::into).or_else(|| {
            (self.dialect != Dialect::default()).then(|| self.dialect.default_extensibility())
        })
    }

    /// A comment placed at the top of each generated module.
    pub fn header_comment(&mut self, comment: impl Into<String>) -> &mut Self {
        self.header_comment = Some(comment.into());
        self
    }

    /// Whether to emit the `type_objects` module.
    ///
    /// On by default. It holds one XCDR2-LE serialised minimal
    /// `TypeObject` per named type, which is what a participant hands to
    /// a peer that asks over the TypeLookup service — a type with no
    /// object cannot be resolved remotely, and the peer finds that out
    /// at discovery time rather than here.
    pub fn typeobject(&mut self, enabled: bool) -> &mut Self {
        self.typeobject = enabled;
        self
    }

    /// Whether to emit the `type_identifiers` module.
    ///
    /// Off by default, because the module's constants are spelled in
    /// `zerodds_types` terms and pull that crate into the consumer's
    /// dependencies. A crate that implements `zerodds_dcps::DdsType` by
    /// hand over the generated codec turns this on to get
    /// `TYPE_IDENTIFIER` from the same computation discovery uses,
    /// instead of pinning a hash nothing re-checks.
    pub fn type_identifier_constants(&mut self, enabled: bool) -> &mut Self {
        self.type_identifier_constants = enabled;
        self
    }

    /// Whether to emit `impl zerodds_dcps::DdsType` for each generated
    /// type.
    ///
    /// With this on, a generated type is a topic type:
    /// `participant.create_topic::<Robot>("entity.robot")` takes it, and
    /// the four codec pairs, the KeyHash holder, the `TypeIdentifier`
    /// and the `TypeObject` all come from the model rather than from a
    /// bridge somebody wrote by hand beside the generated file.
    ///
    /// **Off by default**, for the reason
    /// [`Self::type_identifier_constants`] is off: the impl is spelled
    /// in `zerodds_dcps`, `zerodds_types` and `zerodds_wire` terms and
    /// puts all three into the consumer's dependencies. That is the
    /// whole DCPS runtime, and this crate is also used by consumers that
    /// want the codec and nothing else — `crates/wire-conformance`
    /// compiles the generated modules with no DDS in the graph at all,
    /// and would stop compiling the day a default turned this on. A
    /// default that breaks a build nobody changed is worse than a line
    /// in a `build.rs`.
    ///
    /// Turning it on implies nothing else: the identifier and the object
    /// are written into the impl, not read out of the
    /// [`Self::type_identifier_constants`] or [`Self::typeobject`]
    /// modules, whose constants are keyed by a type's **simple** name
    /// and would collide between two modules declaring one name.
    pub fn dds_type_impls(&mut self, enabled: bool) -> &mut Self {
        self.dds_type_impls = enabled;
        self
    }

    /// Whether to print `cargo:rerun-if-changed` lines.
    ///
    /// On by default and worth leaving on: cargo does not re-run a build
    /// script for a path it was not told about, so turning this off means
    /// an edit to an `.idl` produces no rebuild and no message.
    pub fn emit_cargo_directives(&mut self, enabled: bool) -> &mut Self {
        self.emit_cargo_directives = enabled;
        self
    }

    /// How the generated Rust holds an IDL `map`, as `zerodds-idlc
    /// --rust-map` selects it.
    ///
    /// [`RustMapForm::Entries`] is the default: `Vec<zerodds_wire::MapEntry<K, V>>`,
    /// the sequence of `MapEntry` that XTypes 1.3 §7.5.1.3.6 binds a map to
    /// where a language has no mapping of its own. [`RustMapForm::BTreeMap`]
    /// is the option: a `BTreeMap<K, V>`, whose entries are in ascending key
    /// order and not in the order they were put in.
    pub fn rust_map(&mut self, form: RustMapForm) -> &mut Self {
        self.rust_map = form;
        self
    }

    /// Compile several units into **one** Rust module.
    ///
    /// The units are lowered into a **shared model**, so a module two of
    /// them declare is one node and the generator emits one `pub mod`
    /// for it. `compile` gives each unit its own `.rs`, and two of those
    /// pulled into one Rust scope with `include!` both open `pub mod
    /// common` — `error[E0428]: the name `common` is defined multiple
    /// times`, which is issue #39.
    ///
    /// That is a structural answer and not a textual one. Concatenating
    /// the units, or writing a synthetic file full of `#include` lines,
    /// reaches the same output for IDL and buys nothing anywhere else:
    /// the model is shared with DDS-XML, ROS and a `TypeObject` off the
    /// wire, and node identity is what merges those. `zerodds-idl-compose`
    /// holds the merge; this only names the output.
    ///
    /// `name` is the output's stem — `world` writes `world.rs`.
    ///
    /// # Errors
    /// As [`Self::compile`].
    pub fn compile_bundle(&self, idls: &[impl AsRef<Path>], name: &str) -> Result<(), Error> {
        let out_dir = self.resolve_out_dir()?;
        let loader = PathLoader::new(self.include_dirs.clone());
        let paths: Vec<PathBuf> = idls.iter().map(|p| p.as_ref().to_path_buf()).collect();

        if self.emit_cargo_directives {
            for path in &paths {
                println!("cargo:rerun-if-changed={}", path.display());
            }
        }

        let mut composed = zerodds_idl_compose::compose_all(
            &paths,
            &loader,
            ComposeOptions {
                default_extensibility: self.compose_default_extensibility(),
                default_nested: None,
                defines: self.defines.clone(),
                // The specification's readings by default — Cyclone's
                // and Fast DDS's for the counter, IDL 4.2 §7.5.2's
                // refusal for the enclosing scope — and `self.dialect`'s
                // once [`Self::dialect`] has been called: a build script
                // has no reason to take RTI's or Fast DDS's reading
                // unless it is talking to that peer, and `Self::dialect`
                // is the knob for saying so.
                hashed_id_advances_the_counter: if self.lowering().hashed_id_advances_the_counter {
                    zerodds_idl_compose::HashedIdCounter::Advances
                } else {
                    zerodds_idl_compose::HashedIdCounter::Skipped
                },
                hashed_name_spelling: if self.lowering().hashid_reads_the_escaped_spelling {
                    zerodds_idl_compose::HashedNameSpelling::AsWritten
                } else {
                    zerodds_idl_compose::HashedNameSpelling::Stripped
                },
                name_of_the_enclosing_scope: if self.lowering().admits_name_of_enclosing_scope {
                    zerodds_idl_compose::NameOfTheEnclosingScope::Admitted
                } else {
                    zerodds_idl_compose::NameOfTheEnclosingScope::Refused
                },
                // The front end carries `@key` on a union discriminator by
                // default (DDS-XTypes 1.3 Table 21). This build asks for the
                // refusal: the Rust union codec has no key holder to emit
                // for a keyed discriminator (`dds_type.rs`), so carrying it
                // would fail later, in the generator, with a message about
                // methods instead of about the declaration.
                key_on_union_discriminator: zerodds_idl_compose::KeyOnUnionDiscriminator::Refused,
                // A build reads the IDL it was pointed at, which may be a
                // vendor's. Refusing an extension here would fail the
                // build over a portability question nobody asked;
                // `zerodds-idlc --strict` is where that question is put.
                dialect: zerodds_idl_compose::ComposeOptions::default().dialect,
            },
        )
        .map_err(|detail| Error::Read {
            path: paths.first().cloned().unwrap_or_default(),
            detail,
        })?;

        if self.emit_cargo_directives {
            for opened in loader.opened() {
                println!("cargo:rerun-if-changed={opened}");
            }
        }
        let bundle = PathBuf::from(format!("{name}.idl"));
        if composed.failed() {
            return Err(Error::Compose {
                path: bundle,
                diagnostics: composed
                    .diagnostics
                    .iter()
                    .filter(|d| d.severity == zerodds_idl_parse::Severity::Error)
                    // A bundle's resolve diagnostics name no unit: their
                    // spans could be in any of them, and a line read from
                    // the first would be a guess.
                    .map(|d| located(d, None))
                    .collect(),
            });
        }

        self.leave_out_what_xtypes_has_no_type_for(&bundle, &mut composed.ast)?;
        let code = self.render_with_stem(&bundle, &composed.ast, name)?;
        let target = out_dir.join(format!("{name}.rs"));
        std::fs::write(&target, code).map_err(|error| Error::Write {
            path: target,
            detail: error.to_string(),
        })
    }

    fn resolve_out_dir(&self) -> Result<PathBuf, Error> {
        match &self.out_dir {
            Some(dir) => Ok(dir.clone()),
            None => Ok(PathBuf::from(
                std::env::var("OUT_DIR").map_err(|_| Error::NoOutDir)?,
            )),
        }
    }

    /// Compile each unit into one Rust module.
    ///
    /// # Errors
    /// [`Error`], naming the unit. A unit that fails stops the run: a
    /// build that carried on would write a module that does not match
    /// the IDL beside it.
    pub fn compile(&self, idls: &[impl AsRef<Path>]) -> Result<(), Error> {
        let out_dir = self.resolve_out_dir()?;
        for idl in idls {
            self.compile_one(idl.as_ref(), &out_dir)?;
        }
        Ok(())
    }

    fn compile_one(&self, idl: &Path, out_dir: &Path) -> Result<(), Error> {
        let loader = PathLoader::new(self.include_dirs.clone());
        let path = idl.to_string_lossy().into_owned();
        let mut composed = compose_with(
            &path,
            &loader,
            ComposeOptions {
                default_extensibility: self.compose_default_extensibility(),
                default_nested: None,
                defines: self.defines.clone(),
                // The specification's readings by default — Cyclone's
                // and Fast DDS's for the counter, IDL 4.2 §7.5.2's
                // refusal for the enclosing scope — and `self.dialect`'s
                // once [`Self::dialect`] has been called: a build script
                // has no reason to take RTI's or Fast DDS's reading
                // unless it is talking to that peer, and `Self::dialect`
                // is the knob for saying so.
                hashed_id_advances_the_counter: if self.lowering().hashed_id_advances_the_counter {
                    zerodds_idl_compose::HashedIdCounter::Advances
                } else {
                    zerodds_idl_compose::HashedIdCounter::Skipped
                },
                hashed_name_spelling: if self.lowering().hashid_reads_the_escaped_spelling {
                    zerodds_idl_compose::HashedNameSpelling::AsWritten
                } else {
                    zerodds_idl_compose::HashedNameSpelling::Stripped
                },
                name_of_the_enclosing_scope: if self.lowering().admits_name_of_enclosing_scope {
                    zerodds_idl_compose::NameOfTheEnclosingScope::Admitted
                } else {
                    zerodds_idl_compose::NameOfTheEnclosingScope::Refused
                },
                // The front end carries `@key` on a union discriminator by
                // default (DDS-XTypes 1.3 Table 21). This build asks for the
                // refusal: the Rust union codec has no key holder to emit
                // for a keyed discriminator (`dds_type.rs`), so carrying it
                // would fail later, in the generator, with a message about
                // methods instead of about the declaration.
                key_on_union_discriminator: zerodds_idl_compose::KeyOnUnionDiscriminator::Refused,
                // A build reads the IDL it was pointed at, which may be a
                // vendor's. Refusing an extension here would fail the
                // build over a portability question nobody asked;
                // `zerodds-idlc --strict` is where that question is put.
                dialect: zerodds_idl_compose::ComposeOptions::default().dialect,
            },
        )
        .map_err(|detail| Error::Read {
            path: idl.to_path_buf(),
            detail,
        })?;

        if self.emit_cargo_directives {
            // The unit itself, and every file it pulled in. The second
            // half is the one that is easy to leave out and impossible
            // to notice: the build simply stops re-running.
            println!("cargo:rerun-if-changed={path}");
            for opened in loader.opened() {
                println!("cargo:rerun-if-changed={opened}");
            }
        }

        if composed.failed() {
            return Err(Error::Compose {
                path: idl.to_path_buf(),
                diagnostics: composed
                    .diagnostics
                    .iter()
                    .filter(|d| d.severity == zerodds_idl_parse::Severity::Error)
                    .map(|d| located(d, Some(idl)))
                    .collect(),
            });
        }

        let stem = idl
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("generated");
        self.leave_out_what_xtypes_has_no_type_for(idl, &mut composed.ast)?;
        let code = self.render_with_stem(idl, &composed.ast, stem)?;
        let target = out_dir.join(format!("{stem}.rs"));
        std::fs::write(&target, code).map_err(|error| Error::Write {
            path: target,
            detail: error.to_string(),
        })
    }

    /// Leave out the declarations DDS-XTypes 1.3 has no type for, and say
    /// so.
    ///
    /// §7.3.1.1.1: constructs outside the building blocks are legal in
    /// the file and "compilers that do not support them shall ignore them
    /// or issue a warning rather than halting with an error". A struct
    /// that holds an `any`, a `ValueBase`, a `native` or an object
    /// reference has no type identifier and no wire form, so it gets no
    /// Rust type and no codec; the build goes on with the rest of the
    /// file, and `cargo:warning` names each declaration left out when
    /// cargo directives are on.
    fn leave_out_what_xtypes_has_no_type_for(
        &self,
        idl: &Path,
        ast: &mut Ast,
    ) -> Result<(), Error> {
        let left_out = ast
            .drop_outside_profile(zerodds_ast::Profile::XTypes13)
            .map_err(|error| Error::Codegen {
                path: idl.to_path_buf(),
                detail: error.to_string(),
            })?;
        if self.emit_cargo_directives {
            for dropped in &left_out {
                println!(
                    "cargo:warning={}: {} reaches `{}`, for which DDS-XTypes 1.3 has no type \
                     ({}); no code is generated for it ({})",
                    idl.display(),
                    dropped.name,
                    dropped.construct,
                    dropped.detail,
                    dropped.spec_reference
                );
            }
        }
        Ok(())
    }

    /// The Rust module for one model: the types and their codecs, folded
    /// into the module tree once.
    fn render_with_stem(&self, idl: &Path, ast: &Ast, stem: &str) -> Result<String, Error> {
        let _map_form = RustMapFormScope::enter(self.rust_map);
        let graph = ModelGraph::of(ast);
        let applied = apply(&graph, &rust::catalogue()).map_err(|error| Error::Codegen {
            path: idl.to_path_buf(),
            detail: format!("{error:?}"),
        })?;

        // Types and codecs go through **one** fold. Folding them
        // separately and concatenating declares every shared ancestor
        // twice, and Rust allows a name once per scope.
        let mut pairs = rust_item_pairs(ast, &applied).map_err(|error| Error::Codegen {
            path: idl.to_path_buf(),
            detail: format!("{error:?}"),
        })?;
        // The types that got a second, XCDR1 codec. Only the `DdsType`
        // impl needs it, so it is only planned when that is asked for:
        // an XCDR1 plan is a second walk of every type, and a consumer
        // that wants the XCDR2 codec alone should not pay for it.
        let mut xcdr1: Vec<Id<TypeNode>> = Vec::new();
        for (id, node) in ast.types() {
            if node.name.is_empty() || !has_codec(&node.kind) {
                continue;
            }
            let key = self.layout_key(encoding_for(ast, id), Endianness::Little)?;
            for item in rust_codec(ast, id, key).map_err(|error| Error::Codegen {
                path: idl.to_path_buf(),
                detail: format!("{error:?}"),
            })? {
                pairs.push((node.module, item));
            }
            if !self.dds_type_impls {
                continue;
            }
            // A refusal here is **not** fatal and **not** silent: XCDR1
            // cannot frame every type XCDR2 can, and the `DdsType` impl
            // answers for the gap by leaving the four XCDR1 methods at
            // the trait's refusing defaults and naming the type in a
            // comment. Failing the whole build instead would refuse a
            // type whose XCDR2 wire — the one a DDS 1.4 `XCDR2`
            // representation asks for — is perfectly good.
            let v1 = self.layout_key(xcdr1_encoding_for(ast, id), Endianness::Little)?;
            if let Ok(items) = rust_codec_pair(ast, id, v1) {
                for item in items {
                    pairs.push((node.module, item));
                }
                xcdr1.push(id);
            }
        }
        let items = nest_by_module(ast, pairs).map_err(|error| Error::Codegen {
            path: idl.to_path_buf(),
            detail: format!("{error:?}"),
        })?;
        // `@verbatim(placement = BEGIN_FILE | END_FILE)`, OMG IDL 4.2
        // §8.3.5.1, around everything this file holds.
        let items = zerodds_lang_rules::verbatim::weave_file(
            ast,
            zerodds_lang_rules::verbatim::aliases::RUST,
            items,
        );

        let mut doc = vec![
            format!(
                "Generated by zerodds-build from {}.",
                idl.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("the IDL")
            ),
            String::new(),
            String::from("Do not edit: the point of this file is that nobody did."),
        ];
        if let Some(comment) = &self.header_comment {
            doc.push(String::new());
            doc.push(comment.clone());
        }
        let doc: Vec<&str> = doc.iter().map(String::as_str).collect();

        // Only the runtime names this file actually mentions.
        //
        // The five were imported unconditionally, and a type whose codec
        // needs none of `Narrowed`, `TryConstruct` or `WireError` — a
        // struct of plain scalars is one — carried them anyway. Under
        // `-D warnings`, which a `build.rs` inherits from the crate it
        // builds for, `unused_imports` is then an error and the
        // generated file does not compile. The consumer sees a build
        // failure in code they did not write.
        //
        // Which names appear is only knowable once the items are
        // rendered, so they are rendered twice: once to look, once to
        // keep. Both passes are string building over a tree that is
        // already built.
        let probe = zerodds_lang_render::rust::file(&zerodds_lang_cst::rust::File {
            inner_attrs: Vec::new(),
            doc: zerodds_lang_cst::common::Doc::lines(&[]),
            uses: Vec::new(),
            items: items.clone(),
        });
        let names: Vec<zerodds_lang_cst::common::Ident> = RUNTIME_IMPORTS
            .iter()
            .filter(|name| mentions_word(&probe, name))
            .map(|name| zerodds_lang_cst::common::Ident::new(name))
            .collect();
        // The file is included at the crate root; every nested module
        // reaches these through its own `use super::*`.
        let mut uses = Vec::new();
        if !names.is_empty() {
            uses.push(zerodds_lang_cst::rust::Use {
                path: zerodds_lang_cst::common::Path::single("zerodds_wire"),
                names,
                visibility: zerodds_lang_cst::common::Visibility::Private,
            });
        }
        // Under [`RustMapForm::BTreeMap`] `adapter::cst` renders an IDL
        // `map` as a bare `BTreeMap<K, V>`, so a file that carries one has
        // to bring the name in; the default form names
        // `zerodds_wire::MapEntry` by its path. Conditional for the
        // same reason the runtime names are: an unused import is an
        // error under the `-D warnings` a `build.rs` inherits, and the
        // consumer would see it in code they did not write.
        if mentions_word(&probe, "BTreeMap") {
            uses.push(zerodds_lang_cst::rust::Use {
                path: zerodds_lang_cst::common::Path::new(&["std", "collections"]),
                names: vec![zerodds_lang_cst::common::Ident::new("BTreeMap")],
                visibility: zerodds_lang_cst::common::Visibility::Private,
            });
        }
        let file = zerodds_lang_cst::rust::File {
            inner_attrs: Vec::new(),
            doc: zerodds_lang_cst::common::Doc::lines(&doc),
            uses,
            items,
        };
        let mut code = zerodds_lang_render::rust::file(&file);
        if self.typeobject {
            code.push_str(&type_objects_module(ast, stem));
        }
        if self.type_identifier_constants {
            code.push_str(&type_identifiers_module(ast, stem));
        }
        if self.dds_type_impls {
            let key_for = |encoding: Encoding| {
                self.layout_key(encoding, Endianness::Little)
                    .map_err(|error| error.to_string())
            };
            code.push_str(
                &dds_type::dds_type_impls(ast, &xcdr1, &key_for).map_err(|detail| {
                    Error::Codegen {
                        path: idl.to_path_buf(),
                        detail,
                    }
                })?,
            );
        }
        Ok(code)
    }
}

/// The type-identifier module: one `zerodds_types::TypeIdentifier`
/// constant per named type, in the **complete** form — the one a
/// hand-written `DdsType` impl carries as `TYPE_IDENTIFIER`.
///
/// Named after the unit for the same reason `type_objects_module` is:
/// two units in one scope must not collide.
///
/// A type whose identifier this module cannot spell as a constant —
/// today that is everything but an equivalence hash and a strongly
/// connected component — is **named in a comment** rather than left
/// out silently, so a reader who misses a constant finds the reason in
/// the same place the constant would be.
fn type_identifiers_module(ast: &Ast, stem: &str) -> String {
    use core::fmt::Write as _;

    let mut body = String::new();
    for (id, node) in ast.types() {
        if node.name.is_empty() || !has_codec(&node.kind) {
            continue;
        }
        let Ok(identifier) = zerodds_types::from_ast::type_identifier(
            ast,
            id,
            zerodds_types::from_ast::Form::Complete,
        ) else {
            let _ = writeln!(
                body,
                "    // `{}`: its identifier could not be computed from the model.",
                node.name
            );
            continue;
        };
        match identifier {
            zerodds_types::TypeIdentifier::EquivalenceHashComplete(hash) => {
                let octets: Vec<String> = hash.0.iter().map(|b| format!("0x{b:02x}")).collect();
                let _ = write!(
                    body,
                    "    /// Complete equivalence hash of `{}`, XTypes 1.3 \u{a7}7.3.4.6.\n    \
                     pub const {}: zerodds_types::TypeIdentifier =\n        \
                     zerodds_types::TypeIdentifier::EquivalenceHashComplete(\
                     zerodds_types::EquivalenceHash([{}]));\n",
                    node.name,
                    constant_name(&node.name),
                    octets.join(", ")
                );
            }
            // A type in a strongly connected component is identified by the
            // component's hash and its own position (§7.3.4.9.3).
            zerodds_types::TypeIdentifier::StronglyConnectedComponent(scc) => {
                let octets: Vec<String> = scc.hash.0.iter().map(|b| format!("0x{b:02x}")).collect();
                let _ = write!(
                    body,
                    "    /// Complete component identifier of `{}`, XTypes 1.3 \u{a7}7.3.4.9.3.\n    \
                     pub const {}: zerodds_types::TypeIdentifier =\n        \
                     zerodds_types::TypeIdentifier::StronglyConnectedComponent(\n            \
                     zerodds_types::StronglyConnectedComponentId {{\n                \
                     equivalence_kind: 0x{:02X},\n                hash: \
                     zerodds_types::EquivalenceHash([{}]),\n                scc_length: {},\n                \
                     scc_index: {},\n            }});\n",
                    node.name,
                    constant_name(&node.name),
                    scc.equivalence_kind,
                    octets.join(", "),
                    scc.scc_length,
                    scc.scc_index
                );
            }
            other => {
                let _ = writeln!(
                    body,
                    "    // `{}`: {other:?} is not an equivalence hash and this module \
                     does not spell it as a constant yet.",
                    node.name
                );
            }
        }
    }
    if body.is_empty() {
        return String::new();
    }
    format!(
        "\n#[allow(dead_code, non_upper_case_globals)]\npub mod type_identifiers_{} {{\n{body}}}\n",
        constant_name(stem).to_lowercase()
    )
}

/// The type-object module: one XCDR2-LE minimal `TypeObject` per named
/// type, as a byte slice.
///
/// **Named after the unit**, `type_objects_robot` and not
/// `type_objects`. Two `.idl` files pulled into one scope with
/// `include!` is the ordinary way to use this crate, and a fixed name
/// makes the second one `error[E0428]: the name `type_objects` is
/// defined multiple times` — reported as issue #25, fixed once, and
/// reintroduced here when this module was rebuilt. The stem is what
/// distinguishes them, because it is what already distinguishes the two
/// output files.
///
/// A type whose object cannot be built is left out rather than emitted
/// empty. An empty blob is a `TypeObject` that resolves to nothing, and
/// a peer that received one would report a type mismatch instead of a
/// missing type — the second is true and the first is not.
fn type_objects_module(ast: &Ast, stem: &str) -> String {
    use core::fmt::Write as _;

    let mut body = String::new();
    for (id, node) in ast.types() {
        if node.name.is_empty() || !has_codec(&node.kind) {
            continue;
        }
        let Ok(object) = zerodds_types::from_ast::minimal_type_object(ast, id) else {
            continue;
        };
        let Ok(bytes) = zerodds_types::type_object::TypeObject::Minimal(object).to_bytes_le()
        else {
            continue;
        };
        let octets: Vec<String> = bytes.iter().map(|b| format!("0x{b:02x}")).collect();
        let _ = write!(
            body,
            "    /// Minimal TypeObject for `{}`, XCDR2-LE.\n    pub const {}: &[u8] = &[{}];\n",
            node.name,
            constant_name(&node.name),
            octets.join(", ")
        );
    }
    if body.is_empty() {
        return String::new();
    }
    format!(
        "\n#[allow(dead_code, non_upper_case_globals)]\npub mod type_objects_{} {{\n{body}}}\n",
        constant_name(stem).to_lowercase()
    )
}

/// A type name as a Rust constant name.
///
/// The source case is kept rather than shouted: `Robot_sPose` and
/// `ROBOT_S_POSE` are not the same name to a reader looking for the type
/// they wrote, and the module allows `non_upper_case_globals` for that
/// reason.
fn constant_name(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

/// Whether a type gets a generated codec.
///
/// The kinds that carry members or values. A `typedef` and a bare
/// `sequence` do not: what they alias does.
pub(crate) const fn has_codec(kind: &TypeKind) -> bool {
    matches!(
        kind,
        TypeKind::Structure { .. }
            | TypeKind::Union { .. }
            | TypeKind::Enumeration { .. }
            | TypeKind::Bitmask { .. }
            | TypeKind::Bitset { .. }
    )
}

/// The encoding a type's own extensibility asks for, under XCDR1.
fn xcdr1_encoding_for(ast: &Ast, id: Id<TypeNode>) -> Encoding {
    dds_type::xcdr1_encoding(ast.effective_extensibility(id))
}

/// The encoding a type's own extensibility asks for, under XCDR2.
pub(crate) fn encoding_for(ast: &Ast, id: Id<TypeNode>) -> Encoding {
    match ast.effective_extensibility(id) {
        Extensibility::Final => Encoding::PlainCdr2,
        Extensibility::Appendable => Encoding::DelimitedCdr,
        Extensibility::Mutable => Encoding::PlCdr2,
    }
}

/// A diagnostic as `file:line:column: message`.
///
/// The file is the one the diagnostic names, or `unit` when it names
/// none; line and column are read from that file as it is on disk, and
/// left out when it cannot be read.
fn located(diagnostic: &zerodds_idl_parse::Diagnostic, unit: Option<&Path>) -> String {
    let path = match (diagnostic.file.as_deref(), unit) {
        (Some(file), _) => PathBuf::from(file),
        (None, Some(unit)) => unit.to_path_buf(),
        (None, None) => return diagnostic.message.clone(),
    };
    let place = std::fs::read_to_string(&path).ok().map(|text| {
        let offset = (diagnostic.primary.start as usize).min(text.len());
        let before = text.get(..offset).unwrap_or(&text);
        let line = before.matches('\n').count() + 1;
        let column = before
            .rsplit('\n')
            .next()
            .map_or(0, |last| last.chars().count())
            + 1;
        format!("{line}:{column}")
    });
    match place {
        Some(place) => format!("{}:{place}: {}", path.display(), diagnostic.message),
        None => format!("{}: {}", path.display(), diagnostic.message),
    }
}

/// Compile with the defaults — the one-liner form.
///
/// # Errors
/// As [`Config::compile`].
pub fn compile(idls: &[impl AsRef<Path>]) -> Result<(), Error> {
    Config::new().compile(idls)
}

/// The runtime names a generated codec can reach for.
///
/// Kept as a list rather than derived from the emitter, because the
/// emitter names them in the expressions it builds and has no place to
/// report them from. A name added there and not here would compile
/// (`use super::*` reaches nothing that is not imported, so it would be
/// an unresolved-name error, loudly) — the failure mode is the safe
/// direction.
///
/// `ParameterRole` was the one that took that direction. The `@mutable`
/// codec writes every member behind `Writer::write_pid(id, role, must_understand)`
/// (XTypes 1.3 §7.4.3.4.1 — the EMHEADER's member id and flags), so the
/// emitter names it, and it was not in this list. The consequence was
/// not a stray warning: no `@mutable` struct compiled through
/// `zerodds-build` at all, because the generated file named a type it
/// had not imported. `interop/shapes/omg-dds-xtypes.idl` is the first
/// IDL in this tree to carry one, and it found it.
const RUNTIME_IMPORTS: [&str; 6] = [
    "Narrowed",
    "ParameterRole",
    "Reader",
    "TryConstruct",
    "WireError",
    "Writer",
];

/// Does `haystack` mention `word` as a word of its own?
///
/// A substring test would count `Reader` inside `ReaderProxy` and keep
/// an import nothing uses. Rust identifiers are ASCII alphanumeric plus
/// `_`, so the neighbours decide it.
fn mentions_word(haystack: &str, word: &str) -> bool {
    let is_ident = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let bytes = haystack.as_bytes();
    let mut from = 0;
    while let Some(found) = haystack[from..].find(word) {
        let at = from + found;
        let before_ok = at == 0 || !is_ident(char::from(bytes[at - 1]));
        let after = at + word.len();
        let after_ok = after >= bytes.len() || !is_ident(char::from(bytes[after]));
        if before_ok && after_ok {
            return true;
        }
        from = at + 1;
    }
    false
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("zerodds-build-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        dir
    }

    #[test]
    fn one_unit_becomes_one_module_named_after_it() {
        let dir = scratch("basic");
        let idl = dir.join("Robot.idl");
        std::fs::write(
            &idl,
            "module robot { @appendable struct Pose { @key long id; double x; }; };",
        )
        .expect("write idl");

        Config::new()
            .out_dir(&dir)
            .emit_cargo_directives(false)
            .compile(&[&idl])
            .expect("compile");

        let generated = std::fs::read_to_string(dir.join("Robot.rs")).expect("the module");
        assert!(generated.contains("pub mod robot"), "{generated}");
        assert!(generated.contains("pub struct Pose"), "{generated}");
        assert!(generated.contains("fn encode"), "{generated}");
        assert!(generated.contains("use super::*"), "{generated}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// DDS-XTypes 1.3 §7.3.1.1.1: constructs outside the building blocks
    /// are ignored or warned about "rather than halting with an error". A
    /// struct that holds an `any` has no type identifier and no wire form;
    /// the rest of the unit still builds.
    #[test]
    fn a_struct_holding_an_any_is_left_out_and_the_rest_of_the_unit_builds() {
        let dir = scratch("any");
        let idl = dir.join("Mixed.idl");
        std::fs::write(
            &idl,
            "module m { struct Plain { long a; }; struct Holds { any a; }; \
             struct Deep { Holds h; }; struct Beside { Plain p; string s; }; };",
        )
        .expect("write idl");

        Config::new()
            .out_dir(&dir)
            .emit_cargo_directives(false)
            .compile(&[&idl])
            .expect("a unit with an `any` member builds");

        let generated = std::fs::read_to_string(dir.join("Mixed.rs")).expect("the module");
        assert!(generated.contains("pub struct Plain"), "{generated}");
        assert!(generated.contains("pub struct Beside"), "{generated}");
        assert!(!generated.contains("Holds"), "{generated}");
        assert!(!generated.contains("Deep"), "{generated}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A unit that does not compose fails the build and carries every
    /// error, not the first.
    #[test]
    fn a_broken_unit_fails_and_names_what_is_wrong() {
        let dir = scratch("broken");
        let idl = dir.join("Bad.idl");
        std::fs::write(&idl, "module m { struct S { long ; }; };").expect("write idl");

        let Err(error) = Config::new()
            .out_dir(&dir)
            .emit_cargo_directives(false)
            .compile(&[&idl])
        else {
            panic!("a unit with a syntax error must not compile")
        };
        assert!(matches!(error, Error::Compose { .. }), "{error}");
        assert!(!dir.join("Bad.rs").exists(), "nothing was written");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The default fills silence and does not overrule the source.
    #[test]
    fn the_default_extensibility_reaches_the_generated_codec() {
        let dir = scratch("ext");
        let idl = dir.join("Ext.idl");
        std::fs::write(&idl, "module m { struct Silent { long a; }; };").expect("write idl");

        Config::new()
            .out_dir(&dir)
            .emit_cargo_directives(false)
            .default_extensibility(DefaultExt::Final)
            .compile(&[&idl])
            .expect("compile");
        let with_final = std::fs::read_to_string(dir.join("Ext.rs")).expect("the module");

        Config::new()
            .out_dir(&dir)
            .emit_cargo_directives(false)
            .compile(&[&idl])
            .expect("compile");
        let with_default = std::fs::read_to_string(dir.join("Ext.rs")).expect("the module");

        assert_ne!(
            with_final, with_default,
            "@final and the specification's @appendable are different wire plans"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// An `#include` resolves against the search list, and the header
    /// comment reaches the file.
    #[test]
    fn an_include_dir_is_searched_and_the_comment_is_written() {
        let dir = scratch("include");
        let headers = dir.join("headers");
        std::fs::create_dir_all(&headers).expect("headers");
        std::fs::write(headers.join("common.idl"), "struct Shared { long a; };").expect("common");
        let idl = dir.join("Main.idl");
        std::fs::write(&idl, "#include \"common.idl\"\nstruct Main { long b; };").expect("main");

        Config::new()
            .out_dir(&dir)
            .include_dir(&headers)
            .header_comment("built for the test")
            .emit_cargo_directives(false)
            .compile(&[&idl])
            .expect("compile");

        let generated = std::fs::read_to_string(dir.join("Main.rs")).expect("the module");
        assert!(generated.contains("Shared"), "the include came in");
        assert!(generated.contains("built for the test"), "{generated}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn without_out_dir_and_without_cargo_it_says_so() {
        // `OUT_DIR` is set while the test suite itself is built by cargo,
        // so this asserts the mapping rather than the environment.
        let error = Error::NoOutDir;
        assert!(error.to_string().contains("OUT_DIR"), "{error}");
    }
}
