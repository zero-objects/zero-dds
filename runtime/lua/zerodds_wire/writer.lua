-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
-- Putting bytes on the wire.
--
-- The Lua counterpart of `crates/wire/src/writer.rs` and of
-- `runtime/python/zerodds_wire/writer.py` — a further implementation of
-- the same clauses, not a binding to either. Where two columns disagree
-- on an octet, one of them has misread DDS-XTypes 1.3, and that is the
-- point of having several.
--
-- Alignment is relative to the start of the stream, not to the start of
-- the value: XCDR aligns each primitive to its own size counted from
-- the encapsulation. XTypes 1.3 §7.4.1.1.
--
-- The writer is a plain table and every operation is a function taking
-- it first — `zdw.write_u32(w, x)` — because that is the shape the
-- shared codec walk emits through `emit/lua.rs` and the shape C's
-- runtime already has. No metatables: a missing operation is then a
-- call to `nil`, which Lua reports by name at the line, rather than a
-- lookup that quietly answered nothing.
--
-- Three things Lua makes easy to get wrong, and which are therefore
-- decided here rather than assumed:
--
-- * a Lua integer is 64-bit two's complement and wraps, so `u64` takes
--   the bit pattern as it stands and every narrower width is
--   range-checked before a byte goes out;
-- * `#s` on a Lua string counts bytes, which is what a `string`'s
--   length field counts — and what a `wstring`'s does **not**, so the
--   wide form goes through code units and never through `#`;
-- * a value the type cannot hold is refused with `error`, never masked:
--   a masked value is a different number on the wire and a report of
--   success.

local float128 = require("zerodds_wire.float128")

local M = {}

-- The parameter-list constants of XTypes 1.3 §7.4.1.2.2.
M.PID_EXTENDED = 0x3F01
M.PID_LIST_END = 0x3F02
M.PID_SHORT_MAX_ID = 0x3F00
M.PID_IMPL_EXTENSION = 0x8000
M.PID_IGNORE = 0x3F03
M.PID_SHORT_MAX_LEN = 0xFFFF
M.PID_MUST_UNDERSTAND = 0x4000
-- The extended `eMemberHeader`'s flags. §7.4.1.2.1.
local E_FLAG_MUST_UNDERSTAND = 0x40000000
local E_FLAG_IMPL_EXTENSION = 0x80000000
local E_MEMBER_ID_MASK = 0x0FFFFFFF
M.E_FLAG_MUST_UNDERSTAND = E_FLAG_MUST_UNDERSTAND
M.E_FLAG_IMPL_EXTENSION = E_FLAG_IMPL_EXTENSION
M.E_MEMBER_ID_MASK = E_MEMBER_ID_MASK

--- What flags a PL_CDR1 parameter id carries at `position` — "member",
--- "sentinel" or "extended-marker". The measurement lives on
--- `zerodds_ast_layout::PidFlagPolicy`; the policies here are its
--- `as_str` spellings, and an unknown one raises rather than falling
--- back: a typo in a wire-affecting setting must be loud.
function M.pid_flags_at(policy, position)
  if policy == "sentinel-must-understand" then
    if position == "sentinel" or position == "extended-marker" then
      return M.PID_MUST_UNDERSTAND
    end
    return 0
  end
  if policy == "bare" then
    return 0
  end
  if policy == "impl-extension-members" then
    if position == "member" or position == "extended-marker" then
      return M.PID_IMPL_EXTENSION | M.PID_MUST_UNDERSTAND
    end
    return 0
  end
  error(("`%s` is not a PID-flag policy"):format(tostring(policy)), 0)
end

-- Every `wstring` length convention, as `WstringLengthPolicy::as_str`
-- spells it, each cell answering three questions: does the count count
-- characters, is there a terminator, and how wide is a unit. One table
-- for the names and the coordinate spellings both, so a cell cannot
-- answer one question and be forgotten by another.
local WSTRING_CELLS = {
  ["specification"] = { false, false, 2 },
  ["character-count"] = { true, false, 2 },
  ["character-count-terminated"] = { true, true, 4 },
  ["specification-wide-units"] = { false, false, 4 },
  ["ros-type-support"] = { true, false, 4 },
  ["octets/terminated/2"] = { false, true, 2 },
  ["octets/terminated/4"] = { false, true, 4 },
  ["characters/terminated/2"] = { true, true, 2 },
  -- The coordinate spellings of the named cells, so a cell measured and
  -- not yet named is still reachable.
  ["octets/unterminated/2"] = { false, false, 2 },
  ["characters/unterminated/2"] = { true, false, 2 },
  ["characters/terminated/4"] = { true, true, 4 },
  ["octets/unterminated/4"] = { false, false, 4 },
  ["characters/unterminated/4"] = { true, false, 4 },
}
M.WSTRING_CELLS = WSTRING_CELLS

local WSTRING_UNIT_ORDERS = {
  ["stream"] = true,
  ["big-endian-always"] = true,
  ["byte-order-mark"] = true,
  ["host-unswapped"] = true,
}
M.WSTRING_UNIT_ORDERS = WSTRING_UNIT_ORDERS

-- The UTF-16 byte-order mark. `U+FFFE` is not a character: a mark that
-- reads as one says the units behind it are byte-swapped.
M.BYTE_ORDER_MARK = 0xFEFF
M.SWAPPED_BYTE_ORDER_MARK = 0xFFFE

