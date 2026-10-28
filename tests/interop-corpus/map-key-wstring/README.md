# map-key-wstring

**Construct** — a `map` keyed by `wstring`

IDL 4.2 §7.4.13 admits a `wstring` key. It is the key type on which the implementations already differ most, because the width of a `wchar` is left to the mapping — `docs/language-limits.md` and `crates/ast-layout` carry four measured widths — and the entries are written in the key's order. The corpus keys a map by `long`, `string`, `boolean`, `char`, an enumeration and `unsigned long long`, never by `wstring`.

## The axis

Derived from the specification text rather than from the corpus: a place
where a construct meets a bound, a default, a recursion, an empty case, a
maximum, a collision or another feature. The clause is named above. It is
on the wire axis because the disagreement shows up as octets, and a
refusal by another implementation is a measurement too.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 48 bytes — `28000000020000000a00000061006c0070006800610000007929edff080000006200650074006100ffffff7fffffffff` |
| `--encoding xcdr2 --endian be` | 48 bytes — `00000028000000020000000a0061006c0070006800610000ffed29790000000800620065007400617fffffffffffffff` |
| `--encoding xcdr1 --endian le` | 44 bytes — `020000000a00000061006c0070006800610000007929edff080000006200650074006100ffffff7fffffffff` |
| `--encoding xcdr1 --endian be` | 44 bytes — `000000020000000a0061006c0070006800610000ffed29790000000800620065007400617fffffffffffffff` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.
