// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Does naming a product reach the product?
//!
//! A dialect name is a promise: `--dialect opendds` and **nothing
//! else** talks to OpenDDS. Not "`--dialect specification` plus three
//! further flags, and nobody tells you which three" — that is a promise
//! whose terms are somewhere else.
//!
//! ## What this gate is, and what it replaces
//!
//! The result tables answer a different question. They ask, per row,
//! *which of the shipped configurations reproduces this byte string*,
//! and a peer whose rows are covered by four different configurations
//! comes out of that question looking fully reached. OpenDDS did: 936
//! rows, every one of them reproduced, by `impl-extension-pids` and
//! `impl-extension-pids-rti` and `enumerator-ordinal` and
//! `sized-length-codes` and `union-branches-from-zero` and
//! `short-pid-all-fourteen-bits` between them, and by no single one.
//! The tables could not see that, because they never asked one setting
//! to answer for one peer.
//!
//! This gate asks exactly that, for all eleven measured systems.
//!
//! ## The one thing a row is allowed not to reproduce
//!
//! Padding content — `docs/wire-axes-followup.md` group G. It is the
//! one difference that reaches no reader, and it is not taken on trust:
//! [`zerodds_interop_results::dialect_fit`] requires all three of the
//! criteria that document sets before it will call a row padding, and
//! the rows that pass are counted and attributed here rather than
//! waved through.
//!
//! ## Mutation-checked, at the coordinate as well as at the value
//!
//! Eight mutations, each applied on its own against the committed
//! record and then reverted. What each did:
//!
//! | mutation | verdict |
//! |---|---|
//! | `Dialect::Cyclone` back to `UnionLabelMatch::SignExtended` | **red**, 24 rows |
//! | `Dialect::RosCyclone`'s scalar `wchar` widened to four octets | **red**, 6 rows, and `the_two_ros_type_supports_are_two_dialects` with it |
//! | `Dialect::FastDds`'s `MapDelimiter` back to `EitherHalf` | **red**, 12 rows |
//! | `dialect_of` answering `Dialect::Cyclone` for **every** system — the coordinate and not the value | **red**, 1435 rows, and the padding assertion with it |
//! | the padding allowance without its reader criterion | **green** — a finding, below |
//! | the padding allowance without its layout-mask criterion | **green** — the same finding |
//! | both of the last two **together with a real miss** (`Dialect::Cyclone` reverted) | **red**, the same 24 rows: a weakened allowance does not swallow a difference that reaches a reader |
//! | `MapEntryOrder::AscendingKey` taken back out of `Dialect::FastDds` | **red** in `every_value_no_dialect_selects_is_named_with_a_reason`, which names `map-entries = ascending-key` as unaccounted-for, and in the row gate with it |
//!
//! **The finding.** Two of group G's three criteria can be removed here
//! without turning anything red, because over this record the three
//! never disagree — which is what
//! `tools/interop-results/tests/padding_positions.rs` measures and
//! asserts directly, and it is that file, not this one, whose mutations
//! separate them: moving `Writer::align_to`'s recorded span by one
//! octet takes its layout criterion from 104 rows to 23. This gate
//! consumes the classification; it does not test it. What the seventh
//! mutation establishes is the property this gate does depend on — that
//! the allowance cannot hide a real divergence — and that one is red.
//!
//! ### The three the default's move added
//!
//! Run on the measurement host against this commit, each applied on its
//! own and reverted from a copy rather than by `git checkout`.
//!
//! | mutation | verdict |
//! |---|---|
//! | `rust_column::DEFAULT` back to `"cyclone"` — the tables' name for the default, against `Dialect::default` | **red** in `the_default_configuration_is_the_default_dialect` |
//! | `Dialect::Specification` given `LengthCodePolicy::UniversalNextInt`, which no product holds | **red** in `no_single_axis_separates_the_default_from_every_measured_system`, naming `xcdr1/length-codes` and `xcdr2/length-codes` |
//! | `unattested_slots(Dialect::default())` → `unattested_slots(Dialect::Cyclone)` — the **coordinate** and not the value | **red** in `the_default_writes_octets_no_measured_system_wrote`: 147 slots and 45 cases where 84 and 31 are asserted |
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::collections::BTreeMap;

use zerodds_interop_results as results;
use zerodds_interop_results::assemble::Inputs;
use zerodds_interop_results::dialect_fit::{self, Row, Verdict};
use zerodds_interop_results::systems::{SYSTEMS, System};

/// What one system's own dialect made of its own recorded rows.
struct Answer {
    system: &'static System,
    rows: Vec<Row>,
    verdicts: Vec<Verdict>,
}

fn answers() -> Vec<Answer> {
    let root = results::repository_root();
    let results_root = results::results_root(&root);
    let cases = results::corpus(&root).expect("the corpus");
    let inputs = Inputs::read(&results_root.join("sources")).expect("the sources");
    SYSTEMS
        .iter()
        .filter(|system| dialect_fit::dialect_gated(system))
        .map(|system| {
            let rows = dialect_fit::rows(system, &cases, &inputs.foreign, &inputs.orbs);
            let dialect = dialect_fit::dialect_of(system)
                .unwrap_or_else(|| panic!("`{}` has no dialect of its own name", system.id));
            let verdicts = dialect_fit::under_dialect(&cases, &rows, dialect).verdicts;
            Answer {
                system,
                rows,
                verdicts,
            }
        })
        .collect()
}

