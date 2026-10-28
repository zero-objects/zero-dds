# bitmask-sparse-positions

**Construct** — `bitmask` with implicit and explicit positions

`@position` on some bits and not others. XTypes 1.3 §7.3.1.2.1.6 gives an unannotated bit the position after the previous one, so `ALPHA` is 0, `BETA` is 5 and `GAMMA` is 6 — not 1 and not 2. The sample sets bits 0, 6 and 15 and leaves 5 clear, which no other numbering reproduces.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 5 bytes — `41800100ab` |
| `--encoding xcdr2 --endian be` | 5 bytes — `80410001ab` |
| `--encoding xcdr1 --endian le` | 5 bytes — `41800100ab` |
| `--encoding xcdr1 --endian be` | 5 bytes — `80410001ab` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.
