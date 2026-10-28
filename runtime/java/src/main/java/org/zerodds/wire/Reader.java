// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
package org.zerodds.wire;

import java.nio.ByteBuffer;
import java.nio.charset.CharacterCodingException;
import java.nio.charset.CharsetDecoder;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;

/**
 * Taking bytes off the wire.
 *
 * <p>The Java counterpart of {@code crates/wire/src/reader.rs} and the mirror
 * of {@link Writer} clause for clause. Every read that cannot be satisfied
 * raises a {@link WireException} naming what was expected and where: a reader
 * that returned a zero for a truncated stream would hand the caller a value
 * that was never sent.
 *
 * <p>The same two Java hazards the writer names apply in reverse. An unsigned
 * value that does not fit a signed Java integer is widened —
 * {@link #readU32()} returns a {@code long} — except for
 * {@link #readU64()}, where there is nothing to widen into and the result is
 * the bit pattern.
 */
public final class Reader {

    /** The low fourteen bits of a short parameter id, which are the id. */
    public static final int PID_ID_MASK = 0x3FFF;

    private final byte[] bytes;
    private int at;
    private final Endianness endianness;
    private final int maxAlign;
    private int origin;
    private final PidFlagPolicy pidFlags;
    private final int highestShortId;
    private Status status = Status.OK;
    private PlCdr1Framing framing = PlCdr1Framing.defaults();

    /**
     * A reader over {@code data}, capped at {@code maxAlign}.
     *
     * <p>Four for XCDR2, eight for XCDR1 — the writer's counterpart. A reader
     * that skipped padding the writer never wrote reads the wrong bytes from
     * the second member on.
     *
     * @param data the bytes; kept by reference and never modified
     * @param endianness the order the encapsulation identifier announced
     * @param maxAlign the encoding's MAXALIGN
     * @param pidFlags the writer's flag policy. The reader needs it for one
     *     thing: under {@link PidFlagPolicy#IMPL_EXTENSION_ON_MEMBERS} every
     *     member id carries {@code FLAG_IMPL_EXTENSION}, so the flag no longer
     *     tells a vendor-private parameter from a user member.
     * @param highestShortId the other thing it needs: §7.4.1.2.2 stops the
     *     short form's ids at {@code 0x3F00} inclusive and one implementation
     *     carries the whole low fourteen bits, so a reader holding the
     *     clause's threshold as a constant reports {@code @id(16383)} as a
     *     reserved parameter and drops the member.
     */
    public Reader(
            byte[] data,
            Endianness endianness,
            int maxAlign,
            PidFlagPolicy pidFlags,
            int highestShortId) {
        if (data == null || endianness == null) {
            throw new IllegalArgumentException("bytes and a byte order, not null");
        }
        this.bytes = data;
        this.at = 0;
        this.endianness = endianness;
        this.maxAlign = Math.max(1, maxAlign);
        this.origin = 0;
        this.pidFlags = pidFlags == null ? PidFlagPolicy.SENTINEL_MUST_UNDERSTAND : pidFlags;
        this.highestShortId = highestShortId;
    }

    /**
     * A reader over {@code data} expecting one implementation's PL_CDR1
     * framing whole.
     *
     * <p>The counterpart of {@code zdw_reader_set_pl_cdr1}. The two parts the
     * five-argument constructor takes are read off the framing, so a caller
     * cannot set one and forget the other.
     */
    public Reader(byte[] data, Endianness endianness, int maxAlign, PlCdr1Framing framing) {
        this(
                data,
                endianness,
                maxAlign,
                framing == null ? PidFlagPolicy.SENTINEL_MUST_UNDERSTAND : framing.pidFlags(),
                framing == null ? Writer.PID_SHORT_MAX_ID : framing.highestShortId());
        this.framing = framing == null ? PlCdr1Framing.defaults() : framing;
    }

    /**
     * A reader over {@code data} with XCDR1's MAXALIGN of eight and the
     * clause's PL_CDR1 framing.
     */
    public Reader(byte[] data, Endianness endianness) {
        this(data, endianness, 8, PidFlagPolicy.SENTINEL_MUST_UNDERSTAND, Writer.PID_SHORT_MAX_ID);
    }

    // -- the status a generated decoder reads ------------------------------

    /**
     * How the read is going.
     *
     * <p>The counterpart of {@code zdw_status} in {@code runtime/c}, and it
     * exists for the same reason: the runtime owns the wire and the generated
     * decoder owns the type, and some refusals are the type's — an integer
     * that names no enumerator, a bitmask bit the declaration does not have, a
     * sequence longer than its bound, a member that did not arrive. This
     * reader cannot see any of them.
     *
     * <p>A malformed <i>stream</i> still raises a {@link WireException}: a
     * length that runs past the end is not something a caller can carry on
     * from, and an exception names where it happened without every call site
     * having to ask. What the status carries is the refusals the generated
     * code makes — which are the ones it then has to read back.
     */
    public enum Status {
        /** Nothing has been refused. */
        OK,
        /**
         * §7.2.2.7: the sample is dropped and the receiver carries on. Not a
         * failure of the stream — the writer's type and the reader's differ,
         * and a caller that cannot tell that from a broken peer cannot tell a
         * narrowed type from a corrupt one.
         */
        DISCARDED,
        /** A length no stream that long can carry. */
        IMPOSSIBLE_LENGTH,
        /** A value outside what the type admits. */
        OUT_OF_RANGE,
        /** The stream ended where a value was expected. */
        TRUNCATED
    }

    /**
     * Everything a two-pass read has to put back.
     *
     * <p>One caller: a {@code map} under {@code rule-15-omits}, where a
     * delimited map states no entry count and the entries are read once to
     * count them and again into storage sized from that count. Java allocates
     * its own values, so there is no heap mark to carry — the cursor, the
     * stream origin and the status are the whole of it.
     */
    public static final class Mark {
        private final int at;
        private final int origin;
        private final Status status;

        Mark(int at, int origin, Status status) {
            this.at = at;
            this.origin = origin;
            this.status = status;
        }
    }

    /** Whether a refusal has been made, so a later read does nothing. */
    private boolean stopped() {
        return status != Status.OK;
    }

    /** How the read is going. */
    public Status readerStatus() {
        return status;
    }

    /** Refuse the stream from outside the runtime. The first refusal sticks. */
    public void readerRefuse(Status refusal) {
        if (refusal != null && refusal != Status.OK && status == Status.OK) {
            status = refusal;
        }
    }

    /**
     * Take back a {@link Status#DISCARDED}, and only that one.
     *
     * <p>A member whose own {@code @try_construct} is {@code USE_DEFAULT} but
     * which also carries an explicit {@code @default} cannot be answered
     * inside the type's decoder: the decoder knows Table 9's value for the
     * type and not the literal the member's declaration gave it, and
     * §7.2.2.4.4.4.9 puts the literal first. So the call site asks for
     * DISCARD, and where it gets one it clears it and assigns the literal.
     *
     * <p>Every other status stays: a truncated stream is not a narrowing.
     *
     * @return whether there was one
     */
    public boolean readerClearDiscard() {
        if (status == Status.DISCARDED) {
            status = Status.OK;
            return true;
        }
        return false;
    }

