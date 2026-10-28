// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.ImmutablePolicyException;
import org.omg.dds.core.InconsistentPolicyException;
import org.omg.dds.core.PreconditionNotMetException;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.DataWriterQos;
import org.omg.dds.pub.Publisher;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.DataReaderQos;
import org.omg.dds.sub.Subscriber;
import org.omg.dds.topic.ContentFilteredTopic;
import org.omg.dds.topic.Topic;
import org.omg.dds.topic.TopicDescription;
import org.omg.dds.topic.TopicQos;
import org.zerodds.cdr.ByteArrayTypeSupport;
import org.zerodds.domain.DomainParticipantImpl;
import org.zerodds.internal.InProcessBus;
import org.zerodds.pub.DataWriterImpl;
import org.zerodds.sub.SubscriberImpl;

import java.lang.reflect.Proxy;
import java.util.Collections;
import java.util.concurrent.TimeUnit;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertThrows;

/**
 * DDS-Java-PSM 1.0 Table 7.1 and DDS 1.4 §2.2.3: RETCODE_IMMUTABLE_POLICY,
 * RETCODE_INCONSISTENT_POLICY, RETCODE_UNSUPPORTED and RETCODE_PRECONDITION_NOT_MET
 * as the exceptions of the entities.
 */
class CoreQosRulesTest {

    private static final int DOMAIN = 1505;

