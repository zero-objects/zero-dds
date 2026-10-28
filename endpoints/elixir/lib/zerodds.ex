# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Native pure-Elixir endpoint SDK (ADR 0013): a from-scratch XCDR wire-core on
# the BEAM, byte-identical to the Rust core and the other SDKs. Sync (receive
# with a timeout) and async (a process/mailbox — the idiomatic OTP model).
#
# No native addon, no NIF: Elixir binaries + bitstring syntax carry the wire.

defmodule ZeroDDS.Wire do
  @moduledoc """
  XCDR writer/reader.

  The encoding is a constructor argument with no default. XTypes 1.3 §7.4.1
  puts MAXALIGN at eight under XCDR1 and four under XCDR2, so a writer that
  did not know which one it was in would be writing XCDR2 and calling it XCDR
  — which is what this SDK did until the cap became a field. `align` read a
  constant four, and every `long long` in an XCDR1 sample landed four octets
  early with nothing in the interface to say so.

  Elixir's integers are arbitrary precision and `<<v::16>>` truncates a value
  that does not fit without a word, so every integer write here carries a
  guard: `put_u16(w, 70000)` raises rather than answering `7011`. The bound is
  the union of the signed and the unsigned range, because a holder of that
  width carries either and the pattern is the same.
  """

  # XTypes 1.3 §7.4.1: MAXALIGN is eight under XCDR1 and four under XCDR2.
  @max_align %{xcdr1: 8, xcdr2: 4}

  # The member header length codes of XTypes 1.3 §7.4.3.4.2 Table 34. The code
  # occupies bits 28..30 of the EMHEADER and decides how the member's byte
  # length is recovered: LC0..LC3 give it from the code alone, LC4..LC7 from a
  # word that follows or from the body's own leading word.
  def lc0, do: 0
  def lc1, do: 1
  def lc2, do: 2
  def lc3, do: 3
  def lc4, do: 4
  def lc5, do: 5
  def lc6, do: 6
  def lc7, do: 7

  # PID_LIST_END, and the PID that introduces the twelve-octet parameter
  # header. XTypes 1.3 §7.4.1.2 Table 31.
  @pid_list_end_id 0x3F02
  @pid_extended_id 0x3F01
  # The highest member id a short parameter header carries, inclusive — which
  # is what RTI and Fast DDS write for `@id(16128)`.
  @pid_short_max_id 0x3F00
  # Bit 14 of a short parameter id. Bit 15 is the implementation-extension
  # flag and is never set here.
  @pid_flag_must_understand 0x4000
  # Bit 30 of the eMemberHeader inside an extended parameter header — not bit
  # 31, which is the impl-extension flag.
  @em_flag_must_understand 0x4000_0000

  def pid_list_end_id, do: @pid_list_end_id
  def pid_extended_id, do: @pid_extended_id
  def pid_short_max_id, do: @pid_short_max_id

  # --- Writer: a %{buf, endian, max_align, origin} accumulator ---

  @doc """
  A writer for one byte order (`:little` | `:big`) and one encoding
  (`:xcdr1` | `:xcdr2`).

  `origin` is the position alignment is measured from. Zero for the stream,
  and moved to the start of a PL_CDR1 parameter's value for the length of that
  parameter — §7.4.1.2 aligns a parameter's value against its own block, not
  against the stream. Without it a `double` in a parameter takes four octets
  of padding and the header declares twelve where Cyclone, RTI and Fast DDS
  all declare eight.
  """
  def writer(endian, encoding) when encoding in [:xcdr1, :xcdr2] do
    %{buf: <<>>, endian: endian, max_align: @max_align[encoding], origin: 0}
  end

  def writer(_endian, encoding) do
    raise ArgumentError, "the encoding is :xcdr1 or :xcdr2, not #{inspect(encoding)}"
  end

  defp align(w, a) do
    cap = min(a, w.max_align)
    pad = rem(cap - rem(byte_size(w.buf) - w.origin, cap), cap)
    %{w | buf: w.buf <> :binary.copy(<<0>>, pad)}
  end

  # le: the value already encoded little-endian; reversed for :big.
  defp put(w, a, le) do
    w = align(w, a)
    bytes = if w.endian == :big, do: rev(le), else: le
    %{w | buf: w.buf <> bytes}
  end

  defp rev(bin), do: bin |> :binary.bin_to_list() |> Enum.reverse() |> :binary.list_to_bin()

  # The bound is the union of the signed and the unsigned range: both
  # spellings of an integer argument reach the wire through the same call, and
  # the pattern is the same either way. Refused and not truncated — `<<300::8>>`
  # is `<<44>>` and `<<70000::little-16>>` is `<<0x70, 0x11>>`, each a frame of
  # the right length carrying a different number.
  defp fits?(v, bits) when is_integer(v) do
    v <= Bitwise.bsl(1, bits) - 1 and v >= -Bitwise.bsl(1, bits - 1)
  end

  defp fits?(_v, _bits), do: false

  defp refuse(v, bits) do
    raise ArgumentError,
          "#{inspect(v)} does not fit #{bits} bits; both the unsigned pattern and " <>
            "the signed value are taken and nothing outside their union is"
  end

  def put_u8(w, v) do
    if fits?(v, 8), do: %{w | buf: w.buf <> <<v::8>>}, else: refuse(v, 8)
  end

  def put_u16(w, v) do
    if fits?(v, 16), do: put(w, 2, <<v::little-16>>), else: refuse(v, 16)
  end

  def put_u32(w, v) do
    if fits?(v, 32), do: put(w, 4, <<v::little-32>>), else: refuse(v, 32)
  end

  @doc """
  A 64-bit integer at its natural eight-octet alignment, which `max_align`
  then caps: eight under XCDR1 and four under XCDR2. XTypes 1.3 §7.4.1. It
  passed 4 here regardless of the encoding, so every `long long` in an XCDR1
  sample landed four octets early.

  Elixir's integers have no upper bound, so nothing here refuses on its own:
  `<<:math.pow(2, 64) |> trunc()::little-64>>` is eight zero octets. The guard
  is the only place that difference is visible.
  """
  def put_u64(w, v) do
    if fits?(v, 64), do: put(w, 8, <<v::little-64>>), else: refuse(v, 64)
  end

  @doc """
  Binary32 as its IEEE-754 bit pattern.

  The BEAM has no 32-bit float, so `<<v::float-little-32>>` is where the
  narrowing happens — and it *raises* for a value outside the binary32 range
  rather than producing an infinity, which is why `1.0e39` has no form here.
  """
  def put_f32(w, v), do: put(w, 4, <<v::float-little-32>>)

  @doc """
  Binary64 as its IEEE-754 bit pattern, at the same alignment as `put_u64/2`.
  `double` is one of the fifteen primitives of XTypes 1.3 §7.2.2.2 and had no
  writer here at all. The BEAM's float is already a binary64, so nothing
  narrows on the way; it has no infinity and no NaN, so those two of the
  format's values have no Elixir literal at all.
  """
  def put_f64(w, v), do: put(w, 8, <<v::float-little-64>>)

  @doc "A `boolean` as the one octet §7.4.1.1.1 Table 30 gives it: 0 or 1."
  def put_bool(w, v) when is_boolean(v), do: put_u8(w, if(v, do: 1, else: 0))

  def put_bytes(w, b), do: %{w | buf: w.buf <> b}

  def put_string(w, s) do
    w = put_u32(w, byte_size(s) + 1)
    w = put_bytes(w, s)
    put_u8(w, 0)
  end

  def put_seq_u8(w, b) do
    w = put_u32(w, byte_size(b))
    put_bytes(w, b)
  end

  @doc """
  An IDL `char`: the one octet XTypes 1.3 §7.4.1.1.1 Table 30 gives it.

  A code point above U+00FF is refused and not truncated. `š` is U+0161 and
  its low octet is 0x61, which is `a`: a truncating writer produces a frame of
  the right length holding a different, plausible character and nothing
  downstream can tell.
  """
  def put_char(w, code_point) when is_integer(code_point) and code_point in 0..0xFF do
    put_u8(w, code_point)
  end

  def put_char(_w, code_point) do
    raise ArgumentError, "expected a character below U+0100, found #{inspect(code_point)}"
  end

  @doc """
  An IDL `wchar`: one two-octet code unit, 2-aligned, in the stream's byte
  order.

  The four policy axes a wide character carries are answered here and not by
  the caller: the unit is two octets wide, it goes out in the stream's byte
  order, there is no GIOP length octet before it, and no byte-order mark. That
  is what Cyclone and Fast DDS write in both encodings and what RTI writes
  under XCDR2; RTI's XCDR1 `wchar` is four octets, which is the one reading
  this binding does not take.

  A code point above U+FFFF is refused. The refusal is width-independent — a
  four-octet form would zero-extend a code *unit*, not a code point.
  """
  def put_wchar(w, code_point) when is_integer(code_point) and code_point in 0..0xFFFF do
    put_u16(w, code_point)
  end

  def put_wchar(_w, code_point) do
    raise ArgumentError, "expected a BMP character, found #{inspect(code_point)}"
  end

  @doc """
  An IDL `wstring` from its UTF-16 code units, as a list of integers.

  The length word counts **octets** and not code units, there is no
  terminator, there is no byte-order mark, and each unit goes out two octets
  wide in the stream's own byte order. Those are the four axes and they are
  the binding's answer. Cyclone writes exactly this in both encodings and RTI
  under XCDR2; Fast DDS counts units, which is the reading this binding does
  not take.
  """
  def put_wstring(w, units) when is_list(units) do
    w = put_u32(w, length(units) * 2)
    Enum.reduce(units, w, fn unit, acc -> put_u16(acc, unit) end)
  end

  @doc """
  A `bitset` as its anchor word: the holder the declared bits pack into,
  written at the anchor's own width and therefore at the anchor's own
  alignment. XTypes 1.3 §7.3.1.4 and §7.4.1.

  `bits` is the packed holder and `anchor` its width in octets, both
  properties of the declared type. What this call decides is the pair the type
  does not: which write the holder goes out through, and what that write
  aligns to.
  """
  def put_bitset(w, bits, 1), do: put_u8(w, bits)
  def put_bitset(w, bits, 2), do: put_u16(w, bits)
  def put_bitset(w, bits, 4), do: put_u32(w, bits)
  def put_bitset(w, bits, 8), do: put_u64(w, bits)

  def put_bitset(_w, _bits, anchor) do
    raise ArgumentError,
          "a bitset anchor is one, two, four or eight octets, not #{inspect(anchor)}"
  end

  @doc """
  An IDL `fixed<digits,scale>` as packed BCD.
  CDR (OMG CORBA 3.3) §9.3.2.8 / IDL 4.2 §7.4.1.4.5.

  See `fixed_octets/3` for the rule. The value is a decimal string and not a
  float: `fixed<31,0>` has thirty-one significant digits and the BEAM's float
  holds about sixteen.
  """
  def put_fixed(w, value, digits, scale), do: put_bytes(w, fixed_octets(value, digits, scale))

  @doc """
  The packing `put_fixed/4` writes, apart so it can be tested without a writer.

  `value` is the decimal as the sample wrote it — an optional sign, an integer
  part and an optional fraction. Everything after that is this call's:

    * the integer part is left-padded and the fraction right-padded to exactly
      `digits` decimal digits, with the point implied `scale` from the right;
      the scale itself is not on the wire,
    * a leading zero nibble is prepended when `digits + 1` is odd, so digits
      and sign pair up,
    * two nibbles to the octet, most significant digit first, high nibble
      first,
    * the last nibble is the sign: 0xD negative, 0xC otherwise. A negative
      zero goes out positive, because there is one zero to write,
    * alignment is one, and no multi-octet integer is written, so the byte
      order never enters.

  The octet count is therefore `ceil((digits + 1) / 2)` whatever the value,
  which is the point omniORB 4.3.3 and JacORB 3.9 differ on and the one this
  binding takes from omniORB.
  """
  def fixed_octets(value, digits, scale) do
    if not (is_integer(digits) and digits >= 1 and digits <= 31) do
      raise ArgumentError, "fixed<#{inspect(digits)},#{inspect(scale)}>: a digit count is 1..31"
    end

    if not (is_integer(scale) and scale >= 0 and scale <= digits) do
      raise ArgumentError, "fixed<#{digits},#{inspect(scale)}>: a scale is 0..digits"
    end

    {negative, text} =
      case value do
        "-" <> rest -> {true, rest}
        "+" <> rest -> {false, rest}
        other -> {false, other}
      end

    {integral, fraction} =
      case String.split(text, ".", parts: 2) do
        [i, f] -> {i, f}
        [i] -> {i, ""}
      end

    if integral == "" and fraction == "" do
      raise ArgumentError, "fixed<#{digits},#{scale}>: `#{value}` has no digits"
    end

    if not (Enum.all?(String.to_charlist(integral <> fraction), &(&1 in ?0..?9))) do
      raise ArgumentError, "fixed<#{digits},#{scale}>: `#{value}` is not a decimal"
    end

    # Leading zeros carry no value, so they go before the width is checked:
    # `000000.001` is one significant integer digit, not six.
    integral = String.replace_leading(integral, "0", "")

    if String.length(integral) > digits - scale do
      raise ArgumentError,
            "fixed<#{digits},#{scale}>: `#{value}` needs #{String.length(integral)} " <>
              "integer digits and the type has #{digits - scale}"
    end

    # A trailing zero in the fraction is padding either way, so it goes before
    # the width is checked and is re-added below.
    fraction = trim_fraction(fraction, scale)

    if String.length(fraction) > scale do
      raise ArgumentError,
            "fixed<#{digits},#{scale}>: `#{value}` needs #{String.length(fraction)} " <>
              "fraction digits and the type has #{scale}"
    end

    lead = if rem(digits + 1, 2) == 1, do: [0], else: []

    nibbles =
      lead ++
        List.duplicate(0, digits - scale - String.length(integral)) ++
        Enum.map(String.to_charlist(integral), &(&1 - ?0)) ++
        Enum.map(String.to_charlist(fraction), &(&1 - ?0)) ++
        List.duplicate(0, scale - String.length(fraction))

    sign = if negative and Enum.any?(nibbles, &(&1 != 0)), do: 0x0D, else: 0x0C

    (nibbles ++ [sign])
    |> Enum.chunk_every(2)
    |> Enum.map(fn [hi, lo] -> Bitwise.bsl(hi, 4) |> Bitwise.bor(lo) end)
    |> :binary.list_to_bin()
  end

  defp trim_fraction(fraction, scale) do
    if String.length(fraction) > scale and String.ends_with?(fraction, "0") do
      trim_fraction(String.slice(fraction, 0..-2//1), scale)
    else
      fraction
    end
  end

  def bytes(w), do: w.buf

  # --- back-patched framing ------------------------------------------------
  #
  # A DHEADER, a NEXTINT and a PL_CDR1 parameter length are all the same
  # shape: a slot reserved before the body and filled in once the body's
  # length is known. `patch` is the one place that writes into an
  # already-written position, and it honours the wire byte order like every
  # other write. `at` is a 0-based offset, so the arithmetic reads the same as
  # the other fourteen bindings, and the buffer's length never moves.

  # Split with `binary_part/3` and not a bitstring pattern. A size spec
  # that reads an outer variable is the one construct Elixir changed
  # under us in both directions: `size(at)` warns from 1.19 on ("you must
  # precede it with ^"), and `size(^at)` is a compile error on 1.14, the
  # version Debian ships and the CI installs. `binary_part/3` has no size
  # spec, so it says the same thing to every version.
  defp patch(w, at, width, v) do
    head = binary_part(w.buf, 0, at)
    rest_at = at + width
    tail = binary_part(w.buf, rest_at, byte_size(w.buf) - rest_at)
    le = if width == 2, do: <<v::little-16>>, else: <<v::little-32>>
    field = if w.endian == :big, do: rev(le), else: le
    %{w | buf: head <> field <> tail}
  end

  @doc """
  Reserve a 4-aligned DHEADER slot. Returns `{writer, body_start}`, and
  `body_start` goes back to `dheader_end/2`. XTypes 1.3 §7.4.3.5.

  `@appendable` is the default extensibility of XTypes (§7.3.1.2.1.8), so a
  struct written with no annotation at all had no form here before this.
  """
  def dheader_begin(w) do
    w = put_u32(w, 0)
    {w, byte_size(w.buf)}
  end

  @doc """
  Back-patch the DHEADER with the byte length written since `body_start`. The
  DHEADER itself is not counted — §7.4.3.5 measures the content that follows.
  """
  def dheader_end(w, body_start) do
    patch(w, body_start - 4, 4, byte_size(w.buf) - body_start)
  end

  @doc """
  The EMHEADER for one member of a `@mutable` aggregate. §7.4.3.4.2:

      EMHEADER = (must_understand <<< 31) ||| (length_code <<< 28) ||| member_id

  with a 28-bit member id. For LC4 a NEXTINT slot is reserved after it and
  `emheader_end/3` fills it in; for LC0..LC3 the code alone gives the length
  and nothing follows; for LC5..LC7 the body's own leading four octets are the
  length, so no separate NEXTINT is written and writing one would put every
  following member at the wrong offset.

  Returns `{writer, body_start}`.
  """
  def emheader_begin(w, id, must_understand, length_code) do
    if not (is_integer(id) and id >= 0 and id <= 0x0FFF_FFFF) do
      raise ArgumentError, "member id #{inspect(id)} does not fit the 28 bits of an EMHEADER"
    end

    if not (is_integer(length_code) and length_code >= 0 and length_code <= 7) do
      raise ArgumentError, "length code #{inspect(length_code)} is not one of LC0..LC7"
    end

    header = Bitwise.bor(Bitwise.bsl(length_code, 28), id)
    header = if must_understand, do: Bitwise.bor(header, Bitwise.bsl(1, 31)), else: header
    w = put_u32(w, header)
    w = if length_code == 4, do: put_u32(w, 0), else: w
    {w, byte_size(w.buf)}
  end

  @doc """
  Close the member `emheader_begin/4` opened.

  The length code is taken back rather than remembered, so a caller that
  paired an LC4 begin with a differently-coded end gets a refusal instead of a
  NEXTINT patched over the first four octets of a member body.
  """
  def emheader_end(w, body_start, length_code) do
    written = byte_size(w.buf) - body_start

    cond do
      length_code == 4 ->
        patch(w, body_start - 4, 4, written)

      length_code <= 3 ->
        # 1, 2, 4, 8 — the code *is* the length, so a body of any other size
        # means the plan and the member disagree. Silence here would put a
        # length on the wire that no octet count backs.
        want = Bitwise.bsl(1, length_code)

        if written != want do
          raise ArgumentError, "LC#{length_code} frames #{want} octets and the member wrote #{written}"
        end

        w

      written < 4 ->
        raise ArgumentError,
              "LC#{length_code} takes its length from the body's first four octets " <>
                "and the member wrote #{written}"

      true ->
        w
    end
  end

  @doc """
  A PL_CDR1 short parameter header. XTypes 1.3 §7.4.1.2: at a four-octet
  boundary an `unsigned short` parameter id, then an `unsigned short` byte
  length of the value that follows. Both go out in the stream's byte order.

  The must-understand flag is bit 14 of the id field. Bit 15 is the
  implementation-extension flag and is not set here.

  The parameter's value aligns against the block and not against the stream,
  so the alignment origin moves for the length of the parameter and
  `pid_end/2` restores it. Returns `{writer, open_header}`.
  """
  def pid_begin(w, id, must_understand) do
    if not (is_integer(id) and id >= 0 and id <= @pid_short_max_id) do
      raise ArgumentError, "member id #{inspect(id)} needs the extended parameter header"
    end

    w = align(w, 4)
    field = if must_understand, do: Bitwise.bor(id, @pid_flag_must_understand), else: id
    w = put(w, 2, <<field::little-16>>)
    at = byte_size(w.buf)
    w = %{w | buf: w.buf <> <<0, 0>>}
    {%{w | origin: byte_size(w.buf)}, %{at: at, extended: false, saved_origin: w.origin}}
  end

  @doc """
  The twelve-octet parameter header §7.4.1.2 requires for a member id above
  0x3F00 or a value longer than 65535 octets:

      [PID_EXTENDED | must-understand : u16][8 : u16][eMemberHeader : u32][length : u32]

  The must-understand flag on the marker is not the member's — it is on the
  marker under this framing whatever the member says, and the member's own
  flag is bit 30 of the eMemberHeader. Reading it off the marker instead makes
  every parameter must-understand.
  """
  def pid_extended_begin(w, member_id, must_understand) do
    if not (is_integer(member_id) and member_id >= 0 and member_id <= 0x0FFF_FFFF) do
      raise ArgumentError,
            "member id #{inspect(member_id)} does not fit the 28 bits of an eMemberHeader"
    end

    w = align(w, 4)
    marker = Bitwise.bor(@pid_extended_id, @pid_flag_must_understand)
    w = put(w, 2, <<marker::little-16>>)
    w = put(w, 2, <<8::little-16>>)

    header =
      if must_understand,
        do: Bitwise.bor(member_id, @em_flag_must_understand),
        else: member_id

    w = put(w, 4, <<header::little-32>>)
    at = byte_size(w.buf)
    w = %{w | buf: w.buf <> <<0, 0, 0, 0>>}
    {%{w | origin: byte_size(w.buf)}, %{at: at, extended: true, saved_origin: w.origin}}
  end

  @doc """
  Close the parameter `pid_begin/3` or `pid_extended_begin/3` opened.

  The declared length counts the value's octets **and** the padding that
  brings the next header back to a four-octet boundary: a reader skips
  `length` from the end of the header and has to land on the next parameter.
  The padding is written after the length is patched, so the two cannot
  disagree.
  """
  def pid_end(w, header) do
    width = if header.extended, do: 4, else: 2
    length = byte_size(w.buf) - header.at - width
    # The parameter's own origin is still in force here, so the round-up is
    # measured from the block that the length describes.
    past = rem(byte_size(w.buf) - header.saved_origin, 4)
    length = if past != 0, do: length + (4 - past), else: length

    if not header.extended and length > 0xFFFF do
      raise ArgumentError,
            "a short parameter header describes at most 65535 octets, not #{length}"
    end

    w = patch(w, header.at, width, length)
    align(%{w | origin: header.saved_origin}, 4)
  end

  @doc """
  The sentinel that closes a PL_CDR1 parameter list: the id PID_LIST_END with
  a zero length, four-aligned. §7.4.1.2.

  The must-understand flag is set on the sentinel — 0x7F02 on the wire, which
  is what Cyclone and RTI write. Fast DDS writes the bare 0x3F02; the flag is
  not the sentinel's meaning either way, and this is the majority form.
  """
  def pid_list_end(w) do
    w = align(w, 4)
    id = Bitwise.bor(@pid_list_end_id, @pid_flag_must_understand)
    w = put(w, 2, <<id::little-16>>)
    put(w, 2, <<0::little-16>>)
  end

  # --- Reader: a %{bin, pos, endian, max_align, origin} cursor ---

  def reader(bin, endian, encoding) when encoding in [:xcdr1, :xcdr2] do
    %{bin: bin, pos: 0, endian: endian, max_align: @max_align[encoding], origin: 0}
  end

  def reader(_bin, _endian, encoding) do
    raise ArgumentError, "the encoding is :xcdr1 or :xcdr2, not #{inspect(encoding)}"
  end

  defp take(r, a, n) do
    cap = min(a, r.max_align)
    pad = rem(cap - rem(r.pos - r.origin, cap), cap)
    pos = r.pos + pad
    chunk = binary_part(r.bin, pos, n)
    chunk = if r.endian == :big, do: rev(chunk), else: chunk
    {chunk, %{r | pos: pos + n}}
  end

  def get_u32(r) do
    {chunk, r} = take(r, 4, 4)
    <<v::little-32>> = chunk
    {v, r}
  end

  # --- primitives beyond get_u32 — the byte-exact inverse of the Writer ---

  def get_u8(r) do
    <<v::8>> = binary_part(r.bin, r.pos, 1)
    {v, %{r | pos: r.pos + 1}}
  end

  def get_u16(r) do
    {chunk, r} = take(r, 2, 2)
    <<v::little-16>> = chunk
    {v, r}
  end

  @doc """
  A 64-bit integer at its natural eight-octet alignment, which `max_align`
  then caps — the same eight the writer passes. It passed 4 here while the
  writer passed 4 as well, so both were wrong together under XCDR1 and the
  round trip never showed it.
  """
  def get_u64(r) do
    {chunk, r} = take(r, 8, 8)
    <<v::little-64>> = chunk
    {v, r}
  end

  def get_f32(r) do
    {chunk, r} = take(r, 4, 4)
    <<v::float-little-32>> = chunk
    {v, r}
  end

  def get_f64(r) do
    {chunk, r} = take(r, 8, 8)
    <<v::float-little-64>> = chunk
    {v, r}
  end

  def get_bool(r) do
    {v, r} = get_u8(r)
    {v != 0, r}
  end

  @doc "The one octet an IDL `char` occupies."
  def get_char(r), do: get_u8(r)

  @doc "One two-octet `wchar` code unit."
  def get_wchar(r), do: get_u16(r)

  @doc """
  A `wstring` back into its UTF-16 code units. The length word counts octets,
  so the unit count is half of it.
  """
  def get_wstring(r) do
    {octets, r} = get_u32(r)

    Enum.reduce(1..div(octets, 2)//1, {[], r}, fn _, {acc, r} ->
      {unit, r} = get_u16(r)
      {[unit | acc], r}
    end)
    |> then(fn {acc, r} -> {Enum.reverse(acc), r} end)
  end

  @doc "A `bitset` anchor of the given width back into its holder word."
  def get_bitset(r, 1), do: get_u8(r)
  def get_bitset(r, 2), do: get_u16(r)
  def get_bitset(r, 4), do: get_u32(r)
  def get_bitset(r, 8), do: get_u64(r)

  def get_bitset(_r, anchor) do
    raise ArgumentError,
          "a bitset anchor is one, two, four or eight octets, not #{inspect(anchor)}"
  end

  @doc """
  A `fixed<digits,scale>` back into the decimal it was written from: a leading
  `-` when the sign nibble is 0xD, `digits - scale` integer digits, and a `.`
  with `scale` fraction digits where the scale is not zero. The scale is not
  on the wire, so it is an argument here for the same reason it is one in
  `put_fixed/4`.
  """
  def get_fixed(r, digits, scale) do
    octets = div(digits + 2, 2)

    {nibbles, r} =
      Enum.reduce(1..octets//1, {[], r}, fn _, {acc, r} ->
        {b, r} = get_u8(r)
        {[Bitwise.band(b, 0x0F), Bitwise.bsr(b, 4) | acc], r}
      end)

    nibbles = Enum.reverse(nibbles)
    sign = List.last(nibbles)
    body = Enum.slice(nibbles, length(nibbles) - 1 - digits, digits)

    text =
      body
      |> Enum.with_index()
      |> Enum.map_join(fn {n, i} ->
        point = if scale != 0 and i == digits - scale, do: ".", else: ""
        point <> <<?0 + n>>
      end)

    {if(sign == 0x0D, do: "-" <> text, else: text), r}
  end

  def get_string(r) do
    {n, r} = get_u32(r)
    s = binary_part(r.bin, r.pos, n - 1)
    {s, %{r | pos: r.pos + n}}
  end

  def get_seq_u8(r) do
    {n, r} = get_u32(r)
    b = binary_part(r.bin, r.pos, n)
    {b, %{r | pos: r.pos + n}}
  end

  @doc "A DHEADER, as the announced body byte length."
  def dheader_read(r), do: get_u32(r)

  @doc """
  A member header: `{id, must_understand, length_code, extent, reader}`, where
  `extent` is the member body's size in octets measured from the position this
  call leaves the cursor at.

  LC0..LC3 take the extent from the code. LC4 consumes the NEXTINT that
  follows the header. LC5..LC7 consume nothing: the next four octets are the
  member's own leading word and belong to the body, so they are peeked and the
  extent is `4 + word` for LC5, `4 + 4*word` for LC6 and `4 + 8*word` for LC7
  — the last two count elements, not octets.
  """
  def emheader_read(r) do
    {header, r} = get_u32(r)
    id = Bitwise.band(header, 0x0FFF_FFFF)
    must_understand = Bitwise.band(header, Bitwise.bsl(1, 31)) != 0
    length_code = Bitwise.band(Bitwise.bsr(header, 28), 0x7)

    {extent, r} =
      cond do
        length_code <= 3 ->
          {Bitwise.bsl(1, length_code), r}

        length_code == 4 ->
          get_u32(r)

        true ->
          at = r.pos
          {word, peeked} = get_u32(r)

          extent =
            case length_code do
              5 -> 4 + word
              6 -> 4 + 4 * word
              _ -> 4 + 8 * word
            end

          {extent, %{peeked | pos: at}}
      end

    {id, must_understand, length_code, extent, r}
  end

  @doc """
  A PL_CDR1 parameter header in either form:
  `{id, must_understand, length, list_end?, reader}`, with the id's flags
  stripped.

  The extended form is recognised by the marker PID_EXTENDED, and the
  must-understand answer then comes from bit 30 of the eMemberHeader and never
  from the marker — the marker carries the flag under this framing whatever
  the member says.

  The value's alignment origin is moved to the position this call leaves, and
  `pid_skip/3` restores it.
  """
  def pid_read(r) do
    {raw, r} = get_u16(r)
    short = Bitwise.band(raw, 0x3FFF)

    cond do
      short == @pid_list_end_id ->
        {_, r} = get_u16(r)
        {0, false, 0, true, r}

      short == @pid_extended_id ->
        # the slength, always 8
        {_, r} = get_u16(r)
        {header, r} = get_u32(r)
        {length, r} = get_u32(r)

        {Bitwise.band(header, 0x0FFF_FFFF),
         Bitwise.band(header, @em_flag_must_understand) != 0, length, false,
         %{r | origin: r.pos}}

      true ->
        {length, r} = get_u16(r)

        {short, Bitwise.band(raw, @pid_flag_must_understand) != 0, length, false,
         %{r | origin: r.pos}}
    end
  end

  @doc """
  Move the cursor past a parameter value of `length` octets from `body_start`
  and restore the stream's alignment origin.
  """
  def pid_skip(r, body_start, length) do
    pos = body_start + length
    pos = pos + rem(4 - rem(pos, 4), 4)
    %{r | pos: pos, origin: 0}
  end
end

defmodule ZeroDDS.Endpoint do
  @moduledoc "XRCE WRITE_DATA framing (8-byte header + sample body)."

  import Bitwise

  @session_nokey 0x80
  @stream_best_effort 0x01
  # E flag (little-endian body) + DataFormat FORMAT_DATA (0b000) in bits 1..3 —
  # DDS-XRCE 1.0 §8.3.5.8. FORMAT_SAMPLE (0b001, i.e. 0x03) would additionally
  # require a SampleInfo in the body, which this profile does not carry.
  @write_flags 0x01
  # Wire size of the BaseObjectRequest that precedes the sample (§7.7.8).
  @base_size 4
  # OBJK_DATAWRITER — the ObjectKind of a DataWriter (§7.2.2).
  @objk_datawriter 0x05

  def session_nokey, do: @session_nokey
  def stream_best_effort, do: @stream_best_effort
  def base_size, do: @base_size
  def write_flags, do: @write_flags
  def objk_datawriter, do: @objk_datawriter

  @doc "An ObjectId (§7.2.1) from a 12-bit raw id and a 4-bit ObjectKind."
  def object_id(raw_id, kind), do: bor(bsl(band(raw_id, 0x0FFF), 4), band(kind, 0x0F))

  # The 16-bit submessage_length field cannot describe a sample larger than
  # 0xFFFF; raise rather than truncate the length while appending the full
  # payload (mirrors the C SDK's rejection).
  def write_frame(_session, _stream, _seq, _object_id, sample)
      when byte_size(sample) + 4 > 0xFFFF do
    raise ArgumentError, "sample exceeds 16-bit submessage_length"
  end

  # The submessage body is the BaseObjectRequest (§7.7.8: request_id then
  # object_id, each two octets most significant first) followed by the sample.
  # `object_id` names the target DataWriter — without it no XRCE agent can
  # route the sample. The request_id is the message's own `seq`, which §7.7.8
  # only requires to be unique per stream. The flag byte is the E flag
  # (little-endian body) plus DataFormat FORMAT_DATA (0b000) — §8.3.5.8.
  def write_frame(session, stream, seq, object_id, sample) do
    n = @base_size + byte_size(sample)

    <<session::8, stream::8, seq::little-16, 0x07::8, @write_flags::8, n::little-16,
      seq::big-16, object_id::big-16>> <> sample
  end

  # Frame layout: [session, stream, seq_lo, seq_hi, sm_id, flags, len_lo,
  # len_hi] then the sample body. The 16-bit little-endian submessage_length
  # (bytes 6..7) bounds the body exactly: binary_part(frame, 8, sm_len), never
  # byte_size(frame) - 8, so trailing padding or an appended submessage is not
  # folded into the sample.
  #
  # Accepts WRITE_DATA (0x07, endpoint->hub / loopback) and DATA (0x09,
  # hub->endpoint) — the hub pushes samples with DATA. See DDS-XRCE spec 8.3.5.
  # Returns :error for a short header, a wrong submessage id, or a declared
  # length that runs past the datagram (truncation / wrong length).
  def read_frame(<<_::binary-size(4), id, _flags, sm_len::little-16, rest::binary>> = frame)
      when byte_size(frame) >= 8 + @base_size and (id == 0x07 or id == 0x09) and
             sm_len >= @base_size and byte_size(rest) >= sm_len do
    {:ok, binary_part(rest, @base_size, sm_len - @base_size)}
  end

  def read_frame(_), do: :error

  @doc """
  `{:ok, {request_id, object_id}}` of a data frame (§7.7.8), or `:error` under
  the same rules as `read_frame/1`.
  """
  def frame_base(<<_::binary-size(8), request_id::big-16, object_id::big-16, _::binary>> = frame) do
    case read_frame(frame) do
      {:ok, _} -> {:ok, {request_id, object_id}}
      :error -> :error
    end
  end

  def frame_base(_), do: :error
end

defmodule ZeroDDS.Client do
  @moduledoc """
  Sync endpoint. `transport` is `%{deliver: fun/1, receive: fun/0}` where
  receive returns a frame binary or nil.
  """

  def new(transport, object_id) do
    %{
      transport: transport,
      session: ZeroDDS.Endpoint.session_nokey(),
      stream: ZeroDDS.Endpoint.stream_best_effort(),
      # ObjectId of the target DataWriter (§7.7.8 BaseObjectRequest).
      object_id: object_id,
      seq: 1
    }
  end

  def write(c, sample) do
    frame = ZeroDDS.Endpoint.write_frame(c.session, c.stream, c.seq, c.object_id, sample)
    c.transport.deliver.(frame)
    %{c | seq: rem(c.seq + 1, 0x10000)}
  end

  # One non-blocking receive: returns the sample body, or nil.
  def poll(c) do
    case c.transport.receive.() do
      nil ->
        nil

      frame ->
        case ZeroDDS.Endpoint.read_frame(frame) do
          {:ok, body} -> body
          :error -> nil
        end
    end
  end
end

defmodule ZeroDDS.AsyncReader do
  @moduledoc """
  Async endpoint, idiomatic BEAM: spawns a process that polls the transport and
  sends `{:zerodds_sample, body}` to `target`. The consumer receives in its own
  mailbox. `stop/1` tells the reader to exit.
  """

  def start(transport, target) do
    spawn(fn -> loop(transport, target) end)
  end

  def stop(pid), do: send(pid, :zerodds_stop)

  defp loop(transport, target) do
    receive do
      :zerodds_stop -> :ok
    after
      0 ->
        case transport.receive.() do
          nil ->
            Process.sleep(1)
            loop(transport, target)

          frame ->
            case ZeroDDS.Endpoint.read_frame(frame) do
              {:ok, body} -> send(target, {:zerodds_sample, body})
              :error -> :ok
            end

            loop(transport, target)
        end
    end
  end
end

defmodule ZeroDDS.MemTransport do
  @moduledoc "In-memory FIFO transport (an Agent) for tests + example."

  def new do
    {:ok, pid} = Agent.start_link(fn -> :queue.new() end)

    %{
      deliver: fn frame ->
        Agent.update(pid, fn q -> :queue.in(frame, q) end)
        true
      end,
      receive: fn ->
        Agent.get_and_update(pid, fn q ->
          case :queue.out(q) do
            {{:value, f}, q2} -> {f, q2}
            {:empty, q2} -> {nil, q2}
          end
        end)
      end
    }
  end
end
