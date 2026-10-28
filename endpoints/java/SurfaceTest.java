// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The wire-core operations this SDK grew so that a caller can write every
// construct XTypes 1.3 defines: the 64-bit float, the XCDR1 alignment cap,
// the delimiter, the member header in all eight length codes, the PL_CDR1
// parameter list, the wide character and string, the packed decimal and
// the bitset anchor.
//
// The expected octets are not this file's opinion. Every multi-member case
// is the byte string tests/interop-results/<case>/zerodds-rust/
// <encoding>.<endian>.bin already holds, named in the test, so a change
// here that agreed with itself and not with the reference column fails.
//
// Each operation is exercised at the ends of its range and not only in the
// middle, because that is where the last four defects in these bindings
// were.

public final class SurfaceTest {

    static int failures = 0;

    static String hex(Zdw.Writer w) {
        byte[] b = w.bytes();
        StringBuilder out = new StringBuilder(b.length * 2);
        for (byte x : b) out.append(String.format("%02x", x));
        return out.toString();
    }

    static void check(String name, String got, String want) {
        if (got.equals(want)) {
            System.out.println("ok " + name);
        } else {
            failures++;
            System.out.println("FAIL " + name + ": got " + got + " want " + want);
        }
    }

    static void checkTrue(String name, boolean value) {
        if (value) {
            System.out.println("ok " + name);
        } else {
            failures++;
            System.out.println("FAIL " + name);
        }
    }

    interface Refusable {
        void run();
    }

    static boolean refused(Refusable r) {
        try {
            r.run();
            return false;
        } catch (IllegalArgumentException e) {
            return true;
        } catch (IllegalStateException e) {
            return true;
        }
    }

    public static void main(String[] args) {
        alignmentCap();
        f64Patterns();
        dheader();
        memberHeader();
        integerRange();
        parameterList();
        wideForms();
        bitset();
        fixed();
        readingDirection();
        framingReadsBack();
        parameterListReadsBack();

        if (failures != 0) {
            System.err.println(failures + " surface checks FAILED");
            System.exit(1);
        }
        System.out.println("ALL OK");
    }

    static void alignmentCap() {
        // XTypes 1.3 s7.4.1. One octet, then a `long long`: under XCDR1
        // the 64-bit value aligns to 8 and takes seven octets of padding;
        // under XCDR2 the cap is 4 and it takes three.
        Zdw.Writer one = new Zdw.Writer(Zdw.LE, Zdw.XCDR1);
        one.u8(0xAB);
        one.u64(0x0102030405060708L);
        check("xcdr1 aligns eight", hex(one), "ab000000000000000807060504030201");
        Zdw.Writer two = new Zdw.Writer(Zdw.LE, Zdw.XCDR2);
        two.u8(0xAB);
        two.u64(0x0102030405060708L);
        check("xcdr2 aligns four", hex(two), "ab0000000807060504030201");
        checkTrue("the encoding decides maxAlign",
            new Zdw.Writer(Zdw.BE, Zdw.XCDR1).maxAlign() == 8
            && new Zdw.Writer(Zdw.BE, Zdw.XCDR2).maxAlign() == 4);
    }

    static void f64Patterns() {
        // Pi as tests/interop-corpus/struct-mutable carries it, and the
        // two ends of the exponent range either side of it. IEEE 754
        // gives negative zero its own bit pattern.
        double[] values = { 3.141592653589793, 0.0, -0.0,
                            Double.MAX_VALUE, Double.MIN_VALUE };
        String[] want = { "182d4454fb210940", "0000000000000000",
                          "0000000000000080", "ffffffffffffef7f",
                          "0100000000000000" };
        for (int i = 0; i < values.length; i++) {
            Zdw.Writer w = new Zdw.Writer(Zdw.LE, Zdw.XCDR2);
            w.f64(values[i]);
            check("f64 " + values[i], hex(w), want[i]);
        }
    }

    static void dheader() {
        // XTypes 1.3 s7.4.3.5. Three octets of content, so the DHEADER
        // reads 3 and the four octets it occupies are not in it.
        Zdw.Writer w = new Zdw.Writer(Zdw.LE, Zdw.XCDR2);
        int mark = w.dheaderBegin();
        w.u8(1);
        w.u8(2);
        w.u8(3);
        w.dheaderEnd(mark);
        check("dheader counts the body and not itself", hex(w), "03000000010203");
        Zdw.Writer e = new Zdw.Writer(Zdw.BE, Zdw.XCDR2);
        e.dheaderEnd(e.dheaderBegin());
        check("an empty dheader body is zero", hex(e), "00000000");
    }

