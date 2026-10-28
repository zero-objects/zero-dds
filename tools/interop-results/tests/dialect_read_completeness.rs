// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Does naming a product **read** the product?
//!
//! `dialect_completeness.rs` proves one half of a promise:
//! `--dialect opendds` and nothing else *writes* every OpenDDS byte
//! string this record holds. This file is the other half, and it did not
//! exist: `--dialect opendds` and nothing else has to *read* those same
//! octets back into the sample they were made from.
//!
//! **Every figure in the two result tables is a write.** A dialect that
//! wrote a peer's bytes perfectly and read them into the wrong value
//! would come out of that question fully reached, and the first sign of
//! it would be a peer reporting a wrong value in the field. That is the
//! shape this gate exists to catch. Against the record as committed it
//! finds none: all 6732 readable rows of the eleven systems come back as
//! the sample under the naming dialect. What it also does — and what a
//! green row count cannot — is say which axes that green rests on, which
//! is `every_axis_the_reader_does_not_consult_is_named_with_a_reason`.
//!
//! ## What is compared
//!
//! Every recorded row of every measured system, through
//! `zerodds_wire_probe::decode` under that system's own dialect, against
//! **the case's own sample** — never against another decoder's reading
//! of the same bytes. Two decoders wrong in the same way agree with each
//! other and disagree with the sample.
//!
//! ## The two allowances, and why neither is a class
//!
//! **A row the peer's own writer left nothing readable in.** 24 of
//! Cyclone's 964. Cyclone matches no negative case label on a signed
//! discriminator narrower than 32 bits, so for `union-discriminator-short`
//! it writes the discriminator and stops: the member the sample names is not on the
//! wire, and a reader that returned it would be inventing it.
//! [`dialect_fit::ReadVerdict::NothingToRead`] is that row, and it is
//! reached by two measured conditions together — the dialect suppresses,
//! **and** this project's own writer and reader do not return the sample
//! either. Neither alone: the first would excuse every row of a dialect
//! that happens to suppress, the second would excuse any decoder defect
//! the encoder shares.
//!
//! **A case with no canonical sample.** A corpus gap, reported as its
//! own verdict so it cannot be counted as agreement.
//!
//! Nothing else is allowed. There is no per-case exemption list here,
//! because a list is what stops being true when a case is added.
//!
//! ## Mutation-checked, at the coordinate as well as at the value
//!
//! Four mutations, each applied on its own on the measurement host
//! against the committed record and then reverted from a copy.
//!
//! | mutation | verdict |
//! |---|---|
//! | `Reader::with_wstring_unit_order`'s value replaced by `WstringUnitOrder::Stream` in the sweep's request — the reader stops taking one axis from the plan | **red**, 58 rows, the first being `cyclone/array-1d-wstring/plain-cdr1/be` read as `"", "娀", "㐒"` where the sample is `"", "äZ", "ሴ"`; and **red** in the axis sweep with it |
//! | `read_under_dialect` handed `Dialect::Cyclone` for **every** system — the coordinate and not the value | **red** |
//! | the `NothingToRead` allowance without its second condition — the dialect suppresses and nothing else is asked | **red**: the allowance stops being 24 rows of one system |
//! | `Dialect::Cyclone` given `UnionLabelMatch::SignExtended`, so its 24 rows have a branch to read | **red** in both count assertions |
//!
//! The first is the defect this gate was built for, and it is the shape
//! that was reported as present: it is not, and the mutation is what
//! says so rather than a green run.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::collections::BTreeMap;

use zerodds_interop_results as results;
use zerodds_interop_results::assemble::Inputs;
use zerodds_interop_results::dialect_fit::{self, ReadVerdict};
use zerodds_interop_results::systems::SYSTEMS;

/// What one system's own dialect read back out of its own recorded rows.
struct Answer {
    system: &'static str,
    /// One line per row that did not come back as the sample.
    missed: Vec<String>,
    same: usize,
    nothing_to_read: usize,
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
            let verdicts = dialect_fit::read_under_dialect(&cases, &rows, dialect);
            let mut missed = Vec::new();
            let mut same = 0;
            let mut nothing_to_read = 0;
            for (row, verdict) in rows.iter().zip(&verdicts) {
                let where_ = format!(
                    "{}/{}/{}/{}",
                    system.id, cases[row.case].name, row.encoding_name, row.endian_name
                );
                match verdict {
                    ReadVerdict::Same => same += 1,
                    ReadVerdict::NothingToRead(_) => nothing_to_read += 1,
                    ReadVerdict::Different(mine) => {
                        missed.push(format!("  {where_}: --dialect {} read {mine}", system.id))
                    }
                    ReadVerdict::Refused(why) => {
                        missed.push(format!(
                            "  {where_}: --dialect {} refused — {why}",
                            system.id
                        ));
                    }
                    ReadVerdict::NoSample(why) => {
                        missed.push(format!("  {where_}: {why}"));
                    }
                }
            }
            Answer {
                system: system.id,
                missed,
                same,
                nothing_to_read,
            }
        })
        .collect()
}