/// Every recorded row of every measured system, under that system's own
/// dialect and no other flag.
///
/// The failure names the row and both byte strings, because a dialect
/// that is right about everything but one thing is useful and one that
/// claims to be a product and quietly is not costs somebody a week.
/// A recorded row the peer's own dialect is known not to reach, with
/// the reading that is missing. The register is checked in both
/// directions, like `three_column_gate.rs`'s: a miss with no entry
/// fails, and an entry that matches nothing fails too, so a closed
/// axis cannot leave a stale exemption behind.
///
/// Every entry is a finding of the 411-case five-vendor record and is
/// carried in `docs/OPEN-ITEMS.md` under "the dialect axes the new
/// corpus cases opened".
struct DialectGap {
    /// The `system/case` prefix of the rows this covers.
    prefix: &'static str,
    /// The missing reading, in one line.
    why: &'static str,
}

/// The rows no dialect of ours reaches.
///
/// It held four entries: the typedef'd array's inner DHEADER under
/// Cyclone DDS and Fast DDS, the forward-declared member's size under
/// Cyclone, and one PID flag bit under OpenDDS. All four are reached
/// now, by the wire fixes that gave each of them an axis.
///
/// They stayed listed because this test asserts `missed.is_empty()`
/// before it reads this list, and `missed` was not empty: fifteen rows
/// of `resolve-module-named-as-type` and `identifier-escaped-keyword`
/// failed first, so the run never got here to say the four were stale.
/// An entry added later is a real finding again; an entry that stops
/// missing anything is reported by the assertion below.
///
/// It held two RTI rows after that, the recursive `@optional` member
/// RTI writes with `PID_EXTENDED`. They are reached by
/// `PlCdr1HeaderPolicy::ExtendedForConstructedInLargeSamples`: RTI
/// decides the header form for the sample's type, and a recursion has no
/// maximum size; `crates/wire-probe/tests/rti_large_sample_headers.rs`
/// holds the measurements that placed the step.
const DIALECT_GAPS: &[DialectGap] = &[];

