// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
package org.zerodds.wire;

import java.nio.ByteBuffer;
import java.nio.CharBuffer;
import java.nio.charset.CharacterCodingException;
import java.nio.charset.CharsetEncoder;
import java.nio.charset.CodingErrorAction;
import java.util.Arrays;

/**
 * Putting bytes on the wire.
 *
 * <p>The Java counterpart of {@code crates/wire/src/writer.rs},
 * {@code runtime/c/src/writer.c} and
 * {@code runtime/python/zerodds_wire/writer.py} — a fourth implementation of
 * the same clauses, not a binding to any of them. Where two of the four
 * disagree, one of them has misread DDS-XTypes 1.3, and that is the point of
 * having four.
 *
 * <p>Alignment is relative to the start of the stream, not to the start of the
 * value: XCDR aligns each primitive to its own size counted from the
 * encapsulation, which is why a {@code long} after a {@code char} sits at 4 and
 * not at 1. XTypes 1.3 §7.4.1.1.
 *
 * <h2>What Java makes easy to get wrong here</h2>
 *
 * <ul>
 *   <li><b>There are no unsigned integers.</b> Every method below says in its
 *       own documentation whether its argument is a <i>number</i> that is
 *       range-checked ({@link #writeU8(int)}, {@link #writeU16(int)},
 *       {@link #writeU32(long)}) or a <i>bit pattern</i> whose every value is
 *       admissible ({@link #writeU64(long)}). A silent {@code int} overflow
 *       here puts a different number on the wire and reports success.</li>
 *   <li><b>{@code String.length()} is neither bytes nor characters.</b> It is
 *       UTF-16 code units, so it is the length of neither a {@code string} nor
 *       a {@code wstring} on the wire. {@link #writeString(String)} takes its
 *       length from the encoded UTF-8; {@link #writeWstring} takes its length
 *       from the code units and from what the policy counts.</li>
 *   <li><b>{@code String.getBytes(UTF_8)} does not fail.</b> It replaces an
 *       unpaired surrogate with {@code '?'} and reports success. This class
 *       encodes through a reporting {@code CharsetEncoder} and raises
 *       {@link WireException.NotUtf8} instead.</li>
 *   <li><b>A {@code float} field must be rounded once.</b>
 *       {@link #writeF32(double)} takes a {@code double} and performs the one
 *       narrowing itself, which is where a magnitude no {@code float} holds
 *       becomes an infinity — the wire-correct answer, and what every peer
 *       reads back.</li>
 * </ul>
 *
 * <p>This class makes no decision about a type. Which members are written, in
 * what order, with which header and which length code is the layout plan's
 * answer and arrives here as arguments.
 */
public final class Writer {

    /** The parameter id that introduces the extended form. §7.4.1.2.2. */
    public static final int PID_EXTENDED = 0x3F01;

    /** The parameter id that ends a parameter list. §7.4.1.2.4. */
    public static final int PID_LIST_END = 0x3F02;

    /**
     * The highest member id a short parameter header can carry. §7.4.1.2.2
     * gives the range as 0 to {@code 0x3F00} <i>inclusive</i>, and
     * {@link #PID_EXTENDED} is one above it. RTI Connext and Fast DDS both
     * write the short form for {@code 0x3F00} (case {@code pid_3f00},
     * 2026-08-10).
     */
    public static final int PID_SHORT_MAX_ID = 0x3F00;

    /**
     * {@code FLAG_IMPL_EXTENSION}, bit 15 of a short parameter id.
     * §7.4.1.2.2: a parameter that sets it is the writing implementation's
     * own, and user-defined members "will never set the FLAG_IMPL_EXTENSION
     * bit".
     */
    public static final int PID_IMPL_EXTENSION = 0x8000;

    /**
     * {@code PID_IGNORE}. §7.4.1.2.2: "All consumers of this Data
     * Representation shall ignore parameters with this ID" — including a
     * consumer that would otherwise discard the sample over the
     * must-understand bit.
     */
    public static final int PID_IGNORE = 0x3F03;

    /**
     * The longest value a short parameter header can state the length of.
     * §7.4.1.2.2 gives {@code ssize} sixteen bits; {@link #finishPid} rewrites
     * the header rather than truncating a longer length into two bytes.
     */
    public static final int PID_SHORT_MAX_LEN = 0xFFFF;

    /** The must-understand bit of a parameter id. DDSI-RTPS 2.5 §9.4.2.11.2. */
    public static final int PID_MUST_UNDERSTAND = 0x4000;

    /**
     * {@code FLAG_2} of the extended {@code eMemberHeader}, which is the
     * <b>member's</b> must-understand. §7.4.1.2.1.
     */
    public static final int E_FLAG_MUST_UNDERSTAND = 0x40000000;

    /**
     * {@code FLAG_1} of the extended {@code eMemberHeader}, which is
     * {@code FLAG_IMPL_EXTENSION} — the same property as
     * {@link #PID_IMPL_EXTENSION} in the short form. §7.4.1.2.1.
     */
    public static final int E_FLAG_IMPL_EXTENSION = 0x80000000;

    /** The mask of the extended header's 28-bit member id. §7.4.1.2.1. */
    public static final int E_MEMBER_ID_MASK = 0x0FFFFFFF;

    /** A KeyHash is sixteen bytes. DDSI-RTPS 2.5 §9.6.3.8. */
    public static final int KEY_HASH_LEN = 16;

    /** The UTF-16 byte-order mark. */
    static final int BYTE_ORDER_MARK = 0xFEFF;

    /**
     * The byte-swapped byte-order mark. {@code U+FFFE} is not a character: a
     * mark that reads as one says the units behind it are the other way round
     * from the order they were read in.
     */
    static final int SWAPPED_BYTE_ORDER_MARK = 0xFFFE;

    private byte[] out;
    private int len;
    private final Endianness endianness;
    /**
     * Where alignment is counted from. Zero for a sample; a PL_CDR1 parameter
     * value moves it, because a parameter is its own encapsulated block.
     * XTypes 1.3 §7.4.1.2.
     */
    private int origin;
    private final int maxAlign;
    private final PlCdr1Framing plCdr1;
    private final boolean shortLengthCodes;

    /**
     * A writer in {@code endianness} whose alignment is capped at
     * {@code maxAlign}.
     *
     * <p>Four for XCDR2, eight for XCDR1. XTypes 1.3 §7.4.1: {@code ALIGN(N)}
     * aligns to {@code MIN(N, XCDR.maxalign)}. A writer that did not know this
     * padded an eight-byte primitive to eight under XCDR2 and put four bytes
     * on the wire that no peer expects. {@link Encoding#maxAlignment()} is
     * where the number comes from.
     *
     * @param endianness which end of a multi-byte value goes out first
     * @param maxAlign the encoding's MAXALIGN
     * @param framing how a PL_CDR1 parameter list is framed; {@code null} for
     *     {@link PlCdr1Framing#defaults()}
     * @param shortLengthCodes whether an LC4 header whose member turns out to
     *     be 1, 2, 4 or 8 bytes is rewritten with a short length code. Set it
     *     from {@code LengthCodePolicy::short_code_where_it_fits}; the layout
     *     plan already answers for every member whose type fixes the size, and
     *     this is the other half.
     */
    public Writer(
            Endianness endianness,
            int maxAlign,
            PlCdr1Framing framing,
            boolean shortLengthCodes) {
        if (endianness == null) {
            throw new IllegalArgumentException("a byte order, not null");
        }
        this.out = new byte[64];
        this.len = 0;
        this.endianness = endianness;
        this.origin = 0;
        this.maxAlign = Math.max(1, maxAlign);
        this.plCdr1 = framing == null ? PlCdr1Framing.defaults() : framing;
        this.shortLengthCodes = shortLengthCodes;
    }

    /**
     * A writer in {@code endianness} with XCDR1's MAXALIGN of eight, the
     * clause's PL_CDR1 framing, and no short-code rewriting.
     *
     * <p>Eight and not four, for the same reason the Python column's default
     * is eight: it is the value that pads nothing away, so a caller that has
     * not thought about the cap gets a stream that is wrong under XCDR2 in a
     * way a reader notices, rather than one that silently drops padding an
     * XCDR1 peer expects.
     */
    public Writer(Endianness endianness) {
        this(endianness, 8, null, false);
    }

