-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
-- IEEE 754 binary128, in plain Lua.
--
-- The Lua counterpart of `runtime/python/zerodds_wire/float128.py` — a
-- further implementation of the same clauses, not a binding to any of
-- them. CORBA 3.3 part 2 §9.3.1.3 with Figure 9.2 gives CDR
-- `long double` one sign bit, a fifteen-bit exponent biased by 16383
-- and a 112-bit stored fraction; DDS-XTypes 1.3 Table 31 names the
-- result: sixteen bytes, aligned to eight.
--
-- A Lua float is a C `double` — 53 of the 113 significand bits — so the
-- value never passes through one. The member holds the decimal text
-- (see `crates/lang-rules/src/adapter/lua.rs`), this module converts
-- the text to the sixteen-byte pattern with a single correct rounding
-- (to nearest, ties to even), and the read direction answers the exact
-- decimal expansion in the corpus's canonical form.
--
-- Lua has no arbitrary-precision integer, so this file carries its own:
-- little-endian arrays of 24-bit limbs, sized so every intermediate
-- product and every division step fits a 64-bit Lua integer with room
-- to spare. Nothing outside this file sees a limb.
--
-- The 128-bit pattern crosses the module boundary as two 64-bit Lua
-- integers `hi, lo` — bit 127 is `hi`'s bit 63 — because that is what a
-- writer splits into bytes and a reader assembles from them.

local M = {}

-- Fields of binary128. IEEE 754-2008 clause 3.6.
local FRACTION_BITS = 112
local EXPONENT_BIAS = 16383
local EXPONENT_MAX = 0x7FFF
local FRACTION_HI_MASK = 0xFFFFFFFFFFFF -- bits 111..64, inside `hi`

-- -- the bignum ----------------------------------------------------------
--
-- Little-endian arrays of limbs below 2^24, normalised: no trailing
-- zero limb, and zero is the empty array. 24 bits, because a limb times
-- a small factor plus a carry stays below 2^48 and a division step's
-- partial dividend below 2^48 too — both inside a Lua integer without
-- ever touching its sign bit.

local BASE_BITS = 24
local BASE_MASK = (1 << BASE_BITS) - 1

local function bn_is_zero(a)
  return #a == 0
end

