// SPDX-License-Identifier: Apache-2.0
package org.zerodds.ddsrpc;

import java.io.ByteArrayOutputStream;
import java.lang.reflect.Field;
import java.lang.reflect.Modifier;
import java.lang.reflect.ParameterizedType;
import java.lang.reflect.Type;
import org.omg.dds.rpc.HeaderCodec;
import org.omg.dds.rpc.HeaderInReplyType;
import org.omg.dds.rpc.HeaderInRequestType;
import org.omg.dds.rpc.ReplyHeader;
import org.omg.dds.rpc.RequestHeader;
import org.omg.dds.rpc.ServiceMapping;
import org.omg.dds.rpc.TypeSupport;
import org.omg.dds.rpc.WireEncoding;

/**
 * One sample of a DDS-RPC topic as a value of its topic type and as the octets the topic
 * carries: the request of §7.5.1.1.6 or the reply of §7.5.1.1.7, with the header in front of
 * the call or the return in the Basic mapping and without one in the Enhanced mapping
 * (§7.5.1.2). The requester and the replier frame the samples they send and read themselves;
 * this is the same framing for the DataWriter and DataReader of the DDS Java PSM the
 * application obtains from them ({@code Requester.getRequestDataWriter()} and the other three,
 * DDS-RPC 1.0 §7.11.2), typed by the topic type.
 *
 * <p>The header of a sample is where the value keeps it: in the value for a type whose
 * support implements {@link HeaderInRequestType} or {@link HeaderInReplyType} (§7.3.1.4), in
 * the public {@code header} member the generated topic types of an interface declare
 * (§7.5.1.1.6), and nowhere in the Enhanced mapping. Encoding a value whose {@code header}
 * member is {@code null} writes the header before a requester or replier stamps it.
 *
 * @param <T> the topic type
 */
public final class TopicSample<T> {

    private final Class<T> type;
    private final TypeSupport<T> support;
    private final ServiceMapping mapping;
    private final WireEncoding encoding;
    private final boolean request;
    private final Field header;

    private TopicSample(Class<T> type, TypeSupport<T> support, ServiceMapping mapping,
            WireEncoding encoding, boolean request) {
        this.type = type;
        this.support = support;
        this.mapping = mapping;
        this.encoding = encoding;
        this.request = request;
        this.header = mapping == ServiceMapping.BASIC ? headerMember(type, request) : null;
    }

    /**
     * The request samples {@code support} reads and writes; {@code null} when the support does
     * not name the class of its type ({@link #classOf}).
     */
    public static <T> TopicSample<T> request(
            TypeSupport<T> support, ServiceMapping mapping, WireEncoding encoding) {
        return of(support, mapping, encoding, true);
    }

    /** The reply samples {@code support} reads and writes; see {@link #request}. */
    public static <T> TopicSample<T> reply(
            TypeSupport<T> support, ServiceMapping mapping, WireEncoding encoding) {
        return of(support, mapping, encoding, false);
    }

    @SuppressWarnings("unchecked")
    private static <T> TopicSample<T> of(
            TypeSupport<T> support, ServiceMapping mapping, WireEncoding encoding, boolean request) {
        Class<?> type = classOf(support);
        return type == null ? null
                : new TopicSample<>((Class<T>) type, support, mapping, encoding, request);
    }

    /**
     * The class {@code support} is the type support of: the type argument it gives
     * {@code TypeSupport<T>} where its class, or a superclass, implements the interface — what
     * every generated support does. {@code null} for a support that leaves it open, such as a
     * lambda or a generic class.
     */
    public static Class<?> classOf(TypeSupport<?> support) {
        if (support == null) {
            return null;
        }
        for (Class<?> at = support.getClass(); at != null && at != Object.class;
                at = at.getSuperclass()) {
            for (Type implemented : at.getGenericInterfaces()) {
                if (implemented instanceof ParameterizedType
                        && ((ParameterizedType) implemented).getRawType() == TypeSupport.class) {
                    Type argument = ((ParameterizedType) implemented).getActualTypeArguments()[0];
                    if (argument instanceof Class) {
                        return (Class<?>) argument;
                    }
                    if (argument instanceof ParameterizedType
                            && ((ParameterizedType) argument).getRawType() instanceof Class) {
                        return (Class<?>) ((ParameterizedType) argument).getRawType();
                    }
                    return null;
                }
            }
        }
        return null;
    }

    /** The public {@code header} member of a generated topic type, or {@code null}. */
    private static Field headerMember(Class<?> type, boolean request) {
        try {
            Field found = type.getField("header");
            Class<?> expected = request ? RequestHeader.class : ReplyHeader.class;
            return found.getType() == expected && !Modifier.isStatic(found.getModifiers())
                    ? found : null;
        } catch (NoSuchFieldException none) {
            return null;
        }
    }

    /** The topic type. */
    public Class<T> type() {
        return type;
    }

    /** The octets of {@code value} as the topic carries them. */
    public byte[] encode(T value) {
        if (mapping != ServiceMapping.BASIC || headerIn()) {
            return support.serialize(value, 0, encoding);
        }
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        byte[] head = request
                ? HeaderCodec.encodeRequestHeader(requestHeaderOf(value))
                : HeaderCodec.encodeReplyHeader(replyHeaderOf(value));
        out.write(head, 0, head.length);
        HeaderCodec.padTo4(out);
        byte[] payload = support.serialize(value, out.size(), encoding);
        out.write(payload, 0, payload.length);
        return out.toByteArray();
    }

    /** The value {@code octets} are, with the header in the value where it keeps one. */
    public T decode(byte[] octets) {
        if (mapping != ServiceMapping.BASIC || headerIn()) {
            return support.deserialize(octets, new HeaderCodec.Cursor(0), encoding);
        }
        HeaderCodec.Cursor cursor = new HeaderCodec.Cursor(0);
        Object head = request
                ? HeaderCodec.decodeRequestHeader(octets, cursor)
                : HeaderCodec.decodeReplyHeader(octets, cursor);
        HeaderCodec.alignTo4(cursor);
        T value = support.deserialize(octets, cursor, encoding);
        if (header != null && value != null) {
            try {
                header.set(value, head);
            } catch (IllegalAccessException impossible) {
                throw new IllegalStateException(impossible);
            }
        }
        return value;
    }

    private boolean headerIn() {
        return request ? support instanceof HeaderInRequestType : support instanceof HeaderInReplyType;
    }

    private RequestHeader requestHeaderOf(T value) {
        Object found = memberOf(value);
        return found instanceof RequestHeader ? (RequestHeader) found : new RequestHeader();
    }

    private ReplyHeader replyHeaderOf(T value) {
        Object found = memberOf(value);
        return found instanceof ReplyHeader ? (ReplyHeader) found : new ReplyHeader();
    }

    private Object memberOf(T value) {
        if (header == null || value == null) {
            return null;
        }
        try {
            return header.get(value);
        } catch (IllegalAccessException impossible) {
            throw new IllegalStateException(impossible);
        }
    }
}
