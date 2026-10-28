# string-adjacent-widths

**Construct** — `string` and `wstring` side by side

The same three characters as a narrow and as a wide string in one sample. Narrow is six bytes plus a NUL, wide is six bytes with no NUL, and the two lengths therefore differ by one — an implementation that reuses the narrow writer for the wide member produces a sample that is the right length and the wrong content, which no size check catches.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 36 bytes — `06000000c3a974c3a900000006000000e9007400e9000000030000006f6b0000ffffffff` |
| `--encoding xcdr2 --endian be` | 36 bytes — `00000006c3a974c3a90000000000000600e9007400e90000000000036f6b0000ffffffff` |
| `--encoding xcdr1 --endian le` | 36 bytes — `06000000c3a974c3a900000006000000e9007400e9000000030000006f6b0000ffffffff` |
| `--encoding xcdr1 --endian be` | 36 bytes — `00000006c3a974c3a90000000000000600e9007400e90000000000036f6b0000ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.
