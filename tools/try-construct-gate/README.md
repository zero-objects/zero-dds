# The `try_construct` gate

A writer's type serialises a sample, a **narrower** reader's type
deserialises the same bytes, and what the reader does is the
measurement. DDS-XTypes 1.3 §7.2.2.7.

`vendor_bytes.md` compares bytes and `vendor_roundtrip.md` compares
values of one type against itself. Neither can answer this question: a
bound is not on the wire, so `string<32>` and `string<5>` produce and
consume identical bytes and the whole difference is in what the
receiving side decides. There is nothing to diff, so every result is a
behaviour — a value with its content, a dropped sample, or a refusal.

The results are in
`crates/ast-layout/tests/fixtures/try_construct_behaviour.md`.

## Running it

It needs the five vendor toolchains and runs from the checkout — it used
to hard-code `/root/tc-gate` as its own root and could only run from a
copy deployed there:

```
tools/try-construct-gate/gate.sh write str_discard cyclone
tools/try-construct-gate/gate.sh read str_trim opendds xcdr2 <hex>
```

Where each vendor lives is `tools/interop-vendors.sh`, so one setting
per vendor moves the gate to a machine that puts them elsewhere.

`fe/fe.sh` beside the cases asks the question one step earlier: which
positions each front end accepts `@try_construct` in at all.
§7.3.1.2.1.12's list is incomplete and the four disagree about the rest,
and that run is what `docs/interop-evidence.md` §4.3 reports as "two
refusals against two silences is not a reading".

`write` builds the writer side from `writer.idl` and prints the payload
as hex, once per encoding version. `read` builds the reader side from
`reader.idl` and feeds it that hex on stdin. The two halves are separate
programs so that one payload can be handed to every reader: two
implementations that disagree about the bytes would otherwise be
answering different questions.

Cyclone and RTI produce the same writer bytes for every case here, which
is what makes one payload usable by all five readers. Check that before
trusting a new case.

## A case

```
cases/<name>/
    writer.idl       the wider type
    reader.idl       the narrower one, same type name
    writer.omni.idl  optional: omniidl parses no annotations at all
    reader.omni.idl
    type             the top-level type's name
    fill.c.inc       C statements filling `sample`   (Cyclone, RTI)
    fill.rti.inc     optional override where the two C APIs differ
    fill.cpp.inc     C++                              (Fast DDS)
    fill.odds.inc    C++                              (OpenDDS)
    fill.omni.inc    C++                              (omniORB)
    dump.<same>.inc  printing what the reader produced
```

Both IDL files declare the same type name, because the reader's
generated code has to be a drop-in for the writer's. The fill and dump
fragments are per language column and not per vendor where the API is
the same.

## What each vendor needs

| | |
|---|---|
| Cyclone | `$CYCLONE_HOME`, `idlc`, `dds_stream_normalize` + `dds_stream_read_sample` |
| RTI | `$NDDSHOME`, `rtiddsgen -language C`, `PRESTypePlugin_interpretedDeserialize` |
| Fast DDS | `$FASTDDSGEN` + `$FASTDDS_HOME`, the generated `<T>PubSubType::deserialize` |
| OpenDDS | `$OPENDDS_HOME`, `tao_idl` + `opendds_idl`, `Serializer::operator>>` |
| omniORB | `$OMNIIDL -bcxx`, the generated `operator<<=`. Needs `CORBA::ORB_init` before any stream is read — `unmarshalString` reaches into ORB state and segfaults without it |
