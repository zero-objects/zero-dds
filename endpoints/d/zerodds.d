// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Native pure-D endpoint SDK (ADR 0013): a from-scratch XCDR wire-core,
// byte-identical to the Rust core and the other SDKs. Sync (poll) and async
// (std.concurrency spawn/send/receive — Tid message passing, idiomatic D).

module zerodds;

import std.concurrency;

enum Endian { LE, BE }

/// The XCDR version the stream is written in.
///
/// A constructor argument with no default. XTypes 1.3 section 7.4.1 puts
/// MAXALIGN at eight under XCDR1 and four under XCDR2, so a writer that did not
/// know which one it was in would be writing XCDR2 and calling it XCDR — which
/// is what this SDK did until the cap became a field: `alignTo` read a constant
/// four, and every `long` in an XCDR1 sample landed four octets early with
/// nothing in the interface to say so.
enum Encoding {
    XCDR1, /// PLAIN_CDR / PL_CDR — MAXALIGN 8.
    XCDR2  /// PLAIN_CDR2 / DELIMITED_CDR / PL_CDR2 — MAXALIGN 4.
}

/// The alignment cap this encoding imposes. XTypes 1.3 section 7.4.1.
int maxAlign(Encoding e) { return e == Encoding.XCDR1 ? 8 : 4; }

/// The member header length codes of XTypes 1.3 section 7.4.3.4.2 Table 34. The
/// code occupies bits 28..30 of the EMHEADER and decides how the member's byte
/// length is recovered: LC0..LC3 give it from the code alone, LC4..LC7 from the
/// four octets that follow.
///
/// An enum and not an int, so a code outside 0..7 has no spelling at all.
enum LengthCode {
    LC0 = 0, /// The member body is one octet.
    LC1 = 1, /// Two octets.
    LC2 = 2, /// Four octets.
    LC3 = 3, /// Eight octets.
    LC4 = 4, /// A NEXTINT holding the body's byte length follows the EMHEADER.
    LC5 = 5, /// The body's first four octets are themselves the length, and they
             /// are part of the body. NEXTINT is not written a second time.
    LC6 = 6, /// As LC5, and the length counts four-octet elements.
    LC7 = 7  /// As LC5, and the length counts eight-octet elements.
}

/// PID_LIST_END. XTypes 1.3 section 7.4.1.2 Table 31.
enum ushort PidListEndId = 0x3F02;
/// The PID that introduces the twelve-octet parameter header.
enum ushort PidExtendedId = 0x3F01;
/// The highest member id a short parameter header can carry, inclusive.
enum ushort PidShortMaxId = 0x3F00;
/// Bit 14 of a short parameter id. XTypes 1.3 section 7.4.1.2.
private enum ushort PidFlagMustUnderstand = 0x4000;
/// Bit 30 of the eMemberHeader inside an extended parameter header — not bit 31,
/// which is the implementation-extension flag.
private enum uint EmFlagMustUnderstand = 0x4000_0000;

/// A refusal, thrown rather than putting a plausible but wrong octet on the
/// wire. Every message names the clause it comes from.
class WireException : Exception {
    this(string msg) { super(msg); }
}

// --- Writer: a ubyte buffer + endian; alignment relative to the origin -------

struct Writer {
    ubyte[] buf;
    Endian endian;
    /// The alignment cap, from the Encoding the writer was made with.
    int maxAlign;
    /// The position alignment is measured from. Zero for the stream, and moved
    /// to the start of a PL_CDR1 parameter's value for the length of that
    /// parameter — section 7.4.1.2 aligns a parameter's value against its own
    /// block and not against the stream. Without it a `double` in a parameter
    /// takes four octets of padding and the header declares twelve where
    /// Cyclone, RTI and Fast DDS all declare eight.
    int origin;

    /// Both arguments are required. The encoding decides MAXALIGN and nothing in
    /// the call may leave it implied.
    this(Endian e, Encoding enc) {
        endian = e;
        maxAlign = .maxAlign(enc);
        origin = 0;
    }

    void alignTo(int a) {
        int cap = a < maxAlign ? a : maxAlign;
        int pad = (cap - ((cast(int) buf.length - origin) % cap)) % cap;
        foreach (_; 0 .. pad) buf ~= 0;
    }

    // le: the value already encoded little-endian; reversed for BE.
    void put(int a, ubyte[] le) {
        alignTo(a);
        if (endian == Endian.BE)
            foreach_reverse (b; le) buf ~= b;
        else
            foreach (b; le) buf ~= b;
    }

    // The integer writes take the *wire width* and no longer an `int`. They used
    // to mask — `cast(ubyte)(v & 0xff)` and `cast(ubyte) v, cast(ubyte)(v >> 8)`
    // — so `putU16(70000)` wrote `7011` and returned, which is the silent
    // truncation nine of the fifteen bindings shared. D refuses an out-of-range
    // literal and an implicit narrowing at compile time, so there is no spelling
    // of it left.
    void putU8(ubyte v) { buf ~= v; }
    void putU16(ushort v) { put(2, [cast(ubyte) v, cast(ubyte)(v >> 8)]); }

