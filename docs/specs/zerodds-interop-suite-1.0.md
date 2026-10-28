# The interop suite

Status: design. Supersedes the hand-written byte gate at
`/root/byte-gate/gate` on the Linux test host, which proved the method
and does not scale.

## 1. What has to be covered

Interop is not one property. Six axes cross, and a pass on one says
nothing about another.

| axis | values |
|---|---|
| IDL construct | every one the frontend accepts — see §3 |
| encoding | XCDR1 / XCDR2, and per type plain, delimited, parameter-list |
| byte order | little, big |
| implementation | Cyclone DDS, RTI Connext, Fast DDS, OpenDDS, and ZeroDDS in each of five languages |
| direction | for each ordered pair, A writes and B reads |
| layer | bytes, TypeObject, discovery, RTPS messages, live pub/sub, QoS |

The implementation axis has two halves that answer different
questions:

- **Wire identity** — ZeroDDS against Cyclone, RTI, Fast DDS and
  OpenDDS. Does what we put on the wire match what the field runs?
- **Engine identity** — ZeroDDS in Rust against ZeroDDS in C, C++,
  Java and Python. Does every language binding implement every feature
  the same way, or only the features its author happened to test?

The second is the one no vendor comparison can answer, and the one a
user picking a binding depends on. A binding that is byte-identical to
Cyclone for structures and wrong for unions is a binding that fails in
the field and passes every cross-vendor test.

Eight implementations and two directions is fifty-six ordered pairs
per case. The existing gate covers one layer, one direction, one byte
order and sixteen hand-written cases.

## 2. The bottleneck, and the lever

Each case currently needs a hand-written value-filling snippet per
implementation, because the generated C and C++ APIs differ. Forty
constructs times five implementations is two hundred snippets that
have to stay in step with each other. That is why the gate stopped at
sixteen.

The lever is that this project already has a parser, a common AST and
per-language materialisers. A case generator walks the AST and writes
the reference program for each implementation itself: the struct field
names come from the model, and the values come from a deterministic
rule over the type — the *n*th scalar in declaration order gets a value
derived from *n* and its kind. Adding a case becomes adding one IDL
file.

### Why this is not a self-oracle

The generator only places values into the *vendor's* generated
structures. The bytes come from the vendor's own serialiser. A
generator that placed a value wrongly would show up as a disagreement
between the three implementations on that case, not as a false
agreement — the same mechanism that caught the alignment error. The
value rule reads declaration order and primitive kind, never this
project's layout planner.

## 3. The corpus

One IDL file per construct, named after it. The list is the frontend's
own coverage, so a construct the parser accepts and the corpus omits is
a gap that shows up as a missing file rather than as nothing at all.

- primitives: each of the fifteen, alone and mixed
- `string` / `wstring`, bounded and unbounded, empty and non-empty
- `sequence`, bounded and unbounded, of each element kind, empty and
  non-empty, nested one level
- arrays: one, two and three dimensions, of each element kind
- `enum` at bit bounds 8, 16 and 32, with and without `@value`
- `bitmask` at each bit bound, `bitset`
- `union` over each legal discriminator kind, with and without
  `default`, with multiple labels per case
- structures: `@final`, `@appendable`, `@mutable`; inheritance; nested
  two levels; a `@mutable` inside an `@appendable`
- `typedef` chains, including a typedef of a collection
- `map`, of each key and element kind combination the vendors support
- annotations: `@key` including a key inside a nested struct,
  `@optional` present and absent, `@external`, `@must_understand`,
  `@id`, `@hashid`, `@default`
- module nesting and root-anchored scoped names — the shape from
  issues #41 and #42
- `fixed<P,S>`
- `long double`, which is IEEE binary128 in every column — see
  `docs/float128-column.md` for the four vendor measurements that settled
  the representation and the crate that carries it

Each file is annotated with the extensibility under test, and the
generator also emits the unannotated variant, because the vendors
disagree about what that means.

## 3a. The seventeen ZeroDDS probes

One probe per language, same command line, same JSON:

