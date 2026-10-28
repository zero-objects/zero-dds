# primitive-wchar-above-bmp

**Construct** — primitive `wchar` holding a code point outside the Basic Multilingual Plane

`wchar` is one UTF-16 code unit. DDS-XTypes 1.3 §7.2.2.2.1.2.5: "The `CHAR_16_TYPE` shall be restricted to representing Unicode codepoints in the Basic Multilingual Plane. That is Unicode codepoints from 0x0000 to U+FFFF." U+1F600 takes two code units and has no `wchar`.

`too_wide` is `😀`, U+1F600. Its low sixteen bits are `0xF600`, a private-use character that reads back without complaint — the same shape as `primitive-char-above-latin1` at twice the width. `Writer::write_wchar` narrowed to it until it was changed to refuse.

There is no `-unannotated` twin, for the reason `primitive-char-above-latin1` gives.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | **no** — Sample.too_wide: expected a character in the Basic Multilingual Plane, found U+1F600 |
| `--encoding xcdr2 --endian be` | **no** — Sample.too_wide: expected a character in the Basic Multilingual Plane, found U+1F600 |
| `--encoding xcdr1 --endian le` | **no** — Sample.too_wide: expected a character in the Basic Multilingual Plane, found U+1F600 |
| `--encoding xcdr1 --endian be` | **no** — Sample.too_wide: expected a character in the Basic Multilingual Plane, found U+1F600 |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.
