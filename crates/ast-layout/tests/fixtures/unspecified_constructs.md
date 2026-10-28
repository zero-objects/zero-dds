# The constructs the standard does not specify, measured

The nine rows of `docs/wire-feature-inventory.md` verdicted **unspecified
by the standard** cannot be closed by reading the clause, because there
is no clause. They can still be narrowed, and this file is what the
narrowing produced: every foreign byte string the extrapolations in
those rows now rest on, with the implementation, the version and the
command that produced it.

Measured 2026-08-10 on the Linux test host. Byte strings exclude the
four-byte encapsulation header unless the header is the subject.

| | |
|---|---|
| Eclipse Cyclone DDS | 11.0.1, `/opt/cyclone`, `idlc` |
| RTI Connext DDS | 7.7.0, `rtiddsgen` 4.7.0 |
| eProsima Fast DDS | 3.6.1 / Fast CDR 2.3.5, `fastddsgen` 4.3.0 + `Cdr` |
| OpenDDS | 3.34.0, `opendds_idl -Lc++11` + `OpenDDS::DCPS::Serializer` |
| ACE/TAO | 2.5.24, `tao_idl` and `ACE_CDR::Fixed` over `ACE_OutputCDR` |
| omniORB | 4.3.3, `omniidl -bcxx` |
| JacORB | 3.9, `org.jacorb.idl.parser` |

ZeroDDS bytes beside them are `target/debug/wireprobe-zerodds` from this
working tree.

The four harnesses that produced every foreign row here are
`tools/unspecified-reach/`, with a README saying what each one settles.
They resolve every toolchain through `tools/interop-vendors.sh`, so a
machine that puts the vendors elsewhere needs one setting per vendor and
no edit, and a toolchain that is absent is named with the number of rows
the run did not carry.

---

## 1. A bitset in a collection, and as a key member — P5

The row said "planned as a scalar of its anchor width" and column 3 said
**none**. Fast DDS is the only implementation whose front end parses the
keyword (§2 below re-establishes that), so it is the only one that can
be asked, and it had never been asked.

```idl
bitset Packed { bitfield<3> low; bitfield<5> high; };
@final struct Sample { sequence<Packed> many; long tail; };
```
`many = [{5,17},{0,31}], tail = -1`

| | XCDR1 | XCDR2 |
|---|---|---|
| Fast DDS | `020000008df80000ffffffff` | `06000000020000008df8` **`ffff`** `ffffffff` |
| ZeroDDS | `020000008df80000ffffffff` | `06000000020000008df8` **`0000`** `ffffffff` |

**Agreeing.** The two octets in bold are the padding before the
4-aligned `long`; Fast CDR leaves them uninitialised and this project
writes zero, which is A4 and not a disagreement about the map of bits.

```idl
@final struct Sample { Packed items[2]; octet tail; };
```
`items = [{5,17},{7,0}], tail = 0xAB`

| | XCDR1 | XCDR2 |
|---|---|---|
| Fast DDS | `8d07ab` | `020000008d07ab` |
| ZeroDDS | `8d07ab` | `020000008d07ab` |

**Agreeing, byte for byte.** The result that matters is the `02000000`:
under XCDR2 Fast DDS **delimits** a collection whose element is a
bitset. A bitset is therefore not a primitive element for rules (10) and
(12), and it is not exempted the way an enumeration and a bitmask are
exempted by `CollectionDelimiterPolicy::EnumIsPrimitive` — the same
Fast DDS writes **no** delimiter for `sequence<bitmask>` (G13). The
project's planner reaches the same answer for a different reason:
`is_primitive_after_aliases` and `is_enumeration_or_bitmask_after_aliases`
both say no for a `Bitset`, so `collection_is_delimited` says yes.

### As a key member

```idl
bitset Packed { bitfield<3> low; bitfield<5> high; };
@final struct Sample { @key Packed k; octet tail; };
```
`k = {5,17}, tail = 0xAB`

| | payload (both versions) | key holder, big endian | instance handle / KeyHash |
|---|---|---|---|
| Fast DDS | `8dab` | `8d` | `8d000000000000000000000000000000` |
| ZeroDDS | `8dab` | — | `8d000000000000000000000000000000` |

