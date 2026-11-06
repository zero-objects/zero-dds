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

-- -- the operations no corpus case reaches ----------------------------------
--
-- The five the generated code reaches only in a dialect or with a member no
-- corpus case carries: write_pid_extended, write_union_overlay,
-- reader_clear_discard, reader_save and reader_restore
-- (internal/release/OPEN-ITEMS.md). Every runtime checks the same four
-- vectors, each against an oracle outside this code.

-- The first parameter of `annotation-autoid-sequential` as Cyclone DDS
-- recorded it, XCDR1 little endian (tests/interop-corpus/expected-bytes.tsv).
do
  local pid = zdw.writer_new({ endianness = "le", max_align = 8 })
  local header = zdw.write_pid_extended(pid, 0, "member", false)
  zdw.write_i32(pid, -1234567)
  zdw.finish_pid(pid, header)
  check("write_pid_extended writes Cyclone's recorded parameter",
    hex(zdw.finish(pid)) == "017f0800" .. "00000000" .. "04000000" .. "7929edff")
end

-- A `short` read through a four-octet and a one-octet `default:` branch:
-- Cyclone DDS 11.0.1 writes the union's storage, the member's octets with
-- zeros above (`combined-everything`, crates/wire-codec/src/encode.rs
-- `write_selected_branch`).
do
  local overlay = zdw.writer_new({ endianness = "le", max_align = 4 })
  local pattern = zdw.bit_pattern_int(-2, 2)
  zdw.write_union_overlay(overlay, pattern, 4)
  zdw.write_union_overlay(overlay, pattern, 1)
  check("write_union_overlay writes the storage at the branch's width",
    hex(zdw.finish(overlay)) == "feff0000" .. "fe")
end

-- XTypes 1.3 §7.2.2.4.4.4.9: a member's explicit @default comes before
-- Table 9's value, so the call site asks for DISCARD and takes it back. Any
-- other refusal is not a narrowing and stays.
do
  local discarded = zdw.reader_new("\x01\x02\x03\x04", "le", 4)
  zdw.reader_refuse(discarded, "discarded")
  check("reader_clear_discard takes back a discarded",
    zdw.reader_clear_discard(discarded) == true and zdw.reader_status(discarded) == "ok")
  local refused = zdw.reader_new("\x01\x02\x03\x04", "le", 4)
  zdw.reader_refuse(refused, "out-of-range")
  check("reader_clear_discard leaves an out-of-range",
    zdw.reader_clear_discard(refused) == false
      and zdw.reader_status(refused) == "out-of-range")
end

-- XTypes 1.3 §7.4.3.5.3 rule (15): a delimited map<long, long> as its
-- DHEADER and the pairs, without a count. The reader counts the pairs, goes
-- back and reads them again; a refusal made while counting goes back with
-- the cursor.
do
  local map = zdw.reader_new("\x10\x00\x00\x00\x01\x00\x00\x00\x0a\x00\x00\x00"
    .. "\x02\x00\x00\x00\x14\x00\x00\x00", "le", 4)
  local extent = zdw.read_dheader(map)
  local end_at = zdw.reader_position(map) + extent
  local mark = zdw.reader_save(map)
  local pairs = 0
  while zdw.reader_position(map) < end_at and zdw.reader_status(map) == "ok" do
    zdw.read_i32(map)
    zdw.read_i32(map)
    pairs = pairs + 1
  end
  zdw.reader_refuse(map, "out-of-range")
  zdw.reader_restore(map, mark)
  local again = { zdw.read_i32(map), zdw.read_i32(map), zdw.read_i32(map), zdw.read_i32(map) }
  check("reader_save and reader_restore count a delimited map and read it again",
    pairs == 2 and again[1] == 1 and again[2] == 10 and again[3] == 2 and again[4] == 20
      and zdw.reader_position(map) == 20 and zdw.reader_status(map) == "ok")
end

if failed then
  os.exit(1)
end
print("selftest: ok")
