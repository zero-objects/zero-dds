# OpenSplice Community Edition 6.9 in the interop harnesses

OpenSplice CE 6.9 (`OSPL_V6_9_210323OSS_RELEASE`, the last open-source
release of ADLINK's DDS) as a column of the OMG dds-rtps harness and of
the serialisation corpus.

| file | what it is |
|---|---|
| `build-opensplice.sh` | builds and installs OpenSplice from source on a current Linux (GCC 14); every deviation from the release build is a make variable, listed in the script |
| `build-shape-main.sh` | builds the OMG dds-rtps `shape_main` against OpenSplice's classic C++ API (SACPP) and writes the wrapper the harness runs |
| `patch_shape_main.py` | adds the `OPENSPLICE` branches to the OMG `srcCxx/shape_main.cxx`; each edit names the line it expects and fails on a changed upstream file |
| `shape_configurator_opensplice.h` | the vendor configurator `shape_main.cxx` includes |
| `shape.idl` | `ShapeType` in the IDL 3.5 `idlpp` reads (`#pragma keylist`, no annotations); under XCDR1 its bytes equal the `@appendable` ShapeType of the OMG IDL |
| `../wireprobe/wireprobe-opensplice` | the serialisation column (see its header) |

## What OpenSplice 6.9 can and cannot be asked

* One data representation, XCDR1, in the host's byte order. The OMG
  harness defaults to `-x 2`; the OpenSplice `shape_main` refuses XCDR2
  with "not supported", so OpenSplice is measured with `-x 1` on both
  sides.
* No XTypes: `idlpp` rejects `@appendable`, `@key`, `@mutable`, IDL 4
  integer names and `wchar`, and accepts `@optional` without honouring it.
* `get_key_value()` answers an empty key for a NOT_ALIVE instance, so the
  `shape_main` port keeps the colour per instance, as the OMG code already
  does for OpenDDS.

## Running the harness

```sh
tools/opensplice/build-opensplice.sh <opensplice-clone> /opt/opensplice-6.9
tools/opensplice/build-shape-main.sh <dds-rtps-checkout> /opt/opensplice-6.9 <out-dir>
cd <dds-rtps-checkout>
python3 interoperability_report.py -x 1 \
    -P <out-dir>/opensplice_ce-6.9.210323_shape_main_linux -S <other shape_main> \
    -t Test_Reliability_0 … -o report.xml
```

Keep other DDS participants of domain 0 off the host while measuring: a
stray participant (a `ros2` daemon, a forgotten `shape_main`) changes how
OpenSplice's DDSI2 routes its discovery data and so what is measured.
