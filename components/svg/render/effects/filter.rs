/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG filter resolution — converts `<filter>` primitive references
//! into a WebRender SVG filter graph ([`FilterOp::SVGFE*`]).
//!
//! **Single responsibility:** given a render node and its filter
//! definitions, produce the list of WebRender filter operations as a
//! source-graphic-anchored DAG. No tree walking beyond computing the node's
//! local bounding box (needed to resolve the `objectBoundingBox` filter
//! region), no display list management beyond filter op construction.

use webrender_api::units::{LayoutPoint, LayoutRect, LayoutSize};
use webrender_api::{
    ColorF, FilterOp, FilterOpGraphNode, FilterOpGraphPictureBufferId,
    FilterOpGraphPictureReference,
};

use crate::model::document::{DefRef, FeCompositeKind, FilterPrimitive};
use crate::model::element::{SvgNode, SvgTag};

/// A graph node with a single input referencing the given buffer index.
fn single_input_node(input: i16, subregion: LayoutRect) -> FilterOpGraphNode {
    FilterOpGraphNode {
        linear: false,
        input: FilterOpGraphPictureReference {
            buffer_id: FilterOpGraphPictureBufferId::BufferId(input),
        },
        input2: FilterOpGraphPictureReference {
            buffer_id: FilterOpGraphPictureBufferId::None,
        },
        subregion,
    }
}

/// A graph node with two inputs (feComposite / feBlend). `input2` is the
/// source graphic by default — the common `in2="SourceGraphic"` case.
fn composite_input_node(input1: i16, subregion: LayoutRect) -> FilterOpGraphNode {
    FilterOpGraphNode {
        linear: false,
        input: FilterOpGraphPictureReference {
            buffer_id: FilterOpGraphPictureBufferId::BufferId(input1),
        },
        input2: FilterOpGraphPictureReference {
            buffer_id: FilterOpGraphPictureBufferId::BufferId(0),
        },
        subregion,
    }
}

/// A graph node with no inputs (SourceGraphic / feFlood, which synthesize
/// their content).
fn no_input_node(subregion: LayoutRect) -> FilterOpGraphNode {
    FilterOpGraphNode {
        linear: false,
        input: FilterOpGraphPictureReference {
            buffer_id: FilterOpGraphPictureBufferId::None,
        },
        input2: FilterOpGraphPictureReference {
            buffer_id: FilterOpGraphPictureBufferId::None,
        },
        subregion,
    }
}

/// The node's axis-aligned bounding box in local coordinates, used to resolve
/// the `objectBoundingBox`-relative filter region. Zero for nodes with no
/// geometric extent.
fn node_local_bounds(node: &SvgNode) -> LayoutRect {
    match &node.tag {
        SvgTag::Shape(shape) => shape.local_bounds(),
        SvgTag::Text(span) => {
            let fs = if span.font_size > 0.0 { span.font_size } else { 16.0 };
            LayoutRect::from_origin_and_size(
                LayoutPoint::new(span.origin_x(), span.origin_y() - fs),
                LayoutSize::new(span.total_advance().max(1.0), fs * 1.25),
            )
        },
        SvgTag::Image(img) => LayoutRect::from_origin_and_size(
            LayoutPoint::new(img.x, img.y),
            LayoutSize::new(img.width, img.height),
        ),
        SvgTag::Container(_) => {
            let mut bounds: Option<LayoutRect> = None;
            for child in &node.children {
                let cb = node_local_bounds(child);
                if cb.size().width > 0.0 || cb.size().height > 0.0 {
                    bounds = Some(match bounds {
                        Some(b) => b.union(&cb),
                        None => cb,
                    });
                }
            }
            bounds.unwrap_or(LayoutRect::zero())
        },
    }
}

