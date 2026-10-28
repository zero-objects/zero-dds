// SPDX-License-Identifier: Apache-2.0
package org.zerodds.rtps;

import java.util.HashMap;
import java.util.Map;
import java.util.function.Function;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;

/** The defaults of the timings are the values of DDSI-RTPS 2.5; every other value is an option. */
class RtpsTimingsTest {

    private static Function<String, String> of(final Map<String, String> values) {
        return new Function<String, String>() {
            @Override
            public String apply(String name) {
                return values.get(name);
            }
        };
    }

    @Test
    void withoutOptionsTheValuesAreThoseOfTheStandard() {
        RtpsTimings t = RtpsTimings.of(of(new HashMap<String, String>()));
        assertEquals(30_000L, t.spdpPeriodMillis, "9.6.2.4");
        assertEquals(500L, t.heartbeatResponseDelayMillis, "8.4.12.1");
        assertEquals(200L, t.nackResponseDelayMillis, "8.4.7.1");
        assertEquals(1000L, t.heartbeatPeriodMillis, "no default in the standard (Table 8.45: N/A)");
        assertEquals(Integer.MAX_VALUE, t.sendHistoryMaxSamples, "RESOURCE_LIMITS is unlimited");
        assertEquals(Long.MAX_VALUE, t.sendHistoryMaxBytes);
    }

    @Test
    void anOptionSetsTheValue() {
        Map<String, String> v = new HashMap<String, String>();
        v.put("zerodds.rtps.spdp-period-millis", "1000");
        v.put("zerodds.rtps.heartbeat-response-delay-millis", "0");
        v.put("zerodds.rtps.nack-response-delay-millis", "0");
        v.put("zerodds.rtps.heartbeat-period-millis", "250");
        v.put("zerodds.rtps.send-history-max-samples", "256");
        v.put("zerodds.rtps.send-history-max-bytes", "16777216");
        RtpsTimings t = RtpsTimings.of(of(v));
        assertEquals(1000L, t.spdpPeriodMillis);
        assertEquals(0L, t.heartbeatResponseDelayMillis);
        assertEquals(0L, t.nackResponseDelayMillis);
        assertEquals(250L, t.heartbeatPeriodMillis);
        assertEquals(256, t.sendHistoryMaxSamples);
        assertEquals(16L * 1024 * 1024, t.sendHistoryMaxBytes);
    }

    @Test
    void aValueThatIsNoNumberIsRefused() {
        Map<String, String> v = new HashMap<String, String>();
        v.put("zerodds.rtps.spdp-period-millis", "soon");
        assertThrows(IllegalArgumentException.class, () -> RtpsTimings.of(of(v)));
    }
}