    /** A big-endian writer, which is what a KeyHash holder is. */
    public static Writer bigEndian() {
        return new Writer(Endianness.BIG);
    }

    /** A little-endian writer. */
    public static Writer littleEndian() {
        return new Writer(Endianness.LITTLE);
    }

    // -- state ------------------------------------------------------------

    /** The byte order in force. */
    public Endianness endianness() {
        return endianness;
    }

    /** The alignment cap in force. */
    public int maxAlign() {
        return maxAlign;
    }

    /** How a PL_CDR1 parameter list is framed here. */
    public PlCdr1Framing framing() {
        return plCdr1;
    }

    /** Whether an LC4 header is shortened when its member fits a code. */
    public boolean shortLengthCodes() {
        return shortLengthCodes;
    }

    /** How many bytes have been written. */
    public int position() {
        return len;
    }

    /** The bytes, as a copy the caller owns. */
    public byte[] finish() {
        return Arrays.copyOf(out, len);
    }

    // -- alignment --------------------------------------------------------

    /**
     * Pad to {@code align}, counted from the stream's origin and capped at
     * MAXALIGN.
     *
     * <p>Both halves matter. The cap is {@link #maxAlign}, so an eight-byte
     * primitive aligns to four under XCDR2. The origin is where a PL_CDR1
     * parameter's block began and zero everywhere else — never the value's own
     * start, and never zero when a parameter is open.
     */
    public void alignTo(int align) {
        int effective = Math.min(align, maxAlign);
        if (effective <= 1) {
            return;
        }
        int offset = len - origin;
        int padding = (effective - Math.floorMod(offset, effective)) % effective;
        if (padding != 0) {
            need(padding);
            Arrays.fill(out, len, len + padding, (byte) 0);
            len += padding;
        }
    }

    /**
     * Move the alignment origin to the current position.
     *
     * <p>{@code ElementPadding::Packed} is HDDS's measured departure from
     * XTypes 1.3 §7.4.1, which counts every element's alignment from the
     * encapsulation: each collection element and each aggregate value counts
     * its own alignment from its own first octet instead. {@code
     * array-1d-struct-final} is {@code 01000000 0200 03000000 0400 77}, the
     * second {@code Cell}'s {@code long} at offset six where the clause puts
     * it at eight, and {@code typedef-of-struct} is the same inside two
     * {@code @final} members.
     *
     * @return the origin displaced, for {@link #finishPackedElement(int)}
     */
    public int beginPackedElement() {
        int saved = origin;
        origin = len;
        return saved;
    }

    /** Restore the origin {@link #beginPackedElement()} displaced. */
    public void finishPackedElement(int saved) {
        origin = saved;
    }

    // -- primitives -------------------------------------------------------

    /** {@code boolean}, one byte, 0 or 1. */
    public void writeBool(boolean value) {
        append((byte) (value ? 1 : 0));
    }

    /**
     * {@code octet} and {@code uint8}.
     *
     * <p>A <i>number</i> in {@code [0, 255]}, not a pattern: the argument is
     * an {@code int} because Java's {@code byte} is signed, and {@code 200}
     * arriving as {@code -56} would be a caller who has already lost the
     * value. A value outside the range is refused, never masked.
     */
    public void writeU8(int value) {
        check(value, 0, 0xFF, "an octet");
        append((byte) value);
    }

    /**
     * {@code octet}, which CORBA 3.3 part 3 §1.5.2 maps to a Java {@code byte}: written as the
     * number its eight bits are, 0..255, so the byte {@code -56} writes 200.
     */
    public void writeU8(byte value) {
        writeU8(value & 0xFF);
    }

    /** {@code int8}. A number in {@code [-128, 127]}. */
    public void writeI8(int value) {
        check(value, -0x80, 0x7F, "an int8");
        append((byte) (value & 0xFF));
    }

    /**
     * {@code char}, one byte, from a code point.
     *
     * <p>A code point above 255 is refused rather than truncated:
     * §7.2.2.2.1.2.2 says a {@code CHAR_8_TYPE} "shall be representable using
     * 8 bits", and a {@code char} field holding {@code 'ä'} in a UTF-8 source
     * file is a mistake worth stopping on — {@code 0xe4} is a different
     * character.
     */
    public void writeChar(int codePoint) {
        if (codePoint < 0 || codePoint > 0xFF) {
            throw new WireException.OutOfRange(
                    len, "a character below U+0100, not U+" + hex(codePoint));
        }
        append((byte) codePoint);
    }

    /**
     * {@code wchar}, at {@code width} octets and in the shape {@code framing}
     * gives it.
     *
     * <p>Two octets under XTypes 1.3 Table 31 and for seven of the nine
     * measured implementations; four under RTI Connext's XCDR1, under TAO, and
     * under {@code rosidl_typesupport_fastrtps_c}. The width is an argument of
     * its own and not a reading of the {@code wstring} policy, because
     * {@code rmw_cyclonedds_cpp} writes two here and four inside a
     * {@code wstring}.
     *
     * <p>The Basic-Multilingual-Plane restriction of §7.2.2.2.1.2.5 is a
     * property of the type and not of the width: the four-byte form
     * zero-extends a code unit, it does not carry a code point.
     *
     * @param codePoint the character, which must be inside the BMP
     * @param width two octets or four
     * @param framing bare, or CORBA §9.3.1.6's length-prefixed form
     * @param order what byte order the octets take under the prefixed form; a
     *     scalar cannot carry a byte-order mark, so anything but
     *     {@link WstringUnitOrder#STREAM} means CORBA's default, big-endian
     */
    public void writeWchar(
            int codePoint, WcharWidth width, WcharFraming framing, WstringUnitOrder order) {
        if (codePoint < 0 || codePoint > 0xFFFF) {
            throw new WireException.OutOfRange(len, "a BMP character, not U+" + hex(codePoint));
        }
        if (framing == WcharFraming.GIOP_LENGTH_PREFIXED) {
            // CORBA 3.3 part 2 §9.3.1.6's GIOP 1.2 form: the octet count
            // first, then the code unit's octets, all aligned to one. The
            // count is the transmission code set's width and not the value's,
            // so it is the same octet for every character. Measured
            // 2026-08-10: omniORB writes `02 00e4` for U+00E4 in either
            // stream order and TAO `04 e4000000`.
            append((byte) width.octets());
            boolean little = order == WstringUnitOrder.STREAM && endianness.isLittle();
            appendRaw(codePoint & 0xFFFFFFFFL, width.octets(), little);
            return;
        }
        if (width == WcharWidth.OCTETS_4) {
            writeU32(codePoint);
        } else {
            writeU16(codePoint);
        }
    }

    /** {@code short}. A number in {@code [-32768, 32767]}. */
    public void writeI16(int value) {
        check(value, -0x8000, 0x7FFF, "a short");
        writeInt(value & 0xFFFFL, 2);
    }

    /**
     * {@code unsigned short}. A <i>number</i> in {@code [0, 65535]}, refused
     * outside it and never masked.
     */
    public void writeU16(int value) {
        check(value, 0, 0xFFFF, "an unsigned short");
        writeInt(value, 2);
    }

    /**
     * {@code long}. An {@code int} is exactly the field, so every value is
     * admissible and there is nothing to check.
     */
    public void writeI32(int value) {
        writeInt(value & 0xFFFFFFFFL, 4);
    }

    /**
     * {@code unsigned long}. A <i>number</i> in {@code [0, 4294967295]}, which
     * is why it takes a {@code long}: an {@code int} argument would make
     * {@code 3735928559} arrive as {@code -559038737} with the loss already
     * done, and this range check is the whole reason for the wider parameter.
     */
    public void writeU32(long value) {
        check(value, 0L, 0xFFFFFFFFL, "an unsigned long");
        writeInt(value, 4);
    }