    /** Where it is. The generated decoder's name for {@link #position()}. */
    public int readerPosition() {
        return at;
    }

    /** How many bytes are left. */
    public int readerRemaining() {
        return remaining();
    }

    /**
     * Move the alignment origin to the current position.
     *
     * <p>The read-side pair of the writer's {@code beginPackedElement}, for
     * {@code ElementPadding::Packed}. Returns the origin displaced.
     */
    public int readerBeginPackedElement() {
        int saved = origin;
        origin = at;
        return saved;
    }

    /** Restore the origin {@link #readerBeginPackedElement()} displaced. */
    public void readerFinishPackedElement(int saved) {
        origin = saved;
    }

    /**
     * Move the cursor forward to {@code target}, which is never backwards: a
     * framing word pointing behind the cursor means the reader has already
     * read past what it framed, and seeking back would paper over it.
     */
    public void readerSkipTo(int target) {
        if (stopped()) {
            return;
        }
        if (target < at) {
            readerRefuse(Status.OUT_OF_RANGE);
            return;
        }
        if (target > bytes.length) {
            throw new WireException.Truncated(at, (long) target - at, remaining());
        }
        at = target;
    }

    /** The cursor, the origin and the status, to be put back later. */
    public Mark readerSave() {
        return new Mark(at, origin, status);
    }

    /** Put back what {@link #readerSave()} took. */
    public void readerRestore(Mark mark) {
        if (mark == null) {
            return;
        }
        at = mark.at;
        origin = mark.origin;
        status = mark.status;
    }

    /**
     * A {@code string} the reader materialises itself.
     *
     * <p>{@code runtime/c} takes it out of a heap the caller set, because an
     * embedded target decides where memory comes from. Java has one answer to
     * that question and this is it; the name is kept so that one walk names
     * one operation in both columns.
     */
    public String readStringAlloc() {
        if (stopped()) {
            return "";
        }
        return readString();
    }

    /**
     * A {@code wstring}, in whichever length convention the policy names.
     *
     * <p>{@code runtime/c} converts to UTF-8 because that is what its
     * {@code const char *} holds, and its name carries it. A Java
     * {@code String} is UTF-16 already and {@link #readWstring} is the whole
     * of the operation.
     */
    public String readWstringUtf8(WstringLengthPolicy policy, WstringUnitOrder order) {
        if (stopped()) {
            return "";
        }
        return readWstring(policy, order);
    }

    /**
     * How many octets {@code text} is, which is what a {@code string<N>}
     * bound counts: {@code string<5>} is {@code char[6]} in the C mapping of
     * every measured DDS implementation, so a five-scalar value of ten octets
     * is one no peer can hold.
     */
    public static int utf8Octets(String text) {
        return text == null ? 0 : text.getBytes(StandardCharsets.UTF_8).length;
    }

    /**
     * How many UTF-16 code units {@code text} is, which is what a
     * {@code wstring<N>} bound counts. A surrogate pair is two, which is what
     * {@link String#length()} already answers.
     */
    public static int utf16Units(String text) {
        return text == null ? 0 : text.length();
    }

    /**
     * {@code text} cut to at most {@code octets} bytes, never inside a
     * character.
     *
     * <p>Table 11: "The order of the characters in the string ... is
     * preserved", so this is a prefix. A character that would straddle the
     * bound is left out whole: a peer holding {@code char[octets + 1]} cannot
     * store half of one either, and half a UTF-8 sequence is not text.
     */
    public String trimUtf8(String text, int octets) {
        if (text == null || octets < 0) {
            return "";
        }
        StringBuilder out = new StringBuilder();
        int used = 0;
        int index = 0;
        while (index < text.length()) {
            int codePoint = text.codePointAt(index);
            int width = new String(Character.toChars(codePoint))
                    .getBytes(StandardCharsets.UTF_8).length;
            if (used + width > octets) {
                break;
            }
            out.appendCodePoint(codePoint);
            used += width;
            index += Character.charCount(codePoint);
        }
        return out.toString();
    }

