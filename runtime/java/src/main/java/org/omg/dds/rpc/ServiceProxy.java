// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

import java.util.Collection;
import java.util.concurrent.CompletableFuture;
import org.omg.dds.core.Duration;

/**
 * DDS-RPC 1.0 §7.11.1.4.4 through §7.11.2.2.1: "ServiceProxy class
 * defines type-independent operations for Requester and the Client. For
 * example, binding to a specific instance, waiting for an instance to
 * discover, closing the service, and more. ServiceProxy shall not be
 * instantiated directly."
 *
 * <p>The waiting operations of {@code omg-dds-rpc-java.zip} are built on
 * {@link #waitForService()} and {@link #getDiscoveredServiceInstances()}: a
 * proxy that implements those two has the rest.
 */
public interface ServiceProxy extends RPCEntity {

    /**
     * §7.11.1.4.3: "A requester reference may be bound to a specific
     * service instance. Requests sent through a bound requester
     * reference shall be sent to the bound service instance only."
     *
     * @return {@code true} once the proxy is bound to {@code instanceName}
     */
    boolean bind(String instanceName);

    /** Undoes {@link #bind}; {@code true} when the proxy was bound. */
    boolean unbind();

    boolean isBound();

    String boundInstanceName();

    /** The name of the bound instance, {@code ""} when none (the Java binding's spelling). */
    default String getBoundInstanceName() {
        return boundInstanceName();
    }

    /**
     * Whether service matching (§7.6.2.2) is complete now: for a bound proxy with the bound
     * instance, otherwise with any. Does not wait; the {@code waitFor…} operations wait for it.
     */
    boolean isServiceMatched();

    /**
     * The service instances discovered so far: the names the matched service-side readers
     * announced in {@code service_instance_name} (§7.6.2.1.2), without the empty name.
     *
     * @throws RemoteUnsupportedError for a binding that sees no instance names
     */
    default Collection<String> getDiscoveredServiceInstances() {
        throw new RemoteUnsupportedError(
                "this binding does not see service_instance_name (§7.6.2.1)");
    }

    /**
     * Waits until service matching is complete ({@link #isServiceMatched()}), without a time
     * limit — the {@code wait_for_service()} of §7.11.1.4.4.
     */
    default void waitForService() {
        waitForService((Duration) null);
    }

    /**
     * Waits until service matching is complete, for at most {@code maxWait} ({@code null} or
     * the infinite duration: without a limit).
     *
     * @throws TimeoutError when {@code maxWait} ran out first
     */
    default void waitForService(Duration maxWait) {
        await(maxWait, this::isServiceMatched, "no service was matched");
    }

    /** Waits until the service instance {@code instanceName} is discovered. */
    default void waitForService(String instanceName) {
        waitForService(null, instanceName);
    }

    /**
     * As {@link #waitForService(String)}, for at most {@code maxWait}.
     *
     * @throws TimeoutError when {@code maxWait} ran out first
     */
    default void waitForService(Duration maxWait, String instanceName) {
        waitForServices(maxWait, instanceName);
    }

    /** Waits until at least {@code count} service instances are discovered. */
    default void waitForServices(int count) {
        waitForServices(null, count);
    }

    /**
     * As {@link #waitForServices(int)}, for at most {@code maxWait}.
     *
     * @throws TimeoutError when {@code maxWait} ran out first
     */
    default void waitForServices(Duration maxWait, int count) {
        await(maxWait, () -> getDiscoveredServiceInstances().size() >= count,
                "fewer than " + count + " service instances were discovered");
    }

    /** Waits until every named service instance is discovered. */
    default void waitForServices(String... instanceNames) {
        waitForServices(null, instanceNames);
    }

    /**
     * As {@link #waitForServices(String...)}, for at most {@code maxWait}.
     *
     * @throws TimeoutError when {@code maxWait} ran out first
     */
    default void waitForServices(Duration maxWait, String... instanceNames) {
        await(maxWait,
                () -> getDiscoveredServiceInstances().containsAll(java.util.Arrays.asList(instanceNames)),
                "not every one of " + java.util.Arrays.toString(instanceNames) + " was discovered");
    }

    /**
     * The future of {@link #waitForService()}: ready when a service is matched. The four
     * {@code …Async} forms return the {@code java.util.concurrent.Future} of the
     * machine-readable {@code ServiceProxy.java}; the object is an
     * {@link Future org.omg.dds.rpc.Future}.
     */
    default java.util.concurrent.Future<Void> waitForServiceAsync() {
        return ServiceProxyWaits.later(() -> pollUntil(null, this::isServiceMatched));
    }

    default java.util.concurrent.Future<Void> waitForServiceAsync(String instanceName) {
        return ServiceProxyWaits.later(
                () -> pollUntil(null, () -> getDiscoveredServiceInstances().contains(instanceName)));
    }

    default java.util.concurrent.Future<Void> waitForServicesAsync(int count) {
        return ServiceProxyWaits.later(
                () -> pollUntil(null, () -> getDiscoveredServiceInstances().size() >= count));
    }

    default java.util.concurrent.Future<Void> waitForServicesAsync(String... instanceNames) {
        return ServiceProxyWaits.later(
                () -> pollUntil(null, () -> getDiscoveredServiceInstances()
                        .containsAll(java.util.Arrays.asList(instanceNames))));
    }

    /**
     * Polls {@code condition} until it holds; throws {@link TimeoutError} with {@code what}
     * when {@code maxWait} ran out first.
     */
    static void await(Duration maxWait, java.util.function.BooleanSupplier condition, String what) {
        if (!pollUntil(maxWait, condition)) {
            throw new TimeoutError(what + " within the wait");
        }
    }

    /**
     * Polls {@code condition} until it holds, or {@code maxWait} has passed. The
     * infinite duration and {@code null} do not end.
     */
    static boolean pollUntil(Duration maxWait, java.util.function.BooleanSupplier condition) {
        long nanos =
                maxWait == null || maxWait.isInfinite()
                        ? Long.MAX_VALUE
                        : maxWait.getDuration(java.util.concurrent.TimeUnit.NANOSECONDS);
        long start = System.nanoTime();
        while (true) {
            if (condition.getAsBoolean()) {
                return true;
            }
            if (System.nanoTime() - start >= nanos) {
                return false;
            }
            try {
                Thread.sleep(1L);
            } catch (InterruptedException interrupted) {
                Thread.currentThread().interrupt();
                return false;
            }
        }
    }
}
