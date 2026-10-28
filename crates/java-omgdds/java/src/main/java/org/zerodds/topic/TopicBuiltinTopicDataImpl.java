// SPDX-License-Identifier: Apache-2.0
package org.zerodds.topic;

import java.util.ArrayList;
import java.util.List;

import org.omg.dds.core.ServiceEnvironment;
import org.zerodds.ServiceEnvironmentImpl;
import org.omg.dds.core.policy.DataRepresentation;
import org.omg.dds.core.policy.Deadline;
import org.omg.dds.core.policy.DestinationOrder;
import org.omg.dds.core.policy.Durability;
import org.omg.dds.core.policy.DurabilityService;
import org.omg.dds.core.policy.History;
import org.omg.dds.core.policy.LatencyBudget;
import org.omg.dds.core.policy.Lifespan;
import org.omg.dds.core.policy.Liveliness;
import org.omg.dds.core.policy.Ownership;
import org.omg.dds.core.policy.Reliability;
import org.omg.dds.core.policy.ResourceLimits;
import org.omg.dds.core.policy.TopicData;
import org.omg.dds.core.policy.TransportPriority;
import org.omg.dds.core.policy.TypeConsistencyEnforcement;
import org.omg.dds.topic.BuiltinTopicKey;
import org.omg.dds.topic.TopicBuiltinTopicData;
import org.omg.dds.type.typeobject.TypeObject;

/**
 * ZeroDDS {@link TopicBuiltinTopicData} (DDS 1.2 §2.2.1.3.1, DDSI-RTPS 2.5
 * §9.3.1.2): what the "DCPSTopic" built-in topic carries — the key, the name
 * and type of the topic, and the QoS of the topic as it is seen in the
 * domain.
 */
public final class TopicBuiltinTopicDataImpl implements TopicBuiltinTopicData {

    private static final long serialVersionUID = 1L;

    private transient ServiceEnvironment environment;

    private BuiltinTopicKey key;
    private String name;
    private String typeName;
    private ArrayList<String> equivalentTypeName;
    private ArrayList<String> baseTypeName;
    private TypeObject type;
    /** PID_TYPE_INFORMATION (XTypes 1.3 §7.6.3.2.2): the XCDR2 TypeInformation, or {@code null}. */
    private byte[] typeInformation;
    /** PID_TYPE (XTypes 1.3 §7.6.3.2.2): the XTypes 1.1 TypeObject as announced, or {@code null}. */
    private byte[] typeObjectBytes;
    private Durability durability;
    private DurabilityService durabilityService;
    private Deadline deadline;
    private LatencyBudget latencyBudget;
    private Liveliness liveliness;
    private Reliability reliability;
    private TransportPriority transportPriority;
    private Lifespan lifespan;
    private DestinationOrder destinationOrder;
    private History history;
    private ResourceLimits resourceLimits;
    private Ownership ownership;
    private TopicData topicData;
    private DataRepresentation representation;
    private TypeConsistencyEnforcement typeConsistency;

    public TopicBuiltinTopicDataImpl(ServiceEnvironment environment) {
        this.environment = environment;
    }

    @Override
    public BuiltinTopicKey getKey() {
        return key;
    }

    public void setKey(BuiltinTopicKey key) {
        this.key = key;
    }

    @Override
    public String getName() {
        return name;
    }

    public void setName(String name) {
        this.name = name;
    }

    @Override
    public String getTypeName() {
        return typeName;
    }

    public void setTypeName(String typeName) {
        this.typeName = typeName;
    }

    @Override
    public List<String> getEquivalentTypeName() {
        return equivalentTypeName;
    }

    public void setEquivalentTypeName(List<String> equivalentTypeName) {
        this.equivalentTypeName = equivalentTypeName == null ? null : new ArrayList<>(equivalentTypeName);
    }

    @Override
    public List<String> getBaseTypeName() {
        return baseTypeName;
    }

    public void setBaseTypeName(List<String> baseTypeName) {
        this.baseTypeName = baseTypeName == null ? null : new ArrayList<>(baseTypeName);
    }

    @Override
    public TypeObject getType() {
        if (type == null && typeObjectBytes != null) {
            type = org.zerodds.typeobject.TypeObjects.decodeOrNull(getEnvironment(), typeObjectBytes);
        }
        return type;
    }

    public void setType(TypeObject type) {
        this.type = type;
    }

    /** The TypeInformation the entity announced (PID_TYPE_INFORMATION), or {@code null}. */
    public byte[] getTypeInformation() {
        return typeInformation == null ? null : typeInformation.clone();
    }

    public void setTypeInformation(byte[] typeInformation) {
        this.typeInformation = typeInformation == null || typeInformation.length == 0
                ? null : typeInformation.clone();
    }

    /**
     * The serialized TypeObject of XTypes 1.1 the entity announced (PID_TYPE),
     * kept as the octets it was received in, or {@code null}.
     */
    public byte[] getTypeObjectBytes() {
        return typeObjectBytes == null ? null : typeObjectBytes.clone();
    }

    public void setTypeObjectBytes(byte[] typeObjectBytes) {
        this.type = null;
        this.typeObjectBytes = typeObjectBytes == null || typeObjectBytes.length == 0
                ? null : typeObjectBytes.clone();
    }

