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

use std::collections::HashMap;

use webrender_api::units::{LayoutPoint, LayoutRect, LayoutSize};
use webrender_api::{
    ColorF, ComponentTransferFuncType, FilterData, FilterOp, FilterOpGraphNode,
    FilterOpGraphPictureBufferId, FilterOpGraphPictureReference,
};

use crate::model::document::{
    BlendMode, ComponentTransfer, ComponentTransferFunc, Defs, FeCompositeKind, FilterPrimitive,
    MorphologyOperator,
};
use crate::model::element::{SvgNode, SvgTag};

/// A resolved filter: the WebRender graph ops plus the parallel `FilterData`
/// list consumed by `SVGFEComponentTransfer` nodes (one entry per component
/// transfer op, in emission order).
pub(crate) struct FilterOps {
    pub ops: Vec<FilterOp>,
    pub datas: Vec<FilterData>,
}

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

/// A graph node with two inputs (feComposite / feBlend / feDisplacementMap).
fn two_input_node(input1: i16, input2: i16, subregion: LayoutRect) -> FilterOpGraphNode {
    FilterOpGraphNode {
        linear: false,
        input: FilterOpGraphPictureReference {
            buffer_id: FilterOpGraphPictureBufferId::BufferId(input1),
        },
        input2: FilterOpGraphPictureReference {
            buffer_id: FilterOpGraphPictureBufferId::BufferId(input2),
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

/// Resolve an `in`/`in2` filter input name to a buffer index in the op list.
///
/// * `None` (attribute absent) → `fallback` (the previous result for `in1`,
///   `SourceGraphic` for `in2`).
/// * `SourceGraphic` → 0, `SourceAlpha` → 1 (the two nodes emitted upfront).
/// * `BackgroundImage`/`BackgroundAlpha`/`FillPaint`/`StrokePaint` are not
///   produced by this renderer, so they degrade to `SourceGraphic`.
/// * Any other name → a lookup in `result_map`; an unknown result name is a
///   broken reference, so it also degrades to `fallback`.
fn resolve_input(
    name: &Option<String>,
    result_map: &HashMap<String, i16>,
    fallback: i16,
) -> i16 {
    match name.as_deref() {
        None => fallback,
        Some("SourceGraphic") => 0,
        Some("SourceAlpha") => 1,
        Some("BackgroundImage" | "BackgroundAlpha" | "FillPaint" | "StrokePaint") => {
            log::debug!("filter input {:?} unsupported; falling back to SourceGraphic", name);
            0
        },
        Some(s) => result_map.get(s).copied().unwrap_or(fallback),
    }
}

/// Convert a single channel transfer function into WebRender's
/// (`func_type`, `values`) representation.
fn transfer_func_to_wr(func: &ComponentTransferFunc) -> (ComponentTransferFuncType, Vec<f32>) {
    match func {
        ComponentTransferFunc::Identity => (ComponentTransferFuncType::Identity, Vec::new()),
        ComponentTransferFunc::Table(v) => (ComponentTransferFuncType::Table, v.clone()),
        ComponentTransferFunc::Discrete(v) => (ComponentTransferFuncType::Discrete, v.clone()),
        ComponentTransferFunc::Linear { slope, intercept } => {
            (ComponentTransferFuncType::Linear, vec![*slope, *intercept])
        },
        ComponentTransferFunc::Gamma {
            amplitude,
            exponent,
            offset,
        } => (ComponentTransferFuncType::Gamma, vec![*amplitude, *exponent, *offset]),
    }
}

/// Build the WebRender `FilterData` for a `feComponentTransfer` op.
fn component_transfer_data(ct: &ComponentTransfer) -> FilterData {
    let (func_r_type, r_values) = transfer_func_to_wr(&ct.r);
    let (func_g_type, g_values) = transfer_func_to_wr(&ct.g);
    let (func_b_type, b_values) = transfer_func_to_wr(&ct.b);
    let (func_a_type, a_values) = transfer_func_to_wr(&ct.a);
    FilterData {
        func_r_type,
        r_values,
        func_g_type,
        g_values,
        func_b_type,
        b_values,
        func_a_type,
        a_values,
    }
}

/// The node's axis-aligned bounding box in local coordinates, used to resolve
/// `objectBoundingBox`-relative regions (filter region, clip-path and mask
/// content). Zero for nodes with no geometric extent.
pub(crate) fn node_local_bounds(node: &SvgNode) -> LayoutRect {
    match &node.tag {
        SvgTag::Shape(shape) => shape.local_bounds(),
        SvgTag::Text(span) => {
            let fs = if span.font_size > 0.0 {
                span.font_size
            } else {
                16.0
            };
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

/// If the node references a filter, return the resolved WebRender SVG filter
/// graph (ops + component-transfer data). Returns `None` when no filter is
/// present, the referenced filter definition is missing, or the filter resolves
/// to an empty op list.
///
/// `origin` is the node's `cur_origin`: the translation that maps the node's
/// local coordinate space into the current spatial node's space (where the
/// filter stacking context and the primitives live). The filter region is
/// computed in the node's local space and then translated by `origin` so the
/// subregions line up with where the node is actually drawn.
pub(crate) fn get_filter_ops(
    node: &SvgNode,
    origin: LayoutPoint,
    defs: Defs<'_>,
) -> Option<FilterOps> {
    let filter_def = node
        .style
        .filter
        .as_ref()
        .and_then(|f| f.resolve(&defs.filters))?;
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

    let mut ops = Vec::with_capacity(filter_def.primitives.len() + 2);
    let mut datas: Vec<FilterData> = Vec::new();
    // Node 0: the source graphic, clipped to the filter region — the only way
    // WebRender lets the source graphic into the graph.
    ops.push(FilterOp::SVGFESourceGraphic {
        node: no_input_node(region),
    });
    // Node 1: the source alpha, so `in="SourceAlpha"` resolves to a buffer.
    ops.push(FilterOp::SVGFESourceAlpha {
        node: no_input_node(region),
    });

    // Resolve `in`/`in2`/`result` wiring across primitives: each primitive
    // reads from previously emitted buffers (by name or by last-result), and a
    // `result` name registers the buffer index of its output for later use.
    let mut result_map: HashMap<String, i16> = HashMap::new();
    let mut last_buffer: i16 = 0; // SourceGraphic is the implicit first input.
    for prim in &filter_def.primitives {
        // feMerge is special-cased: it expands to a chain of `feComposite over`
        // (one op per input after the first) rather than exactly one node.
        if let FilterPrimitive::Merge(inputs) = &prim.op {
            let mut acc = match inputs.first() {
                Some(first) => resolve_input(&Some(first.clone()), &result_map, last_buffer),
                None => last_buffer,
            };
            for next in inputs.iter().skip(1) {
                let nxt = resolve_input(&Some(next.clone()), &result_map, last_buffer);
                // Painter's algorithm: each subsequent feMergeNode is composited
                // on TOP of the accumulated result. feComposite `over` treats
                // `in` as the source (top) and `in2` as the destination, so the
                // next node is `in` and the accumulated result is `in2`.
                ops.push(FilterOp::SVGFECompositeOver {
                    node: two_input_node(nxt, acc, region),
                });
                acc = (ops.len() - 1) as i16;
            }
            last_buffer = acc;
            if let Some(name) = &prim.result {
                result_map.insert(name.clone(), acc);
            }
            continue;
        }

        let input = resolve_input(&prim.in1, &result_map, last_buffer);
        let input2 = resolve_input(&prim.in2, &result_map, 0);
        let op = match &prim.op {
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
                        node: two_input_node(input, input2, region),
                        k1: *k1,
                        k2: *k2,
                        k3: *k3,
                        k4: *k4,
                    }
                },
                FeCompositeKind::Over => FilterOp::SVGFECompositeOver {
                    node: two_input_node(input, input2, region),
                },
                FeCompositeKind::In => FilterOp::SVGFECompositeIn {
                    node: two_input_node(input, input2, region),
                },
                FeCompositeKind::Out => FilterOp::SVGFECompositeOut {
                    node: two_input_node(input, input2, region),
                },
                FeCompositeKind::Atop => FilterOp::SVGFECompositeATop {
                    node: two_input_node(input, input2, region),
                },
                FeCompositeKind::Xor => FilterOp::SVGFECompositeXOR {
                    node: two_input_node(input, input2, region),
                },
                FeCompositeKind::Lighter => FilterOp::SVGFECompositeLighter {
                    node: two_input_node(input, input2, region),
                },
            },
            FilterPrimitive::Blend(mode) => {
                let node = two_input_node(input, input2, region);
                match mode {
                    BlendMode::Normal => FilterOp::SVGFEBlendNormal { node },
                    BlendMode::Multiply => FilterOp::SVGFEBlendMultiply { node },
                    BlendMode::Screen => FilterOp::SVGFEBlendScreen { node },
                    BlendMode::Darken => FilterOp::SVGFEBlendDarken { node },
                    BlendMode::Lighten => FilterOp::SVGFEBlendLighten { node },
                    BlendMode::ColorDodge => FilterOp::SVGFEBlendColorDodge { node },
                    BlendMode::ColorBurn => FilterOp::SVGFEBlendColorBurn { node },
                    BlendMode::HardLight => FilterOp::SVGFEBlendHardLight { node },
                    BlendMode::SoftLight => FilterOp::SVGFEBlendSoftLight { node },
                    BlendMode::Difference => FilterOp::SVGFEBlendDifference { node },
                    BlendMode::Exclusion => FilterOp::SVGFEBlendExclusion { node },
                    BlendMode::Hue => FilterOp::SVGFEBlendHue { node },
                    BlendMode::Saturation => FilterOp::SVGFEBlendSaturation { node },
                    BlendMode::Color => FilterOp::SVGFEBlendColor { node },
                    BlendMode::Luminosity => FilterOp::SVGFEBlendLuminosity { node },
                }
            },
            FilterPrimitive::Tile => FilterOp::SVGFETile {
                node: single_input_node(input, region),
            },
            FilterPrimitive::Morphology(operator, radius_x, radius_y) => {
                // Negative radii are clamped to zero; a zero radius is a
                // passthrough (the shader samples only the center texel).
                let radius_x = radius_x.max(0.0);
                let radius_y = radius_y.max(0.0);
                match operator {
                    MorphologyOperator::Dilate => FilterOp::SVGFEMorphologyDilate {
                        node: single_input_node(input, region),
                        radius_x,
                        radius_y,
                    },
                    MorphologyOperator::Erode => FilterOp::SVGFEMorphologyErode {
                        node: single_input_node(input, region),
                        radius_x,
                        radius_y,
                    },
                }
            },
            FilterPrimitive::DisplacementMap(scale, x_channel, y_channel) => {
                FilterOp::SVGFEDisplacementMap {
                    node: two_input_node(input, input2, region),
                    scale: *scale,
                    x_channel_selector: x_channel.to_u32(),
                    y_channel_selector: y_channel.to_u32(),
                }
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
                    node: no_input_node(region),
                    sampling_filter: 0,
                    matrix: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
                }
            },
            FilterPrimitive::ComponentTransfer(ct) => {
                // SVGFEComponentTransfer reads its transfer functions from the
                // parallel `filter_datas` list (indexed in emission order), so
                // push a matching FilterData entry alongside the op.
                datas.push(component_transfer_data(ct));
                FilterOp::SVGFEComponentTransfer {
                    node: single_input_node(input, region),
                }
            },
            FilterPrimitive::Merge(_) => unreachable!("feMerge handled above"),
        };
        ops.push(op);
        // Register this primitive's output buffer for `result`-based wiring.
        let idx = (ops.len() - 1) as i16;
        last_buffer = idx;
        if let Some(name) = &prim.result {
            result_map.insert(name.clone(), idx);
        }
    }

    Some(FilterOps { ops, datas })
}
