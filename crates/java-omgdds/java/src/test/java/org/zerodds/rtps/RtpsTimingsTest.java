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
    void rtps_8_4_7_1_without_options_the_timings_are_those_of_the_standard() {
        RtpsTimings t = RtpsTimings.of(of(new HashMap<String, String>()));
        assertEquals(30_000L, t.spdpPeriodMillis, "9.6.2.4");
        assertEquals(500L, t.heartbeatResponseDelayMillis, "8.4.12.1");
        assertEquals(200L, t.nackResponseDelayMillis, "8.4.7.1");
        assertEquals(1000L, t.heartbeatPeriodMillis, "no default in the standard (Table 8.45: N/A)");
        assertEquals(Integer.MAX_VALUE, t.sendHistoryMaxSamples, "RESOURCE_LIMITS is unlimited");
        assertEquals(Long.MAX_VALUE, t.sendHistoryMaxBytes);
    }

}
