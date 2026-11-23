// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The DDS-DCPS extended ports (DDS4CCM 1.1 §7.2.2.2): `DDS_Write`,
//! `DDS_Update`, `DDS_Read`, `DDS_Get`, `DDS_Listen` and `DDS_StateListen`,
//! each the connector fragment of one port with its basic ports under the
//! names of the specification.
//!
//! A basic port the component `uses` is an accessor of the fragment
//! (`data`, `fresh_data`, `data_control`, `filter_config`, `dds_entity`);
//! one the component `provides` is connected to the fragment
//! (`connect_data_listener`, `connect_status`).
//!
//! A subscribing port is configured in two steps, as its attribute
//! `filter` asks: the attribute is set while the port is being configured,
//! [`DdsRead::configuration_complete`] creates the DataReader — on a
//! ContentFilteredTopic if the filter has an expression — and from then on
//! setting the attribute raises `NonChangeable`. The threading policy of
//! the port (§7.4.4) is set the same way, before the configuration is
//! complete.
//!
//! A port is completed by the connector it is connected to
//! ([`crate::connector`]), which gives it the entities of §7.4.1, or by
//! hand with `configuration_complete` and entities the caller made.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use zerodds_dcps::error::return_code;
use zerodds_dcps::listener::{ArcDataReaderListener, ArcDataWriterListener};
use zerodds_dcps::{
    ContentFilteredTopic, DataReader, DataReaderQos, DataWriter, DdsType, DomainParticipant,
    Subscriber, Topic,
};

use crate::ccm_dds::{
    ContentFilterSetting, InternalError, NonChangeable, PortStatusListener, QueryFilter,
};
use crate::ccm_dds_typed::{Listener, StateListener};
use crate::listener::{DataListenerControlFragment, Push, StateListenerControlFragment};
use crate::reader::{GetterFragment, ReaderFragment};
use crate::reader_core::{ReaderCore, lock};
use crate::receptacle::Receptacle;
use crate::status::{ErrorListener, ReaderStatusListener, WriterStatusListener};
use crate::threading::{Facets, ThreadPool, ThreadingPolicy};
use crate::writer::{UpdaterFragment, WriterFragment};

fn set_writer_listener<T: DdsType>(writer: &DataWriter<T>, error_listener: Arc<ErrorListener>) {
    let listener = WriterStatusListener::new(error_listener);
    let mask = listener.mask();
    writer.set_listener(Some(Arc::new(listener) as ArcDataWriterListener), mask);
}

/// Port type `DDS_Write`: `uses Writer data`, `uses DDS::DataWriter
/// dds_entity`.
pub struct DdsWrite<T: DdsType> {
    data: Arc<WriterFragment<T>>,
}

impl<T: DdsType + Send + Sync + 'static> DdsWrite<T> {
    /// The port over `writer`, the DataWriter dedicated to it (§7.4.1). The
    /// statuses of the writer go to `error_listener`, the
    /// `ConnectorStatusListener` of the connector.
    #[must_use]
    pub fn new(writer: DataWriter<T>, error_listener: Arc<ErrorListener>) -> Self {
        set_writer_listener(&writer, error_listener);
        Self {
            data: Arc::new(WriterFragment::new(writer)),
        }
    }

    /// `uses Writer data`.
    #[must_use]
    pub fn data(&self) -> Arc<WriterFragment<T>> {
        Arc::clone(&self.data)
    }

    /// `uses DDS::DataWriter dds_entity`.
    #[must_use]
    pub fn dds_entity(&self) -> &DataWriter<T> {
        self.data.dds_entity()
    }
}

/// Port type `DDS_Update`: `uses Updater data`, `uses DDS::DataWriter
/// dds_entity`.
pub struct DdsUpdate<T: DdsType> {
    data: Arc<UpdaterFragment<T>>,
}