    void putU32(uint v) {
        put(4, [cast(ubyte) v, cast(ubyte)(v >> 8), cast(ubyte)(v >> 16), cast(ubyte)(v >> 24)]);
    }

    /// A ulong at its natural eight-octet alignment, which `maxAlign` then caps:
    /// eight under XCDR1 and four under XCDR2. XTypes 1.3 section 7.4.1.
    void putU64(ulong v) {
        ubyte[] b;
        foreach (i; 0 .. 8) b ~= cast(ubyte)(v >> (8 * i));
        put(8, b);
    }

    void putF32(float v) {
        uint bits = *cast(uint*)&v;
        put(4, [cast(ubyte) bits, cast(ubyte)(bits >> 8), cast(ubyte)(bits >> 16), cast(ubyte)(bits >> 24)]);
    }

    /// A double as its IEEE-754 bit pattern, at the same alignment as putU64.
    /// `double` is one of the fifteen primitives of XTypes 1.3 section 7.2.2.2
    /// and had no writer here at all.
    void putF64(double v) {
        putU64(*cast(ulong*)&v);
    }

    /// A boolean as the one octet XTypes 1.3 section 7.4.1.1.1 Table 30 gives
    /// it: 0 or 1, never any other pattern.
    void putBool(bool v) { putU8(v ? 1 : 0); }

    void putString(string s) {
        putU32(cast(uint)(s.length + 1));
        foreach (c; s) buf ~= cast(ubyte) c;
        putU8(0);
    }

    void putSeqU8(ubyte[] b) {
        putU32(cast(uint) b.length);
        foreach (x; b) buf ~= x;
    }

    /// Raw octets, no alignment and no length word.
    void putBytes(const(ubyte)[] b) {
        foreach (x; b) buf ~= x;
    }

    /// An IDL `char`: the one octet XTypes 1.3 section 7.4.1.1.1 Table 30 gives
    /// it.
    ///
    /// A code point above U+00FF is refused and not truncated. U+0161 has 0x61
    /// as its low octet, which is `a`: a truncating writer produces a frame of
    /// the right length holding a different, plausible character, and nothing
    /// downstream can tell. The Rust core refuses `primitive-char-above-latin1`
    /// in the same words.
    void putChar(uint codePoint) {
        import std.conv : to;
        if (codePoint > 0xFF)
            throw new WireException("expected a character below U+0100, found U+"
                                    ~ to!string(codePoint, 16));
        putU8(cast(ubyte) codePoint);
    }

    /// An IDL `wchar`: one two-octet code unit, 2-aligned, in the stream's byte
    /// order.
    ///
    /// The four policy axes a wide character carries are answered here and not
    /// by the caller: the unit is two octets wide, it goes out in the stream's
    /// byte order, there is no GIOP length octet before it, and no byte-order
    /// mark. Measured against `wchar_member`, where Cyclone and Fast DDS write
    /// `6100 ab` little-endian under both encodings and RTI writes the same
    /// under XCDR2; RTI's XCDR1 `wchar_t` is four octets wide (`61000000 ab`)
    /// and is the one vendor answer this does not reproduce.
    ///
    /// A code point above U+FFFF is refused. The refusal is width-independent —
    /// a four-octet form would zero-extend a code *unit*, not a code point —
    /// which is how the Rust core refuses `primitive-wchar-above-bmp`.
    void putWchar(uint codePoint) {
        import std.conv : to;
        if (codePoint > 0xFFFF)
            throw new WireException("expected a BMP character, found U+"
                                    ~ to!string(codePoint, 16));
        putU16(cast(ushort) codePoint);
    }

    /// An IDL `wstring` from its UTF-16 code units.
    ///
    /// The length word counts **octets** and not code units, there is no
    /// terminator, there is no byte-order mark, and each unit goes out two
    /// octets wide in the stream's own byte order. Those are the four axes, and
    /// they are the binding's answer — the caller hands over units and nothing
    /// else.
    ///
    /// Measured against `wstring_member` (`04000000 6100 6200 ab`, Cyclone under
    /// both encodings and RTI under XCDR2) and `wstring_nonbmp`
    /// (`04000000 34d8 1edd ab`, the same two). Fast DDS writes the unit *count*
    /// there and truncates the non-BMP code point to a single unit; neither is
    /// reproduced.
    void putWstring(const(ushort)[] units) {
        putU32(cast(uint)(units.length * 2));
        foreach (u; units) putU16(u);
    }

