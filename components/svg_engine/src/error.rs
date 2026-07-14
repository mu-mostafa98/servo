/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG engine error types.

use std::fmt;

/// Errors that can occur during SVG attribute extraction and parsing.
#[derive(Debug)]
pub enum SvgEngineError {
    /// A required SVG attribute is missing (e.g. `<rect>` without `width`).
    MissingAttribute(String),
    /// A value could not be parsed into its expected type.
    ParseError(String),
    /// An SVG feature is not yet implemented in this engine.
    UnsupportedFeature(String),
}

impl fmt::Display for SvgEngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SvgEngineError::MissingAttribute(attr) => {
                write!(f, "missing SVG attribute: {attr}")
            },
            SvgEngineError::ParseError(detail) => {
                write!(f, "SVG parse error: {detail}")
            },
            SvgEngineError::UnsupportedFeature(feat) => {
                write!(f, "unsupported SVG feature: {feat}")
            },
        }
    }
}

impl std::error::Error for SvgEngineError {}

/// Convenience alias for `Result<T, SvgEngineError>`.
pub type SvgResult<T> = std::result::Result<T, SvgEngineError>;
