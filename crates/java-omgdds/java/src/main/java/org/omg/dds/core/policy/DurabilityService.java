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

package org.omg.dds.core.policy;

import java.util.concurrent.TimeUnit;

import org.omg.dds.core.Duration;


/**
 * Specifies the configuration of the durability service. That is, the
 * service that implements the {@link Durability.Kind} of
 * {@link Durability.Kind#TRANSIENT} and
 * {@link Durability.Kind#PERSISTENT}.
 * 
 * <b>Concerns:</b> {@link org.omg.dds.topic.Topic}, {@link org.omg.dds.pub.DataWriter}
 * 
 * <b>RxO:</b> No
 * 
 * <b>Changeable:</b> No
 * 
 * This policy is used to configure the {@link org.omg.dds.core.policy.History} and the
 * {@link org.omg.dds.core.policy.ResourceLimits} used by the fictitious {@link org.omg.dds.sub.DataReader}
 * and {@link org.omg.dds.pub.DataWriter} used by the "persistence service." The "persistence
 * service" is the one responsible for implementing
 * {@link Durability.Kind#TRANSIENT} and
 * {@link Durability.Kind#PERSISTENT}.
 * 
 * @see Durability
 */
public interface DurabilityService
extends QosPolicy.ForTopic, QosPolicy.ForDataWriter
{
    public Duration getServiceCleanupDelay();

    /**
     * @return the historyKind
     */
    public History.Kind getHistoryKind();

    /**
     * @return the historyDepth
     */
    public int getHistoryDepth();

    /**
     * @return the maxSamples
     */
    public int getMaxSamples();

    /**
     * @return the maxInstances
     */
    public int getMaxInstances();

    /**
     * @return the maxSamplesPerInstance
     */
    public int getMaxSamplesPerInstance();


    // --- Modification: -----------------------------------------------------

    /**
     * Copy this policy and override the value of the property.
     * 
     * @return  a new policy
     */
    public DurabilityService withServiceCleanupDelay(
            Duration serviceCleanupDelay);

    /**
     * Copy this policy and override the value of the property.
     * 
     * @return  a new policy
     */
    public DurabilityService withServiceCleanupDelay(
            long serviceCleanupDelay,
            TimeUnit unit);

    /**
     * Copy this policy and override the value of the property.
     * 
     * @return  a new policy
     */
    public DurabilityService withHistoryKind(
            History.Kind historyKind);

    /**
     * Copy this policy and override the value of the property.
     * 
     * @return  a new policy
     */
    public DurabilityService withHistoryDepth(int historyDepth);

    /**
     * Copy this policy and override the value of the property.
     * 
     * @return  a new policy
     */
    public DurabilityService withMaxSamples(int maxSamples);

    /**
     * Copy this policy and override the value of the property.
     * 
     * @return  a new policy
     */
    public DurabilityService withMaxInstances(int maxInstances);

    /**
     * Copy this policy and override the value of the property.
     * 
     * @return  a new policy
     */
    public DurabilityService withMaxSamplesPerInstance(
            int maxSamplesPerInstance);
}
