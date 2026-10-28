// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! **The four settings that close OpenDDS's `@mutable` rows.**
//!
//! One file and not four, because the four are one construct seen from
//! four sides — a `@mutable` aggregate's member headers — and because
//! each of them is two to eight recorded rows, so a file apiece would be
//! four copies of the same corpus reader.
//!
//! | axis | rows | what it decides |
//! |---|--:|---|
//! | [`MustUnderstandPolicy::DeclaredAndKey`] | 2 | whether `@key` implies the must-understand attribute |
//! | [`LengthCodePolicy::SizedOrNextInt`] | 6 | which EMHEADER length code a `@mutable` member gets |
//! | [`MutableUnionFraming::BranchesFromZero`] | 8 | how a `@mutable` union numbers its branches, and whether its XCDR1 list is terminated |
//! | [`ShortPidRange::AllFourteenBits`] | 2 | which member ids fit a short parameter header |
//!
//! # The one that is this tree's defect and not a peer's
//!
//! The other three are OpenDDS departing from a clause or reading one
//! the same way as everyone else; the first is the reverse.
//! §7.2.2.4.4.4.8, verbatim: "Key members shall never be optional, and
//! they shall always have their 'must understand' attribute set to
//! true." OpenDDS writes it; Cyclone, RTI and Fast DDS do not. So the
//! clause-by-clause dialect had a conformance defect and the vendor
//! dialects did not, and the fix is [`Dialect::Specification`] taking
//! the clause rather than a compatibility flag on top of a default that
//! was already right. [`the_clause_reaches_only_the_specification_dialect`]
//! is that split as a test.
//!
//! The three others are compatibility flags and no dialect selects any
//! of them: two parameters sharing member id 0 with no terminator, and a
//! member id inside §7.4.1.2.2's reserved range, are frames a conforming
//! reader cannot walk.
//!
//! # Mutation-checked
//!
//! Eight mutations were run and each row is what was **observed**, not
//! what was expected.
//!
//! | mutation | what goes red |
//! |---|---|
//! | `member_must_understand` dropping the `is_key` term | [`the_key_rows_are_reproduced`] |
//! | `MustUnderstandPolicy::writes_flag` answering `false` for `DeclaredAndKey` | [`the_key_rows_are_reproduced`] |
//! | `length_code`'s early return for a policy that reuses no leading word deleted | [`the_length_code_rows_are_reproduced`] |
//! | `MutableUnionFraming::first_branch_id` answering 1 for `BranchesFromZero` | [`the_union_rows_are_reproduced`], [`the_numbering_and_the_sentinel_are_separated_by_the_rows`], [`the_peers_octets_read_back_under_the_setting`] |
//! | `writes_the_sentinel` answering `true` for `BranchesFromZero` | the same three. The row-level separation is inside [`the_union_rows_are_reproduced`]: the four XCDR2 rows stay right under this mutation because version 2 has no sentinel to suppress, and the four XCDR1 rows go four octets long. That is what lets one axis carry two questions |
//! | `ShortPidRange::highest_short_id` answering `0x3f00` for `AllFourteenBits` | [`the_short_pid_rows_are_reproduced`] and [`the_peers_octets_read_back_under_the_setting`] — the second because the **reader** takes the same threshold, and a reader holding the clause's constant reports `@id(16383)` as a reserved parameter and drops the member |
//! | `must_understand` removed from `LayoutKey::offset_key` | [`the_planner_settings_are_in_the_offset_key`] **only**, and that is the finding rather than a gap: every other check here plans afresh through `encode`, and what the field protects is the plan **cache**. It had been missing for as long as the field existed |
//! | `union_framing` removed from `LayoutKey::offset_key` | the same one check, for the same reason |

