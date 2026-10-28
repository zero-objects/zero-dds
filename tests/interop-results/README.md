# The result tree

**Draft. Not published, not linked from the website, not reviewed.**

Every result binary this project holds, in one tree, with the two
generated tables that index it and the generated page that answers
questions about it.

```text
tests/interop-results/
    matrix.html       the answers first: what no peer can do that we cannot, which flag
                      aligns us with which peer, a section per system, the rows last
    write.md          one row per case, encoding and byte order; one column per system
    read.md           the same shape, for what each system read back
    SYSTEMS.md        what each column is, and what a machine without a licensed toolchain reaches
    WARNINGS.md       every warning every front end printed, what it means, and which change a verdict
    ledger.tsv        the one machine-readable record both tables are rendered from
    row-ids.tsv       the stable row numbers, append-only
    sources/          one TSV per system, verbatim from the harness that ran it
    <case>/
        <system>/
            <encoding>.<endian>.bin   the octets that system produced
            read.tsv                  what it got back when it read them
            STATUS                    why it produced nothing, per slot
            WARNINGS                  what its front end said while it did take the case
```

`<case>` is a directory of [`tests/interop-corpus/`](../interop-corpus/),
`<system>` one of the twelve of [`SYSTEMS.md`](SYSTEMS.md), `<encoding>`
one of `plain-cdr1`, `pl-cdr1`, `plain-cdr2`, `delimited-cdr`, `pl-cdr2`
and `<endian>` `le` or `be`.

## Re-running one cell

Every cell is one command. The `.bin` beside it is what that command
printed, so a run that produces different octets is a difference and not
a rerun.

```sh
# ZeroDDS, the Rust column — no vendor needed
cargo run -q -p zerodds-wire-probe --bin wireprobe-zerodds -- \
    --idl tests/interop-corpus/primitive-mixed/type.idl \
    --type "$(cat tests/interop-corpus/primitive-mixed/type)" \
    --values tests/interop-corpus/primitive-mixed/values.json \
    --encoding plain-cdr2 --endian le --emit payload

# the same cell through Cyclone's own serialiser
tools/wireprobe/wireprobe-cyclone \
    --idl tests/interop-corpus/primitive-mixed/type.idl \
    --type "$(cat tests/interop-corpus/primitive-mixed/type)" \
    --values tests/interop-corpus/primitive-mixed/values.json \
    --encoding xcdr2 --endian le

# and what the recorded octets are
xxd tests/interop-results/primitive-mixed/cyclone/plain-cdr2.le.bin
```

A foreign probe answers with hex on stdout; the tree holds the octets.
`xxd -p` one against the other.

If the toolchain is not installed the probe says so by name — never a
silent skip. `tools/interop-vendors.sh` is where each one is looked for
and what to set when it lives somewhere else.

## Recording and checking are two commands

```sh
tools/interop-results/record.sh              # run the systems; needs their toolchains
tools/interop-results/regenerate.sh          # render the tree; needs only Rust
```

**`record.sh`** is the half that needs Cyclone's `idlc`, `fastddsgen`,
`rtiddsgen`, `opendds_idl`, `omniidl`, `tao_idl`, the JacORB IDL
compiler and a ROS 2 installation. It writes one TSV per system under
`sources/` and runs when a vendor version changes or a case is added.
Every system it cannot reach is named with the number of rows it did not
produce.

**`regenerate.sh`** renders the tree, both tables and
[`matrix.html`](matrix.html) out of `sources/` and a fresh in-process
run of the Rust column. A Rust toolchain and nothing else.

The page alone, in ten seconds rather than the minutes a full
regeneration takes:

```sh
cargo run -p zerodds-interop-results -- --page
cargo run -p zerodds-interop-results -- --page --base-url https://example.invalid/tree/
```

Every link on the page is relative to this directory, so it works from
a checkout on disk and from anywhere the directory is served whole.
`--base-url` makes them absolute, which is what to pass the day this
tree is published somewhere a bug reporter can reach; until then no
absolute link is emitted, because a link into a tree that is not
published is a link to a 404.

### What a re-record costs

It used to be one build per **cell**: 1,340 runs of the vendor's code
generator and 1,340 compiles per vendor, because the encoding version
and the byte order were `sed` substitutions in the probe's template and
so part of the binary's identity. Two changes took that apart.

Both settings are arguments now, so one binary answers all four
combinations — one build per **case**.

`tools/wireprobe/wireprobe-batch` then puts many cases in one build.
The corpus becomes one IDL with each case in a module of its own, the
vendor's generator runs once over it, one driver holds one function per
case, and one process walks them all. 60 cases per build by default, so
6 builds rather than 335.

Not the whole corpus in one file, for a measured reason: `fastddsgen`
dies on it with `no such property: PrimitiveTypeCode.CScopedname` and a
stack trace with no line number in it, which names no case to exclude.
A chunk that fails that way splits in half and each half is tried on its
own, down to the single case the failure finally belongs to.

