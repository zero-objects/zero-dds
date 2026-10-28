// SPDX-License-Identifier: Apache-2.0
package org.zerodds.rtps;

import java.util.Arrays;
import org.zerodds.cdr.EndianMode;
import org.zerodds.cdr.ExtensibilityKind;
import org.zerodds.cdr.TopicTypeSupport;
import org.zerodds.cdr.XcdrException;

/** A validated RTPS serialized payload. Representation and byte order survive transport. */
public final class SerializedSample {
    private final byte[] body;
    public final int representation;
    public final EndianMode endian;
    private final int scheme;
    private org.zerodds.internal.SourceIdentity identity;
    private org.zerodds.internal.SourceIdentity related;
    private long sourceTimestampMillis = -1;

    private SerializedSample(byte[] body, int scheme) {
        this.body = body;
        this.scheme = scheme;
        representation = scheme >= 6 ? TopicTypeSupport.XCDR2 : TopicTypeSupport.XCDR1;
        endian = (scheme & 1) == 1 ? EndianMode.LITTLE_ENDIAN : EndianMode.BIG_ENDIAN;
    }

    public byte[] body() { return body.clone(); }

    /** The writer GUID and sequence number of the DATA this payload came in, or {@code null}. */
    public org.zerodds.internal.SourceIdentity identity() { return identity; }

    /** The identity of the sample this one relates to (DDS-RPC 1.0 §7.8.2), or {@code null}. */
    public org.zerodds.internal.SourceIdentity relatedIdentity() { return related; }

    /** This payload with the identity of the DATA it came in. */
    public SerializedSample withSource(org.zerodds.internal.SourceIdentity identity,
            org.zerodds.internal.SourceIdentity related) {
        SerializedSample copy = new SerializedSample(body, scheme);
        copy.identity = identity;
        copy.related = related;
        copy.sourceTimestampMillis = sourceTimestampMillis;
        return copy;
    }

    /**
     * The source timestamp the writer sent in INFO_TS (RTPS 2.5 §8.3.7.9) in milliseconds since
     * the epoch, or -1 when the DATA came without one.
     */
    public long sourceTimestampMillis() { return sourceTimestampMillis; }

    /** This payload with the source timestamp of the INFO_TS that preceded the DATA. */
    public SerializedSample withSourceTimestamp(long millis) {
        SerializedSample copy = new SerializedSample(body, scheme);
        copy.identity = identity;
        copy.related = related;
        copy.sourceTimestampMillis = millis;
        return copy;
    }

    public static SerializedSample parse(byte[] payload) {
        if (payload == null || payload.length < 4 || payload[0] != 0) {
            throw new XcdrException("Missing or invalid RTPS encapsulation");
        }
        int scheme = payload[1] & 255;
        if (!(scheme <= 3 || (scheme >= 6 && scheme <= 11))) {
            throw new XcdrException("Unsupported RTPS encapsulation " + scheme);
        }
        // Only the defined two-bit trailing padding count is accepted.
        if (payload[2] != 0 || (payload[3] & 252) != 0) {
            throw new XcdrException("Unsupported RTPS encapsulation options");
        }
        int padding = payload[3] & 3;
        if (payload.length - 4 < padding) {
            throw new XcdrException("Truncated RTPS encapsulation padding");
        }
        return new SerializedSample(Arrays.copyOfRange(payload, 4, payload.length - padding), scheme);
    }

    public void requireFraming(ExtensibilityKind kind) {
        if ((scheme & ~1) != schemeFor(representation, kind)) {
            throw new XcdrException("Encapsulation does not match " + kind + " descriptor");
        }
    }

    public static byte[] encapsulation(int representation, ExtensibilityKind kind, EndianMode endian) {
        if (endian == null) throw new XcdrException("Missing byte order");
        return new byte[] {0, (byte) (schemeFor(representation, kind)
                + (endian == EndianMode.LITTLE_ENDIAN ? 1 : 0)), 0, 0};
    }

    private static int schemeFor(int representation, ExtensibilityKind kind) {
        if (kind == null) throw new XcdrException("Missing extensibility");
        if (representation == TopicTypeSupport.XCDR1) {
            return kind == ExtensibilityKind.MUTABLE ? 2 : 0;
        }
        if (representation == TopicTypeSupport.XCDR2) {
            switch (kind) {
                // XTypes 1.3 §7.2.2.4.4.4: EXTENSIBLE is the XTypes 1.1 name of APPENDABLE.
                case FINAL: return 6;
                case EXTENSIBLE:
                case APPENDABLE: return 8;
                case MUTABLE: return 10;
                default: break;
            }
        }
        throw new XcdrException("Unsupported data representation " + representation);
    }
}