    static void memberHeader() {
        // tests/interop-results/struct-mutable/zerodds-rust/plain-cdr2.le.bin
        // -- @mutable struct Sample { long a; double b; short c; } with
        // a = -1234567, b = pi, c = -2. LC2, LC3 and LC1: the length is in
        // the code and no NEXTINT is written for any of them.
        Zdw.Writer w = new Zdw.Writer(Zdw.LE, Zdw.XCDR2);
        int d = w.dheaderBegin();
        int m = w.emheaderBegin(0, false, Zdw.LC2);
        w.u32(0xFFED2979L);
        w.emheaderEnd(m, Zdw.LC2);
        m = w.emheaderBegin(1, false, Zdw.LC3);
        w.f64(3.141592653589793);
        w.emheaderEnd(m, Zdw.LC3);
        m = w.emheaderBegin(2, false, Zdw.LC1);
        w.u16(0xFFFE);
        w.emheaderEnd(m, Zdw.LC1);
        w.dheaderEnd(d);
        check("struct-mutable xcdr2 matches the rust column", hex(w),
            "1a000000" + "00000020" + "7929edff"
            + "01000030" + "182d4454fb210940" + "02000010" + "feff");

        // s7.4.3.4.2. LC4 reserves a NEXTINT of its own; LC5 does not.
        Zdw.Writer four = new Zdw.Writer(Zdw.LE, Zdw.XCDR2);
        m = four.emheaderBegin(2, false, Zdw.LC4);
        four.u16(0xFFFF);
        four.u16(0x0002);
        four.emheaderEnd(m, Zdw.LC4);
        check("lc4 writes a nextint", hex(four), "02000040" + "04000000" + "ffff0200");
        Zdw.Writer five = new Zdw.Writer(Zdw.LE, Zdw.XCDR2);
        m = five.emheaderBegin(0, false, Zdw.LC5);
        five.str("abc");
        five.emheaderEnd(m, Zdw.LC5);
        check("lc5 reuses the body's own word", hex(five),
            "00000050" + "04000000" + "61626300");

        // 0x80000000 | (2 << 28) | 0 = 0xA0000000.
        Zdw.Writer mu = new Zdw.Writer(Zdw.LE, Zdw.XCDR2);
        m = mu.emheaderBegin(0, true, Zdw.LC2);
        mu.u32(0);
        mu.emheaderEnd(m, Zdw.LC2);
        check("must-understand is bit thirty-one", hex(mu), "000000a0" + "00000000");

        // The top of the range: 0x0FFFFFFF fits and 0x10000000 does not.
        new Zdw.Writer(Zdw.LE, Zdw.XCDR2).emheaderBegin(0x0FFFFFFFL, false, Zdw.LC2);
        checkTrue("a 29-bit member id is refused", refused(new Refusable() {
            public void run() {
                new Zdw.Writer(Zdw.LE, Zdw.XCDR2).emheaderBegin(0x10000000L, false, Zdw.LC2);
            }
        }));
        checkTrue("a length code that disagrees with the body is refused",
            refused(new Refusable() {
                public void run() {
                    Zdw.Writer lw = new Zdw.Writer(Zdw.LE, Zdw.XCDR2);
                    int lm = lw.emheaderBegin(0, false, Zdw.LC2);
                    lw.u16(1);
                    lw.emheaderEnd(lm, Zdw.LC2);
                }
            }));
    }

    static void integerRange() {
        // The defect class this stops: u16(70000) wrote 0x1170, the low
        // sixteen bits, and the frame carried a plausible wrong value.
        checkTrue("u16 refuses 70000", refused(new Refusable() {
            public void run() { new Zdw.Writer(Zdw.LE, Zdw.XCDR2).u16(70000); }
        }));
        checkTrue("u8 refuses 256", refused(new Refusable() {
            public void run() { new Zdw.Writer(Zdw.LE, Zdw.XCDR2).u8(256); }
        }));
        checkTrue("u32 refuses 2^32", refused(new Refusable() {
            public void run() { new Zdw.Writer(Zdw.LE, Zdw.XCDR2).u32(0x100000000L); }
        }));
        checkTrue("u32 refuses a negative", refused(new Refusable() {
            public void run() { new Zdw.Writer(Zdw.LE, Zdw.XCDR2).u32(-1L); }
        }));
    }

