// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! REST-PSM URI-Routing — Spec §8.3.1 + §8.3.3.

use alloc::string::String;

/// Spec §8.3.1 — URI prefix `/dds/rest1`.
pub const REST_PREFIX: &str = "/dds/rest1";

/// HTTP method (Spec §8.3.2 allows POST/PUT/GET/DELETE + HEAD like GET).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RestMethod {
    /// `POST` — create operations.
    Post,
    /// `PUT` — update operations.
    Put,
    /// `GET` — read operations.
    Get,
    /// `DELETE` — delete operations.
    Delete,
    /// `HEAD` — like GET but no body (Spec §8.3.3).
    Head,
}

/// Spec §8.3.3 Tab 5 — all WebDDS operations as parameterized
/// Routen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestRoute {
    // Root operations.
    /// `POST /applications/` — Root::create_application.
    CreateApplication,
    /// `DELETE /applications/<appname>` — Root::delete_application.
    DeleteApplication {
        /// Application-Name.
        app: String,
    },
    /// `GET /applications` — Root::get_applications.
    GetApplications,
    /// `POST /types` — Root::create_type.
    CreateType,
    /// `DELETE /types/<typename>` — Root::delete_type.
    DeleteType {
        /// Type-Name.
        type_name: String,
    },
    /// `GET /types` — Root::get_types.
    GetTypes,
    /// `POST /qos_libraries` — Root::create_qos_library.
    CreateQosLibrary,
    /// `PUT /qos_libraries/<qosLibName>` — Root::update_qos_library.
    UpdateQosLibrary {
        /// QosLibrary-Name.
        qos_lib: String,
    },
    /// `DELETE /qos_libraries/<qosLibName>` — Root::delete_qos_library.
    DeleteQosLibrary {
        /// QosLibrary-Name.
        qos_lib: String,
    },
    /// `GET /qos_libraries` — Root::get_qos_libraries.
    GetQosLibraries,

    // QosLibrary operations.
    /// `POST /qos_libraries/<qosLibName>/qos_profiles` —
    /// QosLibrary::create_qos_profile.
    CreateQosProfile {
        /// QosLibrary-Name.
        qos_lib: String,
    },
    /// `PUT /qos_libraries/<qosLibName>/qos_profiles/<qosProfileName>` —
    /// QosLibrary::update_qos_profile.
    UpdateQosProfile {
        /// QosLibrary-Name.
        qos_lib: String,
        /// QosProfile-Name.
        profile: String,
    },
    /// `DELETE /qos_libraries/<qosLibName>/qos_profiles/<qosProfileName>` —
    /// QosLibrary::delete_qos_profile.
    DeleteQosProfile {
        /// QosLibrary-Name.
        qos_lib: String,
        /// QosProfile-Name.
        profile: String,
    },
    /// `GET /qos_libraries/<qosLibName>/qos_profiles` —
    /// QosLibrary::get_qos_profiles.
    GetQosProfiles {
        /// QosLibrary-Name.
        qos_lib: String,
    },

    // Application operations.
    /// `POST /applications/<appname>/domain_participants` —
    /// Application::create_participant.
    CreateParticipant {
        /// Application-Name.
        app: String,
    },
    /// `PUT /applications/<appname>/domain_participants/<partname>` —
    /// Application::update_participant.
    UpdateParticipant {
        /// Application-Name.
        app: String,
        /// Participant-Name.
        participant: String,
    },
    /// `DELETE /applications/<appname>/domain_participants/<partname>` —
    /// Application::delete_participant.
    DeleteParticipant {
        /// Application-Name.
        app: String,
        /// Participant-Name.
        participant: String,
    },
    /// `GET /applications/<appname>/domain_participants` —
    /// Application::get_participants.
    GetParticipants {
        /// Application-Name.
        app: String,
    },
    /// `POST /applications/<appname>/waitsets` —
    /// Application::create_waitset.
    CreateWaitset {
        /// Application-Name.
        app: String,
    },
    /// `GET /applications/<appname>/waitsets/<waitsetname>` —
    /// Waitset::get.
    GetWaitset {
        /// Application-Name.
        app: String,
        /// Waitset-Name.
        waitset: String,
    },

    // Participant operations.
    /// `POST /applications/<a>/domain_participants/<p>/topics/` —
    /// Participant::create_topic.
    CreateTopic {
        /// Application-Name.
        app: String,
        /// Participant-Name.
        participant: String,
    },
    /// `POST /applications/<a>/domain_participants/<p>/publishers` —
    /// Participant::create_publisher.
    CreatePublisher {
        /// Application-Name.
        app: String,
        /// Participant-Name.
        participant: String,
    },
    /// `POST /applications/<a>/domain_participants/<p>/subscribers` —
    /// Participant::create_subscriber.
    CreateSubscriber {
        /// Application-Name.
        app: String,
        /// Participant-Name.
        participant: String,
    },

    // DataWriter / DataReader operations.
    /// `POST /applications/<a>/domain_participants/<p>/publishers/<pub>/data_writers/<dwname>` —
    /// DataWriter::write.
    DataWriterWrite {
        /// Application-Name.
        app: String,
        /// Participant-Name.
        participant: String,
        /// Publisher-Name.
        publisher: String,
        /// DataWriter-Name.
        data_writer: String,
    },
    /// `GET /applications/<a>/domain_participants/<p>/subscribers/<sub>/data_readers/<drname>` —
    /// DataReader::read.
    DataReaderRead {
        /// Application-Name.
        app: String,
        /// Participant-Name.
        participant: String,
        /// Subscriber-Name.
        subscriber: String,
        /// DataReader-Name.
        data_reader: String,
    },

    /// `PUT /applications/<appname>/waitsets/<waitsetname>` —
    /// Application::update_waitset.
    UpdateWaitset {
        /// Application-Name.
        app: String,
        /// Waitset-Name.
        waitset: String,
    },
    /// `DELETE /applications/<appname>/waitsets/<waitsetname>` —
    /// Application::delete_waitset.
    DeleteWaitset {
        /// Application-Name.
        app: String,
        /// Waitset-Name.
        waitset: String,
    },
    /// `GET /applications/<appname>/waitsets` — Application::get_waitsets.
    GetWaitsets {
        /// Application-Name.
        app: String,
    },

    /// `POST /applications/<a>/domain_participants/<p>/registered_types/` —
    /// Participant::register_type.
    RegisterType {
        /// Application-Name.
        app: String,
        /// Participant-Name.
        participant: String,
    },
    /// `DELETE /applications/<a>/domain_participants/<p>/registered_types/<t>` —
    /// Participant::unregister_type.
    UnregisterType {
        /// Application-Name.
        app: String,
        /// Participant-Name.
        participant: String,
        /// Registered type name.
        type_name: String,
    },
    /// `GET /applications/<a>/domain_participants/<p>/registered_types` —
    /// Participant::get_registered_types.
    GetRegisteredTypes {
        /// Application-Name.
        app: String,
        /// Participant-Name.
        participant: String,
    },

    /// `PUT /applications/<a>/domain_participants/<p>/topics/<topicname>` —
    /// Participant::update_topic.
    UpdateTopic {
        /// Application-Name.
        app: String,
        /// Participant-Name.
        participant: String,
        /// Topic-Name.
        topic: String,
    },
    /// `DELETE /applications/<a>/domain_participants/<p>/topics/<topicname>` —
    /// Participant::delete_topic.
    DeleteTopic {
        /// Application-Name.
        app: String,
        /// Participant-Name.
        participant: String,
        /// Topic-Name.
        topic: String,
    },
    /// `GET /applications/<a>/domain_participants/<p>/topics` —
    /// Participant::get_topics.
    GetTopics {
        /// Application-Name.
        app: String,
        /// Participant-Name.
        participant: String,
    },

    /// `PUT /applications/<a>/domain_participants/<p>/publishers/<pub>` —
    /// Participant::update_publisher.
    UpdatePublisher {
        /// Application-Name.
        app: String,
        /// Participant-Name.
        participant: String,
        /// Publisher-Name.
        publisher: String,
    },
    /// `DELETE /applications/<a>/domain_participants/<p>/publishers/<pub>` —
    /// Participant::delete_publisher.
    DeletePublisher {
        /// Application-Name.
        app: String,
        /// Participant-Name.
        participant: String,
        /// Publisher-Name.
        publisher: String,
    },
    /// `GET /applications/<a>/domain_participants/<p>/publishers` —
    /// Participant::get_publishers.
    GetPublishers {
        /// Application-Name.
        app: String,
        /// Participant-Name.
        participant: String,
    },

    /// `PUT /applications/<a>/domain_participants/<p>/subscribers/<sub>` —
    /// Participant::update_subscriber.
    UpdateSubscriber {
        /// Application-Name.
        app: String,
        /// Participant-Name.
        participant: String,
        /// Subscriber-Name.
        subscriber: String,
    },
    /// `DELETE /applications/<a>/domain_participants/<p>/subscribers/<sub>` —
    /// Participant::delete_subscriber.
    DeleteSubscriber {
        /// Application-Name.
        app: String,
        /// Participant-Name.
        participant: String,
        /// Subscriber-Name.
        subscriber: String,
    },
    /// `GET /applications/<a>/domain_participants/<p>/subscribers` —
    /// Participant::get_subscribers.
    GetSubscribers {
        /// Application-Name.
        app: String,
        /// Participant-Name.
        participant: String,
    },

    /// `POST /applications/<a>/domain_participants/<p>/publishers/<pub>/data_writers` —
    /// Publisher::create_datawriter.
    CreateDataWriter {
        /// Application-Name.
        app: String,
        /// Participant-Name.
        participant: String,
        /// Publisher-Name.
        publisher: String,
    },
    /// `PUT .../publishers/<pub>/data_writers/<dwname>` —
    /// Publisher::update_datawriter.
    UpdateDataWriter {
        /// Application-Name.
        app: String,
        /// Participant-Name.
        participant: String,
        /// Publisher-Name.
        publisher: String,
        /// DataWriter-Name.
        data_writer: String,
    },
    /// `DELETE .../publishers/<pub>/data_writers/<dwname>` —
    /// Publisher::delete_datawriter.
    DeleteDataWriter {
        /// Application-Name.
        app: String,
        /// Participant-Name.
        participant: String,
        /// Publisher-Name.
        publisher: String,
        /// DataWriter-Name.
        data_writer: String,
    },
    /// `GET .../publishers/<pub>/data_writers` — Publisher::get_datawriters.
    GetDataWriters {
        /// Application-Name.
        app: String,
        /// Participant-Name.
        participant: String,
        /// Publisher-Name.
        publisher: String,
    },

    /// `POST .../subscribers/<sub>/data_readers` —
    /// Subscriber::create_datareader.
    CreateDataReader {
        /// Application-Name.
        app: String,
        /// Participant-Name.
        participant: String,
        /// Subscriber-Name.
        subscriber: String,
    },
    /// `PUT .../subscribers/<sub>/data_readers/<drname>` —
    /// Subscriber::update_datareader.
    UpdateDataReader {
        /// Application-Name.
        app: String,
        /// Participant-Name.
        participant: String,
        /// Subscriber-Name.
        subscriber: String,
        /// DataReader-Name.
        data_reader: String,
    },
    /// `DELETE .../subscribers/<sub>/data_readers/<drname>` —
    /// Subscriber::delete_datareader.
    DeleteDataReader {
        /// Application-Name.
        app: String,
        /// Participant-Name.
        participant: String,
        /// Subscriber-Name.
        subscriber: String,
        /// DataReader-Name.
        data_reader: String,
    },
    /// `GET .../subscribers/<sub>/data_readers` —
    /// Subscriber::get_datareaders.
    GetDataReaders {
        /// Application-Name.
        app: String,
        /// Participant-Name.
        participant: String,
        /// Subscriber-Name.
        subscriber: String,
    },

    /// Any other URI that matches no spec pattern.
    Unknown {
        /// Original path without the prefix.
        path: String,
    },
}

