// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Native pure-Zig endpoint SDK (ADR 0013): a from-scratch XCDR wire-core, no C,
// byte-identical to the Rust core and the other SDKs. Zig has no async/await, so
// the async model is a callback reactor (push) alongside the sync poll (pull).

const std = @import("std");

pub const Endian = enum { little, big };

/// The XCDR version the stream is written in.
///
/// A constructor argument with no default. XTypes 1.3 §7.4.1 puts MAXALIGN at
/// eight under XCDR1 and four under XCDR2, so a writer that did not know which
/// one it was in would be writing XCDR2 and calling it XCDR — which is what this
/// SDK did until the cap became a field: `alignTo` read a constant four, and
/// every `long long` in an XCDR1 sample landed four octets early with nothing in
/// the interface to say so.
pub const Encoding = enum {
    /// PLAIN_CDR / PL_CDR — MAXALIGN 8.
    xcdr1,
    /// PLAIN_CDR2 / DELIMITED_CDR / PL_CDR2 — MAXALIGN 4.
    xcdr2,

    /// The alignment cap this encoding imposes. XTypes 1.3 §7.4.1.
    pub fn maxAlign(self: Encoding) usize {
        return switch (self) {
            .xcdr1 => 8,
            .xcdr2 => 4,
        };
    }
};

/// The member header length codes of XTypes 1.3 §7.4.3.4.2 Table 34. The code
/// occupies bits 28..30 of the EMHEADER and decides how the member's byte length
/// is recovered: LC0..LC3 give it from the code alone, LC4..LC7 from the four
/// octets that follow.
///
/// An enum and not an integer, so a code outside 0..7 cannot be spelt at all and
/// there is no runtime refusal to write for one.
pub const LengthCode = enum(u3) {
    /// The member body is one octet.
    lc0 = 0,
    /// Two octets.
    lc1 = 1,
    /// Four octets.
    lc2 = 2,
    /// Eight octets.
    lc3 = 3,
    /// A NEXTINT holding the body's byte length follows the EMHEADER.
    lc4 = 4,
    /// The body's first four octets are themselves the length, and they are part
    /// of the body. NEXTINT is not written a second time.
    lc5 = 5,
    /// As LC5, and the length counts four-octet elements.
    lc6 = 6,
    /// As LC5, and the length counts eight-octet elements.
    lc7 = 7,
};

/// PID_LIST_END. XTypes 1.3 §7.4.1.2 Table 31.
pub const pid_list_end_id: u16 = 0x3F02;
/// The PID that introduces the twelve-octet parameter header. §7.4.1.2.
pub const pid_extended_id: u16 = 0x3F01;
/// The highest member id a short parameter header can carry, inclusive.
pub const pid_short_max_id: u16 = 0x3F00;
/// Bit 14 of a short parameter id. XTypes 1.3 §7.4.1.2.
const pid_flag_must_understand: u16 = 0x4000;
/// Bit 30 of the eMemberHeader inside an extended parameter header — not bit 31,
/// which is the implementation-extension flag.
const em_flag_must_understand: u32 = 0x4000_0000;

/// The refusals this wire-core answers with rather than putting a plausible but
/// wrong octet on the wire.
pub const WireError = error{
    /// A `char` above U+00FF. Truncating would write a different, valid
    /// character of the right length and nothing downstream could tell.
    CharAboveLatin1,
    /// A `wchar` above U+FFFF: one two-octet code unit cannot hold it.
    WcharAboveBmp,
    /// A member id wider than the 28 bits an EMHEADER or an eMemberHeader has.
    MemberIdTooWide,
    /// A member id above 0x3F00 handed to the short parameter header.
    ParameterIdTooWide,
    /// LC0..LC3 name a fixed body size and the member wrote another, or LC5..LC7
    /// take their length from a leading word the member did not write.
    LengthCodeMismatch,
    /// A `bitset` anchor that is not one, two, four or eight octets.
    BitsetAnchorInvalid,
    /// `fixed<P,S>` outside IDL 4.2 §7.4.1.4.5's 1..31 digits.
    FixedDigitsOutOfRange,
    /// A scale outside 0..digits.
    FixedScaleOutOfRange,
    /// A `fixed` literal that is not an optional sign, digits and an optional
    /// fraction.
    FixedNotADecimal,
    /// More integer digits than `digits - scale`.
    FixedIntegerPartTooWide,
    /// More significant fraction digits than `scale`.
    FixedFractionTooWide,
    /// A short parameter header describes at most 65535 octets.
    ParameterTooLong,
    /// The output buffer a caller handed a reader or a `fixed` write is shorter
    /// than the octets it has to hold.
    OutputTooSmall,
};