```
wireprobe-zerodds             Rust     (reflective)
wireprobe-zerodds-ada         Ada
wireprobe-zerodds-c           C
wireprobe-zerodds-cpp         C++
wireprobe-zerodds-csharp      C#
wireprobe-zerodds-d           D
wireprobe-zerodds-elixir      Elixir
wireprobe-zerodds-go          Go
wireprobe-zerodds-java        Java
wireprobe-zerodds-julia       Julia
wireprobe-zerodds-lua         Lua
wireprobe-zerodds-nim         Nim
wireprobe-zerodds-ocaml       OCaml
wireprobe-zerodds-python      Python
wireprobe-zerodds-swift       Swift
wireprobe-zerodds-typescript  TypeScript
wireprobe-zerodds-zig         Zig
```

### They must not share a serialiser

A binding that calls the Rust core through FFI is byte-identical by
construction and proves nothing. Each language probe uses that
language's own generated code and that language's own wire runtime.
That is what makes a difference between two of them a finding.

The Rust probe is the exception in one respect: it is reflective,
walking the model and the plan rather than generated code, because it
is also the reference the planner is checked against. The other sixteen
go through the code generator, which is what a user gets.

### What each one needs

| piece | Rust | C | C++ | C# | Java | Python | Elixir |
|---|---|---|---|---|---|---|---|
| wire runtime — align, primitives, string, DHEADER, EMHEADER, PID | built | built | built | built | built | built | built |
| materialiser — model to that language's CST | built | built | built | built | built | no CST — see below | built |
| codec emitter — encode/decode from the layout plan | built | built | built | built | encode only | built | built |
| probe binary — JSON in, bytes out | built | built | built | built | built | built | built |
| the same probe the other way — bytes in, values out | built | built | built | built | | built | built |

The ten columns not in the table — Ada, D, Go, Julia, Lua, Nim, OCaml,
Swift, TypeScript and Zig — have the same five pieces; their numbers
live in `docs/HANDOFF.md` level 3 rather than here, and
`docs/language-columns-status.md` carries what each column reaches.

The C column's pieces are `runtime/c`,
`crates/lang-rules/src/adapter/c.rs`,
`crates/lang-rules/src/emit/encode.rs`,
`crates/lang-rules/src/emit/decode.rs` and
`tools/wireprobe/wireprobe-zerodds-c`, which takes `--in` and
`--values-out` and answers with the canonical JSON of
`crates/wire-probe/src/canonical.rs`. Both directions are measured over
the whole corpus, which is what P2 needs from a column.

The Python column's are `runtime/python/zerodds_wire`,
`crates/lang-python` and `tools/wireprobe/wireprobe-zerodds-python`,
which takes the same two flags and answers the same way. Its `to_json`
writes the value dialect and not canonical form; the caller
canonicalises, because one canonicaliser is what makes two columns
comparable and a second one in a second language is a second chance to
part over a spelling. Both directions are measured over the whole
corpus by `crates/wire-probe/tests/three_column_gate.rs`.

The C++ column's pieces are `runtime/cpp`,
`crates/lang-rules/src/adapter/cpp.rs`, the same two shared codec walks
the C column uses through `crates/lang-rules/src/emit/cpp.rs`,
`crates/lang-rules/src/default_cpp.rs` — DDS-XTypes 1.3 Table 9 as C++,
which is what a reader needs and a writer does not — and
`tools/wireprobe/wireprobe-zerodds-cpp`, which takes `--in` and
`--values-out` and answers with the same canonical JSON.

The C# column's pieces are `runtime/csharp`,
`crates/lang-rules/src/adapter/csharp.rs`, the same two shared codec
walks through `crates/lang-rules/src/emit/csharp.rs`,
`crates/lang-rules/src/default_csharp.rs` — Table 9 as C# — and
`tools/wireprobe/wireprobe-zerodds-csharp`, which takes `--in` and
`--values-out` and answers with the same canonical JSON. Both
directions are measured over the whole corpus by
`tools/wireprobe/wireprobe-csharp-corpus`.

Its probe compiles through the SDK's own Roslyn rather than through
`dotnet build`, because a corpus sweep is thousands of compilations and
MSBuild spends two to four seconds per invocation before a file is
read. The runtime is compiled once into a cached assembly keyed on the
contents of its sources, as the Java column caches its class files.

