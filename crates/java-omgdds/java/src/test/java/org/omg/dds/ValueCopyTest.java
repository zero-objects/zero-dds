// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.TimeUnit;

import org.junit.jupiter.api.Test;
import org.omg.dds.core.ModifiableInstanceHandle;
import org.omg.dds.core.ModifiableTime;
import org.omg.dds.core.policy.PolicyFactory;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.sub.DataReader;
import org.omg.dds.topic.Topic;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * DDS-Java-PSM 1.0 7.3.3: the value types (Time, Duration, InstanceHandle, QoS policies, statuses
 * and QoS) can be copied by value: they are Cloneable and Serializable, a copy is equal to its
 * source, and a copy of a modifiable value does not follow later changes of the source.
 */
class ValueCopyTest {

    private static Object copyOf(Object value) throws Exception {
        Method clone = value.getClass().getMethod("clone");
        return clone.invoke(value);
    }

    private static void assertCopyable(Object value) throws Exception {
        assertTrue(value instanceof Cloneable, value.getClass().getSimpleName() + " is Cloneable");
        assertTrue(value instanceof java.io.Serializable, value.getClass().getSimpleName() + " is Serializable");
        Object copy = copyOf(value);
        assertEquals(value, copy, value.getClass().getSimpleName());
        assertEquals(value.hashCode(), copy.hashCode(), value.getClass().getSimpleName());
        assertNotSame(value, copy);
    }

    @Test
    void timeDurationAndHandlesAreCopiedByValue() throws Exception {
        assertCopyable(Qos.env().getSPI().newTime(5, TimeUnit.SECONDS));
        assertCopyable(Qos.env().getSPI().newDuration(3, TimeUnit.MILLISECONDS));
        assertCopyable(Qos.env().getSPI().nilHandle());
        ModifiableTime time = Qos.env().getSPI().newTime(1, TimeUnit.SECONDS).modifiableCopy();
        ModifiableTime copy = (ModifiableTime) copyOf(time);
        time.copyFrom(Qos.env().getSPI().newTime(9, TimeUnit.SECONDS));
        assertEquals(1L, copy.getTime(TimeUnit.SECONDS), "the copy does not follow the source");
        ModifiableInstanceHandle handle = Qos.env().getSPI().nilHandle().modifiableCopy();
        ModifiableInstanceHandle handleCopy = (ModifiableInstanceHandle) copyOf(handle);
        assertEquals(handle, handleCopy);
        assertNotSame(handle, handleCopy);
    }

    @Test
    void everyPolicyOfThePolicyFactoryIsCopiedByValue() throws Exception {
        PolicyFactory factory = Qos.policies();
        int checked = 0;
        for (Method m : PolicyFactory.class.getMethods()) {
            if (m.getParameterCount() == 0 && org.omg.dds.core.policy.QosPolicy.class.isAssignableFrom(m.getReturnType())) {
                assertCopyable(m.invoke(factory));
                checked++;
            }
        }
        assertTrue(checked >= 20, "the policies of the factory: " + checked);
    }

    @Test
    void qosAndStatusObjectsAreCopiedByValue() throws Exception {
        assertCopyable(Qos.writer());
        assertCopyable(Qos.reader());
        assertCopyable(Qos.publisher());
        assertCopyable(Qos.subscriber());
        assertCopyable(Qos.topic());
        DomainParticipant participant = Qos.factory().createParticipant(1416);
        Topic<byte[]> topic = participant.createTopic("ValueCopy", byte[].class);
        DataWriter<byte[]> writer = participant.createPublisher().createDataWriter(topic);
        DataReader<byte[]> reader = participant.createSubscriber().createDataReader(topic);
        List<Object> statuses = new ArrayList<>();
        statuses.add(writer.getPublicationMatchedStatus());
        statuses.add(writer.getLivelinessLostStatus());
        statuses.add(writer.getOfferedDeadlineMissedStatus());
        statuses.add(writer.getOfferedIncompatibleQosStatus());
        statuses.add(reader.getSubscriptionMatchedStatus());
        statuses.add(reader.getLivelinessChangedStatus());
        statuses.add(reader.getSampleLostStatus());
        statuses.add(reader.getSampleRejectedStatus());
        statuses.add(reader.getRequestedDeadlineMissedStatus());
        statuses.add(reader.getRequestedIncompatibleQosStatus());
        statuses.add(topic.getInconsistentTopicStatus());
        for (Object status : statuses) {
            assertTrue(status instanceof Cloneable, status.getClass().getSimpleName());
            assertNotSame(status, copyOf(status));
        }
        participant.close();
    }
}