pub const Writer = struct {
    buf: std.ArrayList(u8),
    /// The allocator `buf` grows from. Zig 0.16's `std.ArrayList` is the
    /// unmanaged one — it does not hold an allocator, and `append`,
    /// `appendSlice` and `deinit` take it as an argument — so the writer
    /// keeps the one it was made with.
    alloc: std.mem.Allocator,
    endian: Endian,
    /// The alignment cap, from the Encoding the writer was made with.
    /// XTypes 1.3 §7.4.1.
    max_align: usize,
    /// The position alignment is measured from. Zero for the stream, and moved
    /// to the start of a PL_CDR1 parameter's value for the length of that
    /// parameter — §7.4.1.2 aligns a parameter's value against its own block and
    /// not against the stream. Without it a `double` in a parameter takes four
    /// octets of padding and the header declares twelve where Cyclone, RTI and
    /// Fast DDS all declare eight.
    origin: usize = 0,

    /// Both arguments are required. The encoding decides MAXALIGN and nothing in
    /// the call may leave it implied.
    pub fn init(alloc: std.mem.Allocator, endian: Endian, encoding: Encoding) Writer {
        return .{
            .buf = .empty,
            .alloc = alloc,
            .endian = endian,
            .max_align = encoding.maxAlign(),
        };
    }
    pub fn deinit(self: *Writer) void {
        self.buf.deinit(self.alloc);
    }

    fn alignTo(self: *Writer, a: usize) !void {
        const cap: usize = if (a > self.max_align) self.max_align else a;
        while ((self.buf.items.len - self.origin) % cap != 0) try self.buf.append(self.alloc, 0);
    }
    fn putLE(self: *Writer, a: usize, le: []const u8) !void {
        try self.alignTo(a);
        if (self.endian == .big) {
            var i: usize = le.len;
            while (i > 0) {
                i -= 1;
                try self.buf.append(self.alloc, le[i]);
            }
        } else {
            try self.buf.appendSlice(self.alloc, le);
        }
    }
    pub fn putU8(self: *Writer, v: u8) !void {
        try self.buf.append(self.alloc, v);
    }
    pub fn putU16(self: *Writer, v: u16) !void {
        try self.putLE(2, &[_]u8{ @truncate(v), @truncate(v >> 8) });
    }
    pub fn putU32(self: *Writer, v: u32) !void {
        try self.putLE(4, &[_]u8{ @truncate(v), @truncate(v >> 8), @truncate(v >> 16), @truncate(v >> 24) });
    }
    /// A u64 at its natural eight-octet alignment, which `max_align` then caps:
    /// eight under XCDR1 and four under XCDR2. XTypes 1.3 §7.4.1.
    pub fn putU64(self: *Writer, v: u64) !void {
        const le = [_]u8{
            @truncate(v),       @truncate(v >> 8),  @truncate(v >> 16), @truncate(v >> 24),
            @truncate(v >> 32), @truncate(v >> 40), @truncate(v >> 48), @truncate(v >> 56),
        };
        try self.putLE(8, &le);
    }
    pub fn putF32(self: *Writer, v: f32) !void {
        const bits: u32 = @bitCast(v);
        try self.putU32(bits);
    }
    /// An f64 as its IEEE-754 bit pattern, at the same alignment as putU64.
    /// `double` is one of the fifteen primitives of XTypes 1.3 §7.2.2.2 and had
    /// no writer here at all.
    pub fn putF64(self: *Writer, v: f64) !void {
        const bits: u64 = @bitCast(v);
        try self.putU64(bits);
    }
    /// A boolean as the one octet XTypes 1.3 §7.4.1.1.1 Table 30 gives it: 0 or
    /// 1, never any other pattern.
    pub fn putBool(self: *Writer, v: bool) !void {
        try self.putU8(if (v) 1 else 0);
    }
    pub fn putBytes(self: *Writer, b: []const u8) !void {
        try self.buf.appendSlice(self.alloc, b);
    }
    pub fn putString(self: *Writer, s: []const u8) !void {
        try self.putU32(@intCast(s.len + 1));
        try self.putBytes(s);
        try self.putU8(0);
    }
    pub fn putSeqU8(self: *Writer, b: []const u8) !void {
        try self.putU32(@intCast(b.len));
        try self.putBytes(b);
    }

    /// An IDL `char`: the one octet XTypes 1.3 §7.4.1.1.1 Table 30 gives it.
    ///
    /// A code point above U+00FF is refused and not truncated. `š` is U+0161 and
    /// its low octet is 0x61, which is `a`: a truncating writer produces a frame
    /// of the right length holding a different, plausible character, and nothing
    /// downstream can tell. The Rust core refuses `primitive-char-above-latin1`
    /// in the same words.
    pub fn putChar(self: *Writer, code_point: u32) !void {
        if (code_point > 0xFF) return error.CharAboveLatin1;
        try self.putU8(@truncate(code_point));
    }

    /// An IDL `wchar`: one two-octet code unit, 2-aligned, in the stream's byte
    /// order.
    ///
    /// The four policy axes a wide character carries are answered here and not by
    /// the caller: the unit is two octets wide, it goes out in the stream's byte
    /// order, there is no GIOP length octet before it, and no byte-order mark.
    /// Measured against `wchar_member`, where Cyclone and Fast DDS write
    /// `6100 ab` little-endian under both encodings and RTI writes the same under
    /// XCDR2; RTI's XCDR1 `wchar_t` is four octets wide (`61000000 ab`) and is
    /// the one vendor answer this does not reproduce.
    ///
    /// A code point above U+FFFF is refused. The refusal is width-independent — a
    /// four-octet form would zero-extend a code *unit*, not a code point — which
    /// is how the Rust core refuses `primitive-wchar-above-bmp`.
    pub fn putWchar(self: *Writer, code_point: u32) !void {
        if (code_point > 0xFFFF) return error.WcharAboveBmp;
        try self.putU16(@truncate(code_point));
    }

    /// An IDL `wstring` from its UTF-16 code units.
    ///
    /// The length word counts **octets** and not code units, there is no
    /// terminator, there is no byte-order mark, and each unit goes out two octets
    /// wide in the stream's own byte order. Those are the four axes, and they are
    /// the binding's answer — the caller hands over units and nothing else.
    ///
    /// Measured against `wstring_member` (`04000000 6100 6200 ab`, Cyclone under
    /// both encodings and RTI under XCDR2) and `wstring_nonbmp`
    /// (`04000000 34d8 1edd ab`, the same two): a code point outside the basic
    /// plane is two units and the length word counts its four octets. Fast DDS
    /// writes the unit *count* there (`02000000`) and truncates the non-BMP code
    /// point to a single unit (`010000001ed1`); neither is reproduced.
    pub fn putWstring(self: *Writer, units: []const u16) !void {
        try self.putU32(@intCast(units.len * 2));
        for (units) |unit| try self.putU16(unit);
    }

    /// A `bitset` as its anchor word: the holder the declared bits pack into,
    /// written at the anchor's own width and therefore at the anchor's own
    /// alignment — eight under XCDR1 and four under XCDR2 for an eight-octet
    /// anchor. XTypes 1.3 §7.3.1.4 and §7.4.1.
    ///
    /// `bits` is the packed holder and `anchor` its width in octets, both
    /// properties of the declared type. What this call decides is the pair the
    /// type does not: which write the holder goes out through, and what that
    /// write aligns to.
    pub fn putBitset(self: *Writer, bits: u64, anchor: usize) !void {
        switch (anchor) {
            1 => try self.putU8(@truncate(bits)),
            2 => try self.putU16(@truncate(bits)),
            4 => try self.putU32(@truncate(bits)),
            8 => try self.putU64(bits),
            else => return error.BitsetAnchorInvalid,
        }
    }

    /// An IDL `fixed<digits,scale>` as packed BCD.
    /// CDR (OMG CORBA 3.3) §9.3.2.8 / IDL 4.2 §7.4.1.4.5.
    ///
    /// `value` is the decimal as the sample wrote it — an optional sign, an
    /// integer part and an optional fraction. Everything after that is this
    /// call's; see `fixedOctets` for the packing and for why the octet count is
    /// ceil((digits + 1) / 2) whatever the value.
    pub fn putFixed(self: *Writer, value: []const u8, digits: usize, scale: usize) !void {
        var scratch: [16]u8 = undefined;
        try self.putBytes(try fixedOctets(value, digits, scale, &scratch));
    }

    // --- back-patched framing ---------------------------------------------
    //
    // A DHEADER, a NEXTINT and a PL_CDR1 parameter length are all the same
    // shape: a slot reserved before the body and filled in once the body's
    // length is known. `patchU32` is the one place that writes into an
    // already-written position, and it honours the wire byte order like every
    // other write.

    fn patchU32(self: *Writer, at: usize, v: u32) void {
        const le = [4]u8{ @truncate(v), @truncate(v >> 8), @truncate(v >> 16), @truncate(v >> 24) };
        var i: usize = 0;
        while (i < 4) : (i += 1) {
            self.buf.items[at + i] = if (self.endian == .big) le[3 - i] else le[i];
        }
    }

    fn patchU16(self: *Writer, at: usize, v: u16) void {
        const le = [2]u8{ @truncate(v), @truncate(v >> 8) };
        if (self.endian == .big) {
            self.buf.items[at] = le[1];
            self.buf.items[at + 1] = le[0];
        } else {
            self.buf.items[at] = le[0];
            self.buf.items[at + 1] = le[1];
        }
    }

    /// Reserve a 4-aligned DHEADER slot and return the position the body starts
    /// at, to hand back to `dheaderEnd`. XTypes 1.3 §7.4.3.5.
    ///
    /// `@appendable` is the default extensibility of XTypes (§7.3.1.2.1.8), so a
    /// struct written without an annotation had no form here at all.
    pub fn dheaderBegin(self: *Writer) !usize {
        try self.putU32(0);
        return self.buf.items.len;
    }

    /// Back-patch the DHEADER with the byte length written since `body_start`.
    /// The DHEADER itself is not counted — §7.4.3.5 measures the content that
    /// follows it.
    pub fn dheaderEnd(self: *Writer, body_start: usize) void {
        self.patchU32(body_start - 4, @intCast(self.buf.items.len - body_start));
    }

    /// Write the EMHEADER for one member of a `@mutable` aggregate and return the
    /// body-start position. XTypes 1.3 §7.4.3.4.2:
    ///
    ///     EMHEADER = (must_understand << 31) | (length_code << 28) | member_id
    ///
    /// with a 28-bit member id. For LC4 a NEXTINT slot is reserved after it and
    /// `emHeaderEnd` fills it in; for LC0..LC3 the code alone gives the length and
    /// nothing follows; for LC5..LC7 the body's own leading four octets are the
    /// length, so no separate NEXTINT is written — writing it twice would put
    /// every following member at the wrong offset.
    pub fn emHeaderBegin(self: *Writer, id: u32, must_understand: bool, lc: LengthCode) !usize {
        if (id > 0x0FFF_FFFF) return error.MemberIdTooWide;
        var header: u32 = (@as(u32, @intFromEnum(lc)) << 28) | id;
        if (must_understand) header |= @as(u32, 1) << 31;
        try self.putU32(header);
        if (lc == .lc4) try self.putU32(0);
        return self.buf.items.len;
    }

    /// Close the member `emHeaderBegin` opened.
    ///
    /// It takes the length code back rather than remembering it, so a caller that
    /// paired an LC4 begin with a differently-coded end gets a refusal instead of
    /// a NEXTINT patched over the first four octets of a member body.
    pub fn emHeaderEnd(self: *Writer, body_start: usize, lc: LengthCode) !void {
        const written = self.buf.items.len - body_start;
        switch (lc) {
            .lc4 => self.patchU32(body_start - 4, @intCast(written)),
            // 1, 2, 4, 8 — the code *is* the length, so a body of any other size
            // means the plan and the member disagree. Silence here would put a
            // length on the wire that no octet count backs.
            .lc0, .lc1, .lc2, .lc3 => {
                const want = @as(usize, 1) << @intFromEnum(lc);
                if (written != want) return error.LengthCodeMismatch;
            },
            .lc5, .lc6, .lc7 => if (written < 4) return error.LengthCodeMismatch,
        }
    }

    /// An open PL_CDR1 parameter header, to be handed back to `pidEnd`.
    pub const Pid = struct {
        /// The position of the length field.
        at: usize,
        /// Whether the length field is four octets or two.
        extended: bool,
        /// The alignment origin to restore when the parameter closes.
        saved_origin: usize,
    };

    /// A PL_CDR1 short parameter header. XTypes 1.3 §7.4.1.2: at a four-octet
    /// boundary an `unsigned short` parameter id, then an `unsigned short` byte
    /// length of the value that follows. Both go out in the stream's byte order.
    ///
    /// The must-understand flag is bit 14 of the id field. Bit 15 is the
    /// implementation-extension flag and is not set here.
    ///
    /// The parameter's value aligns against the block and not against the stream,
    /// so the alignment origin moves for the length of the parameter and `pidEnd`
    /// restores it. Measured on `ext_mutable` under XCDR1, where the `double`
    /// member's header declares eight in Cyclone, RTI and Fast DDS alike — a
    /// stream-relative alignment would declare twelve.
    pub fn pidBegin(self: *Writer, pid: u16, must_understand: bool) !Pid {
        if (pid > pid_short_max_id) return error.ParameterIdTooWide;
        try self.alignTo(4);
        var id = pid;
        if (must_understand) id |= pid_flag_must_understand;
        try self.putLE(2, &[_]u8{ @truncate(id), @truncate(id >> 8) });
        const at = self.buf.items.len;
        try self.buf.appendSlice(self.alloc, &[_]u8{ 0, 0 });
        const saved = self.origin;
        self.origin = self.buf.items.len;
        return .{ .at = at, .extended = false, .saved_origin = saved };
    }

    /// The twelve-octet parameter header XTypes 1.3 §7.4.1.2 requires for a
    /// member id above 0x3F00 or a value longer than 65535 octets:
    ///
    ///     [PID_EXTENDED | must-understand : u16][8 : u16][eMemberHeader : u32][length : u32]
    ///
    /// The must-understand flag on the marker is not the member's — it is on the
    /// marker under this framing whatever the member says, and the member's own
    /// flag is bit 30 of the eMemberHeader. Reading it off the marker instead
    /// makes every parameter must-understand.
    pub fn pidExtendedBegin(self: *Writer, member_id: u32, must_understand: bool) !Pid {
        if (member_id > 0x0FFF_FFFF) return error.MemberIdTooWide;
        try self.alignTo(4);
        const marker: u16 = pid_extended_id | pid_flag_must_understand;
        try self.putLE(2, &[_]u8{ @truncate(marker), @truncate(marker >> 8) });
        try self.putLE(2, &[_]u8{ 8, 0 });
        var header = member_id;
        if (must_understand) header |= em_flag_must_understand;
        try self.putLE(4, &[_]u8{
            @truncate(header),       @truncate(header >> 8),
            @truncate(header >> 16), @truncate(header >> 24),
        });
        const at = self.buf.items.len;
        try self.buf.appendSlice(self.alloc, &[_]u8{ 0, 0, 0, 0 });
        const saved = self.origin;
        self.origin = self.buf.items.len;
        return .{ .at = at, .extended = true, .saved_origin = saved };
    }

    /// Close the parameter `pidBegin` or `pidExtendedBegin` opened.
    ///
    /// The declared length counts the value's octets **and** the padding that
    /// brings the next header back to a four-octet boundary: a reader skips
    /// `length` from the end of the header and has to land on the next parameter.
    /// The padding itself is written after the length is patched, so the two
    /// cannot disagree.
    pub fn pidEnd(self: *Writer, header: Pid) !void {
        const width: usize = if (header.extended) 4 else 2;
        var length = self.buf.items.len - header.at - width;
        // The parameter's own origin is still in force here, so the round-up is
        // measured from the block that the length describes.
        const past = (self.buf.items.len - header.saved_origin) % 4;
        if (past != 0) length += 4 - past;
        if (header.extended) {
            self.patchU32(header.at, @intCast(length));
        } else {
            if (length > 0xFFFF) return error.ParameterTooLong;
            self.patchU16(header.at, @intCast(length));
        }
        self.origin = header.saved_origin;
        try self.alignTo(4);
    }

    /// The sentinel that closes a PL_CDR1 parameter list: the id PID_LIST_END
    /// with a zero length, four-aligned. XTypes 1.3 §7.4.1.2.
    ///
    /// The must-understand flag is set on the sentinel — 0x7F02 on the wire,
    /// which is what Cyclone and RTI write (`ext_mutable` XCDR1 LE ends
    /// `027f0000`). Fast DDS writes the bare 0x3F02; the flag is not the
    /// sentinel's meaning either way, and this is the majority form the Rust
    /// column records.
    pub fn pidListEnd(self: *Writer) !void {
        try self.alignTo(4);
        const id: u16 = pid_list_end_id | pid_flag_must_understand;
        try self.putLE(2, &[_]u8{ @truncate(id), @truncate(id >> 8) });
        try self.putLE(2, &[_]u8{ 0, 0 });
    }

    pub fn bytes(self: *Writer) []const u8 {
        return self.buf.items;
    }
};

