// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The Zig wire runtime: DDS-XTypes 1.3 §7.4 from the clause.
//!
//! The counterpart of `crates/wire`, `runtime/c`, `runtime/d` and the
//! other columns — a further implementation of the same clauses, not a
//! binding to any of them. Where two columns disagree on an octet, one
//! of them has misread the specification, and that is the point of
//! having both. Nothing here decides a type question: which members are
//! written, in what order, under which header and with which length
//! code arrives as arguments from the generated codec.
//!
//! The operations take the shared walk's argument order — `writeEnum
//! (bitBound, value)`, `writeBitset(anchor, value)` — and the walk's
//! spelling in Zig's own case, because this runtime was written against
//! the walk rather than adapted to it afterwards.
//!
//! ## Errors are error unions, and their fallibility is a contract
//!
//! Zig has no exceptions. A malformed stream or a value outside its
//! type answers `WireError` and unwinds through the generated `try`
//! chain to the harness, which turns it into the verdict's exit code; a
//! refusal about the *type* sets the reader's status instead and the
//! generated decoder walks on to the end of the sample. Which
//! operations are fallible is agreed with
//! `crates/lang-rules/src/emit/zig.rs` — a name moved on one side
//! without the other is `expected error union type` from the compiler,
//! never a silent drop.
//!
//! ## An IDL `long double` is a real `f128`
//!
//! Zig has a genuine binary128, so — alone among the columns — nothing
//! here carries a decimal in a string: `writeF128` takes the bits and
//! `readF128` gives them back, and the two byte orders are the sixteen
//! bytes reversed (CORBA 3.3 part 2 §9.3.1.3).
//!
//! ## The PID logic follows the C# reader
//!
//! `finishPidRead` measures the value's extent from `value_start` — the
//! first octet after the header, where §7.4.1.2 starts the parameter's
//! own stream block — checks the declared length against both readings
//! of the padding convention, and only then steps to the next header.
//! `runtime/csharp/src/Reader.cs` is the sibling this clause was read
//! against.

const std = @import("std");

// -- errors ----------------------------------------------------------------

/// Every wire failure, and the allocator's, in one set: a generated
/// codec propagates all of them with one `try`.
pub const WireError = error{
    /// The stream ended before the value did.
    Truncated,
    /// A length field claims more than the stream holds. Separate from
    /// `Truncated`: a length that cannot be true is a malformed stream,
    /// not a short one.
    ImpossibleLength,
    /// A string held bytes that are not UTF-8.
    NotUtf8,
    /// A value outside what its type admits.
    OutOfRange,
    /// The reader's or writer's arena is exhausted.
    OutOfMemory,
};

// -- policies --------------------------------------------------------------

/// The byte order a stream is written or read in.
pub const Endianness = enum { little, big };

/// What a receiving position does with a value it cannot hold.
/// DDS-XTypes 1.3 §7.2.2.7, Table 11.
pub const TryConstruct = enum { discard, use_default, trim };

/// What a reader answers about the type, without failing.
pub const Status = enum {
    ok,
    truncated,
    impossible_length,
    out_of_range,
    discarded,

    /// The status, spelled the way the sibling runtimes log it.
    pub fn text(self: Status) []const u8 {
        return switch (self) {
            .ok => "ok",
            .truncated => "truncated",
            .impossible_length => "impossible-length",
            .out_of_range => "out-of-range",
            .discarded => "discarded",
        };
    }
};

/// Which of §7.4.1.2's two roles a parameter header stands in.
pub const ParameterRole = enum { member, optional };

/// The parameter ids and flags of XTypes 1.3 §7.4.1.2.2 and
/// DDSI-RTPS 2.5 §9.4.2.11.2.
pub const pid_extended: u32 = 0x3F01;
/// Ends a parameter list.
pub const pid_list_end: u32 = 0x3F02;
/// The id every consumer shall ignore.
pub const pid_ignore: u32 = 0x3F03;
/// The longest value a short header can state.
pub const pid_short_max_len: u32 = 0xFFFF;
/// The must-understand bit of a short id.
pub const pid_must_understand: u32 = 0x4000;
/// FLAG_IMPL_EXTENSION, bit 15 of a short id.
pub const pid_impl_extension: u32 = 0x8000;

const e_flag_must_understand: u32 = 0x4000_0000;
const e_flag_impl_extension: u32 = 0x8000_0000;
const e_member_id_mask: u32 = 0x0FFF_FFFF;
const pid_id_mask: u32 = 0x3FFF;

/// Where in a PL_CDR1 list a flag question is asked.
const PidPosition = enum { member, sentinel, extended_marker };

/// What flags a PL_CDR1 parameter id carries at `position` — an
/// unknown policy fails: a typo in a wire-affecting setting must be
/// loud.
fn pidFlagsAt(policy: []const u8, position: PidPosition) WireError!u32 {
    if (std.mem.eql(u8, policy, "sentinel-must-understand")) {
        return switch (position) {
            .sentinel, .extended_marker => pid_must_understand,
            .member => 0,
        };
    }
    if (std.mem.eql(u8, policy, "bare")) {
        return 0;
    }
    if (std.mem.eql(u8, policy, "impl-extension-members")) {
        return switch (position) {
            .member, .extended_marker => pid_impl_extension | pid_must_understand,
            .sentinel => 0,
        };
    }
    return WireError.OutOfRange;
}

/// How a PL_CDR1 parameter list is framed — the ways the measured
/// implementations differ inside an XCDR1 sample.
pub const PlCdr1Framing = struct {
    /// What flags a parameter id carries, asked per position.
    pid_flags: []const u8,
    /// The highest member id a short parameter header carries. Both
    /// sides: `writePid` refuses an id above it rather than putting a
    /// reserved parameter on the wire.
    highest_short_id: u32,
    /// Whether a @mutable member's declared length covers its padding.
    pad_declared_length: bool,
    /// The same question for an @optional member outside @mutable.
    pad_declared_optional_length: bool,
    /// Whether a member's own must-understand bit goes out at all.
    member_must_understand: bool,
    /// Whether the parameter itself writes the padding after its value
    /// (RTI's framing) or leaves the next header to align itself
    /// (Fast DDS's, where a final parameter ends the stream at its
    /// value).
    pad_after_value: bool,

    fn padsDeclaredLength(self: PlCdr1Framing, role: ParameterRole) bool {
        return switch (role) {
            .optional => self.pad_declared_optional_length,
            .member => self.pad_declared_length,
        };
    }
};

/// One cell of the wstring C1 x C2 x C3 product.
const WstringCell = struct {
    counts_characters: bool,
    terminated: bool,
    width: u16,
};

/// The five named forms and the three coordinate spellings; the
/// spellings are `WstringLengthPolicy::as_str`'s. An unknown name
/// fails, for the reason `pidFlagsAt` gives.
fn wstringCellOf(policy: []const u8) WireError!WstringCell {
    const eql = std.mem.eql;
    if (eql(u8, policy, "specification") or eql(u8, policy, "octets/unterminated/2")) {
        return .{ .counts_characters = false, .terminated = false, .width = 2 };
    }
    if (eql(u8, policy, "character-count") or eql(u8, policy, "characters/unterminated/2")) {
        return .{ .counts_characters = true, .terminated = false, .width = 2 };
    }
    if (eql(u8, policy, "character-count-terminated") or eql(u8, policy, "characters/terminated/4")) {
        return .{ .counts_characters = true, .terminated = true, .width = 4 };
    }
    if (eql(u8, policy, "specification-wide-units") or eql(u8, policy, "octets/unterminated/4")) {
        return .{ .counts_characters = false, .terminated = false, .width = 4 };
    }
    if (eql(u8, policy, "ros-type-support") or eql(u8, policy, "characters/unterminated/4")) {
        return .{ .counts_characters = true, .terminated = false, .width = 4 };
    }
    if (eql(u8, policy, "octets/terminated/2")) {
        return .{ .counts_characters = false, .terminated = true, .width = 2 };
    }
    if (eql(u8, policy, "octets/terminated/4")) {
        return .{ .counts_characters = false, .terminated = true, .width = 4 };
    }
    if (eql(u8, policy, "characters/terminated/2")) {
        return .{ .counts_characters = true, .terminated = true, .width = 2 };
    }
    return WireError.OutOfRange;
}

fn wstringOrderKnown(order: []const u8) bool {
    const eql = std.mem.eql;
    return eql(u8, order, "stream") or eql(u8, order, "big-endian-always") or
        eql(u8, order, "byte-order-mark") or eql(u8, order, "host-unswapped");
}

const byte_order_mark: u16 = 0xFEFF;
const swapped_byte_order_mark: u16 = 0xFFFE;

// -- handles ---------------------------------------------------------------

/// A DHEADER waiting for its length; `at` null is the delimiter a
/// @final aggregate does not write.
pub const DHeader = struct {
    at: ?usize = null,
};

/// The DHEADER a value that takes no delimiter does not write; the
/// writer's `finishDheader` ignores it.
pub fn noDheader() DHeader {
    return .{};
}

/// An EMHEADER waiting for its NEXTINT; `next_int_at` is null where the
/// length code carries the length itself.
pub const EmHeader = struct {
    next_int_at: ?usize = null,
    word_at: usize = 0,
    /// The code the header was written with, so that `finishEmheader`
    /// can hold the member's value to the length the code declares.
    /// Codes 0 to 3 are lengths; anything else declares nothing of its
    /// own.
    length_code: u8 = 8,
};

/// A parameter header waiting for its length.
pub const Pid = struct {
    at: usize,
    extended: bool,
    saved_origin: usize,
    role: ParameterRole,
    pid: u32,
    must_understand: bool,
};

/// Everything a two-pass read has to put back.
pub const ReaderMark = struct {
    at: usize,
    origin: usize,
    status: Status,
};

/// An EMHEADER as read. `extent` is how many octets the member occupies
/// after the header word, the NEXTINT included. XTypes 1.3 §7.4.3.4.2.
pub const EmHeaderRead = struct {
    member_id: u32 = 0,
    length_code: u8 = 0,
    must_understand: bool = false,
    next_int: u32 = 0,
    has_next_int: bool = false,
    extent: usize = 0,
};