    /// A `bitset` as its anchor word: the holder the declared bits pack into,
    /// written at the anchor's own width and therefore at the anchor's own
    /// alignment — eight under XCDR1 and four under XCDR2 for an eight-octet
    /// anchor. XTypes 1.3 sections 7.3.1.4 and 7.4.1.
    ///
    /// `bits` is the packed holder and `anchor` its width in octets, both
    /// properties of the declared type. What this call decides is the pair the
    /// type does not: which write the holder goes out through, and what that
    /// write aligns to.
    void putBitset(ulong bits, int anchor) {
        import std.conv : to;
        switch (anchor) {
            case 1: putU8(cast(ubyte) bits); break;
            case 2: putU16(cast(ushort) bits); break;
            case 4: putU32(cast(uint) bits); break;
            case 8: putU64(bits); break;
            default:
                throw new WireException("a bitset anchor is one, two, four or eight octets, not "
                                        ~ to!string(anchor));
        }
    }

    /// An IDL `fixed<digits,scale>` as packed BCD; see `fixedOctets`.
    void putFixed(string value, int digits, int scale) {
        putBytes(fixedOctets(value, digits, scale));
    }

    // --- back-patched framing -------------------------------------------
    //
    // A DHEADER, a NEXTINT and a PL_CDR1 parameter length are all the same
    // shape: a slot reserved before the body and filled in once the body's
    // length is known. `patchU32` is the one place that writes into an
    // already-written position, and it honours the wire byte order like every
    // other write.

    private void patchU32(int at, uint v) {
        ubyte[4] le = [cast(ubyte) v, cast(ubyte)(v >> 8), cast(ubyte)(v >> 16), cast(ubyte)(v >> 24)];
        foreach (i; 0 .. 4) buf[at + i] = endian == Endian.BE ? le[3 - i] : le[i];
    }

    private void patchU16(int at, ushort v) {
        ubyte[2] le = [cast(ubyte) v, cast(ubyte)(v >> 8)];
        foreach (i; 0 .. 2) buf[at + i] = endian == Endian.BE ? le[1 - i] : le[i];
    }

    /// Reserve a 4-aligned DHEADER slot and return the position the body starts
    /// at, to hand back to `dheaderEnd`. XTypes 1.3 section 7.4.3.5.
    ///
    /// `@appendable` is the default extensibility of XTypes (section
    /// 7.3.1.2.1.8), so a struct written without an annotation had no form here
    /// at all.
    int dheaderBegin() {
        putU32(0);
        return cast(int) buf.length;
    }

    /// Back-patch the DHEADER with the byte length written since `bodyStart`.
    /// The DHEADER itself is not counted — section 7.4.3.5 measures the content
    /// that follows it.
    void dheaderEnd(int bodyStart) {
        patchU32(bodyStart - 4, cast(uint)(cast(int) buf.length - bodyStart));
    }

    /// The EMHEADER for one member of a `@mutable` aggregate, returning the
    /// body-start position. XTypes 1.3 section 7.4.3.4.2:
    ///
    ///   EMHEADER = (must_understand << 31) | (length_code << 28) | member_id
    ///
    /// with a 28-bit member id. For LC4 a NEXTINT slot is reserved after it and
    /// `emHeaderEnd` fills it in; for LC0..LC3 the code alone gives the length
    /// and nothing follows; for LC5..LC7 the body's own leading four octets are
    /// the length, so no separate NEXTINT is written — writing it twice would
    /// put every following member at the wrong offset.
    int emHeaderBegin(uint id, bool mustUnderstand, LengthCode lc) {
        import std.conv : to;
        if (id > 0x0FFF_FFFF)
            throw new WireException("member id " ~ to!string(id)
                                    ~ " does not fit the 28 bits of an EMHEADER");
        uint header = (cast(uint) lc << 28) | id;
        if (mustUnderstand) header |= 1u << 31;
        putU32(header);
        if (lc == LengthCode.LC4) putU32(0);
        return cast(int) buf.length;
    }

    /// Close the member `emHeaderBegin` opened.
    ///
    /// It takes the length code back rather than remembering it, so a caller
    /// that paired an LC4 begin with a differently-coded end gets a refusal
    /// instead of a NEXTINT patched over the first four octets of a member body.
    void emHeaderEnd(int bodyStart, LengthCode lc) {
        import std.conv : to;
        int written = cast(int) buf.length - bodyStart;
        final switch (lc) {
            case LengthCode.LC4:
                patchU32(bodyStart - 4, cast(uint) written);
                break;
            // 1, 2, 4, 8 — the code *is* the length, so a body of any other size
            // means the plan and the member disagree. Silence here would put a
            // length on the wire that no octet count backs.
            case LengthCode.LC0:
            case LengthCode.LC1:
            case LengthCode.LC2:
            case LengthCode.LC3:
                int want = 1 << cast(int) lc;
                if (written != want)
                    throw new WireException("LC" ~ to!string(cast(int) lc) ~ " frames "
                        ~ to!string(want) ~ " octets and the member wrote " ~ to!string(written));
                break;
            case LengthCode.LC5:
            case LengthCode.LC6:
            case LengthCode.LC7:
                if (written < 4)
                    throw new WireException("LC" ~ to!string(cast(int) lc)
                        ~ " takes its length from the body's first four octets and the member wrote "
                        ~ to!string(written));
                break;
        }
    }

