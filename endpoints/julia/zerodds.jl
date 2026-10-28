# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Native pure-Julia endpoint SDK (ADR 0013): a from-scratch XCDR wire-core,
# byte-identical to the Rust core and the other SDKs. Sync (poll) and async
# (a Task filling a Channel — the idiomatic Julia concurrency model).

module ZeroDDS

@enum Endian LE BE

"""
    Encoding

The XCDR version the stream is written in. A constructor argument with no
default: XTypes 1.3 §7.4.1 puts MAXALIGN at eight under XCDR1 and four under
XCDR2, so a writer that did not know which one it was in would be writing
XCDR2 and calling it XCDR — which is what this SDK did until the cap became
a field. `align!` read a constant four, and every `long long` in an XCDR1
sample landed four octets early with nothing in the interface to say so.
"""
@enum Encoding XCDR1 XCDR2

"The alignment cap this encoding imposes. XTypes 1.3 §7.4.1."
max_alignment(e::Encoding) = e === XCDR1 ? 8 : 4

# The member header length codes of XTypes 1.3 §7.4.3.4.2 Table 34. The code
# occupies bits 28..30 of the EMHEADER and decides how the member's byte
# length is recovered: LC0..LC3 give it from the code alone, LC4..LC7 from a
# word that follows or from the body's own leading word.
const LC0 = 0   # the member body is one octet
const LC1 = 1   # two octets
const LC2 = 2   # four octets
const LC3 = 3   # eight octets
const LC4 = 4   # a NEXTINT holding the body's byte length follows the header
const LC5 = 5   # the body's first four octets are the length, and are body
const LC6 = 6   # as LC5, and the length counts four-octet elements
const LC7 = 7   # as LC5, and the length counts eight-octet elements

"PID_LIST_END. XTypes 1.3 §7.4.1.2 Table 31."
const PID_LIST_END_ID = 0x3F02
"The PID that introduces the twelve-octet parameter header. §7.4.1.2."
const PID_EXTENDED_ID = 0x3F01
"The highest member id a short parameter header carries, inclusive."
const PID_SHORT_MAX_ID = 0x3F00
# Bit 14 of a short parameter id. Bit 15 is the implementation-extension
# flag and is never set here.
const PID_FLAG_MUST_UNDERSTAND = 0x4000
# Bit 30 of the eMemberHeader inside an extended parameter header — not bit
# 31, which is the impl-extension flag.
const EM_FLAG_MUST_UNDERSTAND = 0x4000_0000

# --- Writer: a UInt8 buffer + endian; alignment relative to the origin ---

mutable struct Writer
    buf::Vector{UInt8}
    endian::Endian
    # The alignment cap, from the Encoding the writer was made with.
    max_align::Int
    # The position alignment is measured from. Zero for the stream, and
    # moved to the start of a PL_CDR1 parameter's value for the length of
    # that parameter — §7.4.1.2 aligns a parameter's value against its own
    # block, not against the stream. Without it a `double` in a parameter
    # takes four octets of padding and the header declares twelve where
    # Cyclone, RTI and Fast DDS all declare eight.
    origin::Int
end
Writer(endian::Endian, encoding::Encoding) =
    Writer(UInt8[], endian, max_alignment(encoding), 0)

function align!(w::Writer, a::Int)
    cap = min(a, w.max_align)
    pad = mod(cap - mod(length(w.buf) - w.origin, cap), cap)
    for _ in 1:pad
        push!(w.buf, 0x00)
    end
end

# le: bytes already encoded little-endian; reversed for BE.
function emit!(w::Writer, a::Int, le::Vector{UInt8})
    align!(w, a)
    append!(w.buf, w.endian == BE ? reverse(le) : le)
end

le_bytes(v::Unsigned, n::Int) = UInt8[UInt8((v >> (8 * (i - 1))) & 0xff) for i in 1:n]

