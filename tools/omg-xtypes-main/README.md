# `zerodds-omg-xtypes-main`

`xtypes_main` reference client for the OMG `dds-xtypes` interoperability
test suite (`github.com/omg-dds/dds-xtypes`, pinned suite `f6acbe08`).
**BUILD + LOCAL-VALIDATE phase artifact.** Nothing from this crate has
been submitted upstream — no CLA PR, no source PR, no release asset, no
push to `github.com/omg-dds`. See "Phase-2 (not done here)" at the
bottom for exactly what remains, and who decides it.

The public product/version name for an eventual upstream submission is
**undecided** (Sandra's call). Local builds use the binary name
`xtypes_main`; the suite's own release binaries are named after the
vendor and version (`rti_connext_dds_5.2_linux`), and no name has been
picked here.

This is a CLI + protocol shim over ZeroDDS's **existing** DCPS API
(`crates/dcps`) and type machinery (`crates/types`). No new
wire/CDR/QoS/assignability code was written for this binary. The four
findings it produced are in `docs/OPEN-ITEMS.md`, "Four findings the OMG
`dds-xtypes` suite produced", and summarised below.

## The other suite

`tools/omg-shape-main` is the client for `dds-rtps`, which is a
different suite with a different IDL, a different command line and a
different question. Nothing is shared between the two crates except the
argument that the type has to be generated. Do not read one's flag table
for the other:

| | `dds-rtps` / `tools/omg-shape-main` | `dds-xtypes` / this crate |
|---|---|---|
| IDL | `interop/shapes/omg-dds-rtps.idl`, 1 type | `interop/shapes/omg-dds-xtypes.idl`, 25 types |
| `string<>` bound | 128 | 32 (`STR_LEN_MAX`) |
| topic | `-t <name>`, default `Square` | `-topic <name>`, default `XTYPESTestTopic` |
| flags | `-P`/`-S`/`-d`/`-t`… (single letters) | `-pub`/`-sub`/`-domain`/`-topic`… (single-dash words) |
| what varies between two peers | the QoS | the **type** |
| harness | `interoperability_report.py`, 105 `pexpect`-driven cases | none — the suite ships one C++ source and a spreadsheet |
| the expectation | `test_suite.py` | `compatibility_matrix.xlsx`, 83 of 625 cells |

`-P` and `-S` are refused here rather than read as publish/subscribe, and
a test pins that (`cli::tests::the_other_suites_flags_are_not_quietly_accepted`).
Someone with both binaries open will type them, and a client that
silently accepted them would start a run whose output looks right and
whose command line the reviewer's notes would not match.

## What the suite measures

25 types, from five member layouts crossed with five extensibility
declarations:

| | `…Default` | `…Final` | `…Extensible` | `…Mutable` | `…MutableExplicitID` |
|---|---|---|---|---|---|
| `Shape1` — `color, x, y, shapesize` | ● | ● | ● | ● | ● |
| `Shape2` — Shape1 + `angle` at the end | ● | ● | ● | ● | ● |
| `Shape3` — Shape1 + `z` in the middle | ● | ● | ● | ● | ● |
| `Shape4` — Shape1's members reordered | ● | ● | ● | ● | ● |
| `Shape5` — inherits Shape1, appends `angle` | ● | ● | ● | ● | ● |

A run is one publisher of one type and one subscriber of another, on the
topic `XTYPESTestTopic`, and the question is whether they match — 625
combinations. `compatibility_matrix.xlsx` carries the expected answer.
It carries it as **cell fill colour, not text**: the 625 cells hold no
value, green means compatible and red means not, and only the upper
triangle is filled, so the expectation is symmetric by construction.
That colouring is transcribed into `SUITE_EXPECTED` in
`tests/compatibility_matrix.rs` — 83 of 625.

The spreadsheet also carries the suite's own notes on what it does not
cover ("Have not tested": sequences and sizes, arrays, unions, modules,
string sizes, member-name/ID collisions, manually assigned IDs that
match the automatic ones) and one on its union `TypeLibaryElementKind`
being wrong. Those are the suite's words, not ours.

## The types come from the suite's IDL

`build.rs` compiles `interop/shapes/omg-dds-xtypes.idl` with
`dds_type_impls(true)`; `src/shape_type.rs` wraps the `include!` and
holds the regression tests. Nothing here is hand-written as a type.

`tools/omg-shape-main` took its `ShapeType` from a hand-written Rust
struct until commit `98b4800bdc3`, and that was the wrong side of the
measurement: a defect in the generator could then not fail the suite,
and a defect in the hand form was charged to us. Both were real there —
the hand form left `DdsType::type_object()` at its default `None` (a
topic type name announced with no type behind it, no
`PID_TYPE_INFORMATION` in SEDP, XTypes 1.3 §7.6.3.2) and did not
override `encode_be`.

For `dds-xtypes` the argument is not an analogy. This suite measures
nothing but the type. Its 625 questions are questions about what a
`TypeObject` says and whether two of them are assignable
(XTypes 1.3 §7.2.4.4.4.4). A hand-written type would put a hand-written
answer on both sides of that question.

**All 25 generate, and all 25 work.** Measured:

- `cargo test -p zerodds-omg-xtypes-main` — 29 tests, all green. Every
  one of the 25 announces a non-`None` `TypeObject` and `TypeIdentifier`,
  keeps its `@key color`, carries the extensibility its IDL declares
  (5 `@final`, 10 `@appendable` counting `…Default`, 10 `@mutable`), and
  round-trips its own sample through its own codec.
- Live, on macOS loopback, one process per side, same type on both:
  **25 of 25** deliver samples, including all ten `@mutable` types.
  (`-samples 1 -timeout 4` per pair, publisher `-samples 5`.)

Two things had to be fixed before that was true, and both were in the
generator, not here:

- `RUNTIME_IMPORTS` in `crates/zerodds-build/src/lib.rs` did not name
  `ParameterRole`, which the `@mutable` codec reaches for
  (`Writer::write_pid`, the EMHEADER of XTypes 1.3 §7.4.3.4.1). The
  generated file named a type it had not imported, so **no `@mutable`
  struct compiled through `zerodds-build` at all**, in any consumer.
  Fixed in this commit — one name in one list. Nothing in the tree had
  put a `@mutable` type through the generator before this IDL.
- The `@mutable` emitter trips five more clippy/rustc lints than the
  four already recorded in `docs/OPEN-ITEMS.md`. They are allowed on the
  one module that wraps the `include!`, not spread through the crate.

The suite's IDL uses only the **prefix** annotation form (`@key`,
`@id(10)`, `@final`, `@appendable`, `@mutable`) — no `//@key` suffix
comments anywhere in `srcCxx/ShapeType.idl`. Our copy is the suite's
form, unrewritten; the suffix-form work going on in the front end does
not touch this file.

