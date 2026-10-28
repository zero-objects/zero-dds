// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! What each front end *said* about each case, which is not the same
//! thing as what it did with it.
//!
//! A vendor front end's warning is a statement, and it says one of
//! three things — our case is ambiguous, the vendor deviates
//! deliberately, or the specification leaves something open. All three
//! are findings, and two of this project's most valuable results came
//! out of one:
//!
//!  * Cyclone's `idlc` writes *"No default extensibility provided …
//!    Currently the default extensibility for these types is 'final',
//!    but this may change to 'appendable' in a future release because
//!    that is the default in the DDS XTypes specification"* — which is
//!    the root cause of issues #41 and #42, and is in no clause and in
//!    no byte;
//!  * `tao_idl` answers an annotation it does not know with `Warning …
//!    error in lookup of symbol`, and without that line TAO reads as a
//!    conforming implementation where in fact it parsed the annotation
//!    and dropped it.
//!
//! Every sweep in this tree used to discard them. `vendor-corpus-sweep`
//! keeps the first line of stderr and only when the probe failed, and
//! `wireprobe-batch` filters warnings out of its blame so that a
//! warning about one case cannot exclude it — which is right, and which
//! is also why no warning survived a run. A system whose bytes agree
//! while its front end warned about the type was therefore recorded as
//! agreeing silently, and those are not the same thing.
//!
//! ## The three parts, and which of them is generated
//!
//! `tools/interop-results/warning-sweep.sh` measures. It records every
//! line every front end printed, with the exit status of the run it
//! came from and the severity the diagnostic itself claims — never the
//! severity its exit status implies, because on this corpus the two
//! contradict each other: `tao_idl` prints `Error - …: fixed data types
//! are not supported` and **exits 0**, on all ten `fixed` cases.
//!
//! [`CLASSIFICATION`] is the analysis, and it is written by hand
//! because a meaning cannot be generated. Every warning text the sweep
//! records must match an entry in it; one that matches none is printed
//! in `WARNINGS.md` as **unclassified**, which is loud and is the whole
//! point — a front end that starts saying something new is exactly what
//! this file exists to notice.
//!
//! The counts are generated, so a figure in `WARNINGS.md` and a line in
//! `sources/warnings.tsv` cannot disagree.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

/// One diagnostic line, as the sweep recorded it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Diagnostic {
    /// Which program printed it. OpenDDS runs two — `tao_idl` writes
    /// the C++ mapping and `opendds_idl` the type support — and they
    /// know different annotations, which is only visible if the two
    /// are kept apart.
    pub program: String,
    /// The exit status of the run it came from.
    pub exit: i32,
    /// What the diagnostic says it is: `warning`, `error`, `noise` or
    /// `unclassified`.
    pub severity: String,
    /// The line, with the working directory, the line and column
    /// numbers and `java.util.logging`'s timestamp normalised out and
    /// nothing else changed.
    pub text: String,
}

/// Every diagnostic, keyed by `<case>/<system>`.
pub type Diagnostics = BTreeMap<String, Vec<Diagnostic>>;

/// The key both this map and the ledger use.
#[must_use]
pub fn key(case: &str, system: &str) -> String {
    format!("{case}/{system}")
}

/// Read `sources/warnings.tsv`.
///
/// A file that is not there is not an error: it is a measurement
/// nobody has made in this checkout, and `WARNINGS.md` says so by name
/// rather than rendering an empty table that reads as "nothing was
/// said".
///
/// # Errors
///
/// When a line has the wrong number of columns. A short line is a
/// truncated run, and a truncated run read as a quieter front end is
/// the failure this whole file is about.
pub fn read(path: &Path) -> Result<Diagnostics, String> {
    let mut out = Diagnostics::new();
    read_into(path, &mut out)?;
    Ok(out)
}

/// Read the same shape from a second file, into the same map.
///
/// `sources/warnings.tsv` is what the **front ends** said and
/// `warning-sweep.sh` writes it whole. `sources/toolchain-warnings.tsv`
/// is what the compilers said, and the byte sweeps write it — one
/// writer per file, because a sweep that `mv`s its output over a file
/// another sweep also writes deletes the other's rows.
///
/// Both are the same six columns and both are read here, so a compiler
/// is one more `program` beside `idlc` and `tao_idl` rather than a
/// second mechanism.
///
/// # Errors
/// As [`read`].
pub fn read_both(front_ends: &Path, toolchains: &Path) -> Result<Diagnostics, String> {
    let mut out = Diagnostics::new();
    read_into(front_ends, &mut out)?;
    read_into(toolchains, &mut out)?;
    Ok(out)
}

