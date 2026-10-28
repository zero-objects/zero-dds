// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

import org.omg.dds.core.InstanceHandle;
import org.omg.dds.core.ModifiableInstanceHandle;
import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.core.Time;
import org.omg.dds.sub.InstanceState;
import org.omg.dds.sub.SampleState;
import org.omg.dds.sub.ViewState;
import org.zerodds.core.Values;

/**
 * DDS-RPC 1.0 §7.11.2.2.6: "The rpc.Sample&lt;T&gt; interface extends
 * dds.sub.Sample&lt;T&gt; interface and defines two additional operations
 * to retrieve the SampleIdentity and the related sample identity, if
 * any."
 *
 * <p>The supertype is [DDS-Java-PSM] §7.7.2's {@code Sample}, which that
 * clause declares an interface ("Each sample is represented by an
 * instance of the {@code org.omg.dds.sub.Sample} interface"), and the two
 * added operations are {@link #identity()} and
 * {@link #relatedIdentity()}. The data accessor, the instance handle, the
 * source timestamp and the three states come from the supertype.
 *
 * @param <T> the topic type
 */
public interface Sample<T> extends org.omg.dds.sub.Sample<T> {

    /** The identity the writer gave this sample. */
    SampleIdentity identity();

    /** The request a reply answers, or {@link SampleIdentity#UNKNOWN}. */
    SampleIdentity relatedIdentity();

    /**
     * The identity, as the machine-readable {@code Sample.java} of the
     * specification names the operation: {@code getIdentity()} returns the
     * {@code org.omg.dds.core.SampleIdentity}.
     */
    default org.omg.dds.core.SampleIdentity getIdentity() {
        return identity();
    }

    /** The related identity, as the machine-readable {@code Sample.java} names it. */
    default org.omg.dds.core.SampleIdentity getRelatedIdentity() {
        return relatedIdentity();
    }

    /**
     * Whether the sample carries data rather than only a state change —
     * the {@code valid_data} of [DDS-Java-PSM] §7.7.2, which is what
     * decides whether {@link #getData()} is a value or {@code null}.
     *
     * @return whether the sample carries data.
     */
    boolean validData();

    // DDS-RPC 1.0 §7.11.2.2.7: "The Sample.Iterator interface is the same as in
    // [DDS-Java-PSM]". This interface declares none: Sample.Iterator is the member type it
    // inherits, org.omg.dds.sub.Sample.Iterator, as in the machine-readable Sample.java.

    /** One sample over a value and the two identities. */
    static <T> Sample<T> of(T data, SampleIdentity identity, SampleIdentity relatedIdentity) {
        ServiceEnvironment env = Values.getDefaultEnvironment();
        return new Sample<T>() {
            @Override
            public ServiceEnvironment getEnvironment() {
                return env;
            }

            @Override
            public T getData() {
                return data;
            }

            @Override
            public SampleIdentity identity() {
                return identity == null ? SampleIdentity.UNKNOWN : identity;
            }

            @Override
            public SampleIdentity relatedIdentity() {
                return relatedIdentity == null ? SampleIdentity.UNKNOWN : relatedIdentity;
            }

            @Override
            public boolean validData() {
                return true;
            }

            /**
             * The identity of §7.2.1.5 is a GUID and a sequence number,
             * not an instance handle; a sample the RPC layer built from a
             * request or a reply belongs to no keyed instance.
             */
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
                return org.zerodds.core.TimeImpl.invalid(env);
            }

            @Override
            public SampleState getSampleState() {
                return SampleState.NOT_READ;
            }

            @Override
            public ViewState getViewState() {
                return ViewState.NEW;
            }

            @Override
            public InstanceState getInstanceState() {
                return InstanceState.ALIVE;
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
                return of(data, identity, relatedIdentity);
            }

            @Override
            public String toString() {
                return "rpc.Sample[" + data + "]";
            }
        };
    }

    /**
     * An iterator over a list of samples, as §7.11.2.2.7 asks for: the
     * {@code Sample.Iterator} of [DDS-Java-PSM], whose elements are these
     * {@code rpc.Sample}s ({@link org.zerodds.ddsrpc.RpcSampleIterator}).
     */
    static <T> org.omg.dds.sub.Sample.Iterator<T> iterate(java.util.List<Sample<T>> samples) {
        return new org.zerodds.ddsrpc.RpcSampleIterator<>(samples);
    }
}
