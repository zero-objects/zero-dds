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
    void timeFromMillisRoundTrip() {
        org.omg.dds.core.Time t = Values.ofMillis(1234);
        assertEquals(1, t.getTime(TimeUnit.SECONDS));
        assertEquals(234_000_000L, t.getRemainder(TimeUnit.SECONDS, TimeUnit.NANOSECONDS));
        assertEquals(1234, t.getTime(TimeUnit.MILLISECONDS));
    }

    @Test
    void durationInfiniteRecognized() {
        // DDS-Java-PSM 1.0 §7.3.4: a magnitude of Long.MAX_VALUE is infinite.
        assertTrue(Values.infiniteDuration().isInfinite());
        assertEquals(Long.MAX_VALUE, Values.infiniteDuration().getDuration(TimeUnit.SECONDS));
        assertFalse(Values.durationOfMillis(100).isInfinite());
        assertTrue(Values.durationOfMillis(0).isZero());
    }

    @Test
    void instanceHandleNilDetected() {
        assertTrue(Values.nil().isNil());
        byte[] nonZero = new byte[16];
        nonZero[5] = 1;
        assertFalse(Values.of(nonZero).isNil());
    }

    @Test
    void instanceHandleRejectsWrongLength() {
        assertThrows(IllegalArgumentException.class,
                () -> Values.of(new byte[8]));
    }

    @Test
    void reliabilityAndHistoryQosDefaults() {
        // DDS 1.2 §7.1.3: BEST_EFFORT is the reader/topic default; HISTORY
        // defaults to KEEP_LAST with depth 1.
        DataReaderQos reader = DataReaderQosImpl.defaults(ENV);
        TopicQos topic = TopicQosImpl.defaults(ENV);
        assertEquals(Reliability.Kind.BEST_EFFORT, reader.get(Reliability.class).getKind());
        assertEquals(Reliability.Kind.BEST_EFFORT, topic.getReliability().getKind());
        assertEquals(Durability.Kind.VOLATILE, topic.getDurability().getKind());
        assertEquals(History.Kind.KEEP_LAST, reader.getHistory().getKind());
        assertEquals(1, reader.getHistory().getDepth());
        // The DataWriter default is RELIABLE.
        DataWriterQos writer = DataWriterQosImpl.defaults(ENV);
        assertEquals(Reliability.Kind.RELIABLE, writer.getReliability().getKind());
    }

    @Test
    void qosCompatibilityReliableReaderRequiresReliableWriter() {
        // DDS 1.2 §7.1.3.6: the offered RELIABILITY must be at least the
        // requested one.
        PolicyFactoryImpl pf = (PolicyFactoryImpl) policyFactory();
        DataReaderQos reliableReader = DataReaderQosImpl.defaults(ENV).withPolicy(
                pf.DataReaderReliability().withReliable());
        DataWriterQos bestEffortWriter = DataWriterQosImpl.defaults(ENV).withPolicy(
                pf.DataWriterReliability().withBestEffort());
        DataWriterQos reliableWriter = DataWriterQosImpl.defaults(ENV).withPolicy(
                pf.DataWriterReliability().withReliable());
        assertFalse(QosCompatibility.isCompatible(
                QosCompatibility.policiesOf(reliableReader),
                QosCompatibility.policiesOf(bestEffortWriter)));
        assertTrue(QosCompatibility.isCompatible(
                QosCompatibility.policiesOf(reliableReader),
                QosCompatibility.policiesOf(reliableWriter)));
        // §7.1.4.1: the status names the refusing policy.
        assertEquals(Reliability.class, QosCompatibility.incompatiblePolicyWith(
                QosCompatibility.policiesOf(reliableReader),
                QosCompatibility.policiesOf(bestEffortWriter)));
    }

    @Test
    void historyDepthMustBeNonNegative() {
        PolicyFactoryImpl pf = (PolicyFactoryImpl) policyFactory();
        assertThrows(IllegalArgumentException.class,
                () -> pf.History().withDepth(-1));
    }
}