# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
"""IEEE 754 binary128, in the standard library.

The Python counterpart of `crates/float128` and `runtime/c/src/float128.c`
— a third implementation of the same clauses, not a binding to either.

CORBA 3.3 part 2 §9.3.1.3 with Figure 9.2 gives CDR `long double` one
sign bit, a fifteen-bit exponent biased by 16383 and a 112-bit stored
fraction, and DDS-XTypes 1.3 Table 31 names the result: quadruple
precision, sixteen bytes, aligned to eight. A Python `float` is a double
and holds 53 of the 113 significand bits, so this runtime used to write a
double and eight zero bytes for it — the right length, the wrong value,
in silence.

## Why not `numpy.longdouble`

Because it is not binary128, and on two of this project's three platforms
it cannot carry a `long double` at all. Measured:

    Linux x86-64, numpy 2.5.2   dtype `float128`, itemsize 16,
                                `finfo.nmant` **63** — x87 80-bit
                                extended. `np.longdouble(1.5).tobytes()`
                                is `00000000000000c0ff3f` followed by six
                                bytes of uninitialised memory, the same
                                leak Fast DDS was measured writing.
                                `1 + 2^-60` survives; `1 + 2^-100` does
                                not.
    macOS arm64, numpy 2.4.2    dtype `float64`, itemsize **8**,
                                `finfo.nmant` 52. `np.float128` does not
                                exist.

`numpy.float128` is an alias for the platform's `long double` and a
promise about its *size*, never about its format. Taking numpy as this
runtime's first dependency would buy the x86 dialect, the padding leak
and a platform where the type is missing. `docs/float128-column.md` §3
has the runs.

Python needs no dependency for this: `int` is arbitrary precision, which
is the only thing an exact binary-to-decimal conversion wants.
"""

from __future__ import annotations

import struct
from decimal import Context, Decimal, localcontext
from fractions import Fraction

__all__ = ["Float128", "Inexact"]

# Fields of binary128. IEEE 754-2008 clause 3.6.
_FRACTION_BITS = 112
_EXPONENT_BIAS = 16383
_EXPONENT_MAX = 0x7FFF
_FRACTION_MASK = (1 << _FRACTION_BITS) - 1
_SIGN_MASK = 1 << 127
_QUIET_BIT = 1 << (_FRACTION_BITS - 1)
_SIGNIFICAND_BITS = _FRACTION_BITS + 1


class Inexact(ValueError):
    """A conversion that could not be exact.

    Carries the rounded result, so a caller that wants it after all takes
    it from the exception rather than repeating the conversion with a
    different function and hoping the two agree.
    """

    def __init__(self, rounded: object, source: "Float128") -> None:
        super().__init__(
            f"the binary128 value 0x{source.bits:032x} does not fit the "
            f"destination type exactly"
        )
        self.rounded = rounded
        self.source = source