#[test]
fn one_dialect_alone_reproduces_every_row_of_its_own_system() {
    let mut missed: Vec<String> = Vec::new();
    let mut registered: BTreeMap<&str, usize> = BTreeMap::new();
    let mut reached: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for answer in answers() {
        let root = results::repository_root();
        let cases = results::corpus(&root).expect("the corpus");
        let mut same = 0;
        let mut padding = 0;
        for (row, verdict) in answer.rows.iter().zip(&answer.verdicts) {
            let where_ = format!(
                "{}/{}/{}/{}",
                answer.system.id, cases[row.case].name, row.encoding_name, row.endian_name
            );
            let register =
                |missed: &mut Vec<String>, registered: &mut BTreeMap<&str, usize>, line: String| {
                    if let Some(gap) = DIALECT_GAPS
                        .iter()
                        .find(|gap| where_.starts_with(gap.prefix))
                    {
                        *registered.entry(gap.prefix).or_default() += 1;
                    } else {
                        missed.push(line);
                    }
                };
            match verdict {
                Verdict::Same => same += 1,
                Verdict::PaddingOnly => padding += 1,
                Verdict::Different(mine) => register(
                    &mut missed,
                    &mut registered,
                    format!(
                        "  {where_}: --dialect {} writes {mine}, the peer wrote {}",
                        answer.system.id, row.octets
                    ),
                ),
                Verdict::Refused(why) => register(
                    &mut missed,
                    &mut registered,
                    format!("  {where_}: --dialect {} refuses — {why}", answer.system.id),
                ),
            }
        }
        reached.insert(answer.system.id, (same, padding));
    }
    assert!(
        missed.is_empty(),
        "{} recorded row(s) that the peer's own dialect does not reach on its own:\n{}",
        missed.len(),
        missed.join("\n")
    );
    // Every stale entry at once, not the first one: a change that closes
    // two gaps and leaves a third open should say so in one run, and an
    // assert per entry reports only as far as the first failure.
    let stale: Vec<String> = DIALECT_GAPS
        .iter()
        .filter(|gap| registered.get(gap.prefix).copied().unwrap_or(0) == 0)
        .map(|gap| format!("  `{}` ({})", gap.prefix, gap.why))
        .collect();
    assert!(
        stale.is_empty(),
        "{} entr(y/ies) of DIALECT_GAPS miss nothing any more — close the axis \
         and drop the entry:\n{}",
        stale.len(),
        stale.join("\n")
    );
    // A gate that passed over an empty set would be the seventh false
    // green this tree has recorded. The counts are asserted so that a
    // recording which loses rows fails here instead of passing quietly.
    // Re-measured after the 411-case five-vendor record; every system
    // grew because the corpus did.
    //
    // Re-measured again when the front-end axes closed the last unreached
    // rows: Cyclone DDS +4, Fast DDS +2 and OpenDDS +2, which are the
    // four entries `DIALECT_GAPS` had held. Those rows had been reached
    // for some time by the wire fixes that gave each its axis; nothing
    // reported it, because the assertion above this one failed first and
    // the run never got here.
    //
    // Re-measured a third time, and this time every moved row is
    // attributed rather than adjusted. The whole difference from the
    // previous set is the three recordings between it and here, and it
    // was established by running the same per-case breakdown on both
    // commits and diffing the two, not by taking the new number:
    //
    // | recording | what moved |
    // |---|---|
    // | the seven cases that stood as not-measured | Cyclone +12, Fast DDS +16, OpenDDS +12, RTI +4, JacORB +3, omniORB +6 padding, TAO +3 padding, ros-cyclone +1, ros-fastrtps +2 — over `annotation-external-{optional-absent,optional-present,typedef,union-branch,union-branch-unannotated}` and `recursive-typedef-sequence` |
    // | the last 112 cells, ORBs and ROS | JacORB +1 on `resolve-anonymous-nested-sequence`; omniORB +2 and TAO +1 on `array-in-union-branch` |
    // | `resolve-module-named-as-type` recognised as inadmissible IDL | ros-cyclone -1, ros-fastrtps -2: the two ROS front ends refuse it now where they wrote octets before |
    //
    // The two ROS columns land back on their own previous number, and
    // that is a coincidence of two movements, not a column that stood
    // still.
    //
    // A tenth entry joined with `Dialect::OpenSplice`: 161 of its 191
    // rows byte-for-byte, 30 padding-only, 0 missed — measured, not
    // carried over from any other dialect, by `dialect_fit::fit`'s
    // search against OpenSplice's own recorded octets.
    // Cyclone, RTI and Fast DDS +4 each for `key-in-base-struct` in its
    // valid form (the base's key only), which all three refused while it
    // keyed the derived type as well; re-recorded with `--cases
    // key-in-base-struct` alone.
    let counts: BTreeMap<&str, (usize, usize)> = [
        ("cyclone", (1148, 0)),
        // Every row RTI wrote, the four XCDR1 rows of `recursive-optional`
        // and `recursive-mutual-optional` included since the header form
        // is decided for the sample's type.
        ("rti", (1090, 0)),
        // +8: `annotation-external-union-branch` and its twin, written
        // once the fill reached the `@external` branch's pointee.
        ("fastdds", (1412, 0)),
        // +16: `fixed-basic`, `fixed-scales` and their unannotated twins,
        // four rows each, written through OpenDDS's FACE mapping with the
        // declared digit count `Dialect::OpenDds` takes. +4:
        // `key-on-union-discriminator`, both encoding versions by both byte
        // orders; OpenDDS is the one system that writes the case.
        ("opendds", (1168, 0)),
        // Joined with `Dialect::Hdds`: 326 rows (163 per encoding
        // version, little-endian only), every one byte-for-byte. The
        // four departures the dialect carries — packed collection
        // elements, every aggregate framed `@final`, a per-element
        // length in front of a `sequence` of a non-`@final` type, and
        // a `wstring` in UTF-8 — are each pinned by rows of its own.
        ("hdds", (326, 0)),
        ("omniorb", (282, 230)),
        ("tao", (135, 111)),
        ("jacorb", (240, 0)),
        ("ros-fastrtps", (291, 0)),
        ("ros-cyclone", (152, 0)),
        // Joined with `Dialect::OpenSplice`. 30 of its 191 rows are
        // padding-only: the same three-criteria allowance the ORBs
        // use, over the alignment fill OpenSplice leaves unwritten.
        ("opensplice", (161, 30)),
    ]
    .into_iter()
    .collect();
    assert_eq!(
        reached, counts,
        "the rows each dialect reached, as (byte-for-byte, padding-content)"
    );
}