fn read_into(path: &Path, out: &mut Diagnostics) -> Result<(), String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("{}: {error}", path.display())),
    };
    for (index, line) in text.lines().enumerate() {
        if line.is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() != 6 {
            return Err(format!(
                "{}:{}: {} columns, not 6",
                path.display(),
                index + 1,
                fields.len()
            ));
        }
        let exit = fields[3]
            .parse()
            .map_err(|error| format!("{}:{}: {error}", path.display(), index + 1))?;
        let lines = out.entry(key(fields[0], fields[1])).or_default();
        // The two sources overlap on purpose and must not be counted
        // twice. A probe runs the vendor's front end and so sees the
        // same line `warning-sweep.sh` sees; the sweep classifies it
        // with that system's own severity table and the probe, which
        // has no such table, records what the words say and calls the
        // rest `unclassified`. Same program, same text, same
        // diagnostic — so the first reading of it stands, and a later
        // one only replaces it when the first said nothing about the
        // severity.
        if let Some(seen) = lines
            .iter_mut()
            .find(|line| line.program == fields[2] && line.text == fields[5])
        {
            // Only a reading that *names* a severity may replace one
            // that does not. `unclassified` and `noise` are both "this
            // reader could not tell", from the two readers' different
            // fallbacks — `warning-sweep.sh` says `unclassified` when
            // its system's table has no entry, and `diagnostics.sh`
            // says `noise` when the line carries no severity word and
            // no `[-W…]` switch.
            //
            // The condition used to be `!= "unclassified"`, which let
            // `noise` overwrite `unclassified`, and `noise` is not
            // rendered. Measured 2026-08-13: `fastddsgen`'s
            // `no such attribute: parent` on `annotation-scoped-name`
            // was `unclassified` in `warnings.tsv` and visible in
            // `WARNINGS.md`; the moment the probe cache went cold and
            // `toolchain-warnings.tsv` gained the same line as `noise`,
            // the row disappeared from the document with nothing going
            // red. A statement a front end made, hidden by a second
            // reader that could not read it.
            const NAMES_A_SEVERITY: [&str; 3] = ["warning", "error", "note"];
            if !NAMES_A_SEVERITY.contains(&seen.severity.as_str())
                && NAMES_A_SEVERITY.contains(&fields[4])
            {
                seen.severity = fields[4].to_string();
            }
            continue;
        }
        lines.push(Diagnostic {
            program: fields[2].to_string(),
            exit,
            severity: fields[4].to_string(),
            text: fields[5].to_string(),
        });
    }
    Ok(())
}

/// The warnings of one (case, system) pair, in the order they were
/// printed and without the errors and the noise beside them.
#[must_use]
pub fn warnings_of<'a>(
    diagnostics: &'a Diagnostics,
    case: &str,
    system: &str,
) -> Vec<&'a Diagnostic> {
    diagnostics
        .get(&key(case, system))
        .map(|lines| {
            lines
                .iter()
                .filter(|line| line.severity == "warning" || line.severity == "unclassified")
                .collect()
        })
        .unwrap_or_default()
}

/// What one warning text means, and whether it changes a verdict.
#[derive(Debug, Clone, Copy)]
pub struct Meaning {
    /// A substring that identifies the warning. The first entry whose
    /// `needle` the text contains wins, so a longer needle comes
    /// first.
    pub needle: &'static str,
    /// Which of the three things a warning can say this one says.
    pub class: Class,
    /// What it means, in this project's own words.
    pub note: &'static str,
    /// Which row of `docs/wire-feature-inventory.md` carries it, or
    /// the empty string where it changes nothing there.
    pub carried_into: &'static str,
}

/// The three things a warning can say. All three are findings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    /// The vendor departs from the specification and knows it.
    DeliberateDeviation,
    /// The vendor has not implemented something and says so.
    NotImplemented,
    /// The construct is one the specification leaves open.
    Unspecified,
    /// The diagnostic follows from another diagnostic and is not an
    /// independent statement about the case.
    Cascade,
    /// The diagnostic is true of the source a generator emitted and
    /// bears on no octet. It is kept because a diagnostic nobody has
    /// read is the shape `tools/wireprobe/diagnostics.sh` exists to
    /// stop, and it is classed apart because reading it as a vendor
    /// statement about the wire would be reading it wrongly.
    AboutTheGeneratedSource,
    /// The diagnostic is about the **machine the sweep ran on** and
    /// about no case, no vendor and no clause: a thread that could not
    /// be created, memory that could not be had.
    ///
    /// Its own class because it is the only one that puts a
    /// *measurement* in doubt rather than explaining one. A verdict
    /// reached while the host was refusing resources is not a verdict
    /// about the system under test, so an entry here has to say what
    /// became of the run it came from — and if a cell was recorded
    /// from it, that cell is not a measurement and the answer is to
    /// re-record it, never to classify it.
    AboutTheMeasuringHost,
}

impl Class {
    /// The word `WARNINGS.md` prints.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::DeliberateDeviation => "deliberate deviation",
            Self::NotImplemented => "not implemented",
            Self::Unspecified => "unspecified by the standard",
            Self::Cascade => "a cascade, not a statement",
            Self::AboutTheGeneratedSource => "about the generated source, not the wire",
            Self::AboutTheMeasuringHost => "about the measuring host, not the case",
        }
    }
}

