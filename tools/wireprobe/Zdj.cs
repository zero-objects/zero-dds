// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The canonical-JSON helpers the C# probe's decode direction calls. The
// C# counterpart of `tools/wireprobe/Zdj.java` and of
// `tools/wireprobe/dumpjson.h`, printing the same form for the same
// value — a difference between two columns' answers must be a difference
// in the bytes and not in how the two were printed.
//
// Two things have to be right here, because no reader can recover them:
// a 64-bit integer is a **string**, since a JSON number is a double and
// loses the low bits past 2^53; and an absent `@optional` member is a
// key that was never printed, never a `null`.

using System;
using System.Globalization;
using System.IO;
using System.Text;

internal static class Zdj
{
    // UTF-8 named outright, and no BOM.
    //
    // `Console.Out` encodes with the console's encoding, which under
    // `LANG=C` is ASCII: every character above U+007F becomes `?` before
    // it ever reaches the pipe. That turned a JacORB run's own output
    // into a vendor finding once — measured 2026-08-10 — and it is the
    // failure this harness exists to avoid. A `StreamWriter` over the
    // standard output stream with `new UTF8Encoding(false)` passes the
    // same octets `dumpjson.h`'s `zdj_string` does.
    private static readonly StreamWriter Out = new StreamWriter(
        Console.OpenStandardOutput(), new UTF8Encoding(false))
    {
        AutoFlush = false,
    };

    internal static void Flush()
    {
        Out.Flush();
    }

    internal static void Raw(string text)
    {
        Out.Write(text);
    }

    // The separator and the key together: the comma belongs to the
    // second and later members, and only the printer knows which those
    // are. A one-element array is the mutable flag a C# local passed by
    // value is not.
    internal static void Key(bool[] first, string name)
    {
        if (!first[0])
        {
            Out.Write(",");
        }

        first[0] = false;
        Out.Write("\"" + name + "\":");
    }

    internal static void Bool(bool value)
    {
        Out.Write(value ? "true" : "false");
    }

    internal static void I64(long value)
    {
        Out.Write(value.ToString(CultureInfo.InvariantCulture));
    }

    internal static void U64(ulong value)
    {
        Out.Write(value.ToString(CultureInfo.InvariantCulture));
    }

    internal static void I64String(long value)
    {
        Out.Write("\"" + value.ToString(CultureInfo.InvariantCulture) + "\"");
    }

    internal static void U64String(ulong value)
    {
        Out.Write("\"" + value.ToString(CultureInfo.InvariantCulture) + "\"");
    }

    // The shortest decimal that reads back as the same value, which is
    // what .NET Core 3.0 and later make `ToString()` produce for a
    // `float` and a `double`. The Rust side reduces both sides to that
    // same form before anything is compared, so the spelling here only
    // has to lose nothing.
    //
    // **Not `ToString("G17")`**, which is the same trap Java's `%.17g`
    // is: it pads the shortest form out to seventeen digits, and π's
    // nearest double then sits close enough to a midpoint that a JSON
    // reader lands on the neighbouring double — a one-ULP difference the
    // round trip reports as a disagreement about the sample.
    internal static void F32(float value)
    {
        Out.Write(value.ToString("R", CultureInfo.InvariantCulture));
    }

    internal static void F64(double value)
    {
        Out.Write(value.ToString("R", CultureInfo.InvariantCulture));
    }

    // An IDL `char` is one octet, so 0xE4 is U+00E4 — the same mapping
    // the Rust reader makes. The C# mapping already handed it over as a
    // `char`, so the octet is the low eight bits.
    internal static void Ch(char value)
    {
        Out.Write("\"\\u" + ((int)value & 0xFF).ToString("x4", CultureInfo.InvariantCulture) + "\"");
    }

    internal static void Wch(char value)
    {
        Out.Write("\"\\u" + ((int)value).ToString("x4", CultureInfo.InvariantCulture) + "\"");
    }

    // A narrow string. A null is printed as `null`, which the canonical
    // form refuses — that is the point: the member arrived unset and the
    // comparison has to say so rather than substitute an empty string.
    internal static void String(string text)
    {
        if (text == null)
        {
            Out.Write("null");
            return;
        }

        StringBuilder built = new StringBuilder("\"");
        foreach (char c in text)
        {
            switch (c)
            {
                case '"': built.Append("\\\""); break;
                case '\\': built.Append("\\\\"); break;
                case '\n': built.Append("\\n"); break;
                case '\r': built.Append("\\r"); break;
                case '\t': built.Append("\\t"); break;
                default:
                    if (c < 0x20)
                    {
                        built.Append("\\u")
                            .Append(((int)c).ToString("x4", CultureInfo.InvariantCulture));
                    }
                    else
                    {
                        built.Append(c);
                    }

                    break;
            }
        }

        Out.Write(built.Append('"').ToString());
    }

    // A wide string, as the UTF-16 code units it is made of.
    //
    // A .NET `string` is UTF-16, so a code point above the Basic
    // Multilingual Plane is already the surrogate pair §7.4.1.1.2 puts on
    // the wire, and indexing walks the units directly. Escaping each unit
    // is exactly what JSON's own escape carries, so no UTF-8 encoder is
    // involved and none can be got wrong.
    internal static void Wstring(string text)
    {
        if (text == null)
        {
            Out.Write("null");
            return;
        }

        StringBuilder built = new StringBuilder("\"");
        foreach (char unit in text)
        {
            built.Append("\\u")
                .Append(((int)unit).ToString("x4", CultureInfo.InvariantCulture));
        }

        Out.Write(built.Append('"').ToString());
    }

    // A `fixed<P,S>`, as the decimal text the column holds it as.
    //
    // `adapter/csharp.rs` says why it is a `string` and not a `decimal`:
    // a `decimal` carries 28 to 29 significant digits where IDL allows
    // 31, and it normalises away the leading zeros the
    // `FixedDigitCount` axis exists to measure. The Rust side normalises
    // to the declared scale before comparing.
    internal static void Fixed(string value)
    {
        if (value == null)
        {
            Out.Write("null");
            return;
        }

        Out.Write("\"" + value + "\"");
    }

    // A value that names no enumerator. Printed as a string no
    // enumerator has, so the comparison refuses it by name — a bare
    // number here would read as some other member's value and pass.
    internal static void EnumUnknown(long value)
    {
        Out.Write("\"<enumerator " + value.ToString(CultureInfo.InvariantCulture) + ">\"");
    }
}