A case excluded from a batch is asked again with the per-case probe,
because the batch is an optimisation and the per-case probe is the
reference. `module-root-anchored` writes `::common::util::Location`, and
inside `module zd_c7 { … }` that path does not exist — four cases the
batch refuses and the per-case probe accepts.

`tools/interop-results/tests/record_shape.rs` is what keeps all of it in
place. A template that moved a setting back to compile time would
produce the same bytes and cost four times as much to produce them, and
nothing else in this tree would notice.

### Re-recording some of the cases and not all of them

Every sweep takes `--corpus <dir>`, and a directory of symbolic links to
the cases that moved is a corpus. The rows it produces replace those
cases' rows in `sources/` and nothing else moves — which is the
difference between a change to twenty-five cases costing 132 probe
builds and costing the whole matrix's.

```sh
mkdir subset && ln -s ../tests/interop-corpus/<case> subset/
tools/interop-results/vendor-corpus-sweep.sh --corpus subset out/vendor.tsv
INTEROP_CORPUS=subset tools/corba-oracle/corba-sweep.sh out/corba.tsv
tools/interop-results/ros-corpus-sweep.sh   --corpus subset out/ros.tsv
tools/interop-results/warning-sweep.sh      --corpus subset out/warnings.tsv
tools/interop-results/read-corpus-sweep.sh  --corpus subset write-all.tsv out/read.tsv
```

One caveat, and it is the batch's: which cases share a chunk decides
which diagnostic a front end prints, so a case refused in a subset can
carry a different **reason** from the same case refused in a full run.
The verdict does not move; the sentence beside it can.

## What a front end said is a column, not noise beside an error

A vendor front end's warning is a statement, and it says one of three
things — our case is ambiguous, the vendor deviates deliberately, or the
specification leaves something open. All three are findings, and two of
this project's most valuable ones came out of a warning: Cyclone's *"No
default extensibility provided … this may change to 'appendable' in a
future release"*, which is the root cause of issues #41 and #42 and is
in no clause and in no byte; and `tao_idl`'s `error in lookup of
symbol`, which is how an annotation that was *parsed and dropped* is
told apart from one that was honoured.

The sweeps above discard them, and rightly: `wireprobe-batch` filters
warnings out of its blame precisely so that a warning about one case
cannot exclude it. So `tools/interop-results/warning-sweep.sh` runs
beside them rather than inside them, records every line every front end
printed into `sources/warnings.tsv`, and [`WARNINGS.md`](WARNINGS.md)
classifies it. A cell of [`write.md`](write.md) that carries `w` and a
number is a cell whose front end said something — **including where the
bytes agree**, because a system whose bytes agree while its front end
warned about the type is not the same as one that agreed silently.

It costs one code generation per case per system and nothing after it:
no fill, no compile, no link, no run. It produces no bytes and changes
none, so it is never a reason to re-record a column.

## Checking that the committed bytes are what a run produces

```sh
cargo test -p zerodds-interop-results          # the tree, both tables, every link
cargo run -q -p zerodds-interop-results --bin interop-results -- --verify
```

`--verify` is what both CI jobs run. It rebuilds every column it can
reach, compares against what is committed, and **names every column it
could not reach with the number of rows that leaves unverified**. A
column it cannot reach is never a failure and never a silent pass:

```text
ros-fastrtps: NOT MEASURED HERE — 1340 rows; produced by
              tools/interop-results/record.sh --ros (fastrtps)
```

## Why a cell is never blank

A blank reads as agreement, and most of the holes in a
cross-implementation matrix are not agreements. Eleven words, each
naming a different kind of answer, each linking to a file that says
more: `bytes`, `same`, `different`, `refused`, `unwritten`, `=`,
`no xcdr2`, `one order`, `no form`, `no probe`, `not measured`.
[`write.md`](write.md) defines each of them above its own table.

## The page is the answer; the tables are the record

[`matrix.html`](matrix.html) is generated from the same `ledger.tsv`
these tables are, plus the corpus's own READMEs and
[`docs/idl-building-blocks.md`](../../docs/idl-building-blocks.md). It
exists because a 3,350-row table answers no question by being present,
and the questions that arrive are always the same three: can a peer
encode something this project cannot, which flag makes this project's
octets equal to that peer's, and what does that peer do on its own. It
puts those first and the rows in an appendix, gives every row and every
section its own address so that a reply on a tracker can be a link, and
carries a per-row *how it is* beside *how it should be* — the shipped
default's octets beside the ones a clause-by-clause reading of XTypes
1.3 produces.

Nothing on it is written by hand and
`the_page_is_the_one_a_run_writes` fails when the committed page is not
what a fresh run produces.

## What the two tables are not

They are not a conformance statement and not a claim that any two
implementations agree. They are a record of what twelve programs did
with one corpus, and — where they differed — which setting of this
project's reproduces each of the others. The argument about which
answer the specification requires is
[`docs/wire-feature-inventory.md`](../../docs/wire-feature-inventory.md),
which cites a clause per row; the entry point to all of it is
[`docs/interop-evidence.md`](../../docs/interop-evidence.md).
