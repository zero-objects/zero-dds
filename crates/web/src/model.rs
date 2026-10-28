// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! WebDDS Object Model — Spec §7.

use alloc::string::String;
use alloc::vec::Vec;

use crate::session::SessionId;
use crate::status::ReturnStatus;

/// Spec §7.3.1 — `WebDDS::Root` singleton. Manages all objects.
#[derive(Debug, Default)]
pub struct WebDdsRoot {
    /// All registered applications.
    pub applications: Vec<Application>,
    /// All registered clients.
    pub clients: Vec<Client>,
}

impl WebDdsRoot {
    /// Spec §7.3.1.1 — `create_application`. Returns
    /// `OBJECT_ALREADY_EXISTS` if an application with the same name
    /// exists.
    ///
    /// # Errors
    /// See [`ReturnStatus`].
    pub fn create_application(&mut self, app: Application) -> Result<SessionId, ReturnStatus> {
        if self.applications.iter().any(|a| a.name == app.name) {
            return Err(ReturnStatus::ObjectAlreadyExists);
        }
        let token = alloc::format!("session-{}", self.applications.len() + 1);
        self.applications.push(app);
        Ok(SessionId::new(token))
    }

    /// Spec §7.3.1.2 — `delete_application`.
    ///
    /// # Errors
    /// `INVALID_OBJECT` if the name does not exist.
    pub fn delete_application(&mut self, name: &str) -> Result<(), ReturnStatus> {
        let idx = self
            .applications
            .iter()
            .position(|a| a.name == name)
            .ok_or(ReturnStatus::InvalidObject)?;
        self.applications.remove(idx);
        Ok(())
    }

    /// Spec §7.3.1.3 — `get_applications` with an fnmatch pattern.
    ///
    /// `expression` is a POSIX fnmatch pattern (`*` = anything,
    /// `?` = one character). We implement a simplified subset
    /// with the `*` wildcard.
    #[must_use]
    pub fn get_applications(&self, expression: &str) -> Vec<&Application> {
        self.applications
            .iter()
            .filter(|a| fnmatch_simple(expression, &a.name))
            .collect()
    }
}

/// Spec §7.3 — `WebDDS::Application` Entity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Application {
    /// Name (unique in the root scope).
    pub name: String,
    /// DomainParticipants of the application.
    pub participants: Vec<DomainParticipant>,
}

/// Spec §7.3.1 — `WebDDS::Client` (User-Principal).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Client {
    /// Client-API-Key.
    pub client_api_key: String,
}

/// Spec §7.3 — `WebDDS::DomainParticipant`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainParticipant {
    /// Name (unique per application).
    pub name: String,
    /// `domain_id` (Spec §7.3 Tab).
    pub domain_id: i32,
}

