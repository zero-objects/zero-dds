// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core.condition;

import org.omg.dds.core.Condition;
import org.omg.dds.core.Duration;
import org.omg.dds.core.GuardCondition;
import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.core.WaitSet;

import java.util.ArrayList;
import java.util.Collection;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Set;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.TimeoutException;

/**
 * ZeroDDS {@link WaitSet} (DDS-Java-PSM 1.0 §7.3.7.4).
 *
 * <p>A wait set blocks its caller until at least one of its conditions has
 * {@link Condition#getTriggerValue()} true, or until the timeout elapses. §7.3.7.4
 * requires the timeout to end in {@link TimeoutException}, never in an empty
 * return; {@code waitForConditions} without a timeout blocks until a condition
 * fires.
 */
public final class WaitSetImpl extends WaitSet {

    private static final long serialVersionUID = 1L;

    private final ServiceEnvironment environment;
    private final Set<Condition> conditions = new LinkedHashSet<>();
    private final Object monitor = new Object();

    public WaitSetImpl(ServiceEnvironment environment) {
        this.environment = environment;
        synchronized (ALL) {
            ALL.add(this);
        }
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    /** Every wait set, for the broadcast of {@link #conditionChanged()}. */
    private static final Set<WaitSetImpl> ALL =
            java.util.Collections.newSetFromMap(new java.util.WeakHashMap<WaitSetImpl, Boolean>());

    /**
     * A condition of any wait set may have become true: wakes every blocked wait set, which
     * evaluates its own conditions again. Called when a communication status is triggered,
     * when a reader receives or changes a sample, and when a guard condition is set.
     */
    public static void conditionChanged() {
        List<WaitSetImpl> all;
        synchronized (ALL) {
            all = new ArrayList<>(ALL);
        }
        for (WaitSetImpl waitSet : all) {
            waitSet.signal();
        }
    }

    /** The thread blocked in this wait set, or null (§7.3.7.4: at most one). */
    private Thread waiting;

    @Override
    public void attachCondition(Condition cond) {
        if (cond == null) {
            throw new IllegalArgumentException("a Condition to attach must not be null");
        }
        synchronized (monitor) {
            // Attaching a condition that is already attached has no effect.
            if (conditions.add(cond) && cond instanceof GuardConditionImpl) {
                ((GuardConditionImpl) cond).attach(this);
            }
            monitor.notifyAll();
        }
    }

    @Override
    public void detachCondition(Condition cond) {
        synchronized (monitor) {
            if (!conditions.remove(cond)) {
                throw new org.zerodds.core.PreconditionNotMetException(
                        "the Condition is not attached to this WaitSet");
            }
            if (cond instanceof GuardConditionImpl) {
                ((GuardConditionImpl) cond).detach(this);
            }
        }
    }

    @Override
    public Collection<Condition> getConditions() {
        synchronized (monitor) {
            return new ArrayList<>(conditions);
        }
    }

    /** The {@link TimeoutException} of §7.3.7.4: no condition fired in time. */
    private static TimeoutException timedOut(long millis) {
        return new TimeoutException(
                "no Condition of the WaitSet became true within " + millis + " ms");
    }

    @Override
    public void waitForConditions() {
        await(null, 0, false);
    }

    @Override
    public void waitForConditions(Collection<Condition> activeConditions) {
        await(activeConditions, 0, false);
    }

    @Override
    public void waitForConditions(Duration timeout) throws TimeoutException {
        long millis = timeout.isInfinite() ? 0 : timeout.getDuration(TimeUnit.MILLISECONDS);
        if (!await(null, millis, !timeout.isInfinite())) {
            throw timedOut(millis);
        }
    }

    @Override
    public void waitForConditions(long timeout, TimeUnit unit) throws TimeoutException {
        if (!await(null, unit.toMillis(timeout), true)) {
            throw timedOut(unit.toMillis(timeout));
        }
    }

    @Override
    public void waitForConditions(Collection<Condition> activeConditions, Duration timeout)
            throws TimeoutException {
        long millis = timeout.isInfinite() ? 0 : timeout.getDuration(TimeUnit.MILLISECONDS);
        if (!await(activeConditions, millis, !timeout.isInfinite())) {
            throw timedOut(millis);
        }
    }

    @Override
    public void waitForConditions(Collection<Condition> activeConditions, long timeout,
            TimeUnit unit) throws TimeoutException {
        if (!await(activeConditions, unit.toMillis(timeout), true)) {
            throw timedOut(unit.toMillis(timeout));
        }
    }

    /**
     * Blocks until one of the attached conditions has {@code getTriggerValue()} true
     * and places all that have into {@code active} (§7.3.7.4: "the result of the wait
     * operation is the list of all the attached conditions that have a triggerValue of
     * true"). Fails with a PreconditionNotMetException when another thread is already
     * waiting.
     *
     * @param bounded whether {@code millis} bounds the wait.
     * @return false when the time ran out with no condition true.
     */
    private boolean await(Collection<Condition> active, long millis, boolean bounded) {
        long deadline = System.currentTimeMillis() + Math.max(0, millis);
        synchronized (monitor) {
            if (waiting != null && waiting != Thread.currentThread()) {
                throw new org.zerodds.core.PreconditionNotMetException(
                        "another thread is already waiting on this WaitSet");
            }
            waiting = Thread.currentThread();
            try {
                while (true) {
                    List<Condition> fired = new ArrayList<>();
                    for (Condition cond : conditions) {
                        boolean triggered;
                        try {
                            triggered = cond.getTriggerValue();
                        } catch (org.omg.dds.core.AlreadyClosedException closedCondition) {
                            triggered = false; // a closed condition never triggers
                        }
                        if (triggered) {
                            fired.add(cond);
                        }
                    }
                    if (!fired.isEmpty()) {
                        if (active != null) {
                            active.clear();
                            active.addAll(fired);
                        }
                        return true;
                    }
                    try {
                        if (!bounded) {
                            monitor.wait();
                        } else {
                            long remaining = deadline - System.currentTimeMillis();
                            if (remaining <= 0) {
                                return false;
                            }
                            monitor.wait(remaining);
                        }
                    } catch (InterruptedException e) {
                        Thread.currentThread().interrupt();
                        return false;
                    }
                }
            } finally {
                waiting = null;
            }
        }
    }

    /** Wakes the thread blocked in this wait set so it evaluates its conditions again. */
    public void signal() {
        synchronized (monitor) {
            monitor.notifyAll();
        }
    }

    @Override
    public String toString() {
        List<String> names = new ArrayList<>();
        for (Condition cond : getConditions()) {
            names.add(cond.getClass().getSimpleName());
        }
        return "WaitSet" + names;
    }
}