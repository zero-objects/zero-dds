// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The one machine-readable record every table cell is rendered from.
//!
//! One line per (case, encoding, byte order, system), carrying both
//! directions. The tables are a rendering of this file and nothing
//! else, so a figure in a table and a figure in the ledger cannot
//! disagree; and `--verify` compares a fresh ledger against the
//! committed one, so a figure in the ledger and a figure a run prints
//! cannot disagree either.

use std::collections::BTreeMap;
use std::fmt::Write as _;

/// What one system did with one (case, encoding, byte order) in one
/// direction.
///
/// Every variant prints a word. **None of them is a blank**: a blank
/// cell reads as agreement, and most of the holes in a cross-vendor
/// matrix are not agreements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Bytes, as lower-case hex. The tree holds them as a `.bin`.
    Bytes(String),
    /// The read direction returned the case's own sample.
    Same,
    /// The read direction returned something else, which is a defect
    /// wherever it appears.
    Different(String),
    /// The system was asked and refused, in its own words.
    Refused(String),
    /// The read direction had nothing to read because the write
    /// direction produced nothing. Says nothing about the reader.
    Unwritten,
    /// This row's run *is* another encoding's run: a vendor's
    /// `--encoding` selects the encoding version and the framing
    /// follows the type's extensibility. Carries the encoding whose
    /// file this row points at.
    VersionAlias(&'static str),
    /// The system has no encoding version 2 at all.
    NoVersion2,
    /// The system writes one byte order and this is the other one.
    OneOrderOnly,
    /// The construct has no form in this system's type model, so its
    /// front end never saw the case. Carries which construct stopped
    /// it.
    ///
    /// A statement about the **specification**, and the two below are
    /// the ones that used to hide inside it. See
    /// `docs/specs/zerodds-verdicts-1.0.md`.
    NoForm(String),
    /// The case is not inside this column's specification at all.
    ///
    /// `Profile::validate`'s answer, and the only thing a denominator
    /// may be built from. Distinct from [`Self::NoForm`], which is one
    /// construct stopping one front end: this is the case not being a
    /// question for this column in the first place.
    ///
    /// Without it, every report picked its own denominator, and
    /// measuring CORBA against 376 XTypes cases produced "TAO refused
    /// 127" — a number about what was handed to TAO.
    OutOfProfile(String),
    /// Its front end took the case and its own generated code does not
    /// build. Carries what was measured.
    ///
    /// A measurement of the toolchain, not of its specification.
    /// `rosidl`'s IDL takes `wchar[3]` without a word and
    /// `rosidl_typesupport_fastrtps_c` 2.2.2 then emits a call to an
    /// undeclared `callbacks`; recording that as `NoForm` says ROS has
    /// no collection of wchar, which is the opposite of what was
    /// measured.
    GeneratorDefect(String),
    /// It carries the case and a documented mapping rule makes the
    /// octets differ. Carries the rule.
    ///
    /// `omniidl` and `tao_idl` both take `long double`; CORBA's C++
    /// mapping makes it x87 extended and not §9.3.1.3's binary128, so
    /// the octets are not the reference's. That is a finding about the
    /// mapping, and `NoForm` hid it behind "CORBA has no long double".
    MappingDivergence(String),
    /// A tool of **ours** has not written the construct, which the
    /// target has. Exit 6.
    HarnessGap(String),
    /// The toolchain exists and no probe in this tree drives it.
    NoProbe,
    /// Expressible, a harness in this tree produces it, and no run is
    /// recorded. Carries the command that would produce it.
    NotMeasured(String),
}

