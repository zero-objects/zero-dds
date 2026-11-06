// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.Test;
import org.omg.dds.core.Duration;
import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.core.Time;
import org.omg.dds.core.policy.Partition;
import org.omg.dds.core.policy.Reliability;
import org.omg.dds.core.status.OfferedIncompatibleQosStatus;
import org.omg.dds.type.TypeKind;
import org.omg.dds.type.dynamic.DynamicType;
import org.omg.dds.type.dynamic.DynamicTypeFactory;
import org.omg.dds.type.dynamic.TypeDescriptor;
import org.zerodds.ServiceEnvironmentImpl;

import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.ObjectInputStream;
import java.io.ObjectOutputStream;
import java.io.Serializable;
import java.util.concurrent.TimeUnit;

import static org.junit.jupiter.api.Assertions.*;

/**
 * DDS-Java-PSM 1.0 §7.3.3: the value types are {@link Serializable}. The
 * {@link ServiceEnvironment} of an object is a runtime object and does not
 * travel; a deserialized object belongs to the default environment.
 */
class SerializationTest {

    private static final ServiceEnvironment ENV = ServiceEnvironmentImpl.getDefaultEnvironment();

    @SuppressWarnings("unchecked")
    private static <T extends Serializable> T roundTrip(T value) throws Exception {
        ByteArrayOutputStream bytes = new ByteArrayOutputStream();
        try (ObjectOutputStream out = new ObjectOutputStream(bytes)) {
            out.writeObject(value);
        }
        try (ObjectInputStream in = new ObjectInputStream(
                new ByteArrayInputStream(bytes.toByteArray()))) {
            return (T) in.readObject();
        }
    }

    @Test
    void javapsm_7_3_3_policies_are_serializable_and_keep_their_value() throws Exception {
        Reliability reliability = ENV.getSPI().getPolicyFactory().Reliability().withReliable();
        Reliability copy = roundTrip(reliability);
        assertEquals(reliability, copy);
        assertSame(ENV, copy.getEnvironment());

        Partition partition = ENV.getSPI().getPolicyFactory().Partition().withName(java.util.Arrays.asList("a", "b"));
        Partition partitionCopy = roundTrip(partition);
        assertEquals(partition, partitionCopy);
        assertEquals(partition.getName(), partitionCopy.getName());
    }

    @Test
    void javapsm_7_3_3_a_status_is_serializable_and_keeps_its_counts() throws Exception {
        OfferedIncompatibleQosStatus status = new org.zerodds.core.status.OfferedIncompatibleQosStatusImpl(
                ENV, 3, 1, Reliability.class, new java.util.LinkedHashSet<>());
        OfferedIncompatibleQosStatus copy = roundTrip(status);
        assertEquals(3, copy.getTotalCount());
        assertEquals(1, copy.getTotalCountChange());
        assertSame(ENV, copy.getEnvironment());
    }

    @Test
    void javapsm_7_3_3_a_type_descriptor_is_serializable_and_carries_the_types_it_names() throws Exception {
        DynamicTypeFactory factory = DynamicTypeFactory.getInstance(ENV);
        DynamicType element = factory.getPrimitiveType(TypeKind.INT_32_TYPE);
        DynamicType sequence = factory.createSequenceType(element);
        TypeDescriptor copy = roundTrip(sequence.getDescriptor());
        assertEquals(TypeKind.SEQUENCE_TYPE, copy.getKind());
        assertEquals(TypeKind.INT_32_TYPE, copy.getElementType().getKind());
        assertSame(ENV, copy.getEnvironment());
    }
}