/// What a **compiler's** diagnostic means, keyed by the switch or code
/// that identifies it rather than by its text.
///
/// A separate list from [`CLASSIFICATION`] because the two are keyed
/// differently and for a measured reason — see [`warning_tag`] — and
/// because the two say different kinds of thing. A front end's warning
/// is about the construct. A compiler's is about the *source somebody
/// generated for* the construct, and the interesting question is whose:
/// the vendor's generator wrote the type support, and this harness's
/// `fill.rs` and `dump.rs` wrote the statements beside it. The note on
/// each entry says which, because that is the actionable half.
///
/// An entry is written only for a tag that has been seen. A speculative
/// entry would be a claim about a diagnostic nobody has read.
pub const COMPILER_CLASSIFICATION: &[Meaning] = &[
    Meaning {
        needle: "-Wmultichar",
        class: Class::NotImplemented,
        note: "**The one this pipeline was built for.** A C character constant holding more \
               than one byte. gcc reports it by default and gives the constant the bytes of \
               the source, so `'\\u00e4'` compiles to `c3a4` and not to `e4`. \
               `crates/wire-probe/src/fill.rs` emitted a `wchar` that way, the line was \
               printed on every probe build that carried the literal, and it was deleted with \
               the temporary directory the probe compiled in. **It changed a verdict**, and \
               56 of them: `c3a4` stood in the results as Cyclone's and RTI's own `wchar` \
               octets, and two vendors were recorded as diverging over a value this harness \
               had put there. Measured on the test host, gcc 14.2.0: `warning: \
               multi-character character constant [-Wmultichar]`. It is a diagnostic about \
               *this project's* emitted source and never about a vendor's.",
        carried_into: "",
    },
    Meaning {
        needle: "-Wswitch-bool",
        class: Class::AboutTheGeneratedSource,
        note: "`omniidl -bcxx` gives a union over `boolean` a `switch` on a `CORBA::Boolean`, and gcc \
               warns because a two-valued switch is usually a mistake. It is not one here: the C++ \
               mapping makes the discriminant a `Boolean` and a `switch` over it is what the generated \
               accessor is. **It changes no verdict.** omniORB, TAO and JacORB all write \
               `union-discriminator-boolean` as `01` then the branch, which is what this project \
               writes, and the only octets that differ are the three fill positions the ORB probes \
               mark. Measured on the test host, gcc 14.2.0, on \
               `union-discriminator-boolean-unannotated`: `gen.hh: warning: switch condition has \
               boolean value [-Wswitch-bool]`. It reached the record only when the probe cache was \
               rebuilt — the cache key had not carried the template, so every ORB build was a hit on an \
               entry from before `diagnostics.sh`, which is the failure that mechanism's own header \
               describes.",
        carried_into: "",
    },
    Meaning {
        needle: "-Wreturn-type",
        class: Class::Cascade,
        note: "Printed after an error in the same function, not about a function that forgot \
               its `return`. OpenDDS 3.34's FACE mapping (`opendds_idl -Lface`, the one its \
               `fixed` goes through) gives an anonymous `sequence` or array member of a `fixed` \
               typedef a type it never declares (`::Sample::_amounts_seq`, \
               `::Sample::_items_forany`); the `gen_skip_over` call naming it does not parse, and \
               gcc reports the function body it abandoned as one with no `return`. **It changes \
               no verdict**: the same build's `error:` lines are what refuse `sequence-fixed` and \
               `array-1d-fixed`. Measured on the test host, gcc 14.2.0, compiling \
               `genTypeSupportImpl.cpp` for `sequence-fixed`: `error: '_amounts_seq' in 'struct \
               Sample' does not name a type` and four `expected` errors at line 476, then \
               `warning: no return statement in function returning non-void [-Wreturn-type]` at \
               line 494.",
        carried_into: "",
    },
];

