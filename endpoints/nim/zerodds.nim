# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Native pure-Nim endpoint SDK (ADR 0013): a from-scratch XCDR wire-core,
# byte-identical to the Rust core and the other SDKs. Sync (poll) and async
# (std/asyncdispatch — async/await/Future, the idiomatic Nim model).

import std/[options, asyncdispatch, deques, strutils]

type Endian* = enum
  eLE
  eBE

type Encoding* = enum
  ## The XCDR version the stream is written in.
  ##
  ## A constructor argument with no default. XTypes 1.3 §7.4.1 puts MAXALIGN at
  ## eight under XCDR1 and four under XCDR2, so a writer that did not know which
  ## one it was in would be writing XCDR2 and calling it XCDR — which is what
  ## this SDK did until the cap became a field: `align` read a constant four, and
  ## every `int64` in an XCDR1 sample landed four octets early with nothing in
  ## the interface to say so.
  eXcdr1 ## PLAIN_CDR / PL_CDR — MAXALIGN 8.
  eXcdr2 ## PLAIN_CDR2 / DELIMITED_CDR / PL_CDR2 — MAXALIGN 4.

proc maxAlign*(e: Encoding): int =
  ## The alignment cap this encoding imposes. XTypes 1.3 §7.4.1.
  if e == eXcdr1: 8 else: 4

type LengthCode* = enum
  ## The member header length codes of XTypes 1.3 §7.4.3.4.2 Table 34. The code
  ## occupies bits 28..30 of the EMHEADER and decides how the member's byte
  ## length is recovered: LC0..LC3 give it from the code alone, LC4..LC7 from
  ## the four octets that follow.
  ##
  ## An enum and not an int, so a code outside 0..7 has no spelling at all.
  lc0 ## The member body is one octet.
  lc1 ## Two octets.
  lc2 ## Four octets.
  lc3 ## Eight octets.
  lc4 ## A NEXTINT holding the body's byte length follows the EMHEADER.
  lc5 ## The body's first four octets are themselves the length, and they are
      ## part of the body. NEXTINT is not written a second time.
  lc6 ## As LC5, and the length counts four-octet elements.
  lc7 ## As LC5, and the length counts eight-octet elements.

const
  PidListEndId* = 0x3F02'u16 ## PID_LIST_END. XTypes 1.3 §7.4.1.2 Table 31.
  PidExtendedId* = 0x3F01'u16
    ## The PID that introduces the twelve-octet parameter header. §7.4.1.2.
  PidShortMaxId* = 0x3F00'u16
    ## The highest member id a short parameter header can carry, inclusive.
  PidFlagMustUnderstand = 0x4000'u16 ## Bit 14 of a short parameter id.
  EmFlagMustUnderstand = 0x4000_0000'u32
    ## Bit 30 of the eMemberHeader inside an extended parameter header — not bit
    ## 31, which is the implementation-extension flag.

type WireError* = object of ValueError
  ## A refusal, raised rather than putting a plausible but wrong octet on the
  ## wire. Every message names the clause it comes from.

proc refuse(msg: string) {.noreturn.} =
  raise newException(WireError, msg)

# --- Writer: a byte seq + endian; alignment relative to the origin ------------

type Writer* = object
  buf: seq[byte]
  endian: Endian
  maxAlign: int
    ## The alignment cap, from the Encoding the writer was made with.
  origin: int
    ## The position alignment is measured from. Zero for the stream, and moved
    ## to the start of a PL_CDR1 parameter's value for the length of that
    ## parameter — §7.4.1.2 aligns a parameter's value against its own block and
    ## not against the stream. Without it a `double` in a parameter takes four
    ## octets of padding and the header declares twelve where Cyclone, RTI and
    ## Fast DDS all declare eight.

proc initWriter*(endian: Endian, encoding: Encoding): Writer =
  ## Both arguments are required. The encoding decides MAXALIGN and nothing in
  ## the call may leave it implied.
  Writer(buf: @[], endian: endian, maxAlign: encoding.maxAlign, origin: 0)

