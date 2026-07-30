// SPDX-License-Identifier: Apache-2.0
package org.zerodds.rtps;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.nio.charset.StandardCharsets;
import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.concurrent.LinkedBlockingQueue;
import java.util.concurrent.TimeUnit;

import org.junit.jupiter.api.Test;

/** Pure-Java RTPS wire stack: byte-layout unit tests + a real UDP roundtrip. */
final class RtpsWireTest {

    @Test
    void parameterListRoundtripAndPadding() {
        ParameterList pl = new ParameterList()
                .add(Rtps.PID_TOPIC_NAME, ParameterList.cdrString("Chatter"))
                .add(Rtps.PID_TYPE_NAME, ParameterList.cdrString("zerodds::RawBytes"));
        byte[] wire = pl.toBytesLe();
        // every parameter length field must be 4-aligned (Rust decoder rejects otherwise)
        int pos = 0;
        while (pos + 4 <= wire.length) {
            int id = Wire.u16le(wire, pos);
            int len = Wire.u16le(wire, pos + 2);
            pos += 4;
            if (id == Rtps.PID_SENTINEL) {
                break;
            }
            assertEquals(0, len % 4, "PID 0x" + Integer.toHexString(id) + " length not 4-aligned");
            pos += len;
        }
        ParameterList back = ParameterList.fromBytesLe(wire, 0, wire.length);
        assertEquals("Chatter", ParameterList.readCdrString(back.find(Rtps.PID_TOPIC_NAME)));
        assertEquals("zerodds::RawBytes", ParameterList.readCdrString(back.find(Rtps.PID_TYPE_NAME)));
    }

    @Test
    void cdrStringLengthIncludesNullTerminator() {
        byte[] v = ParameterList.cdrString("ab");
        // u32 length = bytes+1 = 3, then 'a','b','\0'
        assertEquals(3, (int) Wire.u32le(v, 0));
        assertEquals('a', v[4]);
        assertEquals('b', v[5]);
        assertEquals(0, v[6]);
    }

    @Test
    void dataSubmessageRoundtrip() {
        byte[] prefix = new byte[12];
        for (int i = 0; i < 12; i++) {
            prefix[i] = (byte) (i + 1);
        }
        byte[] body = "body-with-encap".getBytes(StandardCharsets.UTF_8);
        byte[] payload = new byte[4 + body.length];
        System.arraycopy(Rtps.ENCAP_CDR_LE, 0, payload, 0, 4);
        System.arraycopy(body, 0, payload, 4, body.length);
        byte[] readerId = Guid.userEntityId(0, 0, 1, Guid.KIND_USER_READER_NO_KEY);
        byte[] writerId = Guid.userEntityId(0, 0, 2, Guid.KIND_USER_WRITER_NO_KEY);
        byte[] dg = new RtpsMessage.Builder(prefix)
                .infoTimestamp(123, 456)
                .data(readerId, writerId, 7L, null, payload, false)
                .toBytes();
        RtpsMessage.Parsed p = RtpsMessage.decode(dg, dg.length);
        assertNotNull(p);
        assertArrayEquals(prefix, p.header.guidPrefix);
        assertEquals(1, p.data.size());
        RtpsMessage.Data d = p.data.get(0);
        assertArrayEquals(readerId, d.readerId);
        assertArrayEquals(writerId, d.writerId);
        assertEquals(7L, d.writerSn);
        assertArrayEquals(payload, d.serializedPayload);
        assertArrayEquals("body-with-encap".getBytes(StandardCharsets.UTF_8),
                RtpsParticipant.stripEncap(d.serializedPayload));
    }

