/* Copyright 2010, Object Management Group, Inc.
 * Copyright 2010, PrismTech, Inc.
 * Copyright 2010, Real-Time Innovations, Inc.
 * All rights reserved.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

package org.omg.dds.pub;

import org.omg.dds.core.EntityQos;
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
import org.omg.dds.core.policy.OwnershipStrength;
import org.omg.dds.core.policy.QosPolicy;
import org.omg.dds.core.policy.Reliability;
import org.omg.dds.core.policy.ResourceLimits;
import org.omg.dds.core.policy.TransportPriority;
import org.omg.dds.core.policy.TypeConsistencyEnforcement;
import org.omg.dds.core.policy.UserData;
import org.omg.dds.core.policy.WriterDataLifecycle;


public interface DataWriterQos
extends EntityQos<QosPolicy.ForDataWriter>
{
    /**
     * @return the durability
     */
    public Durability getDurability();

    /**
     * @return the durabilityService
     */
    public DurabilityService getDurabilityService();

    /**
     * @return the deadline
     */
    public Deadline getDeadline();

    /**
     * @return the latencyBudget
     */
    public LatencyBudget getLatencyBudget();

    /**
     * @return the liveliness
     */
    public Liveliness getLiveliness();

    /**
     * @return the reliability
     */
    public Reliability getReliability();

    /**
     * @return the destinationOrder
     */
    public DestinationOrder getDestinationOrder();

    /**
     * @return the history
     */
    public History getHistory();

    /**
     * @return the resourceLimits
     */
    public ResourceLimits getResourceLimits();

    /**
     * @return the transportPriority
     */
    public TransportPriority getTransportPriority();

    /**
     * @return the lifespan
     */
    public Lifespan getLifespan();

    /**
     * @return the userData
     */
    public UserData getUserData();

    /**
     * @return the ownership
     */
    public Ownership getOwnership();

    /**
     * @return the ownershipStrength
     */
    public OwnershipStrength getOwnershipStrength();

    /**
     * @return the writerDataLifecycle
     */
    public WriterDataLifecycle getWriterDataLifecycle();

    public DataRepresentation getRepresentation();

    public TypeConsistencyEnforcement getTypeConsistency();


    // --- Modification: -----------------------------------------------------

    public DataWriterQos withPolicy(QosPolicy.ForDataWriter policy);

    public DataWriterQos withPolicies(QosPolicy.ForDataWriter... policy);
}
