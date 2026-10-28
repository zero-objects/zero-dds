# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
defmodule ZeroddsWire.WireError do
  @moduledoc """
  What can go wrong on the wire.

  One exception and a `kind` field rather than a class per variant: the
  callers this runtime has — the generated probe's harness — either
  catch everything wire-shaped and exit 1, or read the kind to tell a
  truncated stream from an impossible length. Every message names what
  was expected and where.
  """
  defexception [:message, :at, :kind]
end

defmodule ZeroddsWire.PlCdr1Framing do
  @moduledoc """
  How a PL_CDR1 parameter list is framed — the measured axes of
  `zerodds_ast_layout::PlCdr1HeaderPolicy` and its neighbours, with the
  clause's own answers as defaults. `runtime/python`'s `PlCdr1Framing`
  carries the measurements at length; the field names here are its.

  The reader takes the whole struct too, which is where this runtime
  differs from the Python one and follows the C# `Reader`: a parameter's
  declared length is *checked* against the framing rather than believed,
  measured from the value's own start, and the trailing padding is
  stepped only under a framing that writes any — `pad_after_value` is
  false under Fast DDS's, and a reader that aligned regardless would
  walk past the last parameter of a stream that ends where its value
  does.
  """
  defstruct pid_flags: :"sentinel-must-understand",
            highest_short_id: 0x3F00,
            pad_declared_length: false,
            pad_declared_optional_length: false,
            member_must_understand: true,
            pad_after_value: true

  @doc "Whether a parameter in `role` declares a padded length."
  def pads_declared_length(%__MODULE__{} = framing, role) do
    case role do
      :optional -> framing.pad_declared_optional_length
      _ -> framing.pad_declared_length
    end
  end
end

