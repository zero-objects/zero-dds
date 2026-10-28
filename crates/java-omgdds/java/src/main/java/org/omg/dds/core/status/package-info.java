/**
 * Statuses may be classified as:
 * 
 * <ul>
 *     <li>Read communication statuses: i.e., those that are related to
 *         arrival of data, namely
 *         {@link org.omg.dds.core.event.DataOnReadersEvent} and
 *         {@link org.omg.dds.core.event.DataAvailableEvent}.</li>
 *     <li>Plain communication statuses: i.e., all the others.</li>
 * </ul>
 * 
 * Read communication statuses are treated slightly differently than the
 * others for they don't change independently. In other words, at least two
 * changes will appear at the same time (DATA_ON_READERS + DATA_AVAILABLE)
 * and even several of the last kind may be part of the set. This 'grouping'
 * has to be communicated to the application.
 * 
 * For each communication status, there is a corresponding structure to hold
 * the status value. These values contain the information related to the
 * change of status, as well as information related to the statuses
 * themselves (e.g., contains cumulative counts). They are used with the two
 * different status-access mechanisms.
 * 
 * Associated with each one of an {@link org.omg.dds.core.Entity}'s
 * communication status is a logical "StatusChangedFlag." This flag indicates
 * whether that particular communication status has changed since the last
 * time the status was "read" by the application. The way the status changes
 * is slightly different for the Plain Communication Status and the Read
 * Communication status.
 * 
 * Note that this description is only conceptual; it simply represents that
 * the Entity knows which specific statuses have changed. It does not imply
 * any particular implementation of the StatusChangedFlag in terms of boolean
 * values.
 * 
 * <b>Changes in Plain Communication Status</b>
 * 
 * For the plain communication status, the StatusChangedFlag flag is
 * initially set to false. It becomes true whenever the plain communication
 * status changes and it is reset to false each time the application accesses
 * the plain communication status via the proper get operation on the Entity.
 * 
 * The communication status is also reset to false whenever the associated
 * listener operation is called as the listener implicitly accesses the
 * status which is passed as a parameter to the operation. The fact that the
 * status is reset prior to calling the listener means that if the
 * application calls the get operation from inside the listener it will see
 * the status already reset.
 * 
 * An exception to this rule is when the associated listener is null. Such a
 * listener is treated as a NO-OP and the act of "calling" a null listener
 * does not reset the communication status.
 * 
 * For example, the value of the StatusChangedFlag associated with the
 * {@link org.omg.dds.core.status.RequestedDeadlineMissedStatus} will become
 * true each time new deadline occurs (which increases the totalCount field
 * within RequestedDeadlineMissedStatus). The value changes to false when the
 * application accesses the status via
 * {@link org.omg.dds.sub.DataReader#getRequestedDeadlineMissedStatus()}.
 * 
 * <b>Changes in Read Communication Statuses</b>
 * 
 * For the read communication status, the StatusChangedFlag flag is initially
 * set to false.
 * 
 * The StatusChangedFlag becomes true when either a data sample arrives or
 * else the {@link org.omg.dds.sub.ViewState},
 * {@link org.omg.dds.sub.SampleState}, or
 * {@link org.omg.dds.sub.InstanceState} of any existing sample changes for
 * any reason other than a call to {@link org.omg.dds.sub.DataReader#read()},\
 * {@link org.omg.dds.sub.DataReader#take()} or their variants. Specifically
 * any of the following events will cause the StatusChangedFlag to become
 * true:
 * 
 * <ul>
 *     <li>The arrival of new data.</li>
 *     <li>A change in the InstanceState of a contained instance. This can be
 *         caused by either:<ul>
 *         <li>The arrival of the notification that an instance has been
 *             disposed by:<ul>
 *             <li>the DataWriter that owns it if
 *                 {@link org.omg.dds.core.policy.Ownership#getKind()}
 *                 = {@link org.omg.dds.core.policy.Ownership.Kind#EXCLUSIVE}</li>
 *             <li>or by any DataWriter if
 *                 {@link org.omg.dds.core.policy.Ownership#getKind()}
 *                 = {@link org.omg.dds.core.policy.Ownership.Kind#SHARED}.</li>
 *             </ul></li>
 *         <li>The loss of liveliness of the DataWriter of an instance for
 *             which there is no other DataWriter.</li>
 *         <li>The arrival of the notification that an instance has been
 *             unregistered by the only DataWriter that is known to be
 *             writing the instance.</li>
 *         </ul></li>
 * </ul>
 * 
 * Depending on the kind of StatusChangedFlag, the flag transitions to false
 * again as follows:
 * 
 * <ul>
 *     <li>The DATA_AVAILABLE StatusChangedFlag becomes false when either the
 *         {@link org.omg.dds.sub.DataReaderListener#onDataAvailable(DataAvailableEvent)}
 *         is called or the {@link org.omg.dds.sub.DataReader#read()} or
 *         {@link org.omg.dds.sub.DataReader#take()} operation (or their
 *         variants) is called on the associated DataReader.</li>
 *     <li>The DATA_ON_READERS StatusChangedFlag becomes false when any of
 *         the following events occur:<ul>
 *         <li>{@link org.omg.dds.sub.SubscriberListener#onDataOnReaders(DataOnReadersEvent)}
 *             is called.</li>
 *         <li>{@link org.omg.dds.sub.DataReaderListener#onDataAvailable(DataAvailableEvent)}
 *             is called on any DataReader belonging to the
 *             {@link org.omg.dds.sub.Subscriber}.</li>
 *         <li>The {@link org.omg.dds.sub.DataReader#read()} or
 *             {@link org.omg.dds.sub.DataReader#take()} operation (or their
 *             variants) is called on any DataReader belonging to the
 *             Subscriber.</li>
 *         </ul></li>
 * </ul>
 * 
 * <b>Access through Listeners</b>
 * 
 * Listeners provide a mechanism for the middleware to asynchronously alert
 * the application of the occurrence of relevant status changes.
 * 
 * All {@link org.omg.dds.core.Entity} types support a listener, the type of
 * which is specialized to the specific type of the related Entity (e.g.
 * {@link org.omg.dds.sub.DataReaderListener} for the
 * {@link org.omg.dds.sub.DataReader}). Listeners are interfaces that the
 * application must implement. Each dedicated listener presents a list of
 * operations that correspond to the relevant communication status changes
 * (i.e. that the application may react to).
 * 
 * Listeners are stateless. It is thus possible to share the same
 * DataReaderListener instance among all the DataReader objects (assuming
 * that they will react similarly on similar status changes). Consequently,
 * the provided parameter contains a reference to the actual concerned Entity
 * (see {@link java.util.EventObject#getSource()}).
 * 
 * <b>Conditions and Wait-sets</b>
 * 
 * As previously mentioned, {@link org.omg.dds.core.Condition}s (in
 * conjunction with {@link org.omg.dds.core.WaitSet}s) provide an alternative
 * mechanism to allow the middleware to communicate communication status
 * changes (including arrival of data) to the application.
 */
package org.omg.dds.core.status;
