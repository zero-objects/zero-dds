# primitive-char-above-latin1

**Construct** — primitive `char` holding a code point wider than one byte

`char` is eight bits. DDS-XTypes 1.3 §7.2.2.2.1.2.2: "This specification does not define an encoding for the `CHAR_8_TYPE`. The only constraint is that it shall be representable using 8 bits." U+0161 is not, so there is no byte that stands for it and the only correct answer is a refusal.

`too_wide` is `š`, U+0161. Its low byte is `0x61`, which is `a` — a byte every reader accepts. That is what makes the case worth having: the sample stays the length the plan says, the member after it lands where it should, and the only thing wrong is that the character on the wire is not the character in the value. `Writer::write_char` wrote that byte until it was changed to refuse; no case in this corpus reached it, because every other `char` sample here is ASCII.

There is no `-unannotated` twin. The twin isolates the vendor default for extensibility, and this case is refused under all four combinations, so there is nothing for the default to change.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | **no** — Sample.too_wide: expected a character below U+0100, found U+0161 |
| `--encoding xcdr2 --endian be` | **no** — Sample.too_wide: expected a character below U+0100, found U+0161 |
| `--encoding xcdr1 --endian le` | **no** — Sample.too_wide: expected a character below U+0100, found U+0161 |
| `--encoding xcdr1 --endian be` | **no** — Sample.too_wide: expected a character below U+0100, found U+0161 |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.
