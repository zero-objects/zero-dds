// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-wireprobe-gen`. Safety classification: **TOOLING**.
//!
//! `wireprobe-zerodds-gen` — the stem the five codec-column generators
//! are renderers on.
//!
//! `crates/lang-rules/src/emit/` is one walk with six backends, and no
//! backend decides a wire question there: a defect in the walk is one
//! defect, visible in every column at once. The generators around that
//! walk were five copies —
//! `tools/wireprobe/wireprobe-zerodds-{c,cpp,csharp,java,typescript}-gen`,
//! 4 470, 2 027, 2 101, 2 034 and 1 206 lines of source, the four newer
//! ones parallel down to the function order in their `fill.rs` and
//! `dump.rs` — and a question answered five times is a question answered
//! five ways. This holds the answers once:
//!
//! * the command line, including every policy axis a `LayoutKey` has;
//! * the IDL file, parsed, lowered and resolved;
//! * the top-level type, by scoped name;
//! * the layout key the dialect and the axes add up to;
//! * the sample, read from `values.json`;
//! * the files, written where `--out-dir` says;
//! * the exit codes of `docs/specs/zerodds-verdicts-1.0.md`.
//!
//! A [`Column`] carries the spelling of its language and nothing else.
//! The five binary names are unchanged, because the probes and the
//! sweeps call them by name.
//!
//! # The six answers this stem is where they live
//!
//! Generating every corpus case in every configuration through each of
//! the five and comparing — `tools/wireprobe/gen-source-snapshot`,
//! 11 280 configurations per column, 56 400 in all — turned up eight
//! places where two columns did the same thing differently. Two were
//! wire defects and were fixed where they lived (`--dialect` reaching
//! the lowering, the encapsulated form's padding and its count). The
//! other six produce no octets, so each was a verdict that would move
//! and none was a byte; they are settled here because here there is one
//! of them.
//!
//! **1. The exit codes are the verdict table's and nothing else.**
//! `docs/specs/zerodds-verdicts-1.0.md` fixes 1 for the command line, 3
//! for `no-form`, 6 for `harness-gap`, 7 for `generator-defect` and 8
//! for `mapping-divergence`. The C generator answered **1** for every
//! generation failure — the code that table gives the command line —
//! and C++, C# and Java answered **3** for everything, which claims the
//! target language has no form for a case their own walk simply stopped
//! on. Only TypeScript classified. [`Refusal`] is that classification
//! for all five, and [`Refusal::code`] is the table.
//!
//! **2. `--read` together with `--values` is refused.** C and C++
//! refused and said "pick one"; C# and Java took both and silently
//! generated the reader, so a caller that passed both was handed a
//! measurement of the direction it did not ask for. A silent drop is
//! the worst shape a defect takes here, so refusing is the answer and
//! [`parse`] is where it is refused.
//!
//! **3. A column with no read direction answers `harness-gap`, not "no
//! such option".** TypeScript answered "`--read` is not an option",
//! exit 2 — but `--read` *is* an option, four columns answer it, and
//! what TypeScript is missing is a `default_typescript` beside
//! `default_c.rs` and `default_java.rs`. That is a tool of ours that
//! has not been written, which is exit 6 by the table.
//! [`Column::read_direction`] is where a column says so and what it
//! says.
//!
//! **4. One reading of one IDL file, and it is the C column's.**
//! [`model_from_idl`] refuses on `parsed.has_errors()` and not on any
//! diagnostic, checks the resolve report, and [`find_type`] resolves
//! `--type` as the scoped name it is. Each of the three is argued at
//! its site.
//!
//! **5. A sample that does not fit is `no-form` where the language has
//! no carrier for it, and the runtime's business otherwise.** See
//! [`Refusal::NoForm`].
//!
//! **6. The fillers.** [`Column::render`] is handed the sample and each
//! renderer spells it; what a filler may decide is stated at
//! [`Column::render`].
#![forbid(unsafe_code)]
#![allow(clippy::print_stderr, clippy::print_stdout, clippy::exit)]

pub mod fill;

use std::collections::BTreeMap;
use std::path::PathBuf;
use zerodds_ast::{Ast, TypeNode};
use zerodds_ast_core::{Id, ScopedName};
use zerodds_ast_layout::{
    CollectionDelimiterPolicy, Dialect, Encoding, Endianness, EnumHolderWidth, EnumeratorValue,
    FixedDigitCount, LayoutKey, LengthCodePolicy, MapDelimiter, MapEntryOrder, MutableUnionFraming,
    PidFlagPolicy, Purpose, ShortPidRange, UnionLabelMatch, WcharFraming, WcharWidth,
    WstringLengthPolicy, WstringUnitOrder,
};
use zerodds_lang_rules::emit::encode::CodecOptions;

