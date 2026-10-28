package org.zerodds.rtps;

import java.util.function.Function;

/**
 * The timing parameters and limits of the RTPS endpoints. The defaults are the values of DDSI-RTPS
 * 2.5; every other value is a configuration, set by the system property named here.
 *
 * <ul>
 * <li>{@code zerodds.rtps.spdp-period-millis}: the SPDP resend period, default 30 000 (9.6.2.4).</li>
 * <li>{@code zerodds.rtps.heartbeat-response-delay-millis}: the reader waits this long before the
 *     ACKNACK to a HEARTBEAT, default 500 (8.4.12.1).</li>
 * <li>{@code zerodds.rtps.nack-response-delay-millis}: the writer waits this long before the
 *     repair for an ACKNACK, default 200 (8.4.7.1).</li>
 * <li>{@code zerodds.rtps.heartbeat-period-millis}: the standard names no default for
 *     W::heartbeatPeriod (Table 8.45: N/A); default 1000, the value this runtime has always used.</li>
 * <li>{@code zerodds.rtps.send-history-max-samples}, {@code zerodds.rtps.send-history-max-bytes}:
 *     bound of the unacknowledged samples a reliable writer holds; default unlimited as
 *     RESOURCE_LIMITS is (DDS 1.4 2.2.3.19), bounded by the QoS the application sets.</li>
 * </ul>
 */
final class RtpsTimings {
    final long spdpPeriodMillis;
    final long heartbeatPeriodMillis;
    final long heartbeatResponseDelayMillis;
    final long nackResponseDelayMillis;
    final int sendHistoryMaxSamples;
    final long sendHistoryMaxBytes;

    private RtpsTimings(Function<String, String> lookup) {
        spdpPeriodMillis = number(lookup, "zerodds.rtps.spdp-period-millis", 30_000L);
        heartbeatPeriodMillis = Math.max(1L, number(lookup, "zerodds.rtps.heartbeat-period-millis", 1000L));
        heartbeatResponseDelayMillis = number(lookup, "zerodds.rtps.heartbeat-response-delay-millis", 500L);
        nackResponseDelayMillis = number(lookup, "zerodds.rtps.nack-response-delay-millis", 200L);
        sendHistoryMaxSamples = (int) Math.min(Integer.MAX_VALUE,
                number(lookup, "zerodds.rtps.send-history-max-samples", Integer.MAX_VALUE));
        sendHistoryMaxBytes = number(lookup, "zerodds.rtps.send-history-max-bytes", Long.MAX_VALUE);
    }

    /** The values of the given properties; a missing or invalid one has the default of the standard. */
    static RtpsTimings of(Function<String, String> lookup) {
        return new RtpsTimings(lookup);
    }

    /** The values of the system properties of this JVM. */
    static final RtpsTimings CURRENT = of(new Function<String, String>() {
        @Override
        public String apply(String name) {
            return System.getProperty(name);
        }
    });

    private static long number(Function<String, String> lookup, String name, long standard) {
        String value = lookup.apply(name);
        if (value == null) {
            return standard;
        }
        try {
            long parsed = Long.parseLong(value.trim());
            return parsed < 0 ? standard : parsed;
        } catch (NumberFormatException e) {
            throw new IllegalArgumentException(name + " is not a number: " + value, e);
        }
    }
}
