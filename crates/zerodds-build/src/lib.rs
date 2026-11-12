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
//! ## A Rust type for an IDL type
//!
//! An IDL `typedef string UtcTimestamp;` is a `String` in the generated
//! Rust, and so is a text that is no timestamp. [`Config::rust_type`]
//! names a Rust type that stands for a typedef, a struct member or a union
//! branch, and the generated code holds that type instead:
//!
//! ```no_run
//! fn main() -> Result<(), zerodds_build::Error> {
//!     zerodds_build::Config::new()
//!         .rust_type("app::UtcTimestamp", "crate::time::UtcTimestamp")
//!         .rust_type("app::Track.id", "core::num::NonZeroU32")
//!         .compile(&["idl/app.idl"])
//! }
//! ```
//!
//! The Rust type implements `zerodds_wire::WireRepr` with `Idl` the IDL
//! type's Rust spelling (`str` for a `string`, `u32` for an `unsigned
//! long`); one from another crate goes through an adapter
//! ([`Config::rust_type_with`], `zerodds_wire::DisplayFromStr` for a
//! `Display + FromStr` type). The octets, the `TypeObject`, the
//! `TypeIdentifier` and the key hash stay the IDL's; a value the Rust type
//! refuses on the way in rejects the sample with that reason.
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
mod rust_type;