/// The BCD packing `Writer.putFixed` writes, into a caller's buffer so it can be
/// measured at the edges of its range without a Writer.
///
///   - the integer part is left-padded and the fraction right-padded to exactly
///     `digits` decimal digits, with the point implied `scale` from the right;
///     the scale itself is not on the wire,
///   - a leading zero nibble is prepended when `digits + 1` is odd, so digits and
///     sign pair up,
///   - two nibbles to the octet, most significant digit first, high nibble first,
///   - the last nibble is the sign: 0xD negative, 0xC otherwise. A negative zero
///     goes out positive, because there is one zero to write,
///   - alignment is one and no multi-octet integer is written, so the byte order
///     never enters.
///
/// The octet count is therefore ceil((digits + 1) / 2), whatever the value.
pub fn fixedOctets(value: []const u8, digits: usize, scale: usize, out: []u8) WireError![]u8 {
    if (digits < 1 or digits > 31) return error.FixedDigitsOutOfRange;
    if (scale > digits) return error.FixedScaleOutOfRange;
    if (out.len < (digits + 2) / 2) return error.OutputTooSmall;

    var text = value;
    var negative = false;
    if (text.len > 0 and (text[0] == '+' or text[0] == '-')) {
        negative = text[0] == '-';
        text = text[1..];
    }
    var integral = text;
    var fraction: []const u8 = "";
    if (std.mem.indexOfScalar(u8, text, '.')) |at| {
        integral = text[0..at];
        fraction = text[at + 1 ..];
    }
    if (integral.len == 0 and fraction.len == 0) return error.FixedNotADecimal;
    for (integral) |c| if (c < '0' or c > '9') return error.FixedNotADecimal;
    for (fraction) |c| if (c < '0' or c > '9') return error.FixedNotADecimal;

    // Leading zeros carry no value, so they are dropped before the width is
    // checked: `000000.001` is one significant integer digit, not six.
    var lead: usize = 0;
    while (lead < integral.len and integral[lead] == '0') lead += 1;
    integral = integral[lead..];
    if (integral.len > digits - scale) return error.FixedIntegerPartTooWide;
    // A trailing zero in the fraction is padding either way, so it is dropped
    // before the width is checked and re-added below.
    while (fraction.len > scale and fraction[fraction.len - 1] == '0')
        fraction = fraction[0 .. fraction.len - 1];
    if (fraction.len > scale) return error.FixedFractionTooWide;

    var nibbles: [34]u8 = undefined;
    var n: usize = 0;
    if ((digits + 1) % 2 == 1) {
        nibbles[n] = 0;
        n += 1;
    }
    var pad = digits - scale - integral.len;
    while (pad > 0) : (pad -= 1) {
        nibbles[n] = 0;
        n += 1;
    }
    for (integral) |c| {
        nibbles[n] = c - '0';
        n += 1;
    }
    for (fraction) |c| {
        nibbles[n] = c - '0';
        n += 1;
    }
    var tail = scale - fraction.len;
    while (tail > 0) : (tail -= 1) {
        nibbles[n] = 0;
        n += 1;
    }
    var non_zero = false;
    for (nibbles[0..n]) |d| {
        if (d != 0) non_zero = true;
    }
    nibbles[n] = if (negative and non_zero) 0x0D else 0x0C;
    n += 1;

    var i: usize = 0;
    var k: usize = 0;
    while (i + 1 < n) : (i += 2) {
        out[k] = nibbles[i] << 4 | nibbles[i + 1];
        k += 1;
    }
    return out[0..k];
}

