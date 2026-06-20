// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.domain;

import org.omg.dds.core.ServiceEnvironment;

import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

/**
 * OMG DDS Java-PSM DomainParticipantFactory — Spec §7.2.4 / §7.4.1.
 *
 * <p>Per Spec §7.4.1 the factory is a <em>per-{@link ServiceEnvironment}</em>
 * singleton, obtained by passing the environment to
 * {@link #getInstance(ServiceEnvironment)}. ZeroDDS being pure Java with a
 * single in-JVM service, all environments share one factory instance; the
 * no-argument {@link #getInstance()} returns the same singleton for callers
 * that bootstrap without an explicit environment.
 */
public final class DomainParticipantFactory {
    private static final DomainParticipantFactory INSTANCE = new DomainParticipantFactory();

    public static DomainParticipantFactory getInstance() {
        return INSTANCE;
    }

    /**
     * Spec §7.4.1 — obtain the per-{@link ServiceEnvironment} factory singleton.
     *
     * @param env the bootstrapped service environment (must not be {@code null}).
     * @return the factory instance for {@code env}.
     */
    public static DomainParticipantFactory getInstance(ServiceEnvironment env) {
        if (env == null) {
            throw new NullPointerException("ServiceEnvironment must not be null");
        }
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
