// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.DDSException;
import org.omg.dds.core.DDSObject;
import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.Publisher;
import org.omg.dds.topic.Topic;
import org.zerodds.cdr.TopicTypeSupport;
import org.zerodds.cdr.XcdrException;
import org.zerodds.internal.InProcessBus;
import org.zerodds.pub.DataWriterImpl;

import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.ObjectInputStream;
import java.io.ObjectOutputStream;
import java.io.ObjectStreamClass;
import java.lang.reflect.Modifier;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * DDS-Java-PSM 1.0 §7.3.2 and Table 7.1: the exception classes of the OMG API
 * are abstract, live in org.omg.dds.core and derive from DDSException, which
 * is a DDSObject; the service provides a concrete class for each of them,
 * RETCODE_ERROR included.
 */
class CoreExceptionsTest {

    private static final int DOMAIN = 1501;

    private DomainParticipant participant;

    @BeforeEach
    void setUp() throws Exception {
        InProcessBus.instance().reset();
        participant = Qos.factory().createParticipant(DOMAIN);
    }

    @AfterEach
    void tearDown() throws Exception {
        participant.close();
        InProcessBus.instance().reset();
    }

    /** The Table 7.1 classes with the serialVersionUID of the OMG source. */
    private static final Object[][] TABLE_7_1 = {
        {org.omg.dds.core.AlreadyClosedException.class, 8818392556145975401L, org.zerodds.core.AlreadyClosedException.class},
        {org.omg.dds.core.IllegalOperationException.class, -6782657920034098505L, org.zerodds.core.IllegalOperationException.class},
        {org.omg.dds.core.ImmutablePolicyException.class, -7676868650200410524L, org.zerodds.core.ImmutablePolicyException.class},
        {org.omg.dds.core.InconsistentPolicyException.class, 5264713016409453514L, org.zerodds.core.InconsistentPolicyException.class},
        {org.omg.dds.core.NotEnabledException.class, -1435103296817695008L, org.zerodds.core.NotEnabledException.class},
        {org.omg.dds.core.OutOfResourcesException.class, 6105570967607779945L, org.zerodds.core.OutOfResourcesException.class},
        {org.omg.dds.core.PreconditionNotMetException.class, -6204457556635480597L, org.zerodds.core.PreconditionNotMetException.class},
    };

    @Test
    void everyTable71ExceptionIsAnAbstractDdsExceptionOfOrgOmgDdsCore() throws Exception {
        assertTrue(Modifier.isAbstract(DDSException.class.getModifiers()));
        assertEquals("org.omg.dds.core", DDSException.class.getPackage().getName());
        for (Object[] row : TABLE_7_1) {
            Class<?> omg = (Class<?>) row[0];
            assertTrue(Modifier.isAbstract(omg.getModifiers()), omg + " is abstract");
            assertEquals("org.omg.dds.core", omg.getPackage().getName());
            assertSame(DDSException.class, omg.getSuperclass(), omg + " extends DDSException");
        }
    }

    @Test
    void ddsExceptionIsADdsObjectAndUnchecked() throws Exception {
        assertTrue(DDSObject.class.isAssignableFrom(DDSException.class));
        assertTrue(RuntimeException.class.isAssignableFrom(DDSException.class));
    }

    @Test
    void theExceptionsKeepTheSerialVersionUidOfTheOmgSource() throws Exception {
        for (Object[] row : TABLE_7_1) {
            Class<?> omg = (Class<?>) row[0];
            assertEquals(((Long) row[1]).longValue(), ObjectStreamClass.lookup(omg).getSerialVersionUID(),
                    omg.getName());
        }
        assertEquals(-7560788483511488746L, ObjectStreamClass.lookup(DDSException.class).getSerialVersionUID());
        assertEquals(-3247742026162310484L,
                ObjectStreamClass.lookup(org.omg.dds.core.ServiceConfigurationException.class).getSerialVersionUID());
        assertEquals(323168673886924537L,
                ObjectStreamClass.lookup(org.omg.dds.core.ServiceInitializationException.class).getSerialVersionUID());
    }

    @Test
    void onlyTimeoutExceptionIsChecked() throws Exception {
        for (Object[] row : TABLE_7_1) {
            assertTrue(RuntimeException.class.isAssignableFrom((Class<?>) row[0]), row[0].toString());
        }
        assertTrue(RuntimeException.class.isAssignableFrom(org.omg.dds.core.ServiceConfigurationException.class));
        assertTrue(RuntimeException.class.isAssignableFrom(org.omg.dds.core.ServiceInitializationException.class));
        Class<?> timeout = java.util.concurrent.TimeoutException.class;
        assertTrue(!RuntimeException.class.isAssignableFrom(timeout));
        // the blocking operations declare it
        assertTrue(java.util.Arrays.asList(org.omg.dds.core.WaitSet.class.getMethod("waitForConditions",
                java.util.Collection.class, org.omg.dds.core.Duration.class).getExceptionTypes()).contains(timeout));
        assertTrue(java.util.Arrays.asList(DataWriter.class.getMethod("write", Object.class)
                .getExceptionTypes()).contains(timeout));
        assertTrue(java.util.Arrays.asList(DomainParticipant.class.getMethod("findTopic", String.class,
                org.omg.dds.core.Duration.class).getExceptionTypes()).contains(timeout));
    }

