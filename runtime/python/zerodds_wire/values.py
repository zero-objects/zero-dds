# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
"""One sample and JSON, in both directions, in the dialect the probes share.

Generated `from_json` and `to_json` functions call these. The dialect is
the one `crates/wire-probe/src/lib.rs` documents, and two of its rules
are the ones a reader gets wrong:

- a 64-bit integer is a **string**, because a JSON number is a double and
  loses the low bits past 2^53;
- an absent `@optional` member is a key that is **left out**, never
  `null`, which is refused.

Every failure names the path in the sample. A generator that reported
only "type mismatch" over a fifty-member value would be one nobody uses
twice.
"""

from __future__ import annotations

from typing import Any
from .float128 import Float128

__all__ = [
    "SampleError",
    "Mismatch",
    "Missing",
    "Unknown",
    "kind_of",
    "as_int",
    "as_float",
    "as_bool",
    "as_char",
    "as_str",
    "as_array",
    "as_object",
    "as_pair",
    "finite",
]


class SampleError(Exception):
    """Why a sample could not be read."""


class Mismatch(SampleError):
    """The value at `path` is not what the type calls for."""

    def __init__(self, path: str, expected: str, found: str) -> None:
        super().__init__(f"{path}: expected {expected}, found {found}")
        self.path = path


class Missing(SampleError):
    """A member the type declares has no value and is not optional."""

    def __init__(self, path: str) -> None:
        super().__init__(f"{path}: no value, and the member is not optional")
        self.path = path


class Unknown(SampleError):
    """The value holds a key the type does not declare.

    An error and not a warning: a misspelled member name would otherwise
    silently test the default value.
    """

    def __init__(self, path: str, declared: list[str]) -> None:
        super().__init__(f"{path}: not a member; the type declares {declared}")
        self.path = path
        self.declared = declared


def kind_of(value: object) -> str:
    """What a JSON value is, in the words the errors use."""
    if value is None:
        return "null"
    if isinstance(value, bool):
        return "a boolean"
    if isinstance(value, (int, float)):
        return "a number"
    if isinstance(value, str):
        return "a string"
    if isinstance(value, list):
        return "an array"
    if isinstance(value, dict):
        return "an object"
    return "something else"


def as_int(value: object, path: str) -> int:
    """An integer, from a JSON number or from a string.

    A string is accepted for every width and required past 2^53: a JSON
    number is a double, and `9007199254740993` does not survive the round
    trip through one. A `bool` is not an integer here even though Python
    says it is a subclass of one — `true` in an integer field is a sample
    that does not mean what it says.
    """
    if isinstance(value, str):
        try:
            return int(value, 10)
        except ValueError as error:
            raise Mismatch(path, "an integer in a string", f"`{value}`") from error
    if isinstance(value, bool) or not isinstance(value, int):
        raise Mismatch(path, "an integer", kind_of(value))
    return value


def as_float(value: object, path: str) -> float:
    """A number, as a double."""
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise Mismatch(path, "a number", kind_of(value))
    return float(value)


def as_float128(value: object, path: str) -> Float128:
    """A `long double`, from a decimal string or from a number.

    A string for the reason a 64-bit integer is one: a JSON number is a
    double and 60 of a long double's 113 significand bits do not fit one.
    A number is still accepted, because `1.5` in a value file means 1.5
    and every double widens into binary128 exactly — but a value that
    needs the extra bits has to be written as text.
    """
    if isinstance(value, Float128):
        return value
    if isinstance(value, str):
        try:
            return Float128.from_decimal(value)
        except (ValueError, ArithmeticError) as error:
            raise Mismatch(path, "a decimal a long double can hold", f"`{value}`") from error
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise Mismatch(path, "a long double, as a decimal string or a number", kind_of(value))
    if isinstance(value, int):
        return Float128.from_int(value)
    return Float128.from_float(value)


def as_bool(value: object, path: str) -> bool:
    """`true` or `false`."""
    if not isinstance(value, bool):
        raise Mismatch(path, "true or false", kind_of(value))
    return value


def as_char(value: object, path: str) -> str:
    """A one-character string."""
    if not isinstance(value, str):
        raise Mismatch(path, "a one-character string", kind_of(value))
    if len(value) != 1:
        raise Mismatch(path, "exactly one character", f"`{value}`")
    return value


def as_str(value: object, path: str) -> str:
    """A string."""
    if not isinstance(value, str):
        raise Mismatch(path, "a string", kind_of(value))
    return value


def as_fixed(value: object, path: str, digits: int, scale: int) -> str:
    """A `fixed<digits, scale>`, in the one decimal form it has.

    A JSON string, because a JSON number is a double and `fixed<31, 2>`
    is exactly what a double cannot hold. The text is then put into the
    form the read direction returns: leading zeros in the whole part
    gone, the fraction padded out to `scale`, and no sign on a zero.

    ## Why it is normalised here and not only checked

    The value has to survive a round trip. `Reader.read_fixed` returns
    `"-0.001"` because the BCD field carries no leading zeros of its
    own; a `from_json` that kept the sample's `"-000000.001"` verbatim
    made every `fixed` member compare unequal to itself across a write
    and a read, in a binding whose values are `dataclasses` and whose
    users compare them with `==`. Both other columns already answered
    `"-0.001"` for both directions — `crates/wire-probe/src/canonical.rs`
    is where that form is defined — so this was the Python column alone.

    Anything that is not a decimal is handed back untouched, so the
    refusal still comes from `Writer.write_fixed` with the message it
    always had. A second validator here would be a second answer to what
    fits a `fixed`.
    """
    text = as_str(value, path)
    body = text[1:] if text[:1] in ("+", "-") else text
    whole, _, fraction = body.partition(".")
    if not (whole + fraction).isdigit() or not whole + fraction:
        return text
    if len(whole.lstrip("0")) > digits - scale or len(fraction) > scale:
        return text
    out = whole.lstrip("0") or "0"
    if scale:
        out = f"{out}.{fraction.ljust(scale, '0')}"
    if text.startswith("-") and (out.lstrip("0.") != ""):
        out = f"-{out}"
    return out


def as_array(value: object, path: str) -> list[Any]:
    """An array."""
    if not isinstance(value, list):
        raise Mismatch(path, "an array", kind_of(value))
    return value


def as_object(value: object, path: str) -> dict[str, Any]:
    """An object."""
    if not isinstance(value, dict):
        raise Mismatch(path, "an object", kind_of(value))
    return value


def as_pair(value: object, path: str) -> tuple[Any, Any]:
    """A two-element array, `[key, value]`.

    A map is an array of pairs and not a JSON object, because a JSON
    object keys on a string and `map<long, float>` does not.
    """
    items = as_array(value, path)
    if len(items) != 2:
        raise Mismatch(path, "a two-element array, `[key, value]`", f"{len(items)} elements")
    return items[0], items[1]


def finite(value: float, path: str) -> float:
    """A float on its way out to JSON, refused if JSON cannot write it.

    JSON has no NaN and no infinity — RFC 8259 §6 — so a `float` or
    `double` member holding one has no JSON form at all. Refused with
    the path rather than written as `NaN`, which `json.dumps` will
    happily emit and no conforming parser will read back; the Rust
    column refuses at the same point (`canonical::number_f64`) with the
    same reasoning, so the two columns part over the value and never
    over one of them having invented a spelling for it.
    """
    if value != value or value in (float("inf"), float("-inf")):
        raise Mismatch(path, "a finite number, which is all JSON can write", repr(value))
    return value
