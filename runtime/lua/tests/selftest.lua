-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
-- A self-test of the Lua wire runtime against fixed octets, one per HDDS
-- axis of `Dialect::Hdds`. Expected octets from the recorded columns:
--
--   tests/interop-results/array-1d-struct-final/hdds/plain-cdr2.le.bin
--     (packed elements),
--   tests/interop-results/sequence-struct-appendable/hdds/plain-cdr2.le.bin
--     (a sequence element's own DHEADER length),
--   tests/interop-results/wstring-member/hdds/plain-cdr2.le.bin,
--   tests/interop-results/wstring-unbounded-utf8/hdds/plain-cdr2.le.bin and
--   tests/interop-results/wstring-bounded-utf8/hdds/plain-cdr2.le.bin
--     (a wstring as its own UTF-8 octets).
--
-- Run with `lua tests/selftest.lua` from runtime/lua. Exit 0 on
-- agreement, exit 1 with every disagreement named.

local here = arg[0]:match("^(.*)/[^/]*$") or "."
package.path = here .. "/../?.lua;" .. here .. "/../?/init.lua;" .. package.path

local zdw = require("zerodds_wire")

local function hex(value)
  return (value:gsub(".", function(c)
    return string.format("%02x", string.byte(c))
  end))
end

local failed = false

local function check(name, ok)
  if not ok then
    io.stderr:write("selftest: " .. name .. "\n")
    failed = true
  end
end

-- ElementPadding::Packed: each collection element counts its alignment
-- from its own first octet. The second Cell's `long` sits at offset six,
-- where the clause would put it at eight.
local w = zdw.writer_new({ endianness = "le" })
local saved = zdw.begin_packed_element(w)
zdw.write_i32(w, 1)
zdw.write_i16(w, 2)
zdw.finish_packed_element(w, saved)
saved = zdw.begin_packed_element(w)
zdw.write_i32(w, 3)
zdw.write_i16(w, 4)
zdw.finish_packed_element(w, saved)
zdw.write_u8(w, 119)
check("a packed element starts where the last ended",
  hex(zdw.finish(w)) == "01000000020003000000040077")

-- And back through the reader's packed pair.
local r = zdw.reader_new("\x01\x00\x00\x00\x02\x00\x03\x00\x00\x00\x04\x00\x77", "le")
local saved = zdw.reader_begin_packed_element(r)
local a1 = zdw.read_i32(r)
local b1 = zdw.read_i16(r)
zdw.reader_finish_packed_element(r, saved)
saved = zdw.reader_begin_packed_element(r)
local a2 = zdw.read_i32(r)
local b2 = zdw.read_i16(r)
zdw.reader_finish_packed_element(r, saved)
check("a packed element reads back",
  a1 == 1 and b1 == 2 and a2 == 3 and b2 == 4 and zdw.read_u8(r) == 119)

-- SequenceElementDelimiter::BeforeEachNonFinalElement: an element's own
-- octet length in DHEADER form, 06 00 00 00 for the six-octet Cell that
-- follows.
local w = zdw.writer_new({ endianness = "le" })
local h = zdw.write_dheader(w)
zdw.write_i32(w, 1)
zdw.write_i16(w, 2)
zdw.finish_dheader(w, h)
check("a sequence element's own length in DHEADER form",
  hex(zdw.finish(w)) == "06000000010000000200")

-- WstringCodeUnits::Utf8: three octets and a NUL, the length 4.
local w = zdw.writer_new({ endianness = "le" })
zdw.write_wstring_utf8_octets(w, "abc", "octets/terminated/2")
check("a UTF-8 wstring is its octets and a NUL",
  hex(zdw.finish(w)) == "0400000061626300")

local w = zdw.writer_new({ endianness = "le" })
zdw.write_wstring_utf8_octets(w, "\xc3\xa4\xc3\xb6\xc3\xbc \xe1\x88\xb4 Z",
  "octets/terminated/2")
check("a multi-byte UTF-8 wstring counts its octets",
  hex(zdw.finish(w)) == "0d000000c3a4c3b6c3bc20e188b4205a00")

local w = zdw.writer_new({ endianness = "le" })
zdw.write_wstring_utf8_octets_bounded(w, "\xe1\x88\xb4\xc3\xa4Z", 3,
  "octets/terminated/2")
check("a bounded UTF-8 wstring holds three units",
  hex(zdw.finish(w)) == "07000000e188b4c3a45a00")

local r = zdw.reader_new(
  "\x0d\x00\x00\x00\xc3\xa4\xc3\xb6\xc3\xbc\x20\xe1\x88\xb4\x20\x5a\x00", "le")
check("a UTF-8 wstring reads back",
  zdw.read_wstring_utf8_octets(r, "octets/terminated/2")
    == "\xc3\xa4\xc3\xb6\xc3\xbc \xe1\x88\xb4 Z")

if failed then
  os.exit(1)
end
print("selftest: ok")