impl<T: DdsType + Send + Sync + 'static> DdsUpdate<T> {
    /// The port over `writer` with lifecycle checks local to the component
    /// (`is_global_scope` FALSE).
    #[must_use]
    pub fn new(writer: DataWriter<T>, error_listener: Arc<ErrorListener>) -> Self {
        set_writer_listener(&writer, error_listener);
        Self {
            data: Arc::new(UpdaterFragment::new(writer)),
        }
    }

    /// The port over `writer` with lifecycle checks global to the data
    /// space (`is_global_scope` TRUE), which look the instances up on
    /// `observer`, a DataReader of the topic the fragment keeps for itself.
    /// The statuses of both go to `error_listener`.
    #[must_use]
    pub fn with_global_scope(
        writer: DataWriter<T>,
        observer: DataReader<T>,
        error_listener: Arc<ErrorListener>,
    ) -> Self {
        let of_observer = ReaderStatusListener::<T>::of_connector(Arc::clone(&error_listener));
        let mask = of_observer.mask();
        observer.set_listener(Some(Arc::new(of_observer) as ArcDataReaderListener), mask);
        set_writer_listener(&writer, error_listener);
        Self {
            data: Arc::new(UpdaterFragment::with_global_scope(writer, observer)),
        }
    }

    /// `uses Updater data`.
    #[must_use]
    pub fn data(&self) -> Arc<UpdaterFragment<T>> {
        Arc::clone(&self.data)
    }

    /// `uses DDS::DataWriter dds_entity`.
    #[must_use]
    pub fn dds_entity(&self) -> &DataWriter<T> {
        self.data.dds_entity()
    }
}

/// What a subscribing port has once its configuration is complete.
struct Configured<T: DdsType> {
    participant: DomainParticipant,
    core: Arc<ReaderCore<T>>,
    /// The topic of attribute `filter`; `None` without an expression.
    content_filter: Option<ContentFilteredTopic<T>>,
    data: Arc<ReaderFragment<T>>,
    fresh_data: Arc<GetterFragment<T>>,
    /// Where the facets of the port are called (§7.4.4).
    facets: Facets,
}

/// The part the four subscribing ports share: attribute `filter`,
/// `filter_config`, `dds_entity`, `status` and the Reader `data`.
pub(crate) struct Subscribing<T: DdsType> {
    filter: Mutex<QueryFilter>,
    /// The threading policy of the port; changed under the lock of
    /// `filter`.
    threading: Mutex<Option<ThreadingPolicy>>,
    status: Arc<Receptacle<dyn PortStatusListener>>,
    /// The push side, of `DDS_Listen` and `DDS_StateListen`.
    push: Option<Arc<Push<T>>>,
    configured: OnceLock<Configured<T>>,
    /// How far the removal of the configured port went. Held across the
    /// deletion: a second caller, the port or its connector, waits and
    /// then continues from the step the first one reached.
    removal: Mutex<Removal>,
}

/// The steps of [`Subscribing::remove`]. A step that fails leaves the
/// removal at the step before it, so the next `remove` tries it again.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Removal {
    /// Nothing is deleted.
    Live,
    /// The conditions of the port are deleted.
    Closed,
    /// The DataReader is deleted as well.
    ReaderDeleted,
    /// The ContentFilteredTopic is deleted as well: nothing is left.
    Removed,
}

/// What the delivery of a push port waits for after its configuration:
/// the data the DataReader holds go to the component's listener only
/// when [`Self::deliver`] is called. The connector calls it after it has
/// released its own locks, so a listener can call the connector.
#[must_use = "the push port delivers nothing before `deliver`"]
pub(crate) struct Delivery<T: DdsType> {
    /// The push side of the port; `None` for a port without one.
    push: Option<Arc<Push<T>>>,
    core: Arc<ReaderCore<T>>,
    facets: Facets,
}

impl<T: DdsType + Send + 'static> Delivery<T> {
    /// Binds the push side of the port to its DataReader and delivers what
    /// the reader holds; nothing for a port without push side.
    pub(crate) fn deliver(self) {
        if let Some(push) = self.push {
            push.bind(self.core, self.facets);
        }
    }
}

/// Content-filtered topics are named per participant; each port names its
/// own.
static FILTER_TOPICS: AtomicU64 = AtomicU64::new(0);