#![allow(clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use zerodds_ast_layout::{
    Dialect, Encoding, Endianness, LayoutKey, LengthCodePolicy, MustUnderstandPolicy,
    MutableUnionFraming, PidFlagPolicy, Purpose, ShortPidRange,
};
use zerodds_wire_probe::canonical::{canonicalise, text};
use zerodds_wire_probe::decode::decode;
use zerodds_wire_probe::encode::{encode, hex, model_from_idl_in_dialect};
use zerodds_wire_probe::value::ProbeRequest;

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

// Every recorded `wrote` row of `vendor-corpus.tsv`, keyed by
// (case, system, version, order).
fn recorded() -> BTreeMap<(String, String, String, String), String> {
    let mut rows = BTreeMap::new();
    let path = repository_root().join("tests/interop-results/sources/vendor-corpus.tsv");
    let text = std::fs::read_to_string(path).expect("tests/interop-results/sources");
    for line in text.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 6 || f[4] != "wrote" {
            continue;
        }
        rows.insert(
            (
                f[0].to_string(),
                f[1].to_string(),
                f[2].to_string(),
                f[3].to_string(),
            ),
            f[5].to_string(),
        );
    }
    rows
}

fn encoding_of(version: &str) -> Encoding {
    if version == "xcdr1" {
        Encoding::PlainCdr1
    } else {
        Encoding::PlainCdr2
    }
}

fn endianness_of(endian: &str) -> Endianness {
    if endian == "be" {
        Endianness::Big
    } else {
        Endianness::Little
    }
}

// Everything one column of the sweep sets on top of a dialect.
#[derive(Clone, Copy, Default)]
struct Settings {
    length_codes: Option<LengthCodePolicy>,
    pid_flags: Option<PidFlagPolicy>,
    union_framing: Option<MutableUnionFraming>,
    short_pid_range: Option<ShortPidRange>,
}

// What this project writes for one corpus case under one dialect and
// one set of overrides.
fn written(
    case: &str,
    version: &str,
    endian: &str,
    dialect: Dialect,
    settings: Settings,
) -> Option<String> {
    let directory = repository_root().join("tests/interop-corpus").join(case);
    let idl = std::fs::read_to_string(directory.join("type.idl")).ok()?;
    let type_name = std::fs::read_to_string(directory.join("type")).ok()?;
    let values: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(directory.join("values.json")).ok()?).ok()?;
    let ast = model_from_idl_in_dialect(&idl, &format!("{case}/type.idl"), dialect).ok()?;
    let mut request = ProbeRequest::in_dialect(
        type_name.trim(),
        encoding_of(version),
        endianness_of(endian),
        dialect,
    );
    if let Some(policy) = settings.length_codes {
        request.length_codes = policy;
    }
    if let Some(policy) = settings.pid_flags {
        request.pid_flags = policy;
    }
    if let Some(policy) = settings.union_framing {
        request.union_framing = policy;
    }
    if let Some(policy) = settings.short_pid_range {
        request.short_pid_range = policy;
    }
    encode(&ast, &request, &values).ok().map(|b| hex(&b))
}

// One column of the sweep: a name for the failure message, the base
// dialect and the overrides.
struct Column {
    what: &'static str,
    dialect: Dialect,
    settings: Settings,
}