    static void parameterList() {
        // tests/interop-results/struct-mutable/zerodds-rust/pl-cdr1.le.bin.
        // The three things this pins: the double's parameter declares 8
        // and not 12, because its value aligns against its own block; the
        // short's declares 4 and not 2, because the padding that brings
        // the next header back to a four-octet boundary is inside the
        // length; and the sentinel carries the must-understand flag, so it
        // is 0x7F02 and not 0x3F02.
        Zdw.Writer w = new Zdw.Writer(Zdw.LE, Zdw.XCDR1);
        Zdw.Pid p = w.pidBegin(0, false);
        w.u32(0xFFED2979L);
        w.pidEnd(p);
        p = w.pidBegin(1, false);
        w.f64(3.141592653589793);
        w.pidEnd(p);
        p = w.pidBegin(2, false);
        w.u16(0xFFFE);
        w.pidEnd(p);
        w.pidListEnd();
        check("struct-mutable xcdr1 matches the rust column", hex(w),
            "00000400" + "7929edff" + "01000800" + "182d4454fb210940"
            + "02000400" + "feff0000" + "027f0000");

        // The same case's pl-cdr1.be.bin: both halves of the header go
        // out in the stream's order.
        w = new Zdw.Writer(Zdw.BE, Zdw.XCDR1);
        p = w.pidBegin(0, false);
        w.u32(0xFFED2979L);
        w.pidEnd(p);
        p = w.pidBegin(1, false);
        w.f64(3.141592653589793);
        w.pidEnd(p);
        p = w.pidBegin(2, false);
        w.u16(0xFFFE);
        w.pidEnd(p);
        w.pidListEnd();
        check("struct-mutable xcdr1 big endian", hex(w),
            "00000004" + "ffed2979" + "00010008" + "400921fb54442d18"
            + "00020004" + "fffe0000" + "7f020000");

        // tests/interop-results/annotation-id-sparse/zerodds-rust/
        // pl-cdr1.le.bin. The id 16383 is past 0x3F00 and forces the long
        // form even though the value is four octets.
        w = new Zdw.Writer(Zdw.LE, Zdw.XCDR1);
        p = w.pidBegin(0, false);
        w.u32(0xFFED2979L);
        w.pidEnd(p);
        p = w.pidBegin(1, false);
        w.u32(0x7FFFFFFFL);
        w.pidEnd(p);
        p = w.pidExtendedBegin(16383, false);
        w.u32(0xFFFFFFFEL);
        w.pidEnd(p);
        p = w.pidExtendedBegin(268435455, false);
        w.u32(0x00000001L);
        w.pidEnd(p);
        w.pidListEnd();
        check("the extended parameter header matches the rust column", hex(w),
            "00000400" + "7929edff" + "01000400" + "ffffff7f"
            + "017f0800" + "ff3f0000" + "04000000" + "feffffff"
            + "017f0800" + "ffffff0f" + "04000000" + "01000000" + "027f0000");

        // tests/interop-results/annotation-must-understand -- the id 0
        // with the flag reads 0040 in a little-endian stream.
        w = new Zdw.Writer(Zdw.LE, Zdw.XCDR1);
        p = w.pidBegin(0, true);
        w.u32(0xFFED2979L);
        w.pidEnd(p);
        check("the parameter must-understand flag is bit fourteen", hex(w),
            "00400400" + "7929edff");

        // 0x3F00 is the last id the short form carries, inclusive.
        new Zdw.Writer(Zdw.LE, Zdw.XCDR1).pidBegin(0x3F00, false);
        checkTrue("an id past the short range is refused by the short header",
            refused(new Refusable() {
                public void run() {
                    new Zdw.Writer(Zdw.LE, Zdw.XCDR1).pidBegin(0x3F01, false);
                }
            }));

        // tests/interop-results/annotation-optional-absent -- the header
        // is written either way and declares zero.
        w = new Zdw.Writer(Zdw.LE, Zdw.XCDR1);
        w.u32(0xFFED2979L);
        w.pidEnd(w.pidBegin(1, false));
        w.u32(0xFFFFFFFEL);
        check("an absent optional parameter keeps its header", hex(w),
            "7929edff" + "01000000" + "feffffff");
    }

