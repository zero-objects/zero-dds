// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.InstanceHandle;
import org.omg.dds.core.ModifiableInstanceHandle;
import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.domain.DomainParticipantFactory;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.Publisher;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.Sample;
import org.omg.dds.sub.Subscriber;
import org.omg.dds.topic.Topic;
import org.omg.dds.type.TypeSupport;
import org.omg.dds.type.builtin.KeyedBytes;
import org.omg.dds.type.builtin.KeyedString;
import org.zerodds.ServiceEnvironmentImpl;
import org.zerodds.internal.InProcessBus;

import java.lang.reflect.Modifier;
import java.util.ArrayList;
import java.util.Collections;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Set;
import java.util.concurrent.Callable;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.concurrent.TimeUnit;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertNotSame;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * DDS-Java-PSM 1.0 §7.3.1: the ServiceEnvironment is abstract and creates every
 * object through its service-provider interface; every DDSObject answers with
 * the environment it came from; the per-environment factory is a singleton
 * also under concurrent access (§7.2.4).
 */
class CoreSpiTest {

    private static final int DOMAIN = 1503;
    private static final ServiceEnvironment ENV = Qos.env();

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() {
        InProcessBus.instance().reset();
    }

    @Test
    void theServiceEnvironmentIsAbstract() throws Exception {
        assertTrue(Modifier.isAbstract(ServiceEnvironment.class.getModifiers()));
        assertTrue(Modifier.isAbstract(ServiceEnvironment.class.getMethod("getSPI").getModifiers()));
        // the environment of this service is a concrete subclass
        assertFalse(Modifier.isAbstract(ENV.getClass().getModifiers()));
        assertTrue(ServiceEnvironment.class.isAssignableFrom(ENV.getClass()));
    }

    @Test
    void aNewInstanceHandleIsNilAndIsFilledByCopyFrom() {
        ModifiableInstanceHandle handle = ENV.getSPI().newInstanceHandle();
        assertTrue(handle.isNil());
        assertSame(ENV, handle.getEnvironment());
        byte[] octets = new byte[16];
        octets[3] = 42;
        InstanceHandle source = org.zerodds.core.Values.of(ENV, octets);
        handle.copyFrom(source);
        assertFalse(handle.isNil());
        assertEquals(source, handle);
        assertTrue(InstanceHandle.newInstanceHandle(ENV).isNil());
        assertTrue(ENV.getSPI().nilHandle().isNil());
        assertNotSame(ENV.getSPI().newInstanceHandle(), ENV.getSPI().newInstanceHandle());
    }

    @Test
    void theInvalidTimeAndANegativeTimeOfTheSpiAreInvalid() {
        assertFalse(ENV.getSPI().invalidTime().isValid());
        assertFalse(ENV.getSPI().newTime(-5, TimeUnit.MILLISECONDS).isValid());
        assertTrue(ENV.getSPI().newTime(5, TimeUnit.MILLISECONDS).isValid());
        assertSame(ENV, ENV.getSPI().invalidTime().getEnvironment());
    }

    @Test
    void aTypeSupportIsRegisteredUnderTheFullyQualifiedNameOfTheType() {
        // §7.5.1: without a name the type is registered under the fully qualified
        // name of the Class (Class.getName), not under that of the Class object
        TypeSupport<byte[]> byDefault = TypeSupport.newTypeSupport(byte[].class, ENV);
        assertEquals(byte[].class.getName(), byDefault.getTypeName());
        assertNotEquals(Class.class.getName(), byDefault.getTypeName());
        assertSame(byte[].class, byDefault.getType());
        assertSame(ENV, byDefault.getEnvironment());

        TypeSupport<KeyedString> keyed = TypeSupport.newTypeSupport(KeyedString.class, ENV);
        assertEquals("org.omg.dds.type.builtin.KeyedString", keyed.getTypeName());

        TypeSupport<byte[]> named = TypeSupport.newTypeSupport(byte[].class, "CoreSpiNamed", ENV);
        assertEquals("CoreSpiNamed", named.getTypeName());
        TypeSupport<byte[]> viaSpi = ENV.getSPI().newTypeSupport(byte[].class, "CoreSpiViaSpi");
        assertEquals("CoreSpiViaSpi", viaSpi.getTypeName());
    }

