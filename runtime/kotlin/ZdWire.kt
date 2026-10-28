// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The Kotlin wire runtime: DDS-XTypes 1.3 §7.4 from the clause.
//
// The counterpart of `runtime/java`, `runtime/zig` and the other columns —
// a further implementation of the same clauses, not a binding to any of
// them. Where two columns disagree on an octet, one of them has misread
// the specification, and that is the point of having both. Nothing here
// decides a type question: which members are written, in what order,
// under which header and with which length code arrives as arguments from
// the generated codec (`crates/lang-rules/src/emit/kotlin.rs`), the same
// way it does for `runtime/java`.
//
// The wire-laying algorithms below — alignment relative to a moving
// origin, the DHEADER/EMHEADER back-patch, the PL_CDR1 short and extended
// parameter headers, the packed-BCD `fixed` — are the ones
// `endpoints/kotlin/src/Zerodds.kt` proved first, as a fixed-configuration
// endpoint SDK. This file is the same algorithms carrying the
// configurability a wire *probe* needs: which PL_CDR1 flag policy, which
// `wstring` length policy, which `wchar` framing — the axes
// `zerodds_ast_layout::LayoutKey` plans and the generated codec names by
// value rather than assumes.
//
// ## No unsigned types, and no exceptions across the codec walk either
//
// Kotlin shares the JVM's value model with Java: `writeU32` takes an
// `Int` and treats it as *a bit pattern*, because there is no unsigned
// `Int` to take it as a number in — `runtime/java`'s `Writer.writeU32`
// documents the same three-place agreement (here, in
// `crates/lang-rules/src/adapter/kotlin.rs`, and in
// `crates/lang-rules/src/emit/kotlin.rs`) and this file is the third.
//
// A malformed stream or a value outside its type throws [WireException],
// unchecked — Kotlin has no checked exceptions to declare (Kotlin
// language specification, "Exceptions") — and a refusal about the
// *type*, as DDS-XTypes 1.3 §7.2.2.7 describes for the reading direction,
// sets the reader's [Reader.Status] instead: the generated decoder walks
// on to the end of the sample rather than unwinding. Which is which is
// the same split `runtime/zig`'s module documentation draws between an
// error union and `readerRefuse`.
//
// ## `long double` has no JVM primitive
//
// [Float128] is a pair of `Long`s holding the sixteen bytes of an IEEE
// 754 binary128 in big-endian order, with `toString`/`parse` built over
// `java.math.BigDecimal` for the decimal round trip DDS-XTypes 1.3
// §7.4.1.4.4.2.2 asks an IDL `long double` for. It is not exact at every
// subnormal boundary; see the type's own documentation.

package zerodds.wire

import java.math.BigDecimal
import java.math.BigInteger

/** Every wire failure this runtime raises. Unchecked, as every Kotlin exception is. */
class WireException(message: String, val at: Int) : RuntimeException(message)

/** The byte order a stream is written or read in. */
enum class Endianness { LITTLE, BIG }

/** What a receiving position does with a value it cannot hold. DDS-XTypes 1.3 §7.2.2.7, Table 11. */
enum class TryConstruct { DISCARD, USE_DEFAULT, TRIM }

/** What a reader answers about the type, without throwing. */
enum class Status {
    OK, TRUNCATED, IMPOSSIBLE_LENGTH, OUT_OF_RANGE, DISCARDED;

    /** The status, spelled the way the sibling runtimes log it. */
    fun text(): String = when (this) {
        OK -> "ok"
        TRUNCATED -> "truncated"
        IMPOSSIBLE_LENGTH -> "impossible-length"
        OUT_OF_RANGE -> "out-of-range"
        DISCARDED -> "discarded"
    }
}

/** Which of §7.4.1.2's two roles a parameter header stands in. */
enum class ParameterRole { MEMBER, OPTIONAL }

/** A scalar `wchar`'s width in octets. */
enum class WcharWidth(val octets: Int) { OCTETS_2(2), OCTETS_4(4) }

/**
 * How a `wchar` is framed on the wire. `GIOP_LENGTH_PREFIXED` is CORBA
 * 3.3 part 2 §9.3.1.6's GIOP 1.2 form: the octet count first, then the
 * code unit's own octets, all aligned to one and big-endian unless the
 * stream is `STREAM`-ordered little-endian.
 */
enum class WcharFraming { FIXED, GIOP_LENGTH_PREFIXED }

/** The byte order a `wstring`'s code units go out in, independent of the stream's own. */
enum class WstringUnitOrder { STREAM, BIG_ENDIAN_ALWAYS, BYTE_ORDER_MARK, HOST_UNSWAPPED }

/**
 * How a `wstring`'s length word and terminator are read, over the
 * C1 x C2 x C3 product `runtime/zig/zdwire.zig` names in its own words:
 * whether the length counts UTF-16 units rather than octets, whether a
 * NUL terminator is written, and the code unit's width.
 */
enum class WstringLengthPolicy(val countsCharacters: Boolean, val terminated: Boolean, val width: Int) {
    /** §7.4.1.1.2: octets, no terminator, two-octet units. */
    SPECIFICATION(false, false, 2),
    /** Characters rather than octets, two-octet units. */
    CHARACTER_COUNT(true, false, 2),
    /** Characters, terminated, four-octet units. */
    CHARACTER_COUNT_TERMINATED(true, true, 4),
    /** Octets in wide (four-octet) units. */
    SPECIFICATION_WIDE_UNITS(false, false, 4),
    /** What `rosidl` writes: characters, four-octet units, unterminated. */
    ROS_TYPE_SUPPORT(true, false, 4),
    /** Octets, terminated, two-octet units. */
    OCTETS_TERMINATED_2(false, true, 2),
    /** Octets, terminated, four-octet units. */
    OCTETS_TERMINATED_4(false, true, 4),
    /** Characters, terminated, two-octet units. */
    CHARACTERS_TERMINATED_2(true, true, 2),
}

/**
 * How many digits a `fixed<P,S>` writes: the declared `P`, as many as the
 * value has (JacORB 3.9), or the integer digits of the decimal as written plus S, made odd
 * by one leading zero (ACE/TAO 2.5.24's `ACE_CDR::Fixed::from_string`, and
 * so OpenDDS; read like VALUE_PRECISION).
 */
enum class FixedDigitCount { DECLARED, VALUE_PRECISION, VALUE_STRING_DIGITS }

/**
 * What flags a PL_CDR1 parameter id carries, by the three positions a
 * flag question is asked at: an ordinary member, the PID_LIST_END
 * sentinel, and the PID_EXTENDED marker.
 */
enum class PidFlagPolicy { SENTINEL_MUST_UNDERSTAND, BARE, IMPL_EXTENSION_ON_MEMBERS }

private const val PID_EXTENDED: Int = 0x3F01
private const val PID_LIST_END: Int = 0x3F02
private const val PID_IGNORE: Int = 0x3F03
private const val PID_SHORT_MAX_LEN: Int = 0xFFFF
private const val PID_MUST_UNDERSTAND: Int = 0x4000
private const val PID_IMPL_EXTENSION: Int = 0x8000
private const val PID_ID_MASK: Int = 0x3FFF
private const val E_FLAG_MUST_UNDERSTAND: Long = 0x4000_0000L
private const val E_FLAG_IMPL_EXTENSION: Long = 0x8000_0000L
private const val E_MEMBER_ID_MASK: Long = 0x0FFF_FFFFL

private enum class PidPosition { MEMBER, SENTINEL, EXTENDED_MARKER }

private fun pidFlagsAt(policy: PidFlagPolicy, position: PidPosition): Int = when (policy) {
    PidFlagPolicy.SENTINEL_MUST_UNDERSTAND -> when (position) {
        PidPosition.SENTINEL, PidPosition.EXTENDED_MARKER -> PID_MUST_UNDERSTAND
        PidPosition.MEMBER -> 0
    }
    PidFlagPolicy.BARE -> 0
    PidFlagPolicy.IMPL_EXTENSION_ON_MEMBERS -> when (position) {
        PidPosition.MEMBER, PidPosition.EXTENDED_MARKER -> PID_IMPL_EXTENSION or PID_MUST_UNDERSTAND
        PidPosition.SENTINEL -> 0
    }
}

/**
 * How a PL_CDR1 parameter list is framed — the ways the measured
 * implementations differ inside an XCDR1 sample. One value serves both
 * [Writer] and [Reader]; the reader uses only [pidFlags] and
 * [highestShortId], because [Reader.finishPidRead] checks both readings
 * of the padding convention rather than assuming one.
 */
