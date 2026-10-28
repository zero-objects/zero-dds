// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Zdw -- ZeroDDS endpoint wire-core, pure Java 8 (ADR 0013).
//
// The `wire-core` unit of the native Java endpoint SDK: the full XCDR
// primitive surface, byte-identical to the Rust core. No JNI, no native
// lib -- runs on any JVM. Byte-by-byte, explicit LE/BE wire order.
//
// The encoding is a constructor argument and has no default. XTypes 1.3
// s7.4.1 puts MAXALIGN at eight under XCDR1 and four under XCDR2, so a
// writer that did not know which one it was in would be writing XCDR2 and
// calling it XCDR -- which is what this SDK did until the cap became this
// argument: `align` read a constant four, and every `long long` in an
// XCDR1 sample landed four octets early with nothing in the interface to
// say so.
//
// Two things are refused rather than corrected, and both were silent
// before: an integer past the wire width it names (`u16(70000)` wrote
// `7011`, the low sixteen bits), and a code point past the form's range
// (`putChar` above U+00FF, `putWchar` above the BMP). Java's `int` and
// `long` are wider than the wire widths they carry here, so the check is
// the only one there is.

import java.nio.charset.Charset;

public final class Zdw {

    public static final int LE = 0;
    public static final int BE = 1;

    /** PLAIN_CDR / PL_CDR -- MAXALIGN 8. */
    public static final int XCDR1 = 1;
    /** PLAIN_CDR2 / DELIMITED_CDR / PL_CDR2 -- MAXALIGN 4. */
    public static final int XCDR2 = 2;

    // The member header length codes of XTypes 1.3 s7.4.3.4.2 Table 34.
    public static final int LC0 = 0;
    public static final int LC1 = 1;
    public static final int LC2 = 2;
    public static final int LC3 = 3;
    public static final int LC4 = 4;
    public static final int LC5 = 5;
    public static final int LC6 = 6;
    public static final int LC7 = 7;

    // PL_CDR1 parameter ids. XTypes 1.3 s7.4.1.2.
    public static final int PID_LIST_END_ID = 0x3F02;
    public static final int PID_EXTENDED_ID = 0x3F01;
    public static final int PID_SHORT_MAX_ID = 0x3F00;

    // Bit 14 of a short parameter id; bit 30 of the eMemberHeader.
    static final int PID_FLAG_MUST_UNDERSTAND = 0x4000;
    static final long EM_FLAG_MUST_UNDERSTAND = 0x40000000L;

    static final Charset UTF8 = Charset.forName("UTF-8");

    private Zdw() {}

    static int maxAlignOf(int encoding) {
        if (encoding == XCDR1) return 8;
        if (encoding == XCDR2) return 4;
        throw new IllegalArgumentException(
            "Writer takes an endian and an encoding (Zdw.XCDR1 or Zdw.XCDR2)");
    }

    /** A block of members serialized under one DHEADER. */
    public interface Body {
        void write(Writer w);
    }

    /** An open PL_CDR1 parameter header, to be handed back to pidEnd. */
    public static final class Pid {
        final int at;
        final boolean extended;
        final int savedOrigin;
        Pid(int at, boolean extended, int savedOrigin) {
            this.at = at;
            this.extended = extended;
            this.savedOrigin = savedOrigin;
        }
    }

    public static final class Writer {
        private byte[] buf = new byte[64];
        private int size = 0;
        private final int endian;
        private final int maxAlign;
        // The position alignment is measured from. Zero for the stream,
        // and moved to the start of a PL_CDR1 parameter's value for the
        // length of that parameter -- s7.4.1.2 aligns a parameter's value
        // against its own block, not against the stream.
        private int origin = 0;

        public Writer(int endian, int encoding) {
            this.endian = endian;
            this.maxAlign = maxAlignOf(encoding);
        }

        public int maxAlign() { return maxAlign; }
        public int size() { return size; }

        private void grow(int n) {
            if (size + n > buf.length) {
                int cap = buf.length * 2;
                while (cap < size + n) cap *= 2;
                byte[] next = new byte[cap];
                System.arraycopy(buf, 0, next, 0, size);
                buf = next;
            }
        }

        private void raw(int v) {
            grow(1);
            buf[size++] = (byte) v;
        }

