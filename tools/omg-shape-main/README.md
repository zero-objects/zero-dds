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
`zerodds-1.0.0-rc.7_shape_main_linux` — until that name is picked.

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
| `-v [e|d]` | `Options::debug_verbosity` -> the `debugln!` diagnostics in `src/main.rs` | full (`d` prints per-step diagnostics on **stderr**, `e` and the absent flag print errors only; stdout is byte-identical either way, so the harness's `pexpect` matching is unaffected) |
| `-P` / `-S` | pub/sub dispatch | full |
| `-d <int>` | `DomainParticipantFactory::create_participant` | full |
| `-b` / `-r` | `ReliabilityQosPolicy` | full (QoS *set* correctly — see "Known runtime quirks": the live SEDP matcher does not *enforce* RELIABILITY compatibility) |
| `-k <depth>` | `HistoryQosPolicy` (0 → KeepAll) | full |
| `-f <ms>` | `DeadlineQosPolicy` | full |
| `-s <strength>` | `OwnershipQosPolicy` + `OwnershipStrengthQosPolicy` | full |
| `-t <topic_name>` | `Topic<ShapeType>` | full |
| `-c <color>` | publish-side instance key; subscribe-side `DataReader::with_filter` on `ShapeType.color` | full (an explicit `--cft` wins over the implicit color filter) |
| `-p <partition>` | `PartitionQosPolicy` | full (single name; the README grammar allows one partition string per invocation, matching upstream) |
| `-D [v|l|t|p]` | `DurabilityQosPolicy` | full |
| `-x [1|2]` | `DataWriterQos`/`DataReaderQos.data_representation` | full |
| `-w` | prints publisher sample lines | full |
| `-z <int>` | shapesize (0 → grows) | full |
| `--size-modulo <uint>` | with `-z 0`, wraps the increasing shapesize sequence modulo N | full — **not in the README's own flag table**; some official cases (`Test_Cft_1`) pass it, so accepting it is what lets their client start. `0` (the default) means no wrap |
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
| `--coherent` (`-T`) | `PresentationQosPolicy.coherent_access` + `Publisher::begin_coherent_changes`/`end_coherent_changes` | wired end to end, with one named limit — the writer brackets each set, every member carries `PID_COHERENT_SET` (0x0056) in its inline QoS, and `end_coherent_changes` sends the end-of-coherent-set sample (DDSI-RTPS 2.5 §8.7.6). A `coherent_access` reader above INSTANCE scope sees the set only once it is closed. With `--access-scope g` the subscriber additionally wraps each read tick in `Subscriber::begin_access()`/`end_access()` and prints `"Reading coherent sets"`. **One gap:** a set member larger than `fragment_size` is refused (`WireError::UnsupportedFeature`) instead of being sent unmarked — `DataFragSubmessage` carries no inline-QoS list, so `--coherent` together with a large `--additional-payload-size` does not work |
| `--ordered` (`-O`) | `PresentationQosPolicy.ordered_access` | full — with an `access_scope` above INSTANCE the reader delivers in write order **across** instances (DDS 1.4 §2.2.3.6): `DataReader::{read,take}_next_instance` resolve to the topic-ordered collection instead of cycling instance by instance, and the subscribe loop takes the plain `read()`/`take()` path. Prints `"Reading with ordered access"` |
| `--coherent-sample-count <int>` (`-H`) | number of write iterations per `begin_coherent_changes`/`end_coherent_changes` bracket | full — one iteration writes one sample per DataWriter+instance, so `n` is the README's "samples per coherent set, per DataWriter+instance". Unset (`0`) forms one-sample sets. Only meaningful together with `--coherent` |
| `--additional-payload-size <bytes>` (`-B`) | `ShapeType::with_payload` — the `sequence<uint8> additional_payload_size` member of the OMG `srcCxx/shape.idl` | full — the writer fills the sequence with that many filler octets, so the length on the wire is the requested byte count |
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

## Local self-interop validation result

Full run: `interoperability_report.py -P ./shape_main -S ./shape_main`
against the live `test_suite.py` (105 test cases fetched read-only from
`raw.githubusercontent.com/omg-dds/dds-rtps/master/*.py` — no clone, no
push).

**`tests="105" errors="0" failures="0"` — 105/105.** Measured on
the Linux test host (Linux, 16 cores, load average 4.5), `time="1036.044283"`, from
the commit on a verification branch that carries the fixes below.

Measurement history, same command, same harness:

| Date | Host | Result |
|---|---|---|
| 2026-07-28 | Linux test host | `failures="36"` — 69/105 |
| 2026-09-01 | Linux test host | `failures="28"` — 77/105, before the PRESENTATION announcement fix |
| 2026-09-01 | Linux test host | `failures="20"` — 85/105, after it |
| 2026-09-04 | Linux test host | `failures="0"` — **105/105** |

The twenty that were left after 2026-09-01 —
`Test_Lifespan_0`–`_7`, `Test_CoherentSets_10,11,12,19,20,21`,
`Test_FinalInstanceState_0,1,2`, `Test_OrderedAccess_10,17` and
`Test_LargeData_0` — are all OK, and so is `Test_Durability_17`, which
the 2026-07-28 row had listed as open.

### The host matters, and it is not the macOS one

The same binary, on a Mac carrying a load average between 24 and 87,
fails cases it passes on the Linux test host: at load 39 the whole `Lifespan`
family reports `DATA_NOT_CORRECT`, and `Test_Reliability_4/5` and
`Test_History_0/1` fail there **also at the commit before this work**
(`git checkout 2222a0a7e43 -- crates/… tools/omg-shape-main/src`, rebuilt,
measured). Measured: at the default `--write-period` of 33 ms a RELIABLE
reader on that host loses 405 of 1146 samples, at `--write-period 200`
none of 264. A macOS host also had a foreign DDS participant holding
`*:7400`/`*:7410` on domain 0 for three days.

The measurement host for this suite is the Linux test host.

## Known runtime quirks (found during local validation)

Eleven genuine gaps in the **shared** `crates/dcps`/`crates/rtps`/
`crates/qos` runtime were found while validating this binary. The first
four were found in the 2026-07-28 pass and fixed later; entries 5 to 11
come from the 2026-09-04 pass and were fixed in it. All eleven are
fixed; the 105/105 above is measured against them.

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
   the freshly-ingested samples. **Fixed in the shared runtime since this
   note was written:** `crates/dcps/src/subscriber.rs`'s
   `read_next_instance`/`take_next_instance` now call
   `ingest_into_cache()` unconditionally, ahead of the
   `next_handle_after` lookup, so a fresh reader discovers its instances
   on the first call. The app-level workaround in `next_instance_batch`
   is therefore redundant and costs one non-consuming read per poll tick;
   it stays until the suite has been run again against the fixed
   runtime.
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

   **Fixed in the shared runtime since this note was written.** Both
   match directions (`wire_writer_to_remote_reader` /
   `wire_reader_to_remote_writer`) now carry a
   `bump(slot, qid::RELIABILITY)` and a `bump(slot, qid::PRESENTATION)`
   before the proxy is built. PRESENTATION additionally needed its
   announcement fixed: §2.2.3.6 puts the policy on the
   Publisher/Subscriber, and `create_datawriter`/`create_datareader`
   announced the (defaulted) per-endpoint field instead of the group's
   value, so both sides announced INSTANCE and compared equal. The
   endpoint now announces its group's value via
   `crates/dcps/src/qos.rs::effective_presentation`. Measured against
   the suite: `Test_OrderedAccess_1,2,5,9` and
   `Test_CoherentSets_1,2,5,9` went from ERROR to OK,
   `Test_Reliability_1` is OK.
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

   **Fixed in the shared runtime since this note was written.** A
   disjoint offer/accept set now rejects the match on both directions
   before the proxy is constructed. `Test_DataRepresentation_1`/`_2`
   are OK.
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

   **Fixed in the shared runtime since this note was written.** Both
   match directions leave the path with a bare `return` on a partition
   mismatch — no `bump()`, no listener dispatch. The endpoint also
   announces its Publisher's/Subscriber's partition list
   (`crates/dcps/src/qos.rs::effective_partition`), which §2.2.3.13
   puts at group level. `Test_Partition_1`/`_2` are OK.

5. **A coherent-set member lost its `PID_COHERENT_SET` on two of the
   three send paths.** DDSI-RTPS 2.5 §8.7.6 requires the marker on every
   DATA of the set. `zerodds-rtps`'s `reliable_writer::append_data` wrote
   a bare 20-octet header and no inline QoS at all; that function serves
   the DATA+piggyback-HEARTBEAT path (`write_with_heartbeat`, which the
   DCPS layer takes whenever a heartbeat is due) and the ACKNACK resend
   in `tick`. Measured on `Test_CoherentSets_11`: three of every four
   members arrived marked and the fourth unmarked, and the reader closed
   the set around the members it recognised — a silently wrong cut.
   **Fixed**: such a change leaves the zero-copy fast path and goes out
   as a full `DataSubmessage`.

6. **`access_scope = INSTANCE` switched coherency off instead of
   bounding it.** DDS 1.4 §2.2.3.6: "order and coherent changes apply to
   each instance separately" — the members of a set are still presented
   together, the guarantee simply does not span two instances.
   `DataReader::coherent_access_active` returned `false` for that scope.
   **Fixed.**

7. **`ordered_access` with `access_scope = INSTANCE` presented in
   reception order.** Same clause: at INSTANCE scope the order holds per
   instance, so the subscriber groups its samples by instance — that
   grouping is what `Test_OrderedAccess_10`'s two subscribers are
   compared on. **Fixed** in `take_filtered`/`read_filtered`.

8. **A dispose/unregister marker could not be materialized for a keyed
   type.** DDS 1.4 §2.2.2.5.1.13 declares the key fields valid on a
   sample with `valid_data == false`. The marker carries no body, so
   `decode_or_keyholder` decoded the `InstanceTracker`'s KeyHolder — a
   PLAIN_CDR2-BE stream of the key members alone — with `T::decode`.
   For `ShapeType` that is `00 00 00 05 "BLUE\0"` read as XCDR2-LE, and
   every `take`/`read` of a marker returned `WireError`. **Fixed**: the
   reader keeps the instance's last body and materializes the marker
   from it.

9. **An instance never became `NOT_ALIVE_NO_WRITERS` when its writer
   simply went away.** §2.2.2.5.1 ties that state to the DataReader
   detecting that no live DataWriter writes the instance; a publisher
   that ends without unregistering sends no marker at all. **Fixed**:
   the `InstanceTracker` keeps the writer set per instance and
   `remove_remote_writer` hands the departure to the reader.

10. **LIFESPAN expired only in the writer's HistoryCache.** §2.2.3.16
    also says "Once the sample expires, the data will be removed from the
    DataReader caches". **Fixed**: the writer's `PID_LIFESPAN` (SEDP) is
    kept per remote writer and the reader drops a sample once its source
    timestamp plus that duration has passed.

11. **`DdsType` could write a key holder and not read one back.**
    XTypes 1.3 §7.6.8 step 4 writes the `FooKeyHolder` as PLAIN_CDR2,
    big-endian, `MAXALIGN` four, key members only, with no encapsulation
    header, no DHEADER and no member headers — a stream `Self::decode`
    cannot read. The trait had `encode_key_holder_be` and no counterpart,
    so the runtime fed the holder to `decode`; for `ShapeType` that is
    `00 00 00 05 "BLUE\0"` read as XCDR2-LE, and every `take`/`read` of a
    dispose/unregister marker returned `WireError` (entry 8 above was the
    symptom). **Fixed**: `DdsType::decode_key_holder_be` is additive with
    a refusing default, the generator emits `key_from_holder_be` from the
    same plan the writer is built from, and the hand-written keyed types
    carry it. Two workarounds went away with it — the reader's
    last-body-per-instance cache, and `crates/py/src/ffi.rs`'s
    `decode_endian`, which told holder from sample by length.

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
zerodds-1.0.0-rc.7_shape_main_linux.zip
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

## Empty ACKNACK of the vendors

Fast DDS (and, as recorded, OpenDDS and RTI Connext) send the ACKNACK of a reader that
holds nothing with `bitmapBase` 0 and `numBits` 0, which DDSI-RTPS 2.5 §8.3.5.5 makes
invalid. ZeroDDS by default drops the rest of such a datagram (§8.3.4.1 rule 6), and with
it the SEDP data that follows in it (measured on the Linux test host, sec-diag16/17: every red
zerodds->fastdds cell had rejections with `left_out` > 0, every green cell none; with the
setting all 24 cells green and no rejection). `shape_main` sets
`wire.accept_empty_acknack_base_zero` itself (`suite_settings` in `src/main.rs`), unless
the environment names `ZERODDS_WIRE_ACCEPT_EMPTY_ACKNACK_BASE_ZERO`, so the official matrix,
which starts the binary without `run_vendors.sh`, runs with it too. The default of ZeroDDS
stays the reading of the spec.

## PL_CDR1 parameters with the rounded length

Cyclone DDS (extended header) and RTI Connext (short header) write the length of a PL_CDR1
parameter rounded up to four, the form of DDSI-RTPS 2.5 §9.4.2.11 (recorded rows;
`ast-layout` `Dialect::Cyclone` and `Dialect::Rti`, `pads_declared_length`). XTypes 1.3
§7.4.1.2.2 has the exact length, and the default reader of ZeroDDS refuses the other form.
`shape_main` sets `wire.pl_cdr1_reading=tolerant` itself, in the same place and on the same
terms (`ZERODDS_WIRE_PL_CDR1_READING`). Fast DDS and OpenDDS write the exact length. The
default of ZeroDDS stays the reading of the spec.

The TypeLookup samples of ZeroDDS are CDR2_LE (XCDR2) by default (XTypes 1.4 Beta 1
§7.6.3.3.3), the form Cyclone DDS 11.0.1, OpenDDS 3.34.0 and RTI Connext 7.7.0 write and the
only one they read (`internal/release/evidence/rc7-typelookup-encapsulation`). `shape_main`
sets nothing for it.

## Instances of a deleted writer and announced payload sizes

`Test_FinalInstanceState_2` expects NOT_ALIVE_NO_WRITERS at the reader when the publisher
ends without unregistering or disposing. Connext 7.7.0, Fast DDS 3.6.1, CoreDX 6.15.3 and
HDDS 1.4.6 as the publisher end there; a deleted ZeroDDS DataWriter unregisters and, with
`autodispose_unregistered_instances`, disposes its instances by default (DDS 1.4 §2.2.2.4.1.6,
§2.2.3.21; dds-rtps issue #92). OpenDDS 3.35.0-dev and InterCOM 4.3.1 dispose and unregister
each instance in one marker when their writer is deleted, which a ZeroDDS reader takes as
both by default (DDSI-RTPS 2.5 §9.6.3.9; measured 20 of 20 and 5 of 5 runs at
NOT_ALIVE_DISPOSED). Fast DDS 3.6.1 announces `PID_TYPE_MAX_SIZE_SERIALIZED` 160 for
ShapeType and sends samples of 100 036 octets (`Test_LargeData_0`), which a ZeroDDS reader
refuses by default (DDSI-RTPS 2.5 §8.4.10.4). `shape_main` sets
`writer.unregister_on_delete=false`, `reader.dispose_with_unregister=false` and
`reader.honor_data_max_size_serialized=false` itself, each unless the environment names it
(`ZERODDS_WRITER_UNREGISTER_ON_DELETE`, `ZERODDS_READER_DISPOSE_WITH_UNREGISTER`,
`ZERODDS_READER_HONOR_DATA_MAX_SIZE_SERIALIZED`). The defaults of ZeroDDS stay the readings
of the specifications.

## Answers to HEARTBEAT and NACK

The suite configures its Connext reader to answer a HEARTBEAT at once and its Connext writer
to answer a NACK at once (`srcCxx/shape_configurator_rti_connext_dds.h`,
`max_heartbeat_response_delay` and `max_nack_response_delay` 0). The Connext reader keeps its
default `heartbeat_suppression_duration` of 62.5 ms (`DDS_RTPS_RELIABLE_READER_PROTOCOL_DEFAULT`
in `include/ndds/dds_c/dds_c_infrastructure.h` of Connext 7.7.0) and ignores a HEARTBEAT that
comes sooner after the previous one. `shape_main` sets the same for its readers and writers:
`reader.heartbeat_response_delay_ms` 0, a HEARTBEAT suppression of 62.5 ms and
`writer.nack_response_delay_ms` 0. Without the suppression a ZeroDDS reader answered every
HEARTBEAT dust_dds 0.16.1 puts between the fragments of a sample with a request for every
sample in flight (`Test_LargeData_0`: 1.5 GB on the wire for 180 samples, a Cyclone reader
26 MB for 199). The runtime's defaults stay those of DDSI-RTPS 2.5 §8.4.10.1 and §8.4.7.1.

## Key bounds of dust_dds

dust_dds 0.16.1 announces the key `color` of ShapeType as an unbounded string: its
`dust_dds_gen` compiles the suite's `string<128>` to a TypeObject with bound 0. A ZeroDDS
reader refuses such a writer by default under XTypes 1.3 Table 19, because its key member is
shorter than the writer's (event `type.inconsistent`; measured: dust_dds -> ZeroDDS 50 cases
`DATA_NOT_RECEIVED`). Connext, Cyclone, Fast DDS, OpenDDS, CoreDX, InterCOM and zzdds receive
from it in `Test_Domain_0` of the published run. `shape_main` sets `xtypes.key_member_bounds=ignored` itself,
unless the environment names `ZERODDS_KEY_MEMBER_BOUNDS`.

## Coherent and ordered access with other vendors

The group information of DDSI-RTPS 2.5 §8.7.5/§8.7.6 (`PID_GROUP_SEQ_NUM`,
`PID_WRITER_GROUP_INFO`, `PID_GROUP_COHERENT_SET`, the GroupInfoFlag of HEARTBEAT and GAP)
is sent by no measured vendor. Connext 7.7.0, HDDS 1.4.8 and zzdds 0.3.2 as subscribers read
the GROUP-scope sets of HDDS and zzdds, which send each writer's set as under TOPIC; with
the group information `Test_CoherentSets_12`, `_18` and `_21` from ZeroDDS were red against
them. `shape_main` gives the participants of these three vendors that form
(`writer.group_info_on_wire`, per vendor through `DcpsRuntime::set_vendor_rtps_behaviour`),
unless the environment names `ZERODDS_WRITER_GROUP_INFO_ON_WIRE`; every other peer gets the
specification's form.

The harness starts the publisher a second before the subscriber. Connext 7.7.0's GROUP
subscriber delivers nothing of a writer whose samples start past SN 1 for it; the suite's
zzdds client waits for a match before it writes (`srcZig/shape_main.zig`), and CoreDX, which
does not, fails `Test_CoherentSets_8`, `_12` and `Test_OrderedAccess_8` against Connext in
the published run as ZeroDDS did. Under GROUP access scope a coherent or ordered `shape_main`
publisher writes its first sample once every writer has a matched reader.

A zzdds 0.3.2 reader takes a HEARTBEAT as the end of a writer's coherent set until it has
seen that writer's end-of-coherent-set DATA (`src/dcps/reader.zig`, `onHeartbeatCb`). A
coherent `shape_main` publisher therefore writes the iterations of one set without a pause and
paces after the set; order and sizes are those of the reference client, the rate one
iteration per write period.

An InterCOM 4.3.1 reader does not take the end-of-coherent-set DATA of DDSI-RTPS 2.5
Table 9.22 (no data, no in-line QoS): it requests the number again and again and delivers
nothing after it. HDDS answers that request with a GAP, which InterCOM takes; InterCOM itself
sends no coherent set. `shape_main` gives InterCOM participants the end of a set as a GAP
(`zerodds_rtps::dialect::EndOfSetForm`).