/// Every warning text this corpus has produced, and what it means.
///
/// Written by hand: a count can be generated and a meaning cannot. A
/// text that matches nothing here is rendered as **unclassified**,
/// never as absent.
pub const CLASSIFICATION: &[Meaning] = &[
    Meaning {
        needle: "No default extensibility provided",
        class: Class::DeliberateDeviation,
        note: "Cyclone's own statement that its answer to \u{a7}7.3.1.2.1.8 is not the \
               specification's. The clause makes an unannotated aggregate `appendable`; `idlc` \
               makes it `final`, says so, and says it intends to change. It is the root cause of \
               GitHub issues #41 and #42, and it is in no clause and in no byte. **It changes a \
               verdict**: on 102 of the 115 of these cases where Cyclone and a peer both \
               produced bytes, every peer's XCDR2 payload is longer than Cyclone's by a \
               positive multiple of four — one DHEADER per defaulted aggregate — which is the \
               H9 divergence measured from the other side. `docs/omg-xtypes-issues.md` \u{a7}B6 \
               is the draft comment.",
        carried_into: "H9",
    },
    Meaning {
        needle: "values for literals of this enumerator are not consecutive",
        class: Class::NotImplemented,
        note: "Cyclone will not range-check an enumeration whose enumerators are sparse, in \
               either direction. Nothing on the wire changes — the value written is the \
               `@value` — so the bytes agree; what does not happen is the check \u{a7}7.2.2.4.1.4 \
               allows a receiver to make. An agreement under this warning is an agreement about \
               the octets and not about the validation behind them.",
        carried_into: "E1",
    },
    Meaning {
        needle: "Explicit defaults are not supported yet",
        class: Class::NotImplemented,
        note: "`idlc` reads `@default` and drops it, so the member takes Table 9's zero rather \
               than the value the type designer declared. This is M10 measured from the front \
               end rather than from the bytes, and it is why M10 has no dialect: reproducing it \
               would mean producing a value the sender's own declaration says is wrong.",
        carried_into: "M10",
    },
    Meaning {
        needle: "Inheriting from appendable structs is unsafe",
        class: Class::Unspecified,
        note: "Cyclone's reading of \u{a7}7.2.2.4.4, which permits a struct to inherit from an \
               `@appendable` base and does not say what a reader does with a base whose length \
               grew. `idlc` generates the type support anyway and exits 0. Nothing in the clause \
               calls it unsafe and nothing in it says it is safe; the vendor's word for the gap \
               is the only one on record.",
        carried_into: "H9",
    },
    Meaning {
        needle: "error in lookup of symbol: bit_bound",
        class: Class::NotImplemented,
        note: "Neither of OpenDDS's two generators resolves `@bit_bound`. **It changes a \
               verdict**: `enum-bitbound-8-plain` is three octets from Cyclone and from this \
               project and nine from OpenDDS, because the enumeration keeps the 32-bit holder \
               \u{a7}7.3.1.2.1.5 lets `@bit_bound` narrow. The table recorded the difference; \
               this is the sentence that says what caused it.",
        carried_into: "E1",
    },
    Meaning {
        needle: "error in lookup of symbol: value",
        class: Class::NotImplemented,
        note: "`tao_idl` drops `@value` on an enumerator, so the C++ mapping the OpenDDS probe \
               fills through numbers the enumerators by ordinal position. **It changes a \
               verdict**: `enum-sparse-values` is `ffff0000 01000000 64000000` from four \
               implementations and `02000000 00000000 01000000` from OpenDDS — the ordinals, \
               not the declared values.",
        carried_into: "E1",
    },
    Meaning {
        needle: "@optional annotation isn't fully supported",
        class: Class::NotImplemented,
        note: "`opendds_idl` says so itself, and every case it fires on is a case the \
               OpenDDS column refuses: nine of them because the generated type support does \
               not compile, and `combined-keyed-topic` and its unannotated twin because \
               `vwrite` in that same generated file will not take a `String_Manager`. The \
               harness had been saying `the probe did not compile` where the vendor had \
               already said which construct it was.",
        carried_into: "",
    },
    Meaning {
        needle: "error in lookup of symbol:",
        class: Class::NotImplemented,
        note: "`tao_idl` 2.5.24 knows none of the XTypes annotations — `@final`, `@appendable`, \
               `@mutable`, `@key`, `@id`, `@hashid`, `@optional`, `@external`, \
               `@must_understand`. It parses each one, drops it, and exits 0, which is a fourth \
               behaviour distinct from accepting, refusing and honouring. For the OpenDDS column \
               it is the C++ *mapping* that loses them and `opendds_idl` that decides the wire, \
               so most of these change nothing; `bit_bound` and `value` are the two that reach \
               the octets and they have their own rows above.",
        carried_into: "G15",
    },
    Meaning {
        needle: "Custom annotation mark will be ignored",
        class: Class::NotImplemented,
        note: "`rtiddsgen` reads `@ann::mark(7)` and drops it. IDL 4.2 rule (218) makes an \
               annotation's name a `<scoped_name>`, so a user annotation declared inside a \
               module is applied by its scoped name and this one is well formed. The generator \
               takes the syntax, says it is ignoring the annotation and exits — which is the \
               parsed-and-dropped behaviour `tao_idl` has for the builtin annotations, on a \
               user one. It changes no octet here: `mark` is a user annotation with no wire \
               effect, and the row is a statement about the reach of the syntax rather than \
               about the bytes.",
        carried_into: "",
    },
    Meaning {
        needle: "no viable alternative at character ':'",
        class: Class::NotImplemented,
        note: "`rtiddsgen`'s parser stops at the `::` of `@::key` — the root-anchored form of \
               rule (218)'s `<scoped_name>`, which rule (4) admits. The two RTI lines on \
               `annotation-scoped-name` are one behaviour seen twice: it takes `@ann::mark`, \
               ignores it, and cannot take `@::key` at all.",
        carried_into: "",
    },
    Meaning {
        needle: "no such attribute: parent",
        class: Class::NotImplemented,
        note: "`fastddsgen` on the same case. It resolves an annotation name by its last \
               segment and then looks for a *parent* attribute it has not been given, so a \
               scoped annotation name reaches its type builder as a shape it has no branch \
               for. Like RTI's two lines this says nothing about the octets — the case \
               produces none from either — and everything about which half of rule (218) each \
               front end implements.",
        carried_into: "",
    },
    Meaning {
        needle: "pthread_create failed (EAGAIN)",
        class: Class::AboutTheMeasuringHost,
        note: "**Not a statement about Fast DDS and not one about a case.** The JVM that runs \
               `fastddsgen` could not create a thread: `[0.293s][warning][os,thread] Failed to \
               start thread \"Unknown thread\" - pthread_create failed (EAGAIN)`, printed by \
               HotSpot's own unified logging, on a run that then exited 1. `EAGAIN` from \
               `pthread_create` is the host at its thread or process limit, and the sweep runs \
               `--jobs N` generators at once, each a JVM. \
               **Which measurement it belonged to, and what became of it.** The row is filed \
               under the case `-`, and `probe_diagnostics_batch` in \
               `tools/wireprobe/diagnostics.sh` files a line under `-` only when the chunk that \
               produced it held **more than one** case — a chunk of one names its case on every \
               line. A multi-case chunk whose generator fails and whose log blames no case is \
               `return 9` in `tools/wireprobe/wireprobe-batch`'s `run_chunk`: the chunk is split \
               and the halves are built again, and no cell is recorded from it. The 52 rows that \
               say `fastddsgen failed and named no case` come from the other branch of that same \
               function, which is reached only at one case remaining, and none of them carries a \
               thread diagnostic — `EAGAIN`, `Failed to start thread`, `unable to create native \
               thread` and `OutOfMemoryError` appear in `sources/` exactly once, here. \
               **So no committed cell was measured under it**, and it is kept because the next \
               one might not be as harmless: a verdict reached while the host was refusing \
               threads is not a verdict about the vendor, and it has to be re-recorded rather \
               than read.",
        carried_into: "",
    },
    Meaning {
        needle: "integer constant is so large that it is unsigned",
        class: Class::AboutTheGeneratedSource,
        note: "gcc on `fill.inc`, in both ROS columns, on `integer-extremes` — **this harness's \
               own emitted C and not a vendor's**. C has no literal for `INT64_MIN`: \
               `-9223372036854775808LL` is a positive constant too large for `long long`, so the \
               constant is given an unsigned type and the unary minus is applied to it. What \
               reaches the field is then an implementation-defined conversion rather than the \
               value the case declares. **It changed no verdict**: `integer-extremes` is \
               `80ff0080 ffff0000 00000080 ffffffff 00000000 00000080 ffffffffffffffff` from \
               both ROS type supports and byte for byte the same from Cyclone, Fast DDS, hdds \
               and the C column, so the intended `i64_min` did arrive. It is classed here and \
               not written off, because the reason it arrived is the compiler's choice and not \
               the literal's meaning. `crates/wire-probe/src/fill.rs` had already spelled the \
               value as `(-9223372036854775807LL - 1)`, the subtraction `<stdint.h>` uses; \
               `fill_ros.rs` and `fill_cpp.rs` had not been given it and now are, so a re-record \
               of the two ROS columns is where this warning stops being printed. \
               Keyed by its text and not by a switch, unlike the entries in \
               `COMPILER_CLASSIFICATION`: gcc prints this one unconditionally and names no \
               `[-W…]`, so there is no switch to key it by.",
        carried_into: "",
    },
    Meaning {
        needle: "unused variable: `member_start`",
        class: Class::AboutTheGeneratedSource,
        note: "rustc on the type support `hdds_gen` writes for `recursive-optional` and \
               `recursive-mutual-optional` — HDDS's emitted source, not this harness's: the \
               name occurs nowhere under `tools/` or `crates/wire-probe`, and in the HDDS crate \
               in `src/core/ser/pl_cdr2.rs`, whose `@mutable` framing the generated decoder \
               follows. **It changes no verdict**: the same build stops on `error[E0072]: \
               recursive type `Node` has infinite size` — HDDS holds the `@optional` member of \
               the type's own type inline — and that refusal is what the cell records. \
               Measured on the test host, rustc through `wireprobe-hdds`.",
        carried_into: "",
    },
    Meaning {
        needle: "error: could not compile `wireprobe-hdds-run-",
        class: Class::Cascade,
        note: "cargo's summary after rustc refused a build of the HDDS probe; it counts the \
               errors and warnings above it and says nothing of its own. The cell records the \
               first `error[...]` of the same build. Seen on `recursive-optional` and \
               `recursive-mutual-optional`, where that error is E0072.",
        carried_into: "",
    },
    Meaning {
        needle: "Confused by pre-processor line directives",
        class: Class::Cascade,
        note: "`omniidl` prints it only on the eight cases it has already refused with a syntax \
               error, and it is a remark about its own recovery rather than about the case. \
               Counted, classified and not carried anywhere.",
        carried_into: "",
    },
];

