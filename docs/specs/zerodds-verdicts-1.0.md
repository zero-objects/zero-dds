# Verdicts: one word per question, produced once

A verdict is what this tree says about one **(case, column, question)**.
There are two questions and they are not the same one:

- **structure** — what did that toolchain's front end make of this IDL?
- **bytes** — what did it put on the wire?

This document fixes the vocabulary both questions answer in, where each
verdict is produced, and the rules that keep a verdict from being an
opinion. It exists because the numbers this tree produced about nine
toolchains were wrong three times in a row, each time in a different
way, and each time the error was a classification and not a
measurement.

## The three failures this is built against

**A denominator nobody declared.** The interop corpus is 376 cases of
XTypes constructs. Measuring CORBA and ROS against all of them and
counting what they would not take produced "TAO refused 127 cases",
which is not a statement about TAO — CORBA 3.3 does not define most of
those constructs. A column's denominator is the corpus **intersected
with its own specification**, and nothing may report a ratio without
naming which.

**Two authorities for one specification.** `zerodds_ast::profile`
answers "is this model inside CORBA 3.3", and
`wireprobe-corba-idl` answers the same question by trying to write the
type as CORBA IDL. They disagreed: the profile admitted a structure
with a base, an enumeration with a `@bit_bound` and an enumerator with
an explicit `@value`, all of which the rewriter refuses with a clause.
The profile was the one reports used, so the disagreement never showed
up as a failing test — it showed up as a wrong table.

**One word for several answers.** "Refused" covered a specification
that has no such construct, a generator that produces code its own
toolchain cannot build, a value that does not fit the type, and a
harness of ours that has not implemented something. Those are four
different findings and three of them are not about the toolchain.

## The vocabulary

Every verdict is one of these. `tools/interop-results/src/ledger.rs`
holds them for the byte question and is the definition; the structure
question uses the same names for the same meanings.

### Not a measurement of the toolchain

| verdict | means | produced by |
|---|---|---|
| `out-of-profile` | the case is not inside this column's specification | `Profile::validate` |
| `no-form` | the target has no form for the construct, in its own words | exit 3 |
| `not-attempted` | no run of this column covered this case | the collection |
| `no-probe` | the toolchain is installed and nothing here drives it | the system table |
| `harness-gap` | the construct is the target's and a tool of **ours** has not written it | exit 6 |
| `not-measured` | expressible, a harness here produces it, no run is recorded — carries the command that would | the assembler |
| `unwritten` | the read direction had nothing to read because the write direction produced nothing; says nothing about the reader | the assembler |

`no-form` had no row of its own for a long time, although the exit
table, the evidence rule and the section below all named it as the word
for exit 3. A name every part of a document uses and no part defines is
an invitation, and it was taken: `tools/wireprobe/wireprobe-endpoint-corpus`
and `tools/interop-results/endpoint-record.sh` grew `out-of-surface`,
which appears in no row of this file, and six hundred cells were
reported under it. Nothing could see it —
`crates/wire-probe/tests/vocabulary_is_documented.rs` reads the
`Outcome` enum against this file, and `out-of-surface` was a string in a
shell script.

### A measurement of the toolchain

| verdict | means | produced by |
|---|---|---|
| `carried` | its front end produced sources | exit 0 |
| `bytes` | it produced octets | the probe |
| `same` / `different` | the read direction returned the sample, or did not | `wireprobe-compare` |
| `refused` | it was asked and said no, in its own words | a non-zero exit with its message |
| `generator-defect` | its front end accepted the case and its own generated code does not build | exit 7, with the emitted line |
| `mapping-divergence` | it carries the case and a documented mapping rule makes the octets differ | exit 8, with the clause |
| `timed-out` | it did not come back inside the clock | exit 124 |

### Kept from the byte side, because encodings are not cases

`version-alias`, `no-version-2`, `one-order-only`: a row that is
another row's run, a system with no encoding version 2, a system that
writes one byte order. They are properties of the *slot*, not of the
case.

## The two verdicts this vocabulary was missing

Both were `no-form` and both are measurements of a toolchain rather
than of a specification.

**`generator-defect`.** `rosidl`'s IDL takes `wchar[3]` and generates
without a word. What `rosidl_typesupport_fastrtps_c` 2.2.2 then emits
contains `callbacks->cdr_serialize(static_cast<wchar_t *>(&array_ptr[i]),
cdr)`, where `callbacks` is declared nowhere in that function and the
cast is from `uint16_t *`. The file does not compile. Recording that as
"ROS has no form for a collection of wchar" states the opposite of what
was measured: ROS has the form, and its generator is broken for it.

**`mapping-divergence`.** `omniidl -bcxx` and `tao_idl` both take
`long double` and generate. CORBA's C++ mapping makes it the platform's
x87 extended precision and not §9.3.1.3's binary128, so the octets
differ from the reference. That is a finding about the mapping, and
recording it as "CORBA has no long double" hides it — CORBA 3.3 has
one.