        private void align(int a) {
            int eff = (a <= maxAlign) ? a : maxAlign;
            while (((size - origin) % eff) != 0) raw(0);
        }

        private void putUint(int a, byte[] le) {
            align(a);
            if (endian == BE) {
                for (int i = le.length - 1; i >= 0; i--) raw(le[i]);
            } else {
                for (int i = 0; i < le.length; i++) raw(le[i]);
            }
        }

        private void patchU32(int at, long v) {
            byte[] le = new byte[] {
                (byte) v, (byte) (v >> 8), (byte) (v >> 16), (byte) (v >> 24) };
            for (int i = 0; i < 4; i++) {
                buf[at + i] = (endian == BE) ? le[3 - i] : le[i];
            }
        }

        private void patchU16(int at, int v) {
            if (endian == BE) {
                buf[at] = (byte) (v >> 8);
                buf[at + 1] = (byte) v;
            } else {
                buf[at] = (byte) v;
                buf[at + 1] = (byte) (v >> 8);
            }
        }

        // A value past the wire width it names is refused, never
        // truncated: u16(70000) writing 0x1170 -- the low sixteen bits --
        // is the defect class every binding's range test exists to stop.
        private static void range(long v, long max, String call) {
            if (v < 0 || v > max) {
                throw new IllegalArgumentException(
                    call + ": " + v + " is not in 0.." + max);
            }
        }

        public void u8(int v) {
            range(v, 0xFF, "u8");
            raw(v);
        }

        public void bool(boolean v) { raw(v ? 1 : 0); }

        public void u16(int v) {
            range(v, 0xFFFF, "u16");
            putUint(2, new byte[] { (byte) v, (byte) (v >> 8) });
        }

        public void u32(long v) {
            range(v, 0xFFFFFFFFL, "u32");
            putUint(4, new byte[] {
                (byte) v, (byte) (v >> 8), (byte) (v >> 16), (byte) (v >> 24) });
        }

        /** The 64-bit pattern: every Java `long` is one, so nothing is
         *  refused here. */
        public void u64(long v) {
            putUint(8, new byte[] {
                (byte) v, (byte) (v >> 8), (byte) (v >> 16), (byte) (v >> 24),
                (byte) (v >> 32), (byte) (v >> 40), (byte) (v >> 48), (byte) (v >> 56) });
        }

        public void f32(float v) { u32(Float.floatToRawIntBits(v) & 0xFFFFFFFFL); }
        public void f64(double v) { u64(Double.doubleToRawLongBits(v)); }

        /** An IDL `char`: one octet. A code point above U+00FF is refused
         *  and not truncated -- U+0161's low octet is 0x61, which is `a`,
         *  a plausible wrong character. */
        public void putChar(int codePoint) {
            if (codePoint < 0 || codePoint > 0xFF) {
                throw new IllegalArgumentException(String.format(
                    "expected a character below U+0100, found U+%04X", codePoint));
            }
            raw(codePoint);
        }

        /** An IDL `wchar`: one two-octet code unit, 2-aligned, in the
         *  stream's byte order. No GIOP length octet, no byte-order mark.
         *  A code point above U+FFFF is refused: a four-octet form would
         *  zero-extend a code *unit*, not a code point. */
        public void putWchar(int codePoint) {
            if (codePoint < 0 || codePoint > 0xFFFF) {
                throw new IllegalArgumentException(String.format(
                    "expected a BMP character, found U+%04X", codePoint));
            }
            u16(codePoint);
        }

        public void str(String s) {
            byte[] utf8 = s.getBytes(UTF8);
            u32(utf8.length + 1);
            for (int i = 0; i < utf8.length; i++) raw(utf8[i]);
            raw(0);
        }

        /** An IDL `wstring` from its UTF-16 code units. The length word
         *  counts octets and not code units, there is no terminator and
         *  no byte-order mark. */
        public void putWstring(int[] units) {
            u32(units.length * 2L);
            for (int i = 0; i < units.length; i++) {
                range(units[i], 0xFFFF, "putWstring");
                u16(units[i]);
            }
        }

        public void seqU8(byte[] data) {
            u32(data.length);
            for (int i = 0; i < data.length; i++) raw(data[i]);
        }

        public void putBytes(byte[] data) {
            for (int i = 0; i < data.length; i++) raw(data[i]);
        }

