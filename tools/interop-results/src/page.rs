// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `matrix.html`: the answers first, the 3,350 rows last.
//!
//! [`crate::tables`] renders the same ledger as two Markdown tables.
//! Those are the record. This is the page somebody reads when a bug
//! report arrives — "your writer and Cyclone's reader disagree", "which
//! flag do I set", "can Fast DDS do something you cannot" — and a
//! 3,350-row table answers none of those questions by being present.
//!
//! So the order is inverted. The page opens with the one answer a
//! reader wants first, follows it with the dialect matrix that turns a
//! divergence into a flag, gives each of the twelve systems a section
//! of its own, and puts the rows in an appendix at the end.
//!
//! ## Everything on it is derived
//!
//! No figure here is written down. The acceptance answer is the
//! ledger's; the dialect matrix is the twelve configurations of
//! [`crate::rust_column`] measured against every foreign byte string in
//! the ledger; the front-end half of the acceptance answer is parsed
//! out of `docs/idl-building-blocks.md`; the issue numbers are parsed
//! out of the corpus cases' own READMEs. A number that moves when the
//! corpus grows moves here on the next regeneration, and `--verify`
//! fails when the committed page is not what a fresh run writes.
//!
//! ## Why the links are relative
//!
//! The page sits in the result tree, so `case/system/plain-cdr1.le.bin`
//! resolves both from a checkout on disk and from anywhere the
//! directory is served whole. Passing `--base-url` prefixes every link,
//! which is what turns them absolute the day the tree is published;
//! until then an absolute link would be a link to nothing, and a reader
//! who follows one and gets a 404 has been told a false thing about the
//! evidence.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use crate::ledger::{Entry, Ledger, Outcome, RowIds};
use crate::rust_column::{self, CONFIGURATIONS, CaseSweep, DEFAULT, reads_back_as_the_sample};
use crate::systems::{Kind, SYSTEMS};
use crate::tables::{self, Direction};
use crate::{Case, ENCODINGS, ORDERS};

/// The public mirror. Named, never linked into: the result tree is not
/// part of it, and a link into a tree that is not there is worse than
/// no link at all.
const MIRROR: &str = "github.com/zero-objects/zero-dds";

/// The clause-by-clause configuration — "how it should be" on every row
/// that has two answers.
const SPECIFICATION: &str = "specification";

/// The file the page is written to.
pub const FILE: &str = "matrix.html";

/// How many constructs a section names before it stops naming them.
const NAMED: usize = 60;

/// One row of the appendix: a case, an encoding, a byte order, and what
/// each of the twelve did with it.
struct Row<'a> {
    id: u32,
    case: usize,
    encoding: &'static str,
    endian: &'static str,
    entries: Vec<&'a Entry>,
    diff: String,
    mitigation: String,
    /// What each of the twelve settings produced for this row, in
    /// [`CONFIGURATIONS`] order. `None` where the setting refused.
    settings: Vec<Option<String>>,
}

impl Row<'_> {
    /// The octets this project's shipped default produced.
    fn ours(&self) -> Option<&str> {
        self.settings
            .get(default_setting())
            .and_then(Option::as_deref)
    }

    /// The octets a clause-by-clause reading of XTypes 1.3 produces.
    fn specification(&self) -> Option<&str> {
        self.settings
            .get(specification_setting())
            .and_then(Option::as_deref)
    }

    /// Which settings produce exactly what `system` produced on this
    /// row, as a bit per setting.
    ///
    /// Zero means two different things and the caller has to know
    /// which: the system produced no octets here, or it produced
    /// octets no setting of this project reproduces. Only the second
    /// is a finding, so every caller checks for octets first.
    fn reproduce(&self, system: &str) -> u32 {
        let Some(entry) = self.entries.iter().find(|entry| entry.system == system) else {
            return 0;
        };
        let Outcome::Bytes(theirs) = &entry.write else {
            return 0;
        };
        let mut mask = 0_u32;
        for (line, mine) in self.settings.iter().enumerate() {
            if mine.as_deref() == Some(theirs.as_str()) {
                mask |= 1 << line;
            }
        }
        mask
    }
}

/// Where the shipped default sits in [`CONFIGURATIONS`].
fn default_setting() -> usize {
    CONFIGURATIONS
        .iter()
        .position(|configuration| configuration.name == DEFAULT)
        .unwrap_or(0)
}

/// Where the clause-by-clause reading sits in [`CONFIGURATIONS`].
fn specification_setting() -> usize {
    CONFIGURATIONS
        .iter()
        .position(|configuration| configuration.name == SPECIFICATION)
        .unwrap_or(0)
}

/// How far one setting gets a reader towards one system: the number of
/// rows where the system produced octets and the setting produced the
/// same ones.
#[derive(Debug, Default, Clone, Copy)]
struct Alignment {
    aligned: usize,
}

/// One group of the rows a setting does **not** reproduce.
///
/// The remainder is the finding, so it is grouped by what is different
/// about the octets rather than left as a number to be subtracted. The
/// shape is measured — a length difference, or the first octet at which
/// two equal-length payloads part — and never guessed at.
struct Remainder {
    /// The shape of the difference, as a phrase a reader can act on.
    shape: String,
    /// How many rows are in this group.
    rows: usize,
    /// The constructs in it, each with the first row number that shows
    /// it.
    cases: BTreeMap<String, u32>,
    /// Settings that *do* reproduce rows of this group, with how many
    /// rows each reaches. A row can be reproduced by more than one.
    reproduced_by: BTreeMap<&'static str, usize>,
    /// Rows of this group no setting of this project reproduces at all.
    unreached: usize,
    /// The question this group is one the comparison deliberately does
    /// not ask, when it is one.
    ///
    /// `None` is a real divergence. `Some` is a **Group G** line: the
    /// rows stay in the count and in the denominator, and the line says
    /// why they are not being treated as a wire decision.
    not_asked: Option<NotAsked>,
    /// The octet at which each row of the group first parts from ours.
    ///
    /// Beside the shape rather than in it: a difference that appears at
    /// octet 4 in one construct and octet 40 in another is one
    /// difference in two types, and grouping by the offset would split
    /// it into two findings that are not two findings.
    octets: BTreeSet<usize>,
}

/// What one system's column holds, in the shape its own section prints.
#[derive(Default)]
struct Profile {
    /// Rows where it produced octets.
    comparable: usize,
    /// Constructs it produced octets for.
    cases: BTreeSet<String>,
    /// Its refusals, grouped by the sentence it gave.
    refusals: BTreeMap<String, BTreeSet<String>>,
    /// What it could not be asked at all, by word and row count.
    structural: BTreeMap<&'static str, usize>,
    /// Rows where it and this project's default both produced octets
    /// and the octets are not the same.
    diverging: usize,
    /// Rows whose octets no setting of this project reproduces, by
    /// construct, with the first row number that shows it.
    unalignable: BTreeMap<String, (usize, u32)>,
    /// Constructs it refuses that this project encodes.
    refused_that_we_take: BTreeSet<String>,
    /// Constructs it encodes that this project refuses.
    taken_that_we_refuse: BTreeSet<String>,
    /// Rows where the read direction gave back something other than the
    /// sample.
    read_different: usize,
}

/// A Markdown pipe table, read back out of a document.
struct MarkdownTable {
    header: Vec<String>,
    rows: Vec<Vec<String>>,
}

/// Everything the page needs that is not in the ledger.
pub struct Documents {
    /// `docs/idl-building-blocks.md`, verbatim.
    pub building_blocks: String,
    /// Every corpus case's README, by case name.
    pub readmes: BTreeMap<String, String>,
}

/// Read the documents the page is derived from.
///
/// # Errors
///
/// When `docs/idl-building-blocks.md` is not there. A README is each
/// case's own and its absence costs an issue number; the building-block
/// document is half of the acceptance answer, and rendering that half
/// as empty would read as "nothing to report".
pub fn documents(root: &std::path::Path, cases: &[Case]) -> Result<Documents, String> {
    let path = root.join("docs").join("idl-building-blocks.md");
    let building_blocks =
        std::fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut readmes = BTreeMap::new();
    for case in cases {
        let readme = root
            .join("tests")
            .join("interop-corpus")
            .join(&case.name)
            .join("README.md");
        if let Ok(text) = std::fs::read_to_string(readme) {
            readmes.insert(case.name.clone(), text);
        }
    }
    Ok(Documents {
        building_blocks,
        readmes,
    })
}

/// What the page says about each system's remainder, as lines for a
/// terminal.
///
/// The same figures the page carries, printed by `--page` so that a run
/// that changed one of them says which without anybody opening a
/// browser.
///
/// # Errors
///
/// Never; the signature matches [`render`]'s inputs so that the two
/// cannot be given different ones.
pub fn summary(
    ledger: &Ledger,
    cases: &[Case],
    sweeps: &BTreeMap<String, CaseSweep>,
    ids: &mut RowIds,
) -> String {
    let index: BTreeMap<&str, usize> = cases
        .iter()
        .enumerate()
        .map(|(at, case)| (case.name.as_str(), at))
        .collect();
    let rows = collect(ledger, &index, sweeps, ids);
    let alignment = align(&rows);
    let profiles = profile(&rows, ledger);
    let best = best_settings(&alignment);
    let mut out = String::new();
    for (column, system) in SYSTEMS.iter().enumerate() {
        let Some(profile) = profiles.get(column) else {
            continue;
        };
        if profile.comparable == 0 {
            continue;
        }
        let line = best.get(column).copied().unwrap_or(0);
        let Some(configuration) = CONFIGURATIONS.get(line) else {
            continue;
        };
        let aligned = alignment
            .get(line)
            .and_then(|line| line.get(column))
            .map_or(0, |cell| cell.aligned);
        let _ = writeln!(
            out,
            "{:<15} {:>5} written, {:>4} unmatched under {}",
            system.id,
            profile.comparable,
            profile.comparable.saturating_sub(aligned),
            configuration.flag
        );
        for group in remainders(&rows, cases, column, line) {
            let _ = writeln!(
                out,
                "                {:>4} rows, {:>3} constructs — {} [{}]",
                group.rows,
                group.cases.len(),
                group.shape,
                if group.unreached == group.rows {
                    "no setting reaches these".to_string()
                } else {
                    // The rows nothing reaches are named even where some
                    // setting reaches the rest of the group: a group
                    // listed only by what does reach it reads as
                    // covered, and the number that matters is the one
                    // left over.
                    let mut parts: Vec<String> = group
                        .reproduced_by
                        .iter()
                        .map(|(name, count)| format!("{name} {count}"))
                        .collect();
                    if group.unreached > 0 {
                        parts.push(format!("{} reached by nothing", group.unreached));
                    }
                    parts.join(", ")
                }
            );
        }
    }
    out
}

/// Render the whole page.
///
/// # Errors
///
/// When a document the page quotes has changed shape under this reader.
pub fn render(
    ledger: &Ledger,
    cases: &[Case],
    sweeps: &BTreeMap<String, CaseSweep>,
    ids: &mut RowIds,
    documents: &Documents,
    base: &str,
) -> Result<String, String> {
    let index: BTreeMap<&str, usize> = cases
        .iter()
        .enumerate()
        .map(|(at, case)| (case.name.as_str(), at))
        .collect();
    let rows = collect(ledger, &index, sweeps, ids);
    let alignment = align(&rows);
    let mut profiles = profile(&rows, ledger);
    unalignable(&rows, &mut profiles);
    let issues = issues(cases, documents, ledger);

    let mut out = String::with_capacity(4 << 20);
    out.push_str("<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n");
    out.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    let _ = writeln!(
        out,
        "<title>{} IDL constructs across twelve implementations</title>",
        cases.len()
    );
    let _ = write!(out, "<style>\n{CSS}</style>\n</head>\n<body>\n");

    out.push_str(&masthead(cases.len(), rows.len(), &profiles));
    out.push_str(&navigation());
    out.push_str("<main>\n");
    out.push_str(&acceptance(cases, &profiles, documents)?);
    out.push_str(&dialects(&alignment, &profiles));
    out.push_str(&reading(base));
    out.push_str(&system_sections(&profiles, &alignment, &rows, cases, base));
    out.push_str(&tracker(&issues, &rows, cases));
    out.push_str(&appendix(cases.len(), rows.len()));
    out.push_str("</main>\n");
    out.push_str(&footer());
    out.push_str(&payload(&rows, cases, &issues, documents, base));
    let _ = write!(out, "<script>\n{SCRIPT}</script>\n</body>\n</html>\n");
    Ok(out)
}

