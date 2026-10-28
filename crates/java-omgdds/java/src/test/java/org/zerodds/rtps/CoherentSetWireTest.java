// SPDX-License-Identifier: Apache-2.0
package org.zerodds.rtps;

import java.util.ArrayList;
import java.util.List;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * Coherent sets on the wire (DDSI-RTPS 2.5 8.7.6, 9.6.4.2) and the way a reader holds them.
 */
class CoherentSetWireTest {

    private static byte[] hex(String h) {
        byte[] out = new byte[h.length() / 2];
        for (int i = 0; i < out.length; i++) {
            out[i] = (byte) Integer.parseInt(h.substring(2 * i, 2 * i + 2), 16);
        }
        return out;
    }

    /**
     * The SequenceNumber_t {@code high = 0, low = 3} in little-endian CDR; the octets are those
     * the Rust implementation writes for {@code coherent_set_bytes(3, true)}
     * (crates/rtps/src/inline_qos.rs): the two halves in the byte order of the submessage.
     */
    @Test
    void theParameterValueIsASequenceNumberInTheByteOrderOfTheSubmessage() {
        assertArrayEquals(hex("0000000003000000"), CoherentSetParameter.encode(3L));
        assertArrayEquals(hex("0100000000000000"), CoherentSetParameter.encode(1L << 32));
        assertEquals(3L, CoherentSetParameter.decode(hex("0000000003000000")));
        assertEquals(1L << 32, CoherentSetParameter.decode(hex("0100000000000000")));
    }

    /** SEQUENCENUMBER_UNKNOWN (high -1, low 0) says the sample is in no set (Table 9.22, Example 2). */
    @Test
    void anUnknownSequenceNumberMeansNoSet() {
        assertEquals(CoherentHold.NO_SET, CoherentSetParameter.decode(hex("ffffffff00000000")));
        assertEquals(CoherentHold.NO_SET, CoherentSetParameter.decode(hex("0000")));
        assertEquals(CoherentHold.NO_SET, CoherentSetParameter.decode(null));
    }

    @Test
    void theInlineQosParameterHasTheIdOfTable914() {
        ParameterList list = new ParameterList().add(Rtps.PID_COHERENT_SET, CoherentSetParameter.encode(3L));
        assertArrayEquals(hex("56000800" + "0000000003000000" + "01000000"), list.toBytesLe());
    }

    /** Example 3 of Table 9.22: no DataFlag, no InlineQosFlag, only the sequence number. */
    @Test
    void theEndOfASetIsADataWithoutSerializedDataAndWithoutInlineQos() {
        byte[] message = new RtpsMessage.Builder(new byte[12])
                .data(new byte[] {0, 0, 0, 0}, new byte[] {0, 0, 1, 3}, 7L, null, new byte[0], false)
                .toBytes();
        // RTPS header (20 octets), then the submessage header: id DATA 0x15, flags E only
        assertEquals(0x15, message[20] & 0xFF);
        assertEquals(0x01, message[21] & 0xFF);
        RtpsMessage.Parsed parsed = RtpsMessage.decode(message, message.length);
        assertEquals(1, parsed.data.size());
        assertEquals(7L, parsed.data.get(0).writerSn);
        assertEquals(0, parsed.data.get(0).serializedPayload.length);
        assertTrue(parsed.data.get(0).inlineQos == null);
    }

    private static SerializedSample sample(int value) {
        return SerializedSample.parse(new byte[] {0, 7, 0, 0, (byte) value, 0, 0, 0});
    }

    private static List<Integer> values(List<SerializedSample> samples) {
        List<Integer> out = new ArrayList<Integer>();
        for (SerializedSample s : samples) {
            out.add(s.body()[0] & 0xFF);
        }
        return out;
    }

    private final List<String> logged = new ArrayList<String>();

    private CoherentHold hold() {
        return new CoherentHold(new java.util.function.Consumer<String>() {
            @Override
            public void accept(String line) {
                logged.add(line);
            }
        });
    }

    @Test
    void aSampleOutsideASetPassesAtOnce() {
        assertEquals(List.of(9), values(hold().accept("w", CoherentHold.NO_SET, 1L, sample(9))));
    }

    @Test
    void theSamplesOfASetAreHeldAndReleasedByTheFirstSampleOutsideIt() {
        CoherentHold hold = hold();
        assertTrue(hold.accept("w", 3L, 3L, sample(1)).isEmpty());
        assertTrue(hold.accept("w", 3L, 4L, sample(2)).isEmpty());
        assertTrue(hold.accept("w", 3L, 5L, sample(3)).isEmpty());
        assertEquals(List.of(1, 2, 3, 9),
                values(hold.accept("w", CoherentHold.NO_SET, 6L, sample(9))));
    }

    @Test
    void aNewSetEndsTheOpenOne() {
        CoherentHold hold = hold();
        hold.accept("w", 3L, 3L, sample(1));
        hold.accept("w", 3L, 4L, sample(2));
        assertEquals(List.of(1, 2), values(hold.accept("w", 5L, 5L, sample(3))));
        assertEquals(List.of(3), values(hold.end("w")));
    }

    @Test
    void theMarkerReleasesTheSetWithoutAnotherSample() {
        CoherentHold hold = hold();
        hold.accept("w", 3L, 3L, sample(1));
        assertEquals(List.of(1), values(hold.end("w")));
        assertTrue(hold.end("w").isEmpty());
    }

    @Test
    void aSetWithAMissingSampleIsNotReleasedAndSaysSo() {
        CoherentHold hold = hold();
        hold.accept("w", 3L, 3L, sample(1));
        hold.accept("w", 3L, 5L, sample(3)); // 4 did not arrive
        assertTrue(hold.end("w").isEmpty());
        assertEquals(1, logged.size());
        assertTrue(logged.get(0).contains("incomplete"));
    }

    @Test
    void aSetWhoseFirstSampleDidNotArriveIsNotReleased() {
        CoherentHold hold = hold();
        hold.accept("w", 3L, 4L, sample(2));
        assertTrue(hold.end("w").isEmpty());
        assertEquals(1, logged.size());
    }

    @Test
    void theSetsOfTwoWritersAreIndependent() {
        CoherentHold hold = hold();
        hold.accept("a", 1L, 1L, sample(1));
        hold.accept("b", 1L, 1L, sample(2));
        assertEquals(List.of(2), values(hold.end("b")));
        assertEquals(List.of(1), values(hold.end("a")));
    }
}