    /** {@code long long}. A {@code long} is exactly the field. */
    public void writeI64(long value) {
        writeInt(value, 8);
    }

    /**
     * {@code unsigned long long}. The argument is the <i>bit pattern</i>, not
     * a number.
     *
     * <p>Java has no integer wider than {@code long}, so there is no type that
     * holds {@code 0xFFFFFFFFFFFFFFFF} as a number. The range check is stated
     * in terms of the pattern and is therefore vacuous: every one of the 2^64
     * patterns is a valid {@code unsigned long long}, and
     * {@code writeU64(-1L)} writes {@code ffffffffffffffff} — which is
     * 18446744073709551615, not minus one. Use
     * {@link Long#toUnsignedString(long)} when the value has to be read by a
     * human, and {@link java.math.BigInteger} when it has to be computed with.
     */
    public void writeU64(long pattern) {
        writeInt(pattern, 8);
    }

    /**
     * The Cyclone union overlay: the low {@code octets} of {@code pattern},
     * written as one unsigned integer of that width in the stream's byte
     * order.
     *
     * <p>A discriminator that carries a label the {@code union-labels}
     * dialect axis says selects nothing makes the peer fall through to
     * {@code default:} and serialise its one C union storage <i>as that
     * arm</i> — the selected member's octets, reinterpreted. {@code pattern}
     * is the {@code long} whose little-endian bytes are the selected member's
     * little-endian storage octets, zero-extended; the {@code bitPattern*}
     * statics below produce it.
     */
    public void writeUnionOverlay(long pattern, int octets) {
        switch (octets) {
            case 1:
                writeU8((int) (pattern & 0xFFL));
                return;
            case 2:
                writeU16((int) (pattern & 0xFFFFL));
                return;
            case 4:
                writeU32(pattern & 0xFFFFFFFFL);
                return;
            case 8:
                writeU64(pattern);
                return;
            default:
                throw new WireException.OutOfRange(
                        len, "a union arm of 1, 2, 4 or 8 octets, not " + octets);
        }
    }

    /**
     * The low {@code octets} of {@code value}'s two's-complement bits,
     * zero-extended — {@link #writeUnionOverlay(long, int)}'s pattern for an
     * integer, character, enumeration or bitmask arm. The mask is the method:
     * a plain {@code long} cast of a negative value sign-extends, and
     * {@code -2} in four octets has to be {@code 0x00000000FFFFFFFEL}.
     */
    public static long bitPatternInt(long value, int octets) {
        return octets >= 8 ? value : value & ((1L << (octets * 8)) - 1);
    }

    /** One octet, 0 or 1 — the boolean form of the overlay pattern. */
    public static long bitPatternBool(boolean value) {
        return value ? 1L : 0L;
    }

    /** The IEEE 754 binary32 bits, zero-extended. */
    public static long bitPatternF32(float value) {
        return Float.floatToRawIntBits(value) & 0xFFFFFFFFL;
    }

    /** The IEEE 754 binary64 bits. */
    public static long bitPatternF64(double value) {
        return Double.doubleToRawLongBits(value);
    }

    /**
     * {@code float}, rounded to single precision once, here.
     *
     * <p>The argument is a {@code double} so that the narrowing is this
     * method's and happens exactly once. A magnitude no {@code float} holds
     * becomes {@code ±Infinity}, which is the wire-correct answer: it is what
     * a C {@code (float)} cast produces and what every peer will read back.
     */
    public void writeF32(double value) {
        writeInt(Float.floatToRawIntBits((float) value) & 0xFFFFFFFFL, 4);
    }

    /** {@code double}. */
    public void writeF64(double value) {
        writeInt(Double.doubleToRawLongBits(value), 8);
    }

    /**
     * {@code long double}: sixteen bytes of IEEE 754 binary128.
     *
     * <p>CORBA 3.3 part 2 §9.3.1.3 with Figure 9.2 gives the layout and shows
     * the two byte orders as the whole sixteen bytes reversed, not two
     * eight-byte halves swapped in place. XTypes 1.3 Table 31 gives the
     * alignment: eight, capped to four by MAXALIGN under XCDR2, which
     * {@link #alignTo} already does.
     *
     * <p>The value is a {@link Float128} and never a {@code double} followed
     * by eight zero bytes: that was the right length and the wrong value and
     * it reported success.
     */
    public void writeF128(Float128 value) {
        alignTo(8);
        byte[] bytes = value.toBytes(!endianness.isLittle());
        need(16);
        System.arraycopy(bytes, 0, out, len, 16);
        len += 16;
    }

    /**
     * A 16-bit value little-endian whatever the stream's order is.
     *
     * <p>Two callers: the code units of the {@code host-unswapped}
     * {@code wstring} form, and the {@code byte-order-mark} unit order.
     * Aligned to two like any other 16-bit value. A number in
     * {@code [0, 65535]}.
     */
    public void writeU16Little(int value) {
        check(value, 0, 0xFFFF, "an unsigned 16-bit value");
        alignTo(2);
        appendRaw(value, 2, true);
    }

    /**
     * A 16-bit value big-endian whatever the stream's order is.
     *
     * <p>The other side of the same axis: what omniORB and JacORB decode a
     * {@code wstring}'s code units as, whatever the stream says.
     */
    public void writeU16Big(int value) {
        check(value, 0, 0xFFFF, "an unsigned 16-bit value");
        alignTo(2);
        appendRaw(value, 2, false);
    }

    /**
     * A 32-bit value little-endian whatever the stream's order is.
     *
     * <p>A four-octet transmission code set crossed with a fixed unit order.
     * No dialect selects the combination; it exists so that two orthogonal
     * settings have an answer for every pairing rather than a hole one of them
     * falls into. A number in {@code [0, 4294967295]}.
     */
    public void writeU32Little(long value) {
        check(value, 0L, 0xFFFFFFFFL, "an unsigned 32-bit value");
        alignTo(4);
        appendRaw(value, 4, true);
    }

    /** A 32-bit value big-endian whatever the stream's order is. */
    public void writeU32Big(long value) {
        check(value, 0L, 0xFFFFFFFFL, "an unsigned 32-bit value");
        alignTo(4);
        appendRaw(value, 4, false);
    }

    // -- strings ----------------------------------------------------------

    /**
     * A {@code string}: a four-byte length including the terminator, the
     * bytes, then a NUL. XTypes 1.3 §7.4.1.1.
     *
     * <p>The length counts the terminator, which is the one place XCDR differs
     * from every other length-prefixed encoding. It counts <b>bytes</b> of
     * UTF-8: {@code value.length()} is UTF-16 code units and would be right
     * only for an ASCII sample, and every corpus string is deliberately not
     * one.
     */
    public void writeString(String value) {
        byte[] encoded = utf8(value);
        writeU32(encoded.length + 1L);
        appendAll(encoded);
        append((byte) 0);
    }

    /**
     * A bounded {@code string}.
     *
     * <p>The same bytes as an unbounded one: the bound is a promise about the
     * source, not a difference on the wire — so this is the only place it can
     * be enforced. An over-long value is refused rather than cut at a
     * character boundary: XTypes 1.3 §7.2.2.7's TRIM is a <i>read</i>-side
     * behaviour that belongs to the receiver's own bound, and a writer that
     * cut the value would put a sample on the wire nobody wrote.
     *
     * @param bound the bound in <b>octets</b>, which is what a
     *     {@code string<N>} bound counts for every measured implementation:
     *     {@code string<5>} is {@code char[6]} in the C mapping of all of them
     * @throws WireException.OutOfRange if {@code value} is longer than
     *     {@code bound}
     */
    public void writeStringBounded(String value, int bound) {
        byte[] encoded = utf8(value);
        if (encoded.length > bound) {
            throw new WireException.OutOfRange(
                    len,
                    "at most " + bound + " octets in a bounded string, and this one has "
                            + encoded.length);
        }
        writeU32(encoded.length + 1L);
        appendAll(encoded);
        append((byte) 0);
    }