class PlCdr1Framing(
    /** What flags a parameter id carries, asked per position. */
    val pidFlags: PidFlagPolicy,
    /** The highest member id a short parameter header carries. */
    val highestShortId: Int,
    /** Whether a `@mutable` member's declared length covers its padding. */
    val padDeclaredLength: Boolean,
    /** The same question for an `@optional` member outside `@mutable`. */
    val padDeclaredOptionalLength: Boolean,
    /** Whether a member's own must-understand bit goes out at all. */
    val memberMustUnderstand: Boolean,
    /** Whether the parameter itself writes the padding after its value, or leaves the next header to align. */
    val padAfterValue: Boolean,
) {
    /** The specification's own reading: RTI's [padAfterValue] is false and every flag axis is at its plainest. */
    companion object {
        /** The specification's defaults, with no vendor's departures applied. */
        fun specification(): PlCdr1Framing = PlCdr1Framing(
            pidFlags = PidFlagPolicy.SENTINEL_MUST_UNDERSTAND,
            highestShortId = 0x3EFF,
            padDeclaredLength = false,
            padDeclaredOptionalLength = false,
            memberMustUnderstand = true,
            padAfterValue = true,
        )
    }

    internal fun padsDeclaredLength(role: ParameterRole): Boolean = when (role) {
        ParameterRole.OPTIONAL -> padDeclaredOptionalLength
        ParameterRole.MEMBER -> padDeclaredLength
    }
}

// -- handles -----------------------------------------------------------------

/** A DHEADER waiting for its length; `at` of `-1` is the delimiter a `@final` aggregate does not write. */
class DHeader internal constructor(internal val at: Int)

/** The DHEADER a value that takes no delimiter does not write; [Writer.finishDheader] ignores it. */
fun noDheader(): DHeader = DHeader(-1)

/** An EMHEADER waiting for its NEXTINT. */
class EmHeader internal constructor(
    internal val nextIntAt: Int,
    internal val wordAt: Int,
    internal val lengthCode: Int,
)

/** A PL_CDR1 parameter header waiting for its length. */
class Pid internal constructor(
    internal val at: Int,
    internal val extended: Boolean,
    internal val savedOrigin: Int,
    internal val role: ParameterRole,
    internal val pid: Int,
    internal val mustUnderstand: Boolean,
)

/** Everything a two-pass read has to put back. */
class ReaderMark internal constructor(
    internal val at: Int,
    internal val origin: Int,
    internal val status: Status,
)

/**
 * An EMHEADER as read. [extent] is how many octets the member occupies
 * after the header word, the NEXTINT included, XTypes 1.3 §7.4.3.4.2.
 */
class EmHeaderRead internal constructor(
    val memberId: Int,
    val lengthCode: Int,
    val mustUnderstand: Boolean,
    internal val nextInt: Long,
    internal val hasNextInt: Boolean,
    val extent: Long,
)

/** A PL_CDR1 parameter header as read. */
class PidRead internal constructor(
    val pid: Int,
    val length: Long,
    val isEnd: Boolean,
    val mustUnderstand: Boolean,
    val implExtension: Boolean,
    val reserved: Boolean,
    val ignore: Boolean,
    val role: ParameterRole,
    internal val savedOrigin: Int,
    internal val valueStart: Int,
) {
    /** Whether this parameter can be a member of the reading type: §7.4.1.2.2, §7.2.2.4.4.4.6. */
    fun isUserMember(): Boolean = !implExtension && !reserved && !ignore
}

// -- helpers the generated code names -----------------------------------------

/**
 * C11 §7.24.4's unsigned-octet order, as the sign the walk compares.
 * [String.compareTo] over Kotlin's UTF-16 `String` is the same order as
 * a byte-wise UTF-8 comparison for every codepoint below U+E000 — every
 * corpus map key — and the ordering the generated codec calls this for
 * is documented at the call site, `crates/lang-rules/src/emit/kotlin.rs`.
 */
fun strcmp(a: String, b: String): Int = a.compareTo(b).let { if (it < 0) -1 else if (it > 0) 1 else 0 }

/** The value's two's-complement bits in the low `octets`, zero-extended — [Writer.writeUnionOverlay]'s pattern. */
fun bitPatternInt(value: Long, octets: Int): Long {
    if (octets >= 8) return value
    val bits = octets * 8
    return value and ((1L shl bits) - 1L)
}

/** One octet, 0 or 1 — the boolean overlay pattern. */
fun bitPatternBool(value: Boolean): Long = if (value) 1L else 0L

/** The IEEE 754 binary32 bits, zero-extended. */
fun bitPatternF32(value: Float): Long = java.lang.Float.floatToRawIntBits(value).toLong() and 0xFFFF_FFFFL

/** The IEEE 754 binary64 bits. */
fun bitPatternF64(value: Double): Long = java.lang.Double.doubleToRawLongBits(value)

/** `point` as the one octet an IDL `char` is, refused above U+00FF rather than truncated. §7.2.2.2.1.2.2. */
fun checkedChar(point: Int, at: Int): Int {
    if (point < 0 || point > 0xFF) throw WireException("expected a character below U+0100, found U+%04X".format(point), at)
    return point
}

/** `point` as the one UTF-16 code unit an IDL `wchar` is, refused outside the BMP. §7.2.2.2.1.2.5. */
fun checkedWchar(point: Int, at: Int): Int {
    if (point < 0 || point > 0xFFFF) throw WireException("expected a BMP character, found U+%04X".format(point), at)
    return point
}

/**
 * How many octets `text` is, which is what a `string<N>` bound counts.
 *
 * `string<5>` is `char[6]` in the C mapping of every measured DDS
 * implementation, so a five-scalar value of ten octets is one no peer can
 * hold. A Kotlin `String` is UTF-16, so the count is of its UTF-8 encoding
 * and not of its length — the same answer `Reader.utf8Octets` gives in the
 * Java column.
 */
fun utf8Octets(text: String): Int = text.toByteArray(Charsets.UTF_8).size

/**
 * How many UTF-16 code units `text` is, which is what a `wstring<N>` bound
 * counts. A surrogate pair is two, which is what [String.length] answers
 * already.
 */
fun utf16Units(text: String): Int = text.length

/**
 * The UTF-8 octets of `text`, refusing a lone surrogate.
 *
 * `String.toByteArray(Charsets.UTF_8)` replaces an unpaired surrogate with
 * `?` and reports success — a different character on the wire, silently. The
 * encoder is configured to report instead. `WstringCodeUnits::Utf8`.
 */
private fun strictUtf8Octets(text: String, at: Int): ByteArray {
    val encoder = Charsets.UTF_8.newEncoder()
        .onMalformedInput(java.nio.charset.CodingErrorAction.REPORT)
        .onUnmappableCharacter(java.nio.charset.CodingErrorAction.REPORT)
    return try {
        val buffer = encoder.encode(java.nio.CharBuffer.wrap(text))
        val octets = ByteArray(buffer.remaining())
        buffer.get(octets)
        octets
    } catch (error: java.nio.charset.CharacterCodingException) {
        throw WireException("an unpaired surrogate is not a code point", at)
    }
}

/**
 * `text` from a run of UTF-8 octets, refusing anything ill formed.
 *
 * `String(bytes, Charsets.UTF_8)` replaces a malformed sequence with U+FFFD
 * and reports success; this is the inverse of [strictUtf8Octets] and refuses
 * instead. `WstringCodeUnits::Utf8`.
 */
private fun strictUtf8Text(octets: ByteArray, from: Int, count: Int, at: Int): String {
    val decoder = Charsets.UTF_8.newDecoder()
        .onMalformedInput(java.nio.charset.CodingErrorAction.REPORT)
        .onUnmappableCharacter(java.nio.charset.CodingErrorAction.REPORT)
    return try {
        decoder.decode(java.nio.ByteBuffer.wrap(octets, from, count)).toString()
    } catch (error: java.nio.charset.CharacterCodingException) {
        throw WireException("a wstring's octets are not UTF-8", at)
    }
}

// -- the writer ----------------------------------------------------------------

/**
 * Accumulates the bytes of one sample. Alignment is relative to a moving
 * origin — the start of the stream ordinarily, and the start of a
 * PL_CDR1 parameter's own value for the length that parameter is open.
 * XTypes 1.3 §7.4.1.1, §7.4.1.2.
 */