// ----------------------------------------------------------------- rows

fn collect<'a>(
    ledger: &'a Ledger,
    index: &BTreeMap<&str, usize>,
    sweeps: &BTreeMap<String, CaseSweep>,
    ids: &mut RowIds,
) -> Vec<Row<'a>> {
    let mut grouped: BTreeMap<String, Vec<&Entry>> = BTreeMap::new();
    let mut order: Vec<String> = Vec::new();
    for entry in &ledger.entries {
        let key = entry.row_key();
        if !grouped.contains_key(&key) {
            order.push(key.clone());
        }
        grouped.entry(key).or_default().push(entry);
    }
    let mut rows = Vec::with_capacity(order.len());
    for key in &order {
        let Some(entries) = grouped.get(key) else {
            continue;
        };
        let Some(first) = entries.first() else {
            continue;
        };
        let sweep = sweeps.get(&first.case);
        let (diff, mitigation) = tables::analysis(Direction::Write, entries, sweep);
        let slot = format!("{}/{}", first.encoding, first.endian);
        let settings = CONFIGURATIONS
            .iter()
            .map(|configuration| {
                match sweep
                    .and_then(|sweep| sweep.get(configuration.name))
                    .and_then(|slots| slots.get(&slot))
                {
                    Some((Outcome::Bytes(hex), _)) => Some(hex.clone()),
                    _ => None,
                }
            })
            .collect();
        rows.push(Row {
            id: ids.number(key),
            case: index.get(first.case.as_str()).copied().unwrap_or(0),
            encoding: first.encoding,
            endian: first.endian,
            entries: entries.clone(),
            diff,
            mitigation,
            settings,
        });
    }
    rows
}

/// The dialect matrix: for every setting and every system, how many of
/// that system's byte strings this project reproduces exactly.
fn align(rows: &[Row]) -> Vec<Vec<Alignment>> {
    let mut matrix = vec![vec![Alignment::default(); SYSTEMS.len()]; CONFIGURATIONS.len()];
    for row in rows {
        for (column, system) in SYSTEMS.iter().enumerate() {
            let mask = row.reproduce(system.id);
            for line in 0..CONFIGURATIONS.len() {
                if mask & (1 << line) == 0 {
                    continue;
                }
                if let Some(cell) = matrix.get_mut(line).and_then(|line| line.get_mut(column)) {
                    cell.aligned += 1;
                }
            }
        }
    }
    matrix
}

/// The rows a setting does not reproduce for a system, grouped by what
/// is different about the octets.
///
/// This is the number the table leads with, so it is the number that
/// has to carry an explanation. Each group says what is different, how
/// many rows and which constructs, and — the actionable part — whether
/// some other setting of this project does produce those octets. A
/// group nothing reaches is a different finding from a group another
/// flag reaches, and only the first is a candidate defect.
/// A question this comparison deliberately does not ask, and why.
///
/// **The line is measurable and is drawn from the octets, not from a
/// list of constructs.** The test is whether the difference reaches a
/// reader: a difference in *length* moves every field behind it, so it
/// always does; a difference of equal length in octets this project
/// writes as `00` — padding — never does, because a reader steps over
/// padding without looking at it.
///
/// Only the second is a question that cannot be asked. It stays out of
/// the divergence count and out of the denominator both, and it renders
/// as its own line carrying the reason and where the reason was
/// measured.
///
/// **What used to be in here and is not any more.** Two groups were set
/// aside on a *cause* rather than on that test: the enumeration holder
/// width (Fast DDS and OpenDDS, `@bit_bound` dropped by the front end)
/// and Cyclone's union with a negative label on a narrow discriminator.
/// Both explanations are correct and neither answers this question. A
/// peer that writes four octets for an `@bit_bound(8)` enumeration
/// writes four octets whatever the reason, and a reader of this
/// project's one octet is four octets out of step from there on. The
/// cause belongs in `docs/wire-axes-followup.md`; the row belongs in
/// the count, and it needs a setting. `EnumHolderWidth` and
/// `UnionLabelMatch` are those settings, and they are named for the
/// defect they reproduce so that nobody reads them as a reading of the
/// specification.
struct NotAsked {
    /// The name the line renders under.
    group: &'static str,
    /// Why the question is not asked.
    why: &'static str,
    /// Where the reason was measured, so the line is checkable.
    measured: &'static str,
}

/// What is left in the octets neither side's serialiser writes.
///
/// XTypes §7.4.1 makes no requirement of their content, and **neither
/// peer in this group determines them**: RTI Connext 7.7.0 and omniORB
/// 4.3.3 both advance the stream pointer over an alignment gap without
/// assigning it, so what a reader receives is whatever was in the
/// caller's buffer. `docs/wire-axes-followup.md`, group G, names the
/// function and line in each and carries the two experiments that
/// establish it.
///
/// Three criteria decide the group and all three are checked, not
/// asserted — see [`not_asked`]. The layout criterion is the one that
/// establishes the *positions*;
/// `tools/interop-results/tests/padding_positions.rs` is where the
/// three are held against each other over the whole corpus.
const PADDING_CONTENT: NotAsked = NotAsked {
    group: "what is left in padding",
    why: "§7.4.1 says nothing about the content of padding, and neither peer in this group \
          writes those octets at all — both advance over the alignment gap and leave the \
          caller's buffer, which is why omniORB delivers different octets between two runs. \
          Every differing octet is at an offset this project's own layout puts fill at, no \
          row differs in length, and decoding the peer's octets returns the case's sample",
    measured: "docs/wire-axes-followup.md, group G — the source line in each implementation, \
               the installed version, and the two experiments; \
               tools/interop-results/tests/padding_positions.rs holds the layout criterion, \
               the octet rule and the decode against each other over the whole corpus",
};

/// The octet rule: same length, and every differing octet is one this
/// project wrote as `00`.
///
/// What it is **not**: a statement about where the fill positions are.
/// `00` is what this project writes wherever it has nothing to say, and
/// that includes a zero value and a length of zero. That is why it is
/// one of three criteria and not the criterion.
///
/// `false` when nothing differs — two equal byte strings are not a row
/// this classifies at all.
fn differs_only_in_our_zero_octets(mine: &str, theirs: &str) -> bool {
    // A length difference moves every field behind it. There is no
    // reading under which that reaches no reader.
    if mine.len() != theirs.len() {
        return false;
    }
    let ours = mine.as_bytes();
    let them = theirs.as_bytes();
    let mut differed = false;
    for octet in 0..ours.len() / 2 {
        let a = &ours[octet * 2..octet * 2 + 2];
        let b = &them[octet * 2..octet * 2 + 2];
        if a == b {
            continue;
        }
        differed = true;
        // A differing octet this project wrote a *value* into is a
        // divergence in a value, whatever its shape.
        if a != b"00" {
            return false;
        }
    }
    differed
}

/// Whether this row's difference reaches a reader, and if not, why not.
///
/// **Three criteria, and all three have to hold.** They are independent
/// and each is capable of accepting a row the other two refuse:
///
/// 1. [`differs_only_in_our_zero_octets`] — the octets, and cheap. It
///    knows what this project wrote and not where the fill is.
/// 2. [`rust_column::differs_only_where_the_plan_pads`] — the
///    **layout**. Every differing offset has to be one §7.4.1's
///    alignment rules put fill at, computed by the planner and the
///    writer rather than read off the difference. A peer octet one
///    position outside the fill is a value, and criterion 1 cannot see
///    that.
/// 3. [`reads_back_as_the_sample`] — the reader. The peer's own octets
///    through this project's decoder have to give the case's own
///    sample.
///
/// A construct list would answer a different question — *why* the peer
/// writes what it writes — and the result tables exist for the first
/// question.
fn not_asked(
    mine: Option<&str>,
    theirs: &str,
    case: &Case,
    configuration: &str,
    encoding: &str,
    endian: &str,
) -> Option<NotAsked> {
    let mine = mine?;
    if !differs_only_in_our_zero_octets(mine, theirs) {
        return None;
    }
    // The layout's own answer, and the one criterion that is not read
    // off the octets at all.
    if !rust_column::differs_only_where_the_plan_pads(
        mine,
        theirs,
        case,
        configuration,
        encoding,
        endian,
    ) {
        return None;
    }
    // The octet rule is a claim about what a reader sees, so the reader
    // is asked. The peer's own octets go through this project's decoder
    // under the plan its bytes were compared against, and the values
    // that come out have to be the case's own. A row that fails here is
    // a divergence and is counted as one, whatever its shape.
    if reads_back_as_the_sample(case, configuration, encoding, endian, theirs) {
        Some(PADDING_CONTENT)
    } else {
        None
    }
}

/// One row's answer from each of the three padding criteria, side by
/// side.
///
/// The audit exists because "the three agree" is a claim about the
/// corpus and not a property of the code: each criterion can accept a
/// row the others refuse, and a disagreement is the finding.
#[derive(Debug, Clone)]
pub struct PaddingVerdict {
    /// The appendix row number.
    pub row: u32,
    /// The case.
    pub case: String,
    /// The peer.
    pub system: &'static str,
    /// The encoding.
    pub encoding: &'static str,
    /// The byte order.
    pub endian: &'static str,
    /// The configuration this peer's row was compared against.
    pub configuration: &'static str,
    /// This project's octets.
    pub mine: String,
    /// The peer's octets.
    pub theirs: String,
    /// Same length, every differing octet one this project wrote `00`.
    pub octet_rule: bool,
    /// Every differing offset is a fill position of the plan.
    pub layout_rule: bool,
    /// The peer's octets decode to the case's own sample.
    pub reader_rule: bool,
}

impl PaddingVerdict {
    /// Whether the criteria answer as the classification requires.
    ///
    /// **Two of the three answer the same question and the third does
    /// not.** The octet rule and the layout rule both name *positions*,
    /// from two independent derivations, so they have to agree on every
    /// row: one accepting where the other refuses is a position one of
    /// them has wrong.
    ///
    /// The reader is a weaker condition and is not required to refuse
    /// where they do. A decoder is insensitive to things a byte
    /// comparison is not — a `map` whose entries arrive in another
    /// order yields the same sample and is a difference in every octet
    /// of the entries — so a row the reader alone accepts is a
    /// divergence and is counted as one. What is required of it is the
    /// other direction: wherever the two position rules call a row
    /// padding, the reader has to get the sample back.
    #[must_use]
    pub const fn agreed(&self) -> bool {
        self.octet_rule == self.layout_rule && (!self.octet_rule || self.reader_rule)
    }

    /// The line a report prints.
    #[must_use]
    pub fn line(&self) -> String {
        format!(
            "row {} {}/{}/{}/{} under {}: octets {}, layout {}, reader {}\n  ours   {}\n  theirs {}",
            self.row,
            self.case,
            self.system,
            self.encoding,
            self.endian,
            self.configuration,
            self.octet_rule,
            self.layout_rule,
            self.reader_rule,
            self.mine,
            self.theirs
        )
    }
}