    /**
     * A bounded {@code wstring}, as {@link #writeStringBounded} is to
     * {@link #writeString(String)}.
     *
     * @param bound the bound in UTF-16 code units, not characters and not
     *     octets, whatever {@code policy} puts on the wire
     * @throws WireException.OutOfRange if {@code value} holds more than
     *     {@code bound} UTF-16 code units
     */
    public void writeWstringBounded(
            String value, int bound, WstringLengthPolicy policy, WstringUnitOrder order) {
        int units = utf16Units(value).length;
        if (units > bound) {
            throw new WireException.OutOfRange(
                    len,
                    "at most " + bound + " UTF-16 code units in a bounded wstring, and this "
                            + "one has " + units);
        }
        writeWstringUtf8(value, policy, order);
    }

    /**
     * The count check every bounded {@code sequence} makes before a byte of
     * it is written. Table 17 makes {@code O2.length <= T1.length} the
     * condition for constructing a {@code sequence<T, N>}, and the wire
     * carries no bound to compare against: a sender whose count already
     * exceeds its own declaration has written a value of a type nobody
     * declared.
     *
     * @throws WireException.OutOfRange if {@code actual} is greater than
     *     {@code bound}
     */
    public void checkSequenceBound(int actual, int bound) {
        if (actual > bound) {
            throw new WireException.OutOfRange(
                    len,
                    "at most " + bound + " elements in a bounded sequence, and this one has "
                            + actual);
        }
    }

    /**
     * The count check every bounded {@code map} makes before a byte of it is
     * written. Its own message and not {@link #checkSequenceBound}'s: the
     * unit is an entry and not an element, per XTypes 1.3 §7.4.1.1.4's
     * map-as-a-sequence-of-pairs equivalence.
     *
     * @throws WireException.OutOfRange if {@code actual} is greater than
     *     {@code bound}
     */
    public void checkMapBound(int actual, int bound) {
        if (actual > bound) {
            throw new WireException.OutOfRange(
                    len, "at most " + bound + " entries in a bounded map, and this one has "
                            + actual);
        }
    }

    /**
     * A {@code wstring}, in the form {@code policy} and {@code order} name.
     *
     * <p>{@link WstringLengthPolicy#SPECIFICATION} is a four-byte byte count
     * and then UTF-16 code units. No BOM and no terminator — XTypes 1.3
     * §7.4.1.1.2 forbids both outright: "The UTF-16 representation … shall not
     * include a Byte Order Mark (BOM). The representation shall also not
     * include any terminating NUL character(s)." The order the units go out in
     * is the stream's, which the encapsulation identifier already announced.
     *
     * <p>The count is of <b>octets</b> under that policy and not of
     * characters: §7.4.1.1.2 says "This is twice the number of characters in
     * the string", with the clause's own rationale being that a code point
     * outside the Basic Multilingual Plane takes two units and a byte count
     * still reaches the end of the string. So {@code "𝄞"} is a length of four.
     *
     * <p>A Java {@code String} already holds UTF-16 code units, which is the
     * one thing this column gets for free. What it does not get for free is
     * the surrogate rule: a lone surrogate is a {@code char} Java admits and
     * is not a code point, so it is refused rather than written.
     */
    public void writeWstringUtf8(
            String value, WstringLengthPolicy policy, WstringUnitOrder order) {
        if (policy == null || order == null) {
            throw new IllegalArgumentException("a wstring policy and unit order, not null");
        }
        if (policy.countsCharacters() && policy.unitWidth() == 2) {
            // Fast CDR counts characters and writes each four-byte `wchar_t`
            // as two bytes, so a code point outside the BMP goes out as its
            // low sixteen bits and arrives as a different character. A
            // vendor's length convention is a policy; its data loss is not, so
            // the value is refused. The refusal is on the pair (characters,
            // two octets) and not on a named form: a character count over
            // four-octet units carries the surrogate pair intact.
            int index = 0;
            while (index < value.length()) {
                int point = value.codePointAt(index);
                if (point > 0xFFFF) {
                    throw new WireException.OutOfRange(
                            index,
                            "a code point inside the BMP: a character count over two-octet "
                                    + "units has no way to carry one outside it");
                }
                index += Character.charCount(point);
            }
        }

        int[] units = utf16Units(value);
        int count = units.length;
        boolean terminated = policy.isTerminated();
        boolean marked = order == WstringUnitOrder.BYTE_ORDER_MARK && count > 0;
        // CORBA §9.3.1.6's GIOP 1.1 rule, and RTI's XCDR1 form: a NUL code
        // unit at the end, inside whatever the length counts. Nothing is lost
        // above the BMP — both units are written.
        //
        // An empty `wstring` carries no mark: omniORB writes `00000000` for
        // one, measured 2026-08-10, byte-identical to every other
        // implementation. A mark on an empty string would be a two-octet
        // string that decodes to nothing, and no peer writes it.
        int total = count + (terminated ? 1 : 0) + (marked ? 1 : 0);
        int width = policy.unitWidth();
        writeU32(policy.countsCharacters() ? total : (long) total * width);
        if (marked) {
            writeWstringUnit(BYTE_ORDER_MARK, width, order);
        }
        for (int index = 0; index < count; index++) {
            writeWstringUnit(units[index], width, order);
        }
        if (terminated) {
            writeWstringUnit(0, width, order);
        }
    }

    /**
     * A {@code wstring} whose characters go out as their own UTF-8 octets.
     *
     * <p>{@code WstringCodeUnits::Utf8} is the measured departure of one
     * product's generated serialiser: XTypes 1.3 Table 31 gives a {@code
     * wstring}'s code unit as {@code Char16} and §7.4.1.1.2 makes the length
     * "twice the number of characters", so the clause has no UTF-8 form at
     * all. The declared length is the UTF-8 octet count and only C2 — the
     * terminator — still reaches the wire. {@code wstring-member} is {@code
     * 04 00 00 00 61 62 63 00} for {@code "abc"}: three octets, a NUL, and the
     * four declares both.
     *
     * @throws WireException.NotUtf8 if {@code value} holds an unpaired
     *     surrogate, which is not a code point
     */
    public void writeWstringUtf8Octets(String value, WstringLengthPolicy policy) {
        if (policy == null) {
            throw new IllegalArgumentException("a wstring length policy, not null");
        }
        byte[] encoded = utf8(value);
        boolean terminated = policy.isTerminated();
        long total = encoded.length + (terminated ? 1 : 0);
        if (total > 0xFFFFFFFFL) {
            throw new WireException.OutOfRange(len, "a wstring length the UInt32 field can hold");
        }
        writeU32(total);
        appendAll(encoded);
        if (terminated) {
            append((byte) 0);
        }
    }

    /**
     * A bounded UTF-8-octet {@code wstring}.
     *
     * <p>The bound still counts UTF-16 code units, which is what a {@code
     * wstring<N>} bound means, while the wire carries the UTF-8 octets.
     *
     * @throws WireException.OutOfRange if {@code value} holds more than
     *     {@code bound} UTF-16 code units
     */
    public void writeWstringUtf8OctetsBounded(
            String value, int bound, WstringLengthPolicy policy) {
        int units = utf16Units(value).length;
        if (units > bound) {
            throw new WireException.OutOfRange(
                    len,
                    "at most " + bound + " UTF-16 code units in a bounded wstring, and this "
                            + "one has " + units);
        }
        writeWstringUtf8Octets(value, policy);
    }

    /** One {@code wstring} code unit, at its width and in its order. */
    private void writeWstringUnit(int unit, int width, WstringUnitOrder order) {
        if (width == 4) {
            if (order == WstringUnitOrder.BIG_ENDIAN_ALWAYS) {
                writeU32Big(unit);
            } else if (order.isFixedLittle()) {
                writeU32Little(unit);
            } else {
                writeU32(unit);
            }
        } else if (order == WstringUnitOrder.BIG_ENDIAN_ALWAYS) {
            writeU16Big(unit);
        } else if (order.isFixedLittle()) {
            writeU16Little(unit);
        } else {
            writeU16(unit);
        }
    }

