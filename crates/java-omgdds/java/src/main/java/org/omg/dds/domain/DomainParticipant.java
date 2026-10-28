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

import java.util.Collection;
import java.util.List;
import java.util.Set;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.TimeoutException;

import org.omg.dds.core.Duration;
import org.omg.dds.core.Entity;
import org.omg.dds.core.InstanceHandle;
import org.omg.dds.core.ModifiableTime;
import org.omg.dds.core.StatusCondition;
import org.omg.dds.core.Time;
import org.omg.dds.core.status.Status;
import org.omg.dds.pub.Publisher;
import org.omg.dds.pub.PublisherListener;
import org.omg.dds.pub.PublisherQos;
import org.omg.dds.sub.Subscriber;
import org.omg.dds.sub.SubscriberListener;
import org.omg.dds.sub.SubscriberQos;
import org.omg.dds.topic.ContentFilteredTopic;
import org.omg.dds.topic.MultiTopic;
import org.omg.dds.topic.ParticipantBuiltinTopicData;
import org.omg.dds.topic.Topic;
import org.omg.dds.topic.TopicBuiltinTopicData;
import org.omg.dds.topic.TopicDescription;
import org.omg.dds.topic.TopicListener;
import org.omg.dds.topic.TopicQos;
import org.omg.dds.type.TypeSupport;
import org.omg.dds.type.dynamic.DynamicType;


/**
 * The DomainParticipant object plays several roles:
 * <ul>
 * <li>It acts as a container for all other {@link org.omg.dds.core.Entity} objects.</li>
 * <li>It acts as factory for the {@link org.omg.dds.pub.Publisher}, {@link org.omg.dds.sub.Subscriber},
 *     {@link org.omg.dds.topic.Topic}, {@link org.omg.dds.topic.ContentFilteredTopic}, and {@link org.omg.dds.topic.MultiTopic}
 *     objects.</li>
 * <li>It represents the participation of the application on a communication
 *     plane that isolates applications running on the same set of physical
 *     computers from each other. A domain establishes a "virtual network"
 *     linking all applications that share the same domainId and isolating
 *     them from applications running on different domains. In this way,
 *     several independent distributed applications can coexist in the same
 *     physical network without interfering, or even being aware of each
 *     other.</li>
 * <li>It provides administration services in the domain, offering operations
 *     that allow the application to "ignore" locally any information about a
 *     given participant ({@link #ignoreParticipant(InstanceHandle)}),
 *     publication ({@link #ignorePublication(InstanceHandle)}),
 *     subscription ({@link #ignoreSubscription(InstanceHandle)}), or topic
 *     ({@link #ignoreTopic(InstanceHandle)}).</li>
 * </ul>
 */