/// Canonicalises the collection segments Table 5 spells two ways.
///
/// Table 5 is not self-consistent about two segment names. The
/// `Application::*_participant` rows (p. 64–65) write
/// `/applications/<app>/domain_participants`, while every `Participant::*`
/// row (p. 66–67) writes `/applications/<app>/participants/<part>`. The
/// same split runs through the endpoint collections: the table's
/// `Publisher::*_datawriter` rows write `datawriters`, the prose and the
/// object representations write `data_writer`.
///
/// Rejecting one of the two spellings would make conforming clients fail on
/// URIs the spec itself prints, so both are accepted and normalised here to
/// the single spelling the match arms below use. Normalisation is
/// positional — only the collection slots, never a user-supplied name — so
/// an application or participant that happens to be called `participants`
/// still routes correctly.
fn canonical_segment(index: usize, segment: &str) -> &str {
    match (index, segment) {
        // /applications/<app>/{domain_participants|participants}/...
        (2, "participants") => "domain_participants",
        // .../publishers/<pub>/{data_writers|datawriters}/...
        (6, "datawriters") => "data_writers",
        // .../subscribers/<sub>/{data_readers|datareaders}/...
        (6, "datareaders") => "data_readers",
        _ => segment,
    }
}

/// Parses method + URI into a [`RestRoute`].
///
/// Expects the URI **without** the query string. The caller must have
/// percent-decoded URLs beforehand (Spec §8.3.2 requires
/// percent-encoding for non-ASCII).
///
/// # Errors
/// Returns `Err(RouteError::MissingPrefix)` if the URI does not start
/// with `/dds/rest1`.
pub fn parse_route(method: RestMethod, uri: &str) -> Result<RestRoute, RouteError> {
    let path = uri
        .strip_prefix(REST_PREFIX)
        .ok_or(RouteError::MissingPrefix)?;
    let segments: alloc::vec::Vec<&str> = path
        .trim_start_matches('/')
        .trim_end_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .enumerate()
        .map(|(i, s)| canonical_segment(i, s))
        .collect();
    // §8.3.3: "the HEAD HTTP method shall be supported on the same URIs as
    // the GET method. The HEAD method shall behave identically to the GET
    // method except for it shall return no body." Routing is therefore the
    // GET routing; suppressing the body is the serving layer's job.
    let method = if matches!(method, RestMethod::Head) {
        RestMethod::Get
    } else {
        method
    };
    Ok(match (method, segments.as_slice()) {
        // Root.
        (RestMethod::Post, ["applications"]) => RestRoute::CreateApplication,
        (RestMethod::Get, ["applications"]) => RestRoute::GetApplications,
        (RestMethod::Delete, ["applications", app]) => RestRoute::DeleteApplication {
            app: String::from(*app),
        },
        // Types.
        (RestMethod::Post, ["types"]) => RestRoute::CreateType,
        (RestMethod::Get, ["types"]) => RestRoute::GetTypes,
        (RestMethod::Delete, ["types", t]) => RestRoute::DeleteType {
            type_name: String::from(*t),
        },
        // QoS-Library.
        (RestMethod::Post, ["qos_libraries"]) => RestRoute::CreateQosLibrary,
        (RestMethod::Get, ["qos_libraries"]) => RestRoute::GetQosLibraries,
        (RestMethod::Put, ["qos_libraries", lib]) => RestRoute::UpdateQosLibrary {
            qos_lib: String::from(*lib),
        },
        (RestMethod::Delete, ["qos_libraries", lib]) => RestRoute::DeleteQosLibrary {
            qos_lib: String::from(*lib),
        },
        // QoS-Profiles.
        (RestMethod::Post, ["qos_libraries", lib, "qos_profiles"]) => RestRoute::CreateQosProfile {
            qos_lib: String::from(*lib),
        },
        (RestMethod::Get, ["qos_libraries", lib, "qos_profiles"]) => RestRoute::GetQosProfiles {
            qos_lib: String::from(*lib),
        },
        (RestMethod::Put, ["qos_libraries", lib, "qos_profiles", p]) => {
            RestRoute::UpdateQosProfile {
                qos_lib: String::from(*lib),
                profile: String::from(*p),
            }
        }
        (RestMethod::Delete, ["qos_libraries", lib, "qos_profiles", p]) => {
            RestRoute::DeleteQosProfile {
                qos_lib: String::from(*lib),
                profile: String::from(*p),
            }
        }
        // Application.
        (RestMethod::Post, ["applications", app, "domain_participants"]) => {
            RestRoute::CreateParticipant {
                app: String::from(*app),
            }
        }
        (RestMethod::Get, ["applications", app, "domain_participants"]) => {
            RestRoute::GetParticipants {
                app: String::from(*app),
            }
        }
        (RestMethod::Put, ["applications", app, "domain_participants", part]) => {
            RestRoute::UpdateParticipant {
                app: String::from(*app),
                participant: String::from(*part),
            }
        }
        (RestMethod::Delete, ["applications", app, "domain_participants", part]) => {
            RestRoute::DeleteParticipant {
                app: String::from(*app),
                participant: String::from(*part),
            }
        }
        // Waitset.
        (RestMethod::Post, ["applications", app, "waitsets"]) => RestRoute::CreateWaitset {
            app: String::from(*app),
        },
        (RestMethod::Get, ["applications", app, "waitsets"]) => RestRoute::GetWaitsets {
            app: String::from(*app),
        },
        (RestMethod::Get, ["applications", app, "waitsets", ws]) => RestRoute::GetWaitset {
            app: String::from(*app),
            waitset: String::from(*ws),
        },
        (RestMethod::Put, ["applications", app, "waitsets", ws]) => RestRoute::UpdateWaitset {
            app: String::from(*app),
            waitset: String::from(*ws),
        },
        (RestMethod::Delete, ["applications", app, "waitsets", ws]) => RestRoute::DeleteWaitset {
            app: String::from(*app),
            waitset: String::from(*ws),
        },
        // Participant — registered types.
        (
            RestMethod::Post,
            [
                "applications",
                app,
                "domain_participants",
                part,
                "registered_types",
            ],
        ) => RestRoute::RegisterType {
            app: String::from(*app),
            participant: String::from(*part),
        },
        (
            RestMethod::Get,
            [
                "applications",
                app,
                "domain_participants",
                part,
                "registered_types",
            ],
        ) => RestRoute::GetRegisteredTypes {
            app: String::from(*app),
            participant: String::from(*part),
        },
        (
            RestMethod::Delete,
            [
                "applications",
                app,
                "domain_participants",
                part,
                "registered_types",
                tn,
            ],
        ) => RestRoute::UnregisterType {
            app: String::from(*app),
            participant: String::from(*part),
            type_name: String::from(*tn),
        },
        // Participant — Topic / Publisher / Subscriber.
        (RestMethod::Post, ["applications", app, "domain_participants", part, "topics"]) => {
            RestRoute::CreateTopic {
                app: String::from(*app),
                participant: String::from(*part),
            }
        }
        (
            RestMethod::Post,
            [
                "applications",
                app,
                "domain_participants",
                part,
                "publishers",
            ],
        ) => RestRoute::CreatePublisher {
            app: String::from(*app),
            participant: String::from(*part),
        },
        (
            RestMethod::Post,
            [
                "applications",
                app,
                "domain_participants",
                part,
                "subscribers",
            ],
        ) => RestRoute::CreateSubscriber {
            app: String::from(*app),
            participant: String::from(*part),
        },
        // Participant — topic / publisher / subscriber update, delete, get.
        (RestMethod::Get, ["applications", app, "domain_participants", part, "topics"]) => {
            RestRoute::GetTopics {
                app: String::from(*app),
                participant: String::from(*part),
            }
        }
        (
            RestMethod::Put,
            [
                "applications",
                app,
                "domain_participants",
                part,
                "topics",
                tp,
            ],
        ) => RestRoute::UpdateTopic {
            app: String::from(*app),
            participant: String::from(*part),
            topic: String::from(*tp),
        },
        (
            RestMethod::Delete,
            [
                "applications",
                app,
                "domain_participants",
                part,
                "topics",
                tp,
            ],
        ) => RestRoute::DeleteTopic {
            app: String::from(*app),
            participant: String::from(*part),
            topic: String::from(*tp),
        },
        (
            RestMethod::Get,
            [
                "applications",
                app,
                "domain_participants",
                part,
                "publishers",
            ],
        ) => RestRoute::GetPublishers {
            app: String::from(*app),
            participant: String::from(*part),
        },
        (
            RestMethod::Put,
            [
                "applications",
                app,
                "domain_participants",
                part,
                "publishers",
                pubn,
            ],
        ) => RestRoute::UpdatePublisher {
            app: String::from(*app),
            participant: String::from(*part),
            publisher: String::from(*pubn),
        },
        (
            RestMethod::Delete,
            [
                "applications",
                app,
                "domain_participants",
                part,
                "publishers",
                pubn,
            ],
        ) => RestRoute::DeletePublisher {
            app: String::from(*app),
            participant: String::from(*part),
            publisher: String::from(*pubn),
        },
        (
            RestMethod::Get,
            [
                "applications",
                app,
                "domain_participants",
                part,
                "subscribers",
            ],
        ) => RestRoute::GetSubscribers {
            app: String::from(*app),
            participant: String::from(*part),
        },
        (
            RestMethod::Put,
            [
                "applications",
                app,
                "domain_participants",
                part,
                "subscribers",
                sub,
            ],
        ) => RestRoute::UpdateSubscriber {
            app: String::from(*app),
            participant: String::from(*part),
            subscriber: String::from(*sub),
        },
        (
            RestMethod::Delete,
            [
                "applications",
                app,
                "domain_participants",
                part,
                "subscribers",
                sub,
            ],
        ) => RestRoute::DeleteSubscriber {
            app: String::from(*app),
            participant: String::from(*part),
            subscriber: String::from(*sub),
        },
        // Publisher — DataWriter collection.
        (
            RestMethod::Post,
            [
                "applications",
                app,
                "domain_participants",
                part,
                "publishers",
                pubn,
                "data_writers",
            ],
        ) => RestRoute::CreateDataWriter {
            app: String::from(*app),
            participant: String::from(*part),
            publisher: String::from(*pubn),
        },
        (
            RestMethod::Get,
            [
                "applications",
                app,
                "domain_participants",
                part,
                "publishers",
                pubn,
                "data_writers",
            ],
        ) => RestRoute::GetDataWriters {
            app: String::from(*app),
            participant: String::from(*part),
            publisher: String::from(*pubn),
        },
        (
            RestMethod::Put,
            [
                "applications",
                app,
                "domain_participants",
                part,
                "publishers",
                pubn,
                "data_writers",
                dw,
            ],
        ) => RestRoute::UpdateDataWriter {
            app: String::from(*app),
            participant: String::from(*part),
            publisher: String::from(*pubn),
            data_writer: String::from(*dw),
        },
        (
            RestMethod::Delete,
            [
                "applications",
                app,
                "domain_participants",
                part,
                "publishers",
                pubn,
                "data_writers",
                dw,
            ],
        ) => RestRoute::DeleteDataWriter {
            app: String::from(*app),
            participant: String::from(*part),
            publisher: String::from(*pubn),
            data_writer: String::from(*dw),
        },
        // Subscriber — DataReader collection.
        (
            RestMethod::Post,
            [
                "applications",
                app,
                "domain_participants",
                part,
                "subscribers",
                sub,
                "data_readers",
            ],
        ) => RestRoute::CreateDataReader {
            app: String::from(*app),
            participant: String::from(*part),
            subscriber: String::from(*sub),
        },
        (
            RestMethod::Get,
            [
                "applications",
                app,
                "domain_participants",
                part,
                "subscribers",
                sub,
                "data_readers",
            ],
        ) => RestRoute::GetDataReaders {
            app: String::from(*app),
            participant: String::from(*part),
            subscriber: String::from(*sub),
        },
        (
            RestMethod::Put,
            [
                "applications",
                app,
                "domain_participants",
                part,
                "subscribers",
                sub,
                "data_readers",
                dr,
            ],
        ) => RestRoute::UpdateDataReader {
            app: String::from(*app),
            participant: String::from(*part),
            subscriber: String::from(*sub),
            data_reader: String::from(*dr),
        },
        (
            RestMethod::Delete,
            [
                "applications",
                app,
                "domain_participants",
                part,
                "subscribers",
                sub,
                "data_readers",
                dr,
            ],
        ) => RestRoute::DeleteDataReader {
            app: String::from(*app),
            participant: String::from(*part),
            subscriber: String::from(*sub),
            data_reader: String::from(*dr),
        },
        // DataWriter::write.
        (
            RestMethod::Post,
            [
                "applications",
                app,
                "domain_participants",
                part,
                "publishers",
                pubn,
                "data_writers",
                dw,
            ],
        ) => RestRoute::DataWriterWrite {
            app: String::from(*app),
            participant: String::from(*part),
            publisher: String::from(*pubn),
            data_writer: String::from(*dw),
        },
        // DataReader::read.
        (
            RestMethod::Get,
            [
                "applications",
                app,
                "domain_participants",
                part,
                "subscribers",
                sub,
                "data_readers",
                dr,
            ],
        ) => RestRoute::DataReaderRead {
            app: String::from(*app),
            participant: String::from(*part),
            subscriber: String::from(*sub),
            data_reader: String::from(*dr),
        },
        _ => RestRoute::Unknown {
            path: String::from(path),
        },
    })
}

