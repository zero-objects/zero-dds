// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

/// The D wire runtime: DDS-XTypes 1.3 §7.4 from the clause.
///
/// The counterpart of crates/wire, runtime/c, runtime/python and
/// runtime/go — a further implementation of the same clauses, not a
/// binding to any of them. Where two columns disagree on an octet, one
/// of them has misread the specification, and that is the point of
/// having both. Nothing here decides a type question: which members are
/// written, in what order, under which header and with which length
/// code arrives as arguments from the generated codec.
///
/// The operations take the shared walk's argument order — WriteEnum
/// (bitBound, value), WriteBitset(anchor, value) — and the walk's
/// spelling raised to one word per segment, because this runtime was
/// written against the walk rather than adapted to it afterwards.
///
/// A malformed stream or a value outside its type throws a WireException
/// and unwinds to the harness, which turns it into the verdict's exit
/// code; a refusal about the *type* sets the reader's status instead and
/// the generated decoder walks on to the end of the sample.
///
/// An IDL `long double` is held as decimal text — D's `real` is 80-bit
/// x87 extended on x86 and plain binary64 on AArch64, and a binary128
/// has 113 significand bits — so the two conversions live here: a
/// correctly rounded parse (to nearest, ties to even, in one rounding)
/// over std.bigint, and the exact decimal expansion in the spelling the
/// sibling columns' renderers produce.
module zdwire;

import std.bigint : BigInt;
import std.conv : to;
import std.typecons : Nullable;

// -- errors ----------------------------------------------------------------

/// Base of every wire failure.
class WireException : Exception
{
    this(string message, string file = __FILE__, size_t line = __LINE__)
    {
        super(message, file, line);
    }
}

/// The stream ended before the value did.
class Truncated : WireException
{
    this(int at, int needed, int available, string file = __FILE__, size_t line = __LINE__)
    {
        super("at " ~ to!string(at) ~ ": " ~ to!string(needed) ~ " bytes needed, "
                ~ to!string(available) ~ " available", file, line);
    }
}

/// A length field claims more than the stream holds. Separate from
/// Truncated: a length that cannot be true is a malformed stream, not a
/// short one.
class ImpossibleLength : WireException
{
    this(int at, int claimed, int available, string file = __FILE__, size_t line = __LINE__)
    {
        super("at " ~ to!string(at) ~ ": a length of " ~ to!string(claimed) ~ " with "
                ~ to!string(available) ~ " bytes left", file, line);
    }
}

/// A string held bytes that are not UTF-8.
class NotUtf8 : WireException
{
    this(int at, string file = __FILE__, size_t line = __LINE__)
    {
        super("at " ~ to!string(at) ~ ": the string is not UTF-8", file, line);
    }
}

/// A value outside what its type admits.
class OutOfRange : WireException
{
    this(int at, string expected, string file = __FILE__, size_t line = __LINE__)
    {
        super("at " ~ to!string(at) ~ ": expected " ~ expected, file, line);
    }
}

// -- policies --------------------------------------------------------------

/// The byte order a stream is written or read in.
enum Endianness
{
    little,
    big,
}

/// What a receiving position does with a value it cannot hold.
/// DDS-XTypes 1.3 §7.2.2.7, Table 11.
enum TryConstruct
{
    discard,
    useDefault,
    trim,
}

/// The walk's spellings, as manifest constants: the shared codec walk
/// emits one constant into every column.
enum TryConstructDiscard = TryConstruct.discard;
/// See `TryConstructDiscard`.
enum TryConstructUseDefault = TryConstruct.useDefault;
/// See `TryConstructDiscard`.
enum TryConstructTrim = TryConstruct.trim;

/// What a reader answers about the type, without throwing.
enum Status
{
    ok,
    truncated,
    impossibleLength,
    outOfRange,
    discarded,
}

/// The walk's spellings.
enum StatusOk = Status.ok;
/// See `StatusOk`.
enum StatusTruncated = Status.truncated;
/// See `StatusOk`.
enum StatusImpossibleLength = Status.impossibleLength;
/// See `StatusOk`.
enum StatusOutOfRange = Status.outOfRange;
/// See `StatusOk`.
enum StatusDiscarded = Status.discarded;

/// A status, spelled the way the sibling runtimes log it.
string statusText(Status status)
{
    final switch (status)
    {
    case Status.ok:
        return "ok";
    case Status.truncated:
        return "truncated";
    case Status.impossibleLength:
        return "impossible-length";
    case Status.outOfRange:
        return "out-of-range";
    case Status.discarded:
        return "discarded";
    }
}

/// Which of §7.4.1.2's two roles a parameter header stands in.
enum ParameterRole
{
    member,
    optional,
}

/// The walk's spellings.
enum ParameterMember = ParameterRole.member;
/// See `ParameterMember`.
enum ParameterOptional = ParameterRole.optional;

/// The parameter ids and flags of XTypes 1.3 §7.4.1.2.2 and
/// DDSI-RTPS 2.5 §9.4.2.11.2.
enum int PidExtended = 0x3F01;
/// Ends a parameter list.
enum int PidListEnd = 0x3F02;
/// The highest member id a short header carries, inclusive.
enum int PidShortMaxID = 0x3F00;
/// FLAG_IMPL_EXTENSION, bit 15 of a short id.
enum int PidImplExtension = 0x8000;
/// The id every consumer shall ignore.
enum int PidIgnore = 0x3F03;
/// The longest value a short header can state.
enum int PidShortMaxLen = 0xFFFF;
/// The must-understand bit of a short id.
enum int PidMustUnderstand = 0x4000;

private enum uint eFlagMustUnderstand = 0x4000_0000;
private enum uint eFlagImplExtension = 0x8000_0000;
private enum uint eMemberIDMask = 0x0FFF_FFFF;
private enum int pidIDMask = 0x3FFF;

/// What flags a PL_CDR1 parameter id carries at `position` — "member",
/// "sentinel" or "extended-marker". An unknown policy throws: a typo in
/// a wire-affecting setting must be loud.
private int pidFlagsAt(string policy, string position)
{
    if (policy == "sentinel-must-understand")
    {
        if (position == "sentinel" || position == "extended-marker")
            return PidMustUnderstand;
        return 0;
    }
    if (policy == "bare")
        return 0;
    if (policy == "impl-extension-members")
    {
        if (position == "member" || position == "extended-marker")
            return PidImplExtension | PidMustUnderstand;
        return 0;
    }
    throw new OutOfRange(0, "a PID-flag policy, not `" ~ policy ~ "`");
}

/// How a PL_CDR1 parameter list is framed — the three ways the measured
/// implementations differ inside an XCDR1 sample.
struct PlCdr1Framing
{
    /// What flags a parameter id carries, asked per position.
    string pidFlags;
    /// The highest member id a short parameter header carries. Both
    /// sides: `WritePid` refuses an id above it rather than putting a
    /// reserved parameter on the wire.
    int highestShortID;
    /// Whether a @mutable member's declared length covers its padding.
    bool padDeclaredLength;
    /// The same question for an @optional member outside @mutable.
    bool padDeclaredOptionalLength;
    /// Whether a member's own must-understand bit goes out at all.
    bool memberMustUnderstand;
    /// Whether the parameter that just ended writes its own trailing
    /// padding to the next four-byte boundary. Every header writer
    /// aligns itself, so the two readings only part at the end of the
    /// stream: RTI pads after a final `@optional` parameter, Fast DDS
    /// does not (`annotation-must-understand-unannotated`, 24 against
    /// 22 octets).
    bool padAfterValue;

    private bool padsDeclaredLength(ParameterRole role) const
    {
        if (role == ParameterRole.optional)
            return padDeclaredOptionalLength;
        return padDeclaredLength;
    }
}

/// One cell of the wstring C1 x C2 x C3 product.
private struct WstringCell
{
    bool countsCharacters;
    bool terminated;
    int width;
}

/// The five named forms and the three coordinate spellings; the
/// spellings are WstringLengthPolicy::as_str's. A second boolean answer
/// says whether the name is a policy at all.
private bool wstringCellOf(string policy, out WstringCell cell)
{
    switch (policy)
    {
    case "specification", "octets/unterminated/2":
        cell = WstringCell(false, false, 2);
        return true;
    case "character-count", "characters/unterminated/2":
        cell = WstringCell(true, false, 2);
        return true;
    case "character-count-terminated", "characters/terminated/4":
        cell = WstringCell(true, true, 4);
        return true;
    case "specification-wide-units", "octets/unterminated/4":
        cell = WstringCell(false, false, 4);
        return true;
    case "ros-type-support", "characters/unterminated/4":
        cell = WstringCell(true, false, 4);
        return true;
    case "octets/terminated/2":
        cell = WstringCell(false, true, 2);
        return true;
    case "octets/terminated/4":
        cell = WstringCell(false, true, 4);
        return true;
    case "characters/terminated/2":
        cell = WstringCell(true, true, 2);
        return true;
    default:
        return false;
    }
}

private bool wstringOrderKnown(string order)
{
    return order == "stream" || order == "big-endian-always"
        || order == "byte-order-mark" || order == "host-unswapped";
}

private enum ushort byteOrderMark = 0xFEFF;
private enum ushort swappedByteOrderMark = 0xFFFE;

// -- handles ---------------------------------------------------------------

/// A DHEADER waiting for its length; `at` below zero is the delimiter a
/// @final aggregate does not write.
struct DHeader
{
    int at = -1;
}

/// The DHEADER a value that takes no delimiter does not write; the
/// writer's FinishDheader ignores it.
DHeader NoDheader()
{
    return DHeader(-1);
}

/// An EMHEADER waiting for its NEXTINT; `nextInt` is -1 where the
/// length code carries the length itself.
struct EmHeader
{
    int nextInt = -1;
    int word = 0;
    /// The code the header was written with, so that `FinishEmheader`
    /// can hold the member's value to the length the code declares.
    /// Codes 0 to 3 are lengths; anything else declares nothing of its
    /// own.
    int lengthCode = 8;
}

/// A parameter header waiting for its length.
struct Pid
{
    int at;
    bool extended;
    int savedOrigin;
    ParameterRole role;
    int pid;
    bool mustUnderstand;
}

/// Everything a two-pass read has to put back.
struct ReaderMark
{
    int at;
    int origin;
    Status status;
}

/// An EMHEADER as read. `Extent` is how many octets the member occupies
/// after the header word, the NEXTINT included. XTypes 1.3 §7.4.3.4.2.
struct EmHeaderRead
{
    int MemberId;
    int LengthCode;
    bool MustUnderstand;
    int NextInt = -1;
    int Extent;
}

