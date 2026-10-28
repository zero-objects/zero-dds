# annotation-external-optional-present

**Construct** — `@optional @external` on one member, with a value

The two annotations answer different questions — DDS-XTypes 1.3 Table 21 says where the binding holds the value, §7.2.2.4.4.4.7 whether it is there at all — and neither changes an octet: the member must frame exactly as the plain `@optional` member of `annotation-optional-present` does. The combination is where a binding that folds the two into one holder writes a state the wire form has no encoding for.

## The axis

Derived from the specification text rather than from the corpus: a place
where a construct meets a bound, a default, a recursion, an empty case, a
maximum, a collision or another feature. The clause is named above. It is
on the wire axis because the disagreement shows up as octets, and a
refusal by another implementation is a measurement too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 16 bytes — `7929edff01000000ffffff7ffeffffff` |
| `--encoding xcdr2 --endian be` | 16 bytes — `ffed2979010000007ffffffffffffffe` |
| `--encoding xcdr1 --endian le` | 16 bytes — `7929edff01000400ffffff7ffeffffff` |
| `--encoding xcdr1 --endian be` | 16 bytes — `ffed2979000100047ffffffffffffffe` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.
