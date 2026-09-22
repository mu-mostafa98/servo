/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use resvg::usvg;
use style::color::ColorSpace;
use style::properties::ComputedValues;
use style::values::computed::svg::{SVGPaint, SVGPaintKind};

pub(crate) fn build_fill(computed: &ComputedValues) -> Option<usvg::Fill> {
    let inherited = computed.get_inherited_svg();
    let paint = resolve_paint(&inherited.fill, computed)?;

    // TODO(dom-to-usvg): apply `fill-opacity` and `fill-rule`.
    Some(usvg::Fill::new(paint))
}

pub(crate) fn build_stroke(computed: &ComputedValues, _diagonal: f32) -> Option<usvg::Stroke> {
    let inherited = computed.get_inherited_svg();
    let paint = resolve_paint(&inherited.stroke, computed)?;

    // TODO(dom-to-usvg): apply `stroke-width`, `stroke-opacity`, `stroke-linecap`,
    // `stroke-linejoin`, `stroke-miterlimit`, `stroke-dasharray`, and `stroke-dashoffset`.
    Some(usvg::Stroke::new(paint))
}

fn resolve_paint(svg_paint: &SVGPaint, computed: &ComputedValues) -> Option<usvg::Paint> {
    match &svg_paint.kind {
        SVGPaintKind::Color(color) => {
            let current_color = computed.clone_color();
            let absolute = color.resolve_to_absolute(&current_color);
            let srgb = absolute.to_color_space(ColorSpace::Srgb);
            Some(usvg::Paint::Color(usvg::Color::new_rgb(
                (srgb.components.0.clamp(0.0, 1.0) * 255.0).round() as u8,
                (srgb.components.1.clamp(0.0, 1.0) * 255.0).round() as u8,
                (srgb.components.2.clamp(0.0, 1.0) * 255.0).round() as u8,
            )))
        },
        SVGPaintKind::None => None,
        _ => None,
    }
}
