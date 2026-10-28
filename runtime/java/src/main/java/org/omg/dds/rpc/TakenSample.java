// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

/** One sample as it came off a topic: the octets and what the middleware knows about it. */
public final class TakenSample {

    private final byte[] octets;
    private final SampleIdentity identity;
    private final SampleIdentity related;
    private final org.omg.dds.sub.InstanceState instanceState;

    public TakenSample(byte[] octets, SampleIdentity identity) {
        this(octets, identity, SampleIdentity.UNKNOWN);
    }

    /**
     * @param octets the sample body as the writer sent it
     * @param identity the writer GUID and sequence number of the DATA, or
     *     {@link SampleIdentity#UNKNOWN} when the binding does not report it
     * @param related the {@code PID_RELATED_SAMPLE_IDENTITY} of the DATA
     *     (DDS-RPC 1.0 §7.8.2.2), or {@link SampleIdentity#UNKNOWN} when it
     *     carried none or the binding does not report it
     */
    public TakenSample(byte[] octets, SampleIdentity identity, SampleIdentity related) {
        this(octets, identity, related, org.omg.dds.sub.InstanceState.ALIVE);
    }

    private TakenSample(byte[] octets, SampleIdentity identity, SampleIdentity related,
            org.omg.dds.sub.InstanceState instanceState) {
        this.octets = octets;
        this.identity = identity == null ? SampleIdentity.UNKNOWN : identity;
        this.related = related == null ? SampleIdentity.UNKNOWN : related;
        this.instanceState = instanceState;
    }

    /**
     * A sample that carries no data, only a change of the state of its instance — the
     * {@code valid_data} false of DDS 1.4 §2.2.2.5.1, such as the end of the instance a
     * writer that was deleted leaves (NOT_ALIVE_DISPOSED under the default
     * WRITER_DATA_LIFECYCLE, NOT_ALIVE_NO_WRITERS without autodispose). A requester or replier hands it over only when
     * {@code receiveNondataSamples(true)} asked for it (DDS-RPC 1.0 §7.11.1.4.3, §7.11.2).
     *
     * @param identity the identity the binding reports for it, or {@code null}
     * @param instanceState the state the instance is in
     */
    public static TakenSample withoutData(
            SampleIdentity identity, org.omg.dds.sub.InstanceState instanceState) {
        return new TakenSample(null, identity, SampleIdentity.UNKNOWN,
                java.util.Objects.requireNonNull(instanceState, "instanceState"));
    }

    /** Whether the sample carries data, octets; {@code false} for {@link #withoutData}. */
    public boolean validData() {
        return octets != null;
    }

    /** The state of the instance when the sample was taken. */
    public org.omg.dds.sub.InstanceState instanceState() {
        return instanceState;
    }

    public byte[] octets() {
        return octets;
    }

    /** The identity the writer gave the sample — §7.2.1.5. */
    public SampleIdentity identity() {
        return identity;
    }

    /** The sample this one answers — §7.8.2.2. */
    public SampleIdentity relatedIdentity() {
        return related;
    }
}
