// SPDX-License-Identifier: Apache-2.0
package org.zerodds.sub;

import org.omg.dds.core.InstanceHandle;
import org.omg.dds.core.ModifiableInstanceHandle;
import org.omg.dds.core.ServiceEnvironment;
import org.zerodds.ServiceEnvironmentImpl;
import org.omg.dds.core.Time;
import org.omg.dds.sub.InstanceState;
import org.omg.dds.sub.Sample;
import org.omg.dds.sub.SampleState;
import org.omg.dds.sub.ViewState;

/**
 * ZeroDDS {@link Sample} (DDS-Java-PSM 1.0 §7.7.2).
 *
 * <p>The PSM represents a data sample as one object carrying both the data
 * and its metadata, handed out as Java-Bean properties. §7.7.2's
 * {@code SampleInfo} of the IDL PSM maps one to one:
 *
 * <ul>
 *     <li>{@code valid_data == false} → {@link #getData()} is {@code null};</li>
 *     <li>{@code sample_state}, {@code view_state} and {@code instance_state}
 *         are the three states, snapshotted when the collection was
 *         obtained (DDS 1.2 §7.1.2.5.1.2, §7.1.2.5.1.3 and §7.1.2.5.1.8);</li>
 *     <li>{@code instance_handle}, {@code source_timestamp} and
 *         {@code publication_handle} are the handles and the timestamp;</li>
 *     <li>{@code disposed_generation_count}, {@code no_writers_generation_count}
 *         and the three ranks are the instance bookkeeping.</li>
 * </ul>
 *
 * <p>Immutable: a sample describes the state at the moment read/take
 * collected it.
 */
public final class SampleImpl<TYPE> implements Sample<TYPE> {

    private static final long serialVersionUID = 1L;

    private transient ServiceEnvironment environment;
    // The value is the application's sample type. DDS-Java-PSM 1.0 §8.1 makes a
    // type that implements Serializable publishable; a sample of a type that
    // does not is not serializable, and writing it throws NotSerializableException.
    @SuppressWarnings("serial")
    private TYPE data;
    private SampleState sampleState;
    private ViewState viewState;
    private InstanceState instanceState;
    private Time sourceTimestamp;
    private ModifiableInstanceHandle instanceHandle;
    private ModifiableInstanceHandle publicationHandle;
    private int disposedGenerationCount;
    private int noWritersGenerationCount;
    private int sampleRank;
    private int generationRank;
    private int absoluteGenerationRank;

    public SampleImpl(ServiceEnvironment environment,
            TYPE data,
            ModifiableInstanceHandle instanceHandle,
            ModifiableInstanceHandle publicationHandle,
            Time sourceTimestamp,
            SampleState sampleState,
            ViewState viewState,
            InstanceState instanceState,
            int disposedGenerationCount,
            int noWritersGenerationCount,
            int sampleRank,
            int generationRank,
            int absoluteGenerationRank) {
        this.environment = environment;
        this.data = data;
        this.instanceHandle = instanceHandle;
        this.publicationHandle = publicationHandle;
        this.sourceTimestamp = sourceTimestamp;
        this.sampleState = sampleState;
        this.viewState = viewState;
        this.instanceState = instanceState;
        this.disposedGenerationCount = disposedGenerationCount;
        this.noWritersGenerationCount = noWritersGenerationCount;
        this.sampleRank = sampleRank;
        this.generationRank = generationRank;
        this.absoluteGenerationRank = absoluteGenerationRank;
    }

    private org.zerodds.internal.SourceIdentity identity;
    private org.zerodds.internal.SourceIdentity related;
    private byte[] serialized;

    /**
     * Records the octets the sample arrived as, for a reader whose type support retains them
     * ({@code TopicTypeSupport.retainsSerialized()}).
     */
    public void setSerialized(byte[] octets) {
        this.serialized = octets;
    }

    /** The octets the sample arrived as, or {@code null} when the reader does not retain them. */
    public byte[] serialized() {
        return serialized == null ? null : serialized.clone();
    }

    /** Records the identity of the sample and of the one it relates to (DDS-RPC 1.0 §7.8.2). */
    public void setIdentity(org.zerodds.internal.SourceIdentity identity,
            org.zerodds.internal.SourceIdentity related) {
        this.identity = identity;
        this.related = related;
    }

    /** The writer GUID and sequence number the sample was sent under, or {@code null}. */
    public org.zerodds.internal.SourceIdentity identity() {
        return identity;
    }

    /** The identity of the sample this one answers or follows, or {@code null}. */
    public org.zerodds.internal.SourceIdentity relatedIdentity() {
        return related;
    }