/// A parameter header as read.
pub const PidRead = struct {
    pid: u32 = 0,
    length: u32 = 0,
    end: bool = false,
    must_understand: bool = false,
    impl_extension: bool = false,
    reserved: bool = false,
    ignore: bool = false,
    role: ParameterRole = .member,
    saved_origin: usize = 0,
    value_start: usize = 0,

    /// Whether this parameter can be a member of the reading type:
    /// false for a vendor-private parameter and for a reserved id.
    /// §7.4.1.2.2, §7.2.2.4.4.4.6.
    pub fn isUserMember(self: PidRead) bool {
        return !self.impl_extension and !self.reserved and !self.ignore;
    }
};

// -- helpers the generated code names --------------------------------------

/// A pointer to a copy of `v`, out of `heap`: what an `@external`
/// member is assigned with — the indirection is the member's storage.
pub fn zdPtr(heap: std.mem.Allocator, v: anytype) WireError!*@TypeOf(v) {
    const p = try heap.create(@TypeOf(v));
    p.* = v;
    return p;
}

/// A mutable slice holding a copy of `values`, out of `heap`: what a
/// harness builds a `sequence` or `map` member from, since a Zig array
/// literal is const storage and the member is `[]T`.
pub fn zdSlice(heap: std.mem.Allocator, comptime T: type, values: []const T) WireError![]T {
    return heap.dupe(T, values);
}

/// C11 §7.24.4's unsigned-octet order, as the sign the walk compares.
/// Zig's byte order over UTF-8 is exactly that order.
pub fn strcmp(a: []const u8, b: []const u8) i32 {
    return switch (std.mem.order(u8, a, b)) {
        .lt => -1,
        .eq => 0,
        .gt => 1,
    };
}

/// `false` before `true` — the ascending-key order's reading of the
/// one-octet wire value. A seam of its own because Zig's `<` refuses a
/// pair of booleans.
pub fn boolLess(left: bool, right: bool) bool {
    return @intFromBool(left) < @intFromBool(right);
}

/// How many octets `text` is, which is what a string<N> bound counts.
pub fn utf8Octets(text: []const u8) usize {
    return text.len;
}

/// How many UTF-16 code units `text` is, which is what a wstring<N>
/// bound counts. A scalar above the basic plane is two.
pub fn utf16Units(text: []const u8) WireError!usize {
    var view = std.unicode.Utf8View.init(text) catch return WireError.OutOfRange;
    var it = view.iterator();
    var units: usize = 0;
    while (it.nextCodepoint()) |point| {
        units += if (point > 0xFFFF) @as(usize, 2) else 1;
    }
    return units;
}

/// The value's two's-complement bits in the low `octets`, zero-extended
/// — `writeUnionOverlay`'s pattern for an integer, character,
/// enumeration or bitmask arm. The mask is the function: a plain
/// widening of a negative value sign-extends, and `-2` in four octets
/// has to be 0x00000000FFFFFFFE.
pub fn bitPatternInt(value: i64, octets: i64) u64 {
    const pattern: u64 = @bitCast(value);
    if (octets >= 8) {
        return pattern;
    }
    const bits: u6 = @intCast(octets * 8);
    return pattern & ((@as(u64, 1) << bits) - 1);
}

/// One octet, 0 or 1 — the boolean overlay pattern.
pub fn bitPatternBool(value: bool) u64 {
    return @intFromBool(value);
}

/// The IEEE 754 binary32 bits, zero-extended.
pub fn bitPatternF32(value: f32) u64 {
    return @as(u32, @bitCast(value));
}

/// The IEEE 754 binary64 bits.
pub fn bitPatternF64(value: f64) u64 {
    return @bitCast(value);
}

/// A value the generated walk widens without seeing its type: the
/// declared enumeration constant an encoder writes, and the union arm
/// an overlay reads its storage bits through. A comptime `switch` on
/// the type answers for an enumeration, an integer of either sign, a
/// boolean and a character alike, so the emitter needs no per-site
/// spelling.
pub fn asI64(v: anytype) i64 {
    return switch (@typeInfo(@TypeOf(v))) {
        .@"enum" => @intFromEnum(v),
        .int => |info| if (info.signedness == .signed)
            @as(i64, @intCast(v))
        else
            @bitCast(@as(u64, v)),
        .bool => @intFromBool(v),
        .comptime_int => v,
        else => @compileError("asI64 takes an integer, enumeration or boolean"),
    };
}

/// The unsigned counterpart, for a bitfield's or bitmask's holder.
pub fn asU64(v: anytype) u64 {
    return switch (@typeInfo(@TypeOf(v))) {
        .@"enum" => @bitCast(@as(i64, @intFromEnum(v))),
        .int => |info| if (info.signedness == .signed)
            @bitCast(@as(i64, @intCast(v)))
        else
            @as(u64, v),
        .bool => @intFromBool(v),
        .comptime_int => v,
        else => @compileError("asU64 takes an integer, enumeration or boolean"),
    };
}

/// `point` as the one octet an IDL char is, refused above U+00FF rather
/// than truncated: the refusal is the sample's — the same one the
/// sibling runtimes raise.
pub fn checkedChar(point: u32) WireError!u8 {
    if (point > 0xFF) {
        return WireError.OutOfRange;
    }
    return @intCast(point);
}

/// `point` as the one UTF-16 code unit an IDL wchar is, refused outside
/// the BMP. §7.2.2.2.1.2.5.
pub fn checkedWchar(point: u32) WireError!u16 {
    if (point > 0xFFFF) {
        return WireError.OutOfRange;
    }
    return @intCast(point);
}

/// `text` cut to at most `octets` bytes, never inside a character.
/// Table 11: the order is preserved, so this is a prefix.
fn trimUtf8Text(text: []const u8, octets: usize) []const u8 {
    var end: usize = 0;
    while (end < text.len) {
        const size = std.unicode.utf8ByteSequenceLength(text[end]) catch break;
        if (end + size > octets) {
            break;
        }
        end += size;
    }
    return text[0..end];
}

/// `text` cut to at most `units` UTF-16 code units.
fn trimUtf16Text(text: []const u8, units: usize) []const u8 {
    var used: usize = 0;
    var end: usize = 0;
    while (end < text.len) {
        const size = std.unicode.utf8ByteSequenceLength(text[end]) catch break;
        if (end + size > text.len) {
            break;
        }
        const point = std.unicode.utf8Decode(text[end .. end + size]) catch break;
        const cost: usize = if (point > 0xFFFF) 2 else 1;
        if (used + cost > units) {
            break;
        }
        used += cost;
        end += size;
    }
    return text[0..end];
}

/// The UTF-16 code units of `v`, without a BOM, into `units`. Invalid
/// UTF-8 is refused: writing its replacement character would put a
/// string on the wire the caller never held.
/// A `std.ArrayList` holds no allocator of its own, so every function
/// that grows one is handed the allocator its caller owns.
fn utf16UnitsOf(heap: std.mem.Allocator, units: *std.ArrayList(u16), v: []const u8) WireError!void {
    var view = std.unicode.Utf8View.init(v) catch return WireError.OutOfRange;
    var it = view.iterator();
    while (it.nextCodepoint()) |point| {
        if (point < 0x10000) {
            try units.append(heap, @intCast(point));
        } else {
            const offset = point - 0x10000;
            try units.append(heap, @intCast(0xD800 + (offset >> 10)));
            try units.append(heap, @intCast(0xDC00 + (offset & 0x3FF)));
        }
    }
}

// -- the writer ------------------------------------------------------------

