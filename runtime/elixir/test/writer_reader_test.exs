# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
defmodule ZeroddsWire.WriterReaderTest do
  @moduledoc """
  The wire runtime against bytes worked out from the clauses — and, for
  the shapes the sibling runtimes carry as measured vendor bytes, the
  same octets those record.
  """
  use ExUnit.Case, async: true

  alias ZeroddsWire.PlCdr1Framing
  alias ZeroddsWire.Reader
  alias ZeroddsWire.WireError
  alias ZeroddsWire.Writer

  # -- alignment and the primitives ----------------------------------------

  test "a long after a char sits at four, from the stream's origin" do
    # XTypes 1.3 §7.4.1.1: alignment is from the stream, so the pad is
    # three bytes.
    w = Writer.new(:le)
    w = Writer.write_char(w, "a")
    w = Writer.write_i32(w, 1)
    assert Writer.finish(w) == <<?a, 0, 0, 0, 1, 0, 0, 0>>
  end

  test "the alignment cap is four under XCDR2" do
    w = Writer.new(:le, 4)
    w = Writer.write_u8(w, 1)
    w = Writer.write_u64(w, 2)
    assert Writer.finish(w) == <<1, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0>>
  end

  test "every integer width round-trips in both byte orders" do
    cases = [
      {&Writer.write_i8/2, &Reader.read_i8/1, -13},
      {&Writer.write_u8/2, &Reader.read_u8/1, 254},
      {&Writer.write_i16/2, &Reader.read_i16/1, -12_345},
      {&Writer.write_u16/2, &Reader.read_u16/1, 48_879},
      {&Writer.write_i32/2, &Reader.read_i32/1, -1_234_567},
      {&Writer.write_u32/2, &Reader.read_u32/1, 3_735_928_559},
      {&Writer.write_i64/2, &Reader.read_i64/1, -987_654_321_987},
      {&Writer.write_u64/2, &Reader.read_u64/1, 16_045_690_984_833_335_998}
    ]

    for order <- [:le, :be], {write, read, value} <- cases do
      bytes = Writer.new(order) |> write.(value) |> Writer.finish()
      {out, _} = read.(Reader.new(bytes, order))
      assert out == value, "#{inspect(value)} under #{order}"
    end
  end

  test "an integer outside its field is refused, not wrapped" do
    assert_raise WireError, fn -> Writer.write_u8(Writer.new(:le), 256) end
    assert_raise WireError, fn -> Writer.write_i16(Writer.new(:le), 0x8000) end
    assert_raise WireError, fn -> Writer.write_u64(Writer.new(:le), -1) end
  end

  test "floats round-trip and a float member is rounded to single once" do
    bytes = Writer.new(:le) |> Writer.write_f32(-3.75) |> Writer.finish()
    assert bytes == <<0x00, 0x00, 0x70, 0xC0>>
    {value, _} = Reader.read_f32(Reader.new(bytes, :le))
    assert value == -3.75

    bytes = Writer.new(:be) |> Writer.write_f64(3.141592653589793) |> Writer.finish()
    {value, _} = Reader.read_f64(Reader.new(bytes, :be))
    assert value == 3.141592653589793
  end

  test "the non-finite floats travel as atoms and canonical patterns" do
    for {value, bits} <- [
          {:nan, 0x7FC00000},
          {:infinity, 0x7F800000},
          {:neg_infinity, 0xFF800000}
        ] do
      bytes = Writer.new(:le) |> Writer.write_f32(value) |> Writer.finish()
      assert bytes == <<bits::unsigned-little-32>>
      {out, _} = Reader.read_f32(Reader.new(bytes, :le))
      assert out == value
    end

    # A magnitude no binary32 holds becomes the infinity a C cast
    # produces.
    bytes = Writer.new(:le) |> Writer.write_f32(1.0e300) |> Writer.finish()
    assert bytes == <<0x7F800000::unsigned-little-32>>
  end

  test "a negative zero double keeps its sign bit" do
    bytes = Writer.new(:le) |> Writer.write_f64(-0.0) |> Writer.finish()
    assert bytes == <<0, 0, 0, 0, 0, 0, 0, 0x80>>
  end

  # -- booleans and characters ---------------------------------------------

  test "a boolean byte that is neither zero nor one is refused" do
    assert_raise WireError, fn -> Reader.read_bool(Reader.new(<<2>>, :le)) end
  end

  test "a char above U+00FF is refused rather than truncated" do
    assert_raise WireError, fn -> Writer.write_char(Writer.new(:le), "š") end
  end

  test "a latin-1 char is one byte out and the same character back" do
    bytes = Writer.new(:le) |> Writer.write_char("ä") |> Writer.finish()
    assert bytes == <<0xE4>>
    {value, _} = Reader.read_char(Reader.new(bytes, :le))
    assert value == "ä"
  end

  test "a wchar is two octets in the stream's order and four under RTI's XCDR1 form" do
    bytes = Writer.new(:le) |> Writer.write_wchar("a", 2, :fixed, :stream) |> Writer.finish()
    assert bytes == <<0x61, 0x00>>

    bytes = Writer.new(:le) |> Writer.write_wchar("a", 4, :fixed, :stream) |> Writer.finish()
    assert bytes == <<0x61, 0x00, 0x00, 0x00>>

    {value, _} = Reader.read_wchar(Reader.new(bytes, :le), 4, :fixed, :stream)
    assert value == "a"
  end

  test "a four-byte wchar using the high bits is refused, not narrowed" do
    assert_raise WireError, fn ->
      Reader.read_wchar(Reader.new(<<0x01, 0x00, 0x01, 0x00>>, :le), 4, :fixed, :stream)
    end
  end

  test "the GIOP wchar carries its octet count and refuses another" do
    # Measured in the Python runtime: omniORB writes `02 00e4` in either
    # stream order for U+00E4.
    bytes =
      Writer.new(:le)
      |> Writer.write_wchar("ä", 2, :"giop-length-prefixed", :"byte-order-mark")
      |> Writer.finish()

    assert bytes == <<0x02, 0x00, 0xE4>>

    {value, _} =
      Reader.read_wchar(Reader.new(bytes, :le), 2, :"giop-length-prefixed", :"byte-order-mark")

    assert value == "ä"

    assert_raise WireError, fn ->
      Reader.read_wchar(Reader.new(<<0xE4, 0x00>>, :le), 2, :"giop-length-prefixed", :stream)
    end
  end

  # -- strings -------------------------------------------------------------

  test "a string's length counts bytes and the terminator" do
    bytes = Writer.new(:le) |> Writer.write_string("été") |> Writer.finish()
    assert bytes == <<6, 0, 0, 0, 0xC3, 0xA9, ?t, 0xC3, 0xA9, 0>>
    {value, _} = Reader.read_string_alloc(Reader.new(bytes, :le))
    assert value == "été"
  end

  test "a string length of zero or past the stream is refused" do
    assert_raise WireError, fn ->
      Reader.read_string_alloc(Reader.new(<<0, 0, 0, 0>>, :le))
    end

    assert_raise WireError, fn ->
      Reader.read_string_alloc(Reader.new(<<255, 0, 0, 0, ?a>>, :le))
    end
  end

  test "the specification wstring counts octets, no BOM and no terminator" do
    # §7.4.1.1.2, and 𝄞 is two code units — a length of four for the
    # one-character string.
    bytes = Writer.new(:le) |> Writer.write_wstring_utf8("𝄞", :specification, :stream) |> Writer.finish()
    assert bytes == <<4, 0, 0, 0, 0x34, 0xD8, 0x1E, 0xDD>>
    {value, _} = Reader.read_wstring_utf8(Reader.new(bytes, :le), :specification, :stream)
    assert value == "𝄞"
  end

  test "the character-count cells count units and the terminated cells carry a NUL" do
    bytes =
      Writer.new(:le)
      |> Writer.write_wstring_utf8("ab", :"character-count", :stream)
      |> Writer.finish()

    assert bytes == <<2, 0, 0, 0, ?a, 0, ?b, 0>>

    bytes =
      Writer.new(:le)
      |> Writer.write_wstring_utf8("a", :"character-count-terminated", :stream)
      |> Writer.finish()

    assert bytes == <<2, 0, 0, 0, ?a, 0, 0, 0, 0, 0, 0, 0>>

    {value, _} =
      Reader.read_wstring_utf8(Reader.new(bytes, :le), :"character-count-terminated", :stream)

    assert value == "a"
  end

  test "a character count over two-octet units refuses a code point above the BMP" do
    assert_raise WireError, fn ->
      Writer.write_wstring_utf8(Writer.new(:le), "𝄞", :"character-count", :stream)
    end
  end

  test "an odd octet count is refused: no encoder produced it" do
    assert_raise WireError, fn ->
      Reader.read_wstring_utf8(Reader.new(<<3, 0, 0, 0, ?a, 0, ?b>>, :le), :specification, :stream)
    end
  end

  test "the byte-order-mark order writes one and the reader obeys it" do
    bytes =
      Writer.new(:be)
      |> Writer.write_wstring_utf8("ab", :specification, :"byte-order-mark")
      |> Writer.finish()

    # The count is big-endian (the stream's), the mark and units little.
    assert bytes == <<0, 0, 0, 6, 0xFF, 0xFE, ?a, 0, ?b, 0>>

    {value, _} =
      Reader.read_wstring_utf8(Reader.new(bytes, :be), :specification, :"byte-order-mark")

    assert value == "ab"

    # A swapped mark says every unit behind it is swapped too.
    swapped = <<6, 0, 0, 0, 0xFE, 0xFF, 0, ?a, 0, ?b>>

    {value, _} =
      Reader.read_wstring_utf8(Reader.new(swapped, :le), :specification, :"byte-order-mark")

    assert value == "ab"
  end

  test "an empty wstring carries no mark" do
    bytes =
      Writer.new(:le)
      |> Writer.write_wstring_utf8("", :specification, :"byte-order-mark")
      |> Writer.finish()

    assert bytes == <<0, 0, 0, 0>>
  end

  # -- fixed ---------------------------------------------------------------

  test "a fixed packs BCD with the sign nibble last" do
    # [CDR] §9.3.2.8: fixed<5,2> of "123.45" is 12 34 5c.
    bytes = Writer.new(:le) |> Writer.write_fixed("123.45", 5, 2, :declared) |> Writer.finish()
    assert bytes == <<0x12, 0x34, 0x5C>>
    {value, _} = Reader.read_fixed(Reader.new(bytes, :le), 5, 2, :declared)
    assert value == "123.45"

    bytes = Writer.new(:le) |> Writer.write_fixed("-0.01", 5, 2, :declared) |> Writer.finish()
    assert bytes == <<0x00, 0x00, 0x1D>>
    {value, _} = Reader.read_fixed(Reader.new(bytes, :le), 5, 2, :declared)
    assert value == "-0.01"
  end

  test "a negative zero fixed is written positive and read back as one zero" do
    bytes = Writer.new(:le) |> Writer.write_fixed("-0.00", 5, 2, :declared) |> Writer.finish()
    assert bytes == <<0x00, 0x00, 0x0C>>
    {value, _} = Reader.read_fixed(Reader.new(bytes, :le), 5, 2, :declared)
    assert value == "0.00"
  end

  test "the value-precision count writes the value's digits and reads to the sign nibble" do
    # JacORB and TAO's form: as many digits as the value has, never
    # fewer than one.
    bytes =
      Writer.new(:le) |> Writer.write_fixed("1.50", 5, 2, :"value-precision") |> Writer.finish()

    assert bytes == <<0x15, 0x0C>>
    {value, _} = Reader.read_fixed(Reader.new(bytes, :le), 5, 2, :"value-precision")
    assert value == "1.50"
  end

  test "a fixed too wide for either half is refused" do
    assert_raise WireError, fn ->
      Writer.write_fixed(Writer.new(:le), "1234.0", 5, 2, :declared)
    end

    assert_raise WireError, fn ->
      Writer.write_fixed(Writer.new(:le), "1.234", 5, 2, :declared)
    end
  end

  # -- the bit-bounded kinds -----------------------------------------------

  test "an enumerator's holder is signed and the bound picks its width" do
    # §7.4.3.5.3 rule (5): @value(-1) at the default bound is ffffffff.
    bytes = Writer.new(:le) |> Writer.write_enum(32, -1) |> Writer.finish()
    assert bytes == <<0xFF, 0xFF, 0xFF, 0xFF>>
    {value, _} = Reader.read_enum(Reader.new(bytes, :le), 32)
    assert value == -1

    # Cyclone's @value(200) under @bit_bound(8) is c8, and the holder
    # sign-extends it.
    bytes = Writer.new(:le) |> Writer.write_enum(8, 200) |> Writer.finish()
    assert bytes == <<0xC8>>
    {value, _} = Reader.read_enum(Reader.new(bytes, :le), 8)
    assert value == -56
  end

  test "a bitmask above its bound is refused on the way out" do
    assert_raise WireError, fn -> Writer.write_bitmask(Writer.new(:le), 8, 256) end
  end

  test "a bitfield wider than its slot or a value outside it is refused" do
    w = Writer.new(:le)
    assert Writer.bitfield(w, 5, 3, 4) == 80
    assert_raise WireError, fn -> Writer.bitfield(w, 8, 3, 0) end
    assert_raise WireError, fn -> Writer.bitfield(w, 1, 8, 60) end
  end

  test "the union overlay writes the low octets of the pattern" do
    assert ZeroddsWire.bit_pattern_int(-2, 4) == 0xFFFFFFFE

    bytes =
      Writer.new(:le)
      |> Writer.write_union_overlay(ZeroddsWire.bit_pattern_int(-2, 4), 4)
      |> Writer.finish()

    assert bytes == <<0xFE, 0xFF, 0xFF, 0xFF>>
  end

  # -- DHEADER and EMHEADER ------------------------------------------------

  test "a DHEADER is patched with the length of what followed" do
    w = Writer.new(:le, 4)
    {d, w} = Writer.write_dheader(w)
    w = Writer.write_u32(w, 7)
    w = Writer.finish_dheader(w, d)
    assert Writer.finish(w) == <<4, 0, 0, 0, 7, 0, 0, 0>>
  end

  test "the absent DHEADER writes nothing and finishes nothing" do
    w = Writer.new(:le, 4)
    d = ZeroddsWire.no_dheader()
    w = Writer.write_u32(w, 7)
    w = Writer.finish_dheader(w, d)
    assert Writer.finish(w) == <<7, 0, 0, 0>>
  end

  test "an LC4 EMHEADER takes a NEXTINT and the short-code policy removes it" do
    w = Writer.new(:le, 4)
    {h, w} = Writer.write_emheader(w, 1, 4, false)
    w = Writer.write_u32(w, 5)
    w = Writer.finish_emheader(w, h)
    # LC4, member 1, NEXTINT 4, then the value.
    assert Writer.finish(w) == <<1, 0, 0, 0x40, 4, 0, 0, 0, 5, 0, 0, 0>>

    w = Writer.new(:le, 4, %PlCdr1Framing{}, true)
    {h, w} = Writer.write_emheader(w, 1, 4, false)
    w = Writer.write_u32(w, 5)
    w = Writer.finish_emheader(w, h)
    # The NEXTINT is gone and the code says four octets — LC2.
    assert Writer.finish(w) == <<1, 0, 0, 0x20, 5, 0, 0, 0>>
  end

  test "the reader's EMHEADER carries the extent a skipping reader advances by" do
    bytes = <<1, 0, 0, 0x40, 4, 0, 0, 0, 5, 0, 0, 0>>
    {h, r} = Reader.read_emheader(Reader.new(bytes, :le, 4))
    assert h.member_id == 1
    assert h.length_code == 4
    assert h.extent == 4
    {value, _} = Reader.read_u32(r)
    assert value == 5

    # Codes 5 to 7 peek their NEXTINT and put the cursor back.
    bytes = <<1, 0, 0, 0x50, 3, 0, 0, 0>>
    {h, r} = Reader.read_emheader(Reader.new(bytes, :le, 4))
    assert h.length_code == 5
    assert h.next_int == 3
    assert h.extent == 4 + 3
    assert Reader.reader_position(r) == 4
  end

  # -- PL_CDR1 -------------------------------------------------------------

  test "a short parameter and the sentinel, under the specification framing" do
    w = Writer.new(:le, 8)
    {p, w} = Writer.write_pid(w, 4, :member, false)
    w = Writer.write_i32(w, 7)
    w = Writer.finish_pid(w, p)
    w = Writer.write_sentinel(w)
    # The member id bare, the sentinel carrying must-understand — the
    # specification framing's answers on both positions.
    assert Writer.finish(w) == <<4, 0, 4, 0, 7, 0, 0, 0, 2, 0x7F, 0, 0>>
  end

  test "a parameter's value is its own block: an eight-byte member takes no inner padding" do
    w = Writer.new(:le, 8)
    w = Writer.write_u32(w, 0)
    {p, w} = Writer.write_pid(w, 1, :member, false)
    w = Writer.write_f64(w, 1.0)
    w = Writer.finish_pid(w, p)
    bytes = Writer.finish(w)
    # Four bytes in, the header at 4..8, the double immediately at 8:
    # its alignment counts from the value's own start.
    assert byte_size(bytes) == 16
    assert binary_part(bytes, 6, 2) == <<8, 0>>
  end

  test "the reader takes RTI's padded length and Fast DDS's exact one for the same member" do
    framing = %PlCdr1Framing{}
    # A two-byte value: declared 4 under a padding framing, 2 under
    # Fast DDS's. Both readings are accepted.
    for declared <- [2, 4] do
      bytes = <<1, 0, declared, 0, 7, 0, 0, 0>>
      r = Reader.new(bytes, :le, 8, framing)
      {p, r} = Reader.read_pid(r)
      assert p.pid == 1
      {value, r} = Reader.read_i16(r)
      assert value == 7
      r = Reader.finish_pid_read(r, p)
      assert Reader.reader_status(r) == :ok
      assert Reader.reader_position(r) == 8
    end

    # A declared length belonging to neither reading is refused.
    bytes = <<1, 0, 9, 0, 7, 0, 0, 0>>
    r = Reader.new(bytes, :le, 8, framing)
    {p, r} = Reader.read_pid(r)
    {_, r} = Reader.read_i16(r)
    r = Reader.finish_pid_read(r, p)
    assert Reader.reader_status(r) == :out_of_range
  end

  test "under a framing that pads nothing the reader does not walk past the last value" do
    framing = %PlCdr1Framing{
      pad_declared_length: false,
      pad_declared_optional_length: false,
      pad_after_value: false,
      member_must_understand: false,
      pid_flags: :bare
    }

    # A stream that ends where the two-byte value does — Fast DDS's 22
    # against 24 octets, in miniature.
    bytes = <<1, 0, 2, 0, 7, 0>>
    r = Reader.new(bytes, :le, 8, framing)
    {p, r} = Reader.read_pid(r)
    {value, r} = Reader.read_i16(r)
    assert value == 7
    r = Reader.finish_pid_read(r, p)
    assert Reader.reader_status(r) == :ok
    assert Reader.reader_remaining(r) == 0
  end

  test "a short header whose value overruns sixteen bits is promoted in place" do
    w = Writer.new(:le, 8)
    {p, w} = Writer.write_pid(w, 4, :member, false)
    long = String.duplicate("x", 0x10000)
    w = Writer.write_string(w, long)
    w = Writer.finish_pid(w, p)
    bytes = Writer.finish(w)
    # The extended marker with the framing's flags, the 8, the id, the
    # 32-bit length.
    assert binary_part(bytes, 0, 2) == <<0x01, 0x7F>>
    assert binary_part(bytes, 2, 2) == <<8, 0>>
    assert binary_part(bytes, 4, 4) == <<4, 0, 0, 0>>

    r = Reader.new(bytes, :le, 8)
    {p, r} = Reader.read_pid(r)
    assert p.pid == 4
    {value, r} = Reader.read_string_alloc(r)
    assert value == long
    r = Reader.finish_pid_read(r, p)
    assert Reader.reader_status(r) == :ok
  end

  test "OpenDDS's framing flags every member and closes with a bare sentinel" do
    framing = %PlCdr1Framing{pid_flags: :"impl-extension-members"}
    w = Writer.new(:le, 8, framing)
    {p, w} = Writer.write_pid(w, 1, :member, false)
    w = Writer.write_i32(w, 7)
    w = Writer.finish_pid(w, p)
    w = Writer.write_sentinel(w)
    assert Writer.finish(w) == <<1, 0xC0, 4, 0, 7, 0, 0, 0, 2, 0x3F, 0, 0>>

    # And the reader under that framing reports the flag absent —
    # it carries no information there.
    r = Reader.new(Writer.finish(w), :le, 8, framing)
    {p, _} = Reader.read_pid(r)
    assert p.pid == 1
    refute p.impl_extension
    assert ZeroddsWire.pid_is_user_member(p)
  end

  test "a stopped reader answers defaults and does not move" do
    r = Reader.new(<<1, 2, 3, 4>>, :le)
    r = Reader.reader_refuse(r, :discarded)
    {value, r} = Reader.read_u32(r)
    assert value == 0
    assert Reader.reader_position(r) == 0
    # The first refusal sticks.
    r = Reader.reader_refuse(r, :out_of_range)
    assert Reader.reader_status(r) == :discarded
    {true, r} = Reader.reader_clear_discard(r)
    assert Reader.reader_status(r) == :ok
  end

  test "a save and restore puts back cursor, origin and status" do
    r = Reader.new(<<1, 0, 0, 0, 2, 0, 0, 0>>, :le)
    mark = Reader.reader_save(r)
    {_, r} = Reader.read_u32(r)
    r = Reader.reader_refuse(r, :out_of_range)
    r = Reader.reader_restore(r, mark)
    assert Reader.reader_position(r) == 0
    assert Reader.reader_status(r) == :ok
  end

  # -- the trims -----------------------------------------------------------

  test "the trims cut at the bound and never inside a character" do
    r = Reader.new(<<>>, :le)
    assert Reader.trim_utf8(r, "étage", 3) == "ét"
    assert Reader.trim_utf8(r, "étage", 2) == "é"
    assert Reader.trim_utf16(r, "a𝄞b", 2) == "a"
    assert Reader.trim_utf16(r, "a𝄞b", 3) == "a𝄞"
  end

  # -- the range edges no corpus sample can carry --------------------------
  #
  # `tools/wireprobe/refusal-range-edges.tsv` is the case list: values
  # with exactly one right answer derived from the clause, which no
  # corpus cell holds because the generator refuses such a sample before
  # a renderer sees it. Four of its rows reach this runtime's headers and
  # its `fixed`. A masked, truncated or canonicalised value on the wire
  # is the silent-drop class, so the answer to each is a refusal.

  test "a member id past the EMHEADER's 28 bits is refused, not masked" do
    # XTypes 1.3 §7.2.2.4.4.4.4 gives ids 0 to 0x0FFFFFFF; §7.4.3.4.2's
    # formula masks with 0x0FFFFFFF. Masking 0x10000000 would write a
    # well-formed header for member 0 — the wrong member, and nothing on
    # the reading side can tell.
    w = Writer.new(:le, 4)
    assert_raise WireError, fn -> Writer.write_emheader(w, 0x1000_0000, 4, false) end
    # The largest id that does fit is written.
    {_, w} = Writer.write_emheader(w, 0x0FFF_FFFF, 2, false)
    assert Writer.finish(w) == <<0xFF, 0xFF, 0xFF, 0x2F>>
  end

  test "an eMemberHeader id past 28 bits is refused in the extended parameter too" do
    # §7.4.1.2.1's eMemberHeader has the same 28 bits as the EMHEADER.
    w = Writer.new(:le, 8)
    assert_raise WireError, fn -> Writer.write_pid_extended(w, 0x1000_0000, :member, false) end
  end

  test "a short parameter id in the reserved range is refused, not written" do
    # §7.4.1.2.2: the short form carries 0 to 0x3F00 inclusive, and
    # 0x3F01 to 0x3FFF are the protocol's own — PID_EXTENDED is 0x3F01
    # itself, so a short header there collides with it.
    w = Writer.new(:le, 8)
    assert_raise WireError, fn -> Writer.write_pid(w, 0x3F01, :member, false) end
    # 0x3F00 exactly is still the short form; the off-by-one was a
    # recorded defect elsewhere and this is the boundary it turns on.
    {_, out} = Writer.write_pid(w, 0x3F00, :member, false)
    assert binary_part(Writer.finish(out), 0, 2) == <<0x00, 0x3F>>
    # And the extended form is what carries the id the short one cannot.
    # The marker is PID_EXTENDED plus whatever the framing puts on that
    # position — the specification framing sets must-understand there,
    # so the octets are 0x7F01 and not 0x3F01. The id itself is in the
    # eMemberHeader that follows the length of 8.
    {_, out} = Writer.write_pid_extended(w, 0x3F01, :member, false)
    bytes = Writer.finish(out)
    assert binary_part(bytes, 0, 4) == <<0x01, 0x7F, 8, 0>>
    assert binary_part(bytes, 4, 4) == <<0x01, 0x3F, 0, 0>>
  end

  test "an LC 0..3 member whose body is not the declared size is refused" do
    # §7.4.3.4.2: under codes 0 to 3 the code *is* the length — 1, 2, 4
    # or 8 octets. A body of another size misdeclares itself and the
    # reader's skip lands inside data. LC 4 to 7 carry every other size.
    w = Writer.new(:le, 4)
    {h, w} = Writer.write_emheader(w, 1, 1, false)
    w = Writer.write_u8(w, 1)
    w = Writer.write_u8(w, 2)
    w = Writer.write_u8(w, 3)
    assert_raise WireError, fn -> Writer.finish_emheader(w, h) end

    # The same header over the two octets it declares finishes.
    w = Writer.new(:le, 4)
    {h, w} = Writer.write_emheader(w, 1, 1, false)
    w = Writer.write_u16(w, 0x0201)
    w = Writer.finish_emheader(w, h)
    assert Writer.finish(w) == <<1, 0, 0, 0x10, 1, 2>>
  end

  test "a fixed wider than 31 digits is refused on both sides" do
    # OMG IDL 4.2 §7.4.1.4.4.3.4 rule (42) caps the BCD rule at 31
    # digits; `fixed<32, 2>` names a packing the rule does not define.
    w = Writer.new(:le, 8)
    assert_raise WireError, fn -> Writer.write_fixed(w, "1.00", 32, 2, :declared) end
    # And the scale within the digits, the same clause.
    assert_raise WireError, fn -> Writer.write_fixed(w, "1.00", 5, 6, :declared) end
    # 31 exactly is the widest that is written.
    w = Writer.write_fixed(w, "1.00", 31, 2, :declared)
    assert byte_size(Writer.finish(w)) == 16

    # The reader refuses the same declaration rather than taking a count
    # of octets the rule does not define.
    r = Reader.new(:binary.copy(<<0>>, 32), :le, 8)
    assert_raise WireError, fn -> Reader.read_fixed(r, 32, 2, :declared) end
  end

  # -- the two language facts the emitter's defaults lean on ---------------

  test "false sorts before true in Erlang's term order" do
    # `CodecTarget::bool_less`'s default is the target's own `<`; this
    # is the measurement that makes the default right here. Sorted
    # rather than compared literally, which the compiler folds and
    # warns about.
    assert Enum.sort([true, false]) == [false, true]
    assert Enum.min([true, false]) == false
  end

  test "binary comparison is bytewise, which is strcmp's unsigned order" do
    # C11 §7.24.4 compares as unsigned char; Erlang binaries compare
    # bytewise, so the map key order the walk spells through `strcmp`
    # is the `<` this runtime's strings already have.
    assert "a" < "b"
    assert "a" < "aa"
    assert <<0x7F>> < <<0x80>>
  end

  # -- the three HDDS axes --------------------------------------------------
  #
  # Expected octets from the recorded columns:
  # tests/interop-results/array-1d-struct-final/hdds/plain-cdr2.le.bin
  # (packed elements),
  # tests/interop-results/sequence-struct-appendable/hdds/plain-cdr2.le.bin
  # (a sequence element's own DHEADER length),
  # tests/interop-results/wstring-member/hdds/plain-cdr2.le.bin,
  # tests/interop-results/wstring-unbounded-utf8/hdds/plain-cdr2.le.bin and
  # tests/interop-results/wstring-bounded-utf8/hdds/plain-cdr2.le.bin
  # (a wstring as its own UTF-8 octets).

  test "a packed element starts where the last ended" do
    # ElementPadding::Packed: the second Cell's `long` sits at offset
    # six, where the clause would put it at eight.
    w = Writer.new(:le)
    {saved, w} = Writer.begin_packed_element(w)
    w = Writer.write_i32(w, 1)
    w = Writer.write_i16(w, 2)
    w = Writer.finish_packed_element(w, saved)
    {saved, w} = Writer.begin_packed_element(w)
    w = Writer.write_i32(w, 3)
    w = Writer.write_i16(w, 4)
    w = Writer.finish_packed_element(w, saved)
    w = Writer.write_u8(w, 119)
    assert Writer.finish(w) == <<1, 0, 0, 0, 2, 0, 3, 0, 0, 0, 4, 0, 119>>
  end

  test "a packed element reads back at its own origin" do
    r = Reader.new(<<1, 0, 0, 0, 2, 0, 3, 0, 0, 0, 4, 0, 119>>, :le)
    {saved, r} = Reader.reader_begin_packed_element(r)
    {a1, r} = Reader.read_i32(r)
    {b1, r} = Reader.read_i16(r)
    r = Reader.reader_finish_packed_element(r, saved)
    {saved, r} = Reader.reader_begin_packed_element(r)
    {a2, r} = Reader.read_i32(r)
    {b2, r} = Reader.read_i16(r)
    r = Reader.reader_finish_packed_element(r, saved)
    {last, _r} = Reader.read_u8(r)
    assert {a1, b1, a2, b2, last} == {1, 2, 3, 4, 119}
  end

  test "a sequence element's own length in DHEADER form" do
    # SequenceElementDelimiter::BeforeEachNonFinalElement: the first
    # `many` Cell is six octets long.
    w = Writer.new(:le)
    {at, w} = Writer.write_dheader(w)
    w = Writer.write_i32(w, 1)
    w = Writer.write_i16(w, 2)
    w = Writer.finish_dheader(w, at)
    assert Writer.finish(w) == <<6, 0, 0, 0, 1, 0, 0, 0, 2, 0>>
  end

  test "a UTF-8 wstring is its octets and a NUL" do
    w = Writer.new(:le)
    w = Writer.write_wstring_utf8_octets(w, "abc", :"octets/terminated/2")
    assert Writer.finish(w) == <<4, 0, 0, 0, ?a, ?b, ?c, 0>>
  end

  test "a multi-byte UTF-8 wstring counts its octets" do
    w = Writer.new(:le)
    w = Writer.write_wstring_utf8_octets(w, "äöü ሴ Z", :"octets/terminated/2")
    assert Writer.finish(w) == Base.decode16!("0D000000C3A4C3B6C3BC20E188B4205A00", case: :mixed)
  end

  test "a bounded UTF-8 wstring holds three units" do
    w = Writer.new(:le)
    w = Writer.write_wstring_utf8_octets_bounded(w, "ሴäZ", 3, :"octets/terminated/2")
    assert Writer.finish(w) == Base.decode16!("07000000E188B4C3A45A00", case: :mixed)
  end

  test "a UTF-8 wstring reads back as its octets" do
    bytes = Base.decode16!("0D000000C3A4C3B6C3BC20E188B4205A00", case: :mixed)
    {text, _r} = Reader.read_wstring_utf8_octets(Reader.new(bytes, :le), :"octets/terminated/2")
    assert text == "äöü ሴ Z"
  end
end