impl<T: DdsType + Send + Sync + 'static> Subscribing<T> {
    fn new(push: Option<Arc<Push<T>>>) -> Arc<Self> {
        Arc::new(Self {
            filter: Mutex::new(QueryFilter::default()),
            threading: Mutex::new(None),
            status: Arc::new(Receptacle::new()),
            push,
            configured: OnceLock::new(),
            removal: Mutex::new(Removal::Live),
        })
    }

    fn filter(&self) -> QueryFilter {
        let mut filter = lock(&self.filter).clone();
        if let Some(topic) = self
            .configured
            .get()
            .and_then(|c| c.content_filter.as_ref())
        {
            // `filter_config` changes the parameters after the
            // configuration.
            filter.parameters = topic.get_filter_parameters();
        }
        filter
    }

    fn set_filter(&self, filter: QueryFilter) -> Result<(), NonChangeable> {
        let mut current = lock(&self.filter);
        if self.configured.get().is_some() {
            return Err(NonChangeable);
        }
        *current = filter;
        Ok(())
    }

    fn set_threading_policy(&self, policy: Option<ThreadingPolicy>) -> Result<(), NonChangeable> {
        let _configuring = lock(&self.filter);
        if self.configured.get().is_some() {
            return Err(NonChangeable);
        }
        *lock(&self.threading) = policy;
        Ok(())
    }

    /// Completes the configuration and delivers what the DataReader holds
    /// to the listener of a push port. `shared_pool` is the thread pool of
    /// the component or the container, used when the port has no policy
    /// of its own (§7.4.4); `None` is the default.
    pub(crate) fn configuration_complete(
        &self,
        participant: &DomainParticipant,
        subscriber: &Subscriber,
        topic: &Topic<T>,
        qos: DataReaderQos,
        error_listener: Arc<ErrorListener>,
        shared_pool: Option<Arc<ThreadPool>>,
    ) -> Result<(), InternalError> {
        self.configure(
            participant,
            subscriber,
            topic,
            qos,
            error_listener,
            shared_pool,
        )?
        .deliver();
        Ok(())
    }

    /// Completes the configuration as [`Self::configuration_complete`]
    /// does, without the first delivery: the caller starts it with
    /// [`Delivery::deliver`] once it holds no lock a listener may need.
    pub(crate) fn configure(
        &self,
        participant: &DomainParticipant,
        subscriber: &Subscriber,
        topic: &Topic<T>,
        qos: DataReaderQos,
        error_listener: Arc<ErrorListener>,
        shared_pool: Option<Arc<ThreadPool>>,
    ) -> Result<Delivery<T>, InternalError> {
        // Held to the end: `set_filter` waits and then finds the port
        // configured.
        let filter = lock(&self.filter);
        if self.configured.get().is_some() {
            return Err(InternalError::code(return_code::PRECONDITION_NOT_MET));
        }
        let facets = match *lock(&self.threading) {
            Some(policy) => Facets::on(Some(
                ThreadPool::new("dds4ccm-port", policy)
                    .map_err(|_| InternalError::code(return_code::OUT_OF_RESOURCES))?,
            )),
            None => Facets::on(shared_pool),
        };
        let (content_filter, reader) = if filter.is_empty() {
            (None, subscriber.create_datareader(topic, qos)?)
        } else {
            let name = format!(
                "{}.dds4ccm.filter.{}",
                topic.name(),
                FILTER_TOPICS.fetch_add(1, Ordering::Relaxed)
            );
            let filtered = participant.create_contentfilteredtopic(
                &name,
                topic,
                &filter.expression,
                filter.parameters.clone(),
            )?;
            match subscriber.create_datareader_with_cft(&filtered, qos) {
                Ok(reader) => (Some(filtered), reader),
                Err(error) => {
                    participant.delete_contentfilteredtopic(&filtered)?;
                    return Err(error.into());
                }
            }
        };
        let core = match ReaderCore::new(reader.clone()) {
            Ok(core) => core,
            Err(error) => {
                // The port stays unconfigured and keeps no entity. The
                // error of the condition is the one to report.
                let _ = subscriber.delete_datareader(&reader);
                if let Some(topic) = &content_filter {
                    let _ = participant.delete_contentfilteredtopic(topic);
                }
                return Err(error.into());
            }
        };
        let mut listener = ReaderStatusListener::new(Arc::clone(&self.status), error_listener)
            .with_facets(facets.clone());
        if let Some(push) = &self.push {
            listener = listener.with_push(push);
        }
        let mask = listener.mask();
        reader.set_listener(Some(Arc::new(listener) as ArcDataReaderListener), mask);
        let configured = Configured {
            participant: participant.clone(),
            data: Arc::new(ReaderFragment::new(Arc::clone(&core))),
            fresh_data: Arc::new(GetterFragment::new(Arc::clone(&core))),
            content_filter,
            core: Arc::clone(&core),
            facets: facets.clone(),
        };
        if self.configured.set(configured).is_err() {
            // Not reached: the lock on `filter` is held since the check.
            return Err(InternalError::code(return_code::PRECONDITION_NOT_MET));
        }
        drop(filter);
        Ok(Delivery {
            push: self.push.clone(),
            core,
            facets,
        })
    }

    /// Deletes the DataReader of the port with its conditions and its
    /// ContentFilteredTopic. A port that is not configured has nothing to
    /// delete; a port removed before, by itself or by its connector,
    /// neither: the call after a successful one returns `Ok`. A call that
    /// fails leaves what it could not delete, and the next call tries it
    /// again; a call while another one deletes waits for it.
    pub(crate) fn remove(&self) -> Result<(), InternalError> {
        let Some(configured) = self.configured.get() else {
            return Ok(());
        };
        let mut removal = lock(&self.removal);
        let reader = configured.core.reader();
        if *removal == Removal::Live {
            reader.set_listener(None, 0);
            configured.core.close()?;
            *removal = Removal::Closed;
        }
        if *removal == Removal::Closed {
            reader.get_subscriber().delete_datareader(reader)?;
            *removal = Removal::ReaderDeleted;
        }
        if *removal == Removal::ReaderDeleted {
            if let Some(topic) = &configured.content_filter {
                configured.participant.delete_contentfilteredtopic(topic)?;
            }
            *removal = Removal::Removed;
        }
        Ok(())
    }
}

