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

import java.util.List;
import java.util.Set;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.TimeoutException;

import org.omg.dds.core.DDSObject;
import org.omg.dds.core.DomainEntity;
import org.omg.dds.core.Duration;
import org.omg.dds.core.InstanceHandle;
import org.omg.dds.core.ModifiableInstanceHandle;
import org.omg.dds.core.StatusCondition;
import org.omg.dds.core.status.LivelinessChangedStatus;
import org.omg.dds.core.status.RequestedDeadlineMissedStatus;
import org.omg.dds.core.status.RequestedIncompatibleQosStatus;
import org.omg.dds.core.status.SampleLostStatus;
import org.omg.dds.core.status.SampleRejectedStatus;
import org.omg.dds.core.status.SubscriptionMatchedStatus;
import org.omg.dds.topic.PublicationBuiltinTopicData;
import org.omg.dds.topic.TopicDescription;


/**
 * A DataReader allows the application (1) to declare the data it wishes to
 * receive (i.e., make a subscription) and (2) to access the data received by
 * the attached {@link org.omg.dds.sub.Subscriber}.
 * 
 * A DataReader refers to exactly one {@link org.omg.dds.topic.TopicDescription} (either a
 * {@link org.omg.dds.topic.Topic}, a {@link org.omg.dds.topic.ContentFilteredTopic}, or a {@link org.omg.dds.topic.MultiTopic})
 * that identifies the data to be read. The subscription has a unique
 * resulting type. The data reader may give access to several instances of
 * the resulting type, which can be distinguished from each other by their
 * keys.
 * 
 * All operations except for the inherited operations
 * {@link #setQos(org.omg.dds.core.EntityQos)}, {@link #getQos()},
 * {@link #setListener(java.util.EventListener)}, {@link #getListener()},
 * {@link #enable()}, {@link #getStatusCondition()}, and {@link #close()} may
 * fail with the exception {@link org.omg.dds.core.NotEnabledException}.
 * 
 * All sample-accessing operations, namely all variants of {@link #read()} or
 * {@link #take()}, may fail with the exception
 * {@link org.omg.dds.core.PreconditionNotMetException}.
 * 
 * <b>Access to the Data</b>
 * 
 * Data is made available to the application by the following operations on
 * DataReader objects: {@link #read()}, {@link #take()}, and the other methods
 * beginning with those prefixes.. The general semantics of the "read"
 * operations is that the application only gets access to the corresponding
 * data; the data remains the middleware's responsibility and can be read
 * again. The semantics of the "take" operations is that the application
 * takes full responsibility for the data; that data will no longer be
 * accessible to the DataReader. Consequently, it is possible for a
 * DataReader to access the same sample multiple times but only if all
 * previous accesses were read operations.
 * 
 * Each of these operations returns an ordered collection of {@link org.omg.dds.sub.Sample}s
 * (data values and associated meta-information). Each data value represents
 * an atom of data information (i.e., a value for one instance). This
 * collection may contain samples related to the same or different instances
 * (identified by the key). Multiple samples can refer to the same instance
 * if the settings of the {@link org.omg.dds.core.policy.History} allow for it.
 * 
 * @param <TYPE>    The concrete type of the data to be read.
 */
