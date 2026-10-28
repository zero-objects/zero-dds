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

import java.util.Collection;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.TimeoutException;


/**
 * A WaitSet object allows an application to wait until one or more of the
 * attached {@link org.omg.dds.core.Condition} objects has a triggerValue of true or else until
 * the timeout expires.
 * 
 * WaitSet is not necessarily associated with a single
 * {@link org.omg.dds.domain.DomainParticipant} and could be used to wait on
 * Condition objects
 * associated with different DomainParticipant objects.
 * 
 * This mechanism is wait-based. Its general use pattern is as follows:
 * 
 * <ul>
 *     <li>The application indicates which relevant information it wants to
 *         get by creating {@link org.omg.dds.core.Condition} objects
 *         ({@link org.omg.dds.core.StatusCondition}, {@link org.omg.dds.sub.ReadCondition} or
 *         {@link org.omg.dds.sub.QueryCondition}) and attaching them to a
 *          WaitSet.</li>
 *     <li>It then waits on that WaitSet until the triggerValue of one or
 *         several Condition objects become true.</li>
 *     <li>It then uses the result of the wait (i.e., the list of Condition
 *         objects with triggerValue == true) to actually get the information
 *         by calling:<ul>
 *         <li>{@link org.omg.dds.core.Entity#getStatusChanges()} and then
 *             <code>get&lt;<i>CommunicationStatus</i>&gt;</code> on the
 *             relevant Entity.</li>
 *         <li>{@link org.omg.dds.core.Entity#getStatusChanges()} and then
 *             {@link org.omg.dds.sub.Subscriber#getDataReaders(Collection)}
 *             on the relevant Subscriber.</li>
 *         <li>{@link org.omg.dds.core.Entity#getStatusChanges()} and then
 *             {@link org.omg.dds.sub.DataReader#read()}/
 *             {@link org.omg.dds.sub.DataReader#take()} on the
 *             relevant DataReader.</li>
 *         <li>Directly call
 *             {@link org.omg.dds.sub.DataReader#read(org.omg.dds.sub.DataReader.Selector)}/
 *             {@link org.omg.dds.sub.DataReader#take(org.omg.dds.sub.DataReader.Selector)}
 *             with the {@link org.omg.dds.sub.DataReader.Selector} wrapping a
 *             ReadCondition or a QueryCondition.</li>
 *         </ul></li>
 * </ul>
 * 
 * Usually the first step is done in an initialization phase, while the
 * others are put in the application main loop.
 * 
 * As there is no extra information passed from the middleware to the
 * application when a wait returns (only the list of triggered Condition
 * objects), Condition objects are meant to embed all that is needed to react
 * properly when enabled. In particular, Entity-related conditions are
 * related to exactly one Entity and cannot be shared.
 * 
 * The blocking behavior of the WaitSet is described below. The result of a
 * {@link #waitForConditions()} operation depends on the state of the
 * WaitSet, which in turn depends on whether at least one attached Condition
 * has a triggerValue of true. If the {@link #waitForConditions()} operation
 * is called on WaitSet that is blocked, it will block the calling thread.
 * If {@link #waitForConditions()} is called on a WaitSet that is unblocked,
 * it will return immediately. In addition, when the WaitSet transitions from
 * BLOCKED to UNBLOCKED it wakes up any threads that had called
 * {@link #waitForConditions()} on it.
 * 
 * Similar to the invocation of listeners, there is no implied "event
 * queuing" in the awakening of a WaitSet in the sense that, if several
 * Conditions attached to the WaitSet have their triggerValue transition to
 * true in sequence the DCPS implementation needs to only unblock the WaitSet
 * once.
 */
public abstract class WaitSet implements DDSObject {
    // -----------------------------------------------------------------------
    // Factory Methods
    // -----------------------------------------------------------------------

    /**
     * @param env       Identifies the Service instance to which the new
     *                  object will belong.
     */
    public static WaitSet newWaitSet(ServiceEnvironment env)
    {
        return env.getSPI().newWaitSet();
    }



    // -----------------------------------------------------------------------
    // Instance Methods
    // -----------------------------------------------------------------------

    /**
     * This operation allows an application thread to wait for the occurrence
     * of certain conditions. If none of the conditions attached to the
     * WaitSet have a triggerValue of true, the operation will block
     * suspending the calling thread.
     * 
     * The result of the wait operation is the list of all the attached
     * conditions that have a triggerValue of true (i.e., the conditions that
     * unblocked the wait).
     * 
     * It is not allowed for more than one application thread to be waiting
     * on the same WaitSet. If the operation is invoked on a WaitSet that
     * already has a thread blocking on it, the operation will fail with the
     * value {@link org.omg.dds.core.PreconditionNotMetException}.
     */
    public abstract void waitForConditions();