"""
    pattern(v, bits)

The two's complement pattern of `v` in a holder of `bits`, or a refusal.

Both spellings of an integer argument reach the wire through the same call:
the unsigned pattern a `uint16` carries and the signed value an `int16`
carries. Both are taken, and the bound is the union of the two ranges, which
is the same bound `narrow` in `tools/wireprobe/wireprobe-endpoint-gen` uses
to decide that a sample fits its member.

Refused and not masked. `put_u8!(w, 300)` was `UInt8(300 & 0xff)` and wrote
`0x2C`: a frame of the right length carrying a different number, which no
reader downstream can tell from the right one. The arithmetic is `Int128` and
not `Int` because Julia's `Int` is the platform's — `Int32` on a 32-bit
target — and a check that overflowed at 32 bits would pass exactly the values
it exists to refuse.

A non-integer is refused before the range is looked at. `2^53 + 1` is a
`Float64` in Julia, because `^` returns a float whatever its arguments are,
and its value is `9007199254740992.0` — one less than what was written.
`Int128` of it is an exact conversion of the wrong number, so the octets
would be a plausible integer nobody asked for.
"""
function pattern(v, bits::Int)
    v isa Integer || throw(ArgumentError(
        "$v is a $(typeof(v)) and an integer write takes an Integer; a float that \
         happens to be integral has already lost whatever did not fit its mantissa"))
    value = Int128(v)
    top = (Int128(1) << bits) - 1
    bottom = -(Int128(1) << (bits - 1))
    if value > top || value < bottom
        throw(ArgumentError("$value does not fit $bits bits"))
    end
    UInt64(value & top)
end

put_u8!(w::Writer, v) = push!(w.buf, UInt8(pattern(v, 8)))
put_u16!(w::Writer, v) = emit!(w, 2, le_bytes(UInt16(pattern(v, 16)), 2))
put_u32!(w::Writer, v) = emit!(w, 4, le_bytes(UInt32(pattern(v, 32)), 4))

"""
    put_u64!(w, v)

A 64-bit integer at its natural eight-octet alignment, which `max_align` then
caps: eight under XCDR1 and four under XCDR2. XTypes 1.3 §7.4.1. It passed 4
here regardless of the encoding, so every `long long` in an XCDR1 sample
landed four octets early.
"""
put_u64!(w::Writer, v) = emit!(w, 8, le_bytes(pattern(v, 64), 8))

put_f32!(w::Writer, v) = emit!(w, 4, le_bytes(reinterpret(UInt32, Float32(v)), 4))

"""
    put_f64!(w, v)

Binary64 as its IEEE-754 bit pattern, at the same alignment as `put_u64!`.
`double` is one of the fifteen primitives of XTypes 1.3 §7.2.2.2 and had no
writer here at all.
"""
put_f64!(w::Writer, v) = emit!(w, 8, le_bytes(reinterpret(UInt64, Float64(v)), 8))

"A `boolean` as the one octet §7.4.1.1.1 Table 30 gives it: 0 or 1."
put_bool!(w::Writer, v::Bool) = put_u8!(w, v ? 1 : 0)

put_bytes!(w::Writer, b) = append!(w.buf, b)

function put_string!(w::Writer, s::AbstractString)
    put_u32!(w, sizeof(s) + 1)
    append!(w.buf, codeunits(s))
    put_u8!(w, 0)
end

function put_seq_u8!(w::Writer, b::Vector{UInt8})
    put_u32!(w, length(b))
    append!(w.buf, b)
end

"""
    put_char!(w, code_point)

An IDL `char`: the one octet XTypes 1.3 §7.4.1.1.1 Table 30 gives it.

A code point above U+00FF is refused and not truncated. `š` is U+0161 and its
low octet is 0x61, which is `a`: a truncating writer produces a frame of the
right length holding a different, plausible character and nothing downstream
can tell.
"""
function put_char!(w::Writer, code_point)
    cp = Int128(code_point)
    (0 <= cp <= 0xFF) ||
        throw(ArgumentError("expected a character below U+0100, found U+$(string(cp, base=16, pad=4))"))
    put_u8!(w, cp)
end

"""
    put_wchar!(w, code_point)

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
function put_wchar!(w::Writer, code_point)
    cp = Int128(code_point)
    (0 <= cp <= 0xFFFF) ||
        throw(ArgumentError("expected a BMP character, found U+$(string(cp, base=16, pad=4))"))
    put_u16!(w, cp)
end

"""
    put_wstring!(w, units)

An IDL `wstring` from its UTF-16 code units.

The length word counts **octets** and not code units, there is no terminator,
there is no byte-order mark, and each unit goes out two octets wide in the
stream's own byte order. Those are the four axes and they are the binding's
answer — the caller hands over units and nothing else. Cyclone writes exactly
this in both encodings and RTI under XCDR2; Fast DDS counts units, which is
the reading this binding does not take.
"""
function put_wstring!(w::Writer, units::Vector{UInt16})
    put_u32!(w, length(units) * 2)
    for unit in units
        put_u16!(w, unit)
    end
end

"""
    put_bitset!(w, bits, anchor)

