/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::fmt;

#[derive(Debug)]
pub enum SvgEngineError {
    MissingAttribute(String),
    ParseError(String),
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

pub type SvgResult<T> = std::result::Result<T, SvgEngineError>;
