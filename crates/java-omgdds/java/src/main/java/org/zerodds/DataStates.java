// SPDX-License-Identifier: Apache-2.0
package org.zerodds;

import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.sub.InstanceState;
import org.omg.dds.sub.SampleState;
import org.omg.dds.sub.Subscriber;
import org.omg.dds.sub.ViewState;
import org.zerodds.sub.DataStateImpl;

/**
 * The {@link Subscriber.DataState} selections of DDS-Java-PSM 1.0 §7.7.3, as
 * named constants: the five sets the clause lists for {@code read}/{@code take}.
 */
public final class DataStates {

    private DataStates() {}

    /** Every sample, view and instance state — what a plain read/take selects. */
    public static Subscriber.DataState all(ServiceEnvironment environment) {
        return DataStateImpl.any(environment);
    }

    /** Only samples the application has not read yet (§7.1.2.5.1.2). */
    public static Subscriber.DataState notRead(ServiceEnvironment environment) {
        // with() adds a state to the set, so this starts from the empty one
        return new DataStateImpl(environment)
                .with(SampleState.NOT_READ)
                .withAnyViewState()
                .withAnyInstanceState();
    }

    /** Only samples of instances that are no longer alive (§7.1.2.4.2.7). */
    public static Subscriber.DataState notAlive(ServiceEnvironment environment) {
        return new DataStateImpl(environment)
                .withAnySampleState()
                .withAnyViewState()
                .withNotAliveInstanceStates();
    }

    /** Only samples of an instance the application has not seen yet (§7.1.2.5.1.8). */
    public static Subscriber.DataState newInstances(ServiceEnvironment environment) {
        return new DataStateImpl(environment)
                .withAnySampleState()
                .with(ViewState.NEW)
                .withAnyInstanceState();
    }

    /** Only samples of instances that are still alive (§7.1.2.4.2.6). */
    public static Subscriber.DataState alive(ServiceEnvironment environment) {
        return new DataStateImpl(environment)
                .withAnySampleState()
                .withAnyViewState()
                .with(InstanceState.ALIVE);
    }
}
