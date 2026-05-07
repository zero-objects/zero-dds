// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Address-Resolution + Wildcard-Routing.
//!
//! Spec-Quelle: dds-amqp-1.0-beta1.pdf §7.3 Address-Resolution-Model.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Spec §7.3 — eine aufgeloeste Address-Konfiguration.
///
/// `partitions` ist eine Sequenz (Spec §7.4.8: DDS PARTITION ist
/// `sequence<string>`). Mehrfach-`?partition=`-Query-Params in der
/// URL werden in der Reihenfolge des Auftretens als Listen-Elemente
/// uebernommen.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AddressResolution {
    /// DDS-Topic-Name.
    pub topic: String,
    /// DDS-Domain-Id (default 0).
    pub domain_id: u32,
    /// DDS-Partition-Sequenz (leer = Default-Partition).
    pub partitions: Vec<String>,
}

impl AddressResolution {
    /// Convenience: erste Partition oder leerer String (fuer
    /// Single-Partition-Code-Pfade).
    #[must_use]
    pub fn first_partition(&self) -> &str {
        self.partitions.first().map(String::as_str).unwrap_or("")
    }
}

/// Routing-Tabelle.
#[derive(Debug, Clone, Default)]
pub struct AddressRouter {
    entries: Vec<RouteEntry>,
}

#[derive(Debug, Clone)]
struct RouteEntry {
    pattern: String,
    target: AddressResolution,
}

/// Resolution-Fehler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolutionError {
    /// Keine matchende Route.
    NoRoute(String),
    /// Address-String konnte nicht geparsed werden.
    Malformed(String),
}

impl AddressRouter {
    /// Neuer leerer Router.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Statisches Mapping ergaenzen.
    pub fn add_route(&mut self, pattern: &str, target: AddressResolution) {
        self.entries.push(RouteEntry {
            pattern: pattern.to_string(),
            target,
        });
    }

    /// Spec §7.3 — eine eingehende Adresse aufloesen.
    ///
    /// Reihenfolge:
    /// 1. Static Aliases (erste matchende Pattern wins).
    /// 2. Bare Address (`tracking`) -> Topic gleichen Namens, Domain 0.
    /// 3. `domain://N/topic[?partition=P]`-URL parsen.
    ///
    /// # Errors
    /// `Malformed` bei syntaktisch ungueltigem Address-String,
    /// `NoRoute` wenn kein statisches Mapping matched und keine
    /// implizite Form anwendbar ist.
    pub fn resolve(&self, address: &str) -> Result<AddressResolution, ResolutionError> {
        // 1. Static aliases.
        for e in &self.entries {
            if match_pattern(&e.pattern, address) {
                return Ok(e.target.clone());
            }
        }
        // 2. domain:// URL.
        if let Some(rest) = address.strip_prefix("domain://") {
            return parse_domain_url(rest);
        }
        // 3. Bare address.
        if !address.is_empty()
            && !address.contains('?')
            && address
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
        {
            return Ok(AddressResolution {
                topic: address.to_string(),
                domain_id: 0,
                partitions: Vec::new(),
            });
        }
        Err(ResolutionError::NoRoute(address.to_string()))
    }
}

/// Spec §7.8 — Konflikt-Resolution zwischen URI-Form
/// (`?partition=`) und Application-Property `dds:partition`.
///
/// Wenn die URI eine nicht-leere Partition-Liste fuehrt, gewinnt
/// die URI-Form ueber die Property; sonst greift die Property.
/// Liefert die effektive Partition-Sequenz fuer das Routing.
#[must_use]
pub fn effective_partitions(uri_form: &[String], property_form: &[String]) -> Vec<String> {
    if uri_form.is_empty() {
        property_form.to_vec()
    } else {
        uri_form.to_vec()
    }
}

fn match_pattern(pattern: &str, addr: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(rest) = pattern.strip_prefix('*') {
        return addr.ends_with(rest);
    }
    if let Some(rest) = pattern.strip_suffix('*') {
        return addr.starts_with(rest);
    }
    pattern == addr
}

fn parse_domain_url(rest: &str) -> Result<AddressResolution, ResolutionError> {
    // Format: <id>/<topic>[?partition=<p>(&partition=<p>)*]
    // Spec §7.4.8: PARTITION ist sequence<string>; mehrfach
    // wiederholte ?partition=-Params sind erlaubt.
    let (id_part, after_id) = rest
        .split_once('/')
        .ok_or_else(|| ResolutionError::Malformed(rest.to_string()))?;
    let domain_id = id_part
        .parse::<u32>()
        .map_err(|_| ResolutionError::Malformed(rest.to_string()))?;
    let (topic, partitions) = if let Some((t, query)) = after_id.split_once('?') {
        let mut parts = Vec::new();
        for kv in query.split('&') {
            let raw = kv
                .strip_prefix("partition=")
                .ok_or_else(|| ResolutionError::Malformed(rest.to_string()))?;
            parts.push(percent_decode(raw)?);
        }
        (t.to_string(), parts)
    } else {
        (after_id.to_string(), Vec::new())
    };
    Ok(AddressResolution {
        topic,
        domain_id,
        partitions,
    })
}

