// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
package org.zerodds.bridge;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assumptions.assumeTrue;

import java.nio.charset.StandardCharsets;
import org.junit.jupiter.api.Test;

/**
 * Java↔Rust multi-process e2e for the gRPC DDS bridge (F2b).
 *
 * <p>Requires a running {@code zerodds-grpc-bridged --features dds-runtime}
 * with topic {@code Demo::Bytes=DemoStream}. The orchestration script
 * {@code run_grpc_bridge_e2e.sh} builds + starts the daemon and passes its
 * port via the {@code ZERODDS_BRIDGE_PORT} env var; without it the test is
 * skipped (so the unit-test suite stays green without the Rust toolchain).
 */
final class GrpcBridgeClientTest {

    private static final String SERVICE = "/zerodds.bridge.v1.DemoStream";

    private static Integer bridgePort() {
        String env = System.getenv("ZERODDS_BRIDGE_PORT");
        if (env == null || env.isEmpty()) {
            env = System.getProperty("zerodds.bridge.port", "");
        }
        if (env.isEmpty()) {
            return null;
        }
        try {
            return Integer.valueOf(env.trim());
        } catch (NumberFormatException e) {
            return null;
        }
    }

    @Test
    void publishThenSubscribeRoundtripThroughDds() throws Exception {
        Integer port = bridgePort();
        assumeTrue(port != null, "ZERODDS_BRIDGE_PORT not set — bridge daemon not running");

        GrpcBridgeClient client = new GrpcBridgeClient("127.0.0.1", port);
        byte[] msg = "hallo-aus-java".getBytes(StandardCharsets.UTF_8);

        // Ingress: Java → gRPC Publish → DDS DataWriter.
        long accepted = client.publish(SERVICE, msg);
        assertEquals(1L, accepted, "PublishAck.accepted should be 1 (DDS write ok)");

        // Egress: DDS DataReader (loopback) → gRPC Subscribe → Java.
        byte[] got = client.subscribeBlocking(SERVICE, System.currentTimeMillis() + 8000);
        assertArrayEquals(msg, got, "Subscribe should return the published payload through real DDS");
    }

    /** Pure-codec self-check that runs without the daemon. */
    @Test
    void protobufSampleRoundtrips() {
        byte[] payload = "xcdr2-bytes".getBytes(StandardCharsets.UTF_8);
        byte[] sample = GrpcBridgeClient.protoSample(payload);
        assertArrayEquals(payload, GrpcBridgeClient.decodeSamplePayload(sample));
    }

    /** PublishAck varint decode. */
    @Test
    void publishAckDecodes() {
        // PublishAck{accepted=1} = 0x08 0x01.
        assertEquals(1L, GrpcBridgeClient.decodePublishAck(new byte[] {0x08, 0x01}));
        // accepted=300 = 0x08 0xAC 0x02 (varint).
        assertEquals(300L, GrpcBridgeClient.decodePublishAck(new byte[] {0x08, (byte) 0xAC, 0x02}));
    }
}
