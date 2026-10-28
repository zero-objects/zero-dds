// SPDX-License-Identifier: Apache-2.0
package org.zerodds.domain;

import org.omg.dds.topic.Topic;
import org.omg.dds.type.dynamic.DynamicData;
import org.omg.dds.type.dynamic.DynamicType;
import org.zerodds.cdr.TypeSupportImpl;

/**
 * The topics of a {@link DynamicType} (DDS-Java-PSM 1.0 §7.5.1, §7.8.1).
 *
 * <p>PSM 1.0 types the topic that {@code createTopic(String, DynamicType)}
 * returns as {@code Topic<DynamicType>}, while the samples of such a topic
 * are {@link DynamicData}: a reader or writer that is to move them is a
 * {@code DataReader<DynamicData>} or {@code DataWriter<DynamicData>} over a
 * {@code Topic<DynamicData>}. The two parameterizations are the same object
 * ({@code Topic} is an interface, its type parameter is erased), so the
 * signatures of the PSM stay as they are and this class is the one place that
 * converts between them.
 *
 * <pre>{@code
 * DynamicType type = ...;                              // built with DynamicTypeFactory
 * Topic<DynamicType> described = participant.createTopic("Reading", type);
 * Topic<DynamicData> topic = DynamicTopics.dataTopic(described);
 *
 * DataWriter<DynamicData> writer = publisher.createDataWriter(topic);
 * DataReader<DynamicData> reader = subscriber.createDataReader(topic);
 *
 * DynamicData sample = DynamicDataFactory.getInstance(env).newDynamicData(type);
 * sample.setInt32Value(1, 42);
 * writer.write(sample);
 * }</pre>
 *
 * <p>{@code DomainParticipantImpl.createDynamicTopic} does both steps.
 */
public final class DynamicTopics {

    private DynamicTopics() {}

    /**
     * The topic of a DynamicType as the topic of its {@link DynamicData}
     * samples.
     *
     * @param topic a topic made by {@code createTopic(String, DynamicType)} of
     *        this implementation
     * @throws IllegalArgumentException if {@code topic} is not such a topic
     */
    @SuppressWarnings("unchecked")
    public static Topic<DynamicData> dataTopic(Topic<DynamicType> topic) {
        if (topic == null || !(topic.getTypeSupport() instanceof TypeSupportImpl)
                || !DynamicData.class.equals((Class<?>) topic.getTypeSupport().getType())) {
            throw new IllegalArgumentException(
                    "not the topic of a DynamicType made by this implementation: " + topic);
        }
        // Topic is an interface; its type parameter is erased, so this is the
        // same object. This and asDynamicType are the only unchecked
        // conversions between the two parameterizations.
        return (Topic<DynamicData>) (Topic<?>) topic;
    }

    /** The PSM 1.0 view of a topic of {@link DynamicData} samples. */
    @SuppressWarnings("unchecked")
    static Topic<DynamicType> asDynamicType(Topic<DynamicData> topic) {
        return (Topic<DynamicType>) (Topic<?>) topic;
    }
}