/// Accumulates the bytes of one sample. Alignment is relative to the
/// start of the stream: XCDR aligns each primitive to its own size
/// counted from the encapsulation. XTypes 1.3 §7.4.1.1.
pub const Writer = struct {
    out: std.ArrayList(u8),
    /// The allocator the accumulated bytes and every scratch list in the
    /// write direction come from. It is a field because a
    /// `std.ArrayList` no longer carries one.
    heap: std.mem.Allocator,
    endianness: Endianness,
    max_align: usize,
    origin: usize = 0,
    pl_cdr1: PlCdr1Framing,
    /// Whether an LC4 header whose member turns out to be 1, 2, 4 or 8
    /// bytes is rewritten with a short length code (Fast DDS's form).
    short_length_codes: bool,

    pub fn init(
        heap: std.mem.Allocator,
        endianness: Endianness,
        max_align: usize,
        pl_cdr1: PlCdr1Framing,
        short_length_codes: bool,
    ) Writer {
        return .{
            .out = .empty,
            .heap = heap,
            .endianness = endianness,
            .max_align = if (max_align < 1) 1 else max_align,
            .pl_cdr1 = pl_cdr1,
            .short_length_codes = short_length_codes,
        };
    }

    /// How many bytes have been written.
    pub fn position(self: *Writer) usize {
        return self.out.items.len;
    }

    /// The bytes written.
    pub fn finish(self: *Writer) []const u8 {
        return self.out.items;
    }

    fn alignTo(self: *Writer, alignment: usize) WireError!void {
        var effective = alignment;
        if (effective > self.max_align) {
            effective = self.max_align;
        }
        if (effective <= 1) {
            return;
        }
        const offset = self.out.items.len - self.origin;
        const padding = (effective - offset % effective) % effective;
        var i: usize = 0;
        while (i < padding) : (i += 1) {
            try self.out.append(self.heap, 0);
        }
    }

    fn writeRaw(self: *Writer, unsigned: u64, width: usize) WireError!void {
        var i: usize = 0;
        while (i < width) : (i += 1) {
            const shift: u6 = @intCast(8 * (if (self.endianness == .little) i else width - 1 - i));
            try self.out.append(self.heap, @truncate(unsigned >> shift));
        }
    }

    fn writeInt(self: *Writer, unsigned: u64, width: usize) WireError!void {
        try self.alignTo(width);
        try self.writeRaw(unsigned, width);
    }

    // -- primitives --------------------------------------------------------

    /// `boolean`: one byte, 0 or 1.
    pub fn writeBool(self: *Writer, v: bool) WireError!void {
        try self.out.append(self.heap, @intFromBool(v));
    }

    /// `octet` and `uint8`.
    pub fn writeU8(self: *Writer, v: u8) WireError!void {
        try self.out.append(self.heap, v);
    }

    /// `int8`.
    pub fn writeI8(self: *Writer, v: i8) WireError!void {
        try self.out.append(self.heap, @bitCast(v));
    }

    /// `char`: one octet — which is the whole member.
    pub fn writeChar(self: *Writer, v: u8) WireError!void {
        try self.out.append(self.heap, v);
    }

    /// `short`.
    pub fn writeI16(self: *Writer, v: i16) WireError!void {
        try self.writeInt(@as(u16, @bitCast(v)), 2);
    }

    /// `unsigned short`.
    pub fn writeU16(self: *Writer, v: u16) WireError!void {
        try self.writeInt(v, 2);
    }

    /// `long` — 32 bits.
    pub fn writeI32(self: *Writer, v: i32) WireError!void {
        try self.writeInt(@as(u32, @bitCast(v)), 4);
    }

    /// `unsigned long`.
    pub fn writeU32(self: *Writer, v: u32) WireError!void {
        try self.writeInt(v, 4);
    }

    /// `long long`.
    pub fn writeI64(self: *Writer, v: i64) WireError!void {
        try self.writeInt(@bitCast(v), 8);
    }

    /// `unsigned long long`.
    pub fn writeU64(self: *Writer, v: u64) WireError!void {
        try self.writeInt(v, 8);
    }

    /// `float`.
    pub fn writeF32(self: *Writer, v: f32) WireError!void {
        try self.writeInt(@as(u32, @bitCast(v)), 4);
    }

    /// `double`.
    pub fn writeF64(self: *Writer, v: f64) WireError!void {
        try self.writeInt(@as(u64, @bitCast(v)), 8);
    }

    /// `long double`: sixteen bytes of IEEE 754 binary128, aligned to
    /// eight (capped by MAXALIGN). The two byte orders are the whole
    /// sixteen bytes reversed — CORBA 3.3 part 2 §9.3.1.3.
    pub fn writeF128(self: *Writer, v: f128) WireError!void {
        const bits: u128 = @bitCast(v);
        try self.alignTo(8);
        var i: usize = 0;
        while (i < 16) : (i += 1) {
            const index = if (self.endianness == .little) i else 15 - i;
            const shift: u7 = @intCast(8 * index);
            try self.out.append(self.heap, @truncate(bits >> shift));
        }
    }

    // -- characters and strings --------------------------------------------

    /// `wchar`, at `width` octets and in `framing`'s shape.
    /// "giop-length-prefixed" is CORBA 3.3 part 2 §9.3.1.6's GIOP 1.2
    /// form: the octet count first, then the code unit's octets, all
    /// aligned to one; every order but "stream" means big-endian there.
    pub fn writeWchar(self: *Writer, v: u16, width: u16, framing: []const u8, order: []const u8) WireError!void {
        if (width != 2 and width != 4) {
            return WireError.OutOfRange;
        }
        if (std.mem.eql(u8, framing, "giop-length-prefixed")) {
            try self.out.append(self.heap, @intCast(width));
            const big = !std.mem.eql(u8, order, "stream") or self.endianness != .little;
            var i: usize = 0;
            while (i < width) : (i += 1) {
                const index = if (big) width - 1 - i else i;
                const shift: u5 = @intCast(8 * index);
                try self.out.append(self.heap, @truncate(@as(u32, v) >> shift));
            }
            return;
        }
        if (!std.mem.eql(u8, framing, "fixed")) {
            return WireError.OutOfRange;
        }
        if (width == 4) {
            try self.writeU32(v);
            return;
        }
        try self.writeU16(v);
    }

    /// A string: a four-byte length including the terminator, the UTF-8
    /// bytes, then a NUL. The length counts bytes. XTypes 1.3 §7.4.1.1.
    pub fn writeString(self: *Writer, v: []const u8) WireError!void {
        try self.writeU32(@intCast(v.len + 1));
        try self.out.appendSlice(self.heap, v);
        try self.out.append(self.heap, 0);
    }

    /// A bounded string: the same bytes `writeString` writes. The bound
    /// is a promise about the source and not a difference on the wire,
    /// so this is the only place it can be enforced — refused rather
    /// than cut at a character boundary, which would put a sample on
    /// the wire nobody wrote.
    pub fn writeStringBounded(self: *Writer, v: []const u8, bound: usize) WireError!void {
        if (v.len > bound) {
            return WireError.OutOfRange;
        }
        try self.writeString(v);
    }

    /// The count check every bounded sequence makes before a byte of it
    /// is written. Table 17 makes `O2.length <= T1.length` the
    /// condition for constructing a `sequence<T, N>`, and the wire
    /// carries no bound to compare against: a sender whose count
    /// already exceeds its own declaration has written a value of a
    /// type nobody declared.
    pub fn checkSequenceBound(self: *Writer, actual: u32, bound: u32) WireError!void {
        _ = self;
        if (actual > bound) {
            return WireError.OutOfRange;
        }
    }

    /// The count check every bounded map makes before a byte of it is
    /// written. Its own message and not `checkSequenceBound`'s: the
    /// unit is an entry and not an element, per XTypes 1.3 §7.4.1.1.4's
    /// map-as-a-sequence-of-pairs equivalence.
    pub fn checkMapBound(self: *Writer, actual: u32, bound: u32) WireError!void {
        _ = self;
        if (actual > bound) {
            return WireError.OutOfRange;
        }
    }

    /// A wstring, in the form `policy` and `order` name. The value
    /// arrives as UTF-8 and the conversion to UTF-16 units happens
    /// here, which is what makes §7.4.1.1.2's surrogate rule this
    /// runtime's own reading.
    pub fn writeWstringUtf8(self: *Writer, v: []const u8, policy: []const u8, order: []const u8) WireError!void {
        const cell = try wstringCellOf(policy);
        if (!wstringOrderKnown(order)) {
            return WireError.OutOfRange;
        }
        var units: std.ArrayList(u16) = .empty;
        defer units.deinit(self.heap);
        try utf16UnitsOf(self.heap, &units, v);
        if (cell.counts_characters and cell.width == 2) {
            // A character count over two-octet units has no way to
            // carry a code point outside the BMP.
            for (units.items) |unit| {
                if (unit >= 0xD800 and unit <= 0xDFFF) {
                    return WireError.OutOfRange;
                }
            }
        }
        if (cell.terminated) {
            try units.append(self.heap, 0);
        }
        // An empty wstring carries no mark.
        if (std.mem.eql(u8, order, "byte-order-mark") and units.items.len > 0) {
            try units.insert(self.heap, 0, byte_order_mark);
        }
        if (cell.counts_characters) {
            try self.writeU32(@intCast(units.items.len));
        } else {
            try self.writeU32(@intCast(units.items.len * cell.width));
        }
        for (units.items) |unit| {
            try self.writeWstringUnit(unit, cell.width, order);
        }
    }

    /// A bounded wstring, as `writeStringBounded` is to `writeString`.
    /// The bound counts UTF-16 code units, not characters and not
    /// octets, whatever `policy` puts on the wire.
    pub fn writeWstringBounded(
        self: *Writer,
        v: []const u8,
        bound: usize,
        policy: []const u8,
        order: []const u8,
    ) WireError!void {
        const units = try utf16Units(v);
        if (units > bound) {
            return WireError.OutOfRange;
        }
        try self.writeWstringUtf8(v, policy, order);
    }

    /// A wstring whose characters go out as their own UTF-8 octets.
    ///
    /// `WstringCodeUnits::Utf8` is the measured departure of one
    /// product's generated serialiser: XTypes 1.3 Table 31 gives a
    /// wstring's code unit as Char16 and §7.4.1.1.2 makes the length
    /// "twice the number of characters", so the clause has no UTF-8 form
    /// at all. The declared length is the UTF-8 octet count plus the
    /// terminator the policy asks for. wstring-member is
    /// `04 00 00 00 61 62 63 00` for "abc": three octets and a NUL, and
    /// the four declares both.
    pub fn writeWstringUtf8Octets(self: *Writer, v: []const u8, policy: []const u8) WireError!void {
        const cell = try wstringCellOf(policy);
        // The same malformed-UTF-8 refusal the UTF-16 writer makes.
        if (!std.unicode.utf8ValidateSlice(v)) {
            return WireError.NotUtf8;
        }
        const length = v.len + (if (cell.terminated) @as(usize, 1) else 0);
        if (length > std.math.maxInt(u32)) {
            return WireError.OutOfRange;
        }
        try self.writeU32(@intCast(length));
        try self.out.appendSlice(self.heap, v);
        if (cell.terminated) {
            try self.out.append(self.heap, 0);
        }
    }

    /// A bounded UTF-8-octet wstring, as `writeWstringBounded` is to
    /// `writeWstringUtf8`: the bound still counts UTF-16 code units,
    /// which is what a wstring<N> bound means to every peer, while the
    /// wire carries the UTF-8 octets.
    pub fn writeWstringUtf8OctetsBounded(
        self: *Writer,
        v: []const u8,
        bound: usize,
        policy: []const u8,
    ) WireError!void {
        const units = try utf16Units(v);
        if (units > bound) {
            return WireError.OutOfRange;
        }
        try self.writeWstringUtf8Octets(v, policy);
    }

    fn writeWstringUnit(self: *Writer, unit: u16, width: u16, order: []const u8) WireError!void {
        const little = std.mem.eql(u8, order, "byte-order-mark") or
            std.mem.eql(u8, order, "host-unswapped");
        if (width == 4) {
            if (std.mem.eql(u8, order, "big-endian-always")) {
                try self.writeU32Big(unit);
            } else if (little) {
                try self.writeU32Little(unit);
            } else {
                try self.writeU32(unit);
            }
            return;
        }
        if (std.mem.eql(u8, order, "big-endian-always")) {
            try self.writeU16Big(unit);
        } else if (little) {
            try self.writeU16Little(unit);
        } else {
            try self.writeU16(unit);
        }
    }

    /// A 16-bit value little-endian whatever the stream's order is.
    pub fn writeU16Little(self: *Writer, v: u16) WireError!void {
        try self.alignTo(2);
        try self.out.append(self.heap, @truncate(v));
        try self.out.append(self.heap, @truncate(v >> 8));
    }

    /// A 16-bit value big-endian whatever the stream's order is.
    pub fn writeU16Big(self: *Writer, v: u16) WireError!void {
        try self.alignTo(2);
        try self.out.append(self.heap, @truncate(v >> 8));
        try self.out.append(self.heap, @truncate(v));
    }

    /// A 32-bit value little-endian whatever the stream's order is.
    pub fn writeU32Little(self: *Writer, v: u32) WireError!void {
        try self.alignTo(4);
        var i: u5 = 0;
        while (i < 4) : (i += 1) {
            try self.out.append(self.heap, @truncate(v >> (8 * @as(u5, i))));
        }
    }

    /// A 32-bit value big-endian whatever the stream's order is.
    pub fn writeU32Big(self: *Writer, v: u32) WireError!void {
        try self.alignTo(4);
        var i: usize = 0;
        while (i < 4) : (i += 1) {
            const shift: u5 = @intCast(8 * (3 - i));
            try self.out.append(self.heap, @truncate(v >> shift));
        }
    }

    // -- fixed -------------------------------------------------------------

    /// `fixed<digits, scale>` as packed BCD, [CDR] §9.3.2.8 — the only
    /// wire definition an IDL fixed has. `count` is "declared",
    /// "value-precision" or "value-string-digits".
    pub fn writeFixed(self: *Writer, v: []const u8, digits: u16, scale: u16, count: []const u8) WireError!void {
        var nibbles: std.ArrayList(u8) = .empty;
        defer nibbles.deinit(self.heap);
        try fixedNibbles(self.heap, &nibbles, v, digits, scale, count);
        var i: usize = 0;
        while (i < nibbles.items.len) : (i += 2) {
            try self.out.append(self.heap, (nibbles.items[i] << 4) | nibbles.items[i + 1]);
        }
    }

    // -- the bit-bounded kinds ---------------------------------------------

    /// An enumerator, as the integer its bit bound calls for. The
    /// holder is signed — §7.4.3.5.3 rule (5) — and what fits is
    /// everything expressible in the bound's bits: -2^(n-1) ..= 2^n - 1.
    pub fn writeEnum(self: *Writer, bit_bound: u16, value: i64) WireError!void {
        if (bit_bound == 0 or bit_bound > 32) {
            return WireError.OutOfRange;
        }
        const bits: u6 = @intCast(bit_bound);
        const low = -(@as(i64, 1) << (bits - 1));
        const high = (@as(i64, 1) << bits) - 1;
        if (value < low or value > high) {
            return WireError.OutOfRange;
        }
        const mask = (@as(u64, 1) << bits) - 1;
        const pattern = @as(u64, @bitCast(value)) & mask;
        if (bit_bound <= 8) {
            try self.writeU8(@truncate(pattern));
        } else if (bit_bound <= 16) {
            try self.writeU16(@truncate(pattern));
        } else {
            try self.writeU32(@truncate(pattern));
        }
    }

    /// A bitmask, as the unsigned integer Table 33 gives its bound.
    pub fn writeBitmask(self: *Writer, bit_bound: u16, value: u64) WireError!void {
        if (bit_bound == 0 or bit_bound > 64) {
            return WireError.OutOfRange;
        }
        if (bit_bound < 64) {
            const bits: u6 = @intCast(bit_bound);
            if (value >= @as(u64, 1) << bits) {
                return WireError.OutOfRange;
            }
        }
        if (bit_bound <= 8) {
            try self.writeU8(@truncate(value));
        } else if (bit_bound <= 16) {
            try self.writeU16(@truncate(value));
        } else if (bit_bound <= 32) {
            try self.writeU32(@truncate(value));
        } else {
            try self.writeU64(value);
        }
    }

    /// One bitfield, shifted into the accumulator a bitset packs into.
    /// A value wider than the field is refused rather than masked.
    pub fn bitfield(self: *Writer, value: u64, width: u16, pos: u16) WireError!u64 {
        _ = self;
        if (width == 0 or width > 64 or pos + width > 64) {
            return WireError.OutOfRange;
        }
        if (width < 64) {
            const bits: u6 = @intCast(width);
            if (value > (@as(u64, 1) << bits) - 1) {
                return WireError.OutOfRange;
            }
        }
        return value << @as(u6, @intCast(pos));
    }

    /// The Cyclone union overlay: the low `octets` of `pattern`,
    /// written as one unsigned integer of that width in the stream's
    /// byte order. A discriminator carrying a label the `union-labels`
    /// dialect axis says selects nothing makes the peer fall through to
    /// `default:` and serialise its one C union storage as that arm —
    /// the selected member's octets, reinterpreted. `bitPatternInt` and
    /// kin produce the pattern.
    pub fn writeUnionOverlay(self: *Writer, pattern: u64, octets: u16) WireError!void {
        switch (octets) {
            1 => try self.writeU8(@truncate(pattern)),
            2 => try self.writeU16(@truncate(pattern)),
            4 => try self.writeU32(@truncate(pattern)),
            8 => try self.writeU64(pattern),
            else => return WireError.OutOfRange,
        }
    }

    /// A bitset, as the anchor integer its bitfields pack into; the
    /// anchor's width comes from the layout planner.
    pub fn writeBitset(self: *Writer, anchor: u16, value: u64) WireError!void {
        switch (anchor) {
            1 => try self.writeU8(@truncate(value)),
            2 => try self.writeU16(@truncate(value)),
            4 => try self.writeU32(@truncate(value)),
            8 => try self.writeU64(value),
            else => return WireError.OutOfRange,
        }
    }

    /// Move the alignment origin to the current position and answer the
    /// origin displaced, for `finishPackedElement`.
    ///
    /// `ElementPadding::Packed` is HDDS's measured departure from
    /// §7.4.1, which counts every element's alignment from the
    /// encapsulation: each collection element and each aggregate value
    /// counts its own alignment from its own first octet instead.
    /// array-1d-struct-final is
    /// `01000000 0200 03000000 0400 77`, the second Cell's long at
    /// offset six where the clause puts it at eight.
    pub fn beginPackedElement(self: *Writer) usize {
        const saved = self.origin;
        self.origin = self.out.items.len;
        return saved;
    }

    /// Restore the origin `beginPackedElement` displaced.
    pub fn finishPackedElement(self: *Writer, saved: usize) void {
        self.origin = saved;
    }

    // -- framing -----------------------------------------------------------

    /// A DHEADER: a four-byte length placeholder, patched when the
    /// members are done.
    pub fn writeDheader(self: *Writer) WireError!DHeader {
        try self.alignTo(4);
        const at = self.out.items.len;
        try self.out.appendSlice(self.heap, &[_]u8{ 0, 0, 0, 0 });
        return .{ .at = at };
    }

    /// Patch a DHEADER with the length of what followed it; a header
    /// from `noDheader` is not there and is ignored.
    pub fn finishDheader(self: *Writer, header: DHeader) WireError!void {
        const at = header.at orelse return;
        try self.patch(at, @intCast(self.out.items.len - at - 4), 4);
    }

    /// An XCDR2 EMHEADER: id, length code and must-understand in one
    /// 32-bit word. §7.4.3.4.2. LC4 reserves a NEXTINT; LC5 to LC7
    /// reuse the member's own leading word.
    pub fn writeEmheader(self: *Writer, member_id: u32, length_code: u8, must_understand: bool) WireError!EmHeader {
        // §7.2.2.4.4.4.4 runs member ids from 0 to 0x0FFFFFFF and
        // §7.4.3.4.2 leaves them the low 28 bits of the word. Masking a
        // larger one writes a well-formed header for `id & 0x0FFFFFFF`:
        // the peer reads a member the caller did not name, and nothing at
        // either end says so.
        if (member_id > e_member_id_mask) {
            return WireError.OutOfRange;
        }
        try self.alignTo(4);
        var word: u32 = (@as(u32, length_code & 0x7) << 28) | (member_id & e_member_id_mask);
        if (must_understand) {
            word |= 0x8000_0000;
        }
        try self.writeU32(word);
        const word_at = self.out.items.len - 4;
        if (length_code == 4) {
            const at = self.out.items.len;
            try self.out.appendSlice(self.heap, &[_]u8{ 0, 0, 0, 0 });
            return .{ .next_int_at = at, .word_at = word_at, .length_code = length_code };
        }
        return .{ .next_int_at = null, .word_at = word_at, .length_code = length_code };
    }

    /// Patch an EMHEADER's NEXTINT; under `short_length_codes` an LC4
    /// header whose member is 1, 2, 4 or 8 bytes loses its NEXTINT and
    /// states the length in the code.
    pub fn finishEmheader(self: *Writer, header: EmHeader) WireError!void {
        const next_int_at = header.next_int_at orelse {
            // Under codes 0 to 3 the code *is* the length — 1, 2, 4 or 8
            // octets, §7.4.3.4.2. A value of another size misdeclares it
            // and the reader's skip lands inside data, so the mismatch is
            // refused here rather than written. Codes 5 to 7 declare
            // nothing of their own: the member's leading length word is
            // the NEXTINT.
            if (header.length_code <= 3 and header.word_at + 4 <= self.out.items.len) {
                const declared = @as(usize, 1) << @intCast(header.length_code);
                const body = self.out.items.len - header.word_at - 4;
                if (body != declared) {
                    return WireError.OutOfRange;
                }
            }
            return;
        };
        const length = self.out.items.len - next_int_at - 4;
        if (self.short_length_codes and
            (length == 1 or length == 2 or length == 4 or length == 8))
        {
            try self.shortenEmheader(header, length);
            return;
        }
        try self.patch(next_int_at, @intCast(length), 4);
    }

    fn shortenEmheader(self: *Writer, header: EmHeader, length: usize) WireError!void {
        const code: u32 = switch (length) {
            1 => 0,
            2 => 1,
            4 => 2,
            else => 3,
        };
        const next_int_at = header.next_int_at orelse return;
        // Take the four NEXTINT octets out; the member slides down.
        var i: usize = next_int_at;
        while (i + 4 < self.out.items.len) : (i += 1) {
            self.out.items[i] = self.out.items[i + 4];
        }
        self.out.items.len -= 4;
        const previous = self.readPatched(header.word_at);
        try self.patch(header.word_at, (previous & ~(@as(u32, 0x7) << 28)) | (code << 28), 4);
    }

    fn readPatched(self: *Writer, at: usize) u32 {
        var value: u32 = 0;
        var i: usize = 0;
        while (i < 4) : (i += 1) {
            const index = if (self.endianness == .little) at + 3 - i else at + i;
            value = (value << 8) | self.out.items[index];
        }
        return value;
    }

    /// A PL_CDR1 parameter header, short form: [id:u16][length:u16].
    pub fn writePid(self: *Writer, id: u32, role: ParameterRole, must_understand: bool) WireError!Pid {
        // §7.4.1.2.2 gives the short form the ids 0 to `highest_short_id`
        // inclusive and reserves what is above it — PID_EXTENDED is 0x3F01
        // itself. A short header with a reserved id collides with the
        // protocol's own parameters, so such an id takes the extended form
        // or none. This is the write side of the constant the reader has
        // always consulted.
        if (id > self.pl_cdr1.highest_short_id) {
            return WireError.OutOfRange;
        }
        try self.alignTo(4);
        var pid = id;
        if (must_understand and self.pl_cdr1.member_must_understand) {
            pid |= pid_must_understand;
        }
        pid |= try pidFlagsAt(self.pl_cdr1.pid_flags, .member);
        const bare = pid & ~(pid_must_understand | pid_impl_extension);
        try self.writeU16(@truncate(pid));
        const at = self.out.items.len;
        try self.out.appendSlice(self.heap, &[_]u8{ 0, 0 });
        return .{
            .at = at,
            .extended = false,
            .saved_origin = self.beginParameterValue(),
            .role = role,
            .pid = bare,
            .must_understand = must_understand,
        };
    }

    /// A PL_CDR1 parameter header, extended form:
    /// [PID_EXTENDED][8][id:u32][length:u32]. §7.4.1.2. The member's
    /// must-understand is FLAG_2 of the eMemberHeader.
    pub fn writePidExtended(self: *Writer, id: u32, role: ParameterRole, must_understand: bool) WireError!Pid {
        // The same 28 bits as an EMHEADER's, here in the eMemberHeader of
        // §7.4.1.2.1. Masking aliases the parameter onto another member
        // exactly as it does there.
        if (id > e_member_id_mask) {
            return WireError.OutOfRange;
        }
        try self.alignTo(4);
        const marker = pid_extended | try pidFlagsAt(self.pl_cdr1.pid_flags, .extended_marker);
        try self.writeU16(@truncate(marker));
        try self.writeU16(8);
        var member_header: u32 = id & e_member_id_mask;
        if (must_understand and self.pl_cdr1.member_must_understand) {
            member_header |= e_flag_must_understand;
        }
        try self.writeU32(member_header);
        const at = self.out.items.len;
        try self.out.appendSlice(self.heap, &[_]u8{ 0, 0, 0, 0 });
        return .{
            .at = at,
            .extended = true,
            .saved_origin = self.beginParameterValue(),
            .role = role,
            .pid = 0,
            .must_understand = false,
        };
    }

    fn beginParameterValue(self: *Writer) usize {
        const saved = self.origin;
        self.origin = self.out.items.len;
        return saved;
    }

    /// Patch a parameter header with its value's length; the padding to
    /// four goes outside the declared length or inside it, by framing.
    pub fn finishPid(self: *Writer, header: Pid) WireError!void {
        const width: usize = if (header.extended) 4 else 2;
        var length = self.out.items.len - header.at - width;
        if (self.pl_cdr1.padsDeclaredLength(header.role)) {
            const past = (self.out.items.len - header.saved_origin) % 4;
            if (past != 0) {
                length += 4 - past;
            }
        }
        if (!header.extended and length > pid_short_max_len) {
            try self.promotePid(header, length);
        } else {
            try self.patch(header.at, @intCast(length), width);
        }
        self.origin = header.saved_origin;
        // Under Fast DDS's framing the parameter writes no padding of
        // its own — the next header aligns itself, and a final
        // parameter ends the stream where its value does.
        if (self.pl_cdr1.pad_after_value) {
            try self.alignTo(4);
        }
    }

    fn promotePid(self: *Writer, header: Pid, length: usize) WireError!void {
        const start = header.at - 2;
        // Eight fresh octets after the short header's length field; the
        // value slides up.
        const old_len = self.out.items.len;
        try self.out.appendSlice(self.heap, &[_]u8{ 0, 0, 0, 0, 0, 0, 0, 0 });
        var i: usize = self.out.items.len;
        while (i > header.at + 2 + 8) {
            i -= 1;
            self.out.items[i] = self.out.items[i - 8];
        }
        _ = old_len;
        const marker = pid_extended | try pidFlagsAt(self.pl_cdr1.pid_flags, .extended_marker);
        try self.patch(start, marker, 2);
        try self.patch(start + 2, 8, 2);
        var member_header: u32 = header.pid & e_member_id_mask;
        if (header.must_understand and self.pl_cdr1.member_must_understand) {
            member_header |= e_flag_must_understand;
        }
        try self.patch(start + 4, member_header, 4);
        try self.patch(start + 8, @intCast(length), 4);
    }

    /// The PID_LIST_END sentinel a PL_CDR1 parameter list ends with;
    /// its flags come from the framing asked at the "sentinel"
    /// position.
    pub fn writeSentinel(self: *Writer) WireError!void {
        try self.alignTo(4);
        const sentinel = pid_list_end | try pidFlagsAt(self.pl_cdr1.pid_flags, .sentinel);
        try self.writeU16(@truncate(sentinel));
        try self.writeU16(0);
    }

    fn patch(self: *Writer, at: usize, value: u64, width: usize) WireError!void {
        if (width < 8) {
            const bits: u6 = @intCast(width * 8);
            if (value >= @as(u64, 1) << bits) {
                return WireError.OutOfRange;
            }
        }
        var i: usize = 0;
        while (i < width) : (i += 1) {
            const shift: u6 = @intCast(8 * i);
            const index = if (self.endianness == .little) at + i else at + width - 1 - i;
            self.out.items[index] = @truncate(value >> shift);
        }
    }
};

