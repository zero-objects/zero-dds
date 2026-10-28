# The unspecified-constructs harnesses

Nine rows of
[`docs/wire-feature-inventory.md`](../../docs/wire-feature-inventory.md)
are verdicted **unspecified by the standard**. They cannot be closed by
measurement, because what is missing is a clause. They can be narrowed,
and these four harnesses are what narrowed them.

The results are
[`crates/ast-layout/tests/fixtures/unspecified_constructs.md`](../../crates/ast-layout/tests/fixtures/unspecified_constructs.md),
and the arguments they support are at rows B3, B6, G15, O4 and P1–P5 of
the inventory. Five of the findings are defects in the standard rather
than in an implementation; those are drafted in
[`docs/omg-xtypes-issues.md`](../../docs/omg-xtypes-issues.md).

## Running them

```sh
tools/unspecified-reach/front-ends.sh          # which front ends parse what
tools/unspecified-reach/bitset-bytes.sh        # P1, P2, P3, P5
tools/unspecified-reach/fixed-bytes.sh         # P4
tools/unspecified-reach/map-length.sh          # DDSXTY14-72
```

Under a minute each on the Linux test host. Every location comes from
[`tools/interop-vendors.sh`](../interop-vendors.sh), so a machine that
puts a toolchain elsewhere needs one setting and no edit. A toolchain
that is not installed is named on stderr with the number of rows the run
therefore does not carry, and `--require` turns that into a failure.

The ZeroDDS side of all four needs no vendor:

```sh
cargo test -p zerodds-wire-probe --test spec_constructs
```

## What each one is for

### `front-ends.sh` — how many implementations can be asked at all

One IDL declaration per unspecified construct, through six front ends,
with the diagnostic kept. It answers a question that comes before any
byte: **how many implementations exist to measure?**

For `bitset` the answer is one. Cyclone's `idlc`, RTI's `rtiddsgen`,
`opendds_idl`, `tao_idl` and `omniidl` all reject the keyword with a
syntax error; only `fastddsgen` parses it. That is why rows P1, P2 and
P3 rest on a single implementation and can never rest on two, and it is
a measurement about the construct rather than a gap in this repository.

The diagnostics are kept because they are themselves the finding in two
cases. `@try_construct` on an alias is refused by Cyclone and by RTI
with **different** accounts of the permitted set, and neither account
matches the three lists the specification gives — that is
`docs/omg-xtypes-issues.md` §B4. Extensibility on a collection is
refused by both with a third pair of messages — §A5.

**Accepting is not implementing.** `tao_idl --idl-version 4 -as` parses
any annotation and warns `error in lookup of symbol` for the ones it
does not define, and `fastddsgen` accepts an extensibility annotation on
an alias and drops it. `bitset-bytes.sh` and the byte gate are what tell
acceptance from implementation, and the answer for a collection is in
§4 of the fixture: the annotated and unannotated forms are byte
identical.

`--idl-version 4 -as` is passed to `tao_idl` deliberately. Without it
every annotated case comes back `Invalid use of this version of IDL`,
which is a statement about the flag and not about the construct. An
earlier run of this measurement omitted it and recorded TAO as refusing
constructs it in fact parses.

### `bitset-bytes.sh` — P1, P2, P3 and P5

Six shapes through Fast DDS: a scalar bitset, one in a sequence, one in
an array, a 24-bit one in a sequence, one as a key member, and one as a
key member of a type whose holder exceeds sixteen bytes so the KeyHash
takes the MD5 branch.

It reports the payload in both encoding versions, the §7.6.8 key holder
(big endian, because the clause says so, not by choice) and the instance
handle. The last of those is the only foreign check this project's
`Md5` arm has ever had.

The shapes that matter and why:

- **`seq_bitset`, `array_bitset`** — whether a bitset is a primitive
  element for rules (10) and (12). It is not: both carry a DHEADER
  under XCDR2. The same Fast DDS writes none for a collection of
  `bitmask`, so the two are not treated alike, and a rule that assumed
  they were would be picking a side of a live divergence (G13).