## The type dispatch

The suite chooses the type per run and pairs two runs of different
types, so one process has to be able to be any of the 25.
`srcCxx/ShapeType_variants.h` does that with `ShapeTypeVariants::
create_writer`/`create_reader`: a `WriterBase`/`ReaderBase` abstract
base, a template `Writer<T,…>`/`Reader<T,…>` per type, and a 25-branch
`strcmp` chain.

`src/shape_type.rs` is the same shape. `WriterBase`/`ReaderBase` are
traits; `TypedWriter<T>`/`TypedReader<T>` are the generic
implementations, monomorphised 25 times; `create_writer`/`create_reader`
are one `match` each with 25 arms over `&str`. The four template
parameters of the C++ collapse to one, because the generated `DdsType`
carries what `TSupport` and `TDataWriter` carried there, and the filler
is a trait method.

The 25 arms are not written 25 times. `variant_dispatch!` takes one list
of type names and writes `VARIANT_NAMES`, both dispatch functions and
the four accessors the tests measure through. `ShapeType_variants.h`
writes the same list four times — twice formatted as `"Shape%d%s"`,
twice as a `strcmp` chain — with nothing holding the copies together; a
name that is in the IDL and not in the one list here fails to compile.

The per-type behaviour the suite needs is two operations, and they are
the `ShapeVariant` trait: `fill_data` (the suite's own
`ShapeNFiller::fill_data` values — `x = count % 250`, `y = 2*count % 250`,
`shapesize = 30`, `z = 3*count % 250`, `angle = (5*count) % 360`) and
`print_data` (the vendor layout the suite's README shows, three spaces
of indent, `%f` for a float). Five macros implement them, one per member
layout; `Shape5` shares `Shape2`'s because it has the same five members.

## Flag and status coverage

Transcribed from `srcCxx/ShapeType_main.cxx` — its `argv` loop, its
`validate_long` ranges, its defaults, its `Info:` lines, its
`ExitStatus`. The suite ships no README flag table; the source is the
table.

