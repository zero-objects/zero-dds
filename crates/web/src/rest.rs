// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! REST-PSM URI-Routing — Spec §8.3.1 + §8.3.3.

use alloc::string::String;

/// Spec §8.3.1 — URI-Prefix `/dds/rest1`.
pub const REST_PREFIX: &str = "/dds/rest1";

/// HTTP-Method (Spec §8.3.2 erlaubt POST/PUT/GET/DELETE + HEAD wie GET).
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

/// Spec §8.3.3 Tab 5 — alle WebDDS-Operations als parametrisierte
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

    /// Beliebige andere URI, die zu keinem Spec-Pattern matched.
    Unknown {
        /// Original-Pfad ohne Prefix.
        path: String,
    },
}

/// Parst Method + URI in eine [`RestRoute`].
///
/// Erwartet die URI **ohne** Query-String. Caller muss URLs
/// vorher percent-decoded haben (Spec §8.3.2 verlangt
/// percent-encoding fuer non-ASCII).
///
/// # Errors
/// Liefert `Err(RouteError::MissingPrefix)` wenn URI nicht mit
/// `/dds/rest1` beginnt.
pub fn parse_route(method: RestMethod, uri: &str) -> Result<RestRoute, RouteError> {
    let path = uri
        .strip_prefix(REST_PREFIX)
        .ok_or(RouteError::MissingPrefix)?;
    let segments: alloc::vec::Vec<&str> = path
        .trim_start_matches('/')
        .trim_end_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();
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
        (RestMethod::Get, ["applications", app, "waitsets", ws]) => RestRoute::GetWaitset {
            app: String::from(*app),
            waitset: String::from(*ws),
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

/// Routing-Fehler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteError {
    /// URI hat nicht den Prefix `/dds/rest1`.
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
            // Sanity-Check: keine Unknown-Variante.
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
        // Spec §8.3.3 — /types ohne <appname>-Prefix.
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
}
