// SPDX-License-Identifier: Apache-2.0
package org.zerodds.rtps;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/** The L flag of the HEARTBEAT (DDSI-RTPS 2.5 8.3.7.5, 9.4.5.5.3): bit 2 of the flags. */
class LivelinessHeartbeatTest {

    private static RtpsMessage.Heartbeat roundTrip(boolean finalFlag, boolean liveliness, int[] flagsOut) {
        byte[] message = new RtpsMessage.Builder(new byte[12])
                .heartbeat(new byte[4], new byte[] {0, 0, 1, 3}, 5L, 4L, 1, finalFlag, liveliness)
                .toBytes();
        flagsOut[0] = message[21] & 0xFF;
        return RtpsMessage.decode(message, message.length).heartbeats.get(0);
    }

    @Test
    void theLivelinessFlagIsTheThirdLowestBitAndIndependentOfFinal() {
        int[] flags = new int[1];
        assertTrue(roundTrip(false, true, flags).livelinessFlag);
        assertEquals(0x01 | 0x04, flags[0]);
        assertTrue(roundTrip(true, true, flags).livelinessFlag);
        assertEquals(0x01 | 0x02 | 0x04, flags[0]);
        RtpsMessage.Heartbeat plain = roundTrip(false, false, flags);
        assertFalse(plain.livelinessFlag);
        assertEquals(0x01, flags[0]);
    }
}