        /** A `bitset` as its anchor word: the holder the declared bits
         *  pack into, written at the anchor's own width and alignment.
         *  XTypes 1.3 s7.3.1.4. */
        public void putBitset(long bits, int anchor) {
            switch (anchor) {
                case 1: u8((int) (bits & 0xFF)); break;
                case 2: u16((int) (bits & 0xFFFF)); break;
                case 4: u32(bits & 0xFFFFFFFFL); break;
                case 8: u64(bits); break;
                default: throw new IllegalArgumentException(
                    "a bitset anchor is one, two, four or eight octets, not " + anchor);
            }
        }

        /** An IDL `fixed<digits,scale>` as packed BCD. CDR s9.3.2.8:
         *  two nibbles to the octet, most significant digit first, the
         *  last nibble the sign (0xD negative, 0xC otherwise), alignment
         *  one, byte-order independent. */
        public void putFixed(String value, int digits, int scale) {
            putBytes(fixedOctets(value, digits, scale));
        }

        static byte[] fixedOctets(String value, int digits, int scale) {
            if (digits < 1 || digits > 31) {
                throw new IllegalArgumentException(
                    "fixed<" + digits + "," + scale + ">: a digit count is 1..31");
            }
            if (scale < 0 || scale > digits) {
                throw new IllegalArgumentException(
                    "fixed<" + digits + "," + scale + ">: a scale is 0..digits");
            }
            String text = value;
            boolean negative = false;
            if (text.startsWith("+") || text.startsWith("-")) {
                negative = text.charAt(0) == '-';
                text = text.substring(1);
            }
            String integral = text, fraction = "";
            int at = text.indexOf('.');
            if (at >= 0) {
                integral = text.substring(0, at);
                fraction = text.substring(at + 1);
            }
            if (integral.isEmpty() && fraction.isEmpty()) {
                throw new IllegalArgumentException(
                    "fixed<" + digits + "," + scale + ">: `" + value + "` has no digits");
            }
            for (String part : new String[] { integral, fraction }) {
                for (int i = 0; i < part.length(); i++) {
                    if (part.charAt(i) < '0' || part.charAt(i) > '9') {
                        throw new IllegalArgumentException(
                            "fixed<" + digits + "," + scale + ">: `" + value
                            + "` is not a decimal");
                    }
                }
            }
            // Leading zeros carry no value: `000000.001` is one
            // significant integer digit, not six.
            int trimmed = 0;
            while (trimmed < integral.length() && integral.charAt(trimmed) == '0') {
                trimmed++;
            }
            integral = integral.substring(trimmed);
            if (integral.length() > digits - scale) {
                throw new IllegalArgumentException(
                    "fixed<" + digits + "," + scale + ">: `" + value + "` needs "
                    + integral.length() + " integer digits and the type has "
                    + (digits - scale));
            }
            // A trailing zero in the fraction is padding either way.
            while (fraction.length() > scale && fraction.endsWith("0")) {
                fraction = fraction.substring(0, fraction.length() - 1);
            }
            if (fraction.length() > scale) {
                throw new IllegalArgumentException(
                    "fixed<" + digits + "," + scale + ">: `" + value + "` needs "
                    + fraction.length() + " fraction digits and the type has " + scale);
            }
            byte[] nibbles = new byte[digits + 2];
            int n = 0;
            if ((digits + 1) % 2 == 1) nibbles[n++] = 0;
            for (int i = 0; i < digits - scale - integral.length(); i++) nibbles[n++] = 0;
            for (int i = 0; i < integral.length(); i++) {
                nibbles[n++] = (byte) (integral.charAt(i) - '0');
            }
            for (int i = 0; i < fraction.length(); i++) {
                nibbles[n++] = (byte) (fraction.charAt(i) - '0');
            }
            for (int i = 0; i < scale - fraction.length(); i++) nibbles[n++] = 0;
            boolean nonZero = false;
            for (int i = 0; i < n; i++) {
                if (nibbles[i] != 0) nonZero = true;
            }
            // A negative zero goes out positive: there is one zero to write.
            nibbles[n++] = (byte) ((negative && nonZero) ? 0x0D : 0x0C);
            byte[] out = new byte[n / 2];
            for (int i = 0; i + 1 < n; i += 2) {
                out[i / 2] = (byte) ((nibbles[i] << 4) | nibbles[i + 1]);
            }
            return out;
        }