/// Every row a peer wrote that no setting reproduces, with each of the
/// three padding criteria's answer.
///
/// The enumeration is `remainders`'s — the same rows, the same
/// per-system best setting — so a row this reports and a row the page
/// counts are the same row.
///
/// # Errors
///
/// Never; the signature matches [`render`]'s inputs so that the two
/// cannot be given different ones.
#[must_use]
pub fn padding_audit(
    ledger: &Ledger,
    cases: &[Case],
    sweeps: &BTreeMap<String, CaseSweep>,
    ids: &mut RowIds,
) -> Vec<PaddingVerdict> {
    let index: BTreeMap<&str, usize> = cases
        .iter()
        .enumerate()
        .map(|(at, case)| (case.name.as_str(), at))
        .collect();
    let rows = collect(ledger, &index, sweeps, ids);
    let best = best_settings(&align(&rows));
    let mut out = Vec::new();
    for (column, system) in SYSTEMS.iter().enumerate() {
        let line = best.get(column).copied().unwrap_or(0);
        let Some(configuration) = CONFIGURATIONS.get(line) else {
            continue;
        };
        for row in &rows {
            let Some(entry) = row.entries.iter().find(|entry| entry.system == system.id) else {
                continue;
            };
            let Outcome::Bytes(theirs) = &entry.write else {
                continue;
            };
            if row.reproduce(system.id) & (1 << line) != 0 {
                continue;
            }
            let Some(mine) = row.settings.get(line).and_then(Option::as_deref) else {
                continue;
            };
            let Some(case) = cases.get(row.case) else {
                continue;
            };
            let octet_rule = differs_only_in_our_zero_octets(mine, theirs);
            let layout_rule = rust_column::differs_only_where_the_plan_pads(
                mine,
                theirs,
                case,
                configuration.name,
                row.encoding,
                row.endian,
            );
            let reader_rule = reads_back_as_the_sample(
                case,
                configuration.name,
                row.encoding,
                row.endian,
                theirs,
            );
            if !octet_rule && !layout_rule && !reader_rule {
                continue;
            }
            out.push(PaddingVerdict {
                row: row.id,
                case: entry.case.clone(),
                system: system.id,
                encoding: row.encoding,
                endian: row.endian,
                configuration: configuration.name,
                mine: mine.to_string(),
                theirs: theirs.clone(),
                octet_rule,
                layout_rule,
                reader_rule,
            });
        }
    }
    out
}

fn remainders(rows: &[Row], cases: &[Case], column: usize, line: usize) -> Vec<Remainder> {
    let Some(system) = SYSTEMS.get(column) else {
        return Vec::new();
    };
    let mut groups: BTreeMap<String, Remainder> = BTreeMap::new();
    for row in rows {
        let Some(entry) = row.entries.iter().find(|entry| entry.system == system.id) else {
            continue;
        };
        let Outcome::Bytes(theirs) = &entry.write else {
            continue;
        };
        let mask = row.reproduce(system.id);
        if mask & (1 << line) != 0 {
            continue;
        }
        let mine = row.settings.get(line).and_then(Option::as_deref);
        let (shape, octet) = shape_of(mine, theirs);
        let set_aside = match (cases.get(row.case), CONFIGURATIONS.get(line)) {
            (Some(case), Some(configuration)) => not_asked(
                mine,
                theirs,
                case,
                configuration.name,
                row.encoding,
                row.endian,
            ),
            _ => None,
        };
        // Grouped by the question as well as by the shape: two rows of
        // the same shape, one of which is a question not asked and one
        // of which is a divergence, are not one finding, and a line
        // that mixed them would carry a reason true of only half of it.
        let key = match &set_aside {
            Some(reason) => format!("{shape} — {}", reason.group),
            None => shape.clone(),
        };
        let group = groups.entry(key).or_insert_with(|| Remainder {
            shape,
            rows: 0,
            cases: BTreeMap::new(),
            reproduced_by: BTreeMap::new(),
            unreached: 0,
            not_asked: set_aside,
            octets: BTreeSet::new(),
        });
        group.rows += 1;
        group.cases.entry(entry.case.clone()).or_insert(row.id);
        if let Some(octet) = octet {
            group.octets.insert(octet);
        }
        if mask == 0 {
            group.unreached += 1;
        } else {
            for (other, configuration) in CONFIGURATIONS.iter().enumerate() {
                if mask & (1 << other) != 0 {
                    *group.reproduced_by.entry(configuration.name).or_default() += 1;
                }
            }
        }
    }
    let mut out: Vec<Remainder> = groups.into_values().collect();
    out.sort_by_key(|group| (usize::MAX - group.rows, group.shape.clone()));
    out
}

/// What is different about two byte strings: a phrase, and the octet at
/// which they first part.
///
/// Measured and never guessed at. A length difference is a length
/// difference and an offset is an offset; *why* they part is a clause
/// question, and a clause question belongs in
/// `docs/wire-feature-inventory.md`, which cites one per row.
fn shape_of(mine: Option<&str>, theirs: &str) -> (String, Option<usize>) {
    let Some(mine) = mine else {
        return (
            "this project produces nothing for the construct".to_string(),
            None,
        );
    };
    let ours = mine.len() / 2;
    let them = theirs.len() / 2;
    let first = mine
        .as_bytes()
        .iter()
        .zip(theirs.as_bytes())
        .position(|(a, b)| a != b)
        .map(|nibble| nibble / 2);
    let phrase = match them.cmp(&ours) {
        std::cmp::Ordering::Greater => format!("their payload is {} octets longer", them - ours),
        std::cmp::Ordering::Less => format!("their payload is {} octets shorter", ours - them),
        // Equal length and no differing octet is equality, which this
        // function is never called with; it is named rather than left
        // to a catch-all so that a caller that does reach it says
        // something wrong out loud instead of printing a plausible
        // phrase.
        std::cmp::Ordering::Equal if first.is_none() => {
            "identical, which means this page has a defect".to_string()
        }
        std::cmp::Ordering::Equal => "the same length, different octets".to_string(),
    };
    (phrase, first)
}

fn profile(rows: &[Row], ledger: &Ledger) -> Vec<Profile> {
    let mut profiles: Vec<Profile> = SYSTEMS.iter().map(|_| Profile::default()).collect();
    // Which constructs this project's Rust column encodes at all: the
    // other half of "can a peer do something we cannot".
    let mut ours: BTreeSet<&str> = BTreeSet::new();
    for entry in &ledger.entries {
        if entry.system == "zerodds-rust" && matches!(entry.write, Outcome::Bytes(_)) {
            ours.insert(entry.case.as_str());
        }
    }
    for row in rows {
        for (column, system) in SYSTEMS.iter().enumerate() {
            let Some(entry) = row.entries.iter().find(|entry| entry.system == system.id) else {
                continue;
            };
            let Some(profile) = profiles.get_mut(column) else {
                continue;
            };
            if matches!(entry.read, Outcome::Different(_)) {
                profile.read_different += 1;
            }
            match &entry.write {
                Outcome::Bytes(theirs) => {
                    profile.comparable += 1;
                    profile.cases.insert(entry.case.clone());
                    if row.ours().is_some_and(|mine| mine != theirs) {
                        profile.diverging += 1;
                    }
                    if !ours.contains(entry.case.as_str()) {
                        profile.taken_that_we_refuse.insert(entry.case.clone());
                    }
                }
                Outcome::Refused(reason) => {
                    profile
                        .refusals
                        .entry(reason.clone())
                        .or_default()
                        .insert(entry.case.clone());
                    if ours.contains(entry.case.as_str()) {
                        profile.refused_that_we_take.insert(entry.case.clone());
                    }
                }
                other => {
                    *profile.structural.entry(other.word()).or_default() += 1;
                }
            }
        }
    }
    profiles
}

/// Rows a system produced that no setting of this project reproduces.
///
/// The same search the mitigation column makes, kept rather than
/// repeated: [`Row::reproduce`] is one pass over the twelve settings and
/// every figure on this page that talks about coverage reads that one
/// mask. Two searches of one space can disagree; one cannot.
fn unalignable(rows: &[Row], profiles: &mut [Profile]) {
    for row in rows {
        for (column, system) in SYSTEMS.iter().enumerate() {
            if system.kind == Kind::Own {
                continue;
            }
            let Some(entry) = row.entries.iter().find(|entry| entry.system == system.id) else {
                continue;
            };
            if !matches!(entry.write, Outcome::Bytes(_)) || row.reproduce(system.id) != 0 {
                continue;
            }
            if let Some(profile) = profiles.get_mut(column) {
                let slot = profile
                    .unalignable
                    .entry(entry.case.clone())
                    .or_insert((0, row.id));
                slot.0 += 1;
            }
        }
    }
}

/// Which reported issues a construct answers.
///
/// Two sources, both in the tree. A case built from a bug report names
/// the report in its own README. Every case whose front end draws the
/// diagnostic that case draws has the same cause, so a reporter who
/// lands on any of them gets the same answer — which is the difference
/// between a matrix and a reply.
/// Whether the `#` at `at` is the one in `issue #6` or `issues #6`.
///
/// Only the word matters, not the case of it, and only one space is
/// allowed between: `issue #6` and `Issue #6` are citations, `§7 #6` is
/// not.
fn preceded_by_the_word_issue(characters: &[char], at: usize) -> bool {
    let mut run: Vec<char> = characters[..at]
        .iter()
        .rev()
        .copied()
        .take_while(|character| character.is_ascii_alphabetic() || *character == ' ')
        .collect();
    run.reverse();
    let before: String = run.into_iter().collect();
    if !before.ends_with(' ') {
        return false;
    }
    let word = before
        .trim_end_matches(' ')
        .rsplit(' ')
        .next()
        .unwrap_or("");
    word.eq_ignore_ascii_case("issue") || word.eq_ignore_ascii_case("issues")
}

fn issues(cases: &[Case], documents: &Documents, ledger: &Ledger) -> BTreeMap<String, Vec<u32>> {
    let mut direct: BTreeMap<String, Vec<u32>> = BTreeMap::new();
    for case in cases {
        let Some(readme) = documents.readmes.get(&case.name) else {
            continue;
        };
        let mut numbers: BTreeSet<u32> = BTreeSet::new();
        let characters: Vec<char> = readme.chars().collect();
        for (at, character) in characters.iter().enumerate() {
            if *character != '#' {
                continue;
            }
            let digits: String = characters
                .iter()
                .skip(at + 1)
                .take_while(|character| character.is_ascii_digit())
                .collect();
            // One digit is a heading level or a section number far more
            // often than it is an issue; two or more is a citation. But
            // the tracker's first two years are single digits — #6 is
            // an IDL parser defect — and a rule that cannot cite them
            // makes those reports unanswerable from this page whatever
            // the corpus holds. A single digit counts when the word
            // before it says what it is.
            if digits.is_empty() {
                continue;
            }
            if digits.len() < 2 && !preceded_by_the_word_issue(&characters, at) {
                continue;
            }
            if let Ok(number) = digits.parse::<u32>() {
                numbers.insert(number);
            }
        }
        if !numbers.is_empty() {
            direct.insert(case.name.clone(), numbers.into_iter().collect());
        }
    }
    let mut family: BTreeSet<String> = BTreeSet::new();
    for (key, lines) in &ledger.warnings {
        let Some((case, system)) = key.split_once('/') else {
            continue;
        };
        if system != "cyclone" {
            continue;
        }
        if lines
            .iter()
            .any(|line| line.contains("No default extensibility provided"))
        {
            family.insert(case.to_string());
        }
    }
    let mut family_numbers: BTreeSet<u32> = BTreeSet::new();
    for case in &family {
        if let Some(numbers) = direct.get(case) {
            family_numbers.extend(numbers.iter().copied());
        }
    }
    let mut out = direct;
    if !family_numbers.is_empty() {
        let numbers: Vec<u32> = family_numbers.into_iter().collect();
        for case in family {
            out.entry(case).or_insert_with(|| numbers.clone());
        }
    }
    out
}

