#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# regenerate.sh — the result tree and the two tables, from what is
# already recorded.
#
#   tools/interop-results/regenerate.sh
#
# ## What runs where
#
# Two stages, and the split is the whole point.
#
# **The sources.** One TSV per system under
# `tests/interop-results/sources/`, each the verbatim output of the
# harness that produced it. Re-deriving one needs the system: Cyclone,
# Fast DDS and OpenDDS are in the pinned `ci-interop` image, RTI is
# licensed and machine-bound, the three ORBs are hand-built and the two
# ROS type supports need a ROS installation. That is
# `tools/interop-results/record.sh`, which names every system it could
# not reach with the row count it did not produce.
#
# `sources/warnings.tsv` is one of those TSVs and is produced the same
# way, by `warning-sweep.sh`, which runs each front end and stops there.
# It holds what each front end *said* about each case, which is a
# different measurement from what it did with it and is the only one
# that can see an annotation that was parsed and dropped.
#
# `sources/toolchain-warnings.tsv` is the other half of that: what each
# system's **compiler** said, which the front-end sweep cannot reach
# because it never compiles. The byte sweeps write it, through
# `tools/wireprobe/diagnostics.sh`, and it is a file of its own because
# `warning-sweep.sh` writes the one above it whole. Same six columns,
# and `warnings::read_both` reads them into one map, so a compiler is
# one more `program` beside `idlc` and `tao_idl`.
#
# **The tree and the tables.** This file: rendered from those TSVs and
# from a fresh in-process run of the Rust column. It needs a Rust
# toolchain and nothing else, which is why a clone can regenerate and
# check the whole tree without any vendor at all. `WARNINGS.md` is
# rendered here too: the counts are generated so a figure in it and a
# line in `sources/warnings.tsv` cannot disagree, and the meanings are
# written by hand in `src/warnings.rs` because a meaning cannot be
# generated.
#
# ## What it does not do
#
# It does not decide anything. Every wire question is answered by the
# system whose column it is, and every hole is a word this tool prints
# rather than a row it leaves out.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
SOURCES="$ROOT/tests/interop-results/sources"

if [ "${1:-}" = "--help" ] || [ "${1:-}" = "-h" ]; then
    sed -n '5,28p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
    exit 0
fi
if [ "${1:-}" = "--sweep" ]; then
    echo "regenerate.sh: --sweep moved to tools/interop-results/record.sh" >&2
    echo "  recording needs the vendor toolchains; this needs only a Rust one" >&2
    exit 2
fi

mkdir -p "$SOURCES"

# The C and Python columns come out of the one gate that measures them,
# so that this tool and `crates/wire-probe/tests/three_column_gate.rs`
# cannot report two different C columns. Without a full-tier run in this
# checkout the two columns say `not measured` in every cell rather than
# borrowing the Rust column's answer.
cargo run -q --release -p zerodds-interop-results --bin interop-results -- --import-columns

# The machine out of the record, before anything is rendered from it.
#
# A probe names the file it was given and is given an absolute path, so
# a refusal recorded on one host carried that host's checkout into
# `sources/` and from there into every table and into `matrix.html`.
# `wireprobe-c-matrix` made it worse than a host difference: it names
# its build directory after its own process id, so two runs on **one**
# host wrote two different rows, and `verify-record.sh` diffs a
# re-record against `sources/` byte for byte.
#
# Here and not in the sweeps because this is the one place every sweep's
# output passes through — `record.sh` ends in this file — so a sweep
# that is added later cannot forget it.
# `tests/committed_tree.rs::the_record_carries_no_machine` is the gate.
cargo run -q --release -p zerodds-interop-results --bin interop-results -- --normalise

cargo run -q --release -p zerodds-interop-results --bin interop-results -- --regenerate