/// The padding allowance is used by three peers and by no other, and
/// only where the octets no reader sees are the whole of the
/// difference.
///
/// Stated as its own assertion because the allowance is the one way a
/// row can pass this gate without being reproduced. A fourth peer
/// appearing here is a finding whatever else is green: it would mean a
/// difference had been reclassified rather than closed.
///
/// **OpenSplice joined at 30**, all of them `plain-cdr1/le` — the one
/// encoding and byte order it writes — over `array-3d-struct`,
/// `union-discriminator-{boolean,char,enum,short,typedef-enum,ushort}`
/// and their unannotated twins, `union-{with,without}-default`,
/// `union-member-struct`, `module-{nesting,reopened,reopened-unqualified}`,
/// `evolution-appendable-nested`, `float-negative-zero-denormal`,
/// `identifier-keyword-member-ids-unannotated`, `string-bounded-overrun`
/// and `array-of-union`: `idlpp`'s C++ leaves the alignment fill this
/// project's writer zeroes unwritten, and every case's own sample reads
/// back the same either way.
#[test]
fn the_padding_allowance_is_group_g_and_nothing_else() {
    let mut by_system: BTreeMap<&str, usize> = BTreeMap::new();
    for answer in answers() {
        let count = answer
            .verdicts
            .iter()
            .filter(|verdict| **verdict == Verdict::PaddingOnly)
            .count();
        if count > 0 {
            by_system.insert(answer.system.id, count);
        }
    }
    // omniORB 224 -> 230 and TAO 108 -> 111 with the seven cases that
    // stood as not-measured: `annotation-external-typedef`,
    // `annotation-external-union-branch` and
    // `annotation-external-union-branch-unannotated` are 2 rows each for
    // omniORB and 1 each for TAO, and nothing else moved. Established by
    // diffing the per-case breakdown against the commit these numbers
    // were last measured on.
    //
    // OpenSplice joined at 30, the count named above and unmoved since:
    // its dialect and its rows both arrived in the same commit.
    assert_eq!(
        by_system,
        [("omniorb", 230), ("opensplice", 30), ("tao", 111)]
            .into_iter()
            .collect(),
        "group G is omniORB 230, OpenSplice 30 and TAO 111, over their own dialects"
    );
}

/// Every axis of every dialect answers, and the two ROS type supports
/// are two dialects rather than one name twice.
///
/// The second half is the claim the measurement forced: `ros-fastrtps`
/// and `ros-cyclone` agree on the `wstring` cell and part on the scalar
/// `wchar`'s width — `e400000034120000` against `e4003412` for
/// `primitive-wchar` — so one dialect with two names would be wrong
/// about one of them.
#[test]
fn the_two_ros_type_supports_are_two_dialects() {
    use zerodds_ast_layout::{Dialect, Encoding};
    let fastrtps = dialect_fit::Assignment::of(Dialect::RosFastRtps, Encoding::PlainCdr1);
    let cyclone = dialect_fit::Assignment::of(Dialect::RosCyclone, Encoding::PlainCdr1);
    assert_ne!(
        fastrtps, cyclone,
        "the two ROS type supports would be one dialect under two names"
    );
    let mut differ: Vec<&str> = dialect_fit::AXES
        .iter()
        .filter(|axis| fastrtps.value_on(axis) != cyclone.value_on(axis))
        .map(|axis| axis.name)
        .collect();
    differ.sort_unstable();
    assert_eq!(
        differ,
        vec!["wchar-width", "wstring-unit-order"],
        "the two part on exactly the two axes the octets separate them on"
    );
}

/// The default configuration of the result tables is the default
/// dialect, spelled the same way.
///
/// `rust_column::DEFAULT` is a `&str` and `Dialect::default` is an
/// enumerant; the tables key the tree's `zerodds-*` columns off the
/// first and the probes plan off the second. They were `"cyclone"` and
/// `Dialect::Cyclone`; a change that moved one and not the other would
/// leave the tree recording octets under a name that no longer produces
/// them, and every cell would still look measured.
#[test]
fn the_default_configuration_is_the_default_dialect() {
    use zerodds_ast_layout::Dialect;
    assert_eq!(
        results::rust_column::DEFAULT,
        Dialect::default().as_str(),
        "the result tables' default configuration and `Dialect::default` are two different \
         dialects"
    );
    assert!(
        results::rust_column::CONFIGURATIONS
            .iter()
            .any(|configuration| configuration.name == results::rust_column::DEFAULT),
        "the default configuration is not one of the twelve"
    );
}