// Assert that every named row is reproduced by `column` and by none of
// the seven bare dialects, and return how many rows were compared.
//
// The second half is what makes each of these a measurement of the axis
// rather than of a preset: a row some dialect already writes says
// nothing about the setting.
fn reproduced_only_by(rows: &[(&str, &str, &str)], column: &Column) -> usize {
    let recorded = recorded();
    assert!(
        !recorded.is_empty(),
        "no recorded rows were read from tests/interop-results/sources — this check must not \
         pass by finding nothing"
    );
    let mut compared = 0usize;
    for (case, version, endian) in rows {
        let key = (
            (*case).to_string(),
            "opendds".to_string(),
            (*version).to_string(),
            (*endian).to_string(),
        );
        let Some(theirs) = recorded.get(&key) else {
            panic!("{case} opendds {version} {endian} is not in vendor-corpus.tsv");
        };
        let ours = written(case, version, endian, column.dialect, column.settings)
            .unwrap_or_else(|| panic!("{case} {version} {endian} produced nothing"));
        assert_eq!(
            &ours, theirs,
            "{case} opendds {version} {endian}: {} does not reproduce the recorded row",
            column.what
        );
        // `--dialect opendds` reproduces the row on its own, which is
        // the whole point of a dialect name: the settings this file
        // measures are OpenDDS's and are in OpenDDS's preset.
        // `tools/interop-results/tests/dialect_completeness.rs` holds
        // that over every recorded row; here it is held over the rows
        // this file names, so a setting that left the dialect fails
        // where the measurement for it lives.
        let named = written(case, version, endian, Dialect::OpenDds, Settings::default())
            .unwrap_or_else(|| {
                panic!("{case} {version} {endian}: --dialect opendds wrote nothing")
            });
        assert_eq!(
            &named, theirs,
            "{case} opendds {version} {endian}: --dialect opendds does not reproduce the \
             recorded row on its own"
        );
        // And no dialect that is **not** OpenDDS's reproduces it
        // without the override, so the row still decides the setting.
        // The column's own base is skipped where it carries the setting
        // itself, which is the `@key` case: the clause is the
        // specification dialect's answer and not an override on top of
        // it.
        // A dialect that writes this row exactly as the column's own base
        // does cannot be told apart from it by the row: OpenSplice answers
        // every axis as the specification does, so on a row the
        // specification writes bare it writes the same bytes.
        let base_bare = written(case, version, endian, column.dialect, Settings::default());
        for dialect in Dialect::ALL {
            if dialect == column.dialect || dialect == Dialect::OpenDds {
                continue;
            }
            let bare = written(case, version, endian, dialect, Settings::default());
            if bare == base_bare {
                continue;
            }
            assert_ne!(
                bare.as_deref(),
                Some(theirs.as_str()),
                "{case} opendds {version} {endian}: --dialect {} reproduces it with no \
                 override, so the row does not decide {}",
                dialect.as_str(),
                column.what
            );
        }
        compared += 1;
    }
    compared
}

// `@key` implies the must-understand attribute. §7.2.2.4.4.4.8.
#[test]
fn the_key_rows_are_reproduced() {
    let compared = reproduced_only_by(
        &[
            ("evolution-mutable-key", "xcdr2", "be"),
            ("evolution-mutable-key", "xcdr2", "le"),
        ],
        &Column {
            what: "--dialect specification",
            dialect: Dialect::Specification,
            settings: Settings::default(),
        },
    );
    assert_eq!(compared, 2);
}

// The clause reaches the specification dialect and no vendor dialect,
// and the two halves are asserted apart.
//
// The vendor half matters as much as the clause half: three
// implementations were measured *not* writing the bit, so a change that
// made `Dialect::Cyclone` conform would break three columns that are
// right today.
#[test]
fn the_clause_reaches_only_the_specification_dialect() {
    // The key members (§7.2.2.4.4.4.8) and, beyond what OpenDDS writes,
    // the discriminator of a `@mutable` union (§7.2.2.4.4.4.6).
    assert_eq!(
        Dialect::Specification.must_understand(),
        MustUnderstandPolicy::DeclaredKeyAndDiscriminator
    );
    assert_eq!(
        Dialect::OpenDds.must_understand(),
        MustUnderstandPolicy::DeclaredAndKey
    );
    for dialect in [Dialect::Cyclone, Dialect::Rti] {
        assert_eq!(dialect.must_understand(), MustUnderstandPolicy::Declared);
    }
    assert_eq!(
        Dialect::FastDds.must_understand(),
        MustUnderstandPolicy::Omitted
    );
    // And the octets say the same thing from the other side: under the
    // vendor dialects this project writes what those vendors wrote.
    let rows = recorded();
    for (system, dialect) in [
        ("cyclone", Dialect::Cyclone),
        ("rti", Dialect::Rti),
        ("fastdds", Dialect::FastDds),
    ] {
        for endian in ["be", "le"] {
            let key = (
                "evolution-mutable-key".to_string(),
                system.to_string(),
                "xcdr2".to_string(),
                endian.to_string(),
            );
            let theirs = rows.get(&key).expect("a recorded row");
            let ours = written(
                "evolution-mutable-key",
                "xcdr2",
                endian,
                dialect,
                Settings::default(),
            )
            .expect("the probe");
            assert_eq!(
                &ours, theirs,
                "evolution-mutable-key {system} xcdr2 {endian}: the clause was applied to a \
                 dialect whose implementation does not write it"
            );
        }
    }
}

