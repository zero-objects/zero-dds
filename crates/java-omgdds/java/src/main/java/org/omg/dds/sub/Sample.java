// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.sub;

import org.omg.dds.core.InstanceHandle;
import org.omg.dds.core.Time;

/**
 * OMG DDS Java-PSM Sample — Spec §7.2.4.
 *
 * <p>Encapsulates the data + SampleInfo (timestamp + instance handle +
 * read state).
 */
public final class Sample<T> {
    public enum SampleState { READ, NOT_READ }
    public enum ViewState { NEW, NOT_NEW }
    public enum InstanceState { ALIVE, NOT_ALIVE_DISPOSED, NOT_ALIVE_NO_WRITERS }

    private final T data;
    private final InstanceHandle instanceHandle;
    private final Time sourceTimestamp;
    private final SampleState sampleState;
    private final ViewState viewState;
    private final InstanceState instanceState;

    public Sample(T data, InstanceHandle instanceHandle, Time sourceTimestamp,
                  SampleState sampleState, ViewState viewState, InstanceState instanceState) {
        this.data = data;
        this.instanceHandle = instanceHandle;
        this.sourceTimestamp = sourceTimestamp;
        this.sampleState = sampleState;
        this.viewState = viewState;
        this.instanceState = instanceState;
    }

    public T data() { return data; }

    /** Spec §7.7 accessor alias for {@link #data()} (JavaBean / {@code get*} form). */
    public T getData() { return data; }
    public InstanceHandle instanceHandle() { return instanceHandle; }
    public Time sourceTimestamp() { return sourceTimestamp; }
    public SampleState sampleState() { return sampleState; }
    public ViewState viewState() { return viewState; }
    public InstanceState instanceState() { return instanceState; }
}
