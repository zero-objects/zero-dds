// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Is a recorded octet one a re-run would produce?
//!
//! `--verify` holds the tree against `sources/`. It never holds
//! `sources/` against a fresh sweep, and it cannot: the sweeps need
//! RTI's licence, hand-built ORBs and a ROS installation. So a probe
//! that writes something different on every run is invisible to it —
//! the row is committed once, every later verify compares the tree to
//! that row, and the tree matches forever.
//!
//! That is not hypothetical. Six rows of `sources/ros-corpus.tsv` held
//! a **heap address** each, in `sequence-bounded-full`,
//! `sequence-of-typedef`, `typedef-of-sequence` and their twins:
//! Fast CDR 1.0.24's `serializeArray` over a multi-byte numeric element
//! writes the address of its pointer argument when the byte order is
//! not the host's. Every verify since they were recorded passed on
//! them, and `docs/open-rows-breakdown.md` counted them as a wire
//! behaviour of the ROS type support that no setting could reproduce —
//! because they are not reproducible at all.
//!
//! ## What can be checked without a toolchain
//!
//! The corpus is built in pairs: `<case>` and `<case>-unannotated`,
//! whose IDL differs only in annotations and whose `values.json` is
//! usually the same file twice. For a column whose front end **drops**
//! the annotations — every ROS message is a `@final` XCDR1 struct
//! whatever its `.idl` says, and CORBA IDL has no extensibility at all
//! — the two are the same type carrying the same sample, so they have
//! to produce the same octets. Two runs of one probe, in effect, from
//! data already committed.
//!
//! It found the six. It is exactly calibrated over everything else:
//! `ros-fastrtps`, `ros-cyclone`, `jacorb` and `tao` agree on every
//! other twin pair the record holds.
//!
//! ## The one allowance, why it is not a hole, and why it now forgives
//! nothing
//!
//! omniORB's twins used to disagree on 23 pairs. Every one of them was
//! same-length and every differing octet was one this project writes as
//! `00` — §7.4.1 says nothing about the content of padding and
//! `cdrMemoryStream(0, false)` left whatever the buffer held there, so
//! two runs of one process differed. That holds on both of omniORB's
//! paths: `cdrStream::alignOutput` is the same line under the probe's
//! `cdrMemoryStream` and under `giopStream`, and over IIOP the buffer
//! holds the previous request (`tools/wire-capture`). So a disagreement
//! is reported as a failure **unless both twins' octets read back as
//! their own sample**, which is [`crate::rust_column::reads_back_as_the_sample`],
//! the same check the page's group G rests on. A difference no reader
//! can see is padding content; a difference a reader sees is a value,
//! and a value that is not a function of the sample is what this
//! module is for.
//!
//! **Since 2026-08-13 the allowance forgives nothing, because there is
//! nothing left to forgive.** `omniorb-template.cpp` and
//! `tao-template.cpp` write a fixed `0xAA` at the fill — the octets the
//! peer does not determine, marked — so the recording is a function of
//! the sample and every twin pair in the record now carries the same
//! octets. Zero disagreements, in every column.
//! `tests/reproducible.rs::the_padding_allowance_has_nothing_left_to_forgive`
//! is the assertion; the allowance stays because it is what would keep a
//! padding difference from reading as a value difference if one
//! reappeared.
//!
//! ## What it does not replace
//!
//! `tools/interop-results/verify-record.sh` re-records the vendors a
//! machine has and diffs them against `sources/`. That is the direct
//! check and it stays the stronger one — it catches a non-reproducible
//! octet whether or not the case has a twin. This one costs nothing,
//! needs no toolchain, and runs in `cargo test` on every clone.

use crate::sources::{Answer, Measured};
use crate::systems::SYSTEMS;
use crate::{Case, rust_column};

/// The suffix the corpus gives the annotation-free half of a pair.
const TWIN_SUFFIX: &str = "-unannotated";

/// One twin pair whose octets differ where they may not.
#[derive(Debug, Clone)]
pub struct Disagreement {
    /// The annotated half's case name.
    pub case: String,
    /// The column.
    pub system: String,
    /// The encoding version.
    pub version: String,
    /// The byte order.
    pub endian: String,
    /// What the annotated half produced.
    pub annotated: String,
    /// What the `-unannotated` half produced.
    pub unannotated: String,
    /// Whether both halves' octets decode to their own sample, which
    /// makes the difference padding content rather than a value.
    pub reaches_no_reader: bool,
}

