# `try_construct`, measured as behaviour

Run on 2026-08-09 on the Linux test host. Five implementations, each
asked the same question: a writer serialises an object of a **wider**
type, a reader built from a **narrower** type deserialises those same
bytes, and what the reader does is the datum.

`vendor_bytes.md` compares bytes and `vendor_roundtrip.md` compares
values of one type against itself. Neither can answer this one: the
bound is not on the wire, so the two types produce and consume the same
bytes and the difference is entirely in what the receiving side decides.
There is nothing here to compare byte for byte, so every cell below is a
**behaviour** — a value with its content, a dropped sample, or a
refusal.

| | |
|---|---|
| Eclipse Cyclone DDS | `/opt/cyclone`, `idlc` + `dds_stream_normalize` + `dds_stream_read_sample` |
| RTI Connext DDS | 7.7.0, `rtiddsgen -language C` + `PRESTypePlugin_interpretedDeserialize` |
| eProsima Fast DDS | `fastddsgen` + the generated `<T>PubSubType::deserialize` |
| OpenDDS | 3.34.0, `opendds_idl` + `Serializer::operator>>` |
| omniORB | 4.x, `omniidl -bcxx` + the generated `operator<<=` |

The harness is `tools/try-construct-gate/`, deployed to
`tools/try-construct-gate/` in this repository. A case holds `writer.idl`,
`reader.idl` and the fill/dump fragment each language column needs; the
gate builds one program per side per vendor and pipes the payload
between them as hex. Both Cyclone and RTI produce the same writer bytes
for every case below, which is what makes one payload usable by all five
readers.

## What the columns mean

| verdict | what the reader did |
|---|---|
| `dropped` | refused the payload; nothing is delivered and no value is produced |
| `""`, `[1 2 3 4]`, … | a sample came back, with that value in the narrowed member |
| `no codegen` | the code generator refused the IDL, so there is no reader |
| `no parse` | the front end refused the annotation itself |