/// Why a generator could not produce what it was asked for.
///
/// Five kinds and not one string, because the exit code **is** the
/// verdict `docs/specs/zerodds-verdicts-1.0.md` records and a caller
/// cannot re-derive it from a message — "The classification is
/// structural and never read out of a message". A generator that
/// answered one code for all five would put "this language has no form
/// for it" and "our walk stopped" in one bucket, which is the
/// distinction the whole ledger turns on.
#[derive(Debug, Clone)]
pub enum Refusal {
    /// The specification itself does not admit the model, in the
    /// planner's own words. Exit 1 — `refused`, "it was asked and said
    /// no".
    ///
    /// A clause of IDL 4.2 is not a gap of this tree, and reporting it
    /// as exit 6 says "a tool of **ours** has not written it", which is
    /// a claim about this tree that is not true. Measured over
    /// `recursive-array` on 2026-08-26: ada and d answered 3, zig
    /// answered 1, and twelve columns answered 6 — one refusal in three
    /// classes.
    Refused(String),
    /// The command line. Exit 2.
    ///
    /// Exit 2 and not 1, because `tools/wireprobe/collect-generated`
    /// reads every code it does not name as `refused`, and 1 is not
    /// named there. A caller that typed the command wrong was being
    /// recorded as a column refusing the case — a harness fault
    /// entering the matrix as a measurement. The ten own-column
    /// generators already answered 2 here; this brings the five that
    /// share this type to them.
    Usage(String),
    /// The target language has no form for the construct. Exit 3.
    ///
    /// The materialiser's answer, or a carrier the language does not
    /// have. Both are statements about the language and neither is
    /// about the wire:
    ///
    /// * `primitive-wchar-above-bmp` holds U+1F600, and a C# `char` and
    ///   a Java `char` are one UTF-16 code unit, so those two columns
    ///   have no carrier for it. C's `wchar` carrier is a `uint16_t`
    ///   and its `char` carrier is a C `char`, so C has neither for
    ///   `primitive-wchar-above-bmp` nor for
    ///   `primitive-char-above-latin1`; C++ declares the same two
    ///   carriers (`char`, `std::uint16_t`, `adapter/cpp.rs`) and
    ///   answers the same.
    /// * TypeScript carries both in a `string`, so it does not refuse at
    ///   generation. What refuses there is §7.2.2.2.1.2.2 — a
    ///   CHAR_8_TYPE "shall be representable using 8 bits" — and that
    ///   is the *runtime* reading the clause, at run time, in its own
    ///   words. The generator does not pre-empt it: the wire question
    ///   belongs to the second implementation, which is the whole point
    ///   of a second column.
    ///
    /// So the same sample is `no-form` in the columns without a carrier
    /// and a run-time refusal in the others, and both are true
    /// statements about different things. What is *not* allowed is the third answer the five used
    /// to give between them: C answered 1 for a carrier it does not
    /// have, which reads as "the command line".
    NoForm(String),
    /// A tool of ours has not written it. Exit 6.
    ///
    /// The shared walk stopping, the sample not matching the model, the
    /// IDL not parsing, a direction or an emit form this column has not
    /// built — everything that is a gap in this tree rather than a
    /// property of the target language. It is also what an untyped
    /// error converts to, through [`From<String>`], because an error
    /// that has not been classified has not been shown to be about the
    /// language.
    Gap(String),
    /// The generator accepted the case and its own generated code does
    /// not build. Exit 7.
    ///
    /// Produced by the shell wrapper, which is the thing that runs the
    /// compiler; it is here so the code has one definition. Four of the
    /// five wrappers answered **5** for it, which is in no row of the
    /// table and lands as `refused` where `generator-defect` belongs.
    GeneratorDefect(String),
    /// The column carries the case and a documented mapping rule makes
    /// the octets differ. Exit 8.
    ///
    /// Not produced by any of these five: it is a verdict about a
    /// *foreign* mapping rule — CORBA's `long double` against
    /// §9.3.1.3's binary128 is the recorded case — and where a second
    /// reading of §7.4 differs from the Rust column's the answer is a
    /// byte difference the ledger shows, not an exit code. It is
    /// defined so that the table has one enumeration in the tree and
    /// not two.
    MappingDivergence(String),
}

impl Refusal {
    /// The exit code this refusal answers in.
    ///
    /// `docs/specs/zerodds-verdicts-1.0.md`, "The exit codes a probe
    /// answers in", and `tools/wireprobe/collect-generated` is what
    /// translates them.
    #[must_use]
    pub const fn code(&self) -> i32 {
        match self {
            Self::Refused(_) => 1,
            Self::Usage(_) => 2,
            Self::NoForm(_) => 3,
            Self::Gap(_) => 6,
            Self::GeneratorDefect(_) => 7,
            Self::MappingDivergence(_) => 8,
        }
    }

    /// The message, for standard error.
    #[must_use]
    pub fn message(&self) -> &str {
        match self {
            Self::Refused(text)
            | Self::Usage(text)
            | Self::NoForm(text)
            | Self::Gap(text)
            | Self::GeneratorDefect(text)
            | Self::MappingDivergence(text) => text,
        }
    }
}

impl std::fmt::Display for Refusal {
    /// The message, so a caller that only wants to print one can.
    ///
    /// The **code** is not in it: a verdict is read off
    /// [`Refusal::code`] and never out of a message.
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(self.message())
    }
}

impl From<String> for Refusal {
    /// An error that has not been classified is a gap in this tree.
    ///
    /// The safe default of the five: claiming `no-form` for an
    /// unclassified stop says the target language is missing something,
    /// which is a statement about somebody else's specification made on
    /// no evidence. `harness-gap` says a tool of ours has not been
    /// written, which is the thing that is certainly true when our walk
    /// stopped.
    fn from(text: String) -> Self {
        Self::Gap(text)
    }
}

/// A refusal that is a gap in this tree, from a format string.
///
/// The one-for-one replacement for `format!` inside an `Err`, so a
/// filler or a dumper written before the classification existed keeps
/// its parentheses and gains its code.
#[macro_export]
macro_rules! gap {
    ($($arg:tt)*) => { $crate::Refusal::Gap(::std::format!($($arg)*)) };
}

/// A refusal that is a statement about the target language, from a
/// format string.
///
/// Reach for this only where the *language* has no form — a carrier
/// that cannot hold the value, a construct the materialiser refuses.
/// [`Refusal::NoForm`] says what that means and what it does not.
#[macro_export]
macro_rules! no_form {
    ($($arg:tt)*) => { $crate::Refusal::NoForm(::std::format!($($arg)*)) };
}

/// What the compiled probe writes out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Emit {
    /// The serialised sample, with no encapsulation header.
    Payload,
    /// The sample behind its four-byte encapsulation header.
    PayloadWithEncapsulation,
    /// The sixteen-byte KeyHash.
    KeyHash,
}

impl Emit {
    /// The name this form goes by on the command line.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Payload => "payload",
            Self::PayloadWithEncapsulation => "payload+encapsulation",
            Self::KeyHash => "keyhash",
        }
    }
}

/// An option one column takes and the others do not.
///
/// There is one of them — TypeScript's `--runtime` — and it is here
/// rather than in a per-column parser so that the command line stays in
/// one place. A column that grows a second gets a second row and no new
/// parser.
#[derive(Debug, Clone, Copy)]
pub struct ExtraOption {
    /// The flag, with its dashes.
    pub flag: &'static str,
    /// What its value is called in the usage text.
    pub value: &'static str,
    /// The help text, already wrapped to the usage column.
    pub help: &'static [&'static str],
}