class Float128:
    """An IEEE 754-2008 binary128 value.

    Held as the 128-bit pattern, because that is what goes on the wire and
    every operation here is defined on it.

    Equality is by **bit pattern**, not IEEE: two `Float128` objects are
    equal when their sixteen bytes are, so `-0.0` and `+0.0` differ and a
    NaN equals itself. That is what a dict key and a corpus expectation
    want, and it is what makes the class hashable. IEEE comparison is
    `ieee_eq`, spelled out so that neither meaning is reached by accident.

    No arithmetic. Nothing in the wire path needs it and a half-correct
    `__add__` is worse than none; `docs/float128-column.md` §6 says what
    adding it would take.
    """

    __slots__ = ("bits",)

    #: Decimal digits that always round-trip: ceil(113 * log10 2) + 1.
    DIGITS_ROUND_TRIP = 36

    def __init__(self, bits: int) -> None:
        if not 0 <= bits < (1 << 128):
            raise ValueError("a binary128 pattern is 128 bits")
        self.bits = bits

    # -- the sixteen bytes ------------------------------------------------

    @classmethod
    def from_bytes(cls, data: bytes, byteorder: str = "little") -> "Float128":
        """From the sixteen bytes in the order named.

        The two orders are the whole sixteen bytes reversed — CORBA
        Figure 9.2 — and not two eight-byte halves swapped in place.
        """
        if len(data) != 16:
            raise ValueError("a binary128 is exactly sixteen bytes")
        return cls(int.from_bytes(data, byteorder))  # type: ignore[arg-type]

    def to_bytes_ordered(self, byteorder: str = "little") -> bytes:
        """The sixteen bytes in the order named."""
        return self.bits.to_bytes(16, byteorder)  # type: ignore[arg-type]

    # -- fields -----------------------------------------------------------

    @property
    def negative(self) -> bool:
        """The sign bit, as stored. True for `-0.0` and a negative NaN."""
        return bool(self.bits & _SIGN_MASK)

    @property
    def _biased_exponent(self) -> int:
        return (self.bits >> _FRACTION_BITS) & _EXPONENT_MAX

    @property
    def _fraction(self) -> int:
        return self.bits & _FRACTION_MASK

    def is_nan(self) -> bool:
        return self._biased_exponent == _EXPONENT_MAX and self._fraction != 0

    def is_infinite(self) -> bool:
        return self._biased_exponent == _EXPONENT_MAX and self._fraction == 0

    def is_finite(self) -> bool:
        return self._biased_exponent != _EXPONENT_MAX

    def is_zero(self) -> bool:
        return self.bits & ~_SIGN_MASK == 0

    def is_subnormal(self) -> bool:
        return self._biased_exponent == 0 and self._fraction != 0

    def decompose(self) -> tuple[bool, int, int] | None:
        """`(negative, significand, exponent)` with
        `value = (-1)^negative * significand * 2^exponent`.

        `None` for an infinity or a NaN. The significand is integral —
        the implied leading one is materialised for a normal value — so
        the triple needs no separate subnormal flag to interpret.
        """
        if not self.is_finite():
            return None
        biased = self._biased_exponent
        fraction = self._fraction
        if biased == 0:
            return (self.negative, fraction, 1 - _EXPONENT_BIAS - _FRACTION_BITS)
        return (
            self.negative,
            fraction | (1 << _FRACTION_BITS),
            biased - _EXPONENT_BIAS - _FRACTION_BITS,
        )

    # -- constants --------------------------------------------------------

    @classmethod
    def zero(cls, negative: bool = False) -> "Float128":
        return cls(_SIGN_MASK if negative else 0)

    @classmethod
    def infinity(cls, negative: bool = False) -> "Float128":
        bits = _EXPONENT_MAX << _FRACTION_BITS
        return cls(bits | _SIGN_MASK if negative else bits)

    @classmethod
    def quiet_nan(cls) -> "Float128":
        return cls((_EXPONENT_MAX << _FRACTION_BITS) | _QUIET_BIT)

    # -- conversion in ----------------------------------------------------

    @classmethod
    def from_float(cls, value: float) -> "Float128":
        """From a Python `float`. Always exact: binary128 covers every
        binary64 value, subnormals included, with 60 bits to spare."""
        (raw,) = struct.unpack("<Q", struct.pack("<d", value))
        negative = bool(raw >> 63)
        biased = (raw >> 52) & 0x7FF
        fraction = raw & 0x000F_FFFF_FFFF_FFFF
        sign = _SIGN_MASK if negative else 0
        if biased == 0x7FF:
            # A NaN keeps its payload, aligned so the double's quiet bit
            # lands on binary128's. An infinity keeps its emptiness.
            return cls(sign | (_EXPONENT_MAX << _FRACTION_BITS) | (fraction << 60))
        if biased == 0 and fraction == 0:
            return cls(sign)
        if biased == 0:
            # A subnormal double becomes a binary128 *normal*.
            return cls._compose(negative, fraction, 1 - 1023 - 52)[0]
        return cls._compose(negative, fraction | (1 << 52), biased - 1023 - 52)[0]

    @classmethod
    def from_int(cls, value: int) -> "Float128":
        """From a Python `int`, refusing one that needs more than 113
        significand bits.

        Raises:
            Inexact: carrying what it rounds to.
        """
        rounded, lost = cls._compose(value < 0, abs(value), 0)
        if lost:
            raise Inexact(rounded, rounded)
        return rounded

    @classmethod
    def from_decimal(cls, text: str | Decimal | Fraction) -> "Float128":
        """From an exact decimal, correctly rounded to nearest, ties to
        even.

        Never through a `float`: a two-step parse rounds twice, and double
        rounding gets the last bit wrong for the values that matter here.
        `Fraction` gives the exact rational and the rounding is done on
        that.
        """
        if isinstance(text, str):
            stripped = text.strip()
            lowered = stripped.lower()
            if lowered in ("inf", "+inf", "infinity", "+infinity"):
                return cls.infinity()
            if lowered in ("-inf", "-infinity"):
                return cls.infinity(negative=True)
            if lowered in ("nan", "+nan", "-nan"):
                nan = cls.quiet_nan()
                return cls(nan.bits | _SIGN_MASK) if lowered.startswith("-") else nan
            negative, significand, exponent = _parse_decimal(stripped)
            if significand == 0:
                return cls.zero(negative)
            if exponent >= 0:
                exact = Fraction(significand * 10**exponent)
            else:
                exact = Fraction(significand, 10 ** -exponent)
        else:
            exact = Fraction(text)
            negative = exact < 0
        return cls._from_fraction(exact, negative)

    @classmethod
    def _from_fraction(cls, exact: Fraction, negative: bool) -> "Float128":
        magnitude = abs(exact)
        if magnitude == 0:
            return cls.zero(negative)
        numerator = magnitude.numerator
        denominator = magnitude.denominator
        # Scale so the quotient carries more bits than binary128 needs,
        # then fold everything discarded into one sticky bit. That is what
        # makes the single rounding at the end the correct one.
        wanted = 130
        shift = wanted - (numerator.bit_length() - denominator.bit_length())
        if shift < 0:
            shift = 0
        quotient, remainder = divmod(numerator << shift, denominator)
        if remainder:
            quotient |= 1
        return cls._compose(negative, quotient, -shift)[0]

    @classmethod
    def _compose(
        cls, negative: bool, significand: int, exponent: int
    ) -> tuple["Float128", bool]:
        """Build a value from a sign, an integral significand and a binary
        exponent, rounding to nearest with ties to even. The second
        element says whether the rounding lost anything."""
        sign = _SIGN_MASK if negative else 0
        if significand == 0:
            return (cls(sign), False)
        # Clamped before anything is added: a decimal parser can hand this
        # an exponent of a hundred thousand.
        if exponent > _EXPONENT_MAX:
            return (cls.infinity(negative), True)
        if exponent < -(_SIGNIFICAND_BITS + _EXPONENT_BIAS + 200):
            return (cls(sign), True)

        lost = False
        leading = significand.bit_length() - 1
        if leading > _FRACTION_BITS:
            drop = leading - _FRACTION_BITS
            significand, lost = _shift_round(significand, drop)
            exponent += drop
            if significand.bit_length() - 1 > _FRACTION_BITS:
                significand >>= 1
                exponent += 1
        else:
            shift = _FRACTION_BITS - leading
            significand <<= shift
            exponent -= shift

        biased = exponent + _EXPONENT_BIAS + _FRACTION_BITS
        if biased >= _EXPONENT_MAX:
            return (cls.infinity(negative), True)
        if biased <= 0:
            # Subnormal. A value that rounds up to the smallest normal
            # carries out of the fraction into the exponent field on its
            # own, so the fraction is deliberately not masked here.
            rounded, dropped = _shift_round(significand, 1 - biased)
            return (cls(sign | rounded), lost or dropped)
        return (
            cls(sign | (biased << _FRACTION_BITS) | (significand & _FRACTION_MASK)),
            lost,
        )

    # -- conversion out ---------------------------------------------------

    def to_float(self) -> float:
        """To a Python `float` when the value fits one exactly.

        Raises:
            Inexact: carrying the rounded result. This is the refusal the
                `long double` column needed — the old writer took a
                `float` and wrote sixteen bytes, so a value with 113
                significand bits was narrowed on the way in and nobody
                was told.
        """
        rounded = self.to_float_rounded()
        if Float128.from_float(rounded).bits != self.bits:
            raise Inexact(rounded, self)
        return rounded

    def to_float_rounded(self) -> float:
        """To a Python `float`, rounding to nearest with ties to even.

        Overflow gives an infinity and underflow a signed zero, which is
        what IEEE 754-2008 clause 7.4 requires of the default rounding.
        """
        if self.is_nan():
            return float("nan")
        if self.is_infinite():
            return float("-inf") if self.negative else float("inf")
        parts = self.decompose()
        assert parts is not None
        negative, significand, exponent = parts
        if significand == 0:
            return -0.0 if negative else 0.0
        # `Fraction` -> `float` is correctly rounded in CPython, and it is
        # the one narrowing here that does not need to be written out.
        value = float(Fraction(significand) * Fraction(2) ** exponent)
        return -value if negative else value

    def to_decimal(self, context: Context | None = None) -> Decimal:
        """The exact value as a `Decimal`.

        Exact and not rounded: every finite binary value has finitely many
        decimal digits, and `Decimal` holds all of them.

        Raises:
            ValueError: for an infinity or a NaN, which `Decimal` can
                hold but which have no digits and would compare oddly.
        """
        parts = self.decompose()
        if parts is None:
            raise ValueError("an infinity or a NaN has no exact decimal")
        negative, significand, exponent = parts
        chosen = context or Context(prec=_exact_precision(significand, exponent))
        with localcontext(chosen):
            value = Decimal(significand) * (Decimal(2) ** exponent)
        return -value if negative else value

    def to_exact_decimal(self) -> str:
        """Every digit the value has, in the corpus's canonical form.

        The *exact* expansion and not the shortest one, and in exactly the
        spelling `crates/float128` and `runtime/c/src/float128_decimal.c`
        produce — the three columns are compared as text, so the three
        renderers have to agree on the form as well as on the value.
        """
        if self.is_nan():
            return "nan"
        if self.is_infinite():
            return "-inf" if self.negative else "inf"
        parts = self.decompose()
        assert parts is not None
        negative, significand, exponent = parts
        if significand == 0:
            return "-0.0" if negative else "0.0"
        if exponent >= 0:
            digits = _digits(significand << exponent)
            point = len(digits)
        else:
            # m * 2^-k is exactly m * 5^k * 10^-k.
            scale = -exponent
            digits = _digits(significand * 5**scale)
            point = len(digits) - scale
        digits = digits.rstrip("0") or "0"
        return _render(negative, digits, point)

    def to_shortest_decimal(self) -> str:
        """The fewest digits that read back as this pattern.

        Not the canonical form — `to_exact_decimal` is — but what a human
        writes into a value file.
        """
        parts = self.decompose()
        if parts is None or self.is_zero():
            return self.to_exact_decimal()
        negative, significand, exponent = parts
        if exponent >= 0:
            digits = _digits(significand << exponent)
            point = len(digits)
        else:
            digits = _digits(significand * 5 ** -exponent)
            point = len(digits) + exponent
        digits = digits.rstrip("0")
        # Round the exact expansion to one digit, then two, and ask the
        # parser whether it comes back. Rounding a `Decimal` instead would
        # put the answer at the mercy of a context precision, which is the
        # kind of setting that is right until someone changes it.
        for length in range(1, min(self.DIGITS_ROUND_TRIP, len(digits))):
            shortened, moved = _round_significant(digits, point, length)
            candidate = _render(negative, shortened, moved)
            if Float128.from_decimal(candidate).bits == self.bits:
                return candidate
        return _render(negative, digits, point)

    # -- comparison -------------------------------------------------------

    def ieee_eq(self, other: "Float128") -> bool:
        """IEEE 754-2008 `compareQuietEqual`: `-0.0 == 0.0`, and a NaN
        equals nothing including itself.

        Spelled out rather than put on `__eq__`, because `__eq__` is what
        a dict and a corpus expectation use and those want the bits.
        """
        if self.is_nan() or other.is_nan():
            return False
        if self.is_zero() and other.is_zero():
            return True
        return self.bits == other.bits

    def total_order_key(self) -> int:
        """A sort key giving IEEE 754-2008 `totalOrder`.

        `-NaN < -inf < ... < -0.0 < +0.0 < ... < +inf < +NaN`, NaNs
        ordered by payload. What a `@key` member needs, where "unordered"
        is not an answer.
        """
        if self.bits & _SIGN_MASK:
            return ~self.bits & ((1 << 128) - 1)
        return self.bits | _SIGN_MASK

    def __eq__(self, other: object) -> bool:
        return isinstance(other, Float128) and self.bits == other.bits

    def __hash__(self) -> int:
        return hash(self.bits)

    def __repr__(self) -> str:
        return f"Float128(0x{self.bits:032x})  # {self.to_exact_decimal()}"

    def __str__(self) -> str:
        return self.to_exact_decimal()


