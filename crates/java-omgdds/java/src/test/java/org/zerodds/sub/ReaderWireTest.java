// SPDX-License-Identifier: Apache-2.0
package org.zerodds.sub;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.Qos;
import org.omg.dds.ReaderRig;
import org.omg.dds.core.event.SampleLostEvent;
import org.omg.dds.core.status.SampleLostStatus;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.DataReaderAdapter;
import org.omg.dds.sub.Sample;
import org.zerodds.cdr.EndianMode;
import org.zerodds.cdr.ExtensibilityKind;
import org.zerodds.cdr.TopicTypeSupport;
import org.zerodds.core.Values;
import org.zerodds.internal.InProcessBus;
import org.zerodds.internal.SourceIdentity;
import org.zerodds.rtps.Guid;
import org.zerodds.rtps.RtpsMessage;
import org.zerodds.rtps.SerializedSample;

import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.omg.dds.ReaderRig.Keyed;
import static org.omg.dds.ReaderRig.sample;

/**
 * What a reader makes of a sample that came over RTPS: the source timestamp of INFO_TS (DDSI-RTPS
 * 2.5 §8.3.7.9), the publication handle named by the writer GUID, and SAMPLE_LOST from a gap in
 * the sequence numbers of a writer (DDS 1.4 §2.2.4.1). The sample is handed to
 * {@code DataReaderImpl.deliverWire} the way the participant does after it parsed the DATA.
 */
class ReaderWireTest {

    private static int dom = 1130;

    private static int nextDom() {
        return dom++;
    }

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() {
        InProcessBus.instance().reset();
    }

    private static byte[] guid(int seed) {
        byte[] out = new byte[16];
        for (int i = 0; i < 16; i++) {
            out[i] = (byte) (seed + i);
        }
        return out;
    }

    /** The payload of a DATA: the encapsulation header and the XCDR2 body. */
    private static SerializedSample payload(Keyed value) {
        byte[] head = SerializedSample.encapsulation(TopicTypeSupport.XCDR2,
                ExtensibilityKind.APPENDABLE, EndianMode.LITTLE_ENDIAN);
        byte[] body = ReaderRig.SUPPORT.encode(value);
        byte[] all = new byte[head.length + body.length];
        System.arraycopy(head, 0, all, 0, head.length);
        System.arraycopy(body, 0, all, head.length, body.length);
        return SerializedSample.parse(all);
    }

    private static DataReaderImpl<Keyed> readerOf(ReaderRig rig) {
        return (DataReaderImpl<Keyed>) rig.keepAllReader();
    }

    @Test
    void javapsm_2_the_source_timestamp_and_the_writer_guid_of_a_data_reach_the_sample_info() {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReaderImpl<Keyed> reader = readerOf(rig);
        reader.deliverWire(payload(sample(1, 0))
                .withSource(new SourceIdentity(guid(1), 1), null)
                .withSourceTimestamp(77_000L));
        long before = System.currentTimeMillis();
        reader.deliverWire(payload(sample(2, 0)).withSource(new SourceIdentity(guid(1), 2), null));

        List<Sample<Keyed>> got = reader.read(new ArrayList<>());
        assertEquals(2, got.size());
        // the timestamp the writer sent
        assertEquals(77_000L, got.get(0).getSourceTimestamp().getTime(TimeUnit.MILLISECONDS));
        // DATA without INFO_TS: the time it arrived
        assertTrue(got.get(1).getSourceTimestamp().getTime(TimeUnit.MILLISECONDS) >= before);
        // the publication is the remote writer, named by its GUID
        assertEquals(Values.of(guid(1)), got.get(0).getPublicationHandle());
        assertEquals(Values.of(guid(1)), got.get(1).getPublicationHandle());
        // and the instance is the one of the key
        assertEquals(reader.lookupInstance(sample(1, 0)), got.get(0).getInstanceHandle());
    }

    @Test
    void javapsm_2_a_gap_in_the_sequence_numbers_of_a_writer_is_sample_lost() {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReaderImpl<Keyed> reader = readerOf(rig);
        reader.deliverWire(payload(sample(1, 0)).withSource(new SourceIdentity(guid(1), 1), null));
        assertEquals(0, reader.getSampleLostStatus().getTotalCount());
        // sequence numbers 2 and 3 never arrive
        reader.deliverWire(payload(sample(1, 1)).withSource(new SourceIdentity(guid(1), 4), null));
        reader.deliverWire(payload(sample(1, 2)).withSource(new SourceIdentity(guid(1), 5), null));
        // another writer starts at 10: nothing before it is missed
        reader.deliverWire(payload(sample(2, 0)).withSource(new SourceIdentity(guid(40), 10), null));

        SampleLostStatus status = reader.getSampleLostStatus();
        assertEquals(2, status.getTotalCount());
        assertEquals(2, status.getTotalCountChange());
        // read: the change starts again
        SampleLostStatus again = reader.getSampleLostStatus();
        assertEquals(2, again.getTotalCount());
        assertEquals(0, again.getTotalCountChange());
        // the samples that did arrive are all there
        assertEquals(4, reader.read(new ArrayList<>()).size());
    }

