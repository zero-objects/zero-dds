# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Putting bytes on the wire and taking them off again — the Julia
# counterpart of `crates/wire/src/{writer,reader}.rs` and the sibling of
# `runtime/python/zerodds_wire`. A further implementation of
# DDS-XTypes 1.3 §7.4, not a binding to any of the others: where two
# runtimes disagree on an octet, one of them has misread the clause, and
# that is the point of having both.
#
# Alignment is relative to the start of the stream, not to the start of
# the value (§7.4.1.1); a PL_CDR1 parameter's value is its own
# encapsulated block and moves the origin (§7.4.1.2).
#
# Three things Julia makes easy to get wrong, and which are therefore
# decided here rather than assumed:
#
# - `Int` is `Int64` on this host and `Int32` on a 32-bit one, so every
#   width this file states is a fixed-width type;
# - an unsigned zero minus one wraps, so every counted range is built
#   over `Int64`;
# - a `struct` is immutable, so the value a generated decoder assembles
#   is a mutable `ZdScratch` and `zd_freeze` builds the declared type at
#   the end. The scratch and the freeze live here because every
#   generated file needs the same pair, and two copies could disagree
#   about what an untouched slot holds (`nothing`).

module ZeroddsWire

# For the one question `zd_freeze` cannot answer from a value: which
# concrete struct an abstract `ZdFwd_*` stand-in has as its subtype.
import InteractiveUtils

export ZdEndianness, ZD_LITTLE, ZD_BIG
export ZdStatus, ZD_STATUS_OK, ZD_STATUS_TRUNCATED, ZD_STATUS_IMPOSSIBLE_LENGTH,
    ZD_STATUS_OUT_OF_RANGE, ZD_STATUS_DISCARDED
export ZdTryConstruct, ZD_TRY_CONSTRUCT_DISCARD, ZD_TRY_CONSTRUCT_USE_DEFAULT,
    ZD_TRY_CONSTRUCT_TRIM
export ZdParameterRole, ZD_PARAMETER_MEMBER, ZD_PARAMETER_OPTIONAL
export ZdWireError, ZdTruncated, ZdImpossibleLength, ZdNotUtf8, ZdOutOfRange
export ZdPlCdr1Framing, pid_flags_at
export ZdWriter, ZdDHeader, ZdEmHeader, ZdPid, no_dheader
export align_to, write_bool, write_u8, write_i8, write_char, write_wchar,
    write_i16, write_u16, write_i32, write_u32, write_i64, write_u64,
    write_f32, write_f64, write_f128, write_string, write_string_bounded,
    write_wstring_utf8, write_wstring_bounded,
    write_wstring_utf8_octets, write_wstring_utf8_octets_bounded,
    begin_packed_element, finish_packed_element,
    write_fixed, write_enum, write_bitmask, bitfield, write_bitset,
    write_union_overlay, write_dheader, finish_dheader, write_emheader,
    finish_emheader, write_pid, write_pid_extended, finish_pid,
    write_sentinel, check_sequence_bound, check_map_bound, zd_writer_bytes
export ZdReader, ZdReaderMark, ZdPidRead, ZdEmHeaderRead
export reader_status, reader_refuse, reader_clear_discard, reader_position,
    reader_remaining, reader_skip_to, reader_save, reader_restore,
    reader_begin_packed_element, reader_finish_packed_element,
    read_bool, read_u8, read_i8, read_char, read_wchar, read_i16, read_u16,
    read_i32, read_u32, read_i64, read_u64, read_f32, read_f64, read_f128,
    read_string_alloc, read_wstring_utf8, read_wstring_utf8_octets, read_fixed, read_enum,
    read_bitmask, read_bitset, read_dheader, read_emheader, read_pid_as,
    finish_pid_read, skip_member, pid_is_user_member, trim_utf8, trim_utf16
export utf8_octets, utf16_units
export bit_pattern_int, bit_pattern_bool, bit_pattern_f32, bit_pattern_f64
export ZdScratch, zd_scratch_of, zd_scratch_entry, zd_nothing_vector,
    zd_entry_vector, zd_freeze, zd_i64
export zd_jtext, zd_jchar, zd_jfloat, zd_f128_decimal

# -- byte order ----------------------------------------------------------

"""Which end of a multi-byte value goes out first."""
@enum ZdEndianness ZD_LITTLE ZD_BIG

is_little(endianness::ZdEndianness) = endianness === ZD_LITTLE

# -- the statuses a generated decoder reads ------------------------------

# What a reader answers about the *type*, without throwing. A malformed
# stream throws; a refusal about the type sets this and the generated
# decoder walks on to the end of the sample, so one test at the top of
# the caller decides what to do with it.
@enum ZdStatus ZD_STATUS_OK ZD_STATUS_TRUNCATED ZD_STATUS_IMPOSSIBLE_LENGTH ZD_STATUS_OUT_OF_RANGE ZD_STATUS_DISCARDED

"""The status's reported spelling, the same words the sibling runtimes use."""
function status_text(status::ZdStatus)
    status === ZD_STATUS_OK && return "ok"
    status === ZD_STATUS_TRUNCATED && return "truncated"
    status === ZD_STATUS_IMPOSSIBLE_LENGTH && return "impossible-length"
    status === ZD_STATUS_OUT_OF_RANGE && return "out-of-range"
    return "discarded"
end
export status_text

# What a receiving type does with a value it cannot hold.
# DDS-XTypes 1.3 §7.2.2.7, Table 11.
@enum ZdTryConstruct ZD_TRY_CONSTRUCT_DISCARD ZD_TRY_CONSTRUCT_USE_DEFAULT ZD_TRY_CONSTRUCT_TRIM

# Which of §7.4.1.2's two roles a parameter header stands in.
@enum ZdParameterRole ZD_PARAMETER_MEMBER ZD_PARAMETER_OPTIONAL

# -- what can go wrong on the wire ---------------------------------------

"""Base of every wire failure. Each names what was expected and where."""
abstract type ZdWireError <: Exception end

"""The stream ended before the value did."""
struct ZdTruncated <: ZdWireError
    at::Int64
    needed::Int64
    available::Int64
end
Base.showerror(io::IO, e::ZdTruncated) =
    print(io, "at $(e.at): $(e.needed) bytes needed, $(e.available) available")

"""A length field claims more than the stream holds."""
struct ZdImpossibleLength <: ZdWireError
    at::Int64
    claimed::Int64
    available::Int64
end
Base.showerror(io::IO, e::ZdImpossibleLength) =
    print(io, "at $(e.at): a length of $(e.claimed) with $(e.available) bytes left")

"""A string held bytes that are not UTF-8."""
struct ZdNotUtf8 <: ZdWireError
    at::Int64
end
Base.showerror(io::IO, e::ZdNotUtf8) = print(io, "at $(e.at): the string is not UTF-8")

"""A value outside what its type admits — raised by the writer too."""
struct ZdOutOfRange <: ZdWireError
    at::Int64
    expected::String
end
Base.showerror(io::IO, e::ZdOutOfRange) = print(io, "at $(e.at): expected $(e.expected)")

# -- the PL_CDR1 constants -----------------------------------------------

const PID_EXTENDED = UInt32(0x3F01)
const PID_LIST_END = UInt32(0x3F02)
const PID_SHORT_MAX_ID = UInt32(0x3F00)
const PID_IMPL_EXTENSION = UInt32(0x8000)
const PID_IGNORE = UInt32(0x3F03)
const PID_SHORT_MAX_LEN = UInt32(0xFFFF)
const PID_MUST_UNDERSTAND = UInt32(0x4000)
const E_FLAG_MUST_UNDERSTAND = UInt32(0x4000_0000)
const E_FLAG_IMPL_EXTENSION = UInt32(0x8000_0000)
const E_MEMBER_ID_MASK = UInt32(0x0FFF_FFFF)
const PID_ID_MASK = UInt32(0x3FFF)

"""What flags a PL_CDR1 parameter id carries at `position`.

XTypes 1.3 §7.4.1.2 splits the sixteen-bit field, and the measured
implementations do not agree about the sentinel's or the extended
marker's flags — the policy names are `PidFlagPolicy::as_str` in
`crates/ast-layout/src/key.rs`, and an unknown one throws rather than
falling back: a typo in a wire-affecting setting must be loud.
"""
function pid_flags_at(policy::AbstractString, position::AbstractString)
    if policy == "sentinel-must-understand"
        (position == "sentinel" || position == "extended-marker") && return PID_MUST_UNDERSTAND
        return UInt32(0)
    end
    policy == "bare" && return UInt32(0)
    if policy == "impl-extension-members"
        (position == "member" || position == "extended-marker") &&
            return PID_IMPL_EXTENSION | PID_MUST_UNDERSTAND
        return UInt32(0)
    end
    throw(ZdOutOfRange(0, "a PID-flag policy, not `$(policy)`"))
end

"""How a PL_CDR1 parameter list is framed.

The axes the measured implementations differ on inside an XCDR1 sample;
the defaults are the clause's (`Dialect::Specification`). The reader
carries the same value the writer does, because `finish_pid_read`
validates the declared length against the framing's convention and pads
after the value only where the writer did.
"""
struct ZdPlCdr1Framing
    pid_flags::String
    highest_short_id::UInt32
    pad_declared_length::Bool
    pad_declared_optional_length::Bool
    member_must_understand::Bool
    pad_after_value::Bool
end

function ZdPlCdr1Framing(;
    pid_flags::AbstractString = "sentinel-must-understand",
    highest_short_id::Integer = PID_SHORT_MAX_ID,
    pad_declared_length::Bool = false,
    pad_declared_optional_length::Bool = false,
    member_must_understand::Bool = true,
    pad_after_value::Bool = true,
)
    return ZdPlCdr1Framing(
        String(pid_flags),
        UInt32(highest_short_id),
        pad_declared_length,
        pad_declared_optional_length,
        member_must_understand,
        pad_after_value,
    )
end

"""Whether a parameter in `role` declares a padded length."""
function pads_declared_length(framing::ZdPlCdr1Framing, role::ZdParameterRole)
    role === ZD_PARAMETER_OPTIONAL && return framing.pad_declared_optional_length
    return framing.pad_declared_length
end

# -- the wstring cells ---------------------------------------------------

# C1 x C2 x C3: whether the count is characters, whether a terminator is
# written, and the unit width. One table so a cell cannot answer one of
# the three and be forgotten by another; the coordinate spellings reach
# every cell exactly as `WstringLengthPolicy::parse` accepts both.
const ZD_WSTRING_CELLS = let
    cells = Dict{String,Tuple{Bool,Bool,Int}}(
        "specification" => (false, false, 2),
        "character-count" => (true, false, 2),
        "character-count-terminated" => (true, true, 4),
        "specification-wide-units" => (false, false, 4),
        "ros-type-support" => (true, false, 4),
        "octets/terminated/2" => (false, true, 2),
        "octets/terminated/4" => (false, true, 4),
        "characters/terminated/2" => (true, true, 2),
    )
    for (counts, terminated, width) in collect(values(cells))
        key = string(
            counts ? "characters" : "octets",
            "/",
            terminated ? "terminated" : "unterminated",
            "/",
            width,
        )
        cells[key] = (counts, terminated, width)
    end
    cells