/// Every recorded row of every measured system, read back under that
/// system's own dialect and no other flag.
///
/// The failure names the row and what came back, because a dialect that
/// reads a peer's bytes into a different value is worse than one that
/// refuses them: a refusal stops, a wrong value travels.
/// The read half of `dialect_completeness.rs::DIALECT_GAPS` — empty,
/// and asserted empty, exactly as that register now is.
///
/// It held the typedef'd array under Cyclone DDS and Fast DDS and the
/// forward-declared member under Cyclone. All three read back under
/// their own dialect now, by the same wire fixes that closed them on
/// the write side, and for the same reason nothing said so: the write
/// register failed first and `cargo test` stops at the first failing
/// target, so this file had not run.
const READ_GAPS: &[&str] = &[];

#[test]
fn one_dialect_alone_reads_back_every_row_of_its_own_system() {
    let answers = answers();
    let mut registered: BTreeMap<&str, usize> = BTreeMap::new();
    let missed: Vec<String> = answers
        .iter()
        .flat_map(|answer| answer.missed.clone())
        .filter(|line| {
            match READ_GAPS
                .iter()
                .find(|prefix| line.trim_start().starts_with(**prefix))
            {
                Some(prefix) => {
                    *registered.entry(prefix).or_default() += 1;
                    false
                }
                None => true,
            }
        })
        .collect();
    // Every stale entry at once, for the reason the write side reports
    // them that way: a change that closes two and leaves a third open
    // should say so in one run.
    let stale: Vec<String> = READ_GAPS
        .iter()
        .filter(|prefix| registered.get(*prefix).copied().unwrap_or(0) == 0)
        .map(|prefix| format!("  `{prefix}`"))
        .collect();
    assert!(
        stale.is_empty(),
        "{} entr(y/ies) of READ_GAPS read back cleanly now — close the axis and drop \
         the entry:\n{}",
        stale.len(),
        stale.join("\n")
    );
    assert!(
        missed.is_empty(),
        "{} recorded row(s) that the peer's own dialect does not read back into the sample:\n{}",
        missed.len(),
        missed.join("\n")
    );
    // A gate that passed over an empty set would be the seventh false
    // green this tree has recorded. Both counts are asserted so that a
    // recording which loses rows fails here instead of passing quietly.
    let reached: BTreeMap<&str, (usize, usize)> = answers
        .iter()
        .map(|answer| (answer.system, (answer.same, answer.nothing_to_read)))
        .collect();
    // Re-measured after the 411-case five-vendor record.
    //
    // Re-measured again, and every moved row attributed by diffing a
    // per-case breakdown against the commit the previous set was
    // measured on — the same method the write side uses. Three
    // recordings carry the whole difference:
    //
    // | recording | what moved |
    // |---|---|
    // | the seven cases that stood as not-measured | Cyclone +12, Fast DDS +16, OpenDDS +12, RTI +4, omniORB +6, TAO +3, JacORB +3, ros-cyclone +1, ros-fastrtps +2 |
    // | the last 112 cells, ORBs and ROS | JacORB +1 on `resolve-anonymous-nested-sequence`; omniORB +2 and TAO +1 on `array-in-union-branch` |
    // | `resolve-module-named-as-type` recognised as inadmissible IDL | ros-cyclone -1, ros-fastrtps -2 |
    //
    // The two ROS columns land back on their own previous number, which
    // is two movements cancelling and not a column that stood still.
    //
    // Cyclone, RTI and Fast DDS +4 each: `key-in-base-struct` keyed a
    // member of the derived type, which every one of them refused; in its
    // valid form (the base's key only) they write it, two encoding versions
    // by two byte orders, and read it back. Re-recorded with `--cases
    // key-in-base-struct` alone.
    //
    // RTI +8: `recursive-optional` and `recursive-mutual-optional`, which
    // RTI writes and no other measured system does; the RTI dialect reads
    // all eight back.
    //
    // Fast DDS +8: `annotation-external-union-branch` and its twin, which
    // the fill had recorded as Fast DDS refusals until it wrote the
    // `@external` branch through its pointee.
    let counts: BTreeMap<&str, (usize, usize)> = [
        ("cyclone", (1124, 24)),
        ("rti", (1090, 0)),
        ("fastdds", (1412, 0)),
        // +16: the four `fixed` cases OpenDDS writes through its FACE
        // mapping, four rows each, read back under `Dialect::OpenDds`.
        // +4: `key-on-union-discriminator`, OpenDDS the one system that
        // writes it.
        ("opendds", (1168, 0)),
        // Joined with `Dialect::Hdds`: all 326 of its rows (163 per
        // encoding version, little-endian only) read back as the sample
        // under its own dialect.
        ("hdds", (326, 0)),
        ("omniorb", (512, 0)),
        ("tao", (246, 0)),
        ("jacorb", (240, 0)),
        ("ros-fastrtps", (291, 0)),
        ("ros-cyclone", (152, 0)),
        // OpenSplice joined: all 191 of its rows read back as the
        // sample, under `UnionLabelMatch::SignExtended` and nothing
        // suppressed.
        ("opensplice", (191, 0)),
    ]
    .into_iter()
    .collect();
    assert_eq!(
        reached, counts,
        "the rows each dialect read back, as (into the sample, nothing to read)"
    );
}