/// A parameter header as read.
struct PidRead
{
    int Pid;
    int Length;
    bool End;
    bool MustUnderstand;
    bool ImplExtension;
    bool Reserved;
    bool Ignore;
    ParameterRole Role;
    int SavedOrigin;
    int ValueAt;

    /// Whether this parameter can be a member of the reading type:
    /// false for a vendor-private parameter and for a reserved id.
    bool IsUserMember() const
    {
        return !ImplExtension && !Reserved && !Ignore;
    }
}

// -- helpers the generated code names --------------------------------------

/// A pointer to a copy of `v`: what an `@external` member is assigned
/// with — D has no address-of on an rvalue, and a temporary per site
/// would need a name no two sites in one scope could share.
T* zdPtr(T)(T v)
{
    // An array of one `T` and not `new T`: `new T` has no form for a
    // dynamic array (`new int[]` asks for a length), and an `@external`
    // sequence is one (annotation-external-sequence: "missing length
    // argument for array" instantiating `zdPtr!(int[])`).
    auto block = new T[](1);
    block[0] = v;
    return &block[0];
}

/// A `Nullable` holding `v`: what an `@optional` member is assigned
/// with. A helper of this runtime rather than `std.typecons.nullable`,
/// so a generated codec names one module and a corpus type called
/// `nullable` cannot shadow it.
Nullable!T zdNullable(T)(T v)
{
    return Nullable!T(v);
}

/// C11 §7.24.4's unsigned-octet order, as the sign the walk compares.
/// D's own string order over UTF-8 is exactly that order.
int Strcmp(string a, string b)
{
    if (a < b)
        return -1;
    if (a > b)
        return 1;
    return 0;
}

/// How many octets `text` is, which is what a string<N> bound counts.
int Utf8Octets(string text)
{
    return cast(int) text.length;
}

/// How many UTF-16 code units `text` is, which is what a wstring<N>
/// bound counts. A scalar above the basic plane is two.
int Utf16Units(string text)
{
    int units = 0;
    foreach (dchar c; text)
        units += c > 0xFFFF ? 2 : 1;
    return units;
}

/// `text` cut to at most `octets` bytes, never inside a character.
/// Table 11: the order is preserved, so this is a prefix.
string TrimUtf8(string text, int octets)
{
    import std.utf : stride;

    size_t used = 0;
    size_t end = 0;
    while (end < text.length)
    {
        const size = stride(text, end);
        if (used + size > cast(size_t) octets)
            break;
        used += size;
        end += size;
    }
    return text[0 .. end];
}

/// `text` cut to at most `units` UTF-16 code units.
string TrimUtf16(string text, int units)
{
    import std.utf : stride;

    int used = 0;
    size_t end = 0;
    while (end < text.length)
    {
        size_t probe = end;
        import std.utf : decode;

        const dchar c = decode(text, probe);
        const cost = c > 0xFFFF ? 2 : 1;
        if (used + cost > units)
            break;
        used += cost;
        end = probe;
    }
    return text[0 .. end];
}

/// `point` as the one octet an IDL char is, refused above U+00FF rather
/// than truncated: the refusal is the sample's — the same one the
/// sibling runtimes raise.
ubyte CheckedChar(uint point)
{
    if (point > 0xFF)
        throw new OutOfRange(0, "a character below U+0100");
    return cast(ubyte) point;
}

/// `point` as the one UTF-16 code unit an IDL wchar is, refused outside
/// the BMP. §7.2.2.2.1.2.5.
ushort CheckedWchar(uint point)
{
    if (point > 0xFFFF)
        throw new OutOfRange(0, "a BMP character");
    return cast(ushort) point;
}

/// A 64-bit signed integer in decimal: a JSON number is a double and
/// loses the low bits past 2^53, so both 64-bit widths travel as
/// strings.
string FormatI64(long v)
{
    return to!string(v);
}

/// A 64-bit unsigned integer in decimal.
string FormatU64(ulong v)
{
    return to!string(v);
}

/// Whether a float has a JSON form at all: NaN and the infinities are
/// not JSON.
bool JSONNumberOK(double v)
{
    import std.math : isNaN, isInfinity;

    return !isNaN(v) && !isInfinity(v);
}

/// The UTF-16 code units of `v`, without a BOM. Invalid UTF-8 is
/// refused: writing its replacement character would put a string on the
/// wire the caller never held.
private ushort[] utf16UnitsOf(string v, int at)
{
    ushort[] units;
    try
    {
        foreach (dchar c; v)
        {
            if (c < 0x10000)
            {
                units ~= cast(ushort) c;
            }
            else
            {
                const offset = c - 0x10000;
                units ~= cast(ushort)(0xD800 + (offset >> 10));
                units ~= cast(ushort)(0xDC00 + (offset & 0x3FF));
            }
        }
    }
    catch (Exception)
    {
        throw new OutOfRange(at, "a UTF-8 wstring");
    }
    return units;
}

// -- the writer ------------------------------------------------------------

/// Accumulates the bytes of one sample. Alignment is relative to the
/// start of the stream: XCDR aligns each primitive to its own size
/// counted from the encapsulation. XTypes 1.3 §7.4.1.1.
class Writer
{
    private ubyte[] outBytes;
    private Endianness endianness;
    private int origin = 0;
    private int maxAlign;
    private PlCdr1Framing plCdr1;
    /// Whether an LC4 header whose member turns out to be 1, 2, 4 or 8
    /// bytes is rewritten with a short length code (Fast DDS's form).
    bool shortLengthCodes;

    this(Endianness endianness, int maxAlign, PlCdr1Framing plCdr1, bool shortLengthCodes)
    {
        this.endianness = endianness;
        this.maxAlign = maxAlign < 1 ? 1 : maxAlign;
        this.plCdr1 = plCdr1;
        this.shortLengthCodes = shortLengthCodes;
    }

    /// How many bytes have been written.
    int Position()
    {
        return cast(int) outBytes.length;
    }

    /// The bytes written, as a copy.
    ubyte[] Finish()
    {
        return outBytes.dup;
    }

    private void alignTo(int alignment)
    {
        if (alignment > maxAlign)
            alignment = maxAlign;
        if (alignment <= 1)
            return;
        const offset = cast(int) outBytes.length - origin;
        const padding = (alignment - offset % alignment) % alignment;
        foreach (i; 0 .. padding)
            outBytes ~= 0;
    }

    private void writeRaw(ulong unsigned, int width)
    {
        if (endianness == Endianness.little)
        {
            foreach (i; 0 .. width)
                outBytes ~= cast(ubyte)(unsigned >> (8 * i));
        }
        else
        {
            foreach_reverse (i; 0 .. width)
                outBytes ~= cast(ubyte)(unsigned >> (8 * i));
        }
    }

    private void writeInt(ulong unsigned, int width)
    {
        alignTo(width);
        writeRaw(unsigned, width);
    }

    // -- primitives --------------------------------------------------------

    /// `boolean`: one byte, 0 or 1.
    void WriteBool(bool v)
    {
        outBytes ~= v ? 1 : 0;
    }

    /// `octet` and `uint8`.
    void WriteU8(ubyte v)
    {
        outBytes ~= v;
    }

    /// `int8`.
    void WriteI8(byte v)
    {
        outBytes ~= cast(ubyte) v;
    }

    /// `char`: one octet — which is the whole member.
    void WriteChar(ubyte v)
    {
        outBytes ~= v;
    }

    /// `short`.
    void WriteI16(short v)
    {
        writeInt(cast(ulong) cast(ushort) v, 2);
    }

    /// `unsigned short`.
    void WriteU16(ushort v)
    {
        writeInt(v, 2);
    }

    /// `long` — 32 bits, whatever D calls its own `long`.
    void WriteI32(int v)
    {
        writeInt(cast(ulong) cast(uint) v, 4);
    }

    /// `unsigned long`.
    void WriteU32(uint v)
    {
        writeInt(v, 4);
    }

    /// `long long`.
    void WriteI64(long v)
    {
        writeInt(cast(ulong) v, 8);
    }

    /// `unsigned long long`.
    void WriteU64(ulong v)
    {
        writeInt(v, 8);
    }

    /// `float`.
    void WriteF32(float v)
    {
        writeInt(*cast(uint*)&v, 4);
    }

    /// `double`.
    void WriteF64(double v)
    {
        writeInt(*cast(ulong*)&v, 8);
    }

    /// `long double`: sixteen bytes of IEEE 754 binary128, aligned to
    /// eight (capped by MAXALIGN). The value arrives as decimal text and
    /// is correctly rounded here, once. The two byte orders are the
    /// whole sixteen bytes reversed — CORBA 3.3 part 2 §9.3.1.3.
    void WriteF128(string v)
    {
        const bytes = f128FromDecimal(v, cast(int) outBytes.length);
        alignTo(8);
        if (endianness == Endianness.little)
        {
            foreach (i; 0 .. 16)
                outBytes ~= bytes[i];
        }
        else
        {
            foreach_reverse (i; 0 .. 16)
                outBytes ~= bytes[i];
        }
    }

    // -- characters and strings --------------------------------------------

    /// `wchar`, at `width` octets and in `framing`'s shape.
    /// "giop-length-prefixed" is CORBA 3.3 part 2 §9.3.1.6's GIOP 1.2
    /// form: the octet count first, then the code unit's octets, all
    /// aligned to one; every order but "stream" means big-endian there.
    void WriteWchar(ushort v, int width, string framing, string order)
    {
        if (width != 2 && width != 4)
            throw new OutOfRange(cast(int) outBytes.length, "a wchar width of 2 or 4");
        if (framing == "giop-length-prefixed")
        {
            outBytes ~= cast(ubyte) width;
            const big = order != "stream" || endianness != Endianness.little;
            if (big)
            {
                foreach_reverse (i; 0 .. width)
                    outBytes ~= cast(ubyte)(cast(uint) v >> (8 * i));
            }
            else
            {
                foreach (i; 0 .. width)
                    outBytes ~= cast(ubyte)(cast(uint) v >> (8 * i));
            }
            return;
        }
        if (framing != "fixed")
            throw new OutOfRange(cast(int) outBytes.length,
                    "a wchar framing, not `" ~ framing ~ "`");
        if (width == 4)
        {
            WriteU32(v);
            return;
        }
        WriteU16(v);
    }

    /// A string: a four-byte length including the terminator, the UTF-8
    /// bytes, then a NUL. The length counts bytes. XTypes 1.3 §7.4.1.1.
    void WriteString(string v)
    {
        WriteU32(cast(uint) v.length + 1);
        outBytes ~= cast(const(ubyte)[]) v;
        outBytes ~= 0;
    }

