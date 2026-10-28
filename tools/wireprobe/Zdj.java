// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The canonical-JSON helpers the JacORB probe's decode direction calls.
// The Java counterpart of `tools/wireprobe/dumpjson.h`, printing the
// same form for the same value — a difference between JacORB's answer
// and a DDS vendor's must be a difference in the bytes and not in how
// the two were printed.
//
// Two things have to be right here, because no reader can recover them:
// a 64-bit integer is a **string**, since a JSON number is a double and
// loses the low bits past 2^53; and every unsigned type has already been
// widened by the caller, because Java has none.

import java.io.PrintStream;

final class Zdj {
    /** UTF-8 named outright, not the platform default.
     *
     *  `System.out` encodes with the default charset, which under
     *  `LANG=C` is US-ASCII: every character above U+007F becomes `?`
     *  before it ever reaches the pipe. That made a JacORB run report
     *  `"gr??e ? ??"` for a string JacORB had read correctly — the
     *  probe's own output turned into a vendor finding, which is the
     *  failure this whole harness exists to avoid. Measured
     *  2026-08-10.
     *
     *  `dumpjson.h`'s `zdj_string` passes UTF-8 octets through
     *  untouched, so this is the same form the C and C++ columns
     *  produce. */
    private static final PrintStream OUT =
            new PrintStream(new java.io.FileOutputStream(java.io.FileDescriptor.out),
                            true, java.nio.charset.StandardCharsets.UTF_8);

    private Zdj() {}

    static void raw(String text) {
        OUT.print(text);
    }

    /** The separator and the key together: the comma belongs to the
     *  second and later members, and only the printer knows which those
     *  are. A one-element array is the mutable flag Java has no `int *`
     *  for. */
    static void key(boolean[] first, String name) {
        if (!first[0]) {
            OUT.print(",");
        }
        first[0] = false;
        OUT.print("\"" + name + "\":");
    }

    static void bool(boolean value) {
        OUT.print(value ? "true" : "false");
    }

    static void i64(long value) {
        OUT.print(Long.toString(value));
    }

    static void u64(long value) {
        OUT.print(Long.toUnsignedString(value));
    }

    static void i64String(long value) {
        OUT.print("\"" + Long.toString(value) + "\"");
    }

    static void u64String(long value) {
        OUT.print("\"" + Long.toUnsignedString(value) + "\"");
    }

    /** The shortest decimal that reads back as the same value, which is
     *  what `Double.toString` and `Float.toString` are defined to
     *  produce. The Rust side reduces both sides to that same form
     *  before anything is compared, so the spelling here only has to
     *  lose nothing.
     *
     *  **Not `String.format("%.17g", …)`, which loses a value.** Java's
     *  `%g` does not expand the double: it rounds
     *  `Double.toString`'s shortest form to the requested precision and
     *  pads with zeros. π's nearest double comes out as
     *  `3.1415926535897930` where its seventeenth significant digit is
     *  a 1, and that decimal sits close enough to the midpoint that
     *  `serde_json` reads it back as the neighbouring double — a
     *  one-ULP difference the round trip then reports as a disagreement
     *  about the sample. Measured against
     *  `tests/interop-corpus/struct-appendable`. */
    static void f32(float value) {
        OUT.print(Float.toString(value));
    }

    static void f64(double value) {
        OUT.print(Double.toString(value));
    }

    /** An IDL `char` is one octet, so 0xE4 is U+00E4 — the same mapping
     *  the Rust reader makes. The Java mapping already handed it over as
     *  a `char`, so the octet is the low eight bits. */
    static void ch(char value) {
        OUT.print(String.format("\"\\u%04x\"", (int) value & 0xFF));
    }

    static void wch(char value) {
        OUT.print(String.format("\"\\u%04x\"", (int) value));
    }

    /** A narrow string. A null is printed as `null`, which the canonical
     *  form refuses — that is the point: the member arrived unset and the
     *  comparison has to say so rather than substitute an empty
     *  string. */
    static void string(String text) {
        if (text == null) {
            OUT.print("null");
            return;
        }
        StringBuilder out = new StringBuilder("\"");
        for (int i = 0; i < text.length(); i++) {
            char c = text.charAt(i);
            switch (c) {
                case '"': out.append("\\\""); break;
                case '\\': out.append("\\\\"); break;
                case '\n': out.append("\\n"); break;
                case '\r': out.append("\\r"); break;
                case '\t': out.append("\\t"); break;
                default:
                    if (c < 0x20) {
                        out.append(String.format("\\u%04x", (int) c));
                    } else {
                        out.append(c);
                    }
                    break;
            }
        }
        OUT.print(out.append('"').toString());
    }

    /** A wide string, as the UTF-16 code units it is made of.
     *
     *  A Java `String` is UTF-16, so a code point above the Basic
     *  Multilingual Plane is already the surrogate pair §7.4.1.1.2 puts
     *  on the wire and `charAt` walks the units directly. Escaping each
     *  unit is exactly what JSON's own escape carries, so no UTF-8
     *  encoder is involved and none can be got wrong. */
    static void wstring(String text) {
        if (text == null) {
            OUT.print("null");
            return;
        }
        StringBuilder out = new StringBuilder("\"");
        for (int i = 0; i < text.length(); i++) {
            out.append(String.format("\\u%04x", (int) text.charAt(i)));
        }
        OUT.print(out.append('"').toString());
    }

    /** A `fixed<P,S>`, as the decimal the canonical form carries. The
     *  digits are JacORB's; the Rust side normalises to the declared
     *  scale before comparing. */
    static void fixed(java.math.BigDecimal value) {
        if (value == null) {
            OUT.print("null");
            return;
        }
        OUT.print("\"" + value.toPlainString() + "\"");
    }

    /** The same value where the column holds it as the decimal text
     *  itself.
     *
     *  The ZeroDDS Java column does, and `adapter/java.rs` says why:
     *  `new BigDecimal("0.00").precision()` is 1, not 3, so a
     *  `BigDecimal` holder has already decided the digit count the
     *  `FixedDigitCount` axis exists to measure. An overload rather
     *  than a second name, so both columns print through one
     *  helper. */
    static void fixed(String value) {
        if (value == null) {
            OUT.print("null");
            return;
        }
        OUT.print("\"" + value + "\"");
    }

    /** A value that names no enumerator. Printed as a string no
     *  enumerator has, so the comparison refuses it by name — a bare
     *  number here would read as some other member's value and pass. */
    static void enumUnknown(long value) {
        OUT.print("\"<enumerator " + value + ">\"");
    }
}