    /// A PL_CDR1 short parameter header. XTypes 1.3 section 7.4.1.2: at a
    /// four-octet boundary an `unsigned short` parameter id, then an `unsigned
    /// short` byte length of the value that follows. Both go out in the stream's
    /// byte order.
    ///
    /// The must-understand flag is bit 14 of the id field. Bit 15 is the
    /// implementation-extension flag and is not set here.
    ///
    /// The parameter's value aligns against the block and not against the
    /// stream, so the alignment origin moves for the length of the parameter and
    /// `pidEnd` restores it. Measured on `ext_mutable` under XCDR1, where the
    /// `double` member's header declares eight in Cyclone, RTI and Fast DDS
    /// alike — a stream-relative alignment would declare twelve.
    Pid pidBegin(ushort pid, bool mustUnderstand) {
        import std.conv : to;
        if (pid > PidShortMaxId)
            throw new WireException("member id " ~ to!string(pid)
                                    ~ " needs the extended parameter header");
        alignTo(4);
        ushort id = pid;
        if (mustUnderstand) id |= PidFlagMustUnderstand;
        put(2, [cast(ubyte) id, cast(ubyte)(id >> 8)]);
        Pid header = Pid(cast(int) buf.length, false, origin);
        buf ~= 0;
        buf ~= 0;
        origin = cast(int) buf.length;
        return header;
    }

    /// The twelve-octet parameter header XTypes 1.3 section 7.4.1.2 requires for
    /// a member id above 0x3F00 or a value longer than 65535 octets:
    ///
    ///   [PID_EXTENDED | must-understand : u16][8 : u16][eMemberHeader : u32][length : u32]
    ///
    /// The must-understand flag on the marker is not the member's — it is on the
    /// marker under this framing whatever the member says, and the member's own
    /// flag is bit 30 of the eMemberHeader. Reading it off the marker instead
    /// makes every parameter must-understand.
    Pid pidExtendedBegin(uint memberId, bool mustUnderstand) {
        import std.conv : to;
        if (memberId > 0x0FFF_FFFF)
            throw new WireException("member id " ~ to!string(memberId)
                                    ~ " does not fit the 28 bits of an eMemberHeader");
        alignTo(4);
        ushort marker = PidExtendedId | PidFlagMustUnderstand;
        put(2, [cast(ubyte) marker, cast(ubyte)(marker >> 8)]);
        put(2, [cast(ubyte) 8, cast(ubyte) 0]);
        uint h = memberId;
        if (mustUnderstand) h |= EmFlagMustUnderstand;
        put(4, [cast(ubyte) h, cast(ubyte)(h >> 8), cast(ubyte)(h >> 16), cast(ubyte)(h >> 24)]);
        Pid header = Pid(cast(int) buf.length, true, origin);
        foreach (_; 0 .. 4) buf ~= 0;
        origin = cast(int) buf.length;
        return header;
    }

    /// Close the parameter `pidBegin` or `pidExtendedBegin` opened.
    ///
    /// The declared length counts the value's octets **and** the padding that
    /// brings the next header back to a four-octet boundary: a reader skips
    /// `length` from the end of the header and has to land on the next
    /// parameter. The padding itself is written after the length is patched, so
    /// the two cannot disagree. Cyclone and RTI round the same way; Fast DDS
    /// declares the value's own length and pads outside it.
    void pidEnd(Pid header) {
        import std.conv : to;
        int width = header.extended ? 4 : 2;
        int length = cast(int) buf.length - header.at - width;
        // The parameter's own origin is still in force here, so the round-up is
        // measured from the block that the length describes.
        int past = (cast(int) buf.length - header.savedOrigin) % 4;
        if (past != 0) length += 4 - past;
        if (header.extended) {
            patchU32(header.at, cast(uint) length);
        } else {
            if (length > 0xFFFF)
                throw new WireException("a short parameter header describes at most 65535 octets, not "
                                        ~ to!string(length));
            patchU16(header.at, cast(ushort) length);
        }
        origin = header.savedOrigin;
        alignTo(4);
    }

    /// The sentinel that closes a PL_CDR1 parameter list: the id PID_LIST_END
    /// with a zero length, four-aligned. XTypes 1.3 section 7.4.1.2.
    ///
    /// The must-understand flag is set on the sentinel — 0x7F02 on the wire,
    /// which is what Cyclone and RTI write (`ext_mutable` XCDR1 LE ends
    /// `027f0000`). Fast DDS writes the bare 0x3F02; the flag is not the
    /// sentinel's meaning either way, and this is the majority form the Rust
    /// column records.
    void pidListEnd() {
        alignTo(4);
        ushort id = PidListEndId | PidFlagMustUnderstand;
        put(2, [cast(ubyte) id, cast(ubyte)(id >> 8)]);
        put(2, [cast(ubyte) 0, cast(ubyte) 0]);
    }