    /// A bounded string: the same bytes `WriteString` writes. The bound
    /// is a promise about the source and not a difference on the wire,
    /// so this is the only place it can be enforced — refused rather
    /// than cut at a character boundary, which would put a sample on
    /// the wire nobody wrote.
    void WriteStringBounded(string v, int bound)
    {
        const octets = cast(int) v.length;
        if (octets > bound)
            throw new OutOfRange(cast(int) outBytes.length,
                    "at most " ~ to!string(bound) ~ " octets in a bounded string, and this "
                    ~ "one has " ~ to!string(octets));
        WriteString(v);
    }

    /// The count check every bounded sequence makes before a byte of it
    /// is written. Table 17 makes `O2.length <= T1.length` the
    /// condition for constructing a `sequence<T, N>`, and the wire
    /// carries no bound to compare against: a sender whose count
    /// already exceeds its own declaration has written a value of a
    /// type nobody declared.
    void CheckSequenceBound(uint actual, uint bound)
    {
        if (actual > bound)
            throw new OutOfRange(cast(int) outBytes.length,
                    "at most " ~ to!string(bound) ~ " elements in a bounded sequence, and this "
                    ~ "one has " ~ to!string(actual));
    }

    /// The count check every bounded map makes before a byte of it is
    /// written. Its own message and not `CheckSequenceBound`'s: the
    /// unit is an entry and not an element, per XTypes 1.3 §7.4.1.1.4's
    /// map-as-a-sequence-of-pairs equivalence.
    void CheckMapBound(uint actual, uint bound)
    {
        if (actual > bound)
            throw new OutOfRange(cast(int) outBytes.length,
                    "at most " ~ to!string(bound) ~ " entries in a bounded map, and this one "
                    ~ "has " ~ to!string(actual));
    }

    /// A wstring, in the form `policy` and `order` name. The value
    /// arrives as UTF-8 and the conversion to UTF-16 units happens
    /// here, which is what makes §7.4.1.1.2's surrogate rule this
    /// runtime's own reading.
    void WriteWstringUtf8(string v, string policy, string order)
    {
        WstringCell cell;
        if (!wstringCellOf(policy, cell))
            throw new OutOfRange(cast(int) outBytes.length,
                    "a wstring length policy, not `" ~ policy ~ "`");
        if (!wstringOrderKnown(order))
            throw new OutOfRange(cast(int) outBytes.length,
                    "a wstring unit order, not `" ~ order ~ "`");
        auto units = utf16UnitsOf(v, cast(int) outBytes.length);
        if (cell.countsCharacters && cell.width == 2)
        {
            // A character count over two-octet units has no way to
            // carry a code point outside the BMP.
            foreach (index, unit; units)
            {
                if (unit >= 0xD800 && unit <= 0xDFFF)
                    throw new OutOfRange(cast(int) index,
                            "a code point inside the BMP: a character count over two-octet units has no way to carry one outside it");
            }
        }
        if (cell.terminated)
            units ~= 0;
        // An empty wstring carries no mark.
        if (order == "byte-order-mark" && units.length > 0)
            units = byteOrderMark ~ units;
        if (cell.countsCharacters)
            WriteU32(cast(uint) units.length);
        else
            WriteU32(cast(uint)(units.length * cell.width));
        foreach (unit; units)
            writeWstringUnit(unit, cell.width, order);
    }

