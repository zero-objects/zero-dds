# annotation-key-single

**Construct** — `@key` on one member

The `keyed` row from `vendor_bytes.md` §1: eight bytes, the key having no effect on the data payload at all. What the key changes is the KeyHash, which `--emit keyhash` produces: the key members alone, big-endian, in member-id order, zero-padded to sixteen bytes. Two peers that disagree about that never match an instance.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 8 bytes — `7929edffffffff7f` |
| `--encoding xcdr2 --endian be` | 8 bytes — `ffed29797fffffff` |
| `--encoding xcdr1 --endian le` | 8 bytes — `7929edffffffff7f` |
| `--encoding xcdr1 --endian be` | 8 bytes — `ffed29797fffffff` |
| `--emit keyhash` | 16 bytes — `ffed2979000000000000000000000000` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.
