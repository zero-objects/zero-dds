// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Guards the REP-2007 / REP-2008 extension surface in
// `rmw_c/rmw_zerodds_ros2_ext.h` against drifting away from its implementation
// in `rmw_c/rmw_zerodds.c`.
//
// The C layer only compiles on a host with ROS 2 dev headers (the e2e runs
// `rmw_c/run_type_adaptation_test.sh` and `rmw_c/run_accel_loan_memory_test.sh`
// on the Linux test host), so a declared-but-undefined extension symbol would surface as a
// link failure on that host only, long after the change that caused it. These
// tests read both files and fail here instead.

#![allow(clippy::expect_used, clippy::panic, missing_docs)]

use std::path::PathBuf;

fn rmw_c_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("rmw_c")
}

fn read(name: &str) -> String {
    let path = rmw_c_dir().join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// Every `rmw_zerodds_*` function declared in the extension header must have a
/// definition in the C implementation. A missing definition links only when the
/// symbol is actually called, so the header is the only place it shows up.
#[test]
fn every_declared_extension_symbol_is_defined() {
    let header = read("rmw_zerodds_ros2_ext.h");
    let impl_src = read("rmw_zerodds.c");

    let mut declared: Vec<String> = Vec::new();
    for line in header.lines() {
        let line = line.trim();
        // Declarations end in `(` (multi-line) or contain `(` on one line, and
        // are never comments or macro definitions.
        if line.starts_with("//") || line.starts_with("#") || line.starts_with("*") {
            continue;
        }
        let Some(start) = line.find("rmw_zerodds_") else {
            continue;
        };
        let rest = &line[start..];
        let name: String = rest
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        // Only take the ones that are being *called* as functions here, i.e.
        // followed by `(`. Type names (`rmw_zerodds_type_adapter_t`) are not.
        if rest[name.len()..].trim_start().starts_with('(') && !declared.contains(&name) {
            declared.push(name);
        }
    }

    assert!(
        declared.len() >= 14,
        "expected the full REP-2007 + REP-2008 surface, found {}: {declared:?}",
        declared.len()
    );

    let missing: Vec<&String> = declared
        .iter()
        .filter(|name| {
            // A definition is `<name>(` at the start of a line, or preceded by
            // a return type on the same line, followed eventually by `{`.
            !impl_src
                .lines()
                .any(|l| l.contains(&format!("{name}(")) && !l.trim_start().starts_with("//"))
        })
        .collect();
    assert!(
        missing.is_empty(),
        "declared but never defined: {missing:?}"
    );
}

/// The REP-2008 kernel-level bitmask the header documents must be the one the
/// implementation returns, and the README vendor template must claim the same
/// levels. Three places, one fact.
#[test]
fn acceleration_kernel_levels_agree_across_header_impl_and_readme() {
    let header = read("rmw_zerodds_ros2_ext.h");
    let impl_src = read("rmw_zerodds.c");
    for macro_name in [
        "RMW_ZERODDS_ACCEL_LEVEL_1",
        "RMW_ZERODDS_ACCEL_LEVEL_2",
        "RMW_ZERODDS_ACCEL_LEVEL_3",
    ] {
        assert!(
            header.contains(&format!("#define {macro_name} ")),
            "{macro_name} not defined in the extension header"
        );
    }
    assert!(
        impl_src.contains("return RMW_ZERODDS_ACCEL_LEVEL_2 | RMW_ZERODDS_ACCEL_LEVEL_3;"),
        "rmw_zerodds_acceleration_kernel_levels() no longer returns levels 2 | 3"
    );
    let readme =
        std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("README.md"))
            .expect("read README.md");
    assert!(
        readme.contains("| Kernel levels served | 2 (rmw), 3 (the middleware itself) |"),
        "the REP-2008 vendor template in README.md claims different kernel levels"
    );
}

/// REP-2007 requires the adapted wire to be indistinguishable from the plain
/// one, which starts with the encapsulation header. The adapted publish path
/// must write the same CDR_LE header as the introspection publish path — a
/// divergence here would make adapted publishers silently unreadable by plain
/// subscriptions, which no unit test on this host can catch.
#[test]
fn adapted_publish_writes_the_same_encapsulation_header_as_plain_publish() {
    let impl_src = read("rmw_zerodds.c");
    let header_writes = impl_src
        .matches("wire[0] = 0x00; wire[1] = 0x01; wire[2] = 0x00; wire[3] = 0x00;")
        .count()
        + impl_src
            .matches("c.buf[0] = 0x00; c.buf[1] = 0x01; c.buf[2] = 0x00; c.buf[3] = 0x00;")
            .count();
    assert!(
        header_writes >= 2,
        "expected the adapted and the plain publish path to write the same \
         CDR_LE encapsulation, found {header_writes} such writes"
    );
}

/// `rmw_get_serialized_message_size` has to hand back a size a caller can
/// pre-allocate against, so the bound it returns must cover the same bytes
/// `rmw_serialize` writes: the four-byte encapsulation header plus every
/// member of the message. Whether the number is large enough for a given
/// message is a behavioural question that needs the rosidl introspection
/// type supports and therefore a ROS 2 host; what this host can hold is the
/// shape of the walk — that the header is added, that every member kind has
/// a branch, and that an unbounded string or sequence is capped rather than
/// counted as nothing.
#[test]
fn the_serialized_size_walk_covers_the_header_and_every_member_kind() {
    let impl_src = read("rmw_zerodds.c");

    assert!(
        impl_src.contains("*size = 4 + zerodds_cdr_max_msg(members);"),
        "the returned size no longer adds the four-byte encapsulation header"
    );

    // Every member kind rmw_serialize can write needs a branch in the bound,
    // otherwise the bound is short for exactly that kind.
    for branch in [
        "size_t ps = prim_size(mem->type_id_);",         // primitives
        "if (mem->type_id_ == 16)",                      // string
        "if (mem->type_id_ == 18)",                      // nested message
        "if (!mem->is_array_)",                          // scalar member
        "mem->array_size_ > 0 && !mem->is_upper_bound_", // fixed array
    ] {
        assert!(
            impl_src.contains(branch),
            "the size walk lost the branch {branch:?}"
        );
    }

    // An unbounded string or sequence has no static size; counting it as zero
    // would make the result an underestimate rather than a bound.
    for cap in ["#define ZERODDS_STR_CAP", "#define ZERODDS_SEQ_CAP"] {
        let line = impl_src
            .lines()
            .find(|l| l.trim_start().starts_with(cap))
            .unwrap_or_else(|| panic!("{cap} is gone"));
        let value: u32 = line
            .split_whitespace()
            .nth(2)
            .and_then(|t| t.trim_end_matches('u').parse().ok())
            .unwrap_or_else(|| panic!("{cap} has no numeric value: {line}"));
        assert!(value > 0, "{cap} must reserve room, not zero");
    }
}