impl Outcome {
    /// The word a table cell prints.
    #[must_use]
    pub const fn word(&self) -> &'static str {
        match self {
            Self::Bytes(_) => "bytes",
            Self::Same => "same",
            Self::Different(_) => "different",
            Self::Refused(_) => "refused",
            Self::Unwritten => "unwritten",
            Self::VersionAlias(_) => "=",
            Self::NoVersion2 => "no xcdr2",
            Self::OneOrderOnly => "one order",
            Self::NoForm(_) => "no form",
            Self::OutOfProfile(_) => "not its spec",
            Self::GeneratorDefect(_) => "generator defect",
            Self::MappingDivergence(_) => "maps differently",
            Self::HarnessGap(_) => "our gap",
            Self::NoProbe => "no probe",
            Self::NotMeasured(_) => "not measured",
        }
    }

    /// The status token the ledger's `status` column carries.
    #[must_use]
    pub const fn token(&self) -> &'static str {
        match self {
            Self::Bytes(_) => "bytes",
            Self::Same => "same",
            Self::Different(_) => "different",
            Self::Refused(_) => "refused",
            Self::Unwritten => "unwritten",
            Self::VersionAlias(_) => "version-alias",
            Self::NoVersion2 => "no-version-2",
            Self::OneOrderOnly => "one-order-only",
            Self::NoForm(_) => "no-form",
            Self::OutOfProfile(_) => "out-of-profile",
            Self::GeneratorDefect(_) => "generator-defect",
            Self::MappingDivergence(_) => "mapping-divergence",
            Self::HarnessGap(_) => "harness-gap",
            Self::NoProbe => "no-probe",
            Self::NotMeasured(_) => "not-measured",
        }
    }

    /// The detail the ledger's last column carries.
    #[must_use]
    pub fn detail(&self) -> String {
        match self {
            Self::Bytes(hex) => hex.clone(),
            Self::Different(detail)
            | Self::Refused(detail)
            | Self::NoForm(detail)
            | Self::OutOfProfile(detail)
            | Self::GeneratorDefect(detail)
            | Self::MappingDivergence(detail)
            | Self::HarnessGap(detail)
            | Self::NotMeasured(detail) => sanitise(detail),
            Self::VersionAlias(encoding) => (*encoding).to_string(),
            Self::Same
            | Self::Unwritten
            | Self::NoVersion2
            | Self::OneOrderOnly
            | Self::NoProbe => String::new(),
        }
    }

    /// Whether this outcome is a measurement of this system at all.
    ///
    /// The diff column compares measurements. An absence is not a
    /// disagreement and must never be counted as one.
    /// `GeneratorDefect` and `MappingDivergence` are measurements: each
    /// is something a toolchain did, and a run where one appears and
    /// disappears is a change worth a diff line. `OutOfProfile` and
    /// `HarnessGap` are not — the first is a property of the case and
    /// the second is a property of this tree.
    #[must_use]
    pub const fn is_measurement(&self) -> bool {
        matches!(
            self,
            Self::Bytes(_)
                | Self::Same
                | Self::Different(_)
                | Self::Refused(_)
                | Self::GeneratorDefect(_)
                | Self::MappingDivergence(_)
        )
    }

    /// Rebuild an outcome from its two ledger columns.
    #[must_use]
    pub fn parse(token: &str, detail: &str) -> Option<Self> {
        Some(match token {
            "bytes" => Self::Bytes(detail.to_string()),
            "same" => Self::Same,
            "different" => Self::Different(detail.to_string()),
            "refused" => Self::Refused(detail.to_string()),
            "unwritten" => Self::Unwritten,
            "version-alias" => Self::VersionAlias(match detail {
                "plain-cdr1" => "plain-cdr1",
                "plain-cdr2" => "plain-cdr2",
                _ => return None,
            }),
            "no-version-2" => Self::NoVersion2,
            "one-order-only" => Self::OneOrderOnly,
            "no-form" => Self::NoForm(detail.to_string()),
            "out-of-profile" => Self::OutOfProfile(detail.to_string()),
            "generator-defect" => Self::GeneratorDefect(detail.to_string()),
            "mapping-divergence" => Self::MappingDivergence(detail.to_string()),
            "harness-gap" => Self::HarnessGap(detail.to_string()),
            "no-probe" => Self::NoProbe,
            "not-measured" => Self::NotMeasured(detail.to_string()),
            _ => return None,
        })
    }
}

/// A tab, a newline or a run of spaces in a probe's sentence would
/// break the record open. Bounded too: a Java stack trace is a
/// diagnostic and not a result.
fn sanitise(text: &str) -> String {
    let flat: String = text
        .chars()
        .map(|character| {
            if character.is_control() || character == '\t' {
                ' '
            } else {
                character
            }
        })
        .collect();
    let flat = flat.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() > 240 {
        let cut: String = flat.chars().take(237).collect();
        format!("{cut}...")
    } else {
        flat
    }
}

