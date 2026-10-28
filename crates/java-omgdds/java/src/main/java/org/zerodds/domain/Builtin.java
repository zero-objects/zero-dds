// SPDX-License-Identifier: Apache-2.0
package org.zerodds.domain;

import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.DataWriterQos;
import org.omg.dds.pub.Publisher;
import org.omg.dds.pub.PublisherQos;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.DataReaderQos;
import org.omg.dds.sub.SubscriberQos;
import org.omg.dds.topic.Topic;
import org.omg.dds.topic.TopicQos;
import org.zerodds.cdr.TopicTypeSupport;
import org.zerodds.cdr.TypeSupportImpl;
import org.zerodds.core.qos.DataWriterQosImpl;
import org.zerodds.core.qos.PublisherQosImpl;
import org.zerodds.core.qos.TopicQosImpl;
import org.zerodds.pub.PublisherImpl;
import org.zerodds.topic.BuiltinData;
import org.zerodds.topic.BuiltinTopicKeyImpl;
import org.zerodds.topic.BuiltinTopics;
import org.zerodds.topic.ParticipantBuiltinTopicDataImpl;
import org.zerodds.topic.PublicationBuiltinTopicDataImpl;
import org.zerodds.topic.SubscriptionBuiltinTopicDataImpl;
import org.zerodds.topic.TopicBuiltinTopicDataImpl;
import org.zerodds.topic.TopicImpl;

/**
 * The built-in topics of a participant (DDS 1.2 §2.2.1.3.1, DDSI-RTPS 2.5
 * §9.3.1.2) and the writers that publish their data.
 *
 * <p>§7.4.2: the participant creates the four built-in topics itself and
 * writes on them whenever an entity of the participant appears or goes
 * away, so an application that reads them through its built-in subscriber
 * sees what the domain contains. The built-in publisher is not one of the
 * participant's application-visible publishers, and the built-in entities do
 * not appear in the data they publish themselves.
 */
final class Builtin {

    private final DomainParticipantImpl participant;
    private final ServiceEnvironment environment;

    /**
     * False once the participant has withdrawn everything it announced; a
     * closed participant announces nothing more.
     */
    private volatile boolean live = true;

    /** What this participant announced and has not withdrawn: only that is retracted. */
    private final java.util.Set<org.omg.dds.core.InstanceHandle> announcedTopics =
            java.util.concurrent.ConcurrentHashMap.newKeySet();
    private final java.util.Set<org.omg.dds.core.InstanceHandle> announcedWriters =
            java.util.concurrent.ConcurrentHashMap.newKeySet();
    private final java.util.Set<org.omg.dds.core.InstanceHandle> announcedReaders =
            java.util.concurrent.ConcurrentHashMap.newKeySet();

    private final Topic<ParticipantBuiltinTopicDataImpl> participantTopic;
    private final Topic<PublicationBuiltinTopicDataImpl> publicationTopic;
    private final Topic<SubscriptionBuiltinTopicDataImpl> subscriptionTopic;
    private final Topic<TopicBuiltinTopicDataImpl> topicTopic;

    private final PublisherImpl publisher;
    private final DataWriter<ParticipantBuiltinTopicDataImpl> participantWriter;
    private final DataWriter<PublicationBuiltinTopicDataImpl> publicationWriter;
    private final DataWriter<SubscriptionBuiltinTopicDataImpl> subscriptionWriter;
    private final DataWriter<TopicBuiltinTopicDataImpl> topicWriter;

    Builtin(DomainParticipantImpl participant, ServiceEnvironment environment) {
        this.participant = participant;
        this.environment = environment;
        participant.creatingBuiltin = true;
        try {
            this.participantTopic = topic(BuiltinTopics.PARTICIPANT,
                    ParticipantBuiltinTopicDataImpl.class);
            this.publicationTopic = topic(BuiltinTopics.PUBLICATION,
                    PublicationBuiltinTopicDataImpl.class);
            this.subscriptionTopic = topic(BuiltinTopics.SUBSCRIPTION,
                    SubscriptionBuiltinTopicDataImpl.class);
            this.topicTopic = topic(BuiltinTopics.TOPIC,
                    TopicBuiltinTopicDataImpl.class);

            this.publisher = new PublisherImpl(environment,
                    participant.getDomainId(), participant,
                    PublisherQosImpl.defaults(environment));
            this.publisher.enableBuiltin();
            this.participantWriter = writer(participantTopic);
            this.publicationWriter = writer(publicationTopic);
            this.subscriptionWriter = writer(subscriptionTopic);
            this.topicWriter = writer(topicTopic);
        } finally {
            participant.creatingBuiltin = false;
        }
    }