The Elixir column's pieces are `runtime/elixir`,
`crates/lang-rules/src/adapter/elixir.rs`, the same two shared codec
walks through `crates/lang-rules/src/emit/elixir.rs`,
`crates/lang-rules/src/default_elixir.rs` — Table 9 as Elixir — and
`tools/wireprobe/wireprobe-zerodds-elixir`, which takes `--in` and
`--values-out` and answers with the same canonical JSON. Elixir has no
mutation, so the emitter hoists every state-changing runtime call into a
named binding and `zerodds_lang_render::elixir` threads each block's
rebindings out as an accumulator tuple; nothing is carried in a process
dictionary, an `Agent` or an ETS table, which would make the generated
codec's state invisible to the code that reads it.

A module segment that names a module of the Elixir distribution is
escaped there, because a redefinition of one does not shadow it, it
*replaces* it: `defmodule Node do defstruct […] end` and `%Node{}` in
one file is `error: Node.__struct__/1 is undefined`, and an IDL
`struct Node { sequence<Node> children; };` is an ordinary recursive
declaration. `zerodds_lang_cst::elixir::RESERVED_MODULES` holds the
list.

The Java column has the write direction only:
`CodecTarget::default_into` refuses for Java, so no reader is generated
and `wireprobe-zerodds-java` says so rather than producing one.

Python has no CST and deliberately so. Its block structure is its
indentation, so the renderer a CST would need owns the whole grammar,
and the guarantee a typed tree buys — that the emitter cannot produce
something unparseable — is bought instead by compiling every generated
module in the test suite. The argument a CST wins for the other four is
that two languages' treatment of one construct is a diff between two
rule tables; that argument is about the *rules*, and every rule is in
`zerodds-ast-layout` and shared by all five. The reasoning is in the
module documentation of `crates/lang-python/src/lib.rs` so that the next
language weighs the same three things.

### What the C column reaches

Measured on 2026-08-09, `wireprobe-zerodds-c` against
`wireprobe-zerodds`, over 320 corpus cases in five encodings and both
byte orders, on two machines: Apple clang and, on the Linux test host, gcc 14.2.0.

| | |
|---|---|
| payload runs byte-identical | 3160 of 3200 |
| payload runs read back as the sample that went in | 3160 of 3160 |
| runs both probes refuse | 40 — `bitset-inheritance`, and the two characters that do not fit their type |
| runs either probe alone refuses | 0 |
| KeyHashes identical, over the keyed cases | 12 of 12 |
| vendor rows reproduced through the compiled C, both directions | 11 of `vendor_bytes.md` |

The read direction is the Rust probe writing and the C probe reading,
compared against the case's own sample and never against the Rust
probe's reading of its own bytes — two decoders wrong in the same way
agree with each other.

Every generated translation unit compiles under `-Wall -Wextra
-Wpedantic -Wconversion -Wsign-conversion -Werror`, which is now the
probe's default and therefore applies to every run rather than to a
separate check. gcc raised no warning clang did not, anywhere.

Ten places the two implementations parted are written up in
`docs/c-column.md`, five of which would have changed a value or a byte:
two defects in the C column the corpus caught, one per direction; two in
`runtime/c`'s reader that no caller had ever reached; two hazards found
before any byte was wrong; one defect in the Rust probe's own
expectation; the length-code trap that produced the closest thing to a
shared blind spot; three warnings the strict flag set turned up once it
applied to every run; and a KeyHash the two columns computed
differently under one dialect. The gates are
`make -C runtime/c check`,
`cargo test -p zerodds-lang-rules --test c_end_to_end` and
`cargo test -p zerodds-wireprobe-c-gen`; the full corpus runs are
`tools/wireprobe/wireprobe-zerodds-c-corpus` and the same with
`--roundtrip`.

### The pattern, for the columns that follow

Addressed to whoever builds C++ and Java. Python took a different route
for a reason it states; these are the parts that are not language
specific.

**Four pieces, in this order.** The runtime first, with its own byte
expectations, because everything above it is untestable until something
can write a `long`. Then the materialiser — the model as that language's
declarations. Then the codec emitter, which reads
`zerodds-ast-layout` and never decides a framing question itself — both
directions from the same plan, because two walks that decided the
framing separately agree only until one of them changes. Then the
probe, which generates, compiles and runs, in both directions. The probe is what makes
the column real: a test that asserted bytes from the emitter's output
would be asserting the layout plan a second time and would pass on a
machine with no compiler.

**The runtime is not a transcription.** Read the clause and implement
it. Transcribing `crates/wire` reproduces its defects, and a second
implementation that does that is not a second opinion — the C column
copied the reader's length-code-5 handling and inherited a bug that had
already been reported. Where the two then disagree, one has misread the
specification, and finding that out is the whole point.

