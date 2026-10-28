# bitset-inheritance

**Construct** — `bitset` extending another `bitset`

A bitset that inherits. The wire form is identical to `bitset-basic` — the same byte from a different declaration, with the base's bits first — so any difference between the two is an inheritance bug and nothing else. Both are `8dab`, and so is Fast DDS.

**No clause says so.** XTypes 1.3 has no serialisation rule for a bitset at all: §7.4.3.5.3 has no matching left-hand side and §7.4.1.1 does not cover the type, so there is nothing to cite for the anchor width, the bit order or where an inherited bitfield sits. This file used to cite §7.3.1.2.1.7, which is "Nested Types" and is about something else; a wrong citation invites the next reader to trust it. `docs/refused-constructs.md`, "bitset", records what is measured instead — Fast DDS is the only implementation that parses the construct, and its `8dab` for this case is byte for byte what it produces for `bitset-basic`.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 2 bytes — `8dab` |
| `--encoding xcdr2 --endian be` | 2 bytes — `8dab` |
| `--encoding xcdr1 --endian le` | 2 bytes — `8dab` |
| `--encoding xcdr1 --endian be` | 2 bytes — `8dab` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.