/// No **axis** separates the default from all eleven measured systems.
///
/// Asked first because it is the cheap question and its answer shapes
/// the expensive one below. For every axis and both encoding versions,
/// the value `Dialect::Specification` holds is held by at least one of
/// the eleven — `MustUnderstandPolicy::DeclaredAndKey` by OpenDDS,
/// `Extensibility::Appendable` by RTI, Fast DDS, OpenDDS and OpenSplice,
/// `UnionLabelMatch::SignExtended` by ten of the eleven, and so on down
/// the list.
///
/// So the default is not a form nobody writes on any single question.
/// Where it stands alone is in **combinations**, and that is what
/// [`the_default_writes_octets_no_measured_system_wrote`] measures
/// against the recorded octets rather than against the axes.
///
/// The list is exact in both directions: an axis that joins it is a
/// finding — the default would be writing a value no product holds —
/// and this test failing empty-handed means the comparison stopped
/// reaching the axes.
#[test]
fn no_single_axis_separates_the_default_from_every_measured_system() {
    use zerodds_ast_layout::{Dialect, Encoding};
    let measured: Vec<Dialect> = Dialect::ALL
        .into_iter()
        .filter(|dialect| *dialect != Dialect::Specification)
        .collect();
    assert_eq!(measured.len(), 11, "eleven measured systems and one clause");

    let mut alone: Vec<String> = Vec::new();
    let mut compared = 0_usize;
    for encoding in [Encoding::PlainCdr1, Encoding::PlainCdr2] {
        let clause = dialect_fit::Assignment::of(Dialect::Specification, encoding);
        let others: Vec<dialect_fit::Assignment> = measured
            .iter()
            .map(|dialect| dialect_fit::Assignment::of(*dialect, encoding))
            .collect();
        for axis in dialect_fit::AXES {
            compared += 1;
            let value = clause.value_on(axis);
            if !others.iter().any(|other| other.value_on(axis) == value) {
                alone.push(format!(
                    "{}/{} = {value}",
                    match encoding {
                        Encoding::PlainCdr1 => "xcdr1",
                        _ => "xcdr2",
                    },
                    axis.name
                ));
            }
        }
    }
    assert_eq!(
        compared,
        dialect_fit::AXES.len() * 2,
        "the sweep did not reach every axis in both versions"
    );
    // The one axis value the clause alone holds: §7.2.2.4.4.4.6's must-understand
    // attribute on the discriminator of a `@mutable` union. RTI Connext and OpenDDS
    // were measured writing the bit clear (`union-mutable`, `vendor-corpus.tsv`); the
    // other measured toolchains refuse a `@mutable` union. It is the maintainer's
    // decision that the default follows the clause and the vendors' reading is a
    // dialect (`MustUnderstandPolicy::DeclaredAndKey`).
    let by_the_clause: Vec<String> = ["xcdr1", "xcdr2"]
        .iter()
        .map(|version| format!("{version}/must-understand = declared-key-and-discriminator"))
        .collect();
    assert_eq!(
        alone, by_the_clause,
        "the axis values of `Dialect::default` held by none of the eleven measured dialects \
         have changed. Each is a finding: on such an axis the default writes a form no \
         product writes, whatever the combination."
    );
}

