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
 * {@link org.omg.dds.sub.DataReader} expects a new sample updating the value of each
 * instance at least once every deadline period. The {@link org.omg.dds.pub.DataWriter}
 * indicates that the application commits to write a new value (using the
 * DataWriter) for each instance managed by the DataWriter at least once
 * every deadline period. It is inconsistent for a DataReader to have a
 * deadline period less than the result of its
 * {@link org.omg.dds.core.policy.TimeBasedFilter#getMinimumSeparation()}. The default value
 * of the deadline period is infinite.
 * 
 * <b>Concerns:</b> {@link org.omg.dds.topic.Topic}, {@link org.omg.dds.sub.DataReader}, {@link org.omg.dds.pub.DataWriter}
 * 
 * <b>RxO:</b> Yes
 * 
 * <b>Changeable:</b> Yes
 * 
 * This policy is useful for cases where a {@link org.omg.dds.topic.Topic} is expected to have
 * each instance updated periodically. On the publishing side this setting
 * establishes a contract that the application must meet. On the subscribing
 * side the setting establishes a minimum requirement for the remote
 * publishers that are expected to supply the data values.
 * 
 * When the Service "matches" a DataWriter and a DataReader it checks whether
 * the settings are compatible (i.e., offered deadline period<= requested
 * deadline period). If they are not, the two entities are informed (via the
 * listener or {@link org.omg.dds.core.Condition} mechanism) of the incompatibility of the QoS
 * settings and communication will not occur.
 * 
 * Assuming that the reader and writer ends have compatible settings, the
 * fulfillment of this contract is monitored by the Service and the
 * application is informed of any violations by means of the proper listener
 * or condition.
 * 
 * The value offered is considered compatible with the value requested if and
 * only if the inequality "offered deadline period <= requested deadline
 * period" evaluates to true.
 * 
 * The setting of the DEADLINE policy must be set consistently with that of
 * the {@link org.omg.dds.core.policy.TimeBasedFilter}. For these two policies to be
 * consistent the settings must be such that "deadline period >=
 * minimum_separation."
 */
public interface Deadline
extends QosPolicy.ForTopic,
        QosPolicy.ForDataReader,
        QosPolicy.ForDataWriter,
        RequestedOffered<Deadline>
{
    public Duration getPeriod();


    // --- Modification: -----------------------------------------------------

    /**
     * Copy this policy and override the value of the property.
     * 
     * @return  a new policy
     */
    public Deadline withPeriod(Duration period);

    /**
     * Copy this policy and override the value of the property.
     * 
     * @return  a new policy
     */
    public Deadline withPeriod(long period, TimeUnit unit);
}
