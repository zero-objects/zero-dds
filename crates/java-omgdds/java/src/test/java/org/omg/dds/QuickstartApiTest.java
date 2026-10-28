// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.ServiceConfigurationException;
import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.type.TypeKind;
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
    void setUp() throws Exception {
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() throws Exception {
        InProcessBus.instance().reset();
    }

    @Test
    void serviceEnvironmentBootstrapBySystemProperty() throws Exception {
        // §7.3.1: the implementation class is named by the system property
        // org.omg.dds.serviceClassName and loaded reflectively.
        String prop = ServiceEnvironment.IMPLEMENTATION_CLASS_NAME_PROPERTY;
        String prev = System.getProperty(prop);
        System.setProperty(prop, "org.zerodds.ServiceEnvironmentImpl");
        ServiceEnvironment env =
                ServiceEnvironment.createInstance(QuickstartApiTest.class.getClassLoader());
        try {
            assertNotNull(env);
            // §7.4.1: the participant factory is a per-ServiceEnvironment
            // singleton, and two environments do not share one.
            assertSame(DomainParticipantFactory.getInstance(env),
                    DomainParticipantFactory.getInstance(env));
            assertNotSame(Qos.factory(),
                    DomainParticipantFactory.getInstance(env));
        } finally {
            if (prev == null) {
                System.clearProperty(prop);
            } else {
                System.setProperty(prop, prev);
            }
        }
    }

    @Test
    void serviceEnvironmentCreatesTheStatusAndConditionFactories() throws Exception {
        // §7.3.1: the environment is the root of every object it creates.
        System.setProperty(ServiceEnvironment.IMPLEMENTATION_CLASS_NAME_PROPERTY,
                "org.zerodds.ServiceEnvironmentImpl");
        ServiceEnvironment env =
                ServiceEnvironment.createInstance(QuickstartApiTest.class.getClassLoader());
        assertNotNull(env.getSPI().newWaitSet());
        assertNotNull(env.getSPI().newGuardCondition());
        assertNotNull(env.getSPI().nilHandle());
        assertTrue(env.getSPI().infiniteDuration().isInfinite());
        assertTrue(env.getSPI().zeroDuration().isZero());
        assertNotNull(env.getSPI().getPolicyFactory().Reliability());
        assertNotNull(env.getSPI().getTypeFactory().getPrimitiveType(TypeKind.INT_32_TYPE));
        assertEquals(13, env.getSPI().allStatusKinds().size());
        assertTrue(env.getSPI().noStatusKinds().isEmpty());
    }

    @Test
    void serviceEnvironmentRejectsUnknownClass() throws Exception {
        String prop = "org.omg.dds.brokenServiceClassName";
        System.setProperty(prop, "does.not.Exist");
        try {
            assertThrows(ServiceConfigurationException.class,
                    () -> ServiceEnvironment.createInstance(prop, null,
                            QuickstartApiTest.class.getClassLoader()));
        } finally {
            System.clearProperty(prop);
        }
    }

    @Test
    void getInstanceRejectsNullEnvironment() throws Exception {
        // §7.4.1 names the environment the factory belongs to, and the
        // normative getInstance(env) of the OMG source reads env.getSPI() straight
        // away, so a null environment fails with a NullPointerException.
        assertThrows(NullPointerException.class,
                () -> DomainParticipantFactory.getInstance(null));
    }

    @Test
    void byteArrayQuickstartRoundTrip() throws Exception {
        ServiceEnvironment env =
                ServiceEnvironment.createInstance(QuickstartApiTest.class.getClassLoader());
        DomainParticipant participant =
                DomainParticipantFactory.getInstance(env).createParticipant(10);

        Topic<byte[]> topic = participant.createTopic("Chatter", byte[].class);
        DataWriter<byte[]> writer = participant.createPublisher().createDataWriter(topic);
        DataReader<byte[]> reader = participant.createSubscriber().createDataReader(topic);

        writer.write("hello".getBytes());

        List<Sample<byte[]>> samples = Qos.take(reader);
        assertEquals(1, samples.size());
        assertEquals("hello", new String(samples.get(0).getData()));
    }

    @Test
    void largeByteArrayGrowsEncoderBuffer() throws Exception {
        DomainParticipant participant =
                Qos.factory().createParticipant(11);
        Topic<byte[]> topic = participant.createTopic("BigBytes", byte[].class);
        DataWriter<byte[]> writer = participant.createPublisher().createDataWriter(topic);
        DataReader<byte[]> reader = participant.createSubscriber().createDataReader(topic);

        byte[] payload = new byte[4096]; // larger than the 256-byte initial buffer.
        for (int i = 0; i < payload.length; i++) {
            payload[i] = (byte) i;
        }
        writer.write(payload);

        List<Sample<byte[]>> samples = Qos.take(reader);
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
    void singleArgFactoryReflectsTypedBean() throws Exception {
        DomainParticipant participant =
                Qos.factory().createParticipant(12);
        Topic<Temperature> topic = participant.createTopic("Temp", Temperature.class);
        DataWriter<Temperature> writer = participant.createPublisher().createDataWriter(topic);
        DataReader<Temperature> reader = participant.createSubscriber().createDataReader(topic);

        Temperature t = new Temperature();
        t.setCelsius(23);
        t.setSensorId("A7");
        writer.write(t);

        List<Sample<Temperature>> samples = Qos.take(reader);
        assertEquals(1, samples.size());
        assertEquals(23, samples.get(0).getData().getCelsius());
        assertEquals("A7", samples.get(0).getData().getSensorId());
    }

    @Test
    void getDataAliasesData() throws Exception {
        DomainParticipant participant =
                Qos.factory().createParticipant(13);
        Topic<byte[]> topic = participant.createTopic("Alias", byte[].class);
        DataWriter<byte[]> writer = participant.createPublisher().createDataWriter(topic);
        DataReader<byte[]> reader = participant.createSubscriber().createDataReader(topic);

        writer.write(new byte[] {1, 2, 3});
        Sample<byte[]> s = Qos.take(reader).get(0);
        assertArrayEquals(s.getData(), s.getData());
    }
}
