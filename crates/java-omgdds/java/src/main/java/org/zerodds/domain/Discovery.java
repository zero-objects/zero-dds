// SPDX-License-Identifier: Apache-2.0
package org.zerodds.domain;

import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.Publisher;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.SubscriberQos;

/**
 * The bridge from a publisher or a subscriber to the built-in topics of the
 * participant that owns it (DDS 1.2 §2.2.1.3.1).
 *
 * <p>A publisher and a subscriber know the participant that created them,
 * but the built-in data is written by the participant itself. A participant
 * of another implementation has no built-in topics of this binding, so every
 * call here is a no-op for one.
 */
public final class Discovery {

    private Discovery() {}

    /** A DataWriter appeared on the "DCPSPublication" built-in topic. */
    public static void writerCreated(DomainParticipant parent,
            DataWriter<?> writer, Publisher publisher) {
        if (parent instanceof DomainParticipantImpl) {
            DomainParticipantImpl participant = (DomainParticipantImpl) parent;
            participant.onWriterCreated(writer, publisher);
        }
    }

    /** A DataWriter went away from the "DCPSPublication" built-in topic. */
    public static void writerClosed(DomainParticipant parent,
            DataWriter<?> writer) {
        if (parent instanceof DomainParticipantImpl) {
            DomainParticipantImpl participant = (DomainParticipantImpl) parent;
            participant.onWriterClosed(writer);
        }
    }

    /** A DataReader appeared on the "DCPSSubscription" built-in topic. */
    public static void readerCreated(DomainParticipant parent,
            DataReader<?> reader, SubscriberQos parentQos) {
        if (parent instanceof DomainParticipantImpl) {
            DomainParticipantImpl participant = (DomainParticipantImpl) parent;
            participant.onReaderCreated(reader, parentQos);
        }
    }

    /** A DataReader went away from the "DCPSSubscription" built-in topic. */
    public static void readerClosed(DomainParticipant parent,
            DataReader<?> reader) {
        if (parent instanceof DomainParticipantImpl) {
            DomainParticipantImpl participant = (DomainParticipantImpl) parent;
            participant.onReaderClosed(reader);
        }
    }
}