**Name a generated support type after the IDL kind, never after the
target language's spelling.** `double` and `long double` are one C type
and two wire types; `octet` and `uint8` are one type in most languages
and `char`, `short`, `octet` and `int8` collapse onto three in Java. A
name built from the target spelling merges two types that do not share a
wire form, and the second one goes out at the first's width with nothing
to say it had.

**Range-check where the value is still wide.** The runtime refusing an
out-of-range `char` is not enough if the generated field is eight bits:
the narrowing happens at the assignment, before the runtime is called.
The check belongs in whatever fills the sample.

**Compile the generated output with the strictest flags the language
has, on every run and not only in a gate.** `-Werror` found the
type-name collision above through an unused function, before any byte
was wrong; making it the probe's default rather than a separate check
then found two more places where the generator emitted something
nothing calls. A generated file that warns also trains its readers to
stop reading warnings. What it must not become is a blanket suppression
— `static inline` on every generated function would have silenced the
one warning that was a real defect along with the three that were not.

**Take the dialect from `zerodds_ast_layout::Dialect` and expose
`--dialect`.** Every probe has to default the same way
`wireprobe-zerodds` does. A column with its own defaults reports a
divergence of its own making, and the matrix cannot tell that from a
real one.

**Ask what the binding holds before deciding what the decoder
assigns.** An enumeration's wire pattern and the value the binding
stores are the same number only below the holder's sign bit.
`@value(200)` under `@bit_bound(8)` goes out as `c8`, reads back as
`-56` because §7.4.3.5.3 makes the holder signed, and the C member
holds `200` because that is what the materialiser's constant says. The
reflective Rust decoder cannot meet this — it returns a *name* and
never holds the integer — so it is a hazard only a generated column
has, and C++, Java and Python all store an enumeration as an integer.
One corpus case has a value above the sign bit and no other does.

**The read direction needs two things the write direction did not.**
Somewhere to put a string or a sequence's elements, because the length
is the stream's; and a way for the generated code to refuse. An integer
that names no enumerator, a bit the bitmask does not declare, a
sequence past its bound and a member that never arrived are refusals of
the *type*, which the wire runtime cannot see. A decoder with no way to
say so hands its caller a value the type does not admit.

**Read the clause for the reader too, and not only for the writer.**
Two defects in `runtime/c`'s reader had been there since it was written
and nothing had found them, because nothing had called it: a PL_CDR1
parameter value aligned from the stream instead of from its own start,
and a parameter id compared against the whole sixteen-bit word so that
Cyclone's flagged marker and the sentinel RTI also writes went
unrecognised. A runtime half of which is untested is a runtime that is
wrong in the half nobody ran.

**A key holder is planned under its own key, and the key threads down.**
`keyhash::member_key` decides which purpose a nested member's codec is
named under, and one dialect turns `Purpose::Key` into `Purpose::Data`
below a keyless aggregate — RTI under XCDR1 stops filtering there and
takes the subtree as the data has it. A column that names every nested
codec by the enclosing purpose produces the recursive answer under every
dialect, which is right three times out of four and gives two peers two
instance handles for one instance the fourth. Neither the RTPS §9.6.4.8
example nor any corpus case shows it: the shape that does is a keyed
struct below a keyless one, and
`wireprobe-zerodds-c-gen/tests/keyhash_nesting.rs` is it.

**Compare a decode against the sample, never against your own
encoder.** A round trip through one implementation passes a decoder
that is wrong in the same way its encoder is wrong, every time. The
comparison that means something is A's bytes into B and B's values
against what went into A.

**Write the disagreements down, including the ones that were your
fault.** `docs/python-column.md` and `docs/c-column.md` are the format:
what was built, what it reaches as a count, and every place the two
implementations parted with the clause that settles it.

### What the Python column reaches

Measured on 2026-08-09, `wireprobe-zerodds-python` against
`wireprobe-zerodds`, over 316 corpus cases in five encodings and both
byte orders:

| | |
|---|---|
| payload runs byte-identical | 3140 of 3160 |
| runs both probes refuse | 20 — `bitset-inheritance` and its twin |
| runs either probe alone refuses | 0 |
| KeyHashes identical, over the keyed cases | 12 of 12 |
| rows reproduced from `vendor_bytes.md` | 26 of 26 |

