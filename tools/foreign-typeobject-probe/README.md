# `zerodds-foreign-typeobject-probe`

Reads a foreign vendor's samples through the `TypeObject` that vendor
announced: the writer's `TypeInformation` from SEDP, its complete
`TypeObject` fetched from the peer's TypeLookup service (`getTypes` and
`getTypeDependencies`, DDS-XTypes 1.3 §7.6.3.3), a `DynamicType` built from
it, and the peer's own samples decoded with it.

Part of [**ZeroDDS**](../../README.md).

```text
zerodds-foreign-typeobject-probe --domain 47 --topic ProbeTopic \
    --type-name probe::Sample --count 3 [--timeout-s 30] [--dialect rti]
```

- `run.sh` starts a Cyclone DDS 11.0.1 writer through its Python binding
  (`cyclone_pub.py`) and compares the decoded values with what it wrote.
- `xtypes_suite_sweep.py` takes every publisher variant of the OMG
  `dds-xtypes` v2 suite, runs it with each vendor's `test_main`, and
  compares one decoded sample with the suite's data file.
- `fastdds_pub.cpp` stands in for the `test_main` the suite has no Fast DDS
  build of: it builds the variant's type with Fast DDS's own IDL parser
  (`DynamicTypeBuilderFactory::create_type_w_uri`), so the `TypeObject` on
  the wire is the one Fast DDS derives, and fills the sample from the data
  file with Fast DDS's JSON reader (`json_deserialize`, OMG format).
  `xtypes_suite_sweep.py` runs it for the vendor name `fastdds`. Build:

  ```text
  g++ -std=c++17 -O1 -o fastdds_pub fastdds_pub.cpp -I$FASTDDS_HOME/include \
      -L$FASTDDS_HOME/lib -Wl,--disable-new-dtags,-rpath,$FASTDDS_HOME/lib -lfastdds -lfastcdr
  ```

  `--disable-new-dtags` makes the run path reach `libfastcdr`, which
  `libfastdds` loads without a run path of its own.

## Measured

`xtypes_suite_sweep.py`, 102 publisher variants per vendor and encoding, the
reader in the writer's dialect. "same" is the decoded sample equal to the
suite's data file; two variants run with `--disable-type-info` and are not
counted.

| Vendor | XCDR2 same | XCDR1 same |
|---|---|---|
| CoreDX DDS 6.15.3 | 100 of 100 | 84 of 100 |
| Eclipse Cyclone DDS 11.0.1 | 100 of 100 | 93 of 100 |
| RTI Connext DDS 7.7.0 | 93 of 100 | 89 of 100 |
| OpenDDS 3.35.0-dev | 91 of 100 | — |
| Fast DDS 3.6.1 (`fastdds_pub`) | 57 of 100 | 57 of 100 |

What the rest is, by cause:

- No `TypeInformation` announced: RTI's `tryc_*` writers (7 under each
  encoding), Cyclone under XCDR1 (4), OpenDDS under XCDR1 (all).
- PL_CDR1 framing no dialect of this tree selects for that writer: a
  parameter length not rounded up to four (RTI 4, CoreDX 4, Cyclone 3) and
  `PID_LIST_END` without flags (CoreDX 12).
- OpenDDS writes a `long double` that does not decode as IEEE 754 binary128
  (4), and five variants delivered no sample in the time allowed (four
  `*_char8`, `struct_members_assignable_1`).
- Fast DDS published 57 variants under each encoding, and all 57 decoded
  equal. The other 43 never reached the wire: its IDL parser built no type
  for 13 (six with "Empty operands stack while parsing string size" in
  `sequences.idl`), and for 30 it built a type after reporting "IDL parsing
  failed" that its JSON reader then could not fill from the data file
  (`appendable_enum`: the literal `VAL1` not found).

In no variant did a vendor's `TypeObject` describe its data differently
from what the data held.
