// SPDX-License-Identifier: Apache-2.0
package org.zerodds.rtps;

import java.util.function.Consumer;

/**
 * Bridge between the OMG DDS {@code DataWriter}/{@code DataReader} and the
 * pure-Java RTPS wire stack ({@link RtpsParticipant}). Lets DDS entities carry
 * samples cross-process over real UDP RTPS <em>alongside</em> the in-process
 * {@code InProcessBus} path.
 *
 * <p>Off by default so the in-process unit-test suite keeps running without
 * opening sockets; enable with {@code -Dzerodds.rtps.enable=true} or the
 * environment variable {@code ZERODDS_RTPS=1} (mirrors the Rust env-selected
 * transport). When enabled, one {@link RtpsParticipant} per domain is shared by
 * all writers/readers in the JVM.
 */
public final class DdsWireBridge {
    private DdsWireBridge() {}

    public static boolean enabled() {
        if (Boolean.parseBoolean(System.getProperty("zerodds.rtps.enable", "false"))) {
            return true;
        }
        String env = System.getenv("ZERODDS_RTPS");
        return env != null && (env.equals("1") || env.equalsIgnoreCase("true"));
    }

    /**
     * Choose the XCDR encapsulation header for a type. Structured
     * extended supports select their actual representation and extensibility.
     * The legacy OMG-only bridge remains plain XCDR1 little-endian.
     */
    public static byte[] encapFor(Object typeSupport) {
        if (typeSupport instanceof org.zerodds.cdr.TopicTypeSupport) {
            org.zerodds.cdr.TopicTypeSupport<?> support = (org.zerodds.cdr.TopicTypeSupport<?>) typeSupport;
            return SerializedSample.encapsulation(support.writeRepresentation(), support.getExtensibility(),
                    org.zerodds.cdr.EndianMode.LITTLE_ENDIAN);
        }
        return Rtps.ENCAP_CDR_LE.clone();
    }

    public static int[] readRepresentations(Object support) {
        int[] reps = support instanceof org.zerodds.cdr.TopicTypeSupport
                ? ((org.zerodds.cdr.TopicTypeSupport<?>) support).readRepresentations()
                : new int[] {Rtps.DATA_REP_XCDR1};
        return validateRepresentations(reps);
    }

    static int[] validateRepresentations(int[] reps) {
        if (reps == null || reps.length == 0) throw new IllegalArgumentException("No read representation");
        for (int rep : reps) {
            if (rep != Rtps.DATA_REP_XCDR1 && rep != Rtps.DATA_REP_XCDR2) {
                throw new IllegalArgumentException("Unsupported read representation " + rep);
            }
        }
        return java.util.Arrays.stream(reps).distinct().toArray();
    }

    public static <T> T decode(org.zerodds.cdr.TopicTypeSupport<T> support, SerializedSample sample) {
        if (!java.util.Arrays.stream(readRepresentations(support)).anyMatch(r -> r == sample.representation)) {
            throw new org.zerodds.cdr.XcdrException("Unadvertised received representation");
        }
        byte[] body = sample.body();
        if (support instanceof org.zerodds.cdr.TopicTypeSupport) {
            org.zerodds.cdr.TopicTypeSupport<T> typed = support;
            sample.requireFraming(typed.getExtensibility());
            return typed.decode(body, 0, body.length, sample.endian, sample.representation);
        }
        sample.requireFraming(org.zerodds.cdr.ExtensibilityKind.FINAL);
        if (sample.endian != org.zerodds.cdr.EndianMode.LITTLE_ENDIAN) {
            throw new org.zerodds.cdr.XcdrException("Legacy OMG descriptor only supports little-endian");
        }
        return support.deserialize(java.nio.ByteBuffer.wrap(body).order(java.nio.ByteOrder.LITTLE_ENDIAN));
    }

    public static RtpsParticipant.WireReader typedReader(int domainId, String topic, String typeName,
            org.zerodds.cdr.TopicTypeSupport<?> support, Consumer<SerializedSample> onSample) {
        return typedReader(domainId, topic, typeName, support, onSample, null);
    }