| Flag | Wired to | Status |
|---|---|---|
| `-help` | `cli::usage`, printed to stderr, exit `OK` | full |
| `-help-types` | `shape_type::print_variants` — five lines of five, exit `OK` | full |
| `-pub` | publisher dispatch (the default when neither `-pub` nor `-sub` is given) | full |
| `-sub` | subscriber dispatch | full |
| `-domain <domainId>` | `DomainParticipantFactory::create_participant`, range 0..=255 as `validate_long` gives it, default 0 | full |
| `-topic <topicName>` | `DomainParticipant::create_topic`, default `XTYPESTestTopic` | full |
| `-type <typeName>` | the 25-arm dispatch; an unknown name is refused with the suite's message and the list, before a participant exists | full |
| `-samples <count>` | publisher: exit after `count` writes; subscriber: after `count` valid samples. Range 1..=100 | full |
| `-timeout <seconds>` | publisher: exit `TIMEOUT`; subscriber: exit `OK` — the asymmetry is the suite's own. Minimum 2 | full |
| exit statuses | `OK`/`ERROR`/`INTERRUPT`/`TIMEOUT` as `0`/`255`/`254`/`253` — the suite's negative returns through POSIX's low eight bits | full |
| SIGINT/SIGTERM | `Stopping...` on stderr, then `INTERRUPT` | partial — `ctrlc` installs one handler for the platform's interrupt signals; the suite additionally arms `SIGHUP`, `SIGQUIT` and `SIGABRT`, which we do not |

| Status the suite prints | Wired to | Status |
|---|---|---|
| `Writing Topic "…", type "…", count N, data:` + `print_data` | publisher loop, stdout, flushed per record | full |
| `Waiting for data on topic "…", type "…"` | subscriber start, stdout | full |
| `Read data for Topic …` + `print_data` | subscriber loop, stdout, valid samples only | full |
| `Info: Starting {publishing,subscribing} application. Domain: N, Type: T` | stderr | full |
| `Info: will stop after N {samples,seconds}` | stderr | full |
| `Info: "-x" was not specified. Defaulting to: …` | stderr, one line per default taken, in the suite's order | full |
| `reached N {samples,seconds}, exiting` | stderr | full |
| `on_subscription_matched: …` | `DataReaderListener`, stderr | partial — the line is exact and fires on a real match; a zero-delta sentinel callback from the shared runtime is filtered out here (finding 4 below) |
| `on_requested_incompatible_qos: …` | `DataReaderListener`, stderr | partial — same filter; and see finding 1: a type mismatch never reaches this status, it is not a match candidate to begin with |
| `on_inconsistent_topic: …` | `TopicListener`, stderr | yes — the runtime adds the conflicts discovery found to the `Topic`'s count (finding 3 below) |
| `create_{participant,datawriter,topic} error…` / `create datareader error…` | the failure paths, stdout as in the suite | full |
| `create_{reader,writer}: Unrecognized type: "…"` | the dispatch returning `None`, stderr | full |

Nothing in the two tables is marked `stub`. What is `partial` is
`partial` for a reason named in the row and measured below.

## What this suite measured

Four findings, all four measured, none of them in this client. The full
text is in `docs/OPEN-ITEMS.md`; this is what a reader of this crate
needs. Findings 1 and 2 are fixed; 3 and 4 are open.

### 1. A pair with two different type names never became a match candidate — FIXED

`SedpCache::match_publications` / `::match_subscriptions`
(`crates/discovery/src/sedp/cache.rs`) filtered on
`topic_name == topic && type_name == type_name`. Everything downstream —
including the assignability check in `crates/dcps/src/runtime.rs` —
only ever saw candidates that had already passed that filter, so
assignability could reject a name-identical pair and could not accept a
name-different one.

That is the case this suite is made of: its two peers always share the
topic `XTYPESTestTopic` and always differ in the type name. Measured on
macOS loopback before the fix: a `Shape1Extensible` writer and a
`Shape2Extensible` reader — which `compatibility_matrix.xlsx` marks
green — produced no match and no sample; the same type on both sides
delivered, 25 out of 25. Of the spreadsheet's 83 expected-compatible
pairs the runtime reached the 25 on the diagonal.