def _parse_decimal(text: str) -> tuple[bool, int, int]:
    """`[sign] digits [. digits] [e [sign] digits]` as
    `(negative, significand, exponent)` with the value
    `(-1)^negative * significand * 10^exponent`.

    Hand-written for the same reason `_digits` is: `int(str)` and
    `Fraction(str)` both raise past 4300 digits on a default CPython, and
    the exact expansion this module *emits* is longer than that, so a
    parser that used them could not read back what the printer wrote.

    An underscore is refused rather than ignored. Python accepts `1_5` as
    fifteen in a literal; in a value file it is a typo, and reading it as
    fifteen is the kind of quiet reinterpretation this project treats as
    a defect.
    """
    negative = False
    body = text
    if body[:1] in ("+", "-"):
        negative = body[0] == "-"
        body = body[1:]
    mantissa, _, exponent_text = body.partition("e") if "e" in body else body.partition("E")
    exponent = int(exponent_text) if exponent_text else 0
    if exponent_text and not (
        exponent_text.lstrip("+-").isdigit() and exponent_text.lstrip("+-")
    ):
        raise ValueError(f"`{text}` has no usable exponent")

    whole, point, fraction = mantissa.partition(".")
    if point and not fraction and not whole:
        raise ValueError(f"`{text}` has no digits")
    digits = whole + fraction
    if not digits or not digits.isdigit():
        raise ValueError(f"`{text}` is not a decimal")
    # Nine digits at a time keeps every step a machine-word multiply.
    significand = 0
    for start in range(0, len(digits), 9):
        chunk = digits[start : start + 9]
        significand = significand * (10 ** len(chunk)) + int(chunk)
    return (negative, significand, exponent - len(fraction))


