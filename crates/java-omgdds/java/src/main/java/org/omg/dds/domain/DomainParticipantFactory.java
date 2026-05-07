// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.domain;

import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

/**
 * OMG DDS Java-PSM DomainParticipantFactory — Spec §7.2.4 Singleton.
 */
public final class DomainParticipantFactory {
    private static final DomainParticipantFactory INSTANCE = new DomainParticipantFactory();

    public static DomainParticipantFactory getInstance() {
        return INSTANCE;
    }

    private final Map<Integer, DomainParticipant> participants = new ConcurrentHashMap<>();

    private DomainParticipantFactory() {}

    public DomainParticipant createParticipant(int domainId) {
        return participants.computeIfAbsent(domainId, id -> {
            DomainParticipant p = new DomainParticipant(id);
            p.enable();
            return p;
        });
    }

    public DomainParticipant lookupParticipant(int domainId) {
        return participants.get(domainId);
    }

    public void deleteParticipant(DomainParticipant participant) {
        participant.close();
        participants.remove(participant.getDomainId(), participant);
    }
}
