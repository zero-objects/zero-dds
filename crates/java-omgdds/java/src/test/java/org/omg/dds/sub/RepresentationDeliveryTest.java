// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.sub;

import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.util.concurrent.atomic.AtomicInteger;
import org.junit.jupiter.api.Test;
import org.zerodds.sub.DataReaderImpl;
import org.zerodds.core.Values;
import org.zerodds.topic.TopicImpl;
import org.zerodds.cdr.EndianMode;
import org.zerodds.cdr.ExtensibilityKind;
import org.zerodds.cdr.TopicTypeSupport;
import org.zerodds.cdr.XcdrException;
import org.zerodds.rtps.SerializedSample;
import static org.junit.jupiter.api.Assertions.*;

class RepresentationDeliveryTest {

    /** The wire path of §7.2.3 is package-private; reach it through the impl. */
    private static void deliverWire(DataReaderImpl<Integer> reader,
            org.zerodds.rtps.SerializedSample sample) {
        try {
            java.lang.reflect.Method m =
                    DataReaderImpl.class.getDeclaredMethod("deliverWire",
                            org.zerodds.rtps.SerializedSample.class);
            m.setAccessible(true);
            m.invoke(reader, sample);
        } catch (java.lang.reflect.InvocationTargetException e) {
            // the method's own exception, as a direct call would raise it
            if (e.getCause() instanceof RuntimeException) {
                throw (RuntimeException) e.getCause();
            }
            throw new IllegalStateException(e);
        } catch (ReflectiveOperationException e) {
            throw new IllegalStateException(e);
        }
    }
    static final class Support implements TopicTypeSupport<Integer> {
        int decodes;
        int keys;
        public String getTypeName() { return "TestInteger"; }
        public boolean isKeyed() { return true; }
        public ExtensibilityKind getExtensibility() { return ExtensibilityKind.FINAL; }
        public int[] readRepresentations() { return new int[] {XCDR1, XCDR2}; }
        public byte[] encode(Integer value) { return encode(value, EndianMode.LITTLE_ENDIAN); }
        public byte[] encode(Integer value, EndianMode endian) {
            return ByteBuffer.allocate(4).order(order(endian)).putInt(value).array();
        }
        public Integer decode(byte[] bytes) { throw new AssertionError("Legacy decoder used"); }
        public Integer decode(byte[] bytes, int offset, int length) { throw new AssertionError("Legacy decoder used"); }
        public Integer decode(byte[] bytes, int offset, int length, EndianMode endian, int representation) {
            decodes++;
            if (length != 4) throw new XcdrException("Expected one integer");
            return ByteBuffer.wrap(bytes, offset, length).order(order(endian)).getInt();
        }
        public byte[] keyHash(Integer value) { keys++; return new byte[16]; }
        private static ByteOrder order(EndianMode endian) {
            return endian == EndianMode.LITTLE_ENDIAN ? ByteOrder.LITTLE_ENDIAN : ByteOrder.BIG_ENDIAN;
        }
    }

    @Test void keyedWireDeliveryDecodesOnceAndPreservesEndianAndRepresentation() {
        for (int rep : new int[] {0, 2}) for (EndianMode endian : EndianMode.values()) {
            Support support = new Support();
            AtomicInteger delivered = new AtomicInteger();
            // A content filter sees the exact object queued by the common reader path.
            // No enable call, sockets, network property or shared bus fixture is needed.
            DataReaderImpl<Integer> reader = new DataReaderImpl<>(0, null,
                    new TopicImpl<>(Values.getDefaultEnvironment(), null, "test", Integer.class, org.omg.dds.Qos.topic()),
                    support,
                    org.omg.dds.Qos.reader(), java.util.Set.of(), value -> { assertEquals(0x1020304, value.intValue()); delivered.incrementAndGet(); return true; });
            byte[] payload = new byte[8];
            System.arraycopy(SerializedSample.encapsulation(rep, ExtensibilityKind.FINAL, endian), 0, payload, 0, 4);
            System.arraycopy(support.encode(0x1020304, endian), 0, payload, 4, 4);
            deliverWire(reader, SerializedSample.parse(payload));
            assertEquals(1, support.decodes);
            assertEquals(1, support.keys);
            assertEquals(1, delivered.get());
            assertThrows(XcdrException.class, () -> deliverWire(reader, SerializedSample.parse(new byte[] {0, 7, 0, 0, 1})));
            assertEquals(1, support.keys);
            assertEquals(1, delivered.get());
        }
    }
}