/// One generation, as the command line described it.
///
/// Every policy axis is here for every column, and that is a widening:
/// four of the five took `--dialect` and nothing else. It is sound
/// because none of these axes is answered by a renderer — each one
/// lands in the [`LayoutKey`] or in [`CodecOptions`], which is what the
/// shared walk and every harness read. A flag that did *not* reach the
/// renderer would be the silent-drop shape this suite exists to find,
/// so `axes_reach_the_generator` holds all five columns to it: if two
/// configurations put different octets on the wire for a case, the
/// generator has to emit different source for them.
#[derive(Debug, Clone)]
pub struct Args {
    /// The IDL source.
    pub idl: PathBuf,
    /// The top-level type, by scoped name.
    pub type_name: String,
    /// The sample to write. `None` in the read direction.
    pub values: Option<PathBuf>,
    /// Whether to generate the read direction.
    pub read: bool,
    /// XCDR1 or XCDR2, in one of its five forms.
    pub encoding: Encoding,
    /// Byte order.
    pub endianness: Endianness,
    /// What the compiled probe writes out.
    pub emit: Emit,
    /// The preset every wire policy is taken from.
    pub dialect: Dialect,
    /// Each of these overrides the dialect's answer, and only when it
    /// was given: a flag left off has to leave the dialect alone, which
    /// is the one thing a policy default must not do.
    pub length_codes: Option<LengthCodePolicy>,
    /// Which collections carry a delimiter.
    pub collection_delimiters: Option<CollectionDelimiterPolicy>,
    /// How a `wstring`'s length is counted.
    pub wstring_length: Option<WstringLengthPolicy>,
    /// How a scalar `wchar` is framed.
    pub wchar_framing: Option<WcharFraming>,
    /// How wide a scalar `wchar` is.
    pub wchar_width: Option<WcharWidth>,
    /// Which of a `map`'s halves decides whether the map is delimited.
    pub map_delimiter: Option<MapDelimiter>,
    /// In what byte order a `wstring`'s code units go out.
    pub wstring_unit_order: Option<WstringUnitOrder>,
    /// What flags a PL_CDR1 parameter id carries, per position.
    pub pid_flags: Option<PidFlagPolicy>,
    /// How wide an enumeration's holder is.
    pub enum_holder: Option<EnumHolderWidth>,
    /// Whether a negative case label of a narrow signed discriminator
    /// selects its branch.
    pub union_label: Option<UnionLabelMatch>,
    /// In what order a `map`'s entries go on the wire.
    pub map_entries: Option<MapEntryOrder>,
    /// Which integer an enumerator serialises as.
    pub enumerator_value: Option<EnumeratorValue>,
    /// How a `@mutable` union numbers its branches, and whether its
    /// XCDR1 parameter list is terminated.
    pub union_framing: Option<MutableUnionFraming>,
    /// Which member ids fit a PL_CDR1 short parameter header.
    pub short_pid_range: Option<ShortPidRange>,
    /// Whether a `fixed<P,S>` carries its declared digit count or the
    /// value's.
    pub fixed_digits: Option<FixedDigitCount>,
    /// Whether a delimited `map` also carries an entry count.
    pub map_length_with_delimiter: Option<bool>,
    /// The options one column takes and the others do not, by flag.
    pub extra: BTreeMap<String, String>,
    /// Where the generated files go.
    pub out_dir: PathBuf,
}

impl Default for Args {
    /// The defaults `wireprobe-zerodds` has, because a column that
    /// defaulted differently would report a divergence of its own
    /// making.
    fn default() -> Self {
        Self {
            idl: PathBuf::new(),
            type_name: String::new(),
            values: None,
            read: false,
            encoding: Encoding::PlainCdr2,
            endianness: Endianness::Little,
            emit: Emit::Payload,
            dialect: Dialect::Specification,
            length_codes: None,
            collection_delimiters: None,
            wstring_length: None,
            wchar_framing: None,
            wchar_width: None,
            map_delimiter: None,
            wstring_unit_order: None,
            pid_flags: None,
            enum_holder: None,
            union_label: None,
            map_entries: None,
            enumerator_value: None,
            union_framing: None,
            short_pid_range: None,
            fixed_digits: None,
            map_length_with_delimiter: None,
            extra: BTreeMap::new(),
            out_dir: PathBuf::new(),
        }
    }
}

impl Args {
    /// The value of a column-specific option, or a default.
    #[must_use]
    pub fn extra_or<'a>(&'a self, flag: &str, fallback: &'a str) -> &'a str {
        self.extra.get(flag).map_or(fallback, String::as_str)
    }

    /// Whether the encapsulation header goes out in front of the
    /// payload.
    #[must_use]
    pub fn encapsulation(&self) -> bool {
        self.emit == Emit::PayloadWithEncapsulation
    }

    /// The layout key one set of arguments plans under.
    ///
    /// The dialect first, then each axis that was given — the order
    /// `wireprobe-zerodds` applies them in, and now the only place any
    /// column applies them, so two columns cannot drift apart over one.
    #[must_use]
    pub fn layout_key(&self) -> LayoutKey {
        let mut key =
            LayoutKey::in_dialect(self.dialect, self.encoding, self.endianness, Purpose::Data);
        if let Some(policy) = self.length_codes {
            key = key.with_length_codes(policy);
        }
        if let Some(policy) = self.collection_delimiters {
            key = key.with_collection_delimiters(policy);
        }
        if let Some(policy) = self.wstring_length {
            key = key.with_wstring_length(policy);
        }
        if let Some(width) = self.wchar_width {
            key = key.with_wchar_width(width);
        }
        if let Some(policy) = self.map_delimiter {
            key = key.with_map_delimiter(policy);
        }
        if let Some(framing) = self.wchar_framing {
            key = key.with_wchar_framing(framing);
        }
        if let Some(order) = self.wstring_unit_order {
            key = key.with_wstring_unit_order(order);
        }
        if let Some(policy) = self.enum_holder {
            key = key.with_enum_holder(policy);
        }
        if let Some(policy) = self.union_label {
            key = key.with_union_label(policy);
        }
        if let Some(policy) = self.map_entries {
            key = key.with_map_entries(policy);
        }
        if let Some(policy) = self.enumerator_value {
            key = key.with_enumerator_value(policy);
        }
        if let Some(policy) = self.union_framing {
            key = key.with_union_framing(policy);
        }
        if let Some(policy) = self.short_pid_range {
            key = key.with_short_pid_range(policy);
        }
        if let Some(policy) = self.fixed_digits {
            key = key.with_fixed_digits(policy);
        }
        if let Some(policy) = self.pid_flags {
            key = key.with_pid_flags(policy);
        }
        key
    }