use zerodds_ast::facet::Extensibility;
use zerodds_ast::{Ast, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ast_layout::key::{Encoding, Endianness, LayoutKey, Purpose};
use zerodds_ast_layout::{AxisFlag, Dialect, DialectLowering, DialectSettings, PlCdr1Reading};
use zerodds_idl_compose::{ComposeOptions, PathLoader, compose_with};
use zerodds_lang_rules::adapter::cst::{
    RustEnumPrefixScope, RustMapFormScope, RustTypeScope, nest_by_module, rust_item_pairs,
};

/// How the generated Rust holds an IDL `map` ([`Config::rust_map`]).
pub use zerodds_lang_rules::adapter::cst::RustMapForm;
use zerodds_lang_rules::encode::{rust_codec, rust_codec_pair};
use zerodds_lang_rules::rule::rust;
/// The naming scheme of the generated Rust ([`Config::rust_naming`]).
pub use zerodds_lang_rules::rust_names::RustNaming;
use zerodds_lang_rules::rust_names::RustNamingScope;
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
    /// A [`Config::rust_type`] or [`Config::rust_type_with`] call that the
    /// IDL does not support: a path that names no typedef, struct member
    /// or union branch of the compiled units, a declaration whose IDL type
    /// no Rust type can stand for, or a Rust type that is no type path.
    RustType {
        /// The IDL path as given.
        idl_path: String,
        /// What is wrong with it.
        detail: String,
    },
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
            Self::RustType { idl_path, detail } => {
                write!(f, "rust_type(\"{idl_path}\"): {detail}")
            }
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
    strip_enum_prefix: bool,
    rust_naming: RustNaming,
    case_sensitive_identifiers: bool,
    rust_types: Vec<rust_type::RustTypeOverride>,
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
            strip_enum_prefix: false,
            rust_naming: RustNaming::Idl,
            case_sensitive_identifiers: false,
            rust_types: Vec::new(),
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
    ///
    /// The module is `type_objects_<unit>`, and inside it each constant
    /// sits in the IDL modules its type is declared in:
    /// `type_objects_robot::common::Pose` for `module common { struct
    /// Pose { … }; };` in `Robot.idl`. Two modules may each declare a type
    /// of the same simple name.
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
    /// modules, which are switched on and off on their own.
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

    /// Cut each enumeration's name off the front of its enumerators' Rust
    /// names, as `zerodds-idlc --strip-enum-prefix` does (GitHub #61).
    ///
    /// Off by default: the generated Rust keeps the IDL's names, so
    /// `enum MyEnum { MY_ENUM_A, MY_ENUM_B };` has the variants
    /// `MY_ENUM_A` and `MY_ENUM_B`. With `strip_enum_prefix(true)` they are
    /// `A` and `B`, like prost's default. The name matches without regard to
    /// case and underscores and ends at an `_` or a lower-to-upper case
    /// change; it is cut only where a valid identifier is left, not empty
    /// and not starting with a digit (`MY_ENUM_1` stays). Two enumerators
    /// that come out as one name stop the build.
    ///
    /// Only the Rust names change: the TypeObject, a content filter and
    /// the type matching read the IDL's, so the wire and what a type
    /// matches stay the same.
    pub fn strip_enum_prefix(&mut self, strip: bool) -> &mut Self {
        self.strip_enum_prefix = strip;
        self
    }

    /// Compare IDL identifiers with regard to case, as `zerodds-idlc
    /// --case-sensitive` and `idlc -f case-sensitive` of Cyclone DDS do
    /// (GitHub #66).
    ///
    /// Off by default: OMG IDL 4.2 §7.2.3 has two identifiers that differ
    /// only in case collide, and §7.2.4 has an identifier collide with a
    /// keyword it differs from only in case, so `module shape` beside
    /// `struct Shape` stops the build. With
    /// `case_sensitive_identifiers(true)` both comparisons are exact: the
    /// two declarations stand, `Struct` is an identifier, and a name is
    /// declared twice only when it is written the same way twice.
    ///
    /// A name was always resolved by its exact spelling, and the
    /// TypeObject, the member ids and the registered type names carry the
    /// spelling of the IDL either way; a file that builds without the
    /// switch gives the same code and the same octets with it. A file that
    /// needs the switch is not OMG IDL: Cyclone DDS reads it with its own
    /// switch, and a front end without one refuses it.
    pub fn case_sensitive_identifiers(&mut self, case_sensitive: bool) -> &mut Self {
        self.case_sensitive_identifiers = case_sensitive;
        self
    }

    /// The naming scheme of the generated Rust, as `zerodds-idlc
    /// --rust-naming` sets it (GitHub #61).
    ///
    /// [`RustNaming::Idl`] is the default: every name is the IDL's, as
    /// IDL4 to Java 1.0 and IDL4 to C# 1.0 §7.1.1 have it for their
    /// languages, and the module that includes the generated file takes
    /// `#[allow(non_snake_case, non_camel_case_types,
    /// non_upper_case_globals)]`. [`RustNaming::Rust`] writes the names
    /// after RFC 430: `UpperCamelCase` for types and variants, `snake_case`
    /// for fields and modules, `SCREAMING_SNAKE_CASE` for constants; an
    /// acronym is one word (`HTTPServer` is `HttpServer`). With
    /// [`Self::strip_enum_prefix`] the enumeration's name is cut first:
    /// `MY_ENUM_A` of `MyEnum` is `A`, and `MyEnumA` without it.
    ///
    /// `@rust_mapping(apply_naming_convention = RUST_NAMING_CONVENTION)`
    /// (or `IDL_NAMING_CONVENTION`) on a module or on a type, constant,
    /// exception or interface replaces the scheme for what that
    /// declaration encloses. Two declarations of one scope that come out
    /// as one name stop the build.
    ///
    /// Only the Rust names change: the TypeObject, member ids, a content
    /// filter, the type matching and the paths [`Self::rust_type`] takes
    /// read the IDL's, so the wire and what a type matches stay the same.
    pub fn rust_naming(&mut self, naming: RustNaming) -> &mut Self {
        self.rust_naming = naming;
        self
    }

    /// Hold an IDL declaration as another Rust type in the generated code.
    ///
    /// `idl_path` names a typedef (`app::UtcTimestamp`), a struct member
    /// (`app::Event.at`) or a union branch (`app::Choice.stamp`), by the
    /// IDL scoped name; `rust_type` is the Rust type that stands for it,
    /// as a path that resolves where the generated file is included —
    /// `crate::time::UtcTimestamp`, `core::num::NonZeroU32`. The type
    /// implements `zerodds_wire::WireRepr` with `Idl` the IDL type's
    /// Rust spelling: `str` for a `string`, `u32` for an `unsigned long`.
    ///
    /// ```no_run
    /// fn main() -> Result<(), zerodds_build::Error> {
    ///     zerodds_build::Config::new()
    ///         .rust_type("app::UtcTimestamp", "crate::time::UtcTimestamp")
    ///         .rust_type("app::Track.id", "core::num::NonZeroU32")
    ///         .compile(&["idl/app.idl"])
    /// }
    /// ```
    ///
    /// The generated struct holds the Rust type, in an `Option` for an
    /// `@optional` member and a `Box` for an `@external` one, and a typedef
    /// is an alias of it wherever it is used — a member, a sequence, array
    /// or map element, a union branch. Encode writes the IDL value through
    /// `to_wire`; decode reads it — a `string` lent out of the sample —
    /// and converts through `from_wire`, and a value the Rust type refuses
    /// rejects the sample with that reason. The `TypeObject`, the
    /// `TypeIdentifier`, assignability and the key hash are the IDL's, and
    /// a peer in another language reads the same octets.
    ///
    /// A member mapping wins over a typedef mapping of the member's type.
    /// The IDL type behind the declaration is a `string` or a primitive
    /// other than `wchar` and `long double`. A Rust type whose `Idl` is
    /// not that type does not compile: the generated file states every
    /// mapping as `zerodds_wire::repr::idl_type_is::<T, Idl>()`. The
    /// generated types derive `Debug`, `Clone` and `PartialEq`, so the
    /// Rust type implements them; a type whose default would need a value
    /// the Rust type refuses — a `NonZeroU32` for Table 9's zero — gets no
    /// `impl Default`, and a sample that leaves such a member out is
    /// rejected.
    ///
    /// A later call for the same path replaces an earlier one.
    ///
    /// # Errors
    /// [`Self::compile`] and [`Self::compile_bundle`] refuse, as
    /// [`Error::RustType`], a path that names no typedef, struct member
    /// or union branch of the units they compile, a declaration of
    /// another IDL type, and a Rust type that is no type path.
    pub fn rust_type(
        &mut self,
        idl_path: impl Into<String>,
        rust_type: impl Into<String>,
    ) -> &mut Self {
        self.push_rust_type(idl_path.into(), rust_type.into(), None)
    }

    /// [`Self::rust_type`] for a type that does not implement
    /// `zerodds_wire::WireRepr` itself — one from another crate — with
    /// `adapter` the path of a value that implements
    /// `zerodds_wire::WireReprWith` for it: a unit struct or a constant.
    /// `zerodds_wire::DisplayFromStr` is the adapter for any `Display +
    /// FromStr` type on an IDL `string`.
    ///
    /// ```no_run
    /// fn main() -> Result<(), zerodds_build::Error> {
    ///     zerodds_build::Config::new()
    ///         .rust_type_with(
    ///             "app::Track.source",
    ///             "std::net::Ipv4Addr",
    ///             "zerodds_wire::DisplayFromStr",
    ///         )
    ///         .compile(&["idl/app.idl"])
    /// }
    /// ```
    ///
    /// # Errors
    /// As [`Self::rust_type`], and an adapter that is no path.
    pub fn rust_type_with(
        &mut self,
        idl_path: impl Into<String>,
        rust_type: impl Into<String>,
        adapter: impl Into<String>,
    ) -> &mut Self {
        self.push_rust_type(idl_path.into(), rust_type.into(), Some(adapter.into()))
    }

    fn push_rust_type(
        &mut self,
        idl_path: String,
        rust_type: String,
        adapter: Option<String>,
    ) -> &mut Self {
        self.rust_types.retain(|given| given.idl_path != idl_path);
        self.rust_types.push(rust_type::RustTypeOverride {
            idl_path,
            rust_type,
            adapter,
        });
        self
    }

    /// The overrides of `rust_type` that no compiled unit declared, as
    /// [`Error::RustType`] for the first.
    fn every_rust_type_found(&self, found: &[bool]) -> Result<(), Error> {
        match self
            .rust_types
            .iter()
            .zip(found)
            .find(|(_, found)| !**found)
        {
            Some((given, _)) => Err(Error::RustType {
                idl_path: given.idl_path.clone(),
                detail: String::from(
                    "names no typedef, struct member or union branch of the compiled units \
                     (an IDL scoped name: `module::Type`, `module::Struct.member`)",
                ),
            }),
            None => Ok(()),
        }
    }

    /// The `rust_type` overrides one unit declares, resolved against its
    /// model.
    fn rust_types_of(&self, ast: &Ast) -> Result<rust_type::Resolved, Error> {
        rust_type::resolve(ast, &self.rust_types)
            .map_err(|(idl_path, detail)| Error::RustType { idl_path, detail })
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
                // `@key` on a union discriminator is carried (DDS-XTypes 1.3
                // Table 21, §7.2.2.4.4.4.8): the union is a keyed topic
                // type and its key holder is the discriminator.
                key_on_union_discriminator: zerodds_idl_compose::KeyOnUnionDiscriminator::Carried,
                // A build reads the IDL it was pointed at, which may be a
                // vendor's. Refusing an extension here would fail the
                // build over a portability question nobody asked;
                // `zerodds-idlc --strict` is where that question is put.
                dialect: {
                    let mut dialect = zerodds_idl_compose::ComposeOptions::default().dialect;
                    dialect.case_sensitive = self.case_sensitive_identifiers;
                    dialect
                },
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
        let rust_types = self.rust_types_of(&composed.ast)?;
        self.every_rust_type_found(&rust_types.found)?;
        let code = self.render_with_stem(&bundle, &composed.ast, name, rust_types)?;
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
        let mut found = vec![false; self.rust_types.len()];
        for idl in idls {
            for (any, here) in found
                .iter_mut()
                .zip(self.compile_one(idl.as_ref(), &out_dir)?)
            {
                *any |= here;
            }
        }
        // A `rust_type` path is checked against every unit of the call: one
        // unit of several need not declare it, but one of them has to.
        self.every_rust_type_found(&found)
    }

    /// Compile one unit, answering per `rust_type` override whether the
    /// unit declares its path.
    fn compile_one(&self, idl: &Path, out_dir: &Path) -> Result<Vec<bool>, Error> {
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
                // `@key` on a union discriminator is carried (DDS-XTypes 1.3
                // Table 21, §7.2.2.4.4.4.8): the union is a keyed topic
                // type and its key holder is the discriminator.
                key_on_union_discriminator: zerodds_idl_compose::KeyOnUnionDiscriminator::Carried,
                // A build reads the IDL it was pointed at, which may be a
                // vendor's. Refusing an extension here would fail the
                // build over a portability question nobody asked;
                // `zerodds-idlc --strict` is where that question is put.
                dialect: {
                    let mut dialect = zerodds_idl_compose::ComposeOptions::default().dialect;
                    dialect.case_sensitive = self.case_sensitive_identifiers;
                    dialect
                },
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
        let rust_types = self.rust_types_of(&composed.ast)?;
        let found = rust_types.found.clone();
        let code = self.render_with_stem(idl, &composed.ast, stem, rust_types)?;
        let target = out_dir.join(format!("{stem}.rs"));
        std::fs::write(&target, code).map_err(|error| Error::Write {
            path: target,
            detail: error.to_string(),
        })?;
        Ok(found)
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
    fn render_with_stem(
        &self,
        idl: &Path,
        ast: &Ast,
        stem: &str,
        rust_types: rust_type::Resolved,
    ) -> Result<String, Error> {
        let _map_form = RustMapFormScope::enter(self.rust_map);
        let _enum_prefix = RustEnumPrefixScope::enter(self.strip_enum_prefix);
        let _naming = RustNamingScope::enter(self.rust_naming);
        // Every walk below spells a type or a member's codec; the ones a
        // `rust_type` call named are held as the Rust type it gave.
        let _rust_types = RustTypeScope::enter(rust_types.mappings);
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
        // Each `rust_type` mapping, held to the IDL type it stands for: a
        // Rust type whose `WireRepr::Idl` is another type fails the build
        // here, at a line that names both.
        code.push_str(&rust_types.assertions);
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
/// two units in one scope must not collide. Inside it, a type's constant
/// sits in one `pub mod` per IDL module, as `type_objects_module`'s does.
///
/// A type whose identifier this module cannot spell as a constant —
/// today that is everything but an equivalence hash and a strongly
/// connected component — is **named in a comment** rather than left
/// out silently, so a reader who misses a constant finds the reason in
/// the same place the constant would be.
fn type_identifiers_module(ast: &Ast, stem: &str) -> String {
    let mut entries = Vec::new();
    for (id, node) in ast.types() {
        if node.name.is_empty() || !has_codec(&node.kind) {
            continue;
        }
        let Ok(identifier) = zerodds_types::from_ast::type_identifier(
            ast,
            id,
            zerodds_types::from_ast::Form::Complete,
        ) else {
            entries.push((
                module_segments(ast, id),
                format!(
                    "// `{}`: its identifier could not be computed from the model.\n",
                    node.name
                ),
            ));
            continue;
        };
        let text = match identifier {
            zerodds_types::TypeIdentifier::EquivalenceHashComplete(hash) => {
                let octets: Vec<String> = hash.0.iter().map(|b| format!("0x{b:02x}")).collect();
                format!(
                    "/// Complete equivalence hash of `{}`, XTypes 1.3 \u{a7}7.3.4.6.\n\
                     pub const {}: zerodds_types::TypeIdentifier =\n    \
                     zerodds_types::TypeIdentifier::EquivalenceHashComplete(\
                     zerodds_types::EquivalenceHash([{}]));\n",
                    node.name,
                    type_constant_name(ast, node),
                    octets.join(", ")
                )
            }
            // A type in a strongly connected component is identified by the
            // component's hash and its own position (§7.3.4.9.3).
            zerodds_types::TypeIdentifier::StronglyConnectedComponent(scc) => {
                let octets: Vec<String> = scc.hash.0.iter().map(|b| format!("0x{b:02x}")).collect();
                format!(
                    "/// Complete component identifier of `{}`, XTypes 1.3 \u{a7}7.3.4.9.3.\n\
                     pub const {}: zerodds_types::TypeIdentifier =\n    \
                     zerodds_types::TypeIdentifier::StronglyConnectedComponent(\n        \
                     zerodds_types::StronglyConnectedComponentId {{\n            \
                     equivalence_kind: 0x{:02X},\n            hash: \
                     zerodds_types::EquivalenceHash([{}]),\n            scc_length: {},\n            \
                     scc_index: {},\n        }});\n",
                    node.name,
                    type_constant_name(ast, node),
                    scc.equivalence_kind,
                    octets.join(", "),
                    scc.scc_length,
                    scc.scc_index
                )
            }
            other => format!(
                "// `{}`: {other:?} is not an equivalence hash and this module \
                 does not spell it as a constant yet.\n",
                node.name
            ),
        };
        entries.push((module_segments(ast, id), text));
    }
    if entries.is_empty() {
        return String::new();
    }
    format!(
        "\n#[allow(dead_code, non_upper_case_globals, non_snake_case)]\npub mod type_identifiers_{} {{\n{}}}\n",
        constant_name(stem).to_lowercase(),
        nested_by_module(&entries, 1)
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
/// **One `pub mod` per IDL module inside it**, `type_objects_robot::v1::Part`
/// and not `type_objects_robot::Part`. A constant is named after its
/// type's simple name, and `module v1 { struct Part … }; module v2 {
/// struct Part … };` is two types with one simple name: in one flat
/// module that was `error[E0428]` twice over, and a unit that declared
/// both did not compile at all. The IDL scope is what keeps the two
/// apart, so the module tree here is the IDL's, the same one the
/// generated types sit in.
///
/// A type whose object cannot be built is left out rather than emitted
/// empty. An empty blob is a `TypeObject` that resolves to nothing, and
/// a peer that received one would report a type mismatch instead of a
/// missing type — the second is true and the first is not.
fn type_objects_module(ast: &Ast, stem: &str) -> String {
    let mut entries = Vec::new();
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
        entries.push((
            module_segments(ast, id),
            format!(
                "/// Minimal TypeObject for `{}`, XCDR2-LE.\npub const {}: &[u8] = &[{}];\n",
                node.name,
                type_constant_name(ast, node),
                octets.join(", ")
            ),
        ));
    }
    if entries.is_empty() {
        return String::new();
    }
    format!(
        "\n#[allow(dead_code, non_upper_case_globals, non_snake_case)]\npub mod type_objects_{} {{\n{}}}\n",
        constant_name(stem).to_lowercase(),
        nested_by_module(&entries, 1)
    )
}

/// The IDL modules a type is declared in, outermost first, spelled as the
/// generated `pub mod`s are.
///
/// Empty for a type at file scope, and for one the model has no module
/// chain for: such a type cannot be named from the file root either, and
/// the constant beside the file-scope ones is the one place left for it.
fn module_segments(ast: &Ast, id: Id<TypeNode>) -> Vec<String> {
    zerodds_lang_rules::adapter::cst::path_from_file_root(ast, id)
        .map(|path| {
            let mut segments: Vec<String> = path.split("::").map(str::to_string).collect();
            segments.pop();
            segments
        })
        .unwrap_or_default()
}

/// Items grouped into one `pub mod` per module path, in the order the
/// first item of each module came, indented `depth` levels.
///
/// zerodds-lint: recursion-depth 64 (one level per IDL module; the module
/// chain is cut at `MAX_MODULE_DEPTH` in `lang-rules/src/adapter/cst.rs`,
/// which `module_segments` reads)
fn nested_by_module(entries: &[(Vec<String>, String)], depth: usize) -> String {
    use core::fmt::Write as _;

    let indent = "    ".repeat(depth);
    let mut out = String::new();
    // Items at this level first, then each module once, in first-seen order.
    for (segments, text) in entries {
        if segments.is_empty() {
            for line in text.lines() {
                out.push_str(&indent);
                out.push_str(line);
                out.push('\n');
            }
        }
    }
    let mut seen: Vec<&str> = Vec::new();
    for (segments, _) in entries {
        let Some(first) = segments.first() else {
            continue;
        };
        if seen.contains(&first.as_str()) {
            continue;
        }
        seen.push(first);
        let inner: Vec<(Vec<String>, String)> = entries
            .iter()
            .filter(|(other, _)| other.first() == Some(first))
            .map(|(other, text)| (other[1..].to_vec(), text.clone()))
            .collect();
        let _ = write!(
            out,
            "{indent}/// The IDL module `{first}`.\n{indent}pub mod {first} {{\n{}{indent}}}\n",
            nested_by_module(&inner, depth + 1)
        );
    }
    out
}

/// A type name as a Rust constant name.
///
/// The source case is kept rather than shouted: `Robot_sPose` and
/// `ROBOT_S_POSE` are not the same name to a reader looking for the type
/// they wrote, and the module allows `non_upper_case_globals` for that
/// reason.
/// [`constant_name`] for a constant named after a type: a type the IDL
/// calls `Self` or `type` gives `Self_` or `r#type`, as its Rust type does
/// (`zerodds_lang_cst::rust::escape_keyword`). Under the Rust naming
/// scheme of the type the constant is shouted like every other one
/// (`HTTPReading` is `HTTP_READING`).
fn type_constant_name(ast: &Ast, node: &TypeNode) -> String {
    let name = constant_name(&node.name);
    match zerodds_lang_rules::rust_names::of_type(ast, node) {
        RustNaming::Idl => zerodds_lang_cst::rust::escape_keyword(&name),
        RustNaming::Rust => zerodds_lang_cst::rust::escape_keyword(
            &zerodds_lang_rules::rust_names::screaming_snake_case(&name),
        ),
    }
}

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
    fn xtypes_7_3_1_1_1_a_struct_holding_an_any_is_left_out_and_the_rest_of_the_unit_builds() {
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
