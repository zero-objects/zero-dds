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

import org.omg.dds.core.InstanceHandle;


/**
 * The {@link org.omg.dds.sub.DataReader} has found a {@link org.omg.dds.pub.DataWriter} that matches the
 * {@link org.omg.dds.topic.Topic} and has compatible QoS, or has ceased to be matched with a
 * DataWriter that was previously considered to be matched.
 *
 * @see org.omg.dds.core.event.SubscriptionMatchedEvent
 * @see PublicationMatchedStatus
 */
public abstract class SubscriptionMatchedStatus extends Status
{
    // -----------------------------------------------------------------------
    // Constants
    // -----------------------------------------------------------------------

    private static final long serialVersionUID = 8269669800428084585L;



    // -----------------------------------------------------------------------
    // Methods
    // -----------------------------------------------------------------------

    /**
     * Total cumulative count the concerned {@link org.omg.dds.sub.DataReader} discovered a
     * "match" with a {@link org.omg.dds.pub.DataWriter}. That is, it found a DataWriter for
     * the same {@link org.omg.dds.topic.Topic} with a requested QoS that is compatible with
     * that offered by the DataReader.
     */
    public abstract int getTotalCount();

    /**
     * The change in totalCount since the last time the listener was called
     * or the status was read.
     */
    public abstract int getTotalCountChange();

    /**
     * The number of {@link org.omg.dds.pub.DataWriter}s currently matched to the concerned
     * {@link org.omg.dds.sub.DataReader}.
     */
    public abstract int getCurrentCount();

    /**
     * The change in currentCount since the last time the listener was called
     * or the status was read.
     */
    public abstract int getCurrentCountChange();

    /**
     * Handle to the last {@link org.omg.dds.pub.DataWriter} that matched the
     * {@link org.omg.dds.sub.DataReader}, causing the status to change.
     */
    public abstract InstanceHandle getLastPublicationHandle();
}