pub const Reader = struct {
    buf: []const u8,
    pos: usize,
    endian: Endian,
    /// The alignment cap, from the Encoding the reader was made with — the same
    /// cap the writer used, or the fields land elsewhere.
    max_align: usize,
    /// Mirrors the writer's: a PL_CDR1 parameter's value aligns against its own
    /// block. §7.4.1.2.
    origin: usize = 0,

    pub fn init(b: []const u8, endian: Endian, encoding: Encoding) Reader {
        return .{ .buf = b, .pos = 0, .endian = endian, .max_align = encoding.maxAlign() };
    }
    fn getLE(self: *Reader, a: usize, out: []u8) void {
        const cap: usize = if (a > self.max_align) self.max_align else a;
        while ((self.pos - self.origin) % cap != 0) self.pos += 1;
        var k: usize = 0;
        while (k < out.len) : (k += 1) out[k] = self.buf[self.pos + k];
        self.pos += out.len;
        if (self.endian == .big) std.mem.reverse(u8, out);
    }
    pub fn getU32(self: *Reader) u32 {
        var le: [4]u8 = undefined;
        self.getLE(4, &le);
        return @as(u32, le[0]) | @as(u32, le[1]) << 8 | @as(u32, le[2]) << 16 | @as(u32, le[3]) << 24;
    }
    // --- primitives beyond getU32 — the byte-exact inverse of the Writer ---
    pub fn getU8(self: *Reader) u8 {
        const b = self.buf[self.pos];
        self.pos += 1;
        return b;
    }
    pub fn getU16(self: *Reader) u16 {
        var le: [2]u8 = undefined;
        self.getLE(2, &le);
        return @as(u16, le[0]) | @as(u16, le[1]) << 8;
    }
    /// A u64 at its natural eight-octet alignment, which `max_align` then caps.
    /// It passed 4 here while the writer passed 8, which under XCDR1 read the
    /// four octets of padding as the value's low half and returned a different
    /// number with no error anywhere.
    pub fn getU64(self: *Reader) u64 {
        var le: [8]u8 = undefined;
        self.getLE(8, &le);
        return @as(u64, le[0]) | @as(u64, le[1]) << 8 | @as(u64, le[2]) << 16 | @as(u64, le[3]) << 24 |
            @as(u64, le[4]) << 32 | @as(u64, le[5]) << 40 | @as(u64, le[6]) << 48 | @as(u64, le[7]) << 56;
    }
    pub fn getF32(self: *Reader) f32 {
        return @bitCast(self.getU32());
    }
    /// An f64 from its IEEE-754 bit pattern.
    pub fn getF64(self: *Reader) f64 {
        return @bitCast(self.getU64());
    }
    /// The one octet a boolean occupies.
    pub fn getBool(self: *Reader) bool {
        return self.getU8() != 0;
    }
    /// The one octet an IDL `char` occupies.
    pub fn getChar(self: *Reader) u32 {
        return self.getU8();
    }
    /// One two-octet `wchar` code unit.
    pub fn getWchar(self: *Reader) u32 {
        return self.getU16();
    }
    /// A `wstring` back into its UTF-16 code units, written into `out`. The
    /// length word counts octets, so the unit count is half of it.
    pub fn getWstring(self: *Reader, out: []u16) WireError![]u16 {
        const octets: usize = self.getU32();
        const units = octets / 2;
        if (out.len < units) return error.OutputTooSmall;
        var i: usize = 0;
        while (i < units) : (i += 1) out[i] = self.getU16();
        return out[0..units];
    }
    /// A `bitset` anchor of the given width, back into its holder word.
    pub fn getBitset(self: *Reader, anchor: usize) WireError!u64 {
        return switch (anchor) {
            1 => self.getU8(),
            2 => self.getU16(),
            4 => self.getU32(),
            8 => self.getU64(),
            else => error.BitsetAnchorInvalid,
        };
    }
    /// A `fixed<digits,scale>` back into the decimal it was written from, into
    /// `out`: a leading `-` when the sign nibble is 0xD, `digits - scale` integer
    /// digits, and a `.` with `scale` fraction digits where the scale is not
    /// zero. The scale is not on the wire, so it is an argument here for the same
    /// reason it is one in `putFixed`.
    pub fn getFixed(self: *Reader, digits: usize, scale: usize, out: []u8) WireError![]const u8 {
        if (digits < 1 or digits > 31) return error.FixedDigitsOutOfRange;
        if (scale > digits) return error.FixedScaleOutOfRange;
        if (out.len < digits + 2) return error.OutputTooSmall;
        const octets = (digits + 2) / 2;
        var nibbles: [34]u8 = undefined;
        var i: usize = 0;
        while (i < octets) : (i += 1) {
            const b = self.getU8();
            nibbles[2 * i] = b >> 4;
            nibbles[2 * i + 1] = b & 0x0F;
        }
        const total = 2 * octets;
        const sign = nibbles[total - 1];
        const body = nibbles[total - 1 - digits .. total - 1];
        var k: usize = 0;
        if (sign == 0x0D) {
            out[k] = '-';
            k += 1;
        }
        for (body, 0..) |d, at| {
            if (scale != 0 and at == digits - scale) {
                out[k] = '.';
                k += 1;
            }
            out[k] = '0' + d;
            k += 1;
        }
        return out[0..k];
    }
    pub fn getString(self: *Reader) []const u8 {
        const n: usize = self.getU32();
        const s = self.buf[self.pos .. self.pos + n - 1];
        self.pos += n;
        return s;
    }
    pub fn getSeqU8(self: *Reader) []const u8 {
        const n: usize = self.getU32();
        const b = self.buf[self.pos .. self.pos + n];
        self.pos += n;
        return b;
    }

    /// Read a DHEADER and return the announced body byte length. §7.4.3.5.
    pub fn dheaderRead(self: *Reader) u32 {
        return self.getU32();
    }

    /// What `emHeaderRead` recovered.
    pub const Member = struct {
        id: u32,
        must_understand: bool,
        lc: LengthCode,
        /// The member body's extent in octets, measured from the position the
        /// read left the cursor at.
        extent: u32,
    };

    /// Read a member header. LC0..LC3 take the extent from the code. LC4 consumes
    /// the NEXTINT that follows the header, and the extent is that word. LC5..LC7
    /// do not consume anything: the next four octets are the member's own leading
    /// word and belong to the body, so they are peeked and the extent is 4 + the
    /// word for LC5, 4 + 4×word for LC6 and 4 + 8×word for LC7 — the last two
    /// count elements, not octets.
    pub fn emHeaderRead(self: *Reader) Member {
        const header = self.getU32();
        const lc: LengthCode = @enumFromInt(@as(u3, @truncate(header >> 28)));
        var extent: u32 = 0;
        switch (lc) {
            .lc0, .lc1, .lc2, .lc3 => extent = @as(u32, 1) << @intFromEnum(lc),
            .lc4 => extent = self.getU32(),
            .lc5, .lc6, .lc7 => {
                const at = self.pos;
                const word = self.getU32();
                self.pos = at;
                extent = switch (lc) {
                    .lc5 => 4 + word,
                    .lc6 => 4 + 4 * word,
                    else => 4 + 8 * word,
                };
            },
        }
        return .{
            .id = header & 0x0FFF_FFFF,
            .must_understand = header & (@as(u32, 1) << 31) != 0,
            .lc = lc,
            .extent = extent,
        };
    }

    /// What `pidRead` recovered.
    pub const Parameter = struct {
        id: u32,
        must_understand: bool,
        /// The declared byte length of the value.
        length: u32,
        /// Whether the list ends here.
        list_end: bool,
    };

    /// Read a PL_CDR1 parameter header in either form.
    ///
    /// The extended form is recognised by the marker PID_EXTENDED, and the
    /// must-understand answer then comes from bit 30 of the eMemberHeader and
    /// never from the marker — the marker carries the flag under this framing
    /// whatever the member says.
    ///
    /// The value's alignment origin is moved to the position this call leaves,
    /// and `pidSkip` restores it.
    pub fn pidRead(self: *Reader) Parameter {
        const raw = self.getU16();
        const short = raw & 0x3FFF;
        if (short == pid_list_end_id) {
            _ = self.getU16();
            return .{ .id = 0, .must_understand = false, .length = 0, .list_end = true };
        }
        if (short == pid_extended_id) {
            _ = self.getU16(); // the slength, always 8
            const header = self.getU32();
            const length = self.getU32();
            self.origin = self.pos;
            return .{
                .id = header & 0x0FFF_FFFF,
                .must_understand = header & em_flag_must_understand != 0,
                .length = length,
                .list_end = false,
            };
        }
        const length: u32 = self.getU16();
        self.origin = self.pos;
        return .{
            .id = short,
            .must_understand = raw & pid_flag_must_understand != 0,
            .length = length,
            .list_end = false,
        };
    }

    /// Move the cursor past a parameter value of `length` octets from
    /// `body_start` and restore the stream's alignment origin.
    pub fn pidSkip(self: *Reader, body_start: usize, length: u32) void {
        self.pos = body_start + length;
        self.origin = 0;
        while (self.pos % 4 != 0) self.pos += 1;
    }
};