    static void wideForms() {
        // tests/interop-results/primitive-wchar.
        Zdw.Writer le = new Zdw.Writer(Zdw.LE, Zdw.XCDR2);
        le.putWchar(0x00E4);
        le.putWchar(0x1234);
        check("wchar is two octets in the stream's order (le)", hex(le), "e4003412");
        Zdw.Writer be = new Zdw.Writer(Zdw.BE, Zdw.XCDR2);
        be.putWchar(0x00E4);
        be.putWchar(0x1234);
        check("wchar is two octets in the stream's order (be)", hex(be), "00e41234");

        // tests/interop-results/char-and-wchar -- one octet of padding
        // before the wide one.
        Zdw.Writer w = new Zdw.Writer(Zdw.LE, Zdw.XCDR2);
        w.putChar('a');
        w.putWchar('z');
        check("wchar aligns to two", hex(w), "61007a00");

        new Zdw.Writer(Zdw.LE, Zdw.XCDR2).putWchar(0xFFFF);
        checkTrue("a code point past the bmp has no wchar form",
            refused(new Refusable() {
                public void run() { new Zdw.Writer(Zdw.LE, Zdw.XCDR2).putWchar(0x10000); }
            }));
        new Zdw.Writer(Zdw.LE, Zdw.XCDR2).putChar(0xFF);
        checkTrue("a code point past latin-1 has no char form",
            refused(new Refusable() {
                public void run() { new Zdw.Writer(Zdw.LE, Zdw.XCDR2).putChar(0x0161); }
            }));

        // tests/interop-results/wstring-unbounded-utf8 -- seven code units
        // and the length word reads 14, not 7.
        w = new Zdw.Writer(Zdw.LE, Zdw.XCDR2);
        w.putWstring(new int[] { 0x00E4, 0x00F6, 0x00FC, 0x0020, 0x1234, 0x0020, 0x005A });
        check("wstring length counts octets", hex(w),
            "0e000000" + "e400" + "f600" + "fc00" + "2000" + "3412" + "2000" + "5a00");
        w = new Zdw.Writer(Zdw.LE, Zdw.XCDR2);
        w.putWstring(new int[] {});
        check("the empty wstring is a length of zero", hex(w), "00000000");
    }

    static void bitset() {
        // bitset-basic: bitfield<3> low = 5, bitfield<5> high = 17.
        Zdw.Writer w = new Zdw.Writer(Zdw.LE, Zdw.XCDR2);
        w.putBitset(5 | 17 << 3, 1);
        check("bitset-basic", hex(w), "8d");
        // bitset-typed-fields: 5, 1, 4001 over 3 + 1 + 12 bits.
        w = new Zdw.Writer(Zdw.LE, Zdw.XCDR2);
        w.putBitset(5 | 1 << 3 | 4001 << 4, 2);
        check("bitset-typed-fields", hex(w), "1dfa");
        w = new Zdw.Writer(Zdw.BE, Zdw.XCDR2);
        w.putBitset(5 | 1 << 3 | 4001 << 4, 2);
        check("bitset-typed-fields be", hex(w), "fa1d");
        // bitset-multiword: 20 + 20 + 24 bits into one eight-octet anchor.
        w = new Zdw.Writer(Zdw.LE, Zdw.XCDR2);
        w.putBitset(0xFFFFFL | 0xBC614EL << 40, 8);
        check("bitset-multiword", hex(w), "ffff0f00004e61bc");
        w = new Zdw.Writer(Zdw.BE, Zdw.XCDR2);
        w.putBitset(0xFFFFFL | 0xBC614EL << 40, 8);
        check("bitset-multiword be", hex(w), "bc614e00000fffff");

        // The one place a bitset differs between the two encodings: the
        // anchor's own alignment, capped at four under XCDR2.
        Zdw.Writer one = new Zdw.Writer(Zdw.LE, Zdw.XCDR1);
        one.u8(0xAB);
        one.putBitset(1, 8);
        check("an eight-octet anchor aligns eight under xcdr1", hex(one),
            "ab00000000000000" + "0100000000000000");
        Zdw.Writer two = new Zdw.Writer(Zdw.LE, Zdw.XCDR2);
        two.u8(0xAB);
        two.putBitset(1, 8);
        check("an eight-octet anchor aligns four under xcdr2", hex(two),
            "ab000000" + "0100000000000000");
        checkTrue("an anchor that is not one, two, four or eight is refused",
            refused(new Refusable() {
                public void run() { new Zdw.Writer(Zdw.LE, Zdw.XCDR2).putBitset(0, 3); }
            }));
    }

