// SPDX-License-Identifier: Apache-2.0
package org.zerodds.rtps;

import java.util.function.Consumer;

/**
 * Bridge between the OMG DDS {@code DataWriter}/{@code DataReader} and the
 * pure-Java RTPS wire stack ({@link RtpsParticipant}). Lets DDS entities carry
 * samples cross-process over real UDP RTPS <em>alongside</em> the in-process
 * {@code InProcessBus} path.
 *
 * <p>Off by default so the in-process unit-test suite keeps running without
 * opening sockets; enable with {@code -Dzerodds.rtps.enable=true} or the
 * environment variable {@code ZERODDS_RTPS=1} (mirrors the Rust env-selected
 * transport). When enabled, one {@link RtpsParticipant} per domain is shared by
 * all writers/readers in the JVM.
 */
public final class DdsWireBridge {
    private DdsWireBridge() {}

    public static boolean enabled() {
        if (Boolean.parseBoolean(System.getProperty("zerodds.rtps.enable", "false"))) {
            return true;
        }
        String env = System.getenv("ZERODDS_RTPS");
        return env != null && (env.equals("1") || env.equalsIgnoreCase("true"));
    }

    /**
     * Choose the XCDR encapsulation header for a type. Structured
     * {@code org.zerodds.cdr.TopicTypeSupport} types serialize XCDR2 (plain CDR2
     * LE); raw {@code byte[]} / other supports use XCDR1 CDR_LE.
     */
    public static byte[] encapFor(Object typeSupport) {
        if (typeSupport instanceof org.zerodds.cdr.TopicTypeSupport) {
            return Rtps.ENCAP_CDR2_LE;
        }
        return Rtps.ENCAP_CDR_LE;
    }

    public static RtpsParticipant.WireWriter writer(int domainId, String topic, String typeName,
                                                    byte[] encap, byte[] typeId, boolean keyed) {
        return RtpsParticipant.get(domainId)
                .createWriter(topic, typeName, encap, typeId, keyed, true);
    }

    public static RtpsParticipant.WireReader reader(int domainId, String topic, String typeName,
                                                    boolean keyed, Consumer<byte[]> onBody) {
        return RtpsParticipant.get(domainId)
                .createReader(topic, typeName, keyed, onBody);
    }
}