        // --- back-patched framing ---------------------------------------

        /** Reserves a 4-aligned DHEADER slot and returns the position the
         *  body starts at, to hand back to dheaderEnd. XTypes 1.3
         *  s7.4.3.5. */
        public int dheaderBegin() {
            u32(0);
            return size;
        }

        /** Back-patches the DHEADER with the byte length written since
         *  bodyStart. The DHEADER itself is not counted. */
        public void dheaderEnd(int bodyStart) {
            patchU32(bodyStart - 4, size - bodyStart);
        }

        /** DHEADER-delimited block, callback form -- begin/end on the one
         *  stream, so the body's alignment is the stream's under either
         *  encoding (a sub-buffer is only alignment-equivalent when the
         *  cap is four). */
        public void dheader(Body body) {
            int mark = dheaderBegin();
            body.write(this);
            dheaderEnd(mark);
        }

        /** Writes the EMHEADER for one member of a @mutable aggregate and
         *  returns the body-start position. XTypes 1.3 s7.4.3.4.2. A
         *  member id past the 28 bits is refused, not masked: the masked
         *  value is a DIFFERENT member's id. */
        public int emheaderBegin(long memberId, boolean mustUnderstand, int lengthCode) {
            if (memberId < 0 || memberId > 0x0FFFFFFFL) {
                throw new IllegalArgumentException(
                    "member id " + memberId + " does not fit the 28 bits of an EMHEADER");
            }
            if (lengthCode < LC0 || lengthCode > LC7) {
                throw new IllegalArgumentException(
                    "length code " + lengthCode + " is not one of LC0..LC7");
            }
            long header = ((long) lengthCode << 28) | memberId;
            if (mustUnderstand) header |= 0x80000000L;
            u32(header);
            if (lengthCode == LC4) u32(0); // the NEXTINT slot
            return size;
        }

        /** Closes the member emheaderBegin opened. The length code comes
         *  back rather than being remembered, so a caller that paired an
         *  LC4 begin with a differently-coded end gets a refusal instead
         *  of a NEXTINT patched over the member's body. */
        public void emheaderEnd(int bodyStart, int lengthCode) {
            int written = size - bodyStart;
            if (lengthCode == LC4) {
                patchU32(bodyStart - 4, written);
            } else if (lengthCode <= LC3) {
                int want = 1 << lengthCode;
                if (written != want) {
                    throw new IllegalStateException(
                        "LC" + lengthCode + " frames " + want
                        + " octets and the member wrote " + written);
                }
            } else if (written < 4) {
                throw new IllegalStateException(
                    "LC" + lengthCode + " takes its length from the body's first "
                    + "four octets and the member wrote " + written);
            }
        }

        /** @mutable member, callback form (LC4). */
        public void emheader(int memberId, boolean mustUnderstand, Body body) {
            int mark = emheaderBegin(memberId, mustUnderstand, LC4);
            body.write(this);
            emheaderEnd(mark, LC4);
        }

        // --- the PL_CDR1 parameter list (XTypes 1.3 s7.4.1.2) ------------

        /** Writes a short parameter header: u16 id, u16 byte length, both
         *  in the stream's order. The must-understand flag is bit 14. The
         *  value aligns against its own block, so the origin moves for
         *  the length of the parameter and pidEnd restores it. */
        public Pid pidBegin(int pid, boolean mustUnderstand) {
            range(pid, 0xFFFF, "pidBegin");
            if (pid > PID_SHORT_MAX_ID) {
                throw new IllegalArgumentException(
                    "member id " + pid + " needs the extended parameter header");
            }
            align(4);
            int header = pid;
            if (mustUnderstand) header |= PID_FLAG_MUST_UNDERSTAND;
            putUint(2, new byte[] { (byte) header, (byte) (header >> 8) });
            int at = size;
            raw(0);
            raw(0);
            int saved = origin;
            origin = size;
            return new Pid(at, false, saved);
        }