def _digits(value: int) -> str:
    """The decimal digits of a non-negative `int`, without `str()`.

    CPython 3.11 caps `str(int)` at 4300 digits by default — an
    `int`/`str` conversion beyond that raises rather than returning. The
    exact expansion of a binary128 runs to 11 563 digits, so every value
    below about `1e-4300` would raise here on a default interpreter and
    the column would fail on precisely the values it exists to carry.
    `sys.set_int_max_str_digits` would fix it by changing a
    process-global setting a library has no business changing, so the
    digits are extracted directly instead: nine at a time, which is the
    largest power of ten that keeps each step a machine-word division.
    """
    if value == 0:
        return "0"
    groups = []
    while value:
        value, group = divmod(value, 1_000_000_000)
        groups.append(group)
    out = [str(groups[-1])]
    out.extend(f"{group:09d}" for group in reversed(groups[:-1]))
    return "".join(out)



def _round_significant(digits: str, point: int, length: int) -> tuple[str, int]:
    """Round `digits` to `length` significant digits, half to even.

    Returns the shortened digits and the adjusted point, which moves when
    the rounding carries `999` up to `1000`.
    """
    kept = list(digits[:length])
    following = digits[length]
    rest_nonzero = any(digit != "0" for digit in digits[length + 1 :])
    if following > "5":
        round_up = True
    elif following < "5":
        round_up = False
    else:
        round_up = rest_nonzero or (int(kept[-1]) % 2 == 1)
    if round_up:
        index = length
        while True:
            if index == 0:
                kept.insert(0, "1")
                del kept[length:]
                point += 1
                break
            index -= 1
            if kept[index] == "9":
                kept[index] = "0"
            else:
                kept[index] = str(int(kept[index]) + 1)
                break
    return ("".join(kept).rstrip("0") or "0", point)