// -- the reader ------------------------------------------------------------

/// Reads one sample's bytes: the mirror of the writer clause for
/// clause. A malformed stream fails; a refusal about the type sets the
/// status and the generated decoder walks on.
pub const Reader = struct {
    data: []const u8,
    at: usize = 0,
    endianness: Endianness,
    max_align: usize,
    origin: usize = 0,
    pid_flags: []const u8,
    highest_short_id: u32,
    status: Status = .ok,
    /// The arena everything the read direction materialises comes
    /// from: strings, sequences, `@external` pointers, Table 9's boxed
    /// defaults.
    heap: std.mem.Allocator,

    pub fn init(
        heap: std.mem.Allocator,
        data: []const u8,
        endianness: Endianness,
        max_align: usize,
        pid_flags: []const u8,
        highest_short_id: u32,
    ) Reader {
        return .{
            .data = data,
            .endianness = endianness,
            .max_align = if (max_align < 1) 1 else max_align,
            .pid_flags = pid_flags,
            .highest_short_id = highest_short_id,
            .heap = heap,
        };
    }

    // -- the status a generated decoder reads ------------------------------

    /// How the read is going.
    pub fn readerStatus(self: *Reader) Status {
        return self.status;
    }

    /// Refuse the stream from outside the runtime; the first refusal
    /// sticks, and `.ok` refuses nothing.
    pub fn readerRefuse(self: *Reader, refusal: Status) void {
        if (refusal != .ok and self.status == .ok) {
            self.status = refusal;
        }
    }

    /// Take back a `.discarded`, and only that one: a truncated stream
    /// is not a narrowing.
    pub fn readerClearDiscard(self: *Reader) bool {
        if (self.status == .discarded) {
            self.status = .ok;
            return true;
        }
        return false;
    }

    /// Where the reader is.
    pub fn readerPosition(self: *Reader) usize {
        return self.at;
    }

    /// How many bytes are left.
    pub fn readerRemaining(self: *Reader) usize {
        if (self.at > self.data.len) {
            return 0;
        }
        return self.data.len - self.at;
    }

    /// Move the cursor forward to `target`, never backwards: a framing
    /// word pointing behind the cursor is a refusal about the stream's
    /// shape, and past the end fails.
    pub fn readerSkipTo(self: *Reader, target: usize) WireError!void {
        if (self.status != .ok) {
            return;
        }
        if (target < self.at) {
            self.readerRefuse(.out_of_range);
            return;
        }
        if (target > self.data.len) {
            return WireError.Truncated;
        }
        self.at = target;
    }

    /// The cursor, the origin and the status, to be put back later.
    pub fn readerSave(self: *Reader) ReaderMark {
        return .{ .at = self.at, .origin = self.origin, .status = self.status };
    }

    /// Put back what `readerSave` took.
    pub fn readerRestore(self: *Reader, mark: ReaderMark) void {
        self.at = mark.at;
        self.origin = mark.origin;
        self.status = mark.status;
    }

    /// Move the alignment origin to the current position and answer the
    /// origin displaced, for `readerFinishPackedElement`. The read-side
    /// half of `beginPackedElement`; see it for the axis.
    pub fn readerBeginPackedElement(self: *Reader) usize {
        const saved = self.origin;
        self.origin = self.at;
        return saved;
    }

    /// Restore the origin `readerBeginPackedElement` displaced.
    pub fn readerFinishPackedElement(self: *Reader, saved: usize) void {
        self.origin = saved;
    }

    // -- the cursor --------------------------------------------------------

    fn alignTo(self: *Reader, alignment: usize) void {
        var effective = alignment;
        if (effective > self.max_align) {
            effective = self.max_align;
        }
        if (effective <= 1) {
            return;
        }
        const offset = self.at - self.origin;
        self.at += (effective - offset % effective) % effective;
    }

    fn take(self: *Reader, count: usize) WireError![]const u8 {
        if (self.at > self.data.len or count > self.data.len - self.at) {
            return WireError.Truncated;
        }
        const chunk = self.data[self.at .. self.at + count];
        self.at += count;
        return chunk;
    }

    fn readUnsigned(self: *Reader, width: usize) WireError!u64 {
        self.alignTo(width);
        const raw = try self.take(width);
        var value: u64 = 0;
        var i: usize = 0;
        while (i < width) : (i += 1) {
            const index = if (self.endianness == .little) width - 1 - i else i;
            value = (value << 8) | raw[index];
        }
        return value;
    }

    // -- primitives --------------------------------------------------------

    /// `boolean`; a byte that is neither 0 nor 1 is refused rather than
    /// coerced.
    pub fn readBool(self: *Reader) WireError!bool {
        const value = (try self.take(1))[0];
        return switch (value) {
            0 => false,
            1 => true,
            else => WireError.OutOfRange,
        };
    }

    /// `octet` and `uint8`.
    pub fn readU8(self: *Reader) WireError!u8 {
        return (try self.take(1))[0];
    }

    /// `int8`.
    pub fn readI8(self: *Reader) WireError!i8 {
        return @bitCast((try self.take(1))[0]);
    }

    /// `char`: one octet.
    pub fn readChar(self: *Reader) WireError!u8 {
        return (try self.take(1))[0];
    }

    /// `short`.
    pub fn readI16(self: *Reader) WireError!i16 {
        return @bitCast(@as(u16, @truncate(try self.readUnsigned(2))));
    }

    /// `unsigned short`.
    pub fn readU16(self: *Reader) WireError!u16 {
        return @truncate(try self.readUnsigned(2));
    }

    /// `long`.
    pub fn readI32(self: *Reader) WireError!i32 {
        return @bitCast(@as(u32, @truncate(try self.readUnsigned(4))));
    }

    /// `unsigned long`.
    pub fn readU32(self: *Reader) WireError!u32 {
        return @truncate(try self.readUnsigned(4));
    }

    /// `long long`.
    pub fn readI64(self: *Reader) WireError!i64 {
        return @bitCast(try self.readUnsigned(8));
    }

    /// `unsigned long long`.
    pub fn readU64(self: *Reader) WireError!u64 {
        return try self.readUnsigned(8);
    }

    /// `float`.
    pub fn readF32(self: *Reader) WireError!f32 {
        return @bitCast(@as(u32, @truncate(try self.readUnsigned(4))));
    }

    /// `double`.
    pub fn readF64(self: *Reader) WireError!f64 {
        return @bitCast(try self.readUnsigned(8));
    }

    /// `long double`: all sixteen bytes, as the binary128 the member
    /// holds.
    pub fn readF128(self: *Reader) WireError!f128 {
        self.alignTo(8);
        const raw = try self.take(16);
        var bits: u128 = 0;
        var i: usize = 0;
        while (i < 16) : (i += 1) {
            const index = if (self.endianness == .little) 15 - i else i;
            bits = (bits << 8) | raw[index];
        }
        return @bitCast(bits);
    }

    /// `wchar`, at `width` octets and in `framing`'s shape. The
    /// four-byte form zero-extends one UTF-16 code unit; a value that
    /// uses the high bits, and a lone surrogate, are refused.
    pub fn readWchar(self: *Reader, width: u16, framing: []const u8, order: []const u8) WireError!u16 {
        if (width != 2 and width != 4) {
            return WireError.OutOfRange;
        }
        var value: u32 = 0;
        if (std.mem.eql(u8, framing, "giop-length-prefixed")) {
            const declared = try self.readU8();
            if (declared != width) {
                return WireError.OutOfRange;
            }
            const big = !std.mem.eql(u8, order, "stream") or self.endianness != .little;
            const raw = try self.take(width);
            var i: usize = 0;
            while (i < width) : (i += 1) {
                const index = if (big) i else width - 1 - i;
                value = (value << 8) | raw[index];
            }
        } else if (!std.mem.eql(u8, framing, "fixed")) {
            return WireError.OutOfRange;
        } else if (width == 4) {
            value = try self.readU32();
            if (value > 0xFFFF) {
                return WireError.OutOfRange;
            }
        } else {
            value = try self.readU16();
        }
        if (value >= 0xD800 and value <= 0xDFFF) {
            return WireError.OutOfRange;
        }
        return @intCast(value);
    }

    // -- strings -----------------------------------------------------------

    /// A string: a length including the terminator, the bytes, the NUL.
    /// The answer aliases the stream, which outlives the sample here;
    /// nothing is copied.
    pub fn readString(self: *Reader) WireError![]const u8 {
        const length = try self.readU32();
        if (length == 0 or length > self.readerRemaining() + 1) {
            return WireError.ImpossibleLength;
        }
        const text = try self.take(length - 1);
        if (!std.unicode.utf8ValidateSlice(text)) {
            return WireError.NotUtf8;
        }
        _ = try self.readU8();
        return text;
    }

    /// `readString` under the name the shared walk calls it by: a Zig
    /// slice into the stream needs no allocation, so the two names are
    /// one method.
    pub fn readStringAlloc(self: *Reader) WireError![]const u8 {
        return self.readString();
    }

    /// A 16-bit value little-endian whatever the stream's order is.
    pub fn readU16Little(self: *Reader) WireError!u16 {
        self.alignTo(2);
        const raw = try self.take(2);
        return @as(u16, raw[0]) | (@as(u16, raw[1]) << 8);
    }

    /// A 16-bit value big-endian whatever the stream's order is.
    pub fn readU16Big(self: *Reader) WireError!u16 {
        self.alignTo(2);
        const raw = try self.take(2);
        return (@as(u16, raw[0]) << 8) | raw[1];
    }

    /// A 32-bit value little-endian whatever the stream's order is.
    pub fn readU32Little(self: *Reader) WireError!u32 {
        self.alignTo(4);
        const raw = try self.take(4);
        return @as(u32, raw[0]) | (@as(u32, raw[1]) << 8) |
            (@as(u32, raw[2]) << 16) | (@as(u32, raw[3]) << 24);
    }

    /// A 32-bit value big-endian whatever the stream's order is.
    pub fn readU32Big(self: *Reader) WireError!u32 {
        self.alignTo(4);
        const raw = try self.take(4);
        return (@as(u32, raw[0]) << 24) | (@as(u32, raw[1]) << 16) |
            (@as(u32, raw[2]) << 8) | raw[3];
    }

    /// A wstring, in the form `policy` and `order` name, answered as
    /// UTF-8 out of the reader's arena — which is what the generated
    /// member holds. The form is never inferred from the stream.
    pub fn readWstringUtf8(self: *Reader, policy: []const u8, order: []const u8) WireError![]const u8 {
        const declared = try self.readU32();
        if (!wstringOrderKnown(order)) {
            return WireError.OutOfRange;
        }
        const cell = try wstringCellOf(policy);
        var count: usize = declared;
        if (!cell.counts_characters) {
            if (declared % cell.width != 0) {
                return WireError.ImpossibleLength;
            }
            count = declared / cell.width;
        }
        var units: std.ArrayList(u16) = .empty;
        defer units.deinit(self.heap);
        try self.wstringUnits(&units, count, cell.width, order);
        var items: []const u16 = units.items;
        if (cell.terminated) {
            if (declared < 1 or items.len == 0) {
                return WireError.ImpossibleLength;
            }
            const last = items[items.len - 1];
            items = items[0 .. items.len - 1];
            if (last != 0) {
                return WireError.OutOfRange;
            }
            for (items) |unit| {
                if (unit == 0) {
                    return WireError.OutOfRange;
                }
            }
        }
        var swapped: std.ArrayList(u16) = .empty;
        defer swapped.deinit(self.heap);
        if (std.mem.eql(u8, order, "byte-order-mark")) {
            items = try stripByteOrderMark(self.heap, &swapped, items);
        }
        var built: std.ArrayList(u8) = .empty;
        errdefer built.deinit(self.heap);
        var index: usize = 0;
        while (index < items.len) {
            const unit = items[index];
            var point: u21 = unit;
            if (unit >= 0xD800 and unit <= 0xDBFF and index + 1 < items.len) {
                const low = items[index + 1];
                if (low >= 0xDC00 and low <= 0xDFFF) {
                    point = @intCast(0x10000 +
                        ((@as(u32, unit) - 0xD800) << 10) + (@as(u32, low) - 0xDC00));
                    index += 2;
                    var buffer: [4]u8 = undefined;
                    const size = std.unicode.utf8Encode(point, &buffer) catch
                        return WireError.OutOfRange;
                    try built.appendSlice(self.heap, buffer[0..size]);
                    continue;
                }
            }
            if (unit >= 0xD800 and unit <= 0xDFFF) {
                return WireError.OutOfRange;
            }
            var buffer: [4]u8 = undefined;
            const size = std.unicode.utf8Encode(point, &buffer) catch
                return WireError.OutOfRange;
            try built.appendSlice(self.heap, buffer[0..size]);
            index += 1;
        }
        return try built.toOwnedSlice(self.heap);
    }

    /// The inverse of `writeWstringUtf8Octets`: the declared length is
    /// the UTF-8 octet count, the terminator the policy asks for is the
    /// last of those octets, and a count the stream cannot hold, a
    /// missing or an interior NUL, or an ill-formed UTF-8 sequence is
    /// refused. See `WstringCodeUnits::Utf8`.
    pub fn readWstringUtf8Octets(self: *Reader, policy: []const u8) WireError![]const u8 {
        const declared = try self.readU32();
        const cell = try wstringCellOf(policy);
        if (declared > self.readerRemaining()) {
            return WireError.ImpossibleLength;
        }
        var data = try self.take(declared);
        if (cell.terminated) {
            if (declared < 1 or data[data.len - 1] != 0) {
                return WireError.OutOfRange;
            }
            data = data[0 .. data.len - 1];
            // A NUL inside the text is the narrow string's problem in
            // wide form: every C peer stops at it.
            for (data) |octet| {
                if (octet == 0) {
                    return WireError.OutOfRange;
                }
            }
        }
        if (!std.unicode.utf8ValidateSlice(data)) {
            return WireError.NotUtf8;
        }
        return data;
    }

    fn wstringUnits(self: *Reader, units: *std.ArrayList(u16), count: usize, width: u16, order: []const u8) WireError!void {
        if (count * width > self.readerRemaining()) {
            return WireError.ImpossibleLength;
        }
        const little = std.mem.eql(u8, order, "byte-order-mark") or
            std.mem.eql(u8, order, "host-unswapped");
        var i: usize = 0;
        while (i < count) : (i += 1) {
            if (width == 4) {
                const wide = if (std.mem.eql(u8, order, "big-endian-always"))
                    try self.readU32Big()
                else if (little)
                    try self.readU32Little()
                else
                    try self.readU32();
                if (wide > 0xFFFF) {
                    return WireError.OutOfRange;
                }
                try units.append(self.heap, @intCast(wide));
                continue;
            }
            const unit = if (std.mem.eql(u8, order, "big-endian-always"))
                try self.readU16Big()
            else if (little)
                try self.readU16Little()
            else
                try self.readU16();
            try units.append(self.heap, unit);
        }
    }

    /// `fixed<digits, scale>`, as the decimal string the writer took:
    /// the leading zeros go, and a negative zero loses its sign. Under
    /// "value-precision" and "value-string-digits" the field ends at the first octet whose low
    /// nibble is a sign, and one that never presents a sign within the
    /// sixteen octets of the widest fixed IDL admits is refused.
    pub fn readFixed(self: *Reader, digits: u16, scale: u16, count: []const u8) WireError![]const u8 {
        const declared = std.mem.eql(u8, count, "declared");
        if (!declared and !std.mem.eql(u8, count, "value-precision") and !std.mem.eql(u8, count, "value-string-digits")) {
            return WireError.OutOfRange;
        }
        // The same bound as the writer's, on this side too: OMG IDL 4.2
        // rule (42) caps the BCD rule at 31 digits, and a reader asked for
        // a wider field would take octets no declaration can name.
        if (digits == 0 or digits > 31 or scale > digits) {
            return WireError.OutOfRange;
        }
        var nibbles: std.ArrayList(u8) = .empty;
        defer nibbles.deinit(self.heap);
        var sign: u8 = 0;
        if (!declared) {
            while (true) {
                if (nibbles.items.len >= 32) {
                    return WireError.OutOfRange;
                }
                const b = (try self.take(1))[0];
                try nibbles.append(self.heap, b >> 4);
                try nibbles.append(self.heap, b & 0x0F);
                if ((b & 0x0F) >= 0x0A) {
                    break;
                }
            }
            sign = nibbles.items[nibbles.items.len - 1];
            nibbles.items.len -= 1;
        } else {
            const raw = try self.take((digits + 2) / 2);
            for (raw) |b| {
                try nibbles.append(self.heap, b >> 4);
                try nibbles.append(self.heap, b & 0x0F);
            }
            sign = nibbles.items[nibbles.items.len - 1];
            nibbles.items.len -= 1;
            if ((digits + 1) % 2 == 1) {
                _ = nibbles.orderedRemove(0);
            }
        }
        for (nibbles.items) |nibble| {
            if (nibble > 9) {
                return WireError.OutOfRange;
            }
        }
        var text: std.ArrayList(u8) = .empty;
        defer text.deinit(self.heap);
        for (nibbles.items) |nibble| {
            try text.append(self.heap, '0' + nibble);
        }
        while (text.items.len < @as(usize, scale) + 1) {
            try text.insert(self.heap, 0, '0');
        }
        const split = text.items.len - scale;
        var whole: []const u8 = text.items[0..split];
        while (whole.len > 1 and whole[0] == '0') {
            whole = whole[1..];
        }
        const fraction = text.items[split..];
        var result: std.ArrayList(u8) = .empty;
        errdefer result.deinit(self.heap);
        var zero = std.mem.eql(u8, whole, "0");
        for (fraction) |digit| {
            if (digit != '0') {
                zero = false;
            }
        }
        if (sign == 0x0D and !zero) {
            try result.append(self.heap, '-');
        }
        try result.appendSlice(self.heap, whole);
        if (scale > 0) {
            try result.append(self.heap, '.');
            try result.appendSlice(self.heap, fraction);
        }
        return try result.toOwnedSlice(self.heap);
    }

    // -- the trims a bound reaches for -------------------------------------

    /// The trim, reached as a method because the shared walk emits it
    /// as a call on the reader.
    pub fn trimUtf8(self: *Reader, text: []const u8, octets: usize) []const u8 {
        _ = self;
        return trimUtf8Text(text, octets);
    }

    /// The wide half.
    pub fn trimUtf16(self: *Reader, text: []const u8, units: usize) []const u8 {
        _ = self;
        return trimUtf16Text(text, units);
    }

    // -- the bit-bounded kinds ---------------------------------------------

    /// An enumerator's holder, as the signed integer §7.4.3.5.3 makes
    /// it — the holder's width and not the bound's.
    pub fn readEnum(self: *Reader, bit_bound: u16) WireError!i64 {
        if (bit_bound == 0 or bit_bound > 32) {
            return WireError.OutOfRange;
        }
        if (bit_bound <= 8) {
            return @as(i8, @bitCast(try self.readU8()));
        }
        if (bit_bound <= 16) {
            return @as(i16, @bitCast(try self.readU16()));
        }
        return @as(i32, @bitCast(try self.readU32()));
    }

    /// A bitmask's holder. Table 33 of §7.4.1.1.3.2.
    pub fn readBitmask(self: *Reader, bit_bound: u16) WireError!u64 {
        if (bit_bound == 0 or bit_bound > 64) {
            return WireError.OutOfRange;
        }
        if (bit_bound <= 8) {
            return try self.readU8();
        }
        if (bit_bound <= 16) {
            return try self.readU16();
        }
        if (bit_bound <= 32) {
            return try self.readU32();
        }
        return try self.readU64();
    }

    /// A bitset's anchor integer, of the width the planner sized.
    pub fn readBitset(self: *Reader, anchor: u16) WireError!u64 {
        return switch (anchor) {
            1 => try self.readU8(),
            2 => try self.readU16(),
            4 => try self.readU32(),
            8 => try self.readU64(),
            else => WireError.OutOfRange,
        };
    }

    // -- framing -----------------------------------------------------------

    /// A DHEADER: the length the members that follow occupy, refusing
    /// one the stream cannot hold.
    pub fn readDheader(self: *Reader) WireError!u32 {
        const length = try self.readU32();
        if (length > self.readerRemaining()) {
            return WireError.ImpossibleLength;
        }
        return length;
    }

    /// An EMHEADER, unpacked. Codes 5 to 7 report the NEXTINT they
    /// read, but the reader is put back before it: that word is the
    /// member's own leading length. §7.4.3.4.2.
    pub fn readEmheader(self: *Reader) WireError!EmHeaderRead {
        const word = try self.readU32();
        var header = EmHeaderRead{
            .member_id = word & e_member_id_mask,
            .length_code = @intCast((word >> 28) & 0x7),
            .must_understand = (word >> 31) != 0,
        };
        if (header.length_code == 4) {
            header.next_int = try self.readU32();
            header.has_next_int = true;
        } else if (header.length_code >= 5) {
            const where = self.at;
            header.next_int = try self.readU32();
            header.has_next_int = true;
            self.at = where;
        }
        header.extent = emExtent(header);
        return header;
    }

    fn emExtent(header: EmHeaderRead) usize {
        if (header.length_code <= 3) {
            return @as(usize, 1) << @as(u2, @intCast(header.length_code));
        }
        if (!header.has_next_int) {
            return 0;
        }
        if (header.length_code == 4) {
            return header.next_int;
        }
        const scale: usize = switch (header.length_code) {
            6 => 4,
            7 => 8,
            else => 1,
        };
        return 4 + @as(usize, header.next_int) * scale;
    }

    fn readPidHeader(self: *Reader, role: ParameterRole) WireError!PidRead {
        self.alignTo(4);
        const pid: u32 = try self.readU16();
        const length: u32 = try self.readU16();
        if ((pid & pid_id_mask) == (pid_extended & pid_id_mask)) {
            // The four flag bits are not part of the id: §7.4.1.2.1
            // splits the eMemberHeader into FLAG_1 to FLAG_4 and a
            // 28-bit member id, and FLAG_2 is the member's
            // must-understand.
            const member_header = try self.readU32();
            const wide_length = try self.readU32();
            return .{
                .pid = member_header & e_member_id_mask,
                .length = wide_length,
                .end = false,
                .must_understand = (member_header & e_flag_must_understand) != 0,
                .impl_extension = (member_header & e_flag_impl_extension) != 0,
                .role = role,
            };
        }
        const identifier = pid & pid_id_mask;
        const end = identifier == (pid_list_end & pid_id_mask);
        // Under a framing that puts FLAG_IMPL_EXTENSION on every member
        // id the bit carries no information and is reported absent.
        const flagged = (pid & pid_impl_extension) != 0 and
            ((try pidFlagsAt(self.pid_flags, .member)) & pid_impl_extension) == 0;
        return .{
            .pid = identifier,
            .length = length,
            .end = end,
            .must_understand = (pid & pid_must_understand) != 0,
            .impl_extension = flagged,
            .reserved = !end and identifier > self.highest_short_id,
            .ignore = identifier == pid_ignore,
            .role = role,
        };
    }

    /// A PL_CDR1 parameter header in the member role.
    pub fn readPid(self: *Reader) WireError!PidRead {
        return self.readPidAs(.member);
    }

    /// A parameter header, and the value block it opens: §7.4.1.2 makes
    /// a parameter's value its own stream block, so a member's internal
    /// alignment is counted from the first octet after the header.
    pub fn readPidAs(self: *Reader, role: ParameterRole) WireError!PidRead {
        var header = try self.readPidHeader(role);
        header.saved_origin = self.origin;
        header.value_start = self.at;
        self.origin = self.at;
        return header;
    }

    /// End a parameter's value and step to the next header. The used
    /// extent is measured from the value's own start, checked against
    /// the declared length under both readings of the padding
    /// convention — a length belonging to neither is a stream this
    /// reader was pointed at with the wrong framing — and only then is
    /// the padding to four stepped, against the enclosing stream.
    /// Nothing follows PID_LIST_END, and a reader that has already
    /// refused does not move.
    pub fn finishPidRead(self: *Reader, header: PidRead) void {
        self.origin = header.saved_origin;
        if (self.status != .ok or header.end) {
            return;
        }
        const used = self.at - header.value_start;
        const declared: usize = header.length;
        const padded = (used + 3) & ~@as(usize, 3);
        // The declared length is checked against the framing rather
        // than believed: it is the exact extent under Fast DDS's
        // convention and the rounded-up one under RTI's, both are
        // accepted — they coincide for every value already a multiple
        // of four — and anything else is a stream this reader was
        // pointed at with the wrong framing, where reading on would put
        // every later member at an offset no writer produced.
        if (declared != used and declared != padded) {
            self.readerRefuse(.out_of_range);
        }
        self.alignTo(4);
    }
};

