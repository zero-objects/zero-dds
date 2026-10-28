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

package org.omg.dds.topic;

import java.util.EventListener;


import org.omg.dds.core.event.InconsistentTopicEvent;


/**
 * Since {@link org.omg.dds.topic.Topic} is a kind of {@link org.omg.dds.core.Entity}, it has the ability to
 * have an associated listener. In this case, the associated listener must be
 * of concrete type TopicListener.
 * 
 * @param <TYPE>    The concrete type of the data published and/or subscribed
 *                  by the readers and writers that use to topic.
 */
public interface TopicListener<TYPE> extends EventListener {
    public void onInconsistentTopic(InconsistentTopicEvent<TYPE> status);
}