def _shift_round(value: int, shift: int) -> tuple[int, bool]:
    """Shift right, rounding to nearest with ties to even. The second
    element says whether anything was discarded."""
    if shift <= 0:
        return (value, False)
    kept = value >> shift
    dropped = value & ((1 << shift) - 1)
    if dropped == 0:
        return (kept, False)
    half = 1 << (shift - 1)
    if dropped > half or (dropped == half and kept & 1):
        kept += 1
    return (kept, True)


def _exact_precision(significand: int, exponent: int) -> int:
    """Digits enough for `significand * 2^exponent` to be exact.

    `Decimal` truncates to the context precision, so a context sized by
    guesswork would silently shorten the very values this class exists to
    carry.
    """
    digits = len(str(significand))
    if exponent < 0:
        # Multiplying by 5^-exponent adds at most that many digits.
        digits += -exponent
    else:
        digits += (exponent * 30103) // 100000 + 2
    return max(digits + 2, 28)


def _render(negative: bool, digits: str, point: int) -> str:
    """`0.<digits> * 10^point`, in the three forms all three columns use."""
    sign = "-" if negative else ""
    if 0 < point <= 34:
        if len(digits) <= point:
            return f"{sign}{digits}{'0' * (point - len(digits))}.0"
        return f"{sign}{digits[:point]}.{digits[point:]}"
    if -6 < point <= 0:
        return f"{sign}0.{'0' * -point}{digits}"
    mantissa = digits[0] + "." + (digits[1:] or "0")
    exponent = point - 1
    return f"{sign}{mantissa}e{'+' if exponent >= 0 else '-'}{abs(exponent)}"
