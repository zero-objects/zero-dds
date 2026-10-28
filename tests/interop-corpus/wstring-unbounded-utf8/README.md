# wstring-unbounded-utf8

**Construct** — unbounded `wstring`, multi-byte

UTF-16 code units, two bytes each, with the length in bytes. Mixing Latin-1 range, a three-byte-in-UTF-8 character and ASCII means a probe that hands the runtime UTF-8 produces a different length and different content, not merely different padding. The trailing `long` is -1 so that any padding the string leaves before it reads as zero bytes against a member that is all ones.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 24 bytes — `0e000000e400f600fc002000341220005a000000ffffffff` |
| `--encoding xcdr2 --endian be` | 24 bytes — `0000000e00e400f600fc002012340020005a0000ffffffff` |
| `--encoding xcdr1 --endian le` | 24 bytes — `0e000000e400f600fc002000341220005a000000ffffffff` |
| `--encoding xcdr1 --endian be` | 24 bytes — `0000000e00e400f600fc002012340020005a0000ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.
