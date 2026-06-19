//! `cert.d` loader for inspect-endpoint auth (R-100..R-104).
//!
//! Self-signed X.509 PEM certs in a folder. Boot-up only — no
//! live watch (R-103). One "access" level per cert (R-102).

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// A loaded cert identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoadedCert {
    /// Source file (relative to the cert.d folder).
    pub source: PathBuf,
    /// SHA-256 fingerprint of the PEM content (32 bytes).
    pub fingerprint: [u8; 32],
    /// Raw PEM content (for the mTLS comparison).
    pub pem: Vec<u8>,
}

/// Result of a `cert.d` loader.
#[derive(Clone, Debug, Default)]
pub struct CertSet {
    /// Loaded certs.
    pub certs: Vec<LoadedCert>,
}

impl CertSet {
    /// Checks whether a given fingerprint is in this set.
    #[must_use]
    pub fn contains_fingerprint(&self, fp: &[u8; 32]) -> bool {
        self.certs.iter().any(|c| &c.fingerprint == fp)
    }

    /// Number of certs.
    #[must_use]
    pub fn len(&self) -> usize {
        self.certs.len()
    }

    /// Set empty?
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.certs.is_empty()
    }
}

/// Loads all PEM files from a `cert.d` folder. Boot-up only,
/// alphanumerically sorted (analogous to the PDE config.d convention).
///
/// # Errors
///
/// Returns `Err` if the directory is not readable. Individual
/// non-PEM files are ignored (not an error).
pub fn load_cert_dir(dir: &Path) -> std::io::Result<CertSet> {
    let mut set = CertSet::default();
    if !dir.exists() {
        return Ok(set);
    }
    let mut entries: Vec<_> = std::fs::read_dir(dir)?
        .filter_map(std::result::Result::ok)
        .collect();
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        if path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
            != Some("pem")
        {
            continue;
        }
        let pem = match std::fs::read(&path) {
            Ok(b) => b,
            Err(_) => continue,
        };
        if !looks_like_pem(&pem) {
            continue;
        }
        let mut hasher = Sha256::new();
        hasher.update(&pem);
        let fingerprint: [u8; 32] = hasher.finalize().into();
        let rel = path.strip_prefix(dir).unwrap_or(&path).to_path_buf();
        set.certs.push(LoadedCert {
            source: rel,
            fingerprint,
            pem,
        });
    }
    Ok(set)
}

fn looks_like_pem(bytes: &[u8]) -> bool {
    let needle = b"-----BEGIN ";
    bytes.windows(needle.len()).any(|w| w == needle)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn fake_pem(payload: &str) -> String {
        format!("-----BEGIN CERTIFICATE-----\n{payload}\n-----END CERTIFICATE-----\n")
    }

    #[test]
    fn missing_dir_returns_empty_set() {
        let s = load_cert_dir(Path::new("/no-such-path-pde-test")).expect("ok empty");
        assert!(s.is_empty());
    }

    #[test]
    fn loads_single_pem_file() {
        let dir = tempdir().expect("tempdir");
        fs::write(dir.path().join("alice.pem"), fake_pem("ALICE")).expect("write");
        let set = load_cert_dir(dir.path()).expect("load");
        assert_eq!(set.len(), 1);
        assert_eq!(set.certs[0].source, PathBuf::from("alice.pem"));
    }

    #[test]
    fn loads_multiple_pems_alphanumeric() {
        let dir = tempdir().expect("tempdir");
        fs::write(dir.path().join("b.pem"), fake_pem("B")).expect("write");
        fs::write(dir.path().join("a.pem"), fake_pem("A")).expect("write");
        let set = load_cert_dir(dir.path()).expect("load");
        assert_eq!(set.len(), 2);
        let names: Vec<&str> = set.certs.iter().filter_map(|c| c.source.to_str()).collect();
        assert_eq!(names, vec!["a.pem", "b.pem"]);
    }

    #[test]
    fn ignores_non_pem_files() {
        let dir = tempdir().expect("tempdir");
        fs::write(dir.path().join("notes.txt"), "not a cert").expect("write");
        fs::write(dir.path().join("malformed.pem"), "not pem either").expect("write");
        let set = load_cert_dir(dir.path()).expect("load");
        assert_eq!(set.len(), 0);
    }

    #[test]
    fn fingerprints_are_distinct_for_different_certs() {
        let dir = tempdir().expect("tempdir");
        fs::write(dir.path().join("a.pem"), fake_pem("DATA-A")).expect("write");
        fs::write(dir.path().join("b.pem"), fake_pem("DATA-B")).expect("write");
        let set = load_cert_dir(dir.path()).expect("load");
        assert_eq!(set.len(), 2);
        assert_ne!(set.certs[0].fingerprint, set.certs[1].fingerprint);
    }

    #[test]
    fn contains_fingerprint_finds_match() {
        let dir = tempdir().expect("tempdir");
        fs::write(dir.path().join("a.pem"), fake_pem("X")).expect("write");
        let set = load_cert_dir(dir.path()).expect("load");
        let fp = set.certs[0].fingerprint;
        assert!(set.contains_fingerprint(&fp));
        let other = [0u8; 32];
        assert!(!set.contains_fingerprint(&other));
    }
}
