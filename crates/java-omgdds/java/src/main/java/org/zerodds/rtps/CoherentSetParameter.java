package org.zerodds.rtps;

/**
 * The value of {@code PID_COHERENT_SET} (DDSI-RTPS 2.5 9.6.4.2): a SequenceNumber_t, CDR-encoded
 * with the byte order of the submessage; the inline QoS of this implementation is little-endian.
 */
final class CoherentSetParameter {

    /** SEQUENCENUMBER_UNKNOWN: {@code high = -1, low = 0} (Table 9.3). */
    private static final long UNKNOWN = -1L << 32;

    private CoherentSetParameter() {
    }

    static byte[] encode(long firstSn) {
        int high = (int) (firstSn >> 32);
        int low = (int) firstSn;
        byte[] out = new byte[8];
        for (int i = 0; i < 4; i++) {
            out[i] = (byte) (high >>> (8 * i));
            out[4 + i] = (byte) (low >>> (8 * i));
        }
        return out;
    }

    /**
     * The sequence number of the first sample of the set, or {@link CoherentHold#NO_SET} if the
     * parameter is absent, short, or SEQUENCENUMBER_UNKNOWN (the sample is in no set).
     */
    static long decode(byte[] value) {
        if (value == null || value.length < 8) {
            return CoherentHold.NO_SET;
        }
        long high = 0;
        long low = 0;
        for (int i = 3; i >= 0; i--) {
            high = (high << 8) | (value[i] & 0xFF);
            low = (low << 8) | (value[4 + i] & 0xFF);
        }
        long sn = (long) (int) high << 32 | low;
        return sn == UNKNOWN || sn <= 0 ? CoherentHold.NO_SET : sn;
    }
}
