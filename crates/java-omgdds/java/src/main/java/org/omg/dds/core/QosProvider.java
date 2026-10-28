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

package org.omg.dds.core;

import org.omg.dds.domain.DomainParticipantFactoryQos;
import org.omg.dds.domain.DomainParticipantQos;
import org.omg.dds.pub.DataWriterQos;
import org.omg.dds.pub.PublisherQos;
import org.omg.dds.sub.DataReaderQos;
import org.omg.dds.sub.SubscriberQos;
import org.omg.dds.topic.TopicQos;

public abstract class QosProvider implements DDSObject {

	public static QosProvider newQosProvider(String uri, 
			                                 String profile, 
			                                 ServiceEnvironment env)
	{
		return env.getSPI().newQosProvider(uri, profile);
	}
	
	public abstract DomainParticipantFactoryQos getDomainParticipantFactoryQos();

	/**
	 * Get the first domain participant QoS with identifier that matches the id.
	 * 
	 * @param id The identifier of the domain participant QoS of interest  
	 * @return Domain participant QoS  
	 */

	public abstract DomainParticipantFactoryQos getDomainParticipantFactoryQos(String id);
	
	public abstract DomainParticipantQos getDomainParticipantQos();
	public abstract DomainParticipantQos getDomainParticipantQos(String id);
	
	public abstract TopicQos getTopicQos();
	public abstract TopicQos getTopicQos(String id);
	
	public abstract SubscriberQos getSubscriberQos();
	public abstract SubscriberQos getSubscriberQos(String id);
	
	public abstract PublisherQos getPublisherQos();
	public abstract PublisherQos getPublisherQos(String id);
	
	public abstract DataReaderQos getDataReaderQos();
	public abstract DataReaderQos getDataReaderQos(String id);
	
	public abstract DataWriterQos getDataWriterQos();
	public abstract DataWriterQos getDataWriterQos(String id);
}
