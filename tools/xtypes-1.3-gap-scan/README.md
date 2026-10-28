# `xtypes-1.3-gap-scan`

What OMG DDS-XTypes 1.3 defines, measured against this tree — and
recorded, so that the size of the gap cannot change without anyone
noticing.

```
tools/xtypes-1.3-gap-scan/roster.py           pull the denominator, print the checks
tools/xtypes-1.3-gap-scan/roster.py --check   check only, write nothing
tools/xtypes-1.3-gap-scan/scan.sh             measure and compare
tools/xtypes-1.3-gap-scan/scan.sh --record    rewrite expected.tsv
tools/xtypes-1.3-gap-scan/scan.sh --report    print the full measurement
tools/xtypes-1.3-gap-scan/scan.sh --explain   with the first diagnostic per probe
PROBE=/path/to/xtypes-gap-probe tools/xtypes-1.3-gap-scan/scan.sh
```

The written-out result is `docs/spec-coverage/dds-xtypes-1.3.gaps.md` and
its `.en.md` twin. This directory holds the evidence those pages cite.

## Why this exists

The coverage pages under `docs/spec-coverage/` are positive lists. They
name what was looked at and how far it got, and their content is derived
from the tree. That makes them unable to answer the one question a
release has to answer: what does the specification define that nobody
here ever fed in. A page with 4,600 `done` entries and no denominator
proves coverage of the examined, not completeness.

`tools/idl-4.2-gap-scan` did this for OMG IDL 4.2. This is the second of
the series, and the denominator is the part that does not carry over.

## The denominator has no Annex A

IDL 4.2 has one: a consolidated, numbered grammar that yields a complete
list with a hole check (`range(1, 228)` against the parsed numbers).
XTypes 1.3 has no grammar at all. Its inventory is a set of normative
tables and IDL blocks, and `roster.py` extracts seven of them, each with
its own completeness check:

| Axis | Items | The check |
|---|---:|---|
| `typekind` | 28 | Annex B declares the constants twice; both listings must agree, and no value twice |
| `typeid` | 31 | every `TI_*` constant is a `case` of `union TypeIdentifier` and every `case` a declared constant; plus Table 29 |
| `annotation` | 24 | §7.3.1.2.1 subclauses 1..16 with no hole; plus the 19 annotations of Table 21 |
| `assign` | 55 | §7.2.4.4 subclauses 1..8 with no hole; Tables 14..19 present; every rule in `anchors.tsv` a literal fragment of the PDF |
| `typeobject` | 13 | `MinimalTypeObject` and `CompleteTypeObject` carry the same `case` labels |
| `typelookup` | 16 | §7.6.3.3 subclauses 1..4, exactly two operations, ten `TypeLookup_*` types |
| `datarep` | 16 | Table 59's four verdicts; all twelve rows of Table 60 parse |

183 items, 19 checks. `roster.py` exits 1 and writes nothing if any check
fails, because a denominator that is not trustworthy makes every number
on the coverage page a guess.

The PDF is OMG intellectual property and is not in this tree.
`roster.py` reads it from
`internal/standards/cache/omg/dds-xtypes-1.3.pdf` (gitignored) through
`pdftotext -layout`, or from `XTYPES_PDF`. What is committed is the
extracted roster — constant names, clause numbers, table row labels — and
the checks.

## Files

| | |
|---|---|
| `roster.py` | the denominator: seven extractors and their completeness checks |
| `roster.tsv` | what it wrote, one line per item of the denominator |
| `anchors.tsv` | the 55 rules of §7.2.4.4, each with a literal fragment of the PDF |
| `cases.tsv` | the probe corpus, one line per probe |
| `expected.tsv` | the recorded state of the gap, written by `--record` |
| `scan.sh` | the driver; its header comment carries the exit codes |
| `probe/` | the measuring instrument: one probe in, three measurements out |

`cases.tsv` columns:

