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
    BlendMode, ComponentTransfer, ComponentTransferFunc, Defs, EdgeMode, FeCompositeKind,
    FilterDef, FilterPrimitive, FilterPrimitiveNode, FilterUnits, LightSource, MorphologyOperator,
    PrimitiveUnits, RegionValue, TurbulenceType,
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

/// Convert an SVG `<feColorMatrix>` matrix from its spec row-major layout
/// (4 output rows × 5 input columns, the `a0..a19` order) into WebRender's
/// 5×4 transposed layout (`matrix[input*4 + output]`), which is what
/// [`FilterOp::SVGFEColorMatrix`] expects.
fn transpose_color_matrix(m: &[f32; 20]) -> [f32; 20] {
    let mut t = [0.0f32; 20];
    for row in 0..4 {
        for col in 0..5 {
            t[col * 4 + row] = m[row * 5 + col];
        }
    }
    t
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

/// Compute a single primitive's subregion in the spatial space the filter ops
/// live in (the node's local space translated by `origin`).
///
/// Per spec (Filter Effects §Filter primitive subregion), a `<percentage>` on
/// a primitive coordinate is resolved against the filter region, while a bare
/// `<number>` consults `primitiveUnits`: user units under `userSpaceOnUse`, a
/// bounding-box fraction under `objectBoundingBox`. `None` means the filter
/// region's own edge/size (the default `0% 0% 100% 100%` subregion).
fn primitive_subregion(
    node: &FilterPrimitiveNode,
    filter_def: &FilterDef,
    bbox: LayoutRect,
    region: LayoutRect,
    origin: LayoutPoint,
) -> LayoutRect {
    // Resolve a start coordinate (x/y): `None` → the filter region edge, a
    // percentage → a fraction of the filter region, a number → user units
    // from the filter region edge, or a bbox fraction under objectBoundingBox.
    let start = |value: Option<RegionValue>, region_min: f32, region_size: f32, bbox_min: f32, bbox_size: f32| {
        match value {
            None => region_min,
            Some(RegionValue::Percent(p)) => region_min + p * region_size,
            Some(RegionValue::Number(n)) => match filter_def.primitive_units {
                PrimitiveUnits::UserSpaceOnUse => region_min + n,
                PrimitiveUnits::ObjectBoundingBox => bbox_min + n * bbox_size,
            },
        }
    };
    // Resolve a size (width/height): `None` → the filter region size, a
    // percentage → a fraction of the filter region, a number → user units or
    // a bbox fraction.
    let size = |value: Option<RegionValue>, region_size: f32, bbox_size: f32| {
        match value {
            None => region_size,
            Some(RegionValue::Percent(p)) => p * region_size,
            Some(RegionValue::Number(n)) => match filter_def.primitive_units {
                PrimitiveUnits::UserSpaceOnUse => n,
                PrimitiveUnits::ObjectBoundingBox => n * bbox_size,
            },
        }
    };
    LayoutRect::from_origin_and_size(
        LayoutPoint::new(
            origin.x
                + start(
                    node.x,
                    region.min.x,
                    region.size().width,
                    bbox.min.x,
                    bbox.size().width,
                ),
            origin.y
                + start(
                    node.y,
                    region.min.y,
                    region.size().height,
                    bbox.min.y,
                    bbox.size().height,
                ),
        ),
        LayoutSize::new(
            size(node.width, region.size().width, bbox.size().width),
            size(node.height, region.size().height, bbox.size().height),
        ),
    )
}

/// Resolve the `<filter>` region (`x`/`y`/`width`/`height`) against the
/// referencing element's bounding box into a local-space rect.
///
/// A `<percentage>` is always a fraction of the bounding box. A bare
/// `<number>` consults `filterUnits`: a bounding-box fraction under
/// `objectBoundingBox` (the default), or a user-unit coordinate under
/// `userSpaceOnUse`.
fn filter_region_rect(filter_def: &FilterDef, bbox: LayoutRect) -> LayoutRect {
    // Resolve a start coordinate (x/y).
    let start = |value: RegionValue, bbox_min: f32, bbox_size: f32| -> f32 {
        match value {
            RegionValue::Percent(p) => bbox_min + p * bbox_size,
            RegionValue::Number(n) => match filter_def.filter_units {
                FilterUnits::UserSpaceOnUse => n,
                FilterUnits::ObjectBoundingBox => bbox_min + n * bbox_size,
            },
        }
    };
    // Resolve a size (width/height).
    let size = |value: RegionValue, bbox_size: f32| -> f32 {
        match value {
            RegionValue::Percent(p) => p * bbox_size,
            RegionValue::Number(n) => match filter_def.filter_units {
                FilterUnits::UserSpaceOnUse => n,
                FilterUnits::ObjectBoundingBox => n * bbox_size,
            },
        }
    };
    LayoutRect::from_origin_and_size(
        LayoutPoint::new(
            start(filter_def.x, bbox.min.x, bbox.size().width),
            start(filter_def.y, bbox.min.y, bbox.size().height),
        ),
        LayoutSize::new(
            size(filter_def.width, bbox.size().width),
            size(filter_def.height, bbox.size().height),
        ),
    )
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
    // Filter region in the node's local space: the object bbox expanded by the
    // `<filter>` x/y/width/height (fractions under the default `filterUnits`).
    // A degenerate bbox (empty geometry) falls back to a unit region so
    // WebRender does not drop the graph.
    let region_local = filter_region_rect(filter_def, bbox);
    let region_local = if region_local.size().width > 0.0 && region_local.size().height > 0.0 {
        region_local
    } else {
        LayoutRect::from_origin_and_size(LayoutPoint::zero(), LayoutSize::new(1.0, 1.0))
    };
    // The same region translated into the current spatial space, where the
    // SourceGraphic/SourceAlpha nodes live.
    let region = LayoutRect::from_origin_and_size(
        LayoutPoint::new(
            origin.x + region_local.min.x,
            origin.y + region_local.min.y,
        ),
        region_local.size(),
    );

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
        // Each primitive's subregion (defaults to the filter region; the
        // primitive's own x/y/width/height may narrow it). Shadowed over the
        // filter region so every arm below uses the per-primitive value.
        let region = primitive_subregion(prim, filter_def, bbox, region_local, origin);
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
                values: transpose_color_matrix(matrix),
            },
            FilterPrimitive::Saturate(s) => {
                let s = s.clamp(0.0, 10.0);
                let lum_r = 0.213;
                let lum_g = 0.715;
                let lum_b = 0.072;
                let matrix = [
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
                ];
                FilterOp::SVGFEColorMatrix {
                    node: single_input_node(input, region),
                    values: transpose_color_matrix(&matrix),
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
            FilterPrimitive::ConvolveMatrix(c) => {
                let node = single_input_node(input, region);
                let preserve_alpha = i32::from(c.preserve_alpha);
                match c.edge_mode {
                    EdgeMode::Duplicate => FilterOp::SVGFEConvolveMatrixEdgeModeDuplicate {
                        node,
                        order_x: c.order_x,
                        order_y: c.order_y,
                        kernel: c.kernel,
                        divisor: c.divisor,
                        bias: c.bias,
                        target_x: c.target_x,
                        target_y: c.target_y,
                        kernel_unit_length_x: c.kernel_unit_length_x,
                        kernel_unit_length_y: c.kernel_unit_length_y,
                        preserve_alpha,
                    },
                    EdgeMode::None_ => FilterOp::SVGFEConvolveMatrixEdgeModeNone {
                        node,
                        order_x: c.order_x,
                        order_y: c.order_y,
                        kernel: c.kernel,
                        divisor: c.divisor,
                        bias: c.bias,
                        target_x: c.target_x,
                        target_y: c.target_y,
                        kernel_unit_length_x: c.kernel_unit_length_x,
                        kernel_unit_length_y: c.kernel_unit_length_y,
                        preserve_alpha,
                    },
                    EdgeMode::Wrap => FilterOp::SVGFEConvolveMatrixEdgeModeWrap {
                        node,
                        order_x: c.order_x,
                        order_y: c.order_y,
                        kernel: c.kernel,
                        divisor: c.divisor,
                        bias: c.bias,
                        target_x: c.target_x,
                        target_y: c.target_y,
                        kernel_unit_length_x: c.kernel_unit_length_x,
                        kernel_unit_length_y: c.kernel_unit_length_y,
                        preserve_alpha,
                    },
                }
            },
            FilterPrimitive::Turbulence(t) => {
                let node = no_input_node(region);
                match (&t.type_, t.stitch_tiles) {
                    (TurbulenceType::FractalNoise, false) => {
                        FilterOp::SVGFETurbulenceWithFractalNoiseWithNoStitching {
                            node,
                            base_frequency_x: t.base_frequency_x,
                            base_frequency_y: t.base_frequency_y,
                            num_octaves: t.num_octaves,
                            seed: t.seed,
                        }
                    },
                    (TurbulenceType::FractalNoise, true) => {
                        FilterOp::SVGFETurbulenceWithFractalNoiseWithStitching {
                            node,
                            base_frequency_x: t.base_frequency_x,
                            base_frequency_y: t.base_frequency_y,
                            num_octaves: t.num_octaves,
                            seed: t.seed,
                        }
                    },
                    (TurbulenceType::Turbulence, false) => {
                        FilterOp::SVGFETurbulenceWithTurbulenceNoiseWithNoStitching {
                            node,
                            base_frequency_x: t.base_frequency_x,
                            base_frequency_y: t.base_frequency_y,
                            num_octaves: t.num_octaves,
                            seed: t.seed,
                        }
                    },
                    (TurbulenceType::Turbulence, true) => {
                        FilterOp::SVGFETurbulenceWithTurbulenceNoiseWithStitching {
                            node,
                            base_frequency_x: t.base_frequency_x,
                            base_frequency_y: t.base_frequency_y,
                            num_octaves: t.num_octaves,
                            seed: t.seed,
                        }
                    },
                }
            },
            FilterPrimitive::DiffuseLighting(d) => {
                let node = single_input_node(input, region);
                match &d.light_source {
                    LightSource::Distant { azimuth, elevation } => {
                        FilterOp::SVGFEDiffuseLightingDistant {
                            node,
                            surface_scale: d.surface_scale,
                            diffuse_constant: d.diffuse_constant,
                            kernel_unit_length_x: d.kernel_unit_length_x,
                            kernel_unit_length_y: d.kernel_unit_length_y,
                            azimuth: *azimuth,
                            elevation: *elevation,
                            lighting_color: ColorF::new(
                                d.lighting_color[0],
                                d.lighting_color[1],
                                d.lighting_color[2],
                                d.lighting_color[3],
                            ),
                        }
                    },
                    LightSource::Point { x, y, z } => FilterOp::SVGFEDiffuseLightingPoint {
                        node,
                        surface_scale: d.surface_scale,
                        diffuse_constant: d.diffuse_constant,
                        kernel_unit_length_x: d.kernel_unit_length_x,
                        kernel_unit_length_y: d.kernel_unit_length_y,
                        x: *x,
                        y: *y,
                        z: *z,
                        lighting_color: ColorF::new(
                            d.lighting_color[0],
                            d.lighting_color[1],
                            d.lighting_color[2],
                            d.lighting_color[3],
                        ),
                    },
                    LightSource::Spot {
                        x,
                        y,
                        z,
                        points_at_x,
                        points_at_y,
                        points_at_z,
                        cone_exponent,
                        limiting_cone_angle,
                    } => FilterOp::SVGFEDiffuseLightingSpot {
                        node,
                        surface_scale: d.surface_scale,
                        diffuse_constant: d.diffuse_constant,
                        kernel_unit_length_x: d.kernel_unit_length_x,
                        kernel_unit_length_y: d.kernel_unit_length_y,
                        x: *x,
                        y: *y,
                        z: *z,
                        points_at_x: *points_at_x,
                        points_at_y: *points_at_y,
                        points_at_z: *points_at_z,
                        cone_exponent: *cone_exponent,
                        limiting_cone_angle: *limiting_cone_angle,
                        lighting_color: ColorF::new(
                            d.lighting_color[0],
                            d.lighting_color[1],
                            d.lighting_color[2],
                            d.lighting_color[3],
                        ),
                    },
                }
            },
            FilterPrimitive::SpecularLighting(s) => {
                let node = single_input_node(input, region);
                match &s.light_source {
                    LightSource::Distant { azimuth, elevation } => {
                        FilterOp::SVGFESpecularLightingDistant {
                            node,
                            surface_scale: s.surface_scale,
                            specular_constant: s.specular_constant,
                            specular_exponent: s.specular_exponent,
                            kernel_unit_length_x: s.kernel_unit_length_x,
                            kernel_unit_length_y: s.kernel_unit_length_y,
                            azimuth: *azimuth,
                            elevation: *elevation,
                            lighting_color: ColorF::new(
                                s.lighting_color[0],
                                s.lighting_color[1],
                                s.lighting_color[2],
                                s.lighting_color[3],
                            ),
                        }
                    },
                    LightSource::Point { x, y, z } => FilterOp::SVGFESpecularLightingPoint {
                        node,
                        surface_scale: s.surface_scale,
                        specular_constant: s.specular_constant,
                        specular_exponent: s.specular_exponent,
                        kernel_unit_length_x: s.kernel_unit_length_x,
                        kernel_unit_length_y: s.kernel_unit_length_y,
                        x: *x,
                        y: *y,
                        z: *z,
                        lighting_color: ColorF::new(
                            s.lighting_color[0],
                            s.lighting_color[1],
                            s.lighting_color[2],
                            s.lighting_color[3],
                        ),
                    },
                    LightSource::Spot {
                        x,
                        y,
                        z,
                        points_at_x,
                        points_at_y,
                        points_at_z,
                        cone_exponent,
                        limiting_cone_angle,
                    } => FilterOp::SVGFESpecularLightingSpot {
                        node,
                        surface_scale: s.surface_scale,
                        specular_constant: s.specular_constant,
                        specular_exponent: s.specular_exponent,
                        kernel_unit_length_x: s.kernel_unit_length_x,
                        kernel_unit_length_y: s.kernel_unit_length_y,
                        x: *x,
                        y: *y,
                        z: *z,
                        points_at_x: *points_at_x,
                        points_at_y: *points_at_y,
                        points_at_z: *points_at_z,
                        cone_exponent: *cone_exponent,
                        limiting_cone_angle: *limiting_cone_angle,
                        lighting_color: ColorF::new(
                            s.lighting_color[0],
                            s.lighting_color[1],
                            s.lighting_color[2],
                            s.lighting_color[3],
                        ),
                    },
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
