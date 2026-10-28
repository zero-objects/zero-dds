-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
-- Native pure-Lua endpoint SDK (ADR 0013): a from-scratch XCDR wire-core built
-- on string.pack/string.unpack (Lua 5.3+), byte-identical to the Rust core and
-- the other SDKs — no C FFI needed. Sync (poll) and async (coroutines, the
-- idiomatic Lua concurrency model).

local M = {}

-- endian is the string.pack prefix: "<" little-endian, ">" big-endian.
M.LE = "<"
M.BE = ">"

-- The XCDR version the stream is written in. A constructor argument with no
-- default: XTypes 1.3 §7.4.1 puts MAXALIGN at eight under XCDR1 and four
-- under XCDR2, so a writer that did not know which one it was in would be
-- writing XCDR2 and calling it XCDR — which is what this SDK did until the
-- cap became a field. `align` read a constant four, and every `long long` in
-- an XCDR1 sample landed four octets early with nothing in the interface to
-- say so.
M.XCDR1 = 8
M.XCDR2 = 4

-- The member header length codes of XTypes 1.3 §7.4.3.4.2 Table 34. The code
-- occupies bits 28..30 of the EMHEADER and decides how the member's byte
-- length is recovered: LC0..LC3 give it from the code alone, LC4..LC7 from a
-- word that follows or from the body's own leading word.
M.LC0 = 0 -- the member body is one octet
M.LC1 = 1 -- two octets
M.LC2 = 2 -- four octets
M.LC3 = 3 -- eight octets
M.LC4 = 4 -- a NEXTINT holding the body's byte length follows the header
M.LC5 = 5 -- the body's first four octets are the length, and are body
M.LC6 = 6 -- as LC5, and the length counts four-octet elements
M.LC7 = 7 -- as LC5, and the length counts eight-octet elements

-- PID_LIST_END, and the PID that introduces the twelve-octet parameter
-- header. XTypes 1.3 §7.4.1.2 Table 31.
M.PID_LIST_END_ID = 0x3F02
M.PID_EXTENDED_ID = 0x3F01
-- The highest member id a short parameter header carries, inclusive — which
-- is what RTI and Fast DDS write for `@id(16128)`.
M.PID_SHORT_MAX_ID = 0x3F00
-- Bit 14 of a short parameter id. Bit 15 is the implementation-extension
-- flag and is never set here.
local PID_FLAG_MUST_UNDERSTAND = 0x4000
-- Bit 30 of the eMemberHeader inside an extended parameter header — not bit
-- 31, which is the impl-extension flag.
local EM_FLAG_MUST_UNDERSTAND = 0x40000000

-- The two's complement pattern of `v` in a holder of `bits`, or a refusal.
--
-- Both spellings of an integer argument reach the wire through the same
-- call: the unsigned pattern a `uint16` carries and the signed value an
-- `int16` carries. Both are taken, and the bound is the union of the two
-- ranges — the same bound the corpus generator uses to decide that a sample
-- fits its member.
--
-- Two things are refused rather than corrected, and both were silent before:
--
--   * a value outside that union. `putU8(300)` was `300 & 0xff` and wrote
--     `0x2C`; `putU16(70000)` reached `string.pack` and answered `7011` in
--     nine of the fifteen bindings. Either is a frame of the right length
--     carrying a different number.
--   * a float. Lua has one number type with two subtypes, and `2^53 + 1` is
--     `9007199254740992.0` because `^` always returns a float — one less
--     than what the caller wrote. `string.pack("I8", …)` takes a float whose
--     value is integral without a word, so the wrong number would go out
--     looking exactly like the right one. `math.type` is the only place that
--     difference is visible.
local function pattern(v, bits)
  if math.type(v) ~= "integer" then
    error(("%s is a %s and an integer write takes an integer; a float that happens " ..
      "to be integral has already lost whatever did not fit its mantissa")
      :format(tostring(v), math.type(v) or type(v)), 3)
  end
  if bits == 64 then
    -- Lua's integer *is* 64 bits, so every pattern fits and the two ranges
    -- are the same set of bit patterns. There is nothing to check.
    return v
  end
  local top = (1 << bits) - 1
  local bottom = -(1 << (bits - 1))
  if v > top or v < bottom then
    error(("%d does not fit %d bits"):format(v, bits), 3)
  end
  return v & top
end
M.pattern = pattern

-- --- Writer: string pieces + endian; alignment relative to the origin ---

local Writer = {}
Writer.__index = Writer
M.Writer = Writer