/// Where the default writes octets no measured system was recorded
/// writing.
///
/// `Dialect::Specification` is the clause-by-clause reading and is
/// `Dialect::default`. **No measured implementation produces it in
/// full**, which is the cost of a default whose warrant is the text
/// rather than a product. This test says where, in recorded slots, and
/// how many — rather than leaving the sentence as an assurance.
///
/// The method. A *slot* is one (case, encoding version, byte order) that
/// at least one of the nine wrote octets for; there are 1332 of them.
/// For each slot the default's octets are held against every system's
/// octets in that slot. A slot where none of them matches is a slot
/// where a reader of the default's bytes has no recorded peer to check
/// them against.
///
/// **84 of 1332.** They fall into five families, and none of them is a
/// single axis — the axis sweep above is empty:
///
/// | family | slots | what the default combines |
/// |---|--:|---|
/// | `map-*`, `typedef-of-map*`, `evolution-mutable-map` | 46 | Fast DDS is the only system that produces a `map`'s octets at all, and it writes the entries ascending by key with only the value half deciding the delimiter. The clause's declaration order and rule (14) read whole are what nobody was measured writing. |
/// | `enum-bitbound-*-unannotated`, `array-1d-bitmask-unannotated`, `array-3d-enum-unannotated`, `sequence-bitmask-unannotated`, `identifier-target-keyword-bitmask-flag-unannotated` | 12 | `@appendable` for an unannotated aggregate — §7.3.1.2.1.8, which RTI, Fast DDS and OpenDDS hold — **together with** a holder in the declared `@bit_bound`, which Cyclone and RTI hold. Each half is some product's; the pair is nobody's. |
/// | `combined-everything`, `combined-everything-unannotated` | 8 | the negative narrow case label selecting its branch, beside the same unannotated/bit-bound pair. |
/// | `identifier-keyword-member-ids`, `struct-inheritance-mutable`, `combined-keyed-topic*`, `evolution-mutable-optional` | 12 | `@mutable` framing under version 1: the short parameter header wherever §7.4.1.2.2 allows it, with the clause's padded `@optional` length. Cyclone writes the extended header throughout, Fast DDS the true length, OpenDDS its own parameter id. |
/// | `evolution-mutable-key` | 2 | §7.2.2.4.4.4.8's must-understand attribute on a `@key` member under version 1: `4001` where Cyclone, RTI and Fast DDS write `0001` and OpenDDS `c001`. OpenDDS is the one system that reaches the bit, and it reaches it by setting `PID_MUST_UNDERSTAND` on every member rather than on the keys. |
///
/// **It is fewer than the default it replaced, and that is measured
/// here too.** Under `Dialect::Cyclone` the same sweep gives 147 slots.
/// The reversal did not trade interoperability for correctness on this
/// record; it lowered both numbers that matter.
///
/// What this does not say: that any of the 84 is undecodable — a
/// conforming reader reads all of them, which is
/// `crates/wire-probe/tests/decode_round_trip.rs` — or that a peer
/// cannot be reached. Every one of the nine is reached by naming it,
/// which is `one_dialect_alone_reproduces_every_row_of_its_own_system`.
#[test]
fn the_default_writes_octets_no_measured_system_wrote() {
    use zerodds_ast_layout::Dialect;
    let (slots, lonely) = unattested_slots(Dialect::default());
    // 1332 before the five-vendor record; the HDDS and OpenDDS cells it
    // added carry the difference.
    //
    // 1572 -> 1596 with the seven cases that stood as not-measured. The
    // 24 are exactly 4 apiece — two encoding versions by two byte
    // orders — over `annotation-external-{optional-absent,
    // optional-present,typedef,union-branch,union-branch-unannotated}`
    // and `recursive-typedef-sequence`. `recursive-array` adds none:
    // it is inadmissible IDL and no system wrote octets for it.
    // Established by diffing the slot keys against the commit this
    // number was last measured on, not by taking the new count.
    //
    // 1596 -> 1600: the four slots of `key-in-base-struct` (two encoding
    // versions by two byte orders), which no system wrote octets for while
    // the case keyed its derived type and three do in its valid form.
    //
    // 1600 -> 1608: the four slots of each of `recursive-optional` and
    // `recursive-mutual-optional`. RTI is the one system that writes them,
    // and its octets differ from the default's in all eight: LC=5 sharing
    // NEXTINT and DHEADER under XCDR2, PID_EXTENDED under XCDR1.
    //
    // 1608 -> 1616: the XCDR2 slots (both byte orders) of `fixed-basic`,
    // `fixed-scales` and their unannotated twins. OpenDDS writes them
    // through its FACE mapping, and no system had written any `fixed`
    // case under XCDR2 before; the XCDR1 slots were the ORBs' already.
    //
    // 1616 -> 1620: the four slots of `key-on-union-discriminator`. OpenDDS
    // is the one system that writes octets for the case.
    assert_eq!(slots, 1620, "the recorded slot count moved");

    let mut by_case: BTreeMap<&str, usize> = BTreeMap::new();
    for key in &lonely {
        let case = key.split('/').next().unwrap_or(key);
        *by_case.entry(case).or_default() += 1;
    }
    let expected: BTreeMap<&str, usize> =
        DEFAULT_WRITES_WHAT_NOBODY_WROTE.iter().copied().collect();
    assert_eq!(
        by_case, expected,
        "the cases where `Dialect::default` writes octets no recorded system wrote have \
         changed. A case that joins the list is a new combination nobody was measured \
         writing; a case that leaves it means some product now agrees with the clause there. \
         Both are findings and belong in the table above."
    );
    // 84 before the 411-case record; the six new slots were the two
    // cases the table above gained (4 + 2). Four more with
    // `resolve-module-named-as-type`, where the default refuses rather
    // than writing octets of its own. Eight more with the two recursive
    // optional cases, which only RTI writes (4 + 4).
    // 102 -> 146: the 44 slots the exact PL_CDR1 length and the must-understand
    // discriminator add (20 cases, named in the list above).
    assert_eq!(lonely.len(), 146, "the slot count moved");

    // The comparison the reversal rests on, measured rather than
    // recalled: the default this one replaced stands alone on more
    // recorded slots than it does.
    // 161 -> 163 with the seven cases that stood as not-measured, and
    // the two are named: `annotation-external-union-branch-unannotated`
    // under `plain-cdr2`, both byte orders. Under `Dialect::Cyclone` an
    // unannotated aggregate is `@final`, so it writes the branch with no
    // DHEADER where OpenDDS, RTI and Fast DDS write one; under the
    // default it agrees with them and the slot is attested. The other
    // six new cases attest under both. Established by diffing the slot
    // list against the commit this number was last measured on.
    // 163 -> 171: the eight slots of the two recursive optional cases,
    // which only RTI writes, stand alone under this dialect as under the
    // default.
    // 171 -> 165: HDDS's rows joined the sweep. Cyclone reads an
    // unannotated aggregate as `@final` and so does HDDS, so six slots
    // that stood alone under Cyclone now have HDDS's octets as a
    // witness; the default's own 102 are unmoved, because the four
    // positions where HDDS departs are positions the default also
    // writes and the slots are already attested by another system.
    // 165 -> 169: the XCDR2 slots (both byte orders) of
    // `fixed-basic-unannotated` and `fixed-scales-unannotated`, which only
    // OpenDDS writes. It frames the unannotated struct `@appendable`, with
    // a DHEADER, as the default does; `Dialect::Cyclone` frames it
    // `@final` and stands alone there.
    let (_, cyclone) = unattested_slots(Dialect::Cyclone);
    assert_eq!(
        cyclone.len(),
        169,
        "the count for the previous default moved; the sentence \"fewer than the default it \
         replaced\" is this number against 102"
    );
    assert!(
        lonely.len() < cyclone.len(),
        "the shipped default now stands alone on more recorded slots than `Dialect::Cyclone` \
         did, which is the one outcome the reversal was not supposed to have"
    );
}