Four places where the two implementations part, one of them a defect in
the Rust reader and one an ambiguity in the wire format itself, are
written up in `docs/python-column.md`. The gate is
`cargo test -p zerodds-lang-python`.

The read direction is measured with the other two columns rather than
here: `tools/wireprobe/three-column-gate --full` runs every case in
every configuration through all three, and every column's `/read` slot
is the canonical form of what its decoder built, held against the
canonical form of the case's own sample.

### Order

C first: it is the binding with the most deployments, and its runtime
is the one an embedded target links. Then Python, because it is the
fastest to a running second opinion and will find the first
disagreement. Then C++ and Java.

C and Python are built. Both reach every corpus case the Rust probe
does, in both directions of the byte-order axis and in all five
encodings. C reads as well as it writes: the whole corpus goes back
through the compiled C and is compared against each case's own sample,
which is what P2 asks of a column. C++ and Java are the two that remain,
and "The pattern, for the columns that follow" above is what the first
two learned.

A language is not counted until its probe reproduces every case in the
corpus that the Rust probe does. A partial language in the matrix is
worse than an absent one — it reads as coverage.

## 4. Phases

Each phase ends with something that fails when interop breaks.

### P1 — bytes, every construct, every implementation, every language

The case generator, the corpus, and a runner that produces, for each
(case, implementation, encoding, byte order), the payload bytes. The
result is checked into `crates/ast-layout/tests/fixtures/` as golden
files, and a Rust test asserts the planner and the generated encoder
predict them. Divergences between implementations become policy flags,
grouped into dialects — see §7.

Deliverable: no construct in the corpus is unmeasured, and every
divergence is named in `vendor_bytes.md`.

### P2 — the decode direction

A dump program per implementation that reads a payload and prints the
values in one canonical text form. Then every ordered pair: A's bytes
into B, B's dump against A's dump. This is what catches a decoder that
is wrong in a way its own encoder is wrong in too.

### P3 — TypeObject and TypeIdentifier

The same IDL through each implementation's type-object generator, and
the hashes compared. Two peers that compute different
`TypeIdentifier`s never match a topic whatever their bytes look like —
this is what issue #42 reports. Needs the TypeObject serialisation,
which the rebuild has not reached yet.

### P4 — RTPS messages

Submessage-level comparison against captures: `DATA`, `DATA_FRAG`,
`HEARTBEAT`, `ACKNACK`, `GAP`, `INFO_DST`, `INFO_TS`, and the
`KeyHash` and `StatusInfo` inline QoS.

### P5 — live pub/sub

Every ordered pair over a domain, per QoS combination that changes
matching: reliability, durability, history, ownership, partition,
deadline, lifespan, liveliness. Docks onto the #28 gate, which already
runs three required jobs on the internal runner.

## 5. Where it runs

Golden files in the repository mean `cargo test` reproduces the whole
byte layer on any machine with no vendor installed. That is legitimate
here because the golden files are *foreign* output, not this project's.

The vendor run itself is a CI job in the `ci-interop` image: it
regenerates the golden files and fails on any difference. A difference
is either a vendor upgrade changing behaviour — which is exactly what
one wants to be told about — or a corpus change that was not
regenerated.

The full cross matrix of P2 and P5 runs nightly, not per commit.

### What a probe keeps besides the octets

A run of a probe produces three things and used to record one. The
octets go to the ledger. The toolchain's own diagnostics go to
`tools/wireprobe/diagnostics.sh`, which exists because a deleted
`-Wmultichar` warning left 56 rows across two vendors standing as a
divergence until someone read `c3a4` back out of the bytes. The
**generated source** went to `/dev/null` with the probe's `mktemp -d`.

That third thing is the answer to a question the ledger cannot hold:
what does each toolchain make of the same IDL. `docs/structural-review.md`
had to reconstruct it by hand, which is how it found that five columns
flatten an inherited struct where HDDS nests it, and that two columns
discarded the IDL spelling of a member a keyword had moved — which
§7.3.1.2.1.1 needs to compute a hashed member id.

`WIREPROBE_KEEP_GENERATED=<dir>` names a directory and every probe
fills `<dir>/<case>/<system>/`. It runs from the exit trap, so a case
a front end **refused** keeps its files too — that is the one a reader
most wants. A cached build never runs its front end, so the sources
are published with the binary and replayed on a hit, the same shape
the diagnostic rows already use.