    @Test
    void theServiceProvidesAConcreteClassForEveryTable71ExceptionAndForRetcodeError() throws Exception {
        for (Object[] row : TABLE_7_1) {
            Class<?> omg = (Class<?>) row[0];
            Class<?> concrete = (Class<?>) row[2];
            assertTrue(omg.isAssignableFrom(concrete), concrete + " implements " + omg);
            assertTrue(!Modifier.isAbstract(concrete.getModifiers()), concrete + " is concrete");
            assertEquals("org.zerodds.core", concrete.getPackage().getName());
        }
        // RETCODE_ERROR is DDSException itself, which is abstract
        assertSame(DDSException.class, org.zerodds.core.ErrorException.class.getSuperclass());
        assertTrue(!Modifier.isAbstract(org.zerodds.core.ErrorException.class.getModifiers()));
    }

    @Test
    void anExceptionBelongsToTheEnvironmentItWasCreatedIn() throws Exception {
        ServiceEnvironment env = Qos.env();
        assertSame(env, new org.zerodds.core.AlreadyClosedException().getEnvironment());
        assertSame(env, new org.zerodds.core.ErrorException("x").getEnvironment());
        ServiceEnvironment other = new org.zerodds.ServiceEnvironmentImpl();
        DDSObject[] given = {
            new org.zerodds.core.AlreadyClosedException(other, "closed"),
            new org.zerodds.core.IllegalOperationException(other, "illegal"),
            new org.zerodds.core.ImmutablePolicyException(other, "immutable"),
            new org.zerodds.core.InconsistentPolicyException(other, "inconsistent"),
            new org.zerodds.core.NotEnabledException(other, "not enabled"),
            new org.zerodds.core.OutOfResourcesException(other, "resources"),
            new org.zerodds.core.PreconditionNotMetException(other, "precondition"),
            new org.zerodds.core.ErrorException(other, "error"),
        };
        for (DDSObject e : given) {
            assertSame(other, e.getEnvironment(), e.getClass().getName());
        }
    }

    @Test
    void anExceptionIsSerializableWithMessageAndCauseAndGetsTheDefaultEnvironment() throws Exception {
        Throwable cause = new IllegalStateException("root");
        org.zerodds.core.ErrorException original = new org.zerodds.core.ErrorException(
                new org.zerodds.ServiceEnvironmentImpl(), "failed", cause);
        ByteArrayOutputStream bytes = new ByteArrayOutputStream();
        try (ObjectOutputStream out = new ObjectOutputStream(bytes)) {
            out.writeObject(original);
        }
        DDSException copy;
        try (ObjectInputStream in = new ObjectInputStream(new ByteArrayInputStream(bytes.toByteArray()))) {
            copy = (DDSException) in.readObject();
        }
        assertEquals("failed", copy.getMessage());
        assertEquals("root", copy.getCause().getMessage());
        assertSame(Qos.env(), copy.getEnvironment());
    }

    // ---- RETCODE_ERROR and RETCODE_OUT_OF_RESOURCES raised by a writer --------

    private static TopicTypeSupport<byte[]> support(Qos.Writer<byte[]> writer) {
        return Qos.support("CoreExceptionsBytes", writer, buf -> new byte[0]);
    }

    @Test
    void aSampleTheEncoderRejectsFailsWithRetcodeError() throws Exception {
        Topic<byte[]> topic = participant.createTopic("CoreErrorTopic", byte[].class);
        Publisher publisher = participant.createPublisher();
        DataWriter<byte[]> writer = Qos.writer(publisher, topic, support((value, buf) -> {
            throw new XcdrException("member out of range");
        }));
        org.zerodds.core.ErrorException error = assertThrows(org.zerodds.core.ErrorException.class,
                () -> writer.write(new byte[] {1}));
        assertTrue(error.getCause() instanceof XcdrException);
        assertTrue(error.getMessage().contains("member out of range"));
        // the writer is still usable for a sample the encoder accepts
        DataWriter<byte[]> sound = Qos.writer(publisher, topic, support((value, buf) -> buf.put(value)));
        sound.write(new byte[] {1});
    }

    @Test
    void anEncodingBeyondTheLimitFailsWithOutOfResources() throws Exception {
        TopicTypeSupport<byte[]> support = support((value, buf) -> buf.put(value));
        // 1000 octets fit into the 1024-octet limit, 2000 octets do not
        assertEquals(1000, DataWriterImpl.serialize(support, new byte[1000], 1024).length);
        assertThrows(org.zerodds.core.OutOfResourcesException.class,
                () -> DataWriterImpl.serialize(support, new byte[2000], 1024));
        assertEquals(1 << 28, DataWriterImpl.MAX_SAMPLE_BYTES);
    }
}