// --- XRCE framing ---

pub const XRCE_SESSION_NOKEY: u8 = 0x80;
pub const XRCE_STREAM_BEST_EFFORT: u8 = 0x01;

// The refusals, as errors rather than as a length of zero.
pub const FrameError = error{
    // The 16-bit submessage_length field cannot describe a sample larger than
    // 0xFFFF, so a sample above it has no WRITE_DATA form: refuse rather than
    // truncate the length while copying the full payload (the C SDK's
    // zdw_xrce_write_frame rejects the same way).
    SampleTooLarge,
    // `out` is shorter than the eight-octet envelope plus the sample. This used
    // to be a slice out of bounds inside `copyForwards` — safety-checked
    // undefined behaviour, which is a panic in Debug and a wrong write in
    // ReleaseFast. Client.write hands its own 1024-octet `txbuf`, so any sample
    // above 1016 octets reached it.
    BufferTooSmall,
};

/// Wire size of the `BaseObjectRequest` (DDS-XRCE 1.0 §7.7.8:
/// `request_id[2]` + `object_id[2]`) that opens every `WRITE_DATA` and
/// `DATA` body.
pub const XRCE_BASE_SIZE: usize = 4;

/// `ObjectId` (§7.2.1) of the endpoint app's DataWriter: the raw id
/// `0x001` in the high twelve bits, `OBJK_DATAWRITER` (0x05) in the low
/// four. The endpoint profile has no CREATE handshake, so this is the
/// fixture both ends agree on — the same value the C, Go, Python, Java
/// and Rust SDKs put on the wire, and the one
/// `zerodds_endpoint_e2e::endpoint_writer_id` builds.
pub const ENDPOINT_WRITER_OBJECT_ID: u16 = (0x001 << 4) | 0x05;

