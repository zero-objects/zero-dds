# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
defmodule ZeroddsWire.Writer do
  @moduledoc """
  Putting bytes on the wire.

  The Elixir counterpart of `crates/wire/src/writer.rs` and of
  `runtime/python/zerodds_wire/writer.py` — a further implementation of
  the same clauses, not a binding to either. Where two disagree, one of
  them has misread DDS-XTypes 1.3, and that is the point of having
  both. The Python module carries the clause quotations and the
  measured vendor bytes at length; this one repeats the decisions and
  points there for the arguments.

  Alignment is relative to the start of the stream, not to the start of
  the value — XTypes 1.3 §7.4.1.1 — and a PL_CDR1 parameter's value is
  its own block, so the origin moves while one is open (§7.4.1.2).

  Elixir is immutable, so every operation answers the writer it
  advanced: `w = Writer.write_u32(w, x)`. An operation that opens a
  header answers `{handle, w}` and the matching finish takes the handle
  back. Three things Elixir makes easy to get wrong are checked here
  rather than assumed:

  - an integer is unbounded and the wire is not, so every integer write
    refuses a value its field cannot hold instead of wrapping it;
  - `String.length/1` counts graphemes and the wire counts bytes, so
    every length is taken from the encoded form;
  - an Erlang float cannot hold a NaN or an infinity, so those arrive
    as the atoms `ZeroddsWire.Float` names and are converted at the
    octet.
  """

  import Bitwise

  alias ZeroddsWire.Float, as: WireFloat
  alias ZeroddsWire.Float128
  alias ZeroddsWire.PlCdr1Framing
  alias ZeroddsWire.WireError

  defstruct out: <<>>,
            endianness: :le,
            origin: 0,
            max_align: 8,
            framing: %PlCdr1Framing{},
            short_length_codes: false

  @pid_short_max_len 0xFFFF

  @doc """
  A writer in `endianness` whose alignment is capped at `max_align` —
  four for XCDR2, eight for XCDR1 (§7.4.1: `ALIGN(N)` aligns to
  `MIN(N, XCDR.maxalign)`).
  """
  def new(endianness, max_align \\ 8, framing \\ %PlCdr1Framing{}, short_length_codes \\ false)
      when endianness in [:le, :be] do
    %__MODULE__{
      out: <<>>,
      endianness: endianness,
      origin: 0,
      max_align: max(1, max_align),
      framing: framing,
      short_length_codes: short_length_codes
    }
  end

  @doc "How many bytes have been written."
  def position(%__MODULE__{out: out}), do: byte_size(out)

  @doc "The bytes written."
  def finish(%__MODULE__{out: out}), do: out

  # -- alignment -----------------------------------------------------------

  @doc "Pad to `align`, counted from the stream's origin."
  def align_to(%__MODULE__{} = w, align) do
    align = min(align, w.max_align)

    if align <= 1 do
      w
    else
      offset = byte_size(w.out) - w.origin
      padding = rem(align - rem(offset, align), align)
      %{w | out: w.out <> :binary.copy(<<0>>, padding)}
    end
  end

  # -- packed collection elements -----------------------------------------

  @doc """
  `ElementPadding::Packed`: each collection element and each aggregate
  value counts its alignment from its own first octet instead of from
  the encapsulation. Returns the origin displaced, for
  `finish_packed_element/2`. `array-1d-struct-final` is
  `01 00 00 00 02 00 03 00 00 00 04 00 77`, the second Cell's `long` at
  offset six where the clause puts it at eight.
  """
  def begin_packed_element(%__MODULE__{} = w) do
    saved = w.origin
    {saved, %{w | origin: byte_size(w.out)}}
  end

  @doc "Restore the origin `begin_packed_element/1` displaced."
  def finish_packed_element(%__MODULE__{} = w, saved) do
    %{w | origin: saved}
  end

  # -- primitives ----------------------------------------------------------

  defp refuse(w, expected) do
    raise WireError,
      message: "at #{byte_size(w.out)}: expected #{expected}",
      at: byte_size(w.out),
      kind: :out_of_range
  end

  defp check(w, value, low, high, expected) do
    if not is_integer(value) or value < low or value > high do
      refuse(w, "#{expected} in [#{low}, #{high}], not #{inspect(value)}")
    end

    :ok
  end

  defp raw(%__MODULE__{endianness: :le} = w, unsigned, width),
    do: %{w | out: w.out <> <<unsigned::unsigned-little-size(width * 8)>>}

  defp raw(%__MODULE__{endianness: :be} = w, unsigned, width),
    do: %{w | out: w.out <> <<unsigned::unsigned-big-size(width * 8)>>}

  defp int(w, unsigned, width), do: w |> align_to(width) |> raw(unsigned, width)

  @doc "`boolean`, one byte, 0 or 1."
  def write_bool(%__MODULE__{} = w, true), do: %{w | out: w.out <> <<1>>}
  def write_bool(%__MODULE__{} = w, false), do: %{w | out: w.out <> <<0>>}
  def write_bool(%__MODULE__{} = w, other), do: refuse(w, "a boolean, not #{inspect(other)}")

  @doc "`octet` and `uint8`."
  def write_u8(%__MODULE__{} = w, value) do
    check(w, value, 0, 0xFF, "an octet")
    %{w | out: w.out <> <<value>>}
  end

  @doc "`int8`."
  def write_i8(%__MODULE__{} = w, value) do
    check(w, value, -0x80, 0x7F, "an int8")
    %{w | out: w.out <> <<value &&& 0xFF>>}
  end

  @doc """
  `char`, one byte. A code point above 255 is refused rather than
  truncated — `0xE4` is a different character from `'ä'`'s UTF-8.
  """
  def write_char(%__MODULE__{} = w, value) do
    point = one_code_point(w, value)

    if point > 0xFF do
      refuse(w, "a character below U+0100, not U+#{Integer.to_string(point, 16)}")
    end

    %{w | out: w.out <> <<point>>}
  end

  @doc """
  `wchar`, at `width` octets and in the shape `framing` does — two
  under XTypes 1.3 Table 31, four under RTI's XCDR1 and TAO's, and
  GIOP 1.2's length-prefixed form for the ORBs. The BMP restriction of
  §7.2.2.2.1.2.5 is a property of the type, not of the width.
  """
  def write_wchar(%__MODULE__{} = w, value, width, framing, order) do
    point = one_code_point(w, value)

    if point > 0xFFFF do
      refuse(w, "a BMP character, not U+#{Integer.to_string(point, 16)}")
    end

    if width not in [2, 4] do
      refuse(w, "a wchar width of 2 or 4, not #{inspect(width)}")
    end

    case framing do
      :"giop-length-prefixed" ->
        # CORBA 3.3 part 2 §9.3.1.6's GIOP 1.2 form: the octet count,
        # then the code unit's octets — big-endian under every order
        # but the stream's own, because a scalar cannot carry a mark.
        w = %{w | out: w.out <> <<width>>}

        big = order != :stream or w.endianness == :be

        if big do
          %{w | out: w.out <> <<point::unsigned-big-size(width * 8)>>}
        else
          %{w | out: w.out <> <<point::unsigned-little-size(width * 8)>>}
        end

      :fixed ->
        if width == 4, do: write_u32(w, point), else: write_u16(w, point)

      other ->
        refuse(w, "a wchar framing, not #{inspect(other)}")
    end
  end

  @doc "`short`."
  def write_i16(%__MODULE__{} = w, value) do
    check(w, value, -0x8000, 0x7FFF, "a short")
    int(w, value &&& 0xFFFF, 2)
  end

  @doc "`unsigned short`."
  def write_u16(%__MODULE__{} = w, value) do
    check(w, value, 0, 0xFFFF, "an unsigned short")
    int(w, value, 2)
  end

  @doc "`long`."
  def write_i32(%__MODULE__{} = w, value) do
    check(w, value, -0x8000_0000, 0x7FFF_FFFF, "a long")
    int(w, value &&& 0xFFFF_FFFF, 4)
  end

  @doc "`unsigned long`."
  def write_u32(%__MODULE__{} = w, value) do
    check(w, value, 0, 0xFFFF_FFFF, "an unsigned long")
    int(w, value, 4)
  end

  @doc "`long long`."
  def write_i64(%__MODULE__{} = w, value) do
    check(w, value, -0x8000_0000_0000_0000, 0x7FFF_FFFF_FFFF_FFFF, "a long long")
    int(w, value &&& 0xFFFF_FFFF_FFFF_FFFF, 8)
  end

  @doc "`unsigned long long`."
  def write_u64(%__MODULE__{} = w, value) do
    check(w, value, 0, 0xFFFF_FFFF_FFFF_FFFF, "an unsigned long long")
    int(w, value, 8)
  end

  @doc "`float`, rounded to single precision once, here."
  def write_f32(%__MODULE__{} = w, value), do: int(w, WireFloat.f32_bits(value), 4)

  @doc "`double`."
  def write_f64(%__MODULE__{} = w, value), do: int(w, WireFloat.f64_bits(value), 8)

  @doc """
  `long double`: sixteen bytes of IEEE 754 binary128, aligned to eight
  (capped to four under XCDR2 by `max_align`). The two byte orders are
  the whole sixteen bytes reversed — CORBA 3.3 part 2 Figure 9.2.

  A plain number is widened exactly — every binary64 is a binary128 —
  and a value that needs more than 53 significand bits has to arrive as
  a `ZeroddsWire.Float128`.
  """
  def write_f128(%__MODULE__{} = w, %Float128{} = value) do
    w = align_to(w, 8)
    %{w | out: w.out <> Float128.to_bytes(value, w.endianness)}
  end

  def write_f128(%__MODULE__{} = w, value) do
    write_f128(w, Float128.from_number(value))
  end

  # -- strings -------------------------------------------------------------

  @doc """
  A string: a four-byte length including the terminator, the UTF-8
  bytes, then a NUL. XTypes 1.3 §7.4.1.1 — the length counts *bytes*,
  terminator included.
  """
  def write_string(%__MODULE__{} = w, value) when is_binary(value) do
    w = write_u32(w, byte_size(value) + 1)
    %{w | out: w.out <> value <> <<0>>}
  end

  def write_string(%__MODULE__{} = w, value), do: refuse(w, "a string, not #{inspect(value)}")

  @doc """
  A bounded `string`: the same bytes `write_string/2` writes. The bound
  is a promise about the source and not a difference on the wire, so
  this is the only place it can be enforced — refused rather than cut
  at a character boundary, which would put a sample on the wire nobody
  wrote.
  """
  def write_string_bounded(%__MODULE__{} = w, value, bound) when is_binary(value) do
    octets = byte_size(value)

    if octets > bound do
      refuse(
        w,
        "at most #{bound} octets in a bounded string, and this one has #{octets}"
      )
    end

    write_string(w, value)
  end

  def write_string_bounded(%__MODULE__{} = w, value, _bound),
    do: refuse(w, "a string, not #{inspect(value)}")

  @doc """
  The count check every bounded `sequence` makes before a byte of it is
  written. Table 17 makes `O2.length <= T1.length` the condition for
  constructing a `sequence<T, N>`, and the wire carries no bound to
  compare against: a sender whose count already exceeds its own
  declaration has written a value of a type nobody declared.
  """
  def check_sequence_bound(%__MODULE__{} = w, actual, bound) do
    if actual > bound do
      refuse(
        w,
        "at most #{bound} elements in a bounded sequence, and this one has #{actual}"
      )
    end

    w
  end

  @doc """
  The count check every bounded `map` makes before a byte of it is
  written. Its own message and not `check_sequence_bound/3`'s: the unit
  is an entry and not an element, per XTypes 1.3 §7.4.1.1.4's
  map-as-a-sequence-of-pairs equivalence.
  """
  def check_map_bound(%__MODULE__{} = w, actual, bound) do
    if actual > bound do
      refuse(w, "at most #{bound} entries in a bounded map, and this one has #{actual}")
    end

    w
  end

  @doc """
  A `wstring`, in the form `policy` and `order` name — the three
  questions (which units, what the length counts, what byte order each
  unit lands in) answered separately, the same product
  `runtime/python`'s `write_wstring` documents cell by cell.
  """
  def write_wstring_utf8(%__MODULE__{} = w, value, policy, order) do
    if not is_binary(value) do
      refuse(w, "a wstring, not #{inspect(value)}")
    end

    cell = ZeroddsWire.wstring_cell(policy)

    if cell == nil do
      refuse(w, "a wstring length policy, not #{inspect(policy)}")
    end

    if not ZeroddsWire.wstring_unit_order?(order) do
      refuse(w, "a wstring unit order, not #{inspect(order)}")
    end

    {counts_characters, terminated, width} = cell

    if counts_characters and width == 2 do
      # A character count over two-octet units has no way to carry a
      # code point outside the BMP; a vendor's length convention is a
      # policy, its data loss is not.
      Enum.with_index(String.to_charlist(value))
      |> Enum.each(fn {point, index} ->
        if point > 0xFFFF do
          raise WireError,
            message:
              "at #{index}: expected a code point inside the BMP: a character count " <>
                "over two-octet units has no way to carry one outside it",
            at: index,
            kind: :out_of_range
        end
      end)
    end

    units = ZeroddsWire.utf16_unit_list(value)
    units = if terminated, do: units ++ [0], else: units

    # An empty `wstring` carries no mark; measured, omniORB writes
    # `00000000` for one.
    units =
      if order == :"byte-order-mark" and units != [] do
        [0xFEFF | units]
      else
        units
      end

    count = if counts_characters, do: length(units), else: length(units) * width
    w = write_u32(w, count)
    Enum.reduce(units, w, fn unit, w -> wstring_unit(w, unit, width, order) end)
  end

  @doc """
  A bounded `wstring`, as `write_string_bounded/3` is to `write_string/2`.
  The bound counts UTF-16 code units, not characters and not octets,
  whatever `policy` puts on the wire.
  """
  def write_wstring_bounded(%__MODULE__{} = w, value, bound, policy, order) do
    units = length(ZeroddsWire.utf16_unit_list(value))

    if units > bound do
      refuse(
        w,
        "at most #{bound} UTF-16 code units in a bounded wstring, and this one has #{units}"
      )
    end

    write_wstring_utf8(w, value, policy, order)
  end

  @doc """
  `WstringCodeUnits::Utf8`: the characters' own UTF-8 octets, with the
  length an octet count rather than §7.4.1.1.2's "twice the number of
  characters". `wstring-member` is `04 00 00 00 61 62 63 00` for
  `"abc"`; only C2 — the terminator — still reaches the wire. A binary
  that is not UTF-8 is refused rather than replaced.
  """
  def write_wstring_utf8_octets(%__MODULE__{} = w, value, policy) when is_binary(value) do
    if not String.valid?(value) do
      refuse(w, "a wstring whose octets are UTF-8")
    end

    cell = ZeroddsWire.wstring_cell(policy)

    if cell == nil do
      refuse(w, "a wstring length policy, not #{inspect(policy)}")
    end

    {_counts_characters, terminated, _width} = cell
    length = byte_size(value) + if terminated, do: 1, else: 0
    w = write_u32(w, length)
    w = %{w | out: w.out <> value}

    if terminated, do: %{w | out: w.out <> <<0>>}, else: w
  end

  def write_wstring_utf8_octets(%__MODULE__{} = w, value, _policy),
    do: refuse(w, "a wstring, not #{inspect(value)}")

  @doc """
  A bounded UTF-8-octet `wstring`, as `write_wstring_bounded/5` is to
  `write_wstring_utf8/4`. The bound still counts UTF-16 code units, which
  is what every `wstring<N>` bound means, while the wire carries the
  UTF-8 octets.
  """
  def write_wstring_utf8_octets_bounded(%__MODULE__{} = w, value, bound, policy) do
    units = length(ZeroddsWire.utf16_unit_list(value))

    if units > bound do
      refuse(
        w,
        "at most #{bound} UTF-16 code units in a bounded wstring, and this one has #{units}"
      )
    end

    write_wstring_utf8_octets(w, value, policy)
  end

  defp wstring_unit(w, unit, width, order) do
    little = order in [:"byte-order-mark", :"host-unswapped"]

    cond do
      order == :"big-endian-always" -> fixed_order_unit(w, unit, width, :be)
      little -> fixed_order_unit(w, unit, width, :le)
      width == 4 -> write_u32(w, unit)
      true -> write_u16(w, unit)
    end
  end

  # A unit in a fixed byte order whatever the stream's is, aligned to
  # its own width like any other value of that width.
  defp fixed_order_unit(w, unit, width, order) do
    w = align_to(w, width)

    case order do
      :le -> %{w | out: w.out <> <<unit::unsigned-little-size(width * 8)>>}
      :be -> %{w | out: w.out <> <<unit::unsigned-big-size(width * 8)>>}
    end
  end

  # -- fixed ---------------------------------------------------------------

  @doc """
  `fixed<digits, scale>` as packed BCD — [CDR] §9.3.2.8, the only wire
  definition an IDL `fixed` has. The value arrives as a decimal string
  because a JSON number is a double and `fixed<31, 2>` is exactly the
  case a double cannot carry.

  `count` is `:declared`, `:"value-precision"` (as many digits as the
  value has, never fewer than one — JacORB 3.9) or
  `:"value-string-digits"`: the integer digits of the decimal as written plus S, made odd
  by one leading zero, which ACE/TAO 2.5.24's `ACE_CDR::Fixed::from_string`
  writes (OpenDDS writes the declared count; `0.12345` under `fixed<5,5>` is `0012345c`; past
  31 digits it is refused).
  """
  def write_fixed(%__MODULE__{} = w, value, digits, scale, count) do
    nibbles = fixed_nibbles(w, value, digits, scale, count)

    packed =
      nibbles
      |> Enum.chunk_every(2)
      |> Enum.map(fn [high, low] -> <<high <<< 4 ||| low>> end)
      |> IO.iodata_to_binary()

    %{w | out: w.out <> packed}
  end

  @doc false
  def fixed_nibbles(w, text, digits, scale, count) do
    if digits == 0, do: refuse(w, "a fixed with at least one digit")
    # OMG IDL 4.2 §7.4.1.4.4.3.4 rule (42) caps the BCD rule at 31
    # digits, and `fixed<31, S>` is the widest declaration every measured
    # ORB family accepts. A wider field has no defined packing, so a
    # value written into one is octets no reader can be told how to read.
    if digits > 31, do: refuse(w, "a fixed of at most 31 digits, not #{digits}")
    if scale > digits, do: refuse(w, "a scale no larger than the digit count")

    {negative, magnitude} =
      case text do
        "-" <> rest -> {true, rest}
        "+" <> rest -> {false, rest}
        _ -> {false, text}
      end

    {whole, fraction} =
      case String.split(magnitude, ".", parts: 2) do
        [whole] -> {whole, ""}
        [whole, fraction] -> {whole, fraction}
      end

    if whole == "" and fraction == "" do
      refuse(w, "a decimal that fits fixed<#{digits}, #{scale}>, not `#{text}`")
    end

    if not digits_only?(whole <> fraction) do
      refuse(w, "a decimal that fits fixed<#{digits}, #{scale}>, not `#{text}`")
    end

    as_written = String.length(whole) + scale
    integral_width = digits - scale
    whole = String.trim_leading(whole, "0")

    if String.length(whole) > integral_width do
      refuse(w, "a decimal whose integer part fits #{integral_width} digits, not `#{text}`")
    end

    if String.length(fraction) > scale do
      refuse(w, "a decimal whose fraction fits #{scale} digits, not `#{text}`")
    end

    if count not in [:declared, :"value-precision", :"value-string-digits"] do
      refuse(w, "a fixed digit count, not `#{inspect(count)}`")
    end

    every =
      List.duplicate(0, integral_width - String.length(whole)) ++
        digit_values(whole) ++
        digit_values(fraction) ++
        List.duplicate(0, scale - String.length(fraction))

    significant = length(Enum.drop_while(every, &(&1 == 0)))

    wire =
      case count do
        :"value-precision" ->
          max(significant, 1)

        :"value-string-digits" ->
          # ACE/TAO 2.5.24: the integer digits as written plus S, made odd by one leading
          # zero. Past 31 no reader can be told the field's width.
          wire = as_written + if(rem(as_written, 2) == 0, do: 1, else: 0)

          if wire > 31 do
            refuse(w, "a fixed of at most 31 digits on the wire, not #{wire} for `#{text}`")
          end

          wire

        :declared ->
          length(every)
      end

    lead = if rem(wire + 1, 2) == 1, do: [0], else: []
    # A count wider than P is filled with leading zeros.
    kept = min(wire, length(every))
    body = List.duplicate(0, wire - kept) ++ Enum.drop(every, length(every) - kept)
    sign = if negative and Enum.any?(every, &(&1 != 0)), do: 0x0D, else: 0x0C
    lead ++ body ++ [sign]
  end

  defp digits_only?(text), do: text != "" and String.match?(text, ~r/^[0-9]*$/)

  defp digit_values(text), do: Enum.map(String.to_charlist(text), &(&1 - ?0))

  # -- the bit-bounded kinds -----------------------------------------------

  @doc """
  An enumerator, as the integer its bit bound calls for — a **signed**
  holder (§7.4.3.5.3 rule (5)), so what fits is `-2^(n-1) ..= 2^n - 1`
  and `@value(-1)` at the default bound goes out as `ffffffff`.
  §7.3.1.2.1.5 caps the bound at 32.
  """
  def write_enum(%__MODULE__{} = w, bit_bound, value) do
    if bit_bound == 0 or bit_bound > 32 do
      refuse(w, "an @bit_bound between 1 and 32, not #{bit_bound}")
    end

    if value < -(1 <<< (bit_bound - 1)) or value > (1 <<< bit_bound) - 1 do
      refuse(w, "an enumerator a #{bit_bound}-bit holder can hold, not #{value}")
    end

    pattern = value &&& (1 <<< bit_bound) - 1

    cond do
      bit_bound <= 8 -> write_u8(w, pattern)
      bit_bound <= 16 -> write_u16(w, pattern)
      true -> write_u32(w, pattern)
    end
  end

  @doc """
  A bitmask, as the unsigned integer its bit bound calls for — Table 33
  of §7.4.1.1.3.2, bit `n` is `1 <<< n` whatever the holder's width.
  """
  def write_bitmask(%__MODULE__{} = w, bit_bound, value) do
    if bit_bound == 0 or bit_bound > 64 do
      refuse(w, "an @bit_bound between 1 and 64, not #{bit_bound}")
    end

    if value < 0 or value >= 1 <<< bit_bound do
      refuse(w, "a bitmask below 2^#{bit_bound}, not #{value}")
    end

    cond do
      bit_bound <= 8 -> write_u8(w, value)
      bit_bound <= 16 -> write_u16(w, value)
      bit_bound <= 32 -> write_u32(w, value)
      true -> write_u64(w, value)
    end
  end

  @doc """
  One bitfield, shifted into the accumulator a bitset packs into. Pure
  — the writer is a parameter only because the shared walk calls every
  operation on a receiver. A value wider than the field is refused
  rather than masked.
  """
  def bitfield(%__MODULE__{} = w, value, width, position) do
    if width <= 0 or width > 64 or position < 0 or position + width > 64 do
      refuse(w, "a bitfield of #{width} bits at #{position}, inside 64")
    end

    limit = (1 <<< width) - 1

    if value < 0 or value > limit do
      refuse(w, "a value inside #{width} bits, not #{value}")
    end

    value <<< position
  end

  @doc "A bitset, as the anchor integer its bitfields pack into."
  def write_bitset(%__MODULE__{} = w, anchor, value) do
    case anchor do
      1 -> write_u8(w, value)
      2 -> write_u16(w, value)
      4 -> write_u32(w, value)
      8 -> write_u64(w, value)
      _ -> refuse(w, "a bitset anchor of 1, 2, 4 or 8 bytes, not #{anchor}")
    end
  end

  @doc """
  The Cyclone union overlay: the low `octets` of `pattern`, as one
  unsigned integer of that width in the stream's byte order.
  """
  def write_union_overlay(%__MODULE__{} = w, pattern, octets) do
    case octets do
      1 -> write_u8(w, pattern &&& 0xFF)
      2 -> write_u16(w, pattern &&& 0xFFFF)
      4 -> write_u32(w, pattern &&& 0xFFFF_FFFF)
      8 -> write_u64(w, pattern &&& 0xFFFF_FFFF_FFFF_FFFF)
      _ -> refuse(w, "a union arm of 1, 2, 4 or 8 octets, not #{octets}")
    end
  end

  # -- framing -------------------------------------------------------------

  @doc """
  A DHEADER: a four-byte placeholder for the length of what follows,
  patched by `finish_dheader/2`. The handle is the placeholder's
  position; `ZeroddsWire.no_dheader/0` is the position below zero a
  `@final` aggregate hands the finish instead.
  """
  def write_dheader(%__MODULE__{} = w) do
    w = align_to(w, 4)
    at = byte_size(w.out)
    {at, %{w | out: w.out <> <<0, 0, 0, 0>>}}
  end

  @doc "Patch a DHEADER with the length of what followed it."
  def finish_dheader(%__MODULE__{} = w, at) when at < 0, do: w

  def finish_dheader(%__MODULE__{} = w, at) do
    patch(w, at, byte_size(w.out) - at - 4, 4)
  end

  @doc """
  An XCDR2 EMHEADER — §7.4.3.4.2. LC4 reserves a NEXTINT; LC5 to LC7
  reuse the member's own leading word as theirs, so none is reserved.

  A member id above `0x0FFF_FFFF` is refused rather than masked.
  §7.2.2.4.4.4.4 gives ids 0 to 0x0FFFFFFF and the header formula of
  §7.4.3.4.2 masks with `0x0FFFFFFF`, so a larger id would produce a
  well-formed header naming `id &&& 0x0FFFFFFF` — another member. The
  reader has no way to notice, which is why it is refused here.
  """
  def write_emheader(%__MODULE__{} = w, member_id, length_code, must_understand) do
    if not is_integer(member_id) or member_id < 0 or
         member_id > ZeroddsWire.e_member_id_mask() do
      refuse(
        w,
        "a member id inside the 28 bits of an EMHEADER, not #{inspect(member_id)}"
      )
    end

    w = align_to(w, 4)

    word =
      if(must_understand, do: 1 <<< 31, else: 0) |||
        (length_code &&& 0x7) <<< 28 ||| (member_id &&& 0x0FFF_FFFF)

    w = write_u32(w, word)
    word_at = byte_size(w.out) - 4

    if length_code == 4 do
      at = byte_size(w.out)

      {%{next_int: at, word: word_at, length_code: length_code},
       %{w | out: w.out <> <<0, 0, 0, 0>>}}
    else
      {%{next_int: nil, word: word_at, length_code: length_code}, w}
    end
  end

  @doc """
  Patch an EMHEADER's NEXTINT with the length of what followed. Under
  `short_length_codes` an LC4 whose member turned out 1, 2, 4 or 8
  bytes long loses its NEXTINT and states the length in the code —
  Fast DDS's form; safe only under XCDR2, where MAXALIGN is four.

  Under LC 0 to 3 there is no NEXTINT and the code *is* the length —
  1, 2, 4 or 8 octets, §7.4.3.4.2. A member body of any other size
  misdeclares itself and the reader's skip lands inside data, so the
  finish measures what was written and refuses a mismatch. LC 4 to 7
  carry every other size, so there is never a reason to lie.
  """
  def finish_emheader(%__MODULE__{} = w, %{next_int: nil} = header) do
    code = Map.get(header, :length_code)
    word_at = Map.get(header, :word)

    if is_integer(code) and code <= 3 and is_integer(word_at) and word_at + 4 <= byte_size(w.out) do
      declared = 1 <<< code
      body = byte_size(w.out) - word_at - 4

      if body != declared do
        refuse(
          w,
          "a member of #{declared} octets under length code #{code}, not #{body}"
        )
      end
    end

    w
  end

  def finish_emheader(%__MODULE__{} = w, %{next_int: at, word: word_at}) do
    length = byte_size(w.out) - at - 4

    if w.short_length_codes and length in [1, 2, 4, 8] do
      code = %{1 => 0, 2 => 1, 4 => 2, 8 => 3}[length]
      # `binary-size(^at)` is the obvious spelling and is a syntax
      # Elixir 1.14 rejects; see `reader.ex`'s `unsigned_of/2` for the
      # whole reason. `binary_part/3` says the same thing and has been
      # in Elixir since 1.0.
      before = binary_part(w.out, 0, at)
      rest = binary_part(w.out, at + 4, byte_size(w.out) - at - 4)
      w = %{w | out: before <> rest}
      previous = read_word(w, word_at)
      patch(w, word_at, (previous &&& bnot(0x7 <<< 28)) ||| code <<< 28, 4)
    else
      patch(w, at, length, 4)
    end
  end

  defp read_word(%__MODULE__{out: out, endianness: order}, at) do
    word = binary_part(out, at, 4)

    case order do
      :le ->
        <<value::unsigned-little-32>> = word
        value

      :be ->
        <<value::unsigned-big-32>> = word
        value
    end
  end

  @doc """
  A PL_CDR1 parameter header, short form: `[id:u16][length:u16]`, with
  the member's own must-understand and the framing's per-position flags
  ORed onto the id (§7.4.1.2.2). Opens the parameter's value block: a
  parameter is its own CDR-encapsulated block and counts alignment from
  its own start.

  A pid above the framing's `highest_short_id` is refused rather than
  written. §7.4.1.2.2 gives the short form ids 0 to 0x3F00 inclusive and
  reserves 0x3F01 to 0x3FFF for the protocol's own parameters —
  `PID_EXTENDED` is 0x3F01 itself — so a short header in that range
  collides with a parameter the peer reads as its own.
  `write_pid_extended/4` is the form that carries such an id. 0x3F00
  exactly still takes the short form; that off-by-one was a recorded
  defect.
  """
  def write_pid(%__MODULE__{} = w, pid, role, must_understand) do
    if not is_integer(pid) or pid < 0 or pid > w.framing.highest_short_id do
      refuse(
        w,
        "a parameter id the short header carries, not #{inspect(pid)}" <>
          " — the extended header takes it"
      )
    end

    w = align_to(w, 4)

    flagged =
      pid
      |> then(fn pid ->
        if must_understand and w.framing.member_must_understand do
          pid ||| ZeroddsWire.pid_must_understand()
        else
          pid
        end
      end)
      |> bor(ZeroddsWire.pid_flags_at(w.framing.pid_flags, :member))

    bare = pid &&& bnot(ZeroddsWire.pid_must_understand() ||| ZeroddsWire.pid_impl_extension())
    w = write_u16(w, flagged)
    at = byte_size(w.out)
    w = %{w | out: w.out <> <<0, 0>>}
    saved = w.origin
    w = %{w | origin: byte_size(w.out)}

    {%{
       at: at,
       extended: false,
       saved_origin: saved,
       role: role,
       pid: bare,
       must_understand: must_understand
     }, w}
  end

  @doc """
  A PL_CDR1 parameter header, extended form:
  `[PID_EXTENDED][8][id:u32][length:u32]`. The member's must-understand
  is `FLAG_2` of the `eMemberHeader` (§7.4.1.2.1), not the marker's own
  bit.

  A member id above `0x0FFF_FFFF` is refused rather than masked, for the
  reason `write_emheader/4` gives: the `eMemberHeader` of §7.4.1.2.1 has
  the same 28 bits, and masking aliases the parameter onto another
  member.
  """
  def write_pid_extended(%__MODULE__{} = w, pid, role, must_understand) do
    if not is_integer(pid) or pid < 0 or pid > ZeroddsWire.e_member_id_mask() do
      refuse(
        w,
        "a member id inside the 28 bits of an eMemberHeader, not #{inspect(pid)}"
      )
    end

    w = align_to(w, 4)

    marker =
      ZeroddsWire.pid_extended() |||
        ZeroddsWire.pid_flags_at(w.framing.pid_flags, :extended_marker)

    w = w |> write_u16(marker) |> write_u16(8)

    member_header =
      (pid &&& ZeroddsWire.e_member_id_mask())
      |> then(fn header ->
        if must_understand and w.framing.member_must_understand do
          header ||| ZeroddsWire.e_flag_must_understand()
        else
          header
        end
      end)

    w = write_u32(w, member_header)
    at = byte_size(w.out)
    w = %{w | out: w.out <> <<0, 0, 0, 0>>}
    saved = w.origin
    w = %{w | origin: byte_size(w.out)}

    {%{
       at: at,
       extended: true,
       saved_origin: saved,
       role: role,
       pid: pid,
       must_understand: must_understand
     }, w}
  end

  @doc """
  Patch a parameter header with its value's length. The declared
  length covers the trailing padding or stops at the value, per the
  framing and the role; the padding itself is written only under a
  framing that pads after the value, and the origin goes back before
  it is. A short header whose length overruns sixteen bits is
  rewritten as the extended form in place — §7.4.1.2.2's `ssize`.
  """
  def finish_pid(%__MODULE__{} = w, header) do
    width = if header.extended, do: 4, else: 2
    length = byte_size(w.out) - header.at - width

    length =
      if PlCdr1Framing.pads_declared_length(w.framing, header.role) do
        past = rem(byte_size(w.out) - header.saved_origin, 4)
        if past != 0, do: length + 4 - past, else: length
      else
        length
      end

    w =
      if not header.extended and length > @pid_short_max_len do
        promote_pid(w, header, length)
      else
        patch(w, header.at, length, width)
      end

    w = %{w | origin: header.saved_origin}
    if w.framing.pad_after_value, do: align_to(w, 4), else: w
  end

  # Rewrite a short parameter header as the extended form: eight bytes
  # inserted between header and value, every offset inside the value
  # keeping its remainder because XCDR1's MAXALIGN is eight.
  defp promote_pid(%__MODULE__{} = w, header, length) do
    start = header.at - 2
    cut = header.at + 2
    before = binary_part(w.out, 0, cut)
    rest = binary_part(w.out, cut, byte_size(w.out) - cut)
    w = %{w | out: before <> :binary.copy(<<0>>, 8) <> rest}

    marker =
      ZeroddsWire.pid_extended() |||
        ZeroddsWire.pid_flags_at(w.framing.pid_flags, :extended_marker)

    member_header =
      (header.pid &&& ZeroddsWire.e_member_id_mask())
      |> then(fn value ->
        if header.must_understand and w.framing.member_must_understand do
          value ||| ZeroddsWire.e_flag_must_understand()
        else
          value
        end
      end)

    w
    |> patch(start, marker, 2)
    |> patch(start + 2, 8, 2)
    |> patch(start + 4, member_header, 4)
    |> patch(start + 8, length, 4)
  end

  @doc """
  The `PID_LIST_END` sentinel a PL_CDR1 parameter list ends with, its
  flags from the framing's `"sentinel"` position — the sentinel is not
  a member and has no declaration of its own.
  """
  def write_sentinel(%__MODULE__{} = w) do
    w = align_to(w, 4)

    sentinel =
      ZeroddsWire.pid_list_end() ||| ZeroddsWire.pid_flags_at(w.framing.pid_flags, :sentinel)

    w |> write_u16(sentinel) |> write_u16(0)
  end

  defp patch(%__MODULE__{} = w, at, value, width) do
    if value < 0 or value >= 1 <<< (width * 8) do
      raise WireError,
        message: "at #{at}: expected a #{width}-byte length, not #{value}",
        at: at,
        kind: :out_of_range
    end

    before = binary_part(w.out, 0, at)
    rest = binary_part(w.out, at + width, byte_size(w.out) - at - width)

    encoded =
      case w.endianness do
        :le -> <<value::unsigned-little-size(width * 8)>>
        :be -> <<value::unsigned-big-size(width * 8)>>
      end

    %{w | out: before <> encoded <> rest}
  end

  defp one_code_point(w, value) do
    case is_binary(value) && String.to_charlist(value) do
      [point] -> point
      _ -> refuse(w, "exactly one character, not #{inspect(value)}")
    end
  end
end
