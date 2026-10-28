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

package org.omg.dds.core.status;

import java.util.Set;

import org.omg.dds.core.policy.QosPolicy;
import org.omg.dds.core.policy.QosPolicyCount;


/**
 * A {@link org.omg.dds.core.policy.QosPolicy} value was incompatible with what was requested.
 *
 * @see org.omg.dds.core.event.OfferedIncompatibleQosEvent
 * @see RequestedIncompatibleQosStatus
 */
public abstract class OfferedIncompatibleQosStatus extends Status
{
    // -----------------------------------------------------------------------
    // Constants
    // -----------------------------------------------------------------------

    private static final long serialVersionUID = 2435270267101388501L;



    // -----------------------------------------------------------------------
    // Methods
    // -----------------------------------------------------------------------

    /**
     * Total cumulative number of times the concerned {@link org.omg.dds.pub.DataWriter}
     * discovered a {@link org.omg.dds.sub.DataReader} for the same {@link org.omg.dds.topic.Topic} with a
     * requested QoS that is incompatible with that offered by the
     * DataWriter.
     */
    public abstract int getTotalCount();

    /**
     * The change in totalCount since the last time the listener was called
     * or the status was read.
     */
    public abstract int getTotalCountChange();

    /**
     * The class of one of the policies that was found to be
     * incompatible the last time an incompatibility was detected.
     */
    public abstract Class<? extends QosPolicy> getLastPolicyClass();

    /**
     * A list containing for each policy the total number of times that the
     * concerned {@link org.omg.dds.pub.DataWriter} discovered a {@link org.omg.dds.sub.DataReader} for the
     * same {@link org.omg.dds.topic.Topic} with a requested QoS that is incompatible with that
     * offered by the DataWriter.
     * 
     * @return  an unmodifiable set of policy counts.
     */
    public abstract Set<QosPolicyCount> getPolicies();

}
