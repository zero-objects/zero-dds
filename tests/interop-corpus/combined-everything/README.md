# combined-everything

**Construct** — one aggregate holding most of the corpus

Every framing decision in one sample: a mutable outer type, an appendable member inside it, a final leaf inside that, a union, a sequence of a constructed type through a typedef, a bitmask, an absent optional and a two-dimensional array. It exists because the individual cases each fix one decision and this one fixes their interaction — an implementation can be right about each of them separately and still compute the outer extents wrong.

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
| `--encoding xcdr2 --endian le` | 152 bytes |
| `--encoding xcdr2 --endian be` | 152 bytes |
| `--encoding xcdr1 --endian le` | 124 bytes |
| `--encoding xcdr1 --endian be` | 124 bytes |
| `--emit keyhash` | 16 bytes — `80208c54a0b66e6ef9dbf93e7dbc3ac1` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.