end

const ZD_WSTRING_UNIT_ORDERS =
    ("stream", "big-endian-always", "byte-order-mark", "host-unswapped")

const ZD_BYTE_ORDER_MARK = UInt16(0xFEFF)
const ZD_SWAPPED_BYTE_ORDER_MARK = UInt16(0xFFFE)

# -- the handles ---------------------------------------------------------

"""A DHEADER waiting for its length; `at` below zero is the delimiter a
`@final` aggregate does not write."""
struct ZdDHeader
    at::Int64
end

"""The DHEADER a value that takes no delimiter does not write.

§7.4.3.5.3 gives the delimiter to `@appendable` and `@mutable` and not
to `@final`, and the shared walk declares one handle per value and
finishes it either way — so the absence is a value of this type.
"""
no_dheader() = ZdDHeader(Int64(-1))

"""An EMHEADER waiting for its NEXTINT (`next_int` below zero: none)."""
struct ZdEmHeader
    next_int::Int64
    word::Int64
    """The code the header was written with, so that `finish_emheader`
    can hold the member's value to the length the code declares. Codes 0
    to 3 are lengths; anything else declares nothing of its own."""
    length_code::Int64
end

"""A parameter header waiting for its length."""
struct ZdPid
    at::Int64
    extended::Bool
    saved_origin::Int64
    role::ZdParameterRole
    pid::UInt32
    must_understand::Bool
end

"""Everything a two-pass read has to put back."""
struct ZdReaderMark
    at::Int64
    origin::Int64
    status::ZdStatus
end

"""An EMHEADER as read. `next_int` below zero: none.

`extent` is how many octets the member occupies after the header word,
the NEXTINT included where there is one — the same arithmetic
`skip_member` does, carried as a field so a reader that has already
read the member can ask where it should have ended. §7.4.3.4.2.
"""
struct ZdEmHeaderRead
    member_id::UInt32
    length_code::Int64
    must_understand::Bool
    next_int::Int64
    extent::Int64
end

function emheader_extent(length_code::Int64, next_int::Int64)
    length_code <= 3 && return Int64(1) << length_code
    next_int < 0 && return Int64(0)
    length_code == 4 && return next_int
    scale = length_code == 5 ? Int64(1) : length_code == 6 ? Int64(4) : Int64(8)
    return Int64(4) + next_int * scale
end

"""A parameter header as read.

`end_` with the trailing underscore, because `end` is a Julia keyword
and cannot name a field; the codec walk's `header_field` seam maps the
name.
"""
struct ZdPidRead
    pid::UInt32
    length::UInt32
    end_::Bool
    must_understand::Bool
    impl_extension::Bool
    reserved::Bool
    ignore::Bool
    role::ZdParameterRole
    saved_origin::Int64
    value_at::Int64
end

"""Whether this parameter can be a member of the reading type.

False for a vendor-private parameter and for a reserved id; both are
legal for a peer to send and neither is a member. §7.4.1.2.2,
§7.2.2.4.4.4.6.
"""
pid_is_user_member(header::ZdPidRead) =
    !header.impl_extension && !header.reserved && !header.ignore

# -- the writer ----------------------------------------------------------

"""Accumulates the bytes of one sample."""
mutable struct ZdWriter
    out::Vector{UInt8}
    endianness::ZdEndianness
    origin::Int64
    max_align::Int64
    pl_cdr1::ZdPlCdr1Framing
    # Whether an LC4 header whose member turns out to be 1, 2, 4 or 8
    # bytes is rewritten with a short length code (Fast DDS).
    short_length_codes::Bool
end

function ZdWriter(
    endianness::ZdEndianness,
    max_align::Integer = 8,
    pl_cdr1::ZdPlCdr1Framing = ZdPlCdr1Framing(),
    short_length_codes::Bool = false,
)
    # Four for XCDR2, eight for XCDR1: §7.4.1's ALIGN(N) aligns to
    # MIN(N, XCDR.maxalign).
    return ZdWriter(UInt8[], endianness, Int64(0), max(Int64(1), Int64(max_align)), pl_cdr1, short_length_codes)
end

"""The bytes written so far, as their own vector."""
zd_writer_bytes(w::ZdWriter) = copy(w.out)

wposition(w::ZdWriter) = Int64(length(w.out))

"""Pad to `align`, counted from the stream's origin."""
function align_to(w::ZdWriter, align::Integer)
    a = min(Int64(align), w.max_align)
    a <= 1 && return nothing
    offset = wposition(w) - w.origin
    padding = mod(a - mod(offset, a), a)
    for _ in 1:padding
        push!(w.out, 0x00)
    end
    return nothing
end

"""Refuse an integer the field cannot hold.

The check the Rust runtime does not need and a dynamic call chain does:
a value that arrived wider than its holder is a value already lost, and
masking it would put a different number on the wire and report success.
"""
function zd_check(w::ZdWriter, value::Integer, low::Integer, high::Integer, expected::String)
    if value < low || value > high
        throw(ZdOutOfRange(wposition(w), "$(expected) in [$(low), $(high)], not $(value)"))
    end
    return nothing
end

function write_raw_uint(w::ZdWriter, unsigned::UInt64, width::Integer)
    if is_little(w.endianness)
        for i in 0:(width - 1)
            push!(w.out, UInt8((unsigned >> (8 * i)) & 0xFF))
        end
    else
        for i in (width - 1):-1:0
            push!(w.out, UInt8((unsigned >> (8 * i)) & 0xFF))
        end
    end
    return nothing
end

function write_int_aligned(w::ZdWriter, unsigned::UInt64, width::Integer)
    align_to(w, width)
    write_raw_uint(w, unsigned, width)
    return nothing
end

"""`boolean`, one byte, 0 or 1."""
write_bool(w::ZdWriter, value::Bool) = (push!(w.out, value ? 0x01 : 0x00); nothing)

"""`octet` and `uint8`."""
function write_u8(w::ZdWriter, value::Integer)
    zd_check(w, value, 0, 0xFF, "an octet")
    push!(w.out, UInt8(value))
    return nothing
end

"""`int8`."""
function write_i8(w::ZdWriter, value::Integer)
    zd_check(w, value, -128, 127, "an int8")
    push!(w.out, UInt8(mod(Int64(value), 256)))
    return nothing
end

"""`char`, one byte. The code unit arrives as the integer the generated
struct holds; a value above 255 is refused rather than truncated."""
function write_char(w::ZdWriter, value::Integer)
    if value < 0 || value > 0xFF
        throw(ZdOutOfRange(wposition(w), "a character below U+0100, not U+$(string(Int64(value), base=16, pad=4))"))
    end
    push!(w.out, UInt8(value))
    return nothing
end

"""A 16-bit value little-endian whatever the stream's order is."""
function write_u16_little(w::ZdWriter, value::Integer)
    zd_check(w, value, 0, 0xFFFF, "an unsigned 16-bit value")
    align_to(w, 2)
    push!(w.out, UInt8(value & 0xFF))
    push!(w.out, UInt8((value >> 8) & 0xFF))
    return nothing
end

"""A 16-bit value big-endian whatever the stream's order is."""
function write_u16_big(w::ZdWriter, value::Integer)
    zd_check(w, value, 0, 0xFFFF, "an unsigned 16-bit value")
    align_to(w, 2)
    push!(w.out, UInt8((value >> 8) & 0xFF))
    push!(w.out, UInt8(value & 0xFF))
    return nothing
end

"""A 32-bit value little-endian whatever the stream's order is."""
function write_u32_little(w::ZdWriter, value::Integer)
    zd_check(w, value, 0, 0xFFFFFFFF, "an unsigned 32-bit value")
    align_to(w, 4)
    for i in 0:3
        push!(w.out, UInt8((value >> (8 * i)) & 0xFF))
    end
    return nothing
end

"""A 32-bit value big-endian whatever the stream's order is."""
function write_u32_big(w::ZdWriter, value::Integer)
    zd_check(w, value, 0, 0xFFFFFFFF, "an unsigned 32-bit value")
    align_to(w, 4)
    for i in 3:-1:0
        push!(w.out, UInt8((value >> (8 * i)) & 0xFF))
    end
    return nothing
end

"""`wchar`, at `width` octets and in the shape `framing` does.

Two octets under Table 31; four under RTI's XCDR1, TAO's and
`rosidl_typesupport_fastrtps_c`'s. `giop-length-prefixed` is CORBA 3.3
part 2 §9.3.1.6's GIOP 1.2 form: the octet count, then the code unit's
octets — big-endian under every order but the stream's own.
"""
function write_wchar(
    w::ZdWriter,
    value::Integer,
    width::Integer = 2,
    framing::AbstractString = "fixed",
    order::AbstractString = "stream",
)
    if value < 0 || value > 0xFFFF
        throw(ZdOutOfRange(wposition(w), "a BMP code unit, not $(value)"))
    end
    if width != 2 && width != 4
        throw(ZdOutOfRange(wposition(w), "a wchar width of 2 or 4, not $(width)"))
    end
    if framing == "giop-length-prefixed"
        push!(w.out, UInt8(width))
        big = order != "stream" || !is_little(w.endianness)
        if big
            for i in (width - 1):-1:0
                push!(w.out, UInt8((value >> (8 * i)) & 0xFF))
            end
        else
            for i in 0:(width - 1)
                push!(w.out, UInt8((value >> (8 * i)) & 0xFF))
            end
        end
        return nothing
    end
    framing == "fixed" || throw(ZdOutOfRange(wposition(w), "a wchar framing, not `$(framing)`"))
    if width == 4
        write_u32(w, value)
    else
        write_u16(w, value)
    end
    return nothing
end

"""`short`."""
function write_i16(w::ZdWriter, value::Integer)
    zd_check(w, value, -32768, 32767, "a short")
    write_int_aligned(w, UInt64(mod(Int64(value), 65536)), 2)
    return nothing
end

"""`unsigned short`."""
function write_u16(w::ZdWriter, value::Integer)
    zd_check(w, value, 0, 0xFFFF, "an unsigned short")
    write_int_aligned(w, UInt64(value), 2)
    return nothing
end

"""`long`."""
function write_i32(w::ZdWriter, value::Integer)
    zd_check(w, value, -2147483648, 2147483647, "a long")
    write_int_aligned(w, UInt64(mod(Int64(value), Int64(1) << 32)), 4)
    return nothing
end