-- `encoding` is M.XCDR1 or M.XCDR2 and is required: it is the alignment cap.
--
-- `origin` is the position alignment is measured from. Zero for the stream,
-- and moved to the start of a PL_CDR1 parameter's value for the length of
-- that parameter — §7.4.1.2 aligns a parameter's value against its own
-- block, not against the stream. Without it a `double` in a parameter takes
-- four octets of padding and the header declares twelve where Cyclone, RTI
-- and Fast DDS all declare eight.
function Writer.new(endian, encoding)
  if encoding ~= M.XCDR1 and encoding ~= M.XCDR2 then
    error("Writer.new takes an endian and an encoding (z.XCDR1 or z.XCDR2)", 2)
  end
  return setmetatable(
    { parts = {}, len = 0, endian = endian, maxAlign = encoding, origin = 0, patches = {} },
    Writer)
end

function Writer:raw(s)
  self.parts[#self.parts + 1] = s
  self.len = self.len + #s
end

function Writer:align(a)
  local cap = a < self.maxAlign and a or self.maxAlign
  local pad = (cap - ((self.len - self.origin) % cap)) % cap
  if pad > 0 then self:raw(string.rep("\0", pad)) end
end

function Writer:putU8(v) self:raw(string.pack("B", pattern(v, 8))) end
function Writer:putU16(v) self:align(2); self:raw(string.pack(self.endian .. "I2", pattern(v, 16))) end
function Writer:putU32(v) self:align(4); self:raw(string.pack(self.endian .. "I4", pattern(v, 32))) end

-- A 64-bit integer at its natural eight-octet alignment, which `maxAlign`
-- then caps: eight under XCDR1 and four under XCDR2. XTypes 1.3 §7.4.1. It
-- passed 4 here regardless of the encoding, so every `long long` in an XCDR1
-- sample landed four octets early.
--
-- `i8` and not `I8`: Lua's integer is signed and `string.pack("I8", -1)` is
-- an "unsigned overflow", while the octets of -1 and of 0xFFFFFFFFFFFFFFFF
-- are the same eight. The signed format takes both spellings of the one
-- pattern, which is the whole of what a 64-bit wire field is.
function Writer:putU64(v)
  self:align(8)
  self:raw(string.pack(self.endian .. "i8", pattern(v, 64)))
end

-- Lua has no 32-bit float, so `string.pack("f", …)` is where the narrowing
-- to binary32 happens — inside the binding, where it can be seen.
function Writer:putF32(v) self:align(4); self:raw(string.pack(self.endian .. "f", v)) end

-- Binary64 as its IEEE-754 bit pattern, at the same alignment as putU64.
-- `double` is one of the fifteen primitives of XTypes 1.3 §7.2.2.2 and had
-- no writer here at all. `d` is Lua's own number width, so nothing narrows.
function Writer:putF64(v) self:align(8); self:raw(string.pack(self.endian .. "d", v)) end

-- A `boolean` as the one octet §7.4.1.1.1 Table 30 gives it: 0 or 1.
function Writer:putBool(v) self:putU8(v and 1 or 0) end

function Writer:putString(s)
  self:putU32(#s + 1)
  self:raw(s)
  self:putU8(0)
end

function Writer:putSeqU8(b) -- b: a string of bytes
  self:putU32(#b)
  self:raw(b)
end

-- An IDL `char`: the one octet XTypes 1.3 §7.4.1.1.1 Table 30 gives it.
--
-- A code point above U+00FF is refused and not truncated. `š` is U+0161 and
-- its low octet is 0x61, which is `a`: a truncating writer produces a frame
-- of the right length holding a different, plausible character and nothing
-- downstream can tell.
function Writer:putChar(codePoint)
  if math.type(codePoint) ~= "integer" or codePoint < 0 or codePoint > 0xFF then
    error(("expected a character below U+0100, found %s"):format(tostring(codePoint)), 2)
  end
  self:putU8(codePoint)
end

-- An IDL `wchar`: one two-octet code unit, 2-aligned, in the stream's byte
-- order.
--
-- The four policy axes a wide character carries are answered here and not by
-- the caller: the unit is two octets wide, it goes out in the stream's byte
-- order, there is no GIOP length octet before it, and no byte-order mark.
-- That is what Cyclone and Fast DDS write in both encodings and what RTI
-- writes under XCDR2; RTI's XCDR1 `wchar` is four octets, which is the one
-- reading this binding does not take.
--
-- A code point above U+FFFF is refused. The refusal is width-independent —
-- a four-octet form would zero-extend a code *unit*, not a code point.
function Writer:putWchar(codePoint)
  if math.type(codePoint) ~= "integer" or codePoint < 0 or codePoint > 0xFFFF then
    error(("expected a BMP character, found %s"):format(tostring(codePoint)), 2)
  end
  self:putU16(codePoint)
end

-- An IDL `wstring` from its UTF-16 code units, as an array of integers.
--
-- The length word counts **octets** and not code units, there is no
-- terminator, there is no byte-order mark, and each unit goes out two octets
-- wide in the stream's own byte order. Those are the four axes and they are
-- the binding's answer. Cyclone writes exactly this in both encodings and
-- RTI under XCDR2; Fast DDS counts units, which is the reading this binding
-- does not take.
function Writer:putWstring(units)
  self:putU32(#units * 2)
  for i = 1, #units do self:putU16(units[i]) end
end

-- A `bitset` as its anchor word: the holder the declared bits pack into,
-- written at the anchor's own width and therefore at the anchor's own
-- alignment. XTypes 1.3 §7.3.1.4 and §7.4.1.
--
-- `bits` is the packed holder and `anchor` its width in octets, both
-- properties of the declared type. What this call decides is the pair the
-- type does not: which write the holder goes out through, and what that
-- write aligns to.
function Writer:putBitset(bits, anchor)
  if anchor == 1 then self:putU8(bits)
  elseif anchor == 2 then self:putU16(bits)
  elseif anchor == 4 then self:putU32(bits)
  elseif anchor == 8 then self:putU64(bits)
  else
    error(("a bitset anchor is one, two, four or eight octets, not %s"):format(tostring(anchor)), 2)
  end
end

-- An IDL `fixed<digits,scale>` as packed BCD.
-- CDR (OMG CORBA 3.3) §9.3.2.8 / IDL 4.2 §7.4.1.4.5.
--
-- `value` is the decimal as the sample wrote it — an optional sign, an
-- integer part and an optional fraction. Everything after that is this
-- call's:
--
--   * the integer part is left-padded and the fraction right-padded to
--     exactly `digits` decimal digits, with the point implied `scale` from
--     the right; the scale itself is not on the wire,
--   * a leading zero nibble is prepended when `digits + 1` is odd, so digits
--     and sign pair up,
--   * two nibbles to the octet, most significant digit first, high nibble
--     first,
--   * the last nibble is the sign: 0xD negative, 0xC otherwise. A negative
--     zero goes out positive, because there is one zero to write,
--   * alignment is one, and no multi-octet integer is written, so the byte
--     order never enters.
--
-- The octet count is therefore ceil((digits + 1) / 2) whatever the value,
-- which is the point omniORB 4.3.3 and JacORB 3.9 differ on and the one this
-- binding takes from omniORB. A decimal string and not a Lua number: Lua's
-- number is a double and `fixed<31,0>` has thirty-one significant digits,
-- eleven more than one holds.
function Writer:putFixed(value, digits, scale)
  self:raw(M.fixedOctets(value, digits, scale))
end

-- The packing putFixed writes, apart so it can be tested without a Writer.
function M.fixedOctets(value, digits, scale)
  if math.type(digits) ~= "integer" or digits < 1 or digits > 31 then
    error(("fixed<%s,%s>: a digit count is 1..31"):format(tostring(digits), tostring(scale)), 2)
  end
  if math.type(scale) ~= "integer" or scale < 0 or scale > digits then
    error(("fixed<%d,%s>: a scale is 0..digits"):format(digits, tostring(scale)), 2)
  end
  local text, negative = value, false
  local sign = text:sub(1, 1)
  if sign == "+" or sign == "-" then
    negative = sign == "-"
    text = text:sub(2)
  end
  local integral, fraction = text, ""
  local at = text:find(".", 1, true)
  if at then
    integral, fraction = text:sub(1, at - 1), text:sub(at + 1)
  end
  if integral == "" and fraction == "" then
    error(("fixed<%d,%d>: `%s` has no digits"):format(digits, scale, value), 2)
  end
  if (integral .. fraction):match("[^0-9]") then
    error(("fixed<%d,%d>: `%s` is not a decimal"):format(digits, scale, value), 2)
  end
  -- Leading zeros carry no value, so they go before the width is checked:
  -- `000000.001` is one significant integer digit, not six.
  integral = integral:gsub("^0+", "")
  if #integral > digits - scale then
    error(("fixed<%d,%d>: `%s` needs %d integer digits and the type has %d")
      :format(digits, scale, value, #integral, digits - scale), 2)
  end
  -- A trailing zero in the fraction is padding either way, so it goes before
  -- the width is checked and is re-added below.
  while #fraction > scale and fraction:sub(-1) == "0" do
    fraction = fraction:sub(1, -2)
  end
  if #fraction > scale then
    error(("fixed<%d,%d>: `%s` needs %d fraction digits and the type has %d")
      :format(digits, scale, value, #fraction, scale), 2)
  end

  local nibbles = {}
  local function push(n) nibbles[#nibbles + 1] = n end
  if (digits + 1) % 2 == 1 then push(0) end
  for _ = 1, digits - scale - #integral do push(0) end
  for i = 1, #integral do push(integral:byte(i) - 48) end
  for i = 1, #fraction do push(fraction:byte(i) - 48) end
  for _ = 1, scale - #fraction do push(0) end
  local nonZero = false
  for i = 1, #nibbles do if nibbles[i] ~= 0 then nonZero = true end end
  push((negative and nonZero) and 0x0D or 0x0C)

  local out = {}
  for i = 1, #nibbles - 1, 2 do
    out[#out + 1] = string.char((nibbles[i] << 4) | nibbles[i + 1])
  end
  return table.concat(out)
end

-- --- back-patched framing ------------------------------------------------
--
-- A DHEADER, a NEXTINT and a PL_CDR1 parameter length are all the same
-- shape: a slot reserved before the body and filled in once the body's
-- length is known. `patch` is the one place that writes into an
-- already-written position, and it honours the wire byte order like every
-- other write.
--
-- A Lua string is immutable, so a patch is recorded and applied once in
-- `bytes()` rather than splicing the buffer where it is taken. A corpus case
-- is a few thousand writes and every `@mutable` member is a patch; splicing
-- each one would make the sweep quadratic in the sample's size for no
-- difference in the octets.
--
-- `at` is a 0-based offset throughout, so the arithmetic reads the same as
-- the other fourteen bindings. `len` never moves: a patch overwrites a slot
-- that was already reserved, it never appends.

function Writer:patch(at, width, v)
  self.patches[#self.patches + 1] = { at = at, width = width, v = v }
end

function Writer:bytes()
  local flat = table.concat(self.parts)
  if #self.patches == 0 then return flat end
  -- The slots never overlap — each was reserved by its own begin — so
  -- walking them in position order splices the whole buffer once.
  local ordered = {}
  for i = 1, #self.patches do ordered[i] = self.patches[i] end
  table.sort(ordered, function(a, b) return a.at < b.at end)
  local out, cursor = {}, 0
  for i = 1, #ordered do
    local p = ordered[i]
    out[#out + 1] = flat:sub(cursor + 1, p.at)
    out[#out + 1] = string.pack(self.endian .. "I" .. p.width, p.v)
    cursor = p.at + p.width
  end
  out[#out + 1] = flat:sub(cursor + 1)
  return table.concat(out)
end

-- Reserve a 4-aligned DHEADER slot and return the position the body starts
-- at, to hand back to dheaderEnd. XTypes 1.3 §7.4.3.5.
--
-- `@appendable` is the default extensibility of XTypes (§7.3.1.2.1.8), so a
-- struct written with no annotation at all had no form here before this.
function Writer:dheaderBegin()
  self:putU32(0)
  return self.len
end

-- Back-patch the DHEADER with the byte length written since bodyStart. The
-- DHEADER itself is not counted — §7.4.3.5 measures the content that follows.
function Writer:dheaderEnd(bodyStart)
  self:patch(bodyStart - 4, 4, self.len - bodyStart)
end

-- The EMHEADER for one member of a `@mutable` aggregate. §7.4.3.4.2:
--
--     EMHEADER = (must_understand << 31) | (length_code << 28) | member_id
--
-- with a 28-bit member id. For LC4 a NEXTINT slot is reserved after it and
-- emheaderEnd fills it in; for LC0..LC3 the code alone gives the length and
-- nothing follows; for LC5..LC7 the body's own leading four octets are the
-- length, so no separate NEXTINT is written and writing one would put every
-- following member at the wrong offset.
function Writer:emheaderBegin(id, mustUnderstand, lengthCode)
  if math.type(id) ~= "integer" or id < 0 or id > 0x0FFFFFFF then
    error(("member id %s does not fit the 28 bits of an EMHEADER"):format(tostring(id)), 2)
  end
  if math.type(lengthCode) ~= "integer" or lengthCode < M.LC0 or lengthCode > M.LC7 then
    error(("length code %s is not one of LC0..LC7"):format(tostring(lengthCode)), 2)
  end
  local header = (lengthCode << 28) | id
  if mustUnderstand then header = header | (1 << 31) end
  self:putU32(header)
  if lengthCode == M.LC4 then self:putU32(0) end
  return self.len
end

-- Close the member emheaderBegin opened.
--
-- The length code is taken back rather than remembered, so a caller that
-- paired an LC4 begin with a differently-coded end gets a refusal instead of
-- a NEXTINT patched over the first four octets of a member body.
function Writer:emheaderEnd(bodyStart, lengthCode)
  local written = self.len - bodyStart
  if lengthCode == M.LC4 then
    self:patch(bodyStart - 4, 4, written)
  elseif lengthCode <= M.LC3 then
    -- 1, 2, 4, 8 — the code *is* the length, so a body of any other size
    -- means the plan and the member disagree. Silence here would put a
    -- length on the wire that no octet count backs.
    local want = 1 << lengthCode
    if written ~= want then
      error(("LC%d frames %d octets and the member wrote %d")
        :format(lengthCode, want, written), 2)
    end
  elseif written < 4 then
    error(("LC%d takes its length from the body's first four octets and the member wrote %d")
      :format(lengthCode, written), 2)
  end
end

-- A PL_CDR1 short parameter header. XTypes 1.3 §7.4.1.2: at a four-octet
-- boundary an `unsigned short` parameter id, then an `unsigned short` byte
-- length of the value that follows. Both go out in the stream's byte order.
--
-- The must-understand flag is bit 14 of the id field. Bit 15 is the
-- implementation-extension flag and is not set here.
--
-- The parameter's value aligns against the block and not against the stream,
-- so the alignment origin moves for the length of the parameter and pidEnd
-- restores it. Returns the open header to hand back.
function Writer:pidBegin(pid, mustUnderstand)
  if math.type(pid) ~= "integer" or pid < 0 or pid > M.PID_SHORT_MAX_ID then
    error(("member id %s needs the extended parameter header"):format(tostring(pid)), 2)
  end
  self:align(4)
  local id = pid
  if mustUnderstand then id = id | PID_FLAG_MUST_UNDERSTAND end
  self:align(2); self:raw(string.pack(self.endian .. "I2", id))
  local at = self.len
  self:raw("\0\0")
  local saved = self.origin
  self.origin = self.len
  return { at = at, extended = false, savedOrigin = saved }
end

-- The twelve-octet parameter header §7.4.1.2 requires for a member id above
-- 0x3F00 or a value longer than 65535 octets:
--
--   [PID_EXTENDED | must-understand : u16][8 : u16][eMemberHeader : u32][length : u32]
--
-- The must-understand flag on the marker is not the member's — it is on the
-- marker under this framing whatever the member says, and the member's own
-- flag is bit 30 of the eMemberHeader. Reading it off the marker instead
-- makes every parameter must-understand.
function Writer:pidExtendedBegin(memberId, mustUnderstand)
  if math.type(memberId) ~= "integer" or memberId < 0 or memberId > 0x0FFFFFFF then
    error(("member id %s does not fit the 28 bits of an eMemberHeader"):format(tostring(memberId)), 2)
  end
  self:align(4)
  local marker = M.PID_EXTENDED_ID | PID_FLAG_MUST_UNDERSTAND
  self:align(2); self:raw(string.pack(self.endian .. "I2", marker))
  self:align(2); self:raw(string.pack(self.endian .. "I2", 8))
  local header = memberId
  if mustUnderstand then header = header | EM_FLAG_MUST_UNDERSTAND end
  self:align(4); self:raw(string.pack(self.endian .. "I4", header))
  local at = self.len
  self:raw("\0\0\0\0")
  local saved = self.origin
  self.origin = self.len
  return { at = at, extended = true, savedOrigin = saved }
end

-- Close the parameter pidBegin or pidExtendedBegin opened.
--
-- The declared length counts the value's octets **and** the padding that
-- brings the next header back to a four-octet boundary: a reader skips
-- `length` from the end of the header and has to land on the next parameter.
-- The padding is written after the length is patched, so the two cannot
-- disagree.
function Writer:pidEnd(header)
  local width = header.extended and 4 or 2
  local length = self.len - header.at - width
  -- The parameter's own origin is still in force here, so the round-up is
  -- measured from the block that the length describes.
  local past = (self.len - header.savedOrigin) % 4
  if past ~= 0 then length = length + 4 - past end
  if not header.extended and length > 0xFFFF then
    error(("a short parameter header describes at most 65535 octets, not %d"):format(length), 2)
  end
  self:patch(header.at, width, length)
  self.origin = header.savedOrigin
  self:align(4)
end

-- The sentinel that closes a PL_CDR1 parameter list: the id PID_LIST_END
-- with a zero length, four-aligned. §7.4.1.2.
--
-- The must-understand flag is set on the sentinel — 0x7F02 on the wire,
-- which is what Cyclone and RTI write. Fast DDS writes the bare 0x3F02; the
-- flag is not the sentinel's meaning either way, and this is the majority
-- form.
function Writer:pidListEnd()
  self:align(4)
  local id = M.PID_LIST_END_ID | PID_FLAG_MUST_UNDERSTAND
  self:align(2); self:raw(string.pack(self.endian .. "I2", id))
  self:align(2); self:raw(string.pack(self.endian .. "I2", 0))
end

-- --- Reader: a cursor over a byte string (pos is 0-based) ---

local Reader = {}
Reader.__index = Reader
M.Reader = Reader

function Reader.new(data, endian, encoding)
  if encoding ~= M.XCDR1 and encoding ~= M.XCDR2 then
    error("Reader.new takes data, an endian and an encoding (z.XCDR1 or z.XCDR2)", 2)
  end
  return setmetatable(
    { data = data, pos = 0, endian = endian, maxAlign = encoding, origin = 0 },
    Reader)
end

function Reader:align(a)
  local cap = a < self.maxAlign and a or self.maxAlign
  local pad = (cap - ((self.pos - self.origin) % cap)) % cap
  self.pos = self.pos + pad
end

function Reader:take(a, fmt)
  self:align(a)
  local v, nextpos = string.unpack(self.endian .. fmt, self.data, self.pos + 1)
  self.pos = nextpos - 1
  return v
end

function Reader:getU32() return self:take(4, "I4") end

-- --- primitives beyond getU32 — the byte-exact inverse of the Writer ---

function Reader:getU8()
  local v = self.data:byte(self.pos + 1)
  self.pos = self.pos + 1
  return v
end

function Reader:getU16() return self:take(2, "I2") end

-- A 64-bit integer at its natural eight-octet alignment, which `maxAlign`
-- then caps — the same eight the writer passes. It passed 4 here while the
-- writer passed 4 as well, so both were wrong together under XCDR1 and the
-- round trip never showed it.
--
-- `i8` mirrors putU64's: `I8` refuses the top half of the range on the way
-- back out, and the octets are the same either way.
function Reader:getU64() return self:take(8, "i8") end

function Reader:getF32() return self:take(4, "f") end
function Reader:getF64() return self:take(8, "d") end
function Reader:getBool() return self:getU8() ~= 0 end
-- The one octet an IDL `char` occupies.
function Reader:getChar() return self:getU8() end
-- One two-octet `wchar` code unit.
function Reader:getWchar() return self:getU16() end

-- A `wstring` back into its UTF-16 code units. The length word counts
-- octets, so the unit count is half of it.
function Reader:getWstring()
  local octets = self:getU32()
  local units = {}
  for i = 1, octets // 2 do units[i] = self:getU16() end
  return units
end

-- A `bitset` anchor of the given width back into its holder word.
function Reader:getBitset(anchor)
  if anchor == 1 then return self:getU8()
  elseif anchor == 2 then return self:getU16()
  elseif anchor == 4 then return self:getU32()
  elseif anchor == 8 then return self:getU64()
  end
  error(("a bitset anchor is one, two, four or eight octets, not %s"):format(tostring(anchor)), 2)
end

-- A `fixed<digits,scale>` back into the decimal it was written from: a
-- leading `-` when the sign nibble is 0xD, `digits - scale` integer digits,
-- and a `.` with `scale` fraction digits where the scale is not zero. The
-- scale is not on the wire, so it is an argument here for the same reason it
-- is one in putFixed.
function Reader:getFixed(digits, scale)
  local octets = (digits + 2) // 2
  local nibbles = {}
  for _ = 1, octets do
    local b = self:getU8()
    nibbles[#nibbles + 1] = b >> 4
    nibbles[#nibbles + 1] = b & 0x0F
  end
  local sign = nibbles[#nibbles]
  local out = {}
  if sign == 0x0D then out[#out + 1] = "-" end
  local first = #nibbles - digits
  for i = 1, digits do
    if scale ~= 0 and i == digits - scale + 1 then out[#out + 1] = "." end
    out[#out + 1] = string.char(48 + nibbles[first + i - 1])
  end
  return table.concat(out)
end

function Reader:getString()
  local n = self:getU32()
  local s = self.data:sub(self.pos + 1, self.pos + n - 1)
  self.pos = self.pos + n
  return s
end

function Reader:getSeqU8()
  local n = self:getU32()
  local b = self.data:sub(self.pos + 1, self.pos + n)
  self.pos = self.pos + n
  return b
end

-- A DHEADER, as the announced body byte length.
function Reader:dheaderRead() return self:getU32() end

-- A member header: the id, the must-understand flag, the length code, and
-- the member body's extent in octets measured from the position this call
-- leaves the cursor at.
--
-- LC0..LC3 take the extent from the code. LC4 consumes the NEXTINT that
-- follows the header. LC5..LC7 consume nothing: the next four octets are the
-- member's own leading word and belong to the body, so they are peeked and
-- the extent is 4 + word for LC5, 4 + 4*word for LC6 and 4 + 8*word for LC7
-- — the last two count elements, not octets.
function Reader:emheaderRead()
  local header = self:getU32()
  local id = header & 0x0FFFFFFF
  local mustUnderstand = (header & (1 << 31)) ~= 0
  local lengthCode = (header >> 28) & 0x7
  local extent
  if lengthCode <= M.LC3 then
    extent = 1 << lengthCode
  elseif lengthCode == M.LC4 then
    extent = self:getU32()
  else
    local at = self.pos
    local word = self:getU32()
    self.pos = at
    if lengthCode == M.LC5 then extent = 4 + word
    elseif lengthCode == M.LC6 then extent = 4 + 4 * word
    else extent = 4 + 8 * word end
  end
  return id, mustUnderstand, lengthCode, extent
end

-- A PL_CDR1 parameter header in either form: the id with its flags stripped,
-- the member's must-understand flag, the declared byte length, and whether
-- the list ends here.
--
-- The extended form is recognised by the marker PID_EXTENDED, and the
-- must-understand answer then comes from bit 30 of the eMemberHeader and
-- never from the marker — the marker carries the flag under this framing
-- whatever the member says.
--
-- The value's alignment origin is moved to the position this call leaves,
-- and pidSkip restores it.
function Reader:pidRead()
  local raw = self:getU16()
  local short = raw & 0x3FFF
  if short == M.PID_LIST_END_ID then
    self:getU16()
    return 0, false, 0, true
  end
  if short == M.PID_EXTENDED_ID then
    self:getU16() -- the slength, always 8
    local header = self:getU32()
    local length = self:getU32()
    self.origin = self.pos
    return header & 0x0FFFFFFF, (header & EM_FLAG_MUST_UNDERSTAND) ~= 0, length, false
  end
  local length = self:getU16()
  self.origin = self.pos
  return short, (raw & PID_FLAG_MUST_UNDERSTAND) ~= 0, length, false
end

-- Move the cursor past a parameter value of `length` octets from `bodyStart`
-- and restore the stream's alignment origin.
function Reader:pidSkip(bodyStart, length)
  self.pos = bodyStart + length
  self.origin = 0
  while self.pos % 4 ~= 0 do self.pos = self.pos + 1 end
end

-- --- XRCE WRITE_DATA framing ---

M.SESSION_NOKEY = 0x80
M.STREAM_BEST_EFFORT = 0x01
-- E flag (little-endian body) + DataFormat FORMAT_DATA (0b000) in bits 1..3 --
-- DDS-XRCE 1.0 8.3.5.8. FORMAT_SAMPLE (0b001, i.e. 0x03) would additionally
-- require a SampleInfo in the body, which this profile does not carry.
M.XRCE_WRITE_FLAGS = 0x01
-- Wire size of the BaseObjectRequest that precedes the sample (7.7.8).
M.XRCE_BASE_SIZE = 4
-- OBJK_DATAWRITER -- the ObjectKind of a DataWriter (7.2.2).
M.OBJK_DATAWRITER = 0x05

-- An ObjectId (7.2.1) from a 12-bit raw id and a 4-bit ObjectKind.
function M.objectId(rawId, kind)
  return ((rawId & 0x0FFF) << 4) | (kind & 0x0F)
end

-- The 16-bit submessage_length field cannot describe a sample larger than
-- 0xFFFF, so a sample above it has no WRITE_DATA form: refuse rather than
-- truncate the length while appending the full payload (the C SDK's
-- zdw_xrce_write_frame rejects the same way).
--
-- `error` and no longer `nil`. `Client:write` handed the nil to
-- `transport.deliver` without looking, so `z.writeFrame(0x80, 0x01, 1,
-- string.rep("x", 0x10000)) == nil` reached the link as nothing at all with no
-- error anywhere.
--
-- The submessage body is the BaseObjectRequest (7.7.8: request_id then
-- object_id, each two octets most significant first) followed by the sample.
-- `objectId` names the target DataWriter -- without it no XRCE agent can route
-- the sample. The request_id is the message's own `seqNo`, which 7.7.8 only
-- requires to be unique per stream.
function M.writeFrame(session, stream, seqNo, objectId, sample)
  local bodyLen = M.XRCE_BASE_SIZE + #sample
  if bodyLen > 0xFFFF then
    error(("%d octets: the 16-bit XRCE submessage_length describes at most 65535")
      :format(#sample), 2)
  end
  local hdr = string.pack("BB", session, stream)
    .. string.pack("<I2", seqNo)
    .. string.pack("BB", 0x07, M.XRCE_WRITE_FLAGS)
    .. string.pack("<I2", bodyLen)
    .. string.pack(">I2I2", seqNo, objectId)
  return hdr .. sample
end

-- Frame layout: [session, stream, seq_lo, seq_hi, sm_id, flags, len_lo, len_hi]
-- then the sample body. The 16-bit little-endian submessage_length (bytes 7..8,
-- 1-based) bounds the body exactly: the returned string is frame[9 .. 8+smLen],
-- never frame:sub(9) to the end, so trailing padding or an appended submessage
-- is not folded into the sample.
--
-- Accepts WRITE_DATA (0x07, endpoint->hub / loopback) and DATA (0x09,
-- hub->endpoint) — the hub pushes samples with DATA. See DDS-XRCE spec 8.3.5.
-- Returns nil for a short header, a wrong submessage id, or a declared length
-- that runs past the datagram (truncation / wrong length).
function M.readFrame(frame)
  if #frame < 8 + M.XRCE_BASE_SIZE then return nil end
  local id = frame:byte(5)
  if id ~= 0x07 and id ~= 0x09 then return nil end
  local smLen = frame:byte(7) | (frame:byte(8) << 8)
  if smLen < M.XRCE_BASE_SIZE or 8 + smLen > #frame then return nil end
  return frame:sub(9 + M.XRCE_BASE_SIZE, 8 + smLen)
end

-- `requestId, objectId` of a data frame (7.7.8), or nil under the same rules
-- as `readFrame`.
function M.frameBase(frame)
  if M.readFrame(frame) == nil then return nil end
  return string.unpack(">I2I2", frame, 9)
end

-- --- sync client ---

local Client = {}
Client.__index = Client
M.Client = Client

-- `objectId` names the target DataWriter in every frame's BaseObjectRequest
-- (7.7.8).
function Client.new(transport, objectId)
  return setmetatable(
    { transport = transport, session = M.SESSION_NOKEY, stream = M.STREAM_BEST_EFFORT,
      objectId = objectId, seq = 1 },
    Client)
end

-- A sample the envelope cannot carry raises out of `writeFrame`, so the
-- sequence is not advanced and nothing reaches the transport.
function Client:write(sample)
  local frame = M.writeFrame(self.session, self.stream, self.seq, self.objectId, sample)
  self.transport.deliver(frame)
  self.seq = (self.seq + 1) & 0xffff
end

-- One non-blocking receive: returns the sample body, or nil.
function Client:poll()
  local frame = self.transport.receive()
  if frame == nil then return nil end
  return M.readFrame(frame)
end

-- --- async reader: a coroutine producer (idiomatic Lua) ---
--
-- Resume it to get the next decoded sample; it yields nil when the transport is
-- momentarily empty, so a cooperative consumer can retry or do other work.
function M.asyncReader(transport)
  return coroutine.wrap(function()
    while true do
      local frame = transport.receive()
      if frame then
        local body = M.readFrame(frame)
        if body then coroutine.yield(body) end
      else
        coroutine.yield(nil)
      end
    end
  end)
end

-- In-memory FIFO transport for tests + example.
function M.memTransport()
  local q, head = {}, 1
  return {
    deliver = function(frame) q[#q + 1] = frame end,
    receive = function()
      if head <= #q then
        local f = q[head]
        q[head] = nil
        head = head + 1
        return f
      end
      return nil
    end,
  }
end

return M