    /// A bounded wstring, as `WriteStringBounded` is to `WriteString`.
    /// The bound counts UTF-16 code units, not characters and not
    /// octets, whatever `policy` puts on the wire.
    void WriteWstringBounded(string v, int bound, string policy, string order)
    {
        const units = cast(int) utf16UnitsOf(v, cast(int) outBytes.length).length;
        if (units > bound)
            throw new OutOfRange(cast(int) outBytes.length,
                    "at most " ~ to!string(bound) ~ " UTF-16 code units in a bounded wstring, "
                    ~ "and this one has " ~ to!string(units));
        WriteWstringUtf8(v, policy, order);
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
    void WriteWstringUtf8Octets(string v, string policy)
    {
        WstringCell cell;
        if (!wstringCellOf(policy, cell))
            throw new OutOfRange(cast(int) outBytes.length,
                    "a wstring length policy, not `" ~ policy ~ "`");
        // The same malformed-UTF-8 refusal WriteWstringUtf8 makes.
        utf16UnitsOf(v, cast(int) outBytes.length);
        const length = cast(ulong) v.length + (cell.terminated ? 1 : 0);
        if (length > uint.max)
            throw new OutOfRange(cast(int) outBytes.length,
                    "a wstring length the UInt32 field can hold");
        WriteU32(cast(uint) length);
        outBytes ~= cast(const(ubyte)[]) v;
        if (cell.terminated)
            outBytes ~= 0;
    }

    /// A bounded UTF-8-octet wstring, as `WriteWstringBounded` is to
    /// `WriteWstringUtf8`: the bound still counts UTF-16 code units,
    /// which is what a wstring<N> bound means to every peer, while the
    /// wire carries the UTF-8 octets.
    void WriteWstringUtf8OctetsBounded(string v, int bound, string policy)
    {
        const units = cast(int) utf16UnitsOf(v, cast(int) outBytes.length).length;
        if (units > bound)
            throw new OutOfRange(cast(int) outBytes.length,
                    "at most " ~ to!string(bound) ~ " UTF-16 code units in a bounded wstring, "
                    ~ "and this one has " ~ to!string(units));
        WriteWstringUtf8Octets(v, policy);
    }

    /// Move the alignment origin to the current position and answer the
    /// origin displaced, for `FinishPackedElement`.
    ///
    /// `ElementPadding::Packed` is HDDS's measured departure from
    /// §7.4.1, which counts every element's alignment from the
    /// encapsulation: each collection element and each aggregate value
    /// counts its own alignment from its own first octet instead.
    /// array-1d-struct-final is
    /// `01000000 0200 03000000 0400 77`, the second Cell's long at
    /// offset six where the clause puts it at eight.
    int BeginPackedElement()
    {
        const saved = origin;
        origin = cast(int) outBytes.length;
        return saved;
    }

    /// Restore the origin `BeginPackedElement` displaced.
    void FinishPackedElement(int saved)
    {
        origin = saved;
    }

    private void writeWstringUnit(ushort unit, int width, string order)
    {
        const little = order == "byte-order-mark" || order == "host-unswapped";
        if (width == 4)
        {
            if (order == "big-endian-always")
                WriteU32Big(unit);
            else if (little)
                WriteU32Little(unit);
            else
                WriteU32(unit);
            return;
        }
        if (order == "big-endian-always")
            WriteU16Big(unit);
        else if (little)
            WriteU16Little(unit);
        else
            WriteU16(unit);
    }

    /// A 16-bit value little-endian whatever the stream's order is.
    void WriteU16Little(ushort v)
    {
        alignTo(2);
        outBytes ~= cast(ubyte) v;
        outBytes ~= cast(ubyte)(v >> 8);
    }

    /// A 16-bit value big-endian whatever the stream's order is.
    void WriteU16Big(ushort v)
    {
        alignTo(2);
        outBytes ~= cast(ubyte)(v >> 8);
        outBytes ~= cast(ubyte) v;
    }

    /// A 32-bit value little-endian whatever the stream's order is.
    void WriteU32Little(uint v)
    {
        alignTo(4);
        foreach (i; 0 .. 4)
            outBytes ~= cast(ubyte)(v >> (8 * i));
    }

    /// A 32-bit value big-endian whatever the stream's order is.
    void WriteU32Big(uint v)
    {
        alignTo(4);
        foreach_reverse (i; 0 .. 4)
            outBytes ~= cast(ubyte)(v >> (8 * i));
    }

    // -- fixed -------------------------------------------------------------

    /// `fixed<digits, scale>` as packed BCD, [CDR] §9.3.2.8 — the only
    /// wire definition an IDL fixed has. `count` is "declared",
    /// "value-precision" or "value-string-digits".
    void WriteFixed(string v, int digits, int scale, string count)
    {
        const nibbles = fixedNibbles(v, digits, scale, count, cast(int) outBytes.length);
        for (size_t i = 0; i < nibbles.length; i += 2)
            outBytes ~= cast(ubyte)((nibbles[i] << 4) | nibbles[i + 1]);
    }

    // -- the bit-bounded kinds ---------------------------------------------

    /// An enumerator, as the integer its bit bound calls for. The holder
    /// is signed — §7.4.3.5.3 rule (5) — and what fits is everything
    /// expressible in the bound's bits: -2^(n-1) ..= 2^n - 1.
    void WriteEnum(int bitBound, long value)
    {
        if (bitBound == 0 || bitBound > 32)
            throw new OutOfRange(cast(int) outBytes.length,
                    "an @bit_bound between 1 and 32, not " ~ to!string(bitBound));
        if (value < -(1L << (bitBound - 1)) || value > (1L << bitBound) - 1)
            throw new OutOfRange(cast(int) outBytes.length,
                    "an enumerator a " ~ to!string(bitBound) ~ "-bit holder can hold");
        const pattern = cast(ulong) value & ((1UL << bitBound) - 1);
        if (bitBound <= 8)
            WriteU8(cast(ubyte) pattern);
        else if (bitBound <= 16)
            WriteU16(cast(ushort) pattern);
        else
            WriteU32(cast(uint) pattern);
    }

    /// A bitmask, as the unsigned integer Table 33 gives its bound.
    void WriteBitmask(int bitBound, ulong value)
    {
        if (bitBound == 0 || bitBound > 64)
            throw new OutOfRange(cast(int) outBytes.length,
                    "an @bit_bound between 1 and 64, not " ~ to!string(bitBound));
        if (bitBound < 64 && value >= 1UL << bitBound)
            throw new OutOfRange(cast(int) outBytes.length,
                    "a bitmask below 2^" ~ to!string(bitBound));
        if (bitBound <= 8)
            WriteU8(cast(ubyte) value);
        else if (bitBound <= 16)
            WriteU16(cast(ushort) value);
        else if (bitBound <= 32)
            WriteU32(cast(uint) value);
        else
            WriteU64(value);
    }

    /// One bitfield, shifted into the accumulator a bitset packs into.
    /// A value wider than the field is refused rather than masked.
    ulong Bitfield(ulong value, int width, int position)
    {
        if (width <= 0 || width > 64 || position < 0 || position + width > 64)
            throw new OutOfRange(cast(int) outBytes.length,
                    "a bitfield of " ~ to!string(width) ~ " bits at "
                    ~ to!string(position) ~ ", inside 64");
        if (width < 64 && value > (1UL << width) - 1)
            throw new OutOfRange(cast(int) outBytes.length,
                    "a value inside " ~ to!string(width) ~ " bits");
        return value << position;
    }

    /// The Cyclone union overlay: the low `octets` of `pattern`,
    /// written as one unsigned integer of that width in the stream's
    /// byte order. A discriminator carrying a label the `union-labels`
    /// dialect axis says selects nothing makes the peer fall through
    /// to `default:` and serialise its one C union storage as that arm
    /// — the selected member's octets, reinterpreted. The pattern is
    /// the ulong whose little-endian bytes are the selected member's
    /// little-endian storage octets, zero-extended; the `BitPattern*`
    /// functions produce it.
    void WriteUnionOverlay(ulong pattern, long octets)
    {
        switch (octets)
        {
        case 1:
            WriteU8(cast(ubyte) pattern);
            return;
        case 2:
            WriteU16(cast(ushort) pattern);
            return;
        case 4:
            WriteU32(cast(uint) pattern);
            return;
        case 8:
            WriteU64(pattern);
            return;
        default:
            throw new OutOfRange(cast(int) outBytes.length,
                    "a union arm of 1, 2, 4 or 8 octets, not " ~ to!string(octets));
        }
    }

    /// A bitset, as the anchor integer its bitfields pack into; the
    /// anchor's width comes from the layout planner.
    void WriteBitset(int anchor, ulong value)
    {
        switch (anchor)
        {
        case 1:
            WriteU8(cast(ubyte) value);
            return;
        case 2:
            WriteU16(cast(ushort) value);
            return;
        case 4:
            WriteU32(cast(uint) value);
            return;
        case 8:
            WriteU64(value);
            return;
        default:
            throw new OutOfRange(cast(int) outBytes.length,
                    "a bitset anchor of 1, 2, 4 or 8 bytes, not " ~ to!string(anchor));
        }
    }

    // -- framing -----------------------------------------------------------

    /// A DHEADER: a four-byte length placeholder, patched when the
    /// members are done.
    DHeader WriteDheader()
    {
        alignTo(4);
        const at = cast(int) outBytes.length;
        outBytes ~= cast(ubyte[])[0, 0, 0, 0];
        return DHeader(at);
    }

    /// Patch a DHEADER with the length of what followed it; a header
    /// from NoDheader is not there and is ignored.
    void FinishDheader(DHeader header)
    {
        if (header.at < 0)
            return;
        patch(header.at, cast(ulong)(cast(int) outBytes.length - header.at - 4), 4);
    }

    /// An XCDR2 EMHEADER: id, length code and must-understand in one
    /// 32-bit word. §7.4.3.4.2. LC4 reserves a NEXTINT; LC5 to LC7
    /// reuse the member's own leading word.
    EmHeader WriteEmheader(int memberID, int lengthCode, bool mustUnderstand)
    {
        // §7.2.2.4.4.4.4 runs member ids from 0 to 0x0FFFFFFF and
        // §7.4.3.4.2 leaves them the low 28 bits of the word. Masking a
        // larger one writes a well-formed header for `id & 0x0FFFFFFF`:
        // the peer reads a member the caller did not name, and nothing at
        // either end says so.
        if (memberID < 0 || cast(uint) memberID > eMemberIDMask)
            throw new OutOfRange(cast(int) outBytes.length,
                    "a member id inside the 28 bits of an EMHEADER, not "
                    ~ to!string(memberID));
        alignTo(4);
        uint word = (cast(uint)(lengthCode & 0x7) << 28) | (cast(uint) memberID & eMemberIDMask);
        if (mustUnderstand)
            word |= 0x8000_0000;
        WriteU32(word);
        const wordAt = cast(int) outBytes.length - 4;
        if (lengthCode == 4)
        {
            const at = cast(int) outBytes.length;
            outBytes ~= cast(ubyte[])[0, 0, 0, 0];
            return EmHeader(at, wordAt, lengthCode);
        }
        return EmHeader(-1, wordAt, lengthCode);
    }

    /// Patch an EMHEADER's NEXTINT; under `shortLengthCodes` an LC4
    /// header whose member is 1, 2, 4 or 8 bytes loses its NEXTINT and
    /// states the length in the code.
    void FinishEmheader(EmHeader header)
    {
        if (header.nextInt < 0)
        {
            // Under codes 0 to 3 the code *is* the length — 1, 2, 4 or 8
            // octets, §7.4.3.4.2. A value of another size misdeclares it
            // and the reader's skip lands inside data, so the mismatch is
            // refused here rather than written. Codes 5 to 7 declare
            // nothing of their own: the member's leading length word is
            // the NEXTINT.
            if (header.lengthCode >= 0 && header.lengthCode <= 3
                    && header.word + 4 <= cast(int) outBytes.length)
            {
                const declared = 1 << header.lengthCode;
                const body_ = cast(int) outBytes.length - header.word - 4;
                if (body_ != declared)
                    throw new OutOfRange(cast(int) outBytes.length,
                            "a member of " ~ to!string(declared)
                            ~ " octets under length code " ~ to!string(header.lengthCode)
                            ~ ", not " ~ to!string(body_));
            }
            return;
        }
        const length = cast(int) outBytes.length - header.nextInt - 4;
        if (shortLengthCodes && (length == 1 || length == 2 || length == 4 || length == 8))
        {
            shortenEmheader(header, length);
            return;
        }
        patch(header.nextInt, cast(ulong) length, 4);
    }

    private void shortenEmheader(EmHeader header, int length)
    {
        uint code;
        switch (length)
        {
        case 1:
            code = 0;
            break;
        case 2:
            code = 1;
            break;
        case 4:
            code = 2;
            break;
        default:
            code = 3;
            break;
        }
        outBytes = outBytes[0 .. header.nextInt] ~ outBytes[header.nextInt + 4 .. $];
        const previous = readPatched(header.word);
        patch(header.word, (previous & ~(0x7u << 28)) | (code << 28), 4);
    }

    private uint readPatched(int at)
    {
        uint value = 0;
        if (endianness == Endianness.little)
        {
            foreach_reverse (i; 0 .. 4)
                value = (value << 8) | outBytes[at + i];
        }
        else
        {
            foreach (i; 0 .. 4)
                value = (value << 8) | outBytes[at + i];
        }
        return value;
    }

    /// A PL_CDR1 parameter header, short form: [id:u16][length:u16].
    Pid WritePid(int pid, ParameterRole role, bool mustUnderstand)
    {
        // §7.4.1.2.2 gives the short form the ids 0 to `highestShortID`
        // inclusive and reserves what is above it — PID_EXTENDED is 0x3F01
        // itself. A short header with a reserved id collides with the
        // protocol's own parameters, so such an id takes the extended form
        // or none. This is the write side of the constant the reader has
        // always consulted.
        if (pid < 0 || pid > plCdr1.highestShortID)
            throw new OutOfRange(cast(int) outBytes.length,
                    "a parameter id the short header carries, not " ~ to!string(pid)
                    ~ " — the extended header takes it");
        alignTo(4);
        if (mustUnderstand && plCdr1.memberMustUnderstand)
            pid |= PidMustUnderstand;
        pid |= pidFlagsAt(plCdr1.pidFlags, "member");
        const bare = pid & ~(PidMustUnderstand | PidImplExtension);
        WriteU16(cast(ushort) pid);
        const at = cast(int) outBytes.length;
        outBytes ~= cast(ubyte[])[0, 0];
        return Pid(at, false, beginParameterValue(), role, bare, mustUnderstand);
    }

    /// A PL_CDR1 parameter header, extended form:
    /// [PID_EXTENDED][8][id:u32][length:u32]. §7.4.1.2. The member's
    /// must-understand is FLAG_2 of the eMemberHeader.
    Pid WritePidExtended(int pid, ParameterRole role, bool mustUnderstand)
    {
        // The same 28 bits as an EMHEADER's, here in the eMemberHeader of
        // §7.4.1.2.1. Masking aliases the parameter onto another member
        // exactly as it does there.
        if (pid < 0 || cast(uint) pid > eMemberIDMask)
            throw new OutOfRange(cast(int) outBytes.length,
                    "a member id inside the 28 bits of an eMemberHeader, not "
                    ~ to!string(pid));
        alignTo(4);
        const marker = PidExtended | pidFlagsAt(plCdr1.pidFlags, "extended-marker");
        WriteU16(cast(ushort) marker);
        WriteU16(8);
        uint memberHeader = cast(uint) pid & eMemberIDMask;
        if (mustUnderstand && plCdr1.memberMustUnderstand)
            memberHeader |= eFlagMustUnderstand;
        WriteU32(memberHeader);
        const at = cast(int) outBytes.length;
        outBytes ~= cast(ubyte[])[0, 0, 0, 0];
        return Pid(at, true, beginParameterValue(), role, 0, false);
    }

    private int beginParameterValue()
    {
        const saved = origin;
        origin = cast(int) outBytes.length;
        return saved;
    }

    /// Patch a parameter header with its value's length; the padding to
    /// four goes outside the declared length or inside it, by framing.
    void FinishPid(Pid header)
    {
        const width = header.extended ? 4 : 2;
        int length = cast(int) outBytes.length - header.at - width;
        if (plCdr1.padsDeclaredLength(header.role))
        {
            const past = (cast(int) outBytes.length - header.savedOrigin) % 4;
            if (past != 0)
                length += 4 - past;
        }
        if (!header.extended && length > PidShortMaxLen)
            promotePid(header, length);
        else
            patch(header.at, cast(ulong) length, width);
        origin = header.savedOrigin;
        // Under Fast DDS's framing the parameter writes no padding of
        // its own — the next header aligns itself, and a final
        // parameter ends the stream where its value does.
        if (plCdr1.padAfterValue)
            alignTo(4);
    }

    private void promotePid(Pid header, int length)
    {
        const start = header.at - 2;
        outBytes = outBytes[0 .. header.at + 2]
            ~ cast(ubyte[])[0, 0, 0, 0, 0, 0, 0, 0] ~ outBytes[header.at + 2 .. $];
        const marker = PidExtended | pidFlagsAt(plCdr1.pidFlags, "extended-marker");
        patch(start, cast(ulong) marker, 2);
        patch(start + 2, 8, 2);
        uint memberHeader = cast(uint) header.pid & eMemberIDMask;
        if (header.mustUnderstand && plCdr1.memberMustUnderstand)
            memberHeader |= eFlagMustUnderstand;
        patch(start + 4, memberHeader, 4);
        patch(start + 8, cast(ulong) length, 4);
    }

    /// The PID_LIST_END sentinel a PL_CDR1 parameter list ends with;
    /// its flags come from the framing asked at the "sentinel" position.
    void WriteSentinel()
    {
        alignTo(4);
        const sentinel = PidListEnd | pidFlagsAt(plCdr1.pidFlags, "sentinel");
        WriteU16(cast(ushort) sentinel);
        WriteU16(0);
    }

    private void patch(int at, ulong value, int width)
    {
        if (value >= 1UL << (width * 8))
            throw new OutOfRange(at, "a " ~ to!string(width) ~ "-byte length");
        if (endianness == Endianness.little)
        {
            foreach (i; 0 .. width)
                outBytes[at + i] = cast(ubyte)(value >> (8 * i));
        }
        else
        {
            foreach (i; 0 .. width)
                outBytes[at + width - 1 - i] = cast(ubyte)(value >> (8 * i));
        }
    }
}

// -- the reader ------------------------------------------------------------

/// Reads one sample's bytes: the mirror of the writer clause for
/// clause. A malformed stream throws; a refusal about the type sets the
/// status and the generated decoder walks on.
class Reader
{
    private const(ubyte)[] data;
    private int at = 0;
    private Endianness endianness;
    private int maxAlign;
    private int origin = 0;
    private string pidFlags;
    private int highestShortID;
    private Status status = Status.ok;

