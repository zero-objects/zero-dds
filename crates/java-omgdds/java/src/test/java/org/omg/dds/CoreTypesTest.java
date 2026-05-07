// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.Test;
import org.omg.dds.core.Duration;
import org.omg.dds.core.InstanceHandle;
import org.omg.dds.core.ReturnCode;
import org.omg.dds.core.Time;
import org.omg.dds.core.policy.Durability;
import org.omg.dds.core.policy.History;
import org.omg.dds.core.policy.QosProfile;
import org.omg.dds.core.policy.Reliability;

import static org.junit.jupiter.api.Assertions.*;

class CoreTypesTest {
    @Test
    void returnCodeOkHasNumericValue0() {
        assertEquals(0, ReturnCode.OK.code());
        assertTrue(ReturnCode.OK.isOk());
    }

    @Test
    void returnCodeRoundTripViaCode() {
        for (ReturnCode rc : ReturnCode.values()) {
            assertEquals(rc, ReturnCode.fromCode(rc.code()));
        }
    }

    @Test
    void returnCodeFromUnknownCodeThrows() {
        assertThrows(IllegalArgumentException.class, () -> ReturnCode.fromCode(99));
    }

    @Test
    void timeFromMillisRoundTrip() {
        Time t = Time.fromMillis(1234);
        assertEquals(1, t.sec());
        assertEquals(234_000_000L, t.nanosec());
        assertEquals(1234, t.toMillis());
    }

    @Test
    void durationInfiniteRecognized() {
        assertTrue(Duration.INFINITE.isInfinite());
        assertFalse(Duration.fromMillis(100).isInfinite());
    }

    @Test
    void instanceHandleNilDetected() {
        assertTrue(InstanceHandle.NIL.isNil());
        byte[] nonZero = new byte[16];
        nonZero[5] = 1;
        assertFalse(InstanceHandle.from(nonZero).isNil());
    }

    @Test
    void instanceHandleRejectsWrongLength() {
        assertThrows(IllegalArgumentException.class,
                () -> InstanceHandle.from(new byte[8]));
    }

    @Test
    void reliabilityAndHistoryQosDefaults() {
        assertEquals(Reliability.Kind.RELIABLE, QosProfile.DEFAULT.reliability().kind());
        assertEquals(Durability.VOLATILE, QosProfile.DEFAULT.durability());
        assertEquals(History.Kind.KEEP_LAST, QosProfile.DEFAULT.history().kind());
        assertEquals(10, QosProfile.DEFAULT.history().depth());
    }

    @Test
    void qosCompatibilityReliableReaderRequiresReliableWriter() {
        QosProfile reliableReader = new QosProfile(
                Reliability.RELIABLE_DEFAULT, Durability.VOLATILE, History.KEEP_LAST_10);
        QosProfile bestEffortWriter = new QosProfile(
                Reliability.BEST_EFFORT_DEFAULT, Durability.VOLATILE, History.KEEP_LAST_10);
        QosProfile reliableWriter = new QosProfile(
                Reliability.RELIABLE_DEFAULT, Durability.VOLATILE, History.KEEP_LAST_10);
        assertFalse(reliableReader.isCompatibleWith(bestEffortWriter));
        assertTrue(reliableReader.isCompatibleWith(reliableWriter));
    }

    @Test
    void historyDepthMustBeNonNegative() {
        assertThrows(IllegalArgumentException.class,
                () -> new History(History.Kind.KEEP_LAST, -1));
    }
}
