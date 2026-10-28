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
 * Determines the mechanism and parameters used by the application to
 * determine whether an {@link org.omg.dds.core.Entity} is "active" (alive). The "liveliness"
 * status of an Entity is used to maintain instance ownership in combination
 * with the setting of the {@link org.omg.dds.core.policy.Ownership}. The application is
 * also informed via an Entity {@link org.omg.dds.core.status.Status} change when an Entity is no
 * longer alive. The {@link org.omg.dds.sub.DataReader} requests that liveliness of the
 * writers is maintained by the requested means and loss of liveliness is
 * detected with delay not to exceed the leaseDuration. The {@link org.omg.dds.pub.DataWriter}
 * commits to signaling its liveliness using the stated means at intervals
 * not to exceed the leaseDuration. The default kind is
 * {@link Liveliness.Kind#AUTOMATIC} and the default value of the
 * leaseDuration is infinite.
 * 
 * <b>Concerns:</b> {@link org.omg.dds.topic.Topic}, {@link org.omg.dds.sub.DataReader}, {@link org.omg.dds.pub.DataWriter}
 * 
 * <b>RxO:</b> Yes
 * 
 * <b>Changeable:</b> No
 * 
 * This policy has several settings to support both data objects that are
 * updated periodically as well as those that are changed sporadically. It
 * also allows customizing for different application requirements in terms of
 * the kinds of failures that will be detected by the liveliness mechanism.
 * 
 * The {@link Kind#AUTOMATIC} liveliness setting is most appropriate for
 * applications that only need to detect failures at the process level, but
 * not application-logic failures within a process. The Service takes
 * responsibility for renewing the leases at the required rates and thus, as
 * long as the local process where a {@link org.omg.dds.domain.DomainParticipant} is running and
 * the link connecting it to remote participants remains connected, the
 * entities within the DomainParticipant will be considered alive. This
 * requires the lowest overhead.
 * 
 * The MANUAL settings ({@link Kind#MANUAL_BY_PARTICIPANT},
 * {@link Kind#MANUAL_BY_TOPIC}) require the application on the publishing
 * side to periodically assert the liveliness before the lease expires to
 * indicate the corresponding Entity is still alive. The action can be
 * explicit by calling the <code>assertLiveliness</code> operations
 * ({@link org.omg.dds.pub.DataWriter#assertLiveliness()},
 * {@link org.omg.dds.domain.DomainParticipant#assertLiveliness()}) or implicit by writing some
 * data.
 * 
 * The two possible manual settings control the granularity at which the
 * application must assert liveliness.
 * 
 * <ul>
 *      <li>The setting MANUAL_BY_PARTICIPANT requires only that one Entity
 *          within the publisher is asserted to be alive to deduce all other
 *          Entity objects within the same DomainParticipant are also alive.
 *      <li>The setting MANUAL_BY_TOPIC requires that at least one instance
 *          within the DataWriter is asserted.</li>
 * </ul>
 * 
 * The value offered is considered compatible with the value requested if and
 * only if the following conditions are met:
 * 
 * <ol>
 *      <li>the inequality "offered kind &gt;= requested kind" evaluates to
 *          true. For the purposes of this inequality, the values of
 *          LIVELINESS kind are considered ordered such that:
 *          <code>AUTOMATIC &lt; MANUAL_BY_PARTICIPANT &lt; MANUAL_BY_TOPIC
 *          </code>.</li>
 *      <li>the inequality "offered leaseDuration <= requested
 *          leaseDuration" evaluates to true.</li>
 * </ol>
 * 
 * Changes in LIVELINESS must be detected by the Service with a
 * time granularity greater or equal to the leaseDuration. This ensures that
 * the value of the {@link org.omg.dds.core.status.LivelinessChangedStatus} is updated at least once
 * during each leaseDuration and the related Listeners and {@link org.omg.dds.core.WaitSet}s
 * are notified within a leaseDuration from the time the LIVELINESS changed.
 */
public interface Liveliness
extends QosPolicy.ForTopic,
        QosPolicy.ForDataReader,
        QosPolicy.ForDataWriter,
        RequestedOffered<Liveliness>
{
    // -----------------------------------------------------------------------
    // Methods
    // -----------------------------------------------------------------------

    /**
     * @return the kind
     */
    public Kind getKind();

    public Duration getLeaseDuration();


    // --- Modification: -----------------------------------------------------

    /**
     * Copy this policy and override the value of the property.
     * 
     * @return  a new policy
     */
    public Liveliness withKind(Kind kind);

    /**
     * Copy this policy and override the value of the property.
     * 
     * @return  a new policy
     */
    public Liveliness withLeaseDuration(Duration leaseDuration);

    /**
     * Copy this policy and override the value of the property.
     * 
     * @return  a new policy
     */
    public Liveliness withLeaseDuration(
            long leaseDuration,
            TimeUnit unit);

    public Liveliness withAutomatic();
    
    public Liveliness withManualByParticipant();
    
    public Liveliness withManualByTopic();
    

    // -----------------------------------------------------------------------
    // Types
    // -----------------------------------------------------------------------

    public enum Kind {
        /**
         * The infrastructure will automatically signal liveliness for the
         * {@link org.omg.dds.pub.DataWriter}s at least as often as required by the
         * leaseDuration.
         */
        AUTOMATIC,

        /**
         * The user application takes responsibility to signal liveliness to
         * the Service. Liveliness must be asserted at least once every
         * leaseDuration otherwise the Service will assume the corresponding
         * {@link org.omg.dds.core.Entity} is no longer "active/alive."
         * 
         * The Service will assume that as long as at least one {@link org.omg.dds.core.Entity}
         * within the {@link org.omg.dds.domain.DomainParticipant} has asserted its liveliness
         * the other Entities in that same DomainParticipant are also alive.
         */
        MANUAL_BY_PARTICIPANT,

        /**
         * The user application takes responsibility to signal liveliness to
         * the Service. Liveliness must be asserted at least once every
         * leaseDuration otherwise the Service will assume the corresponding
         * {@link org.omg.dds.core.Entity} is no longer "active/alive."
         * 
         * The Service will only assume liveliness of the {@link org.omg.dds.pub.DataWriter}
         * if the application has asserted liveliness of that DataWriter
         * itself.
         */
        MANUAL_BY_TOPIC
    }

}