    /**
     * {@code fixed<digits, scale>} as packed BCD. [CDR] §9.3.2.8.
     *
     * <p>DDS-XTypes 1.3 does not carry the type at all — §7.2.2 has no
     * fixed-point kind and §7.4.3.5.3 has no rule whose left-hand side matches
     * one — so the form is CORBA's, which is where an IDL {@code fixed} came
     * from and the only wire definition it has.
     *
     * <p>The value arrives as a decimal string because a JSON number is a
     * double and {@code fixed<31, 2>} is exactly the case a double cannot
     * carry. It is <b>not</b> parsed through {@link java.math.BigDecimal}
     * either: {@code BigDecimal} would accept an exponent and normalise a
     * scale, and neither belongs on this path.
     */
    public void writeFixed(String value, int digits, int scale, FixedDigitCount count) {
        int[] nibbles = fixedNibbles(value, digits, scale, count);
        for (int index = 0; index < nibbles.length; index += 2) {
            append((byte) ((nibbles[index] << 4) | nibbles[index + 1]));
        }
    }

    /**
     * The BCD nibbles of {@code text} under {@code fixed<digits, scale>}.
     *
     * <p>[CDR] §9.3.2.8: digit nibbles most-significant first, then a sign
     * nibble — {@code 0xc} for positive or zero, {@code 0xd} for negative —
     * with a leading zero nibble prepended when the nibble count is odd so the
     * nibbles pair up.
     *
     * <p>Neither half is padded in the direction that would lose a digit: a
     * value too long for its integer part or its fraction is one this type
     * cannot represent, and is refused.
     *
     * <p>How many digits go out is {@link FixedDigitCount}: the clause gives
     * the packing and says nothing about the count, and the four
     * implementations that write a {@code fixed} split two and two.
     */
    public static int[] fixedNibbles(
            String text, int digits, int scale, FixedDigitCount count) {
        if (count == null) {
            throw new WireException.OutOfRange(0, "a fixed digit count, not null");
        }
        if (digits == 0) {
            throw new WireException.OutOfRange(0, "a fixed with at least one digit");
        }
        // OMG IDL 4.2 rule (42) caps the BCD rule at 31 digits, and
        // fixed<31, S> is the widest declaration IDL admits. A wider one has
        // no packing defined for it, so it is refused rather than packed into
        // octets no reader can be told the width of.
        if (digits > 31) {
            throw new WireException.OutOfRange(0, "a fixed of at most 31 digits, not " + digits);
        }
        if (scale > digits) {
            throw new WireException.OutOfRange(0, "a scale no larger than the digit count");
        }
        boolean negative = false;
        String magnitude = text;
        if (magnitude.startsWith("-")) {
            negative = true;
            magnitude = magnitude.substring(1);
        } else if (magnitude.startsWith("+")) {
            magnitude = magnitude.substring(1);
        }
        int dot = magnitude.indexOf('.');
        String whole = dot < 0 ? magnitude : magnitude.substring(0, dot);
        String fraction = dot < 0 ? "" : magnitude.substring(dot + 1);
        String bad = "a decimal that fits fixed<" + digits + ", " + scale + ">, not `" + text + "`";
        if (whole.isEmpty() && fraction.isEmpty()) {
            throw new WireException.OutOfRange(0, bad);
        }
        if (!isAsciiDigits(whole) || !isAsciiDigits(fraction)) {
            throw new WireException.OutOfRange(0, bad);
        }
        int asWritten = whole.length() + scale;
        int integralWidth = digits - scale;
        int first = 0;
        while (first < whole.length() && whole.charAt(first) == '0') {
            first++;
        }
        whole = whole.substring(first);
        if (whole.length() > integralWidth) {
            throw new WireException.OutOfRange(
                    0,
                    "a decimal whose integer part fits " + integralWidth + " digits, not `"
                            + text + "`");
        }
        if (fraction.length() > scale) {
            throw new WireException.OutOfRange(
                    0, "a decimal whose fraction fits " + scale + " digits, not `" + text + "`");
        }

        // The full `digits`-wide field first, whatever the count: the value's
        // precision is that field with its leading zeros removed, so both
        // counts are read off one construction.
        int[] every = new int[digits];
        int at = integralWidth - whole.length();
        for (int index = 0; index < whole.length(); index++) {
            every[at++] = whole.charAt(index) - '0';
        }
        for (int index = 0; index < fraction.length(); index++) {
            every[at++] = fraction.charAt(index) - '0';
        }
        int leadingZeros = 0;
        while (leadingZeros < every.length && every[leadingZeros] == 0) {
            leadingZeros++;
        }
        int significant = every.length - leadingZeros;
        int wire = digits;
        if (count == FixedDigitCount.VALUE_PRECISION) {
            wire = Math.max(significant, 1);
        } else if (count == FixedDigitCount.VALUE_STRING_DIGITS) {
            // ACE/TAO 2.5.24: the integer digits as written plus S, made odd by one leading
            // zero. Past 31 no reader can be told the field's width.
            wire = asWritten + (asWritten % 2 == 0 ? 1 : 0);
            if (wire > 31) {
                throw new WireException.OutOfRange(
                        0,
                        "a fixed of at most 31 digits on the wire, not " + wire + " for `" + text
                                + "`");
            }
        }

        boolean odd = (wire + 1) % 2 == 1;
        int[] nibbles = new int[wire + 1 + (odd ? 1 : 0)];
        int write = 0;
        if (odd) {
            nibbles[write++] = 0;
        }
        // A count wider than P is filled with leading zeros (the array starts
        // zeroed).
        int kept = Math.min(wire, every.length);
        write += wire - kept;
        System.arraycopy(every, every.length - kept, nibbles, write, kept);
        write += kept;
        // A negative zero is written positive, because there is one zero.
        nibbles[write] = negative && significant > 0 ? 0x0D : 0x0C;
        return nibbles;
    }

    private static boolean isAsciiDigits(String text) {
        for (int index = 0; index < text.length(); index++) {
            char character = text.charAt(index);
            if (character < '0' || character > '9') {
                return false;
            }
        }
        return true;
    }

    // -- the bit-bounded kinds --------------------------------------------

    /**
     * An enumerator, as the integer its bit bound calls for.
     *
     * <p>XTypes 1.3 §7.4.3.5.3 rule (5) is
     * {@code XCDR << {O : ENUM_TYPE} = XCDR << {O.value : O.holder_type}}, and
     * the ENUM_TYPE row of the same clause's table says "O.holder_type is
     * either Int8, Int16 or Int32 depending on the value of the @bit_bound
     * annotation". A <b>signed</b> holder: {@code @value(-1)} is the two's
     * complement of one, so at the default bound it goes out as
     * {@code ffffffff}.
     *
     * <p>What fits is everything expressible in the bound's bits, signed or
     * unsigned — {@code -2^(n-1) ..= 2^n - 1}. RTI Connext 7.7 accepts
     * {@code @value(-1)} at the default bound and Cyclone accepts
     * {@code @value(200)} under {@code @bit_bound(8)}; a signed-only range
     * would reject the second and a wider one would accept values neither can
     * express.
     *
     * <p>§7.3.1.2.1.5 caps the bound itself at 1 to 32 for an enumerated type,
     * so a bound outside that has no holder and is refused rather than
     * widened.
     */
    public void writeEnum(int bitBound, long value) {
        if (bitBound <= 0 || bitBound > 32) {
            throw new WireException.OutOfRange(
                    len, "an @bit_bound between 1 and 32, not " + bitBound);
        }
        long low = -(1L << (bitBound - 1));
        long high = (1L << bitBound) - 1;
        if (value < low || value > high) {
            throw new WireException.OutOfRange(
                    len, "an enumerator a " + bitBound + "-bit holder can hold, not " + value);
        }
        long pattern = value & ((1L << bitBound) - 1);
        if (bitBound <= 8) {
            writeU8((int) pattern);
        } else if (bitBound <= 16) {
            writeU16((int) pattern);
        } else {
            writeU32(pattern);
        }
    }