defmodule ZeroddsWire do
  @moduledoc """
  The free functions of the wire runtime: what the shared codec walk
  reaches without a receiver, and the tables the writer and the reader
  both read so the two cannot each accept a slightly different set.

  The policy names are the exact spellings of
  `crates/ast-layout/src/key.rs`'s `as_str` — as atoms, because that is
  what an Elixir function head matches on.
  """

  import Bitwise

  alias ZeroddsWire.WireError

  # The parameter id constants of XTypes 1.3 §7.4.1.2.2 and
  # DDSI-RTPS 2.5 §9.4.2.11.2, shared by the writer and the reader.
  @pid_extended 0x3F01
  @pid_list_end 0x3F02
  @pid_ignore 0x3F03
  @pid_impl_extension 0x8000
  @pid_must_understand 0x4000
  @e_flag_must_understand 0x4000_0000
  @e_flag_impl_extension 0x8000_0000
  @e_member_id_mask 0x0FFF_FFFF

  def pid_extended, do: @pid_extended
  def pid_list_end, do: @pid_list_end
  def pid_ignore, do: @pid_ignore
  def pid_impl_extension, do: @pid_impl_extension
  def pid_must_understand, do: @pid_must_understand
  def e_flag_must_understand, do: @e_flag_must_understand
  def e_flag_impl_extension, do: @e_flag_impl_extension
  def e_member_id_mask, do: @e_member_id_mask

  @doc """
  The DHEADER a value that takes no delimiter does not write.

  §7.4.3.5.3 gives the delimiter to `@appendable` and `@mutable` and not
  to `@final`; the shared walk declares one handle per value and
  finishes it either way, so the absence is a value — a position below
  zero, which `Writer.finish_dheader` ignores.
  """
  def no_dheader, do: -1

  @doc "How many octets `text` is — what a `string<N>` bound counts."
  def utf8_octets(text), do: byte_size(text)

  @doc """
  How many UTF-16 code units `text` is — what a `wstring<N>` bound
  counts. A scalar above the basic plane is two.
  """
  def utf16_units(text) do
    text
    |> String.to_charlist()
    |> Enum.reduce(0, fn point, count -> count + if point > 0xFFFF, do: 2, else: 1 end)
  end

  @doc """
  Whether a PL_CDR1 parameter can be a member of the reading type.

  False for a vendor-private parameter, a reserved id and `PID_IGNORE`;
  each is legal for a peer to send and none is a member, so all take the
  unknown-parameter path. §7.4.1.2.2, §7.2.2.4.4.4.6.
  """
  def pid_is_user_member(header) do
    not header.impl_extension and not header.reserved and not header.ignore
  end

  @doc """
  One half of a `map`'s entry.

  The codec's entries are two-element lists — the read side fills them
  half at a time and a tuple cannot be written into — but a hand-built
  sample may carry tuples, and reading one is not worth crashing over.
  """
  def entry_half({key, _value}, 0), do: key
  def entry_half({_key, value}, 1), do: value
  def entry_half([key, _value], 0), do: key
  def entry_half([_key, value], 1), do: value

  @doc """
  An IDL array's rows as nested lists, `depth` dimensions deep.

  The materialiser declares an array `{T, …}` and a generated reader
  starts from that tuple, but writes each element with
  `put_in(v, [Access.at(i)], x)`, and `Access.at/1` refuses a tuple.
  `array_to_tuples/2` turns the rows back once every element is in.
  """
  def array_to_lists(rows, 1) when is_tuple(rows), do: Tuple.to_list(rows)

  def array_to_lists(rows, depth) when is_tuple(rows) and depth > 1 do
    rows |> Tuple.to_list() |> Enum.map(&array_to_lists(&1, depth - 1))
  end

  @doc """
  An IDL array's rows as the nested tuples the materialiser declares,
  `depth` dimensions deep — the inverse of `array_to_lists/2`.
  """
  def array_to_tuples(rows, 1) when is_list(rows), do: List.to_tuple(rows)

  def array_to_tuples(rows, depth) when is_list(rows) and depth > 1 do
    rows |> Enum.map(&array_to_tuples(&1, depth - 1)) |> List.to_tuple()
  end

  @doc """
  What flags a PL_CDR1 parameter id carries at `position`.

  The measured table `runtime/python`'s `pid_flags_at` carries in full:
  the sentinel's answer cannot be read off the member's, and the
  extended marker's off neither. An unknown policy raises, because a
  typo in a wire-affecting setting must be loud.
  """
  def pid_flags_at(:"sentinel-must-understand", position)
      when position in [:sentinel, :extended_marker],
      do: @pid_must_understand

  def pid_flags_at(:"sentinel-must-understand", _position), do: 0
  def pid_flags_at(:bare, _position), do: 0

  def pid_flags_at(:"impl-extension-members", position)
      when position in [:member, :extended_marker],
      do: @pid_impl_extension ||| @pid_must_understand

  def pid_flags_at(:"impl-extension-members", _position), do: 0

  def pid_flags_at(policy, _position) do
    raise WireError, message: "`#{inspect(policy)}` is not a PID-flag policy", at: 0
  end

  @doc """
  The three coordinates of a `wstring` length policy: whether the count
  is characters, whether a terminator goes out, and the code-unit
  width. One table for both directions, the named cells and the
  coordinate spellings alike; `nil` for a name that is not a policy.
  """
  def wstring_cell(policy) do
    case policy do
      :specification -> {false, false, 2}
      :"character-count" -> {true, false, 2}
      :"character-count-terminated" -> {true, true, 4}
      :"specification-wide-units" -> {false, false, 4}
      :"ros-type-support" -> {true, false, 4}
      :"octets/terminated/2" -> {false, true, 2}
      :"octets/terminated/4" -> {false, true, 4}
      :"characters/terminated/2" -> {true, true, 2}
      :"octets/unterminated/2" -> {false, false, 2}
      :"octets/unterminated/4" -> {false, false, 4}
      :"characters/unterminated/2" -> {true, false, 2}
      :"characters/unterminated/4" -> {true, false, 4}
      :"characters/terminated/4" -> {true, true, 4}
      _ -> nil
    end
  end

  @doc "Whether a `wstring` unit order is one this runtime knows."
  def wstring_unit_order?(order) do
    order in [:stream, :"big-endian-always", :"byte-order-mark", :"host-unswapped"]
  end

  @doc """
  The UTF-16 code units of `text`, without a BOM.

  §7.4.1.1.2 counts a `String<Char16>` in code units, so a code point
  outside the basic plane is its surrogate pair. An Elixir binary
  cannot hold a lone surrogate at all, so the refusal the Python
  runtime writes here has nothing to fire on.
  """
  def utf16_unit_list(text) do
    text
    |> String.to_charlist()
    |> Enum.flat_map(fn point ->
      if point < 0x10000 do
        [point]
      else
        offset = point - 0x10000
        [0xD800 + (offset >>> 10), 0xDC00 + (offset &&& 0x3FF)]
      end
    end)
  end

  # -- the union-overlay bit patterns --------------------------------------

  @doc """
  The low `octets` of `value`'s two's-complement bits, zero-extended —
  `Writer.write_union_overlay`'s pattern for an integer, character,
  enumeration or bitmask arm. `-2` in four octets is `0xFFFFFFFE`.
  """
  def bit_pattern_int(value, octets) when octets in [1, 2, 4, 8] do
    value &&& (1 <<< (octets * 8)) - 1
  end

  @doc "One octet, 0 or 1 — the boolean overlay pattern."
  def bit_pattern_bool(true), do: 1
  def bit_pattern_bool(false), do: 0

  @doc "The IEEE 754 binary32 bits, zero-extended."
  def bit_pattern_f32(value), do: ZeroddsWire.Float.f32_bits(value)

  @doc "The IEEE 754 binary64 bits."
  def bit_pattern_f64(value), do: ZeroddsWire.Float.f64_bits(value)