// OpenDDS's EMHEADER length codes: short where the serialized length
// fits a code, LC4 otherwise, and never a reused leading word.
#[test]
fn the_length_code_rows_are_reproduced() {
    let compared = reproduced_only_by(
        &[
            ("evolution-mutable-middle", "xcdr2", "be"),
            ("evolution-mutable-middle", "xcdr2", "le"),
            ("evolution-mutable-defaults", "xcdr2", "be"),
            ("evolution-mutable-defaults", "xcdr2", "le"),
            ("struct-mutable-length-codes", "xcdr2", "be"),
            ("struct-mutable-length-codes", "xcdr2", "le"),
        ],
        &Column {
            what: "--dialect specification --length-codes sized-or-nextint",
            dialect: Dialect::Specification,
            settings: Settings {
                length_codes: Some(LengthCodePolicy::SizedOrNextInt),
                ..Settings::default()
            },
        },
    );
    assert_eq!(compared, 6);
}

// The new value is the fourth position on the axis and not a rename of
// one of the three.
//
// Each of the four answers the two questions of
// [`LengthCodePolicy`]'s documentation differently, and no two share a
// pair. A value added by copying another's answers would be a column
// measuring nothing new, which is what this asserts against.
#[test]
fn the_four_positions_are_four_pairs() {
    let mut seen = Vec::new();
    for policy in LengthCodePolicy::ALL {
        let pair = (
            policy.reuses_a_leading_word(),
            policy.reuses_nested_delimiter(),
            policy.short_code_where_it_fits(),
        );
        assert!(
            !seen.contains(&pair),
            "{} answers exactly as another value does",
            policy.as_str()
        );
        seen.push(pair);
    }
    assert_eq!(seen.len(), 5);
    // The two that never spend a code on a word the member carries.
    assert!(!LengthCodePolicy::UniversalNextInt.reuses_a_leading_word());
    assert!(!LengthCodePolicy::SizedOrNextInt.reuses_a_leading_word());
    // And the one thing that separates those two from each other.
    assert!(!LengthCodePolicy::UniversalNextInt.short_code_where_it_fits());
    assert!(LengthCodePolicy::SizedOrNextInt.short_code_where_it_fits());
}

// OpenDDS's `@mutable` union: branches numbered from 0, no sentinel.
#[test]
fn the_union_rows_are_reproduced() {
    let compared = reproduced_only_by(
        &[
            ("union-mutable", "xcdr1", "be"),
            ("union-mutable", "xcdr1", "le"),
            ("union-mutable", "xcdr2", "be"),
            ("union-mutable", "xcdr2", "le"),
            ("evolution-union-unselected", "xcdr1", "be"),
            ("evolution-union-unselected", "xcdr1", "le"),
            ("evolution-union-unselected", "xcdr2", "be"),
            ("evolution-union-unselected", "xcdr2", "le"),
        ],
        &Column {
            what: "--dialect fastdds --pid-flags impl-extension-members --union-framing \
                   branches-from-zero",
            dialect: Dialect::FastDds,
            settings: Settings {
                pid_flags: Some(PidFlagPolicy::ImplExtensionOnMembers),
                union_framing: Some(MutableUnionFraming::BranchesFromZero),
                ..Settings::default()
            },
        },
    );
    assert_eq!(compared, 8);
}