impl<T: DdsType + Send + Sync + 'static> ContentFilterSetting for Subscribing<T> {
    fn set_filter_parameters(&self, parameters: Vec<String>) -> Result<(), InternalError> {
        match self
            .configured
            .get()
            .and_then(|c| c.content_filter.as_ref())
        {
            Some(topic) => Ok(topic.set_filter_parameters(parameters)?),
            // No content filter to set the parameters of: the port is not
            // configured, or its `filter` has no expression.
            None => Err(InternalError::code(return_code::PRECONDITION_NOT_MET)),
        }
    }
}

/// The members every subscribing port has: attribute `filter`,
/// `uses Reader data`, `uses ContentFilterSetting filter_config`,
/// `uses DDS::DataReader dds_entity`, `provides PortStatusListener status`.
macro_rules! subscribing_port {
    ($port:ident) => {
        impl<T: DdsType + Send + Sync + 'static> $port<T> {
            /// Attribute `filter`: the content filter of the port. After
            /// the configuration its parameters are those `filter_config`
            /// set last.
            #[must_use]
            pub fn filter(&self) -> QueryFilter {
                self.port.filter()
            }

            /// Sets attribute `filter`. An expression makes the port
            /// subscribe through a ContentFilteredTopic.
            ///
            /// # Errors
            /// `NonChangeable` once the configuration is complete.
            pub fn set_filter(&self, filter: QueryFilter) -> Result<(), NonChangeable> {
                self.port.set_filter(filter)
            }

            /// The threading policy of the port (§7.4.4): the thread pool
            /// that calls its facets. `None` leaves the choice to the
            /// component, then the container, then the default.
            #[must_use]
            pub fn threading_policy(&self) -> Option<ThreadingPolicy> {
                *lock(&self.port.threading)
            }

            /// Sets the threading policy of the port.
            ///
            /// # Errors
            /// `NonChangeable` once the configuration is complete: the
            /// thread pool is chosen when the facets are activated.
            pub fn set_threading_policy(
                &self,
                policy: Option<ThreadingPolicy>,
            ) -> Result<(), NonChangeable> {
                self.port.set_threading_policy(policy)
            }

            /// The thread pool that calls the facets of the port; `None`
            /// until the configuration is complete, and for the default.
            #[must_use]
            pub fn thread_pool(&self) -> Option<Arc<ThreadPool>> {
                let configured = self.port.configured.get()?;
                configured.facets.pool().map(Arc::clone)
            }

            /// Completes the configuration by hand, for a port that is not
            /// connected to a connector: creates the DataReader of the
            /// port (§7.4.1) with `qos` in `subscriber`, on `topic` or, if
            /// `filter` has an expression, on a ContentFilteredTopic of
            /// `topic` in `participant`. The statuses of the reader go to
            /// `status` of the port and to `error_listener`, the
            /// `ConnectorStatusListener` of the connector. The facets are
            /// called on the thread pool of the port's threading policy,
            /// without one by the default of §7.4.4.
            ///
            /// # Errors
            /// `InternalError` with the code DDS returned, also for a
            /// filter DDS does not accept; `PRECONDITION_NOT_MET` for a
            /// port that is configured; `OUT_OF_RESOURCES` if the thread
            /// pool does not start.
            pub fn configuration_complete(
                &self,
                participant: &DomainParticipant,
                subscriber: &Subscriber,
                topic: &Topic<T>,
                qos: DataReaderQos,
                error_listener: Arc<ErrorListener>,
            ) -> Result<(), InternalError> {
                self.port.configuration_complete(
                    participant,
                    subscriber,
                    topic,
                    qos,
                    error_listener,
                    None,
                )
            }

            pub(crate) fn fragment(&self) -> &Arc<Subscribing<T>> {
                &self.port
            }

            /// `uses Reader data`; `None` until the configuration is
            /// complete.
            #[must_use]
            pub fn data(&self) -> Option<Arc<ReaderFragment<T>>> {
                self.port.configured.get().map(|c| Arc::clone(&c.data))
            }

            /// `uses ContentFilterSetting filter_config`.
            #[must_use]
            pub fn filter_config(&self) -> Arc<dyn ContentFilterSetting> {
                Arc::clone(&self.port) as Arc<dyn ContentFilterSetting>
            }

            /// `uses DDS::DataReader dds_entity`; `None` until the
            /// configuration is complete.
            #[must_use]
            pub fn dds_entity(&self) -> Option<DataReader<T>> {
                self.port.configured.get().map(|c| c.core.reader().clone())
            }

            /// Connects `provides PortStatusListener status` and returns
            /// the listener it replaces.
            pub fn connect_status(
                &self,
                listener: Arc<dyn PortStatusListener>,
            ) -> Option<Arc<dyn PortStatusListener>> {
                self.port.status.connect(listener)
            }

            /// Disconnects `status`.
            pub fn disconnect_status(&self) -> Option<Arc<dyn PortStatusListener>> {
                self.port.status.disconnect()
            }

            /// Removes the port: deletes its DataReader with its
            /// conditions and its ContentFilteredTopic. A port that is not
            /// configured, or was removed before by itself or by its
            /// connector, has nothing to delete: `Ok`.
            ///
            /// # Errors
            /// `InternalError` with the code DDS returned. What was not
            /// deleted stays with the port, and the next call tries it
            /// again; `PRECONDITION_NOT_MET`, for example, while the
            /// application holds a ReadCondition of `dds_entity`.
            pub fn remove(&self) -> Result<(), InternalError> {
                self.port.remove()
            }
        }
    };
}

