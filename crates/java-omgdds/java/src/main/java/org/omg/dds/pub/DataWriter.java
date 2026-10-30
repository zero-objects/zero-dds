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

package org.omg.dds.pub;

import java.util.Set;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.TimeoutException;

import org.omg.dds.core.DomainEntity;
import org.omg.dds.core.Duration;
import org.omg.dds.core.InstanceHandle;
import org.omg.dds.core.ModifiableInstanceHandle;
import org.omg.dds.core.StatusCondition;
import org.omg.dds.core.Time;
import org.omg.dds.core.status.LivelinessLostStatus;
import org.omg.dds.core.status.OfferedDeadlineMissedStatus;
import org.omg.dds.core.status.OfferedIncompatibleQosStatus;
import org.omg.dds.core.status.PublicationMatchedStatus;
import org.omg.dds.topic.SubscriptionBuiltinTopicData;
import org.omg.dds.topic.Topic;


/**
 * DataWriter allows the application to set the value of the data to be
 * published under a given {@link org.omg.dds.topic.Topic}.
 * 
 * A DataWriter is attached to exactly one {@link org.omg.dds.pub.Publisher} that acts as a
 * factory for it. A DataWriter is bound to exactly one Topic and therefore
 * to exactly one data type. The Topic must exist prior to the DataWriter's
 * creation.
 * 
 * All operations except for the inherited operations
 * {@link #setQos(org.omg.dds.core.EntityQos)}, {@link #getQos()},
 * {@link #setListener(java.util.EventListener)},{@link #getListener()},
 * {@link #enable()}, {@link #getStatusCondition()}, and {@link #close()} may
 * fail with the exception {@link org.omg.dds.core.NotEnabledException}.
 * 
 * @param <TYPE>    The concrete type of the data to be published over the
 *                  the topic.
 */