/// POSIX fnmatch subset (only the `*` wildcard, no `?`/`[...]` classes).
fn fnmatch_simple(pattern: &str, name: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    // Simple prefix*-suffix matcher.
    if let Some(idx) = pattern.find('*') {
        let (prefix, suffix) = pattern.split_at(idx);
        let suffix = &suffix[1..]; // skip '*'.
        return name.starts_with(prefix) && name.ends_with(suffix);
    }
    pattern == name
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn app(name: &str) -> Application {
        Application {
            name: String::from(name),
            participants: Vec::new(),
        }
    }

    #[test]
    fn create_application_returns_session_id() {
        // Spec §7.3.1.1.
        let mut root = WebDdsRoot::default();
        let s = root.create_application(app("app1")).expect("ok");
        assert!(s.is_valid());
    }

    #[test]
    fn duplicate_application_yields_object_already_exists() {
        // Spec — same name → OBJECT_ALREADY_EXISTS.
        let mut root = WebDdsRoot::default();
        root.create_application(app("dup")).expect("first ok");
        assert_eq!(
            root.create_application(app("dup")),
            Err(ReturnStatus::ObjectAlreadyExists)
        );
    }

    #[test]
    fn delete_application_removes_existing() {
        let mut root = WebDdsRoot::default();
        root.create_application(app("a")).expect("ok");
        assert_eq!(root.delete_application("a"), Ok(()));
        assert!(root.applications.is_empty());
    }

    #[test]
    fn delete_unknown_application_yields_invalid_object() {
        let mut root = WebDdsRoot::default();
        assert_eq!(
            root.delete_application("missing"),
            Err(ReturnStatus::InvalidObject)
        );
    }

    #[test]
    fn get_applications_with_wildcard_matches_all() {
        // Spec §7.3.1.3 — fnmatch.
        let mut root = WebDdsRoot::default();
        root.create_application(app("a1")).expect("ok");
        root.create_application(app("a2")).expect("ok");
        let all = root.get_applications("*");
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn get_applications_with_prefix_pattern_matches_subset() {
        let mut root = WebDdsRoot::default();
        root.create_application(app("temp_sensor")).expect("ok");
        root.create_application(app("temp_actuator")).expect("ok");
        root.create_application(app("pressure_sensor")).expect("ok");
        let temps = root.get_applications("temp_*");
        assert_eq!(temps.len(), 2);
    }

    #[test]
    fn get_applications_with_exact_match() {
        let mut root = WebDdsRoot::default();
        root.create_application(app("exactly")).expect("ok");
        root.create_application(app("else")).expect("ok");
        let one = root.get_applications("exactly");
        assert_eq!(one.len(), 1);
        assert_eq!(one[0].name, "exactly");
    }

    #[test]
    fn application_carries_participants() {
        let a = Application {
            name: String::from("trader"),
            participants: alloc::vec![
                DomainParticipant {
                    name: String::from("p0"),
                    domain_id: 0,
                },
                DomainParticipant {
                    name: String::from("p1"),
                    domain_id: 1,
                },
            ],
        };
        assert_eq!(a.participants.len(), 2);
        assert_eq!(a.participants[0].domain_id, 0);
    }

    #[test]
    fn root_starts_empty_and_is_the_only_container() {
        // Spec §7.2: `WebDDS::Root` is a singleton — every other
        // object of the model is reached through it, so a fresh root
        // holds nothing and nothing exists outside it.
        let root = WebDdsRoot::default();
        assert!(root.applications.is_empty());
        assert!(root.clients.is_empty());
        assert!(root.get_applications("*").is_empty());
    }

    #[test]
    fn every_model_object_is_reachable_from_the_root() {
        // Spec §7.1/§7.2: the object model is a containment tree
        // rooted in `WebDDS::Root` — Application below Root,
        // DomainParticipant below Application, Client below Root.
        let mut root = WebDdsRoot::default();
        root.create_application(Application {
            name: String::from("trader"),
            participants: alloc::vec![DomainParticipant {
                name: String::from("p0"),
                domain_id: 7,
            }],
        })
        .expect("created");
        root.clients.push(Client {
            client_api_key: String::from("key-1"),
        });

        let apps = root.get_applications("*");
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, "trader");
        assert_eq!(apps[0].participants[0].domain_id, 7);
        assert_eq!(root.clients[0].client_api_key, "key-1");

        // Deleting through the root removes the subtree with it.
        root.delete_application("trader").expect("deleted");
        assert!(root.get_applications("*").is_empty());
    }

    #[test]
    fn root_holds_one_entry_per_application_name() {
        // Spec §7.2: the singleton root is the uniqueness scope —
        // two applications cannot share a name.
        let mut root = WebDdsRoot::default();
        let first = root.create_application(app("a")).expect("first");
        assert_eq!(
            root.create_application(app("a")),
            Err(ReturnStatus::ObjectAlreadyExists)
        );
        let second = root
            .create_application(app("b"))
            .expect("a second name is fine");
        assert_ne!(first, second, "each application gets its own session");
        assert_eq!(root.applications.len(), 2);
    }
}
