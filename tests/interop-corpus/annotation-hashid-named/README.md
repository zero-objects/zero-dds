# annotation-hashid-named

**Construct** — `@hashid("name")`

The argument replaces the name that is hashed, so `renamed` carries the id a member actually called `alpha` would have — the mechanism that lets a member be renamed in a later version without breaking the wire. Measured: `renamed` gets 54728492 (0x0343172C), which is what `@hashid` on a member named `alpha` also produces, and which Cyclone DDS, RTI Connext 7.7 and Fast DDS all three write into the EMHEADER for the same source. Adding such a member to this type is rejected by the frontend as `DuplicateMemberId`, which is the behaviour XTypes 1.3 §7.3.1.2.1.1 requires and the reason the colliding pair is not a case of its own.

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