/// What the **default** alone reads of the same 5207 rows, and why the
/// rest is not a defect.
///
/// Reading a peer's octets **without naming the peer** is reading them
/// under the clause, and a peer that departs from the clause is a peer
/// the clause cannot read. That is not a reader defect and it is easy to
/// take for one: `array-1d-wstring` little-endian-labelled from Cyclone
/// under `--dialect cyclone` is `"", "äZ", "ሴ"`, and with no dialect it
/// is `"", "娀", "㐒"` — Cyclone's big-endian `wstring` read as the
/// stream says it is labelled, which is exactly the divergence
/// `Dialect::wstring_unit_order` records.
///
/// So the number is recorded here rather than left to be rediscovered
/// as a suspicion, and it is recorded as three counts and not one: read
/// back, read into another value, refused. **Read into another value is
/// the dangerous one** — a refusal names the setting the bytes do fit
/// and stops, a wrong value travels — which is why the split is
/// asserted and not the total.
#[test]
fn what_the_default_alone_reads_of_every_systems_rows() {
    let root = results::repository_root();
    let results_root = results::results_root(&root);
    let cases = results::corpus(&root).expect("the corpus");
    let inputs = Inputs::read(&results_root.join("sources")).expect("the sources");
    let sweep = dialect_fit::Sweep::new(&cases);

    let mut same = 0_usize;
    let mut different = 0_usize;
    let mut refused = 0_usize;
    let mut nothing = 0_usize;
    let mut by_system: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for system in SYSTEMS
        .iter()
        .filter(|system| dialect_fit::dialect_gated(system))
    {
        let rows = dialect_fit::rows(system, &cases, &inputs.foreign, &inputs.orbs);
        let verdicts = sweep.read(&cases, &rows, &|encoding| {
            dialect_fit::Assignment::of(zerodds_ast_layout::Dialect::default(), encoding)
        });
        let mut wrong = 0;
        let mut no = 0;
        for verdict in &verdicts {
            match verdict {
                ReadVerdict::Same => same += 1,
                ReadVerdict::Different(_) => {
                    different += 1;
                    wrong += 1;
                }
                ReadVerdict::Refused(_) => {
                    refused += 1;
                    no += 1;
                }
                ReadVerdict::NothingToRead(_) => nothing += 1,
                ReadVerdict::NoSample(why) => panic!("{why}"),
            }
        }
        if wrong + no > 0 {
            by_system.insert(system.id, (wrong, no));
        }
    }

    // 5207 before the 411-case five-vendor record.
    //
    // 6151 -> 6211: 63 rows the three recordings added (the seven cases
    // that stood as not-measured, plus `resolve-anonymous-nested-sequence`
    // and `array-in-union-branch` from the last 112 ORB and ROS cells),
    // less 3 that `resolve-module-named-as-type` lost when the two ROS
    // front ends began refusing it as inadmissible IDL.
    //
    // 6211 -> 6402: OpenSplice's 191 rows joined, all of them read into
    // the sample under the clause — its answer is the clause's on every
    // axis, so nothing here moves `different` or `refused`.
    //
    // 6402 -> 6414: the twelve rows Cyclone, RTI and Fast DDS wrote for
    // `key-in-base-struct` once it keyed the base only (4 apiece).
    //
    // 6414 -> 6422: the eight rows RTI wrote for `recursive-optional` and
    // `recursive-mutual-optional` (4 apiece).
    //
    // 6422 -> 6430: the eight rows Fast DDS wrote for
    // `annotation-external-union-branch` and its twin (4 apiece).
    //
    // 6430 -> 6756: HDDS's 326 rows joined. Under the clause 159 read
    // into the sample, 16 into another value and 151 are refused — the
    // departures the clause does not write are the ones it cannot read
    // back without naming HDDS. Broken down per case and in the
    // by-system map below.
    //
    // 6756 -> 6772: the sixteen rows OpenDDS wrote through its FACE
    // mapping for `fixed-basic`, `fixed-scales` and their unannotated
    // twins, all read into the sample under the clause's declared width.
    //
    // 6772 -> 6776: the four rows OpenDDS wrote for
    // `key-on-union-discriminator`, all read into the sample.
    assert_eq!(
        same + different + refused + nothing,
        6776,
        "the recorded row count moved"
    );
    // Nothing is suppressed under the default — it takes the clause's
    // `UnionLabelMatch::SignExtended` — so the allowance that Cyclone's
    // own dialect needs is not in force here.
    assert_eq!(
        nothing, 0,
        "the default suppresses a negative narrow case label, which it must not"
    );
    // (4374, 111, 722) before the 411-case five-vendor record.
    //
    // (5212, 125, 814) before the refusal OMG IDL 4.2 §7.5.2 asks for.
    // Eight rows of `resolve-module-named-as-type` are refused by this
    // reader — measured, and they are every row of that case any system
    // wrote octets for: four from Fast DDS and four from RTI Connext.
    // It was eleven; the two ROS front ends now refuse the case
    // themselves as inadmissible IDL and write no octets, so their
    // three rows are not there to be read. All eight are
    // read by naming that vendor, which is the `enclosing-scope-name`
    // axis; the default is the clause. The two that moved into "another
    // value" are `resolve-array-of-typedef-array`, where the default
    // frames a typedef'd array as §7.4.3.5.3 reads it and Cyclone DDS
    // and Fast DDS do not — reached by naming either of them.
    //
    // The numbers were not re-measured when that refusal landed because
    // this target never ran: `cargo test` stops at the first failing
    // target, and `dialect_completeness` failed ahead of it.
    // (5205, 127, 819) -> (5268, 127, 816). Every one of the 63 new
    // rows reads into the sample under the clause, so `different` does
    // not move; the three the ROS columns lost were all refusals, which
    // is where the -3 sits. Measured per case, not adjusted.
    // (5268, 127, 816) -> (5269, 166, 776): the XCDR1 appendable-tail read of
    // §7.2.4.2 (crates/wire-codec `appended_members_end_the_sample`, tested in
    // appendable_tail_xcdr1.rs). Under the Specification default an unannotated
    // aggregate is appendable, so a one-member reader takes a longer writer's
    // trailing octets as appended members and reads the base value instead of
    // refusing: 39 rows move refused -> another value, 1 -> into the sample.
    // The read is dialect-gated: `Dialect::Cyclone` keeps such unannotated
    // rows final and still refuses them (below). Measured per case.
    // (5460, 166, 776) -> (5472, 166, 776): the twelve rows Cyclone, RTI and
    // Fast DDS wrote for `key-in-base-struct` in its valid form, all read
    // into the sample.
    // (5472, 166, 776) -> (5480, 166, 776): the eight rows RTI wrote for
    // `recursive-optional` and `recursive-mutual-optional`, all read into the
    // sample under the clause.
    // (5480, 166, 776) -> (5488, 166, 776): the eight rows Fast DDS wrote for
    // `annotation-external-union-branch` and its twin, all read into the
    // sample under the clause.
    // (5488, 166, 776) -> (5647, 182, 927): HDDS's 326 rows joined. Its
    // packed elements, headerless aggregates and UTF-8 `wstring` are
    // misframed under the clause, so most are refused and a few read
    // into another value; all 326 read back under `Dialect::Hdds`.
    // (5647, 182, 927) -> (5663, 182, 927): the sixteen rows OpenDDS wrote
    // for the four `fixed` cases, all read into the sample under the
    // clause.
    // (5663, 182, 927) -> (5593, 180, 999): the default reader takes the exact
    // parameter length of XTypes 1.3 §7.4.1.2 and reports a length rounded up
    // to four (Cyclone DDS, RTI Connext) as `WrongFraming`; 72 rows that read
    // before are refused now, and they read under the dialect of the system
    // that wrote them or under `PlCdr1Reading::Tolerant`.
    // (5593, 180, 999) -> (5597, 180, 999): the four rows OpenDDS wrote for
    // `key-on-union-discriminator`, all read into the sample.
    assert_eq!(
        (same, different, refused),
        (5597, 180, 999),
        "what the default alone makes of the eleven systems' rows, as (into the sample, another \
         value, refused)"
    );
    assert_eq!(
        by_system,
        [
            // Re-measured after the 411-case five-vendor record, and
            // again with the §7.5.2 refusal above. What moved, per case:
            // `resolve-module-named-as-type` is refused four times each
            // under Fast DDS and RTI Connext, twice under `ros-fastrtps`
            // and once under `ros-cyclone`; `resolve-array-of-typedef-array`
            // reads into another value twice under OpenDDS, where the
            // default frames the typedef'd array as §7.4.3.5.3 reads it.
            // Cyclone DDS moved the other way, refusing two rows fewer in
            // the `identifier-*-unannotated` family.
            //
            // Then `resolve-module-named-as-type` was recognised as
            // inadmissible IDL and the two ROS front ends stopped
            // writing octets for it: `ros-cyclone` 13 -> 12 refusals and
            // `ros-fastrtps` 36 -> 34, which are the one and the two
            // rows named above. The eight that remain refused for that
            // case are Fast DDS's four and RTI Connext's four. No other
            // entry moved — the 63 rows the recordings added all read
            // into the sample under the clause.
            // 2026-09-16: the §7.2.4.2 appendable-tail read moves 39 rows per
            // system from refused to another value (base members, appended
            // ones ignored): fastdds +6, jacorb +2, omniorb +4, opendds +6,
            // ros-fastrtps +4, rti +14, tao +3. Cyclone and ros-cyclone carry
            // no unannotated top-level @appendable XCDR1 row here and do not
            // move. Measured per case.
            ("cyclone", (15, 304)),
            ("fastdds", (16, 182)),
            // HDDS's rows under the clause: its packed elements and
            // headerless aggregates are read as the clause frames them,
            // which is not what HDDS wrote. All 326 read back under
            // `Dialect::Hdds`.
            ("hdds", (16, 151)),
            ("jacorb", (8, 12)),
            ("omniorb", (41, 14)),
            ("opendds", (20, 122)),
            ("ros-cyclone", (0, 12)),
            ("ros-fastrtps", (8, 30)),
            ("rti", (38, 162)),
            ("tao", (18, 10)),
        ]
        .into_iter()
        .collect(),
        "which system's rows the clause does not read into the sample, as (another value, \
         refused). Every one of them reads back under that system's own dialect, which is \
         `one_dialect_alone_reads_back_every_row_of_its_own_system`; this is the price of not \
         naming the peer and not a defect in the reader."
    );
    // The comparison that says whether the split is acceptable, and the
    // reason it is stated as three numbers and not one: a refusal names
    // the setting the bytes do fit and stops, a wrong value travels.
    //
    // The default this one replaced reads **less** of the record, not
    // more: `Dialect::Cyclone` over the same rows gives 4673 into the
    // sample against 5268, and 143 into another value against 127. It
    // reads its own rows and pays for them everywhere else, which is
    // what a default that is one product's answers costs on the read
    // side. Both triples are asserted so the comparison cannot go stale.
    let (cyclone, exhausted_tails) = read_totals(
        &cases,
        &inputs,
        &sweep,
        zerodds_ast_layout::Dialect::Cyclone,
    );
    // 57f4d99f made read_aggregate use the plan's dialect-aware
    // extensibility_of, not Ast's implicit Appendable default. Cyclone's
    // unannotated aggregates are Final. These foreign PLAIN_CDR2 bytes,
    // misframed under Cyclone, exhaust the stream before the required tail.
    // Previously Tail::reached silently defaulted that tail to zero and
    // returned Different. A Final reader must refuse, not invent it.
    // Pin every affected coordinate AND its octets: this is a refusal
    // regression, not an exemption or a count-only reclassification.
    assert_eq!(exhausted_tails, required_tail_refusal_octets());
    assert_eq!(
        cyclone,
        // (3809, 151, 1159) before the 411-case five-vendor record.
        //
        // (4616, 167, 1280) -> (4673, 167, 1283). Of the 63 rows the
        // three recordings added, this dialect reads 57 into the sample
        // and refuses 6 — Fast DDS's `annotation-external-optional-absent`
        // and `-present` under version 2, two rows each, and OpenDDS's
        // `annotation-external-union-branch-unannotated`, also two. The
        // clause reads all 63. The refusal count then loses the three
        // rows `resolve-module-named-as-type` no longer has, which is
        // 1280 + 6 - 3. Measured per case against the commit the
        // previous triple was measured on.
        // The same 6211 recorded rows, with precisely the 24 required-tail
        // coordinates above moving Different -> Refused; no new Same rows.
        // (4673, 143, 1307) -> (4674, 149, 1300): under the Cyclone dialect the
        // same appendable-tail read applies only to rows that are EXPLICITLY
        // @appendable (Cyclone keeps unannotated aggregates final), so 7 rows
        // move refused -> read (6 to another value, 1 into the sample) where
        // the default moved 40. Measured per case.
        //
        // (4674, 149, 1300) -> (4863, 149, 1300): OpenSplice's 191 rows
        // joined. 189 read into the sample; the other two —
        // `union-discriminator-short` and its unannotated twin, the one
        // byte order and encoding version it writes — are
        // `NothingToRead` under `Dialect::Cyclone`'s
        // `UnionLabelMatch::NegativeLabelUnmatchedBelow32`, which
        // suppresses the branch OpenSplice's own dialect reads back:
        // excluded from this triple for the reason every other
        // suppressed row is. `different` and `refused` do not move.
        //
        // (4863, 149, 1300) -> (4875, 149, 1300): the twelve rows Cyclone,
        // RTI and Fast DDS wrote for `key-in-base-struct` in its valid
        // form, `@final` on both structs, all read into the sample.
        // (4875, 149, 1300) -> (4879, 149, 1304): the eight rows RTI wrote
        // for `recursive-optional` and `recursive-mutual-optional`, four read
        // into the sample and four refused under this dialect. Taken from the
        // totals; which four is not broken down here.
        // (4879, 149, 1304) -> (4885, 149, 1306): the eight rows Fast DDS wrote
        // for `annotation-external-union-branch` and its twin, six read into
        // the sample and two refused under this dialect. Taken from the
        // totals; which two is not broken down here.
        //
        // (4885, 149, 1306) -> (5091, 161, 1414): HDDS's 326 rows joined
        // the sweep. 206 read into the sample, 12 into another value and
        // 108 are refused — Cyclone's `@final` unannotated aggregates
        // agree with HDDS's on the simple shapes and part on the packed
        // elements, the sequence-element length and the UTF-8 `wstring`.
        //
        // (5091, 161, 1414) -> (5103, 161, 1418): the sixteen rows OpenDDS
        // wrote for the four `fixed` cases, twelve read into the sample and
        // four refused. Four is the count of XCDR2 rows of the two
        // unannotated cases, which OpenDDS frames `@appendable` with a
        // DHEADER and this dialect reads as `@final`; taken from the totals,
        // not broken down per row here.
        //
        // (5103, 161, 1418) -> (5107, 161, 1418): the four rows OpenDDS
        // wrote for `key-on-union-discriminator`, all read into the sample.
        (5107, 161, 1418),
        "the previous default's reading reach moved; the sentence this test carries is these \
         three numbers against the three above"
    );
    // Cyclone is a reference, not a ceiling: it may itself change tomorrow, and
    // the Specification default is deliberately the spec clause, not a copy of
    // one vendor. The default reads at least as much of the record into the
    // sample as the product it replaced — the one direction moving the default
    // is still not allowed to take. It DOES read more into another value, but
    // that is the §7.2.4.2 appendable-tail evolution (base members, appended
    // ones ignored) reached under the appendable-by-default extensibility, not
    // the default inventing a wrong value; the pinned per-case tallies above
    // are the guard against a genuinely wrong read.
    assert!(
        same >= cyclone.0,
        "the clause reads less of the record into the sample than the product it replaced — \
         the one direction moving the default is still not allowed to take."
    );
}

