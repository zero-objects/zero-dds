# primitive-longdouble-unannotated

**Construct** — primitive `long double`, unannotated

The `primitive-longdouble` type with every extensibility annotation removed, and nothing else changed. The vendors do not agree on what that means: `vendor_bytes.md` §3 records Cyclone's idlc reading an unannotated aggregate as final and RTI Connext reading it as appendable, while XTypes 1.3 §7.3.1.2.1.8 says appendable. Under XCDR2 the difference is a four-byte DHEADER in front of every aggregate in the file, which for this shape means the sample changes length and every offset after the first aggregate moves. Diffed against `primitive-longdouble` it isolates the default, because the two files differ in nothing else.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 100 bytes |
| `--encoding xcdr2 --endian be` | 100 bytes |
| `--encoding xcdr1 --endian le` | 96 bytes — `000000000000000000000000006000c00000000000001000000000000000ff3f0100000000000000000000000000ff3f00000000000000000000000000000100ffffffffffffffffffffffffffff0000fffffffffffffffffffffffffffffe7f` |
| `--encoding xcdr1 --endian be` | 96 bytes — `c00060000000000000000000000000003fff00000000000000100000000000003fff0000000000000000000000000001000100000000000000000000000000000000ffffffffffffffffffffffffffff7ffeffffffffffffffffffffffffffff` |

The six members and what each one catches are in
`primitive-longdouble/README.md`, together with what the four vendors do
with `long double` — which is four different things, none of them the
clause. The three vendor emitters refuse this case for that reason.

The bytes are what the Rust probe produces, recorded so that this case is
known to have been run and not merely written. They are not a reference:
the reference is the foreign implementation's own output, which the suite
compares against.