A `bitset` as its anchor word: the holder the declared bits pack into,
written at the anchor's own width and therefore at the anchor's own
alignment. XTypes 1.3 §7.3.1.4 and §7.4.1.

`bits` is the packed holder and `anchor` its width in octets, both properties
of the declared type. What this call decides is the pair the type does not:
which write the holder goes out through, and what that write aligns to.
"""
function put_bitset!(w::Writer, bits::UInt64, anchor::Int)
    if anchor == 1
        put_u8!(w, bits)
    elseif anchor == 2
        put_u16!(w, bits)
    elseif anchor == 4
        put_u32!(w, bits)
    elseif anchor == 8
        put_u64!(w, bits)
    else
        throw(ArgumentError("a bitset anchor is one, two, four or eight octets, not $anchor"))
    end
end

"""
    put_fixed!(w, value, digits, scale)

An IDL `fixed<digits,scale>` as packed BCD.
CDR (OMG CORBA 3.3) §9.3.2.8 / IDL 4.2 §7.4.1.4.5.

`value` is the decimal as the sample wrote it — an optional sign, an integer
part and an optional fraction. Everything after that is this call's:

  * the integer part is left-padded and the fraction right-padded to exactly
    `digits` decimal digits, with the point implied `scale` from the right;
    the scale itself is not on the wire,
  * a leading zero nibble is prepended when `digits + 1` is odd, so digits and
    sign pair up,
  * two nibbles to the octet, most significant digit first, high nibble first,
  * the last nibble is the sign: 0xD negative, 0xC otherwise. A negative zero
    goes out positive, because there is one zero to write,
  * alignment is one, and no multi-octet integer is written, so the byte order
    never enters.

The octet count is therefore `ceil((digits + 1) / 2)` whatever the value,
which is the point omniORB 4.3.3 and JacORB 3.9 differ on and the one this
binding takes from omniORB.
"""
put_fixed!(w::Writer, value::AbstractString, digits::Int, scale::Int) =
    append!(w.buf, fixed_octets(value, digits, scale))

"The packing `put_fixed!` writes, apart so it can be tested without a Writer."
function fixed_octets(value::AbstractString, digits::Int, scale::Int)
    (1 <= digits <= 31) ||
        throw(ArgumentError("fixed<$digits,$scale>: a digit count is 1..31"))
    (0 <= scale <= digits) ||
        throw(ArgumentError("fixed<$digits,$scale>: a scale is 0..digits"))
    chars = collect(value)
    negative = false
    if !isempty(chars) && (chars[1] == '+' || chars[1] == '-')
        negative = chars[1] == '-'
        chars = chars[2:end]
    end
    at = findfirst(==('.'), chars)
    integral, fraction = at === nothing ? (chars, Char[]) :
                         (chars[1:(at - 1)], chars[(at + 1):end])
    (isempty(integral) && isempty(fraction)) &&
        throw(ArgumentError("fixed<$digits,$scale>: `$value` has no digits"))
    for part in (integral, fraction), c in part
        ('0' <= c <= '9') ||
            throw(ArgumentError("fixed<$digits,$scale>: `$value` is not a decimal"))
    end
    # Leading zeros carry no value, so they go before the width is checked:
    # `000000.001` is one significant integer digit, not six.
    trimmed = 1
    while trimmed <= length(integral) && integral[trimmed] == '0'
        trimmed += 1
    end
    integral = integral[trimmed:end]
    length(integral) <= digits - scale || throw(ArgumentError(
        "fixed<$digits,$scale>: `$value` needs $(length(integral)) integer digits and the type has $(digits - scale)"))
    # A trailing zero in the fraction is padding either way, so it goes
    # before the width is checked and is re-added below.
    while length(fraction) > scale && fraction[end] == '0'
        fraction = fraction[1:(end - 1)]
    end
    length(fraction) <= scale || throw(ArgumentError(
        "fixed<$digits,$scale>: `$value` needs $(length(fraction)) fraction digits and the type has $scale"))

    nibbles = UInt8[]
    isodd(digits + 1) && push!(nibbles, 0x00)
    for _ in 1:(digits - scale - length(integral))
        push!(nibbles, 0x00)
    end
    for c in integral
        push!(nibbles, UInt8(c - '0'))
    end
    for c in fraction
        push!(nibbles, UInt8(c - '0'))
    end
    for _ in 1:(scale - length(fraction))
        push!(nibbles, 0x00)
    end
    push!(nibbles, (negative && any(!iszero, nibbles)) ? 0x0D : 0x0C)
    UInt8[(nibbles[i] << 4) | nibbles[i + 1] for i in 1:2:(length(nibbles) - 1)]
end

bytes(w::Writer) = w.buf

# --- back-patched framing ------------------------------------------------
#
# A DHEADER, a NEXTINT and a PL_CDR1 parameter length are all the same shape:
# a slot reserved before the body and filled in once the body's length is
# known. These two are the only places that write into an already-written
# position, and they honour the wire byte order like every other write.
#
# `at` is a 0-based buffer offset throughout, so the arithmetic reads the
# same as the other fourteen bindings; the indexing adds the 1 Julia wants.

function patch_u32!(w::Writer, at::Int, v::UInt32)
    le = le_bytes(v, 4)
    w.endian == BE && (le = reverse(le))
    for i in 1:4
        w.buf[at + i] = le[i]
    end
end

function patch_u16!(w::Writer, at::Int, v::UInt16)
    le = le_bytes(v, 2)
    w.endian == BE && (le = reverse(le))
    w.buf[at + 1] = le[1]
    w.buf[at + 2] = le[2]
end

"""
    dheader_begin!(w) -> Int