/// The three read counts and exact octets of exhausted required-tail refusals.
fn read_totals(
    cases: &[results::Case],
    inputs: &Inputs,
    sweep: &dialect_fit::Sweep,
    dialect: zerodds_ast_layout::Dialect,
) -> ((usize, usize, usize), BTreeMap<String, String>) {
    let (mut same, mut different, mut refused) = (0, 0, 0);
    let mut exhausted_tails = BTreeMap::new();
    for system in SYSTEMS
        .iter()
        .filter(|system| dialect_fit::dialect_gated(system))
    {
        let rows = dialect_fit::rows(system, cases, &inputs.foreign, &inputs.orbs);
        for (row, verdict) in rows.iter().zip(sweep.read(cases, &rows, &|encoding| {
            dialect_fit::Assignment::of(dialect, encoding)
        })) {
            match verdict {
                ReadVerdict::Same => same += 1,
                ReadVerdict::Different(_) => different += 1,
                ReadVerdict::Refused(why) => {
                    if why.starts_with("Sample.tail:")
                        && why.ends_with("4 bytes needed, 0 available")
                    {
                        exhausted_tails.insert(
                            format!(
                                "{}/{}/{}/{}",
                                system.id, cases[row.case].name, row.encoding_name, row.endian_name
                            ),
                            row.octets.clone(),
                        );
                    }
                    refused += 1;
                }
                // A suppressed branch is not a reading and is counted
                // nowhere here: this function compares two dialects'
                // reach and one of them suppresses.
                ReadVerdict::NothingToRead(_) | ReadVerdict::NoSample(_) => {}
            }
        }
    }
    ((same, different, refused), exhausted_tails)
}