--- A writer in `endianness` ("le" or "be") whose alignment is capped at
--- `max_align` — four for XCDR2, eight for XCDR1 (XTypes 1.3 §7.4.1:
--- `ALIGN(N)` aligns to `MIN(N, XCDR.maxalign)`).
---
--- `framing` carries the PL_CDR1 knobs, each a measured axis:
--- `pid_flags`, `highest_short_id`, `pad_declared_length`,
--- `pad_declared_optional_length`, `member_must_understand`,
--- `pad_after_value`. `short_length_codes` is the XCDR2 counterpart:
--- whether an LC4 EMHEADER whose member turns out to be 1, 2, 4 or 8
--- bytes is rewritten with a short length code.
function M.writer_new(options)
  return {
    out = {},
    endianness = options.endianness or "le",
    max_align = math.max(1, options.max_align or 8),
    -- Where alignment is counted from. Zero for a sample; a PL_CDR1
    -- parameter value moves it, because a parameter is its own
    -- encapsulated block. XTypes 1.3 §7.4.1.2.
    origin = 0,
    pid_flags = options.pid_flags or "sentinel-must-understand",
    -- The highest member id the short parameter header carries.
    -- §7.4.1.2.2 stops at 0x3F00 inclusive and reserves the rest of the
    -- low fourteen bits; one implementation carries all fourteen. Read
    -- by `M.write_pid`, which refuses an id above it, and by the reader.
    highest_short_id = options.highest_short_id or M.PID_SHORT_MAX_ID,
    pad_declared_length = options.pad_declared_length == true,
    pad_declared_optional_length = options.pad_declared_optional_length == true,
    member_must_understand = options.member_must_understand ~= false,
    pad_after_value = options.pad_after_value ~= false,
    short_length_codes = options.short_length_codes == true,
  }
end

--- How many bytes have been written.
function M.position(w)
  return #w.out
end