/// What a text means, or `None` when nothing claims it.
#[must_use]
pub fn meaning(text: &str) -> Option<&'static Meaning> {
    if let Some(tag) = warning_tag(text) {
        if let Some(found) = COMPILER_CLASSIFICATION
            .iter()
            .find(|meaning| meaning.needle == tag)
        {
            return Some(found);
        }
    }
    CLASSIFICATION
        .iter()
        .find(|meaning| text.contains(meaning.needle))
}

/// The switch or code a compiler names its own diagnostic by, if it
/// named one.
///
/// Each compiler family this pipeline runs names its diagnostic in its
/// own way, and the stable identity is the name and never the sentence:
///
///  * **gcc and clang** put the switch that controls a diagnostic in a
///    trailing bracket: `warning: multi-character character constant
///    [-Wmultichar]` is `-Wmultichar`;
///  * **javac** names the lint the same way: `Note: X.java uses
///    unchecked or unsafe operations. [unchecked]` is `unchecked`;
///  * **rustc** names an error's code in the bracket it follows —
///    `error[E0308]: mismatched types` is `E0308` — and names a lint in
///    the note beneath a warning, where ``#[warn(unused_variables)]``
///    is `unused_variables`;
///  * the **C# compiler** names its diagnostic `CSnnnn` and brackets
///    nothing: `z.cs(1,2): warning CS0168: …` is `CS0168`.
///
/// The bracketed form is the last bracketed group on the line, because
/// a diagnostic can quote a bracketed identifier from the source before
/// it. A line from a front end names none of these and is `None`.
///
/// **Why the tag and not the text.** Everything else in this file is
/// keyed by a substring of what a front end printed, which works
/// because those sentences are the vendor's and change when the vendor
/// changes. A compiler's sentence is not stable in the same way: gcc
/// rewords its diagnostics between releases and the switch that
/// controls one is the thing it promises to keep. A text-keyed entry
/// would go on matching until a compiler upgrade reworded the line, and
/// then fall back to `unclassified` — silently, and looking exactly
/// like a diagnostic nobody had classified yet. That is the failure
/// mode this whole file exists to avoid, one level down.
#[must_use]
pub fn warning_tag(text: &str) -> Option<&str> {
    // The lint name first: a rustc note carries both a bracket and a
    // `#[warn(…)]`, and the generic bracket reader would answer
    // `warn(unused_variables)` for it.
    if let Some(lint) = rustc_lint(text) {
        return Some(lint);
    }
    if let Some(tag) = bracketed_tag(text) {
        return Some(tag);
    }
    csharp_code(text)
}

/// The last bracketed group, which is gcc's and clang's `-W…` switch,
/// javac's lint and rustc's `Exxxx` code.
fn bracketed_tag(text: &str) -> Option<&str> {
    let close = text.rfind(']')?;
    let open = text[..close].rfind('[')?;
    let tag = &text[open + 1..close];
    if tag.is_empty() || tag.contains(' ') {
        return None;
    }
    Some(tag)
}