/// The `E_LITTLE_ENDIAN` flag alone. §8.3.5.8 puts the data format in
/// bits 1..3 and `FORMAT_DATA` is `0b000`; `0x03` would announce
/// `FORMAT_SAMPLE` (0b001) and promise a `SampleInfo` this profile does
/// not carry.
pub const XRCE_WRITE_FLAGS: u8 = 0x01;

// Returns the frame length. It used to return 0 for a refusal and Client.write
// delivered `txbuf[0..0]`, so an oversize sample went out as an empty datagram
// and `write` returned whatever `deliver` did. Zig makes a caller handle an
// error union, which is what the zero never made anyone do.
//
// The body is the `BaseObjectRequest` and then the sample, and the flags
// are `0x01`. This wrote the bare sample under `0x03` and called itself
// "an older, non-conformant frame" — the fourth place in this tree with
// that exact shape, after the C, Python and Java SDKs and this file's own
// `reliable.zig`. §7.7.8 makes the `BaseObjectRequest` the only thing in
// the message that names the DataWriter, so a body without it cannot be
// routed by any XRCE agent, and `crates/xrce`'s `WriteDataPayload` — the
// decoder `zerodds-endpoint-e2e`'s peer uses — refuses a body shorter
// than four octets. Measured: `zig_endpoint_sync` and `zig_endpoint_async`
// reported `not an XRCE frame` while the datagram was on the wire.
//
// Octet for octet `zdw_xrce_write_frame` in
// `endpoints/c/src/zerodds_endpoint.c`.
pub fn xrceWriteFrame(out: []u8, session: u8, stream: u8, seq: u16, sample: []const u8) FrameError!usize {
    if (sample.len + XRCE_BASE_SIZE > 0xFFFF) return error.SampleTooLarge;
    if (out.len < 8 + XRCE_BASE_SIZE + sample.len) return error.BufferTooSmall;
    const body_len: u16 = @intCast(XRCE_BASE_SIZE + sample.len);
    out[0] = session;
    out[1] = stream;
    out[2] = @truncate(seq); // sequence_nr, little-endian
    out[3] = @truncate(seq >> 8);
    out[4] = 0x07; // WRITE_DATA
    out[5] = XRCE_WRITE_FLAGS;
    out[6] = @truncate(body_len); // submessage_length, little-endian
    out[7] = @truncate(body_len >> 8);
    // BaseObjectRequest: RequestId(2) then ObjectId(2), most significant
    // octet first (§7.7.8 / §7.2.1). The request id is the message's own
    // sequence number, which is all §7.7.8 asks of it.
    out[8] = @truncate(seq >> 8);
    out[9] = @truncate(seq);
    out[10] = @truncate(ENDPOINT_WRITER_OBJECT_ID >> 8);
    out[11] = @truncate(ENDPOINT_WRITER_OBJECT_ID);
    std.mem.copyForwards(u8, out[8 + XRCE_BASE_SIZE ..][0..sample.len], sample);
    return 8 + XRCE_BASE_SIZE + sample.len;
}

// Frame layout: [session, stream, seq_lo, seq_hi, sm_id, flags, len_lo, len_hi]
// then the body, which is the `BaseObjectRequest` and then the sample. The
// 16-bit little-endian submessage_length (bytes 6..7) bounds the body exactly:
// the slice ends at 8+sm_len, never at the end of the datagram, so trailing
// padding or an appended submessage is not folded into the sample.
//
// Accepts WRITE_DATA (0x07, endpoint->hub / loopback) and DATA (0x09,
// hub->endpoint) — the hub pushes samples with DATA. See DDS-XRCE spec 8.3.5.
// Returns null for a short header, a wrong submessage id, a declared length
// that runs past the datagram (truncation / wrong length), or a body too short
// to hold the `BaseObjectRequest` — the last is `zdw_xrce_locate_body`'s
// refusal, and it is a refusal rather than a truncated sample because a body
// that short is not a data submessage.
pub fn xrceReadFrame(frame: []const u8) ?[]const u8 {
    if (frame.len < 8 or (frame[4] != 0x07 and frame[4] != 0x09)) return null;
    const sm_len: usize = @as(usize, frame[6]) | (@as(usize, frame[7]) << 8);
    if (8 + sm_len > frame.len) return null;
    if (sm_len < XRCE_BASE_SIZE) return null;
    return frame[8 + XRCE_BASE_SIZE .. 8 + sm_len];
}

/// The `BaseObjectRequest` of a received frame: `(request_id, object_id)`,
/// each read most significant octet first. `null` on the same refusals as
/// `xrceReadFrame`.
///
/// The counterpart of `zdw_xrce_frame_base`. A caller that has to route a
/// sample needs the id the sender named, and it was on the wire with
/// nowhere to read it from.
pub fn xrceFrameBase(frame: []const u8) ?struct { request_id: u16, object_id: u16 } {
    if (frame.len < 8 + XRCE_BASE_SIZE or (frame[4] != 0x07 and frame[4] != 0x09)) return null;
    const sm_len: usize = @as(usize, frame[6]) | (@as(usize, frame[7]) << 8);
    if (8 + sm_len > frame.len or sm_len < XRCE_BASE_SIZE) return null;
    return .{
        .request_id = @as(u16, frame[8]) << 8 | @as(u16, frame[9]),
        .object_id = @as(u16, frame[10]) << 8 | @as(u16, frame[11]),
    };
}

// --- transport (function-pointer vtable, no heap) ---

pub const Transport = struct {
    ctx: *anyopaque,
    deliver: *const fn (ctx: *anyopaque, frame: []const u8) bool,
    // returns the frame length, or null when nothing is ready (again).
    receive: *const fn (ctx: *anyopaque, buf: []u8) ?usize,
};

// --- sync client: pull ---

/// The best-effort client of the endpoint profile: one `WRITE_DATA` per
/// sample, no CREATE handshake, the DataWriter named by the fixture
/// `ENDPOINT_WRITER_OBJECT_ID`.
///
/// Its frame used to be described here as "an older, in-house,
/// non-conformant" one, and it was: flags `0x03` and a body without the
/// `BaseObjectRequest`. It is the conformant frame now — see
/// `xrceWriteFrame` — so the difference to `xrce_client.zig` is no longer
/// the octets but the protocol above them: that file speaks the full
/// object lifecycle (CREATE_CLIENT, CREATE, DELETE, READ_DATA) with a
/// real agent, this one writes samples to a peer that already knows the
/// endpoint.
pub const Client = struct {
    transport: *const Transport,
    session: u8 = XRCE_SESSION_NOKEY,
    stream: u8 = XRCE_STREAM_BEST_EFFORT,
    seq: u16 = 1,
    txbuf: [1024]u8 = undefined,
    rxbuf: [2048]u8 = undefined,

    // `FrameError!bool` and no longer `bool`: a sample the envelope cannot
    // carry is a refusal the caller has to handle, where the old zero length
    // was delivered as an empty slice and reported as whatever `deliver`
    // returned. The sequence is not advanced, because nothing was written.
    pub fn write(self: *Client, sample: []const u8) FrameError!bool {
        const n = try xrceWriteFrame(&self.txbuf, self.session, self.stream, self.seq, sample);
        self.seq +%= 1;
        return self.transport.deliver(self.transport.ctx, self.txbuf[0..n]);
    }
    // One synchronous receive: returns the sample body, or null when nothing.
    pub fn poll(self: *Client) ?[]const u8 {
        const n = self.transport.receive(self.transport.ctx, &self.rxbuf) orelse return null;
        return xrceReadFrame(self.rxbuf[0..n]);
    }
};