    ubyte[] bytes() { return buf; }
}

/// An open PL_CDR1 parameter header, to be handed back to `Writer.pidEnd`.
struct Pid {
    int at;          /// The position of the length field.
    bool extended;   /// Whether the length field is four octets or two.
    int savedOrigin; /// The alignment origin to restore when the parameter closes.
}

/// The BCD packing `Writer.putFixed` writes, apart so it can be measured at the
/// edges of its range without a Writer. CDR (OMG CORBA 3.3) section 9.3.2.8 /
/// IDL 4.2 section 7.4.1.4.5.
///
/// * the integer part is left-padded and the fraction right-padded to exactly
///   `digits` decimal digits, with the point implied `scale` from the right; the
///   scale itself is not on the wire,
/// * a leading zero nibble is prepended when `digits + 1` is odd, so digits and
///   sign pair up,
/// * two nibbles to the octet, most significant digit first, high nibble first,
/// * the last nibble is the sign: 0xD negative, 0xC otherwise. A negative zero
///   goes out positive, because there is one zero to write,
/// * alignment is one and no multi-octet integer is written, so the byte order
///   never enters.
///
/// The octet count is therefore ceil((digits + 1) / 2), whatever the value.
ubyte[] fixedOctets(string value, int digits, int scale) {
    import std.conv : to;
    if (digits < 1 || digits > 31)
        throw new WireException("fixed<" ~ to!string(digits) ~ "," ~ to!string(scale)
                                ~ ">: a digit count is 1..31");
    if (scale < 0 || scale > digits)
        throw new WireException("fixed<" ~ to!string(digits) ~ "," ~ to!string(scale)
                                ~ ">: a scale is 0..digits");
    string text = value;
    bool negative = false;
    if (text.length > 0 && (text[0] == '+' || text[0] == '-')) {
        negative = text[0] == '-';
        text = text[1 .. $];
    }
    string integral = text;
    string fraction = "";
    foreach (i, c; text) {
        if (c == '.') {
            integral = text[0 .. i];
            fraction = text[i + 1 .. $];
            break;
        }
    }
    if (integral.length == 0 && fraction.length == 0)
        throw new WireException("fixed<" ~ to!string(digits) ~ "," ~ to!string(scale)
                                ~ ">: `" ~ value ~ "` has no digits");
    foreach (part; [integral, fraction])
        foreach (c; part)
            if (c < '0' || c > '9')
                throw new WireException("fixed<" ~ to!string(digits) ~ "," ~ to!string(scale)
                                        ~ ">: `" ~ value ~ "` is not a decimal");
    // Leading zeros carry no value, so they are dropped before the width is
    // checked: `000000.001` is one significant integer digit, not six.
    size_t lead = 0;
    while (lead < integral.length && integral[lead] == '0') lead++;
    integral = integral[lead .. $];
    if (integral.length > digits - scale)
        throw new WireException("fixed<" ~ to!string(digits) ~ "," ~ to!string(scale)
            ~ ">: `" ~ value ~ "` needs " ~ to!string(integral.length)
            ~ " integer digits and the type has " ~ to!string(digits - scale));
    // A trailing zero in the fraction is padding either way, so it is dropped
    // before the width is checked and re-added below.
    while (fraction.length > scale && fraction[$ - 1] == '0')
        fraction = fraction[0 .. $ - 1];
    if (fraction.length > scale)
        throw new WireException("fixed<" ~ to!string(digits) ~ "," ~ to!string(scale)
            ~ ">: `" ~ value ~ "` needs " ~ to!string(fraction.length)
            ~ " fraction digits and the type has " ~ to!string(scale));
    ubyte[] nibbles;
    if ((digits + 1) % 2 == 1) nibbles ~= 0;
    foreach (_; 0 .. digits - scale - cast(int) integral.length) nibbles ~= 0;
    foreach (c; integral) nibbles ~= cast(ubyte)(c - '0');
    foreach (c; fraction) nibbles ~= cast(ubyte)(c - '0');
    foreach (_; 0 .. scale - cast(int) fraction.length) nibbles ~= 0;
    bool nonZero = false;
    foreach (n; nibbles) if (n != 0) nonZero = true;
    nibbles ~= negative && nonZero ? cast(ubyte) 0x0D : cast(ubyte) 0x0C;
    ubyte[] outp;
    for (size_t i = 0; i + 1 < nibbles.length; i += 2)
        outp ~= cast(ubyte)((nibbles[i] << 4) | nibbles[i + 1]);
    return outp;
}

// --- Reader: a byte cursor ---

struct Reader {
    const(ubyte)[] data;
    int pos;
    Endian endian;
    /// The alignment cap, from the Encoding the reader was made with — the same
    /// cap the writer used, or the fields land elsewhere.
    int maxAlign;
    /// Mirrors the writer's: a PL_CDR1 parameter's value aligns against its own
    /// block. Section 7.4.1.2.
    int origin;