Reserve a 4-aligned DHEADER slot and return the position the body starts at,
to hand back to `dheader_end!`. XTypes 1.3 §7.4.3.5.

`@appendable` is the default extensibility of XTypes (§7.3.1.2.1.8), so a
struct written with no annotation at all had no form here before this.
"""
function dheader_begin!(w::Writer)
    put_u32!(w, 0)
    length(w.buf)
end

"""
    dheader_end!(w, body_start)

Back-patch the DHEADER with the byte length written since `body_start`. The
DHEADER itself is not counted — §7.4.3.5 measures the content that follows it.
"""
dheader_end!(w::Writer, body_start::Int) =
    patch_u32!(w, body_start - 4, UInt32(length(w.buf) - body_start))

"""
    emheader_begin!(w, id, must_understand, length_code) -> Int

The EMHEADER for one member of a `@mutable` aggregate. XTypes 1.3 §7.4.3.4.2:

    EMHEADER = (must_understand << 31) | (length_code << 28) | member_id

with a 28-bit member id. For LC4 a NEXTINT slot is reserved after it and
`emheader_end!` fills it in; for LC0..LC3 the code alone gives the length and
nothing follows; for LC5..LC7 the body's own leading four octets are the
length, so no separate NEXTINT is written and writing one would put every
following member at the wrong offset.
"""
function emheader_begin!(w::Writer, id, must_understand::Bool, length_code::Int)
    member = UInt64(id)
    member <= 0x0FFF_FFFF ||
        throw(ArgumentError("member id $member does not fit the 28 bits of an EMHEADER"))
    (LC0 <= length_code <= LC7) ||
        throw(ArgumentError("length code $length_code is not one of LC0..LC7"))
    header = (UInt32(length_code) << 28) | UInt32(member)
    must_understand && (header |= UInt32(1) << 31)
    put_u32!(w, header)
    length_code == LC4 && put_u32!(w, 0)
    length(w.buf)
end

"""
    emheader_end!(w, body_start, length_code)

Close the member `emheader_begin!` opened.

The length code is taken back rather than remembered, so a caller that paired
an LC4 begin with a differently-coded end gets a refusal instead of a NEXTINT
patched over the first four octets of a member body.
"""
function emheader_end!(w::Writer, body_start::Int, length_code::Int)
    written = length(w.buf) - body_start
    if length_code == LC4
        patch_u32!(w, body_start - 4, UInt32(written))
    elseif length_code <= LC3
        # 1, 2, 4, 8 — the code *is* the length, so a body of any other size
        # means the plan and the member disagree. Silence here would put a
        # length on the wire that no octet count backs.
        want = 1 << length_code
        written == want || throw(ArgumentError(
            "LC$length_code frames $want octets and the member wrote $written"))
    else
        written >= 4 || throw(ArgumentError(
            "LC$length_code takes its length from the body's first four octets and the member wrote $written"))
    end
    nothing
end

"An open PL_CDR1 parameter header, to be handed back to `pid_end!`."
struct Pid
    at::Int          # the position of the length field
    extended::Bool   # whether the length field is four octets or two
    saved_origin::Int
end

"""
    pid_begin!(w, pid, must_understand) -> Pid