    /**
     * A bitmask, as the unsigned integer its bit bound calls for.
     *
     * <p>XTypes 1.3 §7.4.3.5.3 rule (6) writes {@code O.value} as
     * {@code O.holder_type}, and Table 33 of §7.4.1.1.3.2 gives the holder:
     * {@code UInt8} for a bound of 1 to 8, {@code UInt16} to 16,
     * {@code UInt32} to 32 and {@code UInt64} to 64. The same clause fixes the
     * direction — "Bit indexes are counted from zero starting at the
     * least-significant bit of the full byte size of the bitmask" — so bit
     * {@code n} is {@code 1 << n} whatever the holder's width.
     *
     * <p>Below a bound of 64 the argument is a <i>number</i> and a bit at or
     * past the bound is refused: the clause says those bits "are not
     * considered to be part of the bitmask", so setting one would put a value
     * on the wire no peer must read back. At a bound of exactly 64 the
     * argument is the <i>bit pattern</i>, because there is no wider Java
     * integer to hold bit 63 as a number.
     */
    public void writeBitmask(int bitBound, long bits) {
        if (bitBound <= 0 || bitBound > 64) {
            throw new WireException.OutOfRange(
                    len, "an @bit_bound between 1 and 64, not " + bitBound);
        }
        if (bitBound < 64 && (bits < 0 || (bits >>> bitBound) != 0)) {
            throw new WireException.OutOfRange(len, "a bitmask below 2^" + bitBound + ", not " + bits);
        }
        if (bitBound <= 8) {
            writeU8((int) bits);
        } else if (bitBound <= 16) {
            writeU16((int) bits);
        } else if (bitBound <= 32) {
            writeU32(bits);
        } else {
            writeU64(bits);
        }
    }

    /**
     * One bitfield, shifted into the accumulator a bitset packs into.
     *
     * The counterpart of {@code zdw_bitfield}. A value wider than the
     * field is refused rather than masked: masking would put a different
     * number on the wire than the caller asked for, and the caller would
     * never find out.
     *
     * @param value the field's value
     * @param width how many bits the field declared
     * @param position the field's low bit inside the anchor
     * @return the value in place, or zero with the writer already failed
     */
    public long bitfield(long value, int width, int position) {
        if (width <= 0 || width > 64 || position < 0 || position + width > 64) {
            throw new WireException.OutOfRange(
                    len, "a bitfield of " + width + " bits at " + position + ", inside 64");
        }
        long limit = width >= 64 ? -1L : ((1L << width) - 1L);
        // Unsigned, because a 64-bit field's limit is every bit set and
        // a signed comparison would read that as -1.
        if (Long.compareUnsigned(value, limit) > 0) {
            throw new WireException.OutOfRange(
                    len, "a value inside " + width + " bits, not " + Long.toUnsignedString(value));
        }
        return value << position;
    }

    /**
     * A bitset, as the anchor integer its bitfields pack into.
     *
     * <p>DDS-XTypes 1.3 has no serialisation rule for a bitset — §7.4.3.5.3
     * runs from rule (1) to rule (30) and no left-hand side matches one. How
     * wide the anchor is comes from the layout planner and arrives here as a
     * number, the same as every other framing decision.
     *
     * <p>At an anchor of eight the argument is the bit pattern, for the same
     * reason {@link #writeU64(long)}'s is. Below that it is a number, and one
     * the anchor cannot hold is refused rather than masked: masking would
     * drop the bitfields above the cut and report success.
     */
    public void writeBitset(int anchorBytes, long bits) {
        switch (anchorBytes) {
            case 1:
                check(bits, 0L, 0xFFL, "a one-byte bitset anchor");
                writeU8((int) bits);
                return;
            case 2:
                check(bits, 0L, 0xFFFFL, "a two-byte bitset anchor");
                writeU16((int) bits);
                return;
            case 4:
                check(bits, 0L, 0xFFFFFFFFL, "a four-byte bitset anchor");
                writeU32(bits);
                return;
            case 8:
                writeU64(bits);
                return;
            default:
                throw new WireException.OutOfRange(
                        len, "a bitset anchor of 1, 2, 4 or 8 bytes, not " + anchorBytes);
        }
    }

    // -- framing ----------------------------------------------------------

    /** A DHEADER waiting for its length. */
    public static final class DHeader {
        /** Where the four-byte placeholder sits. */
        final int at;
        /** Whether there is one at all. */
        final boolean present;

        DHeader(int at, boolean present) {
            this.at = at;
            this.present = present;
        }

        /** Where the placeholder sits. */
        public int at() {
            return at;
        }

        /** Whether a DHEADER was written at all. */
        public boolean isPresent() {
            return present;
        }
    }

    /**
     * A DHEADER: a four-byte length the members that follow occupy.
     *
     * <p>Written as a placeholder and patched by {@link #finishDheader} when
     * the members are done, because the length is not known until then.
     */
    public DHeader writeDheader() {
        alignTo(4);
        int at = len;
        appendZeros(4);
        return new DHeader(at, true);
    }

    /**
     * A DHEADER that is not written at all, so that generated code can hold
     * one unconditionally and let the plan decide.
     */
    public static DHeader noDheader() {
        return new DHeader(0, false);
    }

    /** Patch a DHEADER with the length of what followed it. */
    public void finishDheader(DHeader header) {
        if (!header.present) {
            return;
        }
        patch(header.at, len - header.at - 4, 4);
    }

    /** An EMHEADER waiting for its NEXTINT, when it has one. */
    public static final class EmHeader {
        /** Where the NEXTINT placeholder sits, when there is one. */
        final int nextInt;
        /** Whether a placeholder was reserved. */
        final boolean hasNextInt;
        /**
         * Where the header word itself sits, so a short length code can be
         * patched into it once the value's length is known.
         */
        final int word;
        /**
         * The code the header was written with, so that
         * {@link Writer#finishEmheader} can hold the member's value to the
         * length the code declares. Codes 0 to 3 are lengths; anything else
         * declares nothing of its own.
         */
        final int lengthCode;

        EmHeader(int nextInt, boolean hasNextInt, int word, int lengthCode) {
            this.nextInt = nextInt;
            this.hasNextInt = hasNextInt;
            this.word = word;
            this.lengthCode = lengthCode;
        }

        /** Where the header word sits. */
        public int word() {
            return word;
        }

        /** Whether a NEXTINT placeholder was reserved. */
        public boolean hasNextInt() {
            return hasNextInt;
        }
    }

    /**
     * An XCDR2 EMHEADER: the member id, its length code and the
     * must-understand flag in one 32-bit word. XTypes 1.3 §7.4.3.4.2.
     *
     * <p>The length code is the layout plan's decision and arrives here as a
     * number; this runtime does not choose one.
     *
     * <p>LC0 through LC3 carry the length in the code itself. LC4 is followed
     * by a NEXTINT holding the member's length. LC5 through LC7 also have a
     * NEXTINT, but §7.4.3.4.2 says it "shall be reused also as part of the
     * serialized member … because the serialization of certain members also
     * starts with an integer length, which would take exactly the same value
     * as NEXTINT" — so <b>no placeholder is reserved for them</b>: the
     * member's own leading word lands in that position and <i>is</i> the
     * NEXTINT. Reserving one would write the length twice.
     */
    public EmHeader writeEmheader(int memberId, int lengthCode, boolean mustUnderstand) {
        if (lengthCode < 0 || lengthCode > 7) {
            throw new WireException.OutOfRange(len, "a length code between 0 and 7, not " + lengthCode);
        }
        // §7.2.2.4.4.4.4 runs member ids from 0 to 0x0FFFFFFF and §7.4.3.4.2
        // leaves them the low 28 bits of the word. Masking a larger one writes
        // a well-formed header for `id & 0x0FFFFFFF`: the peer reads a member
        // the caller did not name, and nothing at either end says so.
        if (memberId < 0 || memberId > E_MEMBER_ID_MASK) {
            throw new WireException.OutOfRange(
                    len, "a member id inside the 28 bits of an EMHEADER, not " + memberId);
        }
        alignTo(4);
        long word = ((mustUnderstand ? 1L : 0L) << 31)
                | ((long) (lengthCode & 0x7) << 28)
                | (memberId & (long) E_MEMBER_ID_MASK);
        writeU32(word);
        int wordAt = len - 4;
        if (lengthCode == 4) {
            int at = len;
            appendZeros(4);
            return new EmHeader(at, true, wordAt, lengthCode);
        }
        return new EmHeader(0, false, wordAt, lengthCode);
    }