/// Port type `DDS_Read`: `uses Reader data`, attribute `filter`,
/// `uses ContentFilterSetting filter_config`, `uses DDS::DataReader
/// dds_entity`, `provides PortStatusListener status`.
pub struct DdsRead<T: DdsType> {
    port: Arc<Subscribing<T>>,
}

/// Port type `DDS_Get`: the members of [`DdsRead`] and `uses Getter
/// fresh_data`.
pub struct DdsGet<T: DdsType> {
    port: Arc<Subscribing<T>>,
}

/// Port type `DDS_Listen`: the members of [`DdsRead`], `uses
/// DataListenerControl data_control` and `provides Listener data_listener`.
pub struct DdsListen<T: DdsType> {
    port: Arc<Subscribing<T>>,
    push: Arc<Push<T>>,
    data_control: Arc<DataListenerControlFragment<T>>,
}

/// Port type `DDS_StateListen`: the members of [`DdsRead`], `uses
/// StateListenerControl data_control` and `provides StateListener
/// data_listener`.
pub struct DdsStateListen<T: DdsType> {
    port: Arc<Subscribing<T>>,
    push: Arc<Push<T>>,
    data_control: Arc<StateListenerControlFragment<T>>,
}

subscribing_port!(DdsRead);
subscribing_port!(DdsGet);
subscribing_port!(DdsListen);
subscribing_port!(DdsStateListen);

