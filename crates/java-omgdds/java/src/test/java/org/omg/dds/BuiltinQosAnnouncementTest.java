// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import java.util.List;

import org.junit.jupiter.api.Test;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.Sample;
import org.omg.dds.topic.Topic;
import org.zerodds.topic.BuiltinTopics;
import org.zerodds.topic.TopicBuiltinTopicDataImpl;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;

/**
 * DDS 1.4 2.2.5: the built-in topic data carries the QoS of the entity; a changed QoS (set_qos)
 * is a new sample of the same instance. Here for a Topic, seen by the built-in reader of
 * another participant.
 */
class BuiltinQosAnnouncementTest {

    @Test
    void aChangedTopicQosIsANewSampleOfTheTopicInstance() {
        int domain = 1411;
        DomainParticipant observer = Qos.factory().createParticipant(domain);
        DomainParticipant owner = Qos.factory().createParticipant(domain);
        Topic<byte[]> topic = owner.createTopic("BuiltinTopicQos", byte[].class);
        DataReader<TopicBuiltinTopicDataImpl> topics =
                observer.getBuiltinSubscriber().lookupDataReader(BuiltinTopics.TOPIC);
        Qos.take(topics);

        topic.setQos(topic.getQos().withPolicy(Qos.pf().TransportPriority().withValue(7)));
        Integer seen = null;
        List<Sample<TopicBuiltinTopicDataImpl>> samples = Qos.take(topics);
        for (Sample<TopicBuiltinTopicDataImpl> sample : samples) {
            if (sample.getData() != null && "BuiltinTopicQos".equals(sample.getData().getName())) {
                seen = sample.getData().getTransportPriority().getValue();
            }
        }
        assertNotNull(seen, "the changed topic is announced again");
        assertEquals(7, seen.intValue());
    }
}