A PL_CDR1 short parameter header. XTypes 1.3 §7.4.1.2: at a four-octet
boundary an `unsigned short` parameter id, then an `unsigned short` byte
length of the value that follows. Both go out in the stream's byte order.

The must-understand flag is bit 14 of the id field. Bit 15 is the
implementation-extension flag and is not set here.

The parameter's value aligns against the block and not against the stream, so
the alignment origin moves for the length of the parameter and `pid_end!`
restores it. 0x3F00 is the last id the short form carries, inclusive — which
is what RTI and Fast DDS write for `@id(16128)`.
"""
function pid_begin!(w::Writer, pid, must_understand::Bool)
    id = UInt64(pid)
    id <= PID_SHORT_MAX_ID ||
        throw(ArgumentError("member id $id needs the extended parameter header"))
    align!(w, 4)
    field = UInt16(id)
    must_understand && (field |= UInt16(PID_FLAG_MUST_UNDERSTAND))
    emit!(w, 2, le_bytes(field, 2))
    at = length(w.buf)
    append!(w.buf, UInt8[0x00, 0x00])
    saved = w.origin
    w.origin = length(w.buf)
    Pid(at, false, saved)
end

"""
    pid_extended_begin!(w, member_id, must_understand) -> Pid

The twelve-octet parameter header XTypes 1.3 §7.4.1.2 requires for a member
id above 0x3F00 or a value longer than 65535 octets:

    [PID_EXTENDED | must-understand : u16][8 : u16][eMemberHeader : u32][length : u32]

The must-understand flag on the marker is not the member's — it is on the
marker under this framing whatever the member says, and the member's own flag
is bit 30 of the eMemberHeader. Reading it off the marker instead makes every
parameter must-understand.
"""
function pid_extended_begin!(w::Writer, member_id, must_understand::Bool)
    id = UInt64(member_id)
    id <= 0x0FFF_FFFF ||
        throw(ArgumentError("member id $id does not fit the 28 bits of an eMemberHeader"))
    align!(w, 4)
    marker = UInt16(PID_EXTENDED_ID) | UInt16(PID_FLAG_MUST_UNDERSTAND)
    emit!(w, 2, le_bytes(marker, 2))
    emit!(w, 2, le_bytes(UInt16(8), 2))
    header = UInt32(id)
    must_understand && (header |= UInt32(EM_FLAG_MUST_UNDERSTAND))
    emit!(w, 4, le_bytes(header, 4))
    at = length(w.buf)
    append!(w.buf, UInt8[0x00, 0x00, 0x00, 0x00])
    saved = w.origin
    w.origin = length(w.buf)
    Pid(at, true, saved)
end

"""
    pid_end!(w, header)

Close the parameter `pid_begin!` or `pid_extended_begin!` opened.

The declared length counts the value's octets **and** the padding that brings
the next header back to a four-octet boundary: a reader skips `length` from
the end of the header and has to land on the next parameter. The padding is
written after the length is patched, so the two cannot disagree.
"""
function pid_end!(w::Writer, header::Pid)
    width = header.extended ? 4 : 2
    len = length(w.buf) - header.at - width
    # The parameter's own origin is still in force here, so the round-up is
    # measured from the block that the length describes.
    past = mod(length(w.buf) - header.saved_origin, 4)
    past != 0 && (len += 4 - past)
    if header.extended
        patch_u32!(w, header.at, UInt32(len))
    else
        len <= 0xFFFF || throw(ArgumentError(
            "a short parameter header describes at most 65535 octets, not $len"))
        patch_u16!(w, header.at, UInt16(len))
    end
    w.origin = header.saved_origin
    align!(w, 4)
end

"""
    pid_list_end!(w)

The sentinel that closes a PL_CDR1 parameter list: the id PID_LIST_END with a
zero length, four-aligned. XTypes 1.3 §7.4.1.2.