// The two halves of the union axis are told apart by the rows, which is
// what a merged axis has to be able to say.
//
// Version 2 has no sentinel to suppress, so the four XCDR2 rows carry
// the **numbering** alone: they are reproduced with the sentinel half
// of the setting made irrelevant. The four XCDR1 rows carry both. A
// mutation of either half is therefore red on one of the two groups,
// and the merge costs no coverage.
#[test]
fn the_numbering_and_the_sentinel_are_separated_by_the_rows() {
    assert_eq!(MutableUnionFraming::Specification.first_branch_id(), 1);
    assert_eq!(MutableUnionFraming::BranchesFromZero.first_branch_id(), 0);
    assert!(MutableUnionFraming::Specification.writes_the_sentinel());
    assert!(!MutableUnionFraming::BranchesFromZero.writes_the_sentinel());

    // The XCDR2 rows: the sentinel is not in them at all, so the whole
    // of the difference from this project's default is the numbering.
    let rows = recorded();
    for endian in ["be", "le"] {
        let key = (
            "union-mutable".to_string(),
            "opendds".to_string(),
            "xcdr2".to_string(),
            endian.to_string(),
        );
        let theirs = rows.get(&key).expect("a recorded row");
        assert!(
            !theirs.contains("3f02") && !theirs.contains("023f"),
            "union-mutable xcdr2 {endian} carries a sentinel after all: {theirs}"
        );
    }
    // The XCDR1 rows: this project's default writes four octets more,
    // and they are the sentinel.
    for endian in ["be", "le"] {
        let key = (
            "union-mutable".to_string(),
            "opendds".to_string(),
            "xcdr1".to_string(),
            endian.to_string(),
        );
        let theirs = rows.get(&key).expect("a recorded row");
        let numbering_only = written(
            "union-mutable",
            "xcdr1",
            endian,
            Dialect::FastDds,
            Settings {
                pid_flags: Some(PidFlagPolicy::ImplExtensionOnMembers),
                ..Settings::default()
            },
        )
        .expect("the probe");
        assert_eq!(
            numbering_only.len(),
            theirs.len() + 8,
            "union-mutable xcdr1 {endian}: the sentinel is not the four octets of difference"
        );
    }
}

// A member id inside §7.4.1.2.2's reserved range, in the short header.
#[test]
fn the_short_pid_rows_are_reproduced() {
    let compared = reproduced_only_by(
        &[
            ("annotation-id-sparse", "xcdr1", "be"),
            ("annotation-id-sparse", "xcdr1", "le"),
        ],
        &Column {
            what: "--dialect rti --pid-flags impl-extension-members --short-pid-range \
                   all-fourteen-bits",
            dialect: Dialect::Rti,
            settings: Settings {
                pid_flags: Some(PidFlagPolicy::ImplExtensionOnMembers),
                short_pid_range: Some(ShortPidRange::AllFourteenBits),
                ..Settings::default()
            },
        },
    );
    assert_eq!(compared, 2);
    // The threshold itself, so that the two values differ by the
    // reserved range and by nothing else.
    assert_eq!(ShortPidRange::UpToReserved.highest_short_id(), 0x3f00);
    assert_eq!(ShortPidRange::AllFourteenBits.highest_short_id(), 0x3fff);
}