    @Test
    void theTypeSupportOfABuiltInKeyedTypeCreatesItsSamples() {
        KeyedString string = TypeSupport.newTypeSupport(KeyedString.class, ENV).newData();
        assertEquals("", string.getKey());
        assertEquals("", string.getValue());
        KeyedBytes bytes = TypeSupport.newTypeSupport(KeyedBytes.class, ENV).newData();
        assertEquals("", bytes.getKey());
        assertEquals(0, bytes.getValue().length);
    }

    @Test
    void keyedStringAndKeyedBytesMutatorsReturnTheObject() {
        KeyedString string = KeyedString.newKeyedString(ENV);
        assertSame(string, string.setKey("k"));
        assertSame(string, string.setKey("k").setValue("v"));
        assertEquals("k", string.getKey());
        assertEquals("v", string.getValue());
        assertSame(ENV, string.getEnvironment());
        KeyedString copy = string.clone();
        assertEquals(string, copy);
        assertNotSame(string, copy);
        copy.setValue("other");
        assertEquals("v", string.getValue());
        KeyedString target = ENV.getSPI().newKeyedString();
        target.copyFrom(string);
        assertEquals(string, target);
        assertEquals(string.hashCode(), target.hashCode());

        KeyedBytes bytes = ENV.getSPI().newKeyedBytes();
        byte[] value = {1, 2, 3, 4};
        assertSame(bytes, bytes.setKey("b"));
        assertSame(bytes, bytes.setKey("b").setValue(value));
        value[0] = 99;
        // the object keeps its own copy and hands out copies
        assertArrayEquals(new byte[] {1, 2, 3, 4}, bytes.getValue());
        bytes.getValue()[1] = 99;
        assertArrayEquals(new byte[] {1, 2, 3, 4}, bytes.getValue());
        bytes.setValue(new byte[] {9, 8, 7, 6, 5}, 1, 3);
        assertArrayEquals(new byte[] {8, 7, 6}, bytes.getValue());
        assertSame(ENV, KeyedBytes.newKeyedBytes(ENV).getEnvironment());
    }

    @Test
    void aQosProviderOfAnUnsupportedUriSchemeIsRefused() {
        assertThrows(UnsupportedOperationException.class,
                () -> ENV.getSPI().newQosProvider("http://example.org/qos.xml", null));
        assertThrows(IllegalArgumentException.class, () -> ENV.getSPI().newQosProvider(null, null));
    }

    @Test
    void theStaticFactoryMethodsOfTheAbstractValueTypesGoThroughTheSpi() {
        // §7.2.3: Time.newTime, Duration.newDuration, InstanceHandle.newInstanceHandle,
        // QosProvider.newQosProvider, TypeSupport.newTypeSupport, KeyedString.newKeyedString
        // and KeyedBytes.newKeyedBytes
        assertEquals(1500, org.omg.dds.core.Time.newTime(1500, TimeUnit.MILLISECONDS, ENV)
                .getTime(TimeUnit.MILLISECONDS));
        org.omg.dds.core.Duration duration = org.omg.dds.core.Duration.newDuration(250, TimeUnit.MILLISECONDS, ENV);
        assertEquals(250, duration.getDuration(TimeUnit.MILLISECONDS));
        assertSame(ENV, duration.getEnvironment());
        assertTrue(org.omg.dds.core.Duration.infiniteDuration(ENV).isInfinite());
        assertTrue(org.omg.dds.core.Duration.zeroDuration(ENV).isZero());
        assertTrue(InstanceHandle.newInstanceHandle(ENV).isNil());
        assertThrows(UnsupportedOperationException.class,
                () -> org.omg.dds.core.QosProvider.newQosProvider("ftp://example.org/qos.xml", "profile", ENV));
        assertSame(ENV, TypeSupport.newTypeSupport(byte[].class, ENV).getEnvironment());
        assertSame(ENV, KeyedString.newKeyedString(ENV).getEnvironment());
        assertSame(ENV, KeyedBytes.newKeyedBytes(ENV).getEnvironment());
    }

