/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use webrender_api::{
    DisplayListBuilder, ClipChainId, SpatialId,
    BorderSide, BorderStyle,
    BorderDetails, NormalBorder, BorderRadius, ClipMode, ComplexClipRegion,
    units::{LayoutPoint, LayoutRect, LayoutSize, LayoutSideOffsets},
};

use crate::shapes::Rectangle;
use crate::style::*;
use crate::renderer::{Render, make_common_props};

impl Render for Rectangle {
    fn render(
        &self,
        style: &NodeStyle,
        svg_origin: &LayoutPoint,
        spatial_id: SpatialId,
        clip_chain_id: ClipChainId,
        wr: &mut DisplayListBuilder,
    ) {
        let bounds = LayoutRect::from_origin_and_size(
            LayoutPoint::new(svg_origin.x + self.x, svg_origin.y + self.y),
            LayoutSize::new(self.width, self.height),
        );

        let rx = self
            .rx
            .or(self.ry)
            .unwrap_or(0.0)
            .clamp(0.0, self.width / 2.0);
        let ry = self
            .ry
            .or(self.rx)
            .unwrap_or(0.0)
            .clamp(0.0, self.height / 2.0);

        let has_radius = rx > 0.0 || ry > 0.0;
        let radii = if has_radius {
            Some(BorderRadius {
                top_left: LayoutSize::new(rx, ry),
                top_right: LayoutSize::new(rx, ry),
                bottom_left: LayoutSize::new(rx, ry),
                bottom_right: LayoutSize::new(rx, ry),
            })
        } else {
            None
        };

        // ── FILL ──────────────────────────────────────────────────────
        if let Some(fill) = &style.fill {
            if let Some(mut color) = fill.color {
                color.a *= fill.opacity;

                let clip = if let Some(radii) = radii {
                    let clip_id = wr.define_clip_rounded_rect(
                        spatial_id,
                        ComplexClipRegion {
                            rect: bounds,
                            radii,
                            mode: ClipMode::Clip,
                        },
                    );
                    let parent = match clip_chain_id {
                        ClipChainId::INVALID => None,
                        id => Some(id),
                    };
                    wr.define_clip_chain(parent, [clip_id])
                } else {
                    clip_chain_id
                };

                let common = make_common_props(bounds, spatial_id, clip);
                wr.push_rect(&common, bounds, color);
            }
        }

        // ── STROKE ────────────────────────────────────────────────────
        if let Some(stroke) = &style.stroke {
            if let Some(mut color) = stroke.color {
                color.a *= stroke.opacity;
                let widths = LayoutSideOffsets::new_all_same(stroke.width);

                let details = BorderDetails::Normal(NormalBorder {
                    left: BorderSide { color, style: BorderStyle::Solid },
                    right: BorderSide { color, style: BorderStyle::Solid },
                    top: BorderSide { color, style: BorderStyle::Solid },
                    bottom: BorderSide { color, style: BorderStyle::Solid },
                    radius: radii.unwrap_or(BorderRadius {
                        top_left: LayoutSize::zero(),
                        top_right: LayoutSize::zero(),
                        bottom_left: LayoutSize::zero(),
                        bottom_right: LayoutSize::zero(),
                    }),
                    do_aa: true,
                });

                let common = make_common_props(bounds, spatial_id, clip_chain_id);
                wr.push_border(&common, bounds, widths, details);
            }
        }
    }
}