`tools/wireprobe/collect-generated` drives every column into one tree:
the columns whose materialiser is a Rust function render the whole
corpus in one `cargo test` under the switch each already carries
(`ZD_RUST_OUT`, `ZD_PYTHON_OUT`, and the rest), and the columns that
need a toolchain run one process per case. Both land in the same
layout, and which path a system took is not visible in the output.

Two things it has to keep apart, because they look alike in a
side-by-side view and only one is a finding:

- A system with **no directory** has no toolchain on the host that ran
  the collection. Counted from the probe's exit status and not from
  the presence of a directory: the trap copies the input IDL too, so a
  directory exists even for a probe that never reached its front end.
- A system whose **front end ran and whose probe then failed** is a
  kept column, not an absent one. `hddsgen` generated all 376 and its
  probe ran none.

Each probe runs under a clock, `WIREPROBE_TIMEOUT` seconds. Cyclone's
`idlc` on `struct Node { sequence<Node> children; }` — legal IDL 4.2,
with an SCC rule in XTypes §7.2.3 — ran 24 minutes at 99% of a core
and 11 GiB of a 15 GiB host, still growing when it was killed, and it
stopped a collection at case 227 of 376.

### Why the collection runs the whole probe

A front end needs only the IDL, and the collection runs generate,
fill, compile and execute. That looks like the wrong shape for a page
about generated code, and it was worth measuring rather than
reasoning about: how many cases would a generate-only path add?

Counted against the front ends directly, over the cases that have no
sources today:

| toolchain | without sources | its front end alone manages |
|---|---|---|
| Cyclone | 60 | 2 |
| RTI | 105 | 6 |
| Fast DDS | 16 | 6 |
| ROS 2, HDDS | 0 | — |
| TAO, omniORB, JacORB | 127 | the CORBA IDL rewrite is required |

Of Fast DDS's six, **one** is in the published set. The dominant
reason a case has no vendor column is that the vendor's front end
cannot parse the IDL: `idlc` gives a syntax error on `bitset`,
`rtiddsgen` a `mismatched input 'bitmask'`, and the three ORBs are fed
a CORBA rewrite that cannot carry an XTypes construct at all.

So the shape is right and the coverage is not what limits this page.
A generate-only path across nine probes would add a handful of panes;
it is recorded here so the question is answered rather than reopened.

The encoding is the probe's and not the collection's. A CORBA ORB
implements GIOP's CDR, which is §7.4.2 encoding version 1, and refuses
`plain-cdr2` by name; asking every probe for one encoding had four
systems refuse all 376 cases and report as an absent toolchain.

## 6. What is settled and what is not

Settled: the case generator drives the corpus; a disagreement becomes
a named policy rather than a silent choice; and where implementations
disagree the default is the clause's answer, not any product's — §7.

Not settled: whether OpenDDS joins at P1 or later — it is installed on
the test host but its generated C++ API differs enough from Fast DDS
that the generator needs a fourth backend. And whether P3 waits for
the rebuild's TypeObject work or is measured vendor-to-vendor first,
which would at least establish what the target hashes are.

## 7. The default dialect

Where the implementations disagree, ZeroDDS writes what the
**specifications** say unless it is told otherwise. Every measured
implementation's behaviour is reachable by a flag, and reaching it takes
naming it.

This is a reversal of the rule this section carried before, which made
Eclipse Cyclone DDS the default. That rule was written when a dialect
was four names and a handful of framing settings, and it rested on one
argument: a default has to be *some* implementation's whole set of
answers to interoperate out of the box, and "the specification" is not an
implementation. What the argument left out is which answers the default
hands to a caller who names nothing.

The dialect pass recorded in `docs/wire-axes-followup.md` settled that.
Once each product's departures moved into the dialect named for that
product — because `--dialect opendds` has to reach OpenDDS on its own —
the default started writing **Cyclone's defects**:

- a `wstring`'s code units in the host's byte order rather than the
  stream's, so a big-endian `"ab"` reads back through any conforming
  peer as U+6100 U+6200;
- no branch at all for a negative case label on a signed discriminator
  narrower than 32 bits, so a conforming reader takes the member *after*
  the union for the branch.

