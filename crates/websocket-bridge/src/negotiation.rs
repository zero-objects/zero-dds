// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Generic Extension- und Subprotocol-Negotiation nach RFC 6455 §9.
//!
//! Neben permessage-deflate (RFC 7692, siehe `permessage_deflate.rs`)
//! braucht der Handshake-Layer Hilfen fuer:
//!
//! - **Sec-WebSocket-Extensions** generisches Parsing einer
//!   Extension-Liste mit Parametern (z.B. `foo; bar=baz, qux`).
//! - **Sec-WebSocket-Protocol** Subprotocol-Negotiation (Server
//!   waehlt eines der vom Client angebotenen Subprotokolle).

use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Eine geparste Extension mit optionalen Parametern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionOffer {
    /// Extension-Name (token, case-insensitive).
    pub name: String,
    /// Liste der `name=value`-Parameter; `value=None` entspricht
    /// einem Boolean-Parameter (nur Name).
    pub params: Vec<(String, Option<String>)>,
}

/// Parst den Wert eines `Sec-WebSocket-Extensions`-Headers in eine
/// Liste von `ExtensionOffer`s.
///
/// Beispiel: `permessage-deflate; client_max_window_bits, foo`
/// → 2 Eintraege, der erste mit Parameter `client_max_window_bits`
/// (kein Wert), der zweite ohne Parameter.
#[must_use]
pub fn parse_extensions(header: &str) -> Vec<ExtensionOffer> {
    let mut offers = Vec::new();
    for raw in header.split(',') {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        let mut parts = trimmed.split(';').map(str::trim);
        let Some(name) = parts.next() else {
            continue;
        };
        if name.is_empty() {
            continue;
        }
        let mut params = Vec::new();
        for p in parts {
            if p.is_empty() {
                continue;
            }
            if let Some(eq_pos) = p.find('=') {
                let k = p[..eq_pos].trim().to_string();
                let v = p[eq_pos + 1..].trim().trim_matches('"').to_string();
                params.push((k, Some(v)));
            } else {
                params.push((p.to_string(), None));
            }
        }
        offers.push(ExtensionOffer {
            name: name.to_string(),
            params,
        });
    }
    offers
}

/// Parst den Wert eines `Sec-WebSocket-Protocol`-Headers in eine
/// Liste von Subprotocol-Tokens.
#[must_use]
pub fn parse_subprotocols(header: &str) -> Vec<String> {
    header
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Server-Pfad: waehlt das erste vom Client angebotene Subprotocol,
/// das in der Server-Preferenzliste vorkommt. Liefert `None` wenn
/// keine Schnittmenge.
#[must_use]
pub fn select_subprotocol(client_offered: &[String], server_preferred: &[&str]) -> Option<String> {
    for offer in client_offered {
        for pref in server_preferred {
            if offer.eq_ignore_ascii_case(pref) {
                return Some(offer.clone());
            }
        }
    }
    None
}

/// Spec §4.2.2 — Spec-konformer Default-Header-Name fuer Subprotocol.
pub const SUBPROTOCOL_HEADER: &str = "Sec-WebSocket-Protocol";

/// Spec §4.2.1 — Spec-konformer Default-Header-Name fuer Extensions.
pub const EXTENSIONS_HEADER: &str = "Sec-WebSocket-Extensions";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_extensions_single() {
        let offers = parse_extensions("permessage-deflate");
        assert_eq!(offers.len(), 1);
        assert_eq!(offers[0].name, "permessage-deflate");
        assert!(offers[0].params.is_empty());
    }

    #[test]
    fn parse_extensions_with_param() {
        let offers = parse_extensions("permessage-deflate; client_max_window_bits");
        assert_eq!(offers.len(), 1);
        assert_eq!(offers[0].params.len(), 1);
        assert_eq!(offers[0].params[0], ("client_max_window_bits".into(), None));
    }

    #[test]
    fn parse_extensions_with_value_param() {
        let offers = parse_extensions("foo; bar=baz");
        assert_eq!(offers[0].params[0].0, "bar");
        assert_eq!(offers[0].params[0].1.as_deref(), Some("baz"));
    }

    #[test]
    fn parse_extensions_strips_quoted_value() {
        let offers = parse_extensions("foo; bar=\"baz\"");
        assert_eq!(offers[0].params[0].1.as_deref(), Some("baz"));
    }

    #[test]
    fn parse_extensions_multiple_offers() {
        let offers = parse_extensions("foo, bar; x=1, baz");
        assert_eq!(offers.len(), 3);
        assert_eq!(offers[0].name, "foo");
        assert_eq!(offers[1].name, "bar");
        assert_eq!(offers[2].name, "baz");
    }

    #[test]
    fn parse_extensions_empty_returns_empty() {
        assert!(parse_extensions("").is_empty());
        assert!(parse_extensions(" , ").is_empty());
    }

    #[test]
    fn parse_subprotocols_basic() {
        assert_eq!(
            parse_subprotocols("chat, soap, mqtt"),
            vec!["chat".to_string(), "soap".into(), "mqtt".into()]
        );
    }

    #[test]
    fn parse_subprotocols_empty_returns_empty() {
        assert!(parse_subprotocols("").is_empty());
    }

    #[test]
    fn select_subprotocol_picks_first_match() {
        let client = vec!["soap".to_string(), "chat".into()];
        let server = ["mqtt", "chat", "soap"];
        assert_eq!(
            select_subprotocol(&client, &server).as_deref(),
            Some("soap")
        );
    }

    #[test]
    fn select_subprotocol_returns_none_when_no_match() {
        let client = vec!["xmpp".to_string()];
        let server = ["chat", "mqtt"];
        assert!(select_subprotocol(&client, &server).is_none());
    }

    #[test]
    fn select_subprotocol_is_case_insensitive() {
        let client = vec!["CHAT".to_string()];
        let server = ["chat"];
        assert_eq!(
            select_subprotocol(&client, &server).as_deref(),
            Some("CHAT")
        );
    }

    #[test]
    fn header_constants_match_spec() {
        assert_eq!(SUBPROTOCOL_HEADER, "Sec-WebSocket-Protocol");
        assert_eq!(EXTENSIONS_HEADER, "Sec-WebSocket-Extensions");
    }
}