"""`unsigned long`."""
function write_u32(w::ZdWriter, value::Integer)
    zd_check(w, value, 0, 0xFFFF_FFFF, "an unsigned long")
    write_int_aligned(w, UInt64(value), 4)
    return nothing
end

"""`long long`."""
function write_i64(w::ZdWriter, value::Integer)
    zd_check(w, value, -(Int128(1) << 63), (Int128(1) << 63) - 1, "a long long")
    write_int_aligned(w, reinterpret(UInt64, Int64(value)), 8)
    return nothing
end

"""`unsigned long long`."""
function write_u64(w::ZdWriter, value::Integer)
    if value < 0 || Int128(value) > Int128(typemax(UInt64))
        throw(ZdOutOfRange(wposition(w), "an unsigned long long, not $(value)"))
    end
    write_int_aligned(w, UInt64(value), 8)
    return nothing
end

"""`float`, rounded to single precision once, here.

An overflowing magnitude becomes the infinity — what a C `(float)` cast
produces and what every peer reads back — rather than an error the
caller cannot act on.
"""
function write_f32(w::ZdWriter, value::Real)
    write_int_aligned(w, UInt64(reinterpret(UInt32, Float32(value))), 4)
    return nothing
end

"""`double`."""
function write_f64(w::ZdWriter, value::Real)
    write_int_aligned(w, reinterpret(UInt64, Float64(value)), 8)
    return nothing
end

# -- strings -------------------------------------------------------------

"""A string: a four-byte length including the terminator, the UTF-8
bytes, then a NUL. XTypes 1.3 §7.4.1.1. The length counts *bytes*."""
function write_string(w::ZdWriter, value::AbstractString)
    encoded = codeunits(String(value))
    write_u32(w, length(encoded) + 1)
    append!(w.out, encoded)
    push!(w.out, 0x00)
    return nothing
end

"""A bounded `string`: the same bytes [`write_string`](@ref) writes. The
bound is a promise about the source and not a difference on the wire,
so this is the only place it can be enforced — refused rather than cut
at a character boundary, which would put a sample on the wire nobody
wrote."""
function write_string_bounded(w::ZdWriter, value::AbstractString, bound::Integer)
    octets = length(codeunits(String(value)))
    if octets > bound
        throw(ZdOutOfRange(
            wposition(w),
            "at most $(bound) octets in a bounded string, and this one has $(octets)",
        ))
    end
    write_string(w, value)
    return nothing
end

"""The count check every bounded `sequence` makes before a byte of it is
written. Table 17 makes `O2.length <= T1.length` the condition for
constructing a `sequence<T, N>`, and the wire carries no bound to
compare against: a sender whose count already exceeds its own
declaration has written a value of a type nobody declared."""
function check_sequence_bound(w::ZdWriter, actual::Integer, bound::Integer)
    if actual > bound
        throw(ZdOutOfRange(
            wposition(w),
            "at most $(bound) elements in a bounded sequence, and this one has $(actual)",
        ))
    end
    return nothing
end

"""The count check every bounded `map` makes before a byte of it is
written. Its own message and not [`check_sequence_bound`](@ref)'s: the
unit is an entry and not an element, per XTypes 1.3 §7.4.1.1.4's
map-as-a-sequence-of-pairs equivalence."""
function check_map_bound(w::ZdWriter, actual::Integer, bound::Integer)
    if actual > bound
        throw(ZdOutOfRange(
            wposition(w),
            "at most $(bound) entries in a bounded map, and this one has $(actual)",
        ))
    end
    return nothing
end

"""The UTF-16 code units of `text`, without a BOM.

By hand rather than through a transcoder, because the rule this
implements is about code *units*: §7.4.1.1.2 counts a supplementary
character as two. A Julia `String` cannot hold a lone surrogate, so the
refusal the Python runtime writes for one has no case here.
"""
function utf16_units_of(text::AbstractString)
    units = UInt16[]
    for character in text
        point = UInt32(character)
        if point < 0x10000
            push!(units, UInt16(point))
        else
            offset = point - 0x10000
            push!(units, UInt16(0xD800 + (offset >> 10)))
            push!(units, UInt16(0xDC00 + (offset & 0x3FF)))
        end
    end
    return units
end

function write_wstring_unit(w::ZdWriter, unit::Integer, width::Int, order::AbstractString)
    little = order == "byte-order-mark" || order == "host-unswapped"
    if width == 4
        if order == "big-endian-always"
            write_u32_big(w, unit)
        elseif little
            write_u32_little(w, unit)
        else
            write_u32(w, unit)
        end
    elseif order == "big-endian-always"
        write_u16_big(w, unit)
    elseif little
        write_u16_little(w, unit)
    else
        write_u16(w, unit)
    end
    return nothing
end

"""A `wstring`, in the form `policy` and `order` name.

`specification` is a four-byte byte count and then UTF-16 code units,
no BOM and no terminator (§7.4.1.1.2). The three questions — which
units go out, what the length counts them in, in what byte order each
lands — are three axes; the spellings are `WstringLengthPolicy::as_str`
and `WstringUnitOrder::as_str` in `crates/ast-layout/src/key.rs`.
"""
function write_wstring_utf8(
    w::ZdWriter,
    value::AbstractString,
    policy::AbstractString = "specification",
    order::AbstractString = "stream",
)
    cell = get(ZD_WSTRING_CELLS, policy, nothing)
    cell === nothing && throw(ZdOutOfRange(wposition(w), "a wstring length policy, not `$(policy)`"))
    order in ZD_WSTRING_UNIT_ORDERS ||
        throw(ZdOutOfRange(wposition(w), "a wstring unit order, not `$(order)`"))
    counts_characters, terminated, width = cell
    if counts_characters && width == 2
        # A character count over two-octet units has no way to carry a
        # code point outside the BMP: the pair would be counted as two
        # and written as its low bits. A vendor's length convention is a
        # policy; its data loss is not.
        for (index, character) in enumerate(value)
            if UInt32(character) > 0xFFFF
                throw(ZdOutOfRange(
                    Int64(index - 1),
                    "a code point inside the BMP: a character count over two-octet units has no way to carry one outside it",
                ))
            end
        end
    end
    units = utf16_units_of(value)
    if terminated
        push!(units, UInt16(0))
    end
    # An empty wstring carries no mark: a mark on one would be a
    # two-octet string that decodes to nothing, and no peer writes it.
    if order == "byte-order-mark" && !isempty(units)
        pushfirst!(units, ZD_BYTE_ORDER_MARK)
    end
    write_u32(w, counts_characters ? length(units) : length(units) * width)
    for unit in units
        write_wstring_unit(w, unit, width, order)
    end
    return nothing
end

"""A bounded `wstring`, as [`write_string_bounded`](@ref) is to
[`write_string`](@ref). The bound counts UTF-16 code units, not
characters and not octets, whatever `policy` puts on the wire."""
function write_wstring_bounded(
    w::ZdWriter,
    value::AbstractString,
    bound::Integer,
    policy::AbstractString = "specification",
    order::AbstractString = "stream",
)
    units = length(utf16_units_of(value))
    if units > bound
        throw(ZdOutOfRange(
            wposition(w),
            "at most $(bound) UTF-16 code units in a bounded wstring, and this one has " *
            "$(units)",
        ))
    end
    write_wstring_utf8(w, value, policy, order)
    return nothing
end

"""A `wstring` whose characters go out as their own UTF-8 octets.

`WstringCodeUnits::Utf8` is the measured departure of one product's
generated serialiser: XTypes 1.3 Table 31 gives a `wstring`'s code unit
as `Char16` and §7.4.1.1.2 makes the length "twice the number of
characters", so the clause has no UTF-8 form at all. The declared length
is the UTF-8 octet count plus the terminator the policy asks for.
`wstring-member` is `04 00 00 00 61 62 63 00` for `"abc"`: three octets
and a NUL, and the four declares both.
"""
function write_wstring_utf8_octets(
    w::ZdWriter,
    value::AbstractString,
    policy::AbstractString = "specification",
)
    cell = get(ZD_WSTRING_CELLS, policy, nothing)
    cell === nothing && throw(ZdOutOfRange(wposition(w), "a wstring length policy, not `$(policy)`"))
    # The same malformed-UTF-8 refusal the reader makes: a Julia string
    # is a byte array and can hold a sequence that is not UTF-8.
    isvalid(value) || throw(ZdNotUtf8(wposition(w)))
    _, terminated, _ = cell
    octets = codeunits(value)
    length(octets) > typemax(UInt32) &&
        throw(ZdOutOfRange(wposition(w), "a wstring length the UInt32 field can hold"))
    length_ = length(octets) + (terminated ? 1 : 0)
    write_u32(w, length_)
    append!(w.out, octets)
    terminated && push!(w.out, 0x00)
    return nothing
end

"""A bounded UTF-8-octet `wstring`, as [`write_wstring_bounded`](@ref)
is to [`write_wstring_utf8`](@ref): the bound still counts UTF-16 code
units, which is what a `wstring<N>` bound means to every peer, while the
wire carries the UTF-8 octets."""
function write_wstring_utf8_octets_bounded(
    w::ZdWriter,
    value::AbstractString,
    bound::Integer,
    policy::AbstractString = "specification",
)
    units = length(utf16_units_of(value))
    if units > bound
        throw(ZdOutOfRange(
            wposition(w),
            "at most $(bound) UTF-16 code units in a bounded wstring, and this one has " *
            "$(units)",
        ))
    end
    write_wstring_utf8_octets(w, value, policy)
    return nothing
end

"""Move the alignment origin to the current position and answer the
origin displaced, for [`finish_packed_element`](@ref).

`ElementPadding::Packed` is HDDS's measured departure from §7.4.1, which
counts every element's alignment from the encapsulation: each collection
element and each aggregate value counts its own alignment from its own
first octet instead. `array-1d-struct-final` is
`01000000 0200 03000000 0400 77`, the second `Cell`'s `long` at offset
six where the clause puts it at eight."""
function begin_packed_element(w::ZdWriter)
    saved = w.origin
    w.origin = wposition(w)
    return saved
end

"""Restore the origin [`begin_packed_element`](@ref) displaced."""
function finish_packed_element(w::ZdWriter, saved::Integer)
    w.origin = saved
    return nothing
end

# -- fixed ---------------------------------------------------------------

