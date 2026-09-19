/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG text-node whitespace trimming, ported from usvg's `trim_text`/`trim_text_nodes`.
//!
//! The single public entry point is [`trim_text_tree`], which collapses whitespace
//! across every text node under a `<text>` element and returns the result keyed by
//! opaque node identity ([`TrimmedTexts`]).

use std::collections::HashMap;

use html5ever::{LocalName, ns};
use layout_api::{LayoutElement, LayoutNode};
use script::layout_dom::{ServoLayoutElement, ServoLayoutNode};
use style::dom::{NodeInfo, OpaqueNode};

/// SVG whitespace handling mode, driven by the `xml:space` attribute.
#[derive(Clone, Copy, PartialEq, Eq)]
enum XmlSpace {
    Default,
    Preserve,
}

fn xml_space(element: &ServoLayoutElement<'_>) -> Option<XmlSpace> {
    match element.attribute_as_str(&ns!(xml), &LocalName::from("space")) {
        Some("preserve") => Some(XmlSpace::Preserve),
        Some(_) => Some(XmlSpace::Default),
        None => None,
    }
}

/// Collapses whitespace in a single text node per the SVG whitespace spec:
/// line breaks and tabs become spaces, and (in the default mode) runs of spaces
/// collapse to a single space. Ported from usvg's `trim_text`.
fn trim_text(text: &str, space: XmlSpace) -> String {
    let mut s = String::with_capacity(text.len());
    let mut prev = '0';
    for c in text.chars() {
        let c = match c {
            '\r' | '\n' | '\t' => ' ',
            _ => c,
        };
        if space == XmlSpace::Default && c == ' ' && c == prev {
            continue;
        }
        prev = c;
        s.push(c);
    }
    s
}

/// The fully whitespace-trimmed text content for each text node, keyed by the
/// node's opaque identity. Positions (`x`/`y`/`dx`/`dy`/`rotate`) and span
/// building are all resolved against this trimmed text.
pub(crate) type TrimmedTexts = HashMap<OpaqueNode, String>;

/// A text node captured during the whitespace-trimming walk.
struct RawTextNode<'a> {
    node: ServoLayoutNode<'a>,
    depth: usize,
    xml_space: XmlSpace,
    text: String,
}

fn collect_raw_text_nodes<'a>(
    node: ServoLayoutNode<'a>,
    depth: usize,
    inherited: XmlSpace,
    out: &mut Vec<RawTextNode<'a>>,
) {
    for child in node.dom_children() {
        if child.is_text_node() {
            out.push(RawTextNode {
                node: child,
                depth,
                xml_space: inherited,
                text: child.text_content().to_string(),
            });
        } else if let Some(child_element) = child.as_element() {
            let space = xml_space(&child_element).unwrap_or(inherited);
            collect_raw_text_nodes(child, depth + 1, space, out);
        }
    }
}

fn remove_first_space(s: &mut String) {
    debug_assert!(s.starts_with(' '));
    s.remove(0);
}

fn remove_last_space(s: &mut String) {
    debug_assert!(s.ends_with(' '));
    s.pop();
}

/// Removes leading/trailing spaces and collapses spaces at the boundaries
/// between adjacent text nodes, ported from usvg's `trim_text_nodes`.
fn boundary_trim(nodes: &mut [RawTextNode]) {
    let len = nodes.len();
    if len == 0 {
        return;
    }
    if len == 1 {
        if nodes[0].xml_space == XmlSpace::Default {
            nodes[0].text = nodes[0].text.trim_matches(' ').to_string();
        }
        return;
    }

    let mut i = 0;
    while i < len - 1 {
        let idx2 = i + 1;
        let (left, right) = nodes.split_at_mut(idx2);
        let node1 = &mut left[i];
        let node2 = &mut right[0];

        let xmlspace1 = node1.xml_space;
        let xmlspace2 = node2.xml_space;
        let depth1 = node1.depth;
        let depth2 = node2.depth;

        let c1 = node1.text.as_bytes().first().copied();
        let c2 = node1.text.as_bytes().last().copied();
        let c3 = node2.text.as_bytes().first().copied();
        let c4 = node2.text.as_bytes().last().copied();

        if depth1 < depth2 {
            if c3 == Some(b' ') && xmlspace2 == XmlSpace::Default {
                remove_first_space(&mut node2.text);
            }
        } else if c2 == Some(b' ') && c2 == c3 {
            if xmlspace1 == XmlSpace::Default && xmlspace2 == XmlSpace::Default {
                remove_last_space(&mut node1.text);
            } else if xmlspace1 == XmlSpace::Preserve && xmlspace2 == XmlSpace::Default {
                remove_first_space(&mut node2.text);
            }
        }

        let is_first = i == 0;
        let is_last = i == len - 1;

        if is_first && c1 == Some(b' ') && xmlspace1 == XmlSpace::Default && !node1.text.is_empty()
        {
            remove_first_space(&mut node1.text);
        } else if is_last &&
            c4 == Some(b' ') &&
            !node2.text.is_empty() &&
            xmlspace2 == XmlSpace::Default
        {
            remove_last_space(&mut node2.text);
        }

        if is_last &&
            c2 == Some(b' ') &&
            !node1.text.is_empty() &&
            node2.text.is_empty() &&
            node1.text.ends_with(' ')
        {
            remove_last_space(&mut node1.text);
        }

        i += 1;
    }
}

/// Collects the whitespace-trimmed text for every text node under a `<text>`
/// element, keyed by opaque node identity.
pub(crate) fn trim_text_tree(root: ServoLayoutNode<'_>) -> TrimmedTexts {
    let inherited = root
        .as_element()
        .and_then(|e| xml_space(&e))
        .unwrap_or(XmlSpace::Default);
    let mut nodes = Vec::new();
    collect_raw_text_nodes(root, 0, inherited, &mut nodes);
    for n in &mut nodes {
        n.text = trim_text(&n.text, n.xml_space);
    }
    boundary_trim(&mut nodes);
    nodes
        .into_iter()
        .map(|n| (n.node.opaque(), n.text))
        .collect()
}