    // --- The four built-in topics (§2.2.1.3.1) ------------------------------

    private <T> Topic<T> topic(String name, Class<T> dataType) {
        Topic<T> topic = new TopicImpl<>(environment, participant, name,
                BuiltinTopics.typeSupportOf(environment, name, dataType),
                TopicQosImpl.defaults(environment));
        topic.enable();
        participant.addTopic(topic);
        return topic;
    }

    @SuppressWarnings("unchecked")
    private <T> DataWriter<T> writer(Topic<T> topic) {
        TopicTypeSupport<T> codec =
                ((TypeSupportImpl<T>) topic.getTypeSupport()).codec();
        // DDS 1.4 2.2.5: the built-in readers request TRANSIENT_LOCAL, so a reader created
        // later gets what the domain already contains; the writers offer it. A withdrawn
        // built-in entity is an unregistered instance; the built-in writers do not dispose it
        // with the unregister (WRITER_DATA_LIFECYCLE, DDS 1.4 2.2.3.21).
        return publisher.createDataWriter(topic, codec,
                DataWriterQosImpl.defaults(environment)
                        .withPolicy(environment.getSPI().getPolicyFactory().Durability()
                                .withTransientLocal())
                        .withPolicy(environment.getSPI().getPolicyFactory().WriterDataLifecycle()
                                .withAutDisposeUnregisteredInstances(false)));
    }

    // --- The data of the built-in topics ------------------------------------

    /** "DCPSParticipant": this participant (DDS 1.2 §2.2.1.3.1). */
    void announceParticipant() {
        if (!live) {
            return;
        }
        write(participantWriter, new ParticipantBuiltinTopicDataImpl(
                environment,
                BuiltinTopicKeyImpl.of(environment,
                        participant.getInstanceHandle()),
                participant.getQos().getUserData()));
    }

    /** "DCPSTopic": a topic this participant created. */
    void announceTopic(Topic<?> topic) {
        if (!live) {
            return;
        }
        write(topicWriter, topicDataOf(topic));
        announcedTopics.add(topic.getInstanceHandle());
    }

    /** The "DCPSTopic" data of a topic, built from the live entity. */
    TopicBuiltinTopicDataImpl topicDataOf(Topic<?> topic) {
        return BuiltinData.topic(environment, topic);
    }

    /** "DCPSTopic": the topic is gone — the instance is unregistered. */
    void retractTopic(Topic<?> topic) {
        if (!live || !announcedTopics.remove(topic.getInstanceHandle())) {
            return;
        }
        unregister(topicWriter, topic.getInstanceHandle());
    }

    /** "DCPSPublication": a DataWriter this participant created. */
    void announceWriter(DataWriter<?> writer, Publisher parent) {
        if (!live || participant.creatingBuiltin) {
            return;
        }
        PublicationBuiltinTopicDataImpl data = BuiltinData.publication(environment,
                writer, parent, BuiltinTopicKeyImpl.of(environment,
                        participant.getInstanceHandle()));
        write(publicationWriter, data);
        announcedWriters.add(writer.getInstanceHandle());
    }

    /** "DCPSPublication": the DataWriter is gone. */
    void retractWriter(DataWriter<?> writer) {
        if (!live || participant.creatingBuiltin
                || !announcedWriters.remove(writer.getInstanceHandle())) {
            return;
        }
        unregister(publicationWriter, writer.getInstanceHandle());
    }

    /** "DCPSSubscription": a DataReader this participant created. */
    void announceReader(DataReader<?> reader, SubscriberQos parentQos) {
        if (!live || participant.creatingBuiltin) {
            return;
        }
        SubscriptionBuiltinTopicDataImpl data = BuiltinData.subscription(environment,
                reader, parentQos, BuiltinTopicKeyImpl.of(environment,
                        participant.getInstanceHandle()));
        write(subscriptionWriter, data);
        announcedReaders.add(reader.getInstanceHandle());
    }

    /** "DCPSSubscription": the DataReader is gone. */
    void retractReader(DataReader<?> reader) {
        if (!live || participant.creatingBuiltin
                || !announcedReaders.remove(reader.getInstanceHandle())) {
            return;
        }
        unregister(subscriptionWriter, reader.getInstanceHandle());
    }