    @Override
    public Durability getDurability() {
        return durability;
    }

    public void setDurability(Durability durability) {
        this.durability = durability;
    }

    @Override
    public DurabilityService getDurabilityService() {
        return durabilityService;
    }

    public void setDurabilityService(DurabilityService durabilityService) {
        this.durabilityService = durabilityService;
    }

    @Override
    public Deadline getDeadline() {
        return deadline;
    }

    public void setDeadline(Deadline deadline) {
        this.deadline = deadline;
    }

    @Override
    public LatencyBudget getLatencyBudget() {
        return latencyBudget;
    }

    public void setLatencyBudget(LatencyBudget latencyBudget) {
        this.latencyBudget = latencyBudget;
    }

    @Override
    public Liveliness getLiveliness() {
        return liveliness;
    }

    public void setLiveliness(Liveliness liveliness) {
        this.liveliness = liveliness;
    }

    @Override
    public Reliability getReliability() {
        return reliability;
    }

    public void setReliability(Reliability reliability) {
        this.reliability = reliability;
    }

    @Override
    public TransportPriority getTransportPriority() {
        return transportPriority;
    }

    public void setTransportPriority(TransportPriority transportPriority) {
        this.transportPriority = transportPriority;
    }

    @Override
    public Lifespan getLifespan() {
        return lifespan;
    }

    public void setLifespan(Lifespan lifespan) {
        this.lifespan = lifespan;
    }

    @Override
    public DestinationOrder getDestinationOrder() {
        return destinationOrder;
    }

    public void setDestinationOrder(DestinationOrder destinationOrder) {
        this.destinationOrder = destinationOrder;
    }

    @Override
    public History getHistory() {
        return history;
    }

    public void setHistory(History history) {
        this.history = history;
    }

    @Override
    public ResourceLimits getResourceLimits() {
        return resourceLimits;
    }

    public void setResourceLimits(ResourceLimits resourceLimits) {
        this.resourceLimits = resourceLimits;
    }

    @Override
    public Ownership getOwnership() {
        return ownership;
    }

    public void setOwnership(Ownership ownership) {
        this.ownership = ownership;
    }

    @Override
    public TopicData getTopicData() {
        return topicData;
    }

    public void setTopicData(TopicData topicData) {
        this.topicData = topicData;
    }

    @Override
    public DataRepresentation getRepresentation() {
        return representation;
    }

    public void setRepresentation(DataRepresentation representation) {
        this.representation = representation;
    }

    @Override
    public TypeConsistencyEnforcement getTypeConsistency() {
        return typeConsistency;
    }

    public void setTypeConsistency(TypeConsistencyEnforcement typeConsistency) {
        this.typeConsistency = typeConsistency;
    }

    @Override
    public void copyFrom(TopicBuiltinTopicData src) {
        this.key = src.getKey().clone();
        this.name = src.getName();
        this.typeName = src.getTypeName();
        this.equivalentTypeName = src.getEquivalentTypeName() == null ? null
                : new ArrayList<>(src.getEquivalentTypeName());
        this.baseTypeName = src.getBaseTypeName() == null ? null
                : new ArrayList<>(src.getBaseTypeName());
        this.type = src.getType();
        if (src instanceof TopicBuiltinTopicDataImpl) {
            this.typeInformation = ((TopicBuiltinTopicDataImpl) src).typeInformation;
            this.typeObjectBytes = ((TopicBuiltinTopicDataImpl) src).typeObjectBytes;
        } else {
            // the state of the target is overwritten: what only this implementation holds is gone
            this.typeInformation = null;
            this.typeObjectBytes = null;
        }
        this.durability = src.getDurability();
        this.durabilityService = src.getDurabilityService();
        this.deadline = src.getDeadline();
        this.latencyBudget = src.getLatencyBudget();
        this.liveliness = src.getLiveliness();
        this.reliability = src.getReliability();
        this.transportPriority = src.getTransportPriority();
        this.lifespan = src.getLifespan();
        this.destinationOrder = src.getDestinationOrder();
        this.history = src.getHistory();
        this.resourceLimits = src.getResourceLimits();
        this.ownership = src.getOwnership();
        this.topicData = src.getTopicData();
        this.representation = src.getRepresentation();
        this.typeConsistency = src.getTypeConsistency();
    }

    @Override
    public TopicBuiltinTopicData clone() {
        TopicBuiltinTopicDataImpl copy = new TopicBuiltinTopicDataImpl(environment);
        copy.copyFrom(this);
        return copy;
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public String toString() {
        return "TopicBuiltinTopicData[" + name + "," + typeName + "]";
    }

    /**
     * Java serialization (DDS-Java-PSM 1.0 §7.3.3) carries the value, not its
     * {@link ServiceEnvironment}: an environment is a runtime object and is not
     * serializable. A deserialized object belongs to the default environment of
     * this implementation.
     */
    private void readObject(java.io.ObjectInputStream in)
            throws java.io.IOException, ClassNotFoundException {
        in.defaultReadObject();
        environment = ServiceEnvironmentImpl.getDefaultEnvironment();
    }
}