class Writer(
    private val endianness: Endianness,
    maxAlign: Int,
    private val framing: PlCdr1Framing,
    /** Whether an LC4 header whose member turns out to be 1, 2, 4 or 8 bytes is rewritten with a short length code. */
    private val shortLengthCodes: Boolean,
) {
    private val maxAlign: Int = if (maxAlign < 1) 1 else maxAlign
    private val out = ArrayList<Byte>()
    private var origin = 0

    /** How many bytes have been written. */
    fun position(): Int = out.size

    /** The bytes written. */
    fun finish(): ByteArray = out.toByteArray()

    private fun alignTo(alignment: Int) {
        val effective = if (alignment > maxAlign) maxAlign else alignment
        if (effective <= 1) return
        val offset = out.size - origin
        val padding = (effective - offset % effective) % effective
        repeat(padding) { out.add(0) }
    }

    private fun writeRaw(unsigned: Long, width: Int) {
        for (i in 0 until width) {
            val shift = 8 * (if (endianness == Endianness.LITTLE) i else width - 1 - i)
            out.add((unsigned ushr shift).toByte())
        }
    }

    private fun writeInt(unsigned: Long, width: Int) {
        alignTo(width)
        writeRaw(unsigned, width)
    }

    // -- primitives ------------------------------------------------------------

    /** `boolean`: one byte, 0 or 1. */
    fun writeBool(v: Boolean) { out.add(if (v) 1 else 0) }

    /** `octet` and `uint8`, as the low octet of `v`. */
    fun writeU8(v: Int) { out.add(v.toByte()) }

    /** `int8`. */
    fun writeI8(v: Int) { out.add(v.toByte()) }

    /** `char`: one octet — the whole member. */
    fun writeChar(v: Int) { out.add(v.toByte()) }

    /** `short`. */
    fun writeI16(v: Int) = writeInt(v.toLong() and 0xFFFFL, 2)

    /** `unsigned short`. */
    fun writeU16(v: Int) = writeInt(v.toLong() and 0xFFFFL, 2)

    /** `long`. */
    fun writeI32(v: Int) = writeInt(v.toLong() and 0xFFFF_FFFFL, 4)

    /** `unsigned long`. */
    fun writeU32(v: Long) = writeInt(v and 0xFFFF_FFFFL, 4)

    /** `long long`. */
    fun writeI64(v: Long) = writeInt(v, 8)

    /** `unsigned long long`, as the bit pattern `v` holds. */
    fun writeU64(v: Long) = writeInt(v, 8)

    /** `float`. */
    fun writeF32(v: Float) = writeInt(java.lang.Float.floatToRawIntBits(v).toLong() and 0xFFFF_FFFFL, 4)

    /** `double`. */
    fun writeF64(v: Double) = writeInt(java.lang.Double.doubleToRawLongBits(v), 8)

    /** `long double`: sixteen bytes of IEEE 754 binary128, aligned to eight (capped by `maxAlign`). */
    fun writeF128(v: Float128) {
        alignTo(8)
        val bytes = v.bigEndianBytes()
        for (i in 0 until 16) {
            val index = if (endianness == Endianness.LITTLE) 15 - i else i
            out.add(bytes[index])
        }
    }

    // -- characters and strings -------------------------------------------------

    /**
     * `wchar`, at `width.octets` and in `framing`'s shape. `GIOP_LENGTH_PREFIXED`
     * is CORBA 3.3 part 2 §9.3.1.6's GIOP 1.2 form: the octet count first, then
     * the code unit's octets, all aligned to one; every order but `STREAM` means
     * big-endian there.
     */
    fun writeWchar(v: Int, width: WcharWidth, framing: WcharFraming, order: WstringUnitOrder) {
        if (framing == WcharFraming.GIOP_LENGTH_PREFIXED) {
            out.add(width.octets.toByte())
            val big = order != WstringUnitOrder.STREAM || endianness != Endianness.LITTLE
            for (i in 0 until width.octets) {
                val index = if (big) width.octets - 1 - i else i
                out.add(((v ushr (8 * index)) and 0xFF).toByte())
            }
            return
        }
        if (width == WcharWidth.OCTETS_4) writeU32(v.toLong()) else writeU16(v)
    }

    /** A string: a four-byte length including the terminator, the UTF-8 bytes, then a NUL. XTypes 1.3 §7.4.1.1. */
    fun writeString(v: String) {
        val raw = v.toByteArray(Charsets.UTF_8)
        writeU32((raw.size + 1).toLong())
        for (b in raw) out.add(b)
        out.add(0)
    }

    /**
     * A bounded `string`: the same bytes [writeString] writes. The bound is
     * a promise about the source and not a difference on the wire, so this
     * is the only place it can be enforced — refused rather than cut at a
     * character boundary, which would put a sample on the wire nobody wrote.
     */
    fun writeStringBounded(v: String, bound: Int) {
        val octets = v.toByteArray(Charsets.UTF_8).size
        if (octets > bound) {
            throw WireException(
                "at most $bound octets in a bounded string, and this one has $octets",
                position())
        }
        writeString(v)
    }

    /**
     * A bounded `wstring`, as [writeStringBounded] is to [writeString]. The
     * bound counts UTF-16 code units, not characters and not octets,
     * whatever `policy` puts on the wire.
     */
    fun writeWstringBounded(
        v: String, bound: Int, policy: WstringLengthPolicy, order: WstringUnitOrder
    ) {
        val units = utf16Units(v)
        if (units > bound) {
            throw WireException(
                "at most $bound UTF-16 code units in a bounded wstring, and this one has $units",
                position())
        }
        writeWstringUtf8(v, policy, order)
    }

    /**
     * The count check every bounded `sequence` makes before a byte of it is
     * written. Table 17 makes `O2.length <= T1.length` the condition for
     * constructing a `sequence<T, N>`, and the wire carries no bound to
     * compare against: a sender whose count already exceeds its own
     * declaration has written a value of a type nobody declared.
     */
    fun checkSequenceBound(actual: Int, bound: Int) {
        if (actual > bound) {
            throw WireException(
                "at most $bound elements in a bounded sequence, and this one has $actual",
                position())
        }
    }

    /**
     * The count check every bounded `map` makes before a byte of it is
     * written. Its own message and not [checkSequenceBound]'s: the unit is
     * an entry and not an element, per XTypes 1.3 §7.4.1.1.4's
     * map-as-a-sequence-of-pairs equivalence.
     */
    fun checkMapBound(actual: Int, bound: Int) {
        if (actual > bound) {
            throw WireException(
                "at most $bound entries in a bounded map, and this one has $actual",
                position())
        }
    }

    /**
     * A `wstring`, in the form `policy` and `order` name. The value
     * arrives as a Kotlin `String` — already UTF-16 — so the surrogate
     * accounting of §7.4.1.1.2 is read straight off Kotlin's own code
     * units rather than rebuilt from UTF-8, unlike the C-family runtimes.
     */
    fun writeWstring(v: String, policy: WstringLengthPolicy, order: WstringUnitOrder) {
        var units = IntArray(v.length) { v[it].code }
        if (policy.countsCharacters && policy.width == 2) {
            for (unit in units) if (unit in 0xD800..0xDFFF) {
                throw WireException("a character-count wstring cannot carry a codepoint outside the BMP", position())
            }
        }
        if (policy.terminated) units += 0
        if (order == WstringUnitOrder.BYTE_ORDER_MARK && units.isNotEmpty()) {
            units = intArrayOf(0xFEFF) + units
        }
        if (policy.countsCharacters) writeU32(units.size.toLong()) else writeU32((units.size * policy.width).toLong())
        for (unit in units) writeWstringUnit(unit, policy.width, order)
    }

    /**
     * [writeWstring] under the name the shared walk calls it by.
     *
     * `runtime/c` converts to UTF-8 because that is what its `const char *`
     * holds, and its name carries the conversion. A Kotlin `String` is UTF-16
     * already, so there is nothing to convert and [writeWstring] is the whole
     * of the operation — the same delegation `runtime/java`'s
     * `Writer.writeWstringUtf8` makes, for the same reason.
     */
    fun writeWstringUtf8(v: String, policy: WstringLengthPolicy, order: WstringUnitOrder) =
        writeWstring(v, policy, order)

    /**
     * Move the alignment origin to the current position.
     *
     * `ElementPadding::Packed` is HDDS's measured departure from XTypes 1.3
     * §7.4.1, which counts every element's alignment from the encapsulation:
     * each collection element and each aggregate value counts its own
     * alignment from its own first octet instead. `array-1d-struct-final` is
     * `01000000 0200 03000000 0400 77`, the second `Cell`'s `long` at offset
     * six where the clause puts it at eight, and `typedef-of-struct` is the
     * same inside two `@final` members.
     *
     * Returns the origin displaced, for [finishPackedElement].
     */
    fun beginPackedElement(): Int {
        val saved = origin
        origin = out.size
        return saved
    }

    /** Restore the origin [beginPackedElement] displaced. */
    fun finishPackedElement(saved: Int) {
        origin = saved
    }

    /**
     * A `wstring` whose characters go out as their own UTF-8 octets.
     *
     * `WstringCodeUnits::Utf8` is the measured departure of one product's
     * generated serialiser: XTypes 1.3 Table 31 gives a `wstring`'s code unit
     * as `Char16` and §7.4.1.1.2 makes the length "twice the number of
     * characters", so the clause has no UTF-8 form at all. The declared
     * length is the UTF-8 octet count and only C2 — the terminator — still
     * reaches the wire. `wstring-member` is `04 00 00 00 61 62 63 00` for
     * `"abc"`: three octets, a NUL, and the four declares both.
     */
    fun writeWstringUtf8Octets(v: String, policy: WstringLengthPolicy) {
        val raw = strictUtf8Octets(v, position())
        val length = raw.size.toLong() + if (policy.terminated) 1L else 0L
        if (length > 0xFFFF_FFFFL) {
            throw WireException("a wstring length the UInt32 field can hold", position())
        }
        writeU32(length)
        for (b in raw) out.add(b)
        if (policy.terminated) out.add(0)
    }

    /**
     * A bounded UTF-8-octet `wstring`. The bound still counts UTF-16 code
     * units, which is what a `wstring<N>` bound means, while the wire carries
     * the UTF-8 octets.
     */
    fun writeWstringUtf8OctetsBounded(v: String, bound: Int, policy: WstringLengthPolicy) {
        val units = utf16Units(v)
        if (units > bound) {
            throw WireException(
                "at most $bound UTF-16 code units in a bounded wstring, and this one has $units",
                position())
        }
        writeWstringUtf8Octets(v, policy)
    }

    private fun writeWstringUnit(unit: Int, width: Int, order: WstringUnitOrder) {
        val little = order == WstringUnitOrder.BYTE_ORDER_MARK || order == WstringUnitOrder.HOST_UNSWAPPED
        if (width == 4) {
            when {
                order == WstringUnitOrder.BIG_ENDIAN_ALWAYS -> writeU32Big(unit.toLong())
                little -> writeU32Little(unit.toLong())
                else -> writeU32(unit.toLong())
            }
            return
        }
        when {
            order == WstringUnitOrder.BIG_ENDIAN_ALWAYS -> writeU16Big(unit)
            little -> writeU16Little(unit)
            else -> writeU16(unit)
        }
    }

    /** A 16-bit value little-endian whatever the stream's order is. */
    fun writeU16Little(v: Int) {
        alignTo(2)
        out.add(v.toByte())
        out.add((v ushr 8).toByte())
    }

    /** A 16-bit value big-endian whatever the stream's order is. */
    fun writeU16Big(v: Int) {
        alignTo(2)
        out.add((v ushr 8).toByte())
        out.add(v.toByte())
    }

    /** A 32-bit value little-endian whatever the stream's order is. */
    fun writeU32Little(v: Long) {
        alignTo(4)
        for (i in 0 until 4) out.add((v ushr (8 * i)).toByte())
    }

    /** A 32-bit value big-endian whatever the stream's order is. */
    fun writeU32Big(v: Long) {
        alignTo(4)
        for (i in 0 until 4) out.add((v ushr (8 * (3 - i))).toByte())
    }

    // -- fixed -------------------------------------------------------------------

    /** `fixed<digits,scale>` as packed BCD, [CDR] §9.3.2.8 — the only wire definition an IDL `fixed` has. */
    fun writeFixed(v: String, digits: Int, scale: Int, count: FixedDigitCount) {
        val nibbles = fixedNibbles(v, digits, scale, count, position())
        var i = 0
        while (i < nibbles.size) {
            out.add(((nibbles[i] shl 4) or nibbles[i + 1]).toByte())
            i += 2
        }
    }

    // -- the bit-bounded kinds -----------------------------------------------------

    /** An enumerator, as the integer its bit bound calls for. §7.4.3.5.3 rule (5): the holder is signed. */
    fun writeEnum(bitBound: Int, value: Long) {
        if (bitBound !in 1..32) throw WireException("an enumeration's bit bound is 1..32, not $bitBound", position())
        val low = -(1L shl (bitBound - 1))
        val high = (1L shl bitBound) - 1L
        if (value < low || value > high) throw WireException("$value does not fit $bitBound bits", position())
        val mask = if (bitBound == 64) -1L else (1L shl bitBound) - 1L
        val pattern = value and mask
        when {
            bitBound <= 8 -> writeU8(pattern.toInt())
            bitBound <= 16 -> writeU16(pattern.toInt())
            else -> writeU32(pattern)
        }
    }

    /** A bitmask, as the unsigned integer Table 33 gives its bound. */
    fun writeBitmask(bitBound: Int, value: Long) {
        if (bitBound !in 1..64) throw WireException("a bitmask's bit bound is 1..64, not $bitBound", position())
        if (bitBound < 64 && (value ushr bitBound) != 0L) {
            throw WireException("$value does not fit $bitBound bits", position())
        }
        when {
            bitBound <= 8 -> writeU8(value.toInt())
            bitBound <= 16 -> writeU16(value.toInt())
            bitBound <= 32 -> writeU32(value)
            else -> writeU64(value)
        }
    }

    /** One bitfield, shifted into the accumulator a bitset packs into. A value wider than the field is refused. */
    fun bitfield(value: Long, width: Int, pos: Int): Long {
        if (width !in 1..64 || pos + width > 64) {
            throw WireException("a bitfield of $width bits at $pos does not fit the anchor", position())
        }
        if (width < 64 && (value ushr width) != 0L) {
            throw WireException("$value does not fit $width bits", position())
        }
        return value shl pos
    }

    /**
     * The Cyclone union overlay: the low `octets` of `pattern`, written
     * as one unsigned integer of that width in the stream's byte order.
     */
    fun writeUnionOverlay(pattern: Long, octets: Int) {
        when (octets) {
            1 -> writeU8(pattern.toInt())
            2 -> writeU16(pattern.toInt())
            4 -> writeU32(pattern)
            8 -> writeU64(pattern)
            else -> throw WireException("a union overlay is 1, 2, 4 or 8 octets, not $octets", position())
        }
    }

    /** A bitset, as the anchor integer its bitfields pack into. */
    fun writeBitset(anchor: Int, value: Long) {
        when (anchor) {
            1 -> writeU8(value.toInt())
            2 -> writeU16(value.toInt())
            4 -> writeU32(value)
            8 -> writeU64(value)
            else -> throw WireException("a bitset anchor is 1, 2, 4 or 8 octets, not $anchor", position())
        }
    }

    // -- framing -------------------------------------------------------------------

    /** A DHEADER: a four-byte length placeholder, patched when the members are done. */
    fun writeDheader(): DHeader {
        alignTo(4)
        val at = out.size
        out.add(0); out.add(0); out.add(0); out.add(0)
        return DHeader(at)
    }

    /** Patch a DHEADER with the length of what followed it; a header from [noDheader] is ignored. */
    fun finishDheader(header: DHeader) {
        if (header.at < 0) return
        patch(header.at, (out.size - header.at - 4).toLong(), 4)
    }

    /** An XCDR2 EMHEADER: id, length code and must-understand in one 32-bit word. §7.4.3.4.2. */
    fun writeEmheader(memberId: Int, lengthCode: Int, mustUnderstand: Boolean): EmHeader {
        if (memberId.toLong() and E_MEMBER_ID_MASK.inv() != 0L) {
            throw WireException("member id $memberId does not fit the 28 bits of an EMHEADER", position())
        }
        alignTo(4)
        var word = ((lengthCode.toLong() and 0x7L) shl 28) or (memberId.toLong() and E_MEMBER_ID_MASK)
        if (mustUnderstand) word = word or 0x8000_0000L
        writeU32(word)
        val wordAt = out.size - 4
        return if (lengthCode == 4) {
            val at = out.size
            out.add(0); out.add(0); out.add(0); out.add(0)
            EmHeader(at, wordAt, lengthCode)
        } else {
            EmHeader(-1, wordAt, lengthCode)
        }
    }

    /** Patch an EMHEADER's NEXTINT; under [shortLengthCodes] an LC4 member of 1, 2, 4 or 8 bytes loses it. */
    fun finishEmheader(header: EmHeader) {
        if (header.nextIntAt < 0) {
            if (header.lengthCode in 0..3 && header.wordAt + 4 <= out.size) {
                val declared = 1 shl header.lengthCode
                val body = out.size - header.wordAt - 4
                if (body != declared) {
                    throw WireException("LC${header.lengthCode} frames $declared octets and the member wrote $body", position())
                }
            }
            return
        }
        val length = out.size - header.nextIntAt - 4
        if (shortLengthCodes && (length == 1 || length == 2 || length == 4 || length == 8)) {
            shortenEmheader(header, length)
            return
        }
        patch(header.nextIntAt, length.toLong(), 4)
    }

    private fun shortenEmheader(header: EmHeader, length: Int) {
        val code = when (length) { 1 -> 0L; 2 -> 1L; 4 -> 2L; else -> 3L }
        var i = header.nextIntAt
        while (i + 4 < out.size) { out[i] = out[i + 4]; i++ }
        repeat(4) { out.removeAt(out.size - 1) }
        val previous = readPatched(header.wordAt)
        patch(header.wordAt, (previous and (0x7L shl 28).inv()) or (code shl 28), 4)
    }

    private fun readPatched(at: Int): Long {
        var value = 0L
        for (i in 0 until 4) {
            val index = if (endianness == Endianness.LITTLE) at + 3 - i else at + i
            value = (value shl 8) or (out[index].toLong() and 0xFF)
        }
        return value
    }

    /** A PL_CDR1 parameter header, short form: `[id:u16][length:u16]`. */
    fun writePid(id: Int, role: ParameterRole, mustUnderstand: Boolean): Pid {
        if (id > framing.highestShortId) {
            throw WireException("member id $id needs the extended parameter header", position())
        }
        alignTo(4)
        var pid = id
        if (mustUnderstand && framing.memberMustUnderstand) pid = pid or PID_MUST_UNDERSTAND
        pid = pid or pidFlagsAt(framing.pidFlags, PidPosition.MEMBER)
        val bare = pid and (PID_MUST_UNDERSTAND or PID_IMPL_EXTENSION).inv()
        writeU16(pid)
        val at = out.size
        out.add(0); out.add(0)
        return Pid(at, false, beginParameterValue(), role, bare, mustUnderstand)
    }

    /** A PL_CDR1 parameter header, extended form: `[PID_EXTENDED][8][id:u32][length:u32]`. §7.4.1.2. */
    fun writePidExtended(id: Int, role: ParameterRole, mustUnderstand: Boolean): Pid {
        if (id.toLong() and E_MEMBER_ID_MASK.inv() != 0L) {
            throw WireException("member id $id does not fit the 28 bits of an eMemberHeader", position())
        }
        alignTo(4)
        val marker = PID_EXTENDED or pidFlagsAt(framing.pidFlags, PidPosition.EXTENDED_MARKER)
        writeU16(marker)
        writeU16(8)
        var header = id.toLong() and E_MEMBER_ID_MASK
        if (mustUnderstand && framing.memberMustUnderstand) header = header or E_FLAG_MUST_UNDERSTAND
        writeU32(header)
        val at = out.size
        out.add(0); out.add(0); out.add(0); out.add(0)
        return Pid(at, true, beginParameterValue(), role, 0, false)
    }

    private fun beginParameterValue(): Int {
        val saved = origin
        origin = out.size
        return saved
    }

    /** Patch a parameter header with its value's length; the padding to four goes inside it or outside it, by framing. */
    fun finishPid(header: Pid) {
        val width = if (header.extended) 4 else 2
        var length = out.size - header.at - width
        if (framing.padsDeclaredLength(header.role)) {
            val past = (out.size - header.savedOrigin) % 4
            if (past != 0) length += 4 - past
        }
        if (!header.extended && length > PID_SHORT_MAX_LEN) {
            promotePid(header, length)
        } else {
            patch(header.at, length.toLong(), width)
        }
        origin = header.savedOrigin
        if (framing.padAfterValue) alignTo(4)
    }

    private fun promotePid(header: Pid, length: Int) {
        val start = header.at - 2
        repeat(8) { out.add(0) }
        var i = out.size
        while (i > header.at + 2 + 8) {
            i--
            out[i] = out[i - 8]
        }
        val marker = PID_EXTENDED or pidFlagsAt(framing.pidFlags, PidPosition.EXTENDED_MARKER)
        patch(start, marker.toLong(), 2)
        patch(start + 2, 8, 2)
        var memberHeader = header.pid.toLong() and E_MEMBER_ID_MASK
        if (header.mustUnderstand && framing.memberMustUnderstand) memberHeader = memberHeader or E_FLAG_MUST_UNDERSTAND
        patch(start + 4, memberHeader, 4)
        patch(start + 8, length.toLong(), 4)
    }

    /** The PID_LIST_END sentinel a PL_CDR1 parameter list ends with. */
    fun writeSentinel() {
        alignTo(4)
        val sentinel = PID_LIST_END or pidFlagsAt(framing.pidFlags, PidPosition.SENTINEL)
        writeU16(sentinel)
        writeU16(0)
    }

    private fun patch(at: Int, value: Long, width: Int) {
        for (i in 0 until width) {
            val shift = 8 * i
            val index = if (endianness == Endianness.LITTLE) at + i else at + width - 1 - i
            out[index] = (value ushr shift).toByte()
        }
    }
}