    /// What the shared walk is told beyond the key.
    ///
    /// The fallback is the dialect's answer, not a constant: OpenDDS
    /// omits a delimited `map`'s entry count (rule (15) as written,
    /// measured on its own rows), and a default of `true` here kept
    /// every generated column writing the count under `--dialect
    /// opendds` while the native probe had already moved — the
    /// `zerodds-c/opendds-xcdr2-*` map cells drifted by exactly those
    /// four octets. A `--map-length` flag still overrides.
    #[must_use]
    pub fn codec_options(&self) -> CodecOptions {
        CodecOptions {
            map_length_with_delimiter: self
                .map_length_with_delimiter
                .unwrap_or(!self.dialect.map_count_omitted_when_delimited()),
        }
    }
}

/// One file a column produced.
#[derive(Debug, Clone)]
pub struct File {
    /// Its name inside `--out-dir`.
    pub name: String,
    /// Its text.
    pub text: String,
}

impl File {
    /// A file, from a name and its text.
    pub fn new(name: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            text: text.into(),
        }
    }
}

/// Everything a renderer is handed: the model, the plan and the sample.
///
/// One structure and not seven parameters, so that a renderer which
/// needs a thing it was not given asks for it here and every renderer
/// gets it at once.
pub struct Session<'a> {
    /// What the command line said.
    pub args: &'a Args,
    /// The IDL file, parsed, lowered and resolved.
    pub ast: Ast,
    /// The top-level type.
    pub root: Id<TypeNode>,
    /// The payload's plan.
    pub key: LayoutKey,
    /// The key holder's plan: the same types with only their `@key`
    /// members, big-endian, §7.6.8.
    ///
    /// The holder keeps the dialect: §7.6.8 fixes the holder's byte
    /// order and encoding, not what an unannotated aggregate inside it
    /// is taken to be, and that decides whether the holder carries a
    /// DHEADER.
    pub holder_key: LayoutKey,
    /// What the shared walk is told beyond the key.
    pub codec_options: CodecOptions,
    /// The sample, where there is one. `None` in the read direction.
    pub sample: Option<serde_json::Value>,
}

impl Session<'_> {
    /// Read the IDL, lower it, find the type and read the sample.
    ///
    /// # Errors
    /// [`Refusal`], classified so the command can answer in the
    /// verdict's own exit code.
    pub fn open(args: &Args) -> Result<Session<'_>, Refusal> {
        let source = std::fs::read_to_string(&args.idl)
            .map_err(|error| gap!("{}: {error}", args.idl.display()))?;
        let ast = model_from_idl(&source, &args.idl.display().to_string(), args.dialect)?;
        bounded_alias_chains(&ast)?;
        let root = find_type(&ast, &args.type_name)?;

        // Read only where there is one: the read direction has no
        // sample and the file would not exist.
        let sample = match &args.values {
            Some(path) => {
                let text = std::fs::read_to_string(path)
                    .map_err(|error| gap!("{}: {error}", path.display()))?;
                Some(
                    serde_json::from_str::<serde_json::Value>(&text)
                        .map_err(|error| gap!("{}: {error}", path.display()))?,
                )
            }
            None => None,
        };

        let key = args.layout_key();
        Ok(Session {
            args,
            ast,
            root,
            key,
            holder_key: LayoutKey {
                endianness: Endianness::Big,
                purpose: Purpose::Key,
                ..key
            },
            codec_options: args.codec_options(),
            sample,
        })
    }

    /// The sample, in the write direction.
    ///
    /// # Errors
    /// [`Refusal::Gap`] where there is none, which is a caller that
    /// asked a write-direction renderer for the read direction.
    pub fn sample(&self) -> Result<&serde_json::Value, Refusal> {
        self.sample
            .as_ref()
            .ok_or_else(|| gap!("--values is required in the write direction"))
    }

    /// The four octets of an encapsulation header for this
    /// configuration.
    ///
    /// DDSI-RTPS 2.5 §10.5 and XTypes 1.3 §7.4.1.1. The options field —
    /// the last two octets — is zero here: §7.6.3.1.2 puts the count of
    /// the trailing padding in the low two bits of the fourth, and the
    /// payload's length is not known until it is written, so every
    /// harness ors it in at run time.
    #[must_use]
    pub const fn encapsulation_header(&self) -> [u8; 4] {
        encapsulation_header(self.args.encoding, self.args.endianness)
    }
}

/// The four octets of an encapsulation header.
///
/// DDSI-RTPS 2.5 §10.5 and XTypes 1.3 §7.4.1.1.
#[must_use]
pub const fn encapsulation_header(encoding: Encoding, endianness: Endianness) -> [u8; 4] {
    let identifier: u16 = match (encoding, endianness) {
        (Encoding::PlainCdr1, Endianness::Big) => 0x0000,
        (Encoding::PlainCdr1, Endianness::Little) => 0x0001,
        (Encoding::PlCdr1, Endianness::Big) => 0x0002,
        (Encoding::PlCdr1, Endianness::Little) => 0x0003,
        (Encoding::PlainCdr2, Endianness::Big) => 0x0006,
        (Encoding::PlainCdr2, Endianness::Little) => 0x0007,
        (Encoding::DelimitedCdr, Endianness::Big) => 0x0008,
        (Encoding::DelimitedCdr, Endianness::Little) => 0x0009,
        (Encoding::PlCdr2, Endianness::Big) => 0x000a,
        (Encoding::PlCdr2, Endianness::Little) => 0x000b,
    };
    [
        #[allow(clippy::cast_possible_truncation)]
        ((identifier >> 8) as u8),
        #[allow(clippy::cast_possible_truncation)]
        (identifier as u8),
        0,
        0,
    ]
}

