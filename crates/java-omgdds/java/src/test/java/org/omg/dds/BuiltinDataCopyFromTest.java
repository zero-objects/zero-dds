// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.Test;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.Publisher;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.Subscriber;
import org.omg.dds.topic.Topic;
import org.zerodds.topic.BuiltinData;
import org.zerodds.topic.BuiltinTopicKeyImpl;
import org.zerodds.topic.BuiltinTopicWire;
import org.zerodds.topic.ParticipantBuiltinTopicDataImpl;
import org.zerodds.topic.PublicationBuiltinTopicDataImpl;
import org.zerodds.topic.SubscriptionBuiltinTopicDataImpl;
import org.zerodds.topic.TopicBuiltinTopicDataImpl;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;

/**
 * DDS-Java-PSM 1.0 7.3.3: {@code copyFrom} overwrites the state of the target with that of the
 * source, for the four types of the built-in topics. The state is compared by its serialization
 * as built-in topic data.
 */
class BuiltinDataCopyFromTest {

    @Test
    void copyFromOverwritesTheWholeStateOfThePublicationData() {
        DomainParticipant participant = Qos.factory().createParticipant(1412);
        Publisher publisher = participant.createPublisher();
        Topic<byte[]> one = participant.createTopic("CopyOne", byte[].class);
        Topic<byte[]> two = participant.createTopic("CopyTwo", byte[].class);
        DataWriter<byte[]> first = publisher.createDataWriter(one, Qos.writer()
                .withPolicy(Qos.pf().OwnershipStrength().withValue(9))
                .withPolicy(Qos.pf().UserData().withValue(new byte[] {1, 2}, 0, 2)));
        DataWriter<byte[]> second = publisher.createDataWriter(two);
        org.omg.dds.topic.BuiltinTopicKey owner = BuiltinTopicKeyImpl.of(Qos.env(),
                participant.getInstanceHandle());
        PublicationBuiltinTopicDataImpl source = BuiltinData.publication(Qos.env(), first, publisher, owner);
        PublicationBuiltinTopicDataImpl target = BuiltinData.publication(Qos.env(), second, publisher, owner);
        target.setTypeObjectBytes(new byte[] {9, 9, 9}); // state only this implementation holds
        assertFalse(java.util.Arrays.equals(BuiltinTopicWire.encode(source), BuiltinTopicWire.encode(target)));

        target.copyFrom(source);

        assertArrayEquals(BuiltinTopicWire.encode(source), BuiltinTopicWire.encode(target));
        assertArrayEquals(source.getTypeObjectBytes(), target.getTypeObjectBytes());
        participant.close();
    }

    @Test
    void copyFromOverwritesTheWholeStateOfTheSubscriptionData() {
        DomainParticipant participant = Qos.factory().createParticipant(1413);
        Subscriber subscriber = participant.createSubscriber();
        Topic<byte[]> one = participant.createTopic("CopySubOne", byte[].class);
        Topic<byte[]> two = participant.createTopic("CopySubTwo", byte[].class);
        DataReader<byte[]> first = subscriber.createDataReader(one, Qos.reader()
                .withPolicy(Qos.pf().UserData().withValue(new byte[] {4}, 0, 1))
                .withPolicy(Qos.pf().Deadline().withPeriod(3, java.util.concurrent.TimeUnit.SECONDS)));
        DataReader<byte[]> second = subscriber.createDataReader(two);
        org.omg.dds.topic.BuiltinTopicKey owner = BuiltinTopicKeyImpl.of(Qos.env(),
                participant.getInstanceHandle());
        SubscriptionBuiltinTopicDataImpl source = BuiltinData.subscription(Qos.env(), first, subscriber, owner);
        SubscriptionBuiltinTopicDataImpl target = BuiltinData.subscription(Qos.env(), second, subscriber, owner);
        target.setTypeObjectBytes(new byte[] {9, 9, 9});
        target.copyFrom(source);
        assertArrayEquals(BuiltinTopicWire.encode(source), BuiltinTopicWire.encode(target));
        participant.close();
    }

    @Test
    void copyFromOverwritesTheWholeStateOfTheTopicData() {
        DomainParticipant participant = Qos.factory().createParticipant(1414);
        Topic<byte[]> one = participant.createTopic("CopyTopicOne", byte[].class,
                Qos.topic().withPolicy(Qos.pf().TransportPriority().withValue(5)), null,
                java.util.Collections.<Class<? extends org.omg.dds.core.status.Status>>emptyList());
        Topic<byte[]> two = participant.createTopic("CopyTopicTwo", byte[].class);
        TopicBuiltinTopicDataImpl source = BuiltinData.topic(Qos.env(), one);
        TopicBuiltinTopicDataImpl target = BuiltinData.topic(Qos.env(), two);
        target.setTypeObjectBytes(new byte[] {9, 9, 9});
        target.copyFrom(source);
        assertArrayEquals(BuiltinTopicWire.encode(source), BuiltinTopicWire.encode(target));
        participant.close();
    }

    @Test
    void copyFromOverwritesTheStateOfTheParticipantData() {
        DomainParticipant first = Qos.factory().createParticipant(1415);
        DomainParticipant second = Qos.factory().createParticipant(1415);
        ParticipantBuiltinTopicDataImpl source = new ParticipantBuiltinTopicDataImpl(Qos.env(),
                BuiltinTopicKeyImpl.of(Qos.env(), first.getInstanceHandle()),
                Qos.pf().UserData().withValue(new byte[] {7, 8}, 0, 2));
        ParticipantBuiltinTopicDataImpl target = new ParticipantBuiltinTopicDataImpl(Qos.env(),
                BuiltinTopicKeyImpl.of(Qos.env(), second.getInstanceHandle()),
                Qos.pf().UserData().withValue(new byte[] {1}, 0, 1));
        target.copyFrom(source);
        assertArrayEquals(BuiltinTopicWire.encode(source), BuiltinTopicWire.encode(target));
        assertFalse(java.util.Arrays.equals(new byte[0], BuiltinTopicWire.encode(target)));
        first.close();
        second.close();
    }
}
