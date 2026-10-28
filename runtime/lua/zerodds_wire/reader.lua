-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
-- Taking bytes off the wire.
--
-- The Lua counterpart of `crates/wire/src/reader.rs` and of
-- `runtime/python/zerodds_wire/reader.py`, mirror of `writer.lua`
-- clause for clause. Two kinds of failure, and they are not one:
--
-- * a malformed **stream** raises — a length past the buffer's end is
--   not a state a caller can carry on from — with `error` and a message
--   naming what was expected and where;
-- * a refusal about the **type** sets the reader's status and the
--   generated decoder walks on to the end of the sample, which is what
--   lets one test at the top of the caller decide what to do with it.
--   The statuses are the strings `typesupport.lua` lists.

local float128 = require("zerodds_wire.float128")
local typesupport = require("zerodds_wire.typesupport")
local writer = require("zerodds_wire.writer")

local M = {}

-- The low fourteen bits of a short parameter id, which are the id.
local PID_ID_MASK = 0x3FFF

--- A reader over `data` (a Lua string), capped at `max_align` — four
--- for XCDR2, eight for XCDR1, the writer's counterpart. `pid_flags`
--- and `highest_short_id` are the writer's framing, which the reader
--- needs for two things: under "impl-extension-members" the
--- FLAG_IMPL_EXTENSION bit carries no information, and one
--- implementation carries short ids past §7.4.1.2.2's 0x3F00.
function M.reader_new(data, endianness, max_align, pid_flags, highest_short_id)
  return {
    data = data,
    at = 0,
    endianness = endianness or "le",
    max_align = math.max(1, max_align or 8),
    origin = 0,
    pid_flags = pid_flags or "sentinel-must-understand",
    highest_short_id = highest_short_id or writer.PID_SHORT_MAX_ID,
    -- Nothing is refused until something refuses it: the generated
    -- decoder has to reach the end of the type to know how much of the
    -- stream the sample occupied.
    status = "ok",
  }
end

-- -- the status a generated decoder reads --------------------------------

--- How the read is going.
function M.reader_status(r)
  return r.status
end

--- Refuse the stream from outside the runtime. The first refusal
--- sticks: everything read after it sits where no writer need have put
--- anything, and a second refusal would describe that rather than the
--- fault. `"ok"` refuses nothing.
function M.reader_refuse(r, refusal)
  if refusal ~= "ok" and r.status == "ok" then
    r.status = refusal
  end
end

--- Take back a `"discarded"`, and only that one: a member whose
--- `@try_construct` is USE_DEFAULT but which carries an explicit
--- `@default` is answered at the call site (§7.2.2.4.4.4.9 puts the
--- declared literal first), so the site asks for DISCARD and clears it
--- here. Every other status stays — a truncated stream is not a
--- narrowing.
function M.reader_clear_discard(r)
  if r.status == "discarded" then
    r.status = "ok"
    return true
  end
  return false
end

--- Where it is.
function M.reader_position(r)
  return r.at
end