fn stripByteOrderMark(heap: std.mem.Allocator, swapped: *std.ArrayList(u16), units: []const u16) WireError![]const u16 {
    if (units.len == 0) {
        return units;
    }
    if (units[0] == byte_order_mark) {
        return units[1..];
    }
    var rest = units;
    if (units[0] == swapped_byte_order_mark) {
        rest = units[1..];
    }
    for (rest) |unit| {
        try swapped.append(heap, (unit << 8) | (unit >> 8));
    }
    return swapped.items;
}

// -- fixed BCD -------------------------------------------------------------

fn fixedNibbles(heap: std.mem.Allocator, nibbles: *std.ArrayList(u8), text: []const u8, digits: u16, scale: u16, count: []const u8) WireError!void {
    // OMG IDL 4.2 rule (42) caps the BCD rule at 31 digits, and
    // fixed<31, S> is the widest declaration IDL admits. A wider one has
    // no packing defined for it, so it is refused rather than packed into
    // octets no reader can be told the width of.
    if (digits == 0 or digits > 31 or scale > digits) {
        return WireError.OutOfRange;
    }
    const declared = std.mem.eql(u8, count, "declared");
    const as_string = std.mem.eql(u8, count, "value-string-digits");
    if (!declared and !as_string and !std.mem.eql(u8, count, "value-precision")) {
        return WireError.OutOfRange;
    }
    var negative = false;
    var magnitude = text;
    if (magnitude.len > 0 and (magnitude[0] == '-' or magnitude[0] == '+')) {
        negative = magnitude[0] == '-';
        magnitude = magnitude[1..];
    }
    var whole = magnitude;
    var fraction: []const u8 = "";
    for (magnitude, 0..) |c, i| {
        if (c == '.') {
            whole = magnitude[0..i];
            fraction = magnitude[i + 1 ..];
            break;
        }
    }
    if (whole.len == 0 and fraction.len == 0) {
        return WireError.OutOfRange;
    }
    for (whole) |c| {
        if (c < '0' or c > '9') {
            return WireError.OutOfRange;
        }
    }
    for (fraction) |c| {
        if (c < '0' or c > '9') {
            return WireError.OutOfRange;
        }
    }
    const as_written = whole.len + @as(usize, scale);
    const integral_width: usize = digits - scale;
    while (whole.len > 0 and whole[0] == '0') {
        whole = whole[1..];
    }
    if (whole.len > integral_width or fraction.len > scale) {
        return WireError.OutOfRange;
    }
    var every: std.ArrayList(u8) = .empty;
    defer every.deinit(heap);
    var i: usize = 0;
    while (i < integral_width - whole.len) : (i += 1) {
        try every.append(heap, 0);
    }
    for (whole) |c| {
        try every.append(heap, c - '0');
    }
    for (fraction) |c| {
        try every.append(heap, c - '0');
    }
    i = 0;
    while (i < scale - fraction.len) : (i += 1) {
        try every.append(heap, 0);
    }
    var significant: usize = 0;
    for (every.items, 0..) |nibble, index| {
        if (nibble != 0) {
            significant = every.items.len - index;
            break;
        }
    }
    var wire = every.items.len;
    if (as_string) {
        // ACE/TAO 2.5.24's ACE_CDR::Fixed::from_string (OpenDDS writes the declared count): the
        // digits as written, made odd by one leading zero (0.12345 under
        // fixed<5,5> is 0012345c). Past 31 no reader can be told the width.
        wire = as_written + @as(usize, if (as_written % 2 == 0) 1 else 0);
        if (wire > 31) {
            return WireError.OutOfRange;
        }
    } else if (!declared) {
        wire = significant;
        if (wire < 1) {
            wire = 1;
        }
    }
    if ((wire + 1) % 2 == 1) {
        try nibbles.append(heap, 0);
    }
    // A count wider than P is filled with leading zeros.
    const kept = @min(wire, every.items.len);
    i = kept;
    while (i < wire) : (i += 1) {
        try nibbles.append(heap, 0);
    }
    try nibbles.appendSlice(heap, every.items[every.items.len - kept ..]);
    var sign: u8 = 0x0C;
    if (negative and significant != 0) {
        sign = 0x0D;
    }
    try nibbles.append(heap, sign);
}

