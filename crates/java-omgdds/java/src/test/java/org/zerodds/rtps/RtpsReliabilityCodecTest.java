// SPDX-License-Identifier: Apache-2.0
package org.zerodds.rtps;

import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.util.Arrays;
import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

/** Independent submessage vectors, not just encoder/decoder agreement. */
class RtpsReliabilityCodecTest {
    private static final byte[] READER = {0, 0, 1, 4};
    private static final byte[] WRITER = {0, 0, 2, 3};

    private static ByteBuffer body(int size, boolean le) {
        return ByteBuffer.allocate(size).order(le ? ByteOrder.LITTLE_ENDIAN : ByteOrder.BIG_ENDIAN);
    }

    private static void sn(ByteBuffer b, long value) {
        b.putInt((int) (value >>> 32)).putInt((int) value);
    }

    private static byte[] frame(int id, int flags, boolean le, byte[] body) {
        ByteBuffer b = body(24 + body.length, le);
        b.put(new byte[] {'R', 'T', 'P', 'S', 2, 3, 1, 16});
        b.put(new byte[12]).put((byte) id).put((byte) (flags | (le ? 1 : 0)));
        b.putShort((short) body.length).put(body);
        return b.array();
    }

    private static RtpsMessage.Parsed parse(byte[] bytes) {
        return RtpsMessage.decode(bytes, bytes.length);
    }

    @Test void ackNackBothEndiansAndStrictBitmapBounds() {
        for (boolean le : new boolean[] {false, true}) {
            ByteBuffer b = body(32, le);
            b.put(READER).put(WRITER); sn(b, 7);
            b.putInt(33).putInt(0x80000001).putInt(0x80000000).putInt(42);
            byte[] bytes = frame(6, 2, le, b.array());
            RtpsMessage.AckNack ack = parse(bytes).ackNacks.get(0);
            assertEquals(7, ack.base); assertEquals(33, ack.numBits);
            assertArrayEquals(new long[] {0x80000001L, 0x80000000L}, ack.bitmap);
            assertEquals(42, ack.count); assertTrue(ack.finalFlag);
            assertArrayEquals(READER, ack.readerId);
            b.putInt(16, 257);
            assertTrue(parse(frame(6, 0, le, b.array())).ackNacks.isEmpty());
            assertTrue(parse(frame(6, 0, le, Arrays.copyOf(b.array(), 23))).ackNacks.isEmpty());
        }
        assertThrows(IllegalArgumentException.class, () -> new RtpsMessage.Builder(new byte[12])
                .ackNack(READER, WRITER, 1, 257, 1, false));
        assertEquals(Long.MAX_VALUE, parse(new RtpsMessage.Builder(new byte[12])
                .ackNack(READER, WRITER, Long.MAX_VALUE, 1, 1, false).toBytes()).ackNacks.get(0).base);
    }

    @Test void gapBothEndiansAndContiguousBuilder() {
        for (boolean le : new boolean[] {false, true}) {
            ByteBuffer b = body(32, le);
            b.put(READER).put(WRITER); sn(b, 3); sn(b, 8);
            b.putInt(1).putInt(0x80000000);
            RtpsMessage.Gap gap = parse(frame(8, 0, le, b.array())).gaps.get(0);
            assertEquals(3, gap.gapStart); assertEquals(8, gap.gapListBase);
            assertArrayEquals(new long[] {0x80000000L}, gap.bitmap);
            b.putInt(24, 257);
            assertTrue(parse(frame(8, 0, le, b.array())).gaps.isEmpty());
        }
        byte[] destination = new byte[12]; destination[0] = 8;
        RtpsMessage.Gap gap = parse(new RtpsMessage.Builder(new byte[12])
                .infoDestination(destination).gap(READER, WRITER, 3, 8).toBytes()).gaps.get(0);
        assertEquals(0, gap.numBits); assertArrayEquals(destination, gap.destPrefix);
    }

    private static ByteBuffer fragment(boolean le, boolean qos) {
        ByteBuffer b = body(qos ? 52 : 40, le);
        b.putShort((short) 0).putShort((short) 28).put(READER).put(WRITER);
        sn(b, 9); b.putInt(2).putShort((short) 2).putShort((short) 4).putInt(10);
        if (qos) b.putShort((short) 0x70).putShort((short) 4).putInt(123)
                .putShort((short) 1).putShort((short) 0);
        b.put(new byte[] {5, 6, 7, 8, 9, 10}).putShort((short) 0);
        return b;
    }

    @Test void dataFragBothEndiansWithInlineQosKeyAndPadding() {
        for (boolean le : new boolean[] {false, true}) {
            for (boolean qos : new boolean[] {false, true}) {
                RtpsMessage.DataFragment f = parse(frame(0x16, 4 | (qos ? 2 : 0), le,
                        fragment(le, qos).array())).dataFragments.get(0);
                assertEquals(9, f.writerSn); assertEquals(2, f.fragmentStartingNum);
                assertEquals(2, f.fragmentsInSubmessage); assertEquals(4, f.fragmentSize);
                assertEquals(10, f.sampleSize); assertTrue(f.keyFlag);
                assertEquals(qos, f.inlineQos != null);
                assertArrayEquals(new byte[] {5, 6, 7, 8, 9, 10}, f.serializedPayload);
            }
        }
    }

