// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

import org.junit.jupiter.api.Test;
import org.omg.dds.type.Extensibility;
import org.omg.dds.type.Optional;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;

/**
 * XTypes 1.3 §7.4.3.5.2, rule 20: an optional member of a FINAL or APPENDABLE structure in
 * XCDR2 is preceded by the boolean {@code is_present}, and left out when it is 0.
 *
 * <p>The octets are those of the interoperability corpus (tests/interop-corpus,
 * {@code annotation-optional-present}, {@code annotation-optional-absent} and
 * {@code annotation-optional-present-unannotated}, XCDR2 little-endian), which the Rust codec
 * ({@code wireprobe-zerodds}) produced; they are the recorded output of another
 * implementation of the same rules, not values derived here.
 */
class OptionalMemberWireTest {

    @Extensibility(Extensibility.Kind.FINAL_EXTENSIBILITY)
    public static final class FinalSample {
        public int a;
        @Optional
        public Integer b;
        public int c;
    }

    // an unannotated IDL struct is appendable (XTypes 1.3 §7.2.2.4.4.4); a Java class that is
    // not final extends Object directly is APPENDABLE too
    public static class AppendableSample {
        public int a;
        @Optional
        public Integer b;
        public int c;
    }

    private static byte[] hex(String h) {
        byte[] out = new byte[h.length() / 2];
        for (int i = 0; i < out.length; i++) {
            out[i] = (byte) Integer.parseInt(h.substring(2 * i, 2 * i + 2), 16);
        }
        return out;
    }

    private static FinalSample finalSample(Integer b) {
        FinalSample s = new FinalSample();
        s.a = -1234567;
        s.b = b;
        s.c = -2;
        return s;
    }

    @Test
    void aPresentOptionalOfAFinalStructureHasTheBooleanBeforeIt() {
        ReflectionTypeSupport<FinalSample> ts = ReflectionTypeSupport.of(FinalSample.class);
        byte[] bytes = ts.encode(finalSample(2147483647));
        assertArrayEquals(hex("7929edff01000000ffffff7ffeffffff"), bytes);
        FinalSample back = ts.decode(bytes);
        assertEquals(Integer.valueOf(2147483647), back.b);
        assertEquals(-2, back.c);
    }

    @Test
    void anAbsentOptionalOfAFinalStructureIsTheBooleanZeroAlone() {
        ReflectionTypeSupport<FinalSample> ts = ReflectionTypeSupport.of(FinalSample.class);
        byte[] bytes = ts.encode(finalSample(null));
        assertArrayEquals(hex("7929edff00000000feffffff"), bytes);
        FinalSample back = ts.decode(bytes);
        assertNull(back.b);
        assertEquals(-1234567, back.a);
        assertEquals(-2, back.c);
    }

    @Test
    void anOptionalOfAnAppendableStructureHasTheBooleanBeforeItAfterTheDheader() {
        ReflectionTypeSupport<AppendableSample> ts =
                ReflectionTypeSupport.of(AppendableSample.class);
        AppendableSample s = new AppendableSample();
        s.a = -1234567;
        s.b = 2147483647;
        s.c = -2;
        byte[] bytes = ts.encode(s);
        assertArrayEquals(hex("100000007929edff01000000ffffff7ffeffffff"), bytes);
        AppendableSample back = ts.decode(bytes);
        assertEquals(Integer.valueOf(2147483647), back.b);
        s.b = null;
        assertNull(ts.decode(ts.encode(s)).b);
    }
}