Both are what Cyclone puts on the wire, both belong in `--dialect
cyclone`, and neither belongs in what somebody gets who chose nothing.
A dialect is a deliberate choice to talk to one peer and may carry that
peer's defects; a default is what is written when no choice was made,
and its warrant is the clause.

`Dialect::Cyclone` keeps every one of its settings and loses only its
standing: it is a dialect like the other nine, complete for its own
system and preferred in nothing.
`tools/interop-results/tests/dialect_completeness.rs` holds the
completeness of all ten, `--dialect cyclone` included.

`zerodds_ast_layout::Dialect` is the type, and its documentation carries
the table with the evidence for every cell. `--dialect specification |
cyclone | rti | fastdds | opendds | opensplice | omniorb | tao | jacorb |
ros-fastrtps | ros-cyclone` selects one on every probe; the individual
policy flags override whichever dialect is in force, so a peer that is
RTI in every respect but one is `--dialect rti` plus that one flag.

### What changed on the wire

The clause reads an aggregate whose IDL says nothing about extensibility
as `@appendable` — XTypes 1.3 §7.3.1.2.1.8 — where `Dialect::Cyclone`
reads it as `@final`. Under the Cyclone default that was the setting
behind GitHub issues #41 and #42; under this one the clause's reading is
what a caller who names nothing gets, and `--dialect cyclone` is what
restores Cyclone's. The clause itself provides for the choice:
"Implementations may provide a mechanism to override this default
behavior; for example, IDL compilers may provide configuration options
to allow users to specify whether types of unspecified extensibility are
to be considered final, appendable, or mutable." The frontend's own
`Options::default_extensibility` reaches it too, writing the choice into
the model rather than leaving each consumer to apply its own.

### Where the default writes octets no measured system wrote

`Dialect::Specification` is the clause-by-clause reading and **no
measured implementation produces it in full** — that is the cost of the
choice and it is measured rather than glossed.

A *slot* is one (case, encoding version, byte order) that at least one
of the nine systems recorded octets for; there are 1332 of them. In
**84** the default's octets match none of the systems that wrote that
slot.
`tools/interop-results/tests/dialect_completeness.rs::the_default_writes_octets_no_measured_system_wrote`
is the sweep, and it holds the case list in both directions.

No **single axis** accounts for any of it —
`no_single_axis_separates_the_default_from_every_measured_system` sweeps
every axis in both encoding versions and comes back empty. Every value
the default holds is some product's; what nobody was measured writing is
the combination:

| family | slots | what the default combines |
|---|--:|---|
| `map-*`, `typedef-of-map*`, `evolution-mutable-map` | 46 | Fast DDS is the only system that produces a `map`'s octets at all, and it writes the entries ascending by key with only the value half deciding the delimiter. The clause's declaration order and rule (14) read whole have no witness. |
| `enum-bitbound-*-unannotated`, `array-1d-bitmask-unannotated`, `array-3d-enum-unannotated`, `sequence-bitmask-unannotated`, `identifier-target-keyword-bitmask-flag-unannotated` | 12 | `@appendable` for an unannotated aggregate — §7.3.1.2.1.8, held by RTI, Fast DDS and OpenDDS — together with a holder in the declared `@bit_bound`, held by Cyclone and RTI. Each half is somebody's; the pair is nobody's. |
| `combined-everything`, `combined-everything-unannotated` | 8 | the negative narrow case label selecting its branch, beside the same pair. |
| `identifier-keyword-member-ids`, `struct-inheritance-mutable`, `combined-keyed-topic*`, `evolution-mutable-optional` | 12 | `@mutable` framing under version 1: the short parameter header wherever §7.4.1.2.2 allows it, with the clause's padded `@optional` length. |
| `evolution-mutable-key` | 2 | §7.2.2.4.4.4.8's must-understand attribute on a `@key` member under version 1: `4001` where Cyclone, RTI and Fast DDS write `0001` and OpenDDS `c001`. |

**The default this one replaced stood alone on more of the record, not
less.** The same sweep over `Dialect::Cyclone` gives **147** slots, and
the test asserts both numbers so the comparison cannot go stale.

None of the 84 is undecodable — a conforming reader reads all of them —
and every one of the nine systems is still reached by naming it, which
is `dialect_completeness.rs`'s first test.

### The reader takes the same flags

