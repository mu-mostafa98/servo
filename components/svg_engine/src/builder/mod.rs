/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG node construction (Factory Method pattern).
//!
//! The central abstraction is the [`Build`] trait — every SVG type that can
//! be constructed from DOM attributes and/or computed style implements it.
//! The caller passes a single [`SvgBuildInput`] bundle and receives a
//! fully-constructed value.
//!
//! # Architecture
//!
//! ```text
//! SvgRenderNode::build(input)
//!   ├── SvgTag::build(input)         → Container or Shape
//!   │     └── Shape::build(input)    → dispatches by element_name
//!   │           └── Rectangle::build, Circle::build, …
//!   └── NodeStyle::build(input)      → fill, stroke, transforms
//!         ├── FillParams::from_computed_values (internal)
//!         ├── StrokeParams::from_computed_values (internal)
//!         └── Vec<TransformOp>::build          (in style/transform_ops)
//! ```

use style::properties::ComputedValues;

use crate::error::SvgResult;
use crate::render_tree::{Container, SvgRenderNode, SvgTag};
use crate::shapes::Shape;
use crate::style::FromCssAttrs;

// ======================= Build Trait =======================

/// Factory Method trait — every SVG type that can be constructed from DOM
/// attributes and/or computed style implements this.
///
/// Returns [`SvgResult`] so that construction failures carry a reason
/// (missing attribute, parse error, unimplemented feature).
pub trait Build: Sized {
    fn build(input: &SvgBuildInput) -> SvgResult<Self>;
}

// ======================= Build Input =======================

/// Bundle of all data sources needed to construct an SVG node.
///
/// The caller (typically `components/layout/replaced.rs`) constructs one
/// from the current DOM element and passes it by reference — each
/// [`Build`] impl reads only the fields it needs.
pub struct SvgBuildInput<'a> {
    /// Element tag name, e.g. `"rect"`, `"path"`, `"g"`.
    pub element_name: &'a str,
    /// Attribute accessor — given an attribute name, returns its string value.
    /// This is the *only* bridge between the SVG engine and the DOM.
    pub get_attr: &'a dyn Fn(&str) -> Option<String>,
    /// Servo computed style, if available. When `Some`, the engine uses
    /// the fully-resolved style cascade. When `None`, it falls back to
    /// parsing the inline `style` attribute via `get_attr("style")`.
    pub computed_values: Option<&'a ComputedValues>,
}

// ======================= SvgTag Construction =======================

impl Build for SvgTag {
    fn build(input: &SvgBuildInput) -> SvgResult<Self> {
        match input.element_name {
            "svg" => Ok(SvgTag::Container(Container::Svg)),
            "g" => Ok(SvgTag::Container(Container::Group)),
            _ => Shape::build(input).map(SvgTag::Shape),
        }
    }
}

// ======================= SvgRenderNode Construction =======================

impl Build for SvgRenderNode {
    fn build(input: &SvgBuildInput) -> SvgResult<Self> {
        let tag = SvgTag::build(input)?;
        let style = crate::style::NodeStyle::build(input)?;
        Ok(SvgRenderNode {
            id: None,          // caller sets this
            tag,
            style,
            children: vec![],  // caller populates via recursive walk
        })
    }
}

// ======================= Legacy Convenience Wrappers =======================

/// Parse an SVG element name and attribute accessor into a [`SvgTag`].
///
/// Prefer [`SvgTag::build`](Build::build) directly for new code.
pub fn extract_tag(name: &str, get_attr: &dyn Fn(&str) -> Option<String>) -> Option<SvgTag> {
    let input = SvgBuildInput {
        element_name: name,
        get_attr,
        computed_values: None,
    };
    SvgTag::build(&input).ok()
}

/// Convenience wrapper for external callers.
///
/// Prefer [`NodeStyle::build`](Build::build) directly for new code.
pub fn extract_node_style(computed_values: &ComputedValues) -> crate::style::NodeStyle {
    crate::style::NodeStyle::build(&SvgBuildInput {
        element_name: "",
        get_attr: &|_| None,
        computed_values: Some(computed_values),
    })
    .unwrap_or_default()
}

/// Parse a CSS `style` attribute string into a [`NodeStyle`].
///
/// Prefer `NodeStyle::from_css_attrs` directly for new code.
pub fn extract_node_style_from_css(style_str: &str) -> crate::style::NodeStyle {
    crate::style::NodeStyle::from_css_attrs(style_str).unwrap_or_default()
}
