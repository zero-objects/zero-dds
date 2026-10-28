# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
defmodule ZeroddsWire.Float128 do
  @moduledoc """
  IEEE 754 binary128, in plain Elixir integers.

  The counterpart of `crates/float128`, `runtime/c/src/float128.c` and
  `runtime/python/zerodds_wire/float128.py` — a further implementation
  of the same clauses, not a binding to any of them. CORBA 3.3 part 2
  §9.3.1.3 with Figure 9.2 gives CDR `long double` one sign bit, a
  fifteen-bit exponent biased by 16383 and a 112-bit stored fraction;
  DDS-XTypes 1.3 Table 31 names the result. An Erlang float is a
  binary64 with 53 of the 113 significand bits, so the value is held as
  the 128-bit pattern and every conversion is exact integer arithmetic
  — which Elixir's unbounded integers give for free.

  Equality is by bit pattern: `-0.0` and `+0.0` differ and a NaN equals
  itself, which is what a corpus expectation wants.

  No arithmetic. Nothing in the wire path needs it.
  """

  import Bitwise

  alias ZeroddsWire.WireError

  defstruct bits: 0

  @fraction_bits 112
  @exponent_bias 16_383
  @exponent_max 0x7FFF
  @fraction_mask (1 <<< @fraction_bits) - 1
  @sign_mask 1 <<< 127
  @quiet_bit 1 <<< (@fraction_bits - 1)

  @doc "From the 128-bit pattern."
  def from_bits(bits) when bits >= 0 and bits < 1 <<< 128, do: %__MODULE__{bits: bits}

  @doc "The zero bit pattern, negatively signed on request."
  def zero(negative \\ false), do: %__MODULE__{bits: if(negative, do: @sign_mask, else: 0)}

  @doc "An infinity."
  def infinity(negative \\ false) do
    bits = @exponent_max <<< @fraction_bits
    %__MODULE__{bits: if(negative, do: bits ||| @sign_mask, else: bits)}
  end

  @doc "The quiet NaN."
  def quiet_nan, do: %__MODULE__{bits: @exponent_max <<< @fraction_bits ||| @quiet_bit}

  # -- the sixteen bytes ---------------------------------------------------

  @doc """
  From the sixteen bytes in the order named. The two orders are the
  whole sixteen bytes reversed — CORBA Figure 9.2 — and not two
  eight-byte halves swapped in place.
  """
  def from_bytes(<<data::binary-size(16)>>, :le),
    do: %__MODULE__{bits: :binary.decode_unsigned(data, :little)}

  def from_bytes(<<data::binary-size(16)>>, :be),
    do: %__MODULE__{bits: :binary.decode_unsigned(data, :big)}

  @doc "The sixteen bytes in the order named."
  def to_bytes(%__MODULE__{bits: bits}, :le), do: pad16(:binary.encode_unsigned(bits, :little), :le)
  def to_bytes(%__MODULE__{bits: bits}, :be), do: pad16(:binary.encode_unsigned(bits, :big), :be)

  defp pad16(data, order) do
    padding = :binary.copy(<<0>>, 16 - byte_size(data))

    case order do
      :le -> data <> padding
      :be -> padding <> data
    end
  end

  # -- fields --------------------------------------------------------------

  @doc "The sign bit, as stored."
  def negative?(%__MODULE__{bits: bits}), do: (bits &&& @sign_mask) != 0

  defp biased_exponent(bits), do: bits >>> @fraction_bits &&& @exponent_max
  defp fraction(bits), do: bits &&& @fraction_mask

  @doc "Whether the value is a NaN."
  def nan?(%__MODULE__{bits: bits}),
    do: biased_exponent(bits) == @exponent_max and fraction(bits) != 0

  @doc "Whether the value is an infinity."
  def infinite?(%__MODULE__{bits: bits}),
    do: biased_exponent(bits) == @exponent_max and fraction(bits) == 0

  @doc "Whether the value is finite."
  def finite?(%__MODULE__{bits: bits}), do: biased_exponent(bits) != @exponent_max

  @doc """
  `{negative, significand, exponent}` with
  `value = (-1)^negative * significand * 2^exponent`, or `nil` for an
  infinity or a NaN. The significand is integral — the implied leading
  one is materialised for a normal value.
  """
  def decompose(%__MODULE__{bits: bits} = value) do
    if not finite?(value) do
      nil
    else
      negative = (bits &&& @sign_mask) != 0
      biased = biased_exponent(bits)
      stored = fraction(bits)

      if biased == 0 do
        {negative, stored, 1 - @exponent_bias - @fraction_bits}
      else
        {negative, stored ||| 1 <<< @fraction_bits, biased - @exponent_bias - @fraction_bits}
      end
    end
  end

  # -- conversion in -------------------------------------------------------

  @doc """
  From an Erlang number or one of the non-finite atoms. Always exact:
  binary128 covers every binary64 value, subnormals included.
  """
  def from_number(:nan), do: quiet_nan()
  def from_number(:infinity), do: infinity()
  def from_number(:neg_infinity), do: infinity(true)

  def from_number(value) when is_number(value) do
    <<raw::unsigned-64>> = <<value * 1.0::float-64>>
    negative = raw >>> 63 != 0
    biased = raw >>> 52 &&& 0x7FF
    stored = raw &&& 0x000F_FFFF_FFFF_FFFF
    sign = if negative, do: @sign_mask, else: 0

    cond do
      biased == 0x7FF ->
        # Unreachable over an Erlang float, which has no such value; a
        # NaN would keep its payload if one ever arrived through bits.
        %__MODULE__{bits: sign ||| @exponent_max <<< @fraction_bits ||| stored <<< 60}

      biased == 0 and stored == 0 ->
        %__MODULE__{bits: sign}

      biased == 0 ->
        {value, _lost} = compose(negative, stored, 1 - 1023 - 52)
        value

      true ->
        {value, _lost} = compose(negative, stored ||| 1 <<< 52, biased - 1023 - 52)
        value
    end
  end

  @doc """
  From an integer, refusing one that needs more than 113 significand
  bits — the refusal is the point: a rounded integer is a different
  value reported as success.
  """
  def from_int(value) when is_integer(value) do
    {rounded, lost} = compose(value < 0, abs(value), 0)

    if lost do
      raise WireError,
        message:
          "the integer #{value} does not fit a binary128 exactly; " <>
            "it rounds to 0x#{Integer.to_string(rounded.bits, 16)}",
        at: 0,
        kind: :out_of_range
    end

    rounded
  end

  @doc """
  From an exact decimal string, correctly rounded to nearest, ties to
  even — never through a `float`, whose second rounding gets the last
  bit wrong for exactly the values this type exists to carry.
  """
  def from_decimal(text) when is_binary(text) do
    case String.downcase(String.trim(text)) do
      inf when inf in ["inf", "+inf", "infinity", "+infinity"] ->
        infinity()

      neg when neg in ["-inf", "-infinity"] ->
        infinity(true)

      nan when nan in ["nan", "+nan"] ->
        quiet_nan()

      "-nan" ->
        %__MODULE__{bits: quiet_nan().bits ||| @sign_mask}

      _ ->
        {negative, significand, exponent} = parse_decimal(String.trim(text))

        cond do
          significand == 0 ->
            zero(negative)

          exponent >= 0 ->
            from_fraction(significand * pow10(exponent), 1, negative)

          true ->
            from_fraction(significand, pow10(-exponent), negative)
        end
    end
  end

  # `numerator / denominator`, scaled so the quotient carries more bits
  # than binary128 needs, everything discarded folded into one sticky
  # bit — what makes the single rounding at the end the correct one.
  defp from_fraction(numerator, denominator, negative) do
    wanted = 130
    shift = max(wanted - (bit_length(numerator) - bit_length(denominator)), 0)
    quotient = div(numerator <<< shift, denominator)
    remainder = rem(numerator <<< shift, denominator)
    quotient = if remainder != 0, do: quotient ||| 1, else: quotient
    {value, _lost} = compose(negative, quotient, -shift)
    value
  end

  # Build a value from a sign, an integral significand and a binary
  # exponent, rounding to nearest with ties to even; the second element
  # says whether the rounding lost anything.
  defp compose(negative, significand, exponent) do
    sign = if negative, do: @sign_mask, else: 0

    cond do
      significand == 0 ->
        {%__MODULE__{bits: sign}, false}

      exponent > @exponent_max ->
        {infinity(negative), true}

      exponent < -(@fraction_bits + 1 + @exponent_bias + 200) ->
        {%__MODULE__{bits: sign}, true}

      true ->
        leading = bit_length(significand) - 1

        {significand, exponent, lost} =
          if leading > @fraction_bits do
            drop = leading - @fraction_bits
            {rounded, lost} = shift_round(significand, drop)

            if bit_length(rounded) - 1 > @fraction_bits do
              {rounded >>> 1, exponent + drop + 1, lost}
            else
              {rounded, exponent + drop, lost}
            end
          else
            shift = @fraction_bits - leading
            {significand <<< shift, exponent - shift, false}
          end

        biased = exponent + @exponent_bias + @fraction_bits

        cond do
          biased >= @exponent_max ->
            {infinity(negative), true}

          biased <= 0 ->
            # Subnormal; a value that rounds up to the smallest normal
            # carries out of the fraction on its own, so the fraction is
            # deliberately not masked.
            {rounded, dropped} = shift_round(significand, 1 - biased)
            {%__MODULE__{bits: sign ||| rounded}, lost or dropped}

          true ->
            {%__MODULE__{
               bits: sign ||| biased <<< @fraction_bits ||| (significand &&& @fraction_mask)
             }, lost}
        end
    end
  end

  # -- conversion out ------------------------------------------------------

  @doc """
  Every digit the value has, in the corpus's canonical form — the exact
  expansion, in exactly the spelling `crates/float128` and its siblings
  produce, because the columns are compared as text.
  """
  def to_exact_decimal(%__MODULE__{} = value) do
    cond do
      nan?(value) ->
        "nan"

      infinite?(value) ->
        if negative?(value), do: "-inf", else: "inf"

      true ->
        {negative, significand, exponent} = decompose(value)

        if significand == 0 do
          if negative, do: "-0.0", else: "0.0"
        else
          {digits, point} =
            if exponent >= 0 do
              digits = Integer.to_string(significand <<< exponent)
              {digits, String.length(digits)}
            else
              scale = -exponent
              digits = Integer.to_string(significand * pow5(scale))
              {digits, String.length(digits) - scale}
            end

          digits = String.trim_trailing(digits, "0")
          digits = if digits == "", do: "0", else: digits
          render(negative, digits, point)
        end
    end
  end

  # `0.<digits> * 10^point`, in the three forms all the columns use.
  defp render(negative, digits, point) do
    sign = if negative, do: "-", else: ""
    width = String.length(digits)

    cond do
      point > 0 and point <= 34 ->
        if width <= point do
          "#{sign}#{digits}#{String.duplicate("0", point - width)}.0"
        else
          "#{sign}#{String.slice(digits, 0, point)}.#{String.slice(digits, point, width - point)}"
        end

      point > -6 and point <= 0 ->
        "#{sign}0.#{String.duplicate("0", -point)}#{digits}"

      true ->
        mantissa =
          String.slice(digits, 0, 1) <>
            "." <> if(width > 1, do: String.slice(digits, 1, width - 1), else: "0")

        exponent = point - 1
        "#{sign}#{mantissa}e#{if exponent >= 0, do: "+", else: "-"}#{abs(exponent)}"
    end
  end

  # -- the helpers ---------------------------------------------------------

  # `[sign] digits [. digits] [e [sign] digits]` as
  # `{negative, significand, exponent}`. An underscore is refused
  # rather than read as a digit separator: in a value file it is a typo.
  defp parse_decimal(text) do
    {negative, body} =
      case text do
        "-" <> rest -> {true, rest}
        "+" <> rest -> {false, rest}
        _ -> {false, text}
      end

    {mantissa, exponent} =
      case String.split(body, ~r/[eE]/, parts: 2) do
        [mantissa] -> {mantissa, 0}
        [mantissa, exp] -> {mantissa, parse_exponent(text, exp)}
      end

    {whole, fraction} =
      case String.split(mantissa, ".", parts: 2) do
        [whole] -> {whole, ""}
        [whole, fraction] -> {whole, fraction}
      end

    digits = whole <> fraction

    if digits == "" or not String.match?(digits, ~r/^[0-9]+$/) do
      raise WireError, message: "`#{text}` is not a decimal", at: 0, kind: :out_of_range
    end

    {negative, String.to_integer(digits), exponent - String.length(fraction)}
  end

  defp parse_exponent(text, exp) do
    trimmed = String.trim_leading(exp, "+")

    case Integer.parse(trimmed) do
      {value, ""} ->
        value

      _ ->
        raise WireError, message: "`#{text}` has no usable exponent", at: 0, kind: :out_of_range
    end
  end

  # Shift right, rounding to nearest with ties to even; the second
  # element says whether anything was discarded.
  defp shift_round(value, shift) when shift <= 0, do: {value, false}

  defp shift_round(value, shift) do
    kept = value >>> shift
    dropped = value &&& (1 <<< shift) - 1

    if dropped == 0 do
      {kept, false}
    else
      half = 1 <<< (shift - 1)

      kept =
        if dropped > half or (dropped == half and (kept &&& 1) == 1) do
          kept + 1
        else
          kept
        end

      {kept, true}
    end
  end

  defp bit_length(0), do: 0

  defp bit_length(value) when value > 0 do
    # The bit length of a positive integer, via the byte length and the
    # top byte — `Integer.digits(value, 2)` builds a list the size of
    # the number, and the exact decimal path handles four-digit
    # exponents where that list would be tens of thousands long.
    bytes = byte_size(:binary.encode_unsigned(value))
    <<top, _::binary>> = :binary.encode_unsigned(value)
    (bytes - 1) * 8 + (8 - leading_zeros(top))
  end

  defp leading_zeros(byte) do
    cond do
      byte >= 0x80 -> 0
      byte >= 0x40 -> 1
      byte >= 0x20 -> 2
      byte >= 0x10 -> 3
      byte >= 0x08 -> 4
      byte >= 0x04 -> 5
      byte >= 0x02 -> 6
      byte >= 0x01 -> 7
      true -> 8
    end
  end

  defp pow10(0), do: 1
  defp pow10(n), do: 10 |> Integer.pow(n)

  defp pow5(0), do: 1
  defp pow5(n), do: 5 |> Integer.pow(n)
end
