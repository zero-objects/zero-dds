# annotation-hashid-implicit

**Construct** — `@hashid` with no argument

`@hashid` derives the member id from the member's name: the first four bytes of the MD5 of the name read as a little-endian `uint32`, masked to 28 bits, per XTypes 1.3 §7.3.1.2.1.1. Two hashed members and one that falls back to the sequential rule, so a wrong hash and a wrong fallback are distinguishable. Get the mask or the byte order of the digest wrong and every peer sees different members — this project read the digest big-endian until the three vendors were measured against the clause's own worked examples, `crates/ast-layout/tests/fixtures/vendor_bytes.md`.

The third member is where the vendors part: `plain` follows two hashed members, and RTI Connext gives it id 0 rather than the previous member's id plus one.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 28 bytes — `180000002c1743237929edff987bca20ffffff7f997bca20feffffff` |
| `--encoding xcdr2 --endian be` | 28 bytes — `000000182343172cffed297920ca7b987fffffff20ca7b99fffffffe` |
| `--encoding xcdr1 --endian le` | 52 bytes — `017f08002c174303040000007929edff017f0800987bca0004000000ffffff7f017f0800997bca0004000000feffffff027f0000` |
| `--encoding xcdr1 --endian be` | 52 bytes — `7f0100080343172c00000004ffed29797f01000800ca7b98000000047fffffff7f01000800ca7b9900000004fffffffe7f020000` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.
