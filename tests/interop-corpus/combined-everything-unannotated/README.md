# combined-everything-unannotated

**Construct** — one aggregate holding most of the corpus, unannotated

The `combined-everything` type with every extensibility annotation removed, and nothing else changed. The vendors do not agree on what that means: `vendor_bytes.md` §3 records Cyclone's idlc reading an unannotated aggregate as final and RTI Connext reading it as appendable, while XTypes 1.3 §7.3.1.2.1.8 says appendable. Under XCDR2 the difference is a four-byte DHEADER in front of every aggregate in the file, which for this shape means the sample changes length and every offset after the first aggregate moves. Diffed against `combined-everything` it isolates the default, because the two files differ in nothing else.

## The same behaviour, with a `default` to fall into

`Choice` here is `case -1: long whole; default: double fraction;` — a negative
label on a `short` discriminator, which is the shape
`union-discriminator-short` isolates. Cyclone does not match it, and because
this union has a `default` it takes the `double` branch and writes eight octets
of the union's storage rather than nothing:

```text
plain-cdr1/le, the member with id 1
  cyclone  10000000  ffff0000 00000000  ffffff7f 00000000    length 16
  ZD-Rust  08000000  ffff0000           ffffff7f             length 8
  Fast DDS 0800      ffff0000           ffffff7f             length 8
```

The eight octets at offset 8 are the `long`'s bits with four zero octets above
them, which is what a `double` read of the storage a `long` was written into
gives. The member header's own length differs, so a reader that trusts it
disagrees about where member 2 begins.

Under XCDR2 the slot is not pre-aligned to eight and the difference is four
octets rather than eight, which is why the remainder groups these rows in two.
Together with the sixteen rows of `union-discriminator-{int8,short}` and their
twins this accounts for all twenty-four rows Cyclone's remainder holds.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 116 bytes |
| `--encoding xcdr2 --endian be` | 116 bytes |
| `--encoding xcdr1 --endian le` | 92 bytes — `7929edfffeff00000200000001000000000000000a000000c3bc62657220e188b400ffffffffff7f030000000a0000000900810004000000000000000000f83f00000000000004c0000000000000c03f00000000000013c0ffffffff` |
| `--encoding xcdr1 --endian be` | 92 bytes — `ffed2979fffe00000000000200000001000000000000000ac3bc62657220e188b400ffff7fffffff00000003000a000000098100000400003ff8000000000000c0040000000000003fc0000000000000c013000000000000ffffffff` |
| `--emit keyhash` | 16 bytes — `80208c54a0b66e6ef9dbf93e7dbc3ac1` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.
