// SPDX-License-Identifier: Apache-2.0
package org.zerodds.rtps;

/** Focused invariants discovered while joining the existing transport paths. */
public final class ReliableWindowCheck {
    private static RtpsMessage.Data sample(long sequence) {
        RtpsMessage.Data data = new RtpsMessage.Data();
        data.writerSn = sequence; data.serializedPayload = new byte[] {0, 7, 0, 0};
        return data;
    }
    private static void check(boolean value) { if (!value) throw new AssertionError(); }
    public static void main(String[] args) {
        ReliableReceiveState full = new ReliableReceiveState();
        for (long sequence = 2; sequence <= 257; sequence++) check(full.offer(sample(sequence)).isEmpty());
        check(full.offer(sample(1)).size() == 257 && full.base() == 258);
        ReliableReceiveState floor = new ReliableReceiveState();
        floor.offer(sample(2));
        check(floor.heartbeat(3).size() == 1 && floor.base() == 3);
        ReliableReceiveState gaps = new ReliableReceiveState();
        gaps.offer(sample(5)); gaps.gap(5, 6);
        for (long sequence = 1; sequence <= 4; sequence++) gaps.offer(sample(sequence));
        check(gaps.base() == 6);
        for (long sequence = 7; sequence <= 262; sequence++) gaps.offer(sample(sequence));
        check(gaps.offer(sample(6)).size() == 257 && gaps.base() == 263);
        FragmentReassembly reassembly = new FragmentReassembly();
        byte[] prefix = new byte[12];
        RtpsMessage.DataFragment first = new RtpsMessage.DataFragment();
        first.writerId = Guid.TL_REPLY_WRITER; first.readerId = Guid.ENTITYID_UNKNOWN;
        first.writerSn = 1; first.fragmentSize = 4; first.fragmentStartingNum = 1;
        first.fragmentsInSubmessage = 1; first.sampleSize = 8; first.serializedPayload = new byte[4];
        check(reassembly.accept(prefix, first) == null);
        first.readerId = Guid.TL_REPLY_READER; first.fragmentStartingNum = 2;
        RtpsMessage.Data complete = reassembly.accept(prefix, first);
        check(complete != null && java.util.Arrays.equals(complete.readerId, Guid.ENTITYID_UNKNOWN));
        System.out.println("JAVA_RELIABILITY_INVARIANTS=FULL_WINDOW_REPAIR,HISTORY_FLOOR,GAP_RELEASE,DIRECTED_FRAGMENT_REPAIR");
    }
}