    this(const(ubyte)[] d, Endian e, Encoding enc) {
        data = d;
        endian = e;
        maxAlign = .maxAlign(enc);
        origin = 0;
    }

    ubyte[] take(int a, int n) {
        int cap = a < maxAlign ? a : maxAlign;
        int pad = (cap - ((pos - origin) % cap)) % cap;
        pos += pad;
        ubyte[] chunk = data[pos .. pos + n].dup;
        pos += n;
        if (endian == Endian.BE) {
            ubyte[] rev;
            foreach_reverse (b; chunk) rev ~= b;
            return rev;
        }
        return chunk;
    }

    uint getU32() {
        auto b = take(4, 4);
        return cast(uint) b[0] | (cast(uint) b[1] << 8) | (cast(uint) b[2] << 16) | (cast(uint) b[3] << 24);
    }

    // --- primitives beyond getU32 — the byte-exact inverse of the Writer ---
    ubyte getU8() {
        ubyte b = data[pos];
        pos += 1;
        return b;
    }
    ushort getU16() {
        auto b = take(2, 2);
        return cast(ushort)(cast(ushort) b[0] | (cast(ushort) b[1] << 8));
    }
    /// A ulong at its natural eight-octet alignment, which `maxAlign` then caps.
    /// It passed 4 here while the writer passed 8, which under XCDR1 read the
    /// four octets of padding as the value's low half and returned a different
    /// number with no error anywhere.
    ulong getU64() {
        auto b = take(8, 8);
        ulong v = 0;
        foreach (i; 0 .. 8) v |= (cast(ulong) b[i]) << (8 * i);
        return v;
    }
    float getF32() {
        uint bits = getU32();
        return *cast(float*)&bits;
    }
    /// A double from its IEEE-754 bit pattern.
    double getF64() {
        ulong bits = getU64();
        return *cast(double*)&bits;
    }
    /// The one octet a boolean occupies.
    bool getBool() { return getU8() != 0; }
    /// The one octet an IDL `char` occupies.
    uint getChar() { return getU8(); }
    /// One two-octet `wchar` code unit.
    uint getWchar() { return getU16(); }
    /// A `wstring` back into its UTF-16 code units. The length word counts
    /// octets, so the unit count is half of it.
    ushort[] getWstring() {
        uint octets = getU32();
        ushort[] units;
        foreach (_; 0 .. octets / 2) units ~= getU16();
        return units;
    }
    /// A `bitset` anchor of the given width, back into its holder word.
    ulong getBitset(int anchor) {
        import std.conv : to;
        switch (anchor) {
            case 1: return getU8();
            case 2: return getU16();
            case 4: return getU32();
            case 8: return getU64();
            default:
                throw new WireException("a bitset anchor is one, two, four or eight octets, not "
                                        ~ to!string(anchor));
        }
    }
    /// A `fixed<digits,scale>` back into the decimal it was written from: a
    /// leading `-` when the sign nibble is 0xD, `digits - scale` integer digits,
    /// and a `.` with `scale` fraction digits where the scale is not zero.
    ///
    /// The scale is not on the wire, so it is an argument here for the same
    /// reason it is one in `putFixed`.
    string getFixed(int digits, int scale) {
        import std.conv : to;
        if (digits < 1 || digits > 31)
            throw new WireException("fixed<" ~ to!string(digits) ~ "," ~ to!string(scale)
                                    ~ ">: a digit count is 1..31");
        if (scale < 0 || scale > digits)
            throw new WireException("fixed<" ~ to!string(digits) ~ "," ~ to!string(scale)
                                    ~ ">: a scale is 0..digits");
        int octets = (digits + 2) / 2;
        ubyte[] nibbles;
        foreach (_; 0 .. octets) {
            ubyte b = getU8();
            nibbles ~= cast(ubyte)(b >> 4);
            nibbles ~= cast(ubyte)(b & 0x0F);
        }
        ubyte sign = nibbles[$ - 1];
        auto body_ = nibbles[$ - 1 - digits .. $ - 1];
        string outp = sign == 0x0D ? "-" : "";
        foreach (i, n; body_) {
            if (scale != 0 && i == digits - scale) outp ~= '.';
            outp ~= cast(char)('0' + n);
        }
        return outp;
    }
    string getString() {
        uint n = getU32();
        string s = cast(string) data[pos .. pos + n - 1].idup;
        pos += n;
        return s;
    }
    ubyte[] getSeqU8() {
        uint n = getU32();
        ubyte[] b = data[pos .. pos + n].dup;
        pos += n;
        return b;
    }

    /// Read a DHEADER and return the announced body byte length. Section
    /// 7.4.3.5.
    uint dheaderRead() { return getU32(); }

