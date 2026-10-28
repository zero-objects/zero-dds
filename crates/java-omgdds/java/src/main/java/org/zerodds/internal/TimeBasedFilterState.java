// SPDX-License-Identifier: Apache-2.0
package org.zerodds.internal;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.Iterator;
import java.util.List;
import java.util.Map;

/**
 * The state of a reader's TIME_BASED_FILTER (DDS 1.4 §2.2.3.12): per instance, the time at
 * which the last sample was accepted and the newest sample held back since.
 *
 * <p>A sample of an instance is accepted when {@code minimum_separation} has passed since the
 * previous accepted sample of that instance; an earlier one is held back. A held-back sample is
 * not lost: the newest one is handed out by {@link #due} once the separation has passed, so
 * that with RELIABLE the last sample of a burst reaches the reader in the steady state
 * (DDS 1.4 §2.2.3.12: "the system should guarantee delivery the last sample to the DataReader").
 *
 * <p>The caller passes the clock, which makes the state testable without waiting. It is not
 * thread-safe; a reader calls it under its own monitor.
 *
 * @param <S>  what a held-back sample is: whatever the reader needs to store it later.
 */
public final class TimeBasedFilterState<S> {

    private static final class Slot<S> {
        long acceptedAtMillis;
        S held;
    }

    private final Map<Object, Slot<S>> slots = new HashMap<>();

    /**
     * Offers a sample of an instance that arrived at {@code nowMillis}.
     *
     * @param separationMillis  the {@code minimum_separation}; zero or less accepts every sample.
     * @param sample            kept as the held-back sample if it is not accepted.
     * @return whether the sample is accepted (stored now); if not, it is held back.
     */
    public boolean offer(Object instance, long nowMillis, long separationMillis, S sample) {
        Slot<S> slot = slots.get(instance);
        if (slot == null || separationMillis <= 0 || nowMillis - slot.acceptedAtMillis >= separationMillis) {
            if (slot == null) {
                slot = new Slot<>();
                slots.put(instance, slot);
            }
            slot.acceptedAtMillis = nowMillis;
            slot.held = null;
            return true;
        }
        slot.held = sample;
        return false;
    }

    /**
     * The held-back samples whose instance has had {@code minimum_separation} since its last
     * accepted sample at {@code nowMillis}; they count as accepted from this call on.
     */
    public List<S> due(long nowMillis, long separationMillis) {
        List<S> out = new ArrayList<>();
        for (Iterator<Map.Entry<Object, Slot<S>>> it = slots.entrySet().iterator(); it.hasNext();) {
            Slot<S> slot = it.next().getValue();
            if (slot.held != null && nowMillis - slot.acceptedAtMillis >= separationMillis) {
                out.add(slot.held);
                slot.held = null;
                slot.acceptedAtMillis = nowMillis;
            }
        }
        return out;
    }

    /** Whether any instance holds a sample back. */
    public boolean holdsSamples() {
        for (Slot<S> slot : slots.values()) {
            if (slot.held != null) {
                return true;
            }
        }
        return false;
    }

    /** The reader forgot the instance. */
    public void forget(Object instance) {
        slots.remove(instance);
    }
}
