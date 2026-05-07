// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.domain;

import org.omg.dds.core.Entity;
import org.omg.dds.core.InstanceHandle;
import org.omg.dds.core.ReturnCode;
import org.omg.dds.core.policy.QosProfile;
import org.omg.dds.pub.Publisher;
import org.omg.dds.sub.Subscriber;
import org.omg.dds.topic.Topic;

import java.util.UUID;

/**
 * OMG DDS Java-PSM DomainParticipant — Spec §7.2.4.
 */
public final class DomainParticipant implements Entity {
    private final int domainId;
    private final InstanceHandle handle;
    private boolean closed = false;
    private boolean enabled = false;

    public DomainParticipant(int domainId) {
        this.domainId = domainId;
        this.handle = uuidHandle();
    }

    public int getDomainId() {
        return domainId;
    }

    public Publisher createPublisher() {
        return createPublisher(QosProfile.DEFAULT);
    }

    public Publisher createPublisher(QosProfile qos) {
        Publisher p = new Publisher(domainId, qos);
        p.enable();
        return p;
    }

    public Subscriber createSubscriber() {
        return createSubscriber(QosProfile.DEFAULT);
    }

    public Subscriber createSubscriber(QosProfile qos) {
        Subscriber s = new Subscriber(domainId, qos);
        s.enable();
        return s;
    }

    public <T> Topic<T> createTopic(String name, Class<T> type) {
        Topic<T> t = new Topic<>(name, type);
        t.enable();
        return t;
    }

    @Override
    public InstanceHandle getInstanceHandle() {
        return handle;
    }

    @Override
    public ReturnCode enable() {
        if (closed) return ReturnCode.ALREADY_DELETED;
        enabled = true;
        return ReturnCode.OK;
    }

    @Override
    public void close() {
        closed = true;
    }

    @Override
    public boolean isClosed() {
        return closed;
    }

    public boolean isEnabled() {
        return enabled;
    }

    private static InstanceHandle uuidHandle() {
        UUID id = UUID.randomUUID();
        byte[] out = new byte[16];
        long msb = id.getMostSignificantBits();
        long lsb = id.getLeastSignificantBits();
        for (int i = 0; i < 8; i++) out[i] = (byte) (msb >>> (56 - 8 * i));
        for (int i = 0; i < 8; i++) out[8 + i] = (byte) (lsb >>> (56 - 8 * i));
        return InstanceHandle.from(out);
    }
}