    @Test void malformedFragmentsAreDroppedWithoutClamping() {
        for (boolean le : new boolean[] {false, true}) {
            for (int offset : new int[] {20, 24, 26, 28}) {
                ByteBuffer b = fragment(le, false);
                if (offset == 24 || offset == 26) b.putShort(offset, (short) 0);
                else b.putInt(offset, 0);
                assertTrue(parse(frame(0x16, 0, le, b.array())).dataFragments.isEmpty());
            }
            byte[] b = fragment(le, false).array();
            assertTrue(parse(frame(0x16, 0, le, Arrays.copyOf(b, 37))).dataFragments.isEmpty());
            assertTrue(parse(frame(0x16, 0, le, Arrays.copyOf(b, 42))).dataFragments.isEmpty());
            ByteBuffer qos = fragment(le, true); qos.putShort(42, (short) 4);
            assertTrue(parse(frame(0x16, 2, le, qos.array())).dataFragments.isEmpty());
        }
        assertNull(RtpsMessage.decode(null, 20));
        assertNull(RtpsMessage.decode(new byte[20], 21));
    }

    @Test void fragmentFeedbackIndependentEndianVectors() {
        for (boolean le : new boolean[] {false, true}) {
            ByteBuffer nack = body(36, le);
            nack.put(READER).put(WRITER); sn(nack, 11);
            nack.putInt(7).putInt(33).putInt(0x80000001).putInt(0x80000000).putInt(17);
            RtpsMessage.NackFragment n = parse(frame(0x12, 0, le, nack.array())).nackFragments.get(0);
            assertEquals(11, n.writerSn); assertEquals(7, n.bitmapBase);
            assertEquals(33, n.numBits); assertEquals(17, n.count);
            assertArrayEquals(new long[] {0x80000001L, 0x80000000L}, n.bitmap);
            nack.putInt(20, 257);
            assertTrue(parse(frame(0x12, 0, le, nack.array())).nackFragments.isEmpty());
            nack.putInt(20, 33).putInt(16, -1);
            assertTrue(parse(frame(0x12, 0, le, nack.array())).nackFragments.isEmpty());
            assertTrue(parse(frame(0x12, 0, le, Arrays.copyOf(nack.array(), 27))).nackFragments.isEmpty());

            ByteBuffer heartbeat = body(24, le);
            heartbeat.put(READER).put(WRITER); sn(heartbeat, 11);
            heartbeat.putInt(-1).putInt(17);
            RtpsMessage.HeartbeatFragment h = parse(frame(0x13, 0, le, heartbeat.array()))
                    .heartbeatFragments.get(0);
            assertEquals(11, h.writerSn); assertEquals(0xffff_ffffL, h.lastFragmentNum);
            assertEquals(17, h.count);
            heartbeat.putInt(16, 0);
            assertTrue(parse(frame(0x13, 0, le, heartbeat.array())).heartbeatFragments.isEmpty());
        }
    }

    @Test void feedbackBuildersPreserveDestinationAndBoundaryBitmaps() {
        byte[] destination = new byte[12]; destination[0] = 42;
        RtpsMessage.Parsed p = parse(new RtpsMessage.Builder(new byte[12])
                .infoDestination(destination).heartbeat(READER, WRITER, 1, 7, 1, false)
                .heartbeatFrag(READER, WRITER, 7, 33, 2)
                .nackFrag(READER, WRITER, 7, 1, 33, 3).toBytes());
        assertArrayEquals(destination, p.heartbeats.get(0).destPrefix);
        assertArrayEquals(destination, p.heartbeatFragments.get(0).destPrefix);
        assertArrayEquals(destination, p.nackFragments.get(0).destPrefix);
        assertArrayEquals(new long[] {0xffff_ffffL, 0x80000000L}, p.nackFragments.get(0).bitmap);
        assertEquals(0xffff_ffffL, parse(new RtpsMessage.Builder(new byte[12])
                .nackFrag(READER, WRITER, 7, 0xffff_ffffL, 1, 3).toBytes())
                .nackFragments.get(0).bitmapBase);
        assertThrows(IllegalArgumentException.class, () -> new RtpsMessage.Builder(new byte[12])
                .nackFrag(READER, WRITER, 7, 0xffff_ffffL, 2, 3));
        assertThrows(IllegalArgumentException.class, () -> new RtpsMessage.Builder(new byte[12])
                .heartbeatFrag(READER, WRITER, 7, 0, 3));
    }

    @Test void dataAndKeyFlagsAreMutuallyExclusive() {
        for (boolean key : new boolean[] {false, true}) {
            byte[] bytes = new RtpsMessage.Builder(new byte[12])
                    .data(READER, WRITER, 1, null, new byte[] {0, 1, 0, 0}, key).toBytes();
            // Independent DATA PSM flags: D=0x04, K=0x08, never both.
            assertEquals(key ? 0x09 : 0x05, bytes[21] & 0xff);
            assertEquals(key, parse(bytes).data.get(0).keyFlag);
        }
    }

    @Test void fragmentBuilderPreservesPayloadAndFollowingSubmessage() {
        byte[] payload = {5, 6, 7, 8, 9, 10};
        RtpsMessage.Parsed p = parse(new RtpsMessage.Builder(new byte[12])
                .dataFrag(READER, WRITER, 9, 2, 2, 4, 10, null, payload, true)
                .gap(READER, WRITER, 1, 2).toBytes());
        assertArrayEquals(payload, p.dataFragments.get(0).serializedPayload);
        assertTrue(p.dataFragments.get(0).keyFlag); assertEquals(1, p.gaps.size());
        assertThrows(IllegalArgumentException.class, () -> new RtpsMessage.Builder(new byte[12])
                .dataFrag(READER, WRITER, 9, 3, 2, 4, 10, null, payload, false));
    }
}