public interface DataReader<TYPE>
extends DomainEntity<DataReaderListener<TYPE>, DataReaderQos>
{
    /**
     * Cast this data reader to the given type, or throw an exception if
     * the cast fails.
     * 
     * @param <OTHER>   The type of the data subscribed to by this reader,
     *                  according to the caller.
     * @return          this data reader
     * @throws          ClassCastException if the cast fails
     */
    public <OTHER> DataReader<OTHER> cast();

    /**
     * This operation creates a ReadCondition. The returned ReadCondition
     * will be attached and belong to the DataReader.
     * 
     * @param   states  The returned condition will only trigger on samples
     *          with one of these sample states, view states, and
     *          instance states.
     */
    public ReadCondition<TYPE> createReadCondition(
            Subscriber.DataState states);

    /**
     * This operation creates a QueryCondition. The returned QueryCondition
     * will be attached and belong to the DataReader. It will trigger on any
     * sample state, view state, or instance state.
     * 
     * @param   queryExpression The returned condition will only trigger on
     *          samples that pass this content-based filter expression.
     * @param   queryParameters A set of parameter values for the
     *          queryExpression.
     *
     * @see     #createQueryCondition(org.omg.dds.sub.Subscriber.DataState, String, List)
     */
    public QueryCondition<TYPE> createQueryCondition(
            String queryExpression,
            List<String> queryParameters);

    /**
     * This operation creates a QueryCondition. The returned QueryCondition
     * will be attached and belong to the DataReader.
     * 
     * @param   states  The returned condition will only trigger on samples
     *          with one of these sample states, view states, and instance
     *          states.
     * @param   queryExpression The returned condition will only trigger on
     *          samples that pass this content-based filter expression.
     * @param   queryParameters A set of parameter values for the
     *          queryExpression.
     *
     * @see     #createQueryCondition(String, List)
     */
    public QueryCondition<TYPE> createQueryCondition(
            Subscriber.DataState states,
            String queryExpression,
            List<String> queryParameters);

    /**
     * This operation closes all the entities that were created by means of
     * the "create" operations on the DataReader. That is, it closes all
     * contained ReadCondition and QueryCondition objects.
     * 
     * @throws  PreconditionNotMetException     if the any of the contained
     *          entities is in a state where it cannot be closed.
     */
    public void closeContainedEntities();

    /**
     * @return  the TopicDescription associated with the DataReader. This is
     *          the same TopicDescription that was used to create the
     *          DataReader.
     */
    public TopicDescription<TYPE> getTopicDescription();

    /**
     * This operation allows access to the SAMPLE_REJECTED communication
     * status.
     * 
     * @see     org.omg.dds.core.status
     */
    public SampleRejectedStatus getSampleRejectedStatus();

    /**
     * This operation allows access to the LIVELINESS_CHANGED communication
     * status.
     * 
     * @see     org.omg.dds.core.status
     */
    public LivelinessChangedStatus getLivelinessChangedStatus();

    /**
     * This operation allows access to the REQUESTED_DEADLINE_MISSED
     * communication status.
     * 
     * @see     org.omg.dds.core.status
     */
    public RequestedDeadlineMissedStatus getRequestedDeadlineMissedStatus();

    /**
     * This operation allows access to the REQUESTED_INCOMPATIBLE_QOS
     * communication status.
     * 
     * @see     org.omg.dds.core.status
     */
    public RequestedIncompatibleQosStatus getRequestedIncompatibleQosStatus();

    /**
     * This operation allows access to the SUBSCRIPTION_MATCHED communication
     * status. 
     * 
     * @see     org.omg.dds.core.status
     */
    public SubscriptionMatchedStatus getSubscriptionMatchedStatus();

    /**
     * This operation allows access to the SAMPLE_LOST communication status.
     * 
     * @see     org.omg.dds.core.status
     */
    public SampleLostStatus getSampleLostStatus();

    /**
     * This operation is intended only for DataReader entities for which
     * {@link org.omg.dds.core.policy.Durability#getKind()} is not
     * {@link org.omg.dds.core.policy.Durability.Kind#VOLATILE}.
     * 
     * As soon as an application enables a non-VOLATILE DataReader it will
     * start receiving both "historical" data, i.e., the data that was
     * written prior to the time the DataReader joined the domain, as well as
     * any new data written by the DataWriter entities. There are situations
     * where the application logic may require the application to wait until
     * all "historical" data is received. This is the purpose of this
     * operation.
     * 
     * The operation blocks the calling thread until either all "historical"
     * data is received, or else the duration specified by the max_Wait
     * parameter elapses, whichever happens first.
     * 
     * @throws  TimeoutException        if maxWait elapsed before all the
     *          data was received.
     * 
     * @see     #waitForHistoricalData(long, TimeUnit)
     */
    public void waitForHistoricalData(Duration maxWait)
    throws TimeoutException;

    /**
     * This operation is intended only for DataReader entities for which
     * {@link org.omg.dds.core.policy.Durability#getKind()} is not
     * {@link org.omg.dds.core.policy.Durability.Kind#VOLATILE}.
     * 
     * As soon as an application enables a non-VOLATILE DataReader it will
     * start receiving both "historical" data, i.e., the data that was
     * written prior to the time the DataReader joined the domain, as well as
     * any new data written by the DataWriter entities. There are situations
     * where the application logic may require the application to wait until
     * all "historical" data is received. This is the purpose of this
     * operation.
     * 
     * The operation blocks the calling thread until either all "historical"
     * data is received, or else the duration specified by the max_Wait
     * parameter elapses, whichever happens first.
     * 
     * @throws  TimeoutException        if maxWait elapsed before all the
     *          data was received.
     * 
     * @see     #waitForHistoricalData(Duration)
     */
    public void waitForHistoricalData(long maxWait, TimeUnit unit)
    throws TimeoutException;

    /**
     * This operation retrieves the list of publications currently
     * "associated" with the DataReader; that is, publications that have a
     * matching {@link org.omg.dds.topic.Topic} and compatible QoS that the application has not
     * indicated should be "ignored" by means of
     * {@link org.omg.dds.domain.DomainParticipant#ignorePublication(InstanceHandle)}.
     * 
     * The handles returned in the 'publicationHandles' list are the ones
     * that are used by the DDS implementation to locally identify the
     * corresponding matched DataWriter entities. These handles match the
     * ones that appear in {@link org.omg.dds.sub.Sample#getInstanceHandle()} when reading
     * the "DCPSPublications" built-in topic.
     * 
     * The operation may fail if the infrastructure does not locally maintain
     * the connectivity information.
     * 
     * @return  a new collection containing a copy of the information.
     * 
     * @see     #getMatchedPublicationData(InstanceHandle)
     */
    public Set<InstanceHandle> getMatchedPublications();

    /**
     * This operation retrieves information on a publication that is
     * currently "associated" with the DataReader; that is, a publication
     * with a matching {@link org.omg.dds.topic.Topic} and compatible QoS that the application
     * has not indicated should be "ignored" by means of
     * {@link org.omg.dds.domain.DomainParticipant#ignorePublication(InstanceHandle)}.
     * 
     * The operation {@link #getMatchedPublications()} can be used
     * to find the publications that are currently matched with the
     * DataReader.
     * 
     * @param   publicationHandle       a handle to the publication, the
     *          data of which is to be retrieved.
     * 
     * @return  a new object containing a copy of the information.
     * 
     * @throws  IllegalArgumentException        if the publicationHandle does
     *          not correspond to a publication currently associated with the
     *          DataReader.
     * @throws  UnsupportedOperationException   if the infrastructure does
     *          not hold the information necessary to fill in the
     *          publicationData.
     *
     * @see     #getMatchedPublications()
     */
    public PublicationBuiltinTopicData getMatchedPublicationData(
            InstanceHandle publicationHandle);


    // --- Type-specific interface: ------------------------------------------

    /**
     * This operation accesses a collection of samples from this DataReader.
     * It behaves exactly like {@link #read(Selector)} except that the
     * collection of returned samples is not constrained by any Selector.
     * 
     * @return  a non-null unmodifiable iterator over loaned samples.
     * 
     * @see     #read(Selector)
     * @see     #read(List)
     * @see     #read(List, Selector)
     * @see     #readNextSample(Sample)
     * @see     #take()
     */
    public Sample.Iterator<TYPE> read();

    /**
     * This operation accesses a collection of samples from this DataReader.
     * The returned samples will be limited by the given {@link Selector}. The
     * setting of the {@link org.omg.dds.core.policy.Presentation}
     * may impose further limits on the returned samples.
     * 
     * <ol>
     *     <li>If
     *         {@link org.omg.dds.core.policy.Presentation#getAccessScope()}
     *         is
     *         {@link org.omg.dds.core.policy.Presentation.AccessScopeKind#INSTANCE},
     *         then samples belonging to the same data instance are consecutive.
     *         </li>
     *     <li>If
     *         {@link org.omg.dds.core.policy.Presentation#getAccessScope()}
     *         is
     *         {@link org.omg.dds.core.policy.Presentation.AccessScopeKind#TOPIC}
     *         and
     *         {@link org.omg.dds.core.policy.Presentation#isOrderedAccess()}
     *         is set to false, then samples belonging to the same data
     *         instance are consecutive.</li>
     *     <li>If
     *         {@link org.omg.dds.core.policy.Presentation#getAccessScope()}
     *         is
     *         {@link org.omg.dds.core.policy.Presentation.AccessScopeKind#TOPIC}
     *         and
     *         {@link org.omg.dds.core.policy.Presentation#isOrderedAccess()}
     *         is set to true, then samples belonging to the same instance
     *         may or may not be consecutive. This is because to preserve
     *         order it may be necessary to mix samples from different
     *         instances.</li>
     *     <li>If
     *         {@link org.omg.dds.core.policy.Presentation#getAccessScope()}
     *         is
     *         {@link org.omg.dds.core.policy.Presentation.AccessScopeKind#GROUP}
     *         and
     *         {@link org.omg.dds.core.policy.Presentation#isOrderedAccess()}
     *         is set to false, then samples belonging to the same data
     *         instance are consecutive.</li>
     *     <li>If
     *         {@link org.omg.dds.core.policy.Presentation#getAccessScope()}
     *         is
     *         {@link org.omg.dds.core.policy.Presentation.AccessScopeKind#GROUP}
     *         and
     *         {@link org.omg.dds.core.policy.Presentation#isOrderedAccess()}
     *         is set to true, then the returned collection contains at most
     *         one sample. The difference in this case is due to the fact
     *         that it is required that the application is able to read
     *         samples belonging to different DataReader objects in a
     *         specific order.
     *         </li>
     * </ol>
     * 
     * In any case, the relative order between the samples of one instance is
     * consistent with the
     * {@link org.omg.dds.core.policy.DestinationOrder}:
     * 
     * <ul>
     *     <li>If
     *         {@link org.omg.dds.core.policy.DestinationOrder#getKind()}
     *         is
     *         {@link org.omg.dds.core.policy.DestinationOrder.Kind#BY_RECEPTION_TIMESTAMP},
     *         samples belonging to the same instances will appear in the
     *         relative order in which they were received (FIFO, earlier
     *         samples ahead of the later samples).</li>
     *     <li>If
     *         {@link org.omg.dds.core.policy.DestinationOrder#getKind()}
     *         is
     *         {@link org.omg.dds.core.policy.DestinationOrder.Kind#BY_SOURCE_TIMESTAMP},
     *         samples belonging to the same instances will appear in the
     *         relative order implied by the result of
     *         {@link org.omg.dds.sub.Sample#getSourceTimestamp()} (FIFO, smaller values of
     *         the source time stamp ahead of the larger values).<li>
     * </ul>
     * 
     * In addition to the sample data, the read operation also provides
     * sample meta-information ("sample info"). See {@link org.omg.dds.sub.Sample}.
     * 
     * The returned samples are "loaned" by the DataReader. The use of this
     * variant allows for zero-copy (assuming the implementation supports it)
     * access to the data and the application will need to "return the loan"
     * to the DataReader using the {@link Sample.Iterator#close()}
     * operation.
     * 
     * Some elements in the returned collection may not have valid data. If
     * the instance state in the Sample is
     * {@link org.omg.dds.sub.InstanceState#NOT_ALIVE_DISPOSED} or
     * {@link org.omg.dds.sub.InstanceState#NOT_ALIVE_NO_WRITERS}, then the last sample for
     * that instance in the collection, that is, the one with
     * {@link org.omg.dds.sub.Sample#getSampleRank()} == 0, does not contain valid data.
     * Samples that contain no data do not count towards the limits imposed
     * by the {@link org.omg.dds.core.policy.ResourceLimits}.
     * 
     * The act of reading a sample sets its sample state to
     * {@link org.omg.dds.sub.SampleState#READ}. If the sample belongs to the most recent
     * generation of the instance, it will also set the view state of the
     * instance to {@link org.omg.dds.sub.ViewState#NOT_NEW}. It will not affect the
     * instance state of the instance.
     * 
     * If the DataReader has no samples that meet the constraints, the
     * return value will be a non-null iterator that provides no samples.
     * 
     * @return  a non-null unmodifiable iterator over loaned samples.
     * 
     * @see     #read()
     * @see     #read(List)
     * @see     #read(List, Selector)
     * @see     #readNextSample(Sample)
     * @see     #take(Selector)
     */
    public Sample.Iterator<TYPE> read(Selector<TYPE> query);

    /**
     * This operation accesses a collection of samples from this DataReader.
     * It behaves exactly like {@link #read()} except that the returned
     * samples are no more than #maxSamples.
     * 
     * @return  a non-null unmodifiable iterator over loaned samples.
     * 
     * @see     #read()
     * @see     #read(Selector)
     * @see     #read(List, Selector)
     * @see     #readNextSample(Sample)
     * @see     #take(List)
     */
    public Sample.Iterator<TYPE> read(int maxSamples);

    /**
     * This operation accesses a collection of samples from this DataReader.
     * It behaves exactly like {@link #read()} except that the returned
     * samples are not "on loan" from the Service; they are deeply copied to
     * the application. 
     * 
     * If the number of samples read are fewer than the current
     * length of the list, the list will be trimmed to fit the 
     * samples read. If list is null, a new list will be allocated 
     * and its size may be zero or unbounded depending upon the 
     * number of samples read. If there are no samples, the list 
     * reference will be non-null and the list will contain zero 
     * samples. 
     * 
     * The read operation will copy the data and meta-information into the
     * elements already inside the given collection, overwriting any samples
     * that might already be present. The use of this variant forces a copy
     * but the application can control where the copy is placed and the
     * application will not need to "return the loan."
     * 
     * @return  <code>samples</code>, for convenience.
     * 
     * @see     #read()
     * @see     #read(Selector)
     * @see     #read(List, Selector)
     * @see     #readNextSample(Sample)
     * @see     #take(List)
     */
    public List<Sample<TYPE>> read(List<Sample<TYPE>> samples);

    /**
     * This operation accesses a collection of samples from this DataReader.
     * It behaves exactly like {@link #read(Selector)} except that the returned
     * samples are not "on loan" from the Service; they are deeply copied to
     * the application.
     * 
     * The number of samples are specified as the minimum of 
     * {@link Selector#getMaxSamples()} and the length of the list. 
     * If the number of samples read are fewer than the current 
     * length of the list, the list will be trimmed to fit the 
     * samples read. If list is null, a new list will be allocated 
     * and its size may be zero or unbounded depending upon the 
     * number of samples read. If there are no samples, the list 
     * reference will be non-null and the list will contain zero 
     * samples. 
     * 
     * The read operation will copy the data and meta-information into the
     * elements already inside the given collection, overwriting any samples
     * that might already be present. The use of this variant forces a copy
     * but the application can control where the copy is placed and the
     * application will not need to "return the loan."
     * 
     * @return  <code>samples</code>, for convenience.
     * 
     * @see     #read()
     * @see     #read(Selector)
     * @see     #read(List)
     * @see     #readNextSample(Sample)
     * @see     #take(List, Selector)
     */
    public List<Sample<TYPE>> read(
            List<Sample<TYPE>> samples,
            Selector<TYPE> selector);

    /**
     * This operation accesses a collection of samples from this DataReader.
     * It behaves exactly like {@link #take(Selector)} except that the
     * collection of returned samples is not constrained by any Selector.
     * 
     * @return  a non-null unmodifiable iterator over loaned samples.
     * 
     * @see     #take(Selector)
     * @see     #take(List)
     * @see     #take(List, Selector)
     * @see     #takeNextSample(Sample)
     * @see     #read()
     */
    public Sample.Iterator<TYPE> take();

    /**
     * This operation accesses a collection of samples from this DataReader.
     * It behaves exactly like {@link #take(Selector)} except that the
     * collection of returned samples is not constrained by any Selector.
     * 
     * The number of samples accessible via the iterator will not be 
     * more than #maxSamples.
     * 
     * @return  a non-null unmodifiable iterator over loaned samples.
     * 
     * @see     #take(Selector)
     * @see     #take(List)
     * @see     #take(List, Selector)
     * @see     #takeNextSample(Sample)
     * @see     #read()
     */
    public Sample.Iterator<TYPE> take(int maxSamples);

    /**
     * This operation accesses a collection of samples from this DataReader.
     * The number of samples returned is controlled by the
     * {@link org.omg.dds.core.policy.Presentation} and other
     * factors using the same logic as for {@link #read(Selector)}.
     * 
     * The act of taking a sample removes it from the DataReader so it cannot
     * be "read" or "taken" again. If the sample belongs to the most recent
     * generation of the instance, it will also set the view state of the
     * instance to {@link org.omg.dds.sub.ViewState#NOT_NEW}. It will not affect the
     * instance state of the instance.
     * 
     * The behavior of the take operation follows the same rules than the
     * read operation regarding the preconditions and postconditions for the
     * arguments and return results. Similar to read, the take operation will
     * "loan" elements to the application; this loan must then be returned by
     * means of {@link Sample.Iterator#close()}. The only difference
     * with read is that, as stated, the sample returned by take will no
     * longer be accessible to successive calls to read or take.
     * 
     * If the DataReader has no samples that meet the constraints, the
     * return value will be a non-null iterator that provides no samples.
     * 
     * @return  a non-null unmodifiable iterator over loaned samples.
     * 
     * @see     #take()
     * @see     #take(List)
     * @see     #take(List, Selector)
     * @see     #takeNextSample(Sample)
     * @see     #read(Selector)
     */
    public Sample.Iterator<TYPE> take(Selector<TYPE> query);

    /**
     * This operation accesses a collection of samples from this DataReader.
     * It behaves exactly like {@link #take()} except that the returned
     * samples are not "on loan" from the Service; they are deeply copied to
     * the application.
     * 
     * The take operation will copy the data and meta-information into the
     * elements already inside the given collection, overwriting any samples
     * that might already be present. The use of this variant forces a copy
     * but the application can control where the copy is placed and the
     * application will not need to "return the loan."
     * 
     * 
     * 
     * @return  <code>samples</code>, for convenience.
     * 
     * @see     #take()
     * @see     #take(Selector)
     * @see     #take(List, Selector)
     * @see     #takeNextSample(Sample)
     * @see     #read(List)
     */
    public List<Sample<TYPE>> take(List<Sample<TYPE>> samples);

    /**
     * This operation accesses a collection of samples from this DataReader.
     * It behaves exactly like {@link #take(Selector)} except that the returned
     * samples are not "on loan" from the Service; they are deeply copied to
     * the application.
     * 
     * The take operation will copy the data and meta-information into the
     * elements already inside the given collection, overwriting any samples
     * that might already be present. The use of this variant forces a copy
     * but the application can control where the copy is placed and the
     * application will not need to "return the loan."
     * 
     * @return  <code>samples</code>, for convenience.
     * 
     * @see     #take()
     * @see     #take(Selector)
     * @see     #take(List)
     * @see     #takeNextSample(Sample)
     * @see     #read(List, Selector)
     */
    public List<Sample<TYPE>> take(
            List<Sample<TYPE>> samples,
            Selector<TYPE> query);

    /**
     * This operation copies the next, non-previously accessed sample from
     * this DataReader. The implied order among the samples stored in the
     * DataReader is the same as for {@link #read(List, Selector)}.
     * 
     * This operation is semantically equivalent to
     * {@link #read(List, Selector)} where {@link Selector#getMaxSamples()}
     * is 1, {@link Selector#getDataState()} followed by
     * {@link Subscriber.DataState#getSampleStates()} ==
     * {@link org.omg.dds.sub.SampleState#NOT_READ}, {@link Selector#getDataState()} followed by
     * {@link Subscriber.DataState#getViewStates()} contains all view
     * states, and {@link Selector#getDataState()} followed by
     * {@link Subscriber.DataState#getInstanceStates()} contains all
     * instance states.
     * 
     * This operation provides a simplified API to "read" samples avoiding
     * the need for the application to manage iterators and specify queries.
     * 
     * If there is no unread data in the DataReader, the operation will
     * return false and the provided sample is not modified.
     * 
     * @return  true if data was read or false if no data was available.
     * 
     * @see     #read()
     * @see     #read(Selector)
     * @see     #read(List)
     * @see     #read(List, Selector)
     * @see     #takeNextSample(Sample)
     */
    public boolean readNextSample(Sample<TYPE> sample);

    /**
     * This operation copies the next, non-previously accessed sample from
     * this DataReader and "removes" it from the DataReader so it is no
     * longer accessible. This operation is analogous to 
     * {@link #readNextSample(Sample)} except for the fact that the sample is
     * "removed" from the DataReader.
     * 
     * This operation is semantically equivalent to
     * {@link #take(List, Selector)} where {@link Selector#getMaxSamples()}
     * is 1, {@link Selector#getDataState()} followed by
     * {@link Subscriber.DataState#getSampleStates()} ==
     * {@link org.omg.dds.sub.SampleState#NOT_READ}, {@link Selector#getDataState()} followed by
     * {@link Subscriber.DataState#getViewStates()} contains all view
     * states, and {@link Selector#getDataState()} followed by
     * {@link Subscriber.DataState#getInstanceStates()} contains all
     * instance states.
     * 
     * This operation provides a simplified API to "take" samples avoiding
     * the need for the application to manage iterators and specify queries.
     * 
     * If there is no unread data in the DataReader, the operation will
     * return false and the provided sample is not modified.
     * 
     * @return  true if data was taken or false if no data was available.
     * 
     * @see     #take()
     * @see     #take(Selector)
     * @see     #take(List)
     * @see     #take(List, Selector)
     * @see     #readNextSample(Sample)
     */
    public boolean takeNextSample(Sample<TYPE> sample);

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
     * @return  keyHolder, as a convenience to facilitate chaining.
     * 
     * @throws  IllegalArgumentException        if the {@link org.omg.dds.core.InstanceHandle}
     *          does not correspond to an existing data object known to the
     *          DataReader. If the implementation is not able to check
     *          invalid handles, then the result in this situation is
     *          unspecified.
     */
    public TYPE getKeyValue(
            TYPE keyHolder, 
            InstanceHandle handle);

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

    public StatusCondition<DataReader<TYPE>> getStatusCondition();

    public Subscriber getParent();

    /**
     * Provides a {@link Selector} that can be used to refine what {@link #read} or 
     * {@link #take} methods return. 
     * 
     * @return The {@link Selector} object returned by this method
     *         is the default selector. By default it selects 
     *         {@link org.omg.dds.core.policy.ResourceLimits#LENGTH_UNLIMITED} 
     *         samples.  This is equivalent to calling {@link org.omg.dds.sub.DataReader#read} without 
     *         any parameters. 
     * */
    
    public Selector<TYPE> select();
    
    /**
     * Selector class encapsulates different ways of selecting samples from a {@link org.omg.dds.sub.DataReader}.
     * Selector can be used with {@link org.omg.dds.sub.DataReader#read(Selector)} and {@link org.omg.dds.sub.DataReader#take(Selector)}
     * or it can be used stand-alone as it provides {@link #read} and {@link #take} functions.
     * 
     * {@link org.omg.dds.sub.DataReader#select} creates a Selector that is bound to the {@link org.omg.dds.sub.DataReader}.
     *  
     * A Selector may encapsulate any combination of {@link org.omg.dds.core.InstanceHandle}, 
     * {@link Subscriber.DataState}, a query filter. It can be used to bound the maximum
     * number of samples retrieved.
     *      
     * @param <T>    The concrete type of the data to be read.
     */
    public static interface Selector<T> extends DDSObject {
    	
    	// --- Setters ----------------------------------------------
    	
    	public Selector<T> instance(InstanceHandle handle);
    	
    	public Selector<T> nextInstance(boolean retrieveNextInstance);
    	
    	public Selector<T> dataState(Subscriber.DataState state);
    	
    	public Selector<T> Content(String queryExpression, List<String> queryParameters);
    	
    	public Selector<T> maxSamples(int max);
    	
    	// --- Getters ----------------------------------------------    	
    	
    	public InstanceHandle getInstance();
    	
    	public boolean retrieveNextInstance();
    	
    	public Subscriber.DataState getDataState();
    	
    	public String getQueryExpression();
    	
    	public List<String> getQueryParameters();
    	
    	public int getMaxSamples();
    	
    	public ReadCondition<T> getCondition();
    	
        // --- read/take operations ----------------------------------
    	
    	public Sample.Iterator<T> read();
        
        public List<Sample<T>> read(List<Sample<T>> samples);        

        public Sample.Iterator<T> take();
        
        public List<Sample<T>> take(List<Sample<T>> samples);                
    }
}
