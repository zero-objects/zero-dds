package org.omg.dds.core.policy;

import org.omg.dds.core.ServiceEnvironment;

public abstract class PolicyFactory implements org.omg.dds.core.DDSObject {

	public static PolicyFactory getPolicyFactory(ServiceEnvironment env)
	{
		return env.getSPI().getPolicyFactory();
	}
	
    /**
     * @return the durability
     */
    public abstract Durability Durability();

    /**
     * @return the deadline
     */
    public abstract Deadline Deadline();

    /**
     * @return the latencyBudget
     */
    public abstract LatencyBudget LatencyBudget();

    /**
     * @return the liveliness
     */
    public abstract Liveliness Liveliness();

    /**
     * @return the destinationOrder
     */
    public abstract DestinationOrder DestinationOrder();

    /**
     * @return the history
     */
    public abstract History History();

    /**
     * @return the resourceLimits
     */
    public abstract ResourceLimits ResourceLimits();

    /**
     * @return the userData
     */
    public abstract UserData UserData();

    /**
     * @return the ownership
     */
    public abstract Ownership Ownership();

    /**
     * @return the timeBasedFilter
     */
    public abstract TimeBasedFilter TimeBasedFilter();

    /**
     * @return the readerDataLifecycle
     */
    public abstract ReaderDataLifecycle ReaderDataLifecycle();

    /**
     * @return the Representation
     */
    public abstract DataRepresentation Representation();

    /**
     * @return the typeConsistency
     */
    public abstract TypeConsistencyEnforcement TypeConsistency();

    /**
     * @return the durabilityService
     */
    public abstract DurabilityService DurabilityService();

    /**
     * @return the reliability
     */
    public abstract Reliability Reliability();

    /**
     * @return the transportPriority
     */
    public abstract TransportPriority TransportPriority();

    /**
     * @return the lifespan
     */
    public abstract Lifespan Lifespan();

    /**
     * @return the ownershipStrength
     */
    public abstract OwnershipStrength OwnershipStrength();

    /**
     * @return the writerDataLifecycle
     */
    public abstract WriterDataLifecycle WriterDataLifecycle();

    /**
     * @return the partition
     */
    public abstract Partition Partition();

    /**
     * @return the groupData
     */
    public abstract GroupData GroupData();

    /**
     * @return the entityFactory
     */
    public abstract EntityFactory EntityFactory();
}