        /** Writes the twelve-octet parameter header for a member id above
         *  0x3F00 or a value longer than 65535 octets. The flag on the
         *  marker is the framing's, whatever the member says; the
         *  member's own flag is bit 30 of the eMemberHeader. */
        public Pid pidExtendedBegin(long memberId, boolean mustUnderstand) {
            if (memberId < 0 || memberId > 0x0FFFFFFFL) {
                throw new IllegalArgumentException(
                    "member id " + memberId + " does not fit the 28 bits of an eMemberHeader");
            }
            align(4);
            int marker = PID_EXTENDED_ID | PID_FLAG_MUST_UNDERSTAND;
            putUint(2, new byte[] { (byte) marker, (byte) (marker >> 8) });
            putUint(2, new byte[] { 8, 0 });
            long header = memberId;
            if (mustUnderstand) header |= EM_FLAG_MUST_UNDERSTAND;
            putUint(4, new byte[] {
                (byte) header, (byte) (header >> 8),
                (byte) (header >> 16), (byte) (header >> 24) });
            int at = size;
            for (int i = 0; i < 4; i++) raw(0);
            int saved = origin;
            origin = size;
            return new Pid(at, true, saved);
        }

        /** Closes the parameter. The declared length counts the value's
         *  octets AND the padding that brings the next header back to a
         *  four-octet boundary: a reader skips `length` from the end of
         *  the header and has to land on the next parameter. */
        public void pidEnd(Pid header) {
            int width = header.extended ? 4 : 2;
            int length = size - header.at - width;
            int past = (size - header.savedOrigin) % 4;
            if (past != 0) length += 4 - past;
            if (header.extended) {
                patchU32(header.at, length);
            } else {
                if (length > 0xFFFF) {
                    throw new IllegalStateException(
                        "a short parameter header describes at most 65535 octets, not "
                        + length);
                }
                patchU16(header.at, length);
            }
            origin = header.savedOrigin;
            align(4);
        }

        /** The sentinel, with the must-understand flag set -- 0x7F02 on
         *  the wire, which is what Cyclone and RTI write. */
        public void pidListEnd() {
            align(4);
            int id = PID_LIST_END_ID | PID_FLAG_MUST_UNDERSTAND;
            putUint(2, new byte[] { (byte) id, (byte) (id >> 8) });
            putUint(2, new byte[] { 0, 0 });
        }

        public byte[] bytes() {
            byte[] out = new byte[size];
            System.arraycopy(buf, 0, out, 0, size);
            return out;
        }
    }

    public static final class Reader {
        private final byte[] buf;
        private int pos = 0;
        private final int endian;
        private final int maxAlign;
        // Mirrors the writer's: a PL_CDR1 parameter's value aligns
        // against its own block.
        private int origin = 0;

        public Reader(byte[] buf, int endian, int encoding) {
            this.buf = buf;
            this.endian = endian;
            this.maxAlign = maxAlignOf(encoding);
        }

        public int pos() { return pos; }
        public void skip(int n) { pos += n; }

        private void align(int a) {
            int eff = (a <= maxAlign) ? a : maxAlign;
            while (((pos - origin) % eff) != 0) pos++;
        }

        private byte[] getUint(int a, int n) {
            align(a);
            byte[] le = new byte[n];
            for (int i = 0; i < n; i++) le[i] = buf[pos + i];
            pos += n;
            if (endian == BE) {
                for (int i = 0; i < n / 2; i++) {
                    byte t = le[i]; le[i] = le[n - 1 - i]; le[n - 1 - i] = t;
                }
            }
            return le;
        }

        public int u8() { return buf[pos++] & 0xFF; }
        public boolean bool() { return u8() != 0; }

        public int u16() {
            byte[] le = getUint(2, 2);
            return (le[0] & 0xFF) | ((le[1] & 0xFF) << 8);
        }

        public long u32() {
            byte[] le = getUint(4, 4);
            return (le[0] & 0xFFL) | ((le[1] & 0xFFL) << 8)
                 | ((le[2] & 0xFFL) << 16) | ((le[3] & 0xFFL) << 24);
        }

        public long u64() {
            byte[] le = getUint(8, 8);
            long v = 0;
            for (int i = 7; i >= 0; i--) v = (v << 8) | (le[i] & 0xFFL);
            return v;
        }

        public float f32() { return Float.intBitsToFloat((int) u32()); }
        public double f64() { return Double.longBitsToDouble(u64()); }

        public int getChar() { return u8(); }
        public int getWchar() { return u16(); }

        public String str() {
            long n = u32();
            String s = new String(buf, pos, (int) n - 1, UTF8);
            pos += (int) n;
            return s;
        }