    private DomainParticipant participant;
    private Topic<byte[]> topic;
    private Publisher publisher;
    private Subscriber subscriber;
    private DataWriter<byte[]> writer;
    private DataReader<byte[]> reader;

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
        participant = Qos.factory().createParticipant(DOMAIN);
        topic = participant.createTopic("CoreQosRules", byte[].class);
        publisher = participant.createPublisher();
        subscriber = participant.createSubscriber();
        writer = publisher.createDataWriter(topic);
        reader = subscriber.createDataReader(topic);
    }

    @AfterEach
    void tearDown() {
        participant.close();
        InProcessBus.instance().reset();
    }

    // ---- RETCODE_IMMUTABLE_POLICY -------------------------------------------------

    @Test
    void anEnabledWriterRefusesToChangeAnImmutablePolicy() {
        DataWriterQos before = writer.getQos();
        // History and Ownership are not changeable (DDS 1.4 §2.2.3, Changeable = No)
        assertThrows(ImmutablePolicyException.class,
                () -> writer.setQos(before.withPolicy(Qos.pf().History().withKeepAll())));
        assertThrows(ImmutablePolicyException.class,
                () -> writer.setQos(before.withPolicy(Qos.pf().Ownership().withExclusive())));
        assertEquals(before, writer.getQos());
    }

    @Test
    void anEnabledWriterAcceptsTheChangeOfAChangeablePolicy() {
        DataWriterQos changed = writer.getQos()
                .withPolicy(Qos.pf().TransportPriority().withValue(7))
                .withPolicy(Qos.pf().UserData().withValue(new byte[] {1, 2}, 0, 2));
        writer.setQos(changed);
        assertEquals(7, writer.getQos().getTransportPriority().getValue());
        assertArrayEquals(new byte[] {1, 2}, writer.getQos().getUserData().getValue());
        // the same QoS again is not a change
        writer.setQos(writer.getQos());
    }

    @Test
    void anEnabledReaderRefusesImmutableAndAcceptsChangeablePolicies() {
        DataReaderQos before = reader.getQos();
        assertThrows(ImmutablePolicyException.class,
                () -> reader.setQos(before.withPolicy(Qos.pf().Ownership().withExclusive())));
        assertThrows(ImmutablePolicyException.class,
                () -> reader.setQos(before.withPolicy(Qos.pf().History().withKeepAll())));
        assertEquals(before, reader.getQos());
        reader.setQos(before.withPolicy(Qos.pf().TimeBasedFilter().withMinimumSeparation(1, TimeUnit.SECONDS)));
        assertEquals(1, reader.getQos().getTimeBasedFilter().getMinimumSeparation().getDuration(TimeUnit.SECONDS));
    }

    @Test
    void anEnabledTopicRefusesImmutableAndAcceptsChangeablePolicies() {
        TopicQos before = topic.getQos();
        assertThrows(ImmutablePolicyException.class,
                () -> topic.setQos(before.withPolicy(Qos.pf().History().withKeepAll())));
        assertEquals(before, topic.getQos());
        topic.setQos(before.withPolicy(Qos.pf().TransportPriority().withValue(4)));
        assertEquals(4, topic.getQos().getTransportPriority().getValue());
    }

    @Test
    void anEntityThatIsNotEnabledAcceptsAnyChange() {
        DataWriterImpl<byte[]> disabled = new DataWriterImpl<>(DOMAIN, null, topic,
                ByteArrayTypeSupport.INSTANCE, Qos.writer(), Collections.<String>emptySet());
        DataWriterQos changed = Qos.writer().withPolicy(Qos.pf().History().withKeepAll());
        disabled.setQos(changed);
        assertEquals(changed, disabled.getQos());
        disabled.close(); // it belongs to no Publisher and holds the Topic
    }

    // ---- RETCODE_INCONSISTENT_POLICY ---------------------------------------------

    private static DataWriterQos writerQos(int depth, int maxSamples, int maxPerInstance) {
        return Qos.writer()
                .withPolicy(Qos.pf().History().withKeepLast(depth))
                .withPolicy(Qos.pf().ResourceLimits().withMaxSamples(maxSamples)
                        .withMaxSamplesPerInstance(maxPerInstance));
    }

    @Test
    void aHistoryDepthAboveMaxSamplesPerInstanceIsInconsistent() {
        DataWriterQos before = writer.getQos();
        DataWriterQos inconsistent = writerQos(10, 100, 5);
        assertThrows(InconsistentPolicyException.class, () -> publisher.createDataWriter(topic, inconsistent));
        assertThrows(InconsistentPolicyException.class, () -> writer.setQos(inconsistent));
        // the refused QoS did not replace the QoS of the writer
        assertEquals(before, writer.getQos());
        // the depth of 5 against a limit of 5 is consistent
        assertNotNull(publisher.createDataWriter(topic, writerQos(5, 100, 5)));
    }

    @Test
    void maxSamplesBelowMaxSamplesPerInstanceIsInconsistent() {
        DataWriterQos inconsistent = writerQos(1, 4, 5);
        assertThrows(InconsistentPolicyException.class, () -> publisher.createDataWriter(topic, inconsistent));
        DataReaderQos readerQos = Qos.reader().withPolicy(Qos.pf().ResourceLimits()
                .withMaxSamples(4).withMaxSamplesPerInstance(5));
        assertThrows(InconsistentPolicyException.class, () -> subscriber.createDataReader(topic, readerQos));
        assertNotNull(publisher.createDataWriter(topic, writerQos(1, 5, 5)));
    }

    @Test
    void aMinimumSeparationAboveTheDeadlinePeriodIsInconsistent() {
        DataReaderQos inconsistent = Qos.reader()
                .withPolicy(Qos.pf().Deadline().withPeriod(2, TimeUnit.SECONDS))
                .withPolicy(Qos.pf().TimeBasedFilter().withMinimumSeparation(3, TimeUnit.SECONDS));
        assertThrows(InconsistentPolicyException.class, () -> subscriber.createDataReader(topic, inconsistent));
        assertThrows(InconsistentPolicyException.class, () -> reader.setQos(inconsistent));
        DataReaderQos equal = Qos.reader()
                .withPolicy(Qos.pf().Deadline().withPeriod(2, TimeUnit.SECONDS))
                .withPolicy(Qos.pf().TimeBasedFilter().withMinimumSeparation(2, TimeUnit.SECONDS));
        assertNotNull(subscriber.createDataReader(topic, equal));
    }

    @Test
    void aTopicWithAnInconsistentQosIsRefused() {
        TopicQos inconsistent = Qos.topic()
                .withPolicy(Qos.pf().History().withKeepLast(10))
                .withPolicy(Qos.pf().ResourceLimits().withMaxSamplesPerInstance(5));
        assertThrows(InconsistentPolicyException.class,
                () -> ((DomainParticipantImpl) participant).createTopic("CoreQosInconsistent",
                        byte[].class, inconsistent));
    }

    // ---- RETCODE_UNSUPPORTED -------------------------------------------------------

    @SuppressWarnings("unchecked")
    private static <T> T foreign(Class<T> type) {
        return (T) Proxy.newProxyInstance(type.getClassLoader(), new Class<?>[] {type},
                (proxy, method, args) -> null);
    }

    @Test
    void aTopicOfAnotherImplementationIsUnsupported() {
        Topic<byte[]> other = foreign(Topic.class);
        assertThrows(UnsupportedOperationException.class, () -> publisher.createDataWriter(other));
    }

    @Test
    void aTopicDescriptionOfAnotherImplementationIsUnsupported() {
        TopicDescription<byte[]> other = foreign(TopicDescription.class);
        assertThrows(UnsupportedOperationException.class, () -> subscriber.createDataReader(other));
        ContentFilteredTopic<byte[]> filtered = foreign(ContentFilteredTopic.class);
        assertThrows(UnsupportedOperationException.class,
                () -> ((SubscriberImpl) subscriber).createDataReader(filtered, ByteArrayTypeSupport.INSTANCE));
    }

    // ---- RETCODE_PRECONDITION_NOT_MET of the publisher and the subscriber ---------

    @Test
    void resumePublicationsHasToMatchASuspend() {
        assertThrows(PreconditionNotMetException.class, publisher::resumePublications);
        publisher.suspendPublications();
        publisher.suspendPublications();
        publisher.resumePublications();
        publisher.resumePublications();
        assertThrows(PreconditionNotMetException.class, publisher::resumePublications);
    }

    @Test
    void endCoherentChangesHasToMatchABeginAndTheCallsNest() {
        assertThrows(PreconditionNotMetException.class, publisher::endCoherentChanges);
        publisher.beginCoherentChanges();
        publisher.beginCoherentChanges();
        publisher.endCoherentChanges();
        publisher.endCoherentChanges();
        assertThrows(PreconditionNotMetException.class, publisher::endCoherentChanges);
    }

    @Test
    void endAccessHasToMatchABeginAccess() {
        assertThrows(PreconditionNotMetException.class, subscriber::endAccess);
        subscriber.beginAccess();
        subscriber.endAccess();
        assertThrows(PreconditionNotMetException.class, subscriber::endAccess);
    }
}