The must-understand flag is set on the sentinel — 0x7F02 on the wire, which
is what Cyclone and RTI write. Fast DDS writes the bare 0x3F02; the flag is
not the sentinel's meaning either way, and this is the majority form.
"""
function pid_list_end!(w::Writer)
    align!(w, 4)
    id = UInt16(PID_LIST_END_ID) | UInt16(PID_FLAG_MUST_UNDERSTAND)
    emit!(w, 2, le_bytes(id, 2))
    emit!(w, 2, le_bytes(UInt16(0), 2))
end

# --- Reader: a byte cursor (pos is 0-based) ---

mutable struct Reader
    data::Vector{UInt8}
    pos::Int
    endian::Endian
    # The same cap the writer used, or the fields land elsewhere.
    max_align::Int
    # Mirrors the writer's: a PL_CDR1 parameter's value aligns against its
    # own block.
    origin::Int
end
Reader(data::Vector{UInt8}, endian::Endian, encoding::Encoding) =
    Reader(data, 0, endian, max_alignment(encoding), 0)

function take_bytes!(r::Reader, a::Int, n::Int)
    cap = min(a, r.max_align)
    pad = mod(cap - mod(r.pos - r.origin, cap), cap)
    r.pos += pad
    chunk = r.data[(r.pos + 1):(r.pos + n)]
    r.pos += n
    r.endian == BE ? reverse(chunk) : chunk
end

function get_u32(r::Reader)
    b = take_bytes!(r, 4, 4)
    UInt32(b[1]) | (UInt32(b[2]) << 8) | (UInt32(b[3]) << 16) | (UInt32(b[4]) << 24)
end

# --- primitives beyond get_u32 — the byte-exact inverse of the Writer ---

function get_u8(r::Reader)
    v = r.data[r.pos + 1]
    r.pos += 1
    v
end

function get_u16(r::Reader)
    b = take_bytes!(r, 2, 2)
    UInt16(b[1]) | (UInt16(b[2]) << 8)
end

"""
    get_u64(r)

A 64-bit integer at its natural eight-octet alignment, which `max_align` then
caps — the same eight the writer passes. It passed 4 here while the writer
passed 4 as well, so both were wrong together under XCDR1 and the round trip
never showed it.
"""
function get_u64(r::Reader)
    b = take_bytes!(r, 8, 8)
    v = UInt64(0)
    for i in 1:8
        v |= UInt64(b[i]) << (8 * (i - 1))
    end
    v
end

get_f32(r::Reader) = reinterpret(Float32, get_u32(r))
get_f64(r::Reader) = reinterpret(Float64, get_u64(r))
get_bool(r::Reader) = get_u8(r) != 0x00
"The one octet an IDL `char` occupies."
get_char(r::Reader) = UInt32(get_u8(r))
"One two-octet `wchar` code unit."
get_wchar(r::Reader) = UInt32(get_u16(r))

"""
    get_wstring(r)

A `wstring` back into its UTF-16 code units. The length word counts octets,
so the unit count is half of it.
"""
function get_wstring(r::Reader)
    octets = Int(get_u32(r))
    UInt16[get_u16(r) for _ in 1:(octets ÷ 2)]
end

"A `bitset` anchor of the given width back into its holder word."
function get_bitset(r::Reader, anchor::Int)
    if anchor == 1
        UInt64(get_u8(r))
    elseif anchor == 2
        UInt64(get_u16(r))
    elseif anchor == 4
        UInt64(get_u32(r))
    elseif anchor == 8
        get_u64(r)
    else
        throw(ArgumentError("a bitset anchor is one, two, four or eight octets, not $anchor"))
    end
end

"""
    get_fixed(r, digits, scale)

A `fixed<digits,scale>` back into the decimal it was written from: a leading
`-` when the sign nibble is 0xD, `digits - scale` integer digits, and a `.`
with `scale` fraction digits where the scale is not zero. The scale is not on
the wire, so it is an argument here for the same reason it is one in
`put_fixed!`.
"""
function get_fixed(r::Reader, digits::Int, scale::Int)
    octets = (digits + 2) ÷ 2
    nibbles = UInt8[]
    for _ in 1:octets
        b = get_u8(r)
        push!(nibbles, b >> 4)
        push!(nibbles, b & 0x0F)
    end
    sign = nibbles[end]
    body = nibbles[(end - digits):(end - 1)]
    out = IOBuffer()
    sign == 0x0D && print(out, '-')
    for (i, n) in enumerate(body)
        scale != 0 && i == digits - scale + 1 && print(out, '.')
        print(out, '0' + n)
    end
    String(take!(out))
end

function get_string(r::Reader)
    n = Int(get_u32(r))
    s = String(r.data[(r.pos + 1):(r.pos + n - 1)])
    r.pos += n
    s
end

function get_seq_u8(r::Reader)
    n = Int(get_u32(r))
    b = r.data[(r.pos + 1):(r.pos + n)]
    r.pos += n
    b
end

"A DHEADER, as the announced body byte length."
dheader_read(r::Reader) = get_u32(r)

"""
    emheader_read(r) -> (id, must_understand, length_code, extent)