/// Measured foreign bytes that cannot supply a final aggregate's required tail
/// under the Cyclone assignment. Every entry must remain Refused, not Same or
/// Different. The peer's own dialect is independently required to read it.
fn required_tail_refusal_octets() -> BTreeMap<String, String> {
    let mut expected = BTreeMap::new();
    for (systems, case, le, be) in [
        (
            &["rti", "opendds"][..],
            "wstring-bounded-utf8-unannotated",
            "10000000060000003412e4005a000000ffffffff",
            "0000001000000006123400e4005a0000ffffffff",
        ),
        (
            &["rti", "opendds", "fastdds"][..],
            "wstring-unbounded-empty-unannotated",
            "0800000000000000ffffffff",
            "0000000800000000ffffffff",
        ),
        (
            &["rti", "opendds"][..],
            "wstring-unbounded-utf8-unannotated",
            "180000000e000000e400f600fc002000341220005a000000ffffffff",
            "000000180000000e00e400f600fc002012340020005a0000ffffffff",
        ),
        (
            &["fastdds"][..],
            "wstring-bounded-utf8-unannotated",
            "10000000030000003412e4005a000000ffffffff",
            "0000001000000003123400e4005a0000ffffffff",
        ),
        (
            &["fastdds"][..],
            "wstring-unbounded-utf8-unannotated",
            "1800000007000000e400f600fc002000341220005a000000ffffffff",
            "000000180000000700e400f600fc002012340020005a0000ffffffff",
        ),
        (
            &["fastdds"][..],
            "map-empty-unannotated",
            "0800000000000000ffffffff",
            "0000000800000000ffffffff",
        ),
        (
            &["fastdds"][..],
            "map-key-string-unannotated",
            "340000000300000001000000000000007929edff06000000616c706861000000ffffff7f06000000c3bce188b400000001000000ffffffff",
            "00000034000000030000000100000000ffed297900000006616c7068610000007fffffff00000006c3bce188b400000000000001ffffffff",
        ),
        (
            &["fastdds"][..],
            "typedef-of-map-unannotated",
            "240000000200000006000000616c7068610000007929edff03000000c3bc0000ffffff7fffffffff",
            "000000240000000200000006616c706861000000ffed297900000003c3bc00007fffffffffffffff",
        ),
    ] {
        for system in systems {
            for (endian, octets) in [("le", le), ("be", be)] {
                expected.insert(
                    format!("{system}/{case}/plain-cdr2/{endian}"),
                    octets.to_string(),
                );
            }
        }
    }
    // HDDS writes one byte order and one framing per version, so its
    // three exhausted tails are little-endian `plain-cdr1` and have no
    // big-endian twin. They are the `@final` `Cell` sequences, whose
    // elements the clause pads and HDDS packs: the clause's reader wants
    // four bytes of required tail the packed row never wrote.
    for case in [
        "sequence-struct-final",
        "sequence-struct-final-unannotated",
        "sequence-struct-unannotated",
    ] {
        expected.insert(
            format!("hdds/{case}/plain-cdr1/le"),
            "00000000010000007929edfffeff000003000000010000000200ffffffffff7fffffff7f00800000ffffffff"
                .to_string(),
        );
    }
    expected
}