impl<T: DdsType + Send + Sync + 'static> DdsRead<T> {
    /// A port that is not configured yet.
    #[must_use]
    pub fn new() -> Self {
        Self {
            port: Subscribing::new(None),
        }
    }
}

impl<T: DdsType + Send + Sync + 'static> Default for DdsRead<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: DdsType + Send + Sync + 'static> DdsGet<T> {
    /// A port that is not configured yet.
    #[must_use]
    pub fn new() -> Self {
        Self {
            port: Subscribing::new(None),
        }
    }

    /// `uses Getter fresh_data`; `None` until the configuration is
    /// complete.
    #[must_use]
    pub fn fresh_data(&self) -> Option<Arc<GetterFragment<T>>> {
        self.port
            .configured
            .get()
            .map(|c| Arc::clone(&c.fresh_data))
    }
}

impl<T: DdsType + Send + Sync + 'static> Default for DdsGet<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: DdsType + Send + Sync + 'static> DdsListen<T> {
    /// A port that is not configured yet. Its listener mode is
    /// `NOT_ENABLED`.
    #[must_use]
    pub fn new() -> Self {
        let push = Push::new();
        Self {
            port: Subscribing::new(Some(Arc::clone(&push))),
            data_control: Arc::new(DataListenerControlFragment::new(Arc::clone(&push))),
            push,
        }
    }

    /// `uses DataListenerControl data_control`.
    #[must_use]
    pub fn data_control(&self) -> Arc<DataListenerControlFragment<T>> {
        Arc::clone(&self.data_control)
    }

    /// Connects `provides Listener data_listener` and returns the listener
    /// it replaces. With an enabled mode the fresh data the port holds are
    /// delivered to `listener`: before the call returns, or on the thread
    /// pool of the port.
    pub fn connect_data_listener(
        &self,
        listener: Arc<dyn Listener<T>>,
    ) -> Option<Arc<dyn Listener<T>>> {
        let before = self.push.data_listener.connect(listener);
        self.push.deliver();
        before
    }

    /// Disconnects `data_listener`. Data that arrive without a listener
    /// stay fresh in the DataReader.
    pub fn disconnect_data_listener(&self) -> Option<Arc<dyn Listener<T>>> {
        self.push.data_listener.disconnect()
    }
}

impl<T: DdsType + Send + Sync + 'static> Default for DdsListen<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: DdsType + Send + Sync + 'static> DdsStateListen<T> {
    /// A port that is not configured yet. Its listener mode is
    /// `NOT_ENABLED`.
    #[must_use]
    pub fn new() -> Self {
        let push = Push::new();
        Self {
            port: Subscribing::new(Some(Arc::clone(&push))),
            data_control: Arc::new(StateListenerControlFragment::new(Arc::clone(&push))),
            push,
        }
    }

    /// `uses StateListenerControl data_control`.
    #[must_use]
    pub fn data_control(&self) -> Arc<StateListenerControlFragment<T>> {
        Arc::clone(&self.data_control)
    }

    /// Connects `provides StateListener data_listener` and returns the
    /// listener it replaces. With an enabled mode the fresh data the port
    /// holds are delivered to `listener`: before the call returns, or on
    /// the thread pool of the port.
    pub fn connect_data_listener(
        &self,
        listener: Arc<dyn StateListener<T>>,
    ) -> Option<Arc<dyn StateListener<T>>> {
        let before = self.push.state_listener.connect(listener);
        self.push.deliver();
        before
    }

    /// Disconnects `data_listener`. Data that arrive without a listener
    /// stay fresh in the DataReader.
    pub fn disconnect_data_listener(&self) -> Option<Arc<dyn StateListener<T>>> {
        self.push.state_listener.disconnect()
    }
}

impl<T: DdsType + Send + Sync + 'static> Default for DdsStateListen<T> {
    fn default() -> Self {
        Self::new()
    }
}
