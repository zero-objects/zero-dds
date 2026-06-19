// SPDX-License-Identifier: Apache-2.0
package org.zerodds.internal;

import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.function.Consumer;

/**
 * In-process Loopback-Bus for the pure-Java PSM.
 *
 * <p>This is a deliberately simple delivery mechanism: publishers post
 * to a shared {@link ConcurrentHashMap} keyed by `(domainId, topicName)`,
 * and subscribers register listeners. There is no RTPS wire — the
 * intended use is for in-process unit-testing + pure-Java applications
 * that don't need cross-process delivery.
 *
 * <p>Cross-process delivery requires the pure-Java RTPS-stack which is a
 * separate sprint (see audit `dds-java-psm-1.0.md` Open-Items).
 */
public final class InProcessBus {
    private static final InProcessBus INSTANCE = new InProcessBus();

    public static InProcessBus instance() {
        return INSTANCE;
    }

    private final Map<String, CopyOnWriteArrayList<Consumer<byte[]>>> listeners =
            new ConcurrentHashMap<>();

    private InProcessBus() {}

    private static String key(int domainId, String topicName) {
        return domainId + ":" + topicName;
    }

    public void subscribe(int domainId, String topicName, Consumer<byte[]> listener) {
        listeners.computeIfAbsent(key(domainId, topicName), k -> new CopyOnWriteArrayList<>())
                .add(listener);
    }

    public void unsubscribe(int domainId, String topicName, Consumer<byte[]> listener) {
        CopyOnWriteArrayList<Consumer<byte[]>> list = listeners.get(key(domainId, topicName));
        if (list != null) {
            list.remove(listener);
        }
    }

    public void publish(int domainId, String topicName, byte[] sample) {
        CopyOnWriteArrayList<Consumer<byte[]>> list = listeners.get(key(domainId, topicName));
        if (list == null) return;
        for (Consumer<byte[]> l : list) {
            l.accept(sample);
        }
    }

    /** Spec-only test-helper — clears all subscriptions. */
    public void reset() {
        listeners.clear();
    }
}