public interface DataWriter<TYPE>
extends DomainEntity<DataWriterListener<TYPE>, DataWriterQos>
{
    /**
     * Cast this data writer to the given type, or throw an exception if
     * the cast fails.
     * 
     * @param <OTHER>   The type of the data published by this writer,
     *                  according to the caller.
     * @return          this data writer
     * @throws          ClassCastException if the cast fails
     */
    public <OTHER> DataWriter<OTHER> cast();

    /**
     * @return  the {@link org.omg.dds.topic.Topic} associated with the DataWriter. This is the
     *          same Topic that was used to create the DataWriter.
     */
    public Topic<TYPE> getTopic();

    /**
     * This operation is intended to be used only if the DataWriter has
     * {@link org.omg.dds.core.policy.Reliability#getKind()} set to
     * {@link org.omg.dds.core.policy.Reliability.Kind#RELIABLE}.
     * Otherwise the operation will return immediately.
     * 
     * The operation blocks the calling thread until either all data written
     * by the DataWriter is acknowledged by all matched {@link org.omg.dds.sub.DataReader}
     * entities that have
     * {@link org.omg.dds.core.policy.Reliability#getKind()} set to
     * {@link org.omg.dds.core.policy.Reliability.Kind#RELIABLE},
     * or else the duration
     * specified by the maxWait parameter elapses, whichever happens first.
     * 
     * A normal return indicates that all the samples written have been
     * acknowledged by all reliable matched data readers.
     * 
     *  @throws TimeoutException    if maxWait elapsed before all the data
     *          was acknowledged.
     */
    public void waitForAcknowledgments(Duration maxWait)
    throws TimeoutException;

    /**
     * This operation is intended to be used only if the DataWriter has
     * {@link org.omg.dds.core.policy.Reliability#getKind()} set to
     * {@link org.omg.dds.core.policy.Reliability.Kind#RELIABLE}.
     * Otherwise the operation will return immediately.
     * 
     * The operation blocks the calling thread until either all data written
     * by the DataWriter is acknowledged by all matched {@link org.omg.dds.sub.DataReader}
     * entities that have
     * {@link org.omg.dds.core.policy.Reliability#getKind()} set to
     * {@link org.omg.dds.core.policy.Reliability.Kind#RELIABLE},
     * or else the duration
     * specified by the maxWait parameter elapses, whichever happens first.
     * 
     * A normal return indicates that all the samples written have been
     * acknowledged by all reliable matched data readers.
     * 
     *  @throws TimeoutException    if maxWait elapsed before all the data
     *          was acknowledged.
     */
    public void waitForAcknowledgments(long maxWait, TimeUnit unit)
    throws TimeoutException;

    /**
     * This operation allows access to the LIVELINESS_LOST communication
     * status.
     * 
     * @see     org.omg.dds.core.status
     */
    public LivelinessLostStatus getLivelinessLostStatus();

    /**
     * This operation allows access to the OFFERED_DEADLINE_MISSED
     * communication status.
     * 
     * @see     org.omg.dds.core.status
     */
    public OfferedDeadlineMissedStatus getOfferedDeadlineMissedStatus();

    /**
     * This operation allows access to the OFFERED_INCOMPATIBLE_QOS
     * communication status.
     * 
     * @see     org.omg.dds.core.status
     */
    public OfferedIncompatibleQosStatus getOfferedIncompatibleQosStatus();

    /**
     * This operation allows access to the PUBLICATION_MATCHED
     * communication status.
     * 
     * @see     org.omg.dds.core.status
     */
    public PublicationMatchedStatus getPublicationMatchedStatus();

    /**
     * This operation manually asserts the liveliness of the DataWriter. This
     * is used in combination with the
     * {@link org.omg.dds.core.policy.Liveliness} to
     * indicate to the Service that the entity remains active.
     * 
     * This operation need only be used if
     * {@link org.omg.dds.core.policy.Liveliness#getKind()} is either
     * {@link org.omg.dds.core.policy.Liveliness.Kind#MANUAL_BY_PARTICIPANT}
     * or
     * {@link org.omg.dds.core.policy.Liveliness.Kind#MANUAL_BY_TOPIC}.
     * Otherwise, it has no effect.
     * 
     * <b>Note</b> - Writing data via {@link #write(Object)} asserts
     * liveliness on the DataWriter itself and its DomainParticipant.
     * Consequently the use of assertLiveliness is only needed if the
     * application is not writing data regularly.
     */
    public void assertLiveliness();

    /**
     * This operation retrieves the list of subscriptions currently
     * "associated" with the DataWriter; that is, subscriptions that have a
     * matching {@link org.omg.dds.topic.Topic} and compatible QoS that the application has not
     * indicated should be "ignored" by means of
     * {@link org.omg.dds.domain.DomainParticipant#ignoreSubscription(InstanceHandle)}.
     * 
     * The handles returned in the 'subscriptionHandles' list are the ones
     * that are used by the DDS implementation to locally identify the
     * corresponding matched {@link org.omg.dds.sub.DataReader} entities. These handles match
     * the ones that appear in {@link org.omg.dds.sub.Sample#getInstanceHandle()} when
     * reading the "DCPSSubscriptions" built-in topic.
     * 
     * The operation may fail if the infrastructure does not locally maintain
     * the connectivity information.
     * 
     * @return  a new collection containing a copy of the information.
     * 
     * @see     #getMatchedSubscriptionData(InstanceHandle)
     */
    public Set<InstanceHandle> getMatchedSubscriptions();

    /**
     * This operation retrieves information on a subscription that is
     * currently "associated" with the DataWriter; that is, a subscription
     * with a matching {@link org.omg.dds.topic.Topic} and compatible QoS that the application
     * has not indicated should be "ignored" by means of
     * {@link org.omg.dds.domain.DomainParticipant#ignoreSubscription(InstanceHandle)}.
     * 
     * The operation {@link #getMatchedSubscriptions()} can be used
     * to find the subscriptions that are currently matched with the
     * DataWriter.
     * 
     * @param   subscriptionHandle      a handle to the subscription, the
     *          data of which is to be retrieved.
     * 
     * @return  a new object containing a copy of the information.
     * 
     * @throws  IllegalArgumentException        if subscriptionHandle does
     *          not correspond to a subscription currently associated with
     *          the DataWriter.
     * @throws  UnsupportedOperationException   if the infrastructure does
     *          not hold the information necessary to fill in the
     *          subscriptionData.
     *
     * @see     #getMatchedSubscriptions()
     */
    public SubscriptionBuiltinTopicData getMatchedSubscriptionData(
            InstanceHandle subscriptionHandle);


    // --- Type-specific interface: ------------------------------------------

    /**
     * This operation informs the Service that the application will be
     * modifying a particular instance. It gives an opportunity to the
     * Service to pre-configure itself to improve performance.
     * 
     * It takes as a parameter an instance (to get the key value) and returns
     * a handle that can be used in successive {@link #write(Object)} or
     * {@link #dispose(InstanceHandle)} operations.
     * 
     * This operation should be invoked prior to calling any operation that
     * modifies the instance.
     * 
     * A nil handle may be returned by the Service if it does not want to
     * allocate any handle for that instance.
     * 
     * This operation may block and exit with {@link TimeoutException} under
     * the same circumstances described for the {@link #write(Object)}.
     * 
     * The operation is idempotent. If it is called for an already registered
     * instance, it just returns the already allocated handle. This may be
     * used to lookup and retrieve the handle allocated to a given instance.
     * The explicit use of this operation is optional as the application may
     * call directly the write operation and specify a nil handle to indicate
     * that the 'key' should be examined to identify the instance.
     * 
     * @throws  OutOfResourcesException under the same circumstances
     *          described for {@link #write(Object)}.
     * @throws  TimeoutException        under the same circumstances
     *          described for {@link #write(Object)}.
     * 
     * @see     #registerInstance(Object, Time)
     * @see     #registerInstance(Object, long, TimeUnit)
     * @see     #unregisterInstance(InstanceHandle)
     * @see     #unregisterInstance(InstanceHandle, Object)
     * @see     InstanceHandle#nilHandle(org.omg.dds.core.ServiceEnvironment)
     */
    public InstanceHandle registerInstance(
            TYPE instanceData) throws TimeoutException;

    /**
     * This operation performs the same function as
     * {@link #registerInstance(Object)} and can be used instead in the cases
     * where the application desires to specify the value for the source
     * time stamp. The source time stamp potentially affects the relative
     * order in which readers observe events from multiple writers. For
     * details see {@link org.omg.dds.core.policy.DestinationOrder}.
     * 
     * This operation may block and exit with {@link TimeoutException} under
     * the same circumstances described for the {@link #write(Object)}.
     * 
     * @throws  OutOfResourcesException under the same circumstances
     *          described for {@link #write(Object)}.
     * @throws  TimeoutException        under the same circumstances
     *          described for {@link #write(Object)}.
     * 
     * @see     #registerInstance(Object)
     * @see     #registerInstance(Object, long, TimeUnit)
     * @see     #unregisterInstance(InstanceHandle, Object, Time)
     */
    public InstanceHandle registerInstance(
            TYPE instanceData, 
            Time sourceTimestamp) throws TimeoutException;

    /**
     * This operation performs the same function as
     * {@link #registerInstance(Object)} and can be used instead in the cases
     * where the application desires to specify the value for the source
     * time stamp. The source time stamp potentially affects the relative
     * order in which readers observe events from multiple writers. For
     * details see {@link org.omg.dds.core.policy.DestinationOrder}.
     * 
     * This operation may block and exit with {@link TimeoutException} under
     * the same circumstances described for the {@link #write(Object)}.
     * 
     * @throws  OutOfResourcesException under the same circumstances
     *          described for {@link #write(Object)}.
     * @throws  TimeoutException        under the same circumstances
     *          described for {@link #write(Object)}.
     * 
     * @see     #registerInstance(Object)
     * @see     #registerInstance(Object, Time)
     * @see     #unregisterInstance(InstanceHandle, Object, long, TimeUnit)
     */
    public InstanceHandle registerInstance(
            TYPE instanceData, 
            long sourceTimestamp,
            TimeUnit unit) throws TimeoutException;

    /**
     * This operation reverses the action of
     * {@link #registerInstance(Object)}. It should only be called on an
     * instance that is currently registered.
     * 
     * The operation should be called just once per instance, regardless of
     * how many times {@link #registerInstance(Object)} was called for that
     * instance.
     * 
     * This operation informs the Service that the DataWriter is not
     * intending to modify that data instance any more. This operation also
     * indicates that the Service can locally remove all information
     * regarding that instance. The application should not attempt to use the
     * handle previously allocated to that instance after calling
     * {@link #unregisterInstance(InstanceHandle)}.
     * 
     * If handle is any value other than nil, then it must correspond to the
     * value returned by {@link #registerInstance(Object)} when the instance
     * (identified by its key) was registered.
     * 
     * If after that, the application wants to modify (write or dispose) the
     * instance, it has to register it again, or else use a nil handle.
     * 
     * This operation does not indicate that the instance is deleted (that is
     * the purpose of dispose). The operation just indicates that the
     * DataWriter no longer has 'anything to say' about the instance.
     * DataReader entities that are reading the instance will eventually
     * receive a sample with a {@link org.omg.dds.sub.InstanceState#NOT_ALIVE_NO_WRITERS}
     * instance state if no other DataWriter entities are writing the
     * instance.
     * 
     * This operation can affect the ownership of the data instance (see
     * {@link org.omg.dds.core.policy.Ownership}). If the DataWriter was the exclusive owner
     * of the instance, then calling this method will relinquish that
     * ownership.
     * 
     * This operation may block and exit with {@link TimeoutException} under
     * the same circumstances described for the {@link #write(Object)}.
     * 
     * @throws  IllegalArgumentException    if the handle does not correspond
     *          to an existing instance, and if this situation is detectable
     *          by the Service implementation. If the situation is not
     *          detectable, the behavior is unspecified.
     *
     * @see     #unregisterInstance(InstanceHandle, Object)
     * @see     #unregisterInstance(InstanceHandle, Object, Time)
     * @see     #unregisterInstance(InstanceHandle, Object, long, TimeUnit)
     * @see     #registerInstance(Object)
     */
    public void unregisterInstance(
            InstanceHandle handle) throws TimeoutException;

    /**
     * This operation reverses the action of
     * {@link #registerInstance(Object)}. It should only be called on an
     * instance that is currently registered.
     * 
     * The operation should be called just once per instance, regardless of
     * how many times {@link #registerInstance(Object)} was called for that
     * instance.
     * 
     * This operation informs the Service that the DataWriter is not
     * intending to modify that data instance any more. This operation also
     * indicates that the Service can locally remove all information
     * regarding that instance. The application should not attempt to use the
     * handle previously allocated to that instance after calling
     * {@link #unregisterInstance(InstanceHandle)}.
     * 
     * A nil handle can be used for the parameter handle. This indicates that
     * the identity of the instance should be automatically deduced from the
     * instance data (by means of the key).
     * 
     * If handle is any value other than nil, then it must correspond to the
     * value returned by {@link #registerInstance(Object)} when the instance
     * (identified by its key) was registered.
     * 
     * If after that, the application wants to modify (write or dispose) the
     * instance, it has to register it again, or else use a nil handle.
     * 
     * This operation does not indicate that the instance is deleted (that is
     * the purpose of dispose). The operation just indicates that the
     * DataWriter no longer has 'anything to say' about the instance.
     * DataReader entities that are reading the instance will eventually
     * receive a sample with a {@link org.omg.dds.sub.InstanceState#NOT_ALIVE_NO_WRITERS}
     * instance state if no other DataWriter entities are writing the
     * instance.
     * 
     * This operation can affect the ownership of the data instance (see
     * {@link org.omg.dds.core.policy.Ownership}). If the DataWriter was the exclusive owner
     * of the instance, then calling this method will relinquish that
     * ownership.
     * 
     * This operation may block and exit with {@link TimeoutException} under
     * the same circumstances described for the {@link #write(Object)}.
     * 
     * @throws  IllegalArgumentException    if the handle does not correspond
     *          to an existing instance, and if this situation is detectable
     *          by the Service implementation. If the situation is not
     *          detectable, the behavior is unspecified.
     * @throws  PreconditionNotMetException if the handle corresponds to an
     *          existing instance but does not correspond to the same
     *          instance referred by the instancData parameter, and if this
     *          situation is detectable by the Service implementation If the
     *          situation is not detectable, the behavior is unspecified.
     * 
     * @see     #unregisterInstance(InstanceHandle)
     * @see     #unregisterInstance(InstanceHandle, Object, Time)
     * @see     #unregisterInstance(InstanceHandle, Object, long, TimeUnit)
     * @see     #registerInstance(Object)
     * @see     InstanceHandle#nilHandle(org.omg.dds.core.ServiceEnvironment)
     */
    public void unregisterInstance(
            InstanceHandle handle, 
            TYPE instanceData) throws TimeoutException;

    /**
     * This operation performs the same function as
     * {@link #unregisterInstance(InstanceHandle, Object)} and can be used
     * instead in the cases where the application desires to specify the
     * value for the source time stamp. The source time stamp potentially
     * affects the relative order in which readers observe events from
     * multiple writers. For details see {@link org.omg.dds.core.policy.DestinationOrder}.
     * 
     * The constraints on the values of the handle parameter and the
     * corresponding error behavior are the same specified for the
     * {@link #unregisterInstance(InstanceHandle, Object)} operation.
     * 
     * This operation may block and exit with {@link TimeoutException} under
     * the same circumstances described for the {@link #write(Object)}.
     * 
     * @see     #unregisterInstance(InstanceHandle)
     * @see     #unregisterInstance(InstanceHandle, Object)
     * @see     #unregisterInstance(InstanceHandle, Object, long, TimeUnit)
     * @see     #registerInstance(Object, Time)
     */
    public void unregisterInstance(
            InstanceHandle handle, 
            TYPE instanceData,
            Time sourceTimestamp) throws TimeoutException;

    /**
     * This operation performs the same function as
     * {@link #unregisterInstance(InstanceHandle, Object)} and can be used
     * instead in the cases where the application desires to specify the
     * value for the source time stamp. The source time stamp potentially
     * affects the relative order in which readers observe events from
     * multiple writers. For details see {@link org.omg.dds.core.policy.DestinationOrder}.
     * 
     * The constraints on the values of the handle parameter and the
     * corresponding error behavior are the same specified for the
     * {@link #unregisterInstance(InstanceHandle, Object)} operation.
     * 
     * This operation may block and exit with {@link TimeoutException} under
     * the same circumstances described for the {@link #write(Object)}.
     * 
     * @see     #unregisterInstance(InstanceHandle)
     * @see     #unregisterInstance(InstanceHandle, Object)
     * @see     #unregisterInstance(InstanceHandle, Object, Time)
     * @see     #registerInstance(Object, long, TimeUnit)
     */
    public void unregisterInstance(
            InstanceHandle handle, 
            TYPE instanceData,
            long sourceTimestamp,
            TimeUnit unit) throws TimeoutException;

    /**
     * This operation modifies the value of a data instance. When this
     * operation is used, the Service will automatically supply the value of
     * the source time stamp that is made available to {@link org.omg.dds.sub.DataReader}
     * objects by means of {@link org.omg.dds.sub.Sample#getSourceTimestamp()}. See also
     * {@link org.omg.dds.core.policy.DestinationOrder}.
     * 
     * As a side effect, this operation asserts liveliness on the DataWriter
     * itself, the {@link org.omg.dds.pub.Publisher} and the {@link org.omg.dds.domain.DomainParticipant}.
     * 
     * If {@link org.omg.dds.core.policy.Reliability#getKind()} kind
     * is set to
     * {@link org.omg.dds.core.policy.Reliability.Kind#RELIABLE},
     * the operation may block if
     * the modification would cause data to be lost or else cause one of the
     * limits specified in {@link org.omg.dds.core.policy.ResourceLimits} to be exceeded.
     * Under these circumstances,
     * {@link org.omg.dds.core.policy.Reliability#getMaxBlockingTime()}
     * configures the
     * maximum time the operation may block waiting for space to become
     * available. If this duration elapses before the DataWriter is able to
     * store the modification without exceeding the limits, the operation
     * will fail with {@link TimeoutException}.
     * 
     * Specifically, the DataWriter write operation may block in the
     * following situations (note that the list may not be exhaustive), even
     * if {@link org.omg.dds.core.policy.History#getKind()} is
     * {@link org.omg.dds.core.policy.History.Kind#KEEP_LAST}.
     * 
     * <ul>
     *     <li>If ({@link org.omg.dds.core.policy.ResourceLimits#getMaxSamples()} &lt;
     *         {@link org.omg.dds.core.policy.ResourceLimits#getMaxInstances()} *
     *         {@link org.omg.dds.core.policy.History#getDepth()}),
     *         then in the situation
     *         where the max samples resource limit is exhausted the Service
     *         is allowed to discard samples of some other instance as long
     *         as at least one sample remains for such an instance. If it is
     *         still not possible to make space available to store the
     *         modification, the writer is allowed to block.</li>
     *     <li>If ({@link org.omg.dds.core.policy.ResourceLimits#getMaxSamples()} &lt;
     *         {@link org.omg.dds.core.policy.ResourceLimits#getMaxInstances()}), then the
     *         DataWriter may block regardless of the HISTORY depth.</li>
     * </ul>
     * 
     * Instead of blocking, the operation is allowed to fail immediately
     * with {@link org.omg.dds.core.OutOfResourcesException} provided the following two
     * conditions are met:
     * 
     * <ol>
     *     <li>The reason for blocking would be that the RESOURCE_LIMITS are
     *         exceeded.</li>
     *     <li>The service determines that waiting the max blocking time has
     *         no chance of freeing the necessary resources. For example, if
     *         the only way to gain the necessary resources would be for the
     *         user to unregister an instance.</li>
     * </ol>
     * 
     * @throws  OutOfResourcesException     if it is not possible for
     *          sufficient resources to be made available within the
     *          configured max blocking time.
     * @throws  TimeoutException            if the configured maximum
     *          time elapses and the DataWriter is still unable to store the
     *          new sample without exceeding its configured resource limits.
     * 
     * @see     #write(Object, InstanceHandle)
     * @see     #write(Object, InstanceHandle, Time)
     * @see     #write(Object, InstanceHandle, long, TimeUnit)
     */
    public void write(
            TYPE instanceData) throws TimeoutException;
    public void write(
            TYPE instanceData, 
            Time sourceTimestamp) throws TimeoutException;
    public void write(
            TYPE instanceData, 
            long sourceTimestamp,
            TimeUnit unit) throws TimeoutException;

    /**
     * This operation modifies the value of a data instance. When this
     * operation is used, the Service will automatically supply the value of
     * the source time stamp that is made available to {@link org.omg.dds.sub.DataReader}
     * objects by means of {@link org.omg.dds.sub.Sample#getSourceTimestamp()}. See also
     * {@link org.omg.dds.core.policy.DestinationOrder}.
     * 
     * As a side effect, this operation asserts liveliness on the DataWriter
     * itself, the {@link org.omg.dds.pub.Publisher} and the {@link org.omg.dds.domain.DomainParticipant}.
     * 
     * A nil handle can be used for the parameter handle. This indicates that
     * the identity of the instance should be automatically deduced from the
     * instanceData (by means of the key). If handle is not nil, then it must
     * correspond to the value returned by {@link #registerInstance(Object)}
     * when the instance (identified by its key) was registered.
     * 
     * If {@link org.omg.dds.core.policy.Reliability#getKind()} kind
     * is set to
     * {@link org.omg.dds.core.policy.Reliability.Kind#RELIABLE},
     * the operation may block if
     * the modification would cause data to be lost or else cause one of the
     * limits specified in {@link org.omg.dds.core.policy.ResourceLimits} to be exceeded.
     * Under these circumstances,
     * {@link org.omg.dds.core.policy.Reliability#getMaxBlockingTime()}
     * configures the
     * maximum time the operation may block waiting for space to become
     * available. If this duration elapses before the DataWriter is able to
     * store the modification without exceeding the limits, the operation
     * will fail with {@link TimeoutException}.
     * 
     * Specifically, the DataWriter write operation may block in the
     * following situations (note that the list may not be exhaustive), even
     * if {@link org.omg.dds.core.policy.History#getKind()} is
     * {@link org.omg.dds.core.policy.History.Kind#KEEP_LAST}.
     * 
     * <ul>
     *     <li>If ({@link org.omg.dds.core.policy.ResourceLimits#getMaxSamples()} &lt;
     *         {@link org.omg.dds.core.policy.ResourceLimits#getMaxInstances()} *
     *         {@link org.omg.dds.core.policy.History#getDepth()}),
     *         then in the situation
     *         where the max samples resource limit is exhausted the Service
     *         is allowed to discard samples of some other instance as long
     *         as at least one sample remains for such an instance. If it is
     *         still not possible to make space available to store the
     *         modification, the writer is allowed to block.</li>
     *     <li>If ({@link org.omg.dds.core.policy.ResourceLimits#getMaxSamples()} &lt;
     *         {@link org.omg.dds.core.policy.ResourceLimits#getMaxInstances()}), then the
     *         DataWriter may block regardless of the HISTORY depth.</li>
     * </ul>
     * 
     * Instead of blocking, the operation is allowed to fail immediately
     * with {@link org.omg.dds.core.OutOfResourcesException} provided the following two
     * conditions are met:
     * 
     * <ol>
     *     <li>The reason for blocking would be that the RESOURCE_LIMITS are
     *         exceeded.</li>
     *     <li>The service determines that waiting the max blocking time has
     *         no chance of freeing the necessary resources. For example, if
     *         the only way to gain the necessary resources would be for the
     *         user to unregister an instance.</li>
     * </ol>
     * 
     * @throws  IllegalArgumentException    if the handle does not correspond
     *          to an existing instance, and if this situation is detectable
     *          by the Service implementation. If the situation is not
     *          detectable, the behavior is unspecified.
     * @throws  PreconditionNotMetException if the handle corresponds to an
     *          existing instance but does not correspond to the same
     *          instance referred by the instancData parameter, and if this
     *          situation is detectable by the Service implementation If the
     *          situation is not detectable, the behavior is unspecified.
     * @throws  OutOfResourcesException     if it is not possible for
     *          sufficient resources to be made available within the
     *          configured max blocking time.
     * @throws  TimeoutException            if the configured maximum
     *          time elapses and the DataWriter is still unable to store the
     *          new sample without exceeding its configured resource limits.
     * 
     * @see     #write(Object)
     * @see     #write(Object, InstanceHandle, Time)
     * @see     #write(Object, InstanceHandle, long, TimeUnit)
     * @see     InstanceHandle#nilHandle(org.omg.dds.core.ServiceEnvironment)
     */
    public void write(
            TYPE instanceData, 
            InstanceHandle handle) throws TimeoutException;

    /**
     * This operation performs the same function as
     * {@link #write(Object, InstanceHandle)} except that it also provides
     * the value for the source time stamp that is made available to
     * {@link org.omg.dds.sub.DataReader} objects by means of
     * {@link org.omg.dds.sub.Sample#getSourceTimestamp()}. See also
     * {@link org.omg.dds.core.policy.DestinationOrder}.
     * 
     * The constraints on the values of the handle parameter and the
     * corresponding error behavior are the same specified for
     * {@link #write(Object, InstanceHandle)}.
     * 
     * This operation may block and exit with {@link TimeoutException} under
     * the same circumstances described for
     * {@link #write(Object, InstanceHandle)}.
     * 
     * @throws  IllegalArgumentException    under the same circumstances as
     *          {@link #write(Object, InstanceHandle)}.
     * @throws  PreconditionNotMetException under the same circumstances as
     *          {@link #write(Object, InstanceHandle)}.
     * @throws  OutOfResourcesException     under the same circumstances as
     *          {@link #write(Object, InstanceHandle)}.
     * @throws  TimeoutException            under the same circumstances as
     *          {@link #write(Object, InstanceHandle)}.
     * 
     * @see     #write(Object)
     * @see     #write(Object, InstanceHandle)
     * @see     #write(Object, InstanceHandle, long, TimeUnit)
     */
    public void write(
            TYPE instanceData, 
            InstanceHandle handle,
            Time sourceTimestamp) throws TimeoutException;

    /**
     * This operation performs the same function as
     * {@link #write(Object, InstanceHandle)} except that it also provides
     * the value for the source time stamp that is made available to
     * {@link org.omg.dds.sub.DataReader} objects by means of
     * {@link org.omg.dds.sub.Sample#getSourceTimestamp()}. See also
     * {@link org.omg.dds.core.policy.DestinationOrder}.
     * 
     * The constraints on the values of the handle parameter and the
     * corresponding error behavior are the same specified for
     * {@link #write(Object, InstanceHandle)}.
     * 
     * This operation may block and exit with {@link TimeoutException} under
     * the same circumstances described for
     * {@link #write(Object, InstanceHandle)}.
     * 
     * @throws  IllegalArgumentException    under the same circumstances as
     *          {@link #write(Object, InstanceHandle)}.
     * @throws  PreconditionNotMetException under the same circumstances as
     *          {@link #write(Object, InstanceHandle)}.
     * @throws  OutOfResourcesException     under the same circumstances as
     *          {@link #write(Object, InstanceHandle)}.
     * @throws  TimeoutException            under the same circumstances as
     *          {@link #write(Object, InstanceHandle)}.
     * 
     * @see     #write(Object)
     * @see     #write(Object, InstanceHandle)
     * @see     #write(Object, InstanceHandle, Time)
     */
    public void write(
            TYPE instanceData, 
            InstanceHandle handle,
            long sourceTimestamp,
            TimeUnit unit) throws TimeoutException;

    /**
     * This operation requests the middleware to delete the data (the actual
     * deletion is postponed until there is no more use for that data in the
     * whole system). In general, applications are made aware of the deletion
     * by means of operations on the {@link org.omg.dds.sub.DataReader} objects that already
     * knew that instance. DataReader objects that didn't know the instance
     * will never see it.
     * 
     * When this operation is used, the Service will automatically supply the
     * value of the source time stamp that is made available to DataReader
     * objects by means of {@link org.omg.dds.sub.Sample#getSourceTimestamp()}.
     * 
     * The constraints on the values of the instanceHandle parameter and the
     * corresponding error behavior are the same specified for
     * {@link #unregisterInstance(InstanceHandle)}.
     * 
     * This operation may block and exit with {@link TimeoutException} under
     * the same circumstances described for {@link #write(Object)}.
     * 
     * @throws  OutOfResourcesException     under the same circumstances as
     *          {@link #write(Object)}.
     * @throws  TimeoutException            under the same circumstances as
     *          {@link #write(Object)}.
     *
     * @see     #dispose(InstanceHandle, Object)
     * @see     #dispose(InstanceHandle, Object, Time)
     * @see     #dispose(InstanceHandle, Object, long, TimeUnit)
     */
    public void dispose(
            InstanceHandle instanceHandle) throws TimeoutException;

    /**
     * This operation requests the middleware to delete the data (the actual
     * deletion is postponed until there is no more use for that data in the
     * whole system). In general, applications are made aware of the deletion
     * by means of operations on the {@link org.omg.dds.sub.DataReader} objects that already
     * knew that instance. DataReader objects that didn't know the instance
     * will never see it.
     * 
     * This operation does not modify the value of the instance. The
     * instanceData parameter is passed just for the purposes of identifying
     * the instance.
     * 
     * When this operation is used, the Service will automatically supply the
     * value of the source time stamp that is made available to DataReader
     * objects by means of {@link org.omg.dds.sub.Sample#getSourceTimestamp()}.
     * 
     * The constraints on the values of the instanceHandle parameter and the
     * corresponding error behavior are the same specified for
     * {@link #unregisterInstance(InstanceHandle, Object)}.
     * 
     * This operation may block and exit with {@link TimeoutException} under
     * the same circumstances described for
     * {@link #write(Object, InstanceHandle)}.
     * 
     * @throws  OutOfResourcesException     under the same circumstances as
     *          {@link #write(Object, InstanceHandle)}.
     * @throws  TimeoutException            under the same circumstances as
     *          {@link #write(Object, InstanceHandle)}.
     *
     * @see     #dispose(InstanceHandle)
     * @see     #dispose(InstanceHandle, Object, Time)
     * @see     #dispose(InstanceHandle, Object, long, TimeUnit)
     */
    public void dispose(
            InstanceHandle instanceHandle, 
            TYPE instanceData) throws TimeoutException;

    /**
     * This operation performs the same functions as
     * {@link #dispose(InstanceHandle, Object)} except that the application
     * provides the value for the source time stamp that is made available to
     * {@link org.omg.dds.sub.DataReader} objects by means of
     * {@link org.omg.dds.sub.Sample#getSourceTimestamp()}.
     * 
     * The constraints on the values of the instanceHandle parameter and the
     * corresponding error behavior are the same specified for
     * {@link #dispose(InstanceHandle, Object)}.
     * 
     * This operation may block and exit with {@link TimeoutException} under
     * the same circumstances described for
     * {@link #write(Object, InstanceHandle)}.
     * 
     * @throws  IllegalArgumentException    under the same circumstances as
     *          {@link #dispose(InstanceHandle, Object)}.
     * @throws  PreconditionNotMetException under the same circumstances as
     *          {@link #dispose(InstanceHandle, Object)}.
     * @throws  OutOfResourcesException     under the same circumstances as
     *          {@link #write(Object, InstanceHandle)}.
     * @throws  TimeoutException            under the same circumstances as
     *          {@link #write(Object, InstanceHandle)}.
     * 
     * @see     #dispose(InstanceHandle)
     * @see     #dispose(InstanceHandle, Object)
     * @see     #dispose(InstanceHandle, Object, long, TimeUnit)
     */
    public void dispose(
            InstanceHandle instanceHandle, 
            TYPE instanceData,
            Time sourceTimestamp) throws TimeoutException;

    /**
     * This operation performs the same functions as
     * {@link #dispose(InstanceHandle, Object)} except that the application
     * provides the value for the source time stamp that is made available to
     * {@link org.omg.dds.sub.DataReader} objects by means of
     * {@link org.omg.dds.sub.Sample#getSourceTimestamp()}.
     * 
     * The constraints on the values of the instanceHandle parameter and the
     * corresponding error behavior are the same specified for
     * {@link #dispose(InstanceHandle, Object)}.
     * 
     * This operation may block and exit with {@link TimeoutException} under
     * the same circumstances described for
     * {@link #write(Object, InstanceHandle)}.
     * 
     * @throws  IllegalArgumentException    under the same circumstances as
     *          {@link #dispose(InstanceHandle, Object)}.
     * @throws  PreconditionNotMetException under the same circumstances as
     *          {@link #dispose(InstanceHandle, Object)}.
     * @throws  OutOfResourcesException     under the same circumstances as
     *          {@link #write(Object, InstanceHandle)}.
     * @throws  TimeoutException            under the same circumstances as
     *          {@link #write(Object, InstanceHandle)}.
     * 
     * @see     #dispose(InstanceHandle)
     * @see     #dispose(InstanceHandle, Object)
     * @see     #dispose(InstanceHandle, Object, Time)
     */
    public void dispose(
            InstanceHandle instanceHandle, 
            TYPE instanceData,
            long sourceTimestamp,
            TimeUnit unit) throws TimeoutException;

    /**
     * This operation can be used to retrieve the instance key that
     * corresponds to an instance handle. The operation will only fill the
     * fields that form the key inside the keyHolder instance.
     * 
     * @param   keyHolder       a container, into which this method shall
     *          place its result.
     * @param   handle          a handle indicating the instance whose value
     *          this method should get.
     *
     * @return  keyHolder, if it is non-null, or a new object otherwise.
     * 
     * @throws  IllegalArgumentException    if the handle does not correspond
     *          to an existing data object known to the DataWriter. If the
     *          implementation is not able to check invalid handles, then the
     *          result in this situation is unspecified.
     *
     * @see     #getKeyValue(InstanceHandle)
     */
    public TYPE getKeyValue(TYPE keyHolder, InstanceHandle handle);

    /**
     * This operation can be used to retrieve the instance key that
     * corresponds to an instance handle. The operation will only fill the
     * fields that form the key inside the keyHolder instance.
     * 
     * @param   handle          a handle indicating the instance whose value
     *          this method should get.
     *
     * @return  A new "key holder" object. The contents of the non-key fields
     *          are unspecified.
     * 
     * @throws  IllegalArgumentException    if the handle does not correspond
     *          to an existing data object known to the DataWriter. If the
     *          implementation is not able to check invalid handles, then the
     *          result in this situation is unspecified.
     *
     * @see     #getKeyValue(Object, InstanceHandle)
     */
    public TYPE getKeyValue(InstanceHandle handle);

    /**
     * This operation takes as a parameter an instance and returns a handle
     * that can be used in subsequent operations that accept an instance
     * handle as an argument. The instance parameter is only used for the
     * purpose of examining the fields that define the key.
     * 
     * This operation does not register the instance in question. If the
     * instance has not been previously registered, or if for any other
     * reason the Service is unable to provide an instance handle, the
     * Service will return a nil handle.
     * 
     * @param   handle  a container, into which this method shall place its
     *          result.
     * @param   keyHolder       a sample of the instance whose handle this
     *          method should look up.
     *
     * @return  handle, if it is non-null, or a new object otherwise.
     * 
     * @see     #lookupInstance(Object)
     */
    public ModifiableInstanceHandle lookupInstance(
            ModifiableInstanceHandle handle,
            TYPE keyHolder);

    /**
     * This operation takes as a parameter an instance and returns a handle
     * that can be used in subsequent operations that accept an instance
     * handle as an argument. The instance parameter is only used for the
     * purpose of examining the fields that define the key.
     * 
     * This operation does not register the instance in question. If the
     * instance has not been previously registered, or if for any other
     * reason the Service is unable to provide an instance handle, the
     * Service will return a nil handle.
     * 
     * @param   keyHolder       a sample of the instance whose handle this
     *          method should look up.
     *
     * @return  an immutable handle to the instance.
     * 
     * @see     #lookupInstance(ModifiableInstanceHandle, Object)
     */
    public InstanceHandle lookupInstance(TYPE keyHolder);


    // --- From Entity: ------------------------------------------------------

    public StatusCondition<DataWriter<TYPE>> getStatusCondition();

    public Publisher getParent();
}
