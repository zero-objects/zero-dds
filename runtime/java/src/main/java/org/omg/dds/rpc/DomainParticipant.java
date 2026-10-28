// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

/**
 * The entity a {@link Requester}, a {@link Replier}, a {@link Client} and
 * a {@link Service} are created from. See {@link DataWriter} for why this
 * is here and not {@code org.omg.dds.domain.DomainParticipant}.
 */
public interface DomainParticipant {

    /**
     * Create a writer on {@code topic}.
     *
     * @param topic the DDS topic name
     * @param typeName the DDS type name the topic is announced under —
     *     DDS-RPC 1.0 §7.5.1.1.6 and §7.5.1.1.7 name them
     *     {@code ${interfaceName}_Request} and {@code ${interfaceName}_Reply}
     * @param qos the QoS of the writer (§7.10)
     * @param serviceInstanceName the {@code service_instance_name} the
     *     writer announces in discovery (§7.6.2.1.1), or {@code ""} for none.
     *     The service side passes its instance name; "the client-side
     *     DataWriter shall not include service_instance_name", so a
     *     requester passes {@code ""}
     * @return the enabled writer
     */
    DataWriter createWriter(
            String topic, String typeName, RpcQos qos, String serviceInstanceName);

    /**
     * Create a reader on {@code topic}. See {@link #createWriter}; §7.6.2.1.2
     * says the same of the {@code service_instance_name} of a reader.
     */
    DataReader createReader(
            String topic, String typeName, RpcQos qos, String serviceInstanceName);

    /**
     * {@link #createWriter} for a topic type with an XTypes description
     * (§7.5.1.2, §7.7.2.5). An implementation that does not announce type
     * information creates the writer without it, and its peers match it by
     * the type name; this default is that.
     *
     * @param description the description, or {@code null}
     */
    default DataWriter createWriter(
            String topic,
            String typeName,
            RpcQos qos,
            String serviceInstanceName,
            org.zerodds.ddsrpc.TypeDescription description) {
        return createWriter(topic, typeName, qos, serviceInstanceName);
    }

    /** {@link #createReader} for a topic type with an XTypes description; see above. */
    default DataReader createReader(
            String topic,
            String typeName,
            RpcQos qos,
            String serviceInstanceName,
            org.zerodds.ddsrpc.TypeDescription description) {
        return createReader(topic, typeName, qos, serviceInstanceName);
    }

    /**
     * As {@link #createWriter(String, String, RpcQos, String, org.zerodds.ddsrpc.TypeDescription)},
     * with what the params say of the entity in the terms of the DDS Java PSM (§7.11.2): the
     * topic type, the Publisher, the DataWriterQos. A binding that is not the PSM creates the
     * writer it creates without them and refuses an endpoint that names a Publisher or a QoS.
     *
     * @throws RemoteUnsupportedError when {@code psm} names a Publisher or a QoS this binding
     *     cannot use
     */
    default DataWriter createWriter(
            String topic,
            String typeName,
            RpcQos qos,
            String serviceInstanceName,
            org.zerodds.ddsrpc.TypeDescription description,
            org.zerodds.ddsrpc.PsmEndpoint<?> psm) {
        org.zerodds.ddsrpc.PsmEndpoint.refuseWithoutPsm(psm);
        return createWriter(topic, typeName, qos, serviceInstanceName, description);
    }

    /** As {@link #createWriter(String, String, RpcQos, String, org.zerodds.ddsrpc.TypeDescription, org.zerodds.ddsrpc.PsmEndpoint)}, for a reader. */
    default DataReader createReader(
            String topic,
            String typeName,
            RpcQos qos,
            String serviceInstanceName,
            org.zerodds.ddsrpc.TypeDescription description,
            org.zerodds.ddsrpc.PsmEndpoint<?> psm) {
        org.zerodds.ddsrpc.PsmEndpoint.refuseWithoutPsm(psm);
        return createReader(topic, typeName, qos, serviceInstanceName, description);
    }

    /**
     * Delete a writer this participant created — DDS-RPC 1.0 §7.9.3:
     * "Deactivation shall delete the underlying DDS entities (i.e.,
     * DataReader and DataWriter only)."
     */
    void deleteWriter(DataWriter writer);

    /** Delete a reader this participant created — §7.9.3. */
    void deleteReader(DataReader reader);
}
