// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The two dialect answers that are settled before any layout plan
//! exists.
//!
//! Almost everything a dialect decides reaches the octets through the
//! `LayoutKey` the generator builds from `--dialect`. Two answers do
//! not, because member ids are fixed in `zerodds_idl_lower::complete`,
//! which runs before there is a plan to key:
//!
//!   * whether a `@hashid` member advances the sequential counter the
//!     next unannotated member is numbered from — DDS-XTypes 1.3
//!     §7.3.1.2.1.1 states the rule twice and not the same way, and RTI
//!     takes the reading that skips the hashed id;
//!   * which spelling of an escaped identifier the hash reads — IDL 4.2
//!     §7.2.3.2 makes the underscore lexical, so `_long` hashes as
//!     `long`, and RTI hashes the spelling as written.
//!
//! A generator that lowers with `Options::default()` compiles cleanly,
//! renders a chunk `lua` accepts and runs — and writes the
//! specification's member ids into an RTI cell. Nothing but the octets
//! shows it, so the numbers are pinned here.
//!
//! The measured values (rtiddsgen 7.7.0, and
//! `matrix/zerodds-rust/{specification,rti}-xcdr2-be`):
//!
//! | member                     | specification | rti      |
//! |----------------------------|---------------|----------|
//! | `plain` after two @hashid  | 13269913      | 0        |
//! | `_long` (escaped keyword)  | 56906255      | 79659193 |
//! | `plain` after three @hashid| 95841972      | 0        |
// The workspace's no-panic contract is about the library a user links.
// A test says what it found by stopping, and the same allow is on
// `crates/lang-rules/tests/lua_codec.rs` for the same reason.
#![allow(clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

use zerodds_ast_layout::{Dialect, Encoding, Endianness};
use zerodds_wireprobe_lua_gen::{Args, source};

/// The chunk the generator writes for one source under one dialect.
///
/// The write direction, because that is the one that puts a member id in
/// the descriptor and in the header the codec writes.
fn chunk(name: &str, idl: &str, values: &str, dialect: Dialect) -> String {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    std::fs::create_dir_all(&dir).expect("the test's own directory");
    let path = dir.join(format!("{dialect:?}.idl"));
    std::fs::write(&path, idl).expect("the IDL the generator reads");
    let values_path = dir.join(format!("{dialect:?}.jsn"));
    std::fs::write(&values_path, values).expect("the sample the generator fills");
    let args = Args {
        idl: path,
        type_name: "Sample".to_string(),
        values: Some(values_path),
        input: None,
        values_out: None,
        encoding: Encoding::PlainCdr2,
        endianness: Endianness::Big,
        encapsulation: false,
        dialect,
        out_dir: dir,
    };
    source(&args).expect("the generator accepts the case")
}

/// The descriptor line for one member, which is where the id is written
/// out in full.
fn member_line<'a>(chunk: &'a str, member: &str) -> &'a str {
    let needle = format!("name = \"{member}\"");
    chunk
        .lines()
        .find(|line| line.contains(&needle))
        .unwrap_or_else(|| panic!("no descriptor entry for `{member}` in:\n{chunk}"))
}

const HASHID: &str = "\
@mutable struct Sample {
    @hashid long alpha;
    @hashid long beta;
    long plain;
};
";

const HASHID_VALUES: &str = r#"{"alpha": -1234567, "beta": 2147483647, "plain": -2}"#;

const ESCAPED: &str = "\
@mutable struct Sample {
    @hashid long _long;
    @hashid double _double;
    @hashid short _switch;
    long plain;
};
";

const ESCAPED_VALUES: &str =
    r#"{"long": -1234567, "double": 3.141592653589793, "switch": -2, "plain": -1}"#;

#[test]
fn a_hashed_id_advances_the_counter_under_the_specification() {
    let chunk = chunk("hashid", HASHID, HASHID_VALUES, Dialect::Specification);
    assert!(
        member_line(&chunk, "plain").contains("id = 13269913"),
        "the member after `@hashid beta` is numbered from it: {}",
        member_line(&chunk, "plain")
    );
}

#[test]
fn a_hashed_id_is_skipped_under_rti() {
    let chunk = chunk("hashid", HASHID, HASHID_VALUES, Dialect::Rti);
    assert!(
        member_line(&chunk, "plain").contains("id = 0"),
        "RTI numbers from the last declared id, and there is none: {}",
        member_line(&chunk, "plain")
    );
}

#[test]
fn an_escaped_keyword_hashes_without_its_underscore_under_the_specification() {
    let chunk = chunk("escaped", ESCAPED, ESCAPED_VALUES, Dialect::Specification);
    assert!(
        member_line(&chunk, "long").contains("id = 56906255"),
        "`_long` hashes as `long`: {}",
        member_line(&chunk, "long")
    );
    assert!(
        member_line(&chunk, "plain").contains("id = 95841972"),
        "and the unannotated member follows it: {}",
        member_line(&chunk, "plain")
    );
}

#[test]
fn an_escaped_keyword_hashes_as_written_under_rti() {
    let chunk = chunk("escaped", ESCAPED, ESCAPED_VALUES, Dialect::Rti);
    assert!(
        member_line(&chunk, "long").contains("id = 79659193"),
        "RTI hashes `_long`: {}",
        member_line(&chunk, "long")
    );
    assert!(
        member_line(&chunk, "plain").contains("id = 0"),
        "and skips the hashed ids when it numbers the next one: {}",
        member_line(&chunk, "plain")
    );
}

#[test]
fn the_dialect_that_shares_the_specifications_reading_writes_its_ids() {
    // Cyclone and Fast DDS take the second of the two sentences, so
    // their cells have to carry the specification's numbers — the
    // measurement the default rests on.
    for dialect in [Dialect::Cyclone, Dialect::FastDds] {
        let chunk = chunk("hashid", HASHID, HASHID_VALUES, dialect);
        assert!(
            member_line(&chunk, "plain").contains("id = 13269913"),
            "{dialect:?}: {}",
            member_line(&chunk, "plain")
        );
    }
}