// Every setting that reaches a plan is in [`LayoutKey::offset_key`],
// and every setting that does not is out of it.
//
// The plan cache keys on `offset_key`, so a field that reaches
// [`zerodds_ast_layout::TypeLayout`] and is missing here is a **silent
// wrong plan**: one caller's entry served to another whose answer
// differs, with nothing at run time to say so.
//
// `must_understand` was missing for as long as the field existed. It is
// not a length — no offset moves with it — and that is exactly why it
// was overlooked: the rule is not "does it move an offset" but "does it
// reach the plan", and a `MemberHeader` carries the attribute.
#[test]
fn the_planner_settings_are_in_the_offset_key() {
    // Cyclone and not the specification dialect, so that every value
    // asserted below is a *change* from the base: the specification
    // dialect already carries `DeclaredAndKey`, and comparing a key with
    // itself would pass whatever the cache key held.
    let base = LayoutKey::in_dialect(
        Dialect::Cyclone,
        Encoding::PlainCdr1,
        Endianness::Big,
        Purpose::Data,
    );
    // In: each of these changes something the plan carries.
    for changed in [
        base.with_length_codes(LengthCodePolicy::SizedOrNextInt),
        base.with_must_understand(MustUnderstandPolicy::Omitted),
        base.with_must_understand(MustUnderstandPolicy::DeclaredAndKey),
        base.with_union_framing(MutableUnionFraming::BranchesFromZero),
        base.with_short_pid_range(ShortPidRange::AllFourteenBits),
    ] {
        assert_ne!(
            base.offset_key(),
            changed.offset_key(),
            "a setting the plan carries is missing from the cache key"
        );
    }
    // Out: each of these is answered by a writer or a reader and never
    // reaches the plan, so two keys differing in one share an entry.
    for changed in [
        base.with_pid_flags(PidFlagPolicy::ImplExtensionOnMembers),
        base.with_map_entries(zerodds_ast_layout::MapEntryOrder::AscendingKey),
        base.with_enumerator_value(zerodds_ast_layout::EnumeratorValue::Ordinal),
        base.with_union_label(zerodds_ast_layout::UnionLabelMatch::NegativeLabelUnmatchedBelow32),
    ] {
        assert_eq!(
            base.offset_key(),
            changed.offset_key(),
            "a setting no plan carries is in the cache key, which costs an entry per value"
        );
    }
}

// The peers' octets read back as each case's own sample under the
// setting that reproduces them.
//
// Two of the four causes were recorded as reaching no reader — the
// length codes and the `@key` bit both decode to the sample without the
// setting, because a reader takes whatever code or flag it finds. That
// is asserted here rather than left as a sentence, and the other two
// are asserted the other way: a union numbered from zero and a member
// id in the reserved range are frames this project's reader has to be
// *told* about.
#[test]
fn the_peers_octets_read_back_under_the_setting() {
    let rows = recorded();
    let mut checked = 0usize;
    for (case, version, endian, dialect, settings) in [
        (
            "evolution-mutable-key",
            "xcdr2",
            "be",
            Dialect::Specification,
            Settings::default(),
        ),
        (
            "struct-mutable-length-codes",
            "xcdr2",
            "be",
            Dialect::Specification,
            Settings {
                length_codes: Some(LengthCodePolicy::SizedOrNextInt),
                ..Settings::default()
            },
        ),
        (
            "union-mutable",
            "xcdr1",
            "be",
            Dialect::FastDds,
            Settings {
                pid_flags: Some(PidFlagPolicy::ImplExtensionOnMembers),
                union_framing: Some(MutableUnionFraming::BranchesFromZero),
                ..Settings::default()
            },
        ),
        (
            "annotation-id-sparse",
            "xcdr1",
            "be",
            Dialect::Rti,
            Settings {
                pid_flags: Some(PidFlagPolicy::ImplExtensionOnMembers),
                short_pid_range: Some(ShortPidRange::AllFourteenBits),
                ..Settings::default()
            },
        ),
    ] {
        let key = (
            case.to_string(),
            "opendds".to_string(),
            version.to_string(),
            endian.to_string(),
        );
        let theirs = rows.get(&key).expect("a recorded row");
        let directory = repository_root().join("tests/interop-corpus").join(case);
        let idl = std::fs::read_to_string(directory.join("type.idl")).expect("the IDL");
        let type_name = std::fs::read_to_string(directory.join("type")).expect("the type");
        let values: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(directory.join("values.json")).expect("the sample"),
        )
        .expect("the sample as JSON");
        let ast =
            model_from_idl_in_dialect(&idl, &format!("{case}/type.idl"), dialect).expect("model");
        let root = root_of(&ast, type_name.trim());
        let sample =
            text(&canonicalise(&ast, root, &values, type_name.trim()).expect("canonical form"));

        let mut request = ProbeRequest::in_dialect(
            type_name.trim(),
            encoding_of(version),
            endianness_of(endian),
            dialect,
        );
        if let Some(policy) = settings.length_codes {
            request.length_codes = policy;
        }
        if let Some(policy) = settings.pid_flags {
            request.pid_flags = policy;
        }
        if let Some(policy) = settings.union_framing {
            request.union_framing = policy;
        }
        if let Some(policy) = settings.short_pid_range {
            request.short_pid_range = policy;
        }
        let read = decode(&ast, &request, &unhex(theirs)).map(|value| text(&value));
        assert_eq!(
            read.as_deref().ok(),
            Some(sample.as_str()),
            "{case} opendds {version} {endian}: the peer's octets do not read back as the \
             sample under the setting that writes them"
        );
        checked += 1;
    }
    assert_eq!(checked, 4);
}

