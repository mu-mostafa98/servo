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

// ======================= Tests =======================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_missing_attribute() {
        let err = SvgEngineError::MissingAttribute("width".to_owned());
        assert_eq!(err.to_string(), "missing SVG attribute: width");
    }

    #[test]
    fn display_parse_error() {
        let err = SvgEngineError::ParseError("invalid number".to_owned());
        assert_eq!(err.to_string(), "SVG parse error: invalid number");
    }

    #[test]
    fn display_unsupported_feature() {
        let err = SvgEngineError::UnsupportedFeature("gradients".to_owned());
        assert_eq!(err.to_string(), "unsupported SVG feature: gradients");
    }

    #[test]
    fn error_impl() {
        let err = SvgEngineError::MissingAttribute("r".to_owned());
        let err_ref: &dyn std::error::Error = &err;
        assert!(!err_ref.to_string().is_empty());
    }

    #[test]
    fn debug_vs_display() {
        let err = SvgEngineError::ParseError("test".to_owned());
        let debug = format!("{err:?}");
        let display = format!("{err}");
        assert_ne!(debug, display);
    }
}
