// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.Test;
import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.core.policy.Durability;
import org.omg.dds.core.policy.History;
import org.omg.dds.core.policy.PolicyFactory;
import org.omg.dds.core.policy.Reliability;
import org.omg.dds.sub.DataReaderQos;
import org.omg.dds.pub.DataWriterQos;
import org.omg.dds.topic.TopicQos;
import org.zerodds.core.Values;
import org.zerodds.core.policy.PolicyFactoryImpl;
import org.zerodds.core.qos.DataReaderQosImpl;
import org.zerodds.core.qos.DataWriterQosImpl;
import org.zerodds.core.qos.QosCompatibility;
import org.zerodds.core.qos.TopicQosImpl;
import org.zerodds.ServiceEnvironmentImpl;

import java.util.concurrent.TimeUnit;

import static org.junit.jupiter.api.Assertions.*;

class CoreTypesTest {

    private static final ServiceEnvironment ENV = ServiceEnvironmentImpl.getDefaultEnvironment();

    private static PolicyFactory policyFactory() {
        return ENV.getSPI().getPolicyFactory();
    }

    @Test
    void javapsm_7_3_4_a_time_answers_in_seconds_with_its_remainder_and_in_milliseconds() {
        org.omg.dds.core.Time t = Values.ofMillis(1234);
        assertEquals(1, t.getTime(TimeUnit.SECONDS));
        assertEquals(234_000_000L, t.getRemainder(TimeUnit.SECONDS, TimeUnit.NANOSECONDS));
        assertEquals(1234, t.getTime(TimeUnit.MILLISECONDS));
    }

    @Test
    void javapsm_7_3_5_3_history_depth_must_be_non_negative() {
        PolicyFactoryImpl pf = (PolicyFactoryImpl) policyFactory();
        assertThrows(IllegalArgumentException.class,
                () -> pf.History().withDepth(-1));
    }
}