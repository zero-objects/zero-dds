# Rust, C and Python, over the whole corpus, in both directions

Written by `crates/wire-probe/tests/three_column_gate.rs`, which is
the test that produced it and the test that reads it back: a run
that disagreed writes the disagreement here and fails on it, so a
full-tier failure is an assertion and not only a file.

Tier `full`. Rust 26s, Python 70s, C 11s.

## The matrix

421 cases, 12 configurations, 5 encodings, 2 byte orders. Per case and
configuration: the payload and the encapsulated payload in each
encoding and order, the read direction for each of those, and one
KeyHash per encoding — 176820 slots in the Rust column.

| column | slots measured | of the Rust column's |
|---|---:|---:|
| Rust | 176820 | 100% |
| C | 176820 | 100% |
| Python | 151560 | 86% |

## What each column answered

| column | bytes | read back the same | read back different | refused |
|---|---:|---:|---:|---:|
| rust | 101300 | 50020 | 60 | 25440 |
| c | 100580 | 49620 | 60 | 26560 |
| python | 100080 | 49980 | 60 | 1440 |

## What the refusals carry

The class is what the gate compares. The sentence is not: three
implementations write three sentences for one refusal, and asserting
on the English would make the gate fail on a rewording. Where two
columns' messages both name a value path, the paths are compared too.

| class | rust | c | python |
|---|---:|---:|---:|
| no mapping | 0 | 0 | 0 |
| value outside the type | 480 | 600 | 480 |
| no @key | 24020 | 23720 | 0 |
| unwritten | 440 | 800 | 480 |
| other | 500 | 1440 | 480 |

## The disagreements

None.

## Known, understood, and not fixed here

Each entry is a divergence this gate found, with its root cause and
where the fix belongs. The register is checked in both directions: a
divergence outside it fails the gate, and — in the full tier — an
entry that matches nothing fails it too, so a fix cannot leave an
exemption behind to swallow the next regression in the same place.

| slots | root cause |
|---:|---|
| 1260 | C23 §6.7.2.1p8 leaves a structure with no named member undefined, and the probe builds with -pedantic-errors. The Rust and Python columns write the four octets an @appendable empty aggregate frames under XCDR2 — a DHEADER of zero — and C has nowhere to put the type at all. Not a defect and not fixable here: it is the language, and C++ carries the same case because C++ has no such rule. crates/lang-rules/src/adapter/c.rs refuses it by name; docs/language-limits.md is the standing table. |
| 120 | The IDL front end refuses the file: OMG IDL 4.2 §7.5.2 does not admit a name redefined within the immediate scope of the module, structure, union, map, interface, value type, event type, exception or home that carries it, and the clause prints module M { typedef short M; } as its own // Error example. All three columns lose the case from that one root and classify the loss differently only because each meets the refusal at a different stage: the Rust column has the sentence, the C column has no file to compile and the Python column reports the path. The case is in the corpus on purpose — it measures which vendors refuse the file and which resolve it anyway — and tools/idlc/tests/corpus.rs carries it as an intended diagnostic. |

1380 slot(s) matched the register. One of each:

```text
empty-struct cyclone/delimited-cdr/be
    rust   00000000
    c      refused (other): generator: m_S: C has no form for ``m_S`, an aggregate with no members — C23 §6.7.2.1p8 leaves a…
    python 00000000
empty-struct-as-base cyclone/delimited-cdr/be
    rust   00000006ffed2979fffe
    c      refused (other): generator: m_Base: C has no form for ``m_Base`, an aggregate with no members — C23 §6.7.2.1p8 le…
    python 00000006ffed2979fffe
empty-struct-in-sequence cyclone/delimited-cdr/be
    rust   000000240000001000000003000000000000000000000000
    c      refused (other): generator: m_E: C has no form for ``m_E`, an aggregate with no members — C23 §6.7.2.1p8 leaves a…
    python 000000240000001000000003000000000000000000000000
resolve-module-named-as-type fastdds/delimited-cdr/be
    rust   0000000400000001
    c      0000000400000001
    python refused (other): /root/b5-wt/tests/interop-corpus/resolve-module-named-as-type/type.idl: [Diagnostic { severity: …
```

## What this run did not cover

Nothing in the matrix above: the full tier runs every column over
every case.

Outside the matrix in either tier:

- **Cross-column reads over one model, because they are already
  here.** Each column's read slot is that column's reader over the
  bytes of the payload slot beside it, and every payload slot is
  compared across the three columns above — 101300 of them, none
  differing. So column X's bytes into column Y's reader is Y's own
  reader over the same input: the same decoder, the same bytes, the
  same answer. Running it would spend 101040 more probe launches
  reproducing a result the byte comparison already carries.

  The cross-column read that is **not** that is the one where the
  two ends do not share a model, and it is measured:
  `crates/wire-probe/tests/evolution.rs` runs all fourteen
  `evolution-*` pairs through all three columns — `old.idl` writes,
  `type.idl` reads — in four modes each. The reader there is handed
  bytes its own model would never have produced.

  All three verdicts are now reached in one **currency**. Every
  column's read slot is the canonical text of
  `crates/wire-probe/src/canonical.rs`, held against the same
  canonicalisation of the case's own sample. Python reaches it the
  way C does: `crates/lang-python` emits a `to_json` beside its
  `from_json`, `tools/wireprobe/wireprobe-zerodds-python` takes
  `--in` and `--values-out` like the other two probes, and the
  document that comes back is canonicalised in Rust. What the old
  arrangement could not see is what it was replaced for: the
  Python column compared objects against its own `from_json`, so a
  value its decoder built which no writer would emit was outside
  the comparison — the shape of the `fixed` defect, whose reader
  returned the BCD field's leading zeros while the re-encoded
  bytes matched nibble for nibble.
- **The `--pl-cdr1-headers`, `--key-holder` and `--default-extensibility`
  overrides on their own.** All three are reachable through the four
  dialects, and the C generator takes only the first of them on its
  command line, so an isolated column for them would measure two
  implementations and not three.
