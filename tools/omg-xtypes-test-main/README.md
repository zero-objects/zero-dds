# `zerodds-omg-xtypes-test-main`

The ZeroDDS client for the OMG `dds-xtypes` interoperability test suite,
**version 2** — the suite that actually runs.

## Two suites with one name

`github.com/omg-dds/dds-xtypes` carries two different programs, and only
one of them has a harness.

| | `master` (`f6acbe08`) | `feature/xml-driver-enhanced` (`fb74a5d`) |
|---|---|---|
| client source | `srcCxx/ShapeType_main.cxx` | `src/cxx/test_main.cxx`, `src/c/test_main.c`, `src/rs/{DustDDS,HDDS}` |
| types | 25, compiled in | 128, in ten files read at run time |
| command line | `-pub`, `-type Shape1Final` | `-P`, `-y Test::struct_f1`, `--type-folder types` |
| the expectation | `compatibility_matrix.xlsx`, a spreadsheet a person reads | `test_suite.py`, 335 cases |
| the harness | none | `interoperability_report.py`, `pexpect`, JUnit output |
| released binaries | none | `v2.0.2025-wip`, `*_test_main_linux` |
| our client | `tools/omg-xtypes-main` | **this crate** |

`master`'s own workflow (`.github/workflows/1_run_interoperability_tests.yml`)
invokes `interoperability_report.py`, `requirements.txt` and
`generate_xlsx_report.py` — three files that do not exist on `master`. It
cannot run there. On the v2 branch they all exist and it does.

## What the suite asks

335 cases, each one publisher and one subscriber on the topic `test`,
each side naming its own type out of the same type-definition file. The
expectations across all 670 entity slots:

| expected | count |
|---|---|
| `OK` — the pair matches and the sample arrives | 219 |
| `INCONSISTENT_TOPIC` — the types are not assignable | 430 |
| `DATA_NOT_RECEIVED` | 21 |

Every case checks the sample with `tsf.data_is_correct`, which waits for
the line `Received sample is the same as loaded`.

So the suite is one question asked 335 times: **do the two steps of
XTypes 1.3 §7.6.3.4.2 give the answer the clause gives?** That decision
lives in `crates/dcps/src/runtime.rs::decide_type_consistency` and
`crates/types/src/assignability.rs`, and nothing in this crate may
answer it.

## How a run gets its type

The type is not known until the run starts, and cannot be compiled in:
`Test::struct_x1` is one struct in `type_consistency` and a different one
in `type_consistency_force_type_val`, which is what that pair of files is
for. `src/model.rs` reads the file the command line names, through
`zerodds-idl-compose` — the same load path a `build.rs` and
`zerodds-idlc` take — and builds the `CompleteTypeObject` and
`TypeIdentifier` from the model with `zerodds_types::from_ast`.

`src/carrier.rs` presents that to DDS. The sample octets come from
`zerodds_wire_codec::encode` against the same model and the suite's own
`data/json/<name>.json`; `Sample::encode` hands them to the writer
unchanged. Nothing here reframes anything — the DHEADER of an
`@appendable` type and the EMHEADERs of a `@mutable` one are inside the
octets, written by the plan a compiled-in type's generated codec is
compiled from.

`src/listeners.rs` carries every line `interoperability_report.py`
matches on, each with the `expect` index it satisfies.

## The one production change this needed

`DdsType` states a type's name, identity, extensibility and keyedness in
associated **constants**. A type that arrives at run time has none, so no
such type could be a DDS topic type at all. `crates/dcps/src/dds_type.rs`
now also has five accessors — `type_name`, `type_identifier`,
`extensibility`, `has_key`, `key_holder_max_size` — whose defaults return
the constants, and the DCPS entities read those. Every existing
implementation is unchanged. `DomainParticipant::register_type_object`
was added beside it so a caller holding a model can register the types an
aggregate names by identifier (XTypes 1.3 §7.3.4.6).

## Running it

```bash
cargo build -p zerodds-omg-xtypes-test-main

# in a checkout of the suite's feature/xml-driver-enhanced branch:
python3 -m venv .venv && .venv/bin/pip install -r requirements.txt
.venv/bin/python interoperability_report.py \
    --pub-exe .../target/debug/zerodds_test_main \
    --sub-exe .../target/debug/zerodds_test_main \
    -o report.xml
```

The type and data folders are resolved relative to the working
directory, so the harness has to be started from the suite checkout —
that is what the suite's own `--type-folder types` assumes.

## What does not run yet

Six of the ten type-definition files cannot be read through the IDL
front end, and 102 of the 335 cases use them:

* `arrays.idl`, `sequences.idl`, `struct_names.idl` each declare two
  enumerations in module `Test` that share the enumerator names `VAL0`,
  `VAL1`, `VAL2`. IDL 4.2 §7.4.15 puts an enumerator into the scope
  containing the `enum`, so the second declaration is a duplicate name
  and the file is not valid IDL. The XML beside it has no such rule and
  is what the suite's own clients read.
* `unions.idl` declares a member `uint32 DeFault;`. IDL 4.2 §7.2.3
  refuses an identifier that differs from a keyword only in case.
* `try_construct`, `unions_bitmask` and `unions_key_discriminator` exist
  **only** as XML — there is no IDL for them at all.

Reading the XML instead needs a DDS-XML type description to become a
`zerodds_ast::Ast`, which this project does not have yet:
`crates/xml` builds its own `TypeLibrary` and a `MinimalTypeObject` from
it, not a model. That is the next piece of work and it is production
work, not client work.

## Empty ACKNACK of the vendors

Fast DDS (and, as recorded, OpenDDS and RTI Connext) send the ACKNACK of a reader that
holds nothing with `bitmapBase` 0 and `numBits` 0, which DDSI-RTPS 2.5 §8.3.5.5 makes
invalid. ZeroDDS by default drops the rest of such a datagram (§8.3.4.1 rule 6), and with
it the SEDP data that follows in it (measured on the Linux test host, sec-diag16/17: every red
zerodds->fastdds cell had rejections with `left_out` > 0, every green cell none; with the
setting all 24 cells green and no rejection). `run_configs.sh` sets
`ZERODDS_WIRE_ACCEPT_EMPTY_ACKNACK_BASE_ZERO=1` (setting
`wire.accept_empty_acknack_base_zero`) for the ZeroDDS processes of the vendor pairs, in
one place (the block at the top). The default of ZeroDDS stays the reading of the spec.

## PL_CDR1 parameters with the rounded length

Cyclone DDS (extended header) and RTI Connext (short header) write the length of a PL_CDR1
parameter rounded up to four, the form of DDSI-RTPS 2.5 §9.4.2.11 (recorded rows;
`ast-layout` `Dialect::Cyclone` and `Dialect::Rti`, `pads_declared_length`). XTypes 1.3
§7.4.1.2.2 has the exact length, and the default reader of ZeroDDS refuses the other form.
`run_configs.sh` sets `ZERODDS_WIRE_PL_CDR1_READING=tolerant` (setting `wire.pl_cdr1_reading`) for every run (it meets both vendors),
in the same place as the empty-ACKNACK setting. Fast DDS and OpenDDS write the exact
length. The default of ZeroDDS stays the reading of the spec.

The TypeLookup samples of ZeroDDS are CDR2_LE (XCDR2) by default (XTypes 1.4 Beta 1
§7.6.3.3.3), the form Cyclone DDS 11.0.1, OpenDDS 3.34.0 and RTI Connext 7.7.0 write and the
only one they read (`internal/release/evidence/rc7-typelookup-encapsulation`).
`run_configs.sh` sets nothing for it.