// -- the reader ------------------------------------------------------------------

/** Reads one sample's bytes: the mirror of [Writer], clause for clause. */
class Reader(
    private val data: ByteArray,
    private val endianness: Endianness,
    maxAlign: Int,
    private val framing: PlCdr1Framing,
) {
    private val maxAlign: Int = if (maxAlign < 1) 1 else maxAlign
    private var at = 0
    private var origin = 0
    private var status = Status.OK

    /** How the read is going. */
    fun readerStatus(): Status = status

    /** Refuse the stream from outside the runtime; the first refusal sticks, and `OK` refuses nothing. */
    fun readerRefuse(refusal: Status) {
        if (refusal != Status.OK && status == Status.OK) status = refusal
    }

    /** Take back a `DISCARDED`, and only that one: a truncated stream is not a narrowing. */
    fun readerClearDiscard(): Boolean {
        if (status == Status.DISCARDED) { status = Status.OK; return true }
        return false
    }

    /** Where the reader is. */
    fun readerPosition(): Int = at

    /** How many bytes are left. */
    fun readerRemaining(): Int = if (at > data.size) 0 else data.size - at

    /**
     * Move the alignment origin to the current position.
     *
     * The read-side pair of the writer's [Writer.beginPackedElement], for
     * `ElementPadding::Packed`. Returns the origin displaced.
     */
    fun readerBeginPackedElement(): Int {
        val saved = origin
        origin = at
        return saved
    }

    /** Restore the origin [readerBeginPackedElement] displaced. */
    fun readerFinishPackedElement(saved: Int) {
        origin = saved
    }

    /** Move the cursor forward to `target`, never backwards. */
    fun readerSkipTo(target: Int) {
        if (status != Status.OK) return
        if (target < at) { readerRefuse(Status.OUT_OF_RANGE); return }
        if (target > data.size) throw WireException("cannot skip past the end of the stream", at)
        at = target
    }

    /** The cursor, the origin and the status, to be put back later. */
    fun readerSave(): ReaderMark = ReaderMark(at, origin, status)

    /** Put back what [readerSave] took. */
    fun readerRestore(mark: ReaderMark) {
        at = mark.at
        origin = mark.origin
        status = mark.status
    }

    private fun alignTo(alignment: Int) {
        val effective = if (alignment > maxAlign) maxAlign else alignment
        if (effective <= 1) return
        val offset = at - origin
        at += (effective - offset % effective) % effective
    }

    private fun take(count: Int): ByteArray {
        if (at > data.size || count > data.size - at) throw WireException("the stream ended before the value did", at)
        val chunk = data.copyOfRange(at, at + count)
        at += count
        return chunk
    }

    private fun readUnsigned(width: Int): Long {
        alignTo(width)
        val raw = take(width)
        var value = 0L
        for (i in 0 until width) {
            val index = if (endianness == Endianness.LITTLE) width - 1 - i else i
            value = (value shl 8) or (raw[index].toLong() and 0xFF)
        }
        return value
    }

    // -- primitives ---------------------------------------------------------------

    /** `boolean`; a byte that is neither 0 nor 1 is refused rather than coerced. */
    fun readBool(): Boolean = when (val v = take(1)[0].toInt() and 0xFF) {
        0 -> false
        1 -> true
        else -> throw WireException("a boolean is 0 or 1, not $v", at - 1)
    }

    /** `octet` and `uint8`. */
    fun readU8(): Int = take(1)[0].toInt() and 0xFF

    /** `int8`. */
    fun readI8(): Int = take(1)[0].toInt()

    /** `char`: one octet. */
    fun readChar(): Int = take(1)[0].toInt() and 0xFF

    /** `short`. */
    fun readI16(): Int = readUnsigned(2).toShort().toInt()

    /** `unsigned short`. */
    fun readU16(): Int = readUnsigned(2).toInt() and 0xFFFF

    /** `long`. */
    fun readI32(): Int = readUnsigned(4).toInt()

    /** `unsigned long`. */
    fun readU32(): Long = readUnsigned(4) and 0xFFFF_FFFFL

    /** `long long`. */
    fun readI64(): Long = readUnsigned(8)

    /** `unsigned long long`, as the bit pattern the wire held. */
    fun readU64(): Long = readUnsigned(8)

    /** `float`. */
    fun readF32(): Float = java.lang.Float.intBitsToFloat(readUnsigned(4).toInt())

    /** `double`. */
    fun readF64(): Double = java.lang.Double.longBitsToDouble(readUnsigned(8))

    /** `long double`: all sixteen bytes, as the binary128 the member holds. */
    fun readF128(): Float128 {
        alignTo(8)
        val raw = take(16)
        val bytes = ByteArray(16)
        for (i in 0 until 16) {
            val index = if (endianness == Endianness.LITTLE) 15 - i else i
            bytes[i] = raw[index]
        }
        return Float128.ofBigEndianBytes(bytes)
    }

    /** `wchar`, at `width.octets` and in `framing`'s shape. A lone surrogate is refused. */
    fun readWchar(width: WcharWidth, framing: WcharFraming, order: WstringUnitOrder): Int {
        var value: Int
        if (framing == WcharFraming.GIOP_LENGTH_PREFIXED) {
            val declared = readU8()
            if (declared != width.octets) throw WireException("a GIOP wchar declared $declared octets and the type has ${width.octets}", at)
            val big = order != WstringUnitOrder.STREAM || endianness != Endianness.LITTLE
            val raw = take(width.octets)
            value = 0
            for (i in 0 until width.octets) {
                val index = if (big) i else width.octets - 1 - i
                value = (value shl 8) or (raw[index].toInt() and 0xFF)
            }
        } else if (width == WcharWidth.OCTETS_4) {
            val wide = readU32()
            if (wide > 0xFFFFL) throw WireException("a wchar above the BMP has no fixed-width form", at)
            value = wide.toInt()
        } else {
            value = readU16()
        }
        if (value in 0xD800..0xDFFF) throw WireException("a wchar cannot be a lone surrogate", at)
        return value
    }

    // -- strings --------------------------------------------------------------------

    /** A string: a length including the terminator, the bytes, the NUL. */
    fun readString(): String {
        val length = readU32()
        if (length == 0L || length > readerRemaining().toLong() + 1L) {
            throw WireException("a string's declared length does not fit the stream", at)
        }
        val text = take((length - 1).toInt())
        readU8()
        return String(text, Charsets.UTF_8)
    }

    /** [readString] under the name the shared walk calls it by: the value is already a fresh `String`. */
    fun readStringAlloc(): String = readString()

    /** A 16-bit value little-endian whatever the stream's order is. */
    fun readU16Little(): Int {
        alignTo(2)
        val raw = take(2)
        return (raw[0].toInt() and 0xFF) or ((raw[1].toInt() and 0xFF) shl 8)
    }

    /** A 16-bit value big-endian whatever the stream's order is. */
    fun readU16Big(): Int {
        alignTo(2)
        val raw = take(2)
        return ((raw[0].toInt() and 0xFF) shl 8) or (raw[1].toInt() and 0xFF)
    }

    /** A 32-bit value little-endian whatever the stream's order is. */
    fun readU32Little(): Long {
        alignTo(4)
        val raw = take(4)
        var v = 0L
        for (i in 0 until 4) v = v or ((raw[i].toLong() and 0xFF) shl (8 * i))
        return v
    }

    /** A 32-bit value big-endian whatever the stream's order is. */
    fun readU32Big(): Long {
        alignTo(4)
        val raw = take(4)
        var v = 0L
        for (i in 0 until 4) v = (v shl 8) or (raw[i].toLong() and 0xFF)
        return v
    }

    /**
     * [readWstring] under the name the shared walk calls it by: a Kotlin
     * `String` is UTF-16 already and there is nothing to convert, the same
     * delegation `runtime/java`'s `Reader.readWstringUtf8` makes.
     */
    fun readWstringUtf8(policy: WstringLengthPolicy, order: WstringUnitOrder): String =
        readWstring(policy, order)

    /**
     * The inverse of [Writer.writeWstringUtf8Octets].
     *
     * `WstringCodeUnits::Utf8`: the declared length is the UTF-8 octet count,
     * and the terminator `policy` asks for is the last of those octets. A
     * count the stream cannot hold, a missing or an interior NUL, or an
     * ill-formed UTF-8 sequence is refused, never a replacement character or
     * a truncated value.
     */
    fun readWstringUtf8Octets(policy: WstringLengthPolicy): String {
        val declared = readU32()
        if (declared > readerRemaining().toLong()) {
            throw WireException("a wstring's declared length does not fit the stream", at)
        }
        val from = at
        take(declared.toInt())
        var count = declared.toInt()
        if (policy.terminated) {
            if (count == 0 || data[from + count - 1].toInt() != 0) {
                throw WireException("a terminated UTF-8 wstring's last octet is not NUL", at)
            }
            for (i in 0 until count - 1) {
                if (data[from + i].toInt() == 0) {
                    throw WireException("a terminated UTF-8 wstring has an internal NUL", at)
                }
            }
            count -= 1
        }
        return strictUtf8Text(data, from, count, from)
    }

    /** A `wstring`, in the form `policy` and `order` name, as a Kotlin `String`. */
    fun readWstring(policy: WstringLengthPolicy, order: WstringUnitOrder): String {
        val declared = readU32()
        var count = declared
        if (!policy.countsCharacters) {
            if (declared % policy.width != 0L) throw WireException("a wstring's octet length is not a multiple of its unit width", at)
            count = declared / policy.width
        }
        if (count * policy.width > readerRemaining().toLong()) {
            throw WireException("a wstring's declared length does not fit the stream", at)
        }
        val units = ArrayList<Int>(count.toInt())
        val little = order == WstringUnitOrder.BYTE_ORDER_MARK || order == WstringUnitOrder.HOST_UNSWAPPED
        repeat(count.toInt()) {
            if (policy.width == 4) {
                val wide = when {
                    order == WstringUnitOrder.BIG_ENDIAN_ALWAYS -> readU32Big()
                    little -> readU32Little()
                    else -> readU32()
                }
                if (wide > 0xFFFFL) throw WireException("a wide wstring unit does not fit a UTF-16 code unit", at)
                units.add(wide.toInt())
            } else {
                units.add(
                    when {
                        order == WstringUnitOrder.BIG_ENDIAN_ALWAYS -> readU16Big()
                        little -> readU16Little()
                        else -> readU16()
                    }
                )
            }
        }
        var items = units
        if (policy.terminated) {
            if (items.isEmpty()) throw WireException("a terminated wstring has no terminator", at)
            val last = items.removeAt(items.size - 1)
            if (last != 0) throw WireException("a terminated wstring's terminator is not NUL", at)
            if (items.any { it == 0 }) throw WireException("a terminated wstring has an internal NUL", at)
        }
        if (order == WstringUnitOrder.BYTE_ORDER_MARK) items = stripByteOrderMark(items)
        val builder = StringBuilder()
        var index = 0
        while (index < items.size) {
            builder.append(items[index].toChar())
            index++
        }
        return builder.toString()
    }

    private fun stripByteOrderMark(units: List<Int>): ArrayList<Int> {
        if (units.isEmpty()) return ArrayList(units)
        if (units[0] == 0xFEFF) return ArrayList(units.subList(1, units.size))
        val rest = if (units[0] == 0xFFFE) units.subList(1, units.size) else units
        val swapped = ArrayList<Int>(rest.size)
        for (unit in rest) swapped.add(((unit shl 8) or (unit ushr 8)) and 0xFFFF)
        return swapped
    }

    /** `fixed<digits,scale>`, as the decimal string the writer took. */
    fun readFixed(digits: Int, scale: Int, count: FixedDigitCount): String {
        if (digits !in 1..31 || scale > digits) throw WireException("fixed<$digits,$scale> is not a wire-representable shape", at)
        val nibbles = ArrayList<Int>()
        var sign: Int
        if (count == FixedDigitCount.VALUE_PRECISION || count == FixedDigitCount.VALUE_STRING_DIGITS) {
            while (true) {
                if (nibbles.size >= 32) throw WireException("no sign nibble within the widest fixed IDL admits", at)
                val b = readU8()
                nibbles.add(b shr 4); nibbles.add(b and 0x0F)
                if ((b and 0x0F) >= 0x0A) break
            }
            sign = nibbles.removeAt(nibbles.size - 1)
        } else {
            val raw = take((digits + 2) / 2)
            for (b in raw) { nibbles.add((b.toInt() shr 4) and 0xF); nibbles.add(b.toInt() and 0xF) }
            sign = nibbles.removeAt(nibbles.size - 1)
            if ((digits + 1) % 2 == 1) nibbles.removeAt(0)
        }
        for (n in nibbles) if (n > 9) throw WireException("a fixed's nibble is not a decimal digit", at)
        val text = StringBuilder()
        for (n in nibbles) text.append(('0' + n))
        while (text.length < scale + 1) text.insert(0, '0')
        val split = text.length - scale
        var whole = text.substring(0, split)
        while (whole.length > 1 && whole[0] == '0') whole = whole.substring(1)
        val fraction = text.substring(split)
        val zero = whole == "0" && fraction.all { it == '0' }
        val result = StringBuilder()
        if (sign == 0x0D && !zero) result.append('-')
        result.append(whole)
        if (scale > 0) { result.append('.'); result.append(fraction) }
        return result.toString()
    }

    // -- the trims a bound reaches for --------------------------------------------

    /** Cut `text` to at most `octets` UTF-8 bytes, never inside a character. Table 11's TRIM. */
    fun trimUtf8(text: String, octets: Int): String {
        val raw = text.toByteArray(Charsets.UTF_8)
        if (raw.size <= octets) return text
        var end = octets
        // Never split a multi-byte UTF-8 sequence: back off while the next
        // byte is a continuation byte (10xxxxxx).
        while (end > 0 && (raw[end].toInt() and 0xC0) == 0x80) end--
        return String(raw, 0, end, Charsets.UTF_8)
    }

    /** Cut `text` to at most `units` UTF-16 code units. */
    fun trimUtf16(text: String, units: Int): String {
        if (text.length <= units) return text
        var end = units
        // Never split a surrogate pair.
        if (end > 0 && Character.isHighSurrogate(text[end - 1]) && end < text.length && Character.isLowSurrogate(text[end])) end--
        return text.substring(0, end)
    }

    // -- the bit-bounded kinds ------------------------------------------------------

    /** An enumerator's holder, as the signed integer §7.4.3.5.3 makes it — the holder's width, not the bound's. */
    fun readEnum(bitBound: Int): Long = when {
        bitBound !in 1..32 -> throw WireException("an enumeration's bit bound is 1..32, not $bitBound", at)
        bitBound <= 8 -> readI8().toLong()
        bitBound <= 16 -> readI16().toLong()
        else -> readI32().toLong()
    }

    /** A bitmask's holder. Table 33 of §7.4.1.1.3.2. */
    fun readBitmask(bitBound: Int): Long = when {
        bitBound !in 1..64 -> throw WireException("a bitmask's bit bound is 1..64, not $bitBound", at)
        bitBound <= 8 -> readU8().toLong()
        bitBound <= 16 -> readU16().toLong()
        bitBound <= 32 -> readU32()
        else -> readU64()
    }

    /** A bitset's anchor integer, of the width the planner sized. */
    fun readBitset(anchor: Int): Long = when (anchor) {
        1 -> readU8().toLong()
        2 -> readU16().toLong()
        4 -> readU32()
        8 -> readU64()
        else -> throw WireException("a bitset anchor is 1, 2, 4 or 8 octets, not $anchor", at)
    }

    // -- framing ----------------------------------------------------------------------

    /** A DHEADER: the length the members that follow occupy. */
    fun readDheader(): Long {
        val length = readU32()
        if (length > readerRemaining().toLong()) throw WireException("a DHEADER declares more than the stream holds", at)
        return length
    }

    /** An EMHEADER, unpacked. Codes 5 to 7 report the NEXTINT they peeked, but the cursor is put back before it. */
    fun readEmheader(): EmHeaderRead {
        val word = readU32()
        val memberId = (word and E_MEMBER_ID_MASK).toInt()
        val lengthCode = ((word ushr 28) and 0x7L).toInt()
        val mustUnderstand = (word ushr 31) != 0L
        var nextInt = 0L
        var hasNextInt = false
        if (lengthCode == 4) {
            nextInt = readU32()
            hasNextInt = true
        } else if (lengthCode >= 5) {
            val where = at
            nextInt = readU32()
            hasNextInt = true
            at = where
        }
        val extent: Long = when {
            lengthCode <= 3 -> 1L shl lengthCode
            !hasNextInt -> 0L
            lengthCode == 4 -> nextInt
            else -> 4L + nextInt * (if (lengthCode == 6) 4L else if (lengthCode == 7) 8L else 1L)
        }
        return EmHeaderRead(memberId, lengthCode, mustUnderstand, nextInt, hasNextInt, extent)
    }

    private fun readPidHeader(role: ParameterRole): PidRead {
        alignTo(4)
        val pid = readU16()
        val length = readU16().toLong()
        if ((pid and PID_ID_MASK) == (PID_EXTENDED and PID_ID_MASK)) {
            val memberHeader = readU32()
            val wideLength = readU32()
            return PidRead(
                pid = (memberHeader and E_MEMBER_ID_MASK).toInt(),
                length = wideLength,
                isEnd = false,
                mustUnderstand = (memberHeader and E_FLAG_MUST_UNDERSTAND) != 0L,
                implExtension = (memberHeader and E_FLAG_IMPL_EXTENSION) != 0L,
                reserved = false,
                ignore = false,
                role = role,
                savedOrigin = 0,
                valueStart = 0,
            )
        }
        val identifier = pid and PID_ID_MASK
        val end = identifier == (PID_LIST_END and PID_ID_MASK)
        val flagged = (pid and PID_IMPL_EXTENSION) != 0 && (pidFlagsAt(framing.pidFlags, PidPosition.MEMBER) and PID_IMPL_EXTENSION) == 0
        return PidRead(
            pid = identifier,
            length = length,
            isEnd = end,
            mustUnderstand = (pid and PID_MUST_UNDERSTAND) != 0,
            implExtension = flagged,
            reserved = !end && identifier > framing.highestShortId,
            ignore = identifier == PID_IGNORE,
            role = role,
            savedOrigin = 0,
            valueStart = 0,
        )
    }

    /** A PL_CDR1 parameter header in the member role. */
    fun readPid(): PidRead = readPidAs(ParameterRole.MEMBER)

    /** A parameter header, opening the value block §7.4.1.2 gives it its own alignment inside. */
    fun readPidAs(role: ParameterRole): PidRead {
        val header = readPidHeader(role)
        val savedOrigin = origin
        val valueStart = at
        origin = at
        return PidRead(
            header.pid, header.length, header.isEnd, header.mustUnderstand, header.implExtension,
            header.reserved, header.ignore, header.role, savedOrigin, valueStart,
        )
    }

    /** End a parameter's value and step to the next header. */
    fun finishPidRead(header: PidRead) {
        origin = header.savedOrigin
        if (status != Status.OK || header.isEnd) return
        val used = (at - header.valueStart).toLong()
        val declared = header.length
        val padded = (used + 3) and 3L.inv()
        if (declared != used && declared != padded) readerRefuse(Status.OUT_OF_RANGE)
        alignTo(4)
    }
}