public interface DomainParticipant
extends Entity<DomainParticipantListener, DomainParticipantQos>
{
    // --- Create Publisher: -------------------------------------------------

    /**
     * This operation creates a Publisher with default QoS policies and
     * no PublisherListener.
     * 
     * The created Publisher belongs to the DomainParticipant that is its
     * factory.
     * 
     * @see     #createPublisher(PublisherQos, PublisherListener, Collection)
     */
    public Publisher createPublisher();

    /**
     * This operation creates a Publisher.
     * 
     * The created Publisher belongs to the DomainParticipant that is its
     * factory.
     * 
     * @param   qos     The desired QoS policies. If the specified QoS
     *                  policies are not consistent, the operation will
     *                  fail and no Publisher will be created.
     *
     * @see     #createPublisher()
     */
    public Publisher createPublisher(PublisherQos qos);

    /**
     * This operation creates a Publisher.
     * 
     * The created Publisher belongs to the DomainParticipant that is its
     * factory.
     * 
     * @param   qos     The desired QoS policies. If the specified QoS
     *                  policies are not consistent, the operation will
     *                  fail and no Publisher will be created.
     * @param   listener    The listener to be attached.
     * @param   statuses    Of which status changes the listener should be
     *                      notified. A null collection signifies all status
     *                      changes.
     *
     * @see     #createPublisher()
     */
    public Publisher createPublisher(
            PublisherQos qos,
            PublisherListener listener,
            Collection<Class<? extends Status>> statuses);

    // --- Create Subscriber: ------------------------------------------------

    /**
     * This operation creates a Subscriber with default QoS policies and
     * no SubscriberListener.
     * 
     * The created Subscriber belongs to the DomainParticipant that is its
     * factory.
     * 
     * @see     #createSubscriber(SubscriberQos, SubscriberListener, Collection)
     */
    public Subscriber createSubscriber();

    /**
     * This operation creates a Subscriber.
     * 
     * The created Subscriber belongs to the DomainParticipant that is its
     * factory.
     * 
     * @param   qos     The desired QoS policies. If the specified QoS
     *                  policies are not consistent, the operation will
     *                  fail and no Subscriber will be created.
     *
     * @see     #createSubscriber()
     */
    public Subscriber createSubscriber(SubscriberQos qos);

    /**
     * This operation creates a Subscriber.
     * 
     * The created Subscriber belongs to the DomainParticipant that is its
     * factory.
     * 
     * @param   qos     The desired QoS policies. If the specified QoS
     *                  policies are not consistent, the operation will
     *                  fail and no Subscriber will be created.
     * @param   listener    The listener to be attached.
     * @param   statuses    Of which status changes the listener should be
     *                      notified. A null collection signifies all status
     *                      changes.
     *
     * @see     #createSubscriber()
     */
    public Subscriber createSubscriber(
            SubscriberQos qos,
            SubscriberListener listener,
            Collection<Class<? extends Status>> statuses);

    /**
     * This operation allows access to the built-in Subscriber. Each
     * DomainParticipant contains several built-in {@link org.omg.dds.topic.Topic} objects as
     * well as corresponding {@link org.omg.dds.sub.DataReader} objects to access them. All
     * these DataReader objects belong to a single built-in Subscriber.
     * 
     * The built-in Topics are used to communicate information about other
     * DomainParticipant, Topic, {@link org.omg.dds.sub.DataReader}, and {@link org.omg.dds.pub.DataWriter}
     * objects. 
     */
    public Subscriber getBuiltinSubscriber();


    // --- Create Topic with implicit TypeSupport: ---------------------------

    /**
     * This operation creates a Topic with default QoS policies and no
     * TopicListener.
     * 
     * The created Topic belongs to the DomainParticipant that is its
     * factory.
     * 
     * @param   topicName   The name of the new Topic.
     * @param   type        The type of all samples to be published and
     *                      subscribed over the new Topic. The Service will
     *                      attempt to locate an appropriate
     *                      {@link org.omg.dds.type.TypeSupport} instance based on this type.
     */
    public <TYPE> Topic<TYPE> createTopic(
            String topicName,
            Class<TYPE> type);

    /**
     * This operation creates a Topic with the desired QoS policies and
     * attaches to it the specified TopicListener.
     * 
     * The created Topic belongs to the DomainParticipant that is its
     * factory.
     * 
     * @param   topicName   The name of the new Topic.
     * @param   type        The type of all samples to be published and
     *                      subscribed over the new Topic. The Service will
     *                      attempt to locate an appropriate
     *                      {@link org.omg.dds.type.TypeSupport} instance based on this type.
     * @param   qos         The desired QoS policies. If the specified QoS
     *                      policies are not consistent, the operation will
     *                      fail and no Publisher will be created.
     * @param   listener    The listener to be attached.
     * @param   statuses    Of which status changes the listener should be
     *                      notified. A null collection signifies all status
     *                      changes.
     */
    public <TYPE> Topic<TYPE> createTopic(
            String topicName,
            Class<TYPE> type,
            TopicQos qos,
            TopicListener<TYPE> listener,
            Collection<Class<? extends Status>> statuses);

    // --- Create Topic with explicit TypeSupport: ---------------------------

    /**
     * This operation creates a Topic with default QoS policies and no
     * TopicListener.
     * 
     * The created Topic belongs to the DomainParticipant that is its
     * factory.
     * 
     * @param   topicName   The name of the new Topic.
     * @param   type        A {@link org.omg.dds.type.TypeSupport} representing the type of
     *                      all samples to be published and subscribed over
     *                      the new Topic.
     */
    public <TYPE> Topic<TYPE> createTopic(
            String topicName,
            TypeSupport<TYPE> type);

    /**
     * This operation creates a Topic with the desired QoS policies and
     * attaches to it the specified TopicListener.
     * 
     * The created Topic belongs to the DomainParticipant that is its
     * factory.
     * 
     * @param   topicName   The name of the new Topic.
     * @param   type        A {@link org.omg.dds.type.TypeSupport} representing the type of
     *                      all samples to be published and subscribed over
     *                      the new Topic.
     * @param   qos         The desired QoS policies. If the specified QoS
     *                      policies are not consistent, the operation will
     *                      fail and no Publisher will be created.
     * @param   listener    The listener to be attached.
     * @param   statuses    Of which status changes the listener should be
     *                      notified. A null collection signifies all status
     *                      changes.
     */
    public <TYPE> Topic<TYPE> createTopic(
            String topicName,
            TypeSupport<TYPE> type,
            TopicQos qos,
            TopicListener<TYPE> listener,
            Collection<Class<? extends Status>> statuses);

    // --- Create Topic with implicit TypeSupport: ---------------------------

    /**
     * This operation creates a Topic with default QoS policies and no
     * TopicListener.
     * 
     * The created Topic belongs to the DomainParticipant that is its
     * factory.
     * 
     * @param   topicName   The name of the new Topic.
     * @param   type        A {@link org.omg.dds.type.dynamic.DynamicType} of all samples to be published and
     *                      subscribed over the new Topic. The Service will
     *                      attempt to locate an appropriate
     *                      {@link org.omg.dds.type.TypeSupport} instance based on this type.
     */
    public Topic<DynamicType> createTopic(
            String topicName,
            DynamicType type);

    /**
     * This operation creates a Topic with the desired QoS policies and
     * attaches to it the specified TopicListener.
     * 
     * The created Topic belongs to the DomainParticipant that is its
     * factory.
     * 
     * @param   topicName   The name of the new Topic.
     * @param   type        A {@link org.omg.dds.type.dynamic.DynamicType} of all samples to be published and
     *                      subscribed over the new Topic. The Service will
     *                      attempt to locate an appropriate
     *                      {@link org.omg.dds.type.TypeSupport} instance based on this type.
     * @param   qos         The desired QoS policies. If the specified QoS
     *                      policies are not consistent, the operation will
     *                      fail and no Publisher will be created.
     * @param   listener    The listener to be attached.
     * @param   statuses    Of which status changes the listener should be
     *                      notified. A null collection signifies all status
     *                      changes.
     */
    public Topic<DynamicType> createTopic(
            String topicName,
            DynamicType type,
            TopicQos qos,
            TopicListener<DynamicType> listener,
            Collection<Class<? extends Status>> statuses);

    // --- Create Topic with explicit TypeSupport: ---------------------------

    /**
     * This operation creates a Topic with default QoS policies and no
     * TopicListener.
     * 
     * The created Topic belongs to the DomainParticipant that is its
     * factory.
     * 
     * @param   topicName    The name of the new Topic.
     * @param   type         A {@link org.omg.dds.type.dynamic.DynamicType} representing the type of
     *                       all samples to be published and subscribed over
     *                       the new Topic.
     * @param   typeSupport  A {@link org.omg.dds.type.TypeSupport} for {@link org.omg.dds.type.dynamic.DynamicType}.
     */
    public Topic<DynamicType> createTopic(
            String topicName,
            DynamicType type,
            TypeSupport<DynamicType> typeSupport);

    /**
     * This operation creates a Topic with the desired QoS policies and
     * attaches to it the specified TopicListener.
     * 
     * The created Topic belongs to the DomainParticipant that is its
     * factory.
     * 
     * @param   topicName   The name of the new Topic.
     * @param   type        A {@link org.omg.dds.type.dynamic.DynamicType}
     * @param   typeSupport A {@link org.omg.dds.type.TypeSupport} for {@link org.omg.dds.type.dynamic.DynamicType}
     * @param   qos         The desired QoS policies. If the specified QoS
     *                      policies are not consistent, the operation will
     *                      fail and no Publisher will be created.
     * @param   listener    The listener to be attached.
     * @param   statuses    Of which status changes the listener should be
     *                      notified. A null collection signifies all status
     *                      changes.
     */
    public Topic<DynamicType>createTopic(
            String topicName,
            DynamicType type,
            TypeSupport<DynamicType> typeSupport,
            TopicQos qos,
            TopicListener<DynamicType> listener,
            Collection<Class<? extends Status>> statuses);

    
    // --- Other operations: -------------------------------------------------

    /**
     * This operation gives access to an existing (or ready to exist) enabled
     * Topic, based on its name. The operation takes as arguments the name of
     * the Topic and a timeout.
     * 
     * If a Topic of the same name already exists, it gives access to it,
     * otherwise it waits (blocks the caller) until another mechanism creates
     * it (or the specified timeout occurs). This other mechanism can be
     * another thread, a configuration tool, or some other middleware
     * service. Note that the Topic is a local object that acts as a 'proxy'
     * to designate the global concept of topic. Middleware implementations
     * could choose to propagate topics and make remotely created topics
     * locally available.
     * 
     * A Topic obtained by means of findTopic must also be closed by means of
     * {@link org.omg.dds.topic.Topic#close()} so that the local resources can be released. If
     * a Topic is obtained multiple times by means of findTopic or
     * {@link #createTopic(String, Class)}, it must also be closed that same
     * number of times.
     * 
     * Regardless of whether the middleware chooses to propagate topics, the
     * {@link org.omg.dds.topic.Topic#close()} operation disposes of only the local proxy.
     * 
     * @return  a non-null Topic.
     * 
     * @throws  TimeoutException    if the specified timeout elapses and no
     *                              suitable Topic could be found.
     * 
     * @see     #findTopic(String, long, TimeUnit)
     */
    public <TYPE> Topic<TYPE> findTopic(
            String topicName,
            Duration timeout) throws TimeoutException;

    /**
     * This operation gives access to an existing (or ready to exist) enabled
     * Topic, based on its name. The operation takes as arguments the name of
     * the Topic and a timeout.
     * 
     * If a Topic of the same name already exists, it gives access to it,
     * otherwise it waits (blocks the caller) until another mechanism creates
     * it (or the specified timeout occurs). This other mechanism can be
     * another thread, a configuration tool, or some other middleware
     * service. Note that the Topic is a local object that acts as a 'proxy'
     * to designate the global concept of topic. Middleware implementations
     * could choose to propagate topics and make remotely created topics
     * locally available.
     * 
     * A Topic obtained by means of findTopic must also be closed by means of
     * {@link org.omg.dds.topic.Topic#close()} so that the local resources can be released. If
     * a Topic is obtained multiple times by means of findTopic or
     * {@link #createTopic(String, Class)}, it must also be closed that same
     * number of times.
     * 
     * Regardless of whether the middleware chooses to propagate topics, the
     * {@link org.omg.dds.topic.Topic#close()} operation disposes of only the local proxy.
     * 
     * @return  a non-null Topic.
     * 
     * @throws  TimeoutException    if the specified timeout elapses and no
     *                              suitable Topic could be found.
     * 
     * @see     #findTopic(String, Duration)
     */
    public <TYPE> Topic<TYPE> findTopic(
            String topicName,
            long timeout,
            TimeUnit unit)
            throws TimeoutException;

    /**
     * This operation gives access to an existing locally-created
     * TopicDescription based on its name. The operation takes as argument
     * the name of the TopicDescription.
     * 
     * If a TopicDescription of the same name already exists, it gives access
     * to it, otherwise it returns null. The operation never blocks.
     * 
     * The operation may be used to locate any locally-created {@link org.omg.dds.topic.Topic},
     * {@link org.omg.dds.topic.ContentFilteredTopic}, or {@link org.omg.dds.topic.MultiTopic} object.
     * 
     * Unlike {@link #findTopic(String, Duration)}, the operation searches
     * only among the locally created topics. Therefore, it should never
     * create a new TopicDescription. The TopicDescription returned does not
     * require an extra {@link org.omg.dds.topic.TopicDescription#close()}. It is still
     * possible to close the TopicDescription returned by this method,
     * provided it has no readers or writers, but then it is really closed
     * and subsequent lookups will fail.
     * 
     * If the operation fails to locate a TopicDescription, it returns null.
     * 
     * @param <TYPE>    The type of all samples subscribed to with the
     *                  TopicDescription.
     * @param name      The name of the TopicDescription to look up.
     */
    public <TYPE> TopicDescription<TYPE> lookupTopicDescription(String name);

    /**
     * This operation creates a ContentFilteredTopic. A ContentFilteredTopic
     * can be used to do content-based subscriptions.
     * 
     * @param <TYPE>                The type of all samples subscribed to
     *                              with the new ContentFilteredTopic. It may
     *                              be the same as the type of the
     *                              relatedTopic or any supertype of that
     *                              type.
     * @param name                  The name of the new ContentFilteredTopic.
     * @param relatedTopic          The related Topic being subscribed to.
     *                              The ContentFilteredTopic only relates to
     *                              samples published under this Topic,
     *                              filtered according to their content.
     * @param filterExpression      A logical expression that involves the
     *                              values of some of the data fields in the
     *                              sample.
     * @param expressionParameters  Parameters to the filterExpression.
     */
    public <TYPE> ContentFilteredTopic<TYPE> createContentFilteredTopic(
            String name,
            Topic<? extends TYPE> relatedTopic,
            String filterExpression,
            List<String> expressionParameters);

    /**
     * This operation creates a MultiTopic. A MultiTopic can be used to
     * subscribe to multiple topics and combine/filter the received data into
     * a resulting type. In particular, MultiTopic provides a content-based
     * subscription mechanism.
     * 
     * @param   <TYPE>  The type of all samples subscribed to with the new
     *                  MultiTopic.
     * @param   name    The name of the new MultiTopic.
     * @param   type    The resulting type. The Service will attempt to
     *                  locate an appropriate {@link org.omg.dds.type.TypeSupport} instance
     *                  based on this type.
     * @param   subscriptionExpression  The list of topics and the logic used
     *          to combine filter and re-arrange the information from each
     *          Topic.
     * @param   expressionParameters    Parameters to the filterExpression.
     */
    public <TYPE> MultiTopic<TYPE> createMultiTopic(
            String name,
            Class<TYPE> type,
            String subscriptionExpression,
            List<String> expressionParameters);

    /**
     * This operation creates a MultiTopic. A MultiTopic can be used to
     * subscribe to multiple topics and combine/filter the received data into
     * a resulting type. In particular, MultiTopic provides a content-based
     * subscription mechanism.
     * 
     * @param   <TYPE>  The type of all samples subscribed to with the new
     *                  MultiTopic.
     * @param   name    The name of the new MultiTopic.
     * @param   type    A {@link org.omg.dds.type.TypeSupport} representing the resulting
     *                  type.
     * @param   subscriptionExpression  The list of topics and the logic used
     *          to combine filter and re-arrange the information from each
     *          Topic.
     * @param   expressionParameters    Parameters to the filterExpression.
     */
    public <TYPE> MultiTopic<TYPE> createMultiTopic(
            String name,
            TypeSupport<TYPE> type,
            String subscriptionExpression,
            List<String> expressionParameters);

    /**
     * This operation deletes all the entities that were created by means of
     * the "create" operations on the DomainParticipant. That is, it deletes
     * all contained {@link org.omg.dds.pub.Publisher}, {@link org.omg.dds.sub.Subscriber}, {@link org.omg.dds.topic.Topic},
     * {@link org.omg.dds.topic.ContentFilteredTopic}, and {@link org.omg.dds.topic.MultiTopic} objects.
     * 
     * Prior to deleting each contained entity, this operation will
     * recursively call the corresponding closeContainedEntities operation on
     * each contained entity (if applicable). This pattern is applied
     * recursively. In this manner the operation closeContainedEntities on
     * the DomainParticipant will end up deleting all the entities
     * recursively contained in the DomainParticipant, that is also the
     * {@link org.omg.dds.pub.DataWriter}, {@link org.omg.dds.sub.DataReader}, as well as the
     * {@link org.omg.dds.sub.QueryCondition} and {@link org.omg.dds.sub.ReadCondition} objects belonging to
     * the contained DataReaders.
     * 
     * Once closeContainedEntities returns successfully, the application may
     * delete the DomainParticipant knowing that it has no contained
     * entities.
     * 
     * @throws  PreconditionNotMetException     if any of the contained
     *          entities is in a state where it cannot be closed.
     */
    public void closeContainedEntities();

    /**
     * This operation allows an application to instruct the Service to
     * locally ignore a remote domain participant. From that point onwards,
     * the Service will locally behave as if the remote participant did not
     * exist. This means it will ignore any {@link org.omg.dds.topic.Topic}, publication, or
     * subscription that originates on that domain participant.
     * 
     * This operation can be used, in conjunction with the discovery of
     * remote participants offered by means of the "DCPSParticipant" built-in
     * Topic, to provide, for example, access control. Application data can
     * be associated with a DomainParticipant by means of the
     * {@link org.omg.dds.core.policy.UserData}. This application data is propagated as a
     * field in the built-in topic and can be used by an application to
     * implement its own access control policy.
     * 
     * The domain participant to ignore is identified by the handle argument.
     * This handle is the one that appears in the {@link org.omg.dds.sub.Sample} retrieved
     * when reading the data samples available for the built-in DataReader to
     * the "DCPSParticipant" topic. The built-in {@link org.omg.dds.sub.DataReader} is read
     * with the same read/take operations used for any DataReader.
     * 
     * This operation is not required to be reversible. The Service offers no
     * means to reverse it.
     * 
     * @throws  OutOfResourcesException if the Service is unable to ignore
     *          the indicated participant because an internal resource has
     *          been exhausted.
     */
    public void ignoreParticipant(InstanceHandle handle);

    /**
     * This operation allows an application to instruct the Service to
     * locally ignore a {@link org.omg.dds.topic.Topic}. This means it will locally ignore any
     * publication or subscription to the Topic.
     * 
     * This operation can be used to save local resources when the
     * application knows that it will never publish or subscribe to data
     * under certain topics.
     * 
     * The Topic to ignore is identified by the handle argument. This handle
     * is the one that appears in the {@link org.omg.dds.sub.Sample} retrieved when reading
     * the data samples from the built-in {@link org.omg.dds.sub.DataReader} to the
     * "DCPSTopic" topic.
     * 
     * This operation is not required to be reversible. The Service offers no
     * means to reverse it.
     * 
     * @throws  OutOfResourcesException if the Service is unable to ignore
     *          the indicated topic because an internal resource has been
     *          exhausted.
     */
    public void ignoreTopic(InstanceHandle handle);

    /**
     * This operation allows an application to instruct the Service to
     * locally ignore a remote publication; a publication is defined by the
     * association of a topic name, and user data and partition set on the
     * {@link org.omg.dds.pub.Publisher}. After this call, any data written related to that
     * publication will be ignored.
     * 
     * The {@link org.omg.dds.pub.DataWriter} to ignore is identified by the handle argument.
     * This handle is the one that appears in the {@link org.omg.dds.sub.Sample} retrieved
     * when reading the data samples from the built-in {@link org.omg.dds.sub.DataReader} to
     * the "DCPSPublication" topic.
     * 
     * This operation is not required to be reversible. The Service offers no
     * means to reverse it.
     * 
     * @throws  OutOfResourcesException if the Service is unable to ignore
     *          the indicated publication because an internal resource has
     *          been exhausted.
     */
    public void ignorePublication(InstanceHandle handle);

    /**
     * This operation allows an application to instruct the Service to
     * locally ignore a remote subscription; a subscription is defined by the
     * association of a topic name, and user data and partition set on the
     * {@link org.omg.dds.sub.Subscriber}. After this call, any data received related to that
     * subscription will be ignored.
     * 
     * The {@link org.omg.dds.sub.DataReader} to ignore is identified by the handle argument.
     * This handle is the one that appears in the {@link org.omg.dds.sub.Sample} retrieved
     * when reading the data samples from the built-in DataReader to the
     * "DCPSSubscription" topic.
     * 
     * This operation is not required to be reversible. The Service offers no
     * means to reverse it.
     * 
     * @throws  OutOfResourcesException if the Service is unable to ignore
     *          the indicated subscription because an internal resource has
     *          been exhausted.
     */
    public void ignoreSubscription(InstanceHandle handle);

    /**
     * This operation retrieves the domain ID used to create the
     * DomainParticipant. The domain ID identifies the DDS domain to which
     * the DomainParticipant belongs. Each DDS domain represents a separate
     * data "communication plane" isolated from other domains.
     */
    public int getDomainId();

    /**
     * This operation manually asserts the liveliness of the
     * DomainParticipant. This is used in combination with the
     * {@link org.omg.dds.core.policy.Liveliness} to indicate to the
     * Service that the entity remains active.
     * 
     * This operation needs to only be used if the DomainParticipant contains
     * {@link org.omg.dds.pub.DataWriter} entities with the
     * {@link org.omg.dds.core.policy.Liveliness#getKind()} set to
     * {@link org.omg.dds.core.policy.Liveliness.Kind#MANUAL_BY_PARTICIPANT}
     * and it only
     * affects the liveliness of those DataWriter entities. Otherwise, it has
     * no effect.
     * 
     * <b>Note</b> - Writing data via the {@link org.omg.dds.pub.DataWriter#write(Object)}
     * operation on a DataWriter asserts liveliness on the DataWriter itself
     * and its DomainParticipant. Consequently the use of assertLiveliness is
     * only needed if the application is not writing data regularly.
     */
    public void assertLiveliness();

    /**
     * This operation retrieves the default value of the Publisher QoS, that
     * is, the QoS policies which will be used for newly created
     * {@link org.omg.dds.pub.Publisher} entities in the case where the QoS policies are
     * defaulted in the {@link #createPublisher()} operation.
     * 
     * The values retrieved will match the set of values specified on the
     * last successful call to {@link #setDefaultPublisherQos(PublisherQos)},
     * or else, if the call was never made, the default values identified by
     * the DDS specification.
     * 
     * @see     #setDefaultPublisherQos(PublisherQos)
     */
    public PublisherQos getDefaultPublisherQos();

    /**
     * This operation sets a default value of the Publisher QoS policies,
     * which will be used for newly created {@link org.omg.dds.pub.Publisher} entities in the
     * case where the QoS policies are defaulted in the
     * {@link #createPublisher()} operation.
     * 
     * @throws  InconsistentPolicyException if the resulting policies are
     *          not self consistent; in that case, the operation will have no
     *          effect.
     *
     * @see     #getDefaultPublisherQos()
     */
    public void setDefaultPublisherQos(PublisherQos qos);

    /**
     * This operation retrieves the default value of the Subscriber QoS, that
     * is, the QoS policies which will be used for newly created
     * {@link org.omg.dds.sub.Subscriber} entities in the case where the QoS policies are
     * defaulted in the {@link #createSubscriber()} operation.
     * 
     * The values retrieved will match the set of values specified on the
     * last successful call to
     * {@link #setDefaultSubscriberQos(SubscriberQos)}, or else, if the call
     * was never made, the default values identified by the DDS
     * specification.
     * 
     * @see     #setDefaultSubscriberQos(SubscriberQos)
     */
    public SubscriberQos getDefaultSubscriberQos();

    /**
     * This operation sets a default value of the Subscriber QoS policies
     * that will be used for newly created {@link org.omg.dds.sub.Subscriber} entities in the
     * case where the QoS policies are defaulted in the
     * {@link #createSubscriber()} operation.
     * 
     * @throws  InconsistentPolicyException     if the resulting policies are
     *          not self consistent; in that case, the operation will have no
     *          effect.
     *
     * @see     #getDefaultSubscriberQos()
     */
    public void setDefaultSubscriberQos(SubscriberQos qos);

    /**
     * This operation retrieves the default value of the Topic QoS, that is,
     * the QoS policies which will be used for newly created {@link org.omg.dds.topic.Topic}
     * entities in the case where the QoS policies are defaulted in the
     * {@link #createTopic(String, Class)} operation.
     * 
     * The values retrieved will match the set of values specified on the
     * last successful call to {@link #setDefaultTopicQos(TopicQos)}, or
     * else, if the call was never made, the default values identified by the
     * DDS specification.
     * 
     * @see     #setDefaultTopicQos(TopicQos)
     */
    public TopicQos getDefaultTopicQos();

    /**
     * This operation sets a default value of the Topic QoS policies, which
     * will be used for newly created {@link org.omg.dds.topic.Topic} entities in the case
     * where the QoS policies are defaulted in the
     * {@link #createTopic(String, Class)} operation.
     * 
     * @throws  InconsistentPolicyException     if the resulting policies are
     *          not self consistent; in that case, the operation will have no
     *          effect.
     *
     * @see     #getDefaultTopicQos()
     */
    public void setDefaultTopicQos(TopicQos qos);

    /**
     * This operation retrieves the list of DomainParticipants that have been
     * discovered in the domain and that the application has not indicated
     * should be "ignored" by means of the
     * {@link #ignoreParticipant(InstanceHandle)} operation.
     * 
     * @return  a new collection containing a copy of the information.
     * 
     * @throws  UnsupportedOperationException   If the infrastructure does
     *          not locally maintain the connectivity information.
     * 
     * @see     #getDiscoveredParticipantData(InstanceHandle)
     * @see     #getDiscoveredTopics()
     */
    public Set<InstanceHandle> getDiscoveredParticipants();

    /**
     * This operation retrieves information on a DomainParticipant that has
     * been discovered on the network. The participant must be in the same
     * domain as the participant on which this operation is invoked and must
     * not have been "ignored" by means of the
     * {@link #ignoreParticipant(InstanceHandle)} operation.
     * 
     * Use the operation {@link #getDiscoveredParticipants()} to
     * find the DomainParticipants that are currently discovered.
     * 
     * @param   participantHandle       a handle to the participant, the data
     *          of which is to be retrieved.
     * 
     * @return  a new object containing a copy of the information.
     * 
     * @throws  PreconditionNotMetException     if the participantHandle
     *          does not correspond to a DomainParticipant such as is
     *          described above.
     * @throws  UnsupportedOperationException   If the infrastructure does
     *          not locally maintain the connectivity information.
     * 
     * @see     #getDiscoveredParticipants()
     * @see     #getDiscoveredTopicData(InstanceHandle)
     */
    public ParticipantBuiltinTopicData getDiscoveredParticipantData(
            InstanceHandle participantHandle);

    /**
     * This operation retrieves the list of {@link org.omg.dds.topic.Topic}s that have been
     * discovered in the domain and that the application has not indicated
     * should be "ignored" by means of the
     * {@link #ignoreTopic(InstanceHandle)} operation.
     * 
     * @return  a new collection containing a copy of the information.
     * 
     * @throws  UnsupportedOperationException   If the infrastructure does
     *          not locally maintain the connectivity information.
     * 
     * @see     #getDiscoveredTopicData(InstanceHandle)
     * @see     #getDiscoveredParticipants()
     */
    public Set<InstanceHandle> getDiscoveredTopics();

    /**
     * This operation retrieves information on a {@link org.omg.dds.topic.Topic} that has
     * been discovered on the network. The topic must be in the same
     * domain as the participant on which this operation is invoked and must
     * not have been "ignored" by means of the
     * {@link #ignoreTopic(InstanceHandle)} operation.
     * 
     * Use the operation {@link #getDiscoveredTopics()} to
     * find the Topics that are currently discovered.
     * 
     * @param   topicHandle     a handle to the topic, the data
     *          of which is to be retrieved.
     * 
     * @return  a new object containing a copy of the information.
     * 
     * @throws  PreconditionNotMetException     if the topicHandle
     *          does not correspond to a Topic such as is described above.
     * @throws  UnsupportedOperationException   If the infrastructure does
     *          not locally maintain the connectivity information or if the
     *          infrastructure does not hold the information necessary to
     *          fill in the topicData.
     * 
     * @see     #getDiscoveredTopics()
     * @see     #getDiscoveredParticipantData(InstanceHandle)
     */
    public TopicBuiltinTopicData getDiscoveredTopicData(
            InstanceHandle topicHandle);

    /**
     * This operation checks whether or not the given handle represents an
     * {@link org.omg.dds.core.Entity} that was created from the DomainParticipant. The
     * containment applies recursively. That is, it applies both to entities
     * ({@link org.omg.dds.topic.TopicDescription}, {@link org.omg.dds.pub.Publisher}, or {@link org.omg.dds.sub.Subscriber})
     * created directly using the DomainParticipant as well as entities
     * created using a contained Publisher or Subscriber as the factory, and
     * so forth.
     * 
     * The instance handle for an Entity may be obtained from built-in topic
     * data, from various statuses, or from the Entity operation
     * {@link org.omg.dds.core.Entity#getInstanceHandle()}.
     */
    public boolean containsEntity(InstanceHandle handle);

    /**
     * This operation returns the current value of the time that the service
     * uses to time stamp data writes and to set the reception time stamp for
     * the data updates it receives.
     * 
     * @param   currentTime     a container for the current time, which the
     *          Service will overwrite with the result of this operation, or
     *          null, if the Service should store the current time in a new
     *          object.
     * @return  currentTime, if it is non-null, or a new object otherwise.
     * 
     * @see     #getCurrentTime()
     */
    public ModifiableTime getCurrentTime(ModifiableTime currentTime);

    /**
     * This operation returns the current value of the time that the service
     * uses to time stamp data writes and to set the reception time stamp for
     * the data updates it receives.
     * 
     * @return  an immutable object containing the current time.
     * 
     * @see     #getCurrentTime(ModifiableTime)
     */
    public Time getCurrentTime();


    // --- From Entity: ------------------------------------------------------

    public StatusCondition<DomainParticipant> getStatusCondition();
}
