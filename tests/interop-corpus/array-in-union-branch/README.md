# array-in-union-branch

**Construct** — an array declared inside a union branch

IDL 4.2 §7.4.1.4.4.2.4 lets a branch declare an array, which is an anonymous type: its size is in the type and never on the wire, so the branch is fixed-width while the union is not. The corpus holds an array of unions and a union with a sequence branch, and no array branch.

## The axis

Derived from the specification text rather than from the corpus: a place
where a construct meets a bound, a default, a recursion, an empty case, a
maximum, a collision or another feature. The clause is named above. It is
on the wire axis because the disagreement shows up as octets, and a
refusal by another implementation is a measurement too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 24 bytes — `100000000100000001000000feffffffffffff7fffffffff` |
| `--encoding xcdr2 --endian be` | 24 bytes — `000000100000000100000001fffffffe7fffffffffffffff` |
| `--encoding xcdr1 --endian le` | 20 bytes — `0100000001000000feffffffffffff7fffffffff` |
| `--encoding xcdr1 --endian be` | 20 bytes — `0000000100000001fffffffe7fffffffffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.
