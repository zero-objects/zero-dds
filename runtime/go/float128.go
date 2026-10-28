// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

// IEEE 754 binary128, in the standard library.
//
// The Go counterpart of crates/float128, runtime/c/src/float128.c and
// runtime/python/zerodds_wire/float128.py — a further implementation of
// the same clauses, not a binding to any of them. CORBA 3.3 part 2
// §9.3.1.3 with Figure 9.2 gives CDR long double one sign bit, a
// fifteen-bit exponent biased by 16383 and a 112-bit stored fraction;
// DDS-XTypes 1.3 Table 31 names the result quadruple precision, sixteen
// bytes, aligned to eight.
//
// Go has no binary128 and no extended float at all — float64 holds 53
// of the 113 significand bits — so the generated member holds the
// decimal text and the two conversions live here: a correctly rounded
// parse on the way in (to nearest, ties to even, in one rounding) and
// the exact decimal expansion on the way out, in exactly the spelling
// the sibling columns' renderers produce, because the columns are
// compared as text. math/big is what an exact binary-to-decimal
// conversion wants and the standard library has it.

package zdwire

import (
	"math/big"
	"strconv"
	"strings"
)

// Fields of binary128. IEEE 754-2008 clause 3.6.
const (
	f128FractionBits = 112
	f128ExponentBias = 16383
	f128ExponentMax  = 0x7FFF
)

var (
	f128SignMask     = new(big.Int).Lsh(big.NewInt(1), 127)
	f128FractionMask = new(big.Int).Sub(new(big.Int).Lsh(big.NewInt(1), f128FractionBits), big.NewInt(1))
	f128QuietBit     = new(big.Int).Lsh(big.NewInt(1), f128FractionBits-1)
	f128ExponentBits = new(big.Int).Lsh(big.NewInt(f128ExponentMax), f128FractionBits)
)

// f128FromDecimal parses `text` and answers the sixteen bytes,
// least-significant first. A malformed decimal panics with OutOfRange
// at `at`, which the harness turns into the sample's refusal.
func f128FromDecimal(text string, at int) [16]byte {
	refuse := func() {
		panic(OutOfRange{At: at, Expected: "a decimal for a long double, not `" + text + "`"})
	}
	stripped := strings.TrimSpace(text)
	lowered := strings.ToLower(stripped)
	switch lowered {
	case "inf", "+inf", "infinity", "+infinity":
		return f128Bytes(f128ExponentBits)
	case "-inf", "-infinity":
		return f128Bytes(new(big.Int).Or(f128ExponentBits, f128SignMask))
	case "nan", "+nan":
		return f128Bytes(new(big.Int).Or(f128ExponentBits, f128QuietBit))
	case "-nan":
		nan := new(big.Int).Or(f128ExponentBits, f128QuietBit)
		return f128Bytes(nan.Or(nan, f128SignMask))
	}

	negative := false
	body := stripped
	if len(body) > 0 && (body[0] == '+' || body[0] == '-') {
		negative = body[0] == '-'
		body = body[1:]
	}
	mantissa := body
	exponent10 := 0
	if split := strings.IndexAny(body, "eE"); split >= 0 {
		mantissa = body[:split]
		parsed, err := strconv.Atoi(body[split+1:])
		if err != nil {
			refuse()
		}
		exponent10 = parsed
	}
	whole := mantissa
	fraction := ""
	if point := strings.IndexByte(mantissa, '.'); point >= 0 {
		whole = mantissa[:point]
		fraction = mantissa[point+1:]
	}
	digits := whole + fraction
	if digits == "" {
		refuse()
	}
	for i := 0; i < len(digits); i++ {
		// An underscore is refused rather than ignored: in a value
		// file it is a typo, and reading `1_5` as fifteen is a quiet
		// reinterpretation.
		if digits[i] < '0' || digits[i] > '9' {
			refuse()
		}
	}
	significand, ok := new(big.Int).SetString(digits, 10)
	if !ok {
		refuse()
	}
	exponent10 -= len(fraction)

	if significand.Sign() == 0 {
		if negative {
			return f128Bytes(f128SignMask)
		}
		return f128Bytes(big.NewInt(0))
	}
	// The exact rational, then one correct rounding on it. A two-step
	// parse through float64 rounds twice, and double rounding gets the
	// last bit wrong for the values that matter here.
	numerator := new(big.Int).Set(significand)
	denominator := big.NewInt(1)
	if exponent10 >= 0 {
		numerator.Mul(numerator, pow10(exponent10))
	} else {
		denominator = pow10(-exponent10)
	}
	return f128Bytes(f128FromFraction(negative, numerator, denominator))
}

