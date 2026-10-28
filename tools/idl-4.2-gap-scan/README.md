# `idl-4.2-gap-scan`

What OMG IDL 4.2 allows, measured against this tree — and recorded, so
that the size of the gap cannot change without anyone noticing.

```
tools/idl-4.2-gap-scan/scan.sh              measure and compare
tools/idl-4.2-gap-scan/scan.sh --record     rewrite expected.tsv
tools/idl-4.2-gap-scan/scan.sh --report     print the full measurement
IDLC=/path/to/zerodds-idlc tools/idl-4.2-gap-scan/scan.sh
```

The written-out result is `docs/spec-coverage/idl-4.2.gaps.md` and its
`.en.md` twin. This directory holds the evidence those pages cite.

## Why this exists

The coverage pages under `docs/spec-coverage/` are positive lists. They
name what was looked at and how far it got, and their content is derived
from the tree. That makes them unable to answer the one question a
release has to answer: what does the specification allow that nobody here
ever fed to the front end. A page with 4600 `done` entries and no
denominator proves coverage of the examined, not completeness.

The denominator here comes from the specification: Annex A of IDL 4.2
carries all 227 grammar productions, consolidated and numbered, and
clause 8 carries the 24 standardized annotations. Every one of them is
fed in as IDL, and what comes back is recorded.

## Files

| | |
|---|---|
| `cases.tsv` | the probe corpus, one line per probe |
| `expected.tsv` | the recorded state of the gap, written by `--record` |
| `scan.sh` | the driver; its header comment carries the exit codes |

Every probe is run through all six subcommands — `check`, `lint`,
`format`, `dump-ast`, `print-deps`, `generate` — because a construct can
be missing from any one of them independently. The `trip` column formats
a probe, reparses the output and compares the *model*, which is what
catches a construct the parser knows and the formatter drops.

`cases.tsv` columns:

| Column | Meaning |
|---|---|
| `id` | probe name, cited by the coverage page |
| `axis` | `grammar`, `annotation`, `lexical`, `scoping`, `semantic`, `dialect` |
| `refs` | the production numbers or clause the probe stands for |
| `expect` | `accept` or `reject` — what OMG IDL 4.2 says a conforming front end must do, never what this tree happens to do |
| `witness` | an ERE searched over every committed `*.idl`; `-` where no text pattern isolates the construct |
| `idl` | the probe itself, one line, `<NL>` for a newline |

## Adding a probe

Write the line, run `--record`, and write the row into
`docs/spec-coverage/idl-4.2.gaps.md`. Two failure modes cost the most
time and are worth naming:

- **The probe fails for the wrong reason.** An identifier `Alias`
  collides with the keyword `alias`; a union without `default:` is
  refused by the backend over its discriminator and not over the
  production under test. Read the diagnostic before recording it as a
  finding.
- **`expect` is taken from the tree instead of the specification.** The
  column exists to be the specification's answer. `expect=reject` needs a
  sentence from the PDF behind it, quoted in the coverage page. Where the
  specification does not settle the question, there is no probe.

## Doing this for another specification

The shape carries over; the denominator does not. What made IDL 4.2 cheap
is Annex A — a consolidated numbered grammar that yields a complete list
with a hole check (`range(1, 228)` against the parsed numbers). A
specification without such an annex needs its denominator assembled by
hand, and completeness is then a claim rather than a check.

For the wire specifications the probe is bytes rather than a text file,
and the drivers for that already exist: `tools/byte-gate`,
`tools/unspecified-reach`, `tools/wire-oracle`. The four columns become:
does the decoder know the field, does the encoder write it, is there a
test vector, is it deliberately not implemented.

The method note at the end of `docs/spec-coverage/idl-4.2.gaps.md`
carries the cost estimate per corpus.
