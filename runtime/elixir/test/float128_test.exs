# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
defmodule ZeroddsWire.Float128Test do
  @moduledoc """
  The binary128 against known bit patterns — the same anchors
  `runtime/python/zerodds_wire/float128.py` and `crates/float128` are
  held to, because the columns are compared as text and octets.
  """
  use ExUnit.Case, async: true

  import Bitwise

  alias ZeroddsWire.Float128
  alias ZeroddsWire.Reader
  alias ZeroddsWire.WireError
  alias ZeroddsWire.Writer

  test "one point five is the textbook pattern, both byte orders" do
    value = Float128.from_number(1.5)
    assert value.bits == 0x3FFF_8000_0000_0000_0000_0000_0000_0000

    le = Float128.to_bytes(value, :le)
    be = Float128.to_bytes(value, :be)
    # The two orders are the whole sixteen bytes reversed.
    assert le == be |> :binary.bin_to_list() |> Enum.reverse() |> :binary.list_to_bin()
    assert Float128.from_bytes(le, :le) == value
    assert Float128.from_bytes(be, :be) == value
  end

  test "a double is widened exactly, subnormals included" do
    for sample <- [0.0, -0.0, 1.0, -2.5, 3.141592653589793, 5.0e-324, 1.7976931348623157e308] do
      widened = Float128.from_number(sample)
      decimal = Float128.to_exact_decimal(widened)
      # `!= nil` said nothing here: the function's return is a binary and
      # Elixir 1.20's type checker reports the comparison as one that can
      # only ever be true. What the line meant is that there *is* an
      # expansion.
      assert decimal != ""

      # Round-trip through the exact decimal parses back to the same
      # bits: from_decimal is correctly rounded and the expansion is
      # exact. Bit equality, so -0.0 has to keep its sign through the
      # decimal as well.
      assert Float128.from_decimal(decimal) == widened
    end
  end

  test "a decimal that needs more than 53 bits is parsed exactly, not through a double" do
    # 2^-113 has an exact decimal expansion; a parse through a binary64
    # would flush the low bits.
    value = Float128.from_decimal("1.0000000000000000000000000000000001")
    double_rounded = Float128.from_number(1.0)
    assert value != double_rounded
  end

  test "the non-finite spellings parse and print" do
    assert Float128.from_decimal("inf") == Float128.infinity()
    assert Float128.from_decimal("-inf") == Float128.infinity(true)
    assert Float128.to_exact_decimal(Float128.infinity(true)) == "-inf"
    assert Float128.to_exact_decimal(Float128.quiet_nan()) == "nan"
    assert Float128.to_exact_decimal(Float128.zero(true)) == "-0.0"
  end

  test "an integer that does not fit 113 bits is refused, not rounded" do
    exact = Float128.from_int(1 <<< 113)
    assert Float128.to_exact_decimal(exact) != "0.0"

    assert_raise WireError, fn -> Float128.from_int((1 <<< 113) + 1) end
  end

  test "the exact decimal renders the three forms the columns share" do
    assert Float128.to_exact_decimal(Float128.from_number(1.5)) == "1.5"
    assert Float128.to_exact_decimal(Float128.from_number(-0.125)) == "-0.125"
    assert Float128.to_exact_decimal(Float128.from_decimal("1e40")) == "1.0e+40"
  end

  test "the writer aligns a long double to eight and the reader takes all sixteen bytes" do
    w = Writer.new(:le)
    w = Writer.write_u8(w, 1)
    w = Writer.write_f128(w, Float128.from_number(1.5))
    bytes = Writer.finish(w)
    assert byte_size(bytes) == 24

    r = Reader.new(bytes, :le)
    {_, r} = Reader.read_u8(r)
    {value, _} = Reader.read_f128(r)
    assert value == Float128.from_number(1.5)
  end
end