    this(const(ubyte)[] data, Endianness endianness, int maxAlign, string pidFlags,
            int highestShortID)
    {
        this.data = data;
        this.endianness = endianness;
        this.maxAlign = maxAlign < 1 ? 1 : maxAlign;
        this.pidFlags = pidFlags;
        this.highestShortID = highestShortID;
    }

    // -- the status a generated decoder reads ------------------------------

    /// How the read is going.
    Status ReaderStatus()
    {
        return status;
    }

    /// Refuse the stream from outside the runtime; the first refusal
    /// sticks, and StatusOk refuses nothing.
    void ReaderRefuse(Status refusal)
    {
        if (refusal != Status.ok && status == Status.ok)
            status = refusal;
    }

    /// Take back a StatusDiscarded, and only that one: a truncated
    /// stream is not a narrowing.
    bool ReaderClearDiscard()
    {
        if (status == Status.discarded)
        {
            status = Status.ok;
            return true;
        }
        return false;
    }

    /// Where the reader is.
    int ReaderPosition()
    {
        return at;
    }

    /// How many bytes are left.
    int ReaderRemaining()
    {
        if (at > cast(int) data.length)
            return 0;
        return cast(int) data.length - at;
    }

    /// Move the cursor forward to `target`, never backwards: a framing
    /// word pointing behind the cursor is a refusal about the stream's
    /// shape, and past the end throws.
    void ReaderSkipTo(int target)
    {
        if (status != Status.ok)
            return;
        if (target < at)
        {
            ReaderRefuse(Status.outOfRange);
            return;
        }
        if (target > cast(int) data.length)
            throw new Truncated(at, target - at, ReaderRemaining());
        at = target;
    }

    /// The cursor, the origin and the status, to be put back later.
    ReaderMark ReaderSave()
    {
        return ReaderMark(at, origin, status);
    }

    /// Put back what ReaderSave took.
    void ReaderRestore(ReaderMark mark)
    {
        at = mark.at;
        origin = mark.origin;
        status = mark.status;
    }

    /// Move the alignment origin to the current position and answer the
    /// origin displaced, for `ReaderFinishPackedElement`. The read-side
    /// half of `BeginPackedElement`.
    int ReaderBeginPackedElement()
    {
        const saved = origin;
        origin = at;
        return saved;
    }

    /// Restore the origin `ReaderBeginPackedElement` displaced.
    void ReaderFinishPackedElement(int saved)
    {
        origin = saved;
    }

    // -- the cursor --------------------------------------------------------

    private void alignTo(int alignment)
    {
        if (alignment > maxAlign)
            alignment = maxAlign;
        if (alignment <= 1)
            return;
        const offset = at - origin;
        at += (alignment - offset % alignment) % alignment;
    }

    private const(ubyte)[] take(int count)
    {
        const end = at + count;
        if (end > cast(int) data.length)
            throw new Truncated(at, count, ReaderRemaining());
        auto chunk = data[at .. end];
        at = end;
        return chunk;
    }

    private ulong readUnsigned(int width)
    {
        alignTo(width);
        const raw = take(width);
        ulong value = 0;
        if (endianness == Endianness.little)
        {
            foreach_reverse (i; 0 .. width)
                value = (value << 8) | raw[i];
        }
        else
        {
            foreach (i; 0 .. width)
                value = (value << 8) | raw[i];
        }
        return value;
    }

    // -- primitives --------------------------------------------------------

    /// `boolean`; a byte that is neither 0 nor 1 is refused rather than
    /// coerced.
    bool ReadBool()
    {
        const position = at;
        const value = take(1)[0];
        if (value == 0)
            return false;
        if (value == 1)
            return true;
        throw new OutOfRange(position, "a boolean of 0 or 1");
    }

    /// `octet` and `uint8`.
    ubyte ReadU8()
    {
        return take(1)[0];
    }

    /// `int8`.
    byte ReadI8()
    {
        return cast(byte) take(1)[0];
    }

    /// `char`: one octet.
    ubyte ReadChar()
    {
        return take(1)[0];
    }

    /// `short`.
    short ReadI16()
    {
        return cast(short) readUnsigned(2);
    }

    /// `unsigned short`.
    ushort ReadU16()
    {
        return cast(ushort) readUnsigned(2);
    }

    /// `long`.
    int ReadI32()
    {
        return cast(int) readUnsigned(4);
    }

    /// `unsigned long`.
    uint ReadU32()
    {
        return cast(uint) readUnsigned(4);
    }

    /// `long long`.
    long ReadI64()
    {
        return cast(long) readUnsigned(8);
    }

    /// `unsigned long long`.
    ulong ReadU64()
    {
        return readUnsigned(8);
    }

    /// `float`.
    float ReadF32()
    {
        const bits = cast(uint) readUnsigned(4);
        return *cast(const float*)&bits;
    }

    /// `double`.
    double ReadF64()
    {
        const bits = readUnsigned(8);
        return *cast(const double*)&bits;
    }

    /// `long double`: all sixteen bytes, as the exact decimal the
    /// generated member holds.
    string ReadF128()
    {
        alignTo(8);
        const raw = take(16);
        ubyte[16] bytes;
        if (endianness == Endianness.little)
        {
            foreach (i; 0 .. 16)
                bytes[i] = raw[i];
        }
        else
        {
            foreach (i; 0 .. 16)
                bytes[i] = raw[15 - i];
        }
        return f128ToExactDecimal(bytes);
    }

    /// `wchar`, at `width` octets and in `framing`'s shape. The
    /// four-byte form zero-extends one UTF-16 code unit; a value that
    /// uses the high bits, and a lone surrogate, are refused.
    ushort ReadWchar(int width, string framing, string order)
    {
        const position = at;
        if (width != 2 && width != 4)
            throw new OutOfRange(position, "a wchar width of 2 or 4");
        uint value;
        if (framing == "giop-length-prefixed")
        {
            const declared = ReadU8();
            if (declared != width)
                throw new OutOfRange(position,
                        "a GIOP 1.2 wchar of " ~ to!string(width)
                        ~ " octets, the configured transmission code set, and not "
                        ~ to!string(declared));
            const big = order != "stream" || endianness != Endianness.little;
            const raw = take(width);
            value = 0;
            if (big)
            {
                foreach (i; 0 .. width)
                    value = (value << 8) | raw[i];
            }
            else
            {
                foreach_reverse (i; 0 .. width)
                    value = (value << 8) | raw[i];
            }
        }
        else if (framing != "fixed")
        {
            throw new OutOfRange(position, "a wchar framing, not `" ~ framing ~ "`");
        }
        else if (width == 4)
        {
            value = ReadU32();
            if (value > 0xFFFF)
                throw new OutOfRange(position, "a four-byte wchar holding one UTF-16 code unit");
        }
        else
        {
            value = ReadU16();
        }
        if (value >= 0xD800 && value <= 0xDFFF)
            throw new OutOfRange(position, "a Unicode code point");
        return cast(ushort) value;
    }

    // -- strings -----------------------------------------------------------

    /// A string: a length including the terminator, the bytes, the NUL.
    string ReadString()
    {
        import std.utf : validate;

        const position = at;
        const length = cast(int) ReadU32();
        if (length == 0 || length > ReaderRemaining() + 1)
            throw new ImpossibleLength(position, length, ReaderRemaining());
        const text = take(length - 1);
        auto candidate = cast(string) text.idup;
        try
        {
            validate(candidate);
        }
        catch (Exception)
        {
            throw new NotUtf8(position);
        }
        ReadU8();
        return candidate;
    }

    /// ReadString under the name the shared walk calls it by: a D
    /// string owns its storage, so the two names are one method.
    string ReadStringAlloc()
    {
        return ReadString();
    }

    /// A 16-bit value little-endian whatever the stream's order is.
    ushort ReadU16Little()
    {
        alignTo(2);
        const raw = take(2);
        return cast(ushort)(raw[0] | (raw[1] << 8));
    }

    /// A 16-bit value big-endian whatever the stream's order is.
    ushort ReadU16Big()
    {
        alignTo(2);
        const raw = take(2);
        return cast(ushort)((raw[0] << 8) | raw[1]);
    }

    /// A 32-bit value little-endian whatever the stream's order is.
    uint ReadU32Little()
    {
        alignTo(4);
        const raw = take(4);
        return raw[0] | (cast(uint) raw[1] << 8) | (cast(uint) raw[2] << 16)
            | (cast(uint) raw[3] << 24);
    }

    /// A 32-bit value big-endian whatever the stream's order is.
    uint ReadU32Big()
    {
        alignTo(4);
        const raw = take(4);
        return (cast(uint) raw[0] << 24) | (cast(uint) raw[1] << 16)
            | (cast(uint) raw[2] << 8) | raw[3];
    }