`dropped` and an error are not the same thing and the distinction is
[written down below](#what-discard-means).

## The four behaviours, member position, `@mutable`

`@mutable struct S { long a; string<32> m; long b; }` writes
`a=7, m="Hello World!", b=9`; the reader declares `string<5> m` with the
annotation in the header row. XCDR2 little endian, payload
`28000000 00000020 07000000 01000050 0d000000 48656c6c6f20576f726c6421 00000000 02000020 09000000`.

| reader's `m` | Cyclone | RTI 7.7 | Fast DDS | OpenDDS | omniORB |
|---|---|---|---|---|---|
| `string<5> m` — no annotation | `dropped` | `dropped` | `"Hello"` | `dropped` | `dropped` |
| `@try_construct(DISCARD)` | `dropped` | `dropped` | `no codegen` | `dropped` | `no parse` |
| `@try_construct(USE_DEFAULT)` | `dropped` | `dropped` | `no codegen` | **`""`** | `no parse` |
| `@try_construct(TRIM)` | `dropped` | `dropped` | `no codegen` | **`"Hello"`** | `no parse` |
| `@try_construct` — no argument | `dropped` | `dropped` | `no codegen` | **`""`** | `no parse` |

`a=7` and `b=9` came back correct in every row that produced a sample,
so OpenDDS consumes the whole twelve-character string off the wire
before it defaults or truncates. That is the property that matters: a
reader that stopped at its own bound would leave the following member at
the wrong offset.

**OpenDDS is the only implementation of USE_DEFAULT and TRIM**, and it
settles all four rows including the last one: `@try_construct` written
with no argument behaves as USE_DEFAULT, which is what §7.3.1.2.1.12
says and what no other implementation demonstrates.

## The same four, sequence and enumeration

`sequence<long,32>` holding `[1..8]` read as `sequence<long,4>`, and an
enumerator the reader does not declare. `@mutable` throughout.

| reader's member | Cyclone | RTI 7.7 | Fast DDS | OpenDDS |
|---|---|---|---|---|
| `sequence<long,4>` — no annotation | `dropped` | `dropped` | `[1 2 3 4 5 6 7 8]` | `dropped` |
| `@try_construct(USE_DEFAULT) sequence<long,4>` | `dropped` | `dropped` | `no codegen` | **`[]`** |
| `@try_construct(TRIM) sequence<long,4>` | `dropped` | `dropped` | `no codegen` | **`[1 2 3 4]`** |
| `K m` where the writer sent `K3` — no annotation | `dropped` | `dropped` | `m=2` | `dropped` |
| `@try_construct(USE_DEFAULT) K m` | `dropped` | `dropped` | `no codegen` | **`K1`** |

RTI names the reason: `Received sequence length 8 is larger than maximum
4`, `Received string length 12 is larger than maximum 5`, `Received
invalid enumerator value 2`. Cyclone refuses inside
`dds_stream_normalize`, before a value is built.

Fast DDS enforces **no bound at all** on a sequence — the reader's
`sequence<long,4>` came back holding eight elements, which is not a
value of the type the reader declared.

## The element position, §7.3.1.2.1.12.4

`sequence<@try_construct(TRIM) String5, 10>` against a writer's
`sequence<String16, 10>` whose first element is `"Hello World"`.

| | Cyclone | RTI 7.7 | Fast DDS | OpenDDS |
|---|---|---|---|---|
| parses `sequence<@try_construct(TRIM) T, N>` | no — `syntax error` | yes | no — `no viable alternative at input 'sequence<@'` | yes |
| reads the payload | — | ­— | — | **`["Hello"]`** |

OpenDDS's generated element loop is a full implementation of the clause:
on `BoundConstructionFailure` it truncates the element in place and
resets the construction status, and on any other failure it sets
`ElementConstructionFailure`, skips to the end of the collection and
returns false. The *sequence's own* length overflow still fails in the
same function, separately, because the sequence member carries no
annotation — which is exactly the split §7.2.2.7 draws between a member
and its elements.

## Where each front end accepts the annotation at all

One IDL file per position, run through five code generators. This is a
front-end result and says nothing about runtime behaviour.

| position | Cyclone | RTI 7.7 | Fast DDS | OpenDDS | omniORB |
|---|---|---|---|---|---|
| struct member | yes | yes | **no codegen** | yes | no |
| bare `@try_construct` on a member | yes | yes | **no codegen** | yes | no |
| `sequence<@try_construct(…) T, N>` | no | yes | no | yes | no |
| `@try_construct(…) typedef …` (alias) | no — *"can only be applied to struct members and union cases"* | no — *"not applicable for the context: alias"* | yes | yes | no |
| `switch (@try_construct T)` | no — same message | yes | **no codegen** | yes | no |
| `map<@try_construct(…) K, @try_construct(…) V, N>` | no | no | no | key rejected, value accepted | no |
| `@try_construct(TRIM) long m` | **no** — *"`@try_construct(TRIM)` can not be applied to types which cannot have bounds"* | yes | yes | no |

Fast DDS 3.x parses `@try_construct` and then aborts its own templates:
`no such property: com.eprosima.idl.parser.typecode.Member.annotationDiscard`.
Every value fails the same way, so **no Fast DDS type can carry the
annotation in any position on a member or a union**. Its unannotated
behaviour is a property of its C++ mapping rather than a decision about
the clause: `string<5>` becomes `eprosima::fastcdr::fixed_string<5>`,
whose assignment truncates in silence, and `sequence<long,4>` becomes a
plain `std::vector<int32_t>` with no bound to enforce.

omniORB is a CORBA front end and parses no annotation at all
(`Syntax error in definition` on the `@`). Its generated
`operator<<=` calls `unmarshalString(5)`, which raises `CORBA::MARSHAL`
on an over-long string — so it implements the DISCARD default by the
mapping, without knowing the clause exists.

## Whether the annotation survives into the type

| | what the generator does with it |
|---|---|
| Cyclone | writes the TryConstruct kind into the TypeObject member flags — the `MINIMAL`/`COMPLETE` blobs in `gen.c` differ by one byte, `0x01` for DISCARD and `0x03` for TRIM — and the ops descriptor and the runtime path are byte-identical. It records the annotation and ignores it |
| RTI 7.7 | `gen.c` is byte-identical between DISCARD and TRIM. The annotation reaches nothing |
| OpenDDS | in the TypeObject flags **and** in the generated `operator>>` |
| Fast DDS | nothing survives; codegen aborts |

## Where OpenDDS stops

Member-level TryConstruct is emitted **only in the `@mutable`
parameter-list path**. The same reader type declared `@appendable`
refuses all four cases, because `operator>>` for a final or appendable
struct is generated without any construction-status handling at all.
Element-level TryConstruct is emitted for every extensibility.

This is a gap in OpenDDS and not a reading of the clause: §7.2.2.7 is
written about type assignability and says nothing about extensibility.

## What `discard` means

The clause and the implementations agree, and the two halves are
different things to a subscriber:

- §7.2.4.1 — *"the default behavior is to discard the O2 object to avoid
  misinterpretation."*
- §7.2.3 — *"If a received data sample exceeds the limits expected by the
  receiving application, then the sample can be safely ignored (filtered
  out) by the receiver."*

Cyclone's path is the clearest witness. `dds_stream_normalize` fails
inside `ddsi_serdata_from_ser`, which returns `NULL`; `get_serdata` in
`ddsi_receive.c` passes the `NULL` on, `remote_make_sample` returns
`NULL`, and delivery ends at `remote_on_delivery_failure_fastpath`. The
sample never becomes a sample. The application's `take()` returns
nothing, no listener fires, and the only trace is a log line. **A
discarded sample is an absence, not an error.**

That is a different outcome from a malformed stream, which must reach
the caller: a length field larger than the message, bytes that are not
UTF-8, a parameter length that belongs to another vendor's framing.
Those say the *stream* is wrong. A discarded sample says this one sample
does not fit a type that is otherwise perfectly compatible, and the next
sample may well fit.

## What the clause leaves open, and what settles it

| question | the clause | settled by |
|---|---|---|
| `@try_construct` with no argument | §7.2.2.7 says the default behaviour is DISCARD; §7.3.1.2.1.12 declares the annotation's own parameter `default USE_DEFAULT` and says the bare form means USE_DEFAULT | **OpenDDS** — `""`. The two are not in conflict: no annotation is DISCARD, a bare annotation is USE_DEFAULT |
| TRIM on a member that is not a string, wstring, sequence or map | Table 11: *"unspecified and may be treated as an error"* | **Cyclone** refuses it in the front end; **OpenDDS** accepts the IDL and generates the DISCARD branch; **RTI** accepts and ignores. No implementation trims |
| a map's **key** with USE_DEFAULT or TRIM | §7.2.2.7 says a map may declare the behaviour of its key; Table 17 says a key that cannot construct fails the whole map, with no TryConstruct escape | **OpenDDS** — `opendds_idl` aborts with *"map keys may only use DISCARD as their @try_construct mode"*. Table 17 wins |
| `@try_construct` on an alias | not addressed. §7.2.4.4.3 resolves aliases to their base type for assignability | **Cyclone and RTI both refuse it by name**; Fast DDS and OpenDDS accept it silently. Two refusals against two silences |
| the default value of an enumeration under USE_DEFAULT | Table 9 says *"the first literal"*; §7.3.1.2.1.12.5 says the discriminator takes ENUM2, which is the `@default_literal` | **not settled.** OpenDDS produces the first literal and its own source says why: *"For now, simply return the first value of the enumeration. Must be changed, if support for `@default_literal` is desired."* A disclaimed shortcut is not a reading |