    static void fixed() {
        // tests/interop-results/fixed-basic -- fixed<9,3> with
        // "123456.789" and "-000000.001". P + 1 = 10 is even, so there is
        // no leading zero nibble and each value is five octets.
        Zdw.Writer w = new Zdw.Writer(Zdw.LE, Zdw.XCDR2);
        w.putFixed("123456.789", 9, 3);
        w.putFixed("-000000.001", 9, 3);
        w.u8(0xAB);
        check("fixed matches the rust column", hex(w),
            "123456789c" + "000000001d" + "ab");

        // No multi-octet integer is written, so nothing consults the
        // stream's order.
        Zdw.Writer le = new Zdw.Writer(Zdw.LE, Zdw.XCDR1);
        le.putFixed("123456.789", 9, 3);
        Zdw.Writer be = new Zdw.Writer(Zdw.BE, Zdw.XCDR2);
        be.putFixed("123456.789", 9, 3);
        check("fixed is independent of the byte order", hex(le), hex(be));

        String[][] cases = {
            { "1", "1", "0", "1c" },
            { "9", "1", "0", "9c" },
            { "-9", "1", "0", "9d" },
            { "12", "2", "0", "012c" },
            { "0.123", "3", "3", "123c" },
            { "-0.001", "3", "3", "001d" },
            { "-0", "1", "0", "0c" },
            { "-0.000", "3", "3", "000c" },
            { "1234567890123456789012345678901", "31", "0",
              "1234567890123456789012345678901c" },
            { "-1234567890123456789012345678901", "31", "0",
              "1234567890123456789012345678901d" },
            { "7.5", "9", "3", "000007500c" },
        };
        for (String[] tc : cases) {
            int digits = Integer.parseInt(tc[1]);
            int scale = Integer.parseInt(tc[2]);
            w = new Zdw.Writer(Zdw.LE, Zdw.XCDR2);
            w.putFixed(tc[0], digits, scale);
            check("fixed " + tc[0] + " as fixed<" + digits + "," + scale + ">",
                hex(w), tc[3]);
            checkTrue("fixed " + tc[0] + " octet count",
                w.size() == (digits + 2) / 2);
        }

        String[][] refusals = {
            { "1234567", "6", "0" },  // seven integer digits into six
            { "1.2345", "6", "3" },   // four fraction digits into three
            { "1.0", "2", "3" },      // a scale wider than the digit count
            { "twelve", "4", "0" },   // not a decimal
            { "", "4", "0" },         // no digits at all
        };
        for (String[] tc : refusals) {
            final String value = tc[0];
            final int digits = Integer.parseInt(tc[1]);
            final int scale = Integer.parseInt(tc[2]);
            checkTrue("a value wider than fixed<" + digits + "," + scale
                + "> is refused (" + value + ")", refused(new Refusable() {
                    public void run() {
                        new Zdw.Writer(Zdw.LE, Zdw.XCDR2).putFixed(value, digits, scale);
                    }
                }));
        }
    }