And with a holder over sixteen bytes, so the MD5 branch runs:

```idl
bitset Wide { bitfield<12> a; bitfield<12> b; };
@final struct Sample { @key Wide k; @key string<40> s; octet tail; };
```
`k = {0x111, 0x222}, s = "abc", tail = 0xAB`

| | payload (both versions) | key holder, big endian | KeyHash |
|---|---|---|---|
| Fast DDS | `112122000400000061626300ab` | `002221110000000461626300` | `0168f0070ddb81e794da3da67abebd41` |
| ZeroDDS | `112122000400000061626300ab` | — | `0168f0070ddb81e794da3da67abebd41` |

**Agreeing, including the hash.** Three things fall out of the second
case and each is a separate claim:

1. A 24-bit bitset takes a **four-byte** anchor — `11 21 22 00` little
   endian, `00 22 21 11` big endian. That is the 1/2/4/8 band rule of
   P1, measured on a width no earlier case used.
2. In the key holder the bitset is written as that anchor and nothing
   else: no delimiter, no per-bitfield framing. P5's "planned as a
   scalar of its anchor width" is what Fast DDS does.
3. `string<40>` puts the holder's maximum serialized size at 4 + 4 + 44,
   over sixteen, so the hash is the MD5 of the big-endian holder. The
   two agree on it, which is the first time this project's `Md5` arm has
   been checked against a foreign implementation (`codec-coverage-report.md`
   ranked item 10, N7).

Reproducing:

```sh
tools/unspecified-reach/bitset-bytes.sh        # Fast DDS
target/debug/wireprobe-zerodds --idl <case>.idl --type Sample \
    --values <case>.json --emit keyhash        # ZeroDDS
```

## 2. Which front ends parse `bitset` at all — P1, P2, P3

Re-measured against six front ends, because the earlier count of five
predates OpenDDS and the ORBs being on the host. `bitset B { bitfield<3>
a; bitfield<5> b; };`

| front end | verdict |
|---|---|
| Cyclone `idlc` 11.0.1 | `t.idl:1:1: syntax error` |
| RTI `rtiddsgen` 4.7.0 | `mismatched input 'bitset' expecting EOF` |
| OpenDDS `opendds_idl` 3.34.0 | `syntax error` |
| ACE/TAO `tao_idl` 2.5.24 | `syntax error` |
| omniORB `omniidl -bcxx` | `t.idl:1: Syntax error in definition` |
| eProsima `fastddsgen` 4.3.0 | accepts |

**Five refuse the keyword; one parses it.** So P1, P2 and P3 have one
implementation and can have no second one, and the same holds for the
two new shapes in §1. Fast CDR 2.3.5's four `std::bitset<N>` overloads —
`N<9`, `8<N<17`, `16<N<33`, `32<N<65` — remain the second, independent
witness for the anchor bands, because they are source rather than an
IDL front end.

## 3. `fixed<P,S>` — P4

`omniidl -bcxx` accepts the keyword; `tao_idl` refuses it (`fixed data
types are not supported`) while ACE ships `ACE_CDR::Fixed` with CDR
stream operators, and `opendds_idl` accepts the keyword and generates
`write_fixed` over `to_ace_fixed()`. So OpenDDS's marshalling **is**
ACE's, and ACE's is measurable without an IDL front end.

`ACE_CDR::Fixed::from_string(v)` into an `ACE_OutputCDR`:

| declared | value | ACE/TAO 2.5.24 | ZeroDDS |
|---|---|---|---|
| `fixed<5,2>` | `123.45` | `12345c` | `12345c` |
| `fixed<5,2>` | `-123.45` | `12345d` | `12345d` |
| `fixed<4,2>` | `12.34` | `01234c` | `01234c` |
| `fixed<1,0>` | `7` | `7c` | `7c` |
| `fixed<31,0>` | 31 nines | `9999…9c` (16 octets) | the same |
| `fixed<5,5>` | `0.12345` | `0012345c` (4 octets) | `12345c` (3) |
| `fixed<5,2>` | `0.00` | `000c` (2 octets) | `00000c` (3) |

