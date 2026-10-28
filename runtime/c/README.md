# The C wire runtime

The counterpart of `crates/wire`, not a binding to it.

Every rule here is implemented from DDS-XTypes 1.3 rather than by
calling the Rust runtime, because a C probe that went through FFI would
be byte-identical by construction and would prove nothing. Where this
and `crates/wire` disagree, one of the two has misread the
specification, and that disagreement is the finding the interop suite
exists to produce — `docs/specs/zerodds-interop-suite-1.0.md` §3a.

## What is here

```text
include/zerodds_wire.h    the whole interface
src/writer.c              alignment, primitives, strings, framing, BCD
src/reader.c              the mirror
src/md5.c                 the KeyHash digest, RFC 1321
tests/selftest.c          the rules this would be wrong about silently
Makefile                  the library and the self-test
```

```sh
make          # build/libzerodds_wire.a
make check    # the self-test
```

C99, no dependency beyond the freestanding headers, and no `malloc`
anywhere in the write path: the caller owns the buffer and is told when
it is short. That is what lets an embedded target link it.

## The shape of the interface

A writer records the first thing that goes wrong and then does nothing
further:

```c
uint8_t buffer[512];
zdw_writer w;
zdw_writer_init(&w, buffer, sizeof buffer, ZDW_LITTLE, 4);

zdw_dheader d = zdw_write_dheader(&w);
zdw_write_i32(&w, -1234567);
zdw_write_string(&w, "über");
zdw_finish_dheader(&w, d);

if (zdw_writer_status(&w) != ZDW_OK) {
    fprintf(stderr, "%s\n", zdw_status_name(zdw_writer_status(&w)));
    return 1;
}
fwrite(buffer, 1, zdw_writer_length(&w), stdout);
```

The sticky status is deliberate. Carrying on after a short buffer and
writing whatever fits would produce bytes that look like a sample and
are not, which is the failure this project treats as the worst one.

## What the runtime decides and what it is told

It decides the bytes: alignment from the stream origin capped at the
encoding's `MAXALIGN`, the string length convention, the UTF-16 count,
the BCD packing of a `fixed`, the signed holder an enumerator goes into,
and the three framing forms.

It decides nothing about a type. Which members are written, in what
order, with which header and which length code is
`zerodds-ast-layout`'s answer and arrives as arguments — the same
division `crates/wire` keeps, for the same reason: a runtime that
decided any of it would be a second place where the wire format lives.

## The two dialect settings that live here

Most vendor differences are in the layout plan. Two are in the bytes and
so are the writer's:

| setting | what it changes |
|---|---|
| `zdw_wstring_policy` | how a `wstring`'s length is counted |
| `zdw_pl_cdr1_framing` | the must-understand bits on `PID_EXTENDED` and `PID_LIST_END`, and whether a parameter's declared length covers its trailing padding |

Take both from `zerodds_ast_layout::Dialect`, whose documentation
carries the measurement, rather than assembling them by hand.

## The self-test

`tests/selftest.c` pins byte strings, not properties. A property test
over an encoder can pass while the encoder and the specification
disagree, because the property was written from the same reading. Each
expectation names the clause it comes from, or the row of
`crates/ast-layout/tests/fixtures/vendor_bytes.md` where a vendor
measured it.

## Who calls it

`tools/wireprobe/wireprobe-zerodds-c` compiles generated C against this
and writes one sample's bytes. The generator is
`tools/wireprobe/wireprobe-zerodds-c-gen`; the types and the codec come
from `crates/lang-rules` (`adapter::c` and `encode_c`).