The filter is now the topic name alone, and the type is decided by
`DcpsRuntime::decide_type_consistency` in the two steps of XTypes 1.3
§7.6.3.4.2:

- **Step 1** — both sides announced a `TypeObject`: the reader's type
  must be assignable from the writer's under the reader's
  `TypeConsistencyEnforcement` (§7.2.4).
- **Step 2** — either side announced none, which is every DDS without
  XTypes: the two `type_name` fields must be equal exactly, "as in [DDS]
  prior to this specification", and the step fails outright under
  `force_type_validation`.

One function for both steps, called from all three match sites (remote
writer, remote reader, same-runtime pair). Pinned by
`crates/dcps/src/runtime.rs::tests::{xtypes_step1_matches_a_differently_named_assignable_type,
xtypes_step1_refuses_a_differently_named_non_assignable_type,
xtypes_step2_falls_back_to_the_type_name_for_a_peer_without_type_information}`.

### 2. An inherited type was compared without the members it inherits — FIXED

`zerodds_types::assignability::flatten_inheritance` builds the
hypothetical flat type of XTypes 1.3 §7.2.2.4.5 — base members first,
then derived — and it is public and it resolves for all five `Shape5…`
types. `check_minimal_types` and the three per-extensibility checks
under it never called it: they read `member_seq` directly.

So `Shape5Default`, which the IDL declares as
`struct Shape5Default : Shape1Default { float angle; }`, entered
§7.2.4.4.4.4 as a one-member `@appendable` struct whose only member sat
at id 4, and against `Shape1Default`, whose first member sits at id 0,
the shared prefix disagreed at position 0. The consequence was not
confined to this suite: any inherited topic type was matched on its own
members alone.

`check_minimal_types` now flattens both structs before dispatching, and
rejects with a named reason when a base type does not resolve in the
registry. Pinned by
`tests/compatibility_matrix.rs::the_inherited_types_are_compared_with_their_base_members`.

Measured, over the registry populated the way
`DcpsRuntime::register_type_object` populates it, with the default
`TypeConsistencyEnforcement` (XTypes 1.3 §7.6.3.4 — the suite creates
its endpoints with `DATA{WRITER,READER}_QOS_DEFAULT`):

| | compatible pairs of 625 | cells agreeing with the spreadsheet |
|---|---|---|
| `compatibility_matrix.xlsx` | 83 | — |
| `TypeMatcher::match_types` before | 59 | 593 |
| `TypeMatcher::match_types` after | 87 | 621 |

The one wiring moved 32 cells, all 32 toward the spreadsheet: the 28 the
suite called compatible and we did not, the two
`Shape5Mutable`/`Shape5MutableExplicitID` cells that were compatible only
because their member ids had not been folded in, and — into the other
column — the two `Shape2Final`/`Shape5Final` cells below.

**No cell is left that the suite calls compatible and we do not.**

**4 cells we call compatible and the suite does not:**

- `Shape1Mutable -> Shape2Mutable` and the reverse. XTypes 1.3
  §7.2.4.4.4.4 for MUTABLE matches members by id and lets a member only
  one side declares pass unless it is must-understand. `Shape2Mutable`
  is `Shape1Mutable` plus `angle` at the next automatic id, nothing is
  must-understand, so the clause makes the pair assignable — which is
  what we answer. The spreadsheet marks it red **and** marks
  `Shape1Mutable -> Shape5Mutable` green in the next column, although
  `Shape5Mutable` has, after §7.2.2.4.5 flattening, exactly
  `Shape2Mutable`'s five members with exactly its five ids. The two
  cells cannot both be right. Recorded as a divergence, not as a defect
  on either side, and not resolved here.
- `Shape2Final -> Shape5Final` and the reverse, new with the flattening.
  The IDL declares `@final struct Shape5Final : Shape1Final { float
  angle; }` and `@final struct Shape2Final` as `Shape1Final`'s four
  members plus the same `float angle`. §7.2.2.4.5 gives the two the same
  five members with the same five ids and the same five names, and
  §7.2.4.4.4.4 for FINAL asks whether the two member sequences are the
  same one; FINAL carries no DHEADER, so the two are also the same
  octets on the wire. The spreadsheet's Final block is "compatible with
  itself and nothing else" throughout, which is right for the other 23
  Final cells and cannot be right for this one.

Where we and the suite **agree completely**: every diagonal cell, every
cell that crosses two extensibility kinds, and every `@final` cell but
the named pair. Each of those is a test of its own.