/// One language's spelling, and nothing else.
///
/// Everything a column used to hold that was not spelling — the command
/// line, the IDL, the model, the layout key, the sample, the files, the
/// exit codes — is above this trait, so there is one of each.
pub trait Column {
    /// The binary this column is called by. Probes and sweeps call it
    /// by name, so it does not move.
    const BINARY: &'static str;
    /// The language, for the usage text and for a refusal that names
    /// it.
    const LANGUAGE: &'static str;
    /// What one generation writes, for the usage text.
    const OUTPUT: &'static str;
    /// The emit forms this column has a harness for.
    ///
    /// A form outside this list is not "not an option" — every column
    /// takes the same `--emit` and this is which of them it has
    /// *written*, which is [`Refusal::Gap`].
    const FORMS: &'static [Emit];
    /// The options this column takes and the others do not.
    const EXTRA: &'static [ExtraOption] = &[];

    /// Whether this column has a read direction, and what is missing
    /// when it has not.
    ///
    /// # Errors
    /// [`Refusal::Gap`] naming the tool of ours that would have to be
    /// written. Never [`Refusal::Usage`]: `--read` is an option of this
    /// command line and four columns answer it, so a column that has
    /// not built it has a gap and not a typo in front of it.
    fn read_direction() -> Result<(), Refusal> {
        Ok(())
    }

