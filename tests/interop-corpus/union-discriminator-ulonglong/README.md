# union-discriminator-ulonglong

**Construct** — `union` over `unsigned long long`

An unsigned eight-byte discriminator above 2^63, which is why the JSON carries it as a string: a JSON number is a double and loses the low bits. The union is a member rather than the top-level type so that the padding between the discriminator and the selected member, and between the union and what follows it, are both on the wire.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 20 bytes — `0c000000bec2addeefbeadde7929edffffffffff` |
| `--encoding xcdr2 --endian be` | 20 bytes — `0000000cdeadbeefdeadc2beffed2979ffffffff` |
| `--encoding xcdr1 --endian le` | 16 bytes — `bec2addeefbeadde7929edffffffffff` |
| `--encoding xcdr1 --endian be` | 16 bytes — `deadbeefdeadc2beffed2979ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.
