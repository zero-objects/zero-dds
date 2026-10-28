// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;

import org.junit.jupiter.api.Test;

/**
 * DDS-RPC 1.0 §7.6.2.2.1, Figure 7.3: a service holds back the requests of a client until its
 * reply writer has discovered the client's reply reader, and "when the service-side DataWriter
 * completes the discovery of a client DataReader, the service DataReader is ready to accept
 * requests from that specific client". Discovery completes without a sample arriving, so a replier
 * whose listener is called on arrival only (§7.9.2.2 way 1) kept a held request for good: measured
 * with {@code crates/rpc-rust-e2e/tests/java_peer.rs}, a Rust client of {@code Base2} against the
 * Java service of {@code Derived2}, where the reply writer matched the Rust reader after the
 * request had come and the client timed out.
 */
class ReplierClientMatchingTest {

    private static final class Wires {
        final List<TakenSample> requests = new ArrayList<TakenSample>();
        final List<byte[]> replies = new CopyOnWriteArrayList<byte[]>();
        volatile Runnable onRequest;
        volatile int replyReaders;
    }

    private static final TypeSupport<byte[]> OCTETS = new TypeSupport<byte[]>() {
        @Override
        public byte[] serialize(byte[] value) {
            return value.clone();
        }

        @Override
        public byte[] deserialize(byte[] octets, HeaderCodec.Cursor cursor) {
            return octets.clone();
        }
    };

    /** A binding whose reply writer is matched when the test says so. */
    private static DomainParticipant participant(final Wires wires) {
        return new DomainParticipant() {
            @Override
            public DataWriter createWriter(String topic, String typeName, RpcQos qos, String instance) {
                final byte[] guid = new byte[16];
                guid[15] = 3;
                return new DataWriter() {
                    private long sequence;

                    @Override
                    public SampleIdentity write(byte[] sample) {
                        return write(sample, SampleIdentity.UNKNOWN);
                    }

                    @Override
                    public SampleIdentity write(byte[] sample, SampleIdentity related) {
                        if (wires.replyReaders > 0) {
                            wires.replies.add(sample);
                        }
                        return new SampleIdentity(guid, ++sequence);
                    }

                    @Override
                    public boolean carriesRelatedIdentity() {
                        return true;
                    }

                    @Override
                    public SampleIdentity nextIdentity() {
                        return new SampleIdentity(guid, sequence + 1);
                    }

                    @Override
                    public int matchedCount() {
                        return wires.replyReaders;
                    }
                };
            }

            @Override
            public DataReader createReader(String topic, String typeName, RpcQos qos, String instance) {
                return new DataReader() {
                    @Override
                    public List<TakenSample> take() {
                        synchronized (wires.requests) {
                            List<TakenSample> out = new ArrayList<TakenSample>(wires.requests);
                            wires.requests.clear();
                            return out;
                        }
                    }

                    @Override
                    public int matchedCount() {
                        return 1;
                    }

                    @Override
                    public void setDataAvailableListener(Runnable listener) {
                        wires.onRequest = listener;
                    }
                };
            }

            @Override
            public void deleteWriter(DataWriter writer) {
            }

            @Override
            public void deleteReader(DataReader reader) {
            }
        };
    }

    @Test
    void aRequestHeldForItsClientIsAnsweredWhenTheClientBecomesMatched() throws Exception {
        Wires wires = new Wires();
        DomainParticipant participant = participant(wires);
        ReplierParams params = ReplierParams.create()
                .domainParticipant(participant)
                .mapping(ServiceMapping.ENHANCED)
                .requestTypeSupport(OCTETS)
                .replyTypeSupport(OCTETS)
                .replierListener(new ReplierListener<byte[], byte[]>() {
                    @Override
                    public void onRequestAvailable(Replier<byte[], byte[]> replier) {
                        for (Sample<byte[]> request : replier.receiveRequests()) {
                            replier.sendReply(request.getData(), request.identity());
                        }
                    }
                });
        Replier<byte[], byte[]> replier = RPCRuntime.over(participant).createReplier(params);
        try {
            byte[] client = new byte[16];
            client[15] = (byte) 0xc3;
            synchronized (wires.requests) {
                wires.requests.add(new TakenSample(new byte[] {7}, new SampleIdentity(client, 1)));
            }
            wires.onRequest.run();
            Thread.sleep(200);
            assertTrue(wires.replies.isEmpty(),
                    "the request is held while the reply writer has not discovered the client");

            // Discovery completes the client matching; no further sample arrives.
            wires.replyReaders = 1;
            long end = System.currentTimeMillis() + 3_000;
            while (wires.replies.isEmpty() && System.currentTimeMillis() < end) {
                Thread.sleep(10);
            }
            assertEquals(1, wires.replies.size(), "the held request is answered once the client is matched");
            assertTrue(Arrays.equals(new byte[] {7}, wires.replies.get(0)));
        } finally {
            replier.close();
        }
    }
}