// pow10 answers 10^n for n >= 0.
func pow10(n int) *big.Int {
	return new(big.Int).Exp(big.NewInt(10), big.NewInt(int64(n)), nil)
}

// f128FromFraction rounds numerator/denominator to binary128, to
// nearest with ties to even. Scale so the quotient carries more bits
// than binary128 needs, then fold everything discarded into one sticky
// bit: that is what makes the single rounding at the end the correct
// one.
func f128FromFraction(negative bool, numerator *big.Int, denominator *big.Int) *big.Int {
	const wanted = 130
	shift := wanted - (numerator.BitLen() - denominator.BitLen())
	if shift < 0 {
		shift = 0
	}
	scaled := new(big.Int).Lsh(numerator, uint(shift))
	quotient, remainder := new(big.Int).QuoRem(scaled, denominator, new(big.Int))
	if remainder.Sign() != 0 {
		quotient.Or(quotient, big.NewInt(1))
	}
	bits, _ := f128Compose(negative, quotient, -shift)
	return bits
}

// f128Compose builds a pattern from a sign, an integral significand and
// a binary exponent, rounding to nearest with ties to even. The second
// answer says whether the rounding lost anything.
func f128Compose(negative bool, significand *big.Int, exponent int) (*big.Int, bool) {
	sign := big.NewInt(0)
	if negative {
		sign = new(big.Int).Set(f128SignMask)
	}
	if significand.Sign() == 0 {
		return sign, false
	}
	// Clamped before anything is added: a decimal parser can hand this
	// an exponent of a hundred thousand.
	if exponent > f128ExponentMax {
		return new(big.Int).Or(sign, f128ExponentBits), true
	}
	if exponent < -(f128FractionBits + 1 + f128ExponentBias + 200) {
		return sign, true
	}

	lost := false
	value := new(big.Int).Set(significand)
	leading := value.BitLen() - 1
	if leading > f128FractionBits {
		drop := leading - f128FractionBits
		var dropped bool
		value, dropped = f128ShiftRound(value, drop)
		lost = lost || dropped
		exponent += drop
		if value.BitLen()-1 > f128FractionBits {
			value.Rsh(value, 1)
			exponent++
		}
	} else {
		shift := f128FractionBits - leading
		value.Lsh(value, uint(shift))
		exponent -= shift
	}

	biased := exponent + f128ExponentBias + f128FractionBits
	if biased >= f128ExponentMax {
		return new(big.Int).Or(sign, f128ExponentBits), true
	}
	if biased <= 0 {
		// Subnormal. A value that rounds up to the smallest normal
		// carries out of the fraction into the exponent field on its
		// own, so the fraction is deliberately not masked here.
		rounded, dropped := f128ShiftRound(value, 1-biased)
		return new(big.Int).Or(sign, rounded), lost || dropped
	}
	out := new(big.Int).Lsh(big.NewInt(int64(biased)), f128FractionBits)
	out.Or(out, new(big.Int).And(value, f128FractionMask))
	return out.Or(out, sign), lost
}