    @Test
    void spdpParticipantRoundtrip() {
        byte[] prefix = new byte[12];
        prefix[0] = (byte) 0x01;
        prefix[1] = (byte) 0xF0;
        prefix[11] = 0x2A;
        Locator meta = Locator.udpV4("127.0.0.1", 7412);
        byte[] payload = DiscoveryData.buildParticipant(prefix, 0, meta, meta,
                Rtps.EP_ALL_BASIC, 100);
        assertEquals(0x00, payload[0]);
        assertEquals(0x03, payload[1]); // PL_CDR_LE
        DiscoveryData.Participant p = DiscoveryData.parseParticipant(payload);
        assertNotNull(p);
        assertArrayEquals(prefix, p.guidPrefix);
        assertEquals(7412, (int) p.metatrafficUnicast.port);
        assertEquals(Rtps.EP_ALL_BASIC, p.builtinEndpointSet);
    }

    @Test
    void sedpEndpointRoundtripCarriesTypeId() {
        byte[] prefix = new byte[12];
        prefix[3] = 0x09;
        byte[] endpointGuid = new Guid(prefix,
                Guid.userEntityId(0, 0, 5, Guid.KIND_USER_WRITER_NO_KEY)).bytes();
        byte[] typeId = new byte[14];
        for (int i = 0; i < 14; i++) {
            typeId[i] = (byte) (0xA0 + i);
        }
        Locator loc = Locator.udpV4("127.0.0.1", 7411);
        byte[] payload = DiscoveryData.buildEndpoint(true, prefix, endpointGuid, "Chatter",
                "zerodds::RawBytes", Rtps.RELIABILITY_BEST_EFFORT, Rtps.DURABILITY_VOLATILE,
                new int[] {Rtps.DATA_REP_XCDR1}, loc, typeId);
        DiscoveryData.Endpoint e = DiscoveryData.parseEndpoint(true, payload);
        assertNotNull(e);
        assertEquals("Chatter", e.topicName);
        assertEquals("zerodds::RawBytes", e.typeName);
        assertArrayEquals(endpointGuid, e.endpointGuid);
        // the ParameterList pads the 14-byte value up to 16; the hash occupies
        // the first 14 bytes (trailing bytes are alignment padding).
        assertTrue(e.typeId.length >= 14);
        assertArrayEquals(typeId, java.util.Arrays.copyOf(e.typeId, 14));
        assertEquals(7411, (int) e.unicastLocator.port);
    }

    @Test
    void twoParticipantsExchangeSampleOverUdp() throws Exception {
        List<String> logs = new CopyOnWriteArrayList<>();
        RtpsParticipant.LOG = logs::add;
        RtpsParticipant pub = RtpsParticipant.newParticipant(7);
        RtpsParticipant sub = RtpsParticipant.newParticipant(7);
        try {
            LinkedBlockingQueue<byte[]> got = new LinkedBlockingQueue<>();
            sub.createReader("Chatter", "zerodds::RawBytes", false, got::add);
            RtpsParticipant.WireWriter w = pub.createWriter("Chatter", "zerodds::RawBytes",
                    Rtps.ENCAP_CDR_LE, new byte[0], false, false);

            // wait for SEDP match
            long deadline = System.currentTimeMillis() + 10000;
            while (w.matchedCount() == 0 && System.currentTimeMillis() < deadline) {
                Thread.sleep(50);
            }
            assertTrue(w.matchedCount() > 0, "writer never matched a reader over SEDP; logs=" + logs);

            byte[] msg = "hello #42".getBytes(StandardCharsets.UTF_8);
            byte[] received = null;
            for (int i = 0; i < 20 && received == null; i++) {
                w.write(msg);
                received = got.poll(500, TimeUnit.MILLISECONDS);
            }
            assertNotNull(received, "no sample delivered over UDP; logs=" + logs);
            assertArrayEquals(msg, received);
            assertTrue(logs.stream().anyMatch(s -> s.contains("SEDP endpoint-match")),
                    "expected an SEDP endpoint-match log line");
        } finally {
            pub.close();
            sub.close();
            RtpsParticipant.LOG = m -> System.err.println("[rtps] " + m);
        }
    }
}