proc align(w: var Writer, a: int) =
  let cap = min(a, w.maxAlign)
  let pad = (cap - ((w.buf.len - w.origin) mod cap)) mod cap
  for _ in 0 ..< pad:
    w.buf.add(0'u8)

# le: the value already encoded little-endian; reversed for eBE.
proc put(w: var Writer, a: int, le: seq[byte]) =
  w.align(a)
  if w.endian == eBE:
    for i in countdown(le.high, 0):
      w.buf.add(le[i])
  else:
    for b in le:
      w.buf.add(b)

proc leBytes(v: uint64, n: int): seq[byte] =
  result = newSeq[byte](n)
  for i in 0 ..< n:
    result[i] = byte((v shr (8 * i)) and 0xff)

# The integer writes take the *wire width* and no longer an `int`. They used to
# mask — `byte(v and 0xff)` and `leBytes(uint64(v), 2)` — so `putU16(70000)`
# wrote `7011` and returned, which is the silent truncation nine of the fifteen
# bindings shared. Nim refuses a literal outside the range at compile time and a
# runtime value at the conversion, so there is no spelling of it left.
proc putU8*(w: var Writer, v: uint8) = w.buf.add(v)
proc putU16*(w: var Writer, v: uint16) = w.put(2, leBytes(uint64(v), 2))
proc putU32*(w: var Writer, v: uint32) = w.put(4, leBytes(uint64(v), 4))

proc putU64*(w: var Writer, v: uint64) =
  ## A uint64 at its natural eight-octet alignment, which `maxAlign` then caps:
  ## eight under XCDR1 and four under XCDR2. XTypes 1.3 §7.4.1.
  w.put(8, leBytes(v, 8))

proc putF32*(w: var Writer, v: float32) = w.put(4, leBytes(uint64(cast[uint32](v)), 4))

proc putF64*(w: var Writer, v: float64) =
  ## A float64 as its IEEE-754 bit pattern, at the same alignment as putU64.
  ## `double` is one of the fifteen primitives of XTypes 1.3 §7.2.2.2 and had no
  ## writer here at all.
  w.putU64(cast[uint64](v))

proc putBool*(w: var Writer, v: bool) =
  ## A boolean as the one octet XTypes 1.3 §7.4.1.1.1 Table 30 gives it: 0 or 1,
  ## never any other pattern.
  w.putU8(if v: 1'u8 else: 0'u8)

proc putString*(w: var Writer, s: string) =
  w.putU32(uint32(s.len + 1))
  for c in s:
    w.buf.add(byte(c))
  w.putU8(0)

proc putSeqU8*(w: var Writer, b: seq[byte]) =
  w.putU32(uint32(b.len))
  for x in b:
    w.buf.add(x)

proc putBytes*(w: var Writer, b: seq[byte]) =
  ## Raw octets, no alignment and no length word.
  for x in b:
    w.buf.add(x)

proc putChar*(w: var Writer, codePoint: uint32) =
  ## An IDL `char`: the one octet XTypes 1.3 §7.4.1.1.1 Table 30 gives it.
  ##
  ## A code point above U+00FF is refused and not truncated. `š` is U+0161 and
  ## its low octet is 0x61, which is `a`: a truncating writer produces a frame of
  ## the right length holding a different, plausible character, and nothing
  ## downstream can tell. The Rust core refuses `primitive-char-above-latin1` in
  ## the same words.
  if codePoint > 0xFF'u32:
    refuse("expected a character below U+0100, found U+" & toHex(codePoint, 4))
  w.putU8(uint8(codePoint))

proc putWchar*(w: var Writer, codePoint: uint32) =
  ## An IDL `wchar`: one two-octet code unit, 2-aligned, in the stream's byte
  ## order.
  ##
  ## The four policy axes a wide character carries are answered here and not by
  ## the caller: the unit is two octets wide, it goes out in the stream's byte
  ## order, there is no GIOP length octet before it, and no byte-order mark.
  ## Measured against `wchar_member`, where Cyclone and Fast DDS write `6100 ab`
  ## little-endian under both encodings and RTI writes the same under XCDR2;
  ## RTI's XCDR1 `wchar_t` is four octets wide (`61000000 ab`) and is the one
  ## vendor answer this does not reproduce.
  ##
  ## A code point above U+FFFF is refused. The refusal is width-independent — a
  ## four-octet form would zero-extend a code *unit*, not a code point — which is
  ## how the Rust core refuses `primitive-wchar-above-bmp`.
  if codePoint > 0xFFFF'u32:
    refuse("expected a BMP character, found U+" & toHex(codePoint, 4))
  w.putU16(uint16(codePoint))

proc putWstring*(w: var Writer, units: seq[uint16]) =
  ## An IDL `wstring` from its UTF-16 code units.
  ##
  ## The length word counts **octets** and not code units, there is no
  ## terminator, there is no byte-order mark, and each unit goes out two octets
  ## wide in the stream's own byte order. Those are the four axes, and they are
  ## the binding's answer — the caller hands over units and nothing else.
  ##
  ## Measured against `wstring_member` (`04000000 6100 6200 ab`, Cyclone under
  ## both encodings and RTI under XCDR2) and `wstring_nonbmp`
  ## (`04000000 34d8 1edd ab`, the same two). Fast DDS writes the unit *count*
  ## there and truncates the non-BMP code point to a single unit; neither is
  ## reproduced.
  w.putU32(uint32(units.len * 2))
  for u in units:
    w.putU16(u)

proc putBitset*(w: var Writer, bits: uint64, anchor: int) =
  ## A `bitset` as its anchor word: the holder the declared bits pack into,
  ## written at the anchor's own width and therefore at the anchor's own
  ## alignment — eight under XCDR1 and four under XCDR2 for an eight-octet
  ## anchor. XTypes 1.3 §7.3.1.4 and §7.4.1.
  ##
  ## `bits` is the packed holder and `anchor` its width in octets, both
  ## properties of the declared type. What this call decides is the pair the type
  ## does not: which write the holder goes out through, and what that write
  ## aligns to.
  case anchor
  of 1: w.putU8(uint8(bits and 0xFF'u64))
  of 2: w.putU16(uint16(bits and 0xFFFF'u64))
  of 4: w.putU32(uint32(bits and 0xFFFF_FFFF'u64))
  of 8: w.putU64(bits)
  else: refuse("a bitset anchor is one, two, four or eight octets, not " & $anchor)

proc fixedOctets*(value: string, digits, scale: int): seq[byte] =
  ## The BCD packing `putFixed` writes, apart so it can be measured at the edges
  ## of its range without a Writer. CDR (OMG CORBA 3.3) §9.3.2.8 /
  ## IDL 4.2 §7.4.1.4.5.
  ##
  ## * the integer part is left-padded and the fraction right-padded to exactly
  ##   `digits` decimal digits, with the point implied `scale` from the right;
  ##   the scale itself is not on the wire,
  ## * a leading zero nibble is prepended when `digits + 1` is odd, so digits and
  ##   sign pair up,
  ## * two nibbles to the octet, most significant digit first, high nibble first,
  ## * the last nibble is the sign: 0xD negative, 0xC otherwise. A negative zero
  ##   goes out positive, because there is one zero to write,
  ## * alignment is one and no multi-octet integer is written, so the byte order
  ##   never enters.
  ##
  ## The octet count is therefore ceil((digits + 1) / 2), whatever the value.
  if digits < 1 or digits > 31:
    refuse("fixed<" & $digits & "," & $scale & ">: a digit count is 1..31")
  if scale < 0 or scale > digits:
    refuse("fixed<" & $digits & "," & $scale & ">: a scale is 0..digits")
  var text = value
  var negative = false
  if text.len > 0 and (text[0] == '+' or text[0] == '-'):
    negative = text[0] == '-'
    text = text[1 .. ^1]
  var integral = text
  var fraction = ""
  let dot = text.find('.')
  if dot >= 0:
    integral = text[0 ..< dot]
    fraction = text[dot + 1 .. ^1]
  if integral.len == 0 and fraction.len == 0:
    refuse("fixed<" & $digits & "," & $scale & ">: `" & value & "` has no digits")
  for part in [integral, fraction]:
    for c in part:
      if c < '0' or c > '9':
        refuse("fixed<" & $digits & "," & $scale & ">: `" & value & "` is not a decimal")
  # Leading zeros carry no value, so they are dropped before the width is
  # checked: `000000.001` is one significant integer digit, not six.
  var lead = 0
  while lead < integral.len and integral[lead] == '0':
    inc lead
  integral = integral[lead .. ^1]
  if integral.len > digits - scale:
    refuse("fixed<" & $digits & "," & $scale & ">: `" & value & "` needs " &
           $integral.len & " integer digits and the type has " & $(digits - scale))
  # A trailing zero in the fraction is padding either way, so it is dropped
  # before the width is checked and re-added below.
  while fraction.len > scale and fraction[^1] == '0':
    fraction = fraction[0 ..< fraction.len - 1]
  if fraction.len > scale:
    refuse("fixed<" & $digits & "," & $scale & ">: `" & value & "` needs " &
           $fraction.len & " fraction digits and the type has " & $scale)
  var nibbles: seq[byte] = @[]
  if (digits + 1) mod 2 == 1:
    nibbles.add(0'u8)
  for _ in 0 ..< digits - scale - integral.len:
    nibbles.add(0'u8)
  for c in integral:
    nibbles.add(byte(ord(c) - ord('0')))
  for c in fraction:
    nibbles.add(byte(ord(c) - ord('0')))
  for _ in 0 ..< scale - fraction.len:
    nibbles.add(0'u8)
  var nonZero = false
  for n in nibbles:
    if n != 0'u8:
      nonZero = true
  nibbles.add(if negative and nonZero: 0x0D'u8 else: 0x0C'u8)
  result = @[]
  var i = 0
  while i + 1 < nibbles.len:
    result.add(byte((nibbles[i] shl 4) or nibbles[i + 1]))
    i += 2

proc putFixed*(w: var Writer, value: string, digits, scale: int) =
  ## An IDL `fixed<digits,scale>` as packed BCD; see `fixedOctets`.
  w.putBytes(fixedOctets(value, digits, scale))

# --- back-patched framing -----------------------------------------------------
#
# A DHEADER, a NEXTINT and a PL_CDR1 parameter length are all the same shape: a
# slot reserved before the body and filled in once the body's length is known.
# `patchU32` is the one place that writes into an already-written position, and
# it honours the wire byte order like every other write.

proc patchU32(w: var Writer, at: int, v: uint32) =
  let le = leBytes(uint64(v), 4)
  for i in 0 ..< 4:
    w.buf[at + i] = (if w.endian == eBE: le[3 - i] else: le[i])

proc patchU16(w: var Writer, at: int, v: uint16) =
  let le = leBytes(uint64(v), 2)
  for i in 0 ..< 2:
    w.buf[at + i] = (if w.endian == eBE: le[1 - i] else: le[i])

proc dheaderBegin*(w: var Writer): int =
  ## Reserve a 4-aligned DHEADER slot and return the position the body starts at,
  ## to hand back to `dheaderEnd`. XTypes 1.3 §7.4.3.5.
  ##
  ## `@appendable` is the default extensibility of XTypes (§7.3.1.2.1.8), so a
  ## struct written without an annotation had no form here at all.
  w.putU32(0'u32)
  w.buf.len

proc dheaderEnd*(w: var Writer, bodyStart: int) =
  ## Back-patch the DHEADER with the byte length written since `bodyStart`. The
  ## DHEADER itself is not counted — §7.4.3.5 measures the content that follows
  ## it.
  w.patchU32(bodyStart - 4, uint32(w.buf.len - bodyStart))

proc emHeaderBegin*(w: var Writer, id: uint32, mustUnderstand: bool,
                    lc: LengthCode): int =
  ## The EMHEADER for one member of a `@mutable` aggregate, returning the
  ## body-start position. XTypes 1.3 §7.4.3.4.2:
  ##
  ##   EMHEADER = (must_understand shl 31) or (length_code shl 28) or member_id
  ##
  ## with a 28-bit member id. For LC4 a NEXTINT slot is reserved after it and
  ## `emHeaderEnd` fills it in; for LC0..LC3 the code alone gives the length and
  ## nothing follows; for LC5..LC7 the body's own leading four octets are the
  ## length, so no separate NEXTINT is written — writing it twice would put every
  ## following member at the wrong offset.
  if id > 0x0FFF_FFFF'u32:
    refuse("member id " & $id & " does not fit the 28 bits of an EMHEADER")
  var header = (uint32(ord(lc)) shl 28) or id
  if mustUnderstand:
    header = header or (1'u32 shl 31)
  w.putU32(header)
  if lc == lc4:
    w.putU32(0'u32)
  w.buf.len

proc emHeaderEnd*(w: var Writer, bodyStart: int, lc: LengthCode) =
  ## Close the member `emHeaderBegin` opened.
  ##
  ## It takes the length code back rather than remembering it, so a caller that
  ## paired an LC4 begin with a differently-coded end gets a refusal instead of a
  ## NEXTINT patched over the first four octets of a member body.
  let written = w.buf.len - bodyStart
  case lc
  of lc4:
    w.patchU32(bodyStart - 4, uint32(written))
  of lc0, lc1, lc2, lc3:
    # 1, 2, 4, 8 — the code *is* the length, so a body of any other size means
    # the plan and the member disagree. Silence here would put a length on the
    # wire that no octet count backs.
    let want = 1 shl ord(lc)
    if written != want:
      refuse("LC" & $ord(lc) & " frames " & $want & " octets and the member wrote " & $written)
  of lc5, lc6, lc7:
    if written < 4:
      refuse("LC" & $ord(lc) & " takes its length from the body's first four octets and the member wrote " & $written)

type Pid* = object
  ## An open PL_CDR1 parameter header, to be handed back to `pidEnd`.
  at: int         ## The position of the length field.
  extended: bool  ## Whether the length field is four octets or two.
  savedOrigin: int ## The alignment origin to restore when the parameter closes.

proc pidBegin*(w: var Writer, pid: uint16, mustUnderstand: bool): Pid =
  ## A PL_CDR1 short parameter header. XTypes 1.3 §7.4.1.2: at a four-octet
  ## boundary an `unsigned short` parameter id, then an `unsigned short` byte
  ## length of the value that follows. Both go out in the stream's byte order.
  ##
  ## The must-understand flag is bit 14 of the id field. Bit 15 is the
  ## implementation-extension flag and is not set here.
  ##
  ## The parameter's value aligns against the block and not against the stream,
  ## so the alignment origin moves for the length of the parameter and `pidEnd`
  ## restores it. Measured on `ext_mutable` under XCDR1, where the `double`
  ## member's header declares eight in Cyclone, RTI and Fast DDS alike — a
  ## stream-relative alignment would declare twelve.
  if pid > PidShortMaxId:
    refuse("member id " & $pid & " needs the extended parameter header")
  w.align(4)
  var id = pid
  if mustUnderstand:
    id = id or PidFlagMustUnderstand
  w.put(2, leBytes(uint64(id), 2))
  result = Pid(at: w.buf.len, extended: false, savedOrigin: w.origin)
  w.buf.add(0'u8)
  w.buf.add(0'u8)
  w.origin = w.buf.len

proc pidExtendedBegin*(w: var Writer, memberId: uint32, mustUnderstand: bool): Pid =
  ## The twelve-octet parameter header XTypes 1.3 §7.4.1.2 requires for a member
  ## id above 0x3F00 or a value longer than 65535 octets:
  ##
  ##   [PID_EXTENDED or must-understand : u16][8 : u16][eMemberHeader : u32][length : u32]
  ##
  ## The must-understand flag on the marker is not the member's — it is on the
  ## marker under this framing whatever the member says, and the member's own
  ## flag is bit 30 of the eMemberHeader. Reading it off the marker instead makes
  ## every parameter must-understand.
  if memberId > 0x0FFF_FFFF'u32:
    refuse("member id " & $memberId & " does not fit the 28 bits of an eMemberHeader")
  w.align(4)
  w.put(2, leBytes(uint64(PidExtendedId or PidFlagMustUnderstand), 2))
  w.put(2, leBytes(8'u64, 2))
  var header = memberId
  if mustUnderstand:
    header = header or EmFlagMustUnderstand
  w.put(4, leBytes(uint64(header), 4))
  result = Pid(at: w.buf.len, extended: true, savedOrigin: w.origin)
  for _ in 0 ..< 4:
    w.buf.add(0'u8)
  w.origin = w.buf.len

proc pidEnd*(w: var Writer, header: Pid) =
  ## Close the parameter `pidBegin` or `pidExtendedBegin` opened.
  ##
  ## The declared length counts the value's octets **and** the padding that
  ## brings the next header back to a four-octet boundary: a reader skips
  ## `length` from the end of the header and has to land on the next parameter.
  ## The padding itself is written after the length is patched, so the two cannot
  ## disagree. Cyclone and RTI round the same way; Fast DDS declares the value's
  ## own length and pads outside it.
  let width = if header.extended: 4 else: 2
  var length = w.buf.len - header.at - width
  # The parameter's own origin is still in force here, so the round-up is
  # measured from the block that the length describes.
  let past = (w.buf.len - header.savedOrigin) mod 4
  if past != 0:
    length += 4 - past
  if header.extended:
    w.patchU32(header.at, uint32(length))
  else:
    if length > 0xFFFF:
      refuse("a short parameter header describes at most 65535 octets, not " & $length)
    w.patchU16(header.at, uint16(length))
  w.origin = header.savedOrigin
  w.align(4)

proc pidListEnd*(w: var Writer) =
  ## The sentinel that closes a PL_CDR1 parameter list: the id PID_LIST_END with
  ## a zero length, four-aligned. XTypes 1.3 §7.4.1.2.
  ##
  ## The must-understand flag is set on the sentinel — 0x7F02 on the wire, which
  ## is what Cyclone and RTI write (`ext_mutable` XCDR1 LE ends `027f0000`). Fast
  ## DDS writes the bare 0x3F02; the flag is not the sentinel's meaning either
  ## way, and this is the majority form the Rust column records.
  w.align(4)
  w.put(2, leBytes(uint64(PidListEndId or PidFlagMustUnderstand), 2))
  w.put(2, leBytes(0'u64, 2))

proc bytes*(w: Writer): seq[byte] = w.buf

# --- Reader: a byte cursor ---

type Reader* = object
  data: seq[byte]
  pos: int
  endian: Endian
  maxAlign: int
    ## The alignment cap, from the Encoding the reader was made with — the same
    ## cap the writer used, or the fields land elsewhere.
  origin: int
    ## Mirrors the writer's: a PL_CDR1 parameter's value aligns against its own
    ## block. §7.4.1.2.

proc initReader*(data: seq[byte], endian: Endian, encoding: Encoding): Reader =
  Reader(data: data, pos: 0, endian: endian, maxAlign: encoding.maxAlign, origin: 0)

proc take(r: var Reader, a, n: int): seq[byte] =
  let cap = min(a, r.maxAlign)
  let pad = (cap - ((r.pos - r.origin) mod cap)) mod cap
  r.pos += pad
  result = r.data[r.pos ..< r.pos + n]
  r.pos += n
  if r.endian == eBE:
    var rev = newSeq[byte](n)
    for i in 0 ..< n:
      rev[i] = result[n - 1 - i]
    result = rev

proc getU32*(r: var Reader): uint32 =
  let b = r.take(4, 4)
  uint32(b[0]) or (uint32(b[1]) shl 8) or (uint32(b[2]) shl 16) or (uint32(b[3]) shl 24)

# --- primitives beyond getU32 — the byte-exact inverse of the Writer ---

proc getU8*(r: var Reader): uint8 =
  result = r.data[r.pos]
  r.pos += 1

proc getU16*(r: var Reader): uint16 =
  let b = r.take(2, 2)
  uint16(b[0]) or (uint16(b[1]) shl 8)

proc getU64*(r: var Reader): uint64 =
  ## A uint64 at its natural eight-octet alignment, which `maxAlign` then caps.
  ## It passed 4 here while the writer passed 8, which under XCDR1 read the four
  ## octets of padding as the value's low half and returned a different number
  ## with no error anywhere.
  let b = r.take(8, 8)
  var v: uint64 = 0
  for i in 0 ..< 8:
    v = v or (uint64(b[i]) shl (8 * i))
  v

proc getF32*(r: var Reader): float32 =
  cast[float32](r.getU32())

proc getF64*(r: var Reader): float64 =
  ## A float64 from its IEEE-754 bit pattern.
  cast[float64](r.getU64())

proc getBool*(r: var Reader): bool =
  ## The one octet a boolean occupies.
  r.getU8() != 0'u8

proc getChar*(r: var Reader): uint32 =
  ## The one octet an IDL `char` occupies.
  uint32(r.getU8())

proc getWchar*(r: var Reader): uint32 =
  ## One two-octet `wchar` code unit.
  uint32(r.getU16())

proc getWstring*(r: var Reader): seq[uint16] =
  ## A `wstring` back into its UTF-16 code units. The length word counts octets,
  ## so the unit count is half of it.
  let octets = int(r.getU32())
  result = @[]
  for _ in 0 ..< octets div 2:
    result.add(r.getU16())

proc getBitset*(r: var Reader, anchor: int): uint64 =
  ## A `bitset` anchor of the given width, back into its holder word.
  case anchor
  of 1: result = uint64(r.getU8())
  of 2: result = uint64(r.getU16())
  of 4: result = uint64(r.getU32())
  of 8: result = r.getU64()
  else: refuse("a bitset anchor is one, two, four or eight octets, not " & $anchor)

proc getFixed*(r: var Reader, digits, scale: int): string =
  ## A `fixed<digits,scale>` back into the decimal it was written from: a leading
  ## `-` when the sign nibble is 0xD, `digits - scale` integer digits, and a `.`
  ## with `scale` fraction digits where the scale is not zero.
  ##
  ## The scale is not on the wire, so it is an argument here for the same reason
  ## it is one in `putFixed`.
  if digits < 1 or digits > 31:
    refuse("fixed<" & $digits & "," & $scale & ">: a digit count is 1..31")
  if scale < 0 or scale > digits:
    refuse("fixed<" & $digits & "," & $scale & ">: a scale is 0..digits")
  let octets = (digits + 2) div 2
  var nibbles: seq[byte] = @[]
  for _ in 0 ..< octets:
    let b = r.getU8()
    nibbles.add(b shr 4)
    nibbles.add(b and 0x0F'u8)
  let sign = nibbles[^1]
  let body = nibbles[nibbles.len - 1 - digits ..< nibbles.len - 1]
  result = ""
  if sign == 0x0D'u8:
    result.add('-')
  for i, n in body:
    if scale != 0 and i == digits - scale:
      result.add('.')
    result.add(char(ord('0') + int(n)))

proc getString*(r: var Reader): string =
  let n = int(r.getU32())
  result = newString(n - 1)
  for i in 0 ..< n - 1:
    result[i] = char(r.data[r.pos + i])
  r.pos += n

proc getSeqU8*(r: var Reader): seq[byte] =
  let n = int(r.getU32())
  result = r.data[r.pos ..< r.pos + n]
  r.pos += n

proc dheaderRead*(r: var Reader): uint32 =
  ## Read a DHEADER and return the announced body byte length. §7.4.3.5.
  r.getU32()

type Member* = object
  ## What `emHeaderRead` recovered.
  id*: uint32
  mustUnderstand*: bool
  lc*: LengthCode
  extent*: uint32
    ## The member body's extent in octets, from the position the read left the
    ## cursor at.

proc emHeaderRead*(r: var Reader): Member =
  ## Read a member header. LC0..LC3 take the extent from the code. LC4 consumes
  ## the NEXTINT that follows the header, and the extent is that word. LC5..LC7
  ## consume nothing: the next four octets are the member's own leading word and
  ## belong to the body, so they are peeked and the extent is 4 + the word for
  ## LC5, 4 + 4×word for LC6 and 4 + 8×word for LC7 — the last two count
  ## elements, not octets.
  let header = r.getU32()
  let lc = LengthCode(int((header shr 28) and 0x7'u32))
  var extent: uint32 = 0
  case lc
  of lc0, lc1, lc2, lc3:
    extent = 1'u32 shl ord(lc)
  of lc4:
    extent = r.getU32()
  of lc5, lc6, lc7:
    let at = r.pos
    let word = r.getU32()
    r.pos = at
    extent = case lc
             of lc5: 4'u32 + word
             of lc6: 4'u32 + 4'u32 * word
             else: 4'u32 + 8'u32 * word
  Member(id: header and 0x0FFF_FFFF'u32,
         mustUnderstand: (header and (1'u32 shl 31)) != 0'u32,
         lc: lc, extent: extent)

type Parameter* = object
  ## What `pidRead` recovered.
  id*: uint32
  mustUnderstand*: bool
  length*: uint32 ## The declared byte length of the value.
  listEnd*: bool  ## Whether the list ends here.

proc pidRead*(r: var Reader): Parameter =
  ## Read a PL_CDR1 parameter header in either form.
  ##
  ## The extended form is recognised by the marker PID_EXTENDED, and the
  ## must-understand answer then comes from bit 30 of the eMemberHeader and never
  ## from the marker — the marker carries the flag under this framing whatever
  ## the member says.
  ##
  ## The value's alignment origin is moved to the position this call leaves, and
  ## `pidSkip` restores it.
  let raw = r.getU16()
  let short = raw and 0x3FFF'u16
  if short == PidListEndId:
    discard r.getU16()
    return Parameter(id: 0'u32, mustUnderstand: false, length: 0'u32, listEnd: true)
  if short == PidExtendedId:
    discard r.getU16() # the slength, always 8
    let header = r.getU32()
    let length = r.getU32()
    r.origin = r.pos
    return Parameter(id: header and 0x0FFF_FFFF'u32,
                     mustUnderstand: (header and EmFlagMustUnderstand) != 0'u32,
                     length: length, listEnd: false)
  let length = uint32(r.getU16())
  r.origin = r.pos
  Parameter(id: uint32(short),
            mustUnderstand: (raw and PidFlagMustUnderstand) != 0'u16,
            length: length, listEnd: false)

proc pidSkip*(r: var Reader, bodyStart: int, length: uint32) =
  ## Move the cursor past a parameter value of `length` octets from `bodyStart`
  ## and restore the stream's alignment origin.
  r.pos = bodyStart + int(length)
  r.origin = 0
  while r.pos mod 4 != 0:
    r.pos += 1

# --- XRCE WRITE_DATA framing ---

const SessionNoKey* = 0x80'u8
const StreamBestEffort* = 0x01'u8
## E flag (little-endian body) + DataFormat FORMAT_DATA (0b000) in bits 1..3 --
## DDS-XRCE 1.0 8.3.5.8. FORMAT_SAMPLE (0b001, i.e. 0x03) would additionally
## require a SampleInfo in the body, which this profile does not carry.
const XrceWriteFlags* = 0x01'u8
## Wire size of the BaseObjectRequest that precedes the sample (7.7.8).
const XrceBaseSize* = 4
## OBJK_DATAWRITER -- the ObjectKind of a DataWriter (7.2.2).
const ObjkDataWriter* = 0x05'u16

proc xrceObjectId*(rawId, kind: uint16): uint16 =
  ## An ObjectId (7.2.1) from a 12-bit raw id and a 4-bit ObjectKind.
  ((rawId and 0x0FFF'u16) shl 4) or (kind and 0x0F'u16)

type SampleTooLargeError* = object of ValueError
  ## A sample the 16-bit XRCE submessage_length cannot describe.

proc writeFrame*(session, stream: uint8, seqNo: int, objectId: uint16,
                 sample: seq[byte]): seq[byte] =
  ## The 16-bit submessage_length field cannot describe a sample larger than
  ## 0xFFFF, so a sample above it has no WRITE_DATA form: refuse rather than
  ## truncate the length while appending the full payload (the C SDK's
  ## `zdw_xrce_write_frame` rejects the same way).
  ##
  ## Raised and no longer an empty `seq`. `Client.write` handed the empty one
  ## to `deliver` without looking, so `writeFrame(0x80'u8, 0x01'u8, 1,
  ## newSeq[byte](70000)).len == 0` went out as an empty datagram with no error
  ## anywhere.
  let n = XrceBaseSize + sample.len
  if n > 0xFFFF:
    raise newException(
      SampleTooLargeError,
      $sample.len & " octets: the 16-bit XRCE submessage_length describes at most 65535")
  ## The submessage body is the BaseObjectRequest (7.7.8: request_id then
  ## object_id, each two octets most significant first) followed by the sample.
  result = @[session, stream,
             byte(seqNo and 0xff), byte((seqNo shr 8) and 0xff),
             0x07'u8, XrceWriteFlags,
             byte(n and 0xff), byte((n shr 8) and 0xff),
             byte((seqNo shr 8) and 0xff), byte(seqNo and 0xff),
             byte((objectId.int shr 8) and 0xff), byte(objectId.int and 0xff)]
  for x in sample:
    result.add(x)

## Frame layout: [session, stream, seq_lo, seq_hi, sm_id, flags, len_lo, len_hi]
## then the sample body. The 16-bit little-endian submessage_length (bytes 6..7)
## bounds the body exactly: frame[8 ..< 8+smLen], never frame[8 ..< frame.len],
## so trailing padding or an appended submessage is not folded into the sample.
##
## Accepts WRITE_DATA (0x07, endpoint->hub / loopback) and DATA (0x09,
## hub->endpoint) — the hub pushes samples with DATA. See DDS-XRCE spec 8.3.5.
## Returns none for a short header, a wrong submessage id, or a declared length
## that runs past the datagram (truncation / wrong length).
proc readFrame*(frame: seq[byte]): Option[seq[byte]] =
  if frame.len < 8 + XrceBaseSize or (frame[4] != 0x07'u8 and frame[4] != 0x09'u8):
    return none(seq[byte])
  let smLen = int(frame[6]) or (int(frame[7]) shl 8)
  if smLen < XrceBaseSize or 8 + smLen > frame.len:
    return none(seq[byte])
  some(frame[8 + XrceBaseSize ..< 8 + smLen])

proc frameBase*(frame: seq[byte]): Option[(uint16, uint16)] =
  ## `(requestId, objectId)` of a data frame (7.7.8), or none under the same
  ## rules as `readFrame`.
  if readFrame(frame).isNone:
    return none((uint16, uint16))
  some(((uint16(frame[8]) shl 8) or uint16(frame[9]),
        (uint16(frame[10]) shl 8) or uint16(frame[11])))

# --- transport ---

type Transport* = object
  deliver*: proc(frame: seq[byte]) {.closure.}
  receive*: proc(): Option[seq[byte]] {.closure.}

# --- sync client ---

type Client* = ref object
  transport: Transport
  session, stream: uint8
  ## ObjectId of the target DataWriter (7.7.8 BaseObjectRequest).
  objectId: uint16
  seqNo: int

proc newClient*(t: Transport, objectId: uint16): Client =
  Client(transport: t, session: SessionNoKey, stream: StreamBestEffort,
         objectId: objectId, seqNo: 1)

proc write*(c: Client, sample: seq[byte]) =
  ## A sample the envelope cannot carry raises out of `writeFrame`, so the
  ## sequence is not advanced and nothing reaches the transport.
  let frame = writeFrame(c.session, c.stream, c.seqNo, c.objectId, sample)
  c.transport.deliver(frame)
  c.seqNo = (c.seqNo + 1) and 0xffff

# One non-blocking receive: returns the sample body, or none.
proc poll*(c: Client): Option[seq[byte]] =
  let f = c.transport.receive()
  if f.isSome: readFrame(f.get) else: none(seq[byte])

# --- async reader: an async/await Future (idiomatic Nim) ---

type AsyncReader* = ref object
  transport: Transport

proc newAsyncReader*(t: Transport): AsyncReader =
  AsyncReader(transport: t)

proc recv*(r: AsyncReader): Future[seq[byte]] {.async.} =
  while true:
    let f = r.transport.receive()
    if f.isNone:
      await sleepAsync(1)
    else:
      let body = readFrame(f.get)
      if body.isSome:
        return body.get

# In-memory FIFO transport for tests + example.
proc memTransport*(): Transport =
  var q = initDeque[seq[byte]]()
  Transport(
    deliver: proc(frame: seq[byte]) = q.addLast(frame),
    receive: proc(): Option[seq[byte]] =
      if q.len > 0: some(q.popFirst()) else: none(seq[byte])
  )