/// The lint a rustc note names, out of `#[warn(…)]` and its three
/// siblings. rustc prints it on a `= note:` line and not on the warning
/// itself, which is the only place the lint name occurs.
fn rustc_lint(text: &str) -> Option<&str> {
    for level in ["warn", "deny", "allow", "forbid"] {
        let opener = format!("#[{level}(");
        if let Some(start) = text.find(&opener) {
            let rest = &text[start + opener.len()..];
            if let Some(end) = rest.find(')') {
                let name = &rest[..end];
                if !name.is_empty() && !name.contains(' ') {
                    return Some(name);
                }
            }
        }
    }
    None
}

/// The `CSnnnn` code the C# compiler names, which is not bracketed and
/// so reaches neither of the two readers above.
fn csharp_code(text: &str) -> Option<&str> {
    let bytes = text.as_bytes();
    let mut start = 0;
    while start + 6 <= bytes.len() {
        let candidate = &bytes[start..start + 6];
        if candidate[0] == b'C'
            && candidate[1] == b'S'
            && candidate[2..].iter().all(u8::is_ascii_digit)
        {
            // `CS` and exactly four digits, not four digits inside a
            // longer identifier.
            let before_is_name = start > 0 && bytes[start - 1].is_ascii_alphanumeric();
            let after_is_name = start + 6 < bytes.len() && bytes[start + 6].is_ascii_alphanumeric();
            if !before_is_name && !after_is_name {
                return Some(&text[start..start + 6]);
            }
        }
        start += 1;
    }
    None
}

/// One row of the generated document: a warning text, and every case
/// that produced it.
struct Row {
    system: String,
    program: String,
    text: String,
    cases: BTreeSet<String>,
}