    /** A sample without the instance bookkeeping: generation counts 0, ranks -1. */
    public static <TYPE> SampleImpl<TYPE> of(ServiceEnvironment environment,
            TYPE data,
            InstanceHandle instanceHandle,
            InstanceHandle publicationHandle,
            Time sourceTimestamp,
            SampleState sampleState,
            ViewState viewState,
            InstanceState instanceState) {
        ModifiableInstanceHandle instance =
                instanceHandle == null ? null : instanceHandle.modifiableCopy();
        ModifiableInstanceHandle publication =
                publicationHandle == null ? null : publicationHandle.modifiableCopy();
        return new SampleImpl<>(environment, data, instance, publication, sourceTimestamp,
                sampleState, viewState, instanceState, 0, 0, -1, -1, -1);
    }

    /** A sample with its generation counts and ranks (DDS 1.4 §2.2.2.5.1.5, §2.2.2.5.1.6). */
    public static <TYPE> SampleImpl<TYPE> of(ServiceEnvironment environment,
            TYPE data,
            InstanceHandle instanceHandle,
            InstanceHandle publicationHandle,
            Time sourceTimestamp,
            SampleState sampleState,
            ViewState viewState,
            InstanceState instanceState,
            int disposedGenerationCount,
            int noWritersGenerationCount,
            int sampleRank,
            int generationRank,
            int absoluteGenerationRank) {
        ModifiableInstanceHandle instance =
                instanceHandle == null ? null : instanceHandle.modifiableCopy();
        ModifiableInstanceHandle publication =
                publicationHandle == null ? null : publicationHandle.modifiableCopy();
        return new SampleImpl<>(environment, data, instance, publication, sourceTimestamp,
                sampleState, viewState, instanceState, disposedGenerationCount,
                noWritersGenerationCount, sampleRank, generationRank, absoluteGenerationRank);
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public TYPE getData() {
        return data;
    }

    @Override
    public SampleState getSampleState() {
        return sampleState;
    }

    @Override
    public ViewState getViewState() {
        return viewState;
    }

    @Override
    public InstanceState getInstanceState() {
        return instanceState;
    }

    @Override
    public Time getSourceTimestamp() {
        return sourceTimestamp;
    }

    @Override
    public ModifiableInstanceHandle getInstanceHandle() {
        return instanceHandle;
    }

    @Override
    public ModifiableInstanceHandle getPublicationHandle() {
        return publicationHandle;
    }

    @Override
    public int getDisposedGenerationCount() {
        return disposedGenerationCount;
    }

    @Override
    public int getNoWritersGenerationCount() {
        return noWritersGenerationCount;
    }

    @Override
    public int getSampleRank() {
        return sampleRank;
    }

    @Override
    public int getGenerationRank() {
        return generationRank;
    }

    @Override
    public int getAbsoluteGenerationRank() {
        return absoluteGenerationRank;
    }

    /**
     * Overwrites this sample's content with another's — what
     * {@code readNextSample}/{@code takeNextSample} of §7.7.3 do with the
     * sample the application handed in.
     */
    public void copyInto(SampleImpl<TYPE> other) {
        other.data = data;
        other.sampleState = sampleState;
        other.viewState = viewState;
        other.instanceState = instanceState;
        other.sourceTimestamp = sourceTimestamp;
        other.instanceHandle = instanceHandle;
        other.publicationHandle = publicationHandle;
        other.disposedGenerationCount = disposedGenerationCount;
        other.noWritersGenerationCount = noWritersGenerationCount;
        other.sampleRank = sampleRank;
        other.generationRank = generationRank;
        other.absoluteGenerationRank = absoluteGenerationRank;
    }

    @Override
    public Sample<TYPE> clone() {
        return new SampleImpl<>(environment, data, instanceHandle, publicationHandle,
                sourceTimestamp, sampleState, viewState, instanceState,
                disposedGenerationCount, noWritersGenerationCount,
                sampleRank, generationRank, absoluteGenerationRank);
    }

    @Override
    public String toString() {
        return "Sample[" + sampleState + "," + viewState + "," + instanceState
                + "," + data + "]";
    }

    /**
     * Java serialization (DDS-Java-PSM 1.0 §7.3.3) carries the value, not its
     * {@link ServiceEnvironment}: an environment is a runtime object and is not
     * serializable. A deserialized object belongs to the default environment of
     * this implementation.
     */
    private void readObject(java.io.ObjectInputStream in)
            throws java.io.IOException, ClassNotFoundException {
        in.defaultReadObject();
        environment = ServiceEnvironmentImpl.getDefaultEnvironment();
    }
}