**Agreeing on the rule, diverging where the value has fewer digits than
the type declares.** `ACE_CDR::Fixed` carries its own digit count, taken
from the string it was parsed from, and writes `⌈(that+1)/2⌉` octets;
`fixed<5,5>` from `"0.12345"` becomes six digits and `fixed<5,2>` from
`"0.00"` becomes three. That is the same shape as JacORB's
`BigDecimal.precision()`, already recorded, and it is not a second
reading of the encoding — the sign nibble, the BCD packing and the
octet count from a digit count are identical. It is a property of a
runtime type that has no declared precision to be told.

So `fixed<P,S>` now has **three** ORB-family witnesses for the encoding
— omniORB, JacORB and ACE/TAO — and the two that keep the *declared*
digit count are omniORB and this project.

## 4. Extensibility on a collection — G15

The row said the rule "has no IDL that reaches it" because neither of
the two front ends the project measured against accepts the annotation.
Six front ends, four spellings:

| IDL | Cyclone | RTI | OpenDDS | TAO | omniORB | Fast DDS |
|---|---|---|---|---|---|---|
| `@appendable typedef sequence<long> LS;` | refuses¹ | refuses² | accepts | warns³ | refuses⁴ | accepts |
| `@mutable typedef sequence<long> LS;` | refuses¹ | refuses² | accepts | warns³ | refuses⁴ | accepts |
| `@extensibility(APPENDABLE) typedef sequence<long> LS;` | refuses¹ | refuses² | accepts | warns³ | refuses⁴ | accepts |
| `@final struct S { @appendable sequence<long> v; … };` | refuses¹ | refuses⁵ | accepts | warns³ | refuses⁴ | accepts |

¹ `@appendable can only be applied to constructed types`
² `The annotation '@appendable' is not applicable for the context: alias.`
³ `tao_idl --idl-version 4 -as` parses the annotation syntax, prints
  `Warning … error in lookup of symbol: appendable` and exits zero. TAO
  defines no XTypes annotation, so this is neither acceptance nor
  refusal: the annotation is dropped and the author is told. Without
  `--idl-version 4` the same file is `Invalid use of this version of
  IDL`, which is a statement about the flag and not about the construct
  — an earlier run of this measurement omitted the flag and recorded
  TAO as refusing.
⁴ `Syntax error in definition` — CORBA IDL has no annotations at all
⁵ `… not applicable for the context: struct/valuetype field.`

The two that accept were then asked what the annotation *does*.
`fastddsgen` emits `typedef std::vector<int32_t> LS;` — the annotation
is dropped in the front end — and the bytes prove it:

| IDL | Fast DDS XCDR1 | Fast DDS XCDR2 |
|---|---|---|
| `@appendable typedef sequence<long> LS;` | `020000000100000002000000ab` | `020000000100000002000000ab` |
| `typedef sequence<long> LS;` | the same | the same |
| `@appendable typedef sequence<Cell> CS;` | `0100000007000000ab` | `0c000000010000000400000007000000ab` |
| `@mutable typedef sequence<Cell> CS;` | the same | the same |
| `typedef sequence<Cell> CS;` | the same | the same |

**Byte-identical in every pair.** So the row is no longer "no IDL
reaches it": IDL reaches it in two front ends, and the one whose bytes
can be read writes the unannotated form. Rules (29) and (30) applied to
a collection have no observable effect in any measured implementation.

## 5. The map length under XCDR2 — the two readings, both measured

§7.4.1.1.4 makes a map equivalent to `sequence<MapEntry_K_V>`, which
rule (12) writes as `DHEADER`, then `length`, then the elements. Rule
(15), written for `MAP_TYPE` itself, writes `DHEADER` and then the pairs
with no length. Both are implemented.

```idl
@final struct Cell { long a; long b; };
@final struct Sample { map<long, Cell> m; octet tail; };
```
`m = { 7 → {1,2} }, tail = 0xAB`

| | XCDR1 | XCDR2 |
|---|---|---|
| Fast DDS 3.6.1 | `01000000 07000000 01000000 02000000 ab` | `10000000` `01000000` `07000000 01000000 02000000 ab` |
| OpenDDS 3.34.0 | `01000000 07000000 01000000 02000000 ab` | `0c000000` `07000000 01000000 02000000 ab` |
| ZeroDDS, `--map-length sequence-equivalent` | the same | Fast DDS's, byte for byte |
| ZeroDDS, `--map-length rule-15-omits` | the same | OpenDDS's, byte for byte |

