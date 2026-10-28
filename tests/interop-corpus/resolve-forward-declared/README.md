# resolve-forward-declared

**Construct** — a member typed by a forward-declared struct defined later in the file

A struct used before it is defined. Lowering has to leave the name pending and resolve it once the definition arrives; an implementation that resolved eagerly would report the file as broken, and one that never resolved would emit the member without a type.

## The axis

One of the resolution and expansion edge cases: shapes where two
implementations can each do something defensible and produce different
octets. They are here rather than in a front-end test because the
disagreement is **visible on the wire** — a name bound to the wrong
declaration, an alias not followed, a dimension not folded, all of them
change what is written and none of them is reported as an error.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 20 bytes — `100000000c000000010000000400000005000000` |
| `--encoding xcdr2 --endian be` | 20 bytes — `000000100000000c000000010000000400000005` |
| `--encoding xcdr1 --endian le` | 8 bytes — `0100000005000000` |
| `--encoding xcdr1 --endian be` | 8 bytes — `0000000100000005` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.