/// The cases of [`the_default_writes_octets_no_measured_system_wrote`],
/// with how many of their slots stand alone.
///
/// Held in both directions by that test. The families and the mechanism
/// are in its documentation; this is the list a diff can be read
/// against.
const DEFAULT_WRITES_WHAT_NOBODY_WROTE: &[(&str, usize)] = &[
    ("array-1d-bitmask-unannotated", 2),
    ("array-3d-enum-unannotated", 2),
    ("combined-everything", 4),
    ("combined-everything-unannotated", 4),
    ("combined-keyed-topic", 2),
    ("combined-keyed-topic-unannotated", 2),
    ("enum-bitbound-16-plain-unannotated", 2),
    ("enum-bitbound-16-value-unannotated", 2),
    ("enum-bitbound-8-plain-unannotated", 2),
    ("enum-bitbound-8-value-unannotated", 2),
    ("evolution-mutable-key", 2),
    ("evolution-mutable-map", 2),
    ("evolution-mutable-optional", 2),
    // Joined with the 411-case record: the corpus grew into two more
    // combinations no measured system writes — an escaped keyword as an
    // identifier, and a union discriminated by a @bit_bound enum.
    ("identifier-escaped-keyword", 4),
    ("identifier-keyword-member-ids", 4),
    ("identifier-target-keyword-bitmask-flag-unannotated", 2),
    ("map-empty", 2),
    ("map-empty-unannotated", 2),
    ("map-key-boolean", 4),
    ("map-key-boolean-unannotated", 4),
    ("map-key-char", 4),
    ("map-key-char-unannotated", 4),
    ("map-key-long", 4),
    ("map-key-long-unannotated", 4),
    ("map-key-string", 2),
    ("map-key-string-unannotated", 2),
    ("map-key-ulonglong", 4),
    ("map-key-ulonglong-unannotated", 4),
    // The one entry that is a refusal rather than different octets:
    // OMG IDL 4.2 §7.5.2 does not admit a declaration carrying the name
    // of its own scope, so the default front end refuses the file and
    // writes nothing. Fast DDS and RTI Connext take it and wrote these
    // four slots; `--dialect fastdds` and `--dialect rti` reach them,
    // which is what the `enclosing-scope-name` axis is for.
    // RTI is the one system that writes the two recursive optional cases,
    // and the default writes neither as it does: RTI frames the nested
    // `@mutable` value with LC=5, sharing NEXTINT and DHEADER, under XCDR2
    // (`--dialect rti` reaches it) and takes PID_EXTENDED for the member
    // under XCDR1 (registered in DIALECT_GAPS).
    ("recursive-mutual-optional", 4),
    ("recursive-optional", 4),
    ("resolve-module-named-as-type", 4),
    ("sequence-bitmask-unannotated", 2),
    ("struct-inheritance-mutable", 2),
    ("typedef-of-map", 2),
    ("typedef-of-map-unannotated", 2),
    ("union-discriminator-enum-bitbound", 2),
    // Joined with the exact PL_CDR1 parameter length and the must-understand
    // discriminator of a @mutable union (XTypes 1.3 §7.4.1.2, §7.2.2.4.4.4.6): the
    // default writes both and no recorded system writes the pair. The cases are the
    // @mutable ones under version 1 whose member value does not fill four octets,
    // and the cases with a @mutable union.
    ("annotation-autoid-hash", 2),
    ("annotation-autoid-sequential", 2),
    ("annotation-id-explicit", 2),
    ("annotation-must-understand", 2),
    ("annotation-must-understand-unannotated", 2),
    ("annotation-optional-mixed", 2),
    ("annotation-optional-mixed-unannotated", 2),
    ("array-1d-struct-mutable", 2),
    ("evolution-mutable-default-annotation", 2),
    ("evolution-mutable-defaults", 2),
    ("evolution-mutable-reordered", 2),
    ("evolution-union-unselected", 4),
    ("sequence-struct-mutable", 2),
    ("struct-appendable-in-mutable", 2),
    ("struct-final-in-mutable", 2),
    ("struct-mutable", 2),
    ("struct-mutable-in-appendable", 2),
    ("struct-mutable-in-final", 2),
    ("struct-mutable-length-codes", 2),
    ("union-mutable", 4),
];

/// Every recorded slot, and the ones `dialect` writes octets for that no
/// system recorded in that slot.
///
/// A slot is `<case>/<encoding>/<order>` and belongs to every system
/// that wrote octets for it. A refusal counts as a miss and is not
/// distinguished, because the sweep it serves asks whether the octets
/// have a witness and a refusal has none either.
fn unattested_slots(dialect: zerodds_ast_layout::Dialect) -> (usize, Vec<String>) {
    let root = results::repository_root();
    let results_root = results::results_root(&root);
    let cases = results::corpus(&root).expect("the corpus");
    let inputs = Inputs::read(&results_root.join("sources")).expect("the sources");
    let mut slot: BTreeMap<String, bool> = BTreeMap::new();
    for system in SYSTEMS
        .iter()
        .filter(|system| dialect_fit::dialect_gated(system))
    {
        let rows = dialect_fit::rows(system, &cases, &inputs.foreign, &inputs.orbs);
        let verdicts = dialect_fit::under_dialect(&cases, &rows, dialect).verdicts;
        for (row, verdict) in rows.iter().zip(&verdicts) {
            let key = format!(
                "{}/{}/{}",
                cases[row.case].name, row.encoding_name, row.endian_name
            );
            let seen = slot.entry(key).or_insert(false);
            if matches!(verdict, Verdict::Same | Verdict::PaddingOnly) {
                *seen = true;
            }
        }
    }
    let lonely = slot
        .iter()
        .filter(|(_, matched)| !**matched)
        .map(|(key, _)| key.clone())
        .collect();
    (slot.len(), lonely)
}