/// Routing error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteError {
    /// The URI does not have the prefix `/dds/rest1`.
    MissingPrefix,
}

impl core::fmt::Display for RouteError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::MissingPrefix => f.write_str("URI must start with /dds/rest1"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RouteError {}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn rest_prefix_constant_matches_spec() {
        assert_eq!(REST_PREFIX, "/dds/rest1");
    }

    #[test]
    fn missing_prefix_yields_error() {
        assert_eq!(
            parse_route(RestMethod::Get, "/foo/bar"),
            Err(RouteError::MissingPrefix)
        );
    }

    #[test]
    fn create_application_route() {
        // Spec §8.3.3 Tab 5 — POST /applications/.
        let r = parse_route(RestMethod::Post, "/dds/rest1/applications").expect("ok");
        assert_eq!(r, RestRoute::CreateApplication);
    }

    #[test]
    fn get_applications_route() {
        let r = parse_route(RestMethod::Get, "/dds/rest1/applications").expect("ok");
        assert_eq!(r, RestRoute::GetApplications);
    }

    #[test]
    fn delete_application_route_extracts_app_name() {
        let r = parse_route(RestMethod::Delete, "/dds/rest1/applications/myapp").expect("ok");
        assert_eq!(
            r,
            RestRoute::DeleteApplication {
                app: String::from("myapp")
            }
        );
    }

    #[test]
    fn create_participant_route_extracts_app_name() {
        let r = parse_route(
            RestMethod::Post,
            "/dds/rest1/applications/myapp/domain_participants",
        )
        .expect("ok");
        assert_eq!(
            r,
            RestRoute::CreateParticipant {
                app: String::from("myapp")
            }
        );
    }

    #[test]
    fn delete_participant_route_extracts_app_and_participant() {
        let r = parse_route(
            RestMethod::Delete,
            "/dds/rest1/applications/myapp/domain_participants/p1",
        )
        .expect("ok");
        assert_eq!(
            r,
            RestRoute::DeleteParticipant {
                app: String::from("myapp"),
                participant: String::from("p1")
            }
        );
    }

    #[test]
    fn data_writer_write_route_extracts_4_segments() {
        let r = parse_route(
            RestMethod::Post,
            "/dds/rest1/applications/app/domain_participants/p/publishers/pub/data_writers/dw",
        )
        .expect("ok");
        assert_eq!(
            r,
            RestRoute::DataWriterWrite {
                app: String::from("app"),
                participant: String::from("p"),
                publisher: String::from("pub"),
                data_writer: String::from("dw"),
            }
        );
    }

    #[test]
    fn data_reader_read_route_extracts_4_segments() {
        let r = parse_route(
            RestMethod::Get,
            "/dds/rest1/applications/app/domain_participants/p/subscribers/sub/data_readers/dr",
        )
        .expect("ok");
        assert_eq!(
            r,
            RestRoute::DataReaderRead {
                app: String::from("app"),
                participant: String::from("p"),
                subscriber: String::from("sub"),
                data_reader: String::from("dr"),
            }
        );
    }

    #[test]
    fn qos_profile_routes_extract_lib_and_profile_name() {
        let create = parse_route(
            RestMethod::Post,
            "/dds/rest1/qos_libraries/myLib/qos_profiles",
        )
        .expect("ok");
        assert_eq!(
            create,
            RestRoute::CreateQosProfile {
                qos_lib: String::from("myLib")
            }
        );
        let put = parse_route(
            RestMethod::Put,
            "/dds/rest1/qos_libraries/myLib/qos_profiles/highQos",
        )
        .expect("ok");
        assert_eq!(
            put,
            RestRoute::UpdateQosProfile {
                qos_lib: String::from("myLib"),
                profile: String::from("highQos")
            }
        );
    }

    #[test]
    fn waitset_routes_extract_app_and_waitset() {
        let create =
            parse_route(RestMethod::Post, "/dds/rest1/applications/app/waitsets").expect("ok");
        assert_eq!(
            create,
            RestRoute::CreateWaitset {
                app: String::from("app")
            }
        );
        let get =
            parse_route(RestMethod::Get, "/dds/rest1/applications/app/waitsets/ws1").expect("ok");
        assert_eq!(
            get,
            RestRoute::GetWaitset {
                app: String::from("app"),
                waitset: String::from("ws1"),
            }
        );
    }

    #[test]
    fn topic_publisher_subscriber_create_routes() {
        for (segment, expected_variant_test) in [
            ("topics", "topic"),
            ("publishers", "publisher"),
            ("subscribers", "subscriber"),
        ] {
            let uri = alloc::format!("/dds/rest1/applications/app/domain_participants/p/{segment}");
            let r = parse_route(RestMethod::Post, &uri).expect("ok");
            // Sanity check: no Unknown variant.
            assert!(
                !matches!(r, RestRoute::Unknown { .. }),
                "{expected_variant_test} -> got {r:?}"
            );
        }
    }

    #[test]
    fn unknown_paths_yield_unknown_variant() {
        let r = parse_route(RestMethod::Get, "/dds/rest1/foobar").expect("ok");
        match r {
            RestRoute::Unknown { path } => assert_eq!(path, "/foobar"),
            _ => panic!("expected unknown"),
        }
    }

    #[test]
    fn types_routes_match_root_pattern() {
        // Spec §8.3.3 — /types without <appname> prefix.
        let create = parse_route(RestMethod::Post, "/dds/rest1/types").expect("ok");
        assert_eq!(create, RestRoute::CreateType);
        let delete = parse_route(RestMethod::Delete, "/dds/rest1/types/MyType").expect("ok");
        assert_eq!(
            delete,
            RestRoute::DeleteType {
                type_name: String::from("MyType")
            }
        );
    }

    /// Table 5 (§8.3.3, p. 64–67), row by row: PIM operation, HTTP method,
    /// URI. The table is normative and titled "Complete mapping of WebDDS PIM
    /// operations to REST methods", so the check is that *every* row routes
    /// to its own variant — a row that falls through to
    /// [`RestRoute::Unknown`] is an unimplemented operation, and two rows
    /// that collapse onto one variant is a routing collision.
    ///
    /// The URIs are transcribed from the table, with `<appname>` = `app`,
    /// `<partname>` = `p` and so on. Where the table prints a trailing slash
    /// (`/applications/`, `registered_types/`, `topics/`) it is kept, because
    /// the spec prints it and a client may send it.
    const TABLE_5: &[(&str, RestMethod, &str)] = &[
        // Root.
        (
            "Root::create_application",
            RestMethod::Post,
            "/applications/",
        ),
        (
            "Root::delete_application",
            RestMethod::Delete,
            "/applications/app",
        ),
        ("Root::get_applications", RestMethod::Get, "/applications"),
        ("Root::create_type", RestMethod::Post, "/types"),
        ("Root::delete_type", RestMethod::Delete, "/types/MyType"),
        ("Root::get_types", RestMethod::Get, "/types"),
        (
            "Root::create_qos_library",
            RestMethod::Post,
            "/qos_libraries",
        ),
        (
            "Root::update_qos_library",
            RestMethod::Put,
            "/qos_libraries/lib",
        ),
        (
            "Root::delete_qos_library",
            RestMethod::Delete,
            "/qos_libraries/lib",
        ),
        ("Root::get_qos_libraries", RestMethod::Get, "/qos_libraries"),
        // QosLibrary.
        (
            "QosLibrary::create_qos_profile",
            RestMethod::Post,
            "/qos_libraries/lib/qos_profiles",
        ),
        (
            "QosLibrary::update_qos_profile",
            RestMethod::Put,
            "/qos_libraries/lib/qos_profiles/prof",
        ),
        (
            "QosLibrary::delete_qos_profile",
            RestMethod::Delete,
            "/qos_libraries/lib/qos_profiles/prof",
        ),
        (
            "QosLibrary::get_qos_profiles",
            RestMethod::Get,
            "/qos_libraries/lib/qos_profiles",
        ),
        // Application.
        (
            "Application::create_participant",
            RestMethod::Post,
            "/applications/app/domain_participants",
        ),
        (
            "Application::update_participant",
            RestMethod::Put,
            "/applications/app/domain_participants/p",
        ),
        (
            "Application::delete_participant",
            RestMethod::Delete,
            "/applications/app/domain_participants/p",
        ),
        (
            "Application::get_participants",
            RestMethod::Get,
            "/applications/app/domain_participants",
        ),
        (
            "Application::create_waitset",
            RestMethod::Post,
            "/applications/app/waitsets",
        ),
        (
            "Application::update_waitset",
            RestMethod::Put,
            "/applications/app/waitsets/ws",
        ),
        (
            "Application::delete_waitset",
            RestMethod::Delete,
            "/applications/app/waitsets/ws",
        ),
        (
            "Application::get_waitsets",
            RestMethod::Get,
            "/applications/app/waitsets",
        ),
        // Participant.
        (
            "Participant::register_type",
            RestMethod::Post,
            "/applications/app/participants/p/registered_types/",
        ),
        (
            "Participant::unregister_type",
            RestMethod::Delete,
            "/applications/app/participants/p/registered_types/MyType",
        ),
        (
            "Participant::get_registered_types",
            RestMethod::Get,
            "/applications/app/participants/p/registered_types",
        ),
        (
            "Participant::create_topic",
            RestMethod::Post,
            "/applications/app/participants/p/topics/",
        ),
        (
            "Participant::update_topic",
            RestMethod::Put,
            "/applications/app/participants/p/topics/T",
        ),
        (
            "Participant::delete_topic",
            RestMethod::Delete,
            "/applications/app/participants/p/topics/T",
        ),
        (
            "Participant::get_topics",
            RestMethod::Get,
            "/applications/app/participants/p/topics",
        ),
        (
            "Participant::create_publisher",
            RestMethod::Post,
            "/applications/app/participants/p/publishers",
        ),
        (
            "Participant::update_publisher",
            RestMethod::Put,
            "/applications/app/participants/p/publishers/pb",
        ),
        (
            "Participant::delete_publisher",
            RestMethod::Delete,
            "/applications/app/participants/p/publishers/pb",
        ),
        (
            "Participant::get_publishers",
            RestMethod::Get,
            "/applications/app/participants/p/publishers",
        ),
        (
            "Participant::create_subscriber",
            RestMethod::Post,
            "/applications/app/participants/p/subscribers",
        ),
        (
            "Participant::update_subscriber",
            RestMethod::Put,
            "/applications/app/participants/p/subscribers/sb",
        ),
        (
            "Participant::delete_subscriber",
            RestMethod::Delete,
            "/applications/app/participants/p/subscribers/sb",
        ),
        (
            "Participant::get_subscribers",
            RestMethod::Get,
            "/applications/app/participants/p/subscribers",
        ),
        // Publisher.
        (
            "Publisher::create_datawriter",
            RestMethod::Post,
            "/applications/app/participants/p/publishers/pb/datawriters",
        ),
        (
            "Publisher::update_datawriter",
            RestMethod::Put,
            "/applications/app/participants/p/publishers/pb/datawriters/dw",
        ),
        (
            "Publisher::delete_datawriter",
            RestMethod::Delete,
            "/applications/app/participants/p/publishers/pb/datawriters/dw",
        ),
        (
            "Publisher::get_datawriters",
            RestMethod::Get,
            "/applications/app/participants/p/publishers/pb/datawriters",
        ),
        // Subscriber.
        (
            "Subscriber::create_datareader",
            RestMethod::Post,
            "/applications/app/participants/p/subscribers/sb/datareaders",
        ),
        (
            "Subscriber::update_datareader",
            RestMethod::Put,
            "/applications/app/participants/p/subscribers/sb/datareaders/dr",
        ),
        (
            "Subscriber::delete_datareader",
            RestMethod::Delete,
            "/applications/app/participants/p/subscribers/sb/datareaders/dr",
        ),
        (
            "Subscriber::get_datareaders",
            RestMethod::Get,
            "/applications/app/participants/p/subscribers/sb/datareaders",
        ),
        // DataWriter / DataReader / Waitset leaf operations.
        (
            "DataWriter::write",
            RestMethod::Post,
            "/applications/app/participants/p/publishers/pb/datawriters/dw",
        ),
        (
            "DataReader::read",
            RestMethod::Get,
            "/applications/app/participants/p/subscribers/sb/datareaders/dr",
        ),
        (
            "Waitset::get",
            RestMethod::Get,
            "/applications/app/waitsets/ws",
        ),
    ];

    #[test]
    fn table_5_every_pim_operation_has_a_route() {
        let mut unrouted = alloc::vec::Vec::new();
        for (op, method, uri) in TABLE_5 {
            let full = alloc::format!("{REST_PREFIX}{uri}");
            let route = parse_route(*method, &full).expect("prefix is present");
            if matches!(route, RestRoute::Unknown { .. }) {
                unrouted.push(*op);
            }
        }
        assert!(
            unrouted.is_empty(),
            "Table 5 rows with no route: {unrouted:?}"
        );
    }

    #[test]
    fn table_5_rows_do_not_collide() {
        // 48 rows, 48 distinct routes: no two operations may parse to the
        // same variant, or the "complete mapping" would be ambiguous. The
        // count is asserted so that dropping a row from the transcription
        // cannot silently shrink what this test covers.
        assert_eq!(TABLE_5.len(), 48, "Table 5 row count");
        let mut seen: alloc::vec::Vec<(RestRoute, &str)> = alloc::vec::Vec::new();
        for (op, method, uri) in TABLE_5 {
            let full = alloc::format!("{REST_PREFIX}{uri}");
            let route = parse_route(*method, &full).expect("prefix is present");
            if let Some((_, other)) = seen.iter().find(|(r, _)| *r == route) {
                panic!("{op} and {other} both parse to {route:?}");
            }
            seen.push((route, *op));
        }
        assert_eq!(seen.len(), TABLE_5.len());
    }

    #[test]
    fn table_5_alternate_segment_spellings_route_identically() {
        // Table 5 prints `domain_participants` on the Application rows and
        // `participants` on the Participant rows; `data_writer` in the object
        // representations and `datawriters` in the URI column. Both spellings
        // must reach the same route.
        for (a, b) in [
            (
                "/applications/app/domain_participants/p/topics/T",
                "/applications/app/participants/p/topics/T",
            ),
            (
                "/applications/app/domain_participants/p/publishers/pb/data_writers/dw",
                "/applications/app/participants/p/publishers/pb/datawriters/dw",
            ),
            (
                "/applications/app/domain_participants/p/subscribers/sb/data_readers/dr",
                "/applications/app/participants/p/subscribers/sb/datareaders/dr",
            ),
        ] {
            let ra = parse_route(RestMethod::Put, &alloc::format!("{REST_PREFIX}{a}")).expect("ok");
            let rb = parse_route(RestMethod::Put, &alloc::format!("{REST_PREFIX}{b}")).expect("ok");
            assert_eq!(ra, rb, "{a} and {b} must route identically");
            assert!(!matches!(ra, RestRoute::Unknown { .. }), "{a} unrouted");
        }
    }

    #[test]
    fn canonicalisation_is_positional_not_global() {
        // An application literally named `participants` must not be rewritten
        // — only the collection slot at index 2 is canonicalised.
        let r =
            parse_route(RestMethod::Delete, "/dds/rest1/applications/participants").expect("ok");
        assert_eq!(
            r,
            RestRoute::DeleteApplication {
                app: String::from("participants")
            }
        );
        // ... and a participant named `datawriters` stays a name.
        let r = parse_route(
            RestMethod::Delete,
            "/dds/rest1/applications/app/participants/datawriters",
        )
        .expect("ok");
        assert_eq!(
            r,
            RestRoute::DeleteParticipant {
                app: String::from("app"),
                participant: String::from("datawriters"),
            }
        );
    }

    #[test]
    fn head_behaves_like_get_on_the_same_uris() {
        // §8.3.3: "the HEAD HTTP method shall be supported on the same URIs
        // as the GET method. The HEAD method shall behave identically to the
        // GET method except for it shall return no body."
        let mut missing = alloc::vec::Vec::new();
        for (op, method, uri) in TABLE_5 {
            if *method != RestMethod::Get {
                continue;
            }
            let full = alloc::format!("{REST_PREFIX}{uri}");
            let as_get = parse_route(RestMethod::Get, &full).expect("ok");
            let as_head = parse_route(RestMethod::Head, &full).expect("ok");
            if as_head != as_get {
                missing.push((*op, *uri));
            }
        }
        assert!(
            missing.is_empty(),
            "HEAD must route like GET, but does not for: {missing:?}"
        );
    }
}