/// RFC 3986 percent-decoding fuer Query-String-Values.
fn percent_decode(s: &str) -> Result<String, ResolutionError> {
    let mut out = Vec::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len() {
                return Err(ResolutionError::Malformed(s.to_string()));
            }
            let hi =
                hex_digit(bytes[i + 1]).ok_or_else(|| ResolutionError::Malformed(s.to_string()))?;
            let lo =
                hex_digit(bytes[i + 2]).ok_or_else(|| ResolutionError::Malformed(s.to_string()))?;
            out.push((hi << 4) | lo);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).map_err(|_| ResolutionError::Malformed(s.to_string()))
}

fn hex_digit(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn bare_address_resolves_to_default_domain() {
        let r = AddressRouter::new();
        let res = r.resolve("tracking").expect("resolve");
        assert_eq!(res.topic, "tracking");
        assert_eq!(res.domain_id, 0);
        assert!(res.partitions.is_empty());
    }

    #[test]
    fn domain_url_parses_id_and_topic() {
        let r = AddressRouter::new();
        let res = r.resolve("domain://7/Sensor").expect("resolve");
        assert_eq!(res.domain_id, 7);
        assert_eq!(res.topic, "Sensor");
    }

    #[test]
    fn domain_url_with_single_partition_parses() {
        let r = AddressRouter::new();
        let res = r
            .resolve("domain://3/Topic?partition=zone-a")
            .expect("resolve");
        assert_eq!(res.partitions, alloc::vec!["zone-a".to_string()]);
        assert_eq!(res.first_partition(), "zone-a");
    }

    #[test]
    fn domain_url_with_multi_partition_parses() {
        let r = AddressRouter::new();
        let res = r
            .resolve("domain://3/Topic?partition=alpha&partition=beta&partition=gamma")
            .expect("resolve");
        assert_eq!(
            res.partitions,
            alloc::vec!["alpha".to_string(), "beta".to_string(), "gamma".to_string()]
        );
    }

    #[test]
    fn domain_url_with_percent_encoded_partition() {
        let r = AddressRouter::new();
        let res = r
            .resolve("domain://0/Topic?partition=zone%2Fa")
            .expect("resolve");
        assert_eq!(res.partitions, alloc::vec!["zone/a".to_string()]);
    }

    #[test]
    fn static_alias_overrides_implicit_resolution() {
        let mut r = AddressRouter::new();
        r.add_route(
            "alias",
            AddressResolution {
                topic: "Real".to_string(),
                domain_id: 5,
                partitions: Vec::new(),
            },
        );
        let res = r.resolve("alias").expect("resolve");
        assert_eq!(res.topic, "Real");
        assert_eq!(res.domain_id, 5);
    }

    #[test]
    fn wildcard_alias_matches() {
        let mut r = AddressRouter::new();
        r.add_route(
            "sensor.*",
            AddressResolution {
                topic: "AllSensors".to_string(),
                domain_id: 0,
                partitions: Vec::new(),
            },
        );
        assert_eq!(
            r.resolve("sensor.temperature").expect("resolve").topic,
            "AllSensors"
        );
    }

    #[test]
    fn malformed_domain_url_yields_error() {
        let r = AddressRouter::new();
        assert!(matches!(
            r.resolve("domain://not-a-number/Topic"),
            Err(ResolutionError::Malformed(_))
        ));
    }

    #[test]
    fn empty_address_yields_no_route() {
        let r = AddressRouter::new();
        assert!(matches!(r.resolve(""), Err(ResolutionError::NoRoute(_))));
    }

    #[test]
    fn conflict_resolution_uri_wins_when_both_present() {
        // Spec §7.8: URI-Form gewinnt gegen App-Property.
        let uri = alloc::vec!["uri-a".to_string(), "uri-b".to_string()];
        let prop = alloc::vec!["prop-only".to_string()];
        assert_eq!(effective_partitions(&uri, &prop), uri);
    }

    #[test]
    fn conflict_resolution_property_used_when_uri_empty() {
        let uri: alloc::vec::Vec<String> = alloc::vec::Vec::new();
        let prop = alloc::vec!["from-prop".to_string()];
        assert_eq!(effective_partitions(&uri, &prop), prop);
    }

    #[test]
    fn conflict_resolution_both_empty() {
        let uri: alloc::vec::Vec<String> = alloc::vec::Vec::new();
        let prop: alloc::vec::Vec<String> = alloc::vec::Vec::new();
        assert!(effective_partitions(&uri, &prop).is_empty());
    }
}