--- How many bytes are left.
function M.reader_remaining(r)
  return math.max(0, #r.data - r.at)
end

--- Move the cursor forward to `target`, which is never backwards: a
--- framing word pointing behind the cursor means the member would be
--- read twice, and that is a refusal about the stream's shape. Past
--- the end raises — there is no position there to carry on from.
function M.reader_skip_to(r, target)
  if r.status ~= "ok" then
    return
  end
  if target < r.at then
    M.reader_refuse(r, "out-of-range")
    return
  end
  if target > #r.data then
    error(("at %d: %d bytes needed, %d available"):format(r.at, target - r.at, M.reader_remaining(r)), 0)
  end
  r.at = target
end

--- The cursor, the origin and the status, to be put back later. One
--- caller: a `map` under `rule-15-omits`, whose entries are read once
--- to count them and again into storage.
function M.reader_save(r)
  return { at = r.at, origin = r.origin, status = r.status }
end

--- Put back what `reader_save` took.
function M.reader_restore(r, mark)
  r.at = mark.at
  r.origin = mark.origin
  r.status = mark.status
end

--- The read-side pair of `M.begin_packed_element`: move the origin to the
--- current position and answer the one replaced, for
--- `M.reader_finish_packed_element`.
function M.reader_begin_packed_element(r)
  local saved = r.origin
  r.origin = r.at
  return saved
end

--- Restore the origin `M.reader_begin_packed_element` replaced.
function M.reader_finish_packed_element(r, saved)
  r.origin = saved
end

-- -- primitives ----------------------------------------------------------

--- Skip padding to `align`, counted from the stream's origin.
function M.align_to(r, align)
  align = math.min(align, r.max_align)
  if align <= 1 then
    return
  end
  local offset = r.at - r.origin
  r.at = r.at + (align - (offset % align)) % align
end

local function take(r, count)
  local ending = r.at + count
  if ending > #r.data then
    error(("at %d: %d bytes needed, %d available"):format(r.at, count, M.reader_remaining(r)), 0)
  end
  local chunk = string.sub(r.data, r.at + 1, ending)
  r.at = ending
  return chunk
end

local function refuse_here(r, expected)
  error(("at %d: expected %s"):format(r.at, expected), 0)
end

local function impossible(r, at, claimed)
  error(("at %d: a length of %d with %d bytes left"):format(at, claimed, M.reader_remaining(r)), 0)
end

--- The next `width` bytes as an unsigned pattern, aligned to `width`.
local function read_uint(r, width)
  M.align_to(r, width)
  local chunk = take(r, width)
  local value = 0
  if r.endianness == "le" then
    for index = width, 1, -1 do
      value = (value << 8) | string.byte(chunk, index)
    end
  else
    for index = 1, width do
      value = (value << 8) | string.byte(chunk, index)
    end
  end
  return value
end

local function sign_extend(value, bits)
  local sign = 1 << (bits - 1)
  if value >= sign then
    return value - (1 << bits)
  end
  return value
end

--- `boolean`. A byte that is neither 0 nor 1 is a stream that
--- disagrees with its own type, and is refused rather than coerced.
function M.read_bool(r)
  local at = r.at
  local value = string.byte(take(r, 1))
  if value == 0 then
    return false
  end
  if value == 1 then
    return true
  end
  error(("at %d: expected a boolean of 0 or 1"):format(at), 0)
end

--- `octet` and `uint8`.
function M.read_u8(r)
  return string.byte(take(r, 1))
end

--- `int8`.
function M.read_i8(r)
  return sign_extend(string.byte(take(r, 1)), 8)
end

--- `char`, one byte, as the code point this column holds a `char` in.
function M.read_char(r)
  return string.byte(take(r, 1))
end

--- `wchar`, at `width` octets and in `framing`'s shape, as the code
--- point. The four-byte fixed form is one UTF-16 code unit
--- zero-extended, and a value using the high bits is refused rather
--- than narrowed. GIOP's length octet is checked against the
--- configured code-set width rather than believed.
function M.read_wchar(r, width, framing, order)
  local at = r.at
  if width ~= 2 and width ~= 4 then
    refuse_here(r, ("a wchar width of 2 or 4, not %s"):format(tostring(width)))
  end
  local value
  if framing == "giop-length-prefixed" then
    local declared = M.read_u8(r)
    if declared ~= width then
      error(
        ("at %d: expected a GIOP 1.2 wchar of %d octets, the configured transmission code set, and not %d"):format(
          at,
          width,
          declared
        ),
        0
      )
    end
    local little
    if order ~= "stream" then
      little = false
    else
      little = r.endianness == "le"
    end
    local chunk = take(r, width)
    value = 0
    if little then
      for index = width, 1, -1 do
        value = (value << 8) | string.byte(chunk, index)
      end
    else
      for index = 1, width do
        value = (value << 8) | string.byte(chunk, index)
      end
    end
  elseif framing ~= "fixed" then
    refuse_here(r, ("a wchar framing, not %s"):format(tostring(framing)))
  elseif width == 4 then
    value = M.read_u32(r)
    if value > 0xFFFF then
      error(("at %d: expected a four-byte wchar holding one UTF-16 code unit"):format(at), 0)
    end
  else
    value = M.read_u16(r)
  end
  if value >= 0xD800 and value <= 0xDFFF then
    error(("at %d: expected a Unicode code point"):format(at), 0)
  end
  return value
end

--- `short`.
function M.read_i16(r)
  return sign_extend(read_uint(r, 2), 16)
end

--- `unsigned short`.
function M.read_u16(r)
  return read_uint(r, 2)
end

--- `long`.
function M.read_i32(r)
  return sign_extend(read_uint(r, 4), 32)
end

--- `unsigned long`.
function M.read_u32(r)
  return read_uint(r, 4)
end

--- `long long`. Eight bytes are a Lua integer as they stand.
function M.read_i64(r)
  return read_uint(r, 8)
end

--- `unsigned long long`, as the 64-bit bit pattern — a value at or
--- above 2^63 comes back as the negative integer with the same octets,
--- which is the holder the descriptor's `wraps = true` announces.
function M.read_u64(r)
  return read_uint(r, 8)
end

--- `float`. The parenthesis is load-bearing: `string.unpack` answers
--- the value *and* the next position, and a bare `return` would hand a
--- caller both.
function M.read_f32(r)
  M.align_to(r, 4)
  return (string.unpack(r.endianness == "le" and "<f" or ">f", take(r, 4)))
end

--- `double`.
function M.read_f64(r)
  M.align_to(r, 8)
  return (string.unpack(r.endianness == "le" and "<d" or ">d", take(r, 8)))
end

--- `long double`: all sixteen bytes as IEEE 754 binary128, answered as
--- the exact decimal text the member holds — the corpus's canonical
--- spelling, `float128.lua`'s `bits_to_decimal`.
function M.read_f128(r)
  M.align_to(r, 8)
  local chunk = take(r, 16)
  local hi, lo = 0, 0
  if r.endianness == "le" then
    for index = 8, 1, -1 do
      lo = (lo << 8) | string.byte(chunk, index)
    end
    for index = 16, 9, -1 do
      hi = (hi << 8) | string.byte(chunk, index)
    end
  else
    for index = 1, 8 do
      hi = (hi << 8) | string.byte(chunk, index)
    end
    for index = 9, 16 do
      lo = (lo << 8) | string.byte(chunk, index)
    end
  end
  return float128.bits_to_decimal(hi, lo)
end

-- -- strings -------------------------------------------------------------

--- A string: a length including the terminator, the bytes, the NUL.
--- The text is checked to be UTF-8: a Lua string is a run of octets,
--- and handing back one that is not UTF-8 would put the failure at the
--- next reader instead of at the stream.
function M.read_string(r)
  local at = r.at
  local length = M.read_u32(r)
  if length == 0 or length > M.reader_remaining(r) + 1 then
    impossible(r, at, length)
  end
  local text = take(r, length - 1)
  if utf8.len(text) == nil then
    error(("at %d: the string is not UTF-8"):format(at), 0)
  end
  M.read_u8(r)
  return text
end

--- The same operation under the name the shared walk calls it by:
--- `runtime/c` has a second, heap-allocating form and a Lua string
--- owns its own storage, so the two names are one function.
function M.read_string_alloc(r)
  return M.read_string(r)
end

local function read_u16_fixed(r, little)
  M.align_to(r, 2)
  local chunk = take(r, 2)
  if little then
    return string.byte(chunk, 1) | (string.byte(chunk, 2) << 8)
  end
  return (string.byte(chunk, 1) << 8) | string.byte(chunk, 2)
end

local function read_u32_fixed(r, little)
  M.align_to(r, 4)
  local chunk = take(r, 4)
  local value = 0
  if little then
    for index = 4, 1, -1 do
      value = (value << 8) | string.byte(chunk, index)
    end
  else
    for index = 1, 4 do
      value = (value << 8) | string.byte(chunk, index)
    end
  end
  return value
end

--- Take the byte-order mark off the front and apply what it said. The
--- units arrived little-endian, because that is what the
--- `byte-order-mark` order reads; a swapped mark swaps every unit
--- behind it back, and no mark at all means CORBA's default —
--- big-endian — so every unit is swapped. The mark is never a
--- character of the returned string.
local function strip_byte_order_mark(units)
  if #units == 0 then
    return units
  end
  if units[1] == writer.BYTE_ORDER_MARK then
    table.remove(units, 1)
    return units
  end
  if units[1] == writer.SWAPPED_BYTE_ORDER_MARK then
    table.remove(units, 1)
  end
  for index = 1, #units do
    local unit = units[index]
    units[index] = ((unit & 0xFF) << 8) | (unit >> 8)
  end
  return units
end

local function wstring_units(r, at, count, width, order)
  if count * width > M.reader_remaining(r) then
    impossible(r, at, count * width)
  end
  local little = order == "byte-order-mark" or order == "host-unswapped"
  local units = {}
  for _ = 1, count do
    local unit
    if width == 4 then
      if order == "big-endian-always" then
        unit = read_u32_fixed(r, false)
      elseif little then
        unit = read_u32_fixed(r, true)
      else
        unit = M.read_u32(r)
      end
      if unit > 0xFFFF then
        error(("at %d: expected a four-byte wstring character holding one UTF-16 unit"):format(at), 0)
      end
    elseif order == "big-endian-always" then
      unit = read_u16_fixed(r, false)
    elseif little then
      unit = read_u16_fixed(r, true)
    else
      unit = M.read_u16(r)
    end
    units[#units + 1] = unit
  end
  return units
end

--- A `wstring`, in the form `policy` and `order` name, answered as
--- UTF-8. The form is never inferred from the stream: choosing would
--- let the one field a hostile peer controls decide how much is read.
--- Under "specification" the count is octets, and one the unit width
--- does not divide is refused rather than rounded down.
function M.read_wstring_utf8(r, policy, order)
  policy = policy or "specification"
  order = order or "stream"
  local at = r.at
  local declared = M.read_u32(r)
  if not writer.WSTRING_UNIT_ORDERS[order] then
    refuse_here(r, ("a wstring unit order, not %s"):format(tostring(order)))
  end
  local cell = writer.WSTRING_CELLS[policy]
  if cell == nil then
    refuse_here(r, ("a wstring length policy, not %s"):format(tostring(policy)))
  end
  local counts_characters, terminated, width = cell[1], cell[2], cell[3]
  local units
  if counts_characters then
    units = wstring_units(r, at, declared, width, order)
  else
    if declared % width ~= 0 then
      impossible(r, at, declared)
    end
    units = wstring_units(r, at, declared // width, width, order)
  end
  if terminated then
    -- The count includes the NUL, so a count of zero is a frame that
    -- does not hold its own terminator.
    if declared < 1 then
      impossible(r, at, declared)
    end
    local last = table.remove(units)
    if last ~= 0 then
      error(("at %d: expected a NUL character ending the wstring, as its count includes one"):format(at), 0)
    end
    -- A NUL inside the text is the narrow string's problem in wide
    -- form: every C peer stops at it. Only where a terminator is
    -- expected — a cell without one may carry U+0000 as a character.
    for index = 1, #units do
      if units[index] == 0 then
        error(("at %d: expected no NUL inside a wstring a C peer would stop at"):format(at), 0)
      end
    end
  end
  if order == "byte-order-mark" then
    units = strip_byte_order_mark(units)
  end
  local out = {}
  local index = 1
  while index <= #units do
    local unit = units[index]
    if unit >= 0xD800 and unit <= 0xDBFF and index + 1 <= #units then
      local low = units[index + 1]
      if low >= 0xDC00 and low <= 0xDFFF then
        out[#out + 1] = utf8.char(0x10000 + ((unit - 0xD800) << 10) + (low - 0xDC00))
        index = index + 2
        goto continue
      end
    end
    if unit >= 0xD800 and unit <= 0xDFFF then
      error(("at %d: expected a UTF-16 sequence with no lone surrogate"):format(at), 0)
    end
    out[#out + 1] = utf8.char(unit)
    index = index + 1
    ::continue::
  end
  return table.concat(out)
end

--- The inverse of `M.write_wstring_utf8_octets`: the declared length is
--- the UTF-8 octet count, the terminator `policy` asks for is the last of
--- those octets, and a count the stream cannot hold, a missing or an
--- interior NUL, or a value that is not UTF-8 is refused.
function M.read_wstring_utf8_octets(r, policy)
  policy = policy or "specification"
  local at = r.at
  local declared = M.read_u32(r)
  if declared > M.reader_remaining(r) then
    impossible(r, at, declared)
  end
  local cell = writer.WSTRING_CELLS[policy]
  if cell == nil then
    refuse_here(r, ("a wstring length policy, not %s"):format(tostring(policy)))
  end
  local terminated = cell[2]
  local raw = take(r, declared)
  local body = raw
  if terminated then
    if declared < 1 or string.byte(raw, declared) ~= 0 then
      error(("at %d: expected a NUL octet ending the wstring, as its count includes one"):format(at), 0)
    end
    body = string.sub(raw, 1, declared - 1)
    if string.find(body, "\0", 1, true) ~= nil then
      error(("at %d: expected no NUL inside a wstring a C peer would stop at"):format(at), 0)
    end
  end
  if utf8.len(body) == nil then
    error(("at %d: expected the wstring is not UTF-8"):format(at), 0)
  end
  return body
end

--- `fixed<digits, scale>`, as the decimal string the writer took —
--- leading zeros gone and no sign on a zero, which is the canonical
--- spelling every column answers. Under "value-precision" the field's
--- end is read out of the octets: the field ends at the first octet
--- whose low nibble is a sign, within the sixteen octets of the widest
--- `fixed` IDL admits.
function M.read_fixed(r, digits, scale, count)
  count = count or "declared"
  if count ~= "declared" and count ~= "value-precision" and count ~= "value-string-digits" then
    refuse_here(r, ("a fixed digit count, not `%s`"):format(tostring(count)))
  end
  -- The same bound as the writer's, on this side too: OMG IDL 4.2 rule
  -- (42) caps the BCD rule at 31 digits, and a reader asked for a wider
  -- field would take octets no declaration can name.
  if digits <= 0 or digits > 31 or scale > digits then
    refuse_here(r, ("a fixed<P, S> with S <= P and 1 <= P <= 31, not fixed<%d, %d>"):format(
      digits, scale))
  end
  local nibbles = {}
  local sign
  if count == "value-precision" or count == "value-string-digits" then
    while true do
      if #nibbles >= 32 then
        refuse_here(r, "a sign nibble within sixteen octets")
      end
      local byte = string.byte(take(r, 1))
      nibbles[#nibbles + 1] = byte >> 4
      nibbles[#nibbles + 1] = byte & 0x0F
      if (byte & 0x0F) >= 0x0A then
        break
      end
    end
    sign = table.remove(nibbles)
  else
    local chunk = take(r, (digits + 2) // 2)
    for index = 1, #chunk do
      local byte = string.byte(chunk, index)
      nibbles[#nibbles + 1] = byte >> 4
      nibbles[#nibbles + 1] = byte & 0x0F
    end
    sign = table.remove(nibbles)
    if (digits + 1) % 2 == 1 then
      table.remove(nibbles, 1)
    end
  end
  local text = {}
  for index = 1, #nibbles do
    if nibbles[index] > 9 then
      refuse_here(r, "BCD digits")
    end
    text[index] = string.char(0x30 + nibbles[index])
  end
  text = table.concat(text)
  -- The point sits `scale` digits from the right: under the value's
  -- count the field may carry fewer than `digits`, and one carrying
  -- fewer than the scale needs its integer part written as a zero.
  while #text < scale + 1 do
    text = "0" .. text
  end
  local split = #text - scale
  local whole = string.match(string.sub(text, 1, split), "^0*(.-)$")
  if whole == "" then
    whole = "0"
  end
  local fraction = string.sub(text, split + 1)
  local out
  if scale == 0 then
    out = whole
  else
    out = whole .. "." .. fraction
  end
  local zero = whole == "0" and string.match(fraction, "^0*$") ~= nil
  if sign == 0x0D and not zero then
    return "-" .. out
  end
  return out
end

-- -- the trims a bound reaches for ---------------------------------------

--- `text` cut to at most `octets` bytes, never inside a character. The
--- reader-received form of `typesupport.trim_utf8`, reached as an
--- operation on the reader because that is how the shared walk emits
--- the call.
function M.trim_utf8(r, text, octets)
  local _ = r
  return typesupport.trim_utf8(text, octets)
end

--- `text` cut to at most `units` UTF-16 code units.
function M.trim_utf16(r, text, units)
  local _ = r
  return typesupport.trim_utf16(text, units)
end

-- -- the bit-bounded kinds -----------------------------------------------

--- An enumerator's holder, as the signed integer §7.4.3.5.3 makes it —
--- sign-extended from the **holder's** width, not the bound's:
--- `@bit_bound(3)` is carried in an Int8, so the sign bit is bit 7.
function M.read_enum(r, bit_bound)
  if bit_bound == 0 or bit_bound > 32 then
    refuse_here(r, ("an @bit_bound between 1 and 32, not %d"):format(bit_bound))
  end
  if bit_bound <= 8 then
    return sign_extend(M.read_u8(r), 8)
  elseif bit_bound <= 16 then
    return sign_extend(M.read_u16(r), 16)
  end
  return sign_extend(M.read_u32(r), 32)
end

--- A bitmask's holder. Table 33 of XTypes 1.3 §7.4.1.1.3.2.
function M.read_bitmask(r, bit_bound)
  if bit_bound == 0 or bit_bound > 64 then
    refuse_here(r, ("an @bit_bound between 1 and 64, not %d"):format(bit_bound))
  end
  if bit_bound <= 8 then
    return M.read_u8(r)
  elseif bit_bound <= 16 then
    return M.read_u16(r)
  elseif bit_bound <= 32 then
    return M.read_u32(r)
  end
  return M.read_u64(r)
end

--- A bitset's anchor integer, of the width the planner sized.
function M.read_bitset(r, anchor)
  if anchor == 1 then
    return M.read_u8(r)
  elseif anchor == 2 then
    return M.read_u16(r)
  elseif anchor == 4 then
    return M.read_u32(r)
  elseif anchor == 8 then
    return M.read_u64(r)
  end
  refuse_here(r, ("a bitset anchor of 1, 2, 4 or 8 bytes, not %s"):format(tostring(anchor)))
end

-- -- framing -------------------------------------------------------------

--- A sequence's element count, refusing one the stream cannot hold: a
--- count is one element or more, and allocating for a dishonest one is
--- how a bad message becomes an out-of-memory.
function M.read_sequence_length(r)
  local at = r.at
  local count = M.read_u32(r)
  if count > M.reader_remaining(r) then
    impossible(r, at, count)
  end
  return count
end

--- A DHEADER: the length the members that follow occupy.
function M.read_dheader(r)
  local at = r.at
  local length = M.read_u32(r)
  if length > M.reader_remaining(r) then
    impossible(r, at, length)
  end
  return length
end

--- An EMHEADER, unpacked, with the NEXTINT it carries and the extent
--- the member occupies after the header word. Codes 5 to 7 report the
--- NEXTINT they read but the cursor is put back before it: that word
--- is the member's own leading length and the member's decoder has to
--- see it. XTypes 1.3 §7.4.3.4.2.
function M.read_emheader(r)
  local word = M.read_u32(r)
  local length_code = (word >> 28) & 0x7
  local header = {
    member_id = word & writer.E_MEMBER_ID_MASK,
    length_code = length_code,
    must_understand = (word >> 31) ~= 0,
    next_int = false,
    extent = 0,
  }
  if length_code == 4 then
    header.next_int = M.read_u32(r)
  elseif length_code >= 5 then
    local at = r.at
    header.next_int = M.read_u32(r)
    r.at = at
  end
  if length_code <= 3 then
    header.extent = 1 << length_code
  elseif header.next_int == false then
    header.extent = 0
  elseif length_code == 4 then
    header.extent = header.next_int
  else
    local scales = { [5] = 1, [6] = 4, [7] = 8 }
    header.extent = 4 + header.next_int * scales[length_code]
  end
  return header
end

--- A PL_CDR1 parameter header, in whichever form it was written.
--- `role` is the caller's, not the stream's. §7.4.1.2. The extended
--- form's four flag bits are not part of the id (§7.4.1.2.1), and
--- FLAG_2 there is the member's must-understand.
function M.read_pid(r, role)
  role = role or "member"
  M.align_to(r, 4)
  local pid = M.read_u16(r)
  local length = M.read_u16(r)
  if (pid & PID_ID_MASK) == (writer.PID_EXTENDED & PID_ID_MASK) then
    local member_header = M.read_u32(r)
    local wide_length = M.read_u32(r)
    return {
      pid = member_header & writer.E_MEMBER_ID_MASK,
      length = wide_length,
      ["end"] = false,
      must_understand = (member_header & writer.E_FLAG_MUST_UNDERSTAND) ~= 0,
      impl_extension = (member_header & writer.E_FLAG_IMPL_EXTENSION) ~= 0,
      reserved = false,
      ignore = false,
      role = role,
      saved_origin = 0,
      value_at = 0,
    }
  end
  local identifier = pid & PID_ID_MASK
  local ending = identifier == (writer.PID_LIST_END & PID_ID_MASK)
  -- Under a framing that puts FLAG_IMPL_EXTENSION on every member id
  -- the bit carries no information and is reported absent; reporting
  -- it would send every member down the unknown-parameter path.
  local impl = (pid & writer.PID_IMPL_EXTENSION) ~= 0
    and (writer.pid_flags_at(r.pid_flags, "member") & writer.PID_IMPL_EXTENSION) == 0
  return {
    pid = identifier,
    length = length,
    ["end"] = ending,
    must_understand = (pid & writer.PID_MUST_UNDERSTAND) ~= 0,
    impl_extension = impl,
    reserved = (not ending) and identifier > r.highest_short_id,
    ignore = identifier == writer.PID_IGNORE,
    role = role,
    saved_origin = 0,
    value_at = 0,
  }
end

--- A parameter header, and the value block it opens. §7.4.1.2: a
--- parameter's value is its own stream block, so a member's internal
--- alignment counts from the first octet after the header. The shared
--- walk reads a header and later calls `finish_pid_read`, so the block
--- is opened here; the previous origin travels on the header, because
--- parameters do not nest.
function M.read_pid_as(r, role)
  local header = M.read_pid(r, role)
  header.saved_origin = r.origin
  r.origin = r.at
  header.value_at = r.at
  return header
end

--- End a parameter's value and step to the next header — by alignment
--- against the enclosing stream, not by adding the declared length,
--- which is what lets one reader take RTI's rounded-up length and Fast
--- DDS's exact one. Nothing follows `PID_LIST_END`; and a reader that
--- has already refused does not move.
function M.finish_pid_read(r, header)
  r.origin = header.saved_origin
  if r.status ~= "ok" or header["end"] then
    return
  end
  M.align_to(r, 4)
end

--- Whether this parameter can be a member of the reading type: false
--- for a vendor-private parameter, a reserved id and PID_IGNORE, all
--- of which take the unknown-parameter path. §7.4.1.2.2,
--- §7.2.2.4.4.4.6.
function M.pid_is_user_member(header)
  return not (header.impl_extension or header.reserved or header.ignore)
end

--- Step over a member this reader does not know, by its length code.
function M.skip_member(r, header)
  if header.length_code <= 3 then
    take(r, 1 << header.length_code)
    return
  end
  if header.next_int == false then
    refuse_here(r, "a NEXTINT for a length code of 4 or above")
  end
  if header.length_code == 4 then
    take(r, header.next_int)
    return
  end
  local scales = { [5] = 1, [6] = 4, [7] = 8 }
  take(r, 4 + header.next_int * scales[header.length_code])
end

--- Start reading a parameter's value, returning the origin it
--- replaces; the reader's half of the writer's block open.
function M.begin_parameter_value(r)
  local saved = r.origin
  r.origin = r.at
  return saved
end

--- End a parameter's value and step over its trailing padding.
function M.finish_parameter_value(r, saved_origin)
  r.origin = saved_origin
  M.align_to(r, 4)
end

--- Step over `count` bytes, refusing to step past the end.
function M.skip(r, count)
  take(r, count)
end

return M
