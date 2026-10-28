// SPDX-License-Identifier: Apache-2.0
package org.zerodds.internal;

import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.ScheduledFuture;
import java.util.concurrent.ThreadFactory;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicInteger;

/**
 * Shared lease scheduler for the pure-Java PSM.
 *
 * <p>DDS 1.2 §7.1.4.2: listeners are the mechanism by which the middleware
 * <em>asynchronously</em> alerts the application of a status change. Deadline
 * misses and liveliness loss are time-based events, so they cannot be detected
 * on the delivering thread alone; they need a periodic lease that wakes up and
 * evaluates them even when the application never polls the status. This class
 * provides the daemon-thread scheduler those leases run on.
 *
 * <p>Each participating entity schedules its own task at the finest of its
 * finite deadlines/leases; entities whose DEADLINE period and LIVELINESS
 * {@code lease_duration} are both infinite (the spec defaults) schedule
 * nothing, so the common case costs no threads and no timer wake-ups.
 */
public final class LeaseMonitor {

    private static final ScheduledExecutorService EXECUTOR;

    static {
        final AtomicInteger counter = new AtomicInteger();
        ThreadFactory factory = r -> {
            Thread t = new Thread(r, "zerodds-lease-" + counter.incrementAndGet());
            t.setDaemon(true);
            return t;
        };
        EXECUTOR = Executors.newScheduledThreadPool(2, factory);
    }

    private LeaseMonitor() {}

    /**
     * Runs {@code task} once every {@code periodMillis} on a daemon lease
     * thread. The returned future can be cancelled when the entity closes.
     */
    public static ScheduledFuture<?> scheduleAtFixedRate(Runnable task, long periodMillis) {
        return EXECUTOR.scheduleAtFixedRate(
                task, periodMillis, periodMillis, TimeUnit.MILLISECONDS);
    }
}