    @Test
    void allStatusKindsAndNoStatusKindsAreSets() {
        assertEquals(13, ENV.getSPI().allStatusKinds().size());
        assertTrue(ENV.getSPI().noStatusKinds().isEmpty());
        // the all-set is a copy: changing it does not change the next answer
        ENV.getSPI().allStatusKinds().clear();
        assertEquals(13, ENV.getSPI().allStatusKinds().size());
    }

    @Test
    void severalThreadsGetOneParticipantFactoryOfAnEnvironment() throws Exception {
        final int threads = 8;
        for (int round = 0; round < 20; round++) {
            final ServiceEnvironment env = new ServiceEnvironmentImpl();
            final CountDownLatch start = new CountDownLatch(1);
            ExecutorService pool = Executors.newFixedThreadPool(threads);
            try {
                List<Future<DomainParticipantFactory>> results = new ArrayList<>();
                for (int i = 0; i < threads; i++) {
                    results.add(pool.submit(new Callable<DomainParticipantFactory>() {
                        @Override
                        public DomainParticipantFactory call() throws Exception {
                            start.await();
                            return DomainParticipantFactory.getInstance(env);
                        }
                    }));
                }
                start.countDown();
                Set<DomainParticipantFactory> distinct =
                        Collections.newSetFromMap(new IdentityHashMap<DomainParticipantFactory, Boolean>());
                for (Future<DomainParticipantFactory> result : results) {
                    distinct.add(result.get(60, TimeUnit.SECONDS));
                }
                assertEquals(1, distinct.size(), "round " + round);
                assertSame(distinct.iterator().next(), DomainParticipantFactory.getInstance(env));
            } finally {
                pool.shutdownNow();
            }
        }
    }

    @Test
    void everyEntityAnswersWithTheEnvironmentItWasCreatedIn() throws Exception {
        ServiceEnvironment env = new ServiceEnvironmentImpl();
        assertNotSame(ENV, env);
        DomainParticipantFactory factory = DomainParticipantFactory.getInstance(env);
        assertSame(env, factory.getEnvironment());
        DomainParticipant participant = factory.createParticipant(DOMAIN);
        try {
            Topic<byte[]> topic = participant.createTopic("CoreSpiEnvironment", byte[].class);
            Publisher publisher = participant.createPublisher();
            Subscriber subscriber = participant.createSubscriber();
            DataReader<byte[]> reader = subscriber.createDataReader(topic);
            DataWriter<byte[]> writer = publisher.createDataWriter(topic);

            assertSame(env, participant.getEnvironment());
            assertSame(env, topic.getEnvironment());
            assertSame(env, publisher.getEnvironment());
            assertSame(env, subscriber.getEnvironment());
            assertSame(env, writer.getEnvironment());
            assertSame(env, reader.getEnvironment());

            assertSame(env, participant.getInstanceHandle().getEnvironment());
            assertSame(env, writer.getInstanceHandle().getEnvironment());
            assertSame(env, reader.getInstanceHandle().getEnvironment());
            assertSame(env, writer.getStatusCondition().getEnvironment());
            assertSame(env, reader.getStatusCondition().getEnvironment());
            assertSame(env, participant.getCurrentTime().getEnvironment());

            writer.write(new byte[] {1, 2, 3});
            List<Sample<byte[]>> taken = Qos.take(reader);
            assertEquals(1, taken.size());
            assertSame(env, taken.get(0).getEnvironment());
        } finally {
            participant.close();
        }
    }
}
