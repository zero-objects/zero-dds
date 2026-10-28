# annotation-key-multiple

**Construct** — `@key` on several members

Three keys, not adjacent, and one of them variable-length. The KeyHash takes the keys in member-id order and not in the order they appear, and once the key holder exceeds sixteen bytes the hash is the MD5 of it rather than the bytes themselves — DDSI-RTPS 2.5 §9.6.3.8. A variable-length key is what pushes it over.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 20 bytes — `7929edffffffff7f06000000c3bc62657200feff` |
| `--encoding xcdr2 --endian be` | 20 bytes — `ffed29797fffffff00000006c3bc62657200fffe` |
| `--encoding xcdr1 --endian le` | 20 bytes — `7929edffffffff7f06000000c3bc62657200feff` |
| `--encoding xcdr1 --endian be` | 20 bytes — `ffed29797fffffff00000006c3bc62657200fffe` |
| `--emit keyhash` | 16 bytes — `22ac5facfdc0ea087a1d37ca96ef6cb6` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.
