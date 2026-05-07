// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! RepositoryManager — D&C §8.
//!
//! Spec §8.1.1: `RepositoryManager` haelt die `PackageConfiguration`-
//! Bindings im System. Deployers laden Packages hinein, Executors
//! konsumieren sie.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use crate::plan::PackageConfiguration;

/// In-Memory-RepositoryManager — D&C §8.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RepositoryManager {
    packages: BTreeMap<String, PackageConfiguration>,
}

impl RepositoryManager {
    /// Konstruktor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Spec §8.1.2 `installPackage` — registriert ein Package unter
    /// seinem `package.label`. Ueberschreibt bei Duplikat.
    pub fn install_package(&mut self, pkg: PackageConfiguration) {
        let key = pkg.label.clone();
        self.packages.insert(key, pkg);
    }

    /// Spec §8.1.3 `findPackageByName` — sucht ein Package per Label.
    #[must_use]
    pub fn find_package(&self, label: &str) -> Option<&PackageConfiguration> {
        self.packages.get(label)
    }

    /// Spec §8.1.4 `deletePackage` — entfernt ein Package; gibt
    /// `false` wenn nicht gefunden.
    pub fn delete_package(&mut self, label: &str) -> bool {
        self.packages.remove(label).is_some()
    }

    /// Spec §8.1.5 `getAllPackageNames` — Liste aller Package-Labels.
    #[must_use]
    pub fn list_packages(&self) -> Vec<String> {
        self.packages.keys().cloned().collect()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::plan::ComponentPackageDescription;
    use alloc::string::ToString;

    fn make_pkg(label: &str) -> PackageConfiguration {
        PackageConfiguration {
            label: label.into(),
            uuid: alloc::format!("uuid-{label}"),
            package: ComponentPackageDescription {
                label: label.into(),
                ..ComponentPackageDescription::default()
            },
            selected_implementation: 0,
        }
    }

    #[test]
    fn install_and_find_round_trip() {
        let mut r = RepositoryManager::new();
        r.install_package(make_pkg("Echo"));
        assert!(r.find_package("Echo").is_some());
        assert_eq!(r.find_package("NoSuch"), None);
    }

    #[test]
    fn list_packages_returns_all_labels() {
        let mut r = RepositoryManager::new();
        r.install_package(make_pkg("A"));
        r.install_package(make_pkg("B"));
        let mut names = r.list_packages();
        names.sort();
        assert_eq!(names, alloc::vec!["A".to_string(), "B".to_string()]);
    }

    #[test]
    fn delete_existing_returns_true() {
        let mut r = RepositoryManager::new();
        r.install_package(make_pkg("X"));
        assert!(r.delete_package("X"));
        assert!(r.find_package("X").is_none());
    }

    #[test]
    fn delete_unknown_returns_false() {
        let mut r = RepositoryManager::new();
        assert!(!r.delete_package("X"));
    }

    #[test]
    fn install_overwrites_duplicate() {
        let mut r = RepositoryManager::new();
        let mut pkg = make_pkg("Echo");
        pkg.uuid = "first".into();
        r.install_package(pkg);
        let mut pkg2 = make_pkg("Echo");
        pkg2.uuid = "second".into();
        r.install_package(pkg2);
        assert_eq!(r.find_package("Echo").unwrap().uuid, "second");
    }
}