    @Test
    void javapsm_2_sample_lost_is_reported_to_the_listener() {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReaderImpl<Keyed> reader = readerOf(rig);
        AtomicInteger calls = new AtomicInteger();
        AtomicReference<SampleLostStatus> seen = new AtomicReference<>();
        reader.setListener(new DataReaderAdapter<Keyed>() {
            @Override
            public void onSampleLost(SampleLostEvent<Keyed> event) {
                calls.incrementAndGet();
                seen.set(event.getStatus());
            }
        });
        reader.deliverWire(payload(sample(1, 0)).withSource(new SourceIdentity(guid(7), 1), null));
        assertEquals(0, calls.get());
        reader.deliverWire(payload(sample(1, 1)).withSource(new SourceIdentity(guid(7), 3), null));
        assertEquals(1, calls.get());
        assertEquals(1, seen.get().getTotalCount());
        assertEquals(1, seen.get().getTotalCountChange());
        // the listener has read the status
        assertEquals(0, reader.getSampleLostStatus().getTotalCountChange());
        // an old sequence number is no gap
        reader.deliverWire(payload(sample(1, 2)).withSource(new SourceIdentity(guid(7), 2), null));
        assertEquals(1, calls.get());
    }

    /**
     * An RTPS message written out by hand (DDSI-RTPS 2.5 §9.4): the header, an INFO_TS
     * (§9.4.5.12) when {@code seconds} is not negative, and a DATA (§9.4.5.4) with sequence
     * number 7 and the payload {@code 00 07 00 00 01 02 03 04}, little-endian.
     */
    private static byte[] message(int seconds, long fraction) {
        java.nio.ByteBuffer b = java.nio.ByteBuffer.allocate(80).order(java.nio.ByteOrder.LITTLE_ENDIAN);
        b.put(new byte[] {'R', 'T', 'P', 'S', 2, 5, 0x01, 0x10}).put(new byte[12]);
        if (seconds >= 0) {
            b.put((byte) 0x09).put((byte) 0x01).putShort((short) 8);
            b.putInt(seconds).putInt((int) fraction);
        }
        b.put((byte) 0x15).put((byte) 0x05).putShort((short) 28);
        b.putShort((short) 0).putShort((short) 16);
        b.put(new byte[] {0, 0, 1, 4}).put(new byte[] {0, 0, 2, 3});
        b.putInt(0).putInt(7);
        b.put(new byte[] {0, 7, 0, 0, 1, 2, 3, 4});
        return java.util.Arrays.copyOf(b.array(), b.position());
    }

    @Test
    void rtps_8_3_7_9_the_info_timestamp_before_a_data_is_its_source_timestamp() {
        // 1_700_000_000 s and a fraction of 0x80000000 / 2^32 s = 500 ms (Time_t, §9.3.2)
        byte[] stamped = message(1_700_000_000, 0x80000000L);
        RtpsMessage.Data data = RtpsMessage.decode(stamped, stamped.length).data.get(0);
        assertEquals(7L, data.writerSn);
        assertEquals(1_700_000_000_500L, data.sourceTimestampMillis);
        // a DATA without an INFO_TS before it has none
        byte[] plain = message(-1, 0);
        assertEquals(-1L, RtpsMessage.decode(plain, plain.length).data.get(0).sourceTimestampMillis);
    }

    @Test
    void javapsm_2_a_reader_that_is_not_on_the_wire_waits_for_no_historical_data() throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> reader = rig.keepAllReader();
        // everything of this process arrived in enable(): both forms return at once
        reader.waitForHistoricalData(org.omg.dds.core.Duration.newDuration(1, TimeUnit.SECONDS, Qos.env()));
        reader.waitForHistoricalData(1, TimeUnit.SECONDS);
        org.junit.jupiter.api.Assertions.assertThrows(IllegalArgumentException.class,
                () -> reader.waitForHistoricalData(-1, TimeUnit.SECONDS));
    }
}