// -- fixed BCD -------------------------------------------------------------------

private fun fixedNibbles(text: String, digits: Int, scale: Int, count: FixedDigitCount, at: Int): IntArray {
    if (digits !in 1..31 || scale > digits) throw WireException("fixed<$digits,$scale> is not a wire-representable shape", at)
    var negative = false
    var magnitude = text
    if (magnitude.isNotEmpty() && (magnitude[0] == '-' || magnitude[0] == '+')) {
        negative = magnitude[0] == '-'
        magnitude = magnitude.substring(1)
    }
    val dot = magnitude.indexOf('.')
    var whole = if (dot >= 0) magnitude.substring(0, dot) else magnitude
    val fraction = if (dot >= 0) magnitude.substring(dot + 1) else ""
    if (whole.isEmpty() && fraction.isEmpty()) throw WireException("`$text` has no digits", at)
    for (c in whole) if (c !in '0'..'9') throw WireException("`$text` is not a decimal", at)
    for (c in fraction) if (c !in '0'..'9') throw WireException("`$text` is not a decimal", at)
    val asWritten = whole.length + scale
    val integralWidth = digits - scale
    whole = whole.trimStart('0')
    if (whole.isEmpty()) whole = ""
    if (whole.length > integralWidth || fraction.length > scale) {
        throw WireException("`$text` does not fit fixed<$digits,$scale>", at)
    }
    val every = ArrayList<Int>(digits)
    repeat(integralWidth - whole.length) { every.add(0) }
    for (c in whole) every.add(c - '0')
    for (c in fraction) every.add(c - '0')
    repeat(scale - fraction.length) { every.add(0) }
    var significant = 0
    for ((index, nibble) in every.withIndex()) {
        if (nibble != 0) { significant = every.size - index; break }
    }
    var wire = every.size
    if (count == FixedDigitCount.VALUE_PRECISION) {
        wire = if (significant < 1) 1 else significant
    } else if (count == FixedDigitCount.VALUE_STRING_DIGITS) {
        // ACE/TAO 2.5.24: the integer digits as written plus S, made odd by one leading
        // zero. Past 31 no reader can be told the field's width.
        wire = asWritten + (if (asWritten % 2 == 0) 1 else 0)
        if (wire > 31) throw WireException("`$text` is $wire digits on the wire, more than 31", at)
    }
    val nibbles = ArrayList<Int>(wire + 2)
    if ((wire + 1) % 2 == 1) nibbles.add(0)
    // A count wider than P is filled with leading zeros.
    val kept = minOf(wire, every.size)
    repeat(wire - kept) { nibbles.add(0) }
    nibbles.addAll(every.subList(every.size - kept, every.size))
    nibbles.add(if (negative && significant != 0) 0x0D else 0x0C)
    return nibbles.toIntArray()
}

