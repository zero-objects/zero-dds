// SPDX-License-Identifier: Apache-2.0
package org.zerodds.ddsrpc;

import org.omg.dds.core.ModifiableInstanceHandle;
import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.core.Time;
import org.omg.dds.rpc.Sample;
import org.omg.dds.rpc.SampleIdentity;
import org.omg.dds.sub.InstanceState;
import org.omg.dds.sub.SampleState;
import org.omg.dds.sub.ViewState;
import org.zerodds.core.Values;

/**
 * A sample of a request or reply topic that carries no data — {@code valid_data} false
 * (DDS 1.4 §2.2.2.5.1): it tells of a change of the instance only, a dispose or the going away
 * of its writers. A requester or replier hands it over when {@code receiveNondataSamples(true)}
 * asked for it (DDS-RPC 1.0 §7.11.1.4.3, §7.11.2): {@link #validData()} is {@code false},
 * {@link #getData()} {@code null}, and {@link #getInstanceState()} the state the instance is in.
 * It answers no request, so its related identity is {@link SampleIdentity#UNKNOWN}.
 *
 * @param <T> the topic type
 */
public final class NondataSample<T> implements Sample<T> {

    private static final long serialVersionUID = 1L;

    // Java serialization (DDS-Java-PSM 1.0 §7.3.3) of an RPC sample is the PSM's matter; the
    // identity is the RPC layer's and not serializable.
    @SuppressWarnings("serial")
    private final SampleIdentity identity;
    private final InstanceState instanceState;

    /**
     * @param identity the identity the binding reported for the sample, or {@code null}
     * @param instanceState the state of the instance
     */
    public NondataSample(SampleIdentity identity, InstanceState instanceState) {
        this.identity = identity == null ? SampleIdentity.UNKNOWN : identity;
        this.instanceState = java.util.Objects.requireNonNull(instanceState, "instanceState");
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return Values.getDefaultEnvironment();
    }

    @Override
    public T getData() {
        return null;
    }

    @Override
    public SampleIdentity identity() {
        return identity;
    }

    @Override
    public SampleIdentity relatedIdentity() {
        return SampleIdentity.UNKNOWN;
    }

    @Override
    public boolean validData() {
        return false;
    }

    @Override
    public ModifiableInstanceHandle getInstanceHandle() {
        return Values.nil().modifiableCopy();
    }

    @Override
    public ModifiableInstanceHandle getPublicationHandle() {
        return Values.nil().modifiableCopy();
    }

    @Override
    public Time getSourceTimestamp() {
        return org.zerodds.core.TimeImpl.invalid(getEnvironment());
    }

    @Override
    public SampleState getSampleState() {
        return SampleState.NOT_READ;
    }

    @Override
    public ViewState getViewState() {
        return ViewState.NOT_NEW;
    }

    @Override
    public InstanceState getInstanceState() {
        return instanceState;
    }

    @Override
    public int getDisposedGenerationCount() {
        return 0;
    }

    @Override
    public int getNoWritersGenerationCount() {
        return 0;
    }

    @Override
    public int getSampleRank() {
        return -1;
    }

    @Override
    public int getGenerationRank() {
        return -1;
    }

    @Override
    public int getAbsoluteGenerationRank() {
        return -1;
    }

    @Override
    public Sample<T> clone() {
        return new NondataSample<>(identity, instanceState);
    }

    @Override
    public String toString() {
        return "rpc.Sample[no data, " + instanceState + "]";
    }
}
