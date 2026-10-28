// SPDX-License-Identifier: Apache-2.0
package org.zerodds.internal;

import java.util.Map;
import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;

import org.omg.dds.core.InstanceHandle;

/**
 * What the participants of this process have told the Service to ignore
 * (DDS 1.4 §2.2.2.2.1.14 to §2.2.2.2.1.17: ignore_participant, ignore_topic,
 * ignore_publication, ignore_subscription), and which participant owns each endpoint, so that
 * the {@link InProcessBus} can leave a pair of endpoints unmatched when one side's participant
 * ignores the other.
 *
 * <p>It also separates the built-in readers of a participant from the built-in writers of the
 * same participant.
 *
 * <p>A participant that ignores a participant, a topic, a publication or a subscription
 * behaves as if it did not exist: none of its own endpoints is matched with the ignored one,
 * and none of the ignored one's samples reaches them. The operations are not reversible.
 */
public final class Ignores {

    private Ignores() {}

    /** The ignore sets of one participant. */
    public static final class Rules {
        private final Set<InstanceHandle> participants = ConcurrentHashMap.newKeySet();
        private final Set<InstanceHandle> publications = ConcurrentHashMap.newKeySet();
        private final Set<InstanceHandle> subscriptions = ConcurrentHashMap.newKeySet();
        private final Set<String> topics = ConcurrentHashMap.newKeySet();

        public void ignoreParticipant(InstanceHandle handle) {
            participants.add(handle);
        }

        public void ignorePublication(InstanceHandle handle) {
            publications.add(handle);
        }

        public void ignoreSubscription(InstanceHandle handle) {
            subscriptions.add(handle);
        }

        /** Ignores the topic of that name (the topic handle names it). */
        public void ignoreTopic(String topicName) {
            topics.add(topicName);
        }

        public boolean ignoresParticipant(InstanceHandle handle) {
            return participants.contains(handle);
        }

        public boolean ignoresPublication(InstanceHandle handle) {
            return publications.contains(handle);
        }

        public boolean ignoresSubscription(InstanceHandle handle) {
            return subscriptions.contains(handle);
        }

        public boolean ignoresTopic(String topicName) {
            return topics.contains(topicName);
        }
    }

    /** The participant and the topic an endpoint belongs to. */
    private static final class Owner {
        final InstanceHandle participant;
        final String topic;

        Owner(InstanceHandle participant, String topic) {
            this.participant = participant;
            this.topic = topic;
        }
    }

    private static final Map<InstanceHandle, Rules> RULES = new ConcurrentHashMap<>();
    private static final Map<InstanceHandle, Owner> OWNERS = new ConcurrentHashMap<>();

    /** The ignore sets of {@code participant}, created on first use. */
    public static Rules of(InstanceHandle participant) {
        return RULES.computeIfAbsent(participant, p -> new Rules());
    }

    /** The participant's sets are dropped when it is closed. */
    public static void forgetParticipant(InstanceHandle participant) {
        RULES.remove(participant);
    }

    /** {@code endpoint} (a DataWriter or DataReader handle) belongs to {@code participant}. */
    public static void register(InstanceHandle endpoint, InstanceHandle participant,
            String topicName) {
        OWNERS.put(endpoint, new Owner(participant, topicName));
    }

    public static void forget(InstanceHandle endpoint) {
        OWNERS.remove(endpoint);
    }

    /** Whether the participant that owns {@code endpoint}, if known, is {@code participant}. */
    public static boolean isOwnedBy(InstanceHandle endpoint, InstanceHandle participant) {
        Owner owner = OWNERS.get(endpoint);
        return owner != null && owner.participant.equals(participant);
    }

    /**
     * Whether the reader and the writer must not be matched because the participant of one of
     * them ignores the other, its participant or its topic.
     */
    public static boolean pairIgnored(InstanceHandle reader, InstanceHandle writer) {
        if (reader == null || writer == null) {
            return false;
        }
        Owner r = OWNERS.get(reader);
        Owner w = OWNERS.get(writer);
        if (r == null || w == null) {
            return false;
        }
        boolean sameParticipant = r.participant.equals(w.participant);
        if (sameParticipant && r.topic != null && r.topic.equals(w.topic)
                && org.zerodds.topic.BuiltinTopics.isBuiltin(r.topic)) {
            // DDS 1.4 2.2.5: a built-in DataReader gives no data about the entities of its own
            // participant, which the application that created them knows already.
            return true;
        }
        Rules readerSide = RULES.get(r.participant);
        if (readerSide != null && (readerSide.publications.contains(writer)
                || readerSide.topics.contains(r.topic)
                || (!sameParticipant && readerSide.participants.contains(w.participant)))) {
            return true;
        }
        Rules writerSide = RULES.get(w.participant);
        return writerSide != null && (writerSide.subscriptions.contains(reader)
                || writerSide.topics.contains(w.topic)
                || (!sameParticipant && writerSide.participants.contains(r.participant)));
    }
}