A dialect is not a writer setting. `--dialect rti` on a decode run means
"read what RTI writes", and every individual flag applies to both
directions. Most of them reach the read path through the layout plan,
which is computed from the same `LayoutKey` the writer used; the PL_CDR1
framing reaches it through `Reader::with_pl_cdr1_framing`, the same
`PlCdr1Framing` the writer takes, from the same policy.

**That was a claim and is now a gate.** Every figure in the two result
tables is a write, so a dialect that wrote a peer's bytes perfectly and
read them into the wrong value came out of them fully reached.
`tools/interop-results/tests/dialect_read_completeness.rs` is the other
half of `dialect_completeness.rs`: every recorded row of every measured
system, through this project's decoder under that system's own dialect,
against the case's own sample. 5183 of the 5207 rows read back; the other
24 are Cyclone's, where a suppressed negative narrow case label means
the peer wrote no branch for the member the sample names.

A green row count is not the claim on its own, because a reader that
never consults an axis is green wherever the dialect happens to agree
with what it assumed. The second gate moves one coordinate of each
system's assignment at a time and asks whether any recorded row then
reads differently. Twelve of the 225 (system, axis) pairs move a written
byte and no reading, and each is named: nine are axes the stream carries
explicitly — the length code is in the EMHEADER, the must-understand bit
is in the member's header, a `map`'s entry order is the order its entries
arrive in — and three are questions that system's record cannot ask, with
the system whose rows do ask them named beside each.

**Reading a peer's octets without naming the peer reads them under the
clause.** Of the 5207 rows the default alone reads 4374 into the sample,
111 into another value and refuses 722. Under `Dialect::Cyclone` — the
default this one replaced — the same sweep gives 3809, 151 and 1159: the
product default read *less* of the record and more of it wrong. Both
triples are asserted in the gate.

The reader still does not guess. The forms are not separable in-band —
`02000000 6100` is a two-character Fast DDS `wstring` and a two-byte
specification one, and the length field is the one field a hostile peer
controls. Choosing between them from the stream would invent an answer.

What it does instead is check everything the configured form makes
checkable and refuse the rest with an error that names the setting the
bytes *do* fit: a length the form cannot have produced, a declared
parameter length that disagrees with what followed it, a must-understand
bit or a header form the framing does not call for, a frame remainder
alignment cannot account for. `WireError::WrongFraming` is a separate
variant from `WireError::OutOfRange` for that reason — a user who picked
the wrong flag needs to be told which flag, not handed a parse failure.

Measured by `crates/wire-probe/tests/dialect_roundtrip.rs` over four
cases that each isolate one divergence: of the forty-eight ordered
dialect pairs, fourteen write identical bytes for the case and the other
thirty-four are all refused. None returns a wrong sample.

Two things this does not reach, and they are recorded rather than
claimed: a pair whose bytes coincide for the type at hand — a parameter
whose value already fills its four-byte slot declares the same length
under every framing — and the collection-delimiter split, where the
reader following the wrong plan is caught by the frame remainder rather
than by a named framing error.

### Where a preset cannot reproduce its vendor

Two places, both measured, both named in `Dialect`'s own documentation
rather than left silent:

- **Cyclone does not put a `wstring`'s code units in the stream's byte
  order.** Its length word swaps and its content does not, so its
  big-endian `"ab"` reads back as U+6100 U+6200 through any conforming
  peer. `Dialect::Cyclone` reproduces that — `WstringUnitOrder::
  HostUnswapped`, twelve recorded rows — because a preset that named the
  product and wrote something else could not exchange a big-endian
  `wstring` with it at all. It is the *default* that takes the clause's
  order, which is the whole of what moving the default settled.
- **RTI keeps a `@mutable` type's parameter list in the key holder
  under XCDR1**, where the clause forces the holder to `@final`. That
  pushes the holder past sixteen bytes and hashes the framing instead
  of the key. This one *is* reproduced — `KeyHolderExtensibility`, set
  by `Dialect::Rti` under XCDR1 only — because it changes an instance
  handle and not a rendering: two peers that disagree here have two
  instances where there is one.

### What a dialect does not decide

A `map`'s entry count. §7.4.1.1.4 and rule (15) of §7.4.3.5.3
contradict each other, but no vendor row has a map in it — both C
generators refuse the construct — so every dialect would name the same
value and none of them names it. `MapLengthPolicy` stays a standalone
flag until a measurement exists.