    /**
     * {@code text} cut to at most {@code units} UTF-16 code units, never
     * between the halves of a surrogate pair.
     */
    public String trimUtf16(String text, int units) {
        if (text == null || units < 0) {
            return "";
        }
        if (text.length() <= units) {
            return text;
        }
        int end = units;
        // A high surrogate at the cut has lost its pair, and half a pair is
        // not a character.
        if (end > 0 && Character.isHighSurrogate(text.charAt(end - 1))) {
            end -= 1;
        }
        return text.substring(0, end);
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

    /** Where it is. */
    public int position() {
        return at;
    }

    /** How many bytes are left. */
    public int remaining() {
        return Math.max(0, bytes.length - at);
    }

    /** Skip padding to {@code align}, counted from the stream's origin. */
    public void alignTo(int align) {
        // A refusal already made stops every later read: the generated
        // decoder carries on to the end of the type, exactly as the C
        // column's does, and a read that consumed bytes here would put
        // the trailing-bytes check at the top on a position no writer
        // produced.
        if (stopped()) {
            return;
        }
        int effective = Math.min(align, maxAlign);
        if (effective <= 1) {
            return;
        }
        int offset = at - origin;
        at += (effective - Math.floorMod(offset, effective)) % effective;
    }

    /** Step over {@code count} bytes, refusing to step past the end. */
    public void skip(long count) {
        // A refusal already made stops every later read: the generated
        // decoder carries on to the end of the type, exactly as the C
        // column's does, and a read that consumed bytes here would put
        // the trailing-bytes check at the top on a position no writer
        // produced.
        if (stopped()) {
            return;
        }
        take(count);
    }

    /** Move to an absolute offset inside the buffer. */
    public void seek(int target) {
        // A refusal already made stops every later read: the generated
        // decoder carries on to the end of the type, exactly as the C
        // column's does, and a read that consumed bytes here would put
        // the trailing-bytes check at the top on a position no writer
        // produced.
        if (stopped()) {
            return;
        }
        if (target < 0 || target > bytes.length) {
            throw new WireException.Truncated(at, target, bytes.length);
        }
        at = target;
    }

    private int take(long count) {
        if (count < 0) {
            throw new WireException.Truncated(at, count, remaining());
        }
        long end = at + count;
        if (end > bytes.length) {
            throw new WireException.Truncated(at, count, remaining());
        }
        int from = at;
        at = (int) end;
        return from;
    }

    private long readInt(int width, boolean signed) {
        alignTo(width);
        int from = take(width);
        long value = 0;
        for (int index = 0; index < width; index++) {
            int shift = endianness.isLittle() ? index : width - 1 - index;
            value |= (bytes[from + index] & 0xFFL) << (8 * shift);
        }
        if (signed && width < 8) {
            long sign = 1L << (width * 8 - 1);
            if ((value & sign) != 0) {
                value -= 1L << (width * 8);
            }
        }
        return value;
    }

    private long readRaw(int width, boolean little) {
        alignTo(width);
        int from = take(width);
        long value = 0;
        for (int index = 0; index < width; index++) {
            int shift = little ? index : width - 1 - index;
            value |= (bytes[from + index] & 0xFFL) << (8 * shift);
        }
        return value;
    }

    // -- primitives -------------------------------------------------------

    /**
     * {@code boolean}.
     *
     * <p>A byte that is neither 0 nor 1 is a stream that disagrees with its
     * own type, and is refused rather than coerced.
     */
    public boolean readBool() {
        // A refusal already made stops every later read: the generated
        // decoder carries on to the end of the type, exactly as the C
        // column's does, and a read that consumed bytes here would put
        // the trailing-bytes check at the top on a position no writer
        // produced.
        if (stopped()) {
            return false;
        }
        int where = at;
        int value = readU8();
        if (value == 0) {
            return false;
        }
        if (value == 1) {
            return true;
        }
        throw new WireException.OutOfRange(where, "a boolean of 0 or 1");
    }

    /** {@code octet} and {@code uint8}, as a number in {@code [0, 255]}. */
    public int readU8() {
        // A refusal already made stops every later read: the generated
        // decoder carries on to the end of the type, exactly as the C
        // column's does, and a read that consumed bytes here would put
        // the trailing-bytes check at the top on a position no writer
        // produced.
        if (stopped()) {
            return 0;
        }
        return bytes[take(1)] & 0xFF;
    }

    /** {@code int8}. */
    public int readI8() {
        // A refusal already made stops every later read: the generated
        // decoder carries on to the end of the type, exactly as the C
        // column's does, and a read that consumed bytes here would put
        // the trailing-bytes check at the top on a position no writer
        // produced.
        if (stopped()) {
            return 0;
        }
        return bytes[take(1)];
    }

    /** {@code char}, one byte, as a code point in {@code [0, 255]}. */
    public int readChar() {
        // A refusal already made stops every later read: the generated
        // decoder carries on to the end of the type, exactly as the C
        // column's does, and a read that consumed bytes here would put
        // the trailing-bytes check at the top on a position no writer
        // produced.
        if (stopped()) {
            return 0;
        }
        return bytes[take(1)] & 0xFF;
    }

    /**
     * {@code wchar}, at {@code width} octets and in {@code framing}'s shape,
     * as a code point.
     *
     * <p>The four-byte form is RTI Connext's under XCDR1, where the high
     * sixteen bits are padding around one UTF-16 code unit. A value that uses
     * them is refused rather than narrowed: narrowing would return a character
     * the peer never sent.
     *
     * <p>{@link WcharFraming#GIOP_LENGTH_PREFIXED} is CORBA 3.3 part 2
     * §9.3.1.6's GIOP 1.2 form, which all three ORBs write. The octet count is
     * checked against the configured code-set width rather than believed: a
     * stream whose first octet says {@code 0xE4} — which is what a bare
     * two-octet {@code wchar} looks like read this way — would otherwise
     * consume 228 octets of whatever followed and return a character assembled
     * out of the next four members.
     */
    public int readWchar(WcharWidth width, WcharFraming framing, WstringUnitOrder order) {
        // A refusal already made stops every later read: the generated
        // decoder carries on to the end of the type, exactly as the C
        // column's does, and a read that consumed bytes here would put
        // the trailing-bytes check at the top on a position no writer
        // produced.
        if (stopped()) {
            return 0;
        }
        int where = at;
        long value;
        if (framing == WcharFraming.GIOP_LENGTH_PREFIXED) {
            int declared = readU8();
            if (declared != width.octets()) {
                throw new WireException.OutOfRange(
                        where,
                        "a GIOP 1.2 wchar of " + width.octets()
                                + " octets, the configured transmission code set, and not "
                                + declared);
            }
            // A scalar cannot carry a byte-order mark, so `byte-order-mark`
            // means CORBA's default here — big-endian — exactly as
            // `big-endian-always` does.
            boolean little = order == WstringUnitOrder.STREAM && endianness.isLittle();
            int from = take(width.octets());
            value = 0;
            for (int index = 0; index < width.octets(); index++) {
                int shift = little ? index : width.octets() - 1 - index;
                value |= (bytes[from + index] & 0xFFL) << (8 * shift);
            }
        } else if (width == WcharWidth.OCTETS_4) {
            value = readU32();
            if (value > 0xFFFF) {
                throw new WireException.OutOfRange(
                        where, "a four-byte wchar holding one UTF-16 code unit");
            }
        } else {
            value = readU16();
        }
        if (value >= 0xD800 && value <= 0xDFFF) {
            throw new WireException.OutOfRange(where, "a Unicode code point");
        }
        return (int) value;
    }

    /** {@code short}. */
    public int readI16() {
        // A refusal already made stops every later read: the generated
        // decoder carries on to the end of the type, exactly as the C
        // column's does, and a read that consumed bytes here would put
        // the trailing-bytes check at the top on a position no writer
        // produced.
        if (stopped()) {
            return 0;
        }
        return (int) readInt(2, true);
    }

    /** {@code unsigned short}, as a number in {@code [0, 65535]}. */
    public int readU16() {
        // A refusal already made stops every later read: the generated
        // decoder carries on to the end of the type, exactly as the C
        // column's does, and a read that consumed bytes here would put
        // the trailing-bytes check at the top on a position no writer
        // produced.
        if (stopped()) {
            return 0;
        }
        return (int) readInt(2, false);
    }

    /** {@code long}. */
    public int readI32() {
        // A refusal already made stops every later read: the generated
        // decoder carries on to the end of the type, exactly as the C
        // column's does, and a read that consumed bytes here would put
        // the trailing-bytes check at the top on a position no writer
        // produced.
        if (stopped()) {
            return 0;
        }
        return (int) readInt(4, true);
    }

    /**
     * {@code unsigned long}, as a number in {@code [0, 4294967295]} — which is
     * why it returns a {@code long}.
     */
    public long readU32() {
        // A refusal already made stops every later read: the generated
        // decoder carries on to the end of the type, exactly as the C
        // column's does, and a read that consumed bytes here would put
        // the trailing-bytes check at the top on a position no writer
        // produced.
        if (stopped()) {
            return 0L;
        }
        return readInt(4, false);
    }

    /** {@code long long}. */
    public long readI64() {
        // A refusal already made stops every later read: the generated
        // decoder carries on to the end of the type, exactly as the C
        // column's does, and a read that consumed bytes here would put
        // the trailing-bytes check at the top on a position no writer
        // produced.
        if (stopped()) {
            return 0L;
        }
        return readInt(8, true);
    }

    /**
     * {@code unsigned long long}, as the <i>bit pattern</i>.
     *
     * <p>There is no Java integer wider than {@code long}, so a value above
     * {@code Long.MAX_VALUE} comes back negative and is not a negative number:
     * {@code 0xFFFFFFFFFFFFFFFF} reads as {@code -1L}. Print it with
     * {@link Long#toUnsignedString(long)} and compute with it through
     * {@link java.math.BigInteger}.
     */
    public long readU64() {
        // A refusal already made stops every later read: the generated
        // decoder carries on to the end of the type, exactly as the C
        // column's does, and a read that consumed bytes here would put
        // the trailing-bytes check at the top on a position no writer
        // produced.
        if (stopped()) {
            return 0L;
        }
        return readInt(8, false);
    }

    /** {@code float}. */
    public float readF32() {
        // A refusal already made stops every later read: the generated
        // decoder carries on to the end of the type, exactly as the C
        // column's does, and a read that consumed bytes here would put
        // the trailing-bytes check at the top on a position no writer
        // produced.
        if (stopped()) {
            return 0.0f;
        }
        return Float.intBitsToFloat((int) readInt(4, false));
    }

    /** {@code double}. */
    public double readF64() {
        // A refusal already made stops every later read: the generated
        // decoder carries on to the end of the type, exactly as the C
        // column's does, and a read that consumed bytes here would put
        // the trailing-bytes check at the top on a position no writer
        // produced.
        if (stopped()) {
            return 0.0;
        }
        return Double.longBitsToDouble(readInt(8, false));
    }

    /**
     * {@code long double}: all sixteen bytes, as IEEE 754 binary128.
     *
     * <p>The counterpart of {@link Writer#writeF128}. Reading the first eight
     * bytes as a {@code double} and skipping the rest would decode every value
     * a {@code double} cannot hold as a different number and say nothing.
     */
    public Float128 readF128() {
        // A refusal already made stops every later read: the generated
        // decoder carries on to the end of the type, exactly as the C
        // column's does, and a read that consumed bytes here would put
        // the trailing-bytes check at the top on a position no writer
        // produced.
        if (stopped()) {
            return Float128.ZERO;
        }
        alignTo(8);
        int from = take(16);
        return Float128.fromBytes(
                Arrays.copyOfRange(bytes, from, from + 16), !endianness.isLittle());
    }

    /** A 16-bit value little-endian whatever the stream's order is. */
    public int readU16Little() {
        // A refusal already made stops every later read: the generated
        // decoder carries on to the end of the type, exactly as the C
        // column's does, and a read that consumed bytes here would put
        // the trailing-bytes check at the top on a position no writer
        // produced.
        if (stopped()) {
            return 0;
        }
        return (int) readRaw(2, true);
    }

    /** A 16-bit value big-endian whatever the stream's order is. */
    public int readU16Big() {
        // A refusal already made stops every later read: the generated
        // decoder carries on to the end of the type, exactly as the C
        // column's does, and a read that consumed bytes here would put
        // the trailing-bytes check at the top on a position no writer
        // produced.
        if (stopped()) {
            return 0;
        }
        return (int) readRaw(2, false);
    }

    /** A 32-bit value little-endian whatever the stream's order is. */
    public long readU32Little() {
        // A refusal already made stops every later read: the generated
        // decoder carries on to the end of the type, exactly as the C
        // column's does, and a read that consumed bytes here would put
        // the trailing-bytes check at the top on a position no writer
        // produced.
        if (stopped()) {
            return 0L;
        }
        return readRaw(4, true);
    }

    /** A 32-bit value big-endian whatever the stream's order is. */
    public long readU32Big() {
        // A refusal already made stops every later read: the generated
        // decoder carries on to the end of the type, exactly as the C
        // column's does, and a read that consumed bytes here would put
        // the trailing-bytes check at the top on a position no writer
        // produced.
        if (stopped()) {
            return 0L;
        }
        return readRaw(4, false);
    }

    // -- strings ----------------------------------------------------------

    /**
     * A {@code string}: a length including the terminator, the bytes, the NUL.
     */
    public String readString() {
        // A refusal already made stops every later read: the generated
        // decoder carries on to the end of the type, exactly as the C
        // column's does, and a read that consumed bytes here would put
        // the trailing-bytes check at the top on a position no writer
        // produced.
        if (stopped()) {
            return "";
        }
        int where = at;
        long length = readU32();
        if (length == 0 || length > remaining() + 1L) {
            throw new WireException.ImpossibleLength(where, length, remaining());
        }
        int from = take(length - 1);
        String text = decodeUtf8(from, (int) (length - 1), where);
        readU8();
        return text;
    }

    /**
     * A {@code wstring}, in the form {@code policy} and {@code order} name.
     *
     * <p>Under {@link WstringLengthPolicy#SPECIFICATION} the count is bytes
     * and not characters — §7.4.1.1.2 — so a count the code-unit width does
     * not divide is a stream no encoder produced and is refused rather than
     * rounded down: rounding would leave the remainder in the stream and put
     * every later member at the wrong offset.
     *
     * <p>The form is never inferred from the stream. The three are not
     * separable: a Fast DDS {@code "ab"} followed by a {@code long} is
     * byte-identical to a specification {@code "a"} followed by the same
     * {@code long}, differing only in padding no clause fixes. Choosing
     * between them would mean letting the length field — the one field a
     * hostile peer controls — decide how much is read.
     */
    public String readWstring(WstringLengthPolicy policy, WstringUnitOrder order) {
        // A refusal already made stops every later read: the generated
        // decoder carries on to the end of the type, exactly as the C
        // column's does, and a read that consumed bytes here would put
        // the trailing-bytes check at the top on a position no writer
        // produced.
        if (stopped()) {
            return "";
        }
        if (policy == null || order == null) {
            throw new IllegalArgumentException("a wstring policy and unit order, not null");
        }
        int where = at;
        long declared = readU32();
        int width = policy.unitWidth();
        long count;
        if (policy.countsCharacters()) {
            count = declared;
        } else {
            if (declared % width != 0) {
                throw new WireException.ImpossibleLength(where, declared, remaining());
            }
            count = declared / width;
        }
        int[] units = readWstringUnits(where, count, width, order);
        if (policy.isTerminated()) {
            // The count includes the NUL, so a count of zero is a frame that
            // does not hold its own terminator.
            if (declared < 1 || units.length == 0) {
                throw new WireException.ImpossibleLength(where, declared, remaining());
            }
            if (units[units.length - 1] != 0) {
                throw new WireException.OutOfRange(
                        where, "a NUL character ending the wstring, as its count includes one");
            }
            units = Arrays.copyOf(units, units.length - 1);
            // A NUL inside the text is the narrow string's problem in wide
            // form: every C peer stops at it. Only where a terminator is
            // expected — a cell without one is entitled to carry U+0000 as a
            // character.
            for (int unit : units) {
                if (unit == 0) {
                    throw new WireException.OutOfRange(
                            where, "no NUL inside a wstring a C peer would stop at");
                }
            }
        }
        if (order == WstringUnitOrder.BYTE_ORDER_MARK) {
            units = stripByteOrderMark(units);
        }
        StringBuilder out = new StringBuilder(units.length);
        int index = 0;
        while (index < units.length) {
            int unit = units[index];
            if (unit >= 0xD800 && unit <= 0xDBFF && index + 1 < units.length) {
                int low = units[index + 1];
                if (low >= 0xDC00 && low <= 0xDFFF) {
                    out.appendCodePoint(0x10000 + ((unit - 0xD800) << 10) + (low - 0xDC00));
                    index += 2;
                    continue;
                }
            }
            if (unit >= 0xD800 && unit <= 0xDFFF) {
                throw new WireException.OutOfRange(
                        where, "a UTF-16 sequence with no lone surrogate");
            }
            out.append((char) unit);
            index++;
        }
        return out.toString();
    }

    /**
     * The inverse of {@code Writer.writeWstringUtf8Octets}.
     *
     * <p>{@code WstringCodeUnits::Utf8}: the declared length is the UTF-8
     * octet count, and the terminator {@code policy} asks for is the last of
     * those octets. A count the stream cannot hold, a missing or an interior
     * NUL, or an ill-formed UTF-8 sequence is refused, never a replacement
     * character or a truncated value.
     */
    public String readWstringUtf8Octets(WstringLengthPolicy policy) {
        if (stopped()) {
            return "";
        }
        if (policy == null) {
            throw new IllegalArgumentException("a wstring length policy, not null");
        }
        int where = at;
        long declared = readU32();
        if (declared > remaining()) {
            throw new WireException.ImpossibleLength(where, declared, remaining());
        }
        int from = take(declared);
        int count = (int) declared;
        if (policy.isTerminated()) {
            if (count == 0 || bytes[from + count - 1] != 0) {
                throw new WireException.OutOfRange(
                        where, "a NUL octet ending the wstring, as its count includes one");
            }
            // A NUL inside the text is the narrow string's problem in wide
            // form: every C peer stops at it.
            for (int index = 0; index < count - 1; index++) {
                if (bytes[from + index] == 0) {
                    throw new WireException.OutOfRange(
                            where, "no NUL inside a wstring a C peer would stop at");
                }
            }
            count -= 1;
        }
        return decodeUtf8(from, count, where);
    }

    /**
     * {@code count} {@code wstring} characters of {@code width} bytes, as
     * UTF-16 units.
     *
     * <p>The four-byte width is RTI's XCDR1 form, where a character is a code
     * unit zero-extended. A value above {@code 0xFFFF} there is not a form
     * this reader knows how to narrow, so it is refused rather than truncated
     * — truncating is the defect this whole policy exists because of.
     */
    private int[] readWstringUnits(int where, long count, int width, WstringUnitOrder order) {
        if (count < 0 || count * width > remaining()) {
            throw new WireException.ImpossibleLength(where, count * width, remaining());
        }
        int[] units = new int[(int) count];
        boolean little = order.isFixedLittle();
        for (int index = 0; index < units.length; index++) {
            long unit;
            if (width == 4) {
                if (order == WstringUnitOrder.BIG_ENDIAN_ALWAYS) {
                    unit = readU32Big();
                } else if (little) {
                    unit = readU32Little();
                } else {
                    unit = readU32();
                }
                if (unit > 0xFFFF) {
                    throw new WireException.OutOfRange(
                            where, "a four-byte wstring character holding one UTF-16 unit");
                }
            } else if (order == WstringUnitOrder.BIG_ENDIAN_ALWAYS) {
                unit = readU16Big();
            } else if (little) {
                unit = readU16Little();
            } else {
                unit = readU16();
            }
            units[index] = (int) unit;
        }
        return units;
    }

    /**
     * Take the byte-order mark off the front and apply what it said.
     *
     * <p>The units arrived little-endian, because that is what omniORB writes
     * and what the {@code byte-order-mark} order therefore reads. Three cases,
     * and the third is the one that matters:
     *
     * <ul>
     *   <li>{@code FEFF} — the mark is the right way up, the units are as
     *       read.</li>
     *   <li>{@code FFFE} — the mark is byte-swapped, so every unit behind it
     *       is too, and each one is swapped back. This is what a conforming
     *       big-endian omniORB peer produces and what a reader that ignored
     *       the mark would return as different characters, well formed and the
     *       right length.</li>
     *   <li>no mark — CORBA's rule for a GIOP 1.2 {@code wstring} without one
     *       is UTF-16 big-endian, so every unit read little-endian is
     *       swapped.</li>
     * </ul>
     *
     * <p>The mark is never a character of the returned string in any of the
     * three, which is the half §7.4.1.1.2 cares about.
     */
    private static int[] stripByteOrderMark(int[] units) {
        if (units.length == 0) {
            return units;
        }
        if (units[0] == Writer.BYTE_ORDER_MARK) {
            return Arrays.copyOfRange(units, 1, units.length);
        }
        int from = units[0] == Writer.SWAPPED_BYTE_ORDER_MARK ? 1 : 0;
        int[] out = new int[units.length - from];
        for (int index = 0; index < out.length; index++) {
            int unit = units[from + index];
            out[index] = ((unit & 0xFF) << 8) | (unit >> 8);
        }
        return out;
    }

    /**
     * {@code fixed<digits, scale>}, as the decimal string the writer took.
     *
     * <p>The mirror of {@link Writer#writeFixed}:
     * {@code ceil((digits + 1) / 2)} bytes of BCD, digits most-significant
     * first, sign nibble last.
     *
     * <p>The leading zeros go, because the BCD field is {@code digits} wide
     * whatever the value: the nibbles for {@code 0.01} in a {@code fixed<5,2>}
     * spell {@code 00001}, and returning {@code "000.01"} would put a decimal
     * on the read side that the write side would never produce from the same
     * value. A negative zero loses its sign for the same reason: {@code -0.00}
     * and {@code 0.00} are one value, and the sign nibble is the only place
     * they differ.
     *
     * <p>Under {@link FixedDigitCount#VALUE_PRECISION} the field's end is read
     * out of the octets: [CDR] §9.3.2.8 makes a digit {@code 0} to {@code 9}
     * and a sign {@code 0xa} upwards, and the nibble count is even by
     * construction, so the field ends at the first octet whose <b>low</b>
     * nibble is a sign. A field that never presents one within the sixteen
     * octets of the widest {@code fixed} IDL admits is refused.
     */
    public String readFixed(int digits, int scale, FixedDigitCount count) {
        // A refusal already made stops every later read: the generated
        // decoder carries on to the end of the type, exactly as the C
        // column's does, and a read that consumed bytes here would put
        // the trailing-bytes check at the top on a position no writer
        // produced.
        if (stopped()) {
            return "";
        }
        if (count == null) {
            throw new WireException.OutOfRange(at, "a fixed digit count, not null");
        }
        // The same bound as the writer's, on this side too: OMG IDL 4.2 rule
        // (42) caps the BCD rule at 31 digits, and a reader asked for a wider
        // field would take octets no declaration can name.
        if (digits <= 0 || digits > 31 || scale > digits) {
            throw new WireException.OutOfRange(at,
                    "a fixed<P, S> with S <= P and 1 <= P <= 31, not fixed<"
                            + digits + ", " + scale + ">");
        }
        int[] nibbles;
        int used;
        int sign;
        if (count == FixedDigitCount.VALUE_PRECISION
                || count == FixedDigitCount.VALUE_STRING_DIGITS) {
            int[] buffer = new int[32];
            used = 0;
            while (true) {
                if (used >= 32) {
                    throw new WireException.OutOfRange(at, "a sign nibble within sixteen octets");
                }
                int octet = readU8();
                buffer[used++] = octet >> 4;
                buffer[used++] = octet & 0x0F;
                if ((octet & 0x0F) >= FixedDigitCount.FIRST_SIGN_NIBBLE) {
                    break;
                }
            }
            sign = buffer[--used];
            nibbles = Arrays.copyOf(buffer, used);
        } else {
            int octets = (digits + 2) / 2;
            int from = take(octets);
            int[] buffer = new int[octets * 2];
            for (int index = 0; index < octets; index++) {
                buffer[index * 2] = (bytes[from + index] & 0xFF) >> 4;
                buffer[index * 2 + 1] = bytes[from + index] & 0x0F;
            }
            sign = buffer[buffer.length - 1];
            int start = (digits + 1) % 2 == 1 ? 1 : 0;
            nibbles = Arrays.copyOfRange(buffer, start, buffer.length - 1);
        }
        StringBuilder text = new StringBuilder(nibbles.length);
        for (int nibble : nibbles) {
            if (nibble > 9) {
                throw new WireException.OutOfRange(at, "BCD digits");
            }
            text.append((char) ('0' + nibble));
        }
        // The point sits `scale` digits from the right and not
        // `digits - scale` from the left: under the value's count the field
        // carries fewer than `digits`, and one that carries fewer than the
        // scale — `1c` under `fixed<5,2>` — needs its integer part written as
        // a zero.
        while (text.length() < scale + 1) {
            text.insert(0, '0');
        }
        int split = text.length() - scale;
        String whole = trimLeadingZeros(text.substring(0, split));
        String fraction = text.substring(split);
        String out = scale == 0 ? whole : whole + "." + fraction;
        boolean zero = "0".equals(whole) && isAllZeros(fraction);
        return sign == 0x0D && !zero ? "-" + out : out;
    }

    private static String trimLeadingZeros(String text) {
        int first = 0;
        while (first < text.length() && text.charAt(first) == '0') {
            first++;
        }
        return first == text.length() ? "0" : text.substring(first);
    }

    private static boolean isAllZeros(String text) {
        for (int index = 0; index < text.length(); index++) {
            if (text.charAt(index) != '0') {
                return false;
            }
        }
        return true;
    }

    // -- the bit-bounded kinds --------------------------------------------

    /**
     * An enumerator's holder, sign-extended from its bound.
     *
     * <p>Signed, because XTypes 1.3 §7.4.3.5.3 rule (5) makes the holder
     * signed: {@code @value(200)} under a bound of eight goes out as
     * {@code c8}, and that byte read as the type says is {@code -56}.
     * {@code @value(-1)} and {@code @value(255)} are the same byte and
     * nothing in the stream separates them — which one was meant is a
     * question about the <i>type</i>, and the generated decoder answers it
     * by comparing this against each declared enumerator's own pattern,
     * taken the same way.
     *
     * <p>It was unsigned here and signed in {@code runtime/c}, and the two
     * columns' decoders are generated from one walk that compares against
     * the signed pattern. The Java column discarded
     * {@code tests/interop-corpus/enum-bitbound-8-value} and
     * {@code enum-negative-value} outright: no arm of the chain matched, so
     * the sample was a value the type does not admit. Measured, not
     * inferred.
     */
    public long readEnum(int bitBound) {
        // A refusal already made stops every later read: the generated
        // decoder carries on to the end of the type, exactly as the C
        // column's does, and a read that consumed bytes here would put
        // the trailing-bytes check at the top on a position no writer
        // produced.
        if (stopped()) {
            return 0L;
        }
        if (bitBound <= 0 || bitBound > 32) {
            throw new WireException.OutOfRange(
                    at, "an @bit_bound between 1 and 32, not " + bitBound);
        }
        // The holder's width is the bound rounded up to 8, 16 or 32, and
        // the sign bit is the top one of *that* width and not of the
        // bound: `@bit_bound(5)` still occupies an octet, and rule (5)
        // makes the octet signed.
        int width = bitBound <= 8 ? 8 : bitBound <= 16 ? 16 : 32;
        long pattern = width == 8 ? readU8() : width == 16 ? readU16() : readU32();
        long sign = 1L << (width - 1);
        return (pattern & sign) == 0 ? pattern : pattern - (1L << width);
    }

    /** A bitmask's holder. Table 33 of XTypes 1.3 §7.4.1.1.3.2. */
    public long readBitmask(int bitBound) {
        // A refusal already made stops every later read: the generated
        // decoder carries on to the end of the type, exactly as the C
        // column's does, and a read that consumed bytes here would put
        // the trailing-bytes check at the top on a position no writer
        // produced.
        if (stopped()) {
            return 0L;
        }
        if (bitBound <= 0 || bitBound > 64) {
            throw new WireException.OutOfRange(
                    at, "an @bit_bound between 1 and 64, not " + bitBound);
        }
        if (bitBound <= 8) {
            return readU8();
        }
        if (bitBound <= 16) {
            return readU16();
        }
        if (bitBound <= 32) {
            return readU32();
        }
        return readU64();
    }

    /** A bitset's anchor integer, of the width the planner sized. */
    public long readBitset(int anchorBytes) {
        // A refusal already made stops every later read: the generated
        // decoder carries on to the end of the type, exactly as the C
        // column's does, and a read that consumed bytes here would put
        // the trailing-bytes check at the top on a position no writer
        // produced.
        if (stopped()) {
            return 0L;
        }
        switch (anchorBytes) {
            case 1:
                return readU8();
            case 2:
                return readU16();
            case 4:
                return readU32();
            case 8:
                return readU64();
            default:
                throw new WireException.OutOfRange(
                        at, "a bitset anchor of 1, 2, 4 or 8 bytes, not " + anchorBytes);
        }
    }

    // -- framing ----------------------------------------------------------

    /**
     * A sequence's element count, refusing one the stream cannot hold.
     *
     * <p>A count is one element or more, so a stream with fewer bytes left
     * than elements claimed cannot be honest about it. Allocating for it is
     * how a reader turns a bad message into an out-of-memory.
     */
    public int readSequenceLength() {
        // A refusal already made stops every later read: the generated
        // decoder carries on to the end of the type, exactly as the C
        // column's does, and a read that consumed bytes here would put
        // the trailing-bytes check at the top on a position no writer
        // produced.
        if (stopped()) {
            return 0;
        }
        int where = at;
        long count = readU32();
        if (count > remaining()) {
            throw new WireException.ImpossibleLength(where, count, remaining());
        }
        return (int) count;
    }

    /** A DHEADER: the length the members that follow occupy. */
    public int readDheader() {
        // A refusal already made stops every later read: the generated
        // decoder carries on to the end of the type, exactly as the C
        // column's does, and a read that consumed bytes here would put
        // the trailing-bytes check at the top on a position no writer
        // produced.
        if (stopped()) {
            return 0;
        }
        int where = at;
        long length = readU32();
        if (length > remaining()) {
            throw new WireException.ImpossibleLength(where, length, remaining());
        }
        return (int) length;
    }

    /** An EMHEADER as read. */
    public static final class EmHeaderRead {
        private final int memberId;
        private final int lengthCode;
        private final boolean mustUnderstand;
        private final long nextInt;
        private final boolean hasNextInt;

        EmHeaderRead(
                int memberId,
                int lengthCode,
                boolean mustUnderstand,
                long nextInt,
                boolean hasNextInt) {
            this.memberId = memberId;
            this.lengthCode = lengthCode;
            this.mustUnderstand = mustUnderstand;
            this.nextInt = nextInt;
            this.hasNextInt = hasNextInt;
        }

        /** The 28-bit member id. */
        public int memberId() {
            return memberId;
        }

        /** The length code, 0 to 7. */
        public int lengthCode() {
            return lengthCode;
        }

        /** Whether the writer set the member's must-understand flag. */
        public boolean mustUnderstand() {
            return mustUnderstand;
        }

        /** The NEXTINT, for the codes that carry one. */
        public long nextInt() {
            return nextInt;
        }

        /** Whether there was a NEXTINT at all. */
        public boolean hasNextInt() {
            return hasNextInt;
        }

        /**
         * How many bytes the member occupies after the header word, NEXTINT
         * included where there is one.
         *
         * <p>What a reader skipping an unknown member advances by, and what a
         * reader of a known one checks itself against. XTypes 1.3 §7.4.3.4.2:
         * codes 0 to 3 state a size of one, two, four or eight bytes outright;
         * code 4 is followed by a NEXTINT holding the member's length; and
         * codes 5, 6 and 7 have a NEXTINT that is <i>also</i> the member's own
         * leading word, so the member spans that word plus the length it
         * states, times one, four or eight.
         *
         * <p>{@code long} and not {@code int}: a NEXTINT is a {@code uint32}
         * and a Java {@code int} does not hold one, so narrowing here would
         * turn a length past 2^31 into a negative offset without a word. The
         * generated decoder brings it into the offset domain at the use site
         * — {@code zd_ends = r.readerPosition() + (int) zd_h.extent()} — the
         * same entrance every other column uses, and that cast is then a real
         * one rather than a redundant cast {@code -Xlint:cast} reports.
         */
        public long extent() {
            if (lengthCode <= 3) {
                return 1L << lengthCode;
            }
            if (lengthCode == 4) {
                return nextInt;
            }
            long scale = lengthCode == 5 ? 1L : lengthCode == 6 ? 4L : 8L;
            return 4L + nextInt * scale;
        }
    }

    /**
     * An EMHEADER, unpacked into its parts and the NEXTINT it carries.
     *
     * <p>Codes 5 to 7 report the NEXTINT they read, but the reader is put back
     * before it: that word is the member's own leading length and the member's
     * decoder has to see it. XTypes 1.3 §7.4.3.4.2.
     */
    public EmHeaderRead readEmheader() {
        // A refusal already made stops every later read: the generated
        // decoder carries on to the end of the type, exactly as the C
        // column's does, and a read that consumed bytes here would put
        // the trailing-bytes check at the top on a position no writer
        // produced.
        if (stopped()) {
            return new EmHeaderRead(0, 0, false, 0L, false);
        }
        long word = readU32();
        int lengthCode = (int) ((word >>> 28) & 0x7);
        int memberId = (int) (word & 0x0FFFFFFFL);
        boolean mustUnderstand = (word >>> 31) != 0;
        if (lengthCode == 4) {
            return new EmHeaderRead(memberId, lengthCode, mustUnderstand, readU32(), true);
        }
        if (lengthCode >= 5) {
            int where = at;
            long peeked = readU32();
            at = where;
            return new EmHeaderRead(memberId, lengthCode, mustUnderstand, peeked, true);
        }
        return new EmHeaderRead(memberId, lengthCode, mustUnderstand, 0, false);
    }

    /** A parameter header as read. */
    public static final class PidRead {
        private final int pid;
        private final long length;
        private final boolean end;
        private final boolean mustUnderstand;
        private final boolean implExtension;
        private final boolean reserved;
        private final boolean ignore;
        private final ParameterRole role;
        private final int savedOrigin;

        /**
         * Where this parameter's value begins.
         *
         * Not derivable from {@code savedOrigin}, which is the
         * *enclosing* origin this parameter replaced: the two coincide
         * only for the first parameter of a list, whose header starts
         * exactly at the enclosing origin. `finishPidRead` restored the
         * enclosing origin and then measured the value from it, so
         * every parameter after the first measured from the list's
         * start and got a length several members too large.
         *
         * Measured on `struct-mutable` under PL_CDR1: the second
         * member's eight octets came out as sixteen against a declared
         * eight, and the sample was refused with OUT_OF_RANGE at 20.
         */
        private final int valueAt;

        PidRead(
                int pid,
                long length,
                boolean end,
                boolean mustUnderstand,
                boolean implExtension,
                boolean reserved,
                boolean ignore,
                ParameterRole role,
                int savedOrigin,
                int valueAt) {
            this.savedOrigin = savedOrigin;
            this.valueAt = valueAt;
            this.pid = pid;
            this.length = length;
            this.end = end;
            this.mustUnderstand = mustUnderstand;
            this.implExtension = implExtension;
            this.reserved = reserved;
            this.ignore = ignore;
            this.role = role;
        }

        /** Where the value begins, for the declared-length check. */
        int valueAt() {
            return valueAt;
        }

        /** The member id, with the flag bits taken off. */
        public int pid() {
            return pid;
        }

        /** The declared length. */
        public long length() {
            return length;
        }

        /** Whether this is the {@code PID_LIST_END} sentinel. */
        public boolean isEnd() {
            return end;
        }

        /** Whether the writer set the member's must-understand flag. */
        public boolean mustUnderstand() {
            return mustUnderstand;
        }

        /**
         * Whether the writer set {@code FLAG_IMPL_EXTENSION}.
         *
         * <p>§7.4.1.2.2: the parameter is the writing implementation's own,
         * and a user-defined member "will never set the FLAG_IMPL_EXTENSION
         * bit". Masking it off with the rest made a vendor-private parameter
         * indistinguishable from the member with the same low fourteen bits.
         */
        public boolean implExtension() {
            return implExtension;
        }

        /**
         * Whether the short-form id is inside §7.4.1.2.2's reserved range
         * {@code 0x3F01}-{@code 0x3FFF}, which no member id occupies.
         */
        public boolean reserved() {
            return reserved;
        }

        /**
         * {@code PID_IGNORE}, which §7.4.1.2.2 says every consumer shall
         * ignore — with no exception for the must-understand bit.
         */
        public boolean ignore() {
            return ignore;
        }

        /** Which of §7.4.1.2's two roles the caller read it in. */
        public ParameterRole role() {
            return role;
        }

        /**
         * Whether this parameter can be a member of the reading type.
         *
         * <p>False for a vendor-private parameter and for a reserved id. Both
         * are legal for a peer to send and neither is a member, so both take
         * the unknown-parameter path — skipped, or the sample discarded when
         * the writer set must-understand. §7.4.1.2.2, §7.2.2.4.4.4.6.
         */
        public boolean isUserMember() {
            return !implExtension && !reserved && !ignore;
        }
    }

    /** A PL_CDR1 parameter header in the {@link ParameterRole#MEMBER} role. */
    public PidRead readPid() {
        return readPid(ParameterRole.MEMBER);
    }

    /**
     * A PL_CDR1 parameter header, in whichever form it was written.
     *
     * <p>{@code role} is the caller's, not the stream's: the bytes are the
     * same bytes and only what the declared length means differs. §7.4.1.2.
     */
    public PidRead readPid(ParameterRole role) {
        // A refusal already made stops every later read: the generated
        // decoder carries on to the end of the type, exactly as the C
        // column's does, and a read that consumed bytes here would put
        // the trailing-bytes check at the top on a position no writer
        // produced.
        if (stopped()) {
            return new PidRead(0, 0L, true, false, false, false, false, role, origin, at);
        }
        alignTo(4);
        int pid = readU16();
        int length = readU16();
        if ((pid & PID_ID_MASK) == (Writer.PID_EXTENDED & PID_ID_MASK)) {
            // The four flag bits are not part of the id: §7.4.1.2.1 splits the
            // `eMemberHeader` into `FLAG_1` to `FLAG_4` and a 28-bit member
            // id, and `FLAG_2` is the member's must-understand. Taking the
            // whole word gave a Cyclone member declared `@must_understand` an
            // id of `0x40000001`, which matches nothing, and the member was
            // dropped.
            long memberHeader = readU32();
            long wideLength = readU32();
            // The parameter's value is its own stream block: §7.4.1.2 counts
            // a member's internal alignment from the first octet after the
            // header, not from the start of the message. `finishPidRead` puts
            // the enclosing origin back.
            int saved = origin;
            origin = at;
            return new PidRead(
                    (int) (memberHeader & Writer.E_MEMBER_ID_MASK),
                    wideLength,
                    false,
                    (memberHeader & (Writer.E_FLAG_MUST_UNDERSTAND & 0xFFFFFFFFL)) != 0,
                    // FLAG_1 is FLAG_IMPL_EXTENSION here exactly as bit 15 is
                    // in the short header. The extended id is 28 bits and has
                    // no reserved range.
                    (memberHeader & (Writer.E_FLAG_IMPL_EXTENSION & 0xFFFFFFFFL)) != 0,
                    false,
                    false,
                    role,
                    saved,
                    origin);
        }
        int identifier = pid & PID_ID_MASK;
        boolean end = identifier == (Writer.PID_LIST_END & PID_ID_MASK);
        // Under a framing that puts `FLAG_IMPL_EXTENSION` on every member id
        // the bit no longer tells a vendor-private parameter from a user
        // member — OpenDDS writes `c0010004` for `@id(1) long a`. It is
        // reported absent there because under that framing it carries no
        // information, and reporting it would send every member down the
        // unknown-parameter path.
        boolean flagged = (pid & Writer.PID_IMPL_EXTENSION) != 0
                && (pidFlags.flagsAt(PidFlagPolicy.Position.MEMBER) & Writer.PID_IMPL_EXTENSION)
                        == 0;
        int saved = origin;
        origin = at;
        return new PidRead(
                identifier,
                length,
                end,
                (pid & Writer.PID_MUST_UNDERSTAND) != 0,
                flagged,
                !end && identifier > highestShortId,
                identifier == Writer.PID_IGNORE,
                role,
                saved,
                origin);
    }

    /**
     * The same header in a named role, which is what the declared-length
     * check is made against.
     *
     * <p>{@link #readPid()} is {@link ParameterRole#MEMBER}. The name is the
     * one the shared codec walk asks for, so that one operation has one name
     * in both columns.
     */
    public PidRead readPidAs(ParameterRole role) {
        return readPid(role);
    }

    /**
     * Whether this parameter can be one of the reading type's members.
     *
     * <p>False for a vendor-private parameter and for a reserved id; both are
     * legal for a peer to send and neither is a member, so both take the
     * unknown-parameter path. §7.4.1.2.2, §7.2.2.4.4.4.6.
     */
    public static boolean pidIsUserMember(PidRead header) {
        return header != null && header.isUserMember();
    }

    /**
     * End a parameter's value and step to the next header.
     *
     * <p>The padding to four follows the value and is outside the declared
     * length, so the next parameter is found by alignment and not by adding
     * the length — which is what lets one reader take RTI's rounded-up length
     * and Fast DDS's exact one.
     *
     * <p>The declared length is checked against the configured framing rather
     * than believed: a length belonging to the other convention is a stream
     * this reader was pointed at with the wrong framing, and reading on would
     * put every later member at an offset no writer produced.
     */
    public void finishPidRead(PidRead header) {
        if (header == null) {
            return;
        }
        origin = header.savedOrigin;
        if (stopped() || header.isEnd()) {
            return;
        }
        // From the value's own start and not from the restored origin:
        // the two are the same only for the first parameter of a list.
        long used = (long) at - (long) header.valueAt();
        long declared = header.length();
        long padded = (used + 3L) & ~3L;
        boolean pads = framing.padsDeclaredLength(header.role());
        // Both readings of the same value are accepted where they coincide,
        // which is every value already a multiple of four.
        if (declared != (pads ? padded : used) && declared != used && declared != padded) {
            readerRefuse(Status.OUT_OF_RANGE);
        }
        alignTo(4);
    }

    /** How many octets the header itself took, for the length check. */
    private static int headerOctets(PidRead header) {
        return header.pid() > PID_ID_MASK || header.length() > 0xFFFFL ? 12 : 4;
    }

    /**
     * Step over a member this reader does not know.
     *
     * <p>The whole point of the length code. XTypes 1.3 §7.4.3.4.2: codes 0 to
     * 3 state a size of one, two, four or eight bytes outright; code 4 is
     * followed by a NEXTINT holding the member's length; and codes 5, 6 and 7
     * have a NEXTINT that is <i>also</i> the member's own leading word, so the
     * member spans that word plus the length it states, times one, four or
     * eight.
     */
    public void skipMember(EmHeaderRead header) {
        // A refusal already made stops every later read: the generated
        // decoder carries on to the end of the type, exactly as the C
        // column's does, and a read that consumed bytes here would put
        // the trailing-bytes check at the top on a position no writer
        // produced.
        if (stopped()) {
            return;
        }
        if (header.lengthCode() <= 3) {
            skip(1L << header.lengthCode());
            return;
        }
        if (!header.hasNextInt()) {
            throw new WireException.OutOfRange(at, "a NEXTINT for a length code of 4 or above");
        }
        if (header.lengthCode() == 4) {
            skip(header.nextInt());
            return;
        }
        long scale = header.lengthCode() == 5 ? 1 : header.lengthCode() == 6 ? 4 : 8;
        skip(4 + header.nextInt() * scale);
    }

    /**
     * Start reading a parameter's value, returning the origin it replaces.
     * The reader's half of the writer's parameter block.
     */
    public int beginParameterValue() {
        int saved = origin;
        origin = at;
        return saved;
    }

    /** End a parameter's value and step over its trailing padding. */
    public void finishParameterValue(int savedOrigin) {
        origin = savedOrigin;
        alignTo(4);
    }

    // -- text -------------------------------------------------------------

    private String decodeUtf8(int from, int length, int where) {
        CharsetDecoder decoder = StandardCharsets.UTF_8
                .newDecoder()
                .onMalformedInput(CodingErrorAction.REPORT)
                .onUnmappableCharacter(CodingErrorAction.REPORT);
        try {
            return decoder.decode(ByteBuffer.wrap(bytes, from, length)).toString();
        } catch (CharacterCodingException error) {
            throw new WireException.NotUtf8(where);
        }
    }
}