/// Which axes the **read** direction is sensitive to, per system, and a
/// written reason for every one it is not.
///
/// The sweep [`one_dialect_alone_reads_back_every_row_of_its_own_system`]
/// cannot stand in for. That gate is green if the reader is right — and
/// also green if the reader never consults an axis on which the naming
/// dialect happens to agree with whatever the reader assumed. This one
/// asks the question directly, and it is the gate that would have caught
/// a reader taking a `wstring`'s code units in the stream's order while
/// `--dialect cyclone` wrote them in the host's.
///
/// For each measured system and each of the 25 axes, the system's own
/// assignment is taken and that **one** coordinate moved to every other
/// value the axis has. Three outcomes, and they are three different
/// findings:
///
/// - **some recorded row now reads back differently** — a different
///   value or a refusal. The reader consults the axis. Nothing to say.
/// - **no row reads differently and no row is written differently
///   either.** The record cannot ask the question about this system:
///   moving the axis reaches none of its rows. A limit of the corpus.
/// - **no row reads differently and some row is written differently.**
///   The axis reaches this system's rows and the reader does not move
///   with it. That is either an axis the *stream itself* carries — a
///   length code is in the EMHEADER, a must-understand bit is in the
///   header, a `map`'s entry order is the order the entries arrive in —
///   or a reader that is not looking. The difference cannot be
///   measured, so it is written down: [`READ_INSENSITIVE`] carries one
///   line per pair and the test holds the list in both directions.
///
/// A pair that appears without an entry fails, and an entry naming a
/// pair the reader does move on fails. Both directions, because a stale
/// exemption is how the next axis the reader stops consulting stays out
/// of sight.
#[test]
fn every_axis_the_reader_does_not_consult_is_named_with_a_reason() {
    let root = results::repository_root();
    let results_root = results::results_root(&root);
    let cases = results::corpus(&root).expect("the corpus");
    let inputs = Inputs::read(&results_root.join("sources")).expect("the sources");

    let sweep = dialect_fit::Sweep::new(&cases);
    let mut insensitive: Vec<(String, String)> = Vec::new();
    // Kept beside the count so a stale entry of `READ_INSENSITIVE` can
    // say *which* way it went stale: a reader that now consults the axis
    // is a gain, and an axis nothing moves any more is a different
    // finding that needs a different answer.
    let mut consulted_pairs: Vec<(String, String)> = Vec::new();
    let mut unreached = 0_usize;
    let mut consulted = 0_usize;
    for system in SYSTEMS
        .iter()
        .filter(|system| dialect_fit::dialect_gated(system))
    {
        let rows = dialect_fit::rows(system, &cases, &inputs.foreign, &inputs.orbs);
        let dialect = dialect_fit::dialect_of(system)
            .unwrap_or_else(|| panic!("`{}` has no dialect of its own name", system.id));
        let base = sweep.read(&cases, &rows, &|encoding| {
            dialect_fit::Assignment::of(dialect, encoding)
        });
        // The write direction's own baseline. `wrote_moved` used to ask
        // whether the moved point wrote *anything* wrong, which is a
        // different question: while this system had rows no dialect of
        // ours reached, that was true of every axis, and twenty
        // (system, axis) pairs were filed as "the stream carries it,
        // the reader does not consult it" on the strength of rows that
        // were failing for an unrelated reason. Compared against the
        // baseline, the way the read direction above already is.
        let base_write = sweep
            .write(&cases, &rows, &|encoding| {
                dialect_fit::Assignment::of(dialect, encoding)
            })
            .verdicts;
        for axis in dialect_fit::AXES {
            let mut read_moved = false;
            let mut wrote_moved = false;
            for value in axis.values {
                let moved = |encoding| {
                    let mut point = dialect_fit::Assignment::of(dialect, encoding);
                    value(&mut point);
                    point
                };
                if !moves(dialect, axis, &moved) {
                    continue;
                }
                if sweep.read(&cases, &rows, &moved) != base {
                    read_moved = true;
                    break;
                }
                if !wrote_moved {
                    wrote_moved = sweep.write(&cases, &rows, &moved).verdicts != base_write;
                }
            }
            if read_moved {
                consulted += 1;
                consulted_pairs.push((system.id.to_string(), axis.name.to_string()));
            } else if wrote_moved {
                insensitive.push((system.id.to_string(), axis.name.to_string()));
            } else {
                unreached += 1;
            }
        }
    }

    // The sweep ran. A pass over nothing would be the seventh false
    // green this tree has recorded.
    assert_eq!(
        consulted + insensitive.len() + unreached,
        SYSTEMS
            .iter()
            .filter(|s| dialect_fit::dialect_gated(s))
            .count()
            * dialect_fit::AXES.len(),
        "the sweep did not reach every (system, axis) pair"
    );
    assert!(
        consulted > 0,
        "no axis moved a single reading, which means the comparison is not comparing"
    );

    let named: Vec<(String, String)> = READ_INSENSITIVE
        .iter()
        .map(|(system, axis, _)| ((*system).to_string(), (*axis).to_string()))
        .collect();
    let missing: Vec<String> = insensitive
        .iter()
        .filter(|pair| !named.contains(pair))
        .map(|(system, axis)| format!("  {system}/{axis}"))
        .collect();
    assert!(
        missing.is_empty(),
        "{} (system, axis) pair(s) where moving the axis changes what this project writes for \
         a recorded row and changes nothing it reads back, and nothing explains them. Each is \
         either an axis the stream carries — in which case say so in READ_INSENSITIVE — or a \
         reader that is not consulting the dialect, which is half a promise:\n{}",
        missing.len(),
        missing.join("\n")
    );
    let stale: Vec<String> = named
        .iter()
        .filter(|pair| !insensitive.contains(pair))
        .map(|(system, axis)| {
            let how = if consulted_pairs.contains(&((*system).clone(), (*axis).clone())) {
                "the reader consults it now"
            } else {
                "nothing moves on it any more, in either direction"
            };
            format!("  {system}/{axis} — {how}")
        })
        .collect();
    assert!(
        stale.is_empty(),
        "{} entr(y/ies) of READ_INSENSITIVE no longer name a pair that moves the write and \
         not the read:\n{}",
        stale.len(),
        stale.join("\n")
    );
}

