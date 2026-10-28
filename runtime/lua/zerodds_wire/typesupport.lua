-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
-- What a generated Lua codec shares between its two directions.
--
-- The Lua counterpart of `runtime/python/zerodds_wire/typesupport.py`.
-- The statuses and behaviours are plain strings rather than an
-- enumeration, because Lua has none: the codec compares against the
-- same spelling `crates/lang-rules/src/emit/lua.rs` writes, and a
-- misspelt one is a comparison that is never true rather than a value
-- that looks plausible — which is why the spellings are listed here
-- once and read from both sides.
--
--   Status:       "ok" | "truncated" | "impossible-length"
--                 | "out-of-range" | "discarded"
--   TryConstruct: "discard" | "use-default" | "trim"
--   ParameterRole:"member" | "optional"

local M = {}

--- How many octets `text` is, which is what a `string<N>` bound counts:
--- `string<5>` is `char[6]` in the C mapping of every measured DDS
--- implementation. A Lua string is a run of octets already.
function M.utf8_octets(text)
  return #text
end

--- How many UTF-16 code units `text` is, which is what a `wstring<N>`
--- bound counts. A scalar above the basic plane is two.
function M.utf16_units(text)
  local units = 0
  for _, point in utf8.codes(text) do
    units = units + (point > 0xFFFF and 2 or 1)
  end
  return units
end

--- `text` cut to at most `limit` units, never inside a character.
--- DDS-XTypes 1.3 §7.2.2.7, Table 11: the order is preserved, so this
--- is a prefix; a character that would straddle the bound is left out
--- whole.
local function trim(text, limit, wide)
  local used = 0
  local keep = 0
  for start, point in utf8.codes(text) do
    local following = utf8.offset(text, 2, start)
    if following == nil then
      following = #text + 1
    end
    local cost
    if wide then
      cost = point > 0xFFFF and 2 or 1
    else
      cost = following - start
    end
    if used + cost > limit then
      break
    end
    used = used + cost
    keep = following - 1
  end
  return string.sub(text, 1, keep)
end

--- `text` cut to at most `octets` bytes, never inside a character.
function M.trim_utf8(text, octets)
  return trim(text, octets, false)
end

--- `text` cut to at most `units` UTF-16 code units.
function M.trim_utf16(text, units)
  return trim(text, units, true)
end

--- `strcmp`'s sign over two byte strings, compared as unsigned octets.
---
--- Not Lua's own `<` on strings, which the reference manual defines
--- through `strcoll` and therefore through the process's locale — a
--- map's entry order must not depend on `LC_COLLATE`. C11 §7.24.4
--- compares as `unsigned char`, which is what this does.
function M.strcmp(left, right)
  local shorter = math.min(#left, #right)
  for index = 1, shorter do
    local a = string.byte(left, index)
    local b = string.byte(right, index)
    if a ~= b then
      return a < b and -1 or 1
    end
  end
  if #left == #right then
    return 0
  end
  return #left < #right and -1 or 1
end

return M
