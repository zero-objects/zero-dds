# typedef-of-struct

**Construct** — `typedef` of a structure

An aggregate behind an alias, twice. The alias must not add framing of its own: two appendable cells means two DHEADERs, not four, and not two plus an outer one.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 22 bytes — `060000007929edfffeff00000600000001000000ff7f` |
| `--encoding xcdr2 --endian be` | 22 bytes — `00000006ffed2979fffe000000000006000000017fff` |
| `--encoding xcdr1 --endian le` | 14 bytes — `7929edfffeff000001000000ff7f` |
| `--encoding xcdr1 --endian be` | 14 bytes — `ffed2979fffe0000000000017fff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.
