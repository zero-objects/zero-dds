// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core.condition;

import org.omg.dds.core.GuardCondition;
import org.omg.dds.core.ServiceEnvironment;

/**
 * ZeroDDS {@link GuardCondition} (DDS-Java-PSM 1.0 §7.3.7.4).
 *
 * <p>The only condition an application sets itself; {@link #setTriggerValue}
 * wakes every wait set this condition is attached to.
 */
public final class GuardConditionImpl extends GuardCondition {

    private volatile boolean triggerValue;

    private final ServiceEnvironment environment;
    private final java.util.List<WeakWaitSet> waitSets = new java.util.ArrayList<>();

    public GuardConditionImpl(ServiceEnvironment environment) {
        this.environment = environment;
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public boolean getTriggerValue() {
        return triggerValue;
    }

    @Override
    public void setTriggerValue(boolean value) {
        this.triggerValue = value;
        java.util.List<WeakWaitSet> known;
        synchronized (waitSets) {
            known = new java.util.ArrayList<>(waitSets);
        }
        for (WeakWaitSet ref : known) {
            WaitSetImpl waitSet = ref.get();
            if (waitSet != null) {
                waitSet.signal();
            }
        }
    }

    /** Registers a wait set to notify when this condition's trigger value changes. */
    public void attach(WaitSetImpl waitSet) {
        synchronized (waitSets) {
            waitSets.add(new WeakWaitSet(waitSet));
        }
    }

    /** Unregisters a wait set that is no longer interested in this condition. */
    public void detach(WaitSetImpl waitSet) {
        synchronized (waitSets) {
            waitSets.removeIf(ref -> {
                WaitSetImpl known = ref.get();
                return known == null || known == waitSet;
            });
        }
    }

    /** A wait set reference that does not keep it alive past {@code close()}. */
    private static final class WeakWaitSet {
        private final java.lang.ref.WeakReference<WaitSetImpl> ref;

        WeakWaitSet(WaitSetImpl waitSet) {
            this.ref = new java.lang.ref.WeakReference<>(waitSet);
        }

        WaitSetImpl get() {
            return ref.get();
        }
    }
}