// --- async reader: push (callback reactor) ---

pub const OnSample = *const fn (ctx: *anyopaque, body: []const u8) void;

pub const AsyncReader = struct {
    transport: *const Transport,
    on_sample: OnSample,
    ctx: *anyopaque,
    rxbuf: [2048]u8 = undefined,

    // Dispatch one ready frame to the callback. Returns true if dispatched.
    pub fn poll(self: *AsyncReader) bool {
        const n = self.transport.receive(self.transport.ctx, &self.rxbuf) orelse return false;
        if (xrceReadFrame(self.rxbuf[0..n])) |body| {
            self.on_sample(self.ctx, body);
            return true;
        }
        return false;
    }
    // Drain the transport (until nothing ready, or `max` frames). Returns count.
    pub fn run(self: *AsyncReader, max: usize) usize {
        var count: usize = 0;
        while (max == 0 or count < max) {
            if (self.poll()) {
                count += 1;
            } else break;
        }
        return count;
    }
};

// ============================ tests ============================

const testing = std.testing;

fn fixture(w: *Writer) !void {
    try w.putU32(0xA1B2C3D4);
    try w.putU16(0x1234);
    try w.putU8(0x5A);
    try w.putF32(3.5);
    try w.putU64(0x0102030405060708);
    try w.putString("bay-12");
    try w.putSeqU8(&[_]u8{ 0xDE, 0xAD, 0xBE, 0xEF });
}

/// Path of a golden file written by `zerodds-endpoint-golden <dir>`. The
/// directory comes from the environment variable `GOLDEN_DIR` (the one
/// `tools/endpoints-gate` and the other endpoint harnesses export); without it
/// the files are read from `build/` below the working directory (`zig build
/// test` runs in `endpoints/zig`, and the gate copies the goldens there).
fn goldenPath(alloc: std.mem.Allocator, name: []const u8) ![]u8 {
    if (std.testing.environ.getAlloc(alloc, "GOLDEN_DIR")) |dir| {
        defer alloc.free(dir);
        if (dir.len > 0) return std.fs.path.join(alloc, &.{ dir, name });
    } else |_| {}
    return std.fs.path.join(alloc, &.{ "build", name });
}

test "byte identity vs Rust goldens" {
    const cases = [_]struct { endian: Endian, file: []const u8 }{
        .{ .endian = .little, .file = "golden_le.bin" },
        .{ .endian = .big, .file = "golden_be.bin" },
    };
    for (cases) |tc| {
        var w = Writer.init(testing.allocator, tc.endian, .xcdr2);
        defer w.deinit();
        try fixture(&w);
        const path = try goldenPath(testing.allocator, tc.file);
        defer testing.allocator.free(path);
        // Zig 0.16: `std.fs.cwd` moved to `std.Io.Dir.cwd`, and
        // `readFileAlloc` takes the `Io` first and an `Io.Limit` last.
        const golden = try std.Io.Dir.cwd().readFileAlloc(testing.io, path, testing.allocator, .limited(4096));
        defer testing.allocator.free(golden);
        try testing.expectEqualSlices(u8, golden, w.bytes());
    }
}

// In-memory FIFO transport for the loopback tests.
const Fifo = struct {
    frames: [16][256]u8 = undefined,
    lens: [16]usize = undefined,
    head: usize = 0,
    tail: usize = 0,
    count: usize = 0,

    fn deliver(ctx: *anyopaque, frame: []const u8) bool {
        const f: *Fifo = @ptrCast(@alignCast(ctx));
        if (f.count == 16) return false;
        std.mem.copyForwards(u8, f.frames[f.tail][0..frame.len], frame);
        f.lens[f.tail] = frame.len;
        f.tail = (f.tail + 1) % 16;
        f.count += 1;
        return true;
    }
    fn receive(ctx: *anyopaque, buf: []u8) ?usize {
        const f: *Fifo = @ptrCast(@alignCast(ctx));
        if (f.count == 0) return null;
        const l = f.lens[f.head];
        std.mem.copyForwards(u8, buf[0..l], f.frames[f.head][0..l]);
        f.head = (f.head + 1) % 16;
        f.count -= 1;
        return l;
    }
};

fn sampleBody(w: *Writer, id: u32) ![]const u8 {
    try w.putU32(id);
    try w.putU16(0);
    try w.putU8(0);
    return w.bytes();
}

test "sync loopback (pull)" {
    var fifo = Fifo{};
    const t = Transport{ .ctx = &fifo, .deliver = Fifo.deliver, .receive = Fifo.receive };
    var c = Client{ .transport = &t };
    var i: u32 = 0;
    while (i < 5) : (i += 1) {
        var bw = Writer.init(testing.allocator, .little, .xcdr2);
        defer bw.deinit();
        const body = try sampleBody(&bw, 0x3000 + i);
        try testing.expect(try c.write(body));
    }
    i = 0;
    while (i < 5) : (i += 1) {
        const body = c.poll() orelse return error.NoSample;
        var r = Reader.init(body, .little, .xcdr2);
        try testing.expectEqual(@as(u32, 0x3000 + i), r.getU32());
    }
}

const Collector = struct {
    ids: [5]u32 = undefined,
    got: usize = 0,
    fn on(ctx: *anyopaque, body: []const u8) void {
        const c: *Collector = @ptrCast(@alignCast(ctx));
        var r = Reader.init(body, .little, .xcdr2);
        if (c.got < 5) {
            c.ids[c.got] = r.getU32();
            c.got += 1;
        }
    }
};

test "async loopback (push / callback reactor)" {
    var fifo = Fifo{};
    const t = Transport{ .ctx = &fifo, .deliver = Fifo.deliver, .receive = Fifo.receive };
    var w = Client{ .transport = &t };
    var i: u32 = 0;
    while (i < 5) : (i += 1) {
        var bw = Writer.init(testing.allocator, .little, .xcdr2);
        defer bw.deinit();
        const body = try sampleBody(&bw, 0x1000 + i);
        try testing.expect(try w.write(body));
    }
    var col = Collector{};
    var reader = AsyncReader{ .transport = &t, .on_sample = Collector.on, .ctx = &col };
    const n = reader.run(0);
    try testing.expectEqual(@as(usize, 5), n);
    try testing.expectEqual(@as(usize, 5), col.got);
    i = 0;
    while (i < 5) : (i += 1) try testing.expectEqual(@as(u32, 0x1000 + i), col.ids[i]);
}

test "read frame bounds body to declared length, ignoring appended submessage" {
    var buf1: [64]u8 = undefined;
    var buf2: [64]u8 = undefined;
    const n1 = try xrceWriteFrame(&buf1, XRCE_SESSION_NOKEY, XRCE_STREAM_BEST_EFFORT, 1, &[_]u8{ 0xAA, 0xBB, 0xCC });
    const n2 = try xrceWriteFrame(&buf2, XRCE_SESSION_NOKEY, XRCE_STREAM_BEST_EFFORT, 2, &[_]u8{ 0xDD, 0xEE });
    var concat: [128]u8 = undefined;
    std.mem.copyForwards(u8, concat[0..n1], buf1[0..n1]);
    std.mem.copyForwards(u8, concat[n1 .. n1 + n2], buf2[0..n2]);
    const body = xrceReadFrame(concat[0 .. n1 + n2]).?;
    try testing.expectEqualSlices(u8, &[_]u8{ 0xAA, 0xBB, 0xCC }, body);
}

