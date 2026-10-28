// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.util.ArrayList;
import java.util.Collection;
import java.util.Collections;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeSet;
import java.util.concurrent.TimeUnit;

import org.junit.jupiter.api.Test;
import org.omg.dds.core.Entity;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.Publisher;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.Subscriber;
import org.omg.dds.topic.Topic;
import org.omg.dds.type.TypeSupport;

import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * DDS-Java-PSM 1.0 7.3.2, Table 7.1: RETCODE_BAD_PARAMETER is
 * {@code java.lang.IllegalArgumentException}. Every operation of the entities that takes an
 * object argument and gets {@code null} for it throws that exception and no other
 * ({@code NullPointerException} in particular), unless the operation gives {@code null} a meaning
 * (a listener or status mask of none, a key holder or a list to be created, a nil handle).
 */
class NullArgumentTest {

    private static int counter;

    /** Operations (name#parameter index) where DDS 1.4 or the PSM gives null a meaning. */
    private static final java.util.Set<String> MEANINGFUL = new java.util.HashSet<>(java.util.Arrays.asList(
            "getKeyValue#0", "getKeyValue#1", "read#1", "take#1", "write#1", "write#2", "dispose#0", "dispose#1", "unregisterInstance#0",
            "unregisterInstance#1", "registerInstance#1", "read#0", "take#0", "getCurrentTime#0",
            "lookupInstance#0", "lookupInstance#1", "select#0", "waitForConditions#0",
            "setListener#0", "setListener#1", "getMatchedSubscriptionData#0", "getMatchedPublicationData#0"));

    private static final class Rig {
        final DomainParticipant participant;
        final Topic<byte[]> topic;
        final Publisher publisher;
        final Subscriber subscriber;
        final DataWriter<byte[]> writer;
        final DataReader<byte[]> reader;

        Rig() {
            participant = Qos.factory().createParticipant(1420);
            topic = participant.createTopic("NullArgs" + (counter++), byte[].class);
            publisher = participant.createPublisher();
            subscriber = participant.createSubscriber();
            writer = publisher.createDataWriter(topic);
            reader = subscriber.createDataReader(topic);
        }

        Object[] targets() {
            return new Object[] {participant, publisher, subscriber, topic, writer, reader};
        }
    }

    private static Object valid(Class<?> type) {
        String n = type.getName();
        if (type == String.class) {
            return "NullArgsName" + (counter++);
        }
        if (type == Class.class) {
            return byte[].class;
        }
        if (type == TypeSupport.class) {
            return TypeSupport.newTypeSupport(byte[].class, Qos.env());
        }
        if (type == Object.class) {
            return new byte[] {1};
        }
        if (type == TimeUnit.class) {
            return TimeUnit.SECONDS;
        }
        if (type == org.omg.dds.core.Duration.class) {
            return Qos.env().getSPI().newDuration(1, TimeUnit.SECONDS);
        }
        if (type == org.omg.dds.core.Time.class) {
            return Qos.env().getSPI().newTime(1, TimeUnit.SECONDS);
        }
        if (type == org.omg.dds.core.InstanceHandle.class) {
            return Qos.env().getSPI().nilHandle();
        }
        if (type == List.class || type == Collection.class) {
            return new ArrayList<Object>();
        }
        if (type == org.omg.dds.domain.DomainParticipantQos.class) {
            return org.zerodds.core.qos.DomainParticipantQosImpl.defaults(Qos.env());
        }
        if (type == org.omg.dds.pub.PublisherQos.class) {
            return Qos.publisher();
        }
        if (type == org.omg.dds.sub.SubscriberQos.class) {
            return Qos.subscriber();
        }
        if (type == org.omg.dds.topic.TopicQos.class) {
            return Qos.topic();
        }
        if (type == org.omg.dds.pub.DataWriterQos.class) {
            return Qos.writer();
        }
        if (type == org.omg.dds.sub.DataReaderQos.class) {
            return Qos.reader();
        }
        if (n.startsWith("org.omg.dds") && type.isInterface() || n.startsWith("org.omg.dds")) {
            return null; // not built here: the operation is not covered
        }
        return null;
    }

    private static String label(Class<?> iface, Method m, int p) {
        StringBuilder b = new StringBuilder(iface.getSimpleName()).append('.').append(m.getName()).append('(');
        Class<?>[] types = m.getParameterTypes();
        for (int i = 0; i < types.length; i++) {
            b.append(i == 0 ? "" : ", ").append(i == p ? "NULL:" : "").append(types[i].getSimpleName());
        }
        return b.append(')').toString();
    }

    private static boolean nullable(Class<?> type) {
        return type.getSimpleName().endsWith("Listener") || type == Collection.class;
    }

    @Test
    void aNullArgumentIsIllegalArgumentExceptionAndNeverNullPointerException() throws Exception {
        TreeSet<String> failures = new TreeSet<>();
        int calls = 0;
        Class<?>[] interfaces = {DomainParticipant.class, Publisher.class, Subscriber.class,
            Topic.class, DataWriter.class, DataReader.class};
        for (int t = 0; t < interfaces.length; t++) {
            Class<?> iface = interfaces[t];
            for (Method m : iface.getMethods()) {
                if (Modifier.isStatic(m.getModifiers()) || m.getParameterCount() == 0
                        || m.isVarArgs() || m.getName().startsWith("close")
                        || (m.getName().equals("setQos") && m.getParameterTypes()[0] == String.class)
                        || m.getName().equals("enable") || m.getName().equals("retain")
                        || m.getName().equals("waitForAcknowledgments")
                        || m.getName().equals("waitForHistoricalData")) {
                    continue;
                }
                Class<?>[] types = m.getParameterTypes();
                for (int p = 0; p < types.length; p++) {
                    if (types[p].isPrimitive() || nullable(types[p])
                            || MEANINGFUL.contains(m.getName() + "#" + p)) {
                        continue;
                    }
                    Rig rig = new Rig();
                    try {
                        Object target = rig.targets()[t];
                        Object[] args = new Object[types.length];
                        boolean buildable = true;
                        for (int i = 0; i < types.length; i++) {
                            if (types[i].isPrimitive()) {
                                args[i] = types[i] == boolean.class ? Boolean.FALSE
                                        : types[i] == long.class ? Long.valueOf(1)
                                        : types[i] == int.class ? Integer.valueOf(1)
                                        : null;
                                continue;
                            }
                            if (i == p) {
                                continue;
                            }
                            if (nullable(types[i])) {
                                continue;
                            }
                            if (types[i] == Topic.class || types[i] == org.omg.dds.topic.TopicDescription.class) {
                                args[i] = rig.topic;
                                continue;
                            }
                            args[i] = valid(types[i]);
                            if (args[i] == null) {
                                buildable = false;
                            }
                        }
                        if (!buildable) {
                            continue;
                        }
                        calls++;
                        try {
                            m.invoke(target, args);
                            failures.add(label(iface, m, p)
                                    + " accepted null");
                        } catch (InvocationTargetException e) {
                            Throwable c = e.getCause();
                            if (!(c instanceof IllegalArgumentException)) {
                                failures.add(label(iface, m, p) + " "
                                        + c.getClass().getSimpleName());
                            }
                        }
                    } finally {
                        try {
                            rig.participant.close();
                        } catch (RuntimeException ignored) {
                            // the operation under test may have left the rig closed
                        }
                    }
                }
            }
        }
        assertTrue(calls > 40, "operations exercised: " + calls);
        assertTrue(failures.isEmpty(), failures.size() + " of " + calls + ":\n" + String.join("\n", failures));
    }
}