/// `WARNINGS.md`: every distinct warning text, its case count, what it
/// means, and which of them changed a verdict.
///
/// # Panics
///
/// Never: every `write!` here is into a `String`.
#[must_use]
pub fn document(diagnostics: &Diagnostics, systems: &[crate::systems::System]) -> String {
    let mut rows: BTreeMap<(String, String, String), Row> = BTreeMap::new();
    let mut silent: Vec<&str> = Vec::new();
    let mut measured: BTreeSet<&str> = BTreeSet::new();
    for (pair, lines) in diagnostics {
        let (case, system) = pair.split_once('/').unwrap_or((pair.as_str(), ""));
        measured.insert(system);
        for line in lines {
            if line.severity != "warning" && line.severity != "unclassified" {
                continue;
            }
            let identity = (system.to_string(), line.program.clone(), line.text.clone());
            rows.entry(identity)
                .or_insert_with(|| Row {
                    system: system.to_string(),
                    program: line.program.clone(),
                    text: line.text.clone(),
                    cases: BTreeSet::new(),
                })
                .cases
                .insert(case.to_string());
        }
    }
    for system in systems {
        if measured.contains(system.id) && !rows.values().any(|row| row.system == system.id) {
            silent.push(system.id);
        }
    }

    let mut out = String::from(
        "<!-- generated by tools/interop-results; do not edit -->\n\
         # What the front ends said\n\n\
         Every warning every front end printed about every corpus case, \
         with the number of cases it fired on and what it means. A \
         warning is a statement, and it says one of three things — our \
         case is ambiguous, the vendor deviates deliberately, or the \
         specification leaves something open. All three are findings.\n\n\
         Measured by `tools/interop-results/warning-sweep.sh`, which runs \
         each front end and **stops there**: no fill, no compile, no \
         link, no run. It produces no bytes and changes none, so it is a \
         separate measurement and not a re-record.\n\n\
         The severity is the one the diagnostic claims and never the one \
         its exit status implies. The two contradict each other here: \
         `tao_idl` prints `Error - …: fixed data types are not \
         supported` and **exits 0**, on all ten `fixed` cases.\n\n\
         ## What no cell here can say\n\n\
         A front end is not the only program a probe runs. Every probe in \
         `tools/wireprobe` then compiles, and every one of them writes its \
         compiler's output to a `cc.log` inside a `mktemp -d` that a \
         `trap … EXIT` removes, reads back `error:` lines and reads them \
         only when the build failed. `wireprobe-ros` compiles with `-w` \
         and `wireprobe-jacorb` with `javac -nowarn`; \
         `wireprobe-zerodds-c` compiles this project's own generated C \
         with `-Werror`.\n\n\
         So a system with no row below is a system whose **front end** was \
         quiet, and nothing more. The compiler stage has never been \
         listened to, and the one warning that would have caught the \
         `wchar` fill defect at its source — gcc's default \
         `-Wmultichar` — was printed on every probe build that carried the \
         literal and deleted with the directory it was printed into. \
         `docs/compiler-diagnostics-followup.md` is the entry for it.\n\n",
    );

    if rows.is_empty() {
        out.push_str(
            "**Nothing is recorded.** `tests/interop-results/sources/warnings.tsv` is \
             not in this checkout; run `tools/interop-results/warning-sweep.sh` on a \
             machine that has the front ends. An empty table here is not a set of \
             quiet front ends.\n",
        );
        return out;
    }

    let changed = rows
        .values()
        .filter(|row| {
            meaning(&row.text).is_some_and(|meaning| meaning.note.contains("changes a verdict"))
        })
        .count();
    let _ = writeln!(
        out,
        "{} distinct warning texts over {} case-and-system pairs. {changed} of them change what a \
         row of [`write.md`](write.md) says.\n",
        rows.len(),
        rows.values().map(|row| row.cases.len()).sum::<usize>()
    );

    out.push_str("| system | program | cases | class | the warning |\n|---|---|---:|---|---|\n");
    let mut ordered: Vec<&Row> = rows.values().collect();
    ordered.sort_by_key(|row| (std::cmp::Reverse(row.cases.len()), row.system.clone()));
    let mut untagged: BTreeSet<&str> = BTreeSet::new();
    for row in &ordered {
        let class = meaning(&row.text).map_or("**unclassified**", |meaning| meaning.class.word());
        if class == "**unclassified**" {
            if let Some(tag) = warning_tag(&row.text) {
                untagged.insert(tag);
            }
        }
        // A compiler names the switch or code that identifies its
        // diagnostic, and that identity is what
        // `COMPILER_CLASSIFICATION` is keyed by, so the table shows it
        // rather than making a reader find it in the sentence.
        let key = warning_tag(&row.text).map_or_else(String::new, |tag| format!(" `{tag}`"));
        let _ = writeln!(
            out,
            "| `{}` | `{}` | {} | {class}{key} | {} |",
            row.system,
            row.program,
            row.cases.len(),
            escape(&row.text)
        );
    }
    if !untagged.is_empty() {
        let _ = writeln!(
            out,
            "\n**{} compiler switch{} with no entry in `COMPILER_CLASSIFICATION`:** {}. Each is \
             a diagnostic a toolchain printed about generated source and nobody has said what it \
             means. An entry is written by hand, because a meaning cannot be generated, and only \
             for a switch somebody has read.",
            untagged.len(),
            if untagged.len() == 1 { "" } else { "es" },
            untagged
                .iter()
                .map(|tag| format!("`{tag}`"))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    out.push_str("\n## What each of them means\n\n");
    for row in &ordered {
        let _ = writeln!(
            out,
            "### `{}` / `{}` — {} case{}\n\n> {}\n",
            row.system,
            row.program,
            row.cases.len(),
            if row.cases.len() == 1 { "" } else { "s" },
            escape(&row.text)
        );
        match meaning(&row.text) {
            Some(meaning) => {
                let _ = writeln!(out, "**{}.** {}\n", meaning.class.word(), meaning.note);
                if !meaning.carried_into.is_empty() {
                    let _ = writeln!(
                        out,
                        "Carried into [`docs/wire-feature-inventory.md`](../../docs/wire-feature-inventory.md) row **{}**.\n",
                        meaning.carried_into
                    );
                }
            }
            None => {
                out.push_str(
                    "**Unclassified.** No entry of `tools/interop-results/src/warnings.rs` \
                     claims this text, which means a front end has started saying something \
                     this tree has not read yet. That is what this line is for.\n\n",
                );
            }
        }
        let listed: Vec<&str> = row.cases.iter().map(String::as_str).take(12).collect();
        let _ = writeln!(
            out,
            "Cases: {}{}\n",
            listed
                .iter()
                .map(|case| format!("[`{case}`](../interop-corpus/{case}/type.idl)"))
                .collect::<Vec<_>>()
                .join(", "),
            if row.cases.len() > listed.len() {
                format!(" and {} more", row.cases.len() - listed.len())
            } else {
                String::new()
            }
        );
    }

    if !silent.is_empty() {
        let _ = writeln!(
            out,
            "## The front ends that said nothing\n\n\
             `{}` ran over the whole corpus and printed no warning at all. That is a \
             measurement and not an absence: each of them refuses cases the others accept, \
             and each does it with an error and no advice. Where a vendor drops a construct \
             in silence there is nothing here to read, which is the difference between \
             OpenDDS — which names every annotation it could not resolve — and Fast DDS, \
             which resolves `@bit_bound` no better and says nothing.\n",
            silent.join("`, `")
        );
    }
    out
}

/// A diagnostic goes into a Markdown table cell, so the two characters
/// that would break one out are escaped. Nothing else is changed: the
/// text is the vendor's.
fn escape(text: &str) -> String {
    text.replace('|', "\\|").replace('\n', " ")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    /// One diagnostic seen by both sources is one row, and the source
    /// that classified it decides the severity.
    ///
    /// The overlap is real and deliberate: a probe runs the vendor's
    /// front end, so it sees the line `warning-sweep.sh` sees, and it
    /// has no per-system severity table to read it with. Counting it
    /// twice would double every figure in `WARNINGS.md` and fill the
    /// `unclassified` list with lines that are already classified.
    #[test]
    fn a_line_both_sources_saw_is_one_line() {
        let directory = std::env::temp_dir().join(format!("zd-warn-{}", std::process::id()));
        std::fs::create_dir_all(&directory).expect("a temporary directory");
        let front = directory.join("warnings.tsv");
        let tool = directory.join("toolchain-warnings.tsv");
        std::fs::write(
            &front,
            "primitive-mixed\tcyclone\tidlc\t0\twarning\tNo default extensibility provided.\n",
        )
        .expect("the front-end source");
        std::fs::write(
            &tool,
            "primitive-mixed\tcyclone\tidlc\t0\tunclassified\tNo default extensibility \
             provided.\nprimitive-mixed\tcyclone\tcc\t0\twarning\tmulti-character character \
             constant [-Wmultichar]\n",
        )
        .expect("the toolchain source");

        let read = read_both(&front, &tool).expect("both sources");
        let lines = read
            .get(&key("primitive-mixed", "cyclone"))
            .expect("the pair");
        assert_eq!(
            lines.len(),
            2,
            "the front end's line is counted once and the compiler's is kept: {lines:?}"
        );
        assert!(
            lines
                .iter()
                .any(|line| line.program == "idlc" && line.severity == "warning"),
            "the classified reading of the shared line did not win: {lines:?}"
        );
        assert!(
            lines
                .iter()
                .any(|line| line.program == "cc" && line.text.contains("-Wmultichar")),
            "the compiler's line, which only one source has, was dropped: {lines:?}"
        );
        std::fs::remove_dir_all(&directory).ok();
    }

    /// A reading that cannot read the line does not overwrite one that
    /// could not either — and never hides it.
    ///
    /// The two sources have two different fallbacks for "I cannot tell
    /// what this is": `warning-sweep.sh` writes `unclassified`, which
    /// `WARNINGS.md` renders and makes loud, and `diagnostics.sh`
    /// writes `noise`, which it does not render because the lines that
    /// reach it are front-end banners. Letting the second replace the
    /// first turned a visible statement into an absent one.
    ///
    /// Measured on `annotation-scoped-name`: `fastddsgen`'s
    /// `no such attribute: parent` stood in `WARNINGS.md` as
    /// unclassified until the probe cache went cold and the same line
    /// arrived from the toolchain source as `noise`. The row vanished
    /// and no test failed.
    #[test]
    fn noise_does_not_overwrite_a_line_the_other_source_kept() {
        let directory = std::env::temp_dir().join(format!("zd-warn-n-{}", std::process::id()));
        std::fs::create_dir_all(&directory).expect("a temporary directory");
        let front = directory.join("warnings.tsv");
        let tool = directory.join("toolchain-warnings.tsv");
        std::fs::write(
            &front,
            "a-case	fastdds	fastddsgen	255	unclassified	no such attribute: parent
",
        )
        .expect("the front-end source");
        std::fs::write(
            &tool,
            "a-case	fastdds	fastddsgen	255	noise	no such attribute: parent
",
        )
        .expect("the toolchain source");

        let read = read_both(&front, &tool).expect("both sources");
        let lines = read.get(&key("a-case", "fastdds")).expect("the pair");
        assert_eq!(
            lines.len(),
            1,
            "the shared line was counted twice: {lines:?}"
        );
        assert_eq!(
            lines[0].severity, "unclassified",
            "`noise` overwrote `unclassified`, which hides the line from WARNINGS.md: {lines:?}"
        );
        std::fs::remove_dir_all(&directory).ok();
    }

    /// A compiler diagnostic is classified by its switch, and stays
    /// classified when the sentence around it changes.
    ///
    /// The point of keying on the tag. gcc rewords its diagnostics
    /// between releases and keeps the switch; a text-keyed entry would
    /// go on matching until an upgrade and then fall back to
    /// `unclassified`, which looks exactly like a diagnostic nobody has
    /// classified yet.
    #[test]
    fn a_compiler_diagnostic_is_keyed_by_its_switch() {
        let measured = "zdmc.c: warning: multi-character character constant [-Wmultichar]";
        assert_eq!(warning_tag(measured), Some("-Wmultichar"));
        let found = meaning(measured).expect("the measured line is classified");
        assert!(
            found.note.contains("56"),
            "the entry does not say what it changed: {}",
            found.note
        );

        // The same switch, a sentence gcc has not written yet.
        let reworded = "zdmc.c: warning: character constant holds more than one byte \
                        [-Wmultichar]";
        assert_eq!(
            meaning(reworded).map(|found| found.needle),
            Some("-Wmultichar"),
            "a reworded diagnostic with the same switch fell out of the classification"
        );

        // A switch nothing has read is `unclassified`, loudly, rather
        // than matched by a neighbouring entry.
        assert!(
            meaning("t.c: warning: unused variable 'x' [-Wunused-variable]").is_none(),
            "a switch with no entry was classified by something else"
        );

        // A javac diagnostic names its switch the same way.
        assert_eq!(
            warning_tag("Note: X.java uses unchecked or unsafe operations. [unchecked]"),
            Some("unchecked")
        );

        // A front end's sentence carries no switch and still reaches
        // the text-keyed list.
        assert!(warning_tag("Warning - error in lookup of symbol: final").is_none());
        assert!(meaning("Warning - error in lookup of symbol: final").is_some());
    }

    /// Each compiler family this pipeline runs names its diagnostic in
    /// its own shape, and each shape is read.
    ///
    /// The families and their keys: gcc and clang bracket a `-W…`
    /// switch, javac brackets a lint, rustc brackets an `Exxxx` code
    /// and names a lint in a `#[warn(…)]` note, and the C# compiler
    /// writes `CSnnnn` unbracketed. A reader that only knew the bracket
    /// would render every C# line `unclassified`, which is the failure
    /// the `[-W…]` arm was added to stop one family at a time.
    #[test]
    fn each_compiler_family_names_its_diagnostic() {
        // gcc, measured on the test host with `-Wswitch-bool`.
        assert_eq!(
            warning_tag("gen.hh: warning: switch condition has boolean value [-Wswitch-bool]"),
            Some("-Wswitch-bool")
        );
        // clang, the same bracket.
        assert_eq!(
            warning_tag("z.c:5:13: warning: unused variable 'x' [-Wunused-variable]"),
            Some("-Wunused-variable")
        );
        // javac.
        assert_eq!(
            warning_tag("Note: Z.java uses unchecked or unsafe operations. [unchecked]"),
            Some("unchecked")
        );
        // rustc error code.
        assert_eq!(warning_tag("error[E0308]: mismatched types"), Some("E0308"));
        // rustc lint name, which only its note carries.
        assert_eq!(
            warning_tag("= note: `#[warn(unused_variables)]` on by default"),
            Some("unused_variables")
        );
        // C# code, unbracketed.
        assert_eq!(
            warning_tag("z.cs(1,2): warning CS0168: The variable 'x' is declared but never used"),
            Some("CS0168")
        );
        // A C# code inside a longer identifier is not one.
        assert_eq!(
            warning_tag("the identifier XCS0168A appears in the text ARCS0168B here"),
            None
        );
    }
}