"""The BCD nibbles of `text` under `fixed<digits, scale>`.

[CDR] §9.3.2.8: digit nibbles most-significant first, then a sign
nibble — 0xC positive, 0xD negative — with a leading zero nibble where
the count is odd. `count` is `declared` (the type's width, omniORB's
and this project's), `value-precision` (as many digits as the value
has, never fewer than one — JacORB 3.9) or `value-string-digits` (the
integer digits of `text` as written plus S, made odd by one leading zero — ACE/TAO
2.5.24's `ACE_CDR::Fixed::from_string` (OpenDDS writes the declared count): `0.12345` under
`fixed<5,5>` is `0012345c`; past 31 digits it is refused).
"""
function fixed_nibbles(text::AbstractString, digits::Integer, scale::Integer, count::AbstractString)
    digits == 0 && throw(ZdOutOfRange(0, "a fixed with at least one digit"))
    # OMG IDL 4.2 rule (42) caps the BCD rule at 31 digits, and
    # fixed<31, S> is the widest declaration IDL admits. A wider one has
    # no packing defined for it, so it is refused rather than packed into
    # octets no reader can be told the width of.
    digits > 31 && throw(ZdOutOfRange(0, "a fixed of at most 31 digits, not $(digits)"))
    scale > digits && throw(ZdOutOfRange(0, "a scale no larger than the digit count"))
    negative = false
    magnitude = text
    if startswith(magnitude, "-")
        negative = true
        magnitude = magnitude[2:end]
    elseif startswith(magnitude, "+")
        magnitude = magnitude[2:end]
    end
    parts = split(magnitude, "."; limit = 2)
    whole = parts[1]
    fraction = length(parts) == 2 ? parts[2] : ""
    if isempty(whole) && isempty(fraction)
        throw(ZdOutOfRange(0, "a decimal that fits fixed<$(digits), $(scale)>, not `$(text)`"))
    end
    for character in string(whole, fraction)
        if !(character in '0':'9')
            throw(ZdOutOfRange(0, "a decimal that fits fixed<$(digits), $(scale)>, not `$(text)`"))
        end
    end
    as_written = length(whole) + Int(scale)
    integral_width = Int(digits) - Int(scale)
    whole = lstrip(whole, '0')
    length(whole) > integral_width &&
        throw(ZdOutOfRange(0, "a decimal whose integer part fits $(integral_width) digits, not `$(text)`"))
    length(fraction) > scale &&
        throw(ZdOutOfRange(0, "a decimal whose fraction fits $(scale) digits, not `$(text)`"))
    count == "declared" || count == "value-precision" || count == "value-string-digits" ||
        throw(ZdOutOfRange(0, "a fixed digit count, not `$(count)`"))

    every = Int[]
    for _ in 1:(integral_width - length(whole))
        push!(every, 0)
    end
    for character in whole
        push!(every, Int(character) - Int('0'))
    end
    for character in fraction
        push!(every, Int(character) - Int('0'))
    end
    for _ in 1:(Int(scale) - length(fraction))
        push!(every, 0)
    end
    first = 1
    while first <= length(every) && every[first] == 0
        first += 1
    end
    significant = length(every) - first + 1
    wire = count == "value-precision" ? max(significant, 1) : length(every)
    if count == "value-string-digits"
        # ACE/TAO 2.5.24: the integer digits as written plus S, made odd by one leading
        # zero. Past 31 no reader can be told the field's width.
        wire = as_written + (mod(as_written, 2) == 0 ? 1 : 0)
        wire > 31 && throw(ZdOutOfRange(0, "a fixed of at most 31 digits on the wire, not $(wire) for `$(text)`"))
    end

    nibbles = Int[]
    if mod(wire + 1, 2) == 1
        push!(nibbles, 0)
    end
    # A count wider than P is filled with leading zeros.
    kept = min(wire, length(every))
    for _ in 1:(wire - kept)
        push!(nibbles, 0)
    end
    append!(nibbles, every[(length(every) - kept + 1):end])
    zero = all(n -> n == 0, every)
    push!(nibbles, negative && !zero ? 0x0D : 0x0C)
    return nibbles
end

"""`fixed<digits, scale>` as packed BCD. [CDR] §9.3.2.8 — the only wire
definition an IDL `fixed` has; DDS-XTypes carries the type not at all."""
function write_fixed(
    w::ZdWriter,
    value::AbstractString,
    digits::Integer,
    scale::Integer,
    count::AbstractString = "declared",
)
    nibbles = fixed_nibbles(value, digits, scale, count)
    for index in 1:2:length(nibbles)
        push!(w.out, UInt8((nibbles[index] << 4) | nibbles[index + 1]))
    end
    return nothing
end

# -- the bit-bounded kinds -----------------------------------------------

"""An enumerator, as the integer its bit bound calls for.

§7.4.3.5.3 rule (5): the holder is Int8, Int16 or Int32 by the
`@bit_bound` — **signed**, so `@value(-1)` at the default bound goes
out as `ffffffff`. What fits is `-2^(n-1) ..= 2^n - 1`; §7.3.1.2.1.5
caps the bound at 32.
"""
function write_enum(w::ZdWriter, bit_bound::Integer, value::Integer)
    (bit_bound == 0 || bit_bound > 32) &&
        throw(ZdOutOfRange(wposition(w), "an @bit_bound between 1 and 32, not $(bit_bound)"))
    lo = -(Int64(1) << (bit_bound - 1))
    hi = (Int64(1) << bit_bound) - 1
    (value < lo || value > hi) &&
        throw(ZdOutOfRange(wposition(w), "an enumerator a $(bit_bound)-bit holder can hold, not $(value)"))
    pattern = mod(Int64(value), Int64(1) << bit_bound)
    if bit_bound <= 8
        write_u8(w, pattern)
    elseif bit_bound <= 16
        write_u16(w, pattern)
    else
        write_u32(w, pattern)
    end
    return nothing
end

"""A bitmask, as the unsigned integer Table 33 gives for its bound."""
function write_bitmask(w::ZdWriter, bit_bound::Integer, value::Integer)
    (bit_bound == 0 || bit_bound > 64) &&
        throw(ZdOutOfRange(wposition(w), "an @bit_bound between 1 and 64, not $(bit_bound)"))
    wide = UInt64(value)
    if bit_bound < 64 && wide >= (UInt64(1) << bit_bound)
        throw(ZdOutOfRange(wposition(w), "a bitmask below 2^$(bit_bound), not $(value)"))
    end
    if bit_bound <= 8
        write_u8(w, wide)
    elseif bit_bound <= 16
        write_u16(w, wide)
    elseif bit_bound <= 32
        write_u32(w, wide)
    else
        write_u64(w, wide)
    end
    return nothing
end

"""One bitfield, shifted into the accumulator a bitset packs into.

A value wider than the field is refused rather than masked: masking
would put a different number on the wire than the caller asked for.
"""
function bitfield(w::ZdWriter, value::Integer, width::Integer, position::Integer)
    if width <= 0 || width > 64 || position < 0 || position + width > 64
        throw(ZdOutOfRange(wposition(w), "a bitfield of $(width) bits at $(position), inside 64"))
    end
    limit = width == 64 ? typemax(UInt64) : (UInt64(1) << width) - 1
    if value < 0 || UInt64(value) > limit
        throw(ZdOutOfRange(wposition(w), "a value inside $(width) bits, not $(value)"))
    end
    return UInt64(value) << position
end

"""A bitset, as the anchor integer its bitfields pack into. The anchor's
width comes from the layout planner."""
function write_bitset(w::ZdWriter, anchor::Integer, value::Integer)
    if anchor == 1
        write_u8(w, value)
    elseif anchor == 2
        write_u16(w, value)
    elseif anchor == 4
        write_u32(w, value)
    elseif anchor == 8
        write_u64(w, UInt64(value))
    else
        throw(ZdOutOfRange(wposition(w), "a bitset anchor of 1, 2, 4 or 8 bytes, not $(anchor)"))
    end
    return nothing
end

"""The Cyclone union overlay: the low `octets` of `pattern`, as one
unsigned integer in the stream's byte order."""
function write_union_overlay(w::ZdWriter, pattern::Integer, octets::Integer)
    wide = pattern isa UInt64 ? pattern : reinterpret(UInt64, Int64(pattern))
    if octets == 1
        write_u8(w, wide & 0xFF)
    elseif octets == 2
        write_u16(w, wide & 0xFFFF)
    elseif octets == 4
        write_u32(w, wide & 0xFFFF_FFFF)
    elseif octets == 8
        write_u64(w, wide)
    else
        throw(ZdOutOfRange(wposition(w), "a union arm of 1, 2, 4 or 8 octets, not $(octets)"))
    end
    return nothing
end

# -- framing -------------------------------------------------------------

function zd_patch(w::ZdWriter, at::Int64, value::Integer, width::Integer)
    if value < 0 || Int128(value) >= (Int128(1) << (8 * width))
        throw(ZdOutOfRange(at, "a $(width)-byte length, not $(value)"))
    end
    unsigned = UInt64(value)
    if is_little(w.endianness)
        for i in 0:(width - 1)
            w.out[at + 1 + i] = UInt8((unsigned >> (8 * i)) & 0xFF)
        end
    else
        for i in 0:(width - 1)
            w.out[at + 1 + i] = UInt8((unsigned >> (8 * (width - 1 - i))) & 0xFF)
        end
    end
    return nothing
end

"""A DHEADER: a four-byte length the members that follow occupy, written
as a placeholder and patched when they are done."""
function write_dheader(w::ZdWriter)
    align_to(w, 4)
    at = wposition(w)
    append!(w.out, (0x00, 0x00, 0x00, 0x00))
    return ZdDHeader(at)
end

"""Patch a DHEADER; a handle from `no_dheader` is not there and is
ignored."""
function finish_dheader(w::ZdWriter, header::ZdDHeader)
    header.at < 0 && return nothing
    zd_patch(w, header.at, wposition(w) - header.at - 4, 4)
    return nothing
end

"""An XCDR2 EMHEADER. §7.4.3.4.2: LC 0-3 carry the length in the code,
LC 4 is followed by a NEXTINT, LC 5-7 reuse the member's own leading
word as the NEXTINT — so no placeholder is reserved for them."""
function write_emheader(w::ZdWriter, member_id::Integer, length_code::Integer, must_understand::Bool)
    # §7.2.2.4.4.4.4 runs member ids from 0 to 0x0FFFFFFF and §7.4.3.4.2
    # leaves them the low 28 bits of the word. Masking a larger one writes
    # a well-formed header for `id & 0x0FFFFFFF`: the peer reads a member
    # the caller did not name, and nothing at either end says so.
    if member_id < 0 || member_id > E_MEMBER_ID_MASK
        throw(ZdOutOfRange(wposition(w),
            "a member id inside the 28 bits of an EMHEADER, not $(member_id)"))
    end
    align_to(w, 4)
    word =
        (UInt32(must_understand ? 1 : 0) << 31) |
        ((UInt32(length_code) & 0x7) << 28) |
        (UInt32(member_id) & E_MEMBER_ID_MASK)
    write_u32(w, word)
    word_at = wposition(w) - 4
    if length_code == 4
        at = wposition(w)
        append!(w.out, (0x00, 0x00, 0x00, 0x00))
        return ZdEmHeader(at, word_at, Int64(length_code))
    end
    return ZdEmHeader(Int64(-1), word_at, Int64(length_code))
end