    /// A wstring, in the form `policy` and `order` name, answered as
    /// UTF-8 — which is what the generated member holds. The form is
    /// never inferred from the stream.
    string ReadWstringUtf8(string policy, string order)
    {
        const position = at;
        const declared = cast(int) ReadU32();
        if (!wstringOrderKnown(order))
            throw new OutOfRange(position, "a wstring unit order, not `" ~ order ~ "`");
        WstringCell cell;
        if (!wstringCellOf(policy, cell))
            throw new OutOfRange(position, "a wstring length policy, not `" ~ policy ~ "`");
        int count = declared;
        if (!cell.countsCharacters)
        {
            if (declared % cell.width != 0)
                throw new ImpossibleLength(position, declared, ReaderRemaining());
            count = declared / cell.width;
        }
        auto units = wstringUnits(position, count, cell.width, order);
        if (cell.terminated)
        {
            if (declared < 1)
                throw new ImpossibleLength(position, declared, ReaderRemaining());
            const last = units[$ - 1];
            units = units[0 .. $ - 1];
            if (last != 0)
                throw new OutOfRange(position,
                        "a NUL character ending the wstring, as its count includes one");
            foreach (unit; units)
            {
                if (unit == 0)
                    throw new OutOfRange(position,
                            "no NUL inside a wstring a C peer would stop at");
            }
        }
        if (order == "byte-order-mark")
            units = stripByteOrderMark(units);
        char[] built;
        size_t index = 0;
        while (index < units.length)
        {
            const unit = units[index];
            if (unit >= 0xD800 && unit <= 0xDBFF && index + 1 < units.length)
            {
                const low = units[index + 1];
                if (low >= 0xDC00 && low <= 0xDFFF)
                {
                    built ~= cast(dchar)(0x10000 + ((cast(int) unit - 0xD800) << 10)
                            + (cast(int) low - 0xDC00));
                    index += 2;
                    continue;
                }
            }
            if (unit >= 0xD800 && unit <= 0xDFFF)
                throw new OutOfRange(position, "a UTF-16 sequence with no lone surrogate");
            built ~= cast(dchar) unit;
            index++;
        }
        return cast(string) built;
    }

    /// The inverse of `WriteWstringUtf8Octets`: the declared length is
    /// the UTF-8 octet count, the terminator the policy asks for is the
    /// last of those octets, and a count the stream cannot hold, a
    /// missing or an interior NUL, or an ill-formed UTF-8 sequence is
    /// refused. See `WstringCodeUnits::Utf8`.
    string ReadWstringUtf8Octets(string policy)
    {
        import std.utf : validate;

        const position = at;
        const declared = cast(int) ReadU32();
        WstringCell cell;
        if (!wstringCellOf(policy, cell))
            throw new OutOfRange(position, "a wstring length policy, not `" ~ policy ~ "`");
        if (declared > ReaderRemaining())
            throw new ImpossibleLength(position, declared, ReaderRemaining());
        const data = take(declared);
        int length = declared;
        if (cell.terminated)
        {
            if (declared < 1 || data[declared - 1] != 0)
                throw new OutOfRange(position,
                        "a NUL octet ending the wstring, as its count includes one");
            length = declared - 1;
            // A NUL inside the text is the narrow string's problem in
            // wide form: every C peer stops at it.
            foreach (i; 0 .. length)
            {
                if (data[i] == 0)
                    throw new OutOfRange(position,
                            "no NUL inside a wstring a C peer would stop at");
            }
        }
        auto text = cast(string) data[0 .. length].idup;
        try
        {
            validate(text);
        }
        catch (Exception)
        {
            throw new NotUtf8(position);
        }
        return text;
    }

    private ushort[] wstringUnits(int position, int count, int width, string order)
    {
        if (count * width > ReaderRemaining())
            throw new ImpossibleLength(position, count * width, ReaderRemaining());
        const little = order == "byte-order-mark" || order == "host-unswapped";
        ushort[] units;
        units.reserve(count);
        foreach (i; 0 .. count)
        {
            if (width == 4)
            {
                uint wide;
                if (order == "big-endian-always")
                    wide = ReadU32Big();
                else if (little)
                    wide = ReadU32Little();
                else
                    wide = ReadU32();
                if (wide > 0xFFFF)
                    throw new OutOfRange(position,
                            "a four-byte wstring character holding one UTF-16 unit");
                units ~= cast(ushort) wide;
                continue;
            }
            if (order == "big-endian-always")
                units ~= ReadU16Big();
            else if (little)
                units ~= ReadU16Little();
            else
                units ~= ReadU16();
        }
        return units;
    }

    /// `fixed<digits, scale>`, as the decimal string the writer took:
    /// the leading zeros go, and a negative zero loses its sign. Under
    /// "value-precision" and "value-string-digits" the field ends at the
    /// first octet whose low nibble is a sign, and one that never presents
    /// a sign within the sixteen octets of the widest fixed IDL admits is
    /// refused.
    string ReadFixed(int digits, int scale, string count)
    {
        if (count != "declared" && count != "value-precision" && count != "value-string-digits")
            throw new OutOfRange(at, "a fixed digit count, not `" ~ count ~ "`");
        // The same bound as the writer's, on this side too: OMG IDL 4.2
        // rule (42) caps the BCD rule at 31 digits, and a reader asked for
        // a wider field would take octets no declaration can name.
        if (digits <= 0 || digits > 31 || scale > digits)
            throw new OutOfRange(at,
                    "a fixed<P, S> with S <= P and 1 <= P <= 31, not fixed<"
                    ~ to!string(digits) ~ ", " ~ to!string(scale) ~ ">");
        int[] nibbles;
        int sign;
        if (count == "value-precision" || count == "value-string-digits")
        {
            while (true)
            {
                if (nibbles.length >= 32)
                    throw new OutOfRange(at, "a sign nibble within sixteen octets");
                const b = take(1)[0];
                nibbles ~= b >> 4;
                nibbles ~= b & 0x0F;
                if ((b & 0x0F) >= 0x0A)
                    break;
            }
            sign = nibbles[$ - 1];
            nibbles = nibbles[0 .. $ - 1];
        }
        else
        {
            const raw = take((digits + 2) / 2);
            foreach (b; raw)
            {
                nibbles ~= b >> 4;
                nibbles ~= b & 0x0F;
            }
            sign = nibbles[$ - 1];
            nibbles = nibbles[0 .. $ - 1];
            if ((digits + 1) % 2 == 1)
                nibbles = nibbles[1 .. $];
        }
        foreach (nibble; nibbles)
        {
            if (nibble > 9)
                throw new OutOfRange(at, "BCD digits");
        }
        char[] text;
        foreach (nibble; nibbles)
            text ~= cast(char)('0' + nibble);
        while (cast(int) text.length < scale + 1)
            text = '0' ~ text;
        const split = cast(int) text.length - scale;
        auto whole = text[0 .. split];
        while (whole.length > 1 && whole[0] == '0')
            whole = whole[1 .. $];
        const fraction = text[split .. $];
        string result = cast(string) whole.idup;
        if (scale > 0)
            result = result ~ "." ~ cast(string) fraction.idup;
        bool zero = whole == "0";
        foreach (digit; fraction)
        {
            if (digit != '0')
                zero = false;
        }
        if (sign == 0x0D && !zero)
            return "-" ~ result;
        return result;
    }

    // -- the trims a bound reaches for -------------------------------------

    /// The trim, reached as a method because the shared walk emits it as
    /// a call on the reader.
    string TrimUtf8(string text, int octets)
    {
        return zdwire.TrimUtf8(text, octets);
    }

    /// The wide half.
    string TrimUtf16(string text, int units)
    {
        return zdwire.TrimUtf16(text, units);
    }

    // -- the bit-bounded kinds ---------------------------------------------

    /// An enumerator's holder, as the signed integer §7.4.3.5.3 makes
    /// it — the holder's width and not the bound's.
    long ReadEnum(int bitBound)
    {
        if (bitBound == 0 || bitBound > 32)
            throw new OutOfRange(at, "an @bit_bound between 1 and 32, not " ~ to!string(bitBound));
        if (bitBound <= 8)
            return cast(byte) ReadU8();
        if (bitBound <= 16)
            return cast(short) ReadU16();
        return cast(int) ReadU32();
    }

    /// A bitmask's holder. Table 33 of §7.4.1.1.3.2.
    ulong ReadBitmask(int bitBound)
    {
        if (bitBound == 0 || bitBound > 64)
            throw new OutOfRange(at, "an @bit_bound between 1 and 64, not " ~ to!string(bitBound));
        if (bitBound <= 8)
            return ReadU8();
        if (bitBound <= 16)
            return ReadU16();
        if (bitBound <= 32)
            return ReadU32();
        return ReadU64();
    }

    /// A bitset's anchor integer, of the width the planner sized.
    ulong ReadBitset(int anchor)
    {
        switch (anchor)
        {
        case 1:
            return ReadU8();
        case 2:
            return ReadU16();
        case 4:
            return ReadU32();
        case 8:
            return ReadU64();
        default:
            throw new OutOfRange(at, "a bitset anchor of 1, 2, 4 or 8 bytes, not "
                    ~ to!string(anchor));
        }
    }

    // -- framing -----------------------------------------------------------

    /// A DHEADER: the length the members that follow occupy, refusing
    /// one the stream cannot hold.
    uint ReadDheader()
    {
        const position = at;
        const length = ReadU32();
        if (cast(int) length > ReaderRemaining())
            throw new ImpossibleLength(position, cast(int) length, ReaderRemaining());
        return length;
    }

    /// An EMHEADER, unpacked. Codes 5 to 7 report the NEXTINT they
    /// read, but the reader is put back before it: that word is the
    /// member's own leading length. §7.4.3.4.2.
    EmHeaderRead ReadEmheader()
    {
        const word = ReadU32();
        EmHeaderRead header;
        header.MemberId = cast(int)(word & eMemberIDMask);
        header.LengthCode = cast(int)((word >> 28) & 0x7);
        header.MustUnderstand = (word >> 31) != 0;
        header.NextInt = -1;
        if (header.LengthCode == 4)
        {
            header.NextInt = cast(int) ReadU32();
        }
        else if (header.LengthCode >= 5)
        {
            const position = at;
            header.NextInt = cast(int) ReadU32();
            at = position;
        }
        header.Extent = emExtent(header);
        return header;
    }

    private static int emExtent(EmHeaderRead header)
    {
        if (header.LengthCode <= 3)
            return 1 << header.LengthCode;
        if (header.NextInt < 0)
            return 0;
        if (header.LengthCode == 4)
            return header.NextInt;
        int scale = 1;
        if (header.LengthCode == 6)
            scale = 4;
        else if (header.LengthCode == 7)
            scale = 8;
        return 4 + header.NextInt * scale;
    }

