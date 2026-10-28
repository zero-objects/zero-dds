# union-discriminator-short-unannotated

**Construct** — `union` over `short`, unannotated

The `union-discriminator-short` type with every extensibility annotation removed, and nothing else changed. The vendors do not agree on what that means: `vendor_bytes.md` §3 records Cyclone's idlc reading an unannotated aggregate as final and RTI Connext reading it as appendable, while XTypes 1.3 §7.3.1.2.1.8 says appendable. Under XCDR2 the difference is a four-byte DHEADER in front of every aggregate in the file, which for this shape means the sample changes length and every offset after the first aggregate moves. Diffed against `union-discriminator-short` it isolates the default, because the two files differ in nothing else.

## What Cyclone does with the narrow signed discriminator

Measured on the Linux test host over `plain-cdr1/le`, from a fill that is
byte-identical for both C vendors — `sample.picked._u.whole = -1234567;`
followed by `sample.picked._d = -12345;` — and from the recorded tree:

```text
union-discriminator-short   cyclone  c7cf0000 ffffffff
                            RTI      c7cf0000 7929edff ffffffff
                            Fast DDS c7cf0000 7929edff ffffffff
                            ZD-Rust  c7cf0000 7929edff ffffffff
```

Cyclone writes the discriminator and then the member *after* the union. The
selected branch is not on the wire, so a reader takes `tail` for `whole`.

It is the label's width and sign together, not either alone. The same sweep:

| case | discriminator | label | Cyclone |
|---|---|---|---|
| `union-discriminator-int8` | `int8` | `-13` | branch dropped |
| `union-discriminator-short` | `short` | `-12345` | branch dropped |
| `union-discriminator-long` | `long` | `-1234567` | agrees, byte for byte |
| `union-discriminator-octet` | `octet` | `171` | agrees, byte for byte |

A negative label on a 32-bit discriminator matches and an unsigned narrow one
matches; a negative label on a discriminator narrower than 32 bits does not.
`f3` read as 243 is not `-13` and `cfc7` read as 53191 is not `-12345`.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 20 bytes — `1000000008000000c7cf00007929edffffffffff` |
| `--encoding xcdr2 --endian be` | 20 bytes — `0000001000000008cfc70000ffed2979ffffffff` |
| `--encoding xcdr1 --endian le` | 12 bytes — `c7cf00007929edffffffffff` |
| `--encoding xcdr1 --endian be` | 12 bytes — `cfc70000ffed2979ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.
