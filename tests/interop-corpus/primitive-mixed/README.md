# primitive-mixed

**Construct** — every primitive in one aggregate

All fourteen primitives this project can represent, in ascending declaration order, so every internal padding rule fires at least once. This is the case that separates XCDR1 from XCDR2: the eight-byte members align to eight under XCDR1 and to four under XCDR2, so the same sample is two different lengths.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 48 bytes — `01abfef35a00e400c7cfefbe7929edffefbeaddebd08370b1affffffbec2addeefbeadde000070c0182d4454fb210940` |
| `--encoding xcdr2 --endian be` | 48 bytes — `01abfef35a0000e4cfc7beefffed2979deadbeefffffff1a0b3708bddeadbeefdeadc2bec0700000400921fb54442d18` |
| `--encoding xcdr1 --endian le` | 56 bytes — `01abfef35a00e400c7cfefbe7929edffefbeadde00000000bd08370b1affffffbec2addeefbeadde000070c000000000182d4454fb210940` |
| `--encoding xcdr1 --endian be` | 56 bytes — `01abfef35a0000e4cfc7beefffed2979deadbeef00000000ffffff1a0b3708bddeadbeefdeadc2bec070000000000000400921fb54442d18` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.
