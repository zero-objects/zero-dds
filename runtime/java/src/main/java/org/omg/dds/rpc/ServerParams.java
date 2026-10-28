// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

import java.util.concurrent.ExecutorService;

/**
 * The configuration of a {@link Server} — the {@code ServerParams} of
 * {@code omg-dds-rpc-java.zip} (DDS-RPC 1.0 §7.11.1.5.3, §7.11.2): the
 * {@link ServiceParams} every service added to the server starts from, and
 * the executor the server dispatches on.
 */
public interface ServerParams {

    /** Empty params: no default service params, no executor. */
    static ServerParams create() {
        return new ServerParams() {
            private ServiceParams defaults;
            private ExecutorService executor;

            @Override
            public ServerParams withDefaultServiceParams(ServiceParams serviceParams) {
                this.defaults = serviceParams;
                return this;
            }

            @Override
            public ServerParams withExecutor(ExecutorService executor) {
                this.executor = executor;
                return this;
            }

            @Override
            public ServiceParams getDefaultServiceParams() {
                return defaults;
            }

            @Override
            public ExecutorService getExecutor() {
                return executor;
            }
        };
    }

    /** The params of the services the server makes where none are given. */
    ServerParams withDefaultServiceParams(ServiceParams serviceParams);

    /**
     * The executor the server dispatches on: each service of one dispatch round runs as one
     * task, and the round ends when all of them have.
     */
    ServerParams withExecutor(ExecutorService executor);

    /** The default service params, or {@code null}. */
    ServiceParams getDefaultServiceParams();

    /** The executor, or {@code null}: the server dispatches on the calling thread. */
    ExecutorService getExecutor();
}
