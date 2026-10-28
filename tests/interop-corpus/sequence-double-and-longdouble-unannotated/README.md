# sequence-double-and-longdouble-unannotated

**Construct** — `sequence<double>` and `sequence<long double>` in one aggregate, unannotated

Both element types are a C `double` and a Rust `f64` — neither language
has a portable 128-bit float — and they are eight and sixteen bytes on the
wire. A generated support type named after the target language spelling
gives the two sequences one declaration and one encoder, and the second
goes out at the first's width with nothing to say it had.

No other case in the corpus holds both, so the collision was caught by
`-Werror` naming an unreferenced element encoder rather than by a value
comparison. This case makes it a value comparison: under the collision
`wide` is thirty-two bytes and comes back as two doubles read out of the
wrong offsets. `octet` against `uint8` and `unsigned short` against
`wchar` collapse the same way and are harmless; this pair is not.

The `wide` sequence carries one value a `double` holds and one it cannot:
`1 + 2^-112` needs 113 significand bits, so a column that routes a
`long double` through a `double` — or through x86's `long double`, which
has 64 — writes `1.0` there and reports success. That is what this
sequence adds over `primitive-longdouble`: the same discrimination
*inside a collection*, where the element encoder is generated once and
reused.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 76 bytes — `480000000200000000000000000004c0ffffffffffffef7f02000000000000000000000000000000004000c00100000000000000000000000000ff3f010000009a9999999999b93fffffffff` |
| `--encoding xcdr2 --endian be` | 76 bytes — `0000004800000002c0040000000000007fefffffffffffff00000002c00040000000000000000000000000003fff0000000000000000000000000001000000013fb999999999999affffffff` |
| `--encoding xcdr1 --endian le` | 84 bytes — `020000000000000000000000000004c0ffffffffffffef7f0200000000000000000000000000000000000000004000c00100000000000000000000000000ff3f01000000000000009a9999999999b93fffffffff` |
| `--encoding xcdr1 --endian be` | 84 bytes — `0000000200000000c0040000000000007fefffffffffffff0000000200000000c00040000000000000000000000000003fff000000000000000000000000000100000001000000003fb999999999999affffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.