| Column | Meaning |
|---|---|
| `id` | probe name, cited by the coverage page |
| `axis` | one of the seven axes of `roster.tsv` |
| `refs` | the clause or table the probe stands for |
| `expect` | `accept`/`reject`, `assignable`/`not-assignable`, `compatible`/`incompatible` — what DDS-XTypes 1.3 says, never what this tree happens to do |
| `op` | `ti`, `to`, `todh`, `idl`, `assign`, `datarep`, `encaps`, `tl` |
| `witness` | a fixed string searched over every committed source; `-` where no string isolates the construct |
| `arg` | the probe itself; its shape depends on `op` and is documented at the head of `cases.tsv` |

The scan does **not** compare against `expect`. It compares against
`expected.tsv`, so that the specification's answer lives in one place —
the coverage page — and the recorded state of the gap in another. Putting
the answer in two places is how the two drift.

## The probes are octets

That is the advantage of a wire specification and the reason this one
came before CORBA. A `TypeIdentifier` written out by hand from Annex B is
one line: `80F3000020 04` is a plain sequence of `long` with a bound of
32. `probe/src/main.rs` reads `id<TAB>op<TAB>arg` on stdin and writes
`id<TAB>decode<TAB>encode<TAB>rule`; the table of what those three mean
per op is in its header comment.

`probe/` is a workspace of its own. It is deliberately not in the root
member list: a measuring instrument should not change the product's
`Cargo.lock`. Build it with

```
cargo build --release --manifest-path tools/xtypes-1.3-gap-scan/probe/Cargo.toml
```

## Adding a probe

Write the line, run `--record`, and write the row into
`docs/spec-coverage/dds-xtypes-1.3.gaps.md`. Two failure modes cost the
most time and are worth naming:

- **The probe fails for the wrong reason.** A key member is implicitly
  must-understand, so a probe meant for the key rule trips the
  must-understand rule first; an array of appendable elements *is*
  strongly assignable, because §7.2.4.2 makes appendable types delimited
  under encoding version 2. Read the diagnostic (`--explain`) before
  recording it as a finding.
- **`expect` is taken from the tree instead of the specification.** The
  column exists to be the specification's answer. `expect=reject` needs a
  sentence from the PDF behind it, quoted in the coverage page. Where the
  specification does not settle the question, there is no probe. Where
  the specification contradicts *itself* — and it does, three times in
  this survey — the row says which of the two sentences the tree follows
  and points at `docs/omg-xtypes-issues.md`.

## Doing this for another specification

The shape carries over; the denominator does not, and the cost sits in a
different place each time.

| Corpus | Denominator | Estimated effort |
|---|---|---|
| CORBA 3.3 | Yes — Part 1 Annex A carries the same consolidated grammar, plus the GIOP/IIOP message types as tables. The running text is ~1,150 pages over three parts, eight times IDL 4.2. | One day **per part**. The grammar axis is cheap; the "accepted although forbidden" axis is the cost driver and needs the full text clause by clause. The probes need a driver of their own: for GIOP/POA/IR a probe is a Rust test, not an `.idl` file. |
| DDSI-RTPS 2.5, DDS-Security 1.2, DCPS 1.4 | Like XTypes: a set of tables — submessage kinds, plugin operations, QoS policies — each needing its own extractor and its own completeness argument. | One day each, distributed as here: cheap probes, expensive denominator. `tools/byte-gate` and `tools/wire-oracle` are the existing drivers. |
| The tree's own specs under `docs/specs/` | Yes, and cheaper than all the others — 47 Markdown documents with numbered clauses, one extractor for all of them. | Hours for the denominator. The catch is a different one: a spec written here cannot be wrong against itself. What a negative list finds there is a clause without an implementation — useful, but not the same as a gap against a foreign denominator. |

The method note at the end of `docs/spec-coverage/dds-xtypes-1.3.gaps.md`
carries what this one actually cost.
