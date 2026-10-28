// SPDX-License-Identifier: Apache-2.0
package org.zerodds.sub;

import java.util.Collections;
import java.util.EnumSet;
import java.util.LinkedHashSet;
import java.util.Set;

import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.sub.InstanceState;
import org.omg.dds.sub.SampleState;
import org.omg.dds.sub.Subscriber;
import org.omg.dds.sub.ViewState;

/**
 * ZeroDDS {@link Subscriber.DataState} (DDS-Java-PSM 1.0 §7.7.3): the three
 * state sets {@code read}/{@code take} select on.
 *
 * <p>A new data state holds all states — {@link #withAnySampleState()} and
 * the two siblings are what a fresh one starts as, so a plain
 * {@code read}/{@code take} returns every sample the reader holds.
 */
public final class DataStateImpl implements Subscriber.DataState {

    private final ServiceEnvironment environment;
    private final Set<SampleState> sampleStates = new LinkedHashSet<>();
    private final Set<ViewState> viewStates = new LinkedHashSet<>();
    private final Set<InstanceState> instanceStates = new LinkedHashSet<>();

    /** A data state that selects on nothing. */
    public DataStateImpl(ServiceEnvironment environment) {
        this.environment = environment;
    }

    /** A data state that selects on all three states (the read/take default). */
    public static DataStateImpl any(ServiceEnvironment environment) {
        DataStateImpl state = new DataStateImpl(environment);
        state.withAnySampleState();
        state.withAnyViewState();
        state.withAnyInstanceState();
        return state;
    }

    /** A data state over the three sets of a {@code ReadCondition}. */
    public static DataStateImpl of(ServiceEnvironment environment,
            Set<SampleState> sampleStates,
            Set<ViewState> viewStates,
            Set<InstanceState> instanceStates) {
        DataStateImpl state = new DataStateImpl(environment);
        state.sampleStates.addAll(sampleStates);
        state.viewStates.addAll(viewStates);
        state.instanceStates.addAll(instanceStates);
        return state;
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public Set<SampleState> getSampleStates() {
        return Collections.unmodifiableSet(sampleStates);
    }

    @Override
    public Set<ViewState> getViewStates() {
        return Collections.unmodifiableSet(viewStates);
    }

    @Override
    public Set<InstanceState> getInstanceStates() {
        return Collections.unmodifiableSet(instanceStates);
    }

    @Override
    public DataStateImpl with(SampleState state) {
        sampleStates.add(state);
        return this;
    }

    @Override
    public DataStateImpl with(ViewState state) {
        viewStates.add(state);
        return this;
    }

    @Override
    public DataStateImpl with(InstanceState state) {
        instanceStates.add(state);
        return this;
    }

    @Override
    public DataStateImpl withAnySampleState() {
        sampleStates.addAll(EnumSet.allOf(SampleState.class));
        return this;
    }

    @Override
    public DataStateImpl withAnyViewState() {
        viewStates.addAll(EnumSet.allOf(ViewState.class));
        return this;
    }

    @Override
    public DataStateImpl withAnyInstanceState() {
        instanceStates.addAll(EnumSet.allOf(InstanceState.class));
        return this;
    }

    @Override
    public DataStateImpl withNotAliveInstanceStates() {
        instanceStates.add(InstanceState.NOT_ALIVE_DISPOSED);
        instanceStates.add(InstanceState.NOT_ALIVE_NO_WRITERS);
        return this;
    }

    @Override
    public DataStateImpl clone() {
        return of(environment, sampleStates, viewStates, instanceStates);
    }

    @Override
    public boolean equals(Object other) {
        if (this == other) return true;
        if (!(other instanceof DataStateImpl)) return false;
        DataStateImpl o = (DataStateImpl) other;
        return sampleStates.equals(o.sampleStates)
                && viewStates.equals(o.viewStates)
                && instanceStates.equals(o.instanceStates);
    }

    @Override
    public int hashCode() {
        return sampleStates.hashCode() + viewStates.hashCode() + instanceStates.hashCode();
    }

    @Override
    public String toString() {
        return "DataState[" + sampleStates + "," + viewStates + "," + instanceStates + "]";
    }
}