/// Every axis value no dialect selects, with the reason it is not a
/// measured peer's behaviour.
///
/// This is the sweep the dialect pass was asked for, mechanised so it
/// cannot go stale: a value that describes what a measured product puts
/// on the wire belongs in that product's dialect, and the way to find
/// the ones that do not is to enumerate the ones nothing selects and
/// look at each. Eleven of them were in this position before the pass
/// and every one was a peer's behaviour.
///
/// The reason is carried here and not in a document, because a document
/// cannot fail.
///
/// **The list is per axis value and not per `WstringLengthPolicy`
/// cell.** Three of that product's eight cells — octets/terminated/2,
/// octets/terminated/4 and characters/terminated/2 — are written by no
/// measured implementation, and each of their three *coordinates* is
/// selected by some dialect, so this sweep does not reach them. That is
/// the limit of a per-axis check and it is stated rather than left to
/// be discovered: a cell nobody writes is not a value nobody selects.
const SELECTED_BY_NO_DIALECT: &[(&str, &str, &str)] = &[
    (
        "length-codes",
        "universal-nextint",
        "an EMHEADER with an explicit four-octet length whatever the member is: four octets \
         more per member than any measured implementation writes, and none writes it",
    ),
    (
        "map-length",
        "rule-15-omits",
        "rule (15) read as written, which is what OpenDDS's generated `operator<<` does — but \
         from its source and not from octets: its probe refuses a `map` before OpenDDS sees \
         one, so no recorded row separates the two readings. docs/OPEN-ITEMS.md",
    ),
    (
        "pl-cdr1-headers",
        "short-rounded-length",
        "the short header and a parameter length rounded up to four, DDSI-RTPS 2.5 \
         §9.4.2.11; RTI Connext writes it only for a sample within the short form's limit \
         (`extended-for-constructed-in-large-samples`) and Cyclone DDS with the extended \
         header, so no dialect is this form throughout",
    ),
    (
        "default-extensibility",
        "mutable",
        "no front end takes an unannotated aggregate for `@mutable`; §7.3.1.2.1.8 gives \
         `@appendable` and Cyclone gives `@final`",
    ),
];

/// The sweep: every axis value, and whether some dialect holds it.
///
/// A value nothing selects must be in [`SELECTED_BY_NO_DIALECT`] with a
/// reason, and an entry there that some dialect *does* select must
/// leave it — checked in both directions, because a stale exemption is
/// how the next setting that belongs in a dialect stays out of one.
#[test]
fn every_value_no_dialect_selects_is_named_with_a_reason() {
    use zerodds_ast_layout::{Dialect, Encoding};
    let held: Vec<dialect_fit::Assignment> = Dialect::ALL
        .iter()
        .flat_map(|dialect| {
            [Encoding::PlainCdr1, Encoding::PlainCdr2]
                .into_iter()
                .map(move |encoding| dialect_fit::Assignment::of(*dialect, encoding))
        })
        .collect();
    let base = dialect_fit::Assignment::of(Dialect::Specification, Encoding::PlainCdr2);

    let mut unselected: Vec<(&str, String)> = Vec::new();
    for axis in dialect_fit::AXES {
        for value in axis.values {
            let mut point = base;
            value(&mut point);
            let spelled = point.value_on(axis);
            if !held.iter().any(|dialect| dialect.value_on(axis) == spelled) {
                unselected.push((axis.name, spelled));
            }
        }
    }

    let named: Vec<(&str, &str)> = SELECTED_BY_NO_DIALECT
        .iter()
        .map(|(axis, value, _)| (*axis, *value))
        .collect();
    let missing: Vec<String> = unselected
        .iter()
        .filter(|(axis, value)| !named.contains(&(axis, value.as_str())))
        .map(|(axis, value)| format!("  {axis} = {value}"))
        .collect();
    assert!(
        missing.is_empty(),
        "{} axis value(s) no dialect selects and nothing explains. Each is either a measured \
         peer's behaviour — in which case it belongs in that peer's dialect — or a form no \
         peer writes, in which case say so in SELECTED_BY_NO_DIALECT:\n{}",
        missing.len(),
        missing.join("\n")
    );

    let stale: Vec<String> = named
        .iter()
        .filter(|(axis, value)| {
            !unselected
                .iter()
                .any(|(other, held)| other == axis && held == value)
        })
        .map(|(axis, value)| format!("  {axis} = {value}"))
        .collect();
    assert!(
        stale.is_empty(),
        "{} entry of SELECTED_BY_NO_DIALECT names a value some dialect does select:\n{}",
        stale.len(),
        stale.join("\n")
    );
}