A member header, and the member body's extent in octets measured from the
position this call leaves the cursor at.

LC0..LC3 take the extent from the code. LC4 consumes the NEXTINT that follows
the header. LC5..LC7 consume nothing: the next four octets are the member's
own leading word and belong to the body, so they are peeked and the extent is
`4 + word` for LC5, `4 + 4*word` for LC6 and `4 + 8*word` for LC7 — the last
two count elements, not octets.
"""
function emheader_read(r::Reader)
    header = get_u32(r)
    id = header & 0x0FFF_FFFF
    must_understand = (header & (UInt32(1) << 31)) != 0
    length_code = Int((header >> 28) & 0x7)
    extent = if length_code <= LC3
        UInt32(1) << length_code
    elseif length_code == LC4
        get_u32(r)
    else
        at = r.pos
        word = get_u32(r)
        r.pos = at
        length_code == LC5 ? UInt32(4) + word :
        length_code == LC6 ? UInt32(4) + UInt32(4) * word : UInt32(4) + UInt32(8) * word
    end
    (id, must_understand, length_code, extent)
end

"""
    pid_read(r) -> (id, must_understand, length, list_end)

A PL_CDR1 parameter header in either form, with the id's flags stripped.

The extended form is recognised by the marker PID_EXTENDED, and the
must-understand answer then comes from bit 30 of the eMemberHeader and never
from the marker — the marker carries the flag under this framing whatever the
member says.

The value's alignment origin is moved to the position this call leaves, and
`pid_skip!` restores it.
"""
function pid_read(r::Reader)
    raw = get_u16(r)
    short = raw & 0x3FFF
    if short == PID_LIST_END_ID
        get_u16(r)
        return (UInt32(0), false, UInt32(0), true)
    end
    if short == PID_EXTENDED_ID
        get_u16(r)  # the slength, always 8
        header = get_u32(r)
        len = get_u32(r)
        r.origin = r.pos
        return (header & 0x0FFF_FFFF, (header & UInt32(EM_FLAG_MUST_UNDERSTAND)) != 0, len, false)
    end
    len = UInt32(get_u16(r))
    r.origin = r.pos
    (UInt32(short), (raw & UInt16(PID_FLAG_MUST_UNDERSTAND)) != 0, len, false)
end

"""
    pid_skip!(r, body_start, length)

Move the cursor past a parameter value of `length` octets from `body_start`
and restore the stream's alignment origin.
"""
function pid_skip!(r::Reader, body_start::Int, len)
    r.pos = body_start + Int(len)
    r.origin = 0
    while mod(r.pos, 4) != 0
        r.pos += 1
    end
end

# --- XRCE WRITE_DATA framing ---

const SESSION_NOKEY = 0x80
const STREAM_BEST_EFFORT = 0x01
# E flag (little-endian body) + DataFormat FORMAT_DATA (0b000) in bits 1..3 --
# DDS-XRCE 1.0 8.3.5.8. FORMAT_SAMPLE (0b001, i.e. 0x03) would additionally
# require a SampleInfo in the body, which this profile does not carry.
const XRCE_WRITE_FLAGS = 0x01
# Wire size of the BaseObjectRequest that precedes the sample (7.7.8).
const XRCE_BASE_SIZE = 4
# OBJK_DATAWRITER -- the ObjectKind of a DataWriter (7.2.2).
const OBJK_DATAWRITER = 0x05

"An ObjectId (7.2.1) from a 12-bit raw id and a 4-bit ObjectKind."
xrce_object_id(raw_id, kind) = ((raw_id & 0x0FFF) << 4) | (kind & 0x0F)

"""
    write_frame(session, stream, seq, sample) -> Vector{UInt8}

The 16-bit submessage_length field cannot describe a sample larger than 0xFFFF,
so a sample above it has no WRITE_DATA form: refuse rather than truncate the
length while appending the full payload (the C SDK's `zdw_xrce_write_frame`
rejects the same way).