/// Whether moving this axis to `value` changes the assignment at all,
/// in either encoding version.
///
/// A value that is already the one the dialect holds is not a move, and
/// sweeping it would compare a plan against itself.
fn moves(
    dialect: zerodds_ast_layout::Dialect,
    axis: &dialect_fit::Axis,
    moved: &dyn Fn(zerodds_ast_layout::Encoding) -> dialect_fit::Assignment,
) -> bool {
    [
        zerodds_ast_layout::Encoding::PlainCdr1,
        zerodds_ast_layout::Encoding::PlainCdr2,
    ]
    .into_iter()
    .any(|encoding| {
        moved(encoding).value_on(axis)
            != dialect_fit::Assignment::of(dialect, encoding).value_on(axis)
    })
}

/// The (system, axis) pairs the reader does not move on, with why.
///
/// Every one of them is an axis the **stream carries explicitly**: the
/// writer decides a form, the form is in the octets, and a reader reads
/// what is there rather than what a flag told it to expect. That is a
/// property of XCDR and not a shortcut — a reader that refused an
/// EMHEADER whose length code was not the one its dialect writes could
/// not read a peer that took another of the codes §7.4.3.4.2 permits.
///
/// It is written down per system rather than per axis, because whether
/// the record can ask the question is a property of the system's rows:
/// `opendds` separates `short-pid-range` in the read direction and RTI
/// and Fast DDS do not, so the same axis is measured for one and
/// unmeasurable for the others.
const READ_INSENSITIVE: &[(&str, &str, &str)] = &[
    (
        "cyclone",
        "length-codes",
        "§7.4.3.4.2 puts the length code in the EMHEADER. A reader takes the code the stream \
         carries and computes the member's length from it; which code the *writer* chooses is \
         a writer's question and reaches no reading.",
    ),
    (
        "cyclone",
        "must-understand",
        "§7.4.1.2.2 puts the attribute in the member's own header. A reader reads the bit; \
         whether the writer sets it for a `@key` member is a writer's question, and a member \
         this reader knows is read the same either way.",
    ),
    (
        "rti",
        "length-codes",
        "the same as `cyclone/length-codes`: the code is in the stream.",
    ),
    (
        "rti",
        "must-understand",
        "the same as `cyclone/must-understand`: the bit is in the stream.",
    ),
    (
        "rti",
        "short-pid-range",
        "read-side only by construction — `ShortPidRange::highest_short_id` tells a reader \
         which short parameter ids are member ids and which are reserved. No recorded RTI row \
         carries an id above §7.4.1.2.2's `0x3F00` in a short header, so its record cannot ask \
         the question. OpenDDS's two rows do, and `opendds/short-pid-range` is not in this \
         list.",
    ),
    (
        "fastdds",
        "length-codes",
        "the same as `cyclone/length-codes`: the code is in the stream.",
    ),
    (
        "fastdds",
        "must-understand",
        "the same as `cyclone/must-understand`: the bit is in the stream.",
    ),
    (
        "fastdds",
        "map-entries",
        "§7.4.1.4.4 gives a `map` as a length and a run of pairs. The order they arrive in is \
         the order they are read in; `MapEntryOrder` decides the order they go *out* in, and \
         a reader that required one would refuse a peer that chose the other.",
    ),
    (
        "fastdds",
        "short-pid-range",
        "the same as `rti/short-pid-range`: no recorded Fast DDS row carries a short id above \
         `0x3F00`.",
    ),
    (
        "opendds",
        "length-codes",
        "the same as `cyclone/length-codes`: the code is in the stream.",
    ),
    (
        "opendds",
        "must-understand",
        "the same as `cyclone/must-understand`: the bit is in the stream.",
    ),
    (
        "omniorb",
        "fixed-digits",
        "the reader does consult it — `decode.rs::read_fixed` takes the declared width under \
         `FixedDigitCount::Declared` and finds the sign nibble under `ValuePrecision`. What \
         omniORB's record cannot ask is which: its `fixed` values have as many digits as they \
         declare, so the two readings consume the same octets and return the same value. \
         JacORB's rows do separate them, and `jacorb/fixed-digits` is not in this list.",
    ),
    (
        "opendds",
        "fixed-digits",
        "the same as `omniorb/fixed-digits`, for another reason. OpenDDS writes the declared \
         width (`OpenDDS::FaceTypes::Fixed_T<P,S>`, `(P + 2) / 2` octets) whatever the value, \
         and the sign nibble closes that width, so finding the sign nibble under \
         `ValuePrecision` consumes the octets the declared width does and returns the same \
         value. The writer does separate them, on a value with fewer digits than declared — \
         `fixed-basic`'s `-000000.001` under `fixed<9,3>`.",
    ),
];

/// The one allowance is Cyclone's and nobody else's/// The one allowance is Cyclone's and nobody else's, and it is exactly
/// the rows with a suppressed negative narrow case label in them.
///
/// Stated as its own assertion because the allowance is the one way a
/// row can pass this gate without being read back. A second system
/// appearing here is a finding whatever else is green: it would mean a
/// difference had been reclassified rather than closed.
#[test]
fn the_unreadable_rows_are_the_suppressed_branches_and_nothing_else() {
    let by_system: BTreeMap<&str, usize> = answers()
        .iter()
        .filter(|answer| answer.nothing_to_read > 0)
        .map(|answer| (answer.system, answer.nothing_to_read))
        .collect();
    assert_eq!(
        by_system,
        [("cyclone", 24)].into_iter().collect(),
        "the rows no reader can return the sample from are Cyclone's 24 and no others"
    );
}