// -- self-checks -----------------------------------------------------------

test "primitive round trips in both byte orders" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const heap = arena.allocator();
    const framing = PlCdr1Framing{
        .pid_flags = "sentinel-must-understand",
        .highest_short_id = 0x3EFF,
        .pad_declared_length = false,
        .pad_declared_optional_length = false,
        .member_must_understand = true,
        .pad_after_value = true,
    };
    inline for (.{ Endianness.little, Endianness.big }) |order| {
        var w = Writer.init(heap, order, 8, framing, false);
        try w.writeI32(-7);
        try w.writeF64(3.25);
        try w.writeString("hi");
        try w.writeF128(1.5);
        var r = Reader.init(heap, w.finish(), order, 8, "sentinel-must-understand", 0x3EFF);
        try std.testing.expectEqual(@as(i32, -7), try r.readI32());
        try std.testing.expectEqual(@as(f64, 3.25), try r.readF64());
        try std.testing.expectEqualStrings("hi", try r.readString());
        try std.testing.expectEqual(@as(f128, 1.5), try r.readF128());
    }
}

test "the enum holder is signed at every bound" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const heap = arena.allocator();
    const framing = PlCdr1Framing{
        .pid_flags = "bare",
        .highest_short_id = 0x3EFF,
        .pad_declared_length = false,
        .pad_declared_optional_length = false,
        .member_must_understand = true,
        .pad_after_value = true,
    };
    var w = Writer.init(heap, .little, 8, framing, false);
    // Cyclone takes @value(200) under @bit_bound(8) and writes c8,
    // which reads back as the pattern -56.
    try w.writeEnum(8, 200);
    var r = Reader.init(heap, w.finish(), .little, 8, "bare", 0x3EFF);
    try std.testing.expectEqual(@as(i64, -56), try r.readEnum(8));
}

