# wstring-bounded-utf8

**Construct** — bounded `wstring`, multi-byte

The bounded wide string. The bound counts characters; the wire length counts bytes. Three characters, six bytes. The trailing `long` is -1 so that any padding the string leaves before it reads as zero bytes against a member that is all ones.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 16 bytes — `060000003412e4005a000000ffffffff` |
| `--encoding xcdr2 --endian be` | 16 bytes — `00000006123400e4005a0000ffffffff` |
| `--encoding xcdr1 --endian le` | 16 bytes — `060000003412e4005a000000ffffffff` |
| `--encoding xcdr1 --endian be` | 16 bytes — `00000006123400e4005a0000ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.