    /** A reader that announces {@code qos} on SEDP (DDS 1.4 §2.2.5). */
    public static RtpsParticipant.WireReader typedReader(int domainId, String topic, String typeName,
            org.zerodds.cdr.TopicTypeSupport<?> support, Consumer<SerializedSample> onSample,
            QosParameters qos) {
        return typedReader(domainId, topic, typeName, support, onSample, qos, null);
    }

    /** A reader that also matches the remote writers {@code typeMatcher} accepts (type coercion). */
    public static RtpsParticipant.WireReader typedReader(int domainId, String topic, String typeName,
            org.zerodds.cdr.TopicTypeSupport<?> support, Consumer<SerializedSample> onSample,
            QosParameters qos, java.util.function.Predicate<DiscoveryData.Endpoint> typeMatcher) {
        int[] reps = readRepresentations(support);
        byte[] metadata = typeInformation(support);
        if (support instanceof org.zerodds.cdr.TopicTypeSupport<?>)
            RtpsParticipant.get(domainId).registerTypeObjects(((org.zerodds.cdr.TopicTypeSupport<?>) support).typeObjects());
        return RtpsParticipant.get(domainId).createTypedReader(topic, typeName, support.isKeyed(), reps,
                metadata, onSample, qos, typeMatcher, byTypeInformation(support));
    }

    /** Whether the endpoints of {@code support} pair by the assignability of their TypeInformation. */
    static boolean byTypeInformation(Object support) {
        return support instanceof org.zerodds.cdr.TopicTypeSupport
                && ((org.zerodds.cdr.TopicTypeSupport<?>) support).matchesByTypeInformation();
    }

    public static byte[] typeInformation(Object support) {
        byte[] bytes = support instanceof org.zerodds.cdr.TopicTypeSupport
                ? ((org.zerodds.cdr.TopicTypeSupport<?>) support).typeInformation() : new byte[0];
        if (bytes == null || bytes.length > 65532) throw new IllegalArgumentException("Invalid TypeInformation parameter length");
        return bytes.clone();
    }

    public static RtpsParticipant.WireWriter typedWriter(int domainId, String topic,
            org.zerodds.cdr.TopicTypeSupport<?> support) {
        return typedWriter(domainId, topic, support, null);
    }

    /** A writer that announces {@code qos} on SEDP (DDS 1.4 §2.2.5). */
    public static RtpsParticipant.WireWriter typedWriter(int domainId, String topic,
            org.zerodds.cdr.TopicTypeSupport<?> support, QosParameters qos) {
        return typedWriter(domainId, topic, support, qos, null);
    }

    /** A writer that also matches the remote readers {@code typeMatcher} accepts (type coercion). */
    public static RtpsParticipant.WireWriter typedWriter(int domainId, String topic,
            org.zerodds.cdr.TopicTypeSupport<?> support, QosParameters qos,
            java.util.function.Predicate<DiscoveryData.Endpoint> typeMatcher) {
        byte[] metadata = typeInformation(support);
        byte[] vendorId = support instanceof org.zerodds.cdr.TopicTypeSupport
                ? ((org.zerodds.cdr.TopicTypeSupport<?>) support).typeIdentifier() : new byte[0];
        if (support instanceof org.zerodds.cdr.TopicTypeSupport<?>)
            RtpsParticipant.get(domainId).registerTypeObjects(((org.zerodds.cdr.TopicTypeSupport<?>) support).typeObjects());
        return RtpsParticipant.get(domainId).createWriter(topic, support.getTypeName(),
                encapFor(support), vendorId, metadata, support.isKeyed(), true, qos, typeMatcher,
                byTypeInformation(support));
    }

    public static RtpsParticipant.WireWriter writer(int domainId, String topic, String typeName,
                                                    byte[] encap, byte[] typeId, boolean keyed) {
        return RtpsParticipant.get(domainId)
                .createWriter(topic, typeName, encap, typeId, keyed, true);
    }

    public static RtpsParticipant.WireReader reader(int domainId, String topic, String typeName,
                                                    boolean keyed, Consumer<byte[]> onBody) {
        return RtpsParticipant.get(domainId)
                .createReader(topic, typeName, keyed, onBody);
    }
}
