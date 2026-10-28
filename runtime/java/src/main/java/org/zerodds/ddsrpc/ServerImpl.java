// SPDX-License-Identifier: Apache-2.0
package org.zerodds.ddsrpc;

import org.omg.dds.rpc.*;
import java.util.ArrayList;
import java.util.Collections;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Objects;
import java.util.Set;
import java.util.concurrent.TimeUnit;
import java.util.function.BooleanSupplier;
import org.omg.dds.core.Duration;

/** The {@link Server} that dispatches into the services added to it. */
public final class ServerImpl implements Server {

    /** Services that belong to a server, by identity, across all servers. */
    private static final Set<Service> OWNED =
            Collections.synchronizedSet(Collections.newSetFromMap(new IdentityHashMap<>()));

    private final List<Service> services = new ArrayList<>();
    /** The runtime that made this server, or {@code null} — see {@link #getEnvironment}. */
    private final RPCRuntime runtime;
    private volatile boolean closed = false;

    private final ServerParams params;

    public ServerImpl(RPCRuntime runtime) {
        this(runtime, ServerParams.create());
    }

    public ServerImpl(RPCRuntime runtime, ServerParams params) {
        this.runtime = runtime;
        this.params = Objects.requireNonNull(params, "params");
    }

    @Override
    public synchronized void add(Service service) {
        Objects.requireNonNull(service, "service");
        if (closed) {
            throw new IllegalStateException("the server was closed");
        }
        if (!OWNED.add(service)) {
            throw new IllegalStateException(
                    "a Service may belong to only one Server (§7.11.1.5.1)");
        }
        services.add(service);
    }

    @Override
    public synchronized int size() {
        return services.size();
    }

    @Override
    public synchronized int runOnce() {
        java.util.concurrent.ExecutorService executor = params.getExecutor();
        int answered = 0;
        List<java.util.concurrent.Future<Integer>> running = new ArrayList<>();
        for (Service service : services) {
            if (service.isPaused() || service.isNull()) {
                continue;
            }
            if (executor == null) {
                answered += service.dispatch();
            } else {
                running.add(executor.submit(service::dispatch));
            }
        }
        for (java.util.concurrent.Future<Integer> task : running) {
            try {
                answered += task.get();
            } catch (InterruptedException interrupted) {
                Thread.currentThread().interrupt();
                break;
            } catch (java.util.concurrent.ExecutionException failure) {
                Throwable cause = failure.getCause();
                if (cause instanceof RuntimeException) {
                    throw (RuntimeException) cause;
                }
                throw new RemoteUnknownExceptionError("a service failed: " + cause);
            }
        }
        return answered;
    }

    @Override
    public void run(BooleanSupplier until) {
        while (!until.getAsBoolean()) {
            runOnce();
        }
    }

    @Override
    public void run() {
        while (!closed && !Thread.currentThread().isInterrupted()) {
            if (runOnce() == 0 && !pause()) {
                return;
            }
        }
    }

    @Override
    public void run(Duration maxWait) {
        long nanos = maxWait.getDuration(TimeUnit.NANOSECONDS);
        long start = System.nanoTime();
        while (!closed && !Thread.currentThread().isInterrupted()) {
            if (runOnce() > 0) {
                return;
            }
            // The difference of two readings of nanoTime, which is
            // meaningful across the wrap-around; the infinite duration is
            // Long.MAX_VALUE and is never reached.
            if (System.nanoTime() - start >= nanos) {
                return;
            }
            if (!pause()) {
                return;
            }
        }
    }

    /** Give the readers a moment to receive; false when interrupted. */
    private static boolean pause() {
        try {
            Thread.sleep(1L);
            return true;
        } catch (InterruptedException interrupted) {
            Thread.currentThread().interrupt();
            return false;
        }
    }

    @Override
    public ServiceParams getDefaultServiceParams() {
        if (params.getDefaultServiceParams() != null) {
            return params.getDefaultServiceParams().clone();
        }
        return ServiceParams.create()
                .domainParticipant(runtime == null ? null : runtime.domainParticipant());
    }

    @Override
    public synchronized void close() {
        closed = true;
        for (Service service : services) {
            service.close();
            OWNED.remove(service);
        }
        services.clear();
    }

    @Override
    public boolean isNull() {
        return closed;
    }

    @Override
    public org.omg.dds.core.ServiceEnvironment getEnvironment() {
        if (runtime == null) {
            return Server.super.getEnvironment();
        }
        return runtime;
    }
}