impl Disagreement {
    /// The line a report prints.
    #[must_use]
    pub fn line(&self) -> String {
        format!(
            "{}/{}/{}/{} and its {TWIN_SUFFIX} twin carry the same sample \
             and different octets: {} against {}",
            self.case,
            self.system,
            self.version,
            self.endian,
            short(&self.annotated),
            short(&self.unannotated)
        )
    }
}

fn short(hex: &str) -> String {
    if hex.len() <= 48 {
        hex.to_string()
    } else {
        format!("{}… ({} octets)", &hex[..48], hex.len() / 2)
    }
}

/// Every twin pair that carries one sample and two byte strings.
///
/// The columns asked are the ones whose front end drops the
/// annotations; [`crate::systems::System::twins_are_one_type`] carries
/// which, with the measurement beside it.
///
/// Split by [`Disagreement::reaches_no_reader`]: the ones that reach a
/// reader are failures, the ones that do not are padding content and
/// are reported without failing.
#[must_use]
pub fn twin_disagreements(cases: &[Case], recorded: &[&Measured]) -> Vec<Disagreement> {
    let mut found = Vec::new();
    for case in cases {
        if case.name.ends_with(TWIN_SUFFIX) {
            continue;
        }
        let twin_name = format!("{}{TWIN_SUFFIX}", case.name);
        let Some(twin) = cases.iter().find(|other| other.name == twin_name) else {
            continue;
        };
        // Only where the sample is the same one. A pair whose values
        // differ is two measurements and says nothing about either.
        if case.values != twin.values {
            continue;
        }
        for system in SYSTEMS.iter().filter(|system| system.twins_are_one_type) {
            for measured in recorded {
                for (key, answer) in measured.iter() {
                    let Some((name, version, endian)) = split(key, system.id) else {
                        continue;
                    };
                    if name != case.name {
                        continue;
                    }
                    let Answer::Bytes(annotated) = answer else {
                        continue;
                    };
                    let Some(Answer::Bytes(unannotated)) =
                        crate::sources::get(measured, &twin_name, system.id, version, endian)
                    else {
                        continue;
                    };
                    if annotated == unannotated {
                        continue;
                    }
                    // Under *any* shipped configuration: the question
                    // is whether a reader exists that sees no
                    // difference, not which one the row was compared
                    // against. Trying them all can only make this
                    // check more forgiving, and a value that is not a
                    // function of the sample is forgiven by none of
                    // them.
                    //
                    // A source file writes the encoding *version* and
                    // the decoder is asked for a **framing**. Not
                    // `canonical_encoding`, which takes a framing and
                    // answers one: given `xcdr1` it falls through its
                    // own `_` arm and answers `plain-cdr2`, and a
                    // decode under the wrong version reads as "no
                    // reader sees this" for rows where one does.
                    let framing = if version == "xcdr1" {
                        "plain-cdr1"
                    } else {
                        "plain-cdr2"
                    };
                    let reaches_no_reader =
                        rust_column::CONFIGURATIONS.iter().any(|configuration| {
                            rust_column::reads_back_as_the_sample(
                                case,
                                configuration.name,
                                framing,
                                endian,
                                annotated,
                            ) && rust_column::reads_back_as_the_sample(
                                twin,
                                configuration.name,
                                framing,
                                endian,
                                unannotated,
                            )
                        });
                    found.push(Disagreement {
                        case: case.name.clone(),
                        system: system.id.to_string(),
                        version: version.to_string(),
                        endian: endian.to_string(),
                        annotated: annotated.clone(),
                        unannotated: unannotated.clone(),
                        reaches_no_reader,
                    });
                }
            }
        }
    }
    found.sort_by(|a, b| {
        (&a.case, &a.system, &a.version, &a.endian)
            .cmp(&(&b.case, &b.system, &b.version, &b.endian))
    });
    found
}

/// `<case>/<system>/<version>/<endian>`, when the system is this one.
fn split<'a>(key: &'a str, system: &str) -> Option<(&'a str, &'a str, &'a str)> {
    let mut parts = key.rsplitn(4, '/');
    let (endian, version, found, case) =
        (parts.next()?, parts.next()?, parts.next()?, parts.next()?);
    (found == system).then_some((case, version, endian))
}