### 3. The inconsistent-topic count discovery raises is not the one the `Topic` reports

`SedpCache::topic_name_conflicts` detects the DDS 1.4 §2.2.4.2.4
condition and bumps `DcpsRuntime::inconsistent_topic_seq`.
`Topic::inconsistent_topic_status()` reads `Topic`'s own counter, which
only `create_topic` and `Topic::record_inconsistent_topic` bump. The two
never meet, so a conflict found in discovery raises no
`on_inconsistent_topic` — and for this suite that is the one line that
tells a reviewer why a pair produced nothing.

**Fixed in the runtime** (`b037bd5df22`): `Topic::inconsistent_topic_status`
adds `DcpsRuntime::inconsistent_topic_count_for` the topic's name to the
`Topic`'s own count. The application-level `ConflictBridge` this client
carried until then forwarded the same increments a second time, so with
both in place the status counted each conflict twice; it is gone, and
the poll tick only reads the status.

### 4. The first status poll dispatches a callback with no change in it

`poll_subscription_matched` and `poll_requested_incompatible_qos`
(`crates/dcps/src/subscriber.rs`) hold the previous value in an
`AtomicI64` initialised to `-1`. The first `drive_listeners()` compares
`-1` against `0`, finds them unequal, and dispatches — with
`current_count: 0, current_count_change: 0`. DDS 1.4 §2.2.4.1 has a
listener invoked when the status *changes*.

For this suite that is not cosmetic: `on_subscription_matched` is read
as "these two types matched", and 542 of the 625 pairs are expected not
to match. **Fixed in the runtime:** the four polls (`subscription_matched`,
`requested_incompatible_qos`, `publication_matched`,
`offered_incompatible_qos`) take `-1` as a baseline of 0, as
`poll_sample_rejected` and `poll_liveliness_changed` already did, and the
client's `is_real_change` filter is gone.

## What was and was not run

**Run, on this machine (macOS, loopback):**

- `cargo test -p zerodds-omg-xtypes-main` — 29 tests green (22 unit,
  7 in `tests/compatibility_matrix.rs`).
- `cargo clippy -p zerodds-omg-xtypes-main --tests` — clean.
- `cargo fmt -p zerodds-omg-xtypes-main -- --check` — clean.
- The 25 same-type live pairs, one process per side: 25 of 25 delivered.
- Two mismatched-type live pairs (`Shape1Extensible` writer /
  `Shape2Extensible` reader): no match, `on_inconsistent_topic`, no
  sample — which is finding 1.

**Not run:** the suite's own 625 pairs, end to end. There is nothing to
run them with — `dds-xtypes` ships no harness, only `ShapeType_main.cxx`
and the spreadsheet, and driving 625 process pairs is a script nobody
has written yet on either side. **No pass rate against the suite is
claimed.** The 59-of-625 above is the assignability decision measured
directly, not a count of runs; the live numbers above are the 27 pairs
that were actually started as processes.

**Not run here either:** anything on Linux. `crates/dcps`'s Linux-gated
paths are invisible on macOS, and the same-type diagonal should be
re-measured on the Linux test host before it is quoted anywhere outside this file.

## Build recipe — static Linux, glibc ≤ 2.31

The suite's peers are Linux binaries and the vendors' released clients
are built for old glibc. This workspace's release pipeline already
produces that kind of portable binary, and it is reused as-is:

```bash
rustup target add x86_64-unknown-linux-musl
sudo apt-get install -y --no-install-recommends musl-tools

cargo build --release --target x86_64-unknown-linux-musl \
    -p zerodds-omg-xtypes-main

file target/x86_64-unknown-linux-musl/release/xtypes_main
ldd  target/x86_64-unknown-linux-musl/release/xtypes_main   # "not a dynamic executable"
```

## Phase-2 (not done here — Sandra's call)

- Company/product name for an OMG submission, and the binary name that
  follows from it.
- `CLA/CLA_<CompanyName>.md` from the upstream `CLA/` directory, and the
  CLA PR (per that repo's `CONTRIBUTING.md`).
- The source PR adding a vendor directory to `github.com/omg-dds/dds-xtypes`.
- A driver for the 625 pairs — the suite has none, and writing one is a
  contribution to the suite rather than to this crate.
- Downloading any vendor's `dds-xtypes` binary for a real cross-vendor
  run. Binary download requires Sandra's permission.
