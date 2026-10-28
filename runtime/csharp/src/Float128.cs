// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
using System;
using System.Globalization;
using System.Numerics;
using System.Text;

namespace ZeroDds.Idl
{
    /// <summary>
    /// IEEE 754 binary128 — the sixteen octets CDR gives a <c>long double</c>.
    /// </summary>
    /// <remarks>
    /// <para>
    /// The C# counterpart of <c>crates/float128</c>,
    /// <c>runtime/c/src/float128.c</c>, <c>runtime/java/.../Float128.java</c>
    /// and <c>runtime/python/zerodds_wire/float128.py</c>: a fifth
    /// implementation of the same clauses, not a binding to any of them. CORBA
    /// 3.3 part 2 §9.3.1.3 with Figure 9.2 gives the layout — one sign bit, a
    /// fifteen-bit exponent biased by 16383, a 112-bit stored fraction — and
    /// DDS-XTypes 1.3 Table 31 names it: quadruple precision, sixteen bytes,
    /// aligned to eight.
    /// </para>
    /// <para>
    /// <b>Why not <c>decimal</c>.</b> .NET's <c>decimal</c> is a base-ten type
    /// with 28 to 29 significant digits and a scale, not a binary floating
    /// type. <c>1e-30</c> and <c>1e300</c> are both outside it, every value
    /// whose significand needs 113 bits rounds, and there is no signed zero, no
    /// infinity and no NaN. A member typed against it would put a different
    /// number on the wire than the IDL wrote and report success. What is held
    /// here is the <b>pattern</b>, split into its low and high halves, which is
    /// the only holder that loses nothing.
    /// </para>
    /// <para>
    /// <b>Why the pattern and not the value.</b> A finite binary128 is
    /// <c>significand * 2^exponent</c> with the significand an integer under
    /// 2^113, and every binary fraction has a terminating decimal expansion —
    /// so <see cref="ToExactDecimal"/> loses nothing and
    /// <see cref="FromDecimalString"/> rounds once, to nearest, ties to even.
    /// What a decimal cannot carry is a value that is not a number: an
    /// infinity, a NaN, and the distinction between <c>-0.0</c> and
    /// <c>+0.0</c>. Those three live in the pattern, which is why the pattern
    /// is what this type stores and what <see cref="Equals(Float128)"/>
    /// compares.
    /// </para>
    /// <para>
    /// <b>Why it is a <c>struct</c>.</b> Every other generated aggregate in the
    /// C# column is a <c>class</c>, for the reasons
    /// <c>crates/lang-rules/src/adapter/csharp.rs</c> gives. This one is a
    /// primitive: fixed at sixteen octets, never absent and never recursive, so
    /// none of the three reasons applies. <c>readonly</c>, so a copy cannot be
    /// written into by mistake.
    /// </para>
    /// <para>
    /// The namespace is <c>ZeroDds.Idl</c> and not <c>ZeroDds.Wire</c>
    /// deliberately: <c>adapter/csharp.rs</c> writes
    /// <c>global::ZeroDds.Idl.Float128</c> as the member type for a
    /// <c>long double</c>, and a codec that met a second type of the same shape
    /// under a different name would not compile against the declarations. This
    /// is a superset of the struct that materialiser emits into a standalone
    /// file — the same two fields and the same constructor, plus the
    /// conversions the wire path needs.
    /// </para>
    /// <para>
    /// There is no arithmetic. Nothing on the wire path needs it.
    /// </para>
    /// </remarks>
    public readonly struct Float128 : IEquatable<Float128>
    {
        /// <summary>Bits in the stored fraction. IEEE 754-2008 clause 3.6.</summary>
        private const int FractionBits = 112;

        /// <summary>What is added to the unbiased exponent to get the stored one.</summary>
        private const int ExponentBias = 16383;

        /// <summary>The stored exponent of an infinity or a NaN.</summary>
        private const int ExponentMax = 0x7fff;

        /// <summary>Significand bits including the implied leading one.</summary>
        private const int SignificandBits = FractionBits + 1;

        /// <summary>Decimal digits that always round-trip: ceil(113 * log10 2) + 1.</summary>
        public const int DigitsRoundTrip = 36;

        private static readonly BigInteger FractionMask =
            (BigInteger.One << FractionBits) - BigInteger.One;

        private static readonly BigInteger ImpliedOne = BigInteger.One << FractionBits;

        private static readonly BigInteger Five = new BigInteger(5);

        /// <summary>The low 64 bits of the pattern.</summary>
        public readonly ulong Low;

        /// <summary>The high 64 bits: sign, exponent, top fraction.</summary>
        public readonly ulong High;

        /// <summary>The pattern, as its two halves.</summary>
        public Float128(ulong low, ulong high)
        {
            Low = low;
            High = high;
        }

        // -- the named values -------------------------------------------------

        /// <summary>Positive zero.</summary>
        public static Float128 Zero
        {
            get { return new Float128(0UL, 0UL); }
        }

        /// <summary>Negative zero. Distinct from <see cref="Zero"/> on the wire and here.</summary>
        public static Float128 NegativeZero
        {
            get { return new Float128(0UL, 0x8000000000000000UL); }
        }

        /// <summary>One.</summary>
        public static Float128 One
        {
            get { return new Float128(0UL, (ulong)ExponentBias << 48); }
        }

        /// <summary>Positive infinity.</summary>
        public static Float128 PositiveInfinity
        {
            get { return new Float128(0UL, (ulong)ExponentMax << 48); }
        }

        /// <summary>Negative infinity.</summary>
        public static Float128 NegativeInfinity
        {
            get { return new Float128(0UL, ((ulong)ExponentMax << 48) | 0x8000000000000000UL); }
        }

        /// <summary>A quiet NaN with no payload.</summary>
        public static Float128 NaN
        {
            get
            {
                // Bit 111 is the quiet bit; it is bit 47 of the high half.
                return new Float128(0UL, ((ulong)ExponentMax << 48) | (1UL << 47));
            }
        }

        // -- the pattern ------------------------------------------------------

        /// <summary>From the raw 128-bit pattern.</summary>
        /// <exception cref="ArgumentOutOfRangeException">
        /// When the value is negative or wider than 128 bits.
        /// </exception>
        public static Float128 FromBits(BigInteger bits)
        {
            if (bits.Sign < 0 || bits >= BigInteger.One << 128)
            {
                throw new ArgumentOutOfRangeException(
                    nameof(bits), "a binary128 pattern is 128 bits");
            }

            ulong low = (ulong)(bits & ulong.MaxValue);
            ulong high = (ulong)((bits >> 64) & ulong.MaxValue);
            return new Float128(low, high);
        }

        /// <summary>The raw 128-bit pattern.</summary>
        public BigInteger ToBits()
        {
            return ((BigInteger)High << 64) | (BigInteger)Low;
        }

        /// <summary>
        /// From the sixteen bytes in the order named.
        /// </summary>
        /// <remarks>
        /// The two orders are the whole sixteen bytes reversed — CORBA Figure
        /// 9.2 — and not two eight-byte halves swapped in place.
        /// </remarks>
        public static Float128 FromBytes(byte[] data, bool bigEndian)
        {
            if (data == null || data.Length != 16)
            {
                throw new ArgumentException("a binary128 is exactly sixteen bytes", nameof(data));
            }

            byte[] ordered = new byte[16];
            for (int index = 0; index < 16; index++)
            {
                ordered[index] = bigEndian ? data[index] : data[15 - index];
            }

            ulong high = 0;
            ulong low = 0;
            for (int index = 0; index < 8; index++)
            {
                high = (high << 8) | ordered[index];
            }

            for (int index = 8; index < 16; index++)
            {
                low = (low << 8) | ordered[index];
            }

            return new Float128(low, high);
        }

        /// <summary>The sixteen bytes in the order named.</summary>
        public byte[] ToBytes(bool bigEndian)
        {
            byte[] ordered = new byte[16];
            for (int index = 0; index < 8; index++)
            {
                ordered[index] = (byte)(High >> (8 * (7 - index)));
                ordered[index + 8] = (byte)(Low >> (8 * (7 - index)));
            }

            if (bigEndian)
            {
                return ordered;
            }

            byte[] reversed = new byte[16];
            for (int index = 0; index < 16; index++)
            {
                reversed[index] = ordered[15 - index];
            }

            return reversed;
        }

        // -- fields -----------------------------------------------------------

        /// <summary>The sign bit, as stored. True for <c>-0.0</c> and a negative NaN.</summary>
        public bool IsNegative
        {
            get { return (High & 0x8000000000000000UL) != 0; }
        }

        private int BiasedExponent
        {
            get { return (int)((High >> 48) & 0x7fffUL); }
        }

        private BigInteger Fraction
        {
            get { return ToBits() & FractionMask; }
        }

        /// <summary>Whether this is a NaN.</summary>
        public bool IsNaN
        {
            get { return BiasedExponent == ExponentMax && Fraction.Sign != 0; }
        }

        /// <summary>Whether this is an infinity.</summary>
        public bool IsInfinite
        {
            get { return BiasedExponent == ExponentMax && Fraction.Sign == 0; }
        }

        /// <summary>Whether this is a number.</summary>
        public bool IsFinite
        {
            get { return BiasedExponent != ExponentMax; }
        }

        /// <summary>Whether this is a zero of either sign.</summary>
        public bool IsZero
        {
            get { return (High & 0x7fffffffffffffffUL) == 0 && Low == 0; }
        }

        /// <summary>Whether this is a subnormal.</summary>
        public bool IsSubnormal
        {
            get { return BiasedExponent == 0 && Fraction.Sign != 0; }
        }

        // -- the decimal, in both directions ----------------------------------

        /// <summary>
        /// Every digit the value has, in the corpus's canonical spelling.
        /// </summary>
        /// <remarks>
        /// Byte for byte the same text <c>crates/float128</c>,
        /// <c>runtime/c/src/float128_decimal.c</c>,
        /// <c>runtime/java/.../Float128.java</c> and
        /// <c>runtime/python/zerodds_wire/float128.py</c> produce for the same
        /// pattern: the columns are compared as text, so the renderers have to
        /// agree on the form as well as on the value.
        /// </remarks>
        public string ToExactDecimal()
        {
            if (IsNaN)
            {
                return "nan";
            }

            if (IsInfinite)
            {
                return IsNegative ? "-inf" : "inf";
            }

            BigInteger significand = Significand();
            if (significand.Sign == 0)
            {
                return IsNegative ? "-0.0" : "0.0";
            }

            int exponent = Exponent();
            string digits;
            int point;
            if (exponent >= 0)
            {
                digits = (significand << exponent).ToString(CultureInfo.InvariantCulture);
                point = digits.Length;
            }
            else
            {
                digits = (significand * BigInteger.Pow(Five, -exponent))
                    .ToString(CultureInfo.InvariantCulture);
                point = digits.Length + exponent;
            }

            int end = digits.Length;
            while (end > 0 && digits[end - 1] == '0')
            {
                end--;
            }

            digits = end == 0 ? "0" : digits.Substring(0, end);
            return Render(IsNegative, digits, point);
        }

        /// <summary>
        /// From an exact decimal, rounding to nearest with ties to even.
        /// </summary>
        /// <remarks>
        /// Correctly rounded against the exact rational, never through a
        /// <c>double</c>: a two-step conversion rounds twice, and double
        /// rounding gets the last bit wrong for the values this type exists to
        /// carry. Never through <c>decimal</c> either — it holds neither the
        /// range nor the digits.
        /// </remarks>
        public static Float128 FromDecimalString(string text)
        {
            if (text == null)
            {
                throw new ArgumentNullException(nameof(text));
            }

            string trimmed = text.Trim();
            string lowered = trimmed.ToLowerInvariant();
            if (lowered == "inf" || lowered == "+inf" || lowered == "infinity")
            {
                return PositiveInfinity;
            }

            if (lowered == "-inf" || lowered == "-infinity")
            {
                return NegativeInfinity;
            }

            if (lowered == "nan")
            {
                return NaN;
            }

            if (lowered == "-nan")
            {
                Float128 quiet = NaN;
                return new Float128(quiet.Low, quiet.High | 0x8000000000000000UL);
            }

            if (trimmed.IndexOf('_') >= 0)
            {
                throw new FormatException("an underscore is not part of a decimal");
            }

            bool negative;
            BigInteger numerator;
            BigInteger denominator;
            Parse(trimmed, out negative, out numerator, out denominator);
            if (numerator.Sign == 0)
            {
                // The sign is taken from the text, which is the only place a
                // negative zero survives: no decimal type has one.
                return negative ? NegativeZero : Zero;
            }

            return FromRatio(negative, numerator, denominator);
        }

        /// <summary>
        /// The decimal text as a sign and an exact rational.
        /// </summary>
        /// <remarks>
        /// Hand-parsed rather than routed through <c>decimal.Parse</c> or
        /// <c>double.Parse</c>: the first has 28 digits and the second 53 bits,
        /// and every value this type exists for is outside one of the two.
        /// </remarks>
        private static void Parse(
            string text, out bool negative, out BigInteger numerator, out BigInteger denominator)
        {
            int at = 0;
            negative = false;
            if (at < text.Length && (text[at] == '+' || text[at] == '-'))
            {
                negative = text[at] == '-';
                at++;
            }

            StringBuilder digits = new StringBuilder();
            int scale = 0;
            bool seenDigit = false;
            bool seenPoint = false;
            while (at < text.Length)
            {
                char character = text[at];
                if (character >= '0' && character <= '9')
                {
                    digits.Append(character);
                    seenDigit = true;
                    if (seenPoint)
                    {
                        scale++;
                    }

                    at++;
                    continue;
                }

                if (character == '.' && !seenPoint)
                {
                    seenPoint = true;
                    at++;
                    continue;
                }

                break;
            }

            if (!seenDigit)
            {
                throw new FormatException("a decimal needs at least one digit: `" + text + "`");
            }

            int exponent = 0;
            if (at < text.Length && (text[at] == 'e' || text[at] == 'E'))
            {
                at++;
                bool exponentNegative = false;
                if (at < text.Length && (text[at] == '+' || text[at] == '-'))
                {
                    exponentNegative = text[at] == '-';
                    at++;
                }

                int start = at;
                long magnitude = 0;
                while (at < text.Length && text[at] >= '0' && text[at] <= '9')
                {
                    // Capped, because an exponent past this is already an
                    // overflow or an underflow and the cap keeps the
                    // BigInteger.Pow below from being asked for a number no
                    // machine holds.
                    if (magnitude < 1000000)
                    {
                        magnitude = (magnitude * 10) + (text[at] - '0');
                    }

                    at++;
                }

                if (at == start)
                {
                    throw new FormatException("an exponent needs digits: `" + text + "`");
                }

                exponent = exponentNegative ? -(int)magnitude : (int)magnitude;
            }

            if (at != text.Length)
            {
                throw new FormatException("`" + text + "` is not a decimal");
            }

            numerator = BigInteger.Parse(
                digits.Length == 0 ? "0" : digits.ToString(), CultureInfo.InvariantCulture);
            int power = exponent - scale;
            denominator = BigInteger.One;
            if (power > 0)
            {
                numerator *= BigInteger.Pow(10, power);
            }
            else if (power < 0)
            {
                denominator = BigInteger.Pow(10, -power);
            }
        }

        // -- double, with the inexact cases reported --------------------------

        /// <summary>From a <c>double</c>. Always exact: binary128 covers every binary64.</summary>
        public static Float128 FromDouble(double value)
        {
            long raw = BitConverter.DoubleToInt64Bits(value);
            bool negative = raw < 0;
            int biased = (int)((raw >> 52) & 0x7ff);
            long fraction = raw & 0x000fffffffffffffL;
            if (biased == 0x7ff)
            {
                // A NaN keeps its payload, aligned so the double's quiet bit
                // lands on binary128's.
                BigInteger payload = (BigInteger)fraction << 60;
                BigInteger bits = ((BigInteger)ExponentMax << FractionBits) | payload;
                if (negative)
                {
                    bits |= BigInteger.One << 127;
                }

                return FromBits(bits);
            }

            if (biased == 0 && fraction == 0)
            {
                return negative ? NegativeZero : Zero;
            }

            if (biased == 0)
            {
                // A subnormal double becomes a binary128 *normal*: binary128's
                // exponent range swallows binary64's whole subnormal range.
                return Compose(negative, fraction, 1 - 1023 - 52);
            }

            return Compose(negative, fraction | (1L << 52), biased - 1023 - 52);
        }

        /// <summary>To a <c>double</c>, rounding to nearest with ties to even.</summary>
        public double ToDoubleRounded()
        {
            if (IsNaN)
            {
                return double.NaN;
            }

            if (IsInfinite)
            {
                return IsNegative ? double.NegativeInfinity : double.PositiveInfinity;
            }

            if (IsZero)
            {
                return IsNegative ? -0.0 : 0.0;
            }

            // Through the exact decimal rather than through a hand-written
            // narrowing: `double.Parse` is correctly rounded, and a second
            // narrowing written here would be a second place to get it wrong.
            return double.Parse(
                ToExactDecimal(),
                NumberStyles.Float,
                CultureInfo.InvariantCulture);
        }

        /// <summary>
        /// To a <c>double</c> when the value fits one exactly.
        /// </summary>
        /// <exception cref="OverflowException">
        /// Naming the pattern. This is the refusal the <c>long double</c>
        /// column needed: writing a <c>double</c> and eight zero bytes was the
        /// right length and the wrong value.
        /// </exception>
        public double ToDouble()
        {
            double rounded = ToDoubleRounded();
            if (!FromDouble(rounded).Equals(this))
            {
                throw new OverflowException(
                    "the binary128 value 0x" + ToBits().ToString("x", CultureInfo.InvariantCulture)
                    + " does not fit a double exactly");
            }

            return rounded;
        }

        // -- comparison -------------------------------------------------------

        /// <summary>
        /// Bit-pattern equality, which is what a key holder and a corpus
        /// expectation compare: <c>-0.0</c> differs from <c>+0.0</c> and a NaN
        /// equals itself. IEEE equality is <see cref="IeeeEquals"/>, spelled out
        /// so neither meaning is reached by accident.
        /// </summary>
        public bool Equals(Float128 other)
        {
            return Low == other.Low && High == other.High;
        }

        /// <inheritdoc/>
        public override bool Equals(object other)
        {
            return other is Float128 && Equals((Float128)other);
        }

        /// <inheritdoc/>
        public override int GetHashCode()
        {
            return Low.GetHashCode() ^ High.GetHashCode();
        }

        /// <summary>Pattern equality.</summary>
        public static bool operator ==(Float128 left, Float128 right)
        {
            return left.Equals(right);
        }

        /// <summary>Pattern inequality.</summary>
        public static bool operator !=(Float128 left, Float128 right)
        {
            return !left.Equals(right);
        }

        /// <summary>IEEE 754-2008 <c>compareQuietEqual</c>.</summary>
        public bool IeeeEquals(Float128 other)
        {
            if (IsNaN || other.IsNaN)
            {
                return false;
            }

            if (IsZero && other.IsZero)
            {
                return true;
            }

            return Equals(other);
        }

        /// <summary>
        /// A key giving IEEE 754-2008 <c>totalOrder</c> when compared as an
        /// unsigned 128-bit integer.
        /// </summary>
        public BigInteger TotalOrderKey()
        {
            BigInteger bits = ToBits();
            if (IsNegative)
            {
                return bits ^ ((BigInteger.One << 128) - BigInteger.One);
            }

            return bits | (BigInteger.One << 127);
        }

        /// <inheritdoc/>
        public override string ToString()
        {
            return ToExactDecimal();
        }

        // -- the shared decomposition -----------------------------------------

        /// <summary>The integral significand, with the implied one materialised.</summary>
        private BigInteger Significand()
        {
            BigInteger fraction = Fraction;
            return BiasedExponent == 0 ? fraction : fraction | ImpliedOne;
        }

        /// <summary>The binary exponent that scales the significand.</summary>
        private int Exponent()
        {
            int biased = BiasedExponent;
            return (biased == 0 ? 1 : biased) - ExponentBias - FractionBits;
        }

        /// <summary>
        /// Round <c>numerator / denominator</c> to binary128, nearest, ties to
        /// even. The quotient is taken with enough extra bits that the single
        /// rounding at the end is the correct one, and everything discarded is
        /// folded into one sticky bit.
        /// </summary>
        private static Float128 FromRatio(
            bool negative, BigInteger numerator, BigInteger denominator)
        {
            if (numerator.Sign == 0)
            {
                return negative ? NegativeZero : Zero;
            }

            int shift = 130 - (BitLength(numerator) - BitLength(denominator));
            if (shift < 0)
            {
                shift = 0;
            }

            BigInteger remainder;
            BigInteger quotient = BigInteger.DivRem(
                numerator << shift, denominator, out remainder);
            if (remainder.Sign != 0)
            {
                quotient |= BigInteger.One;
            }

            return Compose(negative, quotient, -shift);
        }

        /// <summary>
        /// Build a value from a sign, an integral significand and a binary
        /// exponent, rounding to nearest with ties to even.
        /// </summary>
        private static Float128 Compose(bool negative, BigInteger significand, int exponent)
        {
            BigInteger sign = negative ? BigInteger.One << 127 : BigInteger.Zero;
            if (significand.Sign == 0)
            {
                return FromBits(sign);
            }

            if (exponent > ExponentMax)
            {
                return negative ? NegativeInfinity : PositiveInfinity;
            }

            if (exponent < -(SignificandBits + ExponentBias + 200))
            {
                return FromBits(sign);
            }

            int leading = BitLength(significand) - 1;
            if (leading > FractionBits)
            {
                int drop = leading - FractionBits;
                significand = ShiftRound(significand, drop);
                exponent += drop;
                if (BitLength(significand) - 1 > FractionBits)
                {
                    significand >>= 1;
                    exponent += 1;
                }
            }
            else
            {
                int shift = FractionBits - leading;
                significand <<= shift;
                exponent -= shift;
            }

            int biased = exponent + ExponentBias + FractionBits;
            if (biased >= ExponentMax)
            {
                return negative ? NegativeInfinity : PositiveInfinity;
            }

            if (biased <= 0)
            {
                // Subnormal. A value that rounds up to the smallest normal
                // carries out of the fraction into the exponent field on its
                // own, so the fraction is deliberately not masked here.
                BigInteger rounded = ShiftRound(significand, 1 - biased);
                return FromBits(sign | rounded);
            }

            return FromBits(
                sign
                | ((BigInteger)biased << FractionBits)
                | (significand & FractionMask));
        }

        /// <summary>Shift right, rounding to nearest with ties to even.</summary>
        private static BigInteger ShiftRound(BigInteger value, int shift)
        {
            if (shift <= 0)
            {
                return value;
            }

            BigInteger kept = value >> shift;
            BigInteger dropped = value - (kept << shift);
            if (dropped.Sign == 0)
            {
                return kept;
            }

            BigInteger half = BigInteger.One << (shift - 1);
            int order = dropped.CompareTo(half);
            if (order > 0 || (order == 0 && !(kept & BigInteger.One).IsZero))
            {
                kept += BigInteger.One;
            }

            return kept;
        }

        /// <summary>
        /// How many bits a non-negative value occupies.
        /// </summary>
        /// <remarks>
        /// <c>BigInteger</c> has no <c>bitLength</c>, and
        /// <c>GetByteCount</c> rounds up to an octet. Counted here so the
        /// rounding above is over the same number the Java and Rust columns
        /// round over.
        /// </remarks>
        private static int BitLength(BigInteger value)
        {
            if (value.Sign <= 0)
            {
                return 0;
            }

            int bits = 0;
            BigInteger rest = value;
            // A byte at a time first, so a 113-bit significand takes fourteen
            // steps and not a hundred and thirteen.
            while (rest > byte.MaxValue)
            {
                rest >>= 8;
                bits += 8;
            }

            int tail = (int)rest;
            while (tail > 0)
            {
                tail >>= 1;
                bits++;
            }

            return bits;
        }

        /// <summary><c>0.&lt;digits&gt; * 10^point</c>, in the three forms all columns use.</summary>
        private static string Render(bool negative, string digits, int point)
        {
            StringBuilder outText = new StringBuilder();
            if (negative)
            {
                outText.Append('-');
            }

            if (point > 0 && point <= 34)
            {
                if (digits.Length <= point)
                {
                    outText.Append(digits);
                    for (int index = digits.Length; index < point; index++)
                    {
                        outText.Append('0');
                    }

                    outText.Append(".0");
                }
                else
                {
                    outText.Append(digits, 0, point).Append('.').Append(digits.Substring(point));
                }

                return outText.ToString();
            }

            if (point <= 0 && point > -6)
            {
                outText.Append("0.");
                for (int index = 0; index < -point; index++)
                {
                    outText.Append('0');
                }

                return outText.Append(digits).ToString();
            }

            outText.Append(digits[0]).Append('.');
            outText.Append(digits.Length == 1 ? "0" : digits.Substring(1));
            int exponent = point - 1;
            outText.Append('e').Append(exponent >= 0 ? '+' : '-')
                .Append(Math.Abs(exponent).ToString(CultureInfo.InvariantCulture));
            return outText.ToString();
        }
    }
}