    /**
     * Patch an EMHEADER's NEXTINT with the length of what followed.
     *
     * <p>Under {@link #shortLengthCodes()} an LC4 header whose member turned
     * out to be 1, 2, 4 or 8 bytes long instead loses its NEXTINT and states
     * the length in the code, which is what Fast DDS does.
     */
    public void finishEmheader(EmHeader header) {
        if (!header.hasNextInt) {
            // Under codes 0 to 3 the code *is* the length — 1, 2, 4 or 8
            // octets, §7.4.3.4.2. A value of another size misdeclares it and
            // the reader's skip lands inside data, so the mismatch is refused
            // here rather than written. Codes 5 to 7 declare nothing of their
            // own: the member's leading length word is the NEXTINT.
            if (header.lengthCode >= 0 && header.lengthCode <= 3 && header.word + 4 <= len) {
                int declared = 1 << header.lengthCode;
                int body = len - header.word - 4;
                if (body != declared) {
                    throw new WireException.OutOfRange(len,
                            "a member of " + declared + " octets under length code "
                                    + header.lengthCode + ", not " + body);
                }
            }
            return;
        }
        int length = len - header.nextInt - 4;
        if (shortLengthCodes && (length == 1 || length == 2 || length == 4 || length == 8)) {
            shortenEmheader(header, length);
            return;
        }
        patch(header.nextInt, length, 4);
    }