function read_patched_u32(w::ZdWriter, at::Int64)
    value = UInt32(0)
    if is_little(w.endianness)
        for i in 0:3
            value |= UInt32(w.out[at + 1 + i]) << (8 * i)
        end
    else
        for i in 0:3
            value = (value << 8) | UInt32(w.out[at + 1 + i])
        end
    end
    return value
end

"""Remove an LC4 header's NEXTINT and raise its length code — safe only
under XCDR2, where MAXALIGN is four and the value keeps its offsets."""
function shorten_emheader(w::ZdWriter, header::ZdEmHeader, len::Int64)
    code = len == 1 ? 0 : len == 2 ? 1 : len == 4 ? 2 : 3
    deleteat!(w.out, (header.next_int + 1):(header.next_int + 4))
    previous = read_patched_u32(w, header.word)
    zd_patch(w, header.word, (previous & ~(UInt32(0x7) << 28)) | (UInt32(code) << 28), 4)
    return nothing
end

"""Patch an EMHEADER's NEXTINT with the length of what followed."""
function finish_emheader(w::ZdWriter, header::ZdEmHeader)
    if header.next_int < 0
        # Under codes 0 to 3 the code *is* the length — 1, 2, 4 or 8
        # octets, §7.4.3.4.2. A value of another size misdeclares it and
        # the reader's skip lands inside data, so the mismatch is refused
        # here rather than written. Codes 5 to 7 declare nothing of their
        # own: the member's leading length word is the NEXTINT.
        if 0 <= header.length_code <= 3 && header.word + 4 <= wposition(w)
            declared = 1 << header.length_code
            body = wposition(w) - header.word - 4
            body == declared || throw(ZdOutOfRange(wposition(w),
                "a member of $(declared) octets under length code " *
                "$(header.length_code), not $(body)"))
        end
        return nothing
    end
    len = wposition(w) - header.next_int - 4
    if w.short_length_codes && (len == 1 || len == 2 || len == 4 || len == 8)
        shorten_emheader(w, header, len)
        return nothing
    end
    zd_patch(w, header.next_int, len, 4)
    return nothing
end

"""Start a parameter's value block, returning the origin it replaces.
§7.4.1.2: each parameter is its own CDR-encapsulated block."""
function begin_parameter_value(w::ZdWriter)
    saved = w.origin
    w.origin = wposition(w)
    return saved
end

"""A PL_CDR1 parameter header, short form: `[id:u16][length:u16]`."""
function write_pid(w::ZdWriter, pid::Integer, role::ZdParameterRole, must_understand::Bool)
    # §7.4.1.2.2 gives the short form the ids 0 to `highest_short_id`
    # inclusive and reserves what is above it — PID_EXTENDED is 0x3F01
    # itself. A short header with a reserved id collides with the
    # protocol's own parameters, so such an id takes the extended form or
    # none. This is the write side of the constant the reader has always
    # consulted.
    if pid < 0 || pid > w.pl_cdr1.highest_short_id
        throw(ZdOutOfRange(wposition(w),
            "a parameter id the short header carries, not $(pid)" *
            " — the extended header takes it"))
    end
    align_to(w, 4)
    word = UInt32(pid)
    if must_understand && w.pl_cdr1.member_must_understand
        word |= PID_MUST_UNDERSTAND
    end
    word |= pid_flags_at(w.pl_cdr1.pid_flags, "member")
    bare = word & ~(PID_MUST_UNDERSTAND | PID_IMPL_EXTENSION)
    write_u16(w, word)
    at = wposition(w)
    append!(w.out, (0x00, 0x00))
    return ZdPid(at, false, begin_parameter_value(w), role, bare, must_understand)
end

"""A PL_CDR1 parameter header, extended form:
`[PID_EXTENDED][8][id:u32][length:u32]`. The member's must-understand is
FLAG_2 of the `eMemberHeader`, not the marker's bit."""
function write_pid_extended(w::ZdWriter, member_id::Integer, role::ZdParameterRole, must_understand::Bool)
    # The same 28 bits as an EMHEADER's, here in the `eMemberHeader` of
    # §7.4.1.2.1. Masking aliases the parameter onto another member
    # exactly as it does there.
    if member_id < 0 || member_id > E_MEMBER_ID_MASK
        throw(ZdOutOfRange(wposition(w),
            "a member id inside the 28 bits of an eMemberHeader, not $(member_id)"))
    end
    align_to(w, 4)
    marker = PID_EXTENDED | pid_flags_at(w.pl_cdr1.pid_flags, "extended-marker")
    write_u16(w, marker)
    write_u16(w, 8)
    member_header = UInt32(member_id) & E_MEMBER_ID_MASK
    if must_understand && w.pl_cdr1.member_must_understand
        member_header |= E_FLAG_MUST_UNDERSTAND
    end
    write_u32(w, member_header)
    at = wposition(w)
    append!(w.out, (0x00, 0x00, 0x00, 0x00))
    return ZdPid(at, true, begin_parameter_value(w), role, UInt32(0), must_understand)
end

"""Rewrite a short parameter header as the extended form: §7.4.1.2.2's
`ssize` is sixteen bits and which values overrun it is a property of the
sample. Eight bytes are inserted; XCDR1's MAXALIGN is eight, so every
offset inside the value keeps its remainder."""
function promote_pid(w::ZdWriter, header::ZdPid, len::Int64)
    start = header.at - 2
    for _ in 1:8
        insert!(w.out, Int(header.at) + 2 + 1, 0x00)
    end
    marker = PID_EXTENDED | pid_flags_at(w.pl_cdr1.pid_flags, "extended-marker")
    zd_patch(w, start, marker, 2)
    zd_patch(w, start + 2, 8, 2)
    member_header = header.pid & E_MEMBER_ID_MASK
    if header.must_understand && w.pl_cdr1.member_must_understand
        member_header |= E_FLAG_MUST_UNDERSTAND
    end
    zd_patch(w, start + 4, member_header, 4)
    zd_patch(w, start + 8, len, 4)
    return nothing
end

"""Patch a parameter header with its value's length.

The declared length covers the trailing padding or stops at the value,
by the framing; the origin goes back before the padding is written, and
under a framing that writes none the parameter ends where its value
does — the next header aligns itself.
"""
function finish_pid(w::ZdWriter, header::ZdPid)
    width = header.extended ? 4 : 2
    len = wposition(w) - header.at - width
    if pads_declared_length(w.pl_cdr1, header.role)
        past = mod(wposition(w) - header.saved_origin, 4)
        if past != 0
            len += 4 - past
        end
    end
    if !header.extended && len > PID_SHORT_MAX_LEN
        promote_pid(w, header, len)
    else
        zd_patch(w, header.at, len, width)
    end
    w.origin = header.saved_origin
    if w.pl_cdr1.pad_after_value
        align_to(w, 4)
    end
    return nothing
end

"""The `PID_LIST_END` sentinel; its flags come from the framing asked at
the `sentinel` position — it is not a member and has no declaration."""
function write_sentinel(w::ZdWriter)
    align_to(w, 4)
    write_u16(w, PID_LIST_END | pid_flags_at(w.pl_cdr1.pid_flags, "sentinel"))
    write_u16(w, 0)
    return nothing
end

# -- long double ---------------------------------------------------------

"""The sixteen octets of a binary128, from the decimal text the member
holds.

Julia has no binary128 — `Float64` is 53 significand bits against 113,
and `BigFloat`'s precision is a run-time global, not part of the type —
so a `long double` member is the decimal string and the conversion to
bits happens here, once, at the octet. Parsing under an MPFR precision
of 113 with round-to-nearest gives the correctly rounded significand.
"""
function zd_f128_bits(text::AbstractString)
    negative = startswith(text, "-")
    magnitude = negative ? text[2:end] : (startswith(text, "+") ? text[2:end] : text)
    value = setprecision(BigFloat, 113) do
        parse(BigFloat, magnitude)
    end
    sign_bit = negative ? UInt128(1) << 127 : UInt128(0)
    iszero(value) && return sign_bit
    e = exponent(value)
    biased = e + 16383
    if biased >= 0x7FFF
        # Overflow: the infinity, which is what every narrowing cast
        # produces for a magnitude the format cannot hold.
        return sign_bit | (UInt128(0x7FFF) << 112)
    end
    if biased >= 1
        mant = setprecision(BigFloat, 256) do
            BigInt(round((significand(value) - 1) * (BigFloat(2)^112)))
        end
        return sign_bit | (UInt128(biased) << 112) | UInt128(mant)
    end
    # Subnormal: the value scaled into the fixed 2^-16494 grid, rounded
    # to nearest.
    mant = setprecision(BigFloat, 512) do
        BigInt(round(value * (BigFloat(2)^16494)))
    end
    return sign_bit | UInt128(mant)
end

"""`long double`: sixteen bytes of IEEE 754 binary128, taken from the
decimal string the member holds. Aligned to eight, capped to four by
MAXALIGN under XCDR2, which `align_to` already does. The two byte
orders are the whole sixteen bytes reversed (CORBA 3.3 part 2 §9.3.1.3,
Figure 9.2)."""
function write_f128(w::ZdWriter, value::AbstractString)
    bits = zd_f128_bits(value)
    align_to(w, 8)
    if is_little(w.endianness)
        for i in 0:15
            push!(w.out, UInt8((bits >> (8 * i)) & 0xFF))
        end
    else
        for i in 15:-1:0
            push!(w.out, UInt8((bits >> (8 * i)) & 0xFF))
        end
    end
    return nothing
end

"""The exact decimal of sixteen binary128 octets.

Exact, because the value is `mantissa * 2^e` and a binary fraction's
decimal expansion terminates: for a negative exponent the digits are
`mantissa * 5^-e` with the point `-e` places from the right. The
inverse of `zd_f128_bits` for every finite value; the infinities and
NaN have no decimal and are refused.
"""
function zd_f128_decimal(bits::UInt128)
    negative = (bits >> 127) != 0
    biased = Int64((bits >> 112) & 0x7FFF)
    mantissa = BigInt(bits & ((UInt128(1) << 112) - 1))
    if biased == 0x7FFF
        throw(ZdOutOfRange(0, "a finite long double, which an infinity or NaN is not"))
    end
    significand_int = biased == 0 ? mantissa : mantissa + (BigInt(1) << 112)
    e = biased == 0 ? -16494 : biased - 16383 - 112
    iszero(significand_int) && return "0"
    out = ""
    if e >= 0
        out = string(significand_int << e)
    else
        digits_text = string(significand_int * BigInt(5)^(-e))
        point = -e
        if length(digits_text) <= point
            digits_text = string("0"^(point - length(digits_text) + 1), digits_text)
        end
        whole = digits_text[1:(end - point)]
        fraction = rstrip(digits_text[(end - point + 1):end], '0')
        out = isempty(fraction) ? whole : string(whole, ".", fraction)
    end
    return negative ? string("-", out) : out
