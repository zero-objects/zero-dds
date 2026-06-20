// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.domain.DomainParticipantFactory;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.Sample;
import org.omg.dds.topic.Topic;
import org.zerodds.internal.InProcessBus;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Covers the front-page / docs quickstart surface: ServiceEnvironment
 * bootstrap, the {@code getInstance(ServiceEnvironment)} overload, the
 * single-argument {@code createDataWriter/Reader(Topic)} convenience overloads
 * (auto-derived TypeSupport), the built-in {@code byte[]} type and the
 * {@code Sample.getData()} accessor.
 */
class QuickstartApiTest {

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() {
        InProcessBus.instance().reset();
    }

    @Test
    void serviceEnvironmentBootstrapByClassName() {
        ServiceEnvironment env =
                ServiceEnvironment.createInstance("org.zerodds.ServiceEnvironmentImpl");
        assertNotNull(env);
        assertSame(DomainParticipantFactory.getInstance(),
                DomainParticipantFactory.getInstance(env));
    }

    @Test
    void serviceEnvironmentBootstrapBySystemProperty() {
        String prop = ServiceEnvironment.IMPLEMENTATION_CLASS_NAME_PROPERTY;
        String prev = System.getProperty(prop);
        System.setProperty(prop, "org.zerodds.ServiceEnvironmentImpl");
        try {
            assertNotNull(ServiceEnvironment.createInstance(null));
        } finally {
            if (prev == null) {
                System.clearProperty(prop);
            } else {
                System.setProperty(prop, prev);
            }
        }
    }

    @Test
    void serviceEnvironmentRejectsUnknownClass() {
        assertThrows(ServiceEnvironment.ServiceConfigurationException.class,
                () -> ServiceEnvironment.createInstance("does.not.Exist"));
    }

    @Test
    void getInstanceRejectsNullEnvironment() {
        assertThrows(NullPointerException.class,
                () -> DomainParticipantFactory.getInstance(null));
    }

    @Test
    void byteArrayQuickstartRoundTrip() {
        ServiceEnvironment env =
                ServiceEnvironment.createInstance("org.zerodds.ServiceEnvironmentImpl");
        DomainParticipant participant =
                DomainParticipantFactory.getInstance(env).createParticipant(10);

        Topic<byte[]> topic = participant.createTopic("Chatter", byte[].class);
        DataWriter<byte[]> writer = participant.createPublisher().createDataWriter(topic);
        DataReader<byte[]> reader = participant.createSubscriber().createDataReader(topic);

        writer.write("hello".getBytes());

        List<Sample<byte[]>> samples = reader.take();
        assertEquals(1, samples.size());
        assertEquals("hello", new String(samples.get(0).getData()));
    }

    @Test
    void largeByteArrayGrowsEncoderBuffer() {
        DomainParticipant participant =
                DomainParticipantFactory.getInstance().createParticipant(11);
        Topic<byte[]> topic = participant.createTopic("BigBytes", byte[].class);
        DataWriter<byte[]> writer = participant.createPublisher().createDataWriter(topic);
        DataReader<byte[]> reader = participant.createSubscriber().createDataReader(topic);

        byte[] payload = new byte[4096]; // larger than the 256-byte initial buffer.
        for (int i = 0; i < payload.length; i++) {
            payload[i] = (byte) i;
        }
        writer.write(payload);

        List<Sample<byte[]>> samples = reader.take();
        assertEquals(1, samples.size());
        assertArrayEquals(payload, samples.get(0).getData());
    }

    /** Plain bean — single-arg factory must fall back to ReflectionTypeSupport. */
    public static final class Temperature {
        private int celsius;
        private String sensorId;

        public int getCelsius() { return celsius; }
        public void setCelsius(int celsius) { this.celsius = celsius; }

        public String getSensorId() { return sensorId; }
        public void setSensorId(String sensorId) { this.sensorId = sensorId; }
    }

    @Test
    void singleArgFactoryReflectsTypedBean() {
        DomainParticipant participant =
                DomainParticipantFactory.getInstance().createParticipant(12);
        Topic<Temperature> topic = participant.createTopic("Temp", Temperature.class);
        DataWriter<Temperature> writer = participant.createPublisher().createDataWriter(topic);
        DataReader<Temperature> reader = participant.createSubscriber().createDataReader(topic);

        Temperature t = new Temperature();
        t.setCelsius(23);
        t.setSensorId("A7");
        writer.write(t);

        List<Sample<Temperature>> samples = reader.take();
        assertEquals(1, samples.size());
        assertEquals(23, samples.get(0).getData().getCelsius());
        assertEquals("A7", samples.get(0).getData().getSensorId());
    }

    @Test
    void getDataAliasesData() {
        DomainParticipant participant =
                DomainParticipantFactory.getInstance().createParticipant(13);
        Topic<byte[]> topic = participant.createTopic("Alias", byte[].class);
        DataWriter<byte[]> writer = participant.createPublisher().createDataWriter(topic);
        DataReader<byte[]> reader = participant.createSubscriber().createDataReader(topic);

        writer.write(new byte[] {1, 2, 3});
        Sample<byte[]> s = reader.take().get(0);
        assertArrayEquals(s.data(), s.getData());
    }
}
