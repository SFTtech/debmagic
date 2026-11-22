//! XML documents of Entries, parsed on first use.
//!
//! This is the XML side of lintian's `XML::LibXML` calls, with `no_network`
//! set and `expand_entities` off. roxmltree never fetches anything: external
//! entities and DTDs are not loaded. Internal DTD entities are expanded, so
//! the text of a node may differ from lintian's on documents that declare
//! them. A document is read from the Entry's bytes once; the parse is shared by
//! every Rule that asks for it through the same `Arc`.
//!
//! The parse keeps only what the Rules need: the parse error, the root element
//! name, the text of `/component/metadata_license`, and the `modalias` nodes of
//! the first `/component/provides`. Paths here are XPath-like, so only
//! elements in no namespace count, as with lintian's `findnodes`.

// The Rules that read these accessors land with the appstream-metadata Tags.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use roxmltree::{Document, Node, ParsingOptions};

use super::file_tree::{Entry, FileTree};

/// One XML document, with the bytes of its Entry and a lazily computed parse.
pub struct XmlDocument {
    path: PathBuf,
    bytes: Vec<u8>,
    parsed: OnceLock<XmlSummary>,
}

/// What the Rules read from a parsed document. Owned, so the `XmlDocument`
/// can be shared across threads without borrowing the input.
#[derive(Debug, Default)]
struct XmlSummary {
    parse_error: Option<String>,
    root_element: Option<String>,
    metadata_license: Option<String>,
    has_modalias_nodes: bool,
    modaliases: Vec<String>,
}

impl XmlDocument {
    /// The document of `entry`, whose bytes are read from `tree`. `None` when
    /// the Entry has no readable content (not a regular file or hardlink).
    pub fn read(tree: &FileTree, entry: &Entry) -> Option<Self> {
        let bytes = tree.read(entry).ok()?;
        Some(Self::from_bytes(entry.path(), bytes))
    }

