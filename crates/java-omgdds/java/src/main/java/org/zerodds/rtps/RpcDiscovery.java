// SPDX-License-Identifier: Apache-2.0
package org.zerodds.rtps;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/**
 * The discovery extensions of DDS-RPC 1.0 §7.6.2.1: the {@code service_instance_name}
 * (PID 0x0080), the {@code related_entity_guid} (PID 0x0081, the
 * {@code related_datawriter_key} and {@code related_datareader_key} of the spec) and the
 * {@code topic_aliases} (PID 0x0082) of one endpoint.
 *
 * <p>Immutable. {@link #NONE} is an endpoint that announces none of them.
 */
public final class RpcDiscovery {

    /** The bound of {@code string<256>} (§7.6.2.1.2), in octets. */
    public static final int STRING_BOUND = 256;

    /** An endpoint without the extensions. */
    public static final RpcDiscovery NONE =
            new RpcDiscovery("", null, Collections.<String>emptyList());

    private final String serviceInstanceName;
    private final byte[] relatedEntityGuid;
    private final List<String> topicAliases;

    /**
     * @param serviceInstanceName the name of the service instance, {@code ""} for none
     * @param relatedEntityGuid the 16-octet GUID of the endpoint on the other topic of the
     *     pair, or {@code null} for none
     * @param topicAliases the alternative names of the topic, in order of preference
     * @throws IllegalArgumentException for a name or alias longer than 256 octets, or a GUID
     *     that is not 16 octets
     */
    public RpcDiscovery(String serviceInstanceName, byte[] relatedEntityGuid,
            List<String> topicAliases) {
        this.serviceInstanceName = serviceInstanceName == null ? "" : serviceInstanceName;
        if (utf8(this.serviceInstanceName) > STRING_BOUND) {
            throw new IllegalArgumentException("service_instance_name is longer than string<256>");
        }
        if (relatedEntityGuid != null && relatedEntityGuid.length != 16) {
            throw new IllegalArgumentException("related_entity_guid is 16 octets");
        }
        this.relatedEntityGuid = relatedEntityGuid == null ? null : relatedEntityGuid.clone();
        List<String> aliases = topicAliases == null ? new ArrayList<String>()
                : new ArrayList<String>(topicAliases);
        for (String alias : aliases) {
            if (utf8(alias) > STRING_BOUND) {
                throw new IllegalArgumentException("a topic alias is longer than string<256>");
            }
        }
        this.topicAliases = Collections.unmodifiableList(aliases);
    }

    private static int utf8(String s) {
        return s.getBytes(StandardCharsets.UTF_8).length;
    }

    /** The service instance name, {@code ""} when the endpoint announces none. */
    public String serviceInstanceName() {
        return serviceInstanceName;
    }

    /** The GUID of the related endpoint, or {@code null}. */
    public byte[] relatedEntityGuid() {
        return relatedEntityGuid == null ? null : relatedEntityGuid.clone();
    }

    /** The topic aliases, in order. */
    public List<String> topicAliases() {
        return topicAliases;
    }

    /** Whether no extension is set. */
    public boolean isEmpty() {
        return serviceInstanceName.isEmpty() && relatedEntityGuid == null && topicAliases.isEmpty();
    }

    /**
     * Whether two topics are the same topic for matching: the names are equal, or the name of
     * one is among the aliases of the other (§7.4.3: a topic alias is an alternative name for
     * the topic).
     */
    public static boolean topicsCorrespond(String topic, RpcDiscovery own, String otherTopic,
            RpcDiscovery other) {
        return topic.equals(otherTopic)
                || own.topicAliases.contains(otherTopic)
                || other.topicAliases.contains(topic);
    }

    /** Adds the parameters this value carries to {@code pl}. */
    void writeTo(ParameterList pl) {
        if (!serviceInstanceName.isEmpty()) {
            pl.add(Rtps.PID_SERVICE_INSTANCE_NAME, ParameterList.cdrString(serviceInstanceName));
        }
        if (relatedEntityGuid != null) {
            pl.add(Rtps.PID_RELATED_ENTITY_GUID, relatedEntityGuid.clone());
        }
        if (!topicAliases.isEmpty()) {
            pl.add(Rtps.PID_TOPIC_ALIASES, ParameterList.stringSeq(topicAliases));
        }
    }

    /**
     * Reads the parameters from {@code pl}.
     *
     * @return the value, or {@code null} when a string is longer than {@code string<256>}
     *     (§7.6.2.1.2): the announcement is refused, as the Rust decoder does
     */
    static RpcDiscovery readFrom(ParameterList pl) {
        String name = "";
        byte[] value = pl.find(Rtps.PID_SERVICE_INSTANCE_NAME);
        if (value != null) {
            name = ParameterList.readCdrString(value);
            if (utf8(name) > STRING_BOUND) {
                return null;
            }
        }
        byte[] guid = pl.find(Rtps.PID_RELATED_ENTITY_GUID);
        byte[] related = guid != null && guid.length >= 16 ? Wire.slice(guid, 0, 16) : null;
        List<String> aliases = new ArrayList<String>();
        byte[] seq = pl.find(Rtps.PID_TOPIC_ALIASES);
        if (seq != null && seq.length >= 4) {
            long count = Wire.u32le(seq, 0);
            int at = 4;
            for (long i = 0; i < count; i++) {
                if (at + 4 > seq.length) {
                    break;
                }
                long length = Wire.u32le(seq, at);
                at += 4;
                if (length < 1 || at + length > seq.length) {
                    break;
                }
                String alias = new String(seq, at, (int) length - 1, StandardCharsets.UTF_8);
                if (utf8(alias) > STRING_BOUND) {
                    return null;
                }
                aliases.add(alias);
                at += (int) length;
                at = (at + 3) & ~3;
            }
        }
        return new RpcDiscovery(name, related, aliases);
    }

    @Override
    public boolean equals(Object other) {
        if (!(other instanceof RpcDiscovery)) {
            return false;
        }
        RpcDiscovery o = (RpcDiscovery) other;
        return serviceInstanceName.equals(o.serviceInstanceName)
                && java.util.Arrays.equals(relatedEntityGuid, o.relatedEntityGuid)
                && topicAliases.equals(o.topicAliases);
    }

    @Override
    public int hashCode() {
        return serviceInstanceName.hashCode() * 31 + topicAliases.hashCode()
                + java.util.Arrays.hashCode(relatedEntityGuid);
    }

    @Override
    public String toString() {
        return "RpcDiscovery{instance=\"" + serviceInstanceName + "\", aliases=" + topicAliases
                + (relatedEntityGuid == null ? "" : ", related=" + hex(relatedEntityGuid)) + "}";
    }

    private static String hex(byte[] bytes) {
        StringBuilder out = new StringBuilder();
        for (byte b : bytes) {
            out.append(String.format("%02x", b & 255));
        }
        return out.toString();
    }
}