// f128ShiftRound shifts right, rounding to nearest with ties to even.
// The second answer says whether anything was discarded.
func f128ShiftRound(value *big.Int, shift int) (*big.Int, bool) {
	if shift <= 0 {
		return value, false
	}
	kept := new(big.Int).Rsh(value, uint(shift))
	mask := new(big.Int).Sub(new(big.Int).Lsh(big.NewInt(1), uint(shift)), big.NewInt(1))
	dropped := new(big.Int).And(value, mask)
	if dropped.Sign() == 0 {
		return kept, false
	}
	half := new(big.Int).Lsh(big.NewInt(1), uint(shift-1))
	if dropped.Cmp(half) > 0 || (dropped.Cmp(half) == 0 && kept.Bit(0) == 1) {
		kept.Add(kept, big.NewInt(1))
	}
	return kept, true
}

// f128Bytes answers the pattern's sixteen bytes, least-significant
// first.
func f128Bytes(bits *big.Int) [16]byte {
	var bigEndian [16]byte
	bits.FillBytes(bigEndian[:])
	var out [16]byte
	for i := 0; i < 16; i++ {
		out[i] = bigEndian[15-i]
	}
	return out
}

// f128ToExactDecimal answers every digit the value has, in the corpus's
// canonical form — the exact expansion and not the shortest one, and in
// exactly the spelling crates/float128 and the sibling runtimes
// produce, because the columns are compared as text.
func f128ToExactDecimal(bytes [16]byte) string {
	var bigEndian [16]byte
	for i := 0; i < 16; i++ {
		bigEndian[i] = bytes[15-i]
	}
	bits := new(big.Int).SetBytes(bigEndian[:])

	negative := bits.Cmp(f128SignMask) >= 0 && new(big.Int).And(bits, f128SignMask).Sign() != 0
	biased := int(new(big.Int).Rsh(bits, f128FractionBits).Int64() & f128ExponentMax)
	fraction := new(big.Int).And(bits, f128FractionMask)

	if biased == f128ExponentMax {
		if fraction.Sign() != 0 {
			return "nan"
		}
		if negative {
			return "-inf"
		}
		return "inf"
	}
	var significand *big.Int
	var exponent int
	if biased == 0 {
		significand = fraction
		exponent = 1 - f128ExponentBias - f128FractionBits
	} else {
		significand = new(big.Int).Or(fraction, new(big.Int).Lsh(big.NewInt(1), f128FractionBits))
		exponent = biased - f128ExponentBias - f128FractionBits
	}
	if significand.Sign() == 0 {
		if negative {
			return "-0.0"
		}
		return "0.0"
	}
	var digits string
	var point int
	if exponent >= 0 {
		digits = new(big.Int).Lsh(significand, uint(exponent)).Text(10)
		point = len(digits)
	} else {
		// m * 2^-k is exactly m * 5^k * 10^-k.
		scale := -exponent
		scaled := new(big.Int).Mul(significand, new(big.Int).Exp(big.NewInt(5), big.NewInt(int64(scale)), nil))
		digits = scaled.Text(10)
		point = len(digits) - scale
	}
	digits = strings.TrimRight(digits, "0")
	if digits == "" {
		digits = "0"
	}
	return f128Render(negative, digits, point)
}

// f128Render spells 0.<digits> * 10^point in the three forms all the
// columns use.
func f128Render(negative bool, digits string, point int) string {
	sign := ""
	if negative {
		sign = "-"
	}
	if 0 < point && point <= 34 {
		if len(digits) <= point {
			return sign + digits + strings.Repeat("0", point-len(digits)) + ".0"
		}
		return sign + digits[:point] + "." + digits[point:]
	}
	if -6 < point && point <= 0 {
		return sign + "0." + strings.Repeat("0", -point) + digits
	}
	mantissa := digits[:1] + "."
	if len(digits) > 1 {
		mantissa += digits[1:]
	} else {
		mantissa += "0"
	}
	exponent := point - 1
	if exponent >= 0 {
		return sign + mantissa + "e+" + strconv.Itoa(exponent)
	}
	return sign + mantissa + "e-" + strconv.Itoa(-exponent)
}
