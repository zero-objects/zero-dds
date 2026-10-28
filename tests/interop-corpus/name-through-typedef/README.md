# name-through-typedef

**Construct** — the same type reached through a typedef

One of the name-resolution twins. Every case on this axis declares the
**same structure** with the same values, and differs only in how the
member's type is written. §7.5 admits all of them, so all of them have to
produce the same octets — a difference on this axis is not a matter of
taste, it is a member that did not resolve and therefore never reached
the wire.

The axis exists because that failure is invisible one layer up. A name
that does not resolve leaves the field out of the generated type, the
sample goes out short, and the peer reports a deserialisation error with
no indication of the cause. That is issues #41 and #42, and the reason
they were found by reading a model rather than by a test: of 349 corpus
cases, five used a qualified name and all five wrote the absolute form.

## Twins

`name-absolute` · `name-from-enclosing` · `name-relative` ·
`name-through-typedef` · `name-from-sibling`

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 16 bytes — `7929edff59000000182d4454fb210940` |
| `--encoding xcdr2 --endian be` | 16 bytes — `ffed297900000059400921fb54442d18` |
| `--encoding xcdr1 --endian le` | 16 bytes — `7929edff59000000182d4454fb210940` |
| `--encoding xcdr1 --endian be` | 16 bytes — `ffed297900000059400921fb54442d18` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.