test "the overlay masks a negative value instead of sign-extending" {
    try std.testing.expectEqual(@as(u64, 0x0000_0000_FFFF_FFFE), bitPatternInt(-2, 4));
    try std.testing.expectEqual(@as(u64, 1), bitPatternBool(true));
}

/// The three axes `Dialect::Hdds` selects. Expected octets from the
/// recorded HDDS columns:
/// `tests/interop-results/wstring-member/hdds/plain-cdr2.le.bin`
/// (`04 00 00 00 61 62 63 00` for "abc"),
/// `tests/interop-results/array-1d-struct-final/hdds/plain-cdr2.le.bin`
/// (`01 00 00 00 02 00 03 00 00 00 04 00 77`) and
/// `tests/interop-results/sequence-struct-appendable/hdds/plain-cdr2.le.bin`
/// (the first Cell of `one`, `06 00 00 00 79 29 ed ff fe ff`).
fn hddsFraming() PlCdr1Framing {
    return .{
        .pid_flags = "sentinel-must-understand",
        .highest_short_id = 0x3EFF,
        .pad_declared_length = false,
        .pad_declared_optional_length = false,
        .member_must_understand = true,
        .pad_after_value = true,
    };
}

test "a UTF-8 wstring counts its octets and a NUL" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const heap = arena.allocator();
    var w = Writer.init(heap, .little, 4, hddsFraming(), false);
    try w.writeWstringUtf8Octets("abc", "octets/terminated/2");
    try std.testing.expectEqualSlices(u8, &[_]u8{ 4, 0, 0, 0, 'a', 'b', 'c', 0 }, w.finish());
    var r = Reader.init(heap, w.finish(), .little, 4, "sentinel-must-understand", 0x3EFF);
    try std.testing.expectEqualStrings("abc", try r.readWstringUtf8Octets("octets/terminated/2"));
}