test "read frame rejects over-long length and truncation, writer refuses >0xFFFF" {
    var buf: [64]u8 = undefined;
    const n = try xrceWriteFrame(&buf, XRCE_SESSION_NOKEY, XRCE_STREAM_BEST_EFFORT, 1, &[_]u8{ 0xAA, 0xBB, 0xCC });
    var overlong: [64]u8 = undefined;
    std.mem.copyForwards(u8, overlong[0..n], buf[0..n]);
    overlong[6] = 0xFF;
    overlong[7] = 0xFF;
    try testing.expect(xrceReadFrame(overlong[0..n]) == null);
    try testing.expect(xrceReadFrame(&[_]u8{ 0x80, 0x01, 0x00, 0x00, 0x07 }) == null);
    var big: [0x10000]u8 = undefined;
    var out: [0x10010]u8 = undefined;
    // A refusal and not a length of zero: the zero was delivered as an empty
    // slice by Client.write and reported as whatever the transport returned.
    try testing.expectError(
        error.SampleTooLarge,
        xrceWriteFrame(&out, XRCE_SESSION_NOKEY, XRCE_STREAM_BEST_EFFORT, 1, &big),
    );
    // The 16-bit submessage_length describes the whole body, and the body
    // opens with the four-octet BaseObjectRequest — so the largest sample
    // is four short of the field, and the one above it is refused.
    const largest_sample = 0xFFFF - XRCE_BASE_SIZE;
    try testing.expectError(
        error.SampleTooLarge,
        xrceWriteFrame(&out, XRCE_SESSION_NOKEY, XRCE_STREAM_BEST_EFFORT, 1, big[0 .. largest_sample + 1]),
    );
    // The bound is the field's and not one off it.
    try testing.expectEqual(
        @as(usize, 8 + 0xFFFF),
        try xrceWriteFrame(&out, XRCE_SESSION_NOKEY, XRCE_STREAM_BEST_EFFORT, 1, big[0..largest_sample]),
    );
}

// The form itself, stated as octets rather than implied by a round trip.
// DDS-XRCE 1.0 §7.7.8 opens the body with the BaseObjectRequest and
// §8.3.5.8 puts FORMAT_DATA (0b000) in flag bits 1..3, which leaves the
// E_LITTLE_ENDIAN flag alone at 0x01. `endpoints/c`, `endpoints/go`,
// `crates/xrce` and this file's own `reliable.zig` write exactly this;
// the four assertions below are why that can be said rather than hoped.
test "a WRITE_DATA frame carries the BaseObjectRequest under FORMAT_DATA flags" {
    var out: [32]u8 = undefined;
    const sample = [_]u8{ 0xAA, 0xBB, 0xCC };
    const n = try xrceWriteFrame(&out, XRCE_SESSION_NOKEY, XRCE_STREAM_BEST_EFFORT, 0x0102, &sample);
    try testing.expectEqualSlices(u8, &[_]u8{
        XRCE_SESSION_NOKEY, XRCE_STREAM_BEST_EFFORT,
        0x02, 0x01, // sequence_nr, little-endian
        0x07, // WRITE_DATA
        0x01, // E_LITTLE_ENDIAN, FORMAT_DATA
        0x07, 0x00, // submessage_length = 4 + 3, little-endian
        0x01, 0x02, // request_id = the sequence number, most significant first
        0x00, 0x15, // object_id = raw 0x001 + OBJK_DATAWRITER
        0xAA, 0xBB, 0xCC,
    }, out[0..n]);

    // The reader hands back the sample and not the BaseObjectRequest.
    try testing.expectEqualSlices(u8, &sample, xrceReadFrame(out[0..n]).?);

    // And the ids are readable, because a caller that routes needs them.
    const base = xrceFrameBase(out[0..n]).?;
    try testing.expectEqual(@as(u16, 0x0102), base.request_id);
    try testing.expectEqual(ENDPOINT_WRITER_OBJECT_ID, base.object_id);

    // A body too short to hold the BaseObjectRequest is not a data
    // submessage: refused, never returned as a truncated sample.
    var short = [_]u8{ XRCE_SESSION_NOKEY, XRCE_STREAM_BEST_EFFORT, 0, 0, 0x07, 0x01, 0x03, 0x00, 1, 2, 3 };
    try testing.expect(xrceReadFrame(&short) == null);
    try testing.expect(xrceFrameBase(&short) == null);
}

// Client.write hands its own 1024-octet `txbuf`, so every sample above 1016
// octets ran `copyForwards` past the end of it. In a safety-checked build that
// is a panic and in ReleaseFast it is a write into whatever follows the field;
// neither is a refusal. It is one now, and it is the caller's to handle.
test "the writer refuses a sample its output buffer cannot hold" {
    // The envelope is now twelve octets — eight of header and four of
    // BaseObjectRequest — so a 64-octet buffer holds 52 and refuses 53.
    // Both sides of the boundary, because a bound tested only from above
    // is also satisfied by refusing everything.
    var out: [64]u8 = undefined;
    var fits: [52]u8 = undefined;
    try testing.expectEqual(
        @as(usize, 64),
        try xrceWriteFrame(&out, XRCE_SESSION_NOKEY, XRCE_STREAM_BEST_EFFORT, 1, &fits),
    );
    var sample: [53]u8 = undefined;
    try testing.expectError(
        error.BufferTooSmall,
        xrceWriteFrame(&out, XRCE_SESSION_NOKEY, XRCE_STREAM_BEST_EFFORT, 1, &sample),
    );
    var fifo = Fifo{};
    const t = Transport{ .ctx = &fifo, .deliver = Fifo.deliver, .receive = Fifo.receive };
    var c = Client{ .transport = &t };
    // `Client.txbuf` is 1024, so the first sample it cannot frame is 1013.
    var large: [1013]u8 = undefined;
    try testing.expectError(error.BufferTooSmall, c.write(&large));
    try testing.expect(c.poll() == null);
}

test {
    // Gate the reliable-stream unit tests (`reliable.zig`) through the same
    // `zig test src/zerodds.zig` command CI already runs — it is otherwise only
    // referenced lazily (`zerodds.zig` never `@import`s it eagerly), so its tests
    // would go unrun. Includes the AsyncWriter window-overflow regression test.
    _ = @import("reliable.zig");
    // The XCDR surface measured against octets Cyclone, RTI, Fast DDS and
    // omniORB wrote, plus the range edges of every operation. Referenced here
    // for the same reason `reliable.zig` is: `zig test src/zerodds.zig` is the
    // one command CI runs, and a test file nothing imports goes unrun.
    _ = @import("surface_test.zig");
    // The DDS-XRCE 1.0 client with the full object lifecycle (ported from
    // `crates/xrce-client`), which reaches a real XRCE agent over
    // `Transport`. It used to be "the conformant one" beside a
    // non-conformant `Client`; both frames are conformant now and what
    // separates them is the protocol above the frame. Swept in for the
    // same reason as the two imports above.
    _ = @import("xrce_client.zig");
}