// -- long double -------------------------------------------------------------------

/**
 * An IEEE 754 binary128 value, held as the sixteen bytes of its
 * big-endian representation. The JVM has no such primitive, so an IDL
 * `long double` is carried here rather than in a `Float` or `Double`,
 * which would round it at 24 or 53 bits of significand where
 * DDS-XTypes 1.3 §7.4.1.4.4.2.2 calls for 113.
 *
 * [toString] writes every digit of the value, and [parse] rounds the
 * exact rational a decimal text denotes to nearest, ties to even — the
 * two algorithms of `runtime/java`'s `Float128` (`toExactDecimal`,
 * `fromBigDecimal`), so the Kotlin column writes and reads the octets
 * and the text every other column does. They replaced a [BigDecimal]
 * walk whose `/` rounded each halving to the dividend's scale (`-2.75`
 * became `-2.76`) and which wrote a subnormal as zero.
 */
class Float128 private constructor(private val high: Long, private val low: Long) {
    /** The sixteen bytes, most significant first. */
    fun bigEndianBytes(): ByteArray {
        val out = ByteArray(16)
        for (i in 0 until 8) out[i] = (high ushr (8 * (7 - i))).toByte()
        for (i in 0 until 8) out[8 + i] = (low ushr (8 * (7 - i))).toByte()
        return out
    }

