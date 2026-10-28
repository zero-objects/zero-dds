# wstring-unbounded-empty

**Construct** — unbounded `wstring`, empty

An empty wide string. XTypes 1.3 §7.4.1.1 gives `wstring` a length in bytes and no terminator, unlike `string` — so empty is four bytes of zero here and four bytes of one there. An implementation that shares one code path between the two gets exactly one of these two cases wrong. The trailing `long` is -1 so that any padding the string leaves before it reads as zero bytes against a member that is all ones.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 8 bytes — `00000000ffffffff` |
| `--encoding xcdr2 --endian be` | 8 bytes — `00000000ffffffff` |
| `--encoding xcdr1 --endian le` | 8 bytes — `00000000ffffffff` |
| `--encoding xcdr1 --endian be` | 8 bytes — `00000000ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.