```idl
@final struct Sample { map<long, string> m; octet tail; };
```
`m = { 7 → "abc" }, tail = 0xAB`

| | XCDR2 |
|---|---|
| Fast DDS | `10000000` `01000000` `07000000 04000000 61626300 ab` |
| OpenDDS | `0c000000` `07000000 04000000 61626300 ab` |
| ZeroDDS `sequence-equivalent` / `rule-15-omits` | the first / the second |

**Four bytes apart, both self-consistent, and each DHEADER counts what
follows it in its own reading.** Under XCDR1 the three agree, because
rule (16) has no DHEADER and no ambiguity. A `map<long,long>` — rule
(14), `PMAP_TYPE` — is `01000000 01000000 02000000` from Fast DDS,
OpenDDS and ZeroDDS alike, because a primitive map carries a length and
no DHEADER in both versions.

OpenDDS's is not an accident: `dds/idl/marshal_generator.cpp`
`gen_map_i` writes `strm.write_delimiter(total_size)` in the
`XCDR_VERSION_2` branch and `<< length` only in the `else`, and its
`serialized_size` counts one `primitive_serialized_size_ulong` under the
comment `// either DHeader or Map length`.

This is OMG issue **DDSXTY14-72**, open since 2023-12-05. See
`docs/omg-xtypes-issues.md` §A1.

## 6. `@try_construct` on an alias — the position §7.3.1.2.1.12 omits

| IDL | Cyclone | RTI | OpenDDS | TAO | omniORB | Fast DDS |
|---|---|---|---|---|---|---|
| `@try_construct(TRIM) typedef string<5> ShortStr;` | refuses¹ | refuses² | accepts | warns³ | refuses⁴ | accepts |
| `@try_construct(USE_DEFAULT) typedef string<5> ShortStr;` | refuses¹ | refuses² | accepts | warns³ | refuses⁴ | accepts |

¹ `@try_construct can only be applied to struct members and union cases`
² `The annotation '@try_construct(TRIM)' is not applicable for the context: alias.`
³ `Warning … error in lookup of symbol: try_construct` — as §4 note ³
⁴ `Syntax error in definition`

Two refuse it by name and name a different permitted set each time;
neither of the two that accept it says what it means; and the one that
warns tells the author the annotation was dropped, which is more than
either of the two that accept it does.
`crates/ast-layout/tests/fixtures/try_construct_behaviour.md` has the
five implementations on the positions that *are* listed.

## 7. The encapsulation identifier — Table 39 against Table 60

Every header this run produced, read out of the serialiser rather than
off the wire:

| implementation | XCDR1 FINAL, BE / LE | XCDR2 FINAL, BE / LE |
|---|---|---|
| Fast DDS 3.6.1 | `0000` / `0001` | `0006` / `0007` |

which is Table 60. Table 39 gives `{0x00,0x10}`/`{0x00,0x11}` for
PLAIN_CDR2, and rule (1) is written in terms of Table 39's
`ENC_HEADER`. RTI's measured values are already in `vendor_bytes.md`
("Big endian"), and the two implementations whose probe does not report
a header name Table 60's values in their own source:

| | |
|---|---|
| Cyclone DDS 11.0.1 | `dds/ddsi/ddsi_protocol.h` — `DDSI_RTPS_CDR2_BE 0x0006`, `CDR2_LE 0x0007`, `D_CDR2_BE 0x0008`, `D_CDR2_LE 0x0009`, `PL_CDR2_BE 0x000a`, `PL_CDR2_LE 0x000b` |
| OpenDDS 3.34.0 | `dds/DCPS/EncapsulationHeader.h` — `KIND_CDR2_LE = 0x0007`, `KIND_D_CDR2_LE = 0x0009`, `KIND_PL_CDR2_LE = 0x000b` |

Neither has a constant anywhere in the `0x0010`–`0x0015` range. Four
implementations, none of them Table 39.

`D_CDR2_BE`/`D_CDR2_LE` being named in both is also the first foreign
evidence of any kind for B1's two unmeasured identifiers, though it is
source and not bytes.
