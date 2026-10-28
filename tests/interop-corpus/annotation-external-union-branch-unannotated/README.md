# annotation-external-union-branch-unannotated

**Construct** — `@external` on a union branch, with the aggregate extensibilities left implicit

The twin of `annotation-external-union-branch` without `@appendable` and `@final` on the aggregates: what an implementation defaults to is part of the measurement, and the branch's `@external` must change no octet under the defaulted extensibility either.

## The axis

Derived from the specification text rather than from the corpus: a place
where a construct meets a bound, a default, a recursion, an empty case, a
maximum, a collision or another feature. The clause is named above. It is
on the wire axis because the disagreement shows up as octets, and a
refusal by another implementation is a measurement too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 28 bytes — `180000000e00000001000000060000007929edfffeff000001000000` |
| `--encoding xcdr2 --endian be` | 28 bytes — `000000180000000e0000000100000006ffed2979fffe000000000001` |
| `--encoding xcdr1 --endian le` | 16 bytes — `010000007929edfffeff000001000000` |
| `--encoding xcdr1 --endian be` | 16 bytes — `00000001ffed2979fffe000000000001` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.