end

# -- the reader ----------------------------------------------------------

"""Reads one sample's bytes."""
mutable struct ZdReader
    bytes::Vector{UInt8}
    at::Int64
    endianness::ZdEndianness
    max_align::Int64
    origin::Int64
    framing::ZdPlCdr1Framing
    status::ZdStatus
end

function ZdReader(
    data::Vector{UInt8},
    endianness::ZdEndianness,
    max_align::Integer = 8,
    framing::ZdPlCdr1Framing = ZdPlCdr1Framing(),
)
    return ZdReader(data, Int64(0), endianness, max(Int64(1), Int64(max_align)), Int64(0), framing, ZD_STATUS_OK)
end

"""How the read is going."""
reader_status(r::ZdReader) = r.status

"""Refuse the stream from outside the runtime; the first refusal sticks.

The generated decoder carries on to the end of the type — that is what
lets one test at the top of the caller decide what to do with the
sample — so a later refusal would describe a position no writer need
have put anything at.
"""
function reader_refuse(r::ZdReader, refusal::ZdStatus)
    if refusal !== ZD_STATUS_OK && r.status === ZD_STATUS_OK
        r.status = refusal
    end
    return nothing
end

"""Take back a `ZD_STATUS_DISCARDED`, and only that one: a member whose
`@try_construct` is USE_DEFAULT with an explicit `@default` asks for
DISCARD and clears it here to assign the literal. A truncated stream is
not a narrowing and stays."""
function reader_clear_discard(r::ZdReader)
    if r.status === ZD_STATUS_DISCARDED
        r.status = ZD_STATUS_OK
        return true
    end
    return false
end

"""Where it is."""
reader_position(r::ZdReader) = r.at

"""How many bytes are left."""
reader_remaining(r::ZdReader) = max(Int64(0), Int64(length(r.bytes)) - r.at)

"""Move the cursor forward to `target`, which is never backwards: a
framing word pointing behind the cursor means the member was already
read past, and seeking back would read it twice. Past the end throws —
there is no position there to carry on from."""
function reader_skip_to(r::ZdReader, target::Integer)
    r.status !== ZD_STATUS_OK && return nothing
    goal = Int64(target)
    if goal < r.at
        reader_refuse(r, ZD_STATUS_OUT_OF_RANGE)
        return nothing
    end
    if goal > length(r.bytes)
        throw(ZdTruncated(r.at, goal - r.at, reader_remaining(r)))
    end
    r.at = goal
    return nothing
end

"""The cursor, the origin and the status, to be put back later. The
status deliberately: a refusal made during a counting pass is about
bytes that are about to be read again."""
reader_save(r::ZdReader) = ZdReaderMark(r.at, r.origin, r.status)

"""Put back what `reader_save` took."""
function reader_restore(r::ZdReader, mark::ZdReaderMark)
    r.at = mark.at
    r.origin = mark.origin
    r.status = mark.status
    return nothing
end

"""Move the alignment origin to the current position and answer the
origin displaced, for [`reader_finish_packed_element`](@ref). The
read-side half of [`begin_packed_element`](@ref)."""
function reader_begin_packed_element(r::ZdReader)
    saved = r.origin
    r.origin = r.at
    return saved
end

"""Restore the origin [`reader_begin_packed_element`](@ref) displaced."""
function reader_finish_packed_element(r::ZdReader, saved::Integer)
    r.origin = saved
    return nothing
end

"""Skip padding to `align`, counted from the stream's origin."""
function align_to(r::ZdReader, align::Integer)
    a = min(Int64(align), r.max_align)
    a <= 1 && return nothing
    offset = r.at - r.origin
    r.at += mod(a - mod(offset, a), a)
    return nothing
end

function take_bytes(r::ZdReader, count::Integer)
    stop = r.at + Int64(count)
    if stop > length(r.bytes)
        throw(ZdTruncated(r.at, Int64(count), reader_remaining(r)))
    end
    chunk = r.bytes[(r.at + 1):Int(stop)]
    r.at = stop
    return chunk
end

function read_uint_aligned(r::ZdReader, width::Integer)
    align_to(r, width)
    raw = take_bytes(r, width)
    value = UInt64(0)
    if is_little(r.endianness)
        for i in width:-1:1
            value = (value << 8) | UInt64(raw[i])
        end
    else
        for i in 1:width
            value = (value << 8) | UInt64(raw[i])
        end
    end
    return value
end

"""`boolean` — a byte that is neither 0 nor 1 is a stream that disagrees
with its own type, refused rather than coerced."""
function read_bool(r::ZdReader)
    at = r.at
    value = read_u8(r)
    value == 0 && return false
    value == 1 && return true
    throw(ZdOutOfRange(at, "a boolean of 0 or 1"))
end

"""`octet` and `uint8`."""
read_u8(r::ZdReader) = take_bytes(r, 1)[1]

"""`int8`."""
read_i8(r::ZdReader) = reinterpret(Int8, take_bytes(r, 1)[1])

"""`char`, one byte, as the code unit the generated struct holds."""
read_char(r::ZdReader) = take_bytes(r, 1)[1]

"""`wchar`, at `width` octets and in `framing`'s shape.

The four-byte form zero-extends one UTF-16 code unit; a value that
uses the high bits is refused rather than narrowed. GIOP's length
octet is checked against the configured width rather than believed.
A surrogate is refused: it is a code unit and not a code point.
"""
function read_wchar(
    r::ZdReader,
    width::Integer = 2,
    framing::AbstractString = "fixed",
    order::AbstractString = "stream",
)
    at = r.at
    (width == 2 || width == 4) ||
        throw(ZdOutOfRange(at, "a wchar width of 2 or 4, not $(width)"))
    local value::UInt32
    if framing == "giop-length-prefixed"
        declared = read_u8(r)
        if declared != width
            throw(ZdOutOfRange(
                at,
                "a GIOP 1.2 wchar of $(width) octets, the configured transmission code set, and not $(declared)",
            ))
        end
        raw = take_bytes(r, width)
        big = order != "stream" || !is_little(r.endianness)
        value = UInt32(0)
        if big
            for i in 1:width
                value = (value << 8) | UInt32(raw[i])
            end
        else
            for i in width:-1:1
                value = (value << 8) | UInt32(raw[i])
            end
        end
    elseif framing != "fixed"
        throw(ZdOutOfRange(at, "a wchar framing, not `$(framing)`"))
    elseif width == 4
        value = UInt32(read_u32(r))
        value > 0xFFFF &&
            throw(ZdOutOfRange(at, "a four-byte wchar holding one UTF-16 code unit"))
    else
        value = UInt32(read_u16(r))
    end
    (0xD800 <= value <= 0xDFFF) && throw(ZdOutOfRange(at, "a Unicode code point"))
    return UInt16(value)
end

"""`short`."""
read_i16(r::ZdReader) = reinterpret(Int16, UInt16(read_uint_aligned(r, 2)))

"""`unsigned short`."""
read_u16(r::ZdReader) = UInt16(read_uint_aligned(r, 2))

"""`long`."""
read_i32(r::ZdReader) = reinterpret(Int32, UInt32(read_uint_aligned(r, 4)))

"""`unsigned long`."""
read_u32(r::ZdReader) = UInt32(read_uint_aligned(r, 4))

"""`long long`."""
read_i64(r::ZdReader) = reinterpret(Int64, read_uint_aligned(r, 8))

"""`unsigned long long`."""
read_u64(r::ZdReader) = read_uint_aligned(r, 8)

"""`float`."""
read_f32(r::ZdReader) = reinterpret(Float32, UInt32(read_uint_aligned(r, 4)))

"""`double`."""
read_f64(r::ZdReader) = reinterpret(Float64, read_uint_aligned(r, 8))

"""`long double`: all sixteen bytes, back into the exact decimal the
member holds."""
function read_f128(r::ZdReader)
    align_to(r, 8)
    raw = take_bytes(r, 16)
    bits = UInt128(0)
    if is_little(r.endianness)
        for i in 16:-1:1
            bits = (bits << 8) | UInt128(raw[i])
        end
    else
        for i in 1:16
            bits = (bits << 8) | UInt128(raw[i])
        end
    end
    return zd_f128_decimal(bits)
end

# -- strings -------------------------------------------------------------

"""A string: a length including the terminator, the bytes, the NUL."""
function read_string_alloc(r::ZdReader)
    at = r.at
    len = read_u32(r)
    if len == 0 || Int64(len) > reader_remaining(r) + 1
        throw(ZdImpossibleLength(at, Int64(len), reader_remaining(r)))
    end
    raw = take_bytes(r, Int64(len) - 1)
    text = String(raw)
    isvalid(text) || throw(ZdNotUtf8(at))
    read_u8(r)
    return text
end

"""Take the byte-order mark off the front and apply what it said. The
units arrived little-endian; `FFFE` swaps every unit back, and no mark
means CORBA's default of big-endian. The mark is never a character of
the returned string."""
function strip_byte_order_mark(units::Vector{UInt16})
    isempty(units) && return units
    units[1] == ZD_BYTE_ORDER_MARK && return units[2:end]
    rest = units[1] == ZD_SWAPPED_BYTE_ORDER_MARK ? units[2:end] : units
    return [UInt16(((unit & 0xFF) << 8) | (unit >> 8)) for unit in rest]
end

function read_wstring_units(r::ZdReader, at::Int64, count::Int64, width::Int, order::AbstractString)
    if count * width > reader_remaining(r)
        throw(ZdImpossibleLength(at, count * width, reader_remaining(r)))
    end
    little = order == "byte-order-mark" || order == "host-unswapped"
    units = UInt16[]
    for _ in 1:count
        local wide::UInt32
        if width == 4
            raw32 = if order == "big-endian-always"
                align_to(r, 4)
                b = take_bytes(r, 4)
                (UInt32(b[1]) << 24) | (UInt32(b[2]) << 16) | (UInt32(b[3]) << 8) | UInt32(b[4])
            elseif little
                align_to(r, 4)
                b = take_bytes(r, 4)
                (UInt32(b[4]) << 24) | (UInt32(b[3]) << 16) | (UInt32(b[2]) << 8) | UInt32(b[1])
            else
                UInt32(read_u32(r))
            end
            raw32 > 0xFFFF &&
                throw(ZdOutOfRange(at, "a four-byte wstring character holding one UTF-16 unit"))
            wide = raw32
        else
            wide = if order == "big-endian-always"
                align_to(r, 2)
                b = take_bytes(r, 2)
                (UInt32(b[1]) << 8) | UInt32(b[2])
            elseif little
                align_to(r, 2)
                b = take_bytes(r, 2)
                (UInt32(b[2]) << 8) | UInt32(b[1])
            else
                UInt32(read_u16(r))
            end
        end
        push!(units, UInt16(wide))
    end
    return units
end

