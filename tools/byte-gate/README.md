# The byte gate

One IDL type, one fixed sample, and the bytes each vendor's own
serialiser produces for it — in both encoding versions and both byte
orders. Nothing in this project produces any of them.

The results are
[`crates/ast-layout/tests/fixtures/vendor_bytes.md`](../../crates/ast-layout/tests/fixtures/vendor_bytes.md)
and, as rows a program can read,
[`vendor_bytes.rows.tsv`](../../crates/ast-layout/tests/fixtures/vendor_bytes.rows.tsv)
beside it.

## Running it

```sh
tools/byte-gate/gate.sh --cases point            # one case, every vendor
tools/byte-gate/gate.sh --out regenerated.tsv    # all 52
tools/byte-gate/verify.sh                        # regenerate and compare
```

Twelve minutes on the Linux test host for all four vendors and eight for
the three the CI image carries — 69 cases across both case sets, both
byte orders, both encoding versions. A vendor that is not installed
is named on stderr with the number of rows it did not produce.

`verify.sh` is what the `interop-vendor-bytes` pipeline job runs, with
`--vendors cyclone,fastdds,opendds` — the three the pinned `ci-interop`
image carries. It **fails** when a regenerated row differs from the
recorded one. Re-recording is deliberate and explicit:

```sh
tools/byte-gate/verify.sh --update
```

A commit that does that has to say which rows moved and why.

## A case

```
cases/<name>/
    type.idl        the IDL
    type            the top-level type's generated C name
    type.cpp        optional: the C++ name where the mapping differs
    fill.inc        C statements assigning into `sample`   (Cyclone, RTI)
    fill.rti.inc    optional override where the two C APIs differ
    fill.cpp.inc    C++ setter calls                        (Fast DDS)
    fill.odds.inc   C++ assignments                         (OpenDDS)
```

No sample is zero unless zero is the point. Every scalar carries a value
that moves if an offset, a width or a byte order is wrong, and every
aggregate has a member after the construct under test so that trailing
padding is visible.

## What each vendor needs

`tools/interop-vendors.sh` resolves all of these and says what to set
when a toolchain is somewhere else.

| | |
|---|---|
| Cyclone | `$CYCLONE_HOME`: `idlc`, `dds_stream_write_sampleLE`/`BE`. **Headers required** — the Python wheel has none |
| RTI | `$NDDSHOME`: `rtiddsgen -language C`, `<T>Plugin_serialize_to_cdr_buffer_ex` little endian, `tools/wireprobe/rti-template.c` big |
| Fast DDS | `$FASTDDSGEN` + `$FASTDDS_HOME`: the generated `<T>CdrAux` through `Cdr` |
| OpenDDS | `$OPENDDS_HOME`: `tao_idl` + `opendds_idl`, `OpenDDS::DCPS::Serializer` |

## The two other case sets

`tools/wire-oracle/` and `tools/optional-bits/` are separate `cases/`
directories behind this same driver. The separation is deliberate: a
case added for OpenDDS or for a padding question cannot disturb a run of
the set the layout rules were read off.