--- The bytes, as one Lua string.
function M.finish(w)
  local pieces = {}
  local out = w.out
  local index = 1
  while index <= #out do
    local last = math.min(index + 4095, #out)
    pieces[#pieces + 1] = string.char(table.unpack(out, index, last))
    index = last + 1
  end
  return table.concat(pieces)
end

-- -- alignment -----------------------------------------------------------

--- Pad to `align`, counted from the stream's origin.
function M.align_to(w, align)
  align = math.min(align, w.max_align)
  if align <= 1 then
    return
  end
  local offset = #w.out - w.origin
  local padding = (align - (offset % align)) % align
  for _ = 1, padding do
    w.out[#w.out + 1] = 0
  end
end

-- -- primitives ----------------------------------------------------------

local function refuse(w, expected)
  error(("at %d: expected %s"):format(#w.out, expected), 0)
end

local function check(w, value, low, high, expected)
  if math.type(value) ~= "integer" then
    refuse(w, ("%s, and the value is not a Lua integer"):format(expected))
  end
  if value < low or value > high then
    refuse(w, ("%s in [%d, %d], not %d"):format(expected, low, high, value))
  end
end

--- `unsigned` (a bit pattern whose low `width*8` bits are the value)
--- appended without alignment.
local function raw(w, unsigned, width)
  local out = w.out
  if w.endianness == "le" then
    for index = 0, width - 1 do
      out[#out + 1] = (unsigned >> (index * 8)) & 0xFF
    end
  else
    for index = width - 1, 0, -1 do
      out[#out + 1] = (unsigned >> (index * 8)) & 0xFF
    end
  end
end

--- Align to the value's own width, then write it.
local function write_int(w, unsigned, width)
  M.align_to(w, width)
  raw(w, unsigned, width)
end

--- `boolean`, one byte, 0 or 1.
function M.write_bool(w, value)
  w.out[#w.out + 1] = value and 1 or 0
end

--- `octet` and `uint8`.
function M.write_u8(w, value)
  check(w, value, 0, 0xFF, "an octet")
  w.out[#w.out + 1] = value
end

--- `int8`.
function M.write_i8(w, value)
  check(w, value, -0x80, 0x7F, "an int8")
  w.out[#w.out + 1] = value & 0xFF
end

--- `char`, one byte. The value is the code point — this column holds a
--- `char` member as an integer — and one above 255 is refused rather
--- than truncated: `0xe4` is a different character.
function M.write_char(w, value)
  check(w, value, 0, 0xFF, "a code point below U+0100")
  w.out[#w.out + 1] = value
end

--- `short`.
function M.write_i16(w, value)
  check(w, value, -0x8000, 0x7FFF, "a short")
  write_int(w, value & 0xFFFF, 2)
end

--- `unsigned short`.
function M.write_u16(w, value)
  check(w, value, 0, 0xFFFF, "an unsigned short")
  write_int(w, value, 2)
end

--- `long`.
function M.write_i32(w, value)
  check(w, value, -0x80000000, 0x7FFFFFFF, "a long")
  write_int(w, value & 0xFFFFFFFF, 4)
end

--- `unsigned long`.
function M.write_u32(w, value)
  check(w, value, 0, 0xFFFFFFFF, "an unsigned long")
  write_int(w, value, 4)
end

--- `long long`. Every Lua integer is one.
function M.write_i64(w, value)
  if math.type(value) ~= "integer" then
    refuse(w, "a long long, and the value is not a Lua integer")
  end
  write_int(w, value, 8)
end

--- `unsigned long long`, carried as the 64-bit two's-complement bit
--- pattern — a value at or above 2^63 arrives as the negative integer
--- with the same eight octets, which is exactly what goes out. The
--- descriptor's `wraps = true` is where that is written down.
function M.write_u64(w, value)
  if math.type(value) ~= "integer" then
    refuse(w, "an unsigned long long, and the value is not a Lua integer")
  end
  write_int(w, value, 8)
end

--- `float`, rounded to single precision once, here — string.pack's C
--- cast, which turns an overflow into the IEEE infinity every peer
--- reads back.
function M.write_f32(w, value)
  M.align_to(w, 4)
  local packed = string.pack(w.endianness == "le" and "<f" or ">f", value)
  local out = w.out
  for index = 1, 4 do
    out[#out + 1] = string.byte(packed, index)
  end
end

--- `double`.
function M.write_f64(w, value)
  M.align_to(w, 8)
  local packed = string.pack(w.endianness == "le" and "<d" or ">d", value)
  local out = w.out
  for index = 1, 8 do
    out[#out + 1] = string.byte(packed, index)
  end
end

--- `long double`: sixteen bytes of IEEE 754 binary128, aligned to
--- eight. The value arrives as the decimal text the member holds — a
--- Lua float is a binary64 and cannot carry it — and is rounded once,
--- correctly, in `float128.lua`. The two byte orders are the whole
--- sixteen bytes reversed (CORBA 3.3 part 2 Figure 9.2), not two
--- eight-byte halves swapped in place.
function M.write_f128(w, value)
  if type(value) ~= "string" then
    refuse(w, "a long double as its decimal text")
  end
  local hi, lo = float128.decimal_to_bits(value)
  M.align_to(w, 8)
  local out = w.out
  if w.endianness == "le" then
    for index = 0, 7 do
      out[#out + 1] = (lo >> (index * 8)) & 0xFF
    end
    for index = 0, 7 do
      out[#out + 1] = (hi >> (index * 8)) & 0xFF
    end
  else
    for index = 7, 0, -1 do
      out[#out + 1] = (hi >> (index * 8)) & 0xFF
    end
    for index = 7, 0, -1 do
      out[#out + 1] = (lo >> (index * 8)) & 0xFF
    end
  end
end

-- -- the fixed byte orders a wstring's units may take ---------------------

local function write_u16_fixed(w, value, little)
  M.align_to(w, 2)
  if little then
    w.out[#w.out + 1] = value & 0xFF
    w.out[#w.out + 1] = (value >> 8) & 0xFF
  else
    w.out[#w.out + 1] = (value >> 8) & 0xFF
    w.out[#w.out + 1] = value & 0xFF
  end
end

local function write_u32_fixed(w, value, little)
  M.align_to(w, 4)
  if little then
    for index = 0, 3 do
      w.out[#w.out + 1] = (value >> (index * 8)) & 0xFF
    end
  else
    for index = 3, 0, -1 do
      w.out[#w.out + 1] = (value >> (index * 8)) & 0xFF
    end
  end
end

-- -- characters and strings ----------------------------------------------

--- `wchar`, at `width` octets (2 or 4) and in the shape `framing` does
--- ("fixed", or CORBA's "giop-length-prefixed"). The value is the code
--- point; §7.2.2.2.1.2.5's BMP restriction is a property of the type,
--- so anything above U+FFFF is refused. The four-byte form zero-extends
--- a code unit — RTI's XCDR1, TAO's, `rosidl_typesupport_fastrtps_c`'s.
function M.write_wchar(w, value, width, framing, order)
  check(w, value, 0, 0xFFFF, "a BMP code point")
  if width ~= 2 and width ~= 4 then
    refuse(w, ("a wchar width of 2 or 4, not %s"):format(tostring(width)))
  end
  if framing == "giop-length-prefixed" then
    -- CORBA 3.3 part 2 §9.3.1.6's GIOP 1.2 form: the octet count, then
    -- the code unit's octets, all aligned to one. A scalar cannot carry
    -- a byte-order mark, so every order but "stream" means CORBA's
    -- default — big-endian.
    w.out[#w.out + 1] = width
    local little
    if order ~= "stream" then
      little = false
    else
      little = w.endianness == "le"
    end
    if little then
      for index = 0, width - 1 do
        w.out[#w.out + 1] = (value >> (index * 8)) & 0xFF
      end
    else
      for index = width - 1, 0, -1 do
        w.out[#w.out + 1] = (value >> (index * 8)) & 0xFF
      end
    end
    return
  end
  if framing ~= "fixed" then
    refuse(w, ("a wchar framing, not %s"):format(tostring(framing)))
  end
  if width == 4 then
    M.write_u32(w, value)
  else
    M.write_u16(w, value)
  end
end

--- A string: a four-byte length including the terminator, the UTF-8
--- bytes, then a NUL. XTypes 1.3 §7.4.1.1. The length counts bytes,
--- which `#` already does, and the text is checked to be UTF-8 rather
--- than assumed: a Lua string is a run of octets with no encoding of
--- its own.
function M.write_string(w, value)
  if utf8.len(value) == nil then
    refuse(w, "a UTF-8 string")
  end
  M.write_u32(w, #value + 1)
  local out = w.out
  for index = 1, #value do
    out[#out + 1] = string.byte(value, index)
  end
  out[#out + 1] = 0
end

--- A bounded `string`: the same bytes `M.write_string` writes. The
--- bound is a promise about the source and not a difference on the
--- wire, so this is the only place it can be enforced — refused
--- rather than cut at a character boundary, which would put a sample
--- on the wire nobody wrote.
function M.write_string_bounded(w, value, bound)
  if #value > bound then
    refuse(w, ("at most %d octets in a bounded string, and this one has %d"):format(
      bound, #value))
  end
  M.write_string(w, value)
end

--- The count check every bounded `sequence` makes before a byte of it
--- is written. Table 17 makes `O2.length <= T1.length` the condition
--- for constructing a `sequence<T, N>`, and the wire carries no bound
--- to compare against: a sender whose count already exceeds its own
--- declaration has written a value of a type nobody declared.
function M.check_sequence_bound(w, actual, bound)
  if actual > bound then
    refuse(w, ("at most %d elements in a bounded sequence, and this one has %d"):format(
      bound, actual))
  end
end

--- The count check every bounded `map` makes before a byte of it is
--- written. Its own message and not `M.check_sequence_bound`'s: the
--- unit is an entry and not an element, per XTypes 1.3 §7.4.1.1.4's
--- map-as-a-sequence-of-pairs equivalence.
function M.check_map_bound(w, actual, bound)
  if actual > bound then
    refuse(w, ("at most %d entries in a bounded map, and this one has %d"):format(
      bound, actual))
  end
end

--- The UTF-16 code units of `text`, without a BOM. §7.4.1.1.2 counts a
--- `wstring` in code units, so a code point outside the BMP is two.
local function utf16_units_of(w, text)
  local units = {}
  if utf8.len(text) == nil then
    refuse(w, "a UTF-8 wstring holder")
  end
  for _, point in utf8.codes(text) do
    if point >= 0xD800 and point <= 0xDFFF then
      refuse(w, "a Unicode code point, not a lone surrogate")
    end
    if point < 0x10000 then
      units[#units + 1] = point
    else
      local offset = point - 0x10000
      units[#units + 1] = 0xD800 + (offset >> 10)
      units[#units + 1] = 0xDC00 + (offset & 0x3FF)
    end
  end
  return units
end

local function write_wstring_unit(w, unit, width, order)
  local little = order == "byte-order-mark" or order == "host-unswapped"
  if width == 4 then
    if order == "big-endian-always" then
      write_u32_fixed(w, unit, false)
    elseif little then
      write_u32_fixed(w, unit, true)
    else
      M.write_u32(w, unit)
    end
  elseif order == "big-endian-always" then
    write_u16_fixed(w, unit, false)
  elseif little then
    write_u16_fixed(w, unit, true)
  else
    M.write_u16(w, unit)
  end
end

--- A `wstring`, in the form `policy` and `order` name. Three separate
--- questions — which units go out, what the length counts them in, and
--- what byte order each lands in — and the answers are
--- `WstringLengthPolicy`'s and `WstringUnitOrder`'s measured cells.
--- Under "specification" that is a four-byte octet count, UTF-16 code
--- units, no BOM and no terminator (§7.4.1.1.2 forbids both outright).
function M.write_wstring_utf8(w, value, policy, order)
  policy = policy or "specification"
  order = order or "stream"
  local cell = WSTRING_CELLS[policy]
  if cell == nil then
    refuse(w, ("a wstring length policy, not %s"):format(tostring(policy)))
  end
  if not WSTRING_UNIT_ORDERS[order] then
    refuse(w, ("a wstring unit order, not %s"):format(tostring(order)))
  end
  local counts_characters, terminated, width = cell[1], cell[2], cell[3]
  if counts_characters and width == 2 then
    -- A character count over two-octet units has no way to carry a
    -- code point outside the BMP: Fast CDR narrows it to its low
    -- sixteen bits and a different character arrives. The refusal is
    -- on the pair, not on a named form.
    for _, point in utf8.codes(value) do
      if point > 0xFFFF then
        refuse(
          w,
          "a code point inside the BMP: a character count over two-octet "
            .. "units has no way to carry one outside it"
        )
      end
    end
  end
  local units = utf16_units_of(w, value)
  if terminated then
    -- CORBA §9.3.1.6's GIOP 1.1 rule, and RTI's XCDR1 form: a NUL code
    -- unit at the end, inside whatever the length counts.
    units[#units + 1] = 0
  end
  -- An empty `wstring` carries no mark: a mark on one would be a
  -- two-octet string decoding to nothing, and no peer writes it.
  if order == "byte-order-mark" and #units > 0 then
    table.insert(units, 1, M.BYTE_ORDER_MARK)
  end
  M.write_u32(w, counts_characters and #units or #units * width)
  for index = 1, #units do
    write_wstring_unit(w, units[index], width, order)
  end
end

--- A bounded `wstring`, as `M.write_string_bounded` is to
--- `M.write_string`. The bound counts UTF-16 code units, not
--- characters and not octets, whatever `policy` puts on the wire.
function M.write_wstring_bounded(w, value, bound, policy, order)
  local units = #utf16_units_of(w, value)
  if units > bound then
    refuse(w, ("at most %d UTF-16 code units in a bounded wstring, and this one has %d")
      :format(bound, units))
  end
  M.write_wstring_utf8(w, value, policy, order)
end

--- A `wstring` whose characters go out as their own UTF-8 octets.
--- `WstringCodeUnits::Utf8`: the declared length is the UTF-8 octet count
--- and only C2 -- the terminator -- still reaches the wire.
--- `wstring-member` is `04 00 00 00 61 62 63 00` for `"abc"`. A value
--- that is not UTF-8 is refused rather than replaced.
function M.write_wstring_utf8_octets(w, value, policy)
  policy = policy or "specification"
  local cell = WSTRING_CELLS[policy]
  if cell == nil then
    refuse(w, ("a wstring length policy, not %s"):format(tostring(policy)))
  end
  if utf8.len(value) == nil then
    refuse(w, "a UTF-8 wstring")
  end
  local terminated = cell[2]
  M.write_u32(w, #value + (terminated and 1 or 0))
  local out = w.out
  for index = 1, #value do
    out[#out + 1] = string.byte(value, index)
  end
  if terminated then
    out[#out + 1] = 0
  end
end

--- A bounded UTF-8-octet `wstring`, as `M.write_wstring_bounded` is to
--- `M.write_wstring_utf8`: the bound still counts UTF-16 code units,
--- which is what every `wstring<N>` bound means, while the wire carries
--- the UTF-8 octets.
function M.write_wstring_utf8_octets_bounded(w, value, bound, policy)
  local units = #utf16_units_of(w, value)
  if units > bound then
    refuse(w, ("at most %d UTF-16 code units in a bounded wstring, and this one has %d")
      :format(bound, units))
  end
  M.write_wstring_utf8_octets(w, value, policy)
end

-- -- fixed ---------------------------------------------------------------

--- The BCD nibbles of `text` under `fixed<digits, scale>`. [CDR]
--- §9.3.2.8: digit nibbles most-significant first, then a sign nibble —
--- 0xC for positive or zero, 0xD for negative — with a leading zero
--- nibble prepended when the count is odd. `count` is "declared" —
--- `digits` nibbles whatever the value (omniORB, and this project) —
--- "value-precision", as many as the value has and never fewer than
--- one (JacORB 3.9) — or "value-string-digits", the digits of `text` as
--- written made odd by one leading zero (ACE/TAO 2.5.24's
--- `ACE_CDR::Fixed::from_string` (OpenDDS writes the declared count): `0.12345` under
--- `fixed<5,5>` is `0012345c`; past 31 digits it is refused).
function M.fixed_nibbles(w, text, digits, scale, count)
  if digits == 0 then
    refuse(w, "a fixed with at least one digit")
  end
  -- OMG IDL 4.2 rule (42) caps the BCD rule at 31 digits, and
  -- fixed<31, S> is the widest declaration IDL admits. A wider one has
  -- no packing defined for it, so it is refused rather than packed into
  -- octets no reader can be told the width of.
  if digits > 31 then
    refuse(w, ("a fixed of at most 31 digits, not %d"):format(digits))
  end
  if scale > digits then
    refuse(w, "a scale no larger than the digit count")
  end
  local negative = false
  local magnitude = text
  local sign = string.sub(text, 1, 1)
  if sign == "-" then
    negative = true
    magnitude = string.sub(text, 2)
  elseif sign == "+" then
    magnitude = string.sub(text, 2)
  end
  local whole, fraction = string.match(magnitude, "^(%d*)%.(%d*)$")
  if whole == nil then
    whole, fraction = magnitude, ""
  end
  if (whole .. fraction) == "" or string.match(whole .. fraction, "^%d*$") == nil then
    refuse(w, ("a decimal that fits fixed<%d, %d>, not `%s`"):format(digits, scale, text))
  end
  local as_written = #whole + scale
  local integral_width = digits - scale
  whole = string.match(whole, "^0*(.-)$")
  if #whole > integral_width then
    refuse(w, ("a decimal whose integer part fits %d digits, not `%s`"):format(integral_width, text))
  end
  if #fraction > scale then
    refuse(w, ("a decimal whose fraction fits %d digits, not `%s`"):format(scale, text))
  end
  if count ~= "declared" and count ~= "value-precision" and count ~= "value-string-digits" then
    refuse(w, ("a fixed digit count, not `%s`"):format(tostring(count)))
  end
  -- The full `digits`-wide run first; the value's precision is that
  -- run with its leading zeros removed, so both counts are read off
  -- one construction.
  local every = {}
  for _ = 1, integral_width - #whole do
    every[#every + 1] = 0
  end
  for index = 1, #whole do
    every[#every + 1] = string.byte(whole, index) - 0x30
  end
  for index = 1, #fraction do
    every[#every + 1] = string.byte(fraction, index) - 0x30
  end
  for _ = 1, scale - #fraction do
    every[#every + 1] = 0
  end
  local significant = 0
  local seen = false
  for index = 1, #every do
    if seen or every[index] ~= 0 then
      seen = true
      significant = significant + 1
    end
  end
  local wire
  if count == "value-precision" then
    wire = math.max(significant, 1)
  elseif count == "value-string-digits" then
    -- ACE/TAO 2.5.24: the integer digits as written plus S, made odd by one leading
    -- zero. Past 31 no reader can be told the field's width.
    wire = as_written + ((as_written % 2 == 0) and 1 or 0)
    if wire > 31 then
      refuse(w, ("a fixed of at most 31 digits on the wire, not %d for `%s`"):format(wire, text))
    end
  else
    wire = #every
  end
  local nibbles = {}
  if (wire + 1) % 2 == 1 then
    nibbles[#nibbles + 1] = 0
  end
  -- A count wider than P is filled with leading zeros.
  local kept = math.min(wire, #every)
  for _ = 1, wire - kept do
    nibbles[#nibbles + 1] = 0
  end
  for index = #every - kept + 1, #every do
    nibbles[#nibbles + 1] = every[index]
  end
  -- A negative zero is written positive, because there is one zero.
  local any = false
  for index = 1, #every do
    if every[index] ~= 0 then
      any = true
      break
    end
  end
  nibbles[#nibbles + 1] = (negative and any) and 0x0D or 0x0C
  return nibbles
end

--- `fixed<digits, scale>` as packed BCD. DDS-XTypes 1.3 carries no
--- fixed-point kind at all, so the form is CORBA's — the only wire
--- definition an IDL `fixed` has. The value arrives as a decimal
--- string, because a double cannot carry `fixed<31, 2>`.
function M.write_fixed(w, value, digits, scale, count)
  local nibbles = M.fixed_nibbles(w, value, digits, scale, count or "declared")
  for index = 1, #nibbles, 2 do
    w.out[#w.out + 1] = (nibbles[index] << 4) | nibbles[index + 1]
  end
end

-- -- the bit-bounded kinds -----------------------------------------------

--- An enumerator, as the integer its bit bound calls for. §7.4.3.5.3
--- rule (5): the holder is Int8, Int16 or Int32 by `@bit_bound` — a
--- **signed** holder, so `@value(-1)` at the default bound goes out as
--- `ffffffff`. What fits is everything expressible in the bound's
--- bits, signed or unsigned: `-2^(n-1) ..= 2^n - 1`.
---
--- Takes the bound first, matching the order the shared walk hands the
--- arguments in.
function M.write_enum(w, bit_bound, value)
  if bit_bound == 0 or bit_bound > 32 then
    refuse(w, ("an @bit_bound between 1 and 32, not %d"):format(bit_bound))
  end
  if value < -(1 << (bit_bound - 1)) or value > (1 << bit_bound) - 1 then
    refuse(w, ("an enumerator a %d-bit holder can hold, not %d"):format(bit_bound, value))
  end
  local mask = (1 << bit_bound) - 1
  local pattern = value & mask
  if bit_bound <= 8 then
    M.write_u8(w, pattern)
  elseif bit_bound <= 16 then
    M.write_u16(w, pattern)
  else
    M.write_u32(w, pattern)
  end
end

--- A bitmask, as the unsigned integer its bit bound calls for. Table 33
--- of §7.4.1.1.3.2: UInt8 to a bound of 8, UInt16 to 16, UInt32 to 32,
--- UInt64 to 64, bit `n` always `1 << n`.
function M.write_bitmask(w, bit_bound, value)
  if bit_bound == 0 or bit_bound > 64 then
    refuse(w, ("an @bit_bound between 1 and 64, not %d"):format(bit_bound))
  end
  if bit_bound < 64 and (value < 0 or value >= (1 << bit_bound)) then
    refuse(w, ("a bitmask below 2^%d, not %d"):format(bit_bound, value))
  end
  if bit_bound <= 8 then
    M.write_u8(w, value)
  elseif bit_bound <= 16 then
    M.write_u16(w, value)
  elseif bit_bound <= 32 then
    M.write_u32(w, value)
  else
    M.write_u64(w, value)
  end
end

--- One bitfield, shifted into the accumulator a bitset packs into. A
--- value wider than the field is refused rather than masked: masking
--- would put a different number on the wire than the caller asked for.
function M.bitfield(w, value, width, position)
  if width <= 0 or width > 64 or position < 0 or position + width > 64 then
    refuse(w, ("a bitfield of %d bits at %d, inside 64"):format(width, position))
  end
  if width < 64 then
    local limit = (1 << width) - 1
    if value < 0 or value > limit then
      refuse(w, ("a value inside %d bits, not %d"):format(width, value))
    end
  end
  return value << position
end

--- A bitset, as the anchor integer its bitfields pack into. DDS-XTypes
--- 1.3 has no serialisation rule for a bitset; the anchor width is the
--- layout planner's answer and arrives as a number.
function M.write_bitset(w, anchor, value)
  if anchor == 1 then
    M.write_u8(w, value)
  elseif anchor == 2 then
    M.write_u16(w, value)
  elseif anchor == 4 then
    M.write_u32(w, value)
  elseif anchor == 8 then
    M.write_u64(w, value)
  else
    refuse(w, ("a bitset anchor of 1, 2, 4 or 8 bytes, not %s"):format(tostring(anchor)))
  end
end

--- The Cyclone union overlay: the low `octets` of `pattern`, written as
--- one unsigned integer of that width in the stream's byte order. The
--- `bit_pattern_*` functions produce the pattern.
function M.write_union_overlay(w, pattern, octets)
  if octets == 1 then
    M.write_u8(w, pattern & 0xFF)
  elseif octets == 2 then
    M.write_u16(w, pattern & 0xFFFF)
  elseif octets == 4 then
    M.write_u32(w, pattern & 0xFFFFFFFF)
  elseif octets == 8 then
    M.write_u64(w, pattern)
  else
    refuse(w, ("a union arm of 1, 2, 4 or 8 octets, not %s"):format(tostring(octets)))
  end
end

--- The low `octets` of `value`'s two's-complement bits, zero-extended.
--- The mask is the function: `-2` in four octets has to come out
--- `0x00000000FFFFFFFE`.
function M.bit_pattern_int(value, octets)
  if octets < 8 then
    return value & ((1 << (octets * 8)) - 1)
  end
  return value
end

--- One octet, 0 or 1 — the boolean overlay pattern.
function M.bit_pattern_bool(value)
  return value and 1 or 0
end

--- The IEEE 754 binary32 bits, zero-extended.
function M.bit_pattern_f32(value)
  return (string.unpack("<I4", string.pack("<f", value)))
end

--- The IEEE 754 binary64 bits, as the 64-bit pattern.
function M.bit_pattern_f64(value)
  return (string.unpack("<i8", string.pack("<d", value)))
end

-- -- framing -------------------------------------------------------------

--- Patch `value` into the already-written bytes at 0-based `at`.
local function patch(w, at, value, width)
  if value < 0 or (width < 8 and value >= (1 << (width * 8))) then
    error(("at %d: a %d-byte length, not %d"):format(at, width, value), 0)
  end
  if w.endianness == "le" then
    for index = 0, width - 1 do
      w.out[at + 1 + index] = (value >> (index * 8)) & 0xFF
    end
  else
    for index = 0, width - 1 do
      w.out[at + 1 + index] = (value >> ((width - 1 - index) * 8)) & 0xFF
    end
  end
end

--- A DHEADER: a four-byte length the members that follow occupy,
--- written as a placeholder and patched when they are done.
function M.write_dheader(w)
  M.align_to(w, 4)
  local at = #w.out
  for _ = 1, 4 do
    w.out[#w.out + 1] = 0
  end
  return { at = at }
end

--- The DHEADER a value that takes no delimiter does not write.
--- §7.4.3.5.3 gives the delimiter to `@appendable` and `@mutable` and
--- not to `@final`; the shared walk declares one handle per value and
--- finishes it either way, so the absence is a value and `finish`
--- ignores it.
function M.no_dheader()
  return { at = -1 }
end

--- Patch a DHEADER with the length of what followed it.
function M.finish_dheader(w, header)
  if header.at < 0 then
    return
  end
  patch(w, header.at, #w.out - header.at - 4, 4)
end

--- An XCDR2 EMHEADER: must-understand, the length code and the member
--- id in one 32-bit word (§7.4.3.4.2). LC4 is followed by a NEXTINT
--- placeholder; LC5 to LC7 reuse the member's own leading word as their
--- NEXTINT and reserve nothing.
function M.write_emheader(w, member_id, length_code, must_understand)
  -- §7.2.2.4.4.4.4 runs member ids from 0 to 0x0FFFFFFF and §7.4.3.4.2
  -- leaves them the low 28 bits of the word. Masking a larger one writes
  -- a well-formed header for `id & 0x0FFFFFFF`: the peer reads a member
  -- the caller did not name, and nothing at either end says so.
  check(w, member_id, 0, E_MEMBER_ID_MASK,
    "a member id inside the 28 bits of an EMHEADER")
  M.align_to(w, 4)
  local word = ((must_understand and 1 or 0) << 31)
    | ((length_code & 0x7) << 28)
    | (member_id & E_MEMBER_ID_MASK)
  M.write_u32(w, word)
  local word_at = #w.out - 4
  if length_code == 4 then
    local at = #w.out
    for _ = 1, 4 do
      w.out[#w.out + 1] = 0
    end
    return { next_int = at, word = word_at, length_code = length_code }
  end
  return { next_int = false, word = word_at, length_code = length_code }
end

--- Patch an EMHEADER's NEXTINT with the length of what followed. Under
--- `short_length_codes` an LC4 header whose member turned out to be 1,
--- 2, 4 or 8 bytes instead loses its NEXTINT and states the length in
--- the code — what Fast DDS does. Safe only under XCDR2, whose
--- MAXALIGN is four: the value began four-aligned with the NEXTINT in
--- front and begins four-aligned without it.
function M.finish_emheader(w, header)
  if header.next_int == false or header.next_int == nil then
    -- Under codes 0 to 3 the code *is* the length — 1, 2, 4 or 8
    -- octets, §7.4.3.4.2. A value of another size misdeclares it and
    -- the reader's skip lands inside data, so the mismatch is refused
    -- here rather than written. Codes 5 to 7 declare nothing of their
    -- own: the member's leading length word is the NEXTINT.
    local code = header.length_code
    if code ~= nil and code >= 0 and code <= 3 and header.word + 4 <= #w.out then
      local declared = 1 << code
      local body = #w.out - header.word - 4
      if body ~= declared then
        refuse(w, ("a member of %d octets under length code %d, not %d"):format(
          declared, code, body))
      end
    end
    return
  end
  local length = #w.out - header.next_int - 4
  if w.short_length_codes and (length == 1 or length == 2 or length == 4 or length == 8) then
    local codes = { [1] = 0, [2] = 1, [4] = 2, [8] = 3 }
    local code = codes[length]
    for _ = 1, 4 do
      table.remove(w.out, header.next_int + 1)
    end
    local previous = 0
    if w.endianness == "le" then
      for index = 3, 0, -1 do
        previous = (previous << 8) | w.out[header.word + 1 + index]
      end
    else
      for index = 0, 3 do
        previous = (previous << 8) | w.out[header.word + 1 + index]
      end
    end
    patch(w, header.word, (previous & ~(0x7 << 28)) | (code << 28), 4)
    return
  end
  patch(w, header.next_int, length, 4)
end

--- Start a parameter's value block: §7.4.1.2 makes each parameter "a
--- CDR-encapsulated block of data", so alignment counts from its own
--- start. Returns the origin it replaces.
local function begin_parameter_value(w)
  local saved = w.origin
  w.origin = #w.out
  return saved
end

-- -- packed collection elements -------------------------------------------

--- `ElementPadding::Packed`: each collection element and each aggregate
--- value counts its alignment from its own first octet instead of from
--- the encapsulation. Returns the origin it replaces, for
--- `M.finish_packed_element`. `array-1d-struct-final` is
--- `01 00 00 00 02 00 03 00 00 00 04 00 77`, the second Cell's `long` at
--- offset six where the clause puts it at eight.
function M.begin_packed_element(w)
  local saved = w.origin
  w.origin = #w.out
  return saved
end

--- Restore the origin `M.begin_packed_element` replaced.
function M.finish_packed_element(w, saved)
  w.origin = saved
end

--- A PL_CDR1 parameter header, short form: `[id:u16][length:u16]`.
--- `must_understand` is bit 14 of the id (§7.4.1.2.2), when the framing
--- writes the member's own flag at all; the framing's per-position
--- flags are ORed on regardless.
function M.write_pid(w, pid, role, must_understand)
  -- §7.4.1.2.2 gives the short form the ids 0 to `highest_short_id`
  -- inclusive and reserves what is above it — PID_EXTENDED is 0x3F01
  -- itself. A short header with a reserved id collides with the
  -- protocol's own parameters, so such an id takes the extended form or
  -- none. This is the write side of the constant the reader has always
  -- consulted.
  check(w, pid, 0, w.highest_short_id,
    "a parameter id the short header carries — the extended header takes what is above")
  M.align_to(w, 4)
  if must_understand and w.member_must_understand then
    pid = pid | M.PID_MUST_UNDERSTAND
  end
  pid = pid | M.pid_flags_at(w.pid_flags, "member")
  local bare = pid & ~(M.PID_MUST_UNDERSTAND | M.PID_IMPL_EXTENSION)
  M.write_u16(w, pid)
  local at = #w.out
  w.out[#w.out + 1] = 0
  w.out[#w.out + 1] = 0
  return {
    at = at,
    extended = false,
    saved_origin = begin_parameter_value(w),
    role = role or "member",
    pid = bare,
    must_understand = must_understand == true,
  }
end

--- A PL_CDR1 parameter header, extended form:
--- `[PID_EXTENDED][8][id:u32][length:u32]`. The member's
--- must-understand is `FLAG_2` of the `eMemberHeader` (§7.4.1.2.1) and
--- not the marker's bit above it.
function M.write_pid_extended(w, pid, role, must_understand)
  -- The same 28 bits as an EMHEADER's, here in the `eMemberHeader` of
  -- §7.4.1.2.1. Masking aliases the parameter onto another member
  -- exactly as it does there.
  check(w, pid, 0, E_MEMBER_ID_MASK,
    "a member id inside the 28 bits of an eMemberHeader")
  M.align_to(w, 4)
  M.write_u16(w, M.PID_EXTENDED | M.pid_flags_at(w.pid_flags, "extended-marker"))
  M.write_u16(w, 8)
  local member_header = pid & E_MEMBER_ID_MASK
  if must_understand and w.member_must_understand then
    member_header = member_header | E_FLAG_MUST_UNDERSTAND
  end
  M.write_u32(w, member_header)
  local at = #w.out
  for _ = 1, 4 do
    w.out[#w.out + 1] = 0
  end
  return {
    at = at,
    extended = true,
    saved_origin = begin_parameter_value(w),
    role = role or "member",
    pid = pid & E_MEMBER_ID_MASK,
    must_understand = must_understand == true,
  }
end

--- Rewrite a short parameter header as the extended form, for a value
--- whose length overran §7.4.1.2.2's sixteen-bit `ssize`. Eight bytes
--- go in between header and value; XCDR1's MAXALIGN is eight, so every
--- offset inside the value keeps its remainder.
local function promote_pid(w, header, length)
  local start = header.at - 2
  for _ = 1, 8 do
    table.insert(w.out, header.at + 2 + 1, 0)
  end
  patch(w, start, M.PID_EXTENDED | M.pid_flags_at(w.pid_flags, "extended-marker"), 2)
  patch(w, start + 2, 8, 2)
  local member_header = header.pid & E_MEMBER_ID_MASK
  if header.must_understand and w.member_must_understand then
    member_header = member_header | E_FLAG_MUST_UNDERSTAND
  end
  patch(w, start + 4, member_header, 4)
  patch(w, start + 8, length, 4)
end

--- Patch a parameter header with its value's length. Whether the
--- declared length covers the value's trailing padding is the
--- framing's answer, per role; the padding itself is written with the
--- origin already restored, so the next parameter lands four-aligned
--- against the enclosing stream. XTypes 1.3 §7.4.1.2.
function M.finish_pid(w, header)
  local width = header.extended and 4 or 2
  local length = #w.out - header.at - width
  local pads
  if header.role == "optional" then
    pads = w.pad_declared_optional_length
  else
    pads = w.pad_declared_length
  end
  if pads then
    local past = (#w.out - header.saved_origin) % 4
    if past > 0 then
      length = length + 4 - past
    end
  end
  if not header.extended and length > M.PID_SHORT_MAX_LEN then
    promote_pid(w, header, length)
  else
    patch(w, header.at, length, width)
  end
  w.origin = header.saved_origin
  -- Under Fast DDS's framing the parameter writes no padding of its
  -- own: the next header aligns itself, and a final parameter ends the
  -- stream where its value does.
  if w.pad_after_value then
    M.align_to(w, 4)
  end
end

--- The `PID_LIST_END` sentinel a PL_CDR1 parameter list ends with. Its
--- flags come from the framing asked at the "sentinel" position:
--- `PID_LIST_END` is not a member and has no declaration of its own.
function M.write_sentinel(w)
  M.align_to(w, 4)
  M.write_u16(w, M.PID_LIST_END | M.pid_flags_at(w.pid_flags, "sentinel"))
  M.write_u16(w, 0)
end

return M