// ---------------------------------------------------------------- prose

fn masthead(cases: usize, rows: usize, profiles: &[Profile]) -> String {
    let octets: usize = profiles.iter().map(|profile| profile.comparable).sum();
    format!(
        "<header>\n<h1>{cases} IDL constructs, twelve implementations, and the flag that \
         aligns them</h1>\n\
         <p class=\"lede\">Every construct in this project's interop corpus, put through twelve \
         independent implementations in five encodings and both byte orders: {rows} rows, \
         {} cells, {octets} of them carrying octets a real serialiser really produced. \
         The page answers three questions in this order — whether any of the other eleven \
         encodes something this one cannot, which setting makes this project's octets equal to \
         a given peer's, and what each peer does on its own. The rows are the \
         <a href=\"#rows\">appendix</a>.</p>\n\
         <p class=\"lede\">If you arrived from a bug report: the row it concerns has its own \
         address, the <a href=\"#dialects\">dialect matrix</a> is the one-line answer to \
         \"which flag\", and <a href=\"#issues\">reported issues</a> lists the ones already \
         mapped onto rows.</p>\n</header>\n",
        rows * SYSTEMS.len()
    )
}

fn navigation() -> String {
    let mut out = String::from("<nav aria-label=\"sections\"><ol>\n");
    for (anchor, title) in [
        ("answer", "Can a peer do something we cannot?"),
        ("dialects", "Which flag do I set?"),
        ("reading", "How to read a cell"),
        ("systems", "The twelve, one at a time"),
        ("issues", "Reported issues these rows answer"),
        ("rows", "Appendix: every row"),
    ] {
        let _ = writeln!(out, "<li><a href=\"#{anchor}\">{title}</a></li>");
    }
    out.push_str("</ol></nav>\n");
    out
}

fn acceptance(
    cases: &[Case],
    profiles: &[Profile],
    documents: &Documents,
) -> Result<String, String> {
    let mine = SYSTEMS
        .iter()
        .position(|system| system.id == "zerodds-rust")
        .and_then(|column| profiles.get(column));
    let mut nobody: Vec<&str> = Vec::new();
    let mut ours = 0_usize;
    for case in cases {
        let taken_by_us = mine.is_some_and(|profile| profile.cases.contains(&case.name));
        if taken_by_us {
            ours += 1;
        }
        let taken_by_them = SYSTEMS.iter().enumerate().any(|(column, system)| {
            system.kind != Kind::Own
                && profiles
                    .get(column)
                    .is_some_and(|profile| profile.cases.contains(&case.name))
        });
        if !taken_by_us && !taken_by_them {
            nobody.push(&case.name);
        }
    }
    let gap: BTreeSet<&String> = SYSTEMS
        .iter()
        .enumerate()
        .filter(|(_, system)| system.kind != Kind::Own)
        .filter_map(|(column, _)| profiles.get(column))
        .flat_map(|profile| profile.taken_that_we_refuse.iter())
        .collect();

    let mut out =
        String::from("<section id=\"answer\">\n<h2>Can a peer do something we cannot?</h2>\n");
    let _ = writeln!(
        out,
        "<p class=\"verdict {}\">On the wire, no. Of the {} constructs measured, \
         <strong>{}</strong> are encoded by one of the other eleven implementations and refused \
         by this one. This project encodes {ours} of {}. The {} nothing encodes — {} — are \
         refused by all twelve alike, because the OMG IDL-to-C++ and IDL-to-Java mappings give \
         <code>char</code> and <code>wchar</code> no room for the value the case carries.</p>",
        if gap.is_empty() { "good" } else { "bad" },
        cases.len(),
        gap.len(),
        cases.len(),
        nobody.len(),
        nobody
            .iter()
            .map(|name| format!("<code>{}</code>", escape(name)))
            .collect::<Vec<_>>()
            .join(" and ")
    );
    out.push_str(
        "<p>That is the answer for the corpus, and the corpus is a corpus of <em>data \
         types</em>: things that can be declared, filled with a value and put on a wire. The \
         question has a second half — what each project's IDL compiler will accept at all — and \
         there the answer is not the same one. Both halves are below.</p>\n",
    );

    out.push_str("<h3 id=\"answer-corpus\">What each system took, and what it turned away</h3>\n");
    out.push_str(
        "<p>A construct is <em>taken</em> by a system when that system produced octets for it \
         in at least one encoding and byte order. The last two columns are the question above, \
         asked per system. A system that could not be asked at all — no encoding version 2, one \
         byte order, no form for the construct in its type model — is not refusing anything, \
         and those rows are counted apart in <a href=\"#systems\">its own section</a>.</p>\n",
    );
    out.push_str("<div class=\"scroller\"><table class=\"grid\">\n");
    out.push_str(
        "<thead><tr><th>system</th><th>what it is</th><th class=\"n\">constructs taken</th>\
         <th class=\"n\">rows with octets</th><th class=\"n\">it takes, we refuse</th>\
         <th class=\"n\">we take, it refuses</th></tr></thead>\n<tbody>\n",
    );
    for (column, system) in SYSTEMS.iter().enumerate() {
        let Some(profile) = profiles.get(column) else {
            continue;
        };
        let _ = writeln!(
            out,
            "<tr><th scope=\"row\"><a href=\"#{}\">{}</a></th><td>{}</td>\
             <td class=\"n\">{} <span class=\"of\">of {}</span></td><td class=\"n\">{}</td>\
             <td class=\"n {}\">{}</td><td class=\"n\">{}</td></tr>",
            system.id,
            escape(system.label),
            escape(system.version),
            profile.cases.len(),
            cases.len(),
            profile.comparable,
            if profile.taken_that_we_refuse.is_empty() {
                "good"
            } else {
                "bad"
            },
            profile.taken_that_we_refuse.len(),
            profile.refused_that_we_take.len()
        );
    }
    out.push_str("</tbody></table></div>\n");
    out.push_str(&building_blocks(documents)?);
    out.push_str("</section>\n");
    Ok(out)
}

/// The front-end half of the acceptance answer, parsed out of
/// `docs/idl-building-blocks.md`.
fn building_blocks(documents: &Documents) -> Result<String, String> {
    let ours = markdown_table(&documents.building_blocks, "not taken yet")?;
    let foreign = markdown_table(&documents.building_blocks, "rtiddsgen")?;
    // The rules this front end did not take yet were a table of what each
    // one would cost. All sixteen are closed, and the document replaced
    // that table with what closed them. Either shape is rendered and
    // **neither** is not: a document that carries no such table has
    // changed in a way this page cannot report, and a page that quietly
    // dropped the section would read as "there is nothing to say".
    let (costs, costs_summary) =
        match markdown_table(&documents.building_blocks, "what taking it costs") {
            Ok(table) => (
                table,
                "The rules this compiler does not take yet, and what taking each one costs",
            ),
            Err(open) => match markdown_table(&documents.building_blocks, "what closed it") {
                Ok(table) => (
                    table,
                    "The rules this compiler did not take, and what closed each",
                ),
                Err(closed) => return Err(format!("{open}; and {closed}")),
            },
        };

    let mut out = String::from(
        "<h3 id=\"answer-frontend\">Which half of the language each compiler accepts</h3>\n\
         <p>OMG IDL 4.2 is not one language. §7.4 divides it into fifteen <em>building \
         blocks</em>, rules (1) to (227), and §9 names the profiles — sets of blocks — that \
         middleware is written against. Most implementations pick a profile. This project takes \
         all fifteen, so the column that matters is the last one: the rules it does not take \
         yet, each of which is an item on a list rather than a boundary.</p>\n",
    );
    out.push_str(&render_table(&ours, &[]));

    let mut wider: Vec<String> = Vec::new();
    for row in &foreign.rows {
        let Some(block) = row.get(1) else {
            continue;
        };
        let Some(mine) = ours.rows.iter().find(|line| line.get(1) == Some(block)) else {
            continue;
        };
        if mine
            .get(3)
            .is_some_and(|verdict| verdict.contains("complete"))
        {
            continue;
        }
        let takers: Vec<String> = foreign
            .header
            .iter()
            .enumerate()
            .skip(2)
            .filter(|(at, _)| row.get(*at).is_some_and(|cell| cell.trim() == "yes"))
            .map(|(_, name)| inline(name))
            .collect();
        if takers.is_empty() {
            continue;
        }
        wider.push(format!(
            "<li><strong>{}</strong> — accepted whole by {}. Here: {} ({})</li>",
            inline(block),
            takers.join(", "),
            inline(mine.get(3).map_or("", String::as_str)),
            inline(mine.get(4).map_or("", String::as_str))
        ));
    }
    if wider.is_empty() {
        out.push_str(
            "<p class=\"verdict good\">No building block is accepted whole by a foreign \
             compiler and not by this one.</p>\n",
        );
    } else {
        let _ = writeln!(
            out,
            "<p class=\"verdict bad\">{} building block{} accepted whole by at least one \
             foreign compiler and not by this one. This is where the answer to the question at \
             the top of the page is <em>yes</em> — and it is a list of work, not a boundary, \
             because none of these has a reason that survives the standard:</p>\n\
             <ul class=\"findings\">{}</ul>",
            wider.len(),
            if wider.len() == 1 { " is" } else { "s are" },
            wider.join("\n")
        );
    }
    let _ = writeln!(out, "<details><summary>{costs_summary}</summary>");
    out.push_str(&render_table(&costs, &[]));
    out.push_str("</details>\n");
    out.push_str(
        "<details><summary>The same fifteen blocks through eight foreign compilers</summary>\n",
    );
    out.push_str(&render_table(&foreign, &["yes", "no"]));
    out.push_str(
        "<p class=\"note\">A <em>drop</em> is a third verdict and not a soft <em>yes</em>: the \
         compiler exits 0, prints that it discarded the construct, and generates code without \
         it. Whether a construct a compiler accepted silently also survives to its output is a \
         separate measurement — and it is the one the rest of this page makes.</p>\n",
    );
    out.push_str("</details>\n");
    Ok(out)
}

