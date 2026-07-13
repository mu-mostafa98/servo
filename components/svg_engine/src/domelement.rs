/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Generic DOM element abstraction — allows the SVG engine to work with
//! any DOM implementation.
//!
//! This trait is a **Separated Interface** pattern: the engine crate defines
//! the abstraction, and the layout crate provides the concrete implementation
//! that adapts `ServoLayoutNode` / `ServoLayoutElement`. This keeps the
//! engine crate free of layout-DOM dependencies and enables unit testing
//! with mock DOM data.

/// A generic DOM element handle that provides the SVG engine with the
/// minimal DOM access it needs for render tree construction.
///
/// Implementations wrap real DOM elements (e.g., `ServoLayoutElement`)
/// or test doubles.
pub trait DomElement: Clone {
    /// The type used to represent child nodes during iteration.
    type Child: DomElement;

    /// Return the element's local (tag) name, e.g. `"rect"`, `"linearGradient"`.
    fn local_name(&self) -> &str;

    /// Return an iterator over the element's child DOM nodes that are
    /// themselves elements (text nodes and comments are skipped).
    fn element_children(&self) -> Vec<Self::Child>;

    /// Return the value of attribute `name`, or `None` if missing.
    fn get_attr(&self, name: &str) -> Option<String>;

    /// Return the `id` attribute value, if any.
    fn id(&self) -> Option<String> {
        self.get_attr("id")
    }
}