    /**
     * Remove an LC4 header's NEXTINT and raise its length code.
     *
     * <p>Every byte of the member's value is unchanged and only its position
     * moves, which is safe for one reason and only under XCDR2: MAXALIGN is
     * four there, so the value began at a four-aligned offset with the NEXTINT
     * in front of it and begins at a four-aligned offset without it.
     */
    private void shortenEmheader(EmHeader header, int length) {
        int code;
        switch (length) {
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
        delete(header.nextInt, 4);
        long previous = readBack(header.word, 4);
        patch(header.word, (previous & ~(0x7L << 28)) | ((long) code << 28), 4);
    }

    /** A parameter header waiting for its length. */
    public static final class Pid {
        final int at;
        final boolean extended;
        final int savedOrigin;
        final ParameterRole role;
        /**
         * The member id and its must-understand, kept so that a short header
         * can be rewritten as an extended one when the value turns out not to
         * fit sixteen bits. The bit is in a different word in the two forms.
         */
        final int pid;
        final boolean mustUnderstand;

        Pid(int at, boolean extended, int savedOrigin, ParameterRole role, int pid,
                boolean mustUnderstand) {
            this.at = at;
            this.extended = extended;
            this.savedOrigin = savedOrigin;
            this.role = role;
            this.pid = pid;
            this.mustUnderstand = mustUnderstand;
        }

        /** Where the length field sits. */
        public int at() {
            return at;
        }

        /** Whether the header was written in the extended form. */
        public boolean isExtended() {
            return extended;
        }

        /** Which of §7.4.1.2's two roles it stands in. */
        public ParameterRole role() {
            return role;
        }
    }

    /**
     * A PL_CDR1 parameter header, short form:
     * {@code [id:u16][length:u16]}.
     *
     * <p>{@code mustUnderstand} is bit 14 of the id. §7.4.1.2.2: the flag
     * "shall be set if and only if the must_understand property of the member
     * is set to true". Two claimants on the same word, deliberately: the
     * member's own declaration, and whatever the framing puts on every id
     * regardless — OpenDDS's {@code c0010004} is the second alone.
     *
     * <p>The parameter's value is its own encapsulated block, so this moves
     * the alignment origin; {@link #finishPid} puts it back before aligning to
     * four.
     */
    public Pid writePid(int pid, ParameterRole role, boolean mustUnderstand) {
        check(pid, 0, 0xFFFF, "a parameter id");
        // §7.4.1.2.2 gives the short form the ids 0 to highestShortId
        // inclusive and reserves what is above it — PID_EXTENDED is 0x3F01
        // itself. A short header with a reserved id collides with the
        // protocol's own parameters, so such an id takes the extended form or
        // none.
        if (pid > plCdr1.highestShortId()) {
            throw new WireException.OutOfRange(
                    len,
                    "a parameter id the short header carries, not " + pid
                            + " — the extended header takes it");
        }
        alignTo(4);
        int identifier = pid;
        if (mustUnderstand && plCdr1.memberMustUnderstand()) {
            identifier |= PID_MUST_UNDERSTAND;
        }
        identifier |= plCdr1.pidFlags().flagsAt(PidFlagPolicy.Position.MEMBER);
        int bare = identifier & ~(PID_MUST_UNDERSTAND | PID_IMPL_EXTENSION);
        writeU16(identifier & 0xFFFF);
        int at = len;
        appendZeros(2);
        return new Pid(at, false, beginParameterValue(), role, bare, mustUnderstand);
    }

    /**
     * A PL_CDR1 parameter header, extended form.
     *
     * <p>{@code [PID_EXTENDED][8][id:u32][length:u32]}, required when the id
     * reaches the reserved range or the value is longer than {@code 0xFFFF},
     * and used throughout under Cyclone DDS's framing. XTypes 1.3 §7.4.1.2.
     *
     * <p>The member's must-understand is {@code FLAG_2} of the
     * {@code eMemberHeader} (§7.4.1.2.1) and not the marker's bit above it:
     * Cyclone sets the marker's on every parameter and {@code FLAG_2} only
     * where the member declares it.
     */
    public Pid writePidExtended(int memberId, ParameterRole role, boolean mustUnderstand) {
        // The same 28 bits as an EMHEADER's, here in the {@code eMemberHeader}
        // of §7.4.1.2.1. Masking aliases the parameter onto another member
        // exactly as it does there.
        if (memberId < 0 || memberId > E_MEMBER_ID_MASK) {
            throw new WireException.OutOfRange(
                    len, "a member id inside the 28 bits of an eMemberHeader, not " + memberId);
        }
        alignTo(4);
        int marker = PID_EXTENDED
                | plCdr1.pidFlags().flagsAt(PidFlagPolicy.Position.EXTENDED_MARKER);
        writeU16(marker & 0xFFFF);
        writeU16(8);
        long memberHeader = memberId & (long) E_MEMBER_ID_MASK;
        if (mustUnderstand && plCdr1.memberMustUnderstand()) {
            memberHeader |= E_FLAG_MUST_UNDERSTAND & 0xFFFFFFFFL;
        }
        writeU32(memberHeader);
        int at = len;
        appendZeros(4);
        return new Pid(
                at, true, beginParameterValue(), role, memberId & E_MEMBER_ID_MASK, mustUnderstand);
    }

    /**
     * Start a parameter's value, returning the origin it replaces.
     *
     * <p>"Each element, or parameter, within a parameter list data structure
     * is simply a CDR-encapsulated block of data" — XTypes 1.3 §7.4.1.2. A
     * block counts alignment from its own start, so an eight-byte member at
     * the head of one is already aligned and takes no padding inside its own
     * length.
     */
    private int beginParameterValue() {
        int saved = origin;
        origin = len;
        return saved;
    }

    /**
     * Patch a parameter header with its value's length.
     *
     * <p>The declared length covers the value's trailing padding or stops at
     * the value, depending on the framing — Cyclone and RTI round it up to
     * four, Fast DDS does not. The padding itself starts the next parameter at
     * a four-byte offset measured from the enclosing stream, so the origin
     * goes back before it is written. XTypes 1.3 §7.4.1.2.
     */
    public void finishPid(Pid header) {
        int width = header.extended ? 4 : 2;
        int length = len - header.at - width;
        if (plCdr1.padsDeclaredLength(header.role)) {
            int past = Math.floorMod(len - header.savedOrigin, 4);
            if (past != 0) {
                length += 4 - past;
            }
        }
        if (!header.extended && length > PID_SHORT_MAX_LEN) {
            promotePid(header, length);
        } else {
            patch(header.at, length, width);
        }
        origin = header.savedOrigin;
        // Under Fast DDS's framing the parameter writes no padding of its
        // own — the next header aligns itself, and a final parameter ends
        // the stream where its value does.
        if (plCdr1.padAfterValue()) {
            alignTo(4);
        }
    }

    /**
     * Rewrite a short parameter header as the extended form.
     *
     * <p>§7.4.1.2.2's {@code ssize} is sixteen bits, and which values overrun
     * it is a property of the sample rather than of the type — a
     * {@code string} member is short in almost every sample and over 64 KiB in
     * some. So the plan writes the short form for an unbounded member and the
     * fallback lives here, where the length is known.
     *
     * <p>Eight bytes are inserted between the header and the value. The
     * value's own bytes do not change: XCDR1's MAXALIGN is eight and the
     * insertion is eight, so every offset inside it keeps its remainder.
     */
    private void promotePid(Pid header, int length) {
        int start = header.at - 2;
        insertZeros(header.at + 2, 8);
        int marker = PID_EXTENDED
                | plCdr1.pidFlags().flagsAt(PidFlagPolicy.Position.EXTENDED_MARKER);
        patch(start, marker & 0xFFFF, 2);
        patch(start + 2, 8, 2);
        long memberHeader = header.pid & (long) E_MEMBER_ID_MASK;
        if (header.mustUnderstand && plCdr1.memberMustUnderstand()) {
            memberHeader |= E_FLAG_MUST_UNDERSTAND & 0xFFFFFFFFL;
        }
        patch(start + 4, memberHeader, 4);
        patch(start + 8, length, 4);
    }

    /**
     * The {@code PID_LIST_END} sentinel a PL_CDR1 parameter list ends with.
     *
     * <p>A reader that stops at the last member it recognises instead of at
     * the sentinel misreads whatever follows.
     *
     * <p>The flags come from the framing asked at the sentinel position, which
     * is the only thing that decides them: {@code PID_LIST_END} is not a
     * member and has no declaration of its own.
     */
    public void writeSentinel() {
        alignTo(4);
        int sentinel = PID_LIST_END | plCdr1.pidFlags().flagsAt(PidFlagPolicy.Position.SENTINEL);
        writeU16(sentinel & 0xFFFF);
        writeU16(0);
    }

    // -- the KeyHash ------------------------------------------------------

    /**
     * The holder as a KeyHash, zero-padded to sixteen bytes.
     *
     * <p>For a holder of sixteen bytes or fewer. DDSI-RTPS 2.5 §9.6.3.8.
     */
    public byte[] zeroPadded() {
        byte[] hash = new byte[KEY_HASH_LEN];
        System.arraycopy(out, 0, hash, 0, Math.min(len, KEY_HASH_LEN));
        return hash;
    }

    /**
     * The MD5 of the holder, for one longer than sixteen bytes.
     *
     * <p>MD5 because DDSI-RTPS 2.5 §9.6.3.8 names it, not as a security claim:
     * the KeyHash identifies an instance and is not a signature.
     */
    public byte[] md5() {
        return Md5.digest(out, 0, len);
    }

    // -- the buffer -------------------------------------------------------

    private void check(long value, long low, long high, String expected) {
        if (value < low || value > high) {
            throw new WireException.OutOfRange(
                    len, expected + " in [" + low + ", " + high + "], not " + value);
        }
    }

    /** Align to the value's own width, then write it. */
    private void writeInt(long pattern, int width) {
        alignTo(width);
        appendRaw(pattern, width, endianness.isLittle());
    }

    private void appendRaw(long pattern, int width, boolean little) {
        need(width);
        for (int index = 0; index < width; index++) {
            int shift = little ? index : width - 1 - index;
            out[len + index] = (byte) (pattern >>> (8 * shift));
        }
        len += width;
    }

    /** The {@code width} bytes at {@code at}, in the stream's order. */
    private long readBack(int at, int width) {
        long value = 0;
        for (int index = 0; index < width; index++) {
            int shift = endianness.isLittle() ? index : width - 1 - index;
            value |= (out[at + index] & 0xFFL) << (8 * shift);
        }
        return value;
    }

    private void patch(int at, long value, int width) {
        if (value < 0 || (width < 8 && value >= (1L << (width * 8)))) {
            throw new WireException.OutOfRange(at, "a " + width + "-byte length, not " + value);
        }
        for (int index = 0; index < width; index++) {
            int shift = endianness.isLittle() ? index : width - 1 - index;
            out[at + index] = (byte) (value >>> (8 * shift));
        }
    }

    private void append(byte value) {
        need(1);
        out[len++] = value;
    }

    private void appendAll(byte[] values) {
        need(values.length);
        System.arraycopy(values, 0, out, len, values.length);
        len += values.length;
    }

    private void appendZeros(int count) {
        need(count);
        Arrays.fill(out, len, len + count, (byte) 0);
        len += count;
    }

    private void insertZeros(int at, int count) {
        need(count);
        System.arraycopy(out, at, out, at + count, len - at);
        Arrays.fill(out, at, at + count, (byte) 0);
        len += count;
    }

    private void delete(int at, int count) {
        System.arraycopy(out, at + count, out, at, len - at - count);
        len -= count;
    }

    private void need(int more) {
        if (len + more <= out.length) {
            return;
        }
        int capacity = out.length;
        while (capacity < len + more) {
            capacity *= 2;
        }
        out = Arrays.copyOf(out, capacity);
    }

    // -- text -------------------------------------------------------------

    /**
     * The UTF-8 of {@code text}, refusing anything that is not UTF-8.
     *
     * <p>{@code String.getBytes(UTF_8)} would replace an unpaired surrogate
     * with {@code '?'} and report success — a different character on the wire,
     * silently. The encoder is configured to report instead.
     */
    private byte[] utf8(String text) {
        CharsetEncoder encoder = java.nio.charset.StandardCharsets.UTF_8
                .newEncoder()
                .onMalformedInput(CodingErrorAction.REPORT)
                .onUnmappableCharacter(CodingErrorAction.REPORT);
        try {
            ByteBuffer buffer = encoder.encode(CharBuffer.wrap(text));
            byte[] encoded = new byte[buffer.remaining()];
            buffer.get(encoded);
            return encoded;
        } catch (CharacterCodingException error) {
            throw new WireException.NotUtf8(len, "an unpaired surrogate is not a code point");
        }
    }

    /**
     * The UTF-16 code units of {@code text}, without a BOM.
     *
     * <p>A Java {@code String} is already UTF-16, so this is a copy — but it
     * is also the one place a lone surrogate can be caught. Java admits one in
     * a {@code String}; it is not a code point, and writing it would put a
     * sequence on the wire that is not UTF-16 at all.
     */
    private static int[] utf16Units(String text) {
        int[] units = new int[text.length()];
        int index = 0;
        while (index < text.length()) {
            char unit = text.charAt(index);
            if (Character.isHighSurrogate(unit)
                    && index + 1 < text.length()
                    && Character.isLowSurrogate(text.charAt(index + 1))) {
                // A well-formed pair: both units go out, and a byte count
                // still reaches the end of the string. §7.4.1.1.2.
                units[index] = unit;
                units[index + 1] = text.charAt(index + 1);
                index += 2;
                continue;
            }
            if (Character.isHighSurrogate(unit) || Character.isLowSurrogate(unit)) {
                throw new WireException.OutOfRange(
                        index * 2, "a Unicode code point, not a lone surrogate");
            }
            units[index] = unit;
            index++;
        }
        return units;
    }

    private static String hex(int codePoint) {
        String text = Integer.toHexString(codePoint).toUpperCase(java.util.Locale.ROOT);
        while (text.length() < 4) {
            text = "0" + text;
        }
        return text;
    }
}