/// One (case, encoding, byte order, system) line.
#[derive(Debug, Clone)]
pub struct Entry {
    /// The corpus case.
    pub case: String,
    /// One of the five encodings.
    pub encoding: &'static str,
    /// `le` or `be`.
    pub endian: &'static str,
    /// The column's id.
    pub system: &'static str,
    /// What the system put on the wire.
    pub write: Outcome,
    /// What it got back off it.
    pub read: Outcome,
    /// How many distinct warnings this system's front end printed about
    /// this case.
    ///
    /// A column of its own and not a note beside the refusal, because
    /// the row it matters most in is the one where nothing went wrong:
    /// a system whose bytes agree while its front end warned about the
    /// type is not the same as one that agreed silently, and a record
    /// that cannot tell those apart says the wrong thing about 148 rows
    /// of this corpus.
    pub warnings: usize,
}

impl Entry {
    /// The key the tables group by.
    #[must_use]
    pub fn row_key(&self) -> String {
        format!("{}/{}/{}", self.case, self.encoding, self.endian)
    }
}

/// Every line, in the order the tables print them.
#[derive(Debug, Default)]
pub struct Ledger {
    /// One per (case, encoding, byte order, system).
    pub entries: Vec<Entry>,
    /// Every warning a front end printed, keyed by `<case>/<system>`
    /// and rendered as `<program>\t<text>`.
    ///
    /// Held once per pair rather than once per row: a warning belongs
    /// to the front end, and the front end runs once per case whatever
    /// the encoding and the byte order turn out to be.
    pub warnings: BTreeMap<String, Vec<String>>,
}

/// The row numbers, which are stable across corpus growth because they
/// are never recomputed: a key that has a number keeps it, and a key
/// that has none takes the next free one.
#[derive(Debug, Default)]
pub struct RowIds {
    /// `<case>/<encoding>/<endian>` to its stable number.
    pub by_key: BTreeMap<String, u32>,
    next: u32,
}

impl RowIds {
    /// Read the committed registry, or start an empty one.
    ///
    /// # Errors
    ///
    /// When a line is not `<number>\t<key>`, because a registry that
    /// silently dropped a line would renumber every row after it and
    /// break every citation of them.
    pub fn read(text: &str) -> Result<Self, String> {
        let mut ids = Self::default();
        for (index, line) in text.lines().enumerate() {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (number, key) = line
                .split_once('\t')
                .ok_or_else(|| format!("row-ids.tsv:{}: not `<number><tab><key>`", index + 1))?;
            let number: u32 = number
                .parse()
                .map_err(|error| format!("row-ids.tsv:{}: {error}", index + 1))?;
            if ids.by_key.insert(key.to_string(), number).is_some() {
                return Err(format!("row-ids.tsv:{}: `{key}` twice", index + 1));
            }
            ids.next = ids.next.max(number);
        }
        Ok(ids)
    }

    /// This key's number, allocating one if it has none.
    pub fn number(&mut self, key: &str) -> u32 {
        if let Some(number) = self.by_key.get(key) {
            return *number;
        }
        self.next += 1;
        self.by_key.insert(key.to_string(), self.next);
        self.next
    }

    /// The registry, as it is committed.
    #[must_use]
    pub fn render(&self) -> String {
        let mut ordered: Vec<(&u32, &String)> = self
            .by_key
            .iter()
            .map(|(key, number)| (number, key))
            .collect();
        ordered.sort();
        let mut out = String::from(
            "# The stable row numbers of tests/interop-results/write.md and read.md.\n\
             # Append-only: a key that has a number keeps it, so a row cited once stays\n\
             # citable when the corpus grows. Never regenerate this file from scratch.\n",
        );
        for (number, key) in ordered {
            let _ = writeln!(out, "{number}\t{key}");
        }
        out
    }
}

/// The ledger, as it is committed.
#[must_use]
pub fn render(ledger: &Ledger) -> String {
    let mut out = String::from(
        "case\tencoding\tendian\tsystem\twrite\twrite_detail\tread\tread_detail\twarnings\n",
    );
    for entry in &ledger.entries {
        let _ = writeln!(
            out,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            entry.case,
            entry.encoding,
            entry.endian,
            entry.system,
            entry.write.token(),
            entry.write.detail(),
            entry.read.token(),
            entry.read.detail(),
            entry.warnings
        );
    }
    out
}