- **`seq_bitset_wide`, `key_bitset_long`** — a 12+12 bitset, which is
  the anchor-width rule at a width that is not itself a power of two.
- **`key_bitset_long`** — `string<40>` puts the holder's maximum
  serialized size over sixteen bytes, which is what reaches the MD5.

A generation or compile failure is reported as a row carrying its own
diagnostic, never as a missing row. `fastddsgen` is a JVM tool and two
of them started a second apart collide on `/tmp/hsperfdata_<user>`; that
is retried once and then reported, because a status code with no
diagnostic beside it is a silent skip in better clothes.

### `fixed-bytes.sh` — P4

XTypes has no fixed-point type, and `tao_idl` rejects the keyword with
`fixed data types are not supported`, so the byte gate cannot reach the
construct through OpenDDS's IDL at all. But `opendds_idl` accepts it and
generates `write_fixed(value.f.to_ace_fixed())` — so OpenDDS's encoder
**is** `ACE_CDR::Fixed`, and `ACE_CDR::Fixed` takes a value from a
string with no front end involved. This harness reaches around the front
end that refuses the type to the runtime that encodes it.

The rule is CORBA 3.3 part 2 §9.3.2.8: packed BCD, two digits an octet,
most significant first, the low nibble of the last octet the sign. Seven
values cover the sign nibble in both directions, the leading pad nibble
of an even declared digit count, one digit, the IDL 4.2 maximum of
thirty-one, scale equal to digits, and zero.

`ACE_CDR::Fixed` carries the digit count of the value it parsed rather
than of the type that declared it — the same thing JacORB's
`BigDecimal` does — so it and this project part company on a value with
fewer digits than its type: `fixed<5,2>` holding `0.00` is `000c` from
ACE and `00000c` here. That is a property of a runtime type with no
declared precision, not a second reading of §9.3.2.8. Row P4 has the
argument for why the declared count is the half a reader can use.

### `map-length.sh` — the other side of DDSXTY14-72

§7.4.1.1.4 defines a map by equivalence to `sequence<MapEntry_K_V>`, and
rule (12) writes a sequence as DHEADER, then length, then the elements.
Rule (15), written for `MAP_TYPE` itself, writes DHEADER and then the
pairs with no length. Four bytes apart, and **both are implemented**:
the byte gate has Fast DDS taking the first, and this harness has
OpenDDS taking the second.

Neither stream is malformed and neither reader can detect the other's
shape. A reader expecting rule (12) takes OpenDDS's first key for an
entry count; a reader expecting rule (15) takes Fast DDS's count for a
key. Three cases are produced rather than one, so the divergence is
bounded by measurement and not by assertion: `map<long,Cell>` and
`map<long,string>` diverge under XCDR2, and `map<long,long>` — rule
(14), `PMAP_TYPE` — agrees in both versions, as does everything under
XCDR1 where rule (16) has no DHEADER.

OpenDDS's choice is in its generator and not an accident:
`dds/idl/marshal_generator.cpp`, `gen_map_i`, emits
`strm.write_delimiter(total_size)` in the `XCDR_VERSION_2` branch and
`<< length` only in the `else`.

The harness shadows `dds/OpenDDSConfig.h` with `OPENDDS_CONFIG_SECURITY
0` on the include path. The vendor tree is never modified: this
checkout's generated `DdsSecurityCoreC.h` carries no `DDS::Security`
namespace, so `Discovery.h` will not compile with security on, and the
setting affects nothing this program uses — `Serializer` is the same
code either way.

## Reports

Every harness writes TSV to stdout, or to `--out`. The columns are at
the top of each script. `front-ends.sh` strips ANSI colour and the
absolute path from each diagnostic, because `fastddsgen` colours its
errors and every front end echoes the path it was handed, and neither
belongs in a report that is diffed between machines.
