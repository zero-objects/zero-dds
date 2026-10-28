// SPDX-License-Identifier: Apache-2.0
package org.zerodds.ddsrpc;

import org.omg.dds.rpc.RequesterParams;
import org.omg.dds.rpc.TypeSupport;

/**
 * The type support the DDS Java PSM makes from the class of a DDS-RPC topic type
 * ({@code TypeSupport.newTypeSupport(Class, …)}, DDS-Java-PSM 1.0 §7.5.1;
 * {@code org.zerodds.cdr.TypeSupportResolver} asks here): for the {@code <I>_Request} and
 * {@code <I>_Reply} classes the generator writes for an interface (DDS-RPC 1.0 §7.5.1.1.6,
 * §7.5.1.1.7), the {@link RpcTopicTypeSupport} over the codec, the mapping and the encoding
 * the generated {@code <I>Support} gives, so a topic the application creates from the class
 * carries the samples the requesters and repliers of the interface write.
 */
public final class RpcTopicTypes {

    private RpcTopicTypes() {}

    /**
     * The type support of {@code type} when it is the request or reply type of a generated
     * interface; {@code null} for any other class.
     */
    @SuppressWarnings("unchecked")
    public static <T> org.zerodds.cdr.TopicTypeSupport<T> resolve(Class<T> type) {
        String simple = type.getSimpleName();
        boolean request = simple.endsWith("_Request");
        if (!request && !simple.endsWith("_Reply")) {
            return null;
        }
        Class<?> home = type.getEnclosingClass();
        if (home == null) {
            return null;
        }
        String support = simple.substring(0, simple.length() - (request ? 8 : 6)) + "Support";
        for (Class<?> member : home.getDeclaredClasses()) {
            if (!member.getSimpleName().equals(support)) {
                continue;
            }
            RequesterParams params;
            try {
                params = (RequesterParams) member
                        .getMethod("requesterParams", org.omg.dds.rpc.DomainParticipant.class)
                        .invoke(null, (Object) null);
            } catch (ReflectiveOperationException | ClassCastException notGenerated) {
                return null;
            }
            TypeSupport<T> codec = (TypeSupport<T>)
                    (request ? params.requestTypeSupport() : params.replyTypeSupport());
            TopicSample<T> sample = request
                    ? TopicSample.request(codec, params.mapping(), params.wireEncoding())
                    : TopicSample.reply(codec, params.mapping(), params.wireEncoding());
            if (sample == null || sample.type() != type) {
                return null;
            }
            RawTypeSupport announced = request
                    ? new RawTypeSupport(params.requestTypeName(), params.requestTypeDescription())
                    : new RawTypeSupport(params.replyTypeName(), params.replyTypeDescription());
            return new RpcTopicTypeSupport<>(announced, sample);
        }
        return null;
    }
}