Thrown and no longer an empty vector. `write!` handed the empty one to
`deliver` without looking, so
`write_frame(0x80, 0x01, 1, zeros(UInt8, 70000)) == UInt8[]` went out as an
empty datagram with no error anywhere.
"""
function write_frame(session, stream, seq, object_id, sample::Vector{UInt8})
    n = XRCE_BASE_SIZE + length(sample)
    n > 0xFFFF && throw(ArgumentError(
        "$(length(sample)) octets: the 16-bit XRCE submessage_length describes at most 65535"))
    hdr = UInt8[
        UInt8(session), UInt8(stream),
        UInt8(seq & 0xff), UInt8((seq >> 8) & 0xff),
        0x07, XRCE_WRITE_FLAGS,
        UInt8(n & 0xff), UInt8((n >> 8) & 0xff),
        # BaseObjectRequest (7.7.8): request_id then object_id, each two octets
        # most significant first. `object_id` names the target DataWriter --
        # without it no XRCE agent can route the sample. The request_id is the
        # message's own `seq`, unique per stream as 7.7.8 requires.
        UInt8((seq >> 8) & 0xff), UInt8(seq & 0xff),
        UInt8((object_id >> 8) & 0xff), UInt8(object_id & 0xff),
    ]
    vcat(hdr, sample)
end

# Frame layout (1-based): [session, stream, seq_lo, seq_hi, sm_id, flags, len_lo,
# len_hi] at indices 1..8, then the sample body from index 9. The 16-bit
# little-endian submessage_length (indices 7..8) bounds the body exactly:
# frame[9 : 8+smLen], never frame[9:end], so trailing padding or an appended
# submessage is not folded into the sample.
#
# Accepts WRITE_DATA (0x07, endpoint->hub / loopback) and DATA (0x09,
# hub->endpoint) — the hub pushes samples with DATA. See DDS-XRCE spec 8.3.5.
# Returns nothing for a short header, a wrong submessage id, or a declared
# length that runs past the datagram (truncation / wrong length).
function read_frame(frame::Vector{UInt8})
    (length(frame) >= 8 + XRCE_BASE_SIZE &&
     (frame[5] == 0x07 || frame[5] == 0x09)) || return nothing
    sm_len = Int(frame[7]) | (Int(frame[8]) << 8)
    (sm_len < XRCE_BASE_SIZE || 8 + sm_len > length(frame)) && return nothing
    frame[9 + XRCE_BASE_SIZE : 8 + sm_len]
end

"""`(request_id, object_id)` of a data frame (7.7.8), or `nothing` under the
same rules as `read_frame`."""
function frame_base(frame::Vector{UInt8})
    read_frame(frame) === nothing && return nothing
    ((Int(frame[9]) << 8) | Int(frame[10]), (Int(frame[11]) << 8) | Int(frame[12]))
end

# --- transport ---

struct Transport
    deliver::Function
    receive::Function
end

# --- sync client ---

mutable struct Client
    transport::Transport
    session::UInt8
    stream::UInt8
    # ObjectId of the target DataWriter (7.7.8 BaseObjectRequest).
    object_id::Int
    seq::Int
end
Client(t::Transport, object_id) = Client(t, SESSION_NOKEY, STREAM_BEST_EFFORT, object_id, 1)

# A sample the envelope cannot carry throws out of `write_frame`, so the
# sequence is not advanced and nothing reaches the transport.
function write!(c::Client, sample::Vector{UInt8})
    frame = write_frame(c.session, c.stream, c.seq, c.object_id, sample)
    c.transport.deliver(frame)
    c.seq = (c.seq + 1) & 0xffff
end

# One non-blocking receive: returns the sample body, or nothing.
function poll(c::Client)
    frame = c.transport.receive()
    frame === nothing ? nothing : read_frame(frame)
end

# --- async reader: a Task filling a Channel (idiomatic Julia) ---

mutable struct AsyncReader
    ch::Channel{Vector{UInt8}}
    running::Ref{Bool}
end

function start_reader(transport::Transport)
    ch = Channel{Vector{UInt8}}(32)
    running = Ref(true)
    @async begin
        while running[]
            frame = transport.receive()
            if frame === nothing
                sleep(0.001)
            else
                body = read_frame(frame)
                body !== nothing && put!(ch, body)
            end
        end
    end
    AsyncReader(ch, running)
end

recv(r::AsyncReader) = take!(r.ch)
stop!(r::AsyncReader) = (r.running[] = false)

# In-memory FIFO transport for tests + example.
function mem_transport()
    q = Vector{Vector{UInt8}}()
    Transport(
        frame -> (push!(q, copy(frame)); nothing),
        () -> isempty(q) ? nothing : popfirst!(q),
    )
end

end # module
