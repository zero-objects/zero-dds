# string-unbounded-utf8

**Construct** — unbounded `string`, multi-byte UTF-8

The length prefix counts bytes, not characters. This sample is fourteen characters and twenty-seven UTF-8 bytes, so an implementation that writes a character count produces a length that is wrong by thirteen and a reader that trusts it walks off the end of the member. Includes two-, three-byte and combining sequences. The trailing `long` is -1 so that any padding the string leaves before it reads as zero bytes against a member that is all ones.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 36 bytes — `1c0000006e61c3af766520e2809420c3bcc3b1c3aec3a7c3b8e28882e188b400ffffffff` |
| `--encoding xcdr2 --endian be` | 36 bytes — `0000001c6e61c3af766520e2809420c3bcc3b1c3aec3a7c3b8e28882e188b400ffffffff` |
| `--encoding xcdr1 --endian le` | 36 bytes — `1c0000006e61c3af766520e2809420c3bcc3b1c3aec3a7c3b8e28882e188b400ffffffff` |
| `--encoding xcdr1 --endian be` | 36 bytes — `0000001c6e61c3af766520e2809420c3bcc3b1c3aec3a7c3b8e28882e188b400ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.
