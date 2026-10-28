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
 * Specifies the maximum duration of validity of the data written by the
 * {@link org.omg.dds.pub.DataWriter}. The default value of the lifespan duration is infinite.
 * 
 * <b>Concerns:</b> {@link org.omg.dds.topic.Topic}, {@link org.omg.dds.pub.DataWriter}
 * 
 * <b>RxO:</b> N/A
 * 
 * <b>Changeable:</b> Yes
 * 
 * The purpose of this QoS is to avoid delivering "stale" data to the
 * application.
 * 
 * Each data sample written by the DataWriter has an associated "expiration
 * time" beyond which the data should not be delivered to any application.
 * Once the sample expires, the data will be removed from the DataReader
 * caches as well as from the transient and persistent information caches.
 * 
 * The "expiration time" of each sample is computed by adding the duration
 * specified by the LIFESPAN QoS to the source time stamp. As described in
 * {@link org.omg.dds.pub.DataWriter#write(Object)} and
 * {@link org.omg.dds.pub.DataWriter#write(Object, org.omg.dds.core.Time)}, the source time
 * stamp is either automatically computed by the Service each time the
 * write operation is called, or else supplied by the application.
 * 
 * This QoS relies on the sender and receiving applications having their
 * clocks sufficiently synchronized. If this is not the case and the Service
 * can detect it, the DataReader is allowed to use the reception time stamp
 * instead of the source time stamp in its computation of the "expiration
 * time."
 */
public interface Lifespan
extends QosPolicy.ForTopic, QosPolicy.ForDataWriter
{
    public Duration getDuration();


    // --- Modification: -----------------------------------------------------

    /**
     * Copy this policy and override the value of the property.
     * 
     * @return  a new policy
     */
    public Lifespan withDuration(Duration duration);

    /**
     * Copy this policy and override the value of the property.
     * 
     * @return  a new policy
     */
    public Lifespan withDuration(long duration, TimeUnit unit);
}