    /// Read a member header. LC0..LC3 take the extent from the code. LC4
    /// consumes the NEXTINT that follows the header, and the extent is that
    /// word. LC5..LC7 consume nothing: the next four octets are the member's own
    /// leading word and belong to the body, so they are peeked and the extent is
    /// 4 + the word for LC5, 4 + 4*word for LC6 and 4 + 8*word for LC7 — the
    /// last two count elements, not octets.
    Member emHeaderRead() {
        uint header = getU32();
        LengthCode lc = cast(LengthCode)((header >> 28) & 0x7);
        uint extent = 0;
        final switch (lc) {
            case LengthCode.LC0:
            case LengthCode.LC1:
            case LengthCode.LC2:
            case LengthCode.LC3:
                extent = 1u << cast(int) lc;
                break;
            case LengthCode.LC4:
                extent = getU32();
                break;
            case LengthCode.LC5:
            case LengthCode.LC6:
            case LengthCode.LC7:
                int at = pos;
                uint word = getU32();
                pos = at;
                extent = lc == LengthCode.LC5 ? 4 + word
                       : lc == LengthCode.LC6 ? 4 + 4 * word
                       : 4 + 8 * word;
                break;
        }
        return Member(header & 0x0FFF_FFFF, (header & (1u << 31)) != 0, lc, extent);
    }

    /// Read a PL_CDR1 parameter header in either form.
    ///
    /// The extended form is recognised by the marker PID_EXTENDED, and the
    /// must-understand answer then comes from bit 30 of the eMemberHeader and
    /// never from the marker — the marker carries the flag under this framing
    /// whatever the member says.
    ///
    /// The value's alignment origin is moved to the position this call leaves,
    /// and `pidSkip` restores it.
    Parameter pidRead() {
        ushort raw = getU16();
        ushort shortId = cast(ushort)(raw & 0x3FFF);
        if (shortId == PidListEndId) {
            getU16();
            return Parameter(0, false, 0, true);
        }
        if (shortId == PidExtendedId) {
            getU16(); // the slength, always 8
            uint header = getU32();
            uint length = getU32();
            origin = pos;
            return Parameter(header & 0x0FFF_FFFF,
                             (header & EmFlagMustUnderstand) != 0, length, false);
        }
        uint length = getU16();
        origin = pos;
        return Parameter(shortId, (raw & PidFlagMustUnderstand) != 0, length, false);
    }

    /// Move the cursor past a parameter value of `length` octets from
    /// `bodyStart` and restore the stream's alignment origin.
    void pidSkip(int bodyStart, uint length) {
        pos = bodyStart + cast(int) length;
        origin = 0;
        while (pos % 4 != 0) pos++;
    }
}

/// What `Reader.emHeaderRead` recovered.
struct Member {
    uint id;
    bool mustUnderstand;
    LengthCode lc;
    /// The member body's extent in octets, from the position the read left the
    /// cursor at.
    uint extent;
}

/// What `Reader.pidRead` recovered.
struct Parameter {
    uint id;
    bool mustUnderstand;
    uint length; /// The declared byte length of the value.
    bool listEnd; /// Whether the list ends here.
}

// --- XRCE WRITE_DATA framing ---

enum ubyte SessionNoKey = 0x80;
enum ubyte StreamBestEffort = 0x01;
// E flag (little-endian body) + DataFormat FORMAT_DATA (0b000) in bits 1..3 --
// DDS-XRCE 1.0 section 8.3.5.8. FORMAT_SAMPLE (0b001, i.e. 0x03) would
// additionally require a SampleInfo in the body, which this profile does not
// carry.
enum ubyte XrceWriteFlags = 0x01;
// Wire size of the BaseObjectRequest that precedes the sample (section 7.7.8).
enum size_t XrceBaseSize = 4;
// OBJK_DATAWRITER -- the ObjectKind of a DataWriter (section 7.2.2).
enum ushort ObjkDataWriter = 0x05;

/// An ObjectId (section 7.2.1) from a 12-bit raw id and a 4-bit ObjectKind.
ushort xrceObjectId(ushort rawId, ushort kind) @safe pure nothrow @nogc {
    return cast(ushort)(((rawId & 0x0FFF) << 4) | (kind & 0x0F));
}

/// A sample the 16-bit XRCE submessage_length cannot describe.
class SampleTooLargeException : Exception {
    this(size_t n) {
        import std.conv : to;
        super(to!string(n) ~ " octets: the 16-bit XRCE submessage_length "
              ~ "describes at most 65535");
    }
}