test "a bounded UTF-8 wstring bounds its UTF-16 units" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const heap = arena.allocator();
    var w = Writer.init(heap, .little, 4, hddsFraming(), false);
    try w.writeWstringUtf8OctetsBounded("ሴäZ", 3, "octets/terminated/2");
    try std.testing.expectEqualSlices(
        u8,
        &[_]u8{ 7, 0, 0, 0, 0xe1, 0x88, 0xb4, 0xc3, 0xa4, 'Z', 0 },
        w.finish(),
    );
    var over = Writer.init(heap, .little, 4, hddsFraming(), false);
    try std.testing.expectError(
        WireError.OutOfRange,
        over.writeWstringUtf8OctetsBounded("ሴäZ", 2, "octets/terminated/2"),
    );
}

test "a packed element counts alignment from its own start" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const heap = arena.allocator();
    var w = Writer.init(heap, .little, 4, hddsFraming(), false);
    var saved = w.beginPackedElement();
    try w.writeI32(1);
    try w.writeI16(2);
    w.finishPackedElement(saved);
    saved = w.beginPackedElement();
    try w.writeI32(3);
    try w.writeI16(4);
    w.finishPackedElement(saved);
    try w.writeU8(119);
    try std.testing.expectEqualSlices(
        u8,
        &[_]u8{ 1, 0, 0, 0, 2, 0, 3, 0, 0, 0, 4, 0, 119 },
        w.finish(),
    );
    const recorded = [_]u8{ 1, 0, 0, 0, 2, 0, 3, 0, 0, 0, 4, 0, 119 };
    var r = Reader.init(heap, &recorded, .little, 4, "sentinel-must-understand", 0x3EFF);
    var read = r.readerBeginPackedElement();
    try std.testing.expectEqual(@as(i32, 1), try r.readI32());
    try std.testing.expectEqual(@as(i16, 2), try r.readI16());
    r.readerFinishPackedElement(read);
    read = r.readerBeginPackedElement();
    try std.testing.expectEqual(@as(i32, 3), try r.readI32());
    try std.testing.expectEqual(@as(i16, 4), try r.readI16());
    r.readerFinishPackedElement(read);
    try std.testing.expectEqual(@as(u8, 119), try r.readU8());
}

test "a sequence element length frames the element it precedes" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const heap = arena.allocator();
    var w = Writer.init(heap, .little, 4, hddsFraming(), false);
    const d = try w.writeDheader();
    try w.writeI32(-1234567);
    try w.writeI16(-2);
    try w.finishDheader(d);
    const got = w.finish();
    try std.testing.expectEqualSlices(
        u8,
        &[_]u8{ 6, 0, 0, 0, 0x79, 0x29, 0xed, 0xff, 0xfe, 0xff },
        got,
    );
    var r = Reader.init(heap, got, .little, 4, "sentinel-must-understand", 0x3EFF);
    const declared = try r.readU32();
    const end = r.readerPosition() + declared;
    try std.testing.expectEqual(@as(i32, -1234567), try r.readI32());
    try std.testing.expectEqual(@as(i16, -2), try r.readI16());
    try std.testing.expectEqual(end, r.readerPosition());
}