    /**
     * This operation allows an application thread to wait for the occurrence
     * of certain conditions. If none of the conditions attached to the
     * WaitSet have a triggerValue of true, the operation will block
     * suspending the calling thread.
     * 
     * The result of the wait operation is the list of all the attached
     * conditions that have a triggerValue of true (i.e., the conditions that
     * unblocked the wait).
     * 
     * It is not allowed for more than one application thread to be waiting
     * on the same WaitSet. If the operation is invoked on a WaitSet that
     * already has a thread blocking on it, the operation will fail with the
     * value {@link org.omg.dds.core.PreconditionNotMetException}.
     */
    public abstract void waitForConditions(
            Collection<Condition> activeConditions);

    /**
     * This operation allows an application thread to wait for the occurrence
     * of certain conditions. If none of the conditions attached to the
     * WaitSet have a triggerValue of true, the operation will block
     * suspending the calling thread.
     * 
     * The result of the wait operation is the list of all the attached
     * conditions that have a triggerValue of true (i.e., the conditions that
     * unblocked the wait).
     * 
     * It is not allowed for more than one application thread to be waiting
     * on the same WaitSet. If the operation is invoked on a WaitSet that
     * already has a thread blocking on it, the operation will fail with the
     * value {@link org.omg.dds.core.PreconditionNotMetException}.
     * 
     * @throws  TimeoutException    if the timeout argument, which specifies
     *          the maximum duration for the wait, is exceeded and none of
     *          the attached Condition objects is true.
     */
    public abstract void waitForConditions(Duration timeout)
    throws TimeoutException;

    /**
     * This operation allows an application thread to wait for the occurrence
     * of certain conditions. If none of the conditions attached to the
     * WaitSet have a triggerValue of true, the operation will block
     * suspending the calling thread.
     * 
     * The result of the wait operation is the list of all the attached
     * conditions that have a triggerValue of true (i.e., the conditions that
     * unblocked the wait).
     * 
     * It is not allowed for more than one application thread to be waiting
     * on the same WaitSet. If the operation is invoked on a WaitSet that
     * already has a thread blocking on it, the operation will fail with the
     * value {@link org.omg.dds.core.PreconditionNotMetException}.
     * 
     * @throws  TimeoutException    if the timeout argument, which specifies
     *          the maximum duration for the wait, is exceeded and none of
     *          the attached Condition objects is true.
     */
    public abstract void waitForConditions(long timeout, TimeUnit unit)
    throws TimeoutException;

    /**
     * This operation allows an application thread to wait for the occurrence
     * of certain conditions. If none of the conditions attached to the
     * WaitSet have a triggerValue of true, the operation will block
     * suspending the calling thread.
     * 
     * The result of the wait operation is the list of all the attached
     * conditions that have a triggerValue of true (i.e., the conditions that
     * unblocked the wait).
     * 
     * It is not allowed for more than one application thread to be waiting
     * on the same WaitSet. If the operation is invoked on a WaitSet that
     * already has a thread blocking on it, the operation will fail with the
     * value {@link org.omg.dds.core.PreconditionNotMetException}.
     * 
     * @throws  TimeoutException    if the timeout argument, which specifies
     *          the maximum duration for the wait, is exceeded and none of
     *          the attached Condition objects is true.
     */
    public abstract void waitForConditions(
            Collection<Condition> activeConditions,
            Duration timeout)
    throws TimeoutException;

    /**
     * This operation allows an application thread to wait for the occurrence
     * of certain conditions. If none of the conditions attached to the
     * WaitSet have a triggerValue of true, the operation will block
     * suspending the calling thread.
     * 
     * The result of the wait operation is the list of all the attached
     * conditions that have a triggerValue of true (i.e., the conditions that
     * unblocked the wait).
     * 
     * It is not allowed for more than one application thread to be waiting
     * on the same WaitSet. If the operation is invoked on a WaitSet that
     * already has a thread blocking on it, the operation will fail with the
     * value {@link org.omg.dds.core.PreconditionNotMetException}.
     * 
     * @throws  TimeoutException    if the timeout argument, which specifies
     *          the maximum duration for the wait, is exceeded and none of
     *          the attached Condition objects is true.
     */
    public abstract void waitForConditions(
            Collection<Condition> activeConditions,
            long timeout,
            TimeUnit unit)
    throws TimeoutException;

    /**
     * Attaches a Condition to the WaitSet.
     * 
     * It is possible to attach a Condition on a WaitSet that is currently
     * being waited upon (via the {@link #waitForConditions()} operation).
     * In this case, if the Condition has a triggerValue of true, then
     * attaching the condition will unblock the WaitSet.
     * 
     * Adding a Condition that is already attached to the WaitSet has no
     * effect.
     * 
     * @throws  OutOfResourcesException if attaching the condition requires
     *          the allocation of unavailable middleware or OS resources.
     */
    public abstract void attachCondition(Condition cond);

    /**
     * Detaches a Condition from the WaitSet.
     * 
     * @throws  PreconditionNotMetException     if the Condition was not
     *          attached to the WaitSet.
     */
    public abstract void detachCondition(Condition cond);

    /**
     * This operation retrieves the list of attached conditions.
     * 
     * @return  an unmodifiable collection of the conditions attached to this
     *          wait set.
     */
    public abstract Collection<Condition> getConditions();
}
