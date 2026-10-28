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

package org.omg.dds.domain;

import org.omg.dds.core.event.InconsistentTopicEvent;
import org.omg.dds.pub.PublisherListener;
import org.omg.dds.sub.SubscriberListener;


/**
 * This is the interface that can be implemented by an application-provided
 * class and then registered with the {@link org.omg.dds.domain.DomainParticipant} such that the
 * application can be notified by the DCPS Service of relevant status changes.
 * 
 * The purpose of the DomainParticipantListener is to be the listener of last
 * resort that is notified of all status changes not captured by more specific
 * listeners attached to the {@link org.omg.dds.core.DomainEntity} objects. When a relevant
 * status change occurs, the DCPS Service will first attempt to notify the
 * listener attached to the concerned DomainEntity if one is installed.
 * Otherwise, the DCPS Service will notify the Listener attached to the
 * DomainParticipant.
 */
public interface DomainParticipantListener
extends PublisherListener, SubscriberListener {
    public void onInconsistentTopic(InconsistentTopicEvent<?> status);
}