    private PidRead readPidHeader(ParameterRole role)
    {
        alignTo(4);
        const pid = cast(int) ReadU16();
        const length = cast(int) ReadU16();
        if ((pid & pidIDMask) == (PidExtended & pidIDMask))
        {
            const memberHeader = ReadU32();
            const wideLength = cast(int) ReadU32();
            PidRead header;
            header.Pid = cast(int)(memberHeader & eMemberIDMask);
            header.Length = wideLength;
            header.End = false;
            header.MustUnderstand = (memberHeader & eFlagMustUnderstand) != 0;
            header.ImplExtension = (memberHeader & eFlagImplExtension) != 0;
            header.Role = role;
            return header;
        }
        const identifier = pid & pidIDMask;
        const end = identifier == (PidListEnd & pidIDMask);
        PidRead header;
        header.Pid = identifier;
        header.Length = length;
        header.End = end;
        header.MustUnderstand = (pid & PidMustUnderstand) != 0;
        // Under a framing that puts FLAG_IMPL_EXTENSION on every member
        // id the bit carries no information and is reported absent.
        header.ImplExtension = (pid & PidImplExtension) != 0
            && (pidFlagsAt(pidFlags, "member") & PidImplExtension) == 0;
        header.Reserved = !end && identifier > highestShortID;
        header.Ignore = identifier == PidIgnore;
        header.Role = role;
        return header;
    }

    /// A parameter header, and the value block it opens: §7.4.1.2 makes
    /// a parameter's value its own stream block, so a member's internal
    /// alignment is counted from the first octet after the header.
    PidRead ReadPidAs(ParameterRole role)
    {
        auto header = readPidHeader(role);
        header.SavedOrigin = origin;
        header.ValueAt = at;
        origin = at;
        return header;
    }