"""A `wstring`, in the form `policy` and `order` name.

The form is never inferred from the stream: a Fast DDS `"ab"` followed
by a `long` is byte-identical to a specification `"a"` followed by the
same `long`. A count the unit width does not divide is refused rather
than rounded down — rounding would leave the remainder in the stream.
"""
function read_wstring_utf8(
    r::ZdReader,
    policy::AbstractString = "specification",
    order::AbstractString = "stream",
)
    at = r.at
    declared = Int64(read_u32(r))
    order in ZD_WSTRING_UNIT_ORDERS ||
        throw(ZdOutOfRange(at, "a wstring unit order, not `$(order)`"))
    cell = get(ZD_WSTRING_CELLS, policy, nothing)
    cell === nothing && throw(ZdOutOfRange(at, "a wstring length policy, not `$(policy)`"))
    counts_characters, terminated, width = cell
    units = if counts_characters
        read_wstring_units(r, at, declared, width, order)
    else
        mod(declared, width) == 0 || throw(ZdImpossibleLength(at, declared, reader_remaining(r)))
        read_wstring_units(r, at, div(declared, width), width, order)
    end
    if terminated
        declared < 1 && throw(ZdImpossibleLength(at, declared, reader_remaining(r)))
        last = pop!(units)
        last == 0 ||
            throw(ZdOutOfRange(at, "a NUL character ending the wstring, as its count includes one"))
        # A NUL inside the text is the narrow string's problem in wide
        # form: every C peer stops at it. Only where a terminator is
        # expected — a cell without one may carry U+0000 as a character.
        any(unit -> unit == 0, units) &&
            throw(ZdOutOfRange(at, "no NUL inside a wstring a C peer would stop at"))
    end
    if order == "byte-order-mark"
        units = strip_byte_order_mark(units)
    end
    out = IOBuffer()
    index = 1
    while index <= length(units)
        unit = UInt32(units[index])
        if 0xD800 <= unit <= 0xDBFF && index + 1 <= length(units)
            low = UInt32(units[index + 1])
            if 0xDC00 <= low <= 0xDFFF
                print(out, Char(0x10000 + ((unit - 0xD800) << 10) + (low - 0xDC00)))
                index += 2
                continue
            end
        end
        (0xD800 <= unit <= 0xDFFF) &&
            throw(ZdOutOfRange(at, "a UTF-16 sequence with no lone surrogate"))
        print(out, Char(unit))
        index += 1
    end
    return String(take!(out))
end

"""The inverse of [`write_wstring_utf8_octets`](@ref): the declared
length is the UTF-8 octet count, the terminator the policy asks for is
the last of those octets, and a count the stream cannot hold, a missing
or an interior NUL, or an ill-formed UTF-8 sequence is refused. See
`WstringCodeUnits::Utf8`."""
function read_wstring_utf8_octets(
    r::ZdReader,
    policy::AbstractString = "specification",
)
    at = r.at
    declared = Int64(read_u32(r))
    cell = get(ZD_WSTRING_CELLS, policy, nothing)
    cell === nothing && throw(ZdOutOfRange(at, "a wstring length policy, not `$(policy)`"))
    _, terminated, _ = cell
    declared > reader_remaining(r) &&
        throw(ZdImpossibleLength(at, declared, reader_remaining(r)))
    raw = take_bytes(r, declared)
    if terminated
        (declared < 1 || raw[end] != 0x00) &&
            throw(ZdOutOfRange(at, "a NUL octet ending the wstring, as its count includes one"))
        pop!(raw)
        # A NUL inside the text is the narrow string's problem in wide
        # form: every C peer stops at it.
        any(==(0x00), raw) &&
            throw(ZdOutOfRange(at, "no NUL inside a wstring a C peer would stop at"))
    end
    text = String(raw)
    isvalid(text) || throw(ZdNotUtf8(at))
    return text
end

"""`fixed<digits, scale>`, as the decimal string the writer took: the
leading zeros go — `"0.01"` is what the sample said — and a negative
zero loses its sign, because `-0.00` and `0.00` are one value. Under
`value-precision` the field's end is read out of the octets: the field
ends at the first octet whose low nibble is a sign."""
function read_fixed(r::ZdReader, digits::Integer, scale::Integer, count::AbstractString = "declared")
    count == "declared" || count == "value-precision" || count == "value-string-digits" ||
        throw(ZdOutOfRange(r.at, "a fixed digit count, not `$(count)`"))
    # The same bound as the writer's, on this side too: OMG IDL 4.2 rule
    # (42) caps the BCD rule at 31 digits, and a reader asked for a wider
    # field would take octets no declaration can name.
    (digits >= 1 && digits <= 31 && scale <= digits) ||
        throw(ZdOutOfRange(r.at,
            "a fixed<P, S> with S <= P and 1 <= P <= 31, not fixed<$(digits), $(scale)>"))
    nibbles = Int[]
    local sign::Int
    if count == "value-precision" || count == "value-string-digits"
        while true
            length(nibbles) >= 32 &&
                throw(ZdOutOfRange(r.at, "a sign nibble within sixteen octets"))
            byte = take_bytes(r, 1)[1]
            push!(nibbles, Int(byte >> 4))
            push!(nibbles, Int(byte & 0x0F))
            (byte & 0x0F) >= 0x0A && break
        end
        sign = pop!(nibbles)
    else
        raw = take_bytes(r, div(Int64(digits) + 2, 2))
        for byte in raw
            push!(nibbles, Int(byte >> 4))
            push!(nibbles, Int(byte & 0x0F))
        end
        sign = pop!(nibbles)
        if mod(digits + 1, 2) == 1
            popfirst!(nibbles)
        end
    end
    any(nibble -> nibble > 9, nibbles) && throw(ZdOutOfRange(r.at, "BCD digits"))
    text = join(string(nibble) for nibble in nibbles)
    if length(text) < scale + 1
        text = string("0"^(Int(scale) + 1 - length(text)), text)
    end
    split_at = length(text) - Int(scale)
    whole = lstrip(text[1:split_at], '0')
    isempty(whole) && (whole = "0")
    fraction = text[(split_at + 1):end]
    out = scale == 0 ? whole : string(whole, ".", fraction)
    zero = whole == "0" && all(c -> c == '0', fraction)
    return (sign == 0x0D && !zero) ? string("-", out) : out
end

# -- the trims a bound reaches for ---------------------------------------

"""How many octets `text` is, which is what a `string<N>` bound counts."""
utf8_octets(text::AbstractString) = ncodeunits(String(text))

"""How many UTF-16 code units `text` is, which is what a `wstring<N>`
bound counts; a scalar above the basic plane is two."""
utf16_units(text::AbstractString) = sum(Int64(UInt32(c) > 0xFFFF ? 2 : 1) for c in text; init = Int64(0))

function zd_trim(text::AbstractString, limit::Integer, wide::Bool)
    used = Int64(0)
    out = IOBuffer()
    for character in text
        cost = wide ? Int64(UInt32(character) > 0xFFFF ? 2 : 1) : Int64(ncodeunits(string(character)))
        used + cost > limit && break
        used += cost
        print(out, character)
    end
    return String(take!(out))
end

"""`text` cut to at most `octets` bytes, never inside a character.
§7.2.2.7, Table 11: a prefix and never a reordering."""
trim_utf8(r::ZdReader, text::AbstractString, octets::Integer) = zd_trim(text, octets, false)

"""`text` cut to at most `units` UTF-16 code units; a character above
the basic plane is left out whole rather than half-written."""
trim_utf16(r::ZdReader, text::AbstractString, units::Integer) = zd_trim(text, units, true)

# -- the bit-bounded kinds -----------------------------------------------

"""An enumerator's holder, as the signed integer §7.4.3.5.3 makes it:
rule (5)'s holder is Int8, Int16 or Int32, so the top bit of the bound
reads back negative and the generated decoder matches the pattern."""
function read_enum(r::ZdReader, bit_bound::Integer)
    (bit_bound == 0 || bit_bound > 32) &&
        throw(ZdOutOfRange(r.at, "an @bit_bound between 1 and 32, not $(bit_bound)"))
    if bit_bound <= 8
        return Int64(reinterpret(Int8, read_u8(r)))
    elseif bit_bound <= 16
        return Int64(read_i16(r))
    end
    return Int64(read_i32(r))
end

"""A bitmask's holder. Table 33 of §7.4.1.1.3.2."""
function read_bitmask(r::ZdReader, bit_bound::Integer)
    (bit_bound == 0 || bit_bound > 64) &&
        throw(ZdOutOfRange(r.at, "an @bit_bound between 1 and 64, not $(bit_bound)"))
    bit_bound <= 8 && return UInt64(read_u8(r))
    bit_bound <= 16 && return UInt64(read_u16(r))
    bit_bound <= 32 && return UInt64(read_u32(r))
    return read_u64(r)
end

"""A bitset's anchor integer, of the width the planner sized."""
function read_bitset(r::ZdReader, anchor::Integer)
    anchor == 1 && return UInt64(read_u8(r))
    anchor == 2 && return UInt64(read_u16(r))
    anchor == 4 && return UInt64(read_u32(r))
    anchor == 8 && return read_u64(r)
    throw(ZdOutOfRange(r.at, "a bitset anchor of 1, 2, 4 or 8 bytes, not $(anchor)"))
end

# -- framing -------------------------------------------------------------

"""A DHEADER: the length the members that follow occupy, refused where
the stream cannot hold it."""
function read_dheader(r::ZdReader)
    at = r.at
    len = read_u32(r)
    Int64(len) > reader_remaining(r) && throw(ZdImpossibleLength(at, Int64(len), reader_remaining(r)))
    return len
end

"""An EMHEADER, unpacked. Codes 5 to 7 report the NEXTINT they read but
the cursor is put back before it: that word is the member's own leading
length and the member's decoder has to see it. §7.4.3.4.2."""
function read_emheader(r::ZdReader)
    word = read_u32(r)
    length_code = Int64((word >> 28) & 0x7)
    member_id = word & E_MEMBER_ID_MASK
    must_understand = (word >> 31) != 0
    if length_code == 4
        next = Int64(read_u32(r))
        return ZdEmHeaderRead(member_id, length_code, must_understand, next, emheader_extent(length_code, next))
    end
    if length_code >= 5
        at = r.at
        peeked = Int64(read_u32(r))
        r.at = at
        return ZdEmHeaderRead(member_id, length_code, must_understand, peeked, emheader_extent(length_code, peeked))
    end
    return ZdEmHeaderRead(member_id, length_code, must_understand, Int64(-1), emheader_extent(length_code, Int64(-1)))
end