// The 16-bit submessage_length field cannot describe a sample larger than
// 0xFFFF, so a sample above it has no WRITE_DATA form: refuse rather than
// truncate the length while appending the full payload (the C SDK's
// zdw_xrce_write_frame rejects the same way).
//
// Thrown and no longer a null slice. `Client.write` handed the null to
// `deliver` without looking, so `writeFrame(SessionNoKey, StreamBestEffort, 1,
// new ubyte[70000]) is null` went out as an empty datagram with no error
// anywhere.
// The submessage body is the BaseObjectRequest (section 7.7.8: request_id then
// object_id, each two octets most significant first) followed by the sample.
// `objectId` names the target DataWriter -- without it no XRCE agent can route
// the sample. The request_id is the message's own `seqNo`, which section 7.7.8
// only requires to be unique per stream.
ubyte[] writeFrame(ubyte session, ubyte stream, int seqNo, ushort objectId,
                   const(ubyte)[] sample) {
    size_t bodyLen = XrceBaseSize + sample.length;
    if (bodyLen > 0xFFFF)
        throw new SampleTooLargeException(sample.length);
    ubyte[] r = [
        session, stream,
        cast(ubyte)(seqNo & 0xff), cast(ubyte)((seqNo >> 8) & 0xff),
        0x07, XrceWriteFlags,
        cast(ubyte)(bodyLen & 0xff), cast(ubyte)((bodyLen >> 8) & 0xff),
        cast(ubyte)((seqNo >> 8) & 0xff), cast(ubyte)(seqNo & 0xff),
        cast(ubyte)((objectId >> 8) & 0xff), cast(ubyte)(objectId & 0xff)
    ];
    foreach (x; sample) r ~= x;
    return r;
}

// Frame layout: [session, stream, seq_lo, seq_hi, sm_id, flags, len_lo, len_hi]
// then the sample body. The 16-bit little-endian submessage_length (bytes 6..7)
// bounds the body exactly: frame[8 .. 8+smLen], never frame[8 .. $], so trailing
// padding or an appended submessage is not folded into the sample.
//
// Accepts WRITE_DATA (0x07, endpoint->hub / loopback) and DATA (0x09,
// hub->endpoint) — the hub pushes samples to a client with DATA. See DDS-XRCE
// spec section 8.3.5. Returns null for a short header, a wrong submessage id, or
// a declared length that runs past the datagram (truncation / wrong length).
ubyte[] readFrame(const(ubyte)[] frame) {
    if (frame.length < 8 + XrceBaseSize || (frame[4] != 0x07 && frame[4] != 0x09))
        return null;
    size_t smLen = cast(size_t)frame[6] | (cast(size_t)frame[7] << 8);
    if (smLen < XrceBaseSize || 8 + smLen > frame.length)
        return null;
    return frame[8 + XrceBaseSize .. 8 + smLen].dup;
}

// `(requestId, objectId)` of a data frame (section 7.7.8). Returns false under
// the same rules as `readFrame`.
bool frameBase(const(ubyte)[] frame, ref ushort requestId, ref ushort objectId) {
    if (readFrame(frame) is null)
        return false;
    requestId = cast(ushort)((frame[8] << 8) | frame[9]);
    objectId = cast(ushort)((frame[10] << 8) | frame[11]);
    return true;
}

// --- transport ---

struct Transport {
    void delegate(ubyte[]) deliver;
    ubyte[] delegate() receive; // null if nothing
}

// --- sync client ---

class Client {
    Transport transport;
    ubyte session = SessionNoKey;
    ubyte stream = StreamBestEffort;
    int seqNo = 1;

    // ObjectId of the target DataWriter (section 7.7.8 BaseObjectRequest).
    ushort objectId;

    this(Transport t, ushort objectId) { transport = t; this.objectId = objectId; }

    // A sample the envelope cannot carry throws out of `writeFrame`, so the
    // sequence is not advanced and nothing reaches the transport.
    void write(ubyte[] sample) {
        auto frame = writeFrame(session, stream, seqNo, objectId, sample);
        transport.deliver(frame);
        seqNo = (seqNo + 1) & 0xffff;
    }

    // One non-blocking receive: returns the sample body, or null.
    ubyte[] poll() {
        auto f = transport.receive();
        if (f is null) return null;
        return readFrame(f);
    }
}

// In-memory FIFO transport for tests + example (sync path).
Transport memTransport() {
    ubyte[][] q;
    Transport t;
    t.deliver = (ubyte[] frame) { q ~= frame.dup; };
    t.receive = () {
        if (q.length) { auto f = q[0]; q = q[1 .. $]; return f; }
        return cast(ubyte[]) null;
    };
    return t;
}

// --- async reader: a std.concurrency actor (spawn/send/receive) ---

// The reader thread receives raw frames as messages and sends decoded sample
// bodies back to its owner. Message passing is D's idiomatic concurrency; the
// payloads are immutable so they cross the thread boundary safely.
void readerLoop(Tid owner) {
    bool running = true;
    while (running) {
        receive(
            (immutable(ubyte)[] frame) {
                auto b = readFrame(frame);
                if (b.length) send(owner, b.idup);
            },
            (bool _stop) { running = false; }
        );
    }
}

class AsyncReader {
    Tid tid;

    this() { tid = spawn(&readerLoop, thisTid); }

    void feed(ubyte[] frame) { send(tid, frame.idup); }
    immutable(ubyte)[] recv() { return receiveOnly!(immutable(ubyte)[])(); }
    void stop() { send(tid, true); }
}
