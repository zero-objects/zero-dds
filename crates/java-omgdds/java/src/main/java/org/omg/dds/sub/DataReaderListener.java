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

package org.omg.dds.sub;

import java.util.EventListener;


import org.omg.dds.core.event.DataAvailableEvent;
import org.omg.dds.core.event.LivelinessChangedEvent;
import org.omg.dds.core.event.RequestedDeadlineMissedEvent;
import org.omg.dds.core.event.RequestedIncompatibleQosEvent;
import org.omg.dds.core.event.SampleLostEvent;
import org.omg.dds.core.event.SampleRejectedEvent;
import org.omg.dds.core.event.SubscriptionMatchedEvent;



/**
 * Since a {@link org.omg.dds.sub.DataReader} is a kind of {@link org.omg.dds.core.Entity}, it has the ability
 * to have an associated listener. In this case, the associated listener must
 * be of concrete type DataReaderListener.
 * 
 * The operation {@link #onSubscriptionMatched(SubscriptionMatchedEvent)} is
 * intended to inform the application of the discovery of {@link org.omg.dds.pub.DataWriter}
 * entities that match the DataReader. Some implementations of the service
 * may not propagate this information. In that case the DDS specification
 * does not require this listener operation to be called.
 * 
 * @param <TYPE>    The concrete type of the data that can be delivered by
 *                  the {@link org.omg.dds.sub.DataReader}.
 */
public interface DataReaderListener<TYPE> extends EventListener {
    public void onRequestedDeadlineMissed(
            RequestedDeadlineMissedEvent<TYPE> status);

    public void onRequestedIncompatibleQos(
            RequestedIncompatibleQosEvent<TYPE> status);

    public void onSampleRejected(SampleRejectedEvent<TYPE> status);

    public void onLivelinessChanged(LivelinessChangedEvent<TYPE> status);

    public void onDataAvailable(DataAvailableEvent<TYPE> status);

    public void onSubscriptionMatched(SubscriptionMatchedEvent<TYPE> status);

    public void onSampleLost(SampleLostEvent<TYPE> status);
}
