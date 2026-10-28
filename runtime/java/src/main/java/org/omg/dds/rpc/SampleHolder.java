// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

import org.omg.dds.core.ModifiableInstanceHandle;
import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.core.Time;
import org.omg.dds.sub.InstanceState;
import org.omg.dds.sub.SampleState;
import org.omg.dds.sub.ViewState;

/**
 * A {@link Sample} that a call fills in, for the operations of
 * {@code omg-dds-rpc-java.zip} that take the sample as an argument
 * ({@link Replier#receiveRequest}, {@link Replier#takeRequest},
 * {@link Replier#readRequest}): the specification's Java file passes the sample in, as the
 * C++ binding passes a reference.
 *
 * <p>Before a call fills it, {@link #validData()} is {@code false} and {@link #getData()} is
 * {@code null}.
 *
 * @param <T> the topic type
 */
public final class SampleHolder<T> implements Sample<T> {

    private static final long serialVersionUID = 1L;

    private Sample<T> filled;

    /** An empty holder. */
    public SampleHolder() {}

    /** Takes the contents of {@code from}; called by the runtime that completes the call. */
    public void fill(Sample<T> from) {
        this.filled = from;
    }

    private Sample<T> inner() {
        if (filled == null) {
            throw new IllegalStateException("the sample was not filled by a call");
        }
        return filled;
    }

    @Override
    public SampleIdentity identity() {
        return filled == null ? SampleIdentity.UNKNOWN : filled.identity();
    }

    @Override
    public SampleIdentity relatedIdentity() {
        return filled == null ? SampleIdentity.UNKNOWN : filled.relatedIdentity();
    }

    @Override
    public boolean validData() {
        return filled != null && filled.validData();
    }

    @Override
    public T getData() {
        return filled == null ? null : filled.getData();
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return inner().getEnvironment();
    }

    @Override
    public ModifiableInstanceHandle getInstanceHandle() {
        return inner().getInstanceHandle();
    }

    @Override
    public ModifiableInstanceHandle getPublicationHandle() {
        return inner().getPublicationHandle();
    }

    @Override
    public Time getSourceTimestamp() {
        return inner().getSourceTimestamp();
    }

    @Override
    public SampleState getSampleState() {
        return inner().getSampleState();
    }

    @Override
    public ViewState getViewState() {
        return inner().getViewState();
    }

    @Override
    public InstanceState getInstanceState() {
        return inner().getInstanceState();
    }

    @Override
    public int getDisposedGenerationCount() {
        return inner().getDisposedGenerationCount();
    }

    @Override
    public int getNoWritersGenerationCount() {
        return inner().getNoWritersGenerationCount();
    }

    @Override
    public int getSampleRank() {
        return inner().getSampleRank();
    }

    @Override
    public int getGenerationRank() {
        return inner().getGenerationRank();
    }

    @Override
    public int getAbsoluteGenerationRank() {
        return inner().getAbsoluteGenerationRank();
    }

    @Override
    public Sample<T> clone() {
        return Sample.of(inner().getData(), identity(), relatedIdentity());
    }

    @Override
    public String toString() {
        return filled == null ? "rpc.SampleHolder[empty]" : filled.toString();
    }
}