    /// The files one generation produces.
    ///
    /// # What a renderer may decide
    ///
    /// Its language's spelling, and nothing else. In particular a
    /// filler may not compute a wire value: an enumerator's integer, a
    /// bitmask's bit pattern and a `fixed`'s digit count are the
    /// model's answers and the runtime's, and a filler that computed
    /// one would be a second implementation of it that the octets
    /// cannot tell apart from the first. Where the target language has
    /// a name for the value — a generated constant, an enumerator — the
    /// filler writes the **name** and the generated declaration carries
    /// the number, so a wrong number shows up as a wrong byte and not
    /// as agreement between two copies of the same mistake.
    ///
    /// # Errors
    /// [`Refusal`], classified.
    fn render(session: &Session<'_>) -> Result<Vec<File>, Refusal>;

    /// The one translation unit that carries every emit form and both
    /// directions, chosen at run time.
    ///
    /// The C column has one and the batch driver behind
    /// `wireprobe-c-matrix` is built on it: on macOS the *first*
    /// execution of a freshly linked binary costs about 235 ms of
    /// signature validation against 2 ms for every one after, so the
    /// binary count and not the compiler is what a corpus sweep costs,
    /// and one program per case instead of eight is the difference.
    ///
    /// It is on the trait and not beside the C renderer because it is
    /// the same generation with a different `main`: the model, the
    /// plan, the sample and the codec sets are the stem's either way,
    /// and a renderer that reached around the stem to build them a
    /// second time is how the five columns came to disagree in the
    /// first place.
    ///
    /// # Errors
    /// [`Refusal::Gap`] where a column has not written one — which is a
    /// tool of ours that is missing, not a property of the language.
    fn combined(_session: &Session<'_>, _with_read: bool) -> Result<Vec<File>, Refusal> {
        Err(gap!(
            "{}: the {} column has no combined translation unit",
            Self::BINARY,
            Self::LANGUAGE
        ))
    }
}

/// The usage text of one column.
#[must_use]
pub fn usage<C: Column>() -> String {
    let mut out = format!(
        "{} — the {} the ZeroDDS {} probe compiles\n\n\
         \x20   --idl <file>            the IDL source (required)\n\
         \x20   --type <ScopedName>     the top-level type, by scoped name\n\
         \x20                           (required)\n\
         \x20   --values <file>         the sample, as JSON (required, unless\n\
         \x20                           --read)\n\
         \x20   --read                  generate the read direction instead: the\n\
         \x20                           payload arrives on the probe's standard\n\
         \x20                           input and the values go to its standard\n\
         \x20                           output, as the canonical JSON of\n\
         \x20                           crates/wire-probe/src/canonical.rs\n\
         \x20   --encoding <name>       xcdr1 | xcdr2 | plain-cdr1 | pl-cdr1 |\n\
         \x20                           plain-cdr2 | delimited-cdr | pl-cdr2\n\
         \x20   --endian le|be\n\
         \x20   --emit <form>           ",
        C::BINARY,
        C::LANGUAGE,
        C::LANGUAGE,
    );
    let forms: Vec<&str> = C::FORMS.iter().map(|form| form.as_str()).collect();
    out.push_str(&forms.join(" | "));
    out.push('\n');
    out.push_str(
        "    --length-codes <name>   compact | universal-nextint\n\
         \x20   --collection-delimiters <name>\n\
         \x20                           specification | enum-is-primitive | never\n\
         \x20   --wchar-framing <name>  fixed | giop-length-prefixed\n\
         \x20   --wchar-width <2|4>     how wide a scalar `wchar` is, which is not\n\
         \x20                           the width of a `wstring`'s code units\n\
         \x20   --map-delimiter <name>  either-half | value-only\n\
         \x20   --pid-flags <name>      sentinel-must-understand | bare |\n\
         \x20                           impl-extension-members\n\
         \x20   --enum-holder <name>    declared | bit-bound-dropped\n\
         \x20   --union-labels <name>   sign-extended | negative-unmatched-below-32\n\
         \x20   --enumerator-value <name>\n\
         \x20                           declared | ordinal\n\
         \x20   --union-framing <name>  specification | branches-from-zero\n\
         \x20   --short-pid-range <name>\n\
         \x20                           up-to-reserved | all-fourteen-bits\n\
         \x20   --map-entries <name>    declaration | ascending-key\n\
         \x20   --fixed-digits <name>   declared | value-precision\n\
         \x20   --wstring-units <name>  stream | big-endian-always | byte-order-mark\n\
         \x20   --wstring-length <name> specification | character-count |\n\
         \x20                           character-count-terminated\n\
         \x20   --map-length <name>     sequence-equivalent | rule-15-omits\n\
         \x20   --dialect <name>        specification (the default) | cyclone | rti |\n\
         \x20                           fastdds | opendds | ros-fastrtps |\n\
         \x20                           ros-cyclone | omniorb | tao | jacorb\n",
    );
    for option in C::EXTRA {
        out.push_str(&format!("    {} {}", option.flag, option.value));
        let width = 4 + option.flag.len() + 1 + option.value.len();
        let mut column = width;
        for line in option.help {
            if column >= 28 {
                out.push('\n');
                column = 0;
            }
            out.push_str(&" ".repeat(28 - column));
            out.push_str(line);
            out.push('\n');
            column = 0;
        }
    }
    out.push_str(&format!(
        "    --out-dir <dir>         where {} go (required)\n\n\
         A dialect sets every wire question the implementations answer\n\
         differently at once, and each policy flag overrides one of its\n\
         answers. The defaults are `wireprobe-zerodds`'s, because a column\n\
         that defaulted differently would report a divergence of its own\n\
         making.\n",
        C::OUTPUT
    ));
    out
}

/// Read a command line into [`Args`].
///
/// The whole `argv`, including the program name.
///
/// # Errors
/// [`Refusal::Usage`], naming the option — except where a well-formed
/// command line asks a column for something it has not built, which is
/// [`Refusal::Gap`] and not a typo.
///
/// zerodds-lint: recursion-depth 1 — this does not recurse. What the
/// body calls is `Encoding::parse`, `Dialect::parse` and their siblings,
/// inherent functions on other types; `dds_bounded_recursion` compares only
/// the last segment of a call path, so they read as self-calls.
#[allow(clippy::too_many_lines)]
pub fn parse<C: Column>(argv: Vec<String>) -> Result<Args, Refusal> {
    let mut args = Args::default();
    let mut idl = None;
    let mut type_name = None;
    let mut out_dir = None;

    let mut rest = argv.into_iter().skip(1);
    while let Some(flag) = rest.next() {
        let mut next = || {
            rest.next()
                .ok_or_else(|| Refusal::Usage(format!("`{flag}` takes a value")))
        };
        match flag.as_str() {
            // A help request is not a refusal: it goes to standard
            // output and the process ends well. Four of the five
            // columns printed it to standard error and exited 2, which
            // tells a caller that asked for help that it got something
            // wrong.
            "--help" | "-h" => {
                println!("{}", usage::<C>());
                std::process::exit(0);
            }
            "--idl" => idl = Some(PathBuf::from(next()?)),
            "--type" => type_name = Some(next()?),
            "--values" => args.values = Some(PathBuf::from(next()?)),
            "--read" => args.read = true,
            "--out-dir" => out_dir = Some(PathBuf::from(next()?)),
            "--encoding" => {
                let name = next()?;
                args.encoding = match name.as_str() {
                    "xcdr1" | "plain-cdr1" => Encoding::PlainCdr1,
                    "pl-cdr1" => Encoding::PlCdr1,
                    "xcdr2" | "plain-cdr2" => Encoding::PlainCdr2,
                    "delimited-cdr" => Encoding::DelimitedCdr,
                    "pl-cdr2" => Encoding::PlCdr2,
                    other => {
                        return Err(Refusal::Usage(format!("`{other}` is not an encoding")));
                    }
                };
            }
            "--endian" => {
                let name = next()?;
                args.endianness = match name.as_str() {
                    "le" | "little" => Endianness::Little,
                    "be" | "big" => Endianness::Big,
                    other => {
                        return Err(Refusal::Usage(format!("`{other}` is not a byte order")));
                    }
                };
            }
            // Every column reads the same three names. Which of them a
            // column has *written* is `C::FORMS`, and that is checked
            // below rather than here, because "this column has not
            // built the KeyHash form" and "`keyhsah` is not a form" are
            // different answers and only the second is the caller's
            // mistake.
            "--emit" => {
                let name = next()?;
                args.emit = match name.as_str() {
                    "payload" => Emit::Payload,
                    "payload+encapsulation" => Emit::PayloadWithEncapsulation,
                    "keyhash" => Emit::KeyHash,
                    other => {
                        return Err(Refusal::Usage(format!("`{other}` is not an emit form")));
                    }
                };
            }
            "--length-codes" => {
                let name = next()?;
                args.length_codes = Some(
                    LengthCodePolicy::parse(&name)
                        .ok_or_else(|| usage_value(&name, "a length-code policy"))?,
                );
            }
            "--collection-delimiters" => {
                let name = next()?;
                args.collection_delimiters = Some(
                    CollectionDelimiterPolicy::parse(&name)
                        .ok_or_else(|| usage_value(&name, "a delimiter policy"))?,
                );
            }
            "--pid-flags" => {
                let name = next()?;
                args.pid_flags = Some(
                    PidFlagPolicy::parse(&name)
                        .ok_or_else(|| usage_value(&name, "a PID-flag policy"))?,
                );
            }
            "--enum-holder" => {
                let name = next()?;
                args.enum_holder = Some(
                    EnumHolderWidth::parse(&name)
                        .ok_or_else(|| usage_value(&name, "an enum-holder width"))?,
                );
            }
            "--union-labels" => {
                let name = next()?;
                args.union_label = Some(
                    UnionLabelMatch::parse(&name)
                        .ok_or_else(|| usage_value(&name, "a union-label rule"))?,
                );
            }
            "--short-pid-range" => {
                let name = next()?;
                args.short_pid_range = Some(
                    ShortPidRange::parse(&name)
                        .ok_or_else(|| usage_value(&name, "a short-PID range"))?,
                );
            }
            "--union-framing" => {
                let name = next()?;
                args.union_framing = Some(
                    MutableUnionFraming::parse(&name)
                        .ok_or_else(|| usage_value(&name, "a union framing"))?,
                );
            }
            "--enumerator-value" => {
                let name = next()?;
                args.enumerator_value = Some(
                    EnumeratorValue::parse(&name)
                        .ok_or_else(|| usage_value(&name, "an enumerator-value policy"))?,
                );
            }
            "--map-entries" => {
                let name = next()?;
                args.map_entries = Some(
                    MapEntryOrder::parse(&name)
                        .ok_or_else(|| usage_value(&name, "a map-entry order"))?,
                );
            }
            "--fixed-digits" => {
                let name = next()?;
                args.fixed_digits = Some(
                    FixedDigitCount::parse(&name)
                        .ok_or_else(|| usage_value(&name, "a fixed digit count"))?,
                );
            }
            "--dialect" => {
                let name = next()?;
                args.dialect =
                    Dialect::parse(&name).ok_or_else(|| usage_value(&name, "a dialect"))?;
            }
            "--wchar-width" => {
                let name = next()?;
                args.wchar_width = Some(
                    WcharWidth::parse(&name).ok_or_else(|| usage_value(&name, "a wchar width"))?,
                );
            }
            "--map-delimiter" => {
                let name = next()?;
                args.map_delimiter = Some(
                    MapDelimiter::parse(&name)
                        .ok_or_else(|| usage_value(&name, "a map delimiter policy"))?,
                );
            }
            "--wchar-framing" => {
                let name = next()?;
                args.wchar_framing = Some(
                    WcharFraming::parse(&name)
                        .ok_or_else(|| usage_value(&name, "a wchar framing"))?,
                );
            }
            "--wstring-units" => {
                let name = next()?;
                args.wstring_unit_order = Some(
                    WstringUnitOrder::parse(&name)
                        .ok_or_else(|| usage_value(&name, "a wstring unit order"))?,
                );
            }
            "--wstring-length" => {
                let name = next()?;
                args.wstring_length = Some(
                    WstringLengthPolicy::parse(&name)
                        .ok_or_else(|| usage_value(&name, "a wstring-length policy"))?,
                );
            }
            "--map-length" => {
                let name = next()?;
                args.map_length_with_delimiter = Some(match name.as_str() {
                    "sequence-equivalent" => true,
                    "rule-15-omits" => false,
                    other => {
                        return Err(usage_value(other, "a map-length policy"));
                    }
                });
            }
            other => {
                if let Some(option) = C::EXTRA.iter().find(|option| option.flag == other) {
                    let value = next()?;
                    args.extra.insert(option.flag.to_string(), value);
                } else {
                    return Err(Refusal::Usage(format!("`{other}` is not an option")));
                }
            }
        }
    }

    // One direction or the other, never both: a run that both wrote
    // bytes and read some in would report one of the two and the caller
    // could not tell which. Two of the five took both and silently
    // generated the reader, which handed a caller a measurement of the
    // direction it did not ask for.
    if args.read && args.values.is_some() {
        return Err(Refusal::Usage(
            "--read reads bytes and --values writes them; pick one".to_string(),
        ));
    }
    if !args.read && args.values.is_none() {
        return Err(Refusal::Usage(
            "--values is required, or --read to generate the read direction".to_string(),
        ));
    }
    if args.read && args.emit == Emit::KeyHash {
        return Err(Refusal::Usage(
            "a KeyHash is a digest and holds no value to read back".to_string(),
        ));
    }
    // A well-formed command line asking for something this column has
    // not written: a gap in this tree, exit 6, and never exit 2. The
    // TypeScript column answered "`--read` is not an option" for a
    // direction four columns answer, which recorded a missing
    // `default_typescript` as a caller's typo.
    if args.read {
        C::read_direction()?;
    }
    if !C::FORMS.contains(&args.emit) {
        return Err(gap!(
            "{}: the {} column has no harness for `--emit {}`",
            C::BINARY,
            C::LANGUAGE,
            args.emit.as_str()
        ));
    }

    Ok(Args {
        idl: idl.ok_or_else(|| Refusal::Usage("--idl is required".to_string()))?,
        type_name: type_name.ok_or_else(|| Refusal::Usage("--type is required".to_string()))?,
        out_dir: out_dir.ok_or_else(|| Refusal::Usage("--out-dir is required".to_string()))?,
        ..args
    })
}

/// A value that is not one of the ones a flag takes.
fn usage_value(value: &str, what: &str) -> Refusal {
    Refusal::Usage(format!("`{value}` is not {what}"))
}

/// Generate and write what `--out-dir` says.
///
/// # Errors
/// [`Refusal`], classified, plus any write failure.
pub fn run<C: Column>(args: &Args) -> Result<(), Refusal> {
    let session = Session::open(args)?;
    let files = C::render(&session)?;
    write_all(args, &files)
}

/// Write a column's files under `--out-dir`.
///
/// # Errors
/// [`Refusal::Gap`] naming the path that could not be written.
pub fn write_all(args: &Args, files: &[File]) -> Result<(), Refusal> {
    std::fs::create_dir_all(&args.out_dir)
        .map_err(|error| gap!("{}: {error}", args.out_dir.display()))?;
    for file in files {
        let path = args.out_dir.join(&file.name);
        std::fs::write(&path, &file.text).map_err(|error| gap!("{}: {error}", path.display()))?;
    }
    Ok(())
}

/// The command, with the process's own argument vector.
///
/// # Panics
/// Never; it exits instead, in the code
/// `docs/specs/zerodds-verdicts-1.0.md` gives the verdict.
pub fn cli_main<C: Column>() {
    let args = match parse::<C>(std::env::args().collect()) {
        Ok(args) => args,
        Err(refusal) => {
            eprintln!("{}: {}", C::BINARY, refusal.message());
            std::process::exit(refusal.code());
        }
    };
    if let Err(refusal) = run::<C>(&args) {
        eprintln!("{}: {}", C::BINARY, refusal.message());
        std::process::exit(refusal.code());
    }
}

/// Parse and lower one IDL source into a model with everything
/// resolved.
///
/// # The three answers this settles
///
/// The five columns read one IDL file three ways, and the C column's
/// answer is the one kept in each case.
///
/// **A warning is not an error.** Four columns refused on
/// `!parsed.diagnostics().is_empty()`, which stops on a warning as well
/// — a corpus case that parses with a note about an unused declaration
/// would be recorded as one the column has no form for.
/// `parsed.has_errors()` is the parser's own predicate for the
/// distinction and is what this asks.
///
/// **The resolve report is checked.** `resolve_all_by_name` returns
/// `Ok` with a report that lists what it could not resolve, so four
/// columns that threw the report away carried on with a model holding
/// unresolved references and generated against it. A reference that
/// resolves to nothing is a member that is not written, which is the
/// silent-drop shape.
///
/// # Errors
/// [`Refusal::Gap`]: an IDL file this tree cannot read is a gap in this
/// tree, whatever the target language would have made of it.
pub fn model_from_idl(source: &str, path: &str, dialect: Dialect) -> Result<Ast, Refusal> {
    let parsed = zerodds_idl_parse::parse(source);
    if parsed.has_errors() {
        return Err(gap!("{path}: {:?}", parsed.diagnostics()));
    }
    // One front-end answer belongs to the dialect and not to the plan:
    // whether a `@hashid` member advances the sequential counter, which
    // §7.3.1.2.1.1 states twice and not the same way. RTI and OpenDDS
    // skip it and the others do not, and member ids are fixed here —
    // before a `LayoutKey` exists — so a generator that took
    // `--dialect` and lowered with the default numbers the members of
    // the wrong implementation.
    //
    // Measured on `annotation-hashid-implicit` under pl-cdr2,
    // little-endian, `--dialect rti`: with the answer passed on the
    // third member's id is `00000020` and without it `997bca20`, which
    // a peer reads as a different member or as none.
    let options = zerodds_idl_lower::Options {
        hashed_id_advances_the_counter: if dialect.hashed_id_advances_the_counter() {
            zerodds_idl_lower::HashedIdCounter::Advances
        } else {
            zerodds_idl_lower::HashedIdCounter::Skipped
        },
        // The second front-end answer of the same kind: which spelling
        // of an escaped name a member-id hash reads. RTI hashes
        // `_long` as written where the clause hashes `long`;
        // `model_from_idl_in_dialect` in `zerodds-wire-probe` passes
        // it and this copy of the lowering call did not, so every
        // generated column hashed the clause's ids under
        // `--dialect rti` — measured on `identifier-escaped-keyword`,
        // where the C column's parameter ids stayed 0x0364520F against
        // RTI's 0x04BF80B9.
        hashed_name_spelling: if dialect.hashid_reads_the_escaped_spelling() {
            zerodds_idl_lower::HashedNameSpelling::AsWritten
        } else {
            zerodds_idl_lower::HashedNameSpelling::Stripped
        },
        // The third: whether a declaration may carry the name of the scope
        // it sits in (IDL 4.2 §7.5.2). Fast DDS, RTI and both ROS
        // middlewares take `resolve-module-named-as-type` and write
        // octets; without the dialect's answer this copy refused the case
        // in every dialect, measured by `reference_columns.rs` on the C++
        // column as 140 slots where the Rust column wrote octets.
        name_of_the_enclosing_scope: if dialect.admits_the_name_of_the_enclosing_scope() {
            zerodds_idl_lower::NameOfTheEnclosingScope::Admitted
        } else {
            zerodds_idl_lower::NameOfTheEnclosingScope::Refused
        },
        ..zerodds_idl_lower::Options::default()
    };
    let lowered = zerodds_idl_lower::lower_with(&parsed.syntax(), path, options);
    if !lowered.diagnostics.is_empty() {
        // A clause of the specification refusing the model is the verdict
        // `refused` (exit 1), not a gap of this tree (exit 6).
        let text = format!("{path}: {:?}", lowered.diagnostics);
        return Err(
            if zerodds_idl_parse::refused_by_a_clause(&lowered.diagnostics) {
                Refusal::Refused(text)
            } else {
                Refusal::Gap(text)
            },
        );
    }
    let mut ast = lowered.ast;
    let report = ast.resolve_all_by_name().map_err(|error| gap!("{error}"))?;
    if !report.is_clean() {
        return Err(gap!(
            "{path}: unresolved after the resolve pass: {:?}",
            report.broken
        ));
    }
    Ok(ast)
}

/// How far an alias chain is followed before the model is taken to be
/// malformed.
///
/// The materialiser bounds its own spelling at the same number and IDL
/// nests nowhere near it.
pub const MAX_ALIAS_CHAIN: usize = 64;

/// Refuse a model whose aliases do not terminate.
///
/// # Why this is here and not in a filler
///
/// A filler recurses on the *sample*, which is finite — serde_json has
/// already parsed it, and a JSON document that nests without bound does
/// not get this far. The one place a filler can recurse without the
/// sample getting smaller is `TypeKind::Alias`, where the same value is
/// handed to the aliased type: a cycle among aliases loops for ever.
///
/// That is a property of the **model** and not of any language, so one
/// column having a depth counter and four not having one was not a
/// difference in spelling — it was four columns with a hole. The check
/// is the model's, it runs once, and it covers the fillers, the dumpers
/// and every walk any renderer writes later.
///
/// # Errors
/// [`Refusal::Gap`]: a model this tree cannot walk is a gap in this
/// tree, whatever a target language would have made of it.
pub fn bounded_alias_chains(ast: &Ast) -> Result<(), Refusal> {
    for (id, node) in ast.types() {
        let mut at = id;
        let mut followed = 0usize;
        while let Ok(current) = ast.type_(at) {
            let zerodds_ast::TypeKind::Alias { aliased } = &current.kind else {
                break;
            };
            let Some(next) = aliased.resolved() else {
                break;
            };
            at = next;
            followed += 1;
            if followed > MAX_ALIAS_CHAIN {
                return Err(gap!(
                    "`{}`: the alias chain is longer than {MAX_ALIAS_CHAIN} and does not terminate",
                    node.name.as_str()
                ));
            }
        }
    }
    Ok(())
}

/// The type `--type` names.
///
/// **The scoped name, resolved as one.** Four columns took the first
/// type whose *simple* name matched, which picks arbitrarily between
/// two types of one name in two modules — and the corpus has such
/// cases. `--type` is documented as a scoped name in every column's
/// usage text, and `find_by_scoped_name` is the model's own answer to
/// it.
///
/// # Errors
/// [`Refusal::Gap`]: a type this tree cannot find is a gap in this
/// tree.
pub fn find_type(ast: &Ast, name: &str) -> Result<Id<TypeNode>, Refusal> {
    let segments: Vec<String> = name
        .split("::")
        .filter(|segment| !segment.is_empty())
        .map(std::string::ToString::to_string)
        .collect();
    ast.find_by_scoped_name(&ScopedName::absolute(segments))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)
        .ok_or_else(|| gap!("`{name}` is not in the model"))
}
