# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
defmodule ZeroddsWire.Reader do
  @moduledoc """
  Taking bytes off the wire — the mirror of `ZeroddsWire.Writer` clause
  for clause, and the Elixir counterpart of `crates/wire/src/reader.rs`.

  Two kinds of failure, kept apart the way every sibling runtime keeps
  them. A malformed **stream** raises `ZeroddsWire.WireError`: a length
  past the end of the buffer is not a state a caller can carry on from.
  A refusal about the **type** sets the reader's `status` and the
  generated decoder walks on to the end of the sample, so one test at
  the top of the caller decides what the whole read was.

  A stopped reader answers a default and does not move — the C#
  `Reader`'s discipline, which is what keeps a decoder that carries on
  after a refusal from raising a second, invented, fault at a position
  no writer put anything at.

  Every value-producing operation answers `{value, reader}`; every
  other one answers the reader.

  Two field spellings differ from the Python runtime's, deliberately:

  - a parameter header's sentinel flag is `list_end`, because `end` is
    an Elixir reserved word and does not parse after a dot;
  - `finish_pid_read` measures the value against its own start and
    checks the declared length against the framing rather than
    believing it, and steps the trailing padding only under a framing
    that writes any (`pad_after_value`) — the C# reader's logic, which
    is what lets one reader take RTI's rounded-up length and
    Fast DDS's exact, unpadded one.
  """

  import Bitwise

  alias ZeroddsWire.Float, as: WireFloat
  alias ZeroddsWire.Float128
  alias ZeroddsWire.PlCdr1Framing
  alias ZeroddsWire.WireError

  defstruct bytes: <<>>,
            at: 0,
            endianness: :le,
            max_align: 8,
            origin: 0,
            framing: %PlCdr1Framing{},
            status: :ok

  # The low fourteen bits of a short parameter id, which are the id.
  @pid_id_mask 0x3FFF

  @doc "A reader over `bytes`, capped at `max_align` — the writer's counterpart."
  def new(bytes, endianness, max_align \\ 8, framing \\ %PlCdr1Framing{})
      when endianness in [:le, :be] do
    %__MODULE__{
      bytes: bytes,
      at: 0,
      endianness: endianness,
      max_align: max(1, max_align),
      origin: 0,
      framing: framing,
      status: :ok
    }
  end

  # -- the status a generated decoder reads --------------------------------

  @doc "How the read is going."
  def reader_status(%__MODULE__{status: status}), do: status

  @doc """
  Refuse the stream from outside the runtime. The first refusal sticks:
  everything read after it happens at a position no writer need have
  put anything at, and a second refusal would describe that rather than
  the fault. `:ok` refuses nothing.
  """
  def reader_refuse(%__MODULE__{} = r, :ok), do: r
  def reader_refuse(%__MODULE__{status: :ok} = r, refusal), do: %{r | status: refusal}
  def reader_refuse(%__MODULE__{} = r, _refusal), do: r

  @doc """
  Take back a `:discarded`, and only that one — the `USE_DEFAULT` plus
  explicit `@default` combination the call site answers itself. Every
  other status stays: a truncated stream is not a narrowing.
  """
  def reader_clear_discard(%__MODULE__{status: :discarded} = r), do: {true, %{r | status: :ok}}
  def reader_clear_discard(%__MODULE__{} = r), do: {false, r}

  @doc "Where it is."
  def reader_position(%__MODULE__{at: at}), do: at

  @doc "How many bytes are left."
  def reader_remaining(%__MODULE__{bytes: bytes, at: at}), do: max(0, byte_size(bytes) - at)

  @doc """
  Move the cursor forward to `target` — never backwards, which would
  mean reading a member twice and is a refusal about the stream's
  shape; past the end raises, because there is no position there to
  carry on from.
  """
  def reader_skip_to(%__MODULE__{} = r, target) do
    cond do
      r.status != :ok -> r
      target < r.at -> reader_refuse(r, :out_of_range)
      target > byte_size(r.bytes) -> raise_truncated(r, target - r.at)
      true -> %{r | at: target}
    end
  end

  @doc "The cursor, the origin and the status, to be put back later."
  def reader_save(%__MODULE__{} = r), do: %{at: r.at, origin: r.origin, status: r.status}

  @doc "Put back what `reader_save/1` took."
  def reader_restore(%__MODULE__{} = r, mark) do
    %{r | at: mark.at, origin: mark.origin, status: mark.status}
  end

  @doc "The read-side pair of `Writer.begin_packed_element/1`."
  def reader_begin_packed_element(%__MODULE__{} = r) do
    saved = r.origin
    {saved, %{r | origin: r.at}}
  end

  @doc "Restore the origin `reader_begin_packed_element/1` displaced."
  def reader_finish_packed_element(%__MODULE__{} = r, saved) do
    %{r | origin: saved}
  end

  # -- alignment and the raw take ------------------------------------------

  @doc "Skip padding to `align`, counted from the stream's origin."
  def align_to(%__MODULE__{} = r, align) do
    align = min(align, r.max_align)

    if align <= 1 do
      r
    else
      offset = r.at - r.origin
      %{r | at: r.at + rem(align - rem(offset, align), align)}
    end
  end

  defp raise_truncated(%__MODULE__{} = r, needed) do
    raise WireError,
      message: "at #{r.at}: #{needed} bytes needed, #{reader_remaining(r)} available",
      at: r.at,
      kind: :truncated
  end

  defp raise_impossible(%__MODULE__{} = r, at, claimed) do
    raise WireError,
      message: "at #{at}: a length of #{claimed} with #{reader_remaining(r)} bytes left",
      at: at,
      kind: :impossible_length
  end

  defp raise_out_of_range(at, expected) do
    raise WireError, message: "at #{at}: expected #{expected}", at: at, kind: :out_of_range
  end

  defp take(%__MODULE__{} = r, count) do
    if r.at + count > byte_size(r.bytes) do
      raise_truncated(r, count)
    end

    {binary_part(r.bytes, r.at, count), %{r | at: r.at + count}}
  end

  # An integer of `byte_size(raw)` octets, without a bitstring size
  # specification.
  #
  # `<<v::unsigned-little-size(^bits)>> = raw` is the obvious spelling and
  # cannot be used: the pin in a size specification is a syntax Elixir
  # 1.14 rejects outright — `cannot use ^bits outside of match clauses`,
  # which is what the CI image (Debian bookworm, Elixir 1.14.0) answered
  # — while the same line without the pin draws a deprecation warning
  # from 1.19 on, and warnings are failures here. There is no `size(...)`
  # spelling that both accept.
  #
  # `:binary.decode_unsigned/2` has been in OTP since R14 and takes the
  # width from the binary itself, which is exactly what `bits` was
  # computed from. Same value, no version to depend on.
  defp unsigned_of(raw, order), do: :binary.decode_unsigned(raw, order)

  # The same octets read as two's complement, §7.4.1.1.1: the top bit of
  # the first octet in reading order is the sign, and a set sign means
  # the value less 2^bits.
  defp signed_of(raw, order) do
    bits = byte_size(raw) * 8
    value = :binary.decode_unsigned(raw, order)
    if value >= 1 <<< (bits - 1), do: value - (1 <<< bits), else: value
  end

  defp int(%__MODULE__{} = r, width, signed) do
    r = align_to(r, width)
    {raw, r} = take(r, width)

    value =
      case {r.endianness, signed} do
        {:le, false} -> unsigned_of(raw, :little)
        {:le, true} -> signed_of(raw, :little)
        {:be, false} -> unsigned_of(raw, :big)
        {:be, true} -> signed_of(raw, :big)
      end

    {value, r}
  end

  # A stopped reader answers the default and does not move.
  defp guarded(%__MODULE__{status: status} = r, default, produce) do
    if status != :ok, do: {default, r}, else: produce.(r)
  end

  # -- primitives ----------------------------------------------------------

  @doc "`boolean`. A byte that is neither 0 nor 1 is refused, not coerced."
  def read_bool(%__MODULE__{} = r) do
    guarded(r, false, fn r ->
      at = r.at
      {value, r} = read_u8(r)

      case value do
        0 -> {false, r}
        1 -> {true, r}
        _ -> raise_out_of_range(at, "a boolean of 0 or 1")
      end
    end)
  end

  @doc "`octet` and `uint8`."
  def read_u8(%__MODULE__{} = r) do
    guarded(r, 0, fn r ->
      {<<value>>, r} = take(r, 1)
      {value, r}
    end)
  end

  @doc "`int8`."
  def read_i8(%__MODULE__{} = r) do
    guarded(r, 0, fn r ->
      {<<value::signed-8>>, r} = take(r, 1)
      {value, r}
    end)
  end

  @doc "`char`, one byte, as a one-character binary."
  def read_char(%__MODULE__{} = r) do
    guarded(r, <<0>>, fn r ->
      {<<point>>, r} = take(r, 1)
      {List.to_string([point]), r}
    end)
  end

  @doc """
  `wchar`, at `width` octets and in `framing`'s shape. The four-byte
  form zero-extends one UTF-16 code unit; a value using the high bits
  is refused rather than narrowed. `giop-length-prefixed` checks the
  declared octet count against the configured width rather than
  believing it. A surrogate is refused — it is not a code point, and no
  Elixir binary could hold it anyway.
  """
  def read_wchar(%__MODULE__{} = r, width, framing, order) do
    guarded(r, <<0>>, fn r ->
      at = r.at

      if width not in [2, 4] do
        raise_out_of_range(at, "a wchar width of 2 or 4, not #{inspect(width)}")
      end

      {value, r} =
        case framing do
          :"giop-length-prefixed" ->
            {declared, r} = read_u8(r)

            if declared != width do
              raise_out_of_range(
                at,
                "a GIOP 1.2 wchar of #{width} octets, the configured transmission " <>
                  "code set, and not #{declared}"
              )
            end

            {raw, r} = take(r, width)

            big = order != :stream or r.endianness == :be

            value = unsigned_of(raw, if(big, do: :big, else: :little))

            {value, r}

          :fixed ->
            if width == 4 do
              {value, r} = read_u32(r)

              if value > 0xFFFF do
                raise_out_of_range(at, "a four-byte wchar holding one UTF-16 code unit")
              end

              {value, r}
            else
              read_u16(r)
            end

          other ->
            raise_out_of_range(at, "a wchar framing, not #{inspect(other)}")
        end

      if value >= 0xD800 and value <= 0xDFFF do
        raise_out_of_range(at, "a Unicode code point")
      end

      {List.to_string([value]), r}
    end)
  end

  @doc "`short`."
  def read_i16(%__MODULE__{} = r), do: guarded(r, 0, &int(&1, 2, true))
  @doc "`unsigned short`."
  def read_u16(%__MODULE__{} = r), do: guarded(r, 0, &int(&1, 2, false))
  @doc "`long`."
  def read_i32(%__MODULE__{} = r), do: guarded(r, 0, &int(&1, 4, true))
  @doc "`unsigned long`."
  def read_u32(%__MODULE__{} = r), do: guarded(r, 0, &int(&1, 4, false))
  @doc "`long long`."
  def read_i64(%__MODULE__{} = r), do: guarded(r, 0, &int(&1, 8, true))
  @doc "`unsigned long long`."
  def read_u64(%__MODULE__{} = r), do: guarded(r, 0, &int(&1, 8, false))

  @doc "`float`."
  def read_f32(%__MODULE__{} = r) do
    guarded(r, 0.0, fn r ->
      {bits, r} = int(r, 4, false)
      {WireFloat.from_f32_bits(bits), r}
    end)
  end

  @doc "`double`."
  def read_f64(%__MODULE__{} = r) do
    guarded(r, 0.0, fn r ->
      {bits, r} = int(r, 8, false)
      {WireFloat.from_f64_bits(bits), r}
    end)
  end

  @doc "`long double`: all sixteen bytes, as IEEE 754 binary128."
  def read_f128(%__MODULE__{} = r) do
    guarded(r, Float128.zero(), fn r ->
      r = align_to(r, 8)
      {raw, r} = take(r, 16)
      {Float128.from_bytes(raw, r.endianness), r}
    end)
  end

  # -- strings -------------------------------------------------------------

  @doc """
  A string: a length including the terminator, the bytes, the NUL. A
  length of zero or one past the stream is refused; bytes that are not
  UTF-8 are refused rather than reinterpreted.
  """
  def read_string_alloc(%__MODULE__{} = r) do
    guarded(r, "", fn r ->
      at = r.at
      {length, r} = read_u32(r)

      if length == 0 or length > reader_remaining(r) + 1 do
        raise_impossible(r, at, length)
      end

      {text, r} = take(r, length - 1)

      if not String.valid?(text) do
        raise WireError,
          message: "at #{at}: the string is not UTF-8",
          at: at,
          kind: :not_utf8
      end

      {_, r} = read_u8(r)
      {text, r}
    end)
  end

  @doc """
  A `wstring`, in the form `policy` and `order` name — never inferred
  from the stream, because the forms are not separable there. A count
  the unit width does not divide, a frame that cannot hold its own
  terminator and a lone surrogate are each refused by name.
  """
  def read_wstring_utf8(%__MODULE__{} = r, policy, order) do
    guarded(r, "", fn r ->
      at = r.at
      {declared, r} = read_u32(r)

      if not ZeroddsWire.wstring_unit_order?(order) do
        raise_out_of_range(at, "a wstring unit order, not #{inspect(order)}")
      end

      cell = ZeroddsWire.wstring_cell(policy)

      if cell == nil do
        raise_out_of_range(at, "a wstring length policy, not #{inspect(policy)}")
      end

      {counts_characters, terminated, width} = cell

      count =
        if counts_characters do
          declared
        else
          if rem(declared, width) != 0 do
            raise_impossible(r, at, declared)
          end

          div(declared, width)
        end

      {units, r} = wstring_units(r, at, count, width, order)

      units =
        if terminated do
          # The count includes the NUL, so a count of zero is a frame
          # that does not hold its own terminator; and a NUL inside the
          # text is the narrow string's problem in wide form.
          if declared < 1 do
            raise_impossible(r, at, declared)
          end

          {body, [last]} = Enum.split(units, -1)

          if last != 0 do
            raise_out_of_range(
              at,
              "a NUL character ending the wstring, as its count includes one"
            )
          end

          if 0 in body do
            raise_out_of_range(at, "no NUL inside a wstring a C peer would stop at")
          end

          body
        else
          units
        end

      units = if order == :"byte-order-mark", do: strip_byte_order_mark(units), else: units
      {units_to_string(at, units), r}
    end)
  end

  @doc """
  The inverse of `Writer.write_wstring_utf8_octets/3`: the declared
  length is the UTF-8 octet count, the terminator `policy` asks for is
  the last of those octets, and a count the stream cannot hold, a missing
  or an interior NUL, or a binary that is not UTF-8 is refused.
  """
  def read_wstring_utf8_octets(%__MODULE__{} = r, policy) do
    guarded(r, "", fn r ->
      at = r.at
      {declared, r} = read_u32(r)

      if declared > reader_remaining(r) do
        raise_impossible(r, at, declared)
      end

      cell = ZeroddsWire.wstring_cell(policy)

      if cell == nil do
        raise_out_of_range(at, "a wstring length policy, not #{inspect(policy)}")
      end

      {_counts_characters, terminated, _width} = cell
      {raw, r} = take(r, declared)

      body =
        if terminated do
          if declared == 0 or binary_part(raw, declared - 1, 1) != <<0>> do
            raise_out_of_range(
              at,
              "a NUL octet ending the wstring, as its count includes one"
            )
          end

          kept = binary_part(raw, 0, declared - 1)

          if :binary.match(kept, <<0>>) != :nomatch do
            raise_out_of_range(at, "no NUL inside a wstring a C peer would stop at")
          end

          kept
        else
          raw
        end

      if not String.valid?(body) do
        raise WireError,
          message: "at #{at}: the wstring is not UTF-8",
          at: at,
          kind: :not_utf8
      end

      {body, r}
    end)
  end

  defp wstring_units(%__MODULE__{} = r, at, count, width, order) do
    if count * width > reader_remaining(r) do
      raise_impossible(r, at, count * width)
    end

    little = order in [:"byte-order-mark", :"host-unswapped"]

    Enum.map_reduce(1..count//1, r, fn _, r ->
      {unit, r} =
        cond do
          order == :"big-endian-always" -> fixed_order_unit(r, width, :be)
          little -> fixed_order_unit(r, width, :le)
          width == 4 -> read_u32(r)
          true -> read_u16(r)
        end

      if width == 4 and unit > 0xFFFF do
        raise_out_of_range(at, "a four-byte wstring character holding one UTF-16 unit")
      end

      {unit, r}
    end)
  end

  defp fixed_order_unit(%__MODULE__{} = r, width, order) do
    r = align_to(r, width)
    {raw, r} = take(r, width)

    value =
      case order do
        :le -> unsigned_of(raw, :little)
        :be -> unsigned_of(raw, :big)
      end

    {value, r}
  end

  # Take the byte-order mark off the front and apply what it said; the
  # units arrived little-endian, which is what this order reads.
  # `runtime/python`'s `_strip_byte_order_mark` carries the argument.
  defp strip_byte_order_mark([]), do: []
  defp strip_byte_order_mark([0xFEFF | rest]), do: rest

  defp strip_byte_order_mark([0xFFFE | rest]),
    do: Enum.map(rest, fn unit -> (unit &&& 0xFF) <<< 8 ||| unit >>> 8 end)

  defp strip_byte_order_mark(units),
    do: Enum.map(units, fn unit -> (unit &&& 0xFF) <<< 8 ||| unit >>> 8 end)

  defp units_to_string(at, units) do
    units
    |> combine_surrogates(at, [])
    |> List.to_string()
  end

  defp combine_surrogates([], _at, out), do: Enum.reverse(out)

  defp combine_surrogates([high, low | rest], at, out)
       when high >= 0xD800 and high <= 0xDBFF and low >= 0xDC00 and low <= 0xDFFF do
    point = 0x10000 + ((high - 0xD800) <<< 10) + (low - 0xDC00)
    combine_surrogates(rest, at, [point | out])
  end

  defp combine_surrogates([unit | _], at, _out) when unit >= 0xD800 and unit <= 0xDFFF do
    raise_out_of_range(at, "a UTF-16 sequence with no lone surrogate")
  end

  defp combine_surrogates([unit | rest], at, out), do: combine_surrogates(rest, at, [unit | out])

  # -- fixed ---------------------------------------------------------------

  @doc """
  `fixed<digits, scale>`, as the decimal string the writer took — the
  BCD field back into digits, leading zeros stripped, a negative zero
  losing its sign, exactly as the sibling runtimes answer. Under
  `:"value-precision"` the field's end is read out of the octets: it
  ends at the first octet whose low nibble is a sign.
  """
  def read_fixed(%__MODULE__{} = r, digits, scale, count) do
    guarded(r, zero_decimal(scale), fn r ->
      if count not in [:declared, :"value-precision", :"value-string-digits"] do
        raise_out_of_range(r.at, "a fixed digit count, not `#{inspect(count)}`")
      end

      # OMG IDL 4.2 §7.4.1.4.4.3.4 rule (42) caps the BCD rule at 31
      # digits, and a reader asked for a wider field would take a count
      # of octets the rule does not define — the same refusal the writer
      # makes, on the side where the octets are already there.
      if digits <= 0 or digits > 31 or scale > digits do
        raise_out_of_range(r.at, "a fixed of at most 31 digits with a scale within them")
      end

      {nibbles, sign, r} =
        if count in [:"value-precision", :"value-string-digits"] do
          value_precision_nibbles(r, [])
        else
          {raw, r} = take(r, div(digits + 2, 2))
          nibbles = for <<nibble::4 <- raw>>, do: nibble
          {body, [sign]} = Enum.split(nibbles, -1)
          body = if rem(digits + 1, 2) == 1, do: tl(body), else: body
          {body, sign, r}
        end

      if Enum.any?(nibbles, &(&1 > 9)) do
        raise_out_of_range(r.at, "BCD digits")
      end

      text =
        nibbles
        |> Enum.map(&(&1 + ?0))
        |> List.to_string()
        |> String.pad_leading(scale + 1, "0")

      split = String.length(text) - scale
      whole_part = String.slice(text, 0, split)
      fraction = String.slice(text, split, scale)
      whole_part = String.trim_leading(whole_part, "0")
      whole_part = if whole_part == "", do: "0", else: whole_part
      out = if scale == 0, do: whole_part, else: "#{whole_part}.#{fraction}"
      zero = whole_part == "0" and String.trim(fraction, "0") == ""
      {if(sign == 0x0D and not zero, do: "-#{out}", else: out), r}
    end)
  end

  defp value_precision_nibbles(%__MODULE__{} = r, acc) do
    if length(acc) >= 32 do
      raise_out_of_range(r.at, "a sign nibble within sixteen octets")
    end

    {<<byte>>, r} = take(r, 1)
    high = byte >>> 4
    low = byte &&& 0x0F

    if low >= 0x0A do
      {Enum.reverse([high | acc]), low, r}
    else
      value_precision_nibbles(r, [low, high | acc])
    end
  end

  defp zero_decimal(0), do: "0"
  defp zero_decimal(scale), do: "0." <> String.duplicate("0", scale)

  # -- the trims a bound reaches for ---------------------------------------

  @doc """
  `text` cut to at most `octets` bytes, never inside a character —
  §7.2.2.7 Table 11's TRIM, a prefix and never a reordering.
  """
  def trim_utf8(%__MODULE__{}, text, octets), do: trim(text, octets, false)

  @doc "`text` cut to at most `units` UTF-16 code units."
  def trim_utf16(%__MODULE__{}, text, units), do: trim(text, units, true)

  defp trim(text, limit, wide) do
    {out, _} =
      text
      |> String.to_charlist()
      |> Enum.reduce({[], 0}, fn point, {kept, used} ->
        cost =
          cond do
            wide and point > 0xFFFF -> 2
            wide -> 1
            true -> byte_size(<<point::utf8>>)
          end

        if used + cost > limit do
          {kept, limit + 1}
        else
          {[point | kept], used + cost}
        end
      end)

    out |> Enum.reverse() |> List.to_string()
  end

  # -- the bit-bounded kinds -----------------------------------------------

  @doc """
  An enumerator's holder, as the signed integer §7.4.3.5.3 makes it —
  sign-extended at the *holder's* width, which is what lets the decoder
  match `@value(-1)`'s pattern against its declared arm.
  """
  def read_enum(%__MODULE__{} = r, bit_bound) do
    guarded(r, 0, fn r ->
      if bit_bound == 0 or bit_bound > 32 do
        raise_out_of_range(r.at, "an @bit_bound between 1 and 32, not #{bit_bound}")
      end

      {width, {pattern, r}} =
        cond do
          bit_bound <= 8 -> {8, read_u8(r)}
          bit_bound <= 16 -> {16, read_u16(r)}
          true -> {32, read_u32(r)}
        end

      sign = 1 <<< (width - 1)
      {if(pattern < sign, do: pattern, else: pattern - (1 <<< width)), r}
    end)
  end

  @doc "A bitmask's holder. Table 33 of §7.4.1.1.3.2."
  def read_bitmask(%__MODULE__{} = r, bit_bound) do
    guarded(r, 0, fn r ->
      cond do
        bit_bound == 0 or bit_bound > 64 ->
          raise_out_of_range(r.at, "an @bit_bound between 1 and 64, not #{bit_bound}")

        bit_bound <= 8 ->
          read_u8(r)

        bit_bound <= 16 ->
          read_u16(r)

        bit_bound <= 32 ->
          read_u32(r)

        true ->
          read_u64(r)
      end
    end)
  end

  @doc "A bitset's anchor integer, of the width the planner sized."
  def read_bitset(%__MODULE__{} = r, anchor) do
    guarded(r, 0, fn r ->
      case anchor do
        1 -> read_u8(r)
        2 -> read_u16(r)
        4 -> read_u32(r)
        8 -> read_u64(r)
        _ -> raise_out_of_range(r.at, "a bitset anchor of 1, 2, 4 or 8 bytes, not #{anchor}")
      end
    end)
  end

  # -- framing -------------------------------------------------------------

  @doc "A DHEADER: the length the members that follow occupy."
  def read_dheader(%__MODULE__{} = r) do
    guarded(r, 0, fn r ->
      at = r.at
      {length, r} = read_u32(r)

      if length > reader_remaining(r) do
        raise_impossible(r, at, length)
      end

      {length, r}
    end)
  end

  @doc """
  An EMHEADER, unpacked, with the extent a skipping reader advances by
  already computed — §7.4.3.4.2. Codes 5 to 7 report the NEXTINT they
  peeked and the cursor is put back before it: that word is the
  member's own leading length.
  """
  def read_emheader(%__MODULE__{} = r) do
    default = %{member_id: 0, length_code: 0, must_understand: false, next_int: nil, extent: 1}

    guarded(r, default, fn r ->
      {word, r} = read_u32(r)
      length_code = word >>> 28 &&& 0x7
      member_id = word &&& 0x0FFF_FFFF
      must_understand = word >>> 31 != 0

      {next_int, r} =
        cond do
          length_code == 4 ->
            read_u32(r)

          length_code >= 5 ->
            at = r.at
            {peeked, r} = read_u32(r)
            {peeked, %{r | at: at}}

          true ->
            {nil, r}
        end

      extent =
        cond do
          length_code <= 3 -> 1 <<< length_code
          length_code == 4 -> next_int
          true -> 4 + next_int * %{5 => 1, 6 => 4, 7 => 8}[length_code]
        end

      {%{
         member_id: member_id,
         length_code: length_code,
         must_understand: must_understand,
         next_int: next_int,
         extent: extent
       }, r}
    end)
  end

  @doc "A PL_CDR1 parameter header in the member role; opens its value block."
  def read_pid(%__MODULE__{} = r), do: read_pid_as(r, :member)

  @doc """
  A PL_CDR1 parameter header, in whichever form it was written, and
  the value block it opens: §7.4.1.2 counts a member's internal
  alignment from the first octet after the header, so the origin moves
  here and `finish_pid_read/2` puts it back.
  """
  def read_pid_as(%__MODULE__{} = r, role) do
    stopped = %{
      pid: 0,
      length: 0,
      list_end: true,
      must_understand: false,
      impl_extension: false,
      reserved: false,
      ignore: false,
      role: role,
      saved_origin: r.origin,
      value_at: r.at
    }

    guarded(r, stopped, fn r ->
      r = align_to(r, 4)
      {pid, r} = read_u16(r)
      {declared, r} = read_u16(r)

      if (pid &&& @pid_id_mask) == (ZeroddsWire.pid_extended() &&& @pid_id_mask) do
        # The four flag bits are not part of the id — §7.4.1.2.1 splits
        # the `eMemberHeader` into flags and a 28-bit member id.
        {member_header, r} = read_u32(r)
        {wide_length, r} = read_u32(r)
        saved = r.origin
        r = %{r | origin: r.at}

        {%{
           pid: member_header &&& ZeroddsWire.e_member_id_mask(),
           length: wide_length,
           list_end: false,
           must_understand: (member_header &&& ZeroddsWire.e_flag_must_understand()) != 0,
           impl_extension: (member_header &&& ZeroddsWire.e_flag_impl_extension()) != 0,
           reserved: false,
           ignore: false,
           role: role,
           saved_origin: saved,
           value_at: r.at
         }, r}
      else
        identifier = pid &&& @pid_id_mask
        list_end = identifier == (ZeroddsWire.pid_list_end() &&& @pid_id_mask)

        # Under a framing that puts FLAG_IMPL_EXTENSION on every member
        # id the bit carries no information and is reported absent.
        flagged =
          (pid &&& ZeroddsWire.pid_impl_extension()) != 0 and
            (ZeroddsWire.pid_flags_at(r.framing.pid_flags, :member) &&&
               ZeroddsWire.pid_impl_extension()) == 0

        saved = r.origin
        r = %{r | origin: r.at}

        {%{
           pid: identifier,
           length: declared,
           list_end: list_end,
           must_understand: (pid &&& ZeroddsWire.pid_must_understand()) != 0,
           impl_extension: flagged,
           reserved: not list_end and identifier > r.framing.highest_short_id,
           ignore: identifier == ZeroddsWire.pid_ignore(),
           role: role,
           saved_origin: saved,
           value_at: r.at
         }, r}
      end
    end)
  end

  @doc """
  End a parameter's value and step to the next header.

  The origin goes back first; the value's used length is measured
  against its own start and the declared length is *checked* against
  the framing rather than believed — both readings are accepted where
  they coincide, which is every value already a multiple of four. The
  padding to four is stepped only under a framing that writes any:
  under Fast DDS's the next header aligns itself and a final parameter
  ends the stream where its value does. Nothing follows `PID_LIST_END`.
  """
  def finish_pid_read(%__MODULE__{} = r, header) do
    r = %{r | origin: header.saved_origin}

    if r.status != :ok or header.list_end do
      r
    else
      used = r.at - header.value_at
      padded = used + 3 &&& bnot(3)
      pads = PlCdr1Framing.pads_declared_length(r.framing, header.role)
      expected = if pads, do: padded, else: used

      r =
        if header.length != expected and header.length != used and header.length != padded do
          reader_refuse(r, :out_of_range)
        else
          r
        end

      if r.framing.pad_after_value, do: align_to(r, 4), else: r
    end
  end
end
