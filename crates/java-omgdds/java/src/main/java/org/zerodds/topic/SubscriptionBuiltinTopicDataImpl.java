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
import org.omg.dds.core.policy.GroupData;
import org.omg.dds.core.policy.LatencyBudget;
import org.omg.dds.core.policy.Liveliness;
import org.omg.dds.core.policy.Ownership;
import org.omg.dds.core.policy.Partition;
import org.omg.dds.core.policy.Presentation;
import org.omg.dds.core.policy.Reliability;
import org.omg.dds.core.policy.TimeBasedFilter;
import org.omg.dds.core.policy.TopicData;
import org.omg.dds.core.policy.TypeConsistencyEnforcement;
import org.omg.dds.core.policy.UserData;
import org.omg.dds.topic.BuiltinTopicKey;
import org.omg.dds.topic.SubscriptionBuiltinTopicData;
import org.omg.dds.type.typeobject.TypeObject;

/**
 * ZeroDDS {@link SubscriptionBuiltinTopicData} (DDS 1.2 §2.2.1.3.1,
 * DDSI-RTPS 2.5 §9.3.1.2): what the "DCPSSubscription" built-in topic
 * carries — the key of the discovered DataReader, the key of the participant
 * that created it, the topic it reads, and the QoS it requests.
 */
public final class SubscriptionBuiltinTopicDataImpl
implements SubscriptionBuiltinTopicData {

    private static final long serialVersionUID = 1L;

    private transient ServiceEnvironment environment;

    private BuiltinTopicKey key;
    private BuiltinTopicKey participantKey;
    private String topicName;
    private String typeName;
    private ArrayList<String> equivalentTypeName;
    private ArrayList<String> baseTypeName;
    private TypeObject type;
    /** PID_TYPE_INFORMATION (XTypes 1.3 §7.6.3.2.2): the XCDR2 TypeInformation, or {@code null}. */
    private byte[] typeInformation;
    /** PID_TYPE (XTypes 1.3 §7.6.3.2.2): the XTypes 1.1 TypeObject as announced, or {@code null}. */
    private byte[] typeObjectBytes;
    private Durability durability;
    private Deadline deadline;
    private LatencyBudget latencyBudget;
    private Liveliness liveliness;
    private Reliability reliability;
    private Ownership ownership;
    private DestinationOrder destinationOrder;
    private UserData userData;
    private TimeBasedFilter timeBasedFilter;
    private Presentation presentation;
    private Partition partition;
    private TopicData topicData;
    private GroupData groupData;
    private DataRepresentation representation;
    private TypeConsistencyEnforcement typeConsistency;

    public SubscriptionBuiltinTopicDataImpl(ServiceEnvironment environment) {
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
    public BuiltinTopicKey getParticipantKey() {
        return participantKey;
    }

    public void setParticipantKey(BuiltinTopicKey participantKey) {
        this.participantKey = participantKey;
    }

    @Override
    public String getTopicName() {
        return topicName;
    }

    public void setTopicName(String topicName) {
        this.topicName = topicName;
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
    public Ownership getOwnership() {
        return ownership;
    }

    public void setOwnership(Ownership ownership) {
        this.ownership = ownership;
    }

    @Override
    public DestinationOrder getDestinationOrder() {
        return destinationOrder;
    }

    public void setDestinationOrder(DestinationOrder destinationOrder) {
        this.destinationOrder = destinationOrder;
    }

    @Override
    public UserData getUserData() {
        return userData;
    }

    public void setUserData(UserData userData) {
        this.userData = userData;
    }

    @Override
    public TimeBasedFilter getTimeBasedFilter() {
        return timeBasedFilter;
    }

    public void setTimeBasedFilter(TimeBasedFilter timeBasedFilter) {
        this.timeBasedFilter = timeBasedFilter;
    }

    @Override
    public Presentation getPresentation() {
        return presentation;
    }

    public void setPresentation(Presentation presentation) {
        this.presentation = presentation;
    }

    @Override
    public Partition getPartition() {
        return partition;
    }

    public void setPartition(Partition partition) {
        this.partition = partition;
    }

    @Override
    public TopicData getTopicData() {
        return topicData;
    }

    public void setTopicData(TopicData topicData) {
        this.topicData = topicData;
    }

    @Override
    public GroupData getGroupData() {
        return groupData;
    }

    public void setGroupData(GroupData groupData) {
        this.groupData = groupData;
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
    public void copyFrom(SubscriptionBuiltinTopicData src) {
        this.key = src.getKey().clone();
        this.participantKey = src.getParticipantKey().clone();
        this.topicName = src.getTopicName();
        this.typeName = src.getTypeName();
        this.equivalentTypeName = src.getEquivalentTypeName() == null ? null
                : new ArrayList<>(src.getEquivalentTypeName());
        this.baseTypeName = src.getBaseTypeName() == null ? null
                : new ArrayList<>(src.getBaseTypeName());
        this.type = src.getType();
        if (src instanceof SubscriptionBuiltinTopicDataImpl) {
            this.typeInformation = ((SubscriptionBuiltinTopicDataImpl) src).typeInformation;
            this.typeObjectBytes = ((SubscriptionBuiltinTopicDataImpl) src).typeObjectBytes;
        } else {
            // the state of the target is overwritten: what only this implementation holds is gone
            this.typeInformation = null;
            this.typeObjectBytes = null;
        }
        this.durability = src.getDurability();
        this.deadline = src.getDeadline();
        this.latencyBudget = src.getLatencyBudget();
        this.liveliness = src.getLiveliness();
        this.reliability = src.getReliability();
        this.ownership = src.getOwnership();
        this.destinationOrder = src.getDestinationOrder();
        this.userData = src.getUserData();
        this.timeBasedFilter = src.getTimeBasedFilter();
        this.presentation = src.getPresentation();
        this.partition = src.getPartition();
        this.topicData = src.getTopicData();
        this.groupData = src.getGroupData();
        this.representation = src.getRepresentation();
        this.typeConsistency = src.getTypeConsistency();
    }

    @Override
    public SubscriptionBuiltinTopicData clone() {
        SubscriptionBuiltinTopicDataImpl copy =
                new SubscriptionBuiltinTopicDataImpl(environment);
        copy.copyFrom(this);
        return copy;
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public String toString() {
        return "SubscriptionBuiltinTopicData[" + topicName + "," + typeName + "]";
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
