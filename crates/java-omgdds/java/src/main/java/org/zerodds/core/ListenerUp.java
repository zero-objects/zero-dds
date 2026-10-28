// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core;

import org.omg.dds.core.event.DataAvailableEvent;
import org.omg.dds.core.event.InconsistentTopicEvent;
import org.omg.dds.core.event.LivelinessChangedEvent;
import org.omg.dds.core.event.LivelinessLostEvent;
import org.omg.dds.core.event.OfferedDeadlineMissedEvent;
import org.omg.dds.core.event.OfferedIncompatibleQosEvent;
import org.omg.dds.core.event.PublicationMatchedEvent;
import org.omg.dds.core.event.RequestedDeadlineMissedEvent;
import org.omg.dds.core.event.RequestedIncompatibleQosEvent;
import org.omg.dds.core.event.SampleLostEvent;
import org.omg.dds.core.event.SampleRejectedEvent;
import org.omg.dds.core.event.SubscriptionMatchedEvent;
import org.omg.dds.pub.DataWriterListener;
import org.omg.dds.pub.PublisherListener;
import org.omg.dds.sub.DataReaderListener;
import org.omg.dds.sub.SubscriberListener;
import org.omg.dds.topic.TopicListener;
import org.omg.dds.domain.DomainParticipantListener;

/**
 * DDS 1.4 §2.2.4.3.1: a status change that the entity's own listener does not take (none
 * set, or the status is not in its mask) is notified to the listener of the containing
 * entity: a DataWriter to its Publisher, then the DomainParticipant; a DataReader to its
 * Subscriber, then the DomainParticipant; a Topic to the DomainParticipant. These adapters
 * present the listener of the container as the listener of the entity, so the code that
 * notifies the entity needs one listener to call.
 */
public final class ListenerUp {

    private ListenerUp() {}

    /** A reader's listener that hands each status to a Subscriber's (or participant's) listener. */
    public static <T> DataReaderListener<T> forReader(final SubscriberListener up) {
        return new DataReaderListener<T>() {
            @Override
            public void onRequestedDeadlineMissed(RequestedDeadlineMissedEvent<T> status) {
                up.onRequestedDeadlineMissed(status);
            }

            @Override
            public void onRequestedIncompatibleQos(RequestedIncompatibleQosEvent<T> status) {
                up.onRequestedIncompatibleQos(status);
            }

            @Override
            public void onSampleRejected(SampleRejectedEvent<T> status) {
                up.onSampleRejected(status);
            }

            @Override
            public void onLivelinessChanged(LivelinessChangedEvent<T> status) {
                up.onLivelinessChanged(status);
            }

            @Override
            public void onDataAvailable(DataAvailableEvent<T> status) {
                up.onDataAvailable(status);
            }

            @Override
            public void onSubscriptionMatched(SubscriptionMatchedEvent<T> status) {
                up.onSubscriptionMatched(status);
            }

            @Override
            public void onSampleLost(SampleLostEvent<T> status) {
                up.onSampleLost(status);
            }
        };
    }

    /** A writer's listener that hands each status to a Publisher's (or participant's) listener. */
    public static <T> DataWriterListener<T> forWriter(final PublisherListener up) {
        return new DataWriterListener<T>() {
            @Override
            public void onOfferedDeadlineMissed(OfferedDeadlineMissedEvent<T> status) {
                up.onOfferedDeadlineMissed(status);
            }

            @Override
            public void onOfferedIncompatibleQos(OfferedIncompatibleQosEvent<T> status) {
                up.onOfferedIncompatibleQos(status);
            }

            @Override
            public void onLivelinessLost(LivelinessLostEvent<T> status) {
                up.onLivelinessLost(status);
            }

            @Override
            public void onPublicationMatched(PublicationMatchedEvent<T> status) {
                up.onPublicationMatched(status);
            }
        };
    }

    /** A topic's listener that hands the status to the participant's listener. */
    public static <T> TopicListener<T> forTopic(final DomainParticipantListener up) {
        return new TopicListener<T>() {
            @Override
            public void onInconsistentTopic(InconsistentTopicEvent<T> status) {
                up.onInconsistentTopic(status);
            }
        };
    }
}
