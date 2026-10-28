# string-bounded-utf8

**Construct** — bounded `string`, multi-byte UTF-8

A bounded string whose byte length exceeds its character count. Both a character-counting bound check and a byte-counting one accept this sample, so it isolates the wire question from the validation question: the bytes must be identical to the unbounded form. The trailing `long` is -1 so that any padding the string leaves before it reads as zero bytes against a member that is all ones.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 28 bytes — `130000006772c3bcc39f6520e2869220e4b896e7958c0000ffffffff` |
| `--encoding xcdr2 --endian be` | 28 bytes — `000000136772c3bcc39f6520e2869220e4b896e7958c0000ffffffff` |
| `--encoding xcdr1 --endian le` | 28 bytes — `130000006772c3bcc39f6520e2869220e4b896e7958c0000ffffffff` |
| `--encoding xcdr1 --endian be` | 28 bytes — `000000136772c3bcc39f6520e2869220e4b896e7958c0000ffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.