    private fun signBit(): Boolean = (high ushr 63) and 1L != 0L
    private fun exponentBits(): Int = ((high ushr 48) and 0x7FFF).toInt()
    private fun significandHigh(): Long = high and 0xFFFF_FFFF_FFFFL

    /** Whether this value is a NaN of either kind. */
    fun isNaN(): Boolean = exponentBits() == 0x7FFF && (significandHigh() != 0L || low != 0L)

    /** Whether this value is positive or negative infinity. */
    fun isInfinite(): Boolean = exponentBits() == 0x7FFF && significandHigh() == 0L && low == 0L

    /**
     * Every digit the value has, in the corpus's canonical spelling: the
     * text `runtime/java`'s `Float128.toExactDecimal`, `crates/float128`
     * and the C and Python runtimes write for the same pattern.
     */
    override fun toString(): String {
        if (isNaN()) return if (signBit()) "-nan" else "nan"
        if (isInfinite()) return if (signBit()) "-inf" else "inf"
        val field = exponentBits()
        val fraction = BigInteger.valueOf(significandHigh()).shiftLeft(64)
            .or(BigInteger.valueOf(low).and(UNSIGNED_64))
        val significand = if (field == 0) fraction else fraction.or(IMPLIED_ONE)
        if (significand.signum() == 0) return if (signBit()) "-0.0" else "0.0"
        val exponent = (if (field == 0) 1 else field) - BIAS - FRACTION_BITS
        val all: String
        val point: Int
        if (exponent >= 0) {
            all = significand.shiftLeft(exponent).toString()
            point = all.length
        } else {
            // m * 2^-k == (m * 5^k) * 10^-k, which is exact.
            all = significand.multiply(FIVE.pow(-exponent)).toString()
            point = all.length + exponent
        }
        val digits = all.trimEnd('0').ifEmpty { "0" }
        return render(signBit(), digits, point)
    }