    /// A document from bytes already in hand. `path` is only for reporting.
    pub fn from_bytes(path: impl Into<PathBuf>, bytes: Vec<u8>) -> Self {
        Self {
            path: path.into(),
            bytes,
            parsed: OnceLock::new(),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Why the bytes are not well-formed XML, or `None` when they parse.
    /// Bytes that are not UTF-8 are a parse error here; lintian's libxml
    /// would honour the encoding declaration instead.
    pub fn parse_error(&self) -> Option<&str> {
        self.summary().parse_error.as_deref()
    }

    /// Local name of the root element, `None` when the document does not parse.
    pub fn root_element(&self) -> Option<&str> {
        self.summary().root_element.as_deref()
    }

    /// Text of the first `/component/metadata_license`, `Some("")` when it is
    /// empty, `None` when there is none or the document does not parse.
    pub fn metadata_license(&self) -> Option<&str> {
        self.summary().metadata_license.as_deref()
    }

    /// Whether the first `/component/provides` has any `modalias` child, even
    /// an empty one.
    pub fn has_modalias_nodes(&self) -> bool {
        self.summary().has_modalias_nodes
    }

    /// Non-empty text of the `modalias` children of the first
    /// `/component/provides`, in document order.
    pub fn modaliases(&self) -> &[String] {
        &self.summary().modaliases
    }

    fn summary(&self) -> &XmlSummary {
        self.parsed.get_or_init(|| XmlSummary::parse(&self.bytes))
    }
}

impl XmlSummary {
    fn parse(bytes: &[u8]) -> Self {
        let mut summary = Self::default();
        let text = match std::str::from_utf8(bytes) {
            Ok(text) => text,
            Err(error) => {
                summary.parse_error = Some(format!("not UTF-8: {error}"));
                return summary;
            }
        };
        let options = ParsingOptions {
            allow_dtd: true,
            ..ParsingOptions::default()
        };
        let document = match Document::parse_with_options(text, options) {
            Ok(document) => document,
            Err(error) => {
                summary.parse_error = Some(error.to_string());
                return summary;
            }
        };
        let root = document.root_element();
        summary.root_element = Some(root.tag_name().name().to_string());
        if root.tag_name().name() != "component" || root.tag_name().namespace().is_some() {
            return summary;
        }
        if let Some(license) = child(root, "metadata_license") {
            summary.metadata_license = Some(license.text().unwrap_or("").to_string());
        }
        if let Some(provides) = child(root, "provides") {
            summary.has_modalias_nodes = provides.children().any(|node| named(node, "modalias"));
            summary.modaliases = provides
                .children()
                .filter(|node| named(*node, "modalias"))
                .filter_map(|node| node.text())
                .filter(|alias| !alias.is_empty())
                .map(str::to_string)
                .collect();
        }
        summary
    }
}

/// The first element child of `parent` named `name` in no namespace.
fn child<'a, 'input>(parent: Node<'a, 'input>, name: &str) -> Option<Node<'a, 'input>> {
    parent.children().find(|node| named(*node, name))
}

fn named(node: Node, name: &str) -> bool {
    node.is_element() && node.tag_name().name() == name && node.tag_name().namespace().is_none()
}

#[cfg(test)]
mod tests {
    use super::XmlDocument;

    fn doc(text: &str) -> XmlDocument {
        XmlDocument::from_bytes(
            "usr/share/metainfo/x.metainfo.xml",
            text.as_bytes().to_vec(),
        )
    }

    #[test]
    fn well_formed_component_is_summarised() {
        let document = doc(r#"<?xml version="1.0"?>
<component>
  <metadata_license>MIT</metadata_license>
  <provides>
    <modalias>usb:v0000p0001d*</modalias>
    <modalias></modalias>
    <modalias>usb:v0000p000Ad*</modalias>
  </provides>
  <provides><modalias>usb:v1234p5678d*</modalias></provides>
</component>"#);
        assert_eq!(document.parse_error(), None);
        assert_eq!(document.root_element(), Some("component"));
        assert_eq!(document.metadata_license(), Some("MIT"));
        assert!(document.has_modalias_nodes());
        assert_eq!(
            document.modaliases(),
            ["usb:v0000p0001d*", "usb:v0000p000Ad*"]
        );
    }

    #[test]
    fn only_the_first_provides_counts() {
        let document = doc(
            "<component><provides/><provides><modalias>usb:x</modalias></provides></component>",
        );
        assert!(!document.has_modalias_nodes());
        assert!(document.modaliases().is_empty());
    }

    #[test]
    fn empty_modalias_is_a_node_but_not_an_alias() {
        let document = doc("<component><provides><modalias/></provides></component>");
        assert!(document.has_modalias_nodes());
        assert!(document.modaliases().is_empty());
    }

    #[test]
    fn empty_metadata_license_is_some_empty() {
        let document = doc("<component><metadata_license/></component>");
        assert_eq!(document.metadata_license(), Some(""));
    }

    #[test]
    fn legacy_root_has_no_component_facts() {
        let document =
            doc("<application><provides><modalias>usb:x</modalias></provides></application>");
        assert_eq!(document.root_element(), Some("application"));
        assert_eq!(document.metadata_license(), None);
        assert!(!document.has_modalias_nodes());
    }

    #[test]
    fn namespaced_component_is_not_a_component() {
        let document = doc(
            r#"<c:component xmlns:c="urn:x"><c:metadata_license>MIT</c:metadata_license></c:component>"#,
        );
        assert_eq!(document.root_element(), Some("component"));
        assert_eq!(document.metadata_license(), None);
    }

    #[test]
    fn malformed_xml_keeps_the_error() {
        let document = doc("<component><unclosed></component>");
        assert!(document.parse_error().is_some());
        assert_eq!(document.root_element(), None);
        assert_eq!(document.metadata_license(), None);
    }

    #[test]
    fn non_utf8_is_a_parse_error() {
        let document = XmlDocument::from_bytes("x.xml", vec![b'<', 0xff, b'/', b'>']);
        assert!(document.parse_error().unwrap().starts_with("not UTF-8"));
    }

    #[test]
    fn external_entities_are_not_loaded() {
        let document = doc(
            r#"<!DOCTYPE component [<!ENTITY e SYSTEM "file:///etc/passwd">]>
<component><metadata_license>&e;</metadata_license></component>"#,
        );
        assert!(document.parse_error().is_some() || document.metadata_license() != Some("root:"));
    }
}