    static void readingDirection() {
        for (int encoding : new int[] { Zdw.XCDR1, Zdw.XCDR2 }) {
            for (int endian : new int[] { Zdw.LE, Zdw.BE }) {
                Zdw.Writer w = new Zdw.Writer(endian, encoding);
                w.u8(0xFF);
                w.bool(true);
                w.putChar(0xFF);
                w.u16(0xFFFF);
                w.u32(0xFFFFFFFFL);
                w.u64(-1L); // the pattern 0xFFFFFFFFFFFFFFFF
                w.f32(-3.5f);
                w.f64(3.141592653589793);
                w.putWchar(0xFFFF);
                w.str("grüße");
                w.seqU8(new byte[] { (byte) 0xDE, (byte) 0xAD });
                w.putWstring(new int[] { 0xD83D, 0xDE00 });
                w.putBitset(0xBC614E00000FFFFFL, 8);
                w.putFixed("-000000.001", 9, 3);
                Zdw.Reader r = new Zdw.Reader(w.bytes(), endian, encoding);
                int[] units;
                boolean ok = r.u8() == 0xFF && r.bool() && r.getChar() == 0xFF
                    && r.u16() == 0xFFFF && r.u32() == 0xFFFFFFFFL
                    && r.u64() == -1L && r.f32() == -3.5f
                    && r.f64() == 3.141592653589793
                    && r.getWchar() == 0xFFFF && r.str().equals("grüße");
                byte[] seq = r.seqU8();
                ok = ok && seq.length == 2 && (seq[0] & 0xFF) == 0xDE
                    && (seq[1] & 0xFF) == 0xAD;
                units = r.getWstring();
                ok = ok && units.length == 2 && units[0] == 0xD83D && units[1] == 0xDE00
                    && r.getBitset(8) == 0xBC614E00000FFFFFL
                    && r.getFixed(9, 3).equals("-000000.001")
                    && r.pos() == w.size();
                checkTrue("every write reads back (encoding=" + encoding
                    + " endian=" + endian + ")", ok);
            }
        }
    }

    static void framingReadsBack() {
        Zdw.Writer w = new Zdw.Writer(Zdw.LE, Zdw.XCDR2);
        int d = w.dheaderBegin();
        for (int lc = Zdw.LC0; lc <= Zdw.LC4; lc++) {
            int m = w.emheaderBegin(lc, lc == Zdw.LC4, lc);
            switch (lc) {
                case Zdw.LC0: w.u8(1); break;
                case Zdw.LC1: w.u16(2); break;
                case Zdw.LC2: w.u32(3); break;
                case Zdw.LC3: w.u64(4); break;
                default: w.u16(5); w.u16(6); break;
            }
            w.emheaderEnd(m, lc);
        }
        int m = w.emheaderBegin(5, false, Zdw.LC5);
        w.str("abc");
        w.emheaderEnd(m, Zdw.LC5);
        w.dheaderEnd(d);

        Zdw.Reader r = new Zdw.Reader(w.bytes(), Zdw.LE, Zdw.XCDR2);
        checkTrue("the dheader reads the body length",
            r.dheaderRead() == w.size() - 4);
        long[][] want = { { 0, 0, 0, 1 }, { 1, 0, 1, 2 }, { 2, 0, 2, 4 },
                          { 3, 0, 3, 8 }, { 4, 1, 4, 4 }, { 5, 0, 5, 8 } };
        boolean ok = true;
        for (long[] expect : want) {
            long[] got = r.emheaderReadLc();
            if (got[0] != expect[0] || got[1] != expect[1]
                || got[2] != expect[2] || got[3] != expect[3]) {
                ok = false;
            }
            r.skip((int) got[3]);
        }
        checkTrue("the framing reads back in every length code", ok);
    }

    static void parameterListReadsBack() {
        Zdw.Writer w = new Zdw.Writer(Zdw.LE, Zdw.XCDR1);
        Zdw.Pid p = w.pidBegin(0, false);
        w.u32(0xFFED2979L);
        w.pidEnd(p);
        p = w.pidBegin(1, true);
        w.f64(3.141592653589793);
        w.pidEnd(p);
        p = w.pidExtendedBegin(16383, true);
        w.u16(0xFFFE);
        w.pidEnd(p);
        w.pidListEnd();

        Zdw.Reader r = new Zdw.Reader(w.bytes(), Zdw.LE, Zdw.XCDR1);
        long[][] want = { { 0, 0, 4, 0 }, { 1, 1, 8, 0 }, { 16383, 1, 4, 0 } };
        boolean ok = true;
        for (long[] expect : want) {
            long[] got = r.pidRead();
            if (got[0] != expect[0] || got[1] != expect[1]
                || got[2] != expect[2] || got[3] != expect[3]) {
                ok = false;
            }
            r.pidSkip(r.pos(), got[2]);
        }
        checkTrue("the parameter list reads back", ok);
        checkTrue("the sentinel ends the list", r.pidRead()[3] == 1);
        checkTrue("the reader stops at the list end", r.pos() == w.size());
    }
}
