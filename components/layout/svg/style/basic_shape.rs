/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Conversion from Servo's computed CSS `clip-path` basic shape to the SVG
//! engine's unresolved [`BasicShapeClip`] model type.
//!
//! Percentages are preserved as 0..1 fractions and resolved at render time
//! against the element's fill bounding box; lengths are stored as user units.

use servo_svg::style::{BasicShapeClip, BasicShapePoint, FillRule, ShapeRadius};
use servo_svg::units::LengthOrPercent;
use style::values::computed::Length as CssLength;
use style::values::computed::LengthPercentage;
use style::values::computed::basic_shape::{
    BasicShape, ClipPath, PathOrShapeFunction, ShapeRadius as CssShapeRadius,
};
use style::values::generics::position::PositionOrAuto;

use crate::svg::primitives::geometry::{bez_to_path_data, path_data_to_string};

/// Convert a computed `clip-path` into an inline basic-shape clip, if the value
/// is a `<basic-shape>` (a `url(#…)` or `none` value returns `None`).
pub(crate) fn basic_shape_clip_from_computed(clip_path: &ClipPath) -> Option<BasicShapeClip> {
    match clip_path {
        ClipPath::Shape(basic_shape, _geometry_box) => convert_basic_shape(basic_shape),
        _ => None,
    }
}

fn convert_basic_shape(shape: &BasicShape) -> Option<BasicShapeClip> {
    match shape {
        BasicShape::Rect(inset) => {
            let (top, right, bottom, left) = (
                &inset.rect.0,
                &inset.rect.1,
                &inset.rect.2,
                &inset.rect.3,
            );
            let rx = length_or_percent(&inset.round.top_left.0.width.0);
            let ry = length_or_percent(&inset.round.top_left.0.height.0);
            Some(BasicShapeClip::Inset {
                top: length_or_percent(top),
                right: length_or_percent(right),
                bottom: length_or_percent(bottom),
                left: length_or_percent(left),
                rx,
                ry,
            })
        },
        BasicShape::Circle(circle) => {
            let (cx, cy) = match &circle.position {
                PositionOrAuto::Position(pos) => (
                    length_or_percent(&pos.horizontal),
                    length_or_percent(&pos.vertical),
                ),
                PositionOrAuto::Auto => {
                    (LengthOrPercent::percent(0.5), LengthOrPercent::percent(0.5))
                },
            };
            Some(BasicShapeClip::Circle {
                cx,
                cy,
                r: shape_radius(&circle.radius),
            })
        },
        BasicShape::Ellipse(ellipse) => {
            let (cx, cy) = match &ellipse.position {
                PositionOrAuto::Position(pos) => (
                    length_or_percent(&pos.horizontal),
                    length_or_percent(&pos.vertical),
                ),
                PositionOrAuto::Auto => {
                    (LengthOrPercent::percent(0.5), LengthOrPercent::percent(0.5))
                },
            };
            Some(BasicShapeClip::Ellipse {
                cx,
                cy,
                rx: shape_radius(&ellipse.semiaxis_x),
                ry: shape_radius(&ellipse.semiaxis_y),
            })
        },
        BasicShape::Polygon(polygon) => Some(BasicShapeClip::Polygon {
            points: polygon
                .coordinates
                .iter()
                .map(|coord| BasicShapePoint {
                    x: length_or_percent(&coord.0),
                    y: length_or_percent(&coord.1),
                })
                .collect(),
            fill_rule: css_fill_rule(polygon.fill),
        }),
        BasicShape::PathOrShape(func) => match func {
            PathOrShapeFunction::Path(path) => {
                let d = path_data_to_string(&path.path)?;
                let bez = kurbo::BezPath::from_svg(&d).ok()?;
                Some(BasicShapeClip::Path {
                    path: bez_to_path_data(&bez),
                    fill_rule: css_fill_rule(path.fill),
                })
            },
            // CSS `shape()` — the newer functional form; not yet supported.
            PathOrShapeFunction::Shape(_) => None,
        },
    }
}

fn css_fill_rule(rule: style::values::computed::basic_shape::FillRule) -> FillRule {
    match rule {
        style::values::computed::basic_shape::FillRule::Nonzero => FillRule::NonZero,
        style::values::computed::basic_shape::FillRule::Evenodd => FillRule::EvenOdd,
    }
}

/// Convert a computed [`LengthPercentage`] to the unresolved model form:
/// absolute lengths stay lengths, percentages stay fractions. A `calc()` value
/// (length + percentage) cannot be split without the reference box, so its
/// percentage is dropped — resolving against a zero basis keeps only the
/// length component (a rare and unsupported edge case).
fn length_or_percent(lp: &LengthPercentage) -> LengthOrPercent {
    if let Some(l) = lp.to_length() {
        LengthOrPercent::px(l.px())
    } else if let Some(p) = lp.to_percentage() {
        LengthOrPercent::percent(p.0)
    } else {
        LengthOrPercent::px(lp.resolve(CssLength::new(0.0)).px())
    }
}

/// Convert a computed CSS circle/ellipse radius keyword into the model form.
fn shape_radius(radius: &CssShapeRadius) -> ShapeRadius {
    match radius {
        CssShapeRadius::Length(non_negative) => {
            ShapeRadius::Value(length_or_percent(&non_negative.0))
        },
        CssShapeRadius::ClosestSide => ShapeRadius::ClosestSide,
        CssShapeRadius::FarthestSide => ShapeRadius::FarthestSide,
        CssShapeRadius::FarthestCorner | CssShapeRadius::ClosestCorner => ShapeRadius::Corner,
    }
}
