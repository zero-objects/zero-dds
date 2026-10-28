# annotation-id-sparse

**Construct** — `@id` with gaps and a large value

Member id 0, which is legal and which an implementation using 0 as "unset" loses; 16383, the largest id a short PL_CDR1 parameter id can hold; and 268435455, the largest an EMHEADER's 28-bit field can hold. Each of the three forces a different header form, and `vendor_bytes.md` §2 records the vendors already disagreeing about which form they use for ordinary ids.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 36 bytes — `20000000000000207929edff01000020ffffff7fff3f0020feffffffffffff2f01000000` |
| `--encoding xcdr2 --endian be` | 36 bytes — `0000002020000000ffed2979200000017fffffff20003ffffffffffe2fffffff00000001` |
| `--encoding xcdr1 --endian le` | 52 bytes — `000004007929edff01000400ffffff7f017f0800ff3f000004000000feffffff017f0800ffffff0f0400000001000000027f0000` |
| `--encoding xcdr1 --endian be` | 52 bytes — `00000004ffed2979000100047fffffff7f01000800003fff00000004fffffffe7f0100080fffffff00000004000000017f020000` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.