"""A PL_CDR1 parameter header, in whichever form it was written, and
the value block it opens.

§7.4.1.2: a parameter's value is its own stream block, so a member's
internal alignment is counted from the first octet after the header —
the origin moves here and `finish_pid_read` puts it back. `role` is the
caller's, not the stream's: the bytes are the same bytes and only what
the declared length means differs.
"""
function read_pid_as(r::ZdReader, role::ZdParameterRole)
    if r.status !== ZD_STATUS_OK
        return ZdPidRead(UInt32(0), UInt32(0), true, false, false, false, false, role, r.origin, r.at)
    end
    align_to(r, 4)
    pid = UInt32(read_u16(r))
    declared = UInt32(read_u16(r))
    if (pid & PID_ID_MASK) == (PID_EXTENDED & PID_ID_MASK)
        # The four flag bits are not part of the id: §7.4.1.2.1 splits
        # the eMemberHeader into FLAG_1..FLAG_4 and a 28-bit member id,
        # and FLAG_2 is the member's must-understand.
        member_header = read_u32(r)
        wide_length = read_u32(r)
        saved = r.origin
        r.origin = r.at
        return ZdPidRead(
            member_header & E_MEMBER_ID_MASK,
            wide_length,
            false,
            (member_header & E_FLAG_MUST_UNDERSTAND) != 0,
            (member_header & E_FLAG_IMPL_EXTENSION) != 0,
            false,
            false,
            role,
            saved,
            r.at,
        )
    end
    identifier = pid & PID_ID_MASK
    is_end = identifier == (PID_LIST_END & PID_ID_MASK)
    # Under a framing that puts FLAG_IMPL_EXTENSION on every member id
    # the bit no longer tells a vendor-private parameter from a user
    # member (OpenDDS writes c0010004 for `@id(1) long a`).
    flagged =
        (pid & PID_IMPL_EXTENSION) != 0 &&
        (pid_flags_at(r.framing.pid_flags, "member") & PID_IMPL_EXTENSION) == 0
    saved = r.origin
    r.origin = r.at
    return ZdPidRead(
        identifier,
        declared,
        is_end,
        (pid & PID_MUST_UNDERSTAND) != 0,
        flagged,
        !is_end && identifier > r.framing.highest_short_id,
        identifier == PID_IGNORE,
        role,
        saved,
        r.at,
    )
end

"""End a parameter's value and step to the next header.

The declared length is measured against the value's own start and
checked against the configured framing rather than believed — both
readings are accepted where they coincide, which is every value already
a multiple of four. The block's origin goes back first; the padding to
four is stepped only where the framing writes it (`pad_after_value`):
under Fast DDS's framing a value may be followed immediately by an
unframed member, and aligning past it would take that member's bytes
as padding. Nothing follows `PID_LIST_END`, and a reader that has
already refused does not move.
"""
function finish_pid_read(r::ZdReader, header::ZdPidRead)
    r.origin = header.saved_origin
    (r.status !== ZD_STATUS_OK || header.end_) && return nothing
    used = r.at - header.value_at
    declared = Int64(header.length)
    padded = (used + 3) & ~Int64(3)
    pads = pads_declared_length(r.framing, header.role)
    if declared != (pads ? padded : used) && declared != used && declared != padded
        reader_refuse(r, ZD_STATUS_OUT_OF_RANGE)
    end
    if r.framing.pad_after_value
        align_to(r, 4)
    end
    return nothing
end

"""Step over `count` bytes, refusing to step past the end."""
function skip(r::ZdReader, count::Integer)
    take_bytes(r, count)
    return nothing
end

"""Step over a member this reader does not know — the whole point of
the length code. §7.4.3.4.2."""
function skip_member(r::ZdReader, header::ZdEmHeaderRead)
    if header.length_code <= 3
        skip(r, Int64(1) << header.length_code)
        return nothing
    end
    header.next_int < 0 &&
        throw(ZdOutOfRange(r.at, "a NEXTINT for a length code of 4 or above"))
    if header.length_code == 4
        skip(r, header.next_int)
        return nothing
    end
    scale = header.length_code == 5 ? Int64(1) : header.length_code == 6 ? Int64(4) : Int64(8)
    skip(r, Int64(4) + header.next_int * scale)
    return nothing
end

# -- the union overlay patterns ------------------------------------------

"""The low `octets` of a value's two's-complement bits, zero-extended:
`-2` in four octets is `0x00000000FFFFFFFE`."""
function bit_pattern_int(value::Integer, octets::Integer)
    bits = value isa UInt64 ? value : reinterpret(UInt64, Int64(value))
    octets >= 8 && return bits
    return bits & ((UInt64(1) << (8 * octets)) - 1)
end

"""One octet, 0 or 1 — the boolean overlay pattern."""
bit_pattern_bool(value::Bool) = UInt64(value)

"""The IEEE 754 binary32 bits, zero-extended."""
bit_pattern_f32(value::Real) = UInt64(reinterpret(UInt32, Float32(value)))

"""The IEEE 754 binary64 bits."""
bit_pattern_f64(value::Real) = reinterpret(UInt64, Float64(value))

# -- the scratch a decoder assembles into --------------------------------

"""A mutable stand-in for an immutable declared type.

The generated types are immutable on purpose — a decoded sample is a
value — and the shared codec walk assigns members one at a time. The
scratch takes those stores in a dictionary behind ordinary property
syntax, answers `nothing` for a slot nothing filled (which is exactly
what an absent `@optional` member is), and `zd_freeze` builds the
declared type from it at every return.
"""
mutable struct ZdScratch
    __zd_dict__::Dict{Symbol,Any}
end

Base.getproperty(s::ZdScratch, name::Symbol) =
    name === :__zd_dict__ ? getfield(s, :__zd_dict__) :
    get(getfield(s, :__zd_dict__), name, nothing)
Base.setproperty!(s::ZdScratch, name::Symbol, value) =
    (getfield(s, :__zd_dict__)[name] = value)

"""A scratch from alternating name/value arguments:
`zd_scratch_of("x", Int32(0), "y", 0.0)`."""
function zd_scratch_of(pairs...)
    dict = Dict{Symbol,Any}()
    for index in 1:2:length(pairs)
        dict[Symbol(pairs[index])] = pairs[index + 1]
    end
    return ZdScratch(dict)
end

"""An empty scratch — a map entry before either half arrived."""
zd_scratch_entry() = ZdScratch(Dict{Symbol,Any}())

"""`count` slots of `nothing`, for a sequence under construction."""
zd_nothing_vector(count::Integer) = Any[nothing for _ in 1:Int64(count)]

"""`count` distinct entry scratches, for a map under construction:
each is its own object, so `v[i].key = …` writes one entry."""
zd_entry_vector(count::Integer) = Any[zd_scratch_entry() for _ in 1:Int64(count)]

"""The one arm of a `Union{T, Nothing}` that is not `Nothing`."""
function zd_union_arm(u::Union)
    u.a === Nothing && return u.b
    u.b === Nothing && return u.a
    throw(ZdOutOfRange(0, "a Union{T, Nothing}, which this union is not"))
end

"""The declared value a finished scratch adds up to.

Recursive by the *declared* type: a scratch becomes the struct whose
fields it filled (by `fieldnames`, which are the escaped Julia names
the defaulters and the codec both write), an `Any`-vector becomes the
declared `Vector{T}` or `NTuple`, `Union{T, Nothing}` keeps `nothing`
and freezes the other arm, and a value that is already the declared
type passes through untouched — which is what makes a nested decoder's
answer cheap to embed.
"""
function zd_freeze(::Type{T}, value) where {T}
    if T isa Union
        value === nothing && return nothing
        return zd_freeze(zd_union_arm(T), value)
    end
    value isa T && return value
    # A member inside a type-graph cycle is declared against the
    # abstract `ZdFwd_*` stand-in the materialiser had to name before
    # the struct existed. The stand-in has exactly one subtype in the
    # generated file, and that subtype is the declared type.
    if isabstracttype(T)
        subtypes = InteractiveUtils.subtypes(T)
        length(subtypes) == 1 ||
            throw(ZdOutOfRange(0, "an abstract type with exactly one subtype, not $(T)"))
        return zd_freeze(subtypes[1], value)
    end
    if value isa ZdScratch
        dict = getfield(value, :__zd_dict__)
        args = Any[]
        for (index, field_name) in enumerate(fieldnames(T))
            push!(args, zd_freeze(fieldtype(T, index), get(dict, field_name, nothing)))
        end
        return T(args...)
    end
    if value isa AbstractVector
        if T <: Tuple
            count = fieldcount(T)
            length(value) == count ||
                throw(ZdOutOfRange(0, "an array of exactly $(count) elements, not $(length(value))"))
            element = eltype(T)
            return T(ntuple(i -> zd_freeze(element, value[i]), count))
        end
        if T <: AbstractVector
            element = eltype(T)
            out = Vector{element}(undef, length(value))
            for (index, item) in enumerate(value)
                out[index] = zd_freeze(element, item)
            end
            return out
        end
    end
    if T <: Base.Enum && value isa Integer
        return T(value)
    end
    return convert(T, value)
end

"""A 64-bit signed reading of any holder this column stores.

`Int64(x)` for everything a conversion is exact for, and the bit
pattern for a `UInt64` above `typemax(Int64)` — which is what the walk
means where it asks for its widest integer.
"""
zd_i64(x::UInt64) = reinterpret(Int64, x)
zd_i64(x::Bool) = Int64(x)
zd_i64(x) = Int64(x)

# -- JSON spellings the generated dumpers share --------------------------

"""A JSON string literal, quotes included. The escapes are JSON's; code
points above ASCII go out raw, because the document is UTF-8."""
function zd_jtext(text::AbstractString)
    out = IOBuffer()
    print(out, '"')
    for character in text
        code = UInt32(character)
        if character == '"'
            print(out, "\\\"")
        elseif character == '\\'
            print(out, "\\\\")
        elseif code < 0x20
            print(out, "\\u", string(code, base = 16, pad = 4))
        else
            print(out, character)
        end
    end
    print(out, '"')
    return String(take!(out))
end

"""A one-character JSON string from a code unit, escaped where the unit
is not a printable ASCII character — which also covers the surrogates a
`Char` will not hold."""
function zd_jchar(code::Integer)
    if 0x20 <= code < 0x7F && code != UInt32('"') && code != UInt32('\\')
        return string('"', Char(code), '"')
    end
    return string("\"\\u", string(UInt32(code), base = 16, pad = 4), '"')
end

"""A JSON number from any of this column's binary floats. Through
`Float64` first: Julia prints a `Float32` with an `f0` exponent marker,
which no JSON parser reads. NaN and the infinities have no JSON form
and are refused by name."""
function zd_jfloat(value::Real)
    wide = Float64(value)
    isnan(wide) && throw(ZdOutOfRange(0, "a float JSON can carry, which NaN is not"))
    isinf(wide) && throw(ZdOutOfRange(0, "a float JSON can carry, which an infinity is not"))
    text = string(wide)
    return text
end

end # module ZeroddsWire