fn dialects(alignment: &[Vec<Alignment>], profiles: &[Profile]) -> String {
    let mut out = String::from("<section id=\"dialects\">\n<h2>Which flag do I set?</h2>\n");
    out.push_str(
        "<p>Implementations disagree about questions the specification leaves open, and about a \
         few it does not. Rather than pick one reading and call the rest wrong, this project \
         carries every measured reading as a setting. A <em>dialect</em> is a named bundle of \
         those decisions — one per implementation whose octets were measured — and three further \
         flags cover readings no dialect holds.</p>\n",
    );
    out.push_str(
        "<h3 id=\"dialects-how\">How to read one line of it</h3>\n\
         <p>Take a column: <strong>a peer</strong>. The figure under its name is how many rows \
         that peer produced octets for at all — never 3,350, because no peer was asked every \
         row. A peer with no encoding version 2 was not asked three of the five encodings; one \
         that writes a single byte order was not asked the other; a construct its type model has \
         no form for was never put to it. None of that is disagreement, so none of it is in \
         these numbers.</p>\n\
         <p>Take a cell: <strong>how many of that peer's rows this project does not reproduce \
         under that setting</strong> — the rows where the peer wrote octets and this project, \
         under that flag, wrote different ones. <strong>Lower is better and zero is byte-for-byte \
         everywhere.</strong> The cell shows what is left over rather than what is covered, \
         because the leftover is the finding: 62 rows where RTI and this project write different \
         octets under RTI's own dialect is a list of work, and \"806 covered\" is the same fact \
         with the work hidden behind a subtraction. Every cell is a link into the rows it \
         counts, and each peer's leftover is broken down by cause in \
         <a href=\"#systems\">its own section</a>.</p>\n",
    );

    out.push_str(
        "<div class=\"scroller\"><table class=\"grid matrix\">\n<thead>\
         <tr><th>setting</th><th>flag</th>",
    );
    for system in &SYSTEMS {
        let _ = write!(
            out,
            "<th class=\"n\"><a href=\"#{}\">{}</a></th>",
            system.id,
            escape(system.label)
        );
    }
    out.push_str(
        "</tr>\n<tr class=\"denominator\"><th colspan=\"2\">rows the peer wrote at all</th>",
    );
    for column in 0..SYSTEMS.len() {
        let _ = write!(
            out,
            "<td class=\"n\">{}</td>",
            profiles.get(column).map_or(0, |profile| profile.comparable)
        );
    }
    out.push_str("</tr></thead>\n<tbody>\n");

    let best = best_settings(alignment);
    for (line, configuration) in CONFIGURATIONS.iter().enumerate() {
        let _ = write!(
            out,
            "<tr id=\"setting-{}\"><th scope=\"row\">{}{}</th><td><code>{}</code></td>",
            configuration.name,
            escape(configuration.name),
            // One tag when the two are the same configuration, which
            // they are since the default moved to the clause-by-clause
            // reading. Two tags on one row would read as two rows.
            if configuration.name == DEFAULT && configuration.name == SPECIFICATION {
                " <span class=\"tag\">shipped default</span> \
                 <span class=\"tag spec\">how it should be</span>"
            } else if configuration.name == DEFAULT {
                " <span class=\"tag\">shipped default</span>"
            } else if configuration.name == SPECIFICATION {
                " <span class=\"tag spec\">how it should be</span>"
            } else {
                ""
            },
            escape(configuration.flag)
        );
        for (column, system) in SYSTEMS.iter().enumerate() {
            let aligned = alignment
                .get(line)
                .and_then(|line| line.get(column))
                .map_or(0, |cell| cell.aligned);
            let comparable = profiles.get(column).map_or(0, |profile| profile.comparable);
            let left = comparable.saturating_sub(aligned);
            // Shaded by what is left over rather than by what is
            // covered, so that the darkest cell is the one with the
            // most work in it and the eye lands on the finding.
            let share = share(left, comparable);
            let marker = if best.get(column) == Some(&line) && comparable > 0 {
                " best"
            } else {
                ""
            };
            if comparable == 0 {
                out.push_str("<td class=\"n cover\">—</td>");
                continue;
            }
            let _ = write!(
                out,
                "<td class=\"n cover{marker}\" style=\"--fill:{share:.3}\">\
                 <button type=\"button\" class=\"filter\" data-filter=\"{}:unmatched-{}\" \
                 title=\"{left} of {}'s {comparable} rows are not reproduced by `{}`; \
                 {aligned} are. Click for the rows.\">{left}</button></td>",
                escape(system.id),
                escape(configuration.name),
                escape(system.label),
                escape(configuration.flag)
            );
        }
        out.push_str("</tr>\n");
    }
    out.push_str("</tbody></table></div>\n");
    out.push_str(
        "<p class=\"note\">A cell of <strong>0</strong> means this project writes that peer's \
         octets exactly, on every row that peer wrote. A dash means the peer wrote no octets at \
         all, so there is nothing to be equal to.</p>\n",
    );

    out.push_str("<h3 id=\"dialects-one-line\">One line per peer</h3>\n<ul class=\"findings\">\n");
    for (column, system) in SYSTEMS.iter().enumerate() {
        let Some(profile) = profiles.get(column) else {
            continue;
        };
        // The three own columns are not peers to reach; they are the
        // same plan implemented three times, and what their row says is
        // whether the three agree. That is a different sentence and it
        // comes after these.
        if profile.comparable == 0 || system.kind == Kind::Own {
            continue;
        }
        let line = best.get(column).copied().unwrap_or(0);
        let Some(configuration) = CONFIGURATIONS.get(line) else {
            continue;
        };
        let aligned = alignment
            .get(line)
            .and_then(|line| line.get(column))
            .map_or(0, |cell| cell.aligned);
        let remainder = profile.comparable.saturating_sub(aligned);
        let _ = writeln!(
            out,
            "<li><strong>{}: {} rows written, {remainder} unmatched</strong> under {} — \
             the best of the twelve. {}</li>",
            escape(system.label),
            profile.comparable,
            flag_phrase(configuration.flag),
            if remainder == 0 {
                String::from("Nothing is left over.")
            } else {
                format!(
                    "<a href=\"#{}\">What those {remainder} are</a>, and what would reach them.",
                    system.id
                )
            }
        );
    }
    out.push_str("</ul>\n");
    // The three own columns, said once.
    let own: Vec<(&'static str, usize, usize)> = SYSTEMS
        .iter()
        .enumerate()
        .filter(|(_, system)| system.kind == Kind::Own)
        .filter_map(|(column, system)| {
            let comparable = profiles.get(column)?.comparable;
            let default_line = CONFIGURATIONS
                .iter()
                .position(|configuration| configuration.name == DEFAULT)?;
            let aligned = alignment
                .get(default_line)
                .and_then(|line| line.get(column))
                .map_or(0, |cell| cell.aligned);
            Some((system.label, aligned, comparable))
        })
        .collect();
    if !own.is_empty() {
        let _ = writeln!(
            out,
            "<p class=\"note\">The last three columns are not peers to reach. They are this \
             project's own plan implemented three times over — {} — and what their numbers say \
             is whether the three agree with each other: {}. A single implementation agreeing \
             with itself would be no evidence at all.</p>",
            own.iter()
                .map(|(label, _, _)| format!("<strong>{}</strong>", escape(label)))
                .collect::<Vec<_>>()
                .join(", "),
            own.iter()
                .map(|(label, aligned, comparable)| format!(
                    "{} {aligned} of {comparable}",
                    escape(label)
                ))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    out.push_str("<h3 id=\"dialects-what\">What each setting decides</h3>\n<dl class=\"defs\">\n");
    for configuration in &CONFIGURATIONS {
        let _ = writeln!(
            out,
            "<dt><code>{}</code></dt><dd>{}</dd>",
            escape(configuration.flag),
            inline(configuration.what)
        );
    }
    out.push_str("</dl>\n");
    out.push_str(
        "<p class=\"note\">A dialect is a writer-side and a reader-side setting at once: \
         <code>--dialect rti</code> means \"write what RTI writes\" exactly as it means \"read \
         what RTI writes\". Setting one to reach a peer therefore changes what every other peer \
         sees, which is why the table has twelve columns and not one. The row marked \
         <em>how it should be</em> is not a peer: it is the clause-by-clause reading of XTypes \
         1.3, and its column tells you how far each implementation is from the text. It is \
         also the <em>shipped default</em> — what this project writes when nobody names a \
         dialect is what the text says, not what any product does — so the same row carries \
         both tags.</p>\n",
    );
    out.push_str("</section>\n");
    out
}

/// The setting that reaches furthest towards each system.
///
/// A tie goes to the earlier setting, which is the shipped default
/// first and the dialects before the two single-question overrides.
/// Recommending an override that reaches exactly as far as the default
/// would be recommending a change for nothing. `CONFIGURATIONS` is
/// ordered for this: the default leads it, and
/// `dialect_completeness.rs::the_default_configuration_is_the_default_dialect`
/// holds the name to `Dialect::default`.
fn best_settings(alignment: &[Vec<Alignment>]) -> Vec<usize> {
    (0..SYSTEMS.len())
        .map(|column| {
            let mut best = 0;
            let mut reach = 0;
            for line in 0..CONFIGURATIONS.len() {
                let aligned = alignment
                    .get(line)
                    .and_then(|line| line.get(column))
                    .map_or(0, |cell| cell.aligned);
                if aligned > reach {
                    reach = aligned;
                    best = line;
                }
            }
            best
        })
        .collect()
}

fn reading(base: &str) -> String {
    let mut out = String::from("<section id=\"reading\">\n<h2>How to read a cell</h2>\n");
    out.push_str(
        "<h3 id=\"reading-mode\">The mode: <code>plain-cdr1/le</code> and its nine \
         siblings</h3>\n\
         <p>Every construct is measured ten times, and the mode says which of the ten a \
          row is. \
         It is two things joined by a slash. Before the slash is the <strong>encoding</strong>: \
         how the type's members are framed on the wire — whether there is a length in front of \
         the aggregate, whether each member carries its own id. After it is the \
         <strong>byte order</strong> the numbers inside are written in: <code>le</code> is \
         little-endian, what an x86 or an ARM writes natively, and <code>be</code> is \
         big-endian, which the CDR standard calls the default and which a PowerPC or a SPARC \
         writes. Neither is more correct; a stream declares which one it used, and a reader \
         that ignores the declaration reads every number backwards.</p>\n",
    );
    out.push_str(
        "<div class=\"scroller\"><table class=\"grid\">\n<thead><tr><th>encoding</th>\
         <th>version</th><th>what it frames</th></tr></thead>\n<tbody>\n",
    );
    for (encoding, version, what) in [
        (
            "plain-cdr1",
            "XCDR1",
            "The classic CDR of CORBA and of DDS before XTypes: the members in declaration \
             order, each aligned to its own size, and nothing else. No length prefix, no member \
             ids. A reader must already know the type exactly.",
        ),
        (
            "pl-cdr1",
            "XCDR1",
            "Parameter-list CDR, version 1: each member preceded by its id and its length, so a \
             reader can step over a member it does not know. This is what a <code>@mutable</code> \
             type takes under encoding version 1.",
        ),
        (
            "plain-cdr2",
            "XCDR2",
            "XTypes' version 2 of the plain form. The same idea as <code>plain-cdr1</code> with \
             different alignment rules — an eight-octet member aligns to four, not to eight — \
             which is why the two are not the same octets for the same type.",
        ),
        (
            "delimited-cdr",
            "XCDR2",
            "Version 2 with a delimiter header: a four-octet length in front of the aggregate, \
             so a reader that meets members it does not know can jump the rest. This is what an \
             <code>@appendable</code> type takes, and it is at the centre of most cross-vendor \
             surprises, because whether a type is appendable is exactly what implementations \
             disagree about.",
        ),
        (
            "pl-cdr2",
            "XCDR2",
            "Version 2's parameter list: a delimiter, then every member behind an EMHEADER \
             carrying its id, its length code and its must-understand bit. The framing of a \
             <code>@mutable</code> type under version 2.",
        ),
    ] {
        let _ = writeln!(
            out,
            "<tr><th scope=\"row\"><code>{encoding}</code></th><td>{version}</td>\
             <td>{what}</td></tr>"
        );
    }
    out.push_str("</tbody></table></div>\n");
    out.push_str(
        "<p class=\"note\">A type does not pick its framing directly. Its \
         <em>extensibility</em> does — <code>@final</code>, <code>@appendable</code> or \
         <code>@mutable</code> — and the encoding version picks between the two versions of \
         that framing. That is why a foreign column answers five encodings with two runs: its \
         command line takes the version, and the framing follows from the type. Those rows read \
         <code>= plain-cdr1</code> or <code>= plain-cdr2</code> and link to the run they \
         are.</p>\n",
    );

    out.push_str("<h3 id=\"reading-words\">The words a cell can carry</h3>\n<dl class=\"defs\">\n");
    for (word, meaning) in WORDS {
        let _ = writeln!(out, "<dt>{}</dt><dd>{meaning}</dd>", inline(word));
    }
    out.push_str("</dl>\n");

    out.push_str("<h3 id=\"reading-links\">Following a cell to the real output</h3>\n");
    let _ = writeln!(
        out,
        "<p>A cell that carries octets links to the file holding them, byte for byte, as \
         <code>&lt;construct&gt;/&lt;system&gt;/&lt;encoding&gt;.&lt;order&gt;.bin</code>; \
         every other cell links to the <code>STATUS</code> file carrying that system's own \
         sentence about the slot; a warning marker links to what its compiler printed. The \
         links are relative to the directory this page sits in, which is the result tree \
         itself, so they resolve from a checkout on disk and from anywhere the directory is \
         served whole.{}</p>",
        if base.is_empty() {
            format!(
                " As this page was built the tree is <strong>not</strong> part of the public \
                 mirror at <code>{MIRROR}</code>, so no absolute link is emitted: a link into a \
                 tree that is not published is a link to a 404, and a reader who follows one has \
                 been told something false about the evidence. Regenerating with \
                 <code>--base-url</code> makes every link on this page absolute the day the tree \
                 is published."
            )
        } else {
            format!(
                " This build was given a base, so every link is absolute: \
                 <code>{}</code>.",
                escape(base)
            )
        }
    );
    out.push_str("</section>\n");
    out
}

fn system_sections(
    profiles: &[Profile],
    alignment: &[Vec<Alignment>],
    rows: &[Row],
    cases: &[Case],
    base: &str,
) -> String {
    let mut out = String::from("<section id=\"systems\">\n<h2>The twelve, one at a time</h2>\n");
    out.push_str(
        "<p>Each section stands on its own: what the system is, what it takes, what it turns \
         away and in whose words, where its octets and this project's part company, which \
         setting closes the gap, and what is left over when nothing does.</p>\n",
    );
    let total = cases.len();
    let best = best_settings(alignment);
    let default_line = default_setting();
    // Which foreign peer is furthest from this project under the best
    // setting anybody could choose for it. Derived, so that the peer
    // this sentence is about changes when the measurement does.
    let widest = SYSTEMS
        .iter()
        .enumerate()
        .filter(|(_, system)| system.kind != Kind::Own)
        .max_by_key(|(column, _)| {
            let line = best.get(*column).copied().unwrap_or(0);
            profiles.get(*column).map_or(0, |profile| {
                profile.comparable.saturating_sub(
                    alignment
                        .get(line)
                        .and_then(|line| line.get(*column))
                        .map_or(0, |cell| cell.aligned),
                )
            })
        })
        .map(|(column, _)| column);

    for (column, system) in SYSTEMS.iter().enumerate() {
        let Some(profile) = profiles.get(column) else {
            continue;
        };
        let _ = writeln!(
            out,
            "<article class=\"system\" id=\"{}\">\n<h3>{} <span class=\"version\">{}</span>\
             <a class=\"anchor\" href=\"#{}\" aria-label=\"link to this section\">#</a></h3>",
            system.id,
            escape(system.label),
            escape(system.version),
            system.id
        );
        let _ = writeln!(out, "<p>{}</p>", inline(system.note));

        let line = best.get(column).copied().unwrap_or(0);
        let aligned = alignment
            .get(line)
            .and_then(|line| line.get(column))
            .map_or(0, |cell| cell.aligned);
        let default_aligned = alignment
            .get(default_line)
            .and_then(|line| line.get(column))
            .map_or(0, |cell| cell.aligned);
        let refused: BTreeSet<&String> = profile.refusals.values().flatten().collect();

        out.push_str("<div class=\"figures\">\n");
        for (figure, label) in [
            (
                format!(
                    "{} <span class=\"of\">of {total}</span>",
                    profile.cases.len()
                ),
                "constructs taken",
            ),
            (profile.comparable.to_string(), "rows with octets"),
            (
                profile.comparable.saturating_sub(aligned).to_string(),
                "unmatched under its best setting",
            ),
            (profile.diverging.to_string(), "unmatched under our default"),
            (refused.len().to_string(), "constructs refused"),
            (profile.read_different.to_string(), "rows it misread"),
        ] {
            let _ = writeln!(
                out,
                "<div class=\"figure\"><span class=\"value\">{figure}</span>\
                 <span class=\"label\">{label}</span></div>"
            );
        }
        out.push_str("</div>\n");

        if profile.comparable == 0 {
            out.push_str(
                "<p>No row of this column carries octets, so there is nothing to align to.</p>\n",
            );
        } else if let Some(configuration) = CONFIGURATIONS.get(line) {
            let remainder = profile.comparable.saturating_sub(aligned);
            let _ = writeln!(
                out,
                "<p class=\"verdict {}\"><strong>The flag: {}.</strong> Of the {} rows this \
                 system wrote, <strong>{remainder}</strong> come out different under it{}</p>",
                if remainder == 0 { "good" } else { "warn" },
                flag_phrase(configuration.flag),
                profile.comparable,
                if remainder == 0 {
                    String::from(" — none. It is byte for byte this system, everywhere it wrote.")
                } else if configuration.name == DEFAULT {
                    String::from(
                        ", and the shipped default is that same setting, so this is what a \
                         reader gets without setting anything. They are broken down below.",
                    )
                } else {
                    format!(
                        ", against {} under the shipped default <code>--dialect {DEFAULT}</code>. \
                         They are broken down below.",
                        profile.comparable.saturating_sub(default_aligned)
                    )
                }
            );
            if widest == Some(column) && remainder > 0 {
                // The peer that wrote the most rows, so that "largest
                // remainder" can be shown not to be "asked the most".
                let busiest = SYSTEMS
                    .iter()
                    .enumerate()
                    .filter(|(_, other)| other.kind != Kind::Own)
                    .max_by_key(|(column, _)| {
                        profiles
                            .get(*column)
                            .map_or(0, |profile| profile.comparable)
                    });
                let _ = writeln!(
                    out,
                    "<p class=\"verdict warn\"><strong>This is the largest remainder of any \
                     peer.</strong> {remainder} rows of the {} this system wrote are reproduced \
                     by no setting of this project's — more than any other implementation \
                     measured here leaves over{}. It is the one column where the answer to \
                     \"which flag\" is least of an answer, and the breakdown below is what would \
                     have to be settled instead.</p>",
                    profile.comparable,
                    match busiest {
                        Some((other, system)) if other != column => format!(
                            ", and not because it was asked the most: {} wrote {} rows to this \
                             system's {}",
                            escape(system.label),
                            profiles.get(other).map_or(0, |profile| profile.comparable),
                            profile.comparable
                        ),
                        _ => String::new(),
                    }
                );
            }
            out.push_str(&remainder_table(
                rows,
                cases,
                column,
                line,
                profile.comparable,
            ));
        }

        if profile.refusals.is_empty() {
            out.push_str("<p>It refused nothing in this corpus.</p>\n");
        } else {
            let _ = write!(
                out,
                "<h4>What it refuses — {} construct{}, in its own words</h4>\n\
                 <div class=\"scroller\"><table class=\"grid\">\n<thead><tr>\
                 <th class=\"n\">constructs</th><th>the sentence it gave</th><th>which</th>\
                 </tr></thead>\n<tbody>\n",
                refused.len(),
                if refused.len() == 1 { "" } else { "s" }
            );
            let mut ordered: Vec<(&String, &BTreeSet<String>)> = profile.refusals.iter().collect();
            ordered.sort_by_key(|(reason, cases)| (usize::MAX - cases.len(), reason.as_str()));
            for (reason, drew_it) in ordered {
                let _ = writeln!(
                    out,
                    "<tr><td class=\"n\">{}</td><td class=\"reason\">{}</td><td>{}</td></tr>",
                    drew_it.len(),
                    escape(reason),
                    case_links(drew_it.iter().map(String::as_str), base)
                );
            }
            out.push_str("</tbody></table></div>\n");
        }

        if !profile.structural.is_empty() {
            let mut parts: Vec<String> = profile
                .structural
                .iter()
                .map(|(word, count)| {
                    // `=` is the word a cell prints; on its own in a
                    // tally it says nothing, so the tally says what it
                    // means instead.
                    if *word == "=" {
                        format!("{count} × a row that is another encoding's run")
                    } else {
                        format!("{count} × <code>{}</code>", escape(word))
                    }
                })
                .collect();
            parts.sort();
            let _ = writeln!(
                out,
                "<p class=\"note\"><strong>Rows that were never a question:</strong> {}. These \
                 are properties of the implementation rather than results of a run, and they \
                 are counted apart from refusals everywhere on this page.</p>",
                parts.join(", ")
            );
        }

        if system.kind != Kind::Own && profile.comparable > 0 {
            let left: usize = profile.unalignable.values().map(|(count, _)| count).sum();
            if left == 0 {
                out.push_str(
                    "<p class=\"verdict good\">Every byte string this system produced is \
                     reproducible by one of this project's settings. Nothing it writes is \
                     out of reach; the only question is which flag.</p>\n",
                );
            } else {
                let _ = writeln!(
                    out,
                    "<p class=\"verdict bad\"><strong>{left} rows in {} constructs are out of \
                     reach of every one of the twelve settings.</strong> These are the rows that \
                     cannot be answered by telling somebody what to set, and they are the ones \
                     worth reading first: either this system writes something no reading of the \
                     specification produces, or this project has no form for something it does. \
                     <button type=\"button\" class=\"filter\" data-filter=\"{}:unalignable\">\
                     Open them</button></p>",
                    profile.unalignable.len(),
                    escape(system.id)
                );
            }
        }

        out.push_str("</article>\n");
    }
    out.push_str("</section>\n");
    out
}

/// The remainder, broken down by what is different about the octets.
///
/// The table the whole page turns on. A number of rows left over is a
/// quiz; a number of rows left over with the shape of the difference,
/// the constructs it happens in and whether some other flag produces
/// those octets is a list of work.
fn remainder_table(
    rows: &[Row],
    cases: &[Case],
    column: usize,
    line: usize,
    comparable: usize,
) -> String {
    let groups = remainders(rows, cases, column, line);
    if groups.is_empty() {
        return String::new();
    }
    let Some(system) = SYSTEMS.get(column) else {
        return String::new();
    };
    let Some(configuration) = CONFIGURATIONS.get(line) else {
        return String::new();
    };
    let left: usize = groups.iter().map(|group| group.rows).sum();
    let unreached: usize = groups.iter().map(|group| group.unreached).sum();
    let mut out = format!(
        "<h4>The {left} rows <code>{}</code> does not reproduce</h4>\n",
        escape(configuration.flag)
    );
    let _ = writeln!(
        out,
        "<p>{left} of the {comparable} rows this system wrote, grouped by what is different \
         about the octets. The shape is measured — a length difference is a length difference \
         and an offset is an offset. <em>Why</em> two payloads part is a clause question, and \
         the clause is cited per behaviour in \
         <code>docs/wire-feature-inventory.md</code>. The last column is the actionable one: \
         whether some other setting of this project does produce those octets. {}</p>",
        if unreached == left {
            "For every group here the answer is no, which makes the whole remainder a list of \
             wire behaviour this project has no form for rather than a flag anybody forgot to \
             set."
        } else if unreached == 0 {
            "For every group here the answer is yes, which makes the whole remainder a question \
             of per-construct settings rather than a gap."
        } else {
            "Where it is no, the rows are a gap rather than a flag anybody forgot to set."
        }
    );
    out.push_str(
        "<div class=\"scroller\"><table class=\"grid\">\n<thead><tr><th>what differs</th>\
         <th class=\"n\">rows</th><th>in which constructs</th><th>first differing octet</th>\
         <th>what would reproduce it</th></tr></thead>\n<tbody>\n",
    );
    for group in &groups {
        let constructs = group
            .cases
            .iter()
            .take(NAMED)
            .map(|(case, row)| format!("<a href=\"#row-{row}\"><code>{}</code></a>", escape(case)))
            .collect::<Vec<_>>()
            .join(", ");
        let octets = if group.octets.is_empty() {
            String::from("—")
        } else {
            group
                .octets
                .iter()
                .take(8)
                .map(usize::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        };
        let reach = if group.unreached == group.rows {
            format!(
                "<strong class=\"bad\">no setting of the {}</strong>",
                CONFIGURATIONS.len()
            )
        } else {
            let mut parts: Vec<String> = group
                .reproduced_by
                .iter()
                .map(|(name, count)| {
                    format!(
                        "<a href=\"#setting-{name}\"><code>{}</code></a> {count}",
                        escape(name)
                    )
                })
                .collect();
            if group.unreached > 0 {
                parts.push(format!(
                    "<strong class=\"bad\">{} reached by nothing</strong>",
                    group.unreached
                ));
            }
            parts.join("; ")
        };
        let _ = writeln!(
            out,
            "<tr><th scope=\"row\">{}</th><td class=\"n\">{} <span class=\"of\">/ {} \
             constructs</span></td><td>{constructs}{}</td><td class=\"n\">{octets}{}</td>\
             <td>{reach}</td></tr>",
            match &group.not_asked {
                Some(reason) => format!(
                    "{}<br><span class=\"of\">not asked: {} — {}<br>measured: {}</span>",
                    escape(&group.shape),
                    escape(reason.group),
                    escape(reason.why),
                    escape(reason.measured)
                ),
                None => escape(&group.shape),
            },
            group.rows,
            group.cases.len(),
            if group.cases.len() > NAMED {
                format!(
                    " <span class=\"of\">and {} more</span>",
                    group.cases.len() - NAMED
                )
            } else {
                String::new()
            },
            if group.octets.len() > 8 {
                format!(" <span class=\"of\">+{}</span>", group.octets.len() - 8)
            } else {
                String::new()
            }
        );
    }
    out.push_str("</tbody></table></div>\n");
    let _ = writeln!(
        out,
        "<p><button type=\"button\" class=\"filter\" data-filter=\"{}:unmatched-{}\">Open all \
         {left} in the appendix</button> Each row there carries both byte strings in full.</p>",
        escape(system.id),
        escape(configuration.name)
    );
    out
}

fn tracker(issues: &BTreeMap<String, Vec<u32>>, rows: &[Row], cases: &[Case]) -> String {
    let mut by_issue: BTreeMap<u32, BTreeSet<&str>> = BTreeMap::new();
    for (case, numbers) in issues {
        for number in numbers {
            by_issue.entry(*number).or_default().insert(case.as_str());
        }
    }
    let mut out =
        String::from("<section id=\"issues\">\n<h2>Reported issues these rows answer</h2>\n");
    if by_issue.is_empty() {
        out.push_str("<p>No corpus construct names a reported issue.</p>\n</section>\n");
        return out;
    }
    let _ = writeln!(
        out,
        "<p>A construct built from a bug report names the report in its own README, and every \
         construct whose compiler draws the same diagnostic has the same cause — so a reporter \
         who lands on any of them gets the same answer. The numbers are the tracker's at \
         <code>{MIRROR}</code>.</p>"
    );
    for (number, named) in &by_issue {
        let first = rows
            .iter()
            .find(|row| {
                cases
                    .get(row.case)
                    .is_some_and(|case| named.contains(case.name.as_str()))
            })
            .map(|row| row.id);
        let built_from: Vec<&str> = named
            .iter()
            .copied()
            .filter(|case| case.contains("issue"))
            .collect();
        let _ = writeln!(
            out,
            "<article class=\"issue\" id=\"issue-{number}\"><h3>#{number} \
             <a class=\"anchor\" href=\"#issue-{number}\" aria-label=\"link to this issue\">#</a>\
             </h3>\n<p>{} construct{} in this corpus {} this cause{}. {}\
             <button type=\"button\" class=\"filter\" data-filter=\"#{number}\">Show every row \
             that answers it</button></p></article>",
            named.len(),
            if named.len() == 1 { "" } else { "s" },
            if named.len() == 1 { "carries" } else { "carry" },
            first.map_or(String::new(), |id| format!(
                ", starting at <a href=\"#row-{id}\">row {id}</a>"
            )),
            if built_from.is_empty() {
                String::new()
            } else {
                format!(
                    "Built from the report itself: {}. ",
                    built_from
                        .iter()
                        .map(|case| format!("<code>{}</code>", escape(case)))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        );
    }
    out.push_str(
        "<h3 id=\"issues-cause\">The cause behind the large family, and the two answers to \
         it</h3>\n\
         <p>XTypes 1.3 §7.3.1.2.1.8 lets an implementation choose what an aggregate whose \
         source carried no extensibility annotation is taken to be, and nothing on the wire \
         says which choice was made. Cyclone chooses <code>@final</code> and says so in a \
         compiler warning it prints on every such type. RTI and Fast DDS choose \
         <code>@appendable</code>, which under encoding version 2 puts a four-octet delimiter \
         header in front of the aggregate that the other side is not expecting — so the reader \
         takes the first four octets of the payload for a length and everything after it is \
         wrong. Under encoding version 1 the two readings are byte for byte identical, which is \
         why the same pair of programs interoperates until somebody turns XCDR2 on, and why the \
         symptom appears without anything in the type having changed.</p>\n\
         <p>There are two answers and they are not equivalent. <strong>Annotate the type.</strong> \
         An explicit <code>@final</code> or <code>@appendable</code> in the IDL settles the \
         question for every implementation at once, and it is the only answer that survives a \
         third implementation joining the system. <strong>Or set the dialect</strong> of the \
         peer you are talking to, which settles it for that peer and moves it for the \
         rest — the <a href=\"#dialects\">matrix</a> is the whole of what that costs.</p>\n\
         <p class=\"note\">A report can share this symptom without sharing a row. #27 reaches \
         the same framing question through a different reported failure, and the rows here \
         answer only the framing half of it; the half about whether two participants find each \
         other at all is not a question about octets and is not measured on this page. A report \
         is listed above only when a construct in the corpus was built from it or draws the \
         same diagnostic.</p>\n",
    );
    out.push_str("</section>\n");
    out
}

fn appendix(cases: usize, rows: usize) -> String {
    let mut out = String::from("<section id=\"rows\">\n<h2>Appendix: every row</h2>\n");
    let _ = writeln!(
        out,
        "<p>{rows} rows: {cases} constructs × five encodings × two byte orders. Search by \
         anything a row carries — a construct name, a mode, an issue number, a sentence a \
         compiler printed — or by the symptom, which is what a bug report gives you. Every term \
         you type has to appear, and every row has its own address, so a row is something you \
         can paste into a reply.</p>"
    );
    let _ = writeln!(
        out,
        "<p class=\"note\"><strong>Asking about one system.</strong> A system and a symptom \
         join with a colon: <code>cyclone:differs</code> is every row where Cyclone's octets \
         are not ours, <code>rti:refused</code> every row RTI would not take, \
         <code>fastdds:misread</code> every row Fast DDS read back wrongly, and \
         <code>opendds:warned</code> every row its compiler said something about. The names \
         are {}. On their own, <code>differs</code>, <code>refused</code>, \
         <code>misread</code>, <code>warned</code> and <code>unalignable</code> ask the same \
         questions of every system at once.</p>",
        SYSTEMS
            .iter()
            .map(|system| format!("<code>{}</code>", escape(system.id)))
            .collect::<Vec<_>>()
            .join(", ")
    );
    out.push_str(
        "<div class=\"controls\">\n<label class=\"search\"><span>Search</span>\
         <input type=\"search\" id=\"q\" placeholder=\"cyclone:differs · rti:refused · #41 · \
         plain-cdr2 · annotation-optional-absent\" autocomplete=\"off\" spellcheck=\"false\">\
         </label>\n<div class=\"chips\" id=\"chips\"></div>\n\
         <p class=\"count\" id=\"count\" role=\"status\"></p>\n</div>\n",
    );
    out.push_str(
        "<div class=\"scroller\"><table class=\"grid rows\" id=\"table\">\n<thead><tr>\
         <th class=\"n\">№</th><th>construct</th><th>mode</th>",
    );
    for system in &SYSTEMS {
        let _ = write!(
            out,
            "<th class=\"n\" title=\"{}\"><a href=\"#{}\">{}</a></th>",
            escape(system.version),
            system.id,
            escape(system.label)
        );
    }
    out.push_str("<th>issue</th></tr></thead>\n<tbody id=\"body\"></tbody>\n</table></div>\n");
    out.push_str("<div class=\"pager\" id=\"pager\"></div>\n");
    out.push_str(
        "<noscript><p class=\"verdict warn\">This appendix is paginated by script. Without it, \
         the same rows are in <code>write.md</code> and <code>read.md</code> beside this file, \
         and the machine-readable record both are rendered from is \
         <code>ledger.tsv</code>.</p></noscript>\n",
    );
    out.push_str("</section>\n");
    out
}

fn footer() -> String {
    format!(
        "<footer>\n<p>Generated by <code>tools/interop-results --page</code> from \
         <code>tests/interop-results/ledger.tsv</code>, the corpus in \
         <code>tests/interop-corpus/</code> and <code>docs/idl-building-blocks.md</code>. \
         Nothing here is written by hand: every figure moves when the corpus does, and the \
         build fails when the committed page is not what a fresh run produces. Public mirror: \
         <code>{MIRROR}</code>.</p>\n</footer>\n"
    )
}

/// The glossary, in the order a reader meets the words.
const WORDS: [(&str, &str); 12] = [
    (
        "12B, 48B, a number and a B",
        "The system ran, took the type, took the value, and produced that many octets. The cell \
         links to the file holding exactly those octets, and opening a row shows them in full.",
    ),
    (
        "same",
        "In the read direction: it read its own bytes back and got the value the case says it \
         should. This is the only outcome that demonstrates a round trip rather than an \
         encoding.",
    ),
    (
        "different",
        "It read its own bytes back and got something else. Wherever this appears it is a \
         defect, in whichever implementation it appears in, because nothing but that \
         implementation wrote those bytes.",
    ),
    (
        "refused",
        "It ran and would not take the type or the value, and the cell links to the sentence it \
         gave. A refusal is a result: it is the compiler or the serialiser naming a construct \
         it does not have.",
    ),
    (
        "unwritten",
        "There was nothing to read, because the write direction produced nothing. It says \
         nothing whatever about the reader, and it is printed rather than left blank so that it \
         cannot be mistaken for agreement.",
    ),
    (
        "= plain-cdr1",
        "This row's run is that row's run. A foreign command line selects the encoding version \
         and not the framing, so four of the five encodings point at the two runs the system \
         actually made rather than at five identical files.",
    ),
    (
        "no xcdr2",
        "The system has no encoding version 2 at all. CORBA CDR predates XTypes and Fast CDR \
         1.0.24 has none, so three of the five encodings were never a question that could be \
         put to it.",
    ),
    (
        "one order",
        "The system writes one byte order and this row is the other one. TAO writes little \
         endian only because <code>ACE_OutputCDR</code> swaps only in a build configuration \
         this one did not use; JacORB writes big endian only because its output stream has no \
         method to set the other.",
    ),
    (
        "no form",
        "The construct has no form in this system's type model, so its compiler never saw the \
         case. Nothing was asked, so nothing was refused, and the two are counted apart \
         everywhere on this page.",
    ),
    (
        "no probe",
        "The toolchain exists and no probe in this tree drives it for this slot.",
    ),
    (
        "not measured",
        "The construct is expressible, a harness in this tree produces it, and no run is \
         recorded. The cell names the command that would produce one.",
    ),
    (
        "w1, w2, …",
        "Its compiler said something about the type while accepting it — that many distinct \
         warnings, linked. A marker of its own rather than a note beside a refusal, because the \
         row it matters most in is the one where nothing went wrong: a system whose bytes agree \
         while its compiler warned about the type is not the same as one that agreed silently.",
    ),
];

// -------------------------------------------------------------- payload

/// The row data, as one JSON island the script reads.
///
/// Dictionary-encoded, because the same sentence is the answer for ten
/// rows of the same construct and the same byte string is the answer
/// for several systems at once: interned, the whole record is a small
/// fraction of the ledger it comes from.
/// What a construct is for, in the case's own words and in full.
///
/// `sources::describe`'s version is bounded at 200 characters because
/// it has to fit a Markdown table cell. Nothing here has to fit
/// anywhere, and a description that stops mid-clause is the one thing a
/// reader cannot work around: the sentence that would have said which
/// of two readings the case is about is the sentence that was cut. So
/// the whole paragraph is carried and the bounded form is not used on
/// this page at all.
fn describe_fully(readme: &str, fallback: &str) -> String {
    let mut construct = String::new();
    let mut paragraph: Vec<&str> = Vec::new();
    let mut after = false;
    for line in readme.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("**Construct** — ") {
            construct = rest.trim().to_string();
            after = true;
            continue;
        }
        if !after {
            continue;
        }
        if line.is_empty() {
            if paragraph.is_empty() {
                continue;
            }
            break;
        }
        if line.starts_with('#') {
            break;
        }
        paragraph.push(line);
    }
    let paragraph = paragraph.join(" ");
    match (construct.is_empty(), paragraph.is_empty()) {
        (true, true) => fallback.replace("\\|", "|"),
        (true, false) => paragraph,
        (false, true) => construct,
        (false, false) => format!("{construct} — {paragraph}"),
    }
}

fn payload(
    rows: &[Row],
    cases: &[Case],
    issues: &BTreeMap<String, Vec<u32>>,
    documents: &Documents,
    base: &str,
) -> String {
    let mut arena: Vec<String> = Vec::new();
    let mut lookup: BTreeMap<String, usize> = BTreeMap::new();
    let mut intern = |text: &str| -> usize {
        if let Some(at) = lookup.get(text) {
            return *at;
        }
        arena.push(text.to_string());
        let at = arena.len() - 1;
        lookup.insert(text.to_string(), at);
        at
    };

    // Every system contributes this many values to a row, and the
    // reader in `page.js` walks the flat array by that stride. The
    // fallback below used to push six where the arm beneath it pushes
    // seven, so a row missing one system would have shifted every
    // system after it — a page whose cells belong to the wrong column,
    // with nothing to show that they do. Measured on the committed
    // ledger the arm is unreachable: 3,410 rows, twelve systems on each
    // of them and no other count. It is kept, because "unreachable
    // today" is not a property the ledger writer promises, and the
    // assertion below turns a stride that ever disagrees into a failed
    // generation rather than a quietly wrong page.
    const CELLS_PER_SYSTEM: usize = 7;
    let mut row_values: Vec<serde_json::Value> = Vec::with_capacity(rows.len());
    for row in rows {
        let mut cells: Vec<serde_json::Value> =
            Vec::with_capacity(SYSTEMS.len() * CELLS_PER_SYSTEM);
        for system in &SYSTEMS {
            let Some(entry) = row.entries.iter().find(|entry| entry.system == system.id) else {
                for _ in 0..CELLS_PER_SYSTEM {
                    cells.push(serde_json::Value::from(0));
                }
                continue;
            };
            let agreement = match (&entry.write, row.ours()) {
                (Outcome::Bytes(theirs), Some(mine)) if theirs == mine => 1,
                (Outcome::Bytes(_), Some(_)) => 2,
                _ => 0,
            };
            for value in [
                intern(entry.write.word()),
                intern(&entry.write.detail()),
                intern(entry.read.word()),
                intern(&entry.read.detail()),
                entry.warnings,
                agreement,
                // One bit per setting: which of the twelve produce exactly
                // what this system produced here. The appendix's
                // `<system>:unmatched-<setting>` filter is this mask
                // read back, so a remainder figure and the rows behind
                // it are one measurement rather than two.
                row.reproduce(system.id) as usize,
            ] {
                cells.push(serde_json::Value::from(value));
            }
        }
        assert_eq!(
            cells.len(),
            SYSTEMS.len() * CELLS_PER_SYSTEM,
            "row {} put {} values in a row `page.js` reads at a stride of {CELLS_PER_SYSTEM}; \
             every system after the short one would be read out of the wrong cells",
            row.id,
            cells.len()
        );
        row_values.push(serde_json::json!([
            row.id,
            row.case,
            mode_index(row.encoding, row.endian),
            intern(&row.diff),
            intern(&row.mitigation),
            intern(row.ours().unwrap_or("")),
            intern(row.specification().unwrap_or("")),
            serde_json::Value::Array(cells),
        ]));
    }

    let systems: Vec<serde_json::Value> = SYSTEMS
        .iter()
        .map(|system| {
            serde_json::json!({
                "id": system.id,
                "label": system.label,
                "version": system.version,
            })
        })
        .collect();
    let mut modes: Vec<serde_json::Value> = Vec::new();
    for encoding in ENCODINGS {
        for order in ORDERS {
            modes.push(serde_json::Value::String(format!("{encoding}/{order}")));
        }
    }
    let case_values: Vec<serde_json::Value> = cases
        .iter()
        .map(|case| {
            let readme = documents.readmes.get(&case.name).map_or("", String::as_str);
            serde_json::json!({
                "n": case.name,
                "d": describe_fully(readme, &case.description),
                "i": issues.get(&case.name).cloned().unwrap_or_default(),
            })
        })
        .collect();

    let document = serde_json::json!({
        "systems": systems,
        "modes": modes,
        "cases": case_values,
        "rows": row_values,
        "strings": arena,
        "settings": CONFIGURATIONS
            .iter()
            .map(|configuration| configuration.name)
            .collect::<Vec<_>>(),
        "base": base,
        "default": DEFAULT,
        "specification": SPECIFICATION,
    });
    format!(
        "<script id=\"matrix\" type=\"application/json\">{}</script>\n",
        document.to_string().replace('<', "\\u003c")
    )
}

fn mode_index(encoding: &str, endian: &str) -> usize {
    let encoding = ENCODINGS
        .iter()
        .position(|name| *name == encoding)
        .unwrap_or(0);
    let order = ORDERS.iter().position(|name| *name == endian).unwrap_or(0);
    encoding * ORDERS.len() + order
}

// -------------------------------------------------------------- helpers

/// Find the pipe table whose header row carries `marker`.
///
/// # Errors
///
/// When no such table is there. A silently absent table would render an
/// empty section, and an empty section reads as "nothing to report".
fn markdown_table(text: &str, marker: &str) -> Result<MarkdownTable, String> {
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        let line = line.trim();
        if !line.starts_with('|') || !line.contains(marker) {
            continue;
        }
        let header = split_cells(line);
        match lines.peek() {
            Some(next) if next.trim_start().starts_with('|') && next.contains("---") => {
                let _ = lines.next();
            }
            _ => continue,
        }
        let mut rows = Vec::new();
        for row in lines.by_ref() {
            let row = row.trim();
            if !row.starts_with('|') {
                break;
            }
            rows.push(split_cells(row));
        }
        return Ok(MarkdownTable { header, rows });
    }
    Err(format!(
        "docs/idl-building-blocks.md has no table whose header carries `{marker}`"
    ))
}

fn split_cells(line: &str) -> Vec<String> {
    line.trim()
        .trim_start_matches('|')
        .trim_end_matches('|')
        .split('|')
        .map(|cell| cell.trim().to_string())
        .collect()
}

/// One parsed Markdown table as HTML. `classed` names the cell values
/// that get a colour of their own.
fn render_table(table: &MarkdownTable, classed: &[&str]) -> String {
    let mut out = String::from("<div class=\"scroller\"><table class=\"grid\">\n<thead><tr>");
    for cell in &table.header {
        let _ = write!(out, "<th>{}</th>", inline(cell));
    }
    out.push_str("</tr></thead>\n<tbody>\n");
    for row in &table.rows {
        out.push_str("<tr>");
        for (at, cell) in row.iter().enumerate() {
            let class = if at >= 2 && classed.contains(&cell.trim()) {
                match cell.trim() {
                    "yes" => " class=\"good\"",
                    _ => " class=\"bad\"",
                }
            } else {
                ""
            };
            let _ = write!(out, "<td{class}>{}</td>", inline(cell));
        }
        out.push_str("</tr>\n");
    }
    out.push_str("</tbody></table></div>\n");
    out
}

fn case_links<'a>(cases: impl Iterator<Item = &'a str>, base: &str) -> String {
    let names: Vec<&str> = cases.collect();
    let mut out = names
        .iter()
        .take(12)
        .map(|name| {
            format!(
                "<a href=\"{base}../interop-corpus/{name}/type.idl\"><code>{}</code></a>",
                escape(name)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    if names.len() > 12 {
        let _ = write!(
            out,
            " <span class=\"of\">and {} more</span>",
            names.len() - 12
        );
    }
    out
}

/// How a setting is asked for, in a sentence.
///
/// Two of the twelve are one question changed on top of the shipped
/// default rather than a dialect of their own, and a page that told a
/// reader to set `--length-codes universal-nextint` without saying what
/// it sits on top of would have told them half of it.
fn flag_phrase(flag: &str) -> String {
    if flag.starts_with("--dialect ") {
        format!("<code>{}</code>", escape(flag))
    } else {
        format!("the shipped default with <code>{}</code>", escape(flag))
    }
}

fn share(aligned: usize, comparable: usize) -> f64 {
    if comparable == 0 {
        return 0.0;
    }
    #[allow(clippy::cast_precision_loss)]
    {
        aligned as f64 / comparable as f64
    }
}

/// HTML-escape.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(character),
        }
    }
    out
}

/// Escape, then take Markdown's three inline forms: backticks become
/// `<code>`, `**` becomes `<strong>` and a single `*` becomes `<em>`.
/// The documents this page quotes are Markdown, and printing their
/// source verbatim would show the punctuation instead of the emphasis.
///
/// Inside a code span nothing is emphasis: `*` is a multiplication and
/// an asterisk far more often than it is a mark-up character there, and
/// `sequence<T, *>` rendered as the start of an emphasis run would eat
/// the rest of the sentence.
fn inline(text: &str) -> String {
    let escaped = escape(text);
    let mut out = String::with_capacity(escaped.len());
    let mut code = false;
    let mut strong = false;
    let mut emphasis = false;
    let mut characters = escaped.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '`' => {
                out.push_str(if code { "</code>" } else { "<code>" });
                code = !code;
            }
            '*' if !code && characters.peek() == Some(&'*') => {
                let _ = characters.next();
                out.push_str(if strong { "</strong>" } else { "<strong>" });
                strong = !strong;
            }
            '*' if !code => {
                out.push_str(if emphasis { "</em>" } else { "<em>" });
                emphasis = !emphasis;
            }
            other => out.push(other),
        }
    }
    if emphasis {
        out.push_str("</em>");
    }
    if strong {
        out.push_str("</strong>");
    }
    if code {
        out.push_str("</code>");
    }
    out
}

const CSS: &str = include_str!("page.css");
const SCRIPT: &str = include_str!("page.js");
