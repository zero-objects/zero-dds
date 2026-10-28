// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core.condition;

import java.util.Collections;
import java.util.LinkedHashSet;
import java.util.Set;
import java.util.function.BooleanSupplier;

import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.InstanceState;
import org.omg.dds.sub.ReadCondition;
import org.omg.dds.sub.SampleState;
import org.omg.dds.sub.ViewState;

/**
 * ZeroDDS {@link ReadCondition} (DDS-Java-PSM 1.0 §7.3.7.3).
 *
 * <p>The trigger value is true while the reader holds a sample whose
 * {@code sample_state}, {@code view_state} and {@code instance_state} are all
 * contained in the sets the condition was created with (DDS 1.2 §7.1.2.5).
 */
public class ReadConditionImpl<TYPE> implements ReadCondition<TYPE> {

    private final ServiceEnvironment environment;
    private final DataReader<TYPE> parent;
    private final Set<SampleState> sampleStates;
    private final Set<ViewState> viewStates;
    private final Set<InstanceState> instanceStates;
    private final BooleanSupplier trigger;
    private volatile boolean closed;
    private volatile java.util.function.Consumer<ReadConditionImpl<TYPE>> closeHook;

    public ReadConditionImpl(ServiceEnvironment environment,
            DataReader<TYPE> parent,
            Set<SampleState> sampleStates,
            Set<ViewState> viewStates,
            Set<InstanceState> instanceStates,
            BooleanSupplier trigger) {
        this.environment = environment;
        this.parent = parent;
        this.sampleStates = Collections.unmodifiableSet(new LinkedHashSet<>(sampleStates));
        this.viewStates = Collections.unmodifiableSet(new LinkedHashSet<>(viewStates));
        this.instanceStates = Collections.unmodifiableSet(new LinkedHashSet<>(instanceStates));
        this.trigger = trigger;
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public Set<SampleState> getSampleStates() {
        return sampleStates;
    }

    @Override
    public Set<ViewState> getViewStates() {
        return viewStates;
    }

    @Override
    public Set<InstanceState> getInstanceStates() {
        return instanceStates;
    }

    @Override
    public DataReader<TYPE> getParent() {
        return parent;
    }

    @Override
    public boolean getTriggerValue() {
        ensureOpen();
        return trigger.getAsBoolean();
    }

    /**
     * What the reader does when the condition is closed: it forgets the condition
     * (delete_readcondition, DDS 1.4 §2.2.2.5.3.5).
     */
    public void onClose(java.util.function.Consumer<ReadConditionImpl<TYPE>> hook) {
        this.closeHook = hook;
    }

    /** RETCODE_ALREADY_DELETED (DDS-Java-PSM 1.0 Table 7.1) for a condition that was closed. */
    public void ensureOpen() {
        if (closed) {
            throw new org.zerodds.core.AlreadyClosedException("the ReadCondition is closed");
        }
    }

    /** Whether {@link #close()} was called. */
    public boolean isClosed() {
        return closed;
    }

    @Override
    public void close() {
        if (closed) {
            return;
        }
        closed = true;
        java.util.function.Consumer<ReadConditionImpl<TYPE>> hook = closeHook;
        if (hook != null) {
            hook.accept(this);
        }
    }

    @Override
    public String toString() {
        return getClass().getSimpleName()
                + (closed ? "[closed]" : "[triggerValue=" + getTriggerValue() + "]");
    }
}