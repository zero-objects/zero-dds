// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! A small flat-field reader for request bodies whose shape is a
//! WebDDS object representation (Spec §8.3.4 Tab 6) — `<topic>`,
//! `<data_writer>`, `<waitset>`, ... — rather than a sample (which
//! `crate::dynxml` converts through the type's own descriptor). These
//! bodies carry a handful of named scalar fields (and, for a waitset,
//! one repeated list), so a full recursive `DynamicType`-driven decode
//! is not needed — the field names are the ones Spec §8.3.4/§7.4 fix,
//! not a user type's.

use zerodds_xml_wire::parser::{Event, XmlParser};

/// One parsed object body: every top-level (and one-level-nested, for
/// the `readers` list a waitset body carries) scalar field, by tag
/// name. Later duplicates of the same tag overwrite earlier ones,
/// except entries collected under `list_field`.
#[derive(Debug, Clone, Default)]
pub struct FlatFields {
    scalars: std::collections::BTreeMap<String, String>,
    lists: std::collections::BTreeMap<String, Vec<String>>,
}

impl FlatFields {
    /// A scalar field's text, if present.
    #[must_use]
    pub fn get(&self, tag: &str) -> Option<&str> {
        self.scalars.get(tag).map(String::as_str)
    }

    /// The `<item>` texts of a `<tag><item>.../item>...</tag>` list
    /// field.
    #[must_use]
    pub fn list(&self, tag: &str) -> Vec<String> {
        self.lists.get(tag).cloned().unwrap_or_default()
    }
}

/// Parse error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError(pub String);

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "malformed object body: {}", self.0)
    }
}

impl std::error::Error for ParseError {}

/// Parses `xml` as a flat object body: the root element's direct
/// children are scalar fields (by tag/text), except a child that is
/// itself a `<tag><item>...</item>...</tag>` list, which becomes a
/// `list_field`.
///
/// # Errors
/// [`ParseError`] on invalid XML.
pub fn parse(xml: &str) -> Result<FlatFields, ParseError> {
    let mut out = FlatFields::default();
    // depth 0 = before root; 1 = inside root (fields live here); 2 =
    // inside a field that turned out to be a list (its <item>s).
    let mut depth: u32 = 0;
    let mut current_tag: Option<String> = None;
    let mut current_text = String::new();
    let mut current_is_list = false;
    for event in XmlParser::new(xml) {
        let event = event.map_err(|e| ParseError(e.to_string()))?;
        match event {
            Event::Declaration(_) | Event::CData(_) => {}
            Event::StartElement { name, .. } => {
                depth += 1;
                if depth == 2 {
                    current_tag = Some(name);
                    current_text.clear();
                    current_is_list = false;
                } else if depth == 3 && current_tag.is_some() {
                    current_is_list = true;
                    current_text.clear();
                }
            }
            Event::Text(text) => {
                if depth >= 2 {
                    current_text.push_str(text.trim());
                }
            }
            Event::EndElement(name) => {
                if depth == 3 {
                    if let Some(tag) = &current_tag {
                        out.lists
                            .entry(tag.clone())
                            .or_default()
                            .push(current_text.trim().to_string());
                    }
                    current_text.clear();
                } else if depth == 2 {
                    if !current_is_list {
                        if let Some(tag) = current_tag.take() {
                            out.scalars.insert(tag, current_text.trim().to_string());
                        }
                    }
                    current_tag = None;
                    current_text.clear();
                    current_is_list = false;
                }
                let _ = &name;
                depth = depth.saturating_sub(1);
            }
        }
    }
    Ok(out)
}

/// The first direct child of the root named `tag`, as XML of its own —
/// for a field whose value is a subtree, such as a SOAP `WriteSample`'s
/// `<data>` holding a sample in the REST representation. `None` when the
/// root has no such child.
///
/// # Errors
/// [`ParseError`] on invalid XML.
pub fn child_xml(xml: &str, tag: &str) -> Result<Option<String>, ParseError> {
    use zerodds_xml_wire::emitter::XmlEmitter;
    let mut out = XmlEmitter::new();
    let mut depth: u32 = 0;
    let mut inside = false;
    for event in XmlParser::new(xml) {
        let event = event.map_err(|e| ParseError(e.to_string()))?;
        match event {
            Event::Declaration(_) => {}
            Event::StartElement { name, attrs } => {
                depth += 1;
                if depth == 2 && name == tag {
                    inside = true;
                }
                if inside {
                    let attrs: Vec<(&str, &str)> = attrs
                        .iter()
                        .map(|(k, v)| (k.as_str(), v.as_str()))
                        .collect();
                    out.start_element(&name, &attrs)
                        .map_err(|e| ParseError(format!("{e:?}")))?;
                }
            }
            Event::Text(text) if inside => out.text(&text),
            Event::CData(text) if inside => out.cdata(&text),
            Event::Text(_) | Event::CData(_) => {}
            Event::EndElement(_) => {
                if inside {
                    out.end_element()
                        .map_err(|e| ParseError(format!("{e:?}")))?;
                    if depth == 2 {
                        return Ok(Some(out.finish()));
                    }
                }
                depth = depth.saturating_sub(1);
            }
        }
    }
    Ok(None)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn scalar_fields_are_read() {
        let f =
            parse("<topic><name>Sensor</name><type_name>M::Sensor</type_name></topic>").unwrap();
        assert_eq!(f.get("name"), Some("Sensor"));
        assert_eq!(f.get("type_name"), Some("M::Sensor"));
    }

    #[test]
    fn list_field_collects_items() {
        let f = parse(
            "<waitset><name>ws1</name><readers><item>a</item><item>b</item></readers></waitset>",
        )
        .unwrap();
        assert_eq!(f.get("name"), Some("ws1"));
        assert_eq!(f.list("readers"), vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn missing_field_is_none() {
        let f = parse("<topic><name>Sensor</name></topic>").unwrap();
        assert_eq!(f.get("type_name"), None);
    }

    #[test]
    fn a_child_subtree_comes_back_as_xml() {
        let body = "<WriteSample><writer>dw1</writer>\
                    <data><sensor_id>6</sensor_id><name>a &amp; b</name></data></WriteSample>";
        assert_eq!(
            child_xml(body, "data").unwrap().as_deref(),
            Some("<data><sensor_id>6</sensor_id><name>a &amp; b</name></data>")
        );
        assert_eq!(
            child_xml(body, "sensor_id").unwrap(),
            None,
            "not a direct child"
        );
        assert_eq!(child_xml(body, "missing").unwrap(), None);
    }

    #[test]
    fn malformed_xml_is_an_error() {
        assert!(parse("<topic><name>Sensor</topic>").is_err());
    }
}
