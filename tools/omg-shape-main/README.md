# `zerodds-omg-shape-main`

`shape_main` reference client for the OMG `dds-rtps` interoperability test
suite (`github.com/omg-dds/dds-rtps`). **BUILD + LOCAL-VALIDATE phase
artifact.** Nothing from this crate has been submitted upstream — no CLA
PR, no source PR, no release asset, no push to `github.com/omg-dds`. See
"Phase-2 (not done here)" below for exactly what remains.

The public product/version name for an eventual upstream submission is
**undecided** (Sandra's call). Local builds use the crate/binary name
`shape_main`; the OMG-required release-asset filename convention
(`<product_name_and_version>_shape_main_linux`) is a placeholder —
`zerodds-1.0.0-rc.6_shape_main_linux` — until that name is picked.

This is a CLI + protocol shim over ZeroDDS's **existing** DCPS API
(`crates/dcps`) and QoS machinery (`crates/qos`, `crates/sql-filter`). No
new wire/CDR/QoS code was written for this binary.

## Flag coverage table

Flags and their exact spellings are transcribed from the live
`github.com/omg-dds/dds-rtps` `README.md` "Shape Application parameters"
table, cross-checked against both committed Rust reference clients
(`srcRs/DustDDS/src/main.rs`, `srcRs/HDDS/src/main.rs`).

| Flag | Wired to | Status |
|---|---|---|
| `-h`/`--help` | clap auto | full |
| `-v [e|d]` | `Options::verbosity` | **stub** — accepted, not consumed (no local verbosity knob) |
| `-P` / `-S` | pub/sub dispatch | full |
| `-d <int>` | `DomainParticipantFactory::create_participant` | full |
| `-b` / `-r` | `ReliabilityQosPolicy` | full (QoS *set* correctly — see "Known runtime quirks": the live SEDP matcher does not *enforce* RELIABILITY compatibility) |
| `-k <depth>` | `HistoryQosPolicy` (0 → KeepAll) | full |
| `-f <ms>` | `DeadlineQosPolicy` | full |
| `-s <strength>` | `OwnershipQosPolicy` + `OwnershipStrengthQosPolicy` | full |
| `-t <topic_name>` | `Topic<ShapeType>` | full |
| `-c <color>` | instance key / subscribe-side color | full |
| `-p <partition>` | `PartitionQosPolicy` | full (single name; the README grammar allows one partition string per invocation, matching upstream) |
| `-D [v|l|t|p]` | `DurabilityQosPolicy` | full |
| `-x [1|2]` | `DataWriterQos`/`DataReaderQos.data_representation` | full |
| `-w` | prints publisher sample lines | full |
| `-z <int>` | shapesize (0 → grows) | full |
| `-R` | `read()` vs `take()` variant selection | full |
| `--write-period <ms>` | publish loop sleep | full |
| `--read-period <ms>` | subscribe loop sleep | full |
| `--time-filter <ms>` (`-i`) | `TimeBasedFilterQosPolicy` | full (QoS set; live enforcement not independently re-verified here) |
| `--lifespan <ms>` (`-l`) | `LifespanQosPolicy` | full |
| `--num-iterations <int>` (`-n`) | main-loop iteration cap | full |
| `--num-instances <int>` (`-I`) | `color`, `color1`, `color2`, ... via `register_instance` | full |
| `--num-topics <int>` (`-E`) | `Square`, `Square1`, `Square2`, ... one writer/reader each | full |
| `--final-instance-state [u|d]` (`-M`) | `unregister_instance` / `dispose` after the write loop | full |
| `--access-scope [i|t|g]` (`-C`) | `PresentationQosPolicy.access_scope` | full (QoS set; GROUP+coherent additionally drives `Subscriber::begin_access`/`end_access` — see below) |
| `--coherent` (`-T`) | `PresentationQosPolicy.coherent_access` | **partial** — QoS policy set and negotiated; with `--access-scope g` the subscriber wraps each read tick in `Subscriber::begin_access()`/`end_access()` and prints `"Reading coherent sets"`. No writer-side transactional grouping (`Publisher` has no public `begin_coherent_changes`/`end_coherent_changes` in this crate version) |
| `--ordered` (`-O`) | `PresentationQosPolicy.ordered_access` | **partial** — QoS set; prints `"Reading with ordered access"` as a marker. No additional ordering is enforced beyond ZeroDDS's existing per-instance `DESTINATION_ORDER` QoS |
| `--coherent-sample-count <int>` (`-H`) | parsed, stored | **stub** — no writer-side transactional grouping API to attach it to (see `--coherent`) |
| `--additional-payload-size <bytes>` (`-B`) | parsed, stored | **stub** — the shared `ShapeType` wire type (`crates/dcps/src/interop.rs`, deliberately not touched by this crate) has no `additional_payload_size` field |
| `--take-read` (`-K`) | `take()`/`read()` instead of `take_next_instance()`/`read_next_instance()` | full |
| `--periodic-announcement <ms>` | `RuntimeConfig.spdp_period` via `create_participant_with_config` | full |
| `--cft <expr>` | `DataReader::with_filter` + `zerodds-sql-filter` | full — **vendor extension**, not in the README's own flag table (mirrors DustDDS/HDDS `--cft`); exists so the harness's `'failed to create content filtered topic'` alternative has something to trigger against |

## Exact stdout strings matched by the harness

Extracted from the live `interoperability_report.py` / `test_suite.py` /
`test_suite_functions.py` / `rtps_test_utilities.py` sources
(`github.com/omg-dds/dds-rtps`), cross-validated against
`srcRs/DustDDS/src/main.rs` and `srcRs/HDDS/src/main.rs`. Quoted
verbatim — do not paraphrase:

- `"Create topic:"` — publisher **and** subscriber, right after
  `create_topic`.
- `"Create writer for topic"` — publisher, **no** trailing colon.
- `"Create reader for topic:"` — subscriber, **with** trailing colon.
- `"failed to create content filtered topic"` — subscriber, on a
  malformed `--cft` expression (`ParseError`), printed **instead of**
  `"Create reader for topic:"`.
- `"on_publication_matched()"` — writer listener.
- `"on_offered_incompatible_qos"` — writer listener (harness matches the
  bare substring; we print the full `on_offered_incompatible_qos()`,
  which contains it).
- `"on_offered_deadline_missed()"` — writer listener (checked inside the
  `-w` sample-loop alternation).
- `"on_subscription_matched()"` — reader listener (not `pexpect`-checked
  by the core `interoperability_report.py` flow, printed for spec
  completeness, mirrors both Rust reference clients).
- `"on_requested_incompatible_qos()"` — reader listener.
- `"on_requested_deadline_missed()"` — reader listener.
- Sample line: `<topic> <color> <x> <y> [<shapesize>]`, matched by
  `rtps_test_utilities.py` `basic_check`'s
  `r'\w\s+\w+\s+[0-9]+ [0-9]+ \[([0-9]+)\]'`.
- `"Reading coherent sets"` / `"Reading with ordered access"` — markers
  matched by `test_suite_functions.py` (`re.search`, not a `pexpect`
  alternation).
- `NOT_ALIVE_DISPOSED_INSTANCE_STATE` / `NOT_ALIVE_NO_WRITERS_INSTANCE_STATE`
  — instance-lifecycle markers on a dispose/unregister sample
  (`SampleInfo.valid_data == false`), matched by
  `test_suite_functions.py`'s `NOT_ALIVE_*` regexes.

## Local self-interop validation result (codepit, 2026-07-28)

Full run: `interoperability_report.py -P ./shape_main -S ./shape_main`
against the live `test_suite.py` (105 test cases fetched read-only from
`raw.githubusercontent.com/omg-dds/dds-rtps/master/*.py` — no clone, no
push). JUnit: `tests="105" errors="0" failures="36" skipped="0"`.

**69/105 (65.7%) OK.** The 36 failures, categorized by confirmed root
cause:

| Cause | Test cases | Root cause |
|---|---|---|
| DATA_REPRESENTATION mismatch not rejected | `DataRepresentation_1`, `_2` | Shared-runtime gap — see below |
| RELIABILITY mismatch not rejected | `Reliability_1` | Shared-runtime gap — see below |
| PRESENTATION mismatch not rejected | `OrderedAccess_1,2,5,9,10,17`, `CoherentSets_1,2,5,9` | Shared-runtime gap — see below |
| PARTITION mismatch wrongly reported as `INCOMPATIBLE_QOS` instead of silent non-match | `Partition_1`, `_2` | Shared-runtime bug — see below (new) |
| No writer-side transactional coherent-set grouping | `CoherentSets_10,11,12,19,20,21` | Documented limitation (this crate — see `--coherent` above) |
| Missing subscribe-side `-c <color>` implicit filter | `Cft_0` | This crate — not implemented (new, see below) |
| Missing `--size-modulo <n>` publisher flag | `Cft_1` | This crate — flag not implemented, not in the flag table above because it was only discovered via this failure (new) |
| `--additional-payload-size` stub (documented above) | `LargeData_0` | This crate — documented stub |
| Unregister/dispose lifecycle vs. `test_unregistering_w_instances` | `FinalInstanceState_0,1,2` | Open — not root-caused within this validation pass |
| `-z 0` growth vs. LIFESPAN sample expiry | `Lifespan_0`–`_7` (all 8) | Open — `expire_by_lifespan` exists and is wired in `crates/dcps/src/runtime.rs` (~line 8607), so live LIFESPAN enforcement is *not* obviously absent; the mismatch (`DATA_NOT_CORRECT`/`DATA_NOT_RECEIVED`, "Samples read per instance: 50") was not root-caused further within this pass — flagged for follow-up |
| Late-joiner sample content mismatch | `Durability_17` | Open — not root-caused within this validation pass |

Two **new** gaps found by the full run, beyond the two already documented
below:

- **PARTITION mismatch triggers `on_offered_incompatible_qos`/
  `on_requested_incompatible_qos`, which is spec-wrong.** DDS 1.4
  §2.2.3.13: a PARTITION mismatch must *not* be treated as an RxO
  QoS-compatibility failure — the entities are simply not associated, no
  listener event fires at all (`READER_NOT_MATCHED`, not
  `INCOMPATIBLE_QOS`). `crates/dcps/src/runtime.rs`'s match/reject block
  calls `bump(slot, qid::PARTITION); return;` — the same code path used
  for the genuine RxO policies (DURABILITY, DEADLINE, LIVELINESS,
  OWNERSHIP) — which is the bug: PARTITION needs a silent `return;`
  with no `bump()`/no listener dispatch, not the shared
  incompatible-QoS path.
- **This binary is missing two features**, found only by running the
  real suite (not documented as stubs above because they were unknown
  until this run): (1) `-c <color>` on the **subscriber** side should
  apply an implicit filter to that color/key (`Test_Cft_0` publishes
  BLUE and RED, subscribes with `-c RED`, expects
  `RECEIVING_FROM_ONE`); (2) a `--size-modulo <n>` publisher flag (used
  together with `--cft "shapesize <= N"` in `Test_Cft_1`) is not in the
  README's own flag table and was not implemented — its exact semantics
  were not reverse-engineered within this validation pass.

Full logs: `/root/omg-shape-validate/full_run.log`,
`/root/omg-shape-validate/full_self_interop.xml` on codepit (not copied
into the repo — regenerate via the command above).

## Known runtime quirks (found during local validation)

Four genuine gaps in the **shared** `crates/dcps`/`crates/qos` runtime
were found while validating this binary — none were touched (out of
this crate's isolated scope), all four are called out here for a
separate fix:

1. **`take_next_instance`/`read_next_instance` never discover a new
   instance on a reader that has not yet called a plain
   `take`/`read`/`*_with_info`.** `DataReader::take_next_instance` /
   `read_next_instance` (`crates/dcps/src/subscriber.rs`) resolve the
   next instance via `InstanceTracker::next_handle_after` *before*
   draining the incoming-sample channel into the cache — that drain
   (`ingest_into_cache`) only happens inside `take_instance`/
   `read_instance`, which are never reached if `next_handle_after`
   already returns `None` on an empty tracker. Net effect: the README's
   *default* subscriber behavior (no `--take-read`) silently receives
   zero samples forever on a reader that starts fresh. Worked around at
   the application level in `next_instance_batch` (`src/main.rs`) with a
   throwaway non-consuming `read_with_info()` call per poll tick to force
   ingestion first — `read_with_info` doesn't drain the cache, so the
   following `take_next_instance`/`read_next_instance` call still sees
   the freshly-ingested samples. The real fix belongs in
   `crates/dcps/src/subscriber.rs`: call `ingest_into_cache()`
   unconditionally at the top of `take_next_instance`/
   `read_next_instance`, ahead of the `next_handle_after` lookup.
2. **`RELIABILITY` and `PRESENTATION` QoS compatibility are not enforced
   by the live SEDP endpoint matcher.** `crates/dcps/src/runtime.rs`'s
   writer-side match/reject block explicitly checks DURABILITY,
   DEADLINE, LIVELINESS, OWNERSHIP, PARTITION and
   TYPE_CONSISTENCY_ENFORCEMENT (each via a `bump(slot, qid::…); return;`
   on mismatch) — `qid::RELIABILITY` and `qid::PRESENTATION` appear only
   in the diagnostic name lookup (`qos_policy_id_name`), never as a
   `bump()` target anywhere in that file. Confirmed live: a
   `-b` (BEST_EFFORT) writer matched and successfully delivered samples
   to a `-r` (RELIABLE) reader — per DDS 1.4 §2.2.3.14.4/§2.2.3 Table
   this must be rejected (`offered.kind >= requested.kind`). Separately,
   `crates/qos/src/compatibility.rs` implements the correct per-policy
   `is_compatible_with` logic (including reliability) via
   `compute_compatibility`, but that function has no caller outside its
   own crate example (`crates/qos/examples/qos_check.rs`) — it is not
   wired into the live match path at all. This binary's own QoS mapping
   (`src/qos_map.rs`) sets `-b`/`-r` correctly on both sides; the gap is
   purely in the shared runtime's enforcement.
3. **DATA_REPRESENTATION no-overlap is a documented, intentional
   non-rejection**, not enforced despite `dr::negotiate` returning
   `None` for a disjoint offer/request. `crates/dcps/src/runtime.rs`,
   in the same writer-side match block (~line 5429), reads verbatim:
   `// No overlap → SEDP match spec violation. // We add the proxy
   anyway for best-effort compat; the wire-format default stays XCDR2.
   // A spec-strict caller should reject the match.` Confirmed live:
   `-x 1` (XCDR1-only) writer matched a `-x 2` (XCDR2-only) reader with
   zero representation overlap (`Test_DataRepresentation_1`/`_2` in the
   local validation run below, both expect `INCOMPATIBLE_QOS`).
4. **PARTITION mismatch is misrouted through the RxO
   incompatible-QoS path.** DDS 1.4 §2.2.3.13 is explicit that a
   PARTITION mismatch causes no error and no listener event — the
   Publisher/Subscriber simply do not associate. `crates/dcps/src/runtime.rs`
   calls `bump(slot, qid::PARTITION); return;` on a partition
   mismatch — the identical code path used for the genuine RxO
   policies (DURABILITY, DEADLINE, LIVELINESS, OWNERSHIP) — so it fires
   `on_offered_incompatible_qos`/`on_requested_incompatible_qos`
   exactly like a real QoS-compatibility failure. Confirmed live:
   `Test_Partition_1`/`_2` expect `READER_NOT_MATCHED`/silence, both
   produced `INCOMPATIBLE_QOS`.

## Build recipe — static Linux, glibc ≤ 2.31

The README's own build guidance: *"compile shape_main.cxx with your own
product using the GLIBC version 2.31 or older (eg Ubuntu 2004)"*. This
workspace's release pipeline (`.github/workflows/release.yml`,
`x86_64-unknown-linux-musl` target) already produces exactly that kind of
portable Linux binary — reused as-is:

```bash
rustup target add x86_64-unknown-linux-musl
# musl-tools provides the musl-gcc wrapper cc-rs needs for any C deps.
sudo apt-get install -y --no-install-recommends musl-tools

cargo build --release --target x86_64-unknown-linux-musl \
    -p zerodds-omg-shape-main

# Static, glibc-independent (verify no dynamic libc dependency):
file target/x86_64-unknown-linux-musl/release/shape_main
ldd  target/x86_64-unknown-linux-musl/release/shape_main   # "not a dynamic executable"
```

Release-asset naming per the README's convention (placeholder — final
product name is Sandra's call):

```
zerodds-1.0.0-rc.6_shape_main_linux.zip
```

An `x86_64-unknown-linux-gnu` build against Ubuntu 20.04's glibc 2.31 (in
a container) is the documented alternative if a musl target ever turns
out to be unacceptable to the harness — not needed here since the musl
static binary has no glibc-version constraint at all.

## Phase-2 (not done here — Sandra's call)

- Company/product name for the OMG submission.
- `CLA/CLA_<CompanyName>.md` — fill in from `CLA/CLA_TEMPLATE.md` in the
  upstream repo and open the CLA PR (per `CONTRIBUTING.md`).
- The actual source PR adding a new vendor directory (mirroring
  `srcRs/DustDDS`, `srcRs/HDDS`) with this binary's source.
- The release-asset upload (`<product>_shape_main_linux` zip) to whatever
  distribution channel the OMG process expects.
- Deciding which vendor `shape_main` release binaries to download for a
  real cross-vendor pairwise run (binary download requires Sandra's
  permission — see the interop-validation report for the exact list).