fn root_of(ast: &zerodds_ast::Ast, type_name: &str) -> zerodds_ast_core::Id<zerodds_ast::TypeNode> {
    ast.find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(
        type_name
            .split("::")
            .filter(|segment| !segment.is_empty())
            .map(std::string::ToString::to_string)
            .collect(),
    ))
    .ok()
    .and_then(zerodds_ast::AnyId::as_type)
    .expect("the case's type")
}

fn unhex(text: &str) -> Vec<u8> {
    (0..text.len() / 2)
        .map(|index| {
            u8::from_str_radix(&text[index * 2..index * 2 + 2], 16).expect("a hex payload")
        })
        .collect()
}

// The axis-reaches-every-wrapper check lives once, in
// `enumerator_value.rs::every_axis_reaches_every_wrapper`.
//
// It stood here too, and in three sibling files, as five copies of one
// assertion with five hard-coded surface paths each. When the five
// generators became renderers on `tools/wireprobe/wireprobe-zerodds-gen`
// the flag left the C column's `lib.rs` for the stem, and every copy
// then named a file that no longer held it. Reading the wrappers to fix
// the first copy showed the larger gap the copies had all missed:
// `-cpp`, `-csharp`, `-java` and `-typescript` forwarded **none** of the
// sixteen axes. One check over all sixteen axes and all nine surfaces
// replaced the five.

// The four new configurations are named the same in all four lists.
#[test]
fn the_configurations_are_named_the_same_everywhere() {
    const LISTS: [&str; 4] = [
        "crates/wire-probe/tests/three_column_gate.rs",
        "tools/interop-results/src/rust_column.rs",
        "crates/wire-probe/src/matrix.rs",
        "tools/wireprobe/wireprobe-zerodds-c-gen/src/bin/wireprobe-c-matrix.rs",
    ];
    for path in LISTS {
        let text = std::fs::read_to_string(repository_root().join(path))
            .unwrap_or_else(|error| panic!("{path}: {error}"));
        // One name where there were three. The three settings this file
        // measures — the length code, the `@mutable` union framing and
        // the short-pid range — are OpenDDS's and are in
        // `Dialect::OpenDds`, so the column that drives them is the
        // product's own name.
        assert!(
            text.contains("opendds"),
            "{path} does not drive `--dialect opendds`"
        );
    }
    // And the dialect selects all three, which is the claim the name
    // stands for.
    assert_eq!(
        Dialect::OpenDds.length_codes(),
        zerodds_ast_layout::LengthCodePolicy::SizedOrNextInt
    );
    assert_eq!(
        Dialect::OpenDds.union_framing(),
        zerodds_ast_layout::MutableUnionFraming::BranchesFromZero
    );
    assert_eq!(
        Dialect::OpenDds.short_pid_range(),
        zerodds_ast_layout::ShortPidRange::AllFourteenBits
    );
}
