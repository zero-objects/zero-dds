// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
package org.zerodds.bridge;

import org.junit.jupiter.api.Tag;
import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assumptions.assumeTrue;

import java.nio.charset.StandardCharsets;
import org.junit.jupiter.api.Test;

/**
 * Java↔Rust multi-process e2e for the gRPC DDS bridge (F2b).
 *
 * <p>Requires a running {@code zerodds-grpc-bridged --features dds-runtime}
 * with topic {@code Demo::Bytes} as {@code DemoStream}, TRANSIENT_LOCAL: the
 * test publishes first, and the DataReader its Subscribe stream gets
 * (zerodds-grpc-bridge-1.0 §4.3) receives the sample as a late joiner.
 * {@code tests/integration/with_grpc_bridge.py} (called by
 * {@code run_grpc_bridge_e2e.sh} and the weekly job {@code integration-java-psm}
 * of ci/integration_scenarios.py) starts the daemon and passes its port via the
 * {@code ZERODDS_BRIDGE_PORT} env var. The test carries the tag
 * {@code bridge-daemon}: the merge job {@code merge-java-omgdds}, which starts no
 * daemon, leaves it out; without the port it is skipped, and ci/check_skips.py
 * fails the job on that skip.
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
    @Tag("bridge-daemon")
    void javaomgdds_4_2_a_sample_published_through_the_grpc_bridge_comes_back_through_dds() throws Exception {
        Integer port = bridgePort();
        assumeTrue(port != null, "ZERODDS_BRIDGE_PORT not set — bridge daemon not running");

        GrpcBridgeClient client = new GrpcBridgeClient("127.0.0.1", port);
        // Sample.cdr_payload: the encapsulation header (XCDR1, little
        // endian) and the CDR bytes (zerodds-grpc-bridge-1.0 §4.4).
        byte[] text = "hallo-aus-java".getBytes(StandardCharsets.UTF_8);
        byte[] msg = new byte[4 + text.length];
        msg[1] = 0x01;
        System.arraycopy(text, 0, msg, 4, text.length);

        // Ingress: Java → gRPC Publish → DDS DataWriter.
        long accepted = client.publish(SERVICE, msg);
        assertEquals(1L, accepted, "PublishAck.accepted should be 1 (DDS write ok)");

        // Egress: DDS DataReader (loopback) → gRPC Subscribe → Java.
        byte[] got = client.subscribeBlocking(SERVICE, System.currentTimeMillis() + 8000);
        assertArrayEquals(msg, got, "Subscribe should return the published payload through real DDS");
    }

    /** PublishAck varint decode. */
    @Test
    void javaomgdds_4_2_a_publish_ack_is_read_from_its_protobuf_varint() {
        // PublishAck{accepted=1} = 0x08 0x01.
        assertEquals(1L, GrpcBridgeClient.decodePublishAck(new byte[] {0x08, 0x01}));
        // accepted=300 = 0x08 0xAC 0x02 (varint).
        assertEquals(300L, GrpcBridgeClient.decodePublishAck(new byte[] {0x08, (byte) 0xAC, 0x02}));
    }
}