    companion object {
        private const val BIAS = 16383
        private const val FRACTION_BITS = 112
        private val UNSIGNED_64 = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)
        private val IMPLIED_ONE = BigInteger.ONE.shiftLeft(FRACTION_BITS)
        private val FRACTION_MASK = IMPLIED_ONE.subtract(BigInteger.ONE)
        private val FIVE = BigInteger.valueOf(5)

        /** Zero. */
        fun zero(): Float128 = Float128(0L, 0L)

        /** The sixteen bytes, most significant first, as one value. */
        fun ofBigEndianBytes(bytes: ByteArray): Float128 {
            require(bytes.size == 16) { "a binary128 is sixteen bytes" }
            var high = 0L
            for (i in 0 until 8) high = (high shl 8) or (bytes[i].toLong() and 0xFF)
            var low = 0L
            for (i in 8 until 16) low = (low shl 8) or (bytes[i].toLong() and 0xFF)
            return Float128(high, low)
        }

        /**
         * The value a decimal text denotes, rounded to nearest with ties to
         * even against the exact rational — never through a `Double`, and
         * never through a [BigDecimal] division that rounds on its own.
         */
        fun parse(text: String): Float128 {
            val trimmed = text.trim()
            when (trimmed.lowercase()) {
                "inf", "+inf", "infinity", "+infinity" -> return infinity(false)
                "-inf", "-infinity" -> return infinity(true)
                "nan", "+nan" -> return nan(false)
                "-nan" -> return nan(true)
            }
            val parsed = BigDecimal(trimmed)
            // BigDecimal has no negative zero; the sign survives only in the text.
            if (parsed.signum() == 0) return if (trimmed.startsWith("-")) Float128(1L shl 63, 0L) else zero()
            val magnitude = parsed.abs()
            var numerator = magnitude.unscaledValue()
            var denominator = BigInteger.ONE
            val scale = magnitude.scale()
            if (scale > 0) {
                denominator = BigInteger.TEN.pow(scale)
            } else if (scale < 0) {
                numerator = numerator.multiply(BigInteger.TEN.pow(-scale))
            }
            return fromRatio(parsed.signum() < 0, numerator, denominator)
        }

        /**
         * `numerator / denominator` to binary128: the quotient with enough
         * extra bits that the one rounding at the end is the right one, and
         * everything discarded folded into a sticky bit.
         */
        private fun fromRatio(negative: Boolean, numerator: BigInteger, denominator: BigInteger): Float128 {
            var shift = 130 - (numerator.bitLength() - denominator.bitLength())
            if (shift < 0) shift = 0
            val divided = numerator.shiftLeft(shift).divideAndRemainder(denominator)
            var quotient = divided[0]
            if (divided[1].signum() != 0) quotient = quotient.setBit(0)
            return compose(negative, quotient, -shift)
        }

        /** A sign, an integral significand and a binary exponent, rounded to nearest, ties to even. */
        private fun compose(negative: Boolean, significandIn: BigInteger, exponentIn: Int): Float128 {
            val sign = if (negative) 1L shl 63 else 0L
            var significand = significandIn
            var exponent = exponentIn
            if (significand.signum() == 0) return Float128(sign, 0L)
            if (exponent > 0x7FFF) return infinity(negative)
            if (exponent < -(FRACTION_BITS + 1 + BIAS + 200)) return Float128(sign, 0L)
            val leading = significand.bitLength() - 1
            if (leading > FRACTION_BITS) {
                val drop = leading - FRACTION_BITS
                significand = shiftRound(significand, drop)
                exponent += drop
                if (significand.bitLength() - 1 > FRACTION_BITS) {
                    significand = significand.shiftRight(1)
                    exponent += 1
                }
            } else {
                val up = FRACTION_BITS - leading
                significand = significand.shiftLeft(up)
                exponent -= up
            }
            val biased = exponent + BIAS + FRACTION_BITS
            if (biased >= 0x7FFF) return infinity(negative)
            if (biased <= 0) {
                // Subnormal. A value that rounds up to the smallest normal
                // carries out of the fraction into the exponent field on its own.
                return ofWord(sign, shiftRound(significand, 1 - biased))
            }
            val word = BigInteger.valueOf(biased.toLong()).shiftLeft(FRACTION_BITS).or(significand.and(FRACTION_MASK))
            return ofWord(sign, word)
        }

        /** `value >> shift`, rounded to nearest, ties to even. */
        private fun shiftRound(value: BigInteger, shift: Int): BigInteger {
            if (shift <= 0) return value
            var kept = value.shiftRight(shift)
            val dropped = value.subtract(kept.shiftLeft(shift))
            if (dropped.signum() == 0) return kept
            val order = dropped.compareTo(BigInteger.ONE.shiftLeft(shift - 1))
            if (order > 0 || (order == 0 && kept.testBit(0))) kept = kept.add(BigInteger.ONE)
            return kept
        }

        /** The 127 bits below the sign as one integer, with the sign put on top. */
        private fun ofWord(sign: Long, word: BigInteger): Float128 =
            Float128(word.shiftRight(64).toLong() or sign, word.and(UNSIGNED_64).toLong())

        private fun render(negative: Boolean, digits: String, point: Int): String {
            val out = StringBuilder()
            if (negative) out.append('-')
            if (point > 0 && point <= 34) {
                if (digits.length <= point) {
                    out.append(digits)
                    for (i in digits.length until point) out.append('0')
                    out.append(".0")
                } else {
                    out.append(digits, 0, point).append('.').append(digits.substring(point))
                }
                return out.toString()
            }
            if (point <= 0 && point > -6) {
                out.append("0.")
                for (i in 0 until -point) out.append('0')
                return out.append(digits).toString()
            }
            out.append(digits[0]).append('.')
            out.append(if (digits.length == 1) "0" else digits.substring(1))
            val exponent = point - 1
            out.append('e').append(if (exponent >= 0) '+' else '-').append(kotlin.math.abs(exponent))
            return out.toString()
        }

        private fun infinity(negative: Boolean): Float128 {
            val high = (0x7FFFL shl 48) or (if (negative) 1L shl 63 else 0L)
            return Float128(high, 0L)
        }

        private fun nan(negative: Boolean): Float128 {
            val high = (0x7FFFL shl 48) or (1L shl 47) or (if (negative) 1L shl 63 else 0L)
            return Float128(high, 0L)
        }
    }
}