local function bn_normalise(a)
  while #a > 0 and a[#a] == 0 do
    a[#a] = nil
  end
  return a
end

--- `a * m`, with `m` below 2^24. A fresh array; `a` is not written.
local function bn_mul_small(a, m)
  local out = {}
  local carry = 0
  for index = 1, #a do
    local product = a[index] * m + carry
    out[index] = product & BASE_MASK
    carry = product >> BASE_BITS
  end
  local index = #a
  while carry > 0 do
    index = index + 1
    out[index] = carry & BASE_MASK
    carry = carry >> BASE_BITS
  end
  return bn_normalise(out)
end

--- `a + s`, with `s` below 2^24. A fresh array.
local function bn_add_small(a, s)
  local out = {}
  local carry = s
  for index = 1, #a do
    local sum = a[index] + carry
    out[index] = sum & BASE_MASK
    carry = sum >> BASE_BITS
  end
  local index = #a
  while carry > 0 do
    index = index + 1
    out[index] = carry & BASE_MASK
    carry = carry >> BASE_BITS
  end
  return bn_normalise(out)
end

--- `a << k`.
local function bn_shl(a, k)
  if bn_is_zero(a) or k == 0 then
    local out = {}
    for index = 1, #a do
      out[index] = a[index]
    end
    return out
  end
  local whole = k // BASE_BITS
  local bits = k % BASE_BITS
  local out = {}
  for index = 1, whole do
    out[index] = 0
  end
  local carry = 0
  for index = 1, #a do
    local moved = (a[index] << bits) | carry
    out[whole + index] = moved & BASE_MASK
    carry = moved >> BASE_BITS
  end
  if carry > 0 then
    out[whole + #a + 1] = carry
  end
  return bn_normalise(out)
end

--- `a >> k`, discarding.
local function bn_shr(a, k)
  local whole = k // BASE_BITS
  local bits = k % BASE_BITS
  local out = {}
  for index = whole + 1, #a do
    local value = a[index] >> bits
    if bits > 0 and index + 1 <= #a then
      value = value | ((a[index + 1] << (BASE_BITS - bits)) & BASE_MASK)
    end
    out[index - whole] = value
  end
  return bn_normalise(out)
end

--- Whether any bit below position `k` is set.
local function bn_any_below(a, k)
  if k <= 0 then
    return false
  end
  local whole = k // BASE_BITS
  local bits = k % BASE_BITS
  for index = 1, math.min(whole, #a) do
    if a[index] ~= 0 then
      return true
    end
  end
  if bits > 0 and whole + 1 <= #a then
    return (a[whole + 1] & ((1 << bits) - 1)) ~= 0
  end
  return false
end

--- Bit `k`, as 0 or 1.
local function bn_get_bit(a, k)
  local limb = a[k // BASE_BITS + 1]
  if limb == nil then
    return 0
  end
  return (limb >> (k % BASE_BITS)) & 1
end

--- How many bits `a` has; zero for zero.
local function bn_bit_length(a)
  if bn_is_zero(a) then
    return 0
  end
  local top = a[#a]
  local bits = 0
  while top > 0 do
    bits = bits + 1
    top = top >> 1
  end
  return (#a - 1) * BASE_BITS + bits
end

--- `a >> shift`, rounding to nearest with ties to even. The second
--- result says whether anything was discarded.
local function bn_shift_round(a, shift)
  if shift <= 0 then
    return a, false
  end
  if not bn_any_below(a, shift) then
    return bn_shr(a, shift), false
  end
  local half = bn_get_bit(a, shift - 1)
  local below_half = bn_any_below(a, shift - 1)
  local kept = bn_shr(a, shift)
  if half == 1 and (below_half or bn_get_bit(kept, 0) == 1) then
    kept = bn_add_small(kept, 1)
  end
  return kept, true
end

--- `(a // m, a % m)`, with `m` below 2^24.
local function bn_divmod_small(a, m)
  local out = {}
  local remainder = 0
  for index = #a, 1, -1 do
    local current = (remainder << BASE_BITS) | a[index]
    out[index] = current // m
    remainder = current % m
  end
  return bn_normalise(out), remainder
end

--- `a` with its low bit set — the sticky fold.
local function bn_or_1(a)
  if bn_is_zero(a) then
    return { 1 }
  end
  local out = {}
  for index = 1, #a do
    out[index] = a[index]
  end
  out[1] = out[1] | 1
  return out
end

--- `a * 5^k`.
local FIVE_10 = 9765625 -- 5^10, below 2^24
local function bn_mul_pow5(a, k)
  local out = a
  while k >= 10 do
    out = bn_mul_small(out, FIVE_10)
    k = k - 10
  end
  if k > 0 then
    out = bn_mul_small(out, 5 ^ k // 1)
  end
  return out
end

--- The low 128 bits of `a`, as the pair `hi, lo`. The callers only hand
--- this a value already known to fit.
local function bn_to_bits128(a)
  local hi, lo = 0, 0
  for index = #a, 1, -1 do
    hi = (hi << BASE_BITS) | (lo >> (64 - BASE_BITS))
    lo = (lo << BASE_BITS) | a[index]
  end
  return hi, lo
end

--- A bignum from the 112 fraction bits split as the pattern holds them.
local function bn_from_fraction(frac_hi, lo)
  local out = {}
  out[1] = lo & BASE_MASK
  out[2] = (lo >> 24) & BASE_MASK
  out[3] = ((lo >> 48) & 0xFFFF) | ((frac_hi & 0xFF) << 16)
  out[4] = (frac_hi >> 8) & BASE_MASK
  out[5] = (frac_hi >> 32) & BASE_MASK
  return bn_normalise(out)
end

--- The decimal digits of `a`, as text. Seven at a time, which keeps
--- each division step inside a limb.
local function bn_digits(a)
  if bn_is_zero(a) then
    return "0"
  end
  local groups = {}
  local rest = a
  while not bn_is_zero(rest) do
    local group
    rest, group = bn_divmod_small(rest, 10000000)
    groups[#groups + 1] = group
  end
  local parts = { tostring(groups[#groups]) }
  for index = #groups - 1, 1, -1 do
    parts[#parts + 1] = string.format("%07d", groups[index])
  end
  return table.concat(parts)
end

-- -- the patterns --------------------------------------------------------

local function pattern(negative, biased, frac_hi, lo)
  local hi = frac_hi | (biased << 48)
  if negative then
    hi = hi | (1 << 63)
  end
  return hi, lo
end

local function zero_bits(negative)
  return pattern(negative, 0, 0, 0)
end

local function infinity_bits(negative)
  return pattern(negative, EXPONENT_MAX, 0, 0)
end

local function nan_bits(negative)
  -- The quiet bit is fraction bit 111, which sits at `hi`'s bit 47.
  return pattern(negative, EXPONENT_MAX, 1 << 47, 0)
end

--- Build a pattern from a sign, an integral significand and a binary
--- exponent, rounding to nearest with ties to even. The mirror of
--- `Float128._compose`.
local function compose(negative, significand, exponent)
  if bn_is_zero(significand) then
    return zero_bits(negative)
  end
  if exponent > EXPONENT_MAX then
    return infinity_bits(negative)
  end
  if exponent < -(FRACTION_BITS + 1 + EXPONENT_BIAS + 200) then
    return zero_bits(negative)
  end
  local leading = bn_bit_length(significand) - 1
  if leading > FRACTION_BITS then
    local drop = leading - FRACTION_BITS
    significand = bn_shift_round(significand, drop)
    exponent = exponent + drop
    if bn_bit_length(significand) - 1 > FRACTION_BITS then
      significand = bn_shr(significand, 1)
      exponent = exponent + 1
    end
  else
    local shift = FRACTION_BITS - leading
    significand = bn_shl(significand, shift)
    exponent = exponent - shift
  end
  local biased = exponent + EXPONENT_BIAS + FRACTION_BITS
  if biased >= EXPONENT_MAX then
    return infinity_bits(negative)
  end
  if biased <= 0 then
    -- Subnormal. A value that rounds up to the smallest normal carries
    -- out of the fraction into the exponent field on its own, so the
    -- fraction is deliberately not masked here.
    local rounded = bn_shift_round(significand, 1 - biased)
    local hi, lo = bn_to_bits128(rounded)
    if negative then
      hi = hi | (1 << 63)
    end
    return hi, lo
  end
  local hi, lo = bn_to_bits128(significand)
  return pattern(negative, biased, hi & FRACTION_HI_MASK, lo)
end

-- -- parsing -------------------------------------------------------------

--- `[sign] digits [. digits] [e [sign] digits]` as
--- `negative, significand (bignum), exponent` with the value
--- `(-1)^negative * significand * 10^exponent`. An underscore or any
--- other stray character is refused: in a value file it is a typo, and
--- reading it as a digit separator would be a quiet reinterpretation.
local function parse_decimal(text)
  local negative = false
  local body = text
  local sign = string.sub(body, 1, 1)
  if sign == "+" or sign == "-" then
    negative = sign == "-"
    body = string.sub(body, 2)
  end
  local mantissa, exponent_text = string.match(body, "^([^eE]*)[eE](.*)$")
  local exponent = 0
  if mantissa == nil then
    mantissa = body
  else
    if not string.match(exponent_text, "^[+-]?%d+$") then
      error(("`%s` has no usable exponent"):format(text), 0)
    end
    exponent = math.tointeger(tonumber(exponent_text))
    if exponent == nil then
      error(("`%s` has no usable exponent"):format(text), 0)
    end
  end
  local whole, fraction = string.match(mantissa, "^(%d*)%.(%d*)$")
  if whole == nil then
    whole, fraction = mantissa, ""
  end
  local digits = whole .. fraction
  if digits == "" or string.match(digits, "^%d+$") == nil then
    error(("`%s` is not a decimal"):format(text), 0)
  end
  local significand = {}
  for start = 1, #digits, 7 do
    local chunk = string.sub(digits, start, start + 6)
    significand = bn_mul_small(significand, 10 ^ #chunk // 1)
    significand = bn_add_small(significand, math.tointeger(tonumber(chunk)))
  end
  return negative, significand, exponent - #fraction, #digits
end

--- The pattern an exact decimal rounds to — nearest, ties to even, in
--- one rounding. The mirror of `Float128.from_decimal`.
---
--- Returns `hi, lo`.
function M.decimal_to_bits(text)
  local stripped = string.match(text, "^%s*(.-)%s*$")
  local lowered = string.lower(stripped)
  if lowered == "inf" or lowered == "+inf" or lowered == "infinity" or lowered == "+infinity" then
    return infinity_bits(false)
  end
  if lowered == "-inf" or lowered == "-infinity" then
    return infinity_bits(true)
  end
  if lowered == "nan" or lowered == "+nan" then
    return nan_bits(false)
  end
  if lowered == "-nan" then
    return nan_bits(true)
  end
  local negative, significand, exponent, digit_count = parse_decimal(stripped)
  if bn_is_zero(significand) then
    return zero_bits(negative)
  end
  -- Clamped while the numbers are still small: binary128 tops out near
  -- 1.19e4932 and bottoms out near 6.5e-4966, and a parser fed `1e50000`
  -- must not go and compute 5^50000 to find that out. The margins stay
  -- well clear of both boundaries, where the exact path below still does
  -- the rounding.
  local magnitude = exponent + digit_count
  if magnitude > 4960 then
    return infinity_bits(negative)
  end
  if magnitude < -5000 then
    return zero_bits(negative)
  end
  if exponent >= 0 then
    -- m * 10^e is exactly (m * 5^e) * 2^e.
    return compose(negative, bn_mul_pow5(significand, exponent), exponent)
  end
  -- m * 10^-k is (m * 2^s / 5^k) * 2^-(s+k): scale so the quotient
  -- carries more bits than binary128 needs, divide by 5^k a limb-sized
  -- factor at a time, and fold everything discarded into one sticky
  -- bit. That is what makes the single rounding in `compose` the
  -- correct one. 7/3 over-approximates log2(5), which only makes `s`
  -- generous.
  local k = -exponent
  local s = 130 + ((k * 7 + 2) // 3) - bn_bit_length(significand)
  if s < 0 then
    s = 0
  end
  local quotient = bn_shl(significand, s)
  local sticky = false
  local left = k
  while left >= 10 do
    local remainder
    quotient, remainder = bn_divmod_small(quotient, FIVE_10)
    sticky = sticky or remainder ~= 0
    left = left - 10
  end
  if left > 0 then
    local remainder
    quotient, remainder = bn_divmod_small(quotient, 5 ^ left // 1)
    sticky = sticky or remainder ~= 0
  end
  if sticky then
    quotient = bn_or_1(quotient)
  end
  return compose(negative, quotient, -(s + k))
end

-- -- printing ------------------------------------------------------------

--- `0.<digits> * 10^point`, in the three forms every column renders.
--- The mirror of `float128.py`'s `_render`.
local function render(negative, digits, point)
  local sign = negative and "-" or ""
  if point > 0 and point <= 34 then
    if #digits <= point then
      return sign .. digits .. string.rep("0", point - #digits) .. ".0"
    end
    return sign .. string.sub(digits, 1, point) .. "." .. string.sub(digits, point + 1)
  end
  if point > -6 and point <= 0 then
    return sign .. "0." .. string.rep("0", -point) .. digits
  end
  local rest = string.sub(digits, 2)
  if rest == "" then
    rest = "0"
  end
  local exponent = point - 1
  return sign
    .. string.sub(digits, 1, 1)
    .. "."
    .. rest
    .. "e"
    .. (exponent >= 0 and "+" or "-")
    .. math.abs(exponent)
end

--- `fraction | 2^112` — a normal value's implied leading bit,
--- materialised. Bit 112 sits in limb 5 (bits 96..119) at offset 16.
local function bn_or_implicit(fraction)
  local out = {}
  for index = 1, 5 do
    out[index] = fraction[index] or 0
  end
  out[5] = out[5] | (1 << 16)
  return bn_normalise(out)
end

--- Every digit the pattern has, in the corpus's canonical form — the
--- exact expansion, exactly as `crates/float128`, `runtime/c` and
--- `runtime/python` spell it. The mirror of
--- `Float128.to_exact_decimal`.
function M.bits_to_decimal(hi, lo)
  local negative = (hi >> 63) & 1 == 1
  local biased = (hi >> 48) & EXPONENT_MAX
  local frac_hi = hi & FRACTION_HI_MASK
  if biased == EXPONENT_MAX then
    if frac_hi == 0 and lo == 0 then
      return negative and "-inf" or "inf"
    end
    return "nan"
  end
  local significand = bn_from_fraction(frac_hi, lo)
  local exponent
  if biased == 0 then
    if bn_is_zero(significand) then
      return negative and "-0.0" or "0.0"
    end
    exponent = 1 - EXPONENT_BIAS - FRACTION_BITS
  else
    -- The implied leading one, materialised: fraction bit 112.
    significand = bn_or_implicit(significand)
    exponent = biased - EXPONENT_BIAS - FRACTION_BITS
  end
  local digits, point
  if exponent >= 0 then
    digits = bn_digits(bn_shl(significand, exponent))
    point = #digits
  else
    -- m * 2^-k is exactly m * 5^k * 10^-k.
    local scale = -exponent
    digits = bn_digits(bn_mul_pow5(significand, scale))
    point = #digits - scale
  end
  digits = string.match(digits, "^(.-)0*$")
  if digits == "" then
    digits = "0"
  end
  return render(negative, digits, point)
end

return M