    /// End a parameter's value and step to the next header: the origin
    /// goes back, and the padding to four is stepped against the
    /// enclosing stream. Nothing follows PID_LIST_END, and a reader
    /// that has already refused does not move.
    void FinishPidRead(PidRead header)
    {
        origin = header.SavedOrigin;
        if (status != Status.ok || header.End)
            return;
        alignTo(4);
    }
}

private ushort[] stripByteOrderMark(ushort[] units)
{
    if (units.length == 0)
        return units;
    if (units[0] == byteOrderMark)
        return units[1 .. $];
    auto rest = units;
    if (units[0] == swappedByteOrderMark)
        rest = units[1 .. $];
    auto swapped = new ushort[](rest.length);
    foreach (i, unit; rest)
        swapped[i] = cast(ushort)((unit << 8) | (unit >> 8));
    return swapped;
}

// -- fixed BCD -------------------------------------------------------------

private int[] fixedNibbles(string text, int digits, int scale, string count, int at)
{
    if (digits == 0)
        throw new OutOfRange(at, "a fixed with at least one digit");
    // OMG IDL 4.2 rule (42) caps the BCD rule at 31 digits, and
    // fixed<31, S> is the widest declaration IDL admits. A wider one has
    // no packing defined for it, so it is refused rather than packed into
    // octets no reader can be told the width of.
    if (digits > 31)
        throw new OutOfRange(at, "a fixed of at most 31 digits, not " ~ to!string(digits));
    if (scale > digits)
        throw new OutOfRange(at, "a scale no larger than the digit count");
    if (count != "declared" && count != "value-precision" && count != "value-string-digits")
        throw new OutOfRange(at, "a fixed digit count, not `" ~ count ~ "`");
    bool negative = false;
    auto magnitude = text;
    if (magnitude.length > 0 && (magnitude[0] == '-' || magnitude[0] == '+'))
    {
        negative = magnitude[0] == '-';
        magnitude = magnitude[1 .. $];
    }
    auto whole = magnitude;
    string fraction = "";
    foreach (i; 0 .. magnitude.length)
    {
        if (magnitude[i] == '.')
        {
            whole = magnitude[0 .. i];
            fraction = magnitude[i + 1 .. $];
            break;
        }
    }
    void refuse()
    {
        throw new OutOfRange(at, "a decimal that fits fixed<" ~ to!string(digits)
                ~ ", " ~ to!string(scale) ~ ">, not `" ~ text ~ "`");
    }

    if (whole.length == 0 && fraction.length == 0)
        refuse();
    foreach (c; whole)
    {
        if (c < '0' || c > '9')
            refuse();
    }
    foreach (c; fraction)
    {
        if (c < '0' || c > '9')
            refuse();
    }
    const asWritten = cast(int) whole.length + scale;
    const integralWidth = digits - scale;
    while (whole.length > 0 && whole[0] == '0')
        whole = whole[1 .. $];
    if (cast(int) whole.length > integralWidth)
        throw new OutOfRange(at, "a decimal whose integer part fits "
                ~ to!string(integralWidth) ~ " digits, not `" ~ text ~ "`");
    if (cast(int) fraction.length > scale)
        throw new OutOfRange(at, "a decimal whose fraction fits " ~ to!string(scale)
                ~ " digits, not `" ~ text ~ "`");

    int[] every;
    foreach (i; 0 .. integralWidth - cast(int) whole.length)
        every ~= 0;
    foreach (c; whole)
        every ~= c - '0';
    foreach (c; fraction)
        every ~= c - '0';
    foreach (i; 0 .. scale - cast(int) fraction.length)
        every ~= 0;
    int significant = 0;
    foreach (i, nibble; every)
    {
        if (nibble != 0)
        {
            significant = cast(int) every.length - cast(int) i;
            break;
        }
    }
    int wire = cast(int) every.length;
    if (count == "value-precision")
    {
        wire = significant;
        if (wire < 1)
            wire = 1;
    }
    else if (count == "value-string-digits")
    {
        // ACE/TAO 2.5.24's ACE_CDR::Fixed::from_string (OpenDDS writes the declared count): the
        // digits as written, made odd by one leading zero (0.12345 under
        // fixed<5,5> is 0012345c). Past 31 no reader can be told the width.
        wire = asWritten + (asWritten % 2 == 0 ? 1 : 0);
        if (wire > 31)
            throw new OutOfRange(at, "a fixed of at most 31 digits on the wire, not "
                    ~ to!string(wire) ~ " for `" ~ text ~ "`");
    }
    int[] nibbles;
    if ((wire + 1) % 2 == 1)
        nibbles ~= 0;
    // A count wider than P is filled with leading zeros.
    const kept = wire < cast(int) every.length ? wire : cast(int) every.length;
    foreach (i; kept .. wire)
        nibbles ~= 0;
    nibbles ~= every[$ - kept .. $];
    int sign = 0x0C;
    if (negative && significant != 0)
        sign = 0x0D;
    nibbles ~= sign;
    return nibbles;
}

// -- binary128 -------------------------------------------------------------

private enum int f128FractionBits = 112;
private enum int f128ExponentBias = 16383;
private enum int f128ExponentMax = 0x7FFF;

private BigInt f128One()
{
    return BigInt(1);
}

private BigInt f128SignMask()
{
    return f128One() << 127;
}

private BigInt f128FractionMask()
{
    return (f128One() << f128FractionBits) - 1;
}

private BigInt f128ExponentBits()
{
    return BigInt(f128ExponentMax) << f128FractionBits;
}

private BigInt f128QuietBit()
{
    return f128One() << (f128FractionBits - 1);
}

/// The bit length of a non-negative BigInt: 64 bits at a time, then a
/// binary search over the last word by comparison — std.bigint answers
/// no such question itself.
private int bigBitLength(BigInt v)
{
    int bits = 0;
    const wordCeiling = f128One() << 64;
    while (v >= wordCeiling)
    {
        v >>= 64;
        bits += 64;
    }
    while (v > 0)
    {
        v >>= 1;
        bits += 1;
    }
    return bits;
}

private ubyte[16] f128Bytes(BigInt bits)
{
    ubyte[16] littleEndian;
    foreach (i; 0 .. 16)
    {
        littleEndian[i] = cast(ubyte)(bits & 0xFF).toInt();
        bits >>= 8;
    }
    return littleEndian;
}

/// Parse `text` and answer the sixteen bytes, least-significant first.
/// A malformed decimal throws OutOfRange at `at`.
private ubyte[16] f128FromDecimal(string text, int at)
{
    import std.string : strip, toLower, indexOf;

    void refuse()
    {
        throw new OutOfRange(at, "a decimal for a long double, not `" ~ text ~ "`");
    }

    const stripped = strip(text);
    const lowered = toLower(stripped);
    switch (lowered)
    {
    case "inf", "+inf", "infinity", "+infinity":
        return f128Bytes(f128ExponentBits());
    case "-inf", "-infinity":
        return f128Bytes(f128ExponentBits() | f128SignMask());
    case "nan", "+nan":
        return f128Bytes(f128ExponentBits() | f128QuietBit());
    case "-nan":
        return f128Bytes(f128ExponentBits() | f128QuietBit() | f128SignMask());
    default:
        break;
    }

    bool negative = false;
    string digitsPart = stripped;
    if (digitsPart.length > 0 && (digitsPart[0] == '+' || digitsPart[0] == '-'))
    {
        negative = digitsPart[0] == '-';
        digitsPart = digitsPart[1 .. $];
    }
    string mantissa = digitsPart;
    int exponent10 = 0;
    auto split = indexOf(digitsPart, 'e');
    if (split < 0)
        split = indexOf(digitsPart, 'E');
    if (split >= 0)
    {
        mantissa = digitsPart[0 .. split];
        try
        {
            exponent10 = to!int(digitsPart[split + 1 .. $]);
        }
        catch (Exception)
        {
            refuse();
        }
    }
    string whole = mantissa;
    string fraction = "";
    const point = indexOf(mantissa, '.');
    if (point >= 0)
    {
        whole = mantissa[0 .. point];
        fraction = mantissa[point + 1 .. $];
    }
    const digits = whole ~ fraction;
    if (digits.length == 0)
        refuse();
    foreach (c; digits)
    {
        // An underscore is refused rather than ignored: in a value
        // file it is a typo, not a digit separator.
        if (c < '0' || c > '9')
            refuse();
    }
    auto significand = BigInt(digits);
    exponent10 -= cast(int) fraction.length;

    if (significand == 0)
    {
        if (negative)
            return f128Bytes(f128SignMask());
        return f128Bytes(BigInt(0));
    }
    // The exact rational, then one correct rounding on it: a two-step
    // parse through a double rounds twice.
    auto numerator = significand;
    auto denominator = BigInt(1);
    if (exponent10 >= 0)
        numerator = numerator * (BigInt(10) ^^ cast(uint) exponent10);
    else
        denominator = BigInt(10) ^^ cast(uint)(-exponent10);
    return f128Bytes(f128FromFraction(negative, numerator, denominator));
}

/// Round numerator/denominator to binary128, to nearest with ties to
/// even: scale so the quotient carries more bits than binary128 needs,
/// fold everything discarded into one sticky bit, round once.
private BigInt f128FromFraction(bool negative, BigInt numerator, BigInt denominator)
{
    enum wanted = 130;
    int shift = wanted - (bigBitLength(numerator) - bigBitLength(denominator));
    if (shift < 0)
        shift = 0;
    const scaled = numerator << cast(uint) shift;
    auto quotient = scaled / denominator;
    const remainder = scaled % denominator;
    if (remainder != 0)
        quotient |= 1;
    bool lost;
    return f128Compose(negative, quotient, -shift, lost);
}

/// Build a pattern from a sign, an integral significand and a binary
/// exponent, rounding to nearest with ties to even; `lost` says whether
/// the rounding lost anything.
private BigInt f128Compose(bool negative, BigInt significand, int exponent, out bool lost)
{
    auto sign = negative ? f128SignMask() : BigInt(0);
    lost = false;
    if (significand == 0)
        return sign;
    if (exponent > f128ExponentMax)
    {
        lost = true;
        return sign | f128ExponentBits();
    }
    if (exponent < -(f128FractionBits + 1 + f128ExponentBias + 200))
    {
        lost = true;
        return sign;
    }
    auto value = significand;
    const leading = bigBitLength(value) - 1;
    if (leading > f128FractionBits)
    {
        const drop = leading - f128FractionBits;
        bool dropped;
        value = f128ShiftRound(value, drop, dropped);
        lost = lost || dropped;
        exponent += drop;
        if (bigBitLength(value) - 1 > f128FractionBits)
        {
            value >>= 1;
            exponent += 1;
        }
    }
    else
    {
        const shift = f128FractionBits - leading;
        value <<= cast(uint) shift;
        exponent -= shift;
    }
    const biased = exponent + f128ExponentBias + f128FractionBits;
    if (biased >= f128ExponentMax)
    {
        lost = true;
        return sign | f128ExponentBits();
    }
    if (biased <= 0)
    {
        // Subnormal; a value that rounds up to the smallest normal
        // carries into the exponent field on its own.
        bool dropped;
        const rounded = f128ShiftRound(value, 1 - biased, dropped);
        lost = lost || dropped;
        return sign | rounded;
    }
    return sign | (BigInt(biased) << f128FractionBits) | (value & f128FractionMask());
}

/// Shift right, rounding to nearest with ties to even.
private BigInt f128ShiftRound(BigInt value, int shift, out bool dropped)
{
    dropped = false;
    if (shift <= 0)
        return value;
    auto kept = value >> cast(uint) shift;
    const mask = (f128One() << cast(uint) shift) - 1;
    const remainder = value & mask;
    if (remainder == 0)
        return kept;
    dropped = true;
    const half = f128One() << cast(uint)(shift - 1);
    if (remainder > half || (remainder == half && (kept & 1) == 1))
        kept = kept + 1;
    return kept;
}

/// Every digit the value has, in the corpus's canonical form — the
/// exact expansion, in exactly the spelling the sibling renderers
/// produce, because the columns are compared as text.
private string f128ToExactDecimal(ubyte[16] bytes)
{
    BigInt bits = BigInt(0);
    foreach_reverse (i; 0 .. 16)
        bits = (bits << 8) | BigInt(bytes[i]);

    const negative = (bits & f128SignMask()) != 0;
    const biased = cast(int)((bits >> f128FractionBits) & f128ExponentMax).toInt();
    const fraction = bits & f128FractionMask();

    if (biased == f128ExponentMax)
    {
        if (fraction != 0)
            return "nan";
        return negative ? "-inf" : "inf";
    }
    BigInt significand;
    int exponent;
    if (biased == 0)
    {
        significand = fraction;
        exponent = 1 - f128ExponentBias - f128FractionBits;
    }
    else
    {
        significand = fraction | (f128One() << f128FractionBits);
        exponent = biased - f128ExponentBias - f128FractionBits;
    }
    if (significand == 0)
        return negative ? "-0.0" : "0.0";
    string digits;
    int point;
    if (exponent >= 0)
    {
        digits = to!string(significand << cast(uint) exponent);
        point = cast(int) digits.length;
    }
    else
    {
        // m * 2^-k is exactly m * 5^k * 10^-k.
        const scale = -exponent;
        digits = to!string(significand * (BigInt(5) ^^ cast(uint) scale));
        point = cast(int) digits.length - scale;
    }
    while (digits.length > 0 && digits[$ - 1] == '0')
        digits = digits[0 .. $ - 1];
    if (digits.length == 0)
        digits = "0";
    return f128Render(negative, digits, point);
}

/// 0.<digits> * 10^point, in the three forms all the columns use.
private string f128Render(bool negative, string digits, int point)
{
    string repeatZeros(int count)
    {
        char[] zeros;
        foreach (i; 0 .. count)
            zeros ~= '0';
        return cast(string) zeros;
    }

    const sign = negative ? "-" : "";
    if (0 < point && point <= 34)
    {
        if (cast(int) digits.length <= point)
            return sign ~ digits ~ repeatZeros(point - cast(int) digits.length) ~ ".0";
        return sign ~ digits[0 .. point] ~ "." ~ digits[point .. $];
    }
    if (-6 < point && point <= 0)
        return sign ~ "0." ~ repeatZeros(-point) ~ digits;
    string mantissa = digits[0 .. 1] ~ ".";
    if (digits.length > 1)
        mantissa ~= digits[1 .. $];
    else
        mantissa ~= "0";
    const exponent = point - 1;
    if (exponent >= 0)
        return sign ~ mantissa ~ "e+" ~ to!string(exponent);
    return sign ~ mantissa ~ "e-" ~ to!string(-exponent);
}

/// The low `octets` of `value`'s two's-complement bits, zero-extended —
/// `WriteUnionOverlay`'s pattern for an integer, character, enumeration
/// or bitmask arm. The mask is the function: a plain widening of a
/// negative value sign-extends, and `-2` in four octets has to be
/// 0x00000000FFFFFFFE.
ulong BitPatternInt(long value, long octets)
{
    const pattern = cast(ulong) value;
    return octets >= 8 ? pattern : pattern & ((1UL << (octets * 8)) - 1);
}

/// One octet, 0 or 1 — the boolean overlay pattern.
ulong BitPatternBool(bool value)
{
    return value ? 1 : 0;
}

/// The IEEE 754 binary32 bits, zero-extended.
ulong BitPatternF32(float value)
{
    return *cast(uint*)&value;
}

/// The IEEE 754 binary64 bits.
ulong BitPatternF64(double value)
{
    return *cast(ulong*)&value;
}

version (unittest)
{
    // The three axes Dialect::Hdds selects. Expected octets from the
    // recorded HDDS columns:
    // tests/interop-results/wstring-member/hdds/plain-cdr2.le.bin
    // (`04 00 00 00 61 62 63 00` for "abc"),
    // tests/interop-results/array-1d-struct-final/hdds/plain-cdr2.le.bin
    // (`01 00 00 00 02 00 03 00 00 00 04 00 77`) and
    // tests/interop-results/sequence-struct-appendable/hdds/plain-cdr2.le.bin
    // (the first Cell of `one`, `06 00 00 00 79 29 ed ff fe ff`).
    private PlCdr1Framing hddsFraming()
    {
        return PlCdr1Framing("sentinel-must-understand", 0x3EFF, false, false, true, true);
    }

    @("a UTF-8 wstring counts its octets and a NUL")
    unittest
    {
        auto w = new Writer(Endianness.little, 4, hddsFraming(), false);
        w.WriteWstringUtf8Octets("abc", "octets/terminated/2");
        assert(w.Finish() == cast(ubyte[])[4, 0, 0, 0, 'a', 'b', 'c', 0]);
        auto r = new Reader(w.Finish(), Endianness.little, 4, "sentinel-must-understand", 0x3EFF);
        assert(r.ReadWstringUtf8Octets("octets/terminated/2") == "abc");
    }

    @("a bounded UTF-8 wstring bounds its UTF-16 units")
    unittest
    {
        auto w = new Writer(Endianness.little, 4, hddsFraming(), false);
        w.WriteWstringUtf8OctetsBounded("ሴäZ", 3, "octets/terminated/2");
        assert(w.Finish() == cast(ubyte[])[7, 0, 0, 0, 0xE1, 0x88, 0xB4, 0xC3, 0xA4, 'Z', 0]);
    }

    @("a packed element counts alignment from its own start")
    unittest
    {
        auto w = new Writer(Endianness.little, 4, hddsFraming(), false);
        int saved = w.BeginPackedElement();
        w.WriteI32(1);
        w.WriteI16(2);
        w.FinishPackedElement(saved);
        saved = w.BeginPackedElement();
        w.WriteI32(3);
        w.WriteI16(4);
        w.FinishPackedElement(saved);
        w.WriteU8(119);
        const recorded = cast(ubyte[])[1, 0, 0, 0, 2, 0, 3, 0, 0, 0, 4, 0, 119];
        assert(w.Finish() == recorded);
        auto r = new Reader(recorded, Endianness.little, 4, "sentinel-must-understand", 0x3EFF);
        saved = r.ReaderBeginPackedElement();
        assert(r.ReadI32() == 1);
        assert(r.ReadI16() == 2);
        r.ReaderFinishPackedElement(saved);
        saved = r.ReaderBeginPackedElement();
        assert(r.ReadI32() == 3);
        assert(r.ReadI16() == 4);
        r.ReaderFinishPackedElement(saved);
        assert(r.ReadU8() == 119);
    }

    @("a sequence element length frames the element it precedes")
    unittest
    {
        auto w = new Writer(Endianness.little, 4, hddsFraming(), false);
        const d = w.WriteDheader();
        w.WriteI32(-1234567);
        w.WriteI16(-2);
        w.FinishDheader(d);
        const got = w.Finish();
        assert(got == cast(ubyte[])[6, 0, 0, 0, 0x79, 0x29, 0xED, 0xFF, 0xFE, 0xFF]);
        auto r = new Reader(got, Endianness.little, 4, "sentinel-must-understand", 0x3EFF);
        const declared = cast(int) r.ReadU32();
        const endAt = r.ReaderPosition() + declared;
        assert(r.ReadI32() == -1234567);
        assert(r.ReadI16() == -2);
        assert(r.ReaderPosition() == endAt);
    }
}