        /** The length word counts octets, so the unit count is half. */
        public int[] getWstring() {
            int octets = (int) u32();
            int[] units = new int[octets / 2];
            for (int i = 0; i < units.length; i++) units[i] = u16();
            return units;
        }

        public byte[] seqU8() {
            long n = u32();
            byte[] r = new byte[(int) n];
            for (int i = 0; i < n; i++) r[i] = buf[pos++];
            return r;
        }

        public long getBitset(int anchor) {
            switch (anchor) {
                case 1: return u8();
                case 2: return u16();
                case 4: return u32();
                case 8: return u64();
                default: throw new IllegalArgumentException(
                    "a bitset anchor is one, two, four or eight octets, not " + anchor);
            }
        }

        /** Back into the decimal it was written from: a leading `-` when
         *  the sign nibble is 0xD, `digits - scale` integer digits, and a
         *  `.` with `scale` fraction digits where the scale is not zero. */
        public String getFixed(int digits, int scale) {
            int octets = (digits + 2) / 2;
            byte[] nibbles = new byte[octets * 2];
            for (int i = 0; i < octets; i++) {
                int b = u8();
                nibbles[i * 2] = (byte) (b >> 4);
                nibbles[i * 2 + 1] = (byte) (b & 0x0F);
            }
            int sign = nibbles[nibbles.length - 1];
            StringBuilder out = new StringBuilder();
            if (sign == 0x0D) out.append('-');
            int start = nibbles.length - 1 - digits;
            for (int i = 0; i < digits; i++) {
                if (scale != 0 && i == digits - scale) out.append('.');
                out.append((char) ('0' + nibbles[start + i]));
            }
            return out.toString();
        }

        public long dheaderRead() { return u32(); }

        /** Reads a member header in any length code. Returns {memberId,
         *  mustUnderstand(0/1), lengthCode, extent}, the extent measured
         *  from the position this call leaves the cursor at. LC0..LC3
         *  take it from the code; LC4 consumes the NEXTINT; LC5..LC7 peek
         *  the body's own leading word without consuming it. */
        public long[] emheaderReadLc() {
            long em = u32();
            long memberId = em & 0x0FFFFFFFL;
            long mu = ((em & 0x80000000L) != 0) ? 1 : 0;
            int lc = (int) ((em >> 28) & 0x7);
            long extent;
            if (lc <= LC3) {
                extent = 1L << lc;
            } else if (lc == LC4) {
                extent = u32();
            } else {
                int at = pos;
                long word = u32();
                pos = at;
                if (lc == LC5) extent = 4 + word;
                else if (lc == LC6) extent = 4 + 4 * word;
                else extent = 4 + 8 * word;
            }
            return new long[] { memberId, mu, lc, extent };
        }

        /** Reads an LC4 member header: returns {memberId,
         *  mustUnderstand(0/1), nextint = body byte length}. */
        public long[] emheaderRead() {
            long em = u32();
            long ni = u32();
            return new long[] { em & 0x0FFFFFFF, ((em & 0x80000000L) != 0) ? 1 : 0, ni };
        }

        /** Reads a PL_CDR1 parameter header in either form. Returns
         *  {memberId, mustUnderstand(0/1), length, listEnd(0/1)}. The
         *  extended form is recognised by the marker PID_EXTENDED, and
         *  the must-understand answer then comes from bit 30 of the
         *  eMemberHeader and never from the marker. */
        public long[] pidRead() {
            int raw = u16();
            int shortId = raw & 0x3FFF;
            if (shortId == PID_LIST_END_ID) {
                u16();
                return new long[] { 0, 0, 0, 1 };
            }
            if (shortId == PID_EXTENDED_ID) {
                u16(); // the slength, always 8
                long header = u32();
                long length = u32();
                origin = pos;
                return new long[] { header & 0x0FFFFFFFL,
                    ((header & EM_FLAG_MUST_UNDERSTAND) != 0) ? 1 : 0, length, 0 };
            }
            long length = u16();
            origin = pos;
            return new long[] { shortId,
                ((raw & PID_FLAG_MUST_UNDERSTAND) != 0) ? 1 : 0, length, 0 };
        }

        /** Past the value and the padding, back on the stream's
         *  alignment. */
        public void pidSkip(int bodyStart, long length) {
            pos = bodyStart + (int) length;
            origin = 0;
            while ((pos % 4) != 0) pos++;
        }
    }
}