    /**
     * Withdraws everything the participant announced: a reader in another
     * participant sees the entities leave the built-in topics before the
     * participant itself is closed (DDS 1.2 §2.2.1.3.1).
     */
    void retractAll() {
        for (Publisher publisher : participant.containedPublishers()) {
            for (DataWriter<?> writer : ((PublisherImpl) publisher).getDataWriters()) {
                retractWriter(writer);
            }
        }
        for (org.omg.dds.sub.Subscriber subscriber
                : participant.containedSubscribers()) {
            for (DataReader<?> reader
                    : ((org.zerodds.sub.SubscriberImpl) subscriber).getDataReaders()) {
                retractReader(reader);
            }
        }
        for (Topic<?> topic : participant.containedTopics()) {
            if (!BuiltinTopics.isBuiltin(topic.getName())) {
                retractTopic(topic);
            }
        }
        live = false;
    }

    /** Closes the built-in publisher and with it the four writers. */
    void close() {
        // Closing the built-in publisher closes its writers one after the other; none of
        // them announces its own going on a writer that may be closed by then.
        live = false;
        org.zerodds.rtps.RtpsParticipant.DiscoveryListener attached = remoteListener;
        if (attached != null) {
            org.zerodds.rtps.RtpsParticipant.get(participant.getDomainId())
                    .removeDiscoveryListener(attached);
            remoteListener = null;
        }
        publisher.close();
    }

    // --- Entities of other participants, as SPDP and SEDP announce them ------
    //
    // DDSI-RTPS 2.5 §9.6.2: the participants and endpoints a remote peer
    // announces appear on the built-in topics with the QoS they carry. A
    // participant of the same process is announced by the writers above; an
    // announcement over RTPS reaches the same topics through the listener
    // below.

    private volatile org.zerodds.rtps.RtpsParticipant.DiscoveryListener remoteListener;

    /** Publishes what {@code rtps} discovers on the built-in topics of this participant. */
    void attachRemote(org.zerodds.rtps.RtpsParticipant rtps) {
        org.zerodds.rtps.RtpsParticipant.DiscoveryListener listener =
                new org.zerodds.rtps.RtpsParticipant.DiscoveryListener() {
            @Override
            public void participantDiscovered(org.zerodds.rtps.DiscoveryData.Participant p) {
                if (live) {
                    participant.remoteParticipantSeen(p);
                    write(participantWriter, BuiltinData.participant(environment, p));
                }
            }

            @Override
            public void participantLost(org.zerodds.rtps.DiscoveryData.Participant p) {
                if (live) {
                    participant.remoteParticipantLost(p);
                    unregister(participantWriter, org.zerodds.core.Values.of(environment,
                            BuiltinData.participantGuid(p.guidPrefix)));
                }
            }

            @Override
            public void endpointDiscovered(org.zerodds.rtps.DiscoveryData.Endpoint e) {
                if (!live) {
                    return;
                }
                participant.remoteEndpointSeen(e);
                if (e.writer) {
                    write(publicationWriter, BuiltinData.publication(environment, e));
                } else {
                    write(subscriptionWriter, BuiltinData.subscription(environment, e));
                }
            }

            @Override
            public void endpointLost(org.zerodds.rtps.DiscoveryData.Endpoint e) {
                if (!live) {
                    return;
                }
                participant.remoteEndpointLost(e);
                unregister(e.writer ? publicationWriter : subscriptionWriter,
                        org.zerodds.core.Values.of(environment, e.endpointGuid));
            }
        };
        remoteListener = listener;
        rtps.addDiscoveryListener(listener);
    }

    // --- Writing the built-in data -----------------------------------------
    //
    // §7.3.2: a writer that cannot store the sample within the
    // RELIABILITY.max_blocking_time reports a timeout. The in-process bus
    // hands the sample to its readers synchronously, so the wait never runs
    // out; a timeout here would be a defect of the bus, not of the caller.

    private static void write(DataWriter<?> writer, Object sample) {
        try {
            writeSample(writer, sample);
        } catch (java.util.concurrent.TimeoutException timedOut) {
            throw new java.util.concurrent.CompletionException(
                    "a built-in topic writer timed out", timedOut);
        }
    }

    @SuppressWarnings("unchecked")
    private static void writeSample(DataWriter<?> writer, Object sample)
            throws java.util.concurrent.TimeoutException {
        ((DataWriter<Object>) writer).write(sample);
    }

    private static void unregister(DataWriter<?> writer,
            org.omg.dds.core.InstanceHandle handle) {
        try {
            unregisterInstance(writer, handle);
        } catch (java.util.concurrent.TimeoutException timedOut) {
            throw new java.util.concurrent.CompletionException(
                    "a built-in topic writer timed out", timedOut);
        }
    }

    @SuppressWarnings("unchecked")
    private static void unregisterInstance(DataWriter<?> writer,
            org.omg.dds.core.InstanceHandle handle)
            throws java.util.concurrent.TimeoutException {
        ((DataWriter<Object>) writer).unregisterInstance(handle);
    }
}