end

defmodule ZeroddsWire.Float do
  @moduledoc """
  IEEE 754 bit patterns for the two float widths Erlang can hold — and
  the three values it cannot.

  An Erlang float is a finite binary64: there is no NaN and no infinity
  in the term ordering at all, and a binary match `<<f::float>>` refuses
  their bit patterns. This binding therefore holds them as the atoms
  `:nan`, `:infinity` and `:neg_infinity`, converts at the octet in both
  directions, and canonicalises a NaN to the quiet pattern — the same
  one Rust's `f32::NAN` and `f64::NAN` carry. A NaN payload is not
  preserved; a corpus case that carries one is a measured limitation of
  this column, not a silent substitution, because the dumpers refuse a
  NaN outright (it has no JSON form).
  """

  @doc "The binary32 pattern of a value, rounding once, here."
  def f32_bits(:nan), do: 0x7FC00000
  def f32_bits(:infinity), do: 0x7F800000
  def f32_bits(:neg_infinity), do: 0xFF800000

  def f32_bits(value) when is_number(value) do
    <<bits::unsigned-little-32>> = <<value * 1.0::float-little-32>>
    bits
  rescue
    # A magnitude no binary32 holds becomes the infinity a C `(float)`
    # cast produces — the wire-correct answer, and what every peer
    # reads back.
    ArgumentError ->
      if value < 0, do: 0xFF800000, else: 0x7F800000
  end

  @doc "The binary64 pattern of a value."
  def f64_bits(:nan), do: 0x7FF8000000000000
  def f64_bits(:infinity), do: 0x7FF0000000000000
  def f64_bits(:neg_infinity), do: 0xFFF0000000000000

  def f64_bits(value) when is_number(value) do
    <<bits::unsigned-little-64>> = <<value * 1.0::float-little-64>>
    bits
  end

  @doc "The value of a binary32 pattern, as a float or one of the atoms."
  def from_f32_bits(bits) do
    import Bitwise

    if (bits >>> 23 &&& 0xFF) == 0xFF do
      cond do
        (bits &&& 0x7FFFFF) != 0 -> :nan
        (bits &&& 0x80000000) != 0 -> :neg_infinity
        true -> :infinity
      end
    else
      <<value::float-little-32>> = <<bits::unsigned-little-32>>
      value
    end
  end

  @doc "The value of a binary64 pattern, as a float or one of the atoms."
  def from_f64_bits(bits) do
    import Bitwise

    if (bits >>> 52 &&& 0x7FF) == 0x7FF do
      cond do
        (bits &&& 0xFFFFFFFFFFFFF) != 0 -> :nan
        (bits &&& 0x8000000000000000) != 0 -> :neg_infinity
        true -> :infinity
      end
    else
      <<value::float-little-64>> = <<bits::unsigned-little-64>>
      value
    end
  end
end