## The rules

**A verdict is produced by something that ran and said so.** Never by
the absence of a file. A missing directory is `not-attempted` until a
run says otherwise; it is never `refused` and never `carried`.

**A verdict names its evidence.** The exit code, the message, and the
program that produced it. `no-form` with no clause is not a verdict.

**One authority per question.** Whether a case is inside a
specification is `Profile::validate`'s answer and nobody else's.
`crates/wire-probe/tests/profile_matches_the_rewriter.rs` holds the
profile and the rewriters together over the whole corpus and fails when
they drift; where they answer *different* questions, the exception is
listed there by its reason.

**A ratio names its denominator.** "177 of 181, CORBA 3.3" and never
"177 of 376".

**"Carried" is not "correct".** A front end that emits a type for every
case has a verdict of `carried` for every case and says nothing about
its octets. hddsgen accepts all 376 corpus cases and its generated
`Mid::encode_xcdr1_le` does not write the base's members at all —
`carried` and a byte verdict of `different` are the same toolchain on
the same case.

**A number from a run that is known to be broken is not reported.** It
is discarded and the run is repeated.

## The exit codes a probe answers in

The classification is structural and never read out of a message:

| code | verdict |
|---|---|
| 0 | it carried the case |
| 1 | `refused` — the specification does not admit the model, in the planner's own words |
| 2 | the command line — **not a measurement**; the collection stops |
| 3 | `no-form` |
| 6 | `harness-gap` |
| 7 | `generator-defect` |
| 8 | `mapping-divergence` |
| 9 | `toolchain-absent` — the host has not got the column's compiler; **not a measurement**, the collection stops |
| 124 | `timed-out`, from the collection's clock |
| other | `refused`, with the probe's first line |

`wireprobe-fill`, `wireprobe-corba-idl` and `wireprobe-ros-package`
produce them; `tools/wireprobe/collect-generated` translates them, and
`crates/wire-probe/tests/profile_matches_the_rewriter.rs` compares only
exit 3 against the profile, because that is the one code that claims a
specification does not have something.

### What 1 and 2 said before 2026-08-26

One code, three readings in one tree. This table said 1 was the command
line; `collect-generated` names 0, 3, 6, 7, 8 and 124 and reads
everything else as `refused`, so 1 arrived there as a refusal;
`wireprobe-zerodds-zig-gen` answered 1 *for* a refusal; and the five
columns that share `zerodds-wireprobe-gen` answered 1 for a usage error.
A caller that typed the command wrong was recorded as a column refusing
the case, and nothing downstream could tell the two apart.

The reading that survived is the collection's, because it is the one
that writes the matrix. Usage moves to 2 — where the ten own-column
generators already had it — and the collection stops on 2 instead of
recording it, because a probe that was called wrong measured nothing.

The same measurement moved the planner's refusal onto 1. Over
`recursive-array`, whose cycle IDL 4.2 §7.4.1.4.4.4.4 does not admit,
ada and d answered 3 (their own languages have no form for it, which is
a second true statement), zig answered 1, and twelve columns answered
6 — `harness-gap`, "a tool of **ours** has not written it", which is a
claim about this tree and not about the clause that refused. The clause
now reaches the generators as `EncodeError::SpecRefuses` rather than as
a string, so no generator classifies it from the message.

### Why 9 exists

A host without a column's compiler was also exit 6, and every driver
said so on purpose: "a missing one is a gap in the harness (6), not in
the case". The trouble is what it does to a run. A column whose compiler
is absent does not stop — it writes a cell for every case, each reading
`refused`, and nothing separates those from the refusals that were
measured.

Four columns went that way before it was noticed. `zerodds-nim` wrote
17 760 cells saying `nim is not on the path`; `zerodds-swift` the same
number for `swiftc`; `zerodds-julia` and `zerodds-zig` the same. Each
surfaced by accident, on a cell someone happened to open, days apart.
Two of them had passed a check that ran in an interactive shell — the
service's `PATH` is shorter, and `/root/.juliaup/bin` is not in it.

A run that cannot measure a column should stop and say which compiler is
missing, not fill the matrix with rows about the host.

## Reruns

Two stages, and the split is what makes a clone able to check the tree.

**Record** needs the toolchains: `tools/interop-results/record.sh` for
octets, `tools/wireprobe/collect-generated` for sources. Each names
every system it could not reach rather than leaving the row out, writes
its verdicts with their evidence, and records the toolchain versions it
actually found through `toolchain-provenance.sh`.

**Regenerate** needs a Rust toolchain and nothing else:
`tools/interop-results/regenerate.sh` renders the tree and the tables
from what is recorded, and decides nothing.

A rerun that produces different verdicts from the same inputs is a
finding in itself: `verify-record.sh` answers "did anything change"
with an exit code, and `ledger-diff.py` answers "what changed" by kind.