/// If the node references a filter, return the list of WebRender SVG filter
/// graph ops. Returns `None` when no filter is present, the referenced filter
/// definition is missing, or the filter resolves to an empty op list.
///
/// `origin` is the node's `cur_origin`: the translation that maps the node's
/// local coordinate space into the current spatial node's space (where the
/// filter stacking context and the primitives live). The filter region is
/// computed in the node's local space and then translated by `origin` so the
/// subregions line up with where the node is actually drawn.
pub(crate) fn get_filter_ops(node: &SvgNode, origin: LayoutPoint) -> Option<Vec<FilterOp>> {
    let filter_def = node.style.filter.as_ref().and_then(DefRef::resolved)?;
    if filter_def.primitives.is_empty() {
        return None;
    }

    let bbox = node_local_bounds(node);
    // Filter region: the object bbox expanded by the `<filter>` x/y/width/height
    // (`objectBoundingBox` units — the default `filterUnits`), translated into
    // the current spatial space. A degenerate bbox (empty geometry) falls back
    // to a unit region so WebRender does not drop the graph.
    let region = LayoutRect::from_origin_and_size(
        LayoutPoint::new(
            origin.x + bbox.min.x + filter_def.x * bbox.size().width,
            origin.y + bbox.min.y + filter_def.y * bbox.size().height,
        ),
        LayoutSize::new(
            filter_def.width * bbox.size().width,
            filter_def.height * bbox.size().height,
        ),
    );
    let region = if region.size().width > 0.0 && region.size().height > 0.0 {
        region
    } else {
        LayoutRect::from_origin_and_size(origin, LayoutSize::new(1.0, 1.0))
    };

    let mut ops = Vec::with_capacity(filter_def.primitives.len() + 1);
    // Node 0: the source graphic, clipped to the filter region. This is the
    // only way WebRender lets the source graphic into the graph.
    ops.push(FilterOp::SVGFESourceGraphic {
        node: no_input_node(region),
    });

    // Each subsequent primitive reads the previous node's output (buffer index
    // = its own position in the list minus one).
    for prim in &filter_def.primitives {
        let input = (ops.len() - 1) as i16;
        let op = match prim {
            FilterPrimitive::GaussianBlur(sdx, sdy) => FilterOp::SVGFEGaussianBlur {
                node: single_input_node(input, region),
                std_deviation_x: *sdx,
                std_deviation_y: *sdy,
            },
            FilterPrimitive::DropShadow(dx, dy, sd, r, g, b, a) => FilterOp::SVGFEDropShadow {
                node: single_input_node(input, region),
                color: ColorF::new(*r, *g, *b, *a),
                dx: *dx,
                dy: *dy,
                std_deviation_x: *sd,
                std_deviation_y: *sd,
            },
            FilterPrimitive::ColorMatrix(matrix) => FilterOp::SVGFEColorMatrix {
                node: single_input_node(input, region),
                values: *matrix,
            },
            FilterPrimitive::Saturate(s) => {
                let s = s.clamp(0.0, 10.0);
                let lum_r = 0.213;
                let lum_g = 0.715;
                let lum_b = 0.072;
                FilterOp::SVGFEColorMatrix {
                    node: single_input_node(input, region),
                    values: [
                        lum_r + (1.0 - lum_r) * s,
                        lum_g * (1.0 - s),
                        lum_b * (1.0 - s),
                        0.0,
                        0.0,
                        lum_r * (1.0 - s),
                        lum_g + (1.0 - lum_g) * s,
                        lum_b * (1.0 - s),
                        0.0,
                        0.0,
                        lum_r * (1.0 - s),
                        lum_g * (1.0 - s),
                        lum_b + (1.0 - lum_b) * s,
                        0.0,
                        0.0,
                        0.0,
                        0.0,
                        0.0,
                        1.0,
                        0.0,
                    ],
                }
            },
            FilterPrimitive::LuminanceToAlpha => FilterOp::SVGFEToAlpha {
                node: single_input_node(input, region),
            },
            FilterPrimitive::Offset(dx, dy) => FilterOp::SVGFEOffset {
                node: single_input_node(input, region),
                offset_x: *dx,
                offset_y: *dy,
            },
            FilterPrimitive::Flood(r, g, b, a) => FilterOp::SVGFEFlood {
                node: no_input_node(region),
                color: ColorF::new(*r, *g, *b, *a),
            },
            FilterPrimitive::Composite(composite_kind) => match composite_kind {
                FeCompositeKind::Arithmetic { k1, k2, k3, k4 } => {
                    FilterOp::SVGFECompositeArithmetic {
                        node: composite_input_node(input, region),
                        k1: *k1,
                        k2: *k2,
                        k3: *k3,
                        k4: *k4,
                    }
                },
                FeCompositeKind::Over => FilterOp::SVGFECompositeOver {
                    node: composite_input_node(input, region),
                },
                FeCompositeKind::In => FilterOp::SVGFECompositeIn {
                    node: composite_input_node(input, region),
                },
                FeCompositeKind::Out => FilterOp::SVGFECompositeOut {
                    node: composite_input_node(input, region),
                },
                FeCompositeKind::Atop => FilterOp::SVGFECompositeATop {
                    node: composite_input_node(input, region),
                },
                FeCompositeKind::Xor => FilterOp::SVGFECompositeXOR {
                    node: composite_input_node(input, region),
                },
                FeCompositeKind::Lighter => FilterOp::SVGFECompositeLighter {
                    node: composite_input_node(input, region),
                },
            },
            FilterPrimitive::Tile => FilterOp::SVGFETile {
                node: single_input_node(input, region),
            },
            FilterPrimitive::Image(img_kind) => {
                // feImage renders an external image (or referenced element) as a
                // filter input. The image itself is not yet uploaded to
                // WebRender by the filter path, so the node renders empty until
                // filter image loading is wired up; the op is still emitted as a
                // real graph node rather than a no-op passthrough.
                log::debug!(
                    "feImage ({:?}) emitted as SVGFEImage; image loading not yet wired",
                    img_kind
                );
                // Identity 2x3 affine (scale=1, translate=0).
                FilterOp::SVGFEImage {
                    node: single_input_node(input, region),
                    sampling_filter: 0,
                    matrix: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
                }
            },
        };
        ops.push(op);
    }

    Some(ops)
}
