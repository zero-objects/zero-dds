// SPDX-License-Identifier: Apache-2.0
package org.zerodds.domain;

import java.util.Collection;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.CopyOnWriteArrayList;

import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.core.status.Status;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.domain.DomainParticipantFactory;
import org.omg.dds.domain.DomainParticipantFactoryQos;
import org.omg.dds.domain.DomainParticipantListener;
import org.omg.dds.domain.DomainParticipantQos;
import org.zerodds.core.qos.DomainParticipantFactoryQosImpl;
import org.zerodds.core.qos.DomainParticipantQosImpl;

/**
 * ZeroDDS {@link DomainParticipantFactory} (DDS-Java-PSM 1.0 §7.4.1).
 *
 * <p>§7.4.1 makes the factory a per-{@link ServiceEnvironment} singleton with
 * no factory of its own: {@code getInstance(env)} hands out the one the
 * environment's service-provider interface holds, so two environments know
 * nothing of each other's participants. The only thing this class adds to
 * the OMG definition is the registry {@link #lookupParticipant(int)} and the
 * discovery operations read from: DDS 1.2 §7.1.2.2.2.1 requires every
 * {@code createParticipant} to return a <em>new</em> participant, and
 * §7.1.2.2.2.6 lets {@code lookupParticipant} answer with "one of" them.
 */
public final class DomainParticipantFactoryImpl extends DomainParticipantFactory {

    /**
     * The participants of every environment in this process, by domain: they share the
     * in-process bus, so they discover each other whatever factory created them
     * (DDS 1.4 §2.2.2.2.1.27 to §2.2.2.2.1.30).
     */
    private static final Map<Integer, List<DomainParticipantImpl>> ALL = new ConcurrentHashMap<>();

    private final ServiceEnvironment environment;

    /** The participants of each domain, in creation order (§7.1.2.2.2.6). */
    private final Map<Integer, List<DomainParticipant>> participants =
            new ConcurrentHashMap<>();

    private volatile DomainParticipantFactoryQos qos;
    private volatile DomainParticipantQos defaultParticipantQos;

    public DomainParticipantFactoryImpl(ServiceEnvironment environment) {
        this.environment = environment;
        this.qos = DomainParticipantFactoryQosImpl.defaults(environment);
        this.defaultParticipantQos = DomainParticipantQosImpl.defaults(environment);
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    // --- Create (§7.4.1) ---------------------------------------------------

    @Override
    public DomainParticipant createParticipant() {
        return createParticipant(0);
    }

    @Override
    public DomainParticipant createParticipant(int domainId) {
        return createParticipant(domainId, defaultParticipantQos, null, null);
    }

    @Override
    public DomainParticipant createParticipant(int domainId,
            DomainParticipantQos qos, DomainParticipantListener listener,
            Collection<Class<? extends Status>> statuses) {
        DomainParticipantImpl participant = new DomainParticipantImpl(
                environment, this, domainId, qos, autoEnable());
        if (listener != null) {
            // set before the participant is known to the domain, so nothing it does is missed
            participant.setListener(listener, statuses);
        }
        register(participant);
        return participant;
    }

    /**
     * §7.3.6: whether {@code create_participant} enables what it creates —
     * the ENTITY_FACTORY of this factory, which defaults to true
     * (DDS 1.2 §7.1.3.7).
     */
    private boolean autoEnable() {
        return qos.getEntityFactory() == null
                || qos.getEntityFactory().isAutoEnableCreatedEntities();
    }

    @Override
    public DomainParticipant lookupParticipant(int domainId) {
        List<DomainParticipant> inDomain = participants.get(domainId);
        return inDomain == null || inDomain.isEmpty() ? null : inDomain.get(0);
    }

    // --- The registry the participant and the discovery operations share ---

    void register(DomainParticipantImpl participant) {
        participants.computeIfAbsent(participant.getDomainId(),
                id -> new CopyOnWriteArrayList<>()).add(participant);
        ALL.computeIfAbsent(participant.getDomainId(),
                id -> new CopyOnWriteArrayList<>()).add(participant);
    }

    /** The open participants of {@code domainId} in this process, of every environment. */
    static List<DomainParticipantImpl> participantsInDomain(int domainId) {
        List<DomainParticipantImpl> inDomain = ALL.get(domainId);
        return inDomain == null ? java.util.Collections.<DomainParticipantImpl>emptyList()
                : new java.util.ArrayList<>(inDomain);
    }

    /** A snapshot of the participants of {@code domainId}. */
    List<DomainParticipant> participantsIn(int domainId) {
        List<DomainParticipant> inDomain = participants.get(domainId);
        return inDomain == null ? java.util.Collections.emptyList() : java.util.Collections.unmodifiableList(new java.util.ArrayList<>(inDomain));
    }

    void forget(DomainParticipantImpl participant) {
        List<DomainParticipant> inDomain = participants.get(participant.getDomainId());
        if (inDomain != null) {
            inDomain.remove(participant);
        }
        List<DomainParticipantImpl> all = ALL.get(participant.getDomainId());
        if (all != null) {
            all.remove(participant);
        }
    }

    // --- QoS (§7.4.1) ------------------------------------------------------

    @Override
    public DomainParticipantFactoryQos getQos() {
        return qos;
    }

    @Override
    public void setQos(DomainParticipantFactoryQos qos) {
        org.zerodds.core.qos.QosRules.checkChange(true, this.qos, qos);
        this.qos = qos;
    }

    @Override
    public DomainParticipantQos getDefaultParticipantQos() {
        return defaultParticipantQos;
    }

    @Override
    public void setDefaultParticipantQos(DomainParticipantQos qos) {
        this.defaultParticipantQos = qos;
    }

    @Override
    public String toString() {
        return "DomainParticipantFactory[" + environment + "]";
    }